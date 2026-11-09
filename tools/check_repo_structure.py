#!/usr/bin/env python3
"""Repository structure linter (DESIGN V3 restructure, 2026-11-09).

The lean rule, enforced mechanically:
  1. ROOT ALLOWLIST — the repo root holds only the player-facing
     trio (README.md, INSTALL.md, REMOTE-ACCESS.md), SECURITY.md,
     the agent entry pair (AGENTS.md, CLAUDE.md), repo
     infrastructure dotfiles (.gitignore, .editorconfig,
     .pre-commit-config.yaml, .markdownlint.json, .yamllint.yml,
     pyproject.toml, LICENSE) and the declared folders
     (coi/ bridge/ qos/ plans/ tools/ docs/ dev/ .llms/ .github/).
     Anything else at the top level fails.
  2. README RULE — every component folder carries a readme at its
     own top level (README.md; a mod's readme.txt where packaging
     requires it; qos/ may use SUITE-README.md, .llms/ uses its
     llms.txt index).
  3. REFERENCE INTEGRITY — every relative link in the repo's
     markdown resolves to an existing file.
  4. MOD INVARIANT — every mod folder (bridge/CoiBridge, qos/*)
     keeps manifest.json + readme.txt at its root (the shipped
     layout the game expects).
  5. INDEX FRESHNESS — .llms/llms.txt lists exactly the agent-facing
     set: every agent doc is listed, every listed path exists.

Scope: the committed tree. LOCAL_ONLY names below are Philip's
local data that is never committed (captures, databases, scratch)
and is out of scope by definition — the list is part of the rule,
not an escape hatch: adding to it is a structure change.

Exit 0 = structure holds. Exit 1 = violations (listed).
Run from the repo root:  python3 tools/check_repo_structure.py
"""

import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

ROOT_FILES = {
    "README.md",
    "INSTALL.md",
    "REMOTE-ACCESS.md",
    "SECURITY.md",
    "AGENTS.md",
    "CLAUDE.md",
    "LICENSE",
    ".gitignore",
    ".editorconfig",
    ".pre-commit-config.yaml",
    ".markdownlint.json",
    ".yamllint.yml",
    "pyproject.toml",
}
ROOT_DIRS = {"coi", "bridge", "qos", "plans", "tools", "docs", "dev", ".llms", ".github"}
LOCAL_ONLY_FILES = {"ACCESS.md", "api-samples-0.8.8.md"}
LOCAL_ONLY_DIRS = {
    "live",
    "live-check",
    "live-synthetic-v06",
    "live-synthetic-v064",
    "history-states",
    "snapshots",
    "vendor",
    "qol-recon",
    "refs",
    "dist",
}
SKIP_DIR_NAMES = {"target", "bin", "obj", "__pycache__", ".git", ".ruff_cache"}


def is_local_only(rel: Path) -> bool:
    parts = rel.parts
    if not parts:
        return False
    if parts[0] in LOCAL_ONLY_DIRS or parts[0] in SKIP_DIR_NAMES:
        return True
    if len(parts) == 1 and (
        parts[0] in LOCAL_ONLY_FILES
        or parts[0].startswith(".repush")
        or parts[0].startswith("coi.db")
    ):
        return True
    return any(p in SKIP_DIR_NAMES for p in parts)


def repo_files():
    out = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        rel_dir = Path(dirpath).relative_to(ROOT)
        if is_local_only(rel_dir) and str(rel_dir) != ".":
            dirnames[:] = []
            continue
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIR_NAMES]
        for f in filenames:
            rel = rel_dir / f
            if not is_local_only(rel):
                out.append(rel)
    return out


def main():
    problems = []
    files = repo_files()
    file_set = {str(f) for f in files}

    # 1. Root allowlist
    for entry in sorted(os.listdir(ROOT)):
        rel = Path(entry)
        if entry in (".git",) or is_local_only(rel):
            continue
        if (ROOT / entry).is_dir():
            if entry not in ROOT_DIRS:
                problems.append(f"root: undeclared folder {entry}/")
        else:
            if entry not in ROOT_FILES:
                problems.append(f"root: undeclared file {entry}")

    # 2. README rule
    def has_readme(d: Path, extra=()):
        names = {p.name for p in d.iterdir() if p.is_file()} if d.exists() else set()
        return bool(names & ({"README.md", "readme.txt"} | set(extra)))

    # Build-output staging folders (build/, vm-build/) are not
    # components; a mod folder is one containing a .csproj.
    output_dirs = {"build", "vm-build"}
    components = [
        ROOT / "coi",
        ROOT / "plans",
        ROOT / "tools",
        ROOT / "docs",
        ROOT / "dev",
        ROOT / "bridge",
    ]
    components += (
        sorted(p for p in (ROOT / "bridge").glob("*") if p.is_dir() and p.name not in output_dirs)
        if (ROOT / "bridge").exists()
        else []
    )
    components += (
        sorted(p for p in (ROOT / "qos").glob("*") if p.is_dir() and p.name not in output_dirs)
        if (ROOT / "qos").exists()
        else []
    )
    for d in components:
        extra = ("SUITE-README.md",) if d == ROOT / "qos" else ()
        if not has_readme(d, extra):
            problems.append(f"readme: {d.relative_to(ROOT)}/ has no top-level readme")
    if (ROOT / "qos").exists() and not has_readme(ROOT / "qos", ("SUITE-README.md",)):
        problems.append("readme: qos/ has no top-level readme")
    llms = ROOT / ".llms"
    if not llms.exists() or not (llms / "llms.txt").exists():
        problems.append("readme: .llms/ missing or has no llms.txt index")

    # 3. Reference integrity (markdown links)
    link_re = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")
    for rel in files:
        if rel.suffix != ".md":
            continue
        text = (ROOT / rel).read_text(encoding="utf-8", errors="replace")
        for m in link_re.finditer(text):
            target = m.group(1).strip("<>")
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = target.split("#")[0].split("?")[0]
            if not target:
                continue
            resolved = (rel.parent / target).as_posix()
            # normalize ./ and ../
            resolved = os.path.normpath(resolved)
            if resolved not in file_set and not (ROOT / resolved).is_dir():
                problems.append(f"link: {rel} -> {target} (missing)")

    # 4. Mod invariant
    mod_dirs = []
    if (ROOT / "bridge" / "CoiBridge").exists():
        mod_dirs.append(ROOT / "bridge" / "CoiBridge")
    if (ROOT / "qos").exists():
        mod_dirs += sorted(
            p for p in (ROOT / "qos").glob("*") if p.is_dir() and any(p.glob("*.csproj"))
        )
    for d in mod_dirs:
        for req in ("manifest.json", "readme.txt"):
            if not (d / req).exists():
                problems.append(f"mod: {d.relative_to(ROOT)}/ missing {req} at its root")

    # 5. .llms index freshness
    idx = ROOT / ".llms" / "llms.txt"
    if idx.exists():
        listed = set()
        for m in link_re.finditer(idx.read_text(encoding="utf-8")):
            t = m.group(1).split("#")[0]
            if t and not t.startswith(("http://", "https://")):
                listed.add(os.path.normpath((Path(".llms") / t).as_posix()))
        on_disk = {str(f) for f in files if f.parts[0] == ".llms" and f.suffix == ".md"}
        for f in sorted(on_disk - listed):
            problems.append(f"llms: {f} not listed in .llms/llms.txt")
        for t in sorted(listed):
            if t.startswith(".llms/") and t not in file_set:
                problems.append(f"llms: index lists missing file {t}")

    if problems:
        print(f"STRUCTURE VIOLATIONS ({len(problems)}):")
        for p in problems:
            print(f"  ✗ {p}")
        return 1
    print(f"repo structure OK ({len(files)} files in scope)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
