#!/usr/bin/env bash
# Scratch check for the Place flow (SPEC Addendum XVII, QoSBuilder).
# Serves the companion against the stub game (gameId
# 5583213724730880438) with APPDATA pointed at a scratch dir, then:
#   1. /place lists the library and the scratch jobs folder;
#   2. prepare renders the exact job pinned to the stub's gameId;
#   3. confirm writes ONE job file; an independent Python
#      re-implementation of the canonical-hash spec verifies the
#      file's sha256 (the same check QoSBuilder runs in-game);
#   4. the confirm is single-use (replay writes nothing);
#   5. with the stub down, prepare refuses (502) — no job without
#      a confirmed gameId.
# No real game, no real APPDATA: everything lives in a mktemp dir.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
COI="${1:-$HOME/workspace/coi-companion/coi/target/debug/coi}"
PORT="${PORT:-8487}"
STUB_PORT="${STUB_PORT:-3999}"
WORK="$(mktemp -d)"
trap 'kill ${SRV:-} ${STUB:-} 2>/dev/null || true; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/appdata"

python3 "$HERE/stub_game_server.py" --port "$STUB_PORT" \
  --capture "$HERE/../live-synthetic-v064" >"$WORK/stub.log" 2>&1 &
STUB=$!
for i in $(seq 1 50); do
  curl -s -o /dev/null "http://127.0.0.1:$STUB_PORT/api/v1/game-info" && break || sleep 0.2
done

APPDATA="$WORK/appdata" "$COI" --db "$WORK/coi.db" \
  --game-url "http://127.0.0.1:$STUB_PORT" serve --port "$PORT" \
  >"$WORK/serve.log" 2>&1 &
SRV=$!
for i in $(seq 1 50); do
  curl -s -o /dev/null "http://127.0.0.1:$PORT/login" && break || sleep 0.2
done
curl -s -c "$WORK/cookies" -o /dev/null -X POST "http://127.0.0.1:$PORT/login" \
  --data 'mode=setup&username=tester&password=testpass123&password2=testpass123'

get() { curl -s -b "$WORK/cookies" "http://127.0.0.1:$PORT$1"; }
fail() { echo "FAIL: $1"; exit 1; }
csrf() { get "$1" | grep -o 'name="csrf" value="[^"]*"' | head -1 | sed 's/.*value="//; s/"//'; }

PLACE="$(get '/place')"
grep -q 'Place a blueprint' <<<"$PLACE" || fail '/place heading missing'
grep -q 'Concrete V1' <<<"$PLACE" || fail 'library rows missing (concrete-v1)'
grep -q 'concrete-v1' <<<"$PLACE" || fail 'slug missing'
grep -q "$WORK/appdata/Captain of Industry/QoS/jobs" <<<"$PLACE" \
  || fail 'jobs folder shown is not the APPDATA default'
grep -q 'gameId 5583213724730880438' <<<"$PLACE" || fail 'stub gameId not shown'

CSRF="$(csrf '/place')"
[ -n "$CSRF" ] || fail 'no csrf on /place'

CONFIRM="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/place/prepare" \
  --data "csrf=$CSRF&slug=concrete-v1&ax=3167&ay=762&az=4&rotation=90")"
grep -q 'Confirm placement' <<<"$CONFIRM" || fail 'prepare did not render the confirm page'
grep -q '5583213724730880438' <<<"$CONFIRM" || fail 'confirm page not pinned to the stub gameId'
grep -q '&quot;rotationDeg&quot;: 90' <<<"$CONFIRM" || grep -q '"rotationDeg": 90' <<<"$CONFIRM" \
  || fail 'job JSON missing rotationDeg 90'
TOKEN="$(grep -o 'name="token" value="[^"]*"' <<<"$CONFIRM" | head -1 | sed 's/.*value="//; s/"//')"
SHA_SHOWN="$(grep -o 'sha256 is <code>[0-9a-f]*</code>' <<<"$CONFIRM" | sed 's/.*<code>//; s/<.*//')"
[ -n "$TOKEN" ] || fail 'no confirm token'
[ -n "$SHA_SHOWN" ] || fail 'no sha256 on the confirm page'

DONE="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/place/confirm" \
  --data "csrf=$CSRF&token=$TOKEN")"
grep -q 'Job written' <<<"$DONE" || fail 'confirm did not write the job'

JOBS_DIR="$WORK/appdata/Captain of Industry/QoS/jobs"
COUNT="$(ls "$JOBS_DIR"/*.json 2>/dev/null | wc -l)"
[ "$COUNT" = "1" ] || fail "expected exactly 1 job file, found $COUNT"
JOB_FILE="$(ls "$JOBS_DIR"/*.json)"

python3 - "$JOB_FILE" "$SHA_SHOWN" <<'PY'
import hashlib, json, sys
from datetime import datetime, timezone
job = json.load(open(sys.argv[1]))
shown = sys.argv[2]

def esc(s):
    out = []
    for c in s:
        if c == '"': out.append('\\"')
        elif c == '\\': out.append('\\\\')
        elif ord(c) < 0x20: out.append('\\u%04x' % ord(c))
        else: out.append(c)
    return ''.join(out)

canon = ('{"jobId":"%s","schema":%d,"createdUtc":"%s","expiresUtc":"%s","gameId":"%s",'
         '"companionUser":"%s","blueprintName":"%s","blueprintString":"%s",'
         '"anchor":{"x":%d,"y":%d,"z":%d},"rotationDeg":%d,"applyConfiguration":%s,'
         '"limits":{"maxItems":%d}}') % (
    esc(job['jobId']), job['schema'], esc(job['createdUtc']), esc(job['expiresUtc']),
    esc(job['gameId']), esc(job['companionUser']), esc(job['blueprintName']),
    esc(job['blueprintString']), job['anchor']['x'], job['anchor']['y'],
    job['anchor']['z'], job['rotationDeg'],
    'true' if job['applyConfiguration'] else 'false', job['limits']['maxItems'])
digest = hashlib.sha256(canon.encode('utf-8')).hexdigest()
assert digest == job['sha256'], f"file sha mismatch: {digest} != {job['sha256']}"
assert digest == shown, "confirm-page sha != file sha"
assert job['gameId'] == '5583213724730880438'
assert job['anchor'] == {'x': 3167, 'y': 762, 'z': 4}
assert job['rotationDeg'] == 90
assert job['limits']['maxItems'] == 500
assert job['companionUser'] == 'tester'
assert job['blueprintString'].startswith('B'), 'blueprint string missing'
created = datetime.strptime(job['createdUtc'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=timezone.utc)
expires = datetime.strptime(job['expiresUtc'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=timezone.utc)
assert (expires - created).total_seconds() == 1800, 'TTL is not 30 minutes'
print('job file canonical-hash + fields verified')
PY

# Single-use: replaying the same token writes nothing new.
REPLAY="$(curl -s -b "$WORK/cookies" -X POST "http://127.0.0.1:$PORT/place/confirm" \
  --data "csrf=$CSRF&token=$TOKEN")"
grep -q 'Confirmation expired' <<<"$REPLAY" || fail 'token replay was not refused'
COUNT="$(ls "$JOBS_DIR"/*.json | wc -l)"
[ "$COUNT" = "1" ] || fail 'replay wrote another job file'

LOGPAGE="$(get '/log')"
grep -q 'PlaceBlueprintJob' <<<"$LOGPAGE" || fail 'placement missing from /log'

# Game down: prepare must refuse, writing nothing.
kill $STUB 2>/dev/null || true
sleep 0.3
CODE="$(curl -s -o "$WORK/down.html" -w '%{http_code}' -b "$WORK/cookies" -X POST \
  "http://127.0.0.1:$PORT/place/prepare" \
  --data "csrf=$CSRF&slug=steel-v1&ax=0&ay=0&az=0&rotation=0")"
[ "$CODE" = "502" ] || fail "prepare with game down returned $CODE, expected 502"
grep -q 'Prepare refused' "$WORK/down.html" || fail 'refusal page text missing'

echo "PLACE PAGE CHECK PASS"
