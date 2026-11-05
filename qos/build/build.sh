#!/usr/bin/env bash
# QoS suite VM build: compiles QoSCore + QoSChickens +
# QoSVehicleRecovery + QoSWorldMap (net48) against the game
# assemblies in ../../refs and packages dist zips:
#   dist/QoSCore-mod-<v>.zip            (QoSCore/ at root)
#   dist/QoSChickens-mod-<v>.zip        (QoSChickens/ at root)
#   dist/QoSVehicleRecovery-mod-<v>.zip (QoSVehicleRecovery/ at root)
#   dist/QoSWorldMap-mod-<v>.zip        (QoSWorldMap/ at root)
#   dist/QoS-suite-<suite v>.zip        (all folders + suite readme)
# The suite pack has its OWN version (SUITE_VER below): adding a
# feature bumps the pack, not QoSCore. Older packs stay in dist/
# untouched — this script only ever writes the current SUITE_VER.
# Mirrors bridge/vm-build/build.sh (manifest version is the truth
# and is stamped into the DLLs). Usage: ./build.sh
#
# QoSWorldMap is a UiToolkit window mod: compiling any Window
# subclass needs the game's UnityEngine module assemblies
# (UnityEngine.dll, UnityEngine.CoreModule.dll,
# UnityEngine.UIElementsModule.dll) alongside the Mafi set in the
# refs dir. If they are absent, the worldmap build AND the suite
# pack are SKIPPED with a loud notice (a suite pack that silently
# lacked a member would be a lie); the other three mods still
# build and pack as before.
set -euo pipefail
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$HOME/.dotnet:$PATH"

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

# Reference assemblies: COI_REFS_DIR points at the folder holding the
# game's Mafi*.dll (e.g. your game install's Managed folder). Defaults
# to the repo-local refs/ folder when unset.
REFS_DIR="${COI_REFS_DIR:-$ROOT/refs}"
for dll in Mafi.dll Mafi.Core.dll Mafi.Unity.dll; do
  [ -f "$REFS_DIR/$dll" ] || { echo "missing reference assembly: $REFS_DIR/$dll (set COI_REFS_DIR to the folder with the game's Mafi*.dll)"; exit 1; }
done

WORLDMAP_OK=1
for dll in UnityEngine.dll UnityEngine.CoreModule.dll UnityEngine.UIElementsModule.dll; do
  if [ ! -f "$REFS_DIR/$dll" ]; then
    WORLDMAP_OK=0
    echo "!! QoSWorldMap SKIPPED: missing $REFS_DIR/$dll"
  fi
done
if [ "$WORLDMAP_OK" = "0" ]; then
  echo "!! QoSWorldMap needs the game's UnityEngine module assemblies"
  echo "!! (UnityEngine.dll, UnityEngine.CoreModule.dll,"
  echo "!! UnityEngine.UIElementsModule.dll) in the refs dir — copy them"
  echo "!! from <game install>/Captain of Industry_Data/Managed/ and re-run."
  echo "!! No QoSWorldMap zip and no suite pack will be written this run."
fi

ver() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "$1"; }
CORE_VER="$(ver "$HERE/../QoSCore/manifest.json")"
CHICK_VER="$(ver "$HERE/../QoSChickens/manifest.json")"
REC_VER="$(ver "$HERE/../QoSVehicleRecovery/manifest.json")"
WM_VER="$(ver "$HERE/../QoSWorldMap/manifest.json")"
SUITE_VER="0.3.0"
echo "QoSCore $CORE_VER / QoSChickens $CHICK_VER / QoSVehicleRecovery $REC_VER / QoSWorldMap $WM_VER / suite $SUITE_VER"

# One project per subfolder: two csprojs in one folder share a
# single obj/project.assets.json and stomp each other's restore.
dotnet build "$HERE/core/QoSCore.csproj" -c Release -p:CoiRefsDir="$REFS_DIR/" -p:Version="$CORE_VER" -p:AssemblyVersion="$CORE_VER" -p:FileVersion="$CORE_VER"
dotnet build "$HERE/chickens/QoSChickens.csproj" -c Release -p:CoiRefsDir="$REFS_DIR/" -p:Version="$CHICK_VER" -p:AssemblyVersion="$CHICK_VER" -p:FileVersion="$CHICK_VER"
dotnet build "$HERE/recovery/QoSVehicleRecovery.csproj" -c Release -p:CoiRefsDir="$REFS_DIR/" -p:Version="$REC_VER" -p:AssemblyVersion="$REC_VER" -p:FileVersion="$REC_VER"
if [ "$WORLDMAP_OK" = "1" ]; then
  dotnet build "$HERE/worldmap/QoSWorldMap.csproj" -c Release -p:CoiRefsDir="$REFS_DIR/" -p:Version="$WM_VER" -p:AssemblyVersion="$WM_VER" -p:FileVersion="$WM_VER"
fi

STAGE="$HERE/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/QoSCore" "$STAGE/QoSChickens" "$STAGE/QoSVehicleRecovery"
cp "$HERE/core/bin/QoSCore.dll" "$STAGE/QoSCore/"
cp "$HERE/../QoSCore/manifest.json" "$HERE/../QoSCore/config.json" "$HERE/../QoSCore/readme.txt" "$STAGE/QoSCore/"
cp "$HERE/chickens/bin/QoSChickens.dll" "$STAGE/QoSChickens/"
cp "$HERE/../QoSChickens/manifest.json" "$HERE/../QoSChickens/config.json" "$HERE/../QoSChickens/readme.txt" "$STAGE/QoSChickens/"
cp "$HERE/recovery/bin/QoSVehicleRecovery.dll" "$STAGE/QoSVehicleRecovery/"
cp "$HERE/../QoSVehicleRecovery/manifest.json" "$HERE/../QoSVehicleRecovery/config.json" "$HERE/../QoSVehicleRecovery/readme.txt" "$STAGE/QoSVehicleRecovery/"
if [ "$WORLDMAP_OK" = "1" ]; then
  mkdir -p "$STAGE/QoSWorldMap"
  cp "$HERE/worldmap/bin/QoSWorldMap.dll" "$STAGE/QoSWorldMap/"
  cp "$HERE/../QoSWorldMap/manifest.json" "$HERE/../QoSWorldMap/config.json" "$HERE/../QoSWorldMap/readme.txt" "$STAGE/QoSWorldMap/"
fi

python3 - "$STAGE" "$DIST" "$CORE_VER" "$CHICK_VER" "$REC_VER" "$WM_VER" "$SUITE_VER" "$WORLDMAP_OK" <<'PYEOF'
import os, sys, zipfile
stage, dist, core_ver, chick_ver, rec_ver, wm_ver, suite_ver, wm_ok = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6], sys.argv[7], sys.argv[8] == "1"

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
with zipfile.ZipFile(os.path.join(dist, f"QoSVehicleRecovery-mod-{rec_ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
    folder(z, "QoSVehicleRecovery")
if wm_ok:
    with zipfile.ZipFile(os.path.join(dist, f"QoSWorldMap-mod-{wm_ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
        folder(z, "QoSWorldMap")
    with zipfile.ZipFile(os.path.join(dist, f"QoS-suite-{suite_ver}.zip"), "w", zipfile.ZIP_DEFLATED) as z:
        folder(z, "QoSCore")
        folder(z, "QoSChickens")
        folder(z, "QoSVehicleRecovery")
        folder(z, "QoSWorldMap")
        add(z, "QoS-SUITE-README.txt", os.path.join(stage, "..", "..", "SUITE-README.txt"))
print("zips written")
PYEOF

ls -la "$DIST"/QoS*.zip
sha256sum "$DIST"/QoSCore-mod-"$CORE_VER".zip "$DIST"/QoSChickens-mod-"$CHICK_VER".zip "$DIST"/QoSVehicleRecovery-mod-"$REC_VER".zip
if [ "$WORLDMAP_OK" = "1" ]; then
  sha256sum "$DIST"/QoSWorldMap-mod-"$WM_VER".zip "$DIST"/QoS-suite-"$SUITE_VER".zip
fi
