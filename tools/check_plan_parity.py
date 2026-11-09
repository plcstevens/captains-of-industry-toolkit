#!/usr/bin/env python3
"""check_plan_parity.py — parity gate between the two plan evaluators.

Runs the Rust `coi plan-check` (canonical JSON plan) and compares its
verdicts against the verified Python reference reports
(plans/dev/*.dryrun.md, produced by tools/plan_check.py):

  * per-phase statuses (COMPLETE / ACTIVE / HELD… / NOT ENTERED / NOT REACHED)
  * per-phase entry-gate and exit-gate combined verdicts
  * every listed gate predicate's verdict, in order
  * every phase/global check verdict, in order
  * global invariant verdicts, in order
  * the summary predicate counts (TRUE / FALSE / UNEVALUABLE)
  * next-order statuses (HELD / WOULD PREPARE / WOULD PRESENT) per order id

Evidence STRINGS are not compared (wording may differ cosmetically);
verdicts and counts must match exactly. Exit 0 = parity, 1 = diffs.

Usage:
    python3 tools/check_plan_parity.py [--coi coi/target/debug/superintendent]
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLANS = [
    (
        "fresh-start-v1",
        ROOT / "plans" / "fresh-start-v1.json",
        ROOT / "plans" / "dev" / "fresh-start-v1.dryrun.md",
    ),
    (
        "dragon-isle-continuation-v1",
        ROOT / "plans" / "dragon-isle-continuation-v1.json",
        ROOT / "plans" / "dev" / "dragon-isle-continuation-v1.dryrun.md",
    ),
]


def extract(md):
    """Pull the verdict skeleton out of a dry-run report (either engine)."""
    out = {
        "phase_status": [],  # (phase_id, STATUS)
        "gate_combined": [],  # (phase_id, 'entry'|'exit', VERDICT)
        "gate_items": [],  # (phase_id, 'entry'|'exit', idx, VERDICT)
        "phase_checks": [],  # (phase_id, check_id, VERDICT)
        "invariants": [],  # VERDICT
        "global_checks": [],  # (check_id, VERDICT)
        "counts": None,  # (T, F, U)
        "orders": [],  # (order_id, STATUS-HEAD)
        "active": None,  # active phase id or 'none'
    }
    section = None
    phase = None
    gate = None
    gate_idx = 0
    for line in md.splitlines():
        if line.startswith("## "):
            section = line[3:].strip()
            phase = None
            gate = None
            continue
        if line.startswith("### "):
            header = line[4:].strip()
            if section and section.startswith("Phase-by-phase"):
                phase = header.split(" — ")[0].strip()
            gate = None
            continue
        m = re.match(r"- `([^`]+)` — \*\*([^*]+)\*\*", line)
        if m and section and section.startswith("Active-phase determination"):
            out["phase_status"].append((m.group(1), m.group(2)))
            continue
        m = re.match(r"\*\*Active phase: ([^*]+)\*\*", line)
        if m:
            out["active"] = m.group(1).split(" — ")[0].strip()
            continue
        m = re.match(r"\*\*(Entry|Exit) gate: (TRUE|FALSE|UNEVALUABLE)\*\*", line)
        if m and phase:
            gate = m.group(1).lower()
            gate_idx = 0
            out["gate_combined"].append((phase, gate, m.group(2)))
            continue
        m = re.match(r"- \*\*(TRUE|FALSE|UNEVALUABLE)\*\* — ", line)
        if m:
            if section == "Globals":
                out["invariants"].append(m.group(1))
            elif phase and gate:
                out["gate_items"].append((phase, gate, gate_idx, m.group(1)))
                gate_idx += 1
            continue
        m = re.match(r"- `([^`]+)` \([^)]*\): \*\*(TRUE|FALSE|UNEVALUABLE)\*\*", line)
        if m:
            if section == "Globals":
                out["global_checks"].append((m.group(1), m.group(2)))
            continue
        m = re.match(r"- `([^`]+)` \([^)]*, action=[^)]*\): \*\*(TRUE|FALSE|UNEVALUABLE)\*\*", line)
        if m and phase:
            out["phase_checks"].append((phase, m.group(1), m.group(2)))
            continue
        m = re.match(r"- \*\*([^*]+)\*\* — (HELD|WOULD PREPARE|WOULD PRESENT)", line)
        if m and section and section.startswith("Next orders"):
            out["orders"].append((m.group(1), m.group(2)))
            continue
        m = re.search(r"TRUE (\d+), FALSE (\d+), UNEVALUABLE (\d+)", line)
        if m:
            out["counts"] = tuple(int(x) for x in m.groups())
            continue
    return out


def compare(name, ref, got):
    diffs = []
    for key in (
        "phase_status",
        "gate_combined",
        "gate_items",
        "phase_checks",
        "invariants",
        "global_checks",
        "counts",
        "orders",
        "active",
    ):
        if ref[key] != got[key]:
            diffs.append(f"  [{key}] reference={ref[key]!r}\n          rust     ={got[key]!r}")
    if diffs:
        print(f"== {name}: PARITY FAIL")
        for d in diffs:
            print(d)
    else:
        print(
            f"== {name}: PARITY OK "
            f"({len(ref['gate_items'])} gate items, "
            f"{len(ref['phase_status'])} phases, counts {ref['counts']})"
        )
    return 1 if diffs else 0


def main():
    coi = ROOT / "coi" / "target" / "debug" / "coi"
    if "--coi" in sys.argv:
        coi = pathlib.Path(sys.argv[sys.argv.index("--coi") + 1])
    if not coi.exists():
        print(f"clean error: coi binary not found: {coi}", file=sys.stderr)
        return 2
    rc = 0
    for name, plan_json, ref_md in PLANS:
        rust = subprocess.run(
            [str(coi), "plan-check", str(plan_json), "--state-dir", str(ROOT / "live")],
            capture_output=True,
            text=True,
        )
        if rust.returncode != 0:
            print(f"== {name}: rust engine exited {rust.returncode}\n{rust.stderr}")
            rc = 1
            continue
        rc |= compare(name, extract(ref_md.read_text()), extract(rust.stdout))
    print(f"OVERALL: {'PARITY PASS' if rc == 0 else 'PARITY FAIL'}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
