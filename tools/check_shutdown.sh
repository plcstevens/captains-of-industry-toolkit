#!/usr/bin/env bash
# Scratch check for the in-UI Shutdown (daemon round, SPEC
# Addendum XXIV). Serves the companion against the stub game
# (v065 synthetic capture) in a mktemp dir, then:
#   1. the top bar carries the Shutdown control on a normal page;
#   2. GET /shutdown is the confirm step — it names what will
#      happen and carries the CSRF field;
#   3. a POST without the token is refused (403) and the server
#      stays up;
#   4. a snapshot taken before shutdown is a real write in the db;
#   5. the confirmed POST answers with the landing page ("you can
#      close this tab"), the process exits 0 on its own, and the
#      port is released;
#   6. relaunching on the same database passes PRAGMA
#      integrity_check and still serves the snapshot taken in
#      step 4 (clean close, nothing lost).
# No real game: everything lives in a mktemp dir.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
COI="${1:-$HOME/workspace/coi-companion/coi/target/debug/superintendent}"
PORT="${PORT:-8491}"
STUB_PORT="${STUB_PORT:-3997}"
WORK="$(mktemp -d)"
trap 'kill ${SRV:-} ${STUB:-} ${SRV2:-} 2>/dev/null || true; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/appdata"

python3 "$HERE/stub_game_server.py" --port "$STUB_PORT" \
  --capture "$HERE/../dev/captures/live-synthetic-v065" >"$WORK/stub.log" 2>&1 &
STUB=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$STUB_PORT/api/v1/game-info"; then break; fi
  sleep 0.2
done

APPDATA="$WORK/appdata" "$COI" --db "$WORK/coi.db" \
  --game-url "http://127.0.0.1:$STUB_PORT" serve --port "$PORT" \
  >"$WORK/serve.log" 2>&1 &
SRV=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$PORT/"; then break; fi
  sleep 0.2
done

fail() { echo "FAIL: $1"; exit 1; }
get() { curl -s "http://127.0.0.1:$PORT$1"; }
csrf() { get "$1" | grep -o 'name="csrf" value="[^"]*"' | awk 'NR==1' | sed 's/.*value="//; s/"//'; }

# 1. Top-bar control on a normal page.
HOME_HTML="$(get '/')"
grep -q 'href="/shutdown"' <<<"$HOME_HTML" || fail 'top bar carries no Shutdown control'
grep -q 'Shutdown' <<<"$HOME_HTML" || fail 'Shutdown label missing from top bar'

# 2. The confirm step names the consequences and carries the token.
CONFIRM="$(get '/shutdown')"
grep -q 'What happens when you shut down' <<<"$CONFIRM" || fail 'confirm page missing its explanation'
grep -q 'Your game is not touched' <<<"$CONFIRM" || fail 'confirm page does not state the game is untouched'
CSRF="$(csrf '/shutdown')"
[ -n "$CSRF" ] || fail 'no csrf on /shutdown'

# 3. Tokenless POST: refused, server still up.
CODE="$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT/shutdown" --data 'csrf=wrong')"
[ "$CODE" = "403" ] || fail "tokenless shutdown POST returned $CODE, expected 403"
get '/' >/dev/null || fail 'server went down after a refused shutdown POST'

# 4. A real write lands in the database before shutdown.
SNAP_CSRF="$(csrf '/snapshots')"
curl -s -X POST "http://127.0.0.1:$PORT/snapshots/take" --data "csrf=$SNAP_CSRF" >/dev/null
SNAPSHOTS="$(get '/snapshots')"
grep -qi 'snapshot' <<<"$SNAPSHOTS" || fail 'snapshots page unreadable before shutdown'

# 5. Confirmed POST: landing page, exit 0, port released.
LANDING="$(curl -s -X POST "http://127.0.0.1:$PORT/shutdown" --data "csrf=$CSRF")"
grep -q 'Superintendent has stopped' <<<"$LANDING" || fail 'shutdown POST did not answer with the landing page'
grep -q 'close this tab' <<<"$LANDING" || fail 'landing page does not tell the user to close the tab'
set +e
wait "$SRV"
EXIT_CODE=$?
set -e
[ "$EXIT_CODE" = "0" ] || fail "server exited $EXIT_CODE, expected 0"
grep -q 'stopped cleanly' "$WORK/serve.log" || fail 'serve log lacks the clean-stop line'
if curl -s -o /dev/null "http://127.0.0.1:$PORT/"; then
  fail 'port still answers after shutdown'
fi

# 6. Relaunch on the same db: integrity ok, snapshot still there.
python3 - "$WORK/coi.db" <<'PYEOF' || fail 'PRAGMA integrity_check failed on the closed database'
import sqlite3, sys
conn = sqlite3.connect(sys.argv[1])
row = conn.execute("PRAGMA integrity_check").fetchone()
assert row and row[0] == "ok", row
n = conn.execute("SELECT COUNT(*) FROM snapshots").fetchone()[0]
assert n >= 1, f"expected the pre-shutdown snapshot to survive, found {n}"
print(f"integrity ok; {n} snapshot(s) survived the shutdown")
PYEOF

APPDATA="$WORK/appdata" "$COI" --db "$WORK/coi.db" \
  --game-url "http://127.0.0.1:$STUB_PORT" serve --port "$PORT" \
  >"$WORK/serve2.log" 2>&1 &
SRV2=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$PORT/"; then break; fi
  sleep 0.2
done
RELAUNCHED="$(get '/log')"
grep -q 'shutdown' <<<"$RELAUNCHED" || fail 'shutdown activity event not visible in the Log after relaunch'

echo "SHUTDOWN CHECK PASS"
