#!/usr/bin/env python3
"""plan_to_json.py — emit the canonical JSON form of a CoI game plan.

Round 4 (2026-11-03): the Rust companion gains `coi plan-check`, but the
coi crate has no YAML dependency and serde_yaml is absent from the
offline cargo cache — so the Rust engine consumes JSON. This converter
is the bridge: it loads a plan with the SAME parser the Python
reference evaluator (tools/plan_check.py) and the linter
(tools/validate_plan.py) use (PyYAML safe_load), and writes a canonical
JSON rendering next to the YAML:

    plans/<name>.yaml  ->  plans/<name>.json

Canonical = UTF-8, 2-space indent, keys sorted, one trailing newline.
Type fidelity notes (so the Rust engine sees what the Python engine
saw): PyYAML date/datetime values (e.g. an unquoted `created:
2026-11-02`) become ISO strings; non-string mapping keys are
stringified (JSON object keys are strings); tuples become arrays.
Anchors/merge keys are already resolved by safe_load.

Usage:
    python3 tools/plan_to_json.py [plan.yaml ...]
Defaults to both plans in plans/. Prints one line per conversion.
Exit code 0 on success, 1 on any parse/IO error.
"""
import datetime
import json
import pathlib
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent


def json_default(obj):
    if isinstance(obj, (datetime.date, datetime.datetime)):
        return obj.isoformat()
    raise TypeError(f"not JSON-serializable in a plan: {type(obj).__name__}")


def normalize(node):
    """Recursively rewrite a safe_load tree into JSON-safe shapes."""
    if isinstance(node, dict):
        return {str(k): normalize(v) for k, v in node.items()}
    if isinstance(node, (list, tuple)):
        return [normalize(v) for v in node]
    return node


def convert(path):
    doc = yaml.safe_load(path.read_text())
    if not isinstance(doc, dict) or "plan" not in doc or "phases" not in doc:
        print(f"ERROR: {path}: missing top-level plan/phases", file=sys.stderr)
        return 1
    out = path.with_suffix(".json")
    text = json.dumps(normalize(doc), indent=2, sort_keys=True,
                      ensure_ascii=False, default=json_default) + "\n"
    out.write_text(text)
    meta = doc["plan"] or {}
    print(f"{path.name} -> {out.name} "
          f"(plan {meta.get('id', '?')} v{meta.get('version', '?')}, "
          f"{len(doc['phases'])} phases, {len(text)} bytes)")
    return 0


def main():
    paths = [pathlib.Path(p) for p in sys.argv[1:]] or [
        ROOT / "plans" / "fresh-start-v1.yaml",
        ROOT / "plans" / "dragon-isle-continuation-v1.yaml",
    ]
    rc = 0
    for p in paths:
        if not p.exists():
            print(f"ERROR: plan file not found: {p}", file=sys.stderr)
            rc = 1
            continue
        try:
            rc |= convert(p)
        except yaml.YAMLError as exc:
            print(f"ERROR: {p}: YAML parse error: {exc}", file=sys.stderr)
            rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main())
