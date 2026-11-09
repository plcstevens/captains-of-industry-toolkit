#!/usr/bin/env bash
# Scratch check for the top-bar must-fix badge + its polling
# endpoint (DESIGN V2 addition, 2026-11-06). Serves the companion
# against the stub game (v065 synthetic capture), then:
#   1. before any page load, the endpoint answers count:null —
#      it never fabricates a number and never fetches the game;
#   2. loading /issues feeds the cache; the endpoint's count then
#      equals the number of issue cards on /issues?only=fix-now
#      (the same compute_issues data, rendered two ways);
#   3. the badge markup (id=mustfix, words "must fix", link to
#      the filtered board) is present on two different pages;
#   4. with the stub down, a page load flips the endpoint to
#      reachable:false and the badge renders the count as
#      "last known" — never as live.
# No real game: everything lives in a mktemp dir.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
COI="${1:-$HOME/workspace/coi-companion/coi/target/debug/superintendent}"
PORT="${PORT:-8488}"
STUB_PORT="${STUB_PORT:-3998}"
WORK="$(mktemp -d)"
trap 'kill ${SRV:-} ${STUB:-} 2>/dev/null || true; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/appdata"

python3 "$HERE/stub_game_server.py" --port "$STUB_PORT" \
  --capture "$HERE/../${CAPTURE:-dev/captures/live-synthetic-v065}" >"$WORK/stub.log" 2>&1 &
STUB=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$STUB_PORT/api/v1/game-info"; then break; fi; sleep 0.2
done

APPDATA="$WORK/appdata" "$COI" --db "$WORK/coi.db" \
  --game-url "http://127.0.0.1:$STUB_PORT" serve --port "$PORT" \
  >"$WORK/serve.log" 2>&1 &
SRV=$!
# Wait on the endpoint itself: polling a page would feed the
# badge cache and spoil the cold-cache assertion below.
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$PORT/api/v1/must-fix-count"; then break; fi; sleep 0.2
done

fail() { echo "FAIL: $1"; exit 1; }
get() { curl -s "http://127.0.0.1:$PORT$1"; }

# 1. Cold cache: null count, not reachable, no fabrication.
COLD="$(get '/api/v1/must-fix-count')"
grep -q '"count":null' <<<"$COLD" || fail "cold endpoint did not answer count:null: $COLD"

# 2. A page load feeds the cache; endpoint == filtered board.
get '/issues' >/dev/null
JSON="$(get '/api/v1/must-fix-count')"
COUNT="$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); assert d["reachable"] is True, d; print(d["count"])' "$JSON")"
[ -n "$COUNT" ] || fail "endpoint count missing after /issues load: $JSON"
FILTERED="$(get '/issues?only=fix-now')"
CARDS="$(grep -o 'class="issue2 ' <<<"$FILTERED" | wc -l || true)"
[ "$COUNT" = "$CARDS" ] || fail "endpoint count $COUNT != fix-now cards on filtered board $CARDS"
echo "must-fix count verified against the board: $COUNT"

# 3. Badge markup on two different pages.
for page in '/' '/products'; do
  HTML="$(get "$page")"
  grep -q 'id="mustfix"' <<<"$HTML" || fail "badge missing on $page"
  grep -q 'must fix' <<<"$HTML" || fail "badge words missing on $page"
  grep -q 'href="/issues?only=fix-now"' <<<"$HTML" || fail "badge link missing on $page"
done

# 4. Game down: last-known honesty.
kill $STUB 2>/dev/null || true
sleep 0.3
get '/' >/dev/null
DOWN="$(get '/api/v1/must-fix-count')"
python3 -c 'import json,sys; d=json.loads(sys.argv[1]); assert d["reachable"] is False, d; assert d["count"] is not None, d' "$DOWN" \
  || fail "endpoint did not flip to reachable:false with last count kept: $DOWN"
HTML="$(get '/')"
grep -q 'must fix · last known' <<<"$HTML" || fail 'badge did not render the last-known state'

# 5. Host check: a foreign Host is refused outright.
CODE="$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: evil.example:8488' "http://127.0.0.1:$PORT/")"
[ "$CODE" = "403" ] || fail "foreign Host returned $CODE, expected 403"

echo "BADGE CHECK PASS"
