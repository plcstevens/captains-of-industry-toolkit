#!/usr/bin/env bash
# Scratch check for the unified Log page (SPEC Addendum XVI).
# Builds a throwaway DB holding one legacy (pre-v3) audit row, a
# throwaway QoS feed with two events, serves on a scratch port and
# asserts: migration backfills the legacy row onto /log, the feed
# imports on page load, re-import is idempotent, and the source
# filter separates companion rows from QoS rows. No game needed —
# the Log page never fetches live state.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
COI="${1:-$HERE/../coi/target/debug/superintendent}"
PORT="${PORT:-8486}"
WORK="$(mktemp -d)"
trap 'kill ${SRV:-} 2>/dev/null || true; rm -rf "$WORK"' EXIT

# 1. Legacy audit row in an otherwise empty DB file.
python3 - "$WORK/coi.db" <<'PY'
import sqlite3, sys
conn = sqlite3.connect(sys.argv[1])
conn.execute("CREATE TABLE audit (id INTEGER PRIMARY KEY, ts_unix INTEGER NOT NULL, action TEXT NOT NULL, detail TEXT)")
conn.execute("INSERT INTO audit VALUES (1, 1762000000, 'ToggleEnabledCmd', 'Legacy detail: paused the starter steel line')")
conn.commit()
PY

# 2. QoS feed: one command event, one navigation event.
cat > "$WORK/events.jsonl" <<'JSONL'
{"schema":1,"eventId":"11111111-2222-3333-4444-555555555555","recordedAtUtc":"2026-11-05T13:00:00.0000000Z","source":"qos","modId":"QoSChickens","modVersion":"0.1.1","kind":"command","action":"MoveAnimalsCmd","summary":"equalise: moved +12 into farm #101","data":{"farmId":101,"amount":12,"species":"Chicken","gameDate":"388-2-27"}}
{"schema":1,"eventId":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","recordedAtUtc":"2026-11-05T13:05:00.0000000Z","source":"qos","modId":"QoSWorldMap","modVersion":"0.1.1","kind":"navigation","action":"NavigateToMine","summary":"Navigated to Iron mine","data":{"mine":"Iron mine","product":"Product_IronOre"}}
JSONL

# 3. Serve (no login — the site is loopback-only, DESIGN V2).
COI_QOS_EVENTS_PATH="$WORK/events.jsonl" "$COI" --db "$WORK/coi.db" serve --port "$PORT" >"$WORK/serve.log" 2>&1 &
SRV=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$PORT/"; then break; fi; sleep 0.2
done

get() { curl -s "http://127.0.0.1:$PORT$1"; }
fail() { echo "FAIL: $1"; exit 1; }

ALL="$(get '/log')"
grep -q 'ToggleEnabledCmd' <<<"$ALL" || fail 'legacy audit row missing from /log'
grep -q 'Legacy detail: paused the starter steel line' <<<"$ALL" || fail 'legacy detail text missing'
grep -q 'MoveAnimalsCmd' <<<"$ALL" || fail 'QoS chickens event missing'
grep -q 'NavigateToMine' <<<"$ALL" || fail 'QoS navigation event missing'
grep -q '>New</div><div class="figv">2<' <<<"$ALL" || fail 'first import: New fig wrong'
grep -q '>Skipped</div><div class="figv">0<' <<<"$ALL" || fail 'first import: Skipped fig wrong'
grep -q '>Malformed</div><div class="figv">0<' <<<"$ALL" || fail 'first import: Malformed fig wrong'

QOS="$(get '/log?source=qos')"
grep -q 'MoveAnimalsCmd' <<<"$QOS" || fail 'source=qos lost the chickens event'
grep -q 'ToggleEnabledCmd' <<<"$QOS" && fail 'source=qos leaked the companion row'

COMP="$(get '/log?source=companion')"
grep -q 'ToggleEnabledCmd' <<<"$COMP" || fail 'source=companion lost the legacy row'
grep -q 'MoveAnimalsCmd' <<<"$COMP" && fail 'source=companion leaked a QoS row'

SEARCH="$(get '/log?q=Navigated')"
grep -q 'NavigateToMine' <<<"$SEARCH" || fail 'search lost the navigation event'
grep -q 'MoveAnimalsCmd' <<<"$SEARCH" && fail 'search leaked a non-matching row'

AGAIN="$(get '/log')"
grep -q '>New</div><div class="figv">0<' <<<"$AGAIN" || fail 're-import: New fig wrong'
grep -q '>Skipped</div><div class="figv">2<' <<<"$AGAIN" || fail 're-import not idempotent (Skipped fig)'

echo "LOG PAGE CHECK PASS"
