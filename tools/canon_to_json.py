#!/usr/bin/env python3
"""Generate the canon renderings from docs/canon/*.yaml.

Outputs:
  docs/canon/canon.json        — merged canon, embedded by the website
                                 (include_str! in coi/src/serve.rs) and
                                 rendered by the /why pages.
  --agents-md PATH             — optional Markdown rendering for agents
                                 (the .llms/ index points at it; dual-facing
                                 material is generated, never duplicated).

Same shape as tools/plan_to_json.py: the crate has no YAML dependency, so
YAML is compiled to JSON here, at build time, by this script. Re-run after
editing any canon YAML; check_canon.py verifies code agreement separately.

Run from the repo root:  python3 tools/canon_to_json.py [--agents-md .llms/canon-for-agents.md]
"""

import json
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
CANON_DIR = ROOT / "docs" / "canon"
FAMILIES = ["board", "forecasts", "productivity", "priorities", "earthworks", "plans", "philosophy"]
FAMILY_TITLES = {
    "board": "The issues board",
    "forecasts": "Forecasts",
    "productivity": "Machine productivity",
    "priorities": "Priorities",
    "earthworks": "Earthworks & designations",
    "plans": "Plans",
    "philosophy": "Play philosophy",
}


def load_canon():
    families = {}
    for fam in FAMILIES:
        path = CANON_DIR / f"{fam}.yaml"
        entries = yaml.safe_load(path.read_text(encoding="utf-8"))
        assert isinstance(entries, list), f"{path}: expected a list of entries"
        for e in entries:
            for key in ("id", "title", "player", "rule", "source", "status"):
                assert key in e, f"{path}: entry missing {key}: {e.get('id')}"
            e.setdefault("numbers", [])
        families[fam] = entries
    return families


def main():
    families = load_canon()
    merged = {
        "families": {
            fam: {"title": FAMILY_TITLES[fam], "entries": entries}
            for fam, entries in families.items()
        }
    }
    out = CANON_DIR / "canon.json"
    out.write_text(json.dumps(merged, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    total = sum(len(v) for v in families.values())
    print(f"canon.json written: {len(families)} families, {total} entries")

    if "--agents-md" in sys.argv:
        path = Path(sys.argv[sys.argv.index("--agents-md") + 1])
        lines = [
            "# Canon — agent rendering",
            "",
            "GENERATED from docs/canon/*.yaml by tools/canon_to_json.py.",
            "Do not edit. Cite entries by id; the YAML files are the source.",
            "",
        ]
        for fam, entries in families.items():
            lines.append(f"## {FAMILY_TITLES[fam]} (`{fam}`)")
            lines.append("")
            for e in entries:
                lines.append(f"### {e['id']} — {e['title']}")
                lines.append("")
                lines.append(f"Rule: {e['rule'].strip()}")
                lines.append("")
                if e["numbers"]:
                    lines.append(
                        "Numbers: "
                        + "; ".join(
                            f"{n['name']} = {n['value']} {n.get('unit', '')}".strip()
                            for n in e["numbers"]
                        )
                    )
                    lines.append("")
                lines.append(f"Source: {e['source']} · Status: {e['status']}")
                lines.append("")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(lines), encoding="utf-8")
        print(f"agent rendering written: {path}")


if __name__ == "__main__":
    main()
