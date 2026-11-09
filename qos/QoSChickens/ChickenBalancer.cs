using System;
using System.Collections.Generic;
using Mafi;
using Mafi.Core.Buildings.Farms;
using Mafi.Core.Entities;
using Mafi.Core.Input;
using Mafi.Core.Mods;
using Mafi.Core.Products;
using Mafi.Core.Simulation;
using Mafi.Serialization;
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
// [ManuallyWrittenSerialization] + the hand-written Serialize /
// Deserialize pair below are REQUIRED for save safety — see the
// serialization block comment after the fields. Do not remove.
[ManuallyWrittenSerialization]
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class ChickenBalancer : IDisposable {

	private readonly QoSChickensMod m_mod;
	private readonly IEntitiesManager m_entities;
	private readonly IInputScheduler m_scheduler;
	private readonly ICalendar m_calendar;

	// ---- Save/load serialization (hand-written, game-sanctioned) ----
	//
	// WHY THIS EXISTS: the calendar's NewDayEnd event is a SAVED event
	// (Mafi.EventBase). Subscribing with Add(this, ...) stores a
	// CallbackSaveData whose Serialize() generic-writes THE OWNER —
	// this instance — into every save file. A concrete class is
	// generic-serializer-eligible only if it exposes public static
	// Serialize(T, BlobWriter) / Deserialize(BlobReader) methods:
	// SerializerGenerator.TryGetSerializeMethod probes for exactly
	// those (Public|Static), and BlobWriter's DirectCallSerializer
	// invokes them. Shipped subscribers (e.g. StatsManager) have the
	// pair because the game's serializer generator emitted it for
	// their [GenerateSerializer] classes. Without the pair, saving
	// throws "Failed to create generic serializer for
	// 'ChickenBalancer'" and THE GAME CANNOT SAVE AT ALL — that was
	// the suite 0.5.0 bug (fixed in 0.1.2). The
	// [ManuallyWrittenSerialization] attribute on the class is the
	// game's own marker for hand-written pairs: the generator's
	// validation pass skips marked types, and REQUIRES the mark on
	// any type carrying the methods without [GenerateSerializer].
	//
	// The pair mirrors the generator's output shape (StatsManager):
	// the only persistent state written is the calendar reference —
	// a global dependency written as a generic ref, exactly as
	// shipped code does. Every other dependency is re-resolved from
	// the game's DependencyResolver at load-finalize via
	// RegisterResolvedMember and is never serialized. This service
	// holds no other state (config is re-read from the mod instance
	// on every pass), so nothing else needs restoring. On load the
	// instance is materialised uninitialised (no constructor runs);
	// DeserializeData below restores exactly the fields above.
	private static readonly Action<object, BlobWriter> s_serializeDataDelayedAction =
		(obj, writer) => ((ChickenBalancer)obj).SerializeData(writer);

	private static readonly Action<object, BlobReader> s_deserializeDataDelayedAction =
		(obj, reader) => ((ChickenBalancer)obj).DeserializeData(reader);

	public static void Serialize(ChickenBalancer value, BlobWriter writer) {
		if (writer.TryStartClassSerialization(value)) {
			writer.EnqueueDataSerialization(value, s_serializeDataDelayedAction);
		}
	}

	public static ChickenBalancer Deserialize(BlobReader reader) {
		ChickenBalancer result;
		if (reader.TryStartClassDeserialization<ChickenBalancer>(out result)) {
			reader.EnqueueDataDeserialization(result, s_deserializeDataDelayedAction);
		}
		return result;
	}

	private void SerializeData(BlobWriter writer) {
		writer.WriteGeneric(m_calendar);
	}

	private void DeserializeData(BlobReader reader) {
		reader.SetField(this, "m_calendar", reader.ReadGenericAs<ICalendar>());
		reader.RegisterResolvedMember(this, "m_mod", typeof(QoSChickensMod), isField: true);
		reader.RegisterResolvedMember(this, "m_entities", typeof(IEntitiesManager), isField: true);
		reader.RegisterResolvedMember(this, "m_scheduler", typeof(IInputScheduler), isField: true);
	}

	public ChickenBalancer(QoSChickensMod mod, IEntitiesManager entities,
			IInputScheduler scheduler, ICalendar calendar) {
		m_mod = mod;
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
		if (!m_mod.JsonConfig.GetBool("enabled", false)) {
			QoSLog.Debug("Chickens", "pass skipped: 'enabled' is false");
			return;
		}
		string modeText;
		ChickenGuardMath.BalanceMode mode = ChickenGuardMath.ParseMode(
			m_mod.JsonConfig.GetString("mode", "equalise"), out modeText);
		if (mode == ChickenGuardMath.BalanceMode.Off) {
			QoSLog.Debug("Chickens", "pass skipped: mode is off");
			return;
		}
		if (mode == ChickenGuardMath.BalanceMode.Unknown) {
			QoSLog.Warning("Chickens", "unknown mode '" + modeText +
				"' — doing nothing (valid modes: equalise, fill_first, off)");
			return;
		}
		int budget = ChickenGuardMath.ClampBudget(
			m_mod.JsonConfig.GetInt("max_animals_moved_per_day", 50));
		if (budget <= 0) {
			QoSLog.Debug("Chickens", "pass skipped: daily move cap is 0");
			return;
		}
		int floor = ChickenGuardMath.ClampFloor(
			m_mod.JsonConfig.GetInt("breeding_nucleus_floor", 10));
		bool logMoves = m_mod.JsonConfig.GetBool("log_moves", true);

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
			int moved = mode == ChickenGuardMath.BalanceMode.Equalise
				? BalanceEqualise(group.Value, floor, budget, logMoves)
				: BalanceFillFirst(group.Value, floor, budget, logMoves);
			budget -= moved;
		}
	}

	// ------------------------------------------------------------------
	// Modes. The decisions live in ChickenGuardMath (pure functions
	// over plain values, unit-tested off-game — QUALITY-AUDIT
	// item 5); these wrappers translate between game objects and
	// the parallel arrays it works on, and own the logging and
	// scheduling side effects.
	// ------------------------------------------------------------------

	/// <summary>Converge all eligible farms of one species to within ±1.</summary>
	private int BalanceEqualise(List<AnimalFarm> farms, int floor, int budget, bool logMoves) {
		List<AnimalFarm> eligible;
		if (!TryGetEligible(farms, out eligible)) {
			return 0;
		}
		int[] counts = CountsOf(eligible);
		bool pull;
		int focus, moved;
		int[] predicted;
		ChickenGuardMath.BalanceOutcome outcome = ChickenGuardMath.DecideEqualise(
			counts, CapacitiesOf(eligible), ReceiverCapsOf(eligible), floor, budget,
			out pull, out focus, out moved, out predicted);
		if (outcome == ChickenGuardMath.BalanceOutcome.Move) {
			ScheduleMove(eligible, eligible[focus], pull, moved, predicted, logMoves, "equalise");
			return moved;
		}
		if (outcome == ChickenGuardMath.BalanceOutcome.Balanced) {
			int min = int.MaxValue, max = int.MinValue;
			foreach (int c in counts) {
				if (c < min) min = c;
				if (c > max) max = c;
			}
			QoSLog.Debug("Chickens", "equalise: already within +/-1 (min " + min + ", max " + max + ")");
			return 0;
		}
		if (outcome == ChickenGuardMath.BalanceOutcome.Blocked) {
			int min = int.MaxValue, max = int.MinValue;
			foreach (AnimalFarm f in eligible) {
				if (f.AnimalsCount < min) min = f.AnimalsCount;
				if (f.AnimalsCount > max) max = f.AnimalsCount;
			}
			QoSLog.Debug("Chickens", "equalise: spread " + min + ".." + max +
				" but every candidate move was blocked by the guards");
			return 0;
		}
		return 0; // NoOp: fewer than two eligible farms — silent, as shipped.
	}

	/// <summary>Fill farms in the configured priority order, each to its cap.</summary>
	private int BalanceFillFirst(List<AnimalFarm> farms, int floor, int budget, bool logMoves) {
		List<AnimalFarm> eligible;
		if (!TryGetEligible(farms, out eligible)) {
			return 0;
		}
		string raw = m_mod.JsonConfig.GetString("fill_first_farm_ids", "") ?? "";
		int focus, moved;
		int[] predicted;
		bool targetsFound;
		bool ok = ChickenGuardMath.DecideFillFirst(
			CountsOf(eligible), CapacitiesOf(eligible), ReceiverCapsOf(eligible),
			IdsOf(eligible), raw, floor, budget,
			out focus, out moved, out predicted, out targetsFound);
		if (!targetsFound) {
			QoSLog.Warning("Chickens", "fill_first mode but 'fill_first_farm_ids' names no " +
				"eligible farm — doing nothing (ids are entity ids, e.g. from the companion census)");
			return 0;
		}
		if (!ok) {
			return 0;
		}
		ScheduleMove(eligible, eligible[focus], true, moved, predicted, logMoves, "fill-first");
		return moved;
	}

	// ------------------------------------------------------------------
	// Eligibility + game-object/array translation
	// ------------------------------------------------------------------

	/// <summary>
	/// The eligible set: constructed + enabled + not growth-paused.
	/// Returns false (skip this species today) when a constructed +
	/// enabled growth-paused farm exists: the shipped move command's
	/// peer set cannot exclude it, so any move would touch a farm
	/// the player told to stay put. The rule itself is
	/// ChickenGuardMath.EvaluateEligibility; this wrapper only
	/// reads the flags and carries the log line.
	/// </summary>
	private bool TryGetEligible(List<AnimalFarm> farms, out List<AnimalFarm> eligible) {
		int n = farms.Count;
		var constructed = new bool[n];
		var enabled = new bool[n];
		var growthPaused = new bool[n];
		for (int i = 0; i < n; i++) {
			constructed[i] = farms[i].IsConstructed;
			enabled[i] = farms[i].IsEnabled;
			growthPaused[i] = farms[i].IsGrowthPaused;
		}
		int[] indices;
		int pausedIndex;
		if (!ChickenGuardMath.EvaluateEligibility(constructed, enabled, growthPaused,
				out indices, out pausedIndex)) {
			QoSLog.Debug("Chickens", "species of farm #" + farms[pausedIndex].Id.Value +
				" skipped: a growth-paused farm is in the shipped command's peer set and cannot be excluded from a move");
			eligible = null;
			return false;
		}
		eligible = new List<AnimalFarm>();
		foreach (int i in indices) {
			eligible.Add(farms[i]);
		}
		return true;
	}

	private static int[] CountsOf(List<AnimalFarm> farms) {
		var result = new int[farms.Count];
		for (int i = 0; i < farms.Count; i++) {
			result[i] = farms[i].AnimalsCount;
		}
		return result;
	}

	private static int[] CapacitiesOf(List<AnimalFarm> farms) {
		var result = new int[farms.Count];
		for (int i = 0; i < farms.Count; i++) {
			result[i] = farms[i].Prototype.AnimalsCapacity;
		}
		return result;
	}

	private static int[] ReceiverCapsOf(List<AnimalFarm> farms) {
		var result = new int[farms.Count];
		for (int i = 0; i < farms.Count; i++) {
			result[i] = ChickenGuardMath.ReceiverCap(
				farms[i].Prototype.AnimalsCapacity, farms[i].SlaughterLimit);
		}
		return result;
	}

	private static int[] IdsOf(List<AnimalFarm> farms) {
		var result = new int[farms.Count];
		for (int i = 0; i < farms.Count; i++) {
			result[i] = farms[i].Id.Value;
		}
		return result;
	}

	// ------------------------------------------------------------------
	// Move execution: schedule exactly the move ChickenGuardMath
	// simulated and the guards approved — same amount, same farm.
	// ------------------------------------------------------------------

	private void ScheduleMove(List<AnimalFarm> eligible, AnimalFarm farm, bool pull,
			int moved, int[] predicted, bool logMoves, string modeLabel) {
		int signed = pull ? moved : -moved;
		m_scheduler.ScheduleInputCmd(new MoveAnimalsCmd(farm.Id, signed));
		int min = int.MaxValue, max = int.MinValue;
		foreach (int v in predicted) {
			if (v < min) min = v;
			if (v > max) max = v;
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
	}
}
