#!/usr/bin/env python3
"""Generate coi/data/blueprint-library.json from the blueprint pack.

The companion's /place page embeds this file (include_str!) so the
library travels inside coi.exe — Philip's PC has no pack folder.
Source of truth: ~/workspace/coi-blueprint-pack/ — every <slug>.txt
is one layout (82; `philip-current-export.txt` is Philip's own
export, NOT a pack layout, and is excluded), the sibling <slug>.json
carries the display metadata, and the codec at
~/workspace/skills/coi-blueprints/references/coi_bp.py provides the
per-item decode the entity summary is built from.

Regenerate after any pack change:
    python3 tools/gen_blueprint_library.py

Every entry is decode-verified here: a string that fails the codec
aborts the generation — the page must never offer a blueprint we
cannot describe honestly. Item counts are the DECODED counts (the
pack manifest's entity_count values disagree with the strings in
both directions; the decoded count is what a placement would send).
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACK = ROOT.parent / "coi-blueprint-pack"
CODEC = Path.home() / "workspace" / "skills" / "coi-blueprints" / "references"
OUT = ROOT / "coi" / "data" / "blueprint-library.json"
EXCLUDE = {"philip-current-export"}

sys.path.insert(0, str(CODEC))
import coi_bp  # noqa: E402


def main() -> int:
    manifest = json.loads((PACK / "PACK_MANIFEST.json").read_text())
    manifest_by_slug = {}
    for phase in manifest["phases"]:
        for bp in phase["blueprints"]:
            manifest_by_slug[bp["slug"]] = bp

    entries = []
    for txt_path in sorted(PACK.glob("*.txt")):
        slug = txt_path.stem
        if slug in EXCLUDE:
            continue
        meta = json.loads((PACK / f"{slug}.json").read_text())
        man = manifest_by_slug.get(slug, {})
        string = txt_path.read_text().strip()
        decoded = coi_bp.decode_blueprint(string)
        if decoded["kind"] != "blueprint":
            print(f"{slug}: decodes as {decoded['kind']}, not a blueprint")
            return 1
        kinds = {"buildings": 0, "transports": 0, "trees": 0, "other": 0}
        protos = {}
        for e in decoded["entities"]:
            proto = e["strings"].get("Prototype", "?")
            protos[proto] = protos.get(proto, 0) + 1
            if "trajectory" in e:
                kinds["transports"] += 1
            elif "transform" in e:
                kinds["trees" if "Tree" in proto else "buildings"] += 1
            else:
                kinds["other"] += 1
        entries.append({
            "slug": slug,
            "name": meta.get("name") or man.get("name") or slug,
            "module": meta.get("module") or man.get("module") or "",
            "layer": meta.get("layer") or man.get("layer") or "",
            "version": meta.get("version") or man.get("version") or 0,
            "research_gate": meta.get("research_gate")
                or man.get("research_gate") or "",
            "item_count": len(decoded["entities"]),
            "kinds": kinds,
            "protos": dict(sorted(protos.items())),
            "string": string,
        })
    entries.sort(key=lambda e: e["slug"])
    doc = {
        "generated_by": "tools/gen_blueprint_library.py",
        "pack": manifest["pack"],
        "testing_status": manifest["testing_status_global"],
        "count": len(entries),
        "entries": entries,
    }
    OUT.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    print(f"wrote {OUT} — {len(entries)} blueprints")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
