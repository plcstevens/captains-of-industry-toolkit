#!/usr/bin/env python3
"""plan_check.py — DRY-RUN plan evaluator for CoI game plans (Format v1).

Reads a plan YAML (GAME-PLAN-FORMAT.md) plus the local state captures in
live/ and produces a markdown dry-run report: every phase's entry gate,
exit gate and checks evaluated with the format's three-valued logic
(TRUE / FALSE / UNEVALUABLE, §4.4), the active phase derived per the
§9 play-loop rules, and the "next orders" that WOULD be prepared for
the active phase.

HARD SAFETY: this script is read-only over local files. It imports no
HTTP / socket / subprocess machinery, sends nothing anywhere, and has
no code path that can prepare, confirm or apply a game command. Every
report it emits carries a "DRY RUN — nothing was executed" banner.

Why Python and not a Rust `coi plan-check` subcommand: the coi crate's
Cargo.toml has no YAML dependency and serde_yaml is not present in the
local cargo registry cache, so a Rust build cannot parse YAML offline.
PyYAML 6.0.1 is installed. (See the chunk report for details.)

Usage:
    python3 tools/plan_check.py <plan.yaml> [--live-dir live/] [--out report.md]

State-source mapping (plan predicate naming -> capture files):
    product.quantity/stored/storage_capacity/produced/consumed/net_flow
        live/stats.json        products[<Product_id>].{quantity,stored,
                               storageCapacity,produced[],consumed[]}
                               window index: thisMonth=0, lastMonth=1,
                               thisYear=2, lastYear=3, lifetime=4
    product.unlocked           live/products.json  products[].isUnlocked
    product.pinned             live/pinned.json    membership of the
                               products dict keys (the pinned=true view
                               was captured in stats shape)
    game.paused/speed/date     live/game-info.json {isPaused,speed,date}
    settlement.*               live/bridge-status.json (direct fields)
    research.*                 live/bridge-research.json {current,queue,
                               researchedCount,hasActiveLab};
                               progressPerc is percent x1000.
                               NOTE: the captures predate CoiBridge
                               v0.6.2 — there is NO `researched` list,
                               so research.completed is UNEVALUABLE.
    census.*                   live/bridge-census.json {byProto,machines,
                               transports}
    storage.*                  live/bridge-storages.json storages[];
                               percent = percentFullRaw/1000, aggregated
                               capacity-weighted over matching rows
    trains.line_count          live/bridge-trains.json len(lines)
    power.* / contracts.* / loans.* / edicts.* / time.*
                               no capture exists -> UNEVALUABLE
    (live/snapshot-388-2-27.json bundles game-info/products/stats/
     pinned only — no bridge routes — so the individual capture files
     above are the state source.)
"""

import argparse
import json
import pathlib
import sys

import yaml

TRUE = "TRUE"
FALSE = "FALSE"
UNEVALUABLE = "UNEVALUABLE"

WINDOWS = {"thisMonth": 0, "lastMonth": 1, "thisYear": 2,
           "lastYear": 3, "lifetime": 4}

NON_COMMAND_KINDS = {"place-blueprint", "manual-step", "observe-only", "wait"}


class Unevaluable(Exception):
    """Raised inside a leaf evaluation; converted to UNEVALUABLE."""


# ---------------------------------------------------------------- state

def _load_json(path):
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise Unevaluable(f"capture {path.name} unreadable: {exc}")


class State:
    def __init__(self, live_dir):
        self.live_dir = live_dir
        self.game_info = _load_json(live_dir / "game-info.json") or {}
        stats = _load_json(live_dir / "stats.json") or {}
        self.stats = stats.get("products", {}) if isinstance(stats, dict) else {}
        catalogue = _load_json(live_dir / "products.json") or {}
        plist = catalogue.get("products", []) if isinstance(catalogue, dict) else []
        self.catalogue = {p["id"]: p for p in plist if isinstance(p, dict) and "id" in p}
        pinned = _load_json(live_dir / "pinned.json") or {}
        self.pinned = set((pinned.get("products") or {}).keys()) \
            if isinstance(pinned, dict) else set()
        self.status = _load_json(live_dir / "bridge-status.json") or {}
        self.research = _load_json(live_dir / "bridge-research.json") or {}
        self.census = _load_json(live_dir / "bridge-census.json") or {}
        storages = _load_json(live_dir / "bridge-storages.json") or {}
        self.storages = storages.get("storages", []) \
            if isinstance(storages, dict) else []
        trains = _load_json(live_dir / "bridge-trains.json") or {}
        self.train_lines = trains.get("lines") if isinstance(trains, dict) else None


def load_sequence(seq_dir):
    """Load a §4.3.1 state sequence from a directory.

    Accepts either a directory that itself holds game-info.json (a
    1-snapshot sequence) or a directory whose subdirectories are
    per-snapshot state dirs. Snapshots are sorted by in-game date.
    Returns (states, error) — exactly one of the two is set."""
    if not seq_dir.exists():
        return None, f"clean error: state sequence dir not found: {seq_dir}"
    if (seq_dir / "game-info.json").exists():
        dirs = [seq_dir]
    else:
        dirs = sorted(d for d in seq_dir.iterdir()
                      if d.is_dir() and (d / "game-info.json").exists())
    if not dirs:
        return None, (f"clean error: no snapshot state dirs (each holding "
                      f"game-info.json) found under {seq_dir}")
    states = [State(d) for d in dirs]
    try:
        states.sort(key=lambda s: _abs_day(s.game_info))
    except Unevaluable as exc:
        return None, f"clean error: sequence snapshot date unusable: {exc}"
    ids = {s.game_info.get("gameId") for s in states}
    if len(ids) > 1:
        return None, (f"clean error: state sequence mixes gameIds {ids} — "
                      f"a §4.3.1 sequence is one run (one gameId)")
    return states, None


# ---------------------------------------------------------------- results

class Result:
    def __init__(self, value, evidence):
        self.value = value          # TRUE / FALSE / UNEVALUABLE
        self.evidence = evidence    # concrete numbers / reason string

    def __repr__(self):
        return f"{self.value} ({self.evidence})"


def compare(actual, op, expected):
    """Return bool, or raise Unevaluable if the comparison is ill-typed."""
    if op is None:
        op = "=="
    if expected is None and op in ("==", "!="):
        eq = actual is None
        return eq if op == "==" else not eq
    # `none` sentinel for research.current (YAML writes it as the string)
    if isinstance(expected, str) and expected == "none":
        eq = actual is None
        if op == "==":
            return eq
        if op == "!=":
            return not eq
        raise Unevaluable("ordering comparison against 'none'")
    if isinstance(actual, bool) or isinstance(expected, bool):
        if not (isinstance(actual, bool) and isinstance(expected, bool)):
            # bool vs non-bool equality is still well-defined
            if op == "==":
                return actual == expected
            if op == "!=":
                return actual != expected
            raise Unevaluable(f"cannot order-compare {actual!r} {op} {expected!r}")
        if op == "==":
            return actual == expected
        if op == "!=":
            return actual != expected
        raise Unevaluable(f"cannot order-compare booleans {op}")
    if op == "==":
        return actual == expected
    if op == "!=":
        return actual != expected
    try:
        if op == "<":
            return actual < expected
        if op == "<=":
            return actual <= expected
        if op == ">":
            return actual > expected
        if op == ">=":
            return actual >= expected
    except TypeError:
        raise Unevaluable(f"cannot compare {actual!r} {op} {expected!r}")
    raise Unevaluable(f"unknown operator {op!r}")


def _parse_date(s):
    parts = str(s).split("-")
    if len(parts) != 3:
        raise Unevaluable(f"unparseable in-game date {s!r}")
    try:
        return tuple(int(p) for p in parts)
    except ValueError:
        raise Unevaluable(f"unparseable in-game date {s!r}")


def _node_name(node):
    if isinstance(node, dict):
        if "placeholder" in node:
            return None, node.get("placeholder")
        return None, repr(node)
    return node, None


# ---------------------------------------------------------------- leaves

def eval_leaf(pred, st):
    """Evaluate a leaf predicate dict. Returns Result. Never raises."""
    metric = pred.get("metric")
    op = pred.get("op", "==")
    expected = pred.get("value", True)
    has_op_value = "op" in pred or "value" in pred
    if not has_op_value:
        op, expected = "==", True
    try:
        if metric is None:
            raise Unevaluable("predicate has no 'metric' key")

        # ---- products -------------------------------------------------
        if metric.startswith("product."):
            pid = pred.get("product")
            if not isinstance(pid, str):
                raise Unevaluable(f"{metric}: product selector missing/not a string")
            if metric == "product.unlocked":
                row = st.catalogue.get(pid)
                if row is None:
                    raise Unevaluable(f"{pid} not in products.json catalogue")
                actual = row.get("isUnlocked")
                if actual is None:
                    raise Unevaluable(f"{pid} has no isUnlocked in catalogue")
                return Result(TRUE if compare(actual, op, expected) else FALSE,
                              f"{pid}.isUnlocked = {actual} (products.json)")
            if metric == "product.pinned":
                if not st.pinned:
                    raise Unevaluable("pinned.json capture empty/missing")
                actual = pid in st.pinned
                return Result(TRUE if compare(actual, op, expected) else FALSE,
                              f"{pid} in pinned set = {actual} "
                              f"(pinned.json lists {len(st.pinned)} pinned products)")
            row = st.stats.get(pid)
            if row is None:
                raise Unevaluable(f"{pid} not in stats.json products "
                                  f"(selector matched zero rows)")
            if metric == "product.quantity":
                actual = row["quantity"]
                ev = f"{pid}.quantity = {actual}"
            elif metric == "product.stored":
                actual = row["stored"]
                ev = f"{pid}.stored = {actual} (storageCapacity {row.get('storageCapacity')})"
            elif metric == "product.storage_capacity":
                actual = row["storageCapacity"]
                ev = f"{pid}.storageCapacity = {actual}"
            elif metric in ("product.produced", "product.consumed", "product.net_flow"):
                window = pred.get("window")
                if window not in WINDOWS:
                    raise Unevaluable(f"{metric}: bad/missing window {window!r}")
                idx = WINDOWS[window]
                prod = row["produced"][idx]
                cons = row["consumed"][idx]
                if metric == "product.produced":
                    actual = prod
                elif metric == "product.consumed":
                    actual = cons
                else:
                    actual = prod - cons
                ev = (f"{pid} window={window}: produced {prod} - consumed {cons} "
                      f"= net {prod - cons}" if metric == "product.net_flow"
                      else f"{pid} {metric.split('.')[1]}[{window}] = {actual}")
            else:
                raise Unevaluable(f"metric {metric!r} not in §4.1 vocabulary / "
                                  f"no capture source (closed-vocabulary violation)")
            return Result(TRUE if compare(actual, op, expected) else FALSE, ev)

        # ---- game -----------------------------------------------------
        if metric == "game.paused":
            if "isPaused" not in st.game_info:
                raise Unevaluable("game-info.json has no isPaused")
            actual = st.game_info["isPaused"]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"isPaused = {actual}")
        if metric == "game.speed":
            if "speed" not in st.game_info:
                raise Unevaluable("game-info.json has no speed")
            actual = st.game_info["speed"]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"speed = {actual}")
        if metric == "game.date":
            if "date" not in st.game_info:
                raise Unevaluable("game-info.json has no date")
            actual = _parse_date(st.game_info["date"])
            exp = _parse_date(expected)
            return Result(TRUE if compare(actual, op, exp) else FALSE,
                          f"date = {st.game_info['date']}")

        # ---- settlement -----------------------------------------------
        settle = {
            "settlement.population": "population",
            "settlement.housing_capacity": "totalHousingCapacity",
            "settlement.free_housing": "freeHousingCapacity",
            "settlement.homeless": "numberOfHomeless",
            "settlement.months_of_food": "monthsOfFood",
            "settlement.starving": "arePeopleStarving",
        }
        if metric in settle:
            field = settle[metric]
            if field not in st.status:
                raise Unevaluable(f"bridge-status.json has no {field}")
            actual = st.status[field]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"{field} = {actual} (bridge-status.json)")

        # ---- research -------------------------------------------------
        if metric == "research.completed":
            node, placeholder = _node_name(pred.get("node"))
            if placeholder is not None:
                raise Unevaluable(
                    f"node is a §4.5 placeholder ({placeholder}) — manual-only, "
                    f"never auto-evaluated")
            if node is None:
                raise Unevaluable("research.completed: node selector missing")
            if "researched" not in st.research:
                raise Unevaluable(
                    "bridge-research.json (pre-v0.6.2 capture) has no "
                    "`researched` completed-node list — format G1; "
                    "executor-observed completions are not in the captures")
            actual = node in st.research["researched"]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"{node} in researched list = {actual}")
        if metric == "research.current":
            cur = st.research.get("current")
            actual = cur.get("id") if isinstance(cur, dict) else None
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"research.current = {actual if actual else 'none'}")
        if metric == "research.current_progress_pct":
            cur = st.research.get("current")
            if not isinstance(cur, dict) or "progressPerc" not in cur:
                raise Unevaluable("no active research node in capture")
            actual = cur["progressPerc"] / 1000.0
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"progressPerc {cur['progressPerc']} / 1000 = {actual:.3f}%")
        if metric == "research.queue_contains":
            node, placeholder = _node_name(pred.get("node"))
            if placeholder is not None:
                raise Unevaluable(f"node is a §4.5 placeholder ({placeholder})")
            queue = st.research.get("queue")
            if queue is None:
                raise Unevaluable("bridge-research.json has no queue")
            actual = node in queue
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"queue = {queue}; contains {node} = {actual}")
        if metric == "research.queue_length":
            queue = st.research.get("queue")
            if queue is None:
                raise Unevaluable("bridge-research.json has no queue")
            actual = len(queue)
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"len(queue) = {actual} {queue}")
        if metric == "research.researched_count":
            if "researchedCount" not in st.research:
                raise Unevaluable("bridge-research.json has no researchedCount")
            actual = st.research["researchedCount"]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"researchedCount = {actual}")
        if metric == "research.has_active_lab":
            if "hasActiveLab" not in st.research:
                raise Unevaluable("bridge-research.json has no hasActiveLab")
            actual = st.research["hasActiveLab"]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"hasActiveLab = {actual}")

        # ---- census ---------------------------------------------------
        if metric == "census.proto_count":
            proto = pred.get("proto")
            by = st.census.get("byProto")
            if by is None:
                raise Unevaluable("bridge-census.json has no byProto")
            actual = by.get(proto, 0)   # format: absent proto = 0
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"byProto[{proto}] = {actual} (absent proto counts 0)")
        if metric in ("census.machine_state_count", "census.machine_state_share"):
            proto = pred.get("proto")
            state = pred.get("state")
            machines = st.census.get("machines")
            if machines is None:
                raise Unevaluable("bridge-census.json has no machines")
            if proto == "*":
                count = sum(m.get("states", {}).get(state, 0)
                            for m in machines.values())
                total = sum(sum(m.get("states", {}).values())
                            for m in machines.values())
                scope = "all machine protos"
            else:
                entry = machines.get(proto)
                if entry is None:
                    count, total = 0, 0
                else:
                    count = entry.get("states", {}).get(state, 0)
                    total = sum(entry.get("states", {}).values())
                scope = proto
            if metric == "census.machine_state_count":
                return Result(TRUE if compare(count, op, expected) else FALSE,
                              f"{scope}: state {state} count = {count}")
            if total == 0:
                raise Unevaluable(f"{scope}: total machine-state count is 0")
            actual = count / total
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"{scope}: {state} {count}/{total} = {actual:.3f}")
        if metric == "census.transports_fully_connected_ratio":
            tr = st.census.get("transports")
            if not isinstance(tr, dict) or "total" not in tr:
                raise Unevaluable("bridge-census.json has no transports block")
            if tr["total"] == 0:
                raise Unevaluable("transports.total = 0 (ratio undefined, §4.1)")
            actual = tr["fullyConnected"] / tr["total"]
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"fullyConnected {tr['fullyConnected']}/{tr['total']} "
                          f"= {actual:.4f}")

        # ---- storages -------------------------------------------------
        if metric == "storage.percent_full":
            rows = _storage_rows(pred, st)
            if not rows:
                raise Unevaluable("storage selector matched zero rows in "
                                  "bridge-storages.json")
            cap = sum(r.get("capacity", 0) for r in rows)
            if cap == 0:
                raise Unevaluable("matching storage rows have zero total capacity")
            qty = sum(r.get("quantity", 0) for r in rows)
            # capacity-weighted mean of percentFullRaw/1000 (§4.1)
            actual = sum((r.get("percentFullRaw", 0) / 1000.0) * r.get("capacity", 0)
                         for r in rows) / cap
            ev = (f"{len(rows)} matching row(s): quantity {qty}/{cap} capacity, "
                  f"capacity-weighted percentFull = {actual:.1f}%")
            pid = pred.get("product")
            if isinstance(pid, str) and pid in st.stats:
                srow = st.stats[pid]
                ev += (f"; cross-ref stats.json: stored {srow['stored']}/"
                       f"{srow['storageCapacity']} (stats totals include "
                       f"stock outside the bridge storage rows)")
            return Result(TRUE if compare(actual, op, expected) else FALSE, ev)
        if metric in ("storage.logistics_in_disabled",
                      "storage.logistics_out_disabled"):
            sid = pred.get("storage_id")
            row = next((r for r in st.storages if r.get("id") == sid), None)
            if row is None:
                raise Unevaluable(f"no storage row with id {sid!r}")
            field = ("logisticsInDisabled" if metric.endswith("in_disabled")
                     else "logisticsOutDisabled")
            actual = row.get(field)
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"storage {sid}.{field} = {actual}")

        # ---- trains ---------------------------------------------------
        if metric == "trains.line_count":
            if st.train_lines is None:
                raise Unevaluable("bridge-trains.json has no lines list")
            actual = len(st.train_lines)
            return Result(TRUE if compare(actual, op, expected) else FALSE,
                          f"len(bridge-trains lines) = {actual}")

        # ---- sources with no capture: UNEVALUABLE by design -----------
        if metric.startswith("power."):
            raise Unevaluable(
                f"{metric}: no power capture exists — the captures predate "
                f"CoiBridge v0.6.2's /coi/v1/power route (format G2); "
                f"field {metric} is absent from every capture file")
        if metric.startswith(("contracts.", "loans.", "edicts.")):
            raise Unevaluable(f"{metric}: no v0.6 trade capture in live/")
        if metric.startswith("time."):
            raise Unevaluable(
                f"{metric}: no executor run anchor (plan-start date/tick) "
                f"exists in the captures — dry-run has no run history")
        raise Unevaluable(f"metric {metric!r} not in §4.1 vocabulary / no "
                          f"capture source (closed-vocabulary violation)")
    except Unevaluable as exc:
        return Result(UNEVALUABLE, str(exc))
    except Exception as exc:  # never panic on a malformed predicate
        return Result(UNEVALUABLE, f"evaluation error: {type(exc).__name__}: {exc}")


def _storage_rows(pred, st):
    if "storage_id" in pred:
        return [r for r in st.storages if r.get("id") == pred["storage_id"]]
    if "product" in pred:
        return [r for r in st.storages if r.get("product") == pred["product"]]
    if "proto" in pred:
        return [r for r in st.storages if r.get("proto") == pred["proto"]]
    raise Unevaluable("storage predicate needs storage_id, product or proto")


# ---------------------------------------------------------------- composite

def _abs_day(game_info):
    """Absolute in-game day index (§4.3.1: 12 months x 30 days)."""
    y, m, d = _parse_date(game_info.get("date"))
    return ((y - 1) * 12 + (m - 1)) * 30 + (d - 1)


def _date_of(st):
    return st.game_info.get("date")


def _duration_kind(duration):
    if isinstance(duration, dict):
        if "months" in duration:
            return "months"
        if "evaluations" in duration:
            return "evaluations"
    return None


def _eval_core(core, st, seq):
    """Evaluate a predicate with any `for:` already stripped."""
    if "all" in core:
        subs = [eval_predicate(p, st, seq) for p in core["all"] or []]
        return _combine_all(subs)
    if "any" in core:
        subs = [eval_predicate(p, st, seq) for p in core["any"] or []]
        return _combine_any(subs)
    if "not" in core:
        sub = eval_predicate(core["not"], st, seq)
        return Result({TRUE: FALSE, FALSE: TRUE}.get(sub.value, UNEVALUABLE),
                      f"not({sub.evidence})")
    return eval_leaf(core, st)


def _eval_duration_seq(core, duration, seq):
    """§4.3.1 sequence semantics for a `for:` wrapper. seq has >= 2
    states, sorted by in-game date, seq[-1] is the evaluation point."""
    kind = _duration_kind(duration)
    latest = seq[-1]
    try:
        days = [_abs_day(s.game_info) for s in seq]
    except Unevaluable as exc:
        return Result(UNEVALUABLE,
                      f"sequence snapshot date unusable: {exc} [for: {duration}]")
    if kind == "months":
        months = duration["months"]
        boundary = days[-1] - 30 * months
        anchor = None
        for i, d in enumerate(days):
            if d <= boundary:
                anchor = i
        window = [i for i, d in enumerate(days) if d > boundary]
        if anchor is not None:
            window = [anchor] + window
        results = [(i, _eval_core(core, seq[i], seq[:i + 1])) for i in window]
        span = (days[-1] - days[window[0]]) / 30.0
        for i, r in results:
            if r.value == FALSE:
                return Result(FALSE,
                              f"{r.evidence} [for: {duration} — inner predicate "
                              f"FALSE at {_date_of(seq[i])} inside the trailing "
                              f"{months}-month window, so the duration did "
                              f"not hold]")
        for i, r in results:
            if r.value == UNEVALUABLE:
                return Result(UNEVALUABLE,
                              f"{r.evidence} [for: {duration} — inner predicate "
                              f"UNEVALUABLE at {_date_of(seq[i])} inside the "
                              f"window; the hold cannot be proven]")
        if anchor is None:
            return Result(UNEVALUABLE,
                          f"inner predicate TRUE at all {len(results)} "
                          f"snapshot(s) in the trailing window "
                          f"({_date_of(seq[window[0]])} .. {_date_of(latest)}) "
                          f"[for: {duration} — UNEVALUABLE: history begins "
                          f"inside the window, no anchor snapshot at or "
                          f"before the window start; observed span "
                          f"{span:.2f} months < {months}]")
        return Result(TRUE,
                      f"inner predicate TRUE at every snapshot in the "
                      f"trailing {months}-month window (anchor "
                      f"{_date_of(seq[anchor])}, {len(results)} snapshot(s), "
                      f"span {span:.2f} months) [for: {duration}]")
    if kind == "evaluations":
        n = duration["evaluations"]
        if len(seq) < n:
            return Result(UNEVALUABLE,
                          f"sequence holds {len(seq)} snapshot(s) < the "
                          f"{n} evaluations required [for: {duration}]")
        window = list(range(len(seq) - n, len(seq)))
        results = [(i, _eval_core(core, seq[i], seq[:i + 1])) for i in window]
        for i, r in results:
            if r.value == FALSE:
                return Result(FALSE,
                              f"{r.evidence} [for: {duration} — inner predicate "
                              f"FALSE at {_date_of(seq[i])} within the last "
                              f"{n} evaluations]")
        for i, r in results:
            if r.value == UNEVALUABLE:
                return Result(UNEVALUABLE,
                              f"{r.evidence} [for: {duration} — inner predicate "
                              f"UNEVALUABLE at {_date_of(seq[i])} within the "
                              f"last {n} evaluations]")
        return Result(TRUE,
                      f"inner predicate TRUE at each of the last {n} "
                      f"evaluations ({_date_of(seq[window[0]])} .. "
                      f"{_date_of(latest)}) [for: {duration}]")
    return Result(UNEVALUABLE, f"unknown duration shape {duration!r}")


def eval_predicate(pred, st, seq=None):
    """Evaluate any predicate form (leaf / all / any / not / for:).

    seq, when given, is the state sequence (sorted by in-game date,
    last element == st) against which `for:` durations evaluate per
    §4.3.1. With no sequence (or a 1-element one) the legacy
    single-snapshot readings apply, unchanged."""
    if not isinstance(pred, dict):
        return Result(UNEVALUABLE, f"predicate is not a mapping: {pred!r}")
    duration = pred.get("for")
    core = {k: v for k, v in pred.items() if k != "for"}
    if (duration is not None and seq is not None and len(seq) > 1
            and _duration_kind(duration) is not None):
        return _eval_duration_seq(core, duration, seq)
    res = _eval_core(core, st, seq)
    if duration is not None:
        # §4.3: TRUE only after holding TRUE on every evaluation spanning
        # the duration. A single snapshot cannot prove a duration.
        if res.value == FALSE:
            return Result(FALSE, f"{res.evidence} [for: {duration} — currently "
                                 f"false, so the duration cannot hold]")
        if res.value == TRUE:
            return Result(UNEVALUABLE,
                          f"{res.evidence} [for: {duration} — instantaneous "
                          f"value is TRUE but duration is UNEVALUABLE from a "
                          f"single snapshot: no executor evaluation history "
                          f"in the captures]")
        return Result(UNEVALUABLE,
                      f"{res.evidence} [for: {duration}]")
    return res


def _combine_all(subs):
    if any(s.value == FALSE for s in subs):
        v = FALSE
    elif any(s.value == UNEVALUABLE for s in subs):
        v = UNEVALUABLE
    else:
        v = TRUE
    return Result(v, "; ".join(f"[{s.value}] {s.evidence}" for s in subs))


def _combine_any(subs):
    if any(s.value == TRUE for s in subs):
        v = TRUE
    elif any(s.value == UNEVALUABLE for s in subs):
        v = UNEVALUABLE
    else:
        v = FALSE
    return Result(v, "; ".join(f"[{s.value}] {s.evidence}" for s in subs))


def eval_gate(pred_list, st, seq=None):
    """A gate field is a list read as an implicit all: (§4.2)."""
    if not pred_list:
        return Result(TRUE, "empty gate — enters immediately (§7)"), []
    results = [(p, eval_predicate(p, st, seq)) for p in pred_list]
    combined = _combine_all([r for _, r in results])
    return combined, results


# ---------------------------------------------------------------- rendering

def render_predicate(pred):
    if not isinstance(pred, dict):
        return repr(pred)
    if "all" in pred:
        inner = "; ".join(render_predicate(p) for p in pred["all"] or [])
        s = f"all({inner})"
    elif "any" in pred:
        inner = "; ".join(render_predicate(p) for p in pred["any"] or [])
        s = f"any({inner})"
    elif "not" in pred:
        s = f"not({render_predicate(pred['not'])})"
    else:
        metric = pred.get("metric", "?")
        sel = []
        for key in ("product", "proto", "state", "window", "node",
                    "storage_id", "edict"):
            if key in pred:
                v = pred[key]
                if isinstance(v, dict) and "placeholder" in v:
                    sel.append(f"{key}=<placeholder {v['placeholder']}>")
                else:
                    sel.append(f"{key}={v}")
        op = pred.get("op")
        val = pred.get("value")
        if "value" in pred or op is not None:
            s = f"{metric}({', '.join(sel)}) {op or '=='} {val!r}"
        else:
            s = f"{metric}({', '.join(sel)}) == True (implicit)"
    if "for" in pred:
        s += f"  for: {pred['for']}"
    return s


def _contains_placeholder(node):
    if isinstance(node, dict):
        if "placeholder" in node:
            return True
        return any(_contains_placeholder(v) for v in node.values())
    if isinstance(node, list):
        return any(_contains_placeholder(v) for v in node)
    return False


def render_order(order, st, gate_status, seq=None):
    """One 'next order' line block for the active phase. Never executes."""
    oid = order.get("id", "<no-id>")
    do = order.get("do", "<no-do>")
    lines = []
    # preconditions
    pre = order.get("preconditions") or []
    pre_res = None
    if pre:
        pre_res, _ = eval_gate(pre, st, seq)
    held_reasons = []
    if gate_status != "ACTIVE":
        held_reasons.append(f"phase not entered ({gate_status})")
    if pre_res is not None and pre_res.value != TRUE:
        held_reasons.append(f"preconditions {pre_res.value}: {pre_res.evidence}")
    if _contains_placeholder(order.get("args")):
        # a placeholder in the command args makes the order manual-only (§4.5)
        held_reasons.append("§4.5 placeholder id in args — manual-only "
                            "until a human sources the id")
    if order.get("requires_sandbox_validation"):
        held_reasons.append("requires_sandbox_validation — manual-only on a "
                            "real save (§6.1/G3)")
    if order.get("enabled") == "on-fallback":
        held_reasons.append("enabled: on-fallback — not due in normal flow")

    if do in NON_COMMAND_KINDS:
        kind_desc = {
            "place-blueprint": (
                f"place-blueprint `{order.get('blueprint_ref', '?')}`"
                f" district={order.get('district', '?')}"
                + (f" count={order['count']}" if "count" in order else "")
                + (f" asset_status={order['asset_status']}"
                   if "asset_status" in order else "")),
            "manual-step": f"manual-step: {str(order.get('step', '')).strip()}",
            "observe-only": f"observe-only: {str(order.get('observe', '')).strip()}",
            "wait": f"wait until: {order.get('until')}",
        }[do]
        rendered = kind_desc
    else:
        args = order.get("args") or {}
        rendered = f"command `{do}` args={json.dumps(args, default=str)}"
    status = "HELD — " + "; ".join(held_reasons) if held_reasons else \
        ("WOULD PREPARE (command, §12 prepare step — not sent)" if do not in NON_COMMAND_KINDS
         else "WOULD PRESENT (human/agent task — not executed)")
    lines.append(f"- **{oid}** — {status}")
    lines.append(f"  - {rendered}")
    if order.get("note"):
        lines.append(f"  - note: {str(order['note']).strip()}")
    if pre:
        lines.append(f"  - preconditions: **{pre_res.value}** — {pre_res.evidence}")
    verify = order.get("verify") or []
    if verify:
        vres, _ = eval_gate(verify, st, seq)
        lines.append(f"  - verify (current state): **{vres.value}** — {vres.evidence}")
    return lines


# ---------------------------------------------------------------- report

def derive_statuses(phase_eval):
    """§7/§9 status derivation from evaluated entry/exit gates.
    phase_eval: list of dicts with 'entry'/'exit' Results."""
    statuses = []
    current_idx = None
    for i, pe in enumerate(phase_eval):
        if current_idx is not None:
            statuses.append("NOT REACHED")
        elif pe["exit"].value == TRUE:
            statuses.append("COMPLETE")
        else:
            current_idx = i
            if pe["entry"].value == TRUE:
                statuses.append("ACTIVE")
            elif pe["entry"].value == UNEVALUABLE:
                statuses.append("HELD AT ENTRY GATE (UNEVALUABLE)")
            else:
                statuses.append("NOT ENTERED — entry gate FALSE")
    return statuses, current_idx


def build_report(plan_path, st, seq=None):
    try:
        doc = yaml.safe_load(plan_path.read_text())
    except yaml.YAMLError as exc:
        return None, f"clean error: plan YAML does not parse: {exc}"
    if not isinstance(doc, dict) or "plan" not in doc or "phases" not in doc:
        return None, "clean error: plan document lacks top-level plan/phases"

    meta = doc["plan"] or {}
    globals_ = doc.get("globals") or {}
    gi = st.game_info
    out = []
    out.append("# DRY RUN — nothing was executed")
    out.append("")
    out.append("> **DRY RUN — nothing was executed.** This report is a "
               "read-only evaluation of a plan file against local state "
               "captures. No command was prepared, confirmed, applied or "
               "sent anywhere; no game was contacted. Orders below are "
               "what a supervised executor *would* prepare next.")
    out.append("")
    out.append(f"# Plan dry-run: {meta.get('id', '?')} v{meta.get('version', '?')}")
    out.append("")
    out.append(f"- Title: {meta.get('title', '?')}")
    out.append(f"- Map/difficulty (plan): {meta.get('map')} — executor "
               f"map/difficulty/version match is a live check (§3.1) and "
               f"cannot be settled from captures; not asserted here.")
    out.append(f"- Plan game_version: {meta.get('game_version')}")
    out.append(f"- Requires bridge: {(meta.get('requires') or {}).get('bridge')}")
    out.append(f"- State source: `{st.live_dir}` capture files — game-info "
               f"date **{gi.get('date')}**, tick **{gi.get('tick')}**, "
               f"gameId `{gi.get('gameId')}`, paused={gi.get('isPaused')}, "
               f"speed={gi.get('speed')}")
    if seq is not None and len(seq) > 1:
        out.append(f"- **Sequence mode (§4.3.1):** {len(seq)} snapshots, "
                   f"{_date_of(seq[0])} .. {_date_of(seq[-1])}, one gameId "
                   f"`{gi.get('gameId')}`. `for:` durations evaluate over "
                   f"this history; all other predicates read the latest "
                   f"snapshot. Per-prefix phase history is tabulated "
                   f"below.")
    out.append(f"- Population {st.status.get('population')}, housing "
               f"{st.status.get('totalHousingCapacity')} (free "
               f"{st.status.get('freeHousingCapacity')}, homeless "
               f"{st.status.get('numberOfHomeless')}), monthsOfFood "
               f"{st.status.get('monthsOfFood')}, starving "
               f"{st.status.get('arePeopleStarving')}; researchedCount "
               f"{st.research.get('researchedCount')}, current research "
               f"{(st.research.get('current') or {}).get('id')}")
    out.append("")
    out.append("## State-source mapping used by this evaluator")
    out.append("")
    out.append("- `product.*` quantity/stored/capacity/flows ← `stats.json` "
               "(`Product_`-prefixed ids, exactly as plans name them); "
               "windows thisMonth=0, lastMonth=1, thisYear=2, lastYear=3, "
               "lifetime=4; `net_flow` = produced − consumed.")
    out.append("- `product.unlocked` ← `products.json` catalogue; "
               "`product.pinned` ← membership of `pinned.json`'s product "
               "keys (the pinned=true view, captured in stats shape).")
    out.append("- `game.*` ← `game-info.json`; `settlement.*` ← "
               "`bridge-status.json`; `research.*` ← `bridge-research.json` "
               "(progressPerc ÷ 1000); `census.*` ← `bridge-census.json`; "
               "`storage.*` ← `bridge-storages.json` (percentFullRaw ÷ 1000, "
               "capacity-weighted over matching rows); `trains.line_count` "
               "← `bridge-trains.json`.")
    _researched = st.research.get("researched")
    if isinstance(_researched, list):
        _research_note = (f"`research.completed` reads the capture's "
                          f"v0.6.2 `researched` list ({len(_researched)} "
                          f"nodes), format G1 resolved")
    else:
        _research_note = ("`research.completed` (captures predate "
                          "CoiBridge v0.6.2 — no `researched` list in "
                          "`bridge-research.json`, format G1)")
    if seq is not None and len(seq) > 1:
        _duration_note = ("any `for:` duration the sequence history "
                          "cannot prove — insufficient history or a blind "
                          "snapshot inside the window (§4.3.1)")
    else:
        _duration_note = ("any `for:` duration whose instantaneous value "
                          "is TRUE (a single snapshot cannot prove a "
                          "duration, §4.3 — see interpretation note)")
    out.append("- **UNEVALUABLE by design:** " + _research_note + ", "
               "all `power.*` (no power "
               "capture, G2), `contracts/loans/edicts` (no v0.6 captures), "
               "`time.*` (no executor run anchor), and " + _duration_note + ".")
    out.append("")

    counts = {TRUE: 0, FALSE: 0, UNEVALUABLE: 0}

    def record(res):
        counts[res.value] = counts.get(res.value, 0) + 1
        return res

    # ---- globals -------------------------------------------------------
    out.append("## Globals")
    out.append("")
    inv = globals_.get("invariants") or []
    if inv:
        out.append("### Invariants (§5.2 — evaluated every iteration)")
        out.append("")
        for pred in inv:
            res = record(eval_predicate(pred, st, seq))
            out.append(f"- **{res.value}** — `{render_predicate(pred)}` — {res.evidence}")
        out.append("")
    gchecks = globals_.get("checks") or []
    if gchecks:
        out.append("### Global checks")
        out.append("")
        for chk in gchecks:
            res = record(eval_predicate(chk.get("when"), st, seq))
            verdict = ("WOULD FIRE" if res.value == TRUE else
                       "check blind (UNEVALUABLE)" if res.value == UNEVALUABLE
                       else "not firing")
            out.append(f"- `{chk.get('id')}` ({chk.get('severity')}): "
                       f"**{res.value}** — {verdict} — {res.evidence}")
        out.append("")

    # ---- phases ---------------------------------------------------------
    phases = doc["phases"] or []
    phase_eval = []
    for ph in phases:
        entry_res, entry_detail = eval_gate(ph.get("entry_gate") or [], st, seq)
        exit_res, exit_detail = eval_gate(ph.get("exit_gate") or [], st, seq)
        for _, r in entry_detail + exit_detail:
            record(r)
        checks_eval = []
        for chk in ph.get("checks") or []:
            res = eval_predicate(chk.get("when"), st, seq)
            record(res)
            checks_eval.append((chk, res))
        phase_eval.append({
            "phase": ph, "entry": entry_res, "entry_detail": entry_detail,
            "exit": exit_res, "exit_detail": exit_detail, "checks": checks_eval,
        })

    # Active-phase derivation (§7/§9): phases in order; a phase is
    # COMPLETE when its exit gate is TRUE; the first non-complete phase
    # is the loop position — ACTIVE if its entry gate is TRUE, otherwise
    # held at its entry gate; all later phases are NOT REACHED.
    statuses, current_idx = derive_statuses(phase_eval)
    if current_idx is None:
        plan_status = "PLAN COMPLETE (every phase's exit gate is TRUE)"
        active = None
    else:
        active = phase_eval[current_idx]
        plan_status = (f"loop position: phase {current_idx + 1} of "
                       f"{len(phases)} — {active['phase'].get('id')} "
                       f"[{statuses[current_idx]}]")

    out.append("## Active-phase determination (§9 play-loop rules)")
    out.append("")
    for i, pe in enumerate(phase_eval):
        ph = pe["phase"]
        why = ""
        if statuses[i] == "COMPLETE":
            why = f"exit gate TRUE ({pe['exit'].evidence})"
        elif i == current_idx and statuses[i] == "ACTIVE":
            why = ("first non-complete phase; entry gate TRUE, exit gate "
                   f"{pe['exit'].value}")
        elif i == current_idx:
            why = (f"first non-complete phase; entry gate {pe['entry'].value} "
                   f"— loop holds here, orders do not run (§4.4/§9 step 4)")
        else:
            why = "an earlier phase is not complete; phases never overlap (§7)"
        out.append(f"- `{ph.get('id')}` — **{statuses[i]}** — {why}")
    out.append("")
    out.append(f"**Active phase: "
               f"{active['phase'].get('id') if active else 'none — ' + plan_status}**"
               + (f" — {statuses[current_idx]}" if active else ""))
    out.append("")

    # ---- sequence history (per prefix) -----------------------------------
    if seq is not None and len(seq) > 1:
        out.append("## Sequence phase history (per prefix, §4.3.1)")
        out.append("")
        out.append("Each row re-derives phase statuses using only the "
                   "history up to that snapshot — the way a live executor's "
                   "history grows. The last row is this report's verdict.")
        out.append("")
        short = {"COMPLETE": "COMPLETE", "ACTIVE": "ACTIVE",
                 "HELD AT ENTRY GATE (UNEVALUABLE)": "HELD (U)",
                 "NOT ENTERED — entry gate FALSE": "NOT ENTERED (F)",
                 "NOT REACHED": "NOT REACHED"}
        header = "| snapshot | " + " | ".join(
            f"phase {i}" for i in range(len(phases))) + " |"
        out.append(header)
        out.append("|" + "---|" * (len(phases) + 1))
        for k in range(1, len(seq) + 1):
            prefix = seq[:k]
            pst = prefix[-1]
            p_eval = []
            for ph in phases:
                e_res, _ = eval_gate(ph.get("entry_gate") or [], pst, prefix)
                x_res, _ = eval_gate(ph.get("exit_gate") or [], pst, prefix)
                p_eval.append({"entry": e_res, "exit": x_res})
            p_statuses, _ = derive_statuses(p_eval)
            row = " | ".join(short.get(s, s) for s in p_statuses)
            out.append(f"| {_date_of(pst)} | {row} |")
        out.append("")
        out.append("Phase key: " + "; ".join(
            f"phase {i} = `{ph.get('id')}`" for i, ph in enumerate(phases)))
        out.append("")

    # ---- per-phase detail ------------------------------------------------
    out.append("## Phase-by-phase evaluation")
    for i, pe in enumerate(phase_eval):
        ph = pe["phase"]
        out.append("")
        out.append(f"### {ph.get('id')} — {ph.get('name', '')} [{statuses[i]}]")
        out.append("")
        out.append(f"Intent: {str(ph.get('intent', '')).strip()}")
        out.append("")
        out.append(f"**Entry gate: {pe['entry'].value}**"
                   + (f" — {pe['entry'].evidence}" if not pe["entry_detail"] else ""))
        for pred, res in pe["entry_detail"]:
            out.append(f"- **{res.value}** — `{render_predicate(pred)}` — {res.evidence}")
        out.append("")
        out.append(f"**Exit gate: {pe['exit'].value}**"
                   + (f" — {pe['exit'].evidence}" if not pe["exit_detail"] else ""))
        for pred, res in pe["exit_detail"]:
            out.append(f"- **{res.value}** — `{render_predicate(pred)}` — {res.evidence}")
        if pe["checks"]:
            out.append("")
            out.append("Checks (evaluated against current state):")
            for chk, res in pe["checks"]:
                verdict = ("WOULD FIRE" if res.value == TRUE else
                           "check blind (UNEVALUABLE)" if res.value == UNEVALUABLE
                           else "not firing")
                out.append(f"- `{chk.get('id')}` ({chk.get('severity')}, "
                           f"action={chk.get('action')}): **{res.value}** — "
                           f"{verdict} — {res.evidence}")
        fb = ph.get("fallbacks") or {}
        if fb:
            out.append("")
            out.append(f"Fallbacks: {json.dumps(fb, default=str)}")

    # ---- next orders ------------------------------------------------------
    out.append("")
    out.append("## Next orders (DRY RUN — would be prepared, nothing sent)")
    out.append("")
    if active is None:
        out.append("No active phase — the plan is complete by its gates; "
                   "no orders would be prepared.")
    else:
        ph = active["phase"]
        out.append(f"For active phase `{ph.get('id')}` "
                   f"[{statuses[current_idx]}], in plan order:")
        out.append("")
        for order in ph.get("orders") or []:
            out.extend(render_order(order, st, statuses[current_idx], seq))
        if statuses[current_idx] != "ACTIVE":
            out.append("")
            out.append("All orders above are held: the phase has not been "
                       "entered, so no order is due (§9 step 5).")
    out.append("")
    out.append("## Summary")
    out.append("")
    out.append(f"- Predicates evaluated (globals invariants/checks plus "
               f"phase entry/exit gates and phase checks; composites count "
               f"once): TRUE {counts[TRUE]}, "
               f"FALSE {counts[FALSE]}, UNEVALUABLE {counts[UNEVALUABLE]} "
               f"(leaf evidence inside composites is in the per-phase tables "
               f"via the combined evidence strings).")
    out.append(f"- Plan status: {plan_status}.")
    if seq is not None and len(seq) > 1:
        _note1 = ("- Interpretation notes: (1) `for:` durations evaluate "
                  "over the state sequence per §4.3.1 — TRUE only with an "
                  "anchor snapshot at/before the window start and the "
                  "inner predicate TRUE at every snapshot in the window; "
                  "FALSE if it broke anywhere in the window; UNEVALUABLE "
                  "on insufficient history, never by default. (2) A phase "
                  "counts COMPLETE ")
    else:
        _note1 = ("- Interpretation notes: (1) `for:` durations — instantaneous "
                  "FALSE stays FALSE; instantaneous TRUE becomes UNEVALUABLE "
                  "because one snapshot cannot prove the predicate held for "
                  "the whole duration (§4.3). (2) A phase counts COMPLETE ")
    out.append(_note1 + "purely when its exit gate is TRUE (§7/§9 re-derivation "
               "from state); human-confirm manual orders inside a gate-complete "
               "phase are not re-litigated by the evaluator. (3) Unknown / "
               "out-of-vocabulary metrics evaluate UNEVALUABLE with a "
               "closed-vocabulary note, never a crash and never a guess.")
    out.append("")
    out.append("---")
    out.append("DRY RUN — nothing was executed. Generated by "
               "`tools/plan_check.py` (read-only over local files).")
    return "\n".join(out) + "\n", None


def main(argv=None):
    ap = argparse.ArgumentParser(description="Dry-run CoI plan evaluator "
                                             "(read-only, executes nothing).")
    ap.add_argument("plan", help="plan YAML file")
    ap.add_argument("--live-dir", default=None,
                    help="captures directory (default: <repo>/live)")
    ap.add_argument("--state-seq", default=None,
                    help="directory of per-snapshot state directories "
                         "(§4.3.1 sequence mode); a directory that itself "
                         "holds game-info.json is a 1-snapshot sequence")
    ap.add_argument("--out", default=None, help="write report to this file "
                                                "(default: stdout)")
    args = ap.parse_args(argv)
    plan_path = pathlib.Path(args.plan)
    if args.live_dir:
        live_dir = pathlib.Path(args.live_dir)
    else:
        # tools/plan_check.py -> repo root is the parent of tools/
        live_dir = pathlib.Path(__file__).resolve().parent.parent / "live"
    if not plan_path.exists():
        print(f"clean error: plan file not found: {plan_path}", file=sys.stderr)
        return 2
    seq = None
    if args.state_seq:
        try:
            seq, err = load_sequence(pathlib.Path(args.state_seq))
        except Unevaluable as exc:
            print(f"clean error: {exc}", file=sys.stderr)
            return 2
        if err:
            print(err, file=sys.stderr)
            return 2
        st = seq[-1]
    else:
        if not live_dir.exists():
            print(f"clean error: live captures dir not found: {live_dir}",
                  file=sys.stderr)
            return 2
        try:
            st = State(live_dir)
        except Unevaluable as exc:
            print(f"clean error: {exc}", file=sys.stderr)
            return 2
    report, err = build_report(plan_path, st, seq)
    if err:
        print(err, file=sys.stderr)
        return 2
    if args.out:
        pathlib.Path(args.out).write_text(report)
        print(f"wrote {args.out}")
    else:
        print(report)
    return 0


if __name__ == "__main__":
    sys.exit(main())
