#!/usr/bin/env bash
# QoS guard-arithmetic tests (QUALITY-AUDIT item 5 / finding T2).
#
# Builds and runs qos/build/guardtests — a net8.0 harness that
# compiles the EXACT shipped sources of the three extracted pure
# guard units (QoSChickens/ChickenGuardMath.cs,
# QoSVehicleRecovery/RecoveryGuardMath.cs,
# QoSPriorityActuator/ActuatorGuardMath.cs) and executes them
# against named cases characterised from the pre-extraction code:
# boundaries on both sides of every threshold, check-order pins,
# zero/empty extremes, and fail-closed directions. The net48 mod
# DLLs cannot execute on Linux; the pure units can, and they are
# the same source the mods compile.
#
# Wired into qos/build/build.sh (the suite build refuses to
# package on failure) and part of the standard check sequence
# (tools/README.md).
set -euo pipefail
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$HOME/.dotnet:$PATH"

HERE="$(cd "$(dirname "$0")" && pwd)"
PROJ="$HERE/../qos/build/guardtests/guardtests.csproj"

dotnet run --project "$PROJ" -c Release
