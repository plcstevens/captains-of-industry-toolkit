using System;
using System.Collections.Generic;
using Mafi;
using Mafi.Core.Entities;
using Mafi.Core.Entities.Commands;
using Mafi.Core.Entities.Priorities;
using Mafi.Core.Factory.Machines;
using Mafi.Core.Input;
using Mafi.Core.Mods;
using Mafi.Core.Simulation;
using QoSCore;

namespace QoSPriorityActuator;

/// <summary>
/// Bounded priority nudges for starved machines (PATTERN-SURVEY.md
/// §2.3; design in qos/BUILD-NOTES.md round 5). Once per in-game
/// day (at day end) the pass classifies every machine by its own
/// state — the same cause vocabulary as the companion's
/// productivity estimators (QOL-MOD-PREP.md §5):
///
///   STARVED (Machine.State.NotEnoughInput) — after a configured
///   number of consecutive daily observations, the machine's
///   general priority is stepped UP one step: in the game's value
///   semantics a LOWER number is served sooner
///   (GeneralPriorities: HIGHEST_PRIORITY 0, HIGH 8, DEFAULT 9,
///   assignable 0-14), so stepping up schedules
///   SetGeneralPriorityCmd at current - 1 and cargo jobs for the
///   machine gain precedence. Trucks serving its inputs is the
///   predicted effect; logistics response is systemic and is NOT
///   simulated — this is the bounded-nudge safety class, stated
///   plainly.
///
///   BLOCKED (OutputFull) — never adjusted. Priority cannot fix
///   a downstream blockage, and general priority steers the
///   entity's cargo jobs as a whole (IsCargoAffectedByGeneral-
///   Priority is a single per-entity flag — there is no
///   input-only machine priority), so stepping a blocked machine
///   down would also deprioritise the pickups that empty it.
///   Blocked machines are counted and verbose-logged only.
///
///   PARKED (player-paused, disabled, or state Paused) — never
///   touched, never counted toward persistence. The player's
///   own choice, not a disease.
///
///   Working — the restore class: if the value is still one
///   this mod's rule could have set, it is stepped back toward
///   the proto default, one step per pass.
///
///   Everything else (Broken, NoRecipes, NotEnoughWorkers /
///   Power / Computing, InvalidPlacement, None) is never
///   touched: cargo priority cannot fix it.
///
/// THE OWNERSHIP RULE (stateless — the heart of the mod). Let
/// D = the machine's proto default (Prototype.Costs.
/// DefaultPriority) and N = max_nudge_steps. A machine may be
/// adjusted only while its current value is in the owned set
/// { D - k : 0 &lt;= k &lt;= N }. Anything outside it is treated as
/// player-set and left alone — the mod never fights a value it
/// could not have produced, and needs no memory of its own
/// actions for safety: ownership, the band and the restore test
/// are all computed fresh from the live value every pass, so
/// they survive save reloads exactly. (Accepted imperfection,
/// from the survey: a player's own D - 1 is indistinguishable
/// from the mod's -1; the small band bounds the worst case and
/// every action is logged. Setting any value outside the band
/// defeats the rule entirely.)
///
/// Bounds, all config: persistence (consecutive starved passes
/// before the first nudge), the step band N, a per-machine
/// cooldown between nudges, and a per-day cap on all scheduled
/// actions. Persistence/cooldown bookkeeping is SESSION memory
/// only (the recovery mod's marks pattern): a reload resets it,
/// which only ever delays action.
///
/// Actuation is exclusively the shipped SetGeneralPriorityCmd
/// through IInputScheduler; the mod never writes an entity
/// value directly. Everything is wrapped so a failure is a log
/// line, never an exception into the sim. Config is re-read
/// from the mod's ModJsonConfig on every pass, so settings
/// changes apply from the next day with no restart.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class PriorityActuatorService : IDisposable {

	/// <summary>Assignable range of general priority (GeneralPriorities, recon 2a).</summary>
	private const int MinAssignable = 0;
	private const int MaxAssignable = 14;

	/// <summary>
	/// Session-only observation bookkeeping: consecutive-starved
	/// counting and the per-machine nudge cooldown. Safety never
	/// depends on this (ownership/band/restore are stateless); a
	/// reload simply restarts the counting.
	/// </summary>
	private sealed class Observation {
		public int StarvedStreak;
		public int LastSeenPass;
		public int LastNudgePass = int.MinValue / 2;
	}

	private readonly ModJsonConfig m_config;
	private readonly IEntitiesManager m_entities;
	private readonly IInputScheduler m_scheduler;
	private readonly ICalendar m_calendar;
	private readonly Dictionary<int, Observation> m_observations = new Dictionary<int, Observation>();
	private int m_passIndex;

	public PriorityActuatorService(QoSPriorityActuatorMod mod, IEntitiesManager entities,
			IInputScheduler scheduler, ICalendar calendar) {
		m_config = mod.JsonConfig;
		m_entities = entities;
		m_scheduler = scheduler;
		m_calendar = calendar;
		m_calendar.NewDayEnd.Add(this, OnNewDayEnd);
		QoSLog.Info("PriorityActuator", "actuator armed (daily pass at day end; " +
			"master switch is the 'enabled' mod setting, default off)");
	}

	public void Dispose() {
		m_calendar.NewDayEnd.Remove(this, OnNewDayEnd);
	}

	private void OnNewDayEnd() {
		try {
			RunPass();
		} catch (Exception ex) {
			QoSLog.Error("PriorityActuator", "daily pass failed (no actions taken): " + ex);
		}
	}

	private void RunPass() {
		if (!m_config.GetBool("enabled", false)) {
			QoSLog.Debug("PriorityActuator", "pass skipped: 'enabled' is false");
			return;
		}
		bool nudgeStarved = m_config.GetBool("nudge_starved", true);
		bool restoreWhenWorking = m_config.GetBool("restore_when_working", true);
		int persistenceDays = Math.Max(1, m_config.GetInt("persistence_days", 3));
		int maxNudge = Math.Max(0, m_config.GetInt("max_nudge_steps", 2));
		int cooldownDays = Math.Max(0, m_config.GetInt("cooldown_days", 2));
		int dailyCap = Math.Max(0, m_config.GetInt("max_actions_per_day", 10));
		bool logActions = m_config.GetBool("log_actions", true);

		m_passIndex++;
		int actions = 0;
		int blockedSeen = 0;
		var seen = new HashSet<int>();
		foreach (Machine machine in m_entities.GetAllEntitiesOfType<Machine>()) {
			if (machine.IsDestroyed) {
				continue;
			}
			seen.Add(machine.Id.Value);
			try {
				ProcessMachine(machine, nudgeStarved, restoreWhenWorking, persistenceDays,
					maxNudge, cooldownDays, dailyCap, logActions, ref actions, ref blockedSeen);
			} catch (Exception ex) {
				QoSLog.Error("PriorityActuator", "machine #" + machine.Id.Value +
					" evaluation failed (skipped): " + ex.Message);
			}
		}
		PruneObservations(seen);
		if (blockedSeen > 0) {
			QoSLog.Debug("PriorityActuator", "pass observed " + blockedSeen +
				" BLOCKED machine(s) — never adjusted by design (see readme)");
		}
		if (actions > 0 && logActions) {
			QoSLog.Info("PriorityActuator", "pass complete: " + actions +
				" priority actions scheduled (daily cap " + dailyCap + ")");
		}
	}

	/// <summary>Drop session bookkeeping for machines that no longer exist.</summary>
	private void PruneObservations(HashSet<int> seen) {
		if (m_observations.Count == 0) {
			return;
		}
		var stale = new List<int>();
		foreach (int id in m_observations.Keys) {
			if (!seen.Contains(id)) {
				stale.Add(id);
			}
		}
		foreach (int id in stale) {
			m_observations.Remove(id);
		}
	}

	private Observation GetObservation(int id) {
		Observation obs;
		if (!m_observations.TryGetValue(id, out obs)) {
			obs = new Observation();
			m_observations.Add(id, obs);
		}
		return obs;
	}

	/// <summary>Any non-starved classification breaks the consecutive-starved streak.</summary>
	private void BreakStreak(int id) {
		Observation obs;
		if (m_observations.TryGetValue(id, out obs)) {
			obs.StarvedStreak = 0;
			obs.LastSeenPass = m_passIndex;
		}
	}

	private void ProcessMachine(Machine machine, bool nudgeStarved, bool restoreWhenWorking,
			int persistenceDays, int maxNudge, int cooldownDays, int dailyCap,
			bool logActions, ref int actions, ref int blockedSeen) {
		if (!machine.IsConstructed) {
			return;
		}
		int id = machine.Id.Value;
		Machine.State state = machine.CurrentState;

		// PARKED first (§5's order): the player's own choice —
		// paused, disabled, or reporting the Paused state. Never
		// touched, never counted toward persistence.
		if (machine.IsPaused || !machine.IsEnabled || state == Machine.State.Paused) {
			BreakStreak(id);
			return;
		}

		if (state == Machine.State.OutputFull) {
			// BLOCKED: observed, never adjusted (see class docs).
			blockedSeen++;
			BreakStreak(id);
			return;
		}

		if (state == Machine.State.NotEnoughInput) {
			ProcessStarved(machine, nudgeStarved, persistenceDays, maxNudge,
				cooldownDays, dailyCap, logActions, ref actions);
			return;
		}

		if (state == Machine.State.Working) {
			BreakStreak(id);
			ProcessRestore(machine, restoreWhenWorking, maxNudge, dailyCap,
				logActions, ref actions);
			return;
		}

		// Broken / NoRecipes / NotEnoughWorkers / NotEnoughPower /
		// NotEnoughComputing / InvalidPlacement / None: cargo
		// priority cannot fix any of these. Hands off.
		BreakStreak(id);
	}

	private void ProcessStarved(Machine machine, bool nudgeStarved, int persistenceDays,
			int maxNudge, int cooldownDays, int dailyCap, bool logActions, ref int actions) {
		if (!nudgeStarved) {
			return;
		}
		int id = machine.Id.Value;
		Observation obs = GetObservation(id);
		obs.StarvedStreak = (obs.LastSeenPass == m_passIndex - 1 && obs.StarvedStreak > 0)
			? obs.StarvedStreak + 1
			: 1;
		obs.LastSeenPass = m_passIndex;

		if (obs.StarvedStreak < persistenceDays) {
			QoSLog.Debug("PriorityActuator", "machine #" + id + " (" +
				machine.Prototype.Id.Value + ") cause=starved, streak " +
				obs.StarvedStreak + "/" + persistenceDays + " passes — not yet persistent");
			return;
		}
		if (m_passIndex - obs.LastNudgePass < cooldownDays) {
			QoSLog.Debug("PriorityActuator", "machine #" + id +
				" nudge skipped: per-machine cooldown");
			return;
		}

		IEntityWithGeneralPriority gp = machine as IEntityWithGeneralPriority;
		if (gp == null) {
			return;
		}
		if (!machine.IsCargoAffectedByGeneralPriority) {
			// The value would be inert for this machine's cargo —
			// never perform a no-op action.
			QoSLog.Debug("PriorityActuator", "machine #" + id +
				" nudge skipped: its cargo is not affected by general priority");
			return;
		}
		int protoDefault = machine.Prototype.Costs.DefaultPriority;
		int bandFloor = Math.Max(MinAssignable, protoDefault - maxNudge);
		int current = gp.GeneralPriority;

		// STATELESS OWNERSHIP: only values this mod's rule could
		// have produced may be adjusted. Anything else is the
		// player's and is never fought.
		if (current > protoDefault || current < bandFloor) {
			QoSLog.Debug("PriorityActuator", "machine #" + id + " cause=starved but priority " +
				current + " is outside the owned band [" + bandFloor + ", " + protoDefault +
				"] (proto default " + protoDefault + ") — treated as player-set, hands off");
			return;
		}
		if (current <= bandFloor) {
			QoSLog.Debug("PriorityActuator", "machine #" + id +
				" cause=starved but already at the band floor " + bandFloor + " — no further step");
			return;
		}
		if (actions >= dailyCap) {
			QoSLog.Debug("PriorityActuator", "machine #" + id +
				" nudge skipped: daily action cap reached");
			return;
		}

		int target = current - 1;
		m_scheduler.ScheduleInputCmd(new SetGeneralPriorityCmd(machine, target));
		// Unified activity record (Addendum XVI).
		QoSEventLog.Record("QoSPriorityActuator", QoSPriorityActuatorMod.Version, "command",
			"SetGeneralPriorityCmd",
			"nudge scheduled: machine #" + id + " (" + machine.Prototype.Id.Value +
			") cause=starved (streak " + obs.StarvedStreak + " passes) priority " +
			current + " -> " + target + " (proto default " + protoDefault + ")",
			new Dictionary<string, object> {
				{ "machineId", id },
				{ "proto", machine.Prototype.Id.Value },
				{ "cause", "starved" },
				{ "current", current },
				{ "target", target },
				{ "protoDefault", protoDefault },
				{ "bandFloor", bandFloor },
				{ "streak", obs.StarvedStreak },
			});
		obs.LastNudgePass = m_passIndex;
		actions++;
		if (logActions) {
			QoSLog.Info("PriorityActuator", "nudge scheduled: machine #" + id + " (" +
				machine.Prototype.Id.Value + ") cause=starved (streak " + obs.StarvedStreak +
				" passes) priority " + current + " -> " + target + " (proto default " +
				protoDefault + ", band floor " + bandFloor +
				"); predicted: this machine's cargo jobs are served sooner — " +
				"logistics response is systemic and not simulated");
		}
	}

	private void ProcessRestore(Machine machine, bool restoreWhenWorking, int maxNudge,
			int dailyCap, bool logActions, ref int actions) {
		if (!restoreWhenWorking) {
			return;
		}
		IEntityWithGeneralPriority gp = machine as IEntityWithGeneralPriority;
		if (gp == null) {
			return;
		}
		if (!machine.IsCargoAffectedByGeneralPriority) {
			return;
		}
		int id = machine.Id.Value;
		int protoDefault = machine.Prototype.Costs.DefaultPriority;
		int bandFloor = Math.Max(MinAssignable, protoDefault - maxNudge);
		int current = gp.GeneralPriority;

		// STATELESS RESTORE TEST: step back only a value this
		// mod's rule would have set for the starved cause —
		// exactly default minus 1..N steps. A value at the
		// default needs nothing; anything else is not ours.
		if (current >= protoDefault || current < bandFloor) {
			return;
		}
		int target = current + 1;
		if (target > MaxAssignable) {
			return;
		}
		if (actions >= dailyCap) {
			QoSLog.Debug("PriorityActuator", "machine #" + id +
				" restore skipped: daily action cap reached");
			return;
		}

		m_scheduler.ScheduleInputCmd(new SetGeneralPriorityCmd(machine, target));
		// Unified activity record (Addendum XVI).
		QoSEventLog.Record("QoSPriorityActuator", QoSPriorityActuatorMod.Version, "command",
			"SetGeneralPriorityCmd",
			"restore scheduled: machine #" + id + " (" + machine.Prototype.Id.Value +
			") cause=working priority " + current + " -> " + target +
			" (proto default " + protoDefault + ")",
			new Dictionary<string, object> {
				{ "machineId", id },
				{ "proto", machine.Prototype.Id.Value },
				{ "cause", "working" },
				{ "current", current },
				{ "target", target },
				{ "protoDefault", protoDefault },
				{ "bandFloor", bandFloor },
			});
		actions++;
		if (logActions) {
			QoSLog.Info("PriorityActuator", "restore scheduled: machine #" + id + " (" +
				machine.Prototype.Id.Value + ") cause=working priority " + current +
				" -> " + target + " (proto default " + protoDefault +
				"); predicted: one step back toward the proto default");
		}
	}
}
