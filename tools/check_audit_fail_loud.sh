#!/usr/bin/env bash
# Scratch check for the fail-loud §12 audit writes (QUALITY-AUDIT item 2).
# Serve against the stub game with a scratch DB, break the DB
# (swapped for a directory) after login+prepare, then confirm a placement and apply
# a command: both result pages must carry the AUDIT WRITE FAILED
# error instead of silent success. Healthy path already covered by
# tools/check_place_page.sh + check_log_page.sh.
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
COI="${1:-$HERE/coi/target/debug/coi}"
PORT="${PORT:-8490}"; STUB_PORT="${STUB_PORT:-3997}"
WORK="$(mktemp -d)"
trap 'kill ${SRV:-} ${STUB:-} 2>/dev/null || true; chmod -R u+w "$WORK" 2>/dev/null; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/appdata"

python3 "$HERE/tools/stub_game_server.py" --port "$STUB_PORT" \
  --capture "$HERE/live-synthetic-v064" >"$WORK/stub.log" 2>&1 &
STUB=$!
for i in $(seq 1 50); do curl -s -o /dev/null "http://127.0.0.1:$STUB_PORT/api/v1/game-info" && break || sleep 0.2; done

APPDATA="$WORK/appdata" "$COI" --db "$WORK/coi.db" \
  --game-url "http://127.0.0.1:$STUB_PORT" serve --port "$PORT" >"$WORK/serve.log" 2>&1 &
SRV=$!
for i in $(seq 1 50); do curl -s -o /dev/null "http://127.0.0.1:$PORT/login" && break || sleep 0.2; done
curl -s -c "$WORK/cookies" -o /dev/null -X POST "http://127.0.0.1:$PORT/login" \
  --data 'mode=setup&username=tester&password=testpass123&password2=testpass123'
get() { curl -s -b "$WORK/cookies" "http://127.0.0.1:$PORT$1"; }
csrf() { get "$1" | grep -o 'name="csrf" value="[^"]*"' | head -1 | sed 's/.*value="//; s/"//'; }
fail() { echo "DRILL FAIL: $1"; exit 1; }

CSRF="$(csrf '/place')"
CONFIRM="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/place/prepare" \
  --data "csrf=$CSRF&slug=concrete-v1&ax=3167&ay=762&az=4&rotation=90")"
PTOKEN="$(grep -o 'name="token" value="[^"]*"' <<<"$CONFIRM" | head -1 | sed 's/.*value="//; s/"//')"
[ -n "$PTOKEN" ] || fail 'place prepare produced no token'

# Prepare a §12 command too (generic form: speed change).
CSRF2="$(csrf '/actions')"
PREP="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/actions/prepare-generic" \
  --data "csrf=$CSRF2&type=GameSpeedChangeCmd&arg_newSpeedMultiplier=1")"
ATOKEN="$(grep -o 'name="token" value="[^"]*"' <<<"$PREP" | head -1 | sed 's/.*value="//; s/"//')"
[ -n "$ATOKEN" ] || fail "command prepare produced no token: $(head -c 300 <<<"$PREP")"

# Break the DB: swap the file for a directory (this VM runs as root,
# so chmod read-only would not bite). Every store open now fails.
mv "$WORK/coi.db" "$WORK/coi.db.bak"
rm -f "$WORK/coi.db-wal" "$WORK/coi.db-shm"
mkdir "$WORK/coi.db"

DONE="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/place/confirm" \
  --data "csrf=$CSRF&token=$PTOKEN")"
grep -q 'Job written' <<<"$DONE" || fail 'place confirm did not write the job'
grep -q 'AUDIT WRITE FAILED' <<<"$DONE" || fail 'place confirm hid the audit failure'
grep -q 'missing from the audit trail' <<<"$DONE" || fail 'place banner wording missing'
echo "place-confirm: job written + AUDIT WRITE FAILED surfaced"

APPLIED="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/actions/apply" \
  --data "csrf=$CSRF2&token=$ATOKEN")"
grep -q 'AUDIT WRITE FAILED' <<<"$APPLIED" || fail 'apply hid the audit failure'
grep -q 'missing from the audit trail' <<<"$APPLIED" || fail 'apply banner wording missing'
echo "do_apply: AUDIT WRITE FAILED surfaced (command itself 405s at the stub, as designed)"

grep -q 'left NO audit record' "$WORK/serve.log" || fail 'server log line missing'
echo "server log carries the no-audit-record line"
echo "AUDIT FAIL-LOUD DRILL PASS"
