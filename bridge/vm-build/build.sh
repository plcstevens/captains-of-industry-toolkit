#!/usr/bin/env bash
# Build CoiBridge.dll on this VM and package the finished, drop-in mod.
#   1. Put Philip's game assemblies in ../../refs/ (Mafi*.dll, Assembly-CSharp.dll)
#   2. Run this script.
# Output: ../../dist/CoiBridge-mod-<version>.zip containing
#   CoiBridge/manifest.json, CoiBridge/CoiBridge.dll, CoiBridge/readme.txt
set -euo pipefail
cd "$(dirname "$0")"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
DOTNET="$HOME/.dotnet/dotnet"
[ -x "$DOTNET" ] || { echo "dotnet SDK missing at $DOTNET"; exit 1; }
# Reference assemblies: COI_REFS_DIR points at the folder holding the
# game's Mafi*.dll (e.g. your game install's Managed folder). Defaults
# to the repo-local refs/ folder when unset.
REFS_DIR="${COI_REFS_DIR:-$PWD/../../refs}"
for dll in Mafi.dll Mafi.Core.dll Mafi.Base.dll Mafi.Unity.dll; do
  [ -f "$REFS_DIR/$dll" ] || { echo "missing reference assembly: $REFS_DIR/$dll (set COI_REFS_DIR to the folder with the game's Mafi*.dll)"; exit 1; }
done
# Serializer-pair lint (QUALITY-AUDIT item 1): the bridge itself
# subscribes to nothing, but the check scans qos/ + bridge/ and a
# future bridge subscription without the sanctioned serializer pair
# would break saving exactly as suite 0.5.0 did — fail here too.
python3 "$PWD/../../tools/check_serializer_pairs.py" || { echo "!! serializer-pair lint FAILED — refusing to build (see above)"; exit 1; }
VERSION=$(python3 -c "import json;print(json.load(open('../CoiBridge/manifest.json'))['version'])")
"$DOTNET" build -c Release -p:CoiRefsDir="$REFS_DIR/" -p:Version="$VERSION" -p:FileVersion="$VERSION" -p:AssemblyVersion="$VERSION"
DLL="bin/Release/net48/CoiBridge.dll"
[ -f "$DLL" ] || { echo "build produced no $DLL"; exit 1; }
STAGE=$(mktemp -d)
mkdir -p "$STAGE/CoiBridge"
cp ../CoiBridge/manifest.json ../CoiBridge/readme.txt "$STAGE/CoiBridge/"
cp "$DLL" "$STAGE/CoiBridge/"
OUT="../../dist/CoiBridge-mod-$VERSION.zip"
rm -f "$OUT"
(cd "$STAGE" && zip -q -r - . ) > /dev/null 2>&1 || (cd "$STAGE" && python3 -c "
import zipfile, pathlib
with zipfile.ZipFile('$OUT', 'w', zipfile.ZIP_DEFLATED) as z:
    for f in sorted(pathlib.Path('.').rglob('*')):
        if f.is_file(): z.write(f, f.as_posix())
")
# The zip must have CoiBridge/ at its root; rebuild deterministically with python.
python3 - "$STAGE" "$OUT" <<'PYEOF'
import sys, zipfile, pathlib
stage, out = pathlib.Path(sys.argv[1]), sys.argv[2]
with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED) as z:
    for f in sorted(stage.rglob('*')):
        if f.is_file(): z.write(f, f.relative_to(stage).as_posix())
print("packaged:", out)
with zipfile.ZipFile(out) as z:
    for n in z.namelist(): print(" ", n)
PYEOF
rm -rf "$STAGE"
