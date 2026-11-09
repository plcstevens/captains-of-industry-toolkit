#!/usr/bin/env python3
"""Canon drift check: fail if a coded constant disagrees with docs/canon.

For every canon number carrying a `code` reference:
  contains-check   — the cited file must contain the literal string (which
                     embeds the value, e.g. "cover < 36.0").
  extractor-check  — importance_high / importance_low parse the product
                     lists out of coi/src/main.rs and compare counts (and
                     the canon YAML may pin the member lists too).
Also verifies: every entry id cited in Rust/C# sources as `canon: <id>`
exists in the canon, and every canon family renders into canon.json
(regenerate with tools/canon_to_json.py if stale).

Exit 0 = agreement. Exit 1 = drift (details on stdout). Numbers without a
code reference are reported as UNWIRED (prose/heuristic — allowed, listed).

Run from the repo root:  python3 tools/check_canon.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CANON_DIR = ROOT / "docs" / "canon"
sys.path.insert(0, str(ROOT / "tools"))
from canon_to_json import load_canon  # noqa: E402


def extract_importance(which):
    src = (ROOT / "coi/src/main.rs").read_text(encoding="utf-8")
    out = {}
    for name in ("HIGH", "LOW"):
        m = re.search(rf"const {name}: &\[&str\] = &\[(.*?)\];", src, re.S)
        assert m, f"importance {name} array not found"
        out[name] = re.findall(r'"([A-Za-z]+)"', m.group(1))
    return out["HIGH"] if which == "importance_high" else out["LOW"]


def main():
    families = load_canon()
    problems, unwired, checked = [], [], 0
    ids = set()
    for _fam, entries in families.items():
        for e in entries:
            ids.add(e["id"])
            for n in e.get("numbers", []):
                ref = n.get("code")
                if not ref:
                    unwired.append(f"{e['id']}.{n['name']} (no code ref)")
                    continue
                checked += 1
                path = ROOT / ref["file"]
                if not path.exists():
                    problems.append(f"{e['id']}.{n['name']}: file missing: {ref['file']}")
                    continue
                text = path.read_text(encoding="utf-8")
                if "contains" in ref:
                    if ref["contains"] not in text:
                        problems.append(
                            f"{e['id']}.{n['name']}: {ref['file']} does not contain "
                            f"{ref['contains']!r} (canon value {n['value']})"
                        )
                    else:
                        nums = [float(x) for x in re.findall(r"\d+(?:\.\d+)?", ref["contains"])]
                        if nums and float(n["value"]) not in nums:
                            problems.append(
                                f"{e['id']}.{n['name']}: value {n['value']} not among literals {nums} in the cited string"
                            )
                elif "extractor" in ref:
                    got = extract_importance(ref["extractor"])
                    if len(got) != int(n["value"]):
                        problems.append(
                            f"{e['id']}.{n['name']}: code list has {len(got)} members, "
                            f"canon says {n['value']}: {got}"
                        )
                    pinned = n.get("members")
                    if pinned and sorted(pinned) != sorted(got):
                        problems.append(
                            f"{e['id']}.{n['name']}: member mismatch code={sorted(got)} canon={sorted(pinned)}"
                        )
    # citations in code must resolve
    cited = set()
    for src in list((ROOT / "coi/src").glob("*.rs")) + list((ROOT / "qos").rglob("*.cs")):
        cited |= set(re.findall(r"canon: ([a-z0-9.\-]+)", src.read_text(encoding="utf-8")))
    for c in sorted(cited - ids):
        problems.append(f"code cites unknown canon id: {c}")
    # canon.json freshness
    cj = CANON_DIR / "canon.json"
    if not cj.exists():
        problems.append("docs/canon/canon.json missing — run tools/canon_to_json.py")
    else:
        data = json.loads(cj.read_text(encoding="utf-8"))
        json_ids = {e["id"] for f in data["families"].values() for e in f["entries"]}
        if json_ids != ids:
            problems.append("canon.json is stale — run tools/canon_to_json.py")

    print(
        f"canon check: {checked} wired numbers checked, {len(unwired)} unwired "
        f"(prose/heuristic), {len(cited)} code citations"
    )
    for u in unwired:
        print(f"  unwired: {u}")
    if problems:
        print("DRIFT:")
        for p in problems:
            print(f"  ✗ {p}")
        return 1
    print("canon agrees with code ✓")
    return 0


if __name__ == "__main__":
    sys.exit(main())
