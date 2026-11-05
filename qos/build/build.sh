#!/usr/bin/env bash
# QoS suite VM build: compiles QoSCore + QoSChickens (net48) against
# the game assemblies in ../../refs and packages dist zips:
#   dist/QoSCore-mod-<v>.zip      (QoSCore/ at root)
#   dist/QoSChickens-mod-<v>.zip  (QoSChickens/ at root)
#   dist/QoS-suite-<v>.zip        (both folders + suite readme)
# Mirrors bridge/vm-build/build.sh (manifest version is the truth
# and is stamped into the DLLs). Usage: ./build.sh
set -euo pipefail
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$HOME/.dotnet:$PATH"

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

ver() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "$1"; }
CORE_VER="$(ver "$HERE/../QoSCore/manifest.json")"
CHICK_VER="$(ver "$HERE/../QoSChickens/manifest.json")"
echo "QoSCore $CORE_VER / QoSChickens $CHICK_VER"

# One project per subfolder: two csprojs in one folder share a
# single obj/project.assets.json and stomp each other's restore.
dotnet build "$HERE/core/QoSCore.csproj" -c Release -p:Version="$CORE_VER" -p:AssemblyVersion="$CORE_VER" -p:FileVersion="$CORE_VER"
dotnet build "$HERE/chickens/QoSChickens.csproj" -c Release -p:Version="$CHICK_VER" -p:AssemblyVersion="$CHICK_VER" -p:FileVersion="$CHICK_VER"

STAGE="$HERE/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/QoSCore" "$STAGE/QoSChickens"
cp "$HERE/core/bin/QoSCore.dll" "$STAGE/QoSCore/"
cp "$HERE/../QoSCore/manifest.json" "$HERE/../QoSCore/config.json" "$HERE/../QoSCore/readme.txt" "$STAGE/QoSCore/"
cp "$HERE/chickens/bin/QoSChickens.dll" "$STAGE/QoSChickens/"
cp "$HERE/../QoSChickens/manifest.json" "$HERE/../QoSChickens/config.json" "$HERE/../QoSChickens/readme.txt" "$STAGE/QoSChickens/"

python3 - "$STAGE" "$DIST" "$CORE_VER" "$CHICK_VER" <<'PYEOF'
import os, sys, zipfile
stage, dist, core_ver, chick_ver = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]

def add(z, arc, path):
    info = zipfile.ZipInfo(arc, date_time=(2026, 1, 1, 0, 0, 0))
    info.external_attr = 0o644 << 16
    with open(path, "rb") as f:
        z.writestr(info, f.read())

def folder(z, name):
    base = os.path.join(stage, name)
    for fn in sorted(os.listdir(base)):
        add(z, f"{name}/{fn}", os.path.join(base, fn))

with zipfile.ZipFile(os.path.join(dist, f"QoSCore-mod-{core_ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
    folder(z, "QoSCore")
with zipfile.ZipFile(os.path.join(dist, f"QoSChickens-mod-{chick_ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
    folder(z, "QoSChickens")
suite_ver = core_ver  # suite pack version tracks the core version
with zipfile.ZipFile(os.path.join(dist, f"QoS-suite-{suite_ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
    folder(z, "QoSCore")
    folder(z, "QoSChickens")
    add(z, "QoS-SUITE-README.txt", os.path.join(stage, "..", "..", "SUITE-README.txt"))
print("zips written")
PYEOF

ls -la "$DIST"/QoS*.zip
sha256sum "$DIST"/QoSCore-mod-"$CORE_VER".zip "$DIST"/QoSChickens-mod-"$CHICK_VER".zip "$DIST"/QoS-suite-"$CORE_VER".zip
