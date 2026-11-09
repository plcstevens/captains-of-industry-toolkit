#!/usr/bin/env bash
# QoSBuilder STANDALONE build: compiles QoSBuilder (+ QoSCore via
# the project reference) against the game assemblies in ../../refs
# and packages dist/QoSBuilder-mod-<v>.zip (QoSBuilder/ at root).
#
# This is deliberately NOT part of build.sh yet: suite-pack
# integration (SUITE_VER, SUITE-README membership) happens in a
# later round, once QoSBuilder has been live-verified. Mirrors the
# suite scaffold conventions (one csproj per folder — the shared
# project.assets.json gotcha — and manifest version is the truth,
# stamped into the DLL). Usage: ./build-builder.sh
set -euo pipefail
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$HOME/.dotnet:$PATH"

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

REFS_DIR="${COI_REFS_DIR:-$ROOT/refs}"
for dll in Mafi.dll Mafi.Core.dll Mafi.Unity.dll; do
  [ -f "$REFS_DIR/$dll" ] || { echo "missing reference assembly: $REFS_DIR/$dll (set COI_REFS_DIR to the folder with the game's Mafi*.dll)"; exit 1; }
done

# Serializer-pair lint (QUALITY-AUDIT item 1): QoSBuilder's
# BuilderService subscribes to the calendar's saveable NewDayEnd
# event — fail the build if it (or any discovered subscriber) lacks
# [ManuallyWrittenSerialization] + the pair + manifest
# can_remove_from_saved_game:false. That omission broke ALL saving
# in suite 0.5.0.
python3 "$ROOT/tools/check_serializer_pairs.py" || { echo "!! serializer-pair lint FAILED — refusing to build (see above)"; exit 1; }

VER="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "$HERE/../QoSBuilder/manifest.json")"
echo "QoSBuilder $VER (standalone; suite pack untouched)"

dotnet build "$HERE/builder/QoSBuilder.csproj" -c Release -p:CoiRefsDir="$REFS_DIR/" -p:ContinuousIntegrationBuild=true -p:Version="$VER" -p:AssemblyVersion="$VER" -p:FileVersion="$VER"

STAGE="$HERE/stage-builder"
rm -rf "$STAGE"
mkdir -p "$STAGE/QoSBuilder"
cp "$HERE/builder/bin/QoSBuilder.dll" "$STAGE/QoSBuilder/"
cp "$HERE/../QoSBuilder/manifest.json" "$HERE/../QoSBuilder/config.json" "$HERE/../QoSBuilder/readme.txt" "$STAGE/QoSBuilder/"
# Guard: the QoSCore DLL must never ride inside a feature folder.
if [ -f "$STAGE/QoSBuilder/QoSCore.dll" ]; then
  echo "!! QoSCore.dll leaked into the QoSBuilder folder — aborting"
  exit 1
fi

python3 - "$STAGE" "$DIST" "$VER" <<'PYEOF'
import os, sys, zipfile
stage, dist, ver = sys.argv[1], sys.argv[2], sys.argv[3]

def add(z, arc, path):
    info = zipfile.ZipInfo(arc, date_time=(2026, 1, 1, 0, 0, 0))
    info.external_attr = 0o644 << 16
    with open(path, "rb") as f:
        z.writestr(info, f.read())

with zipfile.ZipFile(os.path.join(dist, f"QoSBuilder-mod-{ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
    base = os.path.join(stage, "QoSBuilder")
    for fn in sorted(os.listdir(base)):
        add(z, f"QoSBuilder/{fn}", os.path.join(base, fn))
print("zip written")
PYEOF

ls -la "$DIST/QoSBuilder-mod-$VER.zip"
sha256sum "$DIST/QoSBuilder-mod-$VER.zip"
