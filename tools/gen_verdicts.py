#!/usr/bin/env python3
"""gen_verdicts.py — bake plan dry-run verdicts for the website /plans page.

Round 4 (2026-11-03): serve.rs embeds plans/verdicts.json (produced
here) alongside the canonical plan JSONs, so the single-file exe can
render each plan's latest dry-run verdicts with NO runtime file
dependencies. Verdicts are computed with the reference evaluator's own
machinery (tools/plan_check.py imported as a module — same State,
same eval_gate/eval_predicate, same active-phase derivation and the
same held-reason rules as its render_order), against the captures in
live/. Re-run this whenever the plans or the captures change, then
rebuild the exe.

Output shape (plans/verdicts.json):
  { "state": {date, tick, gameId, paused, speed},
    "plans": [ { id, title, version, game_version, plan_status,
                 active_phase (id or null),
                 phases: [{id, name, status}],
                 orders: [ {id, do, status_head, held_reasons,
                            note, args, selector_args, concrete_args} ] } ] }
`orders` are the ACTIVE phase's orders in plan order. For command
orders, args are split: concrete_args (plain JSON values the generic
prepare form can post as-is) and selector_args (the §6.2 `$entity`
selector objects, which a human resolves to an entity id — the /plans
page renders them as labelled inputs; nothing auto-resolves them).

Read-only over local files, like everything in the plan toolchain.
"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import plan_check as pc  # the reference evaluator

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN_FILES = [
    ROOT / "plans" / "fresh-start-v1.yaml",
    ROOT / "plans" / "dragon-isle-continuation-v1.yaml",
]


def held_reasons(order, st, gate_status):
    """Mirror plan_check.render_order's held-reason rules, as data."""
    reasons = []
    if gate_status != "ACTIVE":
        reasons.append(f"phase not entered ({gate_status})")
    pre = order.get("preconditions") or []
    if pre:
        pre_res, _ = pc.eval_gate(pre, st)
        if pre_res.value != pc.TRUE:
            reasons.append(f"preconditions {pre_res.value}: {pre_res.evidence}")
    if pc._contains_placeholder(order.get("args")):
        reasons.append("§4.5 placeholder id in args — manual-only "
                       "until a human sources the id")
    if order.get("requires_sandbox_validation"):
        reasons.append("requires_sandbox_validation — manual-only on a "
                       "real save (§6.1/G3)")
    if order.get("enabled") == "on-fallback":
        reasons.append("enabled: on-fallback — not due in normal flow")
    return reasons


def plan_verdicts(doc, st):
    meta = doc["plan"] or {}
    phases = doc["phases"] or []
    evals = []
    for ph in phases:
        entry, _ = pc.eval_gate(ph.get("entry_gate") or [], st)
        exit_, _ = pc.eval_gate(ph.get("exit_gate") or [], st)
        evals.append((entry, exit_))
    statuses = []
    current = None
    for i, (entry, exit_) in enumerate(evals):
        if current is not None:
            statuses.append("NOT REACHED")
        elif exit_.value == pc.TRUE:
            statuses.append("COMPLETE")
        else:
            current = i
            if entry.value == pc.TRUE:
                statuses.append("ACTIVE")
            elif entry.value == pc.UNEVALUABLE:
                statuses.append("HELD AT ENTRY GATE (UNEVALUABLE)")
            else:
                statuses.append("NOT ENTERED — entry gate FALSE")
    if current is None:
        plan_status = "PLAN COMPLETE (every phase's exit gate is TRUE)"
        active_id = None
        orders = []
    else:
        active = phases[current]
        active_id = active.get("id")
        plan_status = (f"loop position: phase {current + 1} of {len(phases)} "
                       f"— {active_id} [{statuses[current]}]")
        orders = []
        for order in active.get("orders") or []:
            do = order.get("do", "<no-do>")
            reasons = held_reasons(order, st, statuses[current])
            if reasons:
                head = "HELD"
            elif do not in pc.NON_COMMAND_KINDS:
                head = "WOULD PREPARE"
            else:
                head = "WOULD PRESENT"
            args = order.get("args") or {}
            selectors, concrete = {}, {}
            for k, v in args.items():
                if isinstance(v, dict) and "$entity" in v:
                    selectors[k] = v["$entity"]
                else:
                    concrete[k] = v
            entry = {
                "id": order.get("id", "<no-id>"),
                "do": do,
                "status_head": head,
                "held_reasons": reasons,
                "note": str(order.get("note", "")).strip(),
                "concrete_args": concrete,
                "selector_args": selectors,
            }
            if do == "place-blueprint":
                entry["blueprint_ref"] = order.get("blueprint_ref")
                entry["asset_status"] = order.get("asset_status")
            orders.append(entry)
    return {
        "id": meta.get("id"),
        "title": meta.get("title"),
        "version": meta.get("version"),
        "game_version": meta.get("game_version"),
        "plan_status": plan_status,
        "active_phase": active_id,
        "phases": [
            {"id": ph.get("id"), "name": ph.get("name"),
             "status": statuses[i]}
            for i, ph in enumerate(phases)
        ],
        "orders": orders,
    }


def main():
    st = pc.State(ROOT / "live")
    gi = st.game_info
    out = {
        "state": {
            "date": gi.get("date"), "tick": gi.get("tick"),
            "gameId": gi.get("gameId"), "paused": gi.get("isPaused"),
            "speed": gi.get("speed"),
            "source": "live/ captures (2026-11-02) — verdicts are as of "
                      "that snapshot, not live state",
        },
        "plans": [],
    }
    for path in PLAN_FILES:
        doc = pc.yaml.safe_load(path.read_text())
        out["plans"].append(plan_verdicts(doc, st))
        print(f"{path.name}: {out['plans'][-1]['plan_status']}")
    dest = ROOT / "plans" / "verdicts.json"
    dest.write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {dest} ({dest.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
