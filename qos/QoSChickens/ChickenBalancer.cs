using System;
using System.Collections.Generic;
using Mafi;
using Mafi.Core.Buildings.Farms;
using Mafi.Core.Entities;
using Mafi.Core.Input;
using Mafi.Core.Mods;
using Mafi.Core.Products;
using Mafi.Core.Simulation;
using QoSCore;

namespace QoSChickens;

/// <summary>
/// The chicken-distribution policy layer (QOL-MOD-PREP.md section 1,
/// refined design). Once per in-game day (at day end, after that
/// day's births and slaughter culls have run) it evaluates
/// same-species animal farms and issues the game's OWN
/// MoveAnimalsCmd through IInputScheduler — the shipped mutation
/// path, with the shipped 1-by-1 round-robin distribution inside
/// each move. The mod adds scheduling and targeting, never a new
/// movement mechanism.
///
/// Hard guards (prep section 1b), enforced by EXACT SIMULATION of
/// the shipped round-robin before any command is scheduled — the
/// predicted per-farm outcome must satisfy every guard, and the
/// scheduled amount is the simulated amount:
///   * never move into a farm at or above its SlaughterLimit
///     (such a farm must gain zero; no farm may end a move above
///     min(capacity, slaughter limit));
///   * never move into a farm that is not constructed + enabled
///     (the eligible set is exactly constructed + enabled farms);
///   * growth-paused farms are excluded — and because the shipped
///     command's peer set cannot exclude them, a species with a
///     constructed + enabled growth-paused farm is skipped
///     entirely that day (logged), rather than letting a move
///     touch a farm the player told to stay put;
///   * never drain a farm below the configured breeding-nucleus
///     floor;
///   * total animals scheduled per day is capped (config).
///
/// Everything is wrapped so a failure is a log line, never an
/// exception into the sim. Config is re-read from the mod's
/// ModJsonConfig on every pass, so settings changes apply from
/// the next day with no restart.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class ChickenBalancer : IDisposable {

	private readonly ModJsonConfig m_config;
	private readonly IEntitiesManager m_entities;
	private readonly IInputScheduler m_scheduler;
	private readonly ICalendar m_calendar;

	public ChickenBalancer(QoSChickensMod mod, IEntitiesManager entities,
			IInputScheduler scheduler, ICalendar calendar) {
		m_config = mod.JsonConfig;
		m_entities = entities;
		m_scheduler = scheduler;
		m_calendar = calendar;
		m_calendar.NewDayEnd.Add(this, OnNewDayEnd);
		QoSLog.Info("Chickens", "balancer armed (daily pass at day end; " +
			"master switch is the 'enabled' mod setting, default off)");
	}

	public void Dispose() {
		m_calendar.NewDayEnd.Remove(this, OnNewDayEnd);
	}

	private void OnNewDayEnd() {
		try {
			RunPass();
		} catch (Exception ex) {
			QoSLog.Error("Chickens", "daily pass failed (no moves made): " + ex);
		}
	}

	private void RunPass() {
		if (!m_config.GetBool("enabled", false)) {
			QoSLog.Debug("Chickens", "pass skipped: 'enabled' is false");
			return;
		}
		string mode = (m_config.GetString("mode", "equalise") ?? "equalise").Trim().ToLowerInvariant();
		if (mode == "off") {
			QoSLog.Debug("Chickens", "pass skipped: mode is off");
			return;
		}
		if (mode != "equalise" && mode != "fill_first") {
			QoSLog.Warning("Chickens", "unknown mode '" + mode +
				"' — doing nothing (valid modes: equalise, fill_first, off)");
			return;
		}
		int budget = Math.Max(0, m_config.GetInt("max_animals_moved_per_day", 50));
		if (budget <= 0) {
			QoSLog.Debug("Chickens", "pass skipped: daily move cap is 0");
			return;
		}
		int floor = Math.Max(0, m_config.GetInt("breeding_nucleus_floor", 10));
		bool logMoves = m_config.GetBool("log_moves", true);

		// Group all animal farms by species (Prototype.Animal), in
		// enumeration order — the same order the game materialises
		// its peer arrays from.
		var groups = new Dictionary<VirtualProductProto, List<AnimalFarm>>();
		foreach (AnimalFarm farm in m_entities.GetAllEntitiesOfType<AnimalFarm>()) {
			if (farm.IsDestroyed) {
				continue;
			}
			VirtualProductProto species = farm.Prototype.Animal;
			List<AnimalFarm> list;
			if (!groups.TryGetValue(species, out list)) {
				list = new List<AnimalFarm>();
				groups.Add(species, list);
			}
			list.Add(farm);
		}

		foreach (KeyValuePair<VirtualProductProto, List<AnimalFarm>> group in groups) {
			if (budget <= 0) {
				break;
			}
			int moved = mode == "equalise"
				? BalanceEqualise(group.Value, floor, budget, logMoves)
				: BalanceFillFirst(group.Value, floor, budget, logMoves);
			budget -= moved;
		}
	}

	// ------------------------------------------------------------------
	// Modes
	// ------------------------------------------------------------------

	/// <summary>Converge all eligible farms of one species to within ±1.</summary>
	private int BalanceEqualise(List<AnimalFarm> farms, int floor, int budget, bool logMoves) {
		List<AnimalFarm> eligible;
		if (!TryGetEligible(farms, out eligible)) {
			return 0;
		}
		if (eligible.Count < 2) {
			return 0;
		}
		int total = 0, min = int.MaxValue, max = int.MinValue;
		foreach (AnimalFarm f in eligible) {
			total += f.AnimalsCount;
			if (f.AnimalsCount < min) min = f.AnimalsCount;
			if (f.AnimalsCount > max) max = f.AnimalsCount;
		}
		if (total <= 0 || max - min <= 1) {
			QoSLog.Debug("Chickens", "equalise: already within +/-1 (min " + min + ", max " + max + ")");
			return 0;
		}
		int target = total / eligible.Count;

		// Pull into the emptiest valid receiver (drains every peer
		// evenly, which also trims the fullest farms).
		AnimalFarm receiver = null;
		foreach (AnimalFarm f in eligible) {
			if (f.AnimalsCount < target && f.AnimalsCount < ReceiverCap(f) &&
					(receiver == null || f.AnimalsCount < receiver.AnimalsCount)) {
				receiver = f;
			}
		}
		if (receiver != null) {
			int moved = TryMove(eligible, receiver, true,
				Math.Min(target - receiver.AnimalsCount, ReceiverCap(receiver) - receiver.AnimalsCount),
				floor, budget, logMoves, "equalise");
			if (moved > 0) {
				return moved;
			}
		}

		// Otherwise push out of the fullest farm (fills every peer evenly).
		AnimalFarm source = null;
		foreach (AnimalFarm f in eligible) {
			if (f.AnimalsCount > target && (source == null || f.AnimalsCount > source.AnimalsCount)) {
				source = f;
			}
		}
		if (source != null) {
			int moved = TryMove(eligible, source, false,
				source.AnimalsCount - target, floor, budget, logMoves, "equalise");
			if (moved > 0) {
				return moved;
			}
		}
		QoSLog.Debug("Chickens", "equalise: spread " + min + ".." + max +
			" but every candidate move was blocked by the guards");
		return 0;
	}

	/// <summary>Fill farms in the configured priority order, each to its cap.</summary>
	private int BalanceFillFirst(List<AnimalFarm> farms, int floor, int budget, bool logMoves) {
		List<AnimalFarm> eligible;
		if (!TryGetEligible(farms, out eligible)) {
			return 0;
		}
		string raw = m_config.GetString("fill_first_farm_ids", "") ?? "";
		var targets = new List<AnimalFarm>();
		foreach (string part in raw.Split(',')) {
			int id;
			if (!int.TryParse(part.Trim(), out id)) {
				continue;
			}
			foreach (AnimalFarm f in eligible) {
				if (f.Id.Value == id && !targets.Contains(f)) {
					targets.Add(f);
				}
			}
		}
		if (targets.Count == 0) {
			QoSLog.Warning("Chickens", "fill_first mode but 'fill_first_farm_ids' names no " +
				"eligible farm — doing nothing (ids are entity ids, e.g. from the companion census)");
			return 0;
		}
		foreach (AnimalFarm target in targets) {
			int cap = ReceiverCap(target);
			if (target.AnimalsCount >= cap) {
				continue;
			}
			int moved = TryMove(eligible, target, true, cap - target.AnimalsCount,
				floor, budget, logMoves, "fill-first");
			if (moved > 0) {
				return moved;
			}
		}
		return 0;
	}

	// ------------------------------------------------------------------
	// Eligibility + guards
	// ------------------------------------------------------------------

	/// <summary>
	/// The eligible set: constructed + enabled + not growth-paused.
	/// Returns false (skip this species today) when a constructed +
	/// enabled growth-paused farm exists: the shipped move command's
	/// peer set cannot exclude it, so any move would touch a farm
	/// the player told to stay put.
	/// </summary>
	private bool TryGetEligible(List<AnimalFarm> farms, out List<AnimalFarm> eligible) {
		eligible = new List<AnimalFarm>();
		foreach (AnimalFarm f in farms) {
			if (!f.IsConstructed || !f.IsEnabled) {
				continue;
			}
			if (f.IsGrowthPaused) {
				QoSLog.Debug("Chickens", "species of farm #" + f.Id.Value +
					" skipped: a growth-paused farm is in the shipped command's peer set and cannot be excluded from a move");
				eligible = null;
				return false;
			}
			eligible.Add(f);
		}
		return true;
	}

	/// <summary>Most a farm may hold under the guards: capacity, tightened by its slaughter limit.</summary>
	private static int ReceiverCap(AnimalFarm farm) {
		int cap = farm.Prototype.AnimalsCapacity;
		if (farm.SlaughterLimit.HasValue && farm.SlaughterLimit.Value < cap) {
			cap = farm.SlaughterLimit.Value;
		}
		return cap;
	}

	/// <summary>Check a predicted post-move state against every hard guard.</summary>
	private static bool GuardsSatisfied(List<AnimalFarm> eligible,
			Dictionary<AnimalFarm, int> predicted, int floor) {
		foreach (AnimalFarm f in eligible) {
			int before = f.AnimalsCount;
			int after = predicted[f];
			if (after < before && after < floor) {
				return false; // drained below the breeding-nucleus floor
			}
			if (after > before) {
				int cap = ReceiverCap(f);
				if (before >= cap) {
					return false; // already at/above its slaughter limit — must gain zero
				}
				if (after > cap) {
					return false; // would end above capacity / slaughter limit
				}
			}
		}
		return true;
	}

	// ------------------------------------------------------------------
	// Move execution: simulate exactly, then schedule the simulated move
	// ------------------------------------------------------------------

	/// <summary>
	/// Try to schedule one move (pull into / push out of
	/// <paramref name="farm"/>), shrinking the amount until the
	/// simulated outcome satisfies the guards. Returns the number
	/// of animals scheduled (0 = nothing safe to do).
	/// </summary>
	private int TryMove(List<AnimalFarm> eligible, AnimalFarm farm, bool pull,
			int desired, int floor, int budget, bool logMoves, string modeLabel) {
		desired = Math.Min(desired, budget);
		while (desired > 0) {
			int moved;
			Dictionary<AnimalFarm, int> predicted = pull
				? SimulatePull(eligible, farm, desired, out moved)
				: SimulatePush(eligible, farm, desired, out moved);
			if (moved <= 0) {
				return 0;
			}
			if (GuardsSatisfied(eligible, predicted, floor)) {
				int signed = pull ? moved : -moved;
				m_scheduler.ScheduleInputCmd(new MoveAnimalsCmd(farm.Id, signed));
				int min = int.MaxValue, max = int.MinValue;
				foreach (KeyValuePair<AnimalFarm, int> kv in predicted) {
					if (kv.Value < min) min = kv.Value;
					if (kv.Value > max) max = kv.Value;
				}
				// Unified activity record (Addendum XVI): every
				// scheduled move, whether or not log_moves is on.
				QoSEventLog.Record("QoSChickens", QoSChickensMod.Version, "command", "MoveAnimalsCmd",
					modeLabel + ": moved " + (pull ? "+" : "-") + moved + " into farm #" +
					farm.Id.Value + " (" + farm.Prototype.Animal.Id.Value +
					"); predicted spread " + min + ".." + max + " across " +
					eligible.Count + " farms",
					new Dictionary<string, object> {
						{ "farmId", farm.Id.Value },
						{ "amount", signed },
						{ "species", farm.Prototype.Animal.Id.Value },
						{ "mode", modeLabel },
						{ "predictedMin", min },
						{ "predictedMax", max },
						{ "eligibleFarms", eligible.Count },
					});
				if (logMoves) {
					QoSLog.Info("Chickens", modeLabel + ": scheduled MoveAnimalsCmd farm #" +
						farm.Id.Value + " " + (pull ? "+" : "-") + moved +
						" (" + farm.Prototype.Animal.Id.Value + "; predicted spread " +
						min + ".." + max + " across " + eligible.Count + " farms)");
				}
				return moved;
			}
			desired = moved / 2;
		}
		return 0;
	}

	/// <summary>
	/// Exact simulation of the shipped pull: MoveAnimalsCmd(target,
	/// +n) loops the peer array taking 1 animal per peer per pass,
	/// clamped by the target's free capacity, until n are moved or a
	/// full pass moves nothing.
	/// </summary>
	private static Dictionary<AnimalFarm, int> SimulatePull(List<AnimalFarm> ordered,
			AnimalFarm target, int n, out int moved) {
		var pred = new Dictionary<AnimalFarm, int>();
		foreach (AnimalFarm f in ordered) {
			pred[f] = f.AnimalsCount;
		}
		moved = 0;
		int capacity = target.Prototype.AnimalsCapacity;
		bool progress = true;
		while (moved < n && progress) {
			progress = false;
			foreach (AnimalFarm peer in ordered) {
				if (moved >= n) {
					break;
				}
				if (ReferenceEquals(peer, target)) {
					continue;
				}
				if (pred[peer] > 0 && pred[target] < capacity) {
					pred[peer]--;
					pred[target]++;
					moved++;
					progress = true;
				}
			}
		}
		return pred;
	}

	/// <summary>
	/// Exact simulation of the shipped push: MoveAnimalsCmd(source,
	/// -n) loops the peer array giving 1 animal per peer per pass,
	/// clamped by each peer's free capacity, until n are moved or a
	/// full pass moves nothing.
	/// </summary>
	private static Dictionary<AnimalFarm, int> SimulatePush(List<AnimalFarm> ordered,
			AnimalFarm source, int n, out int moved) {
		var pred = new Dictionary<AnimalFarm, int>();
		foreach (AnimalFarm f in ordered) {
			pred[f] = f.AnimalsCount;
		}
		moved = 0;
		bool progress = true;
		while (moved < n && progress) {
			progress = false;
			foreach (AnimalFarm peer in ordered) {
				if (moved >= n) {
					break;
				}
				if (ReferenceEquals(peer, source)) {
					continue;
				}
				if (pred[source] > 0 && pred[peer] < peer.Prototype.AnimalsCapacity) {
					pred[peer]++;
					pred[source]--;
					moved++;
					progress = true;
				}
			}
		}
		return pred;
	}
}
