#!/usr/bin/env python3
"""Audit plan `blueprint_ref` values against the CoI blueprint pack.

Plans (plans/*.yaml) name blueprints with logical refs such as
"starter-cluster/maintenance-depot" or "spine/north-south-trunk-with-
district-sidings". The blueprint pack (~/workspace/coi-blueprint-pack/)
is the authority on what actually exists. This tool cross-checks every
ref so a plan never silently points at a blueprint that was neither
authored nor declared as missing.

What counts as "exists" in the pack (the complete, mechanical rule):

  1. Layout file stems at the pack ROOT: the stem of every *.txt or
     *.json file directly in the pack root (e.g. "steel-v1" from
     steel-v1.txt / steel-v1.json), EXCLUDING pack metadata files:
     PACK_MANIFEST.json, NEW_MODULES_*.json and section_log.json.
     (This mechanically includes "philip-current-export", Philip's own
     export .txt sitting at the pack root; it is harmless — no plan ref
     can match it by accident — and excluding it by name would be a
     judgement call, not a mechanical rule.)
  2. Every "slug" value under phases[].blueprints[] in
     PACK_MANIFEST.json.
  3. Every "slug" value in the entries of NEW_MODULES_*.json (each is a
     JSON list of module dicts), plus the top-level "slug" field of each
     root layout *.json (verified in practice to equal its file stem).

  Sources 2 and 3 are currently strict subsets of source 1, but they are
  included so a blueprint registered in the manifest / module files
  before its layout files land still counts as existing.

Verdicts per ref (checked in this order):

  AUTHORED           the order declares `asset_status: authored` and its
                     `pack_slug` (plus `pack_slug_overlay`, when present)
                     exists in the pack. Added 2026-11-04 (Round 8
                     chunk 2) when plans began carrying pack_slug: an
                     authored ref whose slug is NOT in the pack falls
                     through to MISSING-UNDECLARED, never a silent pass.
  FOUND              the full ref string is exactly a pack id.
  FOUND-BY-LEAF      only the ref's last path segment matches a pack id.
                     Reported as a CANDIDATE mapping for a human to
                     confirm — never treated as a silent pass.
  MISSING-DECLARED   no match, but the order declares
                     `asset_status: not-yet-authored`. Expected; the
                     blueprint is known to still need authoring.
  MISSING-UNDECLARED no match and no such declaration. A real problem:
                     the plan points at a blueprint nobody has flagged
                     as missing.

Exit code: 0 if there are no MISSING-UNDECLARED refs, else 1.

Usage: python3 tools/audit_blueprints.py [plan.yaml ...]
       [--pack-dir DIR]
Defaults to both plans in plans/ and the pack at
<workspace>/coi-blueprint-pack (sibling of this repo).
"""
import argparse
import json
import pathlib
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_PLANS = [
    ROOT / "plans" / "fresh-start-v1.yaml",
    ROOT / "plans" / "dragon-isle-continuation-v1.yaml",
]
DEFAULT_PACK = ROOT.parent / "coi-blueprint-pack"

METADATA_JSON = {"PACK_MANIFEST.json", "section_log.json"}


def build_pack_ids(pack_dir):
    """Return {pack_id: set(source descriptions)} per the docstring rule."""
    ids = {}

    def add(pid, source):
        if isinstance(pid, str) and pid:
            ids.setdefault(pid, set()).add(source)

    # Source 1 (+ the slug half of source 3): root layout files.
    for path in sorted(pack_dir.iterdir()):
        if not path.is_file() or path.suffix not in (".txt", ".json"):
            continue
        if path.name in METADATA_JSON or path.name.startswith("NEW_MODULES_"):
            continue
        add(path.stem, f"layout file {path.name}")
        if path.suffix == ".json":
            try:
                data = json.loads(path.read_text())
            except (json.JSONDecodeError, OSError):
                data = None
            if isinstance(data, dict):
                add(data.get("slug"), f"slug field of {path.name}")

    # Source 2: PACK_MANIFEST.json slugs.
    manifest = pack_dir / "PACK_MANIFEST.json"
    if manifest.is_file():
        try:
            data = json.loads(manifest.read_text())
        except (json.JSONDecodeError, OSError):
            data = None
        if isinstance(data, dict):
            for phase in data.get("phases", []):
                if isinstance(phase, dict):
                    for bp in phase.get("blueprints", []):
                        if isinstance(bp, dict):
                            add(bp.get("slug"), "PACK_MANIFEST.json slug")

    # Source 3: NEW_MODULES_*.json entry slugs.
    for path in sorted(pack_dir.glob("NEW_MODULES_*.json")):
        try:
            data = json.loads(path.read_text())
        except (json.JSONDecodeError, OSError):
            continue
        entries = data if isinstance(data, list) else []
        if isinstance(data, dict) and isinstance(data.get("modules"), list):
            entries = data["modules"]
        for entry in entries:
            if isinstance(entry, dict):
                add(entry.get("slug"), f"slug in {path.name}")

    return ids


def extract_refs(node, plan_name, order_id=None, out=None):
    """Walk parsed YAML; record every mapping carrying `blueprint_ref`.

    The order id is the `id` of the mapping that carries the ref when it
    has one, otherwise the nearest enclosing mapping's `id`.
    """
    if out is None:
        out = []
    if isinstance(node, dict):
        current_id = node.get("id", order_id)
        if "blueprint_ref" in node:
            out.append({
                "plan": plan_name,
                "order_id": current_id,
                "blueprint_ref": node.get("blueprint_ref"),
                "asset_status": node.get("asset_status"),
                "pack_slug": node.get("pack_slug"),
                "pack_slug_overlay": node.get("pack_slug_overlay"),
            })
        for value in node.values():
            extract_refs(value, plan_name, current_id, out)
    elif isinstance(node, list):
        for item in node:
            extract_refs(item, plan_name, order_id, out)
    return out


def verdict_for(ref_entry, pack_ids):
    """Return (verdict, note)."""
    ref = ref_entry["blueprint_ref"]
    slug = ref_entry.get("pack_slug")
    overlay = ref_entry.get("pack_slug_overlay")
    if ref_entry["asset_status"] == "authored" and slug:
        if slug in pack_ids and (not overlay or overlay in pack_ids):
            note = f"authored: pack_slug -> {slug}"
            if overlay:
                note += f" + overlay {overlay}"
            note += " (" + ", ".join(sorted(pack_ids[slug])) + ")"
            return "AUTHORED", note
        bad = slug if slug not in pack_ids else overlay
        return ("MISSING-UNDECLARED",
                f"asset_status authored but pack_slug {bad} not in pack")
    declared = ref_entry["asset_status"] == "not-yet-authored"
    if isinstance(ref, str) and ref in pack_ids:
        note = "exact match: " + ", ".join(sorted(pack_ids[ref]))
        if declared:
            note += "; NOTE asset_status says not-yet-authored but it exists"
        return "FOUND", note
    leaf = ref.rsplit("/", 1)[-1] if isinstance(ref, str) else ""
    if leaf and leaf in pack_ids:
        note = (f"candidate mapping -> {leaf} ("
                + ", ".join(sorted(pack_ids[leaf])) + "); needs human confirm")
        if declared:
            note += "; also declared not-yet-authored"
        return "FOUND-BY-LEAF", note
    if declared:
        return "MISSING-DECLARED", "asset_status: not-yet-authored (expected)"
    return "MISSING-UNDECLARED", "no pack match and no asset_status declaration"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("plans", nargs="*", type=pathlib.Path,
                        help="plan YAML files (default: both plans in plans/)")
    parser.add_argument("--pack-dir", type=pathlib.Path, default=DEFAULT_PACK,
                        help=f"blueprint pack directory (default: {DEFAULT_PACK})")
    args = parser.parse_args(argv)

    plans = args.plans or DEFAULT_PLANS
    pack_dir = args.pack_dir
    if not pack_dir.is_dir():
        print(f"ERROR: pack dir not found: {pack_dir}", file=sys.stderr)
        return 2

    pack_ids = build_pack_ids(pack_dir)
    print(f"Pack: {pack_dir} — {len(pack_ids)} blueprint ids")
    print()

    rows = []
    for plan_path in plans:
        data = yaml.safe_load(plan_path.read_text())
        for entry in extract_refs(data, plan_path.name):
            verdict, note = verdict_for(entry, pack_ids)
            rows.append((entry, verdict, note))

    if not rows:
        print("No blueprint_ref values found in the given plans.")
        return 0

    headers = ("PLAN", "ORDER", "BLUEPRINT_REF", "ASSET_STATUS", "VERDICT", "NOTE")
    cells = [(
        e["plan"],
        str(e["order_id"]),
        str(e["blueprint_ref"]),
        str(e["asset_status"] or "-"),
        verdict,
        note,
    ) for e, verdict, note in rows]
    widths = [max(len(headers[i]), *(len(r[i]) for r in cells)) for i in range(6)]

    def fmt(row):
        return "  ".join(c.ljust(widths[i]) for i, c in enumerate(row)).rstrip()

    print(fmt(headers))
    print(fmt(tuple("-" * w for w in widths)))
    for row in cells:
        print(fmt(row))

    counts = {}
    for _, verdict, _ in rows:
        counts[verdict] = counts.get(verdict, 0) + 1
    print()
    print(f"Refs audited: {len(rows)}")
    for verdict in ("AUTHORED", "FOUND", "FOUND-BY-LEAF", "MISSING-DECLARED", "MISSING-UNDECLARED"):
        if verdict in counts:
            print(f"  {verdict}: {counts[verdict]}")

    bad = counts.get("MISSING-UNDECLARED", 0)
    if bad:
        print(f"\nFAIL: {bad} MISSING-UNDECLARED ref(s) — plans point at "
              "blueprints that do not exist and are not declared missing.")
        return 1
    print("\nOK: no MISSING-UNDECLARED refs.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
