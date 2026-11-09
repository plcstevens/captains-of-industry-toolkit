#!/usr/bin/env python3
"""check_serializer_pairs.py — serializer-pair lint (QUALITY-AUDIT item 1).

THE RULE (suite 0.5.0 broke ALL saving; bridge-dev skill, MODDING-NOTES
round 11): the game's calendar events are SAVED events. Subscribing
with `event.Add(this, handler)` stores a CallbackSaveData that
generic-serializes THE OWNER into every save file. A concrete class
is generic-serializer-eligible only if it carries:

  * the [ManuallyWrittenSerialization] attribute (Mafi.Serialization —
    required; the generator's validation pass throws for unmarked
    types carrying the methods), and
  * a hand-written pair: public static void Serialize(T, BlobWriter)
    and public static T Deserialize(BlobReader),

and, because a subscribing service leaves a footprint in saves, its
mod manifest must set can_remove_from_saved_game: false.

DISCOVERY (not a hardcoded list): any `.Add(this, ...)` call in a
.cs file under qos/ or bridge/ is an EventBase subscription — in
this codebase the only two-argument Add(owner, callback) shape is
Mafi's saveable-event API (IGameLoopEvents, the one non-saveable
surface, registers via Register* methods, never Add). Each match is
attributed to the outermost class declaration preceding it in its
file (the subscribing DI service; nested helper classes declared
inside it must not steal the attribution).
For every discovered subscriber class this script asserts the
attribute, both pair members (in the same file), and the manifest
flag of the mod folder the file belongs to (nearest manifest.json
walking up from the file).

Usage:
    python3 tools/check_serializer_pairs.py [ROOT]
ROOT defaults to the repo root (parent of tools/). Exit 0 = every
discovered subscriber satisfies the rule; 1 = at least one failure;
2 = usage/discovery error (e.g. no subscribers found at all — the
net must never silently pass on an empty scan).
"""

import json
import pathlib
import re
import sys

SKIP_DIRS = {"obj", "bin", "stage", "stage-builder", "target", "__pycache__", ".git"}

ADD_THIS = re.compile(r"\.Add\(\s*this\s*,")
CLASS_DECL = re.compile(
    r"^(\s*)(?:public|internal|private|protected)?\s*"
    r"(?:sealed\s+|abstract\s+|static\s+|partial\s+)*class\s+(\w+)",
    re.M,
)


def has_attribute(window):
    """True if [ManuallyWrittenSerialization] appears as a real
    attribute line in `window` — not merely mentioned in a comment
    (the services' doc comments discuss the attribute by name, and
    a lint that passes on prose is worse than no lint)."""
    for line in window.splitlines():
        s = line.strip()
        if s.startswith("//"):
            continue
        if s.startswith("[") and "ManuallyWrittenSerialization]" in s:
            return True
    return False


def discover(root):
    """Yield (cs_path, class_name, class_offset) per subscribing class."""
    found = []
    for tree in ("qos", "bridge"):
        base = root / tree
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*.cs")):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            subs = [m.start() for m in ADD_THIS.finditer(text)]
            if not subs:
                continue
            # Owner attribution: the OUTERMOST class preceding the
            # subscription (smallest indent; latest wins ties). The
            # subscribing services are top-level DI classes; nested
            # helper classes (RecoveryMark, Observation) declared
            # inside them before the subscription line must not steal
            # the attribution.
            classes = [(m.start(), m.group(2), len(m.group(1))) for m in CLASS_DECL.finditer(text)]
            for pos in subs:
                preceding = [c for c in classes if c[0] < pos]
                if not preceding:
                    found.append((path, None, pos))
                    continue
                owner = min(preceding, key=lambda c: (c[2], -c[0]))
                if all(f[1] != owner[1] or f[0] != path for f in found):
                    found.append((path, owner[1], owner[0]))
    return found


def check_class(root, path, cls, class_off):
    """Return a list of failure strings for one subscriber class."""
    fails = []
    text = path.read_text(encoding="utf-8", errors="replace")
    rel = path.relative_to(root)
    if cls is None:
        return [
            f"{rel}: `.Add(this, ...)` subscription found but no "
            f"enclosing class declaration could be identified"
        ]
    # The attribute must sit in the block immediately preceding
    # THIS class declaration (it is 2 lines above in all four
    # services today; 500 chars is a generous but safe window that
    # cannot borrow a previous class's attribute).
    window = text[max(0, class_off - 500) : class_off]
    if not has_attribute(window):
        fails.append(
            f"{rel}: class {cls} subscribes with .Add(this, ...) but "
            f"has no [ManuallyWrittenSerialization] attribute"
        )
    if not re.search(r"public\s+static\s+void\s+Serialize\s*\(\s*" + re.escape(cls) + r"\b", text):
        fails.append(f"{rel}: class {cls} lacks `public static void Serialize({cls}, BlobWriter)`")
    if not re.search(
        r"public\s+static\s+" + re.escape(cls) + r"\s+Deserialize\s*\(\s*BlobReader", text
    ):
        fails.append(f"{rel}: class {cls} lacks `public static {cls} Deserialize(BlobReader)`")
    # Manifest flag: nearest manifest.json walking up from the file.
    manifest = None
    for parent in path.parents:
        cand = parent / "manifest.json"
        if cand.is_file():
            manifest = cand
            break
        if parent == root:
            break
    if manifest is None:
        fails.append(
            f"{rel}: class {cls} — no manifest.json found walking up; cannot verify can_remove flag"
        )
    else:
        try:
            data = json.loads(manifest.read_text(encoding="utf-8"))
        except Exception as exc:
            fails.append(f"{manifest.relative_to(root)}: unreadable ({exc})")
            return fails
        if data.get("can_remove_from_saved_game") is not False:
            fails.append(
                f"{manifest.relative_to(root)}: {cls} subscribes to a "
                f"saveable event, so can_remove_from_saved_game must be "
                f"false (found {data.get('can_remove_from_saved_game')!r})"
            )
    return fails


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    root = (
        pathlib.Path(argv[0]).resolve() if argv else pathlib.Path(__file__).resolve().parent.parent
    )
    subs = discover(root)
    if not subs:
        print(
            "serializer-lint: ERROR — no `.Add(this, ...)` subscribers "
            "discovered under qos/ or bridge/; the scan itself is "
            "suspect, refusing to pass an empty net",
            file=sys.stderr,
        )
        return 2
    all_fails = []
    for path, cls, off in subs:
        rel = path.relative_to(root)
        fails = check_class(root, path, cls, off)
        status = "OK  " if not fails else "FAIL"
        print(f"serializer-lint: {status} {cls or '?'} ({rel})")
        all_fails.extend(fails)
    if all_fails:
        print("\nserializer-lint: FAILURES:", file=sys.stderr)
        for f in all_fails:
            print(f"  - {f}", file=sys.stderr)
        print(
            "A subscribing service without the sanctioned serializer "
            "pair makes the game fail to save (suite 0.5.0). See "
            "QUALITY-AUDIT.md item 1 / bridge-dev skill.",
            file=sys.stderr,
        )
        return 1
    print(
        f"serializer-lint: PASS — {len(subs)} subscribing service(s), "
        "all carry [ManuallyWrittenSerialization] + the Serialize/"
        "Deserialize pair + manifest can_remove_from_saved_game:false"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
