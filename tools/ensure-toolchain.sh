#!/usr/bin/env bash
# ensure-toolchain.sh — make the CoI build toolchain VM-replacement-proof.
#
# The lesson (2026-11-02): VM replacements wipe /usr and /tmp but NOT ~.
# The apt-installed mingw linker vanished twice in one day, and the
# reinstall then queued for ~20 minutes behind the platform's own
# package-recovery job holding the dpkg lock. So: everything the
# builds need now lives under ~, and this script verifies it and
# repairs what it can from local caches, without apt or the network.
#
# Contract: prints TOOLCHAIN_OK and exits 0 when healthy. Prints
# REPAIRED lines when it fixed something. Prints MISSING/FAILED and
# exits 1 when it could not fix the problem.
set -u
TC=/home/hatch/.rustup/toolchains/stable-x86_64-unknown-linux-gnu
GCC=/home/hatch/opt/mingw-w64/usr/bin/x86_64-w64-mingw32-gcc
fail=0

# 1. Rust toolchain + windows-gnu std (lives in ~/.rustup: persists,
#    but verify — if the toolchain itself is gone, that needs a
#    network reinstall, which is a report, not a silent repair).
if [ ! -x "$TC/bin/cargo" ]; then
  echo "MISSING: rust toolchain at $TC"
  fail=1
elif [ ! -d "$TC/lib/rustlib/x86_64-pc-windows-gnu" ]; then
  echo "MISSING: x86_64-pc-windows-gnu std in rustlib"
  fail=1
fi

# 2. Home-prefix mingw: functional test (compile+link a PE binary).
printf 'int main(void){return 0;}\n' > /tmp/.tc.c
if ! "$GCC" -o /tmp/.tc.exe /tmp/.tc.c 2>/dev/null; then
  echo "REPAIRING: mingw prefix from ~/opt/deb-cache"
  mkdir -p /home/hatch/opt/mingw-w64
  for d in /home/hatch/opt/deb-cache/*.deb; do dpkg -x "$d" /home/hatch/opt/mingw-w64; done
  ln -sf x86_64-w64-mingw32-gcc-posix /home/hatch/opt/mingw-w64/usr/bin/x86_64-w64-mingw32-gcc
  ln -sf x86_64-w64-mingw32-g++-posix /home/hatch/opt/mingw-w64/usr/bin/x86_64-w64-mingw32-g++ 2>/dev/null
  if "$GCC" -o /tmp/.tc.exe /tmp/.tc.c 2>/dev/null; then
    echo "REPAIRED: mingw prefix"
  else
    echo "FAILED: mingw repair (debs in ~/opt/deb-cache, prefix ~/opt/mingw-w64)"
    fail=1
  fi
fi

# 3. .NET SDK for CoiBridge builds (installed under ~/.dotnet: persists).
if [ ! -x /home/hatch/.dotnet/dotnet ]; then
  echo "MISSING: dotnet SDK at ~/.dotnet (reinstall per AGENTS.md note)"
  fail=1
fi

rm -f /tmp/.tc.c /tmp/.tc.exe
if [ "$fail" = 0 ]; then
  echo "TOOLCHAIN_OK"
fi
exit $fail
