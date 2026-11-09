#!/usr/bin/env python3
"""Mod-package linter (DESIGN V3, 2026-11-09) — the Hub-submission gate.

Sibling to check_serializer_pairs.py. Two modes:

  source mode (default): lint the mod folders in bridge/ + qos/
  --zips [DIR]:          lint the packages AS SHIPPED — the zips in
                         DIR (default dist/weekend-bundle/, the
                         current shipped set)

Rules (errors fail; warnings print but pass):
  1. manifest.json parses; required fields present (authors,
     can_add_to_saved_game, can_remove_from_saved_game,
     description_short, display_name, id, min_game_version,
     max_verified_game_version, non_locking_dll_load, primary_dlls,
     version); id == folder name; version is semver;
     min_game_version <= max_verified_game_version; game versions
     well-formed (X.Y.Z).
  2. readme.txt line 1 carries the manifest version and opens with
     the display name (or its part before an em-dash); the readme's
     "For Captain of Industry vX" line matches the manifest's
     verified range (X == max_verified_game_version).
  3. mod_dependencies ("Name >= X.Y.Z") resolve against the linted
     set and are satisfied by the shipped versions.
  4. can_remove_from_saved_game == false for any mod whose code
     subscribes to saved calendar events — the subscriber set is
     discovered by check_serializer_pairs.discover (one truth, two
     gates). Source mode only (zips carry no source).
  5. primary_dlls all present in the package; no unlisted DLL
     ships; no stray files beyond manifest.json, readme.txt, the
     DLLs, config.json (a declared asset of the QoS mods) and, in
     source mode, the sources themselves (*.cs, *.csproj).
  6. Privacy sweep over every text file: no C:\\Users\\ paths, no
     email addresses, no tailnet IPs (100.64.0.0/10), no machine
     names or home paths from the 2026-11-09 cleanup scrub list.
  7. Suite consistency: version claims about a mod inside
     SUITE-README ("QoSBuilder 0.2.0", "QoSCore >= 0.2.0", a mod's
     CONTENTS entry) must not contradict its manifest; the authors
     field is uniform across the suite.
  8. description_short: missing/empty or > 400 chars is an ERROR;
     > 200 chars is a WARNING (store cards truncate around there;
     QoSCore ships ~312 today and carries the warning knowingly).

Exit 0 = clean (warnings allowed). Exit 1 = errors.
"""

import importlib.util
import json
import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = [
    "authors",
    "can_add_to_saved_game",
    "can_remove_from_saved_game",
    "description_short",
    "display_name",
    "id",
    "min_game_version",
    "max_verified_game_version",
    "non_locking_dll_load",
    "primary_dlls",
    "version",
]
SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
VERSION_IN_TEXT = re.compile(r"v?(\d+\.\d+\.\d+)")
FOR_GAME = re.compile(r"For Captain of Industry\s+v(\d+\.\d+\.\d+)")
PRIVACY = [
    # A bracketed placeholder (C:\Users\<your-name>, %USERNAME%) is
    # documentation, not a leak; a concrete name is the leak.
    (re.compile(r"[A-Za-z]:\\Users\\(?![<%])[\w.-]+"), "absolute user path"),
    (re.compile(r"[\w.+-]+@[\w-]+\.[\w.]+"), "email address"),
    (re.compile(r"\b100\.(6[4-9]|[7-9]\d|1[0-1]\d|12[0-7])\.\d+\.\d+\b"), "tailnet IP"),
    (
        re.compile(r"cokof|philip-gaming-home|/home/hatch", re.I),
        "personal identifier (cleanup scrub list)",
    ),
]
DESC_WARN = 200
DESC_ERROR = 400


def vtuple(v):
    return tuple(int(x) for x in v.split("."))


class Package:
    def __init__(self, name):
        self.name = name  # folder name (== manifest id)
        self.files = {}  # relpath -> bytes (text-decodable subset used)
        self.names = []  # all entry names


def packages_from_source():
    pkgs = []
    roots = [ROOT / "bridge" / "CoiBridge"]
    roots += sorted(
        p for p in (ROOT / "qos").glob("*") if p.is_dir() and (p / "manifest.json").exists()
    )
    for d in roots:
        pkg = Package(d.name)
        for f in sorted(d.rglob("*")):
            if f.is_file() and not any(
                part in ("bin", "obj", "stage", "__pycache__") for part in f.parts
            ):
                rel = f.relative_to(d).as_posix()
                pkg.names.append(rel)
                if f.suffix in (".json", ".txt", ".cs", ".csproj", ".md"):
                    pkg.files[rel] = f.read_bytes()
        pkgs.append(pkg)
    return pkgs


def packages_from_zips(zipdir):
    pkgs = {}
    for z in sorted(Path(zipdir).glob("*.zip")):
        with zipfile.ZipFile(z) as zf:
            for info in zf.infolist():
                if info.is_dir():
                    continue
                parts = info.filename.split("/")
                if len(parts) < 2:
                    continue  # zip-root files (e.g. QoS-SUITE-README.txt)
                top = parts[0]
                pkg = pkgs.setdefault(top, Package(top))
                rel = "/".join(parts[1:])
                pkg.names.append(rel)
                if Path(rel).suffix in (".json", ".txt", ".cs", ".md"):
                    pkg.files[rel] = zf.read(info)
    return list(pkgs.values())


def subscribing_mods():
    """Mod folders whose code subscribes to saved calendar events,
    per check_serializer_pairs' discovery (imported, not copied)."""
    spec = importlib.util.spec_from_file_location(
        "serializer_pairs", ROOT / "tools" / "check_serializer_pairs.py"
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    mods = set()
    for path, _cls, _off in mod.discover(ROOT):
        rel = path.relative_to(ROOT)
        mods.add(rel.parts[1] if rel.parts[0] == "qos" else "CoiBridge")
    return mods


def lint(pkgs, source_mode, errors, warnings):
    by_id = {}
    manifests = {}
    for pkg in pkgs:
        raw = pkg.files.get("manifest.json")
        if raw is None:
            errors.append(f"{pkg.name}: manifest.json missing")
            continue
        try:
            m = json.loads(raw)
        except Exception as e:
            errors.append(f"{pkg.name}: manifest.json does not parse ({e})")
            continue
        manifests[pkg.name] = m
        by_id[m.get("id")] = pkg.name
        # Rule 1
        for f in REQUIRED:
            if f not in m:
                errors.append(f"{pkg.name}: manifest missing field {f}")
        if m.get("id") != pkg.name:
            errors.append(f"{pkg.name}: manifest id {m.get('id')!r} != folder name")
        if not SEMVER.match(str(m.get("version", ""))):
            errors.append(f"{pkg.name}: version {m.get('version')!r} is not semver")
        for gf in ("min_game_version", "max_verified_game_version"):
            if not SEMVER.match(str(m.get(gf, ""))):
                errors.append(f"{pkg.name}: {gf} {m.get(gf)!r} not well-formed X.Y.Z")
        if (
            SEMVER.match(str(m.get("min_game_version", "")))
            and SEMVER.match(str(m.get("max_verified_game_version", "")))
            and vtuple(m["min_game_version"]) > vtuple(m["max_verified_game_version"])
        ):
            errors.append(f"{pkg.name}: min_game_version > max_verified_game_version")
        # Rule 8
        desc = m.get("description_short") or ""
        if not desc:
            errors.append(f"{pkg.name}: description_short empty")
        elif len(desc) > DESC_ERROR:
            errors.append(f"{pkg.name}: description_short {len(desc)} chars > {DESC_ERROR}")
        elif len(desc) > DESC_WARN:
            warnings.append(
                f"{pkg.name}: description_short {len(desc)} chars > {DESC_WARN} (card truncation risk)"
            )
        # Rule 2
        readme = pkg.files.get("readme.txt")
        if readme is None:
            errors.append(f"{pkg.name}: readme.txt missing")
        else:
            text = readme.decode("utf-8", errors="replace")
            line1 = text.splitlines()[0] if text.splitlines() else ""
            vm = VERSION_IN_TEXT.search(line1)
            if not vm or vm.group(1) != m.get("version"):
                errors.append(
                    f"{pkg.name}: readme line 1 version "
                    f"{vm.group(1) if vm else None!r} != manifest {m.get('version')!r}"
                )
            display_head = str(m.get("display_name", "")).split("—")[0].strip()
            if display_head and not line1.startswith(display_head):
                errors.append(
                    f"{pkg.name}: readme line 1 does not open with display name {display_head!r}"
                )
            gm = FOR_GAME.search(text)
            if not gm:
                errors.append(f"{pkg.name}: readme lacks the 'For Captain of Industry vX' line")
            elif gm.group(1) != m.get("max_verified_game_version"):
                errors.append(
                    f"{pkg.name}: readme game version {gm.group(1)} != manifest max_verified {m.get('max_verified_game_version')}"
                )
        # Rule 5
        declared = set(m.get("primary_dlls") or [])
        shipped_dlls = {n for n in pkg.names if n.endswith(".dll")}
        # Presence is a shipped-package property; source folders
        # legitimately hold no built DLLs (they build into staging).
        if not source_mode:
            for d in declared:
                if d not in pkg.names:
                    errors.append(f"{pkg.name}: primary dll {d} not in package")
        for d in shipped_dlls - declared:
            errors.append(f"{pkg.name}: unlisted dll ships: {d}")
        allowed_extra = {"manifest.json", "readme.txt", "config.json"}
        for n in pkg.names:
            if n in allowed_extra or n in declared:
                continue
            if source_mode and (n.endswith(".cs") or n.endswith(".csproj")):
                continue
            errors.append(f"{pkg.name}: stray file in package: {n}")
        # Rule 6
        for rel, data in pkg.files.items():
            text = data.decode("utf-8", errors="replace")
            for rx, label in PRIVACY:
                hit = rx.search(text)
                if hit:
                    errors.append(f"{pkg.name}/{rel}: privacy sweep — {label}: {hit.group(0)!r}")
    # Rule 3 (cross-package)
    for pkg in pkgs:
        m = manifests.get(pkg.name)
        if not m:
            continue
        for dep in m.get("mod_dependencies") or []:
            dm = re.match(r"(\S+)\s*>=\s*(\d+\.\d+\.\d+)$", str(dep).strip())
            if not dm:
                errors.append(f"{pkg.name}: unparseable dependency {dep!r}")
                continue
            dep_name, dep_min = dm.group(1), dm.group(2)
            target = manifests.get(dep_name) or manifests.get(by_id.get(dep_name, ""))
            if target is None:
                errors.append(f"{pkg.name}: dependency {dep_name} not in the shipped set")
            elif vtuple(target["version"]) < vtuple(dep_min):
                errors.append(
                    f"{pkg.name}: dependency {dep_name} >= {dep_min} not satisfied by {target['version']}"
                )
    # Rule 4 (source mode)
    if source_mode:
        for name in subscribing_mods():
            m = manifests.get(name)
            if m and m.get("can_remove_from_saved_game") is not False:
                errors.append(
                    f"{name}: code subscribes to saved calendar events but "
                    "can_remove_from_saved_game != false"
                )
    # Rule 7 (suite consistency, source mode: SUITE-README on disk)
    if source_mode:
        suite = ROOT / "qos" / "SUITE-README.txt"
        if suite.exists():
            text = suite.read_text(encoding="utf-8", errors="replace")
            authors = {
                tuple(manifests[n].get("authors") or []) for n in manifests if n.startswith("QoS")
            }
            if len(authors) > 1:
                errors.append(f"suite: authors field not uniform: {authors}")
            for name, m in manifests.items():
                for claim in re.finditer(re.escape(name) + r"\s*(?:>=\s*)?v?(\d+\.\d+\.\d+)", text):
                    if claim.group(0).startswith(name + " >="):
                        if vtuple(m["version"]) < vtuple(claim.group(1)):
                            errors.append(
                                f"suite: SUITE-README requires {name} >= {claim.group(1)} but manifest is {m['version']}"
                            )
                    elif claim.group(1) != m["version"]:
                        errors.append(
                            f"suite: SUITE-README claims {name} {claim.group(1)} but manifest is {m['version']}"
                        )
    return manifests


def main(argv):
    source_mode = "--zips" not in argv
    errors, warnings = [], []
    if source_mode:
        pkgs = packages_from_source()
        label = "source folders"
    else:
        i = argv.index("--zips")
        zipdir = argv[i + 1] if i + 1 < len(argv) else ROOT / "dist" / "weekend-bundle"
        pkgs = packages_from_zips(zipdir)
        label = f"zips in {zipdir}"
    manifests = lint(pkgs, source_mode, errors, warnings)
    for w in warnings:
        print(f"  warn: {w}")
    if errors:
        print(f"MOD PACKAGE VIOLATIONS ({len(errors)}) over {label}:")
        for e in errors:
            print(f"  ✗ {e}")
        return 1
    print(f"mod packages OK ({len(manifests)} packages over {label}, {len(warnings)} warning(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
