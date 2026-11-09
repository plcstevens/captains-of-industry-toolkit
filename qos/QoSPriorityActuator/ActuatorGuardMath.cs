using System;

namespace QoSPriorityActuator;

/// <summary>
/// The priority actuator's guard arithmetic, extracted as pure
/// functions over plain values — no game-assembly types — so it
/// can be executed off-game by the guard test harness
/// (qos/build/guardtests, gated by tools/check_qos_guards.sh).
/// QUALITY-AUDIT item 5 / finding T2.
///
/// Every function CHARACTERISES the pre-extraction behaviour of
/// PriorityActuatorService exactly; semantics were derived from
/// the shipped code first and pinned by the harness's named
/// cases, including the ORDER of the checks (the decision enums
/// below are returned in the shipped evaluation order — an
/// earlier check masks every later one). The service calls these
/// functions; it keeps only game-object reads, logging and
/// command scheduling. Ownership, the band and the restore test
/// remain stateless, computed fresh from the live value every
/// pass, exactly as before.
/// </summary>
public static class ActuatorGuardMath {

	/// <summary>Assignable range of general priority (GeneralPriorities, recon 2a).</summary>
	public const int MinAssignable = 0;
	public const int MaxAssignable = 14;

	/// <summary>Persistence (consecutive starved passes before a nudge) clamp: Max(1, configured).</summary>
	public static int ClampPersistenceDays(int configured) {
		return Math.Max(1, configured);
	}

	/// <summary>Step-band clamp: Max(0, configured).</summary>
	public static int ClampMaxNudge(int configured) {
		return Math.Max(0, configured);
	}

	/// <summary>Per-machine cooldown clamp: Max(0, configured).</summary>
	public static int ClampCooldownDays(int configured) {
		return Math.Max(0, configured);
	}

	/// <summary>Daily action cap clamp: Max(0, configured).</summary>
	public static int ClampDailyCap(int configured) {
		return Math.Max(0, configured);
	}

	/// <summary>
	/// PARKED: player-paused, disabled, or reporting the Paused
	/// state. The player's own choice — never touched, never
	/// counted toward persistence.
	/// </summary>
	public static bool IsParked(bool isPaused, bool isEnabled, bool stateIsPaused) {
		return isPaused || !isEnabled || stateIsPaused;
	}

	/// <summary>
	/// The consecutive-starved streak recurrence: it increments
	/// only when the previous observation was on the immediately
	/// preceding pass AND the streak was already running;
	/// anything else restarts it at 1 (this pass counts).
	/// </summary>
	public static int NextStreak(int previousStreak, int lastSeenPass, int currentPass) {
		return (lastSeenPass == currentPass - 1 && previousStreak > 0)
			? previousStreak + 1
			: 1;
	}

	/// <summary>
	/// The owned-set band floor for a proto default D and step
	/// band N: Max(MinAssignable, D - N). The owned set is
	/// { D - k : 0 &lt;= k &lt;= N }, clamped at MinAssignable.
	/// </summary>
	public static int BandFloor(int protoDefault, int maxNudge) {
		return Math.Max(MinAssignable, protoDefault - maxNudge);
	}

	public enum NudgeDecision {
		/// <summary>Streak below the persistence threshold — keep watching.</summary>
		NotPersistent,
		/// <summary>Per-machine cooldown since the last nudge has not elapsed.</summary>
		CoolingDown,
		/// <summary>The machine exposes no general-priority surface at all.</summary>
		NoPrioritySurface,
		/// <summary>Its cargo is not affected by general priority — a nudge would be inert.</summary>
		CargoUnaffected,
		/// <summary>Current value outside the owned band — treated as player-set, hands off.</summary>
		OutsideBand,
		/// <summary>Already at the band floor — no further step.</summary>
		AtBandFloor,
		/// <summary>The per-day action cap is reached.</summary>
		DailyCapReached,
		/// <summary>Schedule the nudge to <c>target</c> (= current - 1).</summary>
		Nudge,
	}

	/// <summary>
	/// The starved-machine nudge decision, in the shipped check
	/// order: persistence, cooldown, priority surface, cargo
	/// flag, ownership band, band floor, daily cap.
	/// <paramref name="lastNudgePass"/> uses the session
	/// bookkeeping's never-nudged sentinel (int.MinValue / 2),
	/// which never satisfies the cooldown for any real pass.
	/// </summary>
	public static NudgeDecision DecideNudge(int streak, int persistenceDays,
			int currentPass, int lastNudgePass, int cooldownDays,
			bool hasPrioritySurface, bool cargoAffected,
			int current, int protoDefault, int maxNudge,
			int actions, int dailyCap, out int target) {
		target = 0;
		if (streak < persistenceDays) {
			return NudgeDecision.NotPersistent;
		}
		if (currentPass - lastNudgePass < cooldownDays) {
			return NudgeDecision.CoolingDown;
		}
		if (!hasPrioritySurface) {
			return NudgeDecision.NoPrioritySurface;
		}
		if (!cargoAffected) {
			return NudgeDecision.CargoUnaffected;
		}
		int bandFloor = BandFloor(protoDefault, maxNudge);
		if (current > protoDefault || current < bandFloor) {
			return NudgeDecision.OutsideBand;
		}
		if (current <= bandFloor) {
			return NudgeDecision.AtBandFloor;
		}
		if (actions >= dailyCap) {
			return NudgeDecision.DailyCapReached;
		}
		target = current - 1;
		return NudgeDecision.Nudge;
	}

	public enum RestoreDecision {
		/// <summary>Not a value this mod's rule could have set (at/above the default, or below the band).</summary>
		NotOurs,
		/// <summary>Stepping back would exceed the assignable maximum.</summary>
		ExceedsAssignable,
		/// <summary>The per-day action cap is reached.</summary>
		DailyCapReached,
		/// <summary>Schedule the restore to <c>target</c> (= current + 1).</summary>
		Restore,
	}

	/// <summary>
	/// The working-machine restore decision, in the shipped check
	/// order: the stateless restore test (exactly default minus
	/// 1..N steps — at the default needs nothing, anything else
	/// is not ours), then the assignable-maximum check on the
	/// target, then the daily cap. Restores are NOT
	/// cooldown-gated, by design.
	/// </summary>
	public static RestoreDecision DecideRestore(int current, int protoDefault, int maxNudge,
			int actions, int dailyCap, out int target) {
		target = 0;
		int bandFloor = BandFloor(protoDefault, maxNudge);
		if (current >= protoDefault || current < bandFloor) {
			return RestoreDecision.NotOurs;
		}
		int t = current + 1;
		if (t > MaxAssignable) {
			return RestoreDecision.ExceedsAssignable;
		}
		if (actions >= dailyCap) {
			return RestoreDecision.DailyCapReached;
		}
		target = t;
		return RestoreDecision.Restore;
	}
}
