using System;
using System.Collections.Generic;
using Mafi;
using Mafi.Core.Input;
using Mafi.Core.Mods;
using Mafi.Core.Population;
using Mafi.Core.Simulation;
using Mafi.Serialization;
using Mafi.Core.Vehicles;
using Mafi.Core.Vehicles.Commands;
using Mafi.Core.Vehicles.Trucks;
using Mafi.Localization;
using QoSCore;

namespace QoSVehicleRecovery;

/// <summary>
/// The stuck-truck cargo rescue policy (QOL-MODS.md items 6-8;
/// design + recon in qos/BUILD-NOTES.md round 3 and
/// qol-recon/06-vehicle-recovery.md). Once per in-game day (at
/// day end) it looks for trucks that are stuck WITH their load
/// and schedules the game's OWN commands through IInputScheduler:
///
///   Stage 1 — RECOVER (RecoverVehicleCmd): vanilla recovery
///   teleports the truck to the closest accepting depot for a
///   flat 0.2 Upoints and — verified from the assemblies — never
///   touches the cargo buffer: the truck drives out of the depot
///   with its load byte-identical and its jobs re-planned. The
///   content is recovered aboard, the strongest form of
///   preservation.
///
///   Stage 2 — EMPTY (DiscardVehicleCargoCmd, default OFF): a
///   truck this mod already recovered whose SAME load is flagged
///   cannot-deliver again on a later pass has the load emptied
///   through the shipped discard: the game's StoreBuffer routing
///   deposits it into real storages in global-priority order
///   (shipyard overflow last) — content into the economy, truck
///   freed. The shipped processor charges 0.1 Upoints on success.
///
/// Hard guards, all checked BEFORE scheduling (a failed vanilla
/// recovery cancels the truck's jobs before it even tries to
/// enqueue, so a doomed attempt must never be scheduled):
///   * CanRecoverVehicle — vanilla's own gate (a depot accepts
///     the truck AND Unity affords the recovery cost);
///   * a Unity reserve floor: the balance after the charge must
///     stay at or above the configured floor, for both stages;
///   * a per-day cap on scheduled recoveries;
///   * never touch a truck that is being recovered, scrapped or
///     replaced — player/depot flows own those;
///   * stuck must be a SUSTAINED signal: the cannot-deliver
///     notification itself, the game's own repeated-navigation-
///     failure flag, or IsStuck with a navigation-failure streak
///     at/above the configured threshold. A transient pathing
///     hiccup never triggers a paid recovery.
///
/// Stage-2 marks are session memory only (truck id -> pass +
/// product): they expire when the truck empties (the load was
/// delivered — episode over) or after 7 passes, and vanish on
/// save reload, which only ever makes the mod MORE conservative.
///
/// There is deliberately NO "preferred storage" setting: no
/// public surface lets a mod steer a truck's cargo into a chosen
/// storage (recon 06, Q6), and a config the game cannot honour
/// would be a lie in the settings UI. The destination policy is
/// the ladder above: aboard first, the economy second.
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
public sealed class VehicleRecoveryService : IDisposable {

	/// <summary>How long (in daily passes) a stage-2 mark stays valid.</summary>
	private const int MarkLifetimePasses = 7;

	private sealed class RecoveryMark {
		public int Pass;
		public string ProductId;
	}

	private readonly QoSVehicleRecoveryMod m_mod;
	private readonly IVehiclesManager m_vehicles;
	private readonly IUpointsManager m_upoints;
	private readonly IInputScheduler m_scheduler;
	private readonly ICalendar m_calendar;
	private readonly Dictionary<int, RecoveryMark> m_marks = new Dictionary<int, RecoveryMark>();
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
	// 'VehicleRecoveryService'" and THE GAME CANNOT SAVE AT ALL —
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
	// RegisterResolvedMember and is never serialized. The marks and
	// the pass index are SESSION-ONLY state (documented semantics:
	// the "came back still broken" memory resets on reload), so
	// they are deliberately not written; DeserializeData
	// re-initialises them empty — on load the instance is
	// materialised uninitialised (no constructor, no field
	// initialisers run), so the reset must happen there.
	private static readonly Action<object, BlobWriter> s_serializeDataDelayedAction =
		(obj, writer) => ((VehicleRecoveryService)obj).SerializeData(writer);

	private static readonly Action<object, BlobReader> s_deserializeDataDelayedAction =
		(obj, reader) => ((VehicleRecoveryService)obj).DeserializeData(reader);

	public static void Serialize(VehicleRecoveryService value, BlobWriter writer) {
		if (writer.TryStartClassSerialization(value)) {
			writer.EnqueueDataSerialization(value, s_serializeDataDelayedAction);
		}
	}

	public static VehicleRecoveryService Deserialize(BlobReader reader) {
		VehicleRecoveryService result;
		if (reader.TryStartClassDeserialization<VehicleRecoveryService>(out result)) {
			reader.EnqueueDataDeserialization(result, s_deserializeDataDelayedAction);
		}
		return result;
	}

	private void SerializeData(BlobWriter writer) {
		writer.WriteGeneric(m_calendar);
	}

	private void DeserializeData(BlobReader reader) {
		reader.SetField(this, "m_calendar", reader.ReadGenericAs<ICalendar>());
		reader.RegisterResolvedMember(this, "m_mod", typeof(QoSVehicleRecoveryMod), isField: true);
		reader.RegisterResolvedMember(this, "m_vehicles", typeof(IVehiclesManager), isField: true);
		reader.RegisterResolvedMember(this, "m_upoints", typeof(IUpointsManager), isField: true);
		reader.RegisterResolvedMember(this, "m_scheduler", typeof(IInputScheduler), isField: true);
		// Session-only state, intentionally not saved: start empty.
		reader.SetField(this, "m_marks", new Dictionary<int, RecoveryMark>());
		reader.SetField(this, "m_passIndex", 0);
	}

	public VehicleRecoveryService(QoSVehicleRecoveryMod mod, IVehiclesManager vehicles,
			IUpointsManager upoints, IInputScheduler scheduler, ICalendar calendar) {
		m_mod = mod;
		m_vehicles = vehicles;
		m_upoints = upoints;
		m_scheduler = scheduler;
		m_calendar = calendar;
		m_calendar.NewDayEnd.Add(this, OnNewDayEnd);
		QoSLog.Info("VehicleRecovery", "rescue service armed (daily pass at day end; " +
			"master switch is the 'enabled' mod setting, default off)");
	}

	public void Dispose() {
		m_calendar.NewDayEnd.Remove(this, OnNewDayEnd);
	}

	private void OnNewDayEnd() {
		try {
			RunPass();
		} catch (Exception ex) {
			QoSLog.Error("VehicleRecovery", "daily pass failed (no actions taken): " + ex);
		}
	}

	private void RunPass() {
		if (!m_mod.JsonConfig.GetBool("enabled", false)) {
			QoSLog.Debug("VehicleRecovery", "pass skipped: 'enabled' is false");
			return;
		}
		bool recoverCannotDeliver = m_mod.JsonConfig.GetBool("recover_cannot_deliver", true);
		bool recoverNavStuck = m_mod.JsonConfig.GetBool("recover_navigation_stuck", true);
		bool emptyEnabled = m_mod.JsonConfig.GetBool("empty_cargo_if_still_undeliverable", false);
		int minStreak = Math.Max(1, m_mod.JsonConfig.GetInt("min_navigation_failed_streak", 3));
		int dailyCap = Math.Max(0, m_mod.JsonConfig.GetInt("max_recoveries_per_day", 5));
		Upoints reserveFloor = Math.Max(0.0, m_mod.JsonConfig.GetDouble("unity_reserve_floor", 5.0)).Upoints();
		bool logActions = m_mod.JsonConfig.GetBool("log_actions", true);

		m_passIndex++;
		ExpireMarks();

		int recoveries = 0;
		foreach (Truck truck in m_vehicles.Trucks) {
			try {
				ProcessTruck(truck, recoverCannotDeliver, recoverNavStuck, emptyEnabled,
					minStreak, dailyCap, reserveFloor, logActions, ref recoveries);
			} catch (Exception ex) {
				QoSLog.Error("VehicleRecovery", "truck #" + truck.Id.Value +
					" evaluation failed (skipped): " + ex.Message);
			}
		}
		if (recoveries > 0 && logActions) {
			QoSLog.Info("VehicleRecovery", "pass complete: " + recoveries +
				" recoveries scheduled (daily cap " + dailyCap + ")");
		}
	}

	/// <summary>Drop marks that are stale or whose load episode has ended.</summary>
	private void ExpireMarks() {
		if (m_marks.Count == 0) {
			return;
		}
		var stale = new List<int>();
		foreach (KeyValuePair<int, RecoveryMark> kv in m_marks) {
			if (m_passIndex - kv.Value.Pass > MarkLifetimePasses) {
				stale.Add(kv.Key);
			}
		}
		foreach (int id in stale) {
			m_marks.Remove(id);
			QoSLog.Debug("VehicleRecovery", "stage-2 mark for truck #" + id + " expired");
		}
	}

	private void ProcessTruck(Truck truck, bool recoverCannotDeliver, bool recoverNavStuck,
			bool emptyEnabled, int minStreak, int dailyCap, Upoints reserveFloor,
			bool logActions, ref int recoveries) {
		// Player/depot-owned flows and in-flight states: hands off.
		if (truck.IsDestroyed || !truck.IsSpawned) {
			return;
		}
		if (m_vehicles.IsVehicleBeingRecovered(truck)) {
			return;
		}
		if (truck.IsOnWayToDepotForScrap || truck.IsOnWayToDepotForReplacement || truck.ReplaceQueued) {
			return;
		}

		int id = truck.Id.Value;
		bool cannotDeliver = truck.IsCannotDeliverNotificationActive;
		bool navStuck = truck.IsFailingToNavigateRepeatedly ||
			(truck.IsStuck && truck.NavigationFailedStreak >= minStreak);

		// A marked truck whose load is gone has finished its
		// episode (delivered after recovery): clear the mark.
		if (truck.IsEmpty) {
			if (m_marks.Remove(id)) {
				QoSLog.Debug("VehicleRecovery", "truck #" + id +
					" is empty — load delivered after recovery, stage-2 mark cleared");
			}
			// An empty truck can still be navigation-stuck; stage 1
			// below may still apply to it.
		}

		// Stage 2 — EMPTY: this mod already rescued this truck
		// once, and the SAME load is flagged undeliverable again.
		// Checked before stage 1 so a repeat rescue never loops.
		RecoveryMark mark;
		if (emptyEnabled && cannotDeliver && truck.IsNotEmpty &&
				m_marks.TryGetValue(id, out mark) &&
				mark.ProductId == truck.Cargo.Product.Id.Value) {
			Upoints discardCost = VehicleCommandsProcessor.COST_TO_DISCARD_CARGO;
			if (!m_upoints.CanConsume(reserveFloor + discardCost)) {
				QoSLog.Debug("VehicleRecovery", "truck #" + id +
					" stage-2 empty skipped: Unity reserve floor would be breached");
				return;
			}
			m_scheduler.ScheduleInputCmd(new DiscardVehicleCargoCmd(truck.Id));
			// Unified activity record (Addendum XVI).
			QoSEventLog.Record("QoSVehicleRecovery", QoSVehicleRecoveryMod.Version, "command",
				"DiscardVehicleCargoCmd",
				"stage-2 EMPTY scheduled: truck #" + id + ", cargo " + DescribeCargo(truck) +
				" -> economy storage routing (load was still undeliverable after a mod recovery)",
				new Dictionary<string, object> {
					{ "truckId", id },
					{ "stage", 2 },
					{ "product", mark.ProductId },
					{ "quantity", truck.Cargo.Quantity.Value },
					{ "costUpoints", UpointsValue(discardCost) },
				});
			m_marks.Remove(id);
			if (logActions) {
				QoSLog.Info("VehicleRecovery", "stage-2 EMPTY scheduled: truck #" + id +
					", cargo " + DescribeCargo(truck) + " -> economy storage routing " +
					"(game StoreBuffer order), predicted cost " + discardCost.Format1Dec() +
					" Upoints (load was still undeliverable after a mod recovery)");
			}
			return;
		}

		// Stage 1 — RECOVER.
		bool wanted = (cannotDeliver && recoverCannotDeliver) || (navStuck && recoverNavStuck);
		if (!wanted) {
			return;
		}
		if (recoveries >= dailyCap) {
			QoSLog.Debug("VehicleRecovery", "truck #" + id +
				" recovery skipped: daily cap reached");
			return;
		}
		// Prediction guards. CanRecoverVehicle is vanilla's own
		// gate; checking it first matters because the shipped
		// recovery request cancels the truck's jobs BEFORE it
		// tries to enqueue — a request that then fails (no depot,
		// no Unity) still disrupts the truck. Never schedule a
		// recovery vanilla would refuse.
		LocStrFormatted gateReason;
		if (!m_vehicles.CanRecoverVehicle(truck, out gateReason)) {
			QoSLog.Debug("VehicleRecovery", "truck #" + id +
				" recovery skipped: vanilla gate refuses (no accepting depot or Unity short)");
			return;
		}
		Upoints cost = VehiclesManager.VEHICLE_RECOVERY_COST;
		if (!m_upoints.CanConsume(reserveFloor + cost)) {
			QoSLog.Debug("VehicleRecovery", "truck #" + id +
				" recovery skipped: Unity reserve floor would be breached");
			return;
		}

		m_scheduler.ScheduleInputCmd(new RecoverVehicleCmd(truck.Id));
		recoveries++;
		if (truck.IsNotEmpty) {
			m_marks[id] = new RecoveryMark {
				Pass = m_passIndex,
				ProductId = truck.Cargo.Product.Id.Value,
			};
		}
		string reason = cannotDeliver
			? "cannot-deliver"
			: "navigation-stuck (streak " + truck.NavigationFailedStreak + ")";
		// Unified activity record (Addendum XVI): every scheduled
		// recovery, whether or not log_actions is on.
		var recordData = new Dictionary<string, object> {
			{ "truckId", id },
			{ "stage", 1 },
			{ "reason", cannotDeliver ? "cannot-deliver" : "navigation-stuck" },
			{ "navigationFailedStreak", truck.NavigationFailedStreak },
			{ "costUpoints", UpointsValue(cost) },
		};
		if (truck.IsNotEmpty) {
			recordData["cargoProduct"] = truck.Cargo.Product.Id.Value;
			recordData["cargoQuantity"] = truck.Cargo.Quantity.Value;
		}
		QoSEventLog.Record("QoSVehicleRecovery", QoSVehicleRecoveryMod.Version, "command",
			"RecoverVehicleCmd",
			"stage-1 RECOVER scheduled: truck #" + id + ", reason " + reason +
			", cargo " + DescribeCargo(truck) + ", predicted cost " + cost.Format1Dec() +
			" Upoints; cargo preserved aboard",
			recordData);
		if (logActions) {
			QoSLog.Info("VehicleRecovery", "stage-1 RECOVER scheduled: truck #" + id +
				", reason " + reason + ", cargo " + DescribeCargo(truck) +
				", predicted cost " + cost.Format1Dec() +
				" Upoints; expected: teleport to closest accepting depot, cargo preserved aboard");
		}
	}

	private static string DescribeCargo(Truck truck) {
		if (truck.IsEmpty) {
			return "none";
		}
		return truck.Cargo.Product.Id.Value + " x" + truck.Cargo.Quantity.Value;
	}

	/// <summary>Upoints as a plain number for the activity record
	/// (Fix32 raw / 65536 — the bridge-verified Fix32 decoding).</summary>
	private static double UpointsValue(Upoints u) {
		return u.Value.RawValue / 65536.0;
	}
}
