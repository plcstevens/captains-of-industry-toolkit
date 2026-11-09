using System;
using QoSChickens;
using QoSVehicleRecovery;
using QoSPriorityActuator;

// QoS guard-arithmetic tests (QUALITY-AUDIT item 5 / finding T2).
//
// Executes the extracted pure guard units (ChickenGuardMath,
// RecoveryGuardMath, ActuatorGuardMath — the exact sources that
// ship inside the mods) against named cases whose expected values
// were CHARACTERISED from the pre-extraction shipped code:
// boundary values on both sides of every threshold, zero/empty
// extremes, check-order pins (an earlier guard masks later ones),
// and the failure direction — every guard must fail CLOSED (no
// act) on degenerate input wherever that was the shipped
// semantics.
//
// Run via tools/check_qos_guards.sh (also wired into
// qos/build/build.sh). Exit code 0 = all cases pass.
class GuardTests {

	static int s_checks = 0;
	static int s_failures = 0;

	static void Eq(string name, object actual, object expected) {
		s_checks++;
		bool ok = Equals(actual, expected);
		if (!ok) s_failures++;
		Console.WriteLine((ok ? "PASS " : "FAIL ") + name +
			" => " + actual + (ok ? "" : " (expected " + expected + ")"));
	}

	static void Arr(string name, int[] actual, params int[] expected) {
		s_checks++;
		bool ok = actual != null && actual.Length == expected.Length;
		if (ok) {
			for (int i = 0; i < expected.Length; i++) {
				if (actual[i] != expected[i]) { ok = false; break; }
			}
		}
		if (!ok) s_failures++;
		Console.WriteLine((ok ? "PASS " : "FAIL ") + name + " => [" +
			(actual == null ? "null" : string.Join(",", actual)) + "]" +
			(ok ? "" : " (expected [" + string.Join(",", expected) + "])"));
	}

	static int Main() {
		Chickens();
		Recovery();
		Actuator();
		Console.WriteLine("----");
		Console.WriteLine(s_checks + " checks, " + s_failures + " failures");
		Console.WriteLine(s_failures == 0 ? "ALL GUARD TESTS PASS" : "GUARD TESTS FAILED");
		return s_failures == 0 ? 0 : 1;
	}

	// ----------------------------------------------------------
	// QoSChickens — ChickenGuardMath
	// ----------------------------------------------------------
	static void Chickens() {
		Console.WriteLine("== QoSChickens ==");
		string norm;

		// Mode gate (ParseMode)
		Eq("mode equalise", ChickenGuardMath.ParseMode("equalise", out norm), ChickenGuardMath.BalanceMode.Equalise);
		Eq("mode fill_first padded/cased", ChickenGuardMath.ParseMode(" Fill_First ", out norm), ChickenGuardMath.BalanceMode.FillFirst);
		Eq("mode normalised text", norm, "fill_first");
		Eq("mode off", ChickenGuardMath.ParseMode("off", out norm), ChickenGuardMath.BalanceMode.Off);
		Eq("mode OFF lowercased", ChickenGuardMath.ParseMode("OFF", out norm), ChickenGuardMath.BalanceMode.Off);
		Eq("mode unknown", ChickenGuardMath.ParseMode("banana", out norm), ChickenGuardMath.BalanceMode.Unknown);
		Eq("mode empty is unknown", ChickenGuardMath.ParseMode("", out norm), ChickenGuardMath.BalanceMode.Unknown);
		Eq("mode null defaults to equalise", ChickenGuardMath.ParseMode(null, out norm), ChickenGuardMath.BalanceMode.Equalise);

		// Config clamps
		Eq("budget clamp negative", ChickenGuardMath.ClampBudget(-5), 0);
		Eq("budget clamp zero", ChickenGuardMath.ClampBudget(0), 0);
		Eq("budget clamp value", ChickenGuardMath.ClampBudget(50), 50);
		Eq("floor clamp negative", ChickenGuardMath.ClampFloor(-1), 0);
		Eq("floor clamp value", ChickenGuardMath.ClampFloor(10), 10);

		// ReceiverCap = min(capacity, slaughter limit)
		Eq("cap no limit", ChickenGuardMath.ReceiverCap(20, null), 20);
		Eq("cap limit below", ChickenGuardMath.ReceiverCap(20, 15), 15);
		Eq("cap limit above ignored", ChickenGuardMath.ReceiverCap(20, 25), 20);
		Eq("cap limit equal", ChickenGuardMath.ReceiverCap(20, 20), 20);
		Eq("cap zero capacity", ChickenGuardMath.ReceiverCap(0, null), 0);
		Eq("cap zero limit", ChickenGuardMath.ReceiverCap(20, 0), 0);

		// Eligibility: constructed+enabled; a constructed+enabled
		// growth-paused farm skips the whole species.
		int[] elig; int paused;
		Eq("elig all eligible", ChickenGuardMath.EvaluateEligibility(
			new[] { true, true }, new[] { true, true }, new[] { false, false }, out elig, out paused), true);
		Arr("elig indices", elig, 0, 1);
		Eq("elig no paused index", paused, -1);
		Eq("elig paused skips species", ChickenGuardMath.EvaluateEligibility(
			new[] { true, true }, new[] { true, true }, new[] { false, true }, out elig, out paused), false);
		Eq("elig paused index", paused, 1);
		Eq("elig paused-but-disabled no skip", ChickenGuardMath.EvaluateEligibility(
			new[] { true, true }, new[] { true, false }, new[] { false, true }, out elig, out paused), true);
		Arr("elig paused-but-disabled indices", elig, 0);
		Eq("elig paused-but-unconstructed no skip", ChickenGuardMath.EvaluateEligibility(
			new[] { true, false }, new[] { true, true }, new[] { false, true }, out elig, out paused), true);
		Arr("elig unconstructed excluded", elig, 0);
		Eq("elig empty", ChickenGuardMath.EvaluateEligibility(
			new bool[0], new bool[0], new bool[0], out elig, out paused), true);
		Arr("elig empty indices", elig);

		// GuardsSatisfied
		Eq("guards loser to exactly floor", ChickenGuardMath.GuardsSatisfied(
			new[] { 10, 10 }, new[] { 5, 15 }, new[] { 30, 30 }, 5), true);
		Eq("guards loser below floor vetoed", ChickenGuardMath.GuardsSatisfied(
			new[] { 10, 10 }, new[] { 4, 15 }, new[] { 30, 30 }, 5), false);
		Eq("guards gainer to exactly cap", ChickenGuardMath.GuardsSatisfied(
			new[] { 10, 10 }, new[] { 10, 20 }, new[] { 30, 20 }, 0), true);
		Eq("guards gainer above cap vetoed", ChickenGuardMath.GuardsSatisfied(
			new[] { 10, 10 }, new[] { 10, 21 }, new[] { 30, 20 }, 0), false);
		Eq("guards at-cap farm gains zero", ChickenGuardMath.GuardsSatisfied(
			new[] { 20, 10 }, new[] { 21, 9 }, new[] { 20, 30 }, 0), false);
		Eq("guards above-cap farm gains zero", ChickenGuardMath.GuardsSatisfied(
			new[] { 25, 10 }, new[] { 26, 9 }, new[] { 20, 30 }, 0), false);
		Eq("guards at-cap farm unchanged passes", ChickenGuardMath.GuardsSatisfied(
			new[] { 20, 10 }, new[] { 20, 11 }, new[] { 20, 30 }, 0), true);
		Eq("guards unchanged below-floor farm unconstrained", ChickenGuardMath.GuardsSatisfied(
			new[] { 3, 20 }, new[] { 3, 10 }, new[] { 30, 30 }, 10), true);
		Eq("guards below-floor farm may not lose any", ChickenGuardMath.GuardsSatisfied(
			new[] { 5, 20 }, new[] { 3, 20 }, new[] { 30, 30 }, 10), false);
		Eq("guards below-floor farm may gain", ChickenGuardMath.GuardsSatisfied(
			new[] { 3, 20 }, new[] { 8, 15 }, new[] { 30, 30 }, 10), true);
		Eq("guards empty state", ChickenGuardMath.GuardsSatisfied(
			new int[0], new int[0], new int[0], 10), true);

		// SimulatePull (round-robin, target clamped by RAW capacity)
		int moved; int[] pred;
		pred = ChickenGuardMath.SimulatePull(new[] { 10, 0, 10 }, 1, 30, 5, out moved);
		Eq("pull basic moved", moved, 5);
		Arr("pull basic pred", pred, 7, 5, 8);
		pred = ChickenGuardMath.SimulatePull(new[] { 10, 28 }, 1, 30, 10, out moved);
		Eq("pull target raw-capacity clamp moved", moved, 2);
		Arr("pull target raw-capacity clamp pred", pred, 8, 30);
		pred = ChickenGuardMath.SimulatePull(new[] { 0, 5 }, 1, 30, 3, out moved);
		Eq("pull no stock moved", moved, 0);
		Arr("pull no stock pred", pred, 0, 5);
		pred = ChickenGuardMath.SimulatePull(new[] { 10, 0 }, 1, 30, 0, out moved);
		Eq("pull n=0 moved", moved, 0);
		pred = ChickenGuardMath.SimulatePull(new[] { 4, 4, 0 }, 2, 100, 3, out moved);
		Eq("pull peer order moved", moved, 3);
		Arr("pull peer order pred", pred, 2, 3, 3);

		// SimulatePush (round-robin, peers clamped by their raw capacity)
		pred = ChickenGuardMath.SimulatePush(new[] { 10, 0, 0 }, new[] { 30, 5, 5 }, 0, 6, out moved);
		Eq("push basic moved", moved, 6);
		Arr("push basic pred", pred, 4, 3, 3);
		pred = ChickenGuardMath.SimulatePush(new[] { 10, 4 }, new[] { 30, 5 }, 0, 5, out moved);
		Eq("push peer capacity clamp moved", moved, 1);
		Arr("push peer capacity clamp pred", pred, 9, 5);
		pred = ChickenGuardMath.SimulatePush(new[] { 0, 0 }, new[] { 30, 30 }, 0, 3, out moved);
		Eq("push empty source moved", moved, 0);
		pred = ChickenGuardMath.SimulatePush(new[] { 5 }, new[] { 30 }, 0, 3, out moved);
		Eq("push no peers moved", moved, 0);

		// FindSafeMove (budget clamp + halving search over the guards)
		int m = ChickenGuardMath.FindSafeMove(new[] { 20, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			0, 50, true, 1, 10, out pred);
		Eq("safe basic moved", m, 10);
		Arr("safe basic pred", pred, 10, 10);
		m = ChickenGuardMath.FindSafeMove(new[] { 20, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			0, 3, true, 1, 10, out pred);
		Eq("safe budget clamp moved", m, 3);
		Arr("safe budget clamp pred", pred, 17, 3);
		m = ChickenGuardMath.FindSafeMove(new[] { 20, 0 }, new[] { 30, 30 }, new[] { 30, 4 },
			0, 50, true, 1, 10, out pred);
		Eq("safe receiver-cap halving moved", m, 2);
		Arr("safe receiver-cap halving pred", pred, 18, 2);
		m = ChickenGuardMath.FindSafeMove(new[] { 3, 0 }, new[] { 30, 30 }, new[] { 30, 2 },
			0, 50, true, 1, 10, out pred);
		Eq("safe halving uses simulated moved", m, 1);
		Arr("safe halving uses simulated moved pred", pred, 2, 1);
		m = ChickenGuardMath.FindSafeMove(new[] { 12, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			10, 50, true, 1, 5, out pred);
		Eq("safe floor veto converges to floor", m, 2);
		Arr("safe floor veto pred", pred, 10, 2);
		m = ChickenGuardMath.FindSafeMove(new[] { 11, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			11, 50, true, 1, 1, out pred);
		Eq("safe floor veto to zero fails closed", m, 0);
		m = ChickenGuardMath.FindSafeMove(new[] { 0, 5 }, new[] { 30, 30 }, new[] { 30, 30 },
			0, 50, true, 1, 3, out pred);
		Eq("safe no stock", m, 0);
		m = ChickenGuardMath.FindSafeMove(new[] { 20, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			0, 50, true, 1, 0, out pred);
		Eq("safe desired zero", m, 0);
		m = ChickenGuardMath.FindSafeMove(new[] { 20, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			0, 0, true, 1, 10, out pred);
		Eq("safe budget zero", m, 0);
		m = ChickenGuardMath.FindSafeMove(new[] { 20, 0 }, new[] { 30, 30 }, new[] { 30, 30 },
			0, 50, false, 0, 10, out pred);
		Eq("safe push moved", m, 10);
		Arr("safe push pred", pred, 10, 10);

		// DecideEqualise
		bool pull; int focus;
		Eq("equalise single farm noop", ChickenGuardMath.DecideEqualise(
			new[] { 10 }, new[] { 30 }, new[] { 30 }, 0, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.NoOp);
		Eq("equalise empty noop", ChickenGuardMath.DecideEqualise(
			new int[0], new int[0], new int[0], 0, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.NoOp);
		Eq("equalise spread 1 balanced", ChickenGuardMath.DecideEqualise(
			new[] { 10, 11 }, new[] { 30, 30 }, new[] { 30, 30 }, 0, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.Balanced);
		Eq("equalise total zero balanced", ChickenGuardMath.DecideEqualise(
			new[] { 0, 0, 0 }, new[] { 30, 30, 30 }, new[] { 30, 30, 30 }, 0, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.Balanced);
		Eq("equalise pull outcome", ChickenGuardMath.DecideEqualise(
			new[] { 0, 10, 11 }, new[] { 30, 30, 30 }, new[] { 30, 30, 30 }, 0, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.Move);
		Eq("equalise pull is pull", pull, true);
		Eq("equalise pull focus", focus, 0);
		Eq("equalise pull moved", m, 7);
		Arr("equalise pull pred", pred, 7, 6, 8);
		Eq("equalise falls back to push", ChickenGuardMath.DecideEqualise(
			new[] { 0, 3, 20 }, new[] { 30, 30, 30 }, new[] { 30, 30, 30 }, 10, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.Move);
		Eq("equalise fallback is push", pull, false);
		Eq("equalise fallback focus", focus, 2);
		Eq("equalise fallback moved", m, 6);
		Arr("equalise fallback pred", pred, 3, 6, 14);
		Eq("equalise slaughter-cap farm can never gain: blocked", ChickenGuardMath.DecideEqualise(
			new[] { 5, 20 }, new[] { 30, 30 }, new[] { 5, 30 }, 0, 50, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.Blocked);
		Eq("equalise respects budget", ChickenGuardMath.DecideEqualise(
			new[] { 0, 20 }, new[] { 30, 30 }, new[] { 30, 30 }, 0, 4, out pull, out focus, out m, out pred),
			ChickenGuardMath.BalanceOutcome.Move);
		Eq("equalise budget moved", m, 4);

		// ParseTargetIds + DecideFillFirst
		Arr("fill parse skips unparseable keeps dupes", ChickenGuardMath.ParseTargetIds("3, abc, 5, 3"), 3, 5, 3);
		Arr("fill parse empty", ChickenGuardMath.ParseTargetIds(""));
		Arr("fill parse trims", ChickenGuardMath.ParseTargetIds(" 7 , 8 "), 7, 8);
		Arr("fill parse negative parses", ChickenGuardMath.ParseTargetIds("-3"), -3);
		bool found;
		Eq("fill no targets named", ChickenGuardMath.DecideFillFirst(
			new[] { 10, 0 }, new[] { 30, 30 }, new[] { 30, 30 }, new[] { 1, 2 }, "", 0, 50,
			out focus, out m, out pred, out found), false);
		Eq("fill no targets found flag", found, false);
		Eq("fill unknown id only", ChickenGuardMath.DecideFillFirst(
			new[] { 10, 0 }, new[] { 30, 30 }, new[] { 30, 30 }, new[] { 1, 2 }, "99", 0, 50,
			out focus, out m, out pred, out found), false);
		Eq("fill unknown id found flag", found, false);
		Eq("fill basic", ChickenGuardMath.DecideFillFirst(
			new[] { 10, 0 }, new[] { 30, 30 }, new[] { 30, 30 }, new[] { 1, 2 }, "2", 0, 50,
			out focus, out m, out pred, out found), true);
		Eq("fill basic focus", focus, 1);
		Eq("fill basic moved", m, 10);
		Arr("fill basic pred", pred, 0, 10);
		Eq("fill skips at-cap target", ChickenGuardMath.DecideFillFirst(
			new[] { 10, 30 }, new[] { 30, 30 }, new[] { 30, 30 }, new[] { 1, 2 }, "2,1", 0, 50,
			out focus, out m, out pred, out found), true);
		Eq("fill skips at-cap focus", focus, 0);
		Eq("fill skips at-cap moved", m, 20);
		Eq("fill sizes by receiver cap not raw cap", ChickenGuardMath.DecideFillFirst(
			new[] { 10, 18 }, new[] { 30, 30 }, new[] { 30, 20 }, new[] { 1, 2 }, "2", 0, 50,
			out focus, out m, out pred, out found), true);
		Eq("fill receiver-cap moved", m, 2);
		Arr("fill receiver-cap pred", pred, 8, 20);
		Eq("fill all targets full", ChickenGuardMath.DecideFillFirst(
			new[] { 30, 30 }, new[] { 30, 30 }, new[] { 30, 30 }, new[] { 1, 2 }, "1,2", 0, 50,
			out focus, out m, out pred, out found), false);
		Eq("fill all full found flag", found, true);
	}

	// ----------------------------------------------------------
	// QoSVehicleRecovery — RecoveryGuardMath
	// ----------------------------------------------------------
	static void Recovery() {
		Console.WriteLine("== QoSVehicleRecovery ==");

		// Config clamps
		Eq("streak clamp zero", RecoveryGuardMath.ClampMinStreak(0), 1);
		Eq("streak clamp negative", RecoveryGuardMath.ClampMinStreak(-3), 1);
		Eq("streak clamp boundary", RecoveryGuardMath.ClampMinStreak(1), 1);
		Eq("streak clamp value", RecoveryGuardMath.ClampMinStreak(3), 3);
		Eq("cap clamp negative", RecoveryGuardMath.ClampDailyCap(-1), 0);
		Eq("cap clamp zero", RecoveryGuardMath.ClampDailyCap(0), 0);
		Eq("cap clamp value", RecoveryGuardMath.ClampDailyCap(5), 5);
		Eq("reserve clamp negative", RecoveryGuardMath.ClampReserveFloor(-0.5), 0.0);
		Eq("reserve clamp value", RecoveryGuardMath.ClampReserveFloor(5.0), 5.0);

		// Hands-off gate
		Eq("handsoff none", RecoveryGuardMath.IsHandsOff(false, false, false, false), false);
		Eq("handsoff being recovered", RecoveryGuardMath.IsHandsOff(true, false, false, false), true);
		Eq("handsoff scrap", RecoveryGuardMath.IsHandsOff(false, true, false, false), true);
		Eq("handsoff replacement", RecoveryGuardMath.IsHandsOff(false, false, true, false), true);
		Eq("handsoff replace queued", RecoveryGuardMath.IsHandsOff(false, false, false, true), true);

		// Sustained-stuck signal
		Eq("navstuck repeated flag alone", RecoveryGuardMath.IsNavStuck(true, false, 0, 3), true);
		Eq("navstuck streak exactly at threshold", RecoveryGuardMath.IsNavStuck(false, true, 3, 3), true);
		Eq("navstuck streak below threshold", RecoveryGuardMath.IsNavStuck(false, true, 2, 3), false);
		Eq("navstuck streak without stuck", RecoveryGuardMath.IsNavStuck(false, false, 99, 3), false);
		Eq("navstuck zero streak min 1", RecoveryGuardMath.IsNavStuck(false, true, 0, 1), false);

		// Stage-1 wanted disjunction
		Eq("wanted cannot-deliver", RecoveryGuardMath.IsStage1Wanted(true, true, false, false), true);
		Eq("wanted cannot-deliver disabled", RecoveryGuardMath.IsStage1Wanted(true, false, false, false), false);
		Eq("wanted cd disabled but nav ok", RecoveryGuardMath.IsStage1Wanted(true, false, true, true), true);
		Eq("wanted nav only", RecoveryGuardMath.IsStage1Wanted(false, false, true, true), true);
		Eq("wanted nav disabled", RecoveryGuardMath.IsStage1Wanted(false, true, true, false), false);
		Eq("wanted nothing", RecoveryGuardMath.IsStage1Wanted(false, false, false, false), false);

		// Stage-2 eligibility (same load, marked, enabled)
		Eq("stage2 eligible", RecoveryGuardMath.IsStage2Eligible(true, true, true, true, "A", "A"), true);
		Eq("stage2 different load", RecoveryGuardMath.IsStage2Eligible(true, true, true, true, "A", "B"), false);
		Eq("stage2 feature off", RecoveryGuardMath.IsStage2Eligible(false, true, true, true, "A", "A"), false);
		Eq("stage2 not flagged", RecoveryGuardMath.IsStage2Eligible(true, false, true, true, "A", "A"), false);
		Eq("stage2 truck empty", RecoveryGuardMath.IsStage2Eligible(true, true, false, true, "A", "A"), false);
		Eq("stage2 no mark", RecoveryGuardMath.IsStage2Eligible(true, true, true, false, null, "A"), false);

		// Mark expiry: valid through mark+7, expired from mark+8.
		Eq("mark exactly 7 passes old lives", RecoveryGuardMath.IsMarkExpired(10, 3), false);
		Eq("mark 8 passes old expires", RecoveryGuardMath.IsMarkExpired(10, 2), true);
		Eq("mark fresh", RecoveryGuardMath.IsMarkExpired(3, 3), false);
		Eq("mark negative age", RecoveryGuardMath.IsMarkExpired(2, 5), false);
		Eq("mark lifetime constant", RecoveryGuardMath.MarkLifetimePasses, 7);

		// Daily cap (stage 1)
		Eq("cap below", RecoveryGuardMath.WithinDailyCap(4, 5), true);
		Eq("cap exactly at fails closed", RecoveryGuardMath.WithinDailyCap(5, 5), false);
		Eq("cap zero cap nothing fits", RecoveryGuardMath.WithinDailyCap(0, 0), false);
		Eq("cap zero used", RecoveryGuardMath.WithinDailyCap(0, 5), true);
		Eq("cap above", RecoveryGuardMath.WithinDailyCap(6, 5), false);

		// Unity reserve composition + decision (Fix32 raw units;
		// 5.0 Upoints = 327680 raw, 0.2 = 13107 raw as shipped
		// constants land — the composition is plain raw addition,
		// identical to Upoints.+ as verified in the assemblies).
		Eq("required raw composition", RecoveryGuardMath.RequiredRaw(327680, 13107), 340787);
		Eq("required raw zero cost", RecoveryGuardMath.RequiredRaw(327680, 0), 327680);
		Eq("required raw zeros", RecoveryGuardMath.RequiredRaw(0, 0), 0);
		Eq("reserve balance exactly at floor+cost allows", RecoveryGuardMath.ReserveAllows(340787, 327680, 13107), true);
		Eq("reserve one raw unit short fails closed", RecoveryGuardMath.ReserveAllows(340786, 327680, 13107), false);
		Eq("reserve zero everything allows", RecoveryGuardMath.ReserveAllows(0, 0, 0), true);
		Eq("reserve balance below floor alone", RecoveryGuardMath.ReserveAllows(100, 200, 0), false);
	}

	// ----------------------------------------------------------
	// QoSPriorityActuator — ActuatorGuardMath
	// ----------------------------------------------------------
	static void Actuator() {
		Console.WriteLine("== QoSPriorityActuator ==");
		const int Never = int.MinValue / 2; // the session sentinel for "never nudged"

		// Config clamps
		Eq("persistence clamp zero", ActuatorGuardMath.ClampPersistenceDays(0), 1);
		Eq("persistence clamp negative", ActuatorGuardMath.ClampPersistenceDays(-2), 1);
		Eq("persistence clamp value", ActuatorGuardMath.ClampPersistenceDays(3), 3);
		Eq("maxnudge clamp negative", ActuatorGuardMath.ClampMaxNudge(-1), 0);
		Eq("maxnudge clamp value", ActuatorGuardMath.ClampMaxNudge(2), 2);
		Eq("cooldown clamp negative", ActuatorGuardMath.ClampCooldownDays(-1), 0);
		Eq("cooldown clamp value", ActuatorGuardMath.ClampCooldownDays(2), 2);
		Eq("cap clamp negative", ActuatorGuardMath.ClampDailyCap(-1), 0);
		Eq("cap clamp value", ActuatorGuardMath.ClampDailyCap(10), 10);

		// Parked predicate
		Eq("parked paused", ActuatorGuardMath.IsParked(true, true, false), true);
		Eq("parked disabled", ActuatorGuardMath.IsParked(false, false, false), true);
		Eq("parked paused state", ActuatorGuardMath.IsParked(false, true, true), true);
		Eq("parked none", ActuatorGuardMath.IsParked(false, true, false), false);

		// Streak recurrence
		Eq("streak consecutive increments", ActuatorGuardMath.NextStreak(2, 4, 5), 3);
		Eq("streak gap restarts", ActuatorGuardMath.NextStreak(2, 3, 5), 1);
		Eq("streak zero previous restarts", ActuatorGuardMath.NextStreak(0, 4, 5), 1);
		Eq("streak from pass zero", ActuatorGuardMath.NextStreak(5, 0, 1), 6);
		Eq("streak two-pass gap restarts", ActuatorGuardMath.NextStreak(5, 0, 2), 1);

		// Band floor = Max(0, D - N)
		Eq("band floor basic", ActuatorGuardMath.BandFloor(9, 2), 7);
		Eq("band floor zero band", ActuatorGuardMath.BandFloor(9, 0), 9);
		Eq("band floor clamps at zero", ActuatorGuardMath.BandFloor(1, 2), 0);
		Eq("band floor default zero", ActuatorGuardMath.BandFloor(0, 2), 0);
		Eq("band floor high default", ActuatorGuardMath.BandFloor(14, 6), 8);
		Eq("band floor oversize band", ActuatorGuardMath.BandFloor(9, 20), 0);

		// DecideNudge — defaults D=9, N=2 (floor 7), persistence 3,
		// cooldown 2, cap 10, surface + cargo on.
		int target;
		Eq("nudge streak below persistence", ActuatorGuardMath.DecideNudge(
			2, 3, 10, Never, 2, true, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.NotPersistent);
		Eq("nudge streak exactly at persistence acts", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.Nudge);
		Eq("nudge at default targets default-1", target, 8);
		Eq("nudge cooldown one day short", ActuatorGuardMath.DecideNudge(
			5, 3, 10, 9, 2, true, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.CoolingDown);
		Eq("nudge cooldown exactly elapsed acts", ActuatorGuardMath.DecideNudge(
			5, 3, 10, 8, 2, true, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.Nudge);
		Eq("nudge zero cooldown never blocks", ActuatorGuardMath.DecideNudge(
			3, 3, 10, 10, 0, true, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.Nudge);
		Eq("nudge no priority surface", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, false, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.NoPrioritySurface);
		Eq("nudge cargo unaffected", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, false, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.CargoUnaffected);
		Eq("nudge order: streak masks cooldown", ActuatorGuardMath.DecideNudge(
			2, 3, 10, 9, 2, true, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.NotPersistent);
		Eq("nudge order: cooldown masks surface", ActuatorGuardMath.DecideNudge(
			5, 3, 10, 9, 2, false, true, 9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.CoolingDown);
		Eq("nudge order: surface masks band", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, false, true, 12, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.NoPrioritySurface);
		Eq("nudge above default is player-set", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 10, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.OutsideBand);
		Eq("nudge below band is player-set", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 6, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.OutsideBand);
		Eq("nudge order: band masks cap", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 6, 9, 2, 10, 10, out target),
			ActuatorGuardMath.NudgeDecision.OutsideBand);
		Eq("nudge at band floor stops", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 7, 9, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.AtBandFloor);
		Eq("nudge daily cap reached", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 8, 9, 2, 10, 10, out target),
			ActuatorGuardMath.NudgeDecision.DailyCapReached);
		Eq("nudge one below cap acts", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 8, 9, 2, 9, 10, out target),
			ActuatorGuardMath.NudgeDecision.Nudge);
		Eq("nudge one below cap target", target, 7);
		Eq("nudge degenerate default zero", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 0, 0, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.AtBandFloor);
		Eq("nudge zero band never nudges", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 9, 9, 0, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.AtBandFloor);
		Eq("nudge high default steps", ActuatorGuardMath.DecideNudge(
			3, 3, 10, Never, 2, true, true, 13, 14, 2, 0, 10, out target),
			ActuatorGuardMath.NudgeDecision.Nudge);
		Eq("nudge high default target", target, 12);

		// DecideRestore — D=9, N=2 (floor 7) unless stated.
		Eq("restore at default not ours", ActuatorGuardMath.DecideRestore(9, 9, 2, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.NotOurs);
		Eq("restore above default not ours", ActuatorGuardMath.DecideRestore(10, 9, 2, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.NotOurs);
		Eq("restore below band not ours", ActuatorGuardMath.DecideRestore(6, 9, 2, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.NotOurs);
		Eq("restore one step", ActuatorGuardMath.DecideRestore(8, 9, 2, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.Restore);
		Eq("restore one step target", target, 9);
		Eq("restore from band floor", ActuatorGuardMath.DecideRestore(7, 9, 2, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.Restore);
		Eq("restore from band floor target", target, 8);
		Eq("restore daily cap", ActuatorGuardMath.DecideRestore(8, 9, 2, 10, 10, out target),
			ActuatorGuardMath.RestoreDecision.DailyCapReached);
		Eq("restore order: band masks cap", ActuatorGuardMath.DecideRestore(10, 9, 2, 10, 10, out target),
			ActuatorGuardMath.RestoreDecision.NotOurs);
		Eq("restore order: exceeds masks cap", ActuatorGuardMath.DecideRestore(14, 16, 2, 10, 10, out target),
			ActuatorGuardMath.RestoreDecision.ExceedsAssignable);
		Eq("restore target at assignable max allowed", ActuatorGuardMath.DecideRestore(13, 14, 2, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.Restore);
		Eq("restore target at max value", target, 14);
		Eq("restore zero band restores nothing", ActuatorGuardMath.DecideRestore(8, 9, 0, 0, 10, out target),
			ActuatorGuardMath.RestoreDecision.NotOurs);
	}
}
