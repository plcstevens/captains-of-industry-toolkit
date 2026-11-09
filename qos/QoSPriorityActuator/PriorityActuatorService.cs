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
using Mafi.Serialization;
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
// [ManuallyWrittenSerialization] + the hand-written Serialize /
// Deserialize pair below are REQUIRED for save safety — see the
// serialization block comment after the fields. Do not remove.
[ManuallyWrittenSerialization]
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class PriorityActuatorService : IDisposable {

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

	private readonly QoSPriorityActuatorMod m_mod;
	private readonly IEntitiesManager m_entities;
	private readonly IInputScheduler m_scheduler;
	private readonly ICalendar m_calendar;
	private readonly Dictionary<int, Observation> m_observations = new Dictionary<int, Observation>();
	private int m_passIndex;

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
	// 'PriorityActuatorService'" and THE GAME CANNOT SAVE AT ALL —
	// that was the suite 0.5.0 bug (fixed in 0.1.2). The
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
	// RegisterResolvedMember and is never serialized. The
	// observations and the pass index are SESSION-ONLY state (see
	// the Observation doc above: safety never depends on them; a
	// reload restarts the counting), so they are deliberately not
	// written; DeserializeData re-initialises them empty — on load
	// the instance is materialised uninitialised (no constructor,
	// no field initialisers run), so the reset must happen there.
	private static readonly Action<object, BlobWriter> s_serializeDataDelayedAction =
		(obj, writer) => ((PriorityActuatorService)obj).SerializeData(writer);

	private static readonly Action<object, BlobReader> s_deserializeDataDelayedAction =
		(obj, reader) => ((PriorityActuatorService)obj).DeserializeData(reader);

	public static void Serialize(PriorityActuatorService value, BlobWriter writer) {
		if (writer.TryStartClassSerialization(value)) {
			writer.EnqueueDataSerialization(value, s_serializeDataDelayedAction);
		}
	}

	public static PriorityActuatorService Deserialize(BlobReader reader) {
		PriorityActuatorService result;
		if (reader.TryStartClassDeserialization<PriorityActuatorService>(out result)) {
			reader.EnqueueDataDeserialization(result, s_deserializeDataDelayedAction);
		}
		return result;
	}

	private void SerializeData(BlobWriter writer) {
		writer.WriteGeneric(m_calendar);
	}

	private void DeserializeData(BlobReader reader) {
		reader.SetField(this, "m_calendar", reader.ReadGenericAs<ICalendar>());
		reader.RegisterResolvedMember(this, "m_mod", typeof(QoSPriorityActuatorMod), isField: true);
		reader.RegisterResolvedMember(this, "m_entities", typeof(IEntitiesManager), isField: true);
		reader.RegisterResolvedMember(this, "m_scheduler", typeof(IInputScheduler), isField: true);
		// Session-only state, intentionally not saved: start empty.
		reader.SetField(this, "m_observations", new Dictionary<int, Observation>());
		reader.SetField(this, "m_passIndex", 0);
	}

	public PriorityActuatorService(QoSPriorityActuatorMod mod, IEntitiesManager entities,
			IInputScheduler scheduler, ICalendar calendar) {
		m_mod = mod;
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
		if (!m_mod.JsonConfig.GetBool("enabled", false)) {
			QoSLog.Debug("PriorityActuator", "pass skipped: 'enabled' is false");
			return;
		}
		bool nudgeStarved = m_mod.JsonConfig.GetBool("nudge_starved", true);
		bool restoreWhenWorking = m_mod.JsonConfig.GetBool("restore_when_working", true);
		int persistenceDays = ActuatorGuardMath.ClampPersistenceDays(
			m_mod.JsonConfig.GetInt("persistence_days", 3));
		int maxNudge = ActuatorGuardMath.ClampMaxNudge(
			m_mod.JsonConfig.GetInt("max_nudge_steps", 2));
		int cooldownDays = ActuatorGuardMath.ClampCooldownDays(
			m_mod.JsonConfig.GetInt("cooldown_days", 2));
		int dailyCap = ActuatorGuardMath.ClampDailyCap(
			m_mod.JsonConfig.GetInt("max_actions_per_day", 10));
		bool logActions = m_mod.JsonConfig.GetBool("log_actions", true);

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
		if (ActuatorGuardMath.IsParked(machine.IsPaused, machine.IsEnabled,
				state == Machine.State.Paused)) {
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
		obs.StarvedStreak = ActuatorGuardMath.NextStreak(
			obs.StarvedStreak, obs.LastSeenPass, m_passIndex);
		obs.LastSeenPass = m_passIndex;

		// The decision — persistence, cooldown, priority surface,
		// cargo flag, ownership band, band floor, daily cap, in
		// that order — is ActuatorGuardMath.DecideNudge (pure,
		// unit-tested; QUALITY-AUDIT item 5). This method reads the
		// live values, carries the log lines, and schedules.
		IEntityWithGeneralPriority gp = machine as IEntityWithGeneralPriority;
		int protoDefault = machine.Prototype.Costs.DefaultPriority;
		int bandFloor = ActuatorGuardMath.BandFloor(protoDefault, maxNudge);
		int current = gp != null ? gp.GeneralPriority : 0;
		int target;
		ActuatorGuardMath.NudgeDecision decision = ActuatorGuardMath.DecideNudge(
			obs.StarvedStreak, persistenceDays, m_passIndex, obs.LastNudgePass, cooldownDays,
			gp != null, machine.IsCargoAffectedByGeneralPriority,
			current, protoDefault, maxNudge, actions, dailyCap, out target);
		switch (decision) {
			case ActuatorGuardMath.NudgeDecision.NotPersistent:
				QoSLog.Debug("PriorityActuator", "machine #" + id + " (" +
					machine.Prototype.Id.Value + ") cause=starved, streak " +
					obs.StarvedStreak + "/" + persistenceDays + " passes — not yet persistent");
				return;
			case ActuatorGuardMath.NudgeDecision.CoolingDown:
				QoSLog.Debug("PriorityActuator", "machine #" + id +
					" nudge skipped: per-machine cooldown");
				return;
			case ActuatorGuardMath.NudgeDecision.NoPrioritySurface:
				return;
			case ActuatorGuardMath.NudgeDecision.CargoUnaffected:
				// The value would be inert for this machine's cargo —
				// never perform a no-op action.
				QoSLog.Debug("PriorityActuator", "machine #" + id +
					" nudge skipped: its cargo is not affected by general priority");
				return;
			case ActuatorGuardMath.NudgeDecision.OutsideBand:
				// STATELESS OWNERSHIP: only values this mod's rule
				// could have produced may be adjusted. Anything else
				// is the player's and is never fought.
				QoSLog.Debug("PriorityActuator", "machine #" + id + " cause=starved but priority " +
					current + " is outside the owned band [" + bandFloor + ", " + protoDefault +
					"] (proto default " + protoDefault + ") — treated as player-set, hands off");
				return;
			case ActuatorGuardMath.NudgeDecision.AtBandFloor:
				QoSLog.Debug("PriorityActuator", "machine #" + id +
					" cause=starved but already at the band floor " + bandFloor + " — no further step");
				return;
			case ActuatorGuardMath.NudgeDecision.DailyCapReached:
				QoSLog.Debug("PriorityActuator", "machine #" + id +
					" nudge skipped: daily action cap reached");
				return;
		}

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
		int bandFloor = ActuatorGuardMath.BandFloor(protoDefault, maxNudge);
		int current = gp.GeneralPriority;

		// STATELESS RESTORE TEST (ActuatorGuardMath.DecideRestore,
		// pure, unit-tested): step back only a value this mod's
		// rule would have set for the starved cause — exactly
		// default minus 1..N steps. A value at the default needs
		// nothing; anything else is not ours.
		int target;
		ActuatorGuardMath.RestoreDecision decision = ActuatorGuardMath.DecideRestore(
			current, protoDefault, maxNudge, actions, dailyCap, out target);
		if (decision == ActuatorGuardMath.RestoreDecision.DailyCapReached) {
			QoSLog.Debug("PriorityActuator", "machine #" + id +
				" restore skipped: daily action cap reached");
			return;
		}
		if (decision != ActuatorGuardMath.RestoreDecision.Restore) {
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
