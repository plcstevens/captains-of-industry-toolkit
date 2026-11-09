#!/usr/bin/env bash
# Scratch check for the Designate flow (SPEC Addendum XX,
# DESIGNATION-DESIGN-NOTE stage 4). Serves the companion against
# the stub game with the v0.6.5/0.6.6 synthetic capture
# (dev/captures/live-synthetic-v065: bridge serves /coi/v1/terrain/region and
# /coi/v1/designations in the 0.6.6 shape) and APPDATA pointed
# at a scratch dir, then:
#   1. /designate lists the live designations (proto strings +
#      corners) and the scratch jobs folder;
#   2. pad prepare renders the exact schema-2 job pinned to the
#      stub's gameId, with cells + edge-step warnings;
#   3. confirm writes ONE job file; an independent Python
#      re-implementation of the canonical-v2 hash verifies it
#      (the same check QoSBuilder 0.2.0 runs in-game), and the
#      outcome page shows the honest "verdict pending" panel;
#   4. the confirm token is single-use (replay writes nothing);
#   5. /designate then shows the latest-job verdict panel;
#   6. remove prepare (origin picked from the live read-back)
#      writes a second, remove-shaped job file, also verified;
#   7. with the stub down, prepare refuses (502) and /designate
#      degrades to 200-with-notice — never a 500.
# No real game, no real APPDATA: everything lives in a mktemp dir.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
COI="${1:-$HOME/workspace/coi-companion/coi/target/debug/superintendent}"
PORT="${PORT:-8488}"
STUB_PORT="${STUB_PORT:-3998}"
WORK="$(mktemp -d)"
trap 'kill ${SRV:-} ${STUB:-} 2>/dev/null || true; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/appdata"

python3 "$HERE/stub_game_server.py" --port "$STUB_PORT" \
  --capture "$HERE/../dev/captures/live-synthetic-v065" >"$WORK/stub.log" 2>&1 &
STUB=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$STUB_PORT/api/v1/game-info"; then break; fi; sleep 0.2
done

APPDATA="$WORK/appdata" "$COI" --db "$WORK/coi.db" \
  --game-url "http://127.0.0.1:$STUB_PORT" serve --port "$PORT" \
  >"$WORK/serve.log" 2>&1 &
SRV=$!
for _ in $(seq 1 50); do
  if curl -s -o /dev/null "http://127.0.0.1:$PORT/"; then break; fi; sleep 0.2
done

get() { curl -s "http://127.0.0.1:$PORT$1"; }
fail() { echo "FAIL: $1"; exit 1; }
csrf() { get "$1" | grep -o 'name="csrf" value="[^"]*"' | awk 'NR==1' | sed 's/.*value="//; s/"//'; }

PAGE="$(get '/designate')"
grep -q 'Designate' <<<"$PAGE" || fail '/designate heading missing'
grep -q 'MiningDesignator' <<<"$PAGE" || fail 'live designation proto missing (MiningDesignator)'
grep -q 'DumpingDesignator' <<<"$PAGE" || fail 'live designation proto missing (DumpingDesignator)'
grep -q '(20, 12)' <<<"$PAGE" || fail 'mining origin (20, 12) not listed'
grep -q '\[6, 9, 9, 6\]' <<<"$PAGE" || fail '0.6.6 corners not shown for the mining designation'
grep -q "$WORK/appdata/Captain of Industry/QoS/jobs" <<<"$PAGE" \
  || fail 'jobs folder shown is not the APPDATA default'
grep -q 'gameId 5583213724730880438' <<<"$PAGE" || fail 'stub gameId not shown'

CSRF="$(csrf '/designate')"
[ -n "$CSRF" ] || fail 'no csrf on /designate'

# --- Pad prepare: rect (44,24)-(56,32) inside the stub's canned
# region (40,20)-(64,38); terrain there is 15.4-17.0 tiles, so a
# target of 12 must produce edge-step warnings on every side.
CONFIRM="$(curl -s -X POST "http://127.0.0.1:$PORT/designate/prepare" \
  --data "csrf=$CSRF&op=pad&x0=44&y0=24&x1=56&y1=32&target=12&proto=MiningDesignator&name=Check%20pad")"
grep -q 'Confirm designation' <<<"$CONFIRM" || fail 'pad prepare did not render the confirm sheet'
grep -q '5583213724730880438' <<<"$CONFIRM" || fail 'confirm sheet not pinned to the stub gameId'
grep -q '6 Mining cells' <<<"$CONFIRM" || fail 'confirm sheet does not show 6 cells'
grep -q 'Edge-step warnings' <<<"$CONFIRM" || fail 'edge-step warnings missing (terrain 15-17 vs target 12)'
grep -q 'designations-add' <<<"$CONFIRM" || fail 'job JSON missing designations-add'
TOKEN="$(grep -o 'name="token" value="[^"]*"' <<<"$CONFIRM" | awk 'NR==1' | sed 's/.*value="//; s/"//')"
SHA_SHOWN="$(grep -o 'sha256 is <code>[0-9a-f]*</code>' <<<"$CONFIRM" | sed 's/.*<code>//; s/<.*//')"
[ -n "$TOKEN" ] || fail 'no confirm token'
[ -n "$SHA_SHOWN" ] || fail 'no sha256 on the confirm sheet'

DONE="$(curl -s -X POST "http://127.0.0.1:$PORT/designate/confirm" \
  --data "csrf=$CSRF&token=$TOKEN")"
grep -q 'Designation job written' <<<"$DONE" || fail 'confirm did not write the job'
grep -q 'verdict pending' <<<"$DONE" || fail 'outcome page lacks the honest verdict-pending panel (stub never applies jobs)'

JOBS_DIR="$WORK/appdata/Captain of Industry/QoS/jobs"
JOB_FILES=("$JOBS_DIR"/*.json); COUNT="${#JOB_FILES[@]}"
[ "$COUNT" = "1" ] || fail "expected exactly 1 job file, found $COUNT"

python3 - "$JOBS_DIR" "$SHA_SHOWN" <<'PY'
import glob, hashlib, json, sys
from datetime import datetime, timezone

def esc(s):
    out = []
    for c in s:
        if c == '"': out.append('\\"')
        elif c == '\\': out.append('\\\\')
        elif ord(c) < 0x20: out.append('\\u%04x' % ord(c))
        else: out.append(c)
    return ''.join(out)

files = glob.glob(sys.argv[1] + '/*.json')
assert len(files) == 1, files
job = json.load(open(files[0]))
shown = sys.argv[2]
cells = ','.join(
    '{"x":%d,"y":%d,"origin":%d,"plusX":%d,"plusXY":%d,"plusY":%d}'
    % (c['x'], c['y'], c['origin'], c['plusX'], c['plusXY'], c['plusY'])
    for c in job['cells'])
canon = ('{"jobId":"%s","schema":%d,"jobType":"%s","createdUtc":"%s","expiresUtc":"%s",'
         '"gameId":"%s","companionUser":"%s","planName":"%s","protoId":"%s",'
         '"cells":[%s],"limits":{"maxCells":%d}}') % (
    esc(job['jobId']), job['schema'], esc(job['jobType']), esc(job['createdUtc']),
    esc(job['expiresUtc']), esc(job['gameId']), esc(job['companionUser']),
    esc(job['planName']), esc(job['protoId']), cells, job['limits']['maxCells'])
digest = hashlib.sha256(canon.encode('utf-8')).hexdigest()
assert digest == job['sha256'], f"file sha mismatch: {digest} != {job['sha256']}"
assert digest == shown, "confirm-sheet sha != file sha"
assert job['schema'] == 2 and job['jobType'] == 'designations-add'
assert job['protoId'] == 'MiningDesignator'
assert job['gameId'] == '5583213724730880438'
assert job['companionUser'] == 'local'
assert job['planName'] == 'Check pad'
assert job['limits']['maxCells'] == 200
origins = [(c['x'], c['y']) for c in job['cells']]
assert origins == [(44, 24), (48, 24), (52, 24), (44, 28), (48, 28), (52, 28)], origins
assert all([c['origin'], c['plusX'], c['plusXY'], c['plusY']] == [12, 12, 12, 12]
           for c in job['cells']), 'pad cells are not level at 12'
created = datetime.strptime(job['createdUtc'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=timezone.utc)
expires = datetime.strptime(job['expiresUtc'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=timezone.utc)
assert (expires - created).total_seconds() == 1800, 'TTL is not 30 minutes'
print('add job file canonical-v2 hash + fields verified')
PY

# Single-use: replaying the same token writes nothing new.
REPLAY="$(curl -s -X POST "http://127.0.0.1:$PORT/designate/confirm" \
  --data "csrf=$CSRF&token=$TOKEN")"
grep -q 'Confirmation expired' <<<"$REPLAY" || fail 'token replay was not refused'
JOB_FILES=("$JOBS_DIR"/*.json); COUNT="${#JOB_FILES[@]}"
[ "$COUNT" = "1" ] || fail 'replay wrote another job file'

# The Designate page now carries the latest-job verdict panel.
PAGE2="$(get '/designate')"
grep -q 'Latest designation job' <<<"$PAGE2" || fail 'latest-job verdict panel missing on /designate'
grep -q 'verdict pending' <<<"$PAGE2" || fail 'latest-job panel does not show verdict pending'

# --- Remove prepare: origin (20, 12) picked from the read-back
# (first row, sel_0). The stub read-back never changes, so the
# outcome verdict stays pending — the honest state.
RCONFIRM="$(curl -s -X POST "http://127.0.0.1:$PORT/designate/prepare" \
  --data "csrf=$CSRF&op=remove&sel_0=20,12")"
grep -q 'Confirm designation' <<<"$RCONFIRM" || fail 'remove prepare did not render the confirm sheet'
grep -q 'MiningDesignator' <<<"$RCONFIRM" || fail 'remove sheet does not show the picked designation kind'
grep -q 'designations-remove' <<<"$RCONFIRM" || fail 'remove job JSON missing designations-remove'
RTOKEN="$(grep -o 'name="token" value="[^"]*"' <<<"$RCONFIRM" | awk 'NR==1' | sed 's/.*value="//; s/"//')"
[ -n "$RTOKEN" ] || fail 'no remove confirm token'

RDONE="$(curl -s -X POST "http://127.0.0.1:$PORT/designate/confirm" \
  --data "csrf=$CSRF&token=$RTOKEN")"
grep -q 'Designation job written' <<<"$RDONE" || fail 'remove confirm did not write the job'
JOB_FILES=("$JOBS_DIR"/*.json); COUNT="${#JOB_FILES[@]}"
[ "$COUNT" = "2" ] || fail "expected 2 job files after remove confirm, found $COUNT"

python3 - "$JOBS_DIR" <<'PY'
import glob, hashlib, json, sys

def esc(s):
    out = []
    for c in s:
        if c == '"': out.append('\\"')
        elif c == '\\': out.append('\\\\')
        elif ord(c) < 0x20: out.append('\\u%04x' % ord(c))
        else: out.append(c)
    return ''.join(out)

found = None
for f in glob.glob(sys.argv[1] + '/*.json'):
    job = json.load(open(f))
    if job.get('jobType') == 'designations-remove':
        found = job
assert found is not None, 'no remove job file written'
job = found
assert 'protoId' not in job and 'cells' not in job, 'remove job carries add-only fields'
origins = ','.join('{"x":%d,"y":%d}' % (o['x'], o['y']) for o in job['origins'])
canon = ('{"jobId":"%s","schema":%d,"jobType":"%s","createdUtc":"%s","expiresUtc":"%s",'
         '"gameId":"%s","companionUser":"%s","planName":"%s",'
         '"origins":[%s],"limits":{"maxCells":%d}}') % (
    esc(job['jobId']), job['schema'], esc(job['jobType']), esc(job['createdUtc']),
    esc(job['expiresUtc']), esc(job['gameId']), esc(job['companionUser']),
    esc(job['planName']), origins, job['limits']['maxCells'])
digest = hashlib.sha256(canon.encode('utf-8')).hexdigest()
assert digest == job['sha256'], f"remove file sha mismatch: {digest} != {job['sha256']}"
assert job['origins'] == [{'x': 20, 'y': 12}], job['origins']
assert job['limits']['maxCells'] == 200
print('remove job file canonical-v2 hash + fields verified')
PY

LOGPAGE="$(get '/log')"
grep -q 'PlaceDesignationJob' <<<"$LOGPAGE" || fail 'designation jobs missing from /log'

# Game down: prepare must refuse (502), /designate degrades to
# 200-with-notice — never a 500, never a job.
kill $STUB 2>/dev/null || true
sleep 0.3
CODE="$(curl -s -o "$WORK/down.html" -w '%{http_code}' -X POST \
  "http://127.0.0.1:$PORT/designate/prepare" \
  --data "csrf=$CSRF&op=pad&x0=44&y0=24&x1=56&y1=32&target=12&proto=MiningDesignator")"
[ "$CODE" = "502" ] || fail "prepare with game down returned $CODE, expected 502"
grep -q 'Prepare refused' "$WORK/down.html" || fail 'refusal page text missing'
CODE="$(curl -s -o "$WORK/down2.html" -w '%{http_code}' \
  "http://127.0.0.1:$PORT/designate")"
[ "$CODE" = "200" ] || fail "/designate with game down returned $CODE, expected 200-with-notice"
grep -q 'Game unreachable' "$WORK/down2.html" || fail '/designate degraded notice missing'
JOB_FILES=("$JOBS_DIR"/*.json); COUNT="${#JOB_FILES[@]}"
[ "$COUNT" = "2" ] || fail 'game-down prepare wrote a job file'

echo "DESIGNATE PAGE CHECK PASS"
