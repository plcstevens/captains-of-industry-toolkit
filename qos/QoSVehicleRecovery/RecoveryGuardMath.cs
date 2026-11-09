using System;

namespace QoSVehicleRecovery;

/// <summary>
/// The vehicle-recovery guard arithmetic, extracted as pure
/// functions over plain values — no game-assembly types — so it
/// can be executed off-game by the guard test harness
/// (qos/build/guardtests, gated by tools/check_qos_guards.sh).
/// QUALITY-AUDIT item 5 / finding T2.
///
/// Every function CHARACTERISES the pre-extraction behaviour of
/// VehicleRecoveryService exactly; semantics were derived from
/// the shipped code first and pinned by the harness's named
/// cases. The service calls these functions; it keeps only
/// game-object reads, the vanilla gates (CanRecoverVehicle and
/// IUpointsManager.CanConsume — game-side by design, see
/// RequiredRaw), logging and command scheduling.
///
/// Deliberately NOT extracted (game-side, inherited by design):
/// vanilla's CanRecoverVehicle gate, and the balance comparison
/// inside CanConsume — which also honours the game's
/// IgnoreMissingUnity setting and rounds via
/// GetQuantityRounded. The mod's own arithmetic in that check is
/// only the composition of the required amount (floor + cost),
/// extracted here as RequiredRaw.
/// </summary>
public static class RecoveryGuardMath {

	/// <summary>How long (in daily passes) a stage-2 mark stays valid.</summary>
	public const int MarkLifetimePasses = 7;

	/// <summary>Minimum navigation-failure streak clamp: Max(1, configured).</summary>
	public static int ClampMinStreak(int configured) {
		return Math.Max(1, configured);
	}

	/// <summary>Daily recovery cap clamp: Max(0, configured).</summary>
	public static int ClampDailyCap(int configured) {
		return Math.Max(0, configured);
	}

	/// <summary>Unity reserve floor clamp: Max(0.0, configured).</summary>
	public static double ClampReserveFloor(double configured) {
		return Math.Max(0.0, configured);
	}

	/// <summary>
	/// Player/depot-owned flows and in-flight states: hands off.
	/// True when the truck must not be acted on. (The service's
	/// destroyed / not-spawned pre-check stays in the service: it
	/// is a trivial state test, and it guards the
	/// IsVehicleBeingRecovered manager query from being evaluated
	/// for a truck that no longer exists.)
	/// </summary>
	public static bool IsHandsOff(bool beingRecovered,
			bool onWayToDepotForScrap, bool onWayToDepotForReplacement, bool replaceQueued) {
		return beingRecovered || onWayToDepotForScrap || onWayToDepotForReplacement || replaceQueued;
	}

	/// <summary>
	/// The sustained-stuck signal: the game's own
	/// repeated-navigation-failure flag, OR stuck with a
	/// navigation-failure streak at/above the configured
	/// threshold. A transient pathing hiccup never qualifies.
	/// </summary>
	public static bool IsNavStuck(bool failingRepeatedly, bool isStuck,
			int navigationFailedStreak, int minStreak) {
		return failingRepeatedly || (isStuck && navigationFailedStreak >= minStreak);
	}

	/// <summary>
	/// Stage-2 (EMPTY) eligibility: the feature is on, the truck
	/// is flagged cannot-deliver again, it is still loaded, this
	/// mod holds a mark for it, and the mark's product is the
	/// load's product — the SAME load that was rescued before.
	/// </summary>
	public static bool IsStage2Eligible(bool emptyEnabled, bool cannotDeliver, bool isNotEmpty,
			bool hasMark, string markProductId, string cargoProductId) {
		return emptyEnabled && cannotDeliver && isNotEmpty && hasMark &&
			markProductId == cargoProductId;
	}

	/// <summary>Stage 1 (RECOVER) is wanted when either enabled signal fires.</summary>
	public static bool IsStage1Wanted(bool cannotDeliver, bool recoverCannotDeliver,
			bool navStuck, bool recoverNavStuck) {
		return (cannotDeliver && recoverCannotDeliver) || (navStuck && recoverNavStuck);
	}

	/// <summary>
	/// A stage-2 mark expires when it is MORE than
	/// MarkLifetimePasses old: a mark set on pass P is still
	/// valid on pass P+7 and expires at the start of pass P+8.
	/// </summary>
	public static bool IsMarkExpired(int currentPass, int markPass) {
		return currentPass - markPass > MarkLifetimePasses;
	}

	/// <summary>The per-day stage-1 cap: another recovery fits while recoveries &lt; cap.</summary>
	public static bool WithinDailyCap(int recoveries, int dailyCap) {
		return recoveries < dailyCap;
	}

	/// <summary>
	/// The required Unity amount for a guarded action, in Fix32
	/// raw units (Upoints = raw / 65536): reserve floor + action
	/// cost. This is EXACTLY the shipped `reserveFloor + cost`:
	/// Upoints.op_Addition is `new Upoints(new Fix32(lhs.Raw +
	/// rhs.Raw))` (verified against the shipped assemblies), so
	/// plain unchecked int addition of the raws is the identical
	/// computation, and the service reconstructs the argument for
	/// CanConsume via Fix32.FromRaw.
	///
	/// The decision this amount feeds: the action is allowed
	/// while balance &gt;= floor + cost — the balance AFTER the
	/// charge stays at or above the reserve floor. Equality
	/// allows. (The game-side comparison additionally honours
	/// IgnoreMissingUnity and GetQuantityRounded rounding; that
	/// inheritance is deliberate and stays in CanConsume.)
	/// </summary>
	public static int RequiredRaw(int reserveFloorRaw, int costRaw) {
		return reserveFloorRaw + costRaw;
	}

	/// <summary>The reserve decision over raw units: balance &gt;= floor + cost.</summary>
	public static bool ReserveAllows(int balanceRaw, int reserveFloorRaw, int costRaw) {
		return balanceRaw >= RequiredRaw(reserveFloorRaw, costRaw);
	}
}
