using System;
using System.Collections.Generic;

namespace QoSChickens;

/// <summary>
/// The chicken balancer's guard arithmetic, extracted as pure
/// functions over plain values (counts, capacities, flags) — no
/// game-assembly types — so it can be executed off-game by the
/// guard test harness (qos/build/guardtests, gated by
/// tools/check_qos_guards.sh). QUALITY-AUDIT item 5 / finding T2.
///
/// Every function here CHARACTERISES the pre-extraction behaviour
/// of ChickenBalancer exactly: the semantics were derived from the
/// shipped code first and pinned by the harness's named cases
/// (boundary values on both sides of every threshold). The
/// service calls these functions; it keeps only game-object
/// reads, logging and command scheduling.
///
/// Arrays passed to the decision functions are parallel: index i
/// is the same farm in counts / capacities / receiverCaps (and,
/// for fill-first, eligibleIds). "capacities" are the farms' raw
/// prototype capacities; "receiverCaps" are ReceiverCap() values
/// (capacity tightened by the slaughter limit). The distinction
/// matters: the shipped round-robin simulation clamps by RAW
/// capacity, and the slaughter limit is enforced afterwards by
/// GuardsSatisfied + the halving search — fail-closed by veto,
/// never by construction.
/// </summary>
public static class ChickenGuardMath {

	public enum BalanceMode {
		Off,
		Equalise,
		FillFirst,
		Unknown,
	}

	public enum BalanceOutcome {
		/// <summary>Fewer than two eligible farms: the shipped pass does nothing, silently.</summary>
		NoOp,
		/// <summary>Total is zero, or the spread is already within +/-1.</summary>
		Balanced,
		/// <summary>A spread exists but every candidate move was vetoed by the guards.</summary>
		Blocked,
		/// <summary>A guard-passing move was found; see the out parameters.</summary>
		Move,
	}

	/// <summary>
	/// Normalise the 'mode' config string exactly as the shipped
	/// pass did: null falls back to "equalise", then trim +
	/// lower-case, then match. Anything else is Unknown (the pass
	/// warns and does nothing). <paramref name="normalised"/> is
	/// the string the warning quotes.
	/// </summary>
	public static BalanceMode ParseMode(string raw, out string normalised) {
		normalised = (raw ?? "equalise").Trim().ToLowerInvariant();
		if (normalised == "off") {
			return BalanceMode.Off;
		}
		if (normalised == "equalise") {
			return BalanceMode.Equalise;
		}
		if (normalised == "fill_first") {
			return BalanceMode.FillFirst;
		}
		return BalanceMode.Unknown;
	}

	/// <summary>Daily move budget clamp: Max(0, configured).</summary>
	public static int ClampBudget(int configured) {
		return Math.Max(0, configured);
	}

	/// <summary>Breeding-nucleus floor clamp: Max(0, configured).</summary>
	public static int ClampFloor(int configured) {
		return Math.Max(0, configured);
	}

	/// <summary>Most a farm may hold under the guards: capacity, tightened by its slaughter limit.</summary>
	public static int ReceiverCap(int capacity, int? slaughterLimit) {
		int cap = capacity;
		if (slaughterLimit.HasValue && slaughterLimit.Value < cap) {
			cap = slaughterLimit.Value;
		}
		return cap;
	}

	/// <summary>
	/// The eligible-set rule over per-farm flags. A farm counts
	/// only when constructed + enabled. Returns false (skip the
	/// whole species today) when a constructed + enabled
	/// growth-paused farm exists: the shipped move command's peer
	/// set cannot exclude it, so any move would touch a farm the
	/// player told to stay put. A growth-paused farm that is not
	/// constructed + enabled triggers neither eligibility nor the
	/// skip. <paramref name="pausedIndex"/> is the first offending
	/// farm (for the log line), or -1.
	/// </summary>
	public static bool EvaluateEligibility(bool[] constructed, bool[] enabled, bool[] growthPaused,
			out int[] eligibleIndices, out int pausedIndex) {
		var list = new List<int>();
		pausedIndex = -1;
		for (int i = 0; i < constructed.Length; i++) {
			if (!constructed[i] || !enabled[i]) {
				continue;
			}
			if (growthPaused[i]) {
				pausedIndex = i;
				eligibleIndices = null;
				return false;
			}
			list.Add(i);
		}
		eligibleIndices = list.ToArray();
		return true;
	}

	/// <summary>
	/// Check a predicted post-move state against every hard guard:
	/// a farm that LOSES animals must not end below the breeding
	/// floor (a farm already below the floor may not lose any);
	/// a farm that GAINS must have been below its receiver cap
	/// (a farm at/above its slaughter limit gains zero) and must
	/// not end above it. An unchanged farm is unconstrained.
	/// </summary>
	public static bool GuardsSatisfied(int[] before, int[] after, int[] receiverCaps, int floor) {
		for (int i = 0; i < before.Length; i++) {
			int b = before[i];
			int a = after[i];
			if (a < b && a < floor) {
				return false; // drained below the breeding-nucleus floor
			}
			if (a > b) {
				int cap = receiverCaps[i];
				if (b >= cap) {
					return false; // already at/above its slaughter limit — must gain zero
				}
				if (a > cap) {
					return false; // would end above capacity / slaughter limit
				}
			}
		}
		return true;
	}

	/// <summary>
	/// Exact simulation of the shipped pull: MoveAnimalsCmd(target,
	/// +n) loops the peer array taking 1 animal per peer per pass,
	/// clamped by the target's raw capacity, until n are moved or
	/// a full pass moves nothing. Returns the predicted counts.
	/// </summary>
	public static int[] SimulatePull(int[] counts, int targetIndex, int targetCapacity,
			int n, out int moved) {
		var pred = (int[])counts.Clone();
		moved = 0;
		bool progress = true;
		while (moved < n && progress) {
			progress = false;
			for (int peer = 0; peer < pred.Length; peer++) {
				if (moved >= n) {
					break;
				}
				if (peer == targetIndex) {
					continue;
				}
				if (pred[peer] > 0 && pred[targetIndex] < targetCapacity) {
					pred[peer]--;
					pred[targetIndex]++;
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
	/// clamped by each peer's raw capacity, until n are moved or a
	/// full pass moves nothing. Returns the predicted counts.
	/// </summary>
	public static int[] SimulatePush(int[] counts, int[] capacities, int sourceIndex,
			int n, out int moved) {
		var pred = (int[])counts.Clone();
		moved = 0;
		bool progress = true;
		while (moved < n && progress) {
			progress = false;
			for (int peer = 0; peer < pred.Length; peer++) {
				if (moved >= n) {
					break;
				}
				if (peer == sourceIndex) {
					continue;
				}
				if (pred[sourceIndex] > 0 && pred[peer] < capacities[peer]) {
					pred[peer]++;
					pred[sourceIndex]--;
					moved++;
					progress = true;
				}
			}
		}
		return pred;
	}

	/// <summary>
	/// The shipped TryMove search, minus the scheduling: clamp the
	/// desired amount by the remaining daily budget, simulate, and
	/// shrink by halving the SIMULATED moved amount
	/// (desired = moved / 2) until the predicted state satisfies
	/// every guard. Returns the safe amount (0 = nothing safe to
	/// do) and, when positive, the predicted post-move counts.
	/// </summary>
	public static int FindSafeMove(int[] counts, int[] capacities, int[] receiverCaps,
			int floor, int budget, bool pull, int focusIndex, int desired,
			out int[] predicted) {
		predicted = null;
		desired = Math.Min(desired, budget);
		while (desired > 0) {
			int moved;
			int[] pred = pull
				? SimulatePull(counts, focusIndex, capacities[focusIndex], desired, out moved)
				: SimulatePush(counts, capacities, focusIndex, desired, out moved);
			if (moved <= 0) {
				return 0;
			}
			if (GuardsSatisfied(counts, pred, receiverCaps, floor)) {
				predicted = pred;
				return moved;
			}
			desired = moved / 2;
		}
		return 0;
	}

	/// <summary>
	/// The equalise decision over one species' eligible farms:
	/// converge to within +/-1 of the mean. First try pulling
	/// into the emptiest valid receiver (drains every peer
	/// evenly); if that yields nothing, push out of the fullest
	/// farm (fills every peer evenly).
	/// </summary>
	public static BalanceOutcome DecideEqualise(int[] counts, int[] capacities, int[] receiverCaps,
			int floor, int budget,
			out bool pull, out int focusIndex, out int moved, out int[] predicted) {
		pull = true;
		focusIndex = -1;
		moved = 0;
		predicted = null;
		if (counts.Length < 2) {
			return BalanceOutcome.NoOp;
		}
		int total = 0, min = int.MaxValue, max = int.MinValue;
		for (int i = 0; i < counts.Length; i++) {
			total += counts[i];
			if (counts[i] < min) min = counts[i];
			if (counts[i] > max) max = counts[i];
		}
		if (total <= 0 || max - min <= 1) {
			return BalanceOutcome.Balanced;
		}
		int target = total / counts.Length;

		// Pull into the emptiest valid receiver.
		int receiver = -1;
		for (int i = 0; i < counts.Length; i++) {
			if (counts[i] < target && counts[i] < receiverCaps[i] &&
					(receiver == -1 || counts[i] < counts[receiver])) {
				receiver = i;
			}
		}
		if (receiver != -1) {
			int m = FindSafeMove(counts, capacities, receiverCaps, floor, budget,
				true, receiver,
				Math.Min(target - counts[receiver], receiverCaps[receiver] - counts[receiver]),
				out int[] pred);
			if (m > 0) {
				pull = true;
				focusIndex = receiver;
				moved = m;
				predicted = pred;
				return BalanceOutcome.Move;
			}
		}

		// Otherwise push out of the fullest farm.
		int source = -1;
		for (int i = 0; i < counts.Length; i++) {
			if (counts[i] > target && (source == -1 || counts[i] > counts[source])) {
				source = i;
			}
		}
		if (source != -1) {
			int m = FindSafeMove(counts, capacities, receiverCaps, floor, budget,
				false, source, counts[source] - target, out int[] pred);
			if (m > 0) {
				pull = false;
				focusIndex = source;
				moved = m;
				predicted = pred;
				return BalanceOutcome.Move;
			}
		}
		return BalanceOutcome.Blocked;
	}

	/// <summary>
	/// Parse the fill_first_farm_ids config string exactly as the
	/// shipped pass did: split on commas, int.TryParse each
	/// trimmed part, skip the unparseable, keep order and dupes
	/// (dedupe happens against the eligible set in
	/// DecideFillFirst).
	/// </summary>
	public static int[] ParseTargetIds(string raw) {
		var ids = new List<int>();
		foreach (string part in (raw ?? "").Split(',')) {
			int id;
			if (int.TryParse(part.Trim(), out id)) {
				ids.Add(id);
			}
		}
		return ids.ToArray();
	}

	/// <summary>
	/// The fill-first decision: fill the named eligible farms in
	/// the configured order, each to its receiver cap; the first
	/// target that yields a guard-passing move wins.
	/// <paramref name="targetsFound"/> is false when the id list
	/// names no eligible farm (the shipped pass warns and does
	/// nothing). <paramref name="eligibleIds"/> is parallel to
	/// the count arrays and carries the farms' entity ids.
	/// </summary>
	public static bool DecideFillFirst(int[] counts, int[] capacities, int[] receiverCaps,
			int[] eligibleIds, string rawIds, int floor, int budget,
			out int focusIndex, out int moved, out int[] predicted, out bool targetsFound) {
		focusIndex = -1;
		moved = 0;
		predicted = null;
		var targets = new List<int>();
		foreach (int id in ParseTargetIds(rawIds)) {
			for (int j = 0; j < eligibleIds.Length; j++) {
				if (eligibleIds[j] == id && !targets.Contains(j)) {
					targets.Add(j);
				}
			}
		}
		targetsFound = targets.Count > 0;
		if (!targetsFound) {
			return false;
		}
		foreach (int t in targets) {
			int cap = receiverCaps[t];
			if (counts[t] >= cap) {
				continue;
			}
			int m = FindSafeMove(counts, capacities, receiverCaps, floor, budget,
				true, t, cap - counts[t], out int[] pred);
			if (m > 0) {
				focusIndex = t;
				moved = m;
				predicted = pred;
				return true;
			}
		}
		return false;
	}
}
