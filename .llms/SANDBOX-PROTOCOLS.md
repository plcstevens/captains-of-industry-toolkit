# Sandbox test protocols — the two untested actuation surfaces

*Round 3, Chunk 4 — documentation only. Written 2026-11-03. **Nothing in
this file has been fired at any game.** Both protocols run only on a
throwaway sandbox save that Philip creates and loads himself; neither
protocol is ever run against the "Expanding" save.*

## What this settles, and why these two

The companion's command catalogue (SPEC Addendum VI/VII) has exactly
two actuation surfaces left whose behaviour is unknown rather than
merely unwired:

- **A — `CreateStaticEntityCmd`** *is* in the web API's `/commands`
  inventory (240 commands, captured `live/commands.json`), so the
  channel will accept the type. What is untested is its **semantics**:
  construction site vs finished entity, `TileTransform` encoding,
  material charging, and validation behaviour.
- **B — `ToggleEdictEnabledCmd`** exists inside the game (an
  `InputCommand` processed by `EdictsManager`, recon in SPEC
  Addendum VII + `bridge/CoiBridge/BridgeRoutesV6.cs` class comment)
  but is **not** in the web API's `/commands` inventory. What is
  untested is whether the web `/command` channel will execute a type
  absent from its own registry at all.

Each protocol is designed to finish in minutes once the sandbox save
is loaded, to answer its unknowns with PASS / FAIL / INCONCLUSIVE
verdicts, and to leave the sandbox exactly as it found it.

## Source discipline

Every command name, argument name, and JSON shape below is copied
from `live/commands.json` (the captured v0.8.8 inventory),
`bridge/MODDING-NOTES.md`, `bridge/CoiBridge/BridgeRoutes.cs` /
`BridgeRoutesV6.cs`, `coi/src/serve.rs`, or SPEC §12 / Addendum VI /
Addendum VII. Anything not copied from those files is labelled
**HYPOTHESIS** inline. Verbatim schemas are collected in the
Appendix.

Key verified facts both protocols rely on:

- The `/command` envelope is `{"type": "<CmdName>", "args": {...}}`,
  POSTed to `/command` at the server root (not under `/api/v1`) —
  `coi/src/serve.rs` `finish_prepare` / `do_apply` build exactly
  `json!({"type": cmd_type, "args": args})` and POST it to
  `/command`. A processed command answers in the shape
  `{"ok":true,"result":<result>,"error":"","processedAtStep":…}`
  (SPEC, Product correction 2026-11-02, live-verified for
  `SetSimPauseStateCmd` / `GameSpeedChangeCmd`).
- Vanilla read routes are under `/api/v1/…` (`game-info`,
  `products`, `products/stats`, `server-info`, …); `GET /commands`
  and `GET /screenshot` are at the root; CoiBridge routes are at the
  root under `/coi/v1/…` (`census`, `topology`, `storages`,
  `status`, `research`, `edicts`, …).
- Bridge **census** (`GET /coi/v1/census`) returns
  `{totalEntities, byProto:{proto:count}, machines:{…states…},
  transports:{total, fullyConnected}, priorities:{…}}`. It has **no
  per-entity constructed flag**.
- Bridge **topology** (`GET /coi/v1/topology`) returns
  `{entities:[{id, proto, pos:[x,y,z]}], transports:[{id, proto,
  from, to, fullyConnected, isMoving}], edges:[[id,id]]}` where
  `pos` is the entity's **centre tile** (`LayoutEntity.CenterTile`,
  BridgeRoutes.cs `BuildScans`). Topology carries **no rotation
  field** — this limits what Protocol A can prove about rotation
  (see A-U3).
- Bridge **storages** (`GET /coi/v1/storages`) returns one row per
  `StorageBase`: `{id, proto, product, quantity, capacity,
  percentFullRaw, logisticsInDisabled, logisticsOutDisabled,
  generalPriority, …}`.
- Bridge **edicts** (`GET /coi/v1/edicts`, CoiBridge v0.6+) returns
  `{edicts:[{id, isEnabled, isActive, canBeEnabled, reason,
  inactiveReason}]}` — field names from `BridgeRoutesV6.cs`
  `Edicts()`. Note: the route exposes **no Unity-cost field**;
  Protocol B's "low cost" selection leg is therefore an in-game
  eyeball read, not an API read.
- **No API surface in the repo reports the Unity balance** (bridge
  `status` is population/housing/food only; vanilla `products` does
  not list Unity). Any Unity cost is observable only from the
  in-game HUD — both protocols treat Unity deltas as eyeball data.

## Shared setup (both protocols)

### Access path for the requests

Run the requests from wherever is most convenient during the test
session; the shapes are identical either way:

1. **On Philip's PC (simplest — recommended while eyeballing):**

   ```bat
   curl -s -X POST http://127.0.0.1:3073/command -H "Content-Type: application/json" -d @body.json
   ```

   Dialling `localhost:3073` / `127.0.0.1:3073` satisfies the game's
   Host rule natively (`ACCESS.md`).
2. **From the Muse VM via the tailnet:** the `ACCESS.md` recipe —
   `curl -s --proxytunnel --proxy "${HTTPS_PROXY%:*}:3130" -H "Host: localhost:3073" "http://philip-gaming-home:3073/command" …`.
   Do **not** enable `allowOtherOriginsPost`; curl sends no Origin
   and needs none.

In the step bodies below, `BASE` means `http://127.0.0.1:3073` in
topology (1) or the tailnet URL in topology (2). Read routes used:

```sh
curl -s $BASE/api/v1/game-info
curl -s $BASE/commands
curl -s $BASE/api/v1/products/stats
curl -s $BASE/coi/v1/census
curl -s $BASE/coi/v1/topology
curl -s $BASE/coi/v1/storages
curl -s $BASE/coi/v1/edicts        # Protocol B; CoiBridge v0.6+
curl -s $BASE/coi/v1/status
```

**Why curl and not the companion UI for these tests:** the companion
only prepares commands present in its catalogue (`coi/src/serve.rs`
`do_prepare` rejects an unknown `type` with `400 Unknown command.`).
`CreateStaticEntityCmd` is catalogued (status `wired`), so the UI
*could* fire Protocol A's placement — but curl is specified anyway
for byte-exact request/response capture. `ToggleEdictEnabledCmd` is
**not** catalogued, so the companion will refuse it by design;
Protocol B is curl-only.

### The Expanding-save guard (hard abort)

The Expanding playthrough's `gameId` is `5583213724730880438`
(`ACCESS.md`, first live pull 2026-11-02; also
`live/game-info.json`). **Before firing anything, read
`GET /api/v1/game-info` and confirm `gameId` is NOT that value and
`sessionId` is the sandbox session's.** If either check is ambiguous,
stop — the wrong save is loaded.

### Run folder and baseline capture (shared)

Create one folder for the run, e.g. `sandbox-run-<YYYYMMDD-HHMM>/`
next to this file, and save every capture into it with the step
prefix used in the protocols (`A0-…`, `B0-…`). Baseline capture —
run once per protocol, before any POST:

```sh
curl -s $BASE/api/v1/game-info        > <run>/X0-game-info.json
curl -s $BASE/commands                > <run>/X0-commands.json
curl -s $BASE/api/v1/products/stats   > <run>/X0-products-stats.json
curl -s $BASE/coi/v1/census           > <run>/X0-census.json
curl -s $BASE/coi/v1/topology         > <run>/X0-topology.json
curl -s $BASE/coi/v1/storages         > <run>/X0-storages.json
curl -s $BASE/coi/v1/status           > <run>/X0-status.json
# Protocol B additionally:
curl -s $BASE/coi/v1/edicts           > <run>/B0-edicts.json
```

Also record in the run notes: in-game date/tick from `game-info`,
`isPaused`, save name as shown in the game's load menu, CoiBridge
version from `GET /coi/v1/hello`, and a screenshot of the test area
(Protocol A) / the edicts panel (Protocol B) — `GET $BASE/screenshot`
or an ordinary screenshot; either is fine as long as it is saved in
the run folder.

**Pause discipline:** keep the game **paused** for every step except
where a protocol explicitly opens a timed unpause window. Pausing is
itself a verified command if needed:
`{"type":"SetSimPauseStateCmd","args":{"isPaused":true}}`
(schema in the Appendix).

---

# PROTOCOL A — `CreateStaticEntityCmd` placement semantics

*Estimated run time once the sandbox save is loaded: ~10 minutes
(~15 with the optional sub-tests A7/A8).*

## A.0 Test subject and location strategy

### Test proto: `StorageUnit` (unit storage, tier 1)

**Pick: `protoId` = `"StorageUnit"`.** Justification, all from repo
data:

1. **The id spelling is verified, not guessed.** `StorageUnit`
   appears as a placed-entity proto in the live topology capture
   (`live/bridge-topology.json`: 8 instances on the Expanding save,
   alongside `StorageUnitT2` / `StorageUnitT3`, `StorageLoose`,
   `StorageFluid`). It is a `StaticEntityProto` — a layout building,
   exactly the class `CreateStaticEntityCmd`'s `protoId:
   StaticEntityProto.ID` addresses.
2. **Cheapest tier.** It is the tier-1 (unsuffixed) unit storage —
   the smallest material stake of any storage in the capture, and
   available at the start of a fresh game with no research gate to
   confound a validation failure. (If the sandbox's build menu shows
   it locked for any reason, stop and record that — do not silently
   substitute another proto; a locked proto is itself a validation
   data point, see A-U5 note.)
3. **Standalone.** An empty unit storage needs no power, water,
   product assignment, or port connection to exist and be observed.
   The protocol never assigns it a product, so it cannot perturb
   logistics while it exists.
4. **Observable on three independent read surfaces.** Census
   `byProto["StorageUnit"]` count, a new topology `entities[]` row
   `{id, proto:"StorageUnit", pos:[x,y,z]}`, and a new storages row
   (if/when the game surfaces it there — itself a discriminator,
   see A-U1). A pipe segment was rejected as the primary subject:
   transports in the topology carry `from`/`to` endpoints and are
   plausibly created through a different path than a single
   `TileTransform`; testing one would confound the transform
   question. Transports stay an explicit open question (Appendix).

### Location strategy (from the baseline topology capture)

Topology `pos` values are centre tiles in absolute tile coordinates
(BridgeRoutes.cs: `LayoutEntity.CenterTile`), and the live base sits
at `z = 4` in the Expanding capture — **never hardcode `z`**; copy it
from a neighbour in the sandbox's own capture.

1. From `A0-topology.json`, build the occupied set: every
   `entities[].pos` plus every `transports[].from` / `.to`.
2. Choose an **anchor**: any existing `StorageUnit` (or other
   storage) entity. Note its `pos` `[ax, ay, az]`.
3. Scan outward from the anchor (suggested: +x in steps of 2 tiles,
   then ±y) for a candidate centre `(X, Y, Z=az)` such that a square
   of **±8 tiles** around `(X, Y)` contains **no** occupied tile —
   the proto's footprint is read from the in-game build ghost at
   step A1, and ±8 covers any storage footprint plus a ≥2-tile
   margin on every side. Keep the candidate ≥6 tiles from the
   nearest transport tile so no port can auto-connect.
4. Record in the run notes: anchor id/proto/pos, chosen `(X, Y, Z)`,
   nearest occupied tile and its distance.
5. **Flatness is an eyeball check, not an API check** — topology has
   no height/slope data. At step A1 Philip confirms in-game that the
   candidate tiles are flat, unmined, and not water.

### The encoding hypotheses the test distinguishes

From `live/commands.json`, the `transform` JSON shape is verbatim:

```
{ position: { x: int, y: int, z: int } | ...,
  rotation?: { dx: int, dy: int } | ..., isReflected?: bool }
| { position: { x: int, y: int, z: int } | ..., rotFlip: int | ... }
```

- **HYPOTHESIS H-A-pos-1 (centre tile):** `position` is the new
  entity's centre tile in absolute tile coordinates — the same
  space topology reports. *Distinguisher:* the new topology row's
  `pos` equals `[X, Y, Z]` exactly.
- **HYPOTHESIS H-A-pos-2 (footprint origin):** `position` is a
  footprint corner/origin tile. *Distinguisher:* the new topology
  row's `pos` is offset from `[X, Y, Z]` by a consistent
  half-footprint vector. Either outcome resolves A-U2; "no entity
  anywhere" is a different failure (see criteria).
- **HYPOTHESIS H-A-rot-1 (direction vector):** rotation is accepted
  as `rotation: {dx:1, dy:0}` — a unit direction vector, `+x` =
  default facing. **HYPOTHESIS H-A-rot-2 (packed int):** the
  deserializer instead wants the alternate union arm,
  `rotFlip: 0`. *Distinguisher:* which body the channel accepts
  (steps A2 vs A2-fallback). Whether rotation *took effect* is a
  separate, weaker question — topology reports no rotation, and
  `StorageUnit`'s footprint may be square; see A-U3 criteria.

## A.1 Prerequisites checklist

- [ ] Fresh throwaway save created for testing (sandbox mode if the
      new-game settings offer it) — **not** the Expanding save, not
      a copy of it. Save named distinctly, e.g. `SANDBOX-proto-A`.
- [ ] CoiBridge installed and enabled on this save, **v0.6+**
      (any version works for A; v0.6+ keeps one mod version across
      both protocols).
- [ ] Mods enabled; console `web_api_start` run; game listening on
      `127.0.0.1:3073`.
- [ ] Companion **or** curl pointed at this sandbox session
      (Shared setup). If the companion is used at all in this
      session, it is only for read pages — all POSTs are curl.
- [ ] **Guard passed:** `game-info.gameId` ≠
      `5583213724730880438`; save name in the load menu is the
      sandbox name.
- [ ] Game **paused**; no construction, mining, or trade activity
      in progress that could move product totals during the test
      (a fresh sandbox normally satisfies this trivially).
- [ ] Baseline capture saved (Shared setup, prefix `A0-`):
      game-info, commands, products/stats, census, topology,
      storages, status.
- [ ] From `A0-commands.json`: `CreateStaticEntityCmd` is present
      with the constructor schema in the Appendix. If absent, stop:
      the inventory differs from the captured one — record and end.
- [ ] From `A0-census.json`: baseline `byProto["StorageUnit"]`
      count noted (call it `N0`; `0` is a valid baseline).
- [ ] From `A0-products-stats.json`: baseline `quantity` and
      `stored` noted for every product whose name contains
      `Construction` or `Concrete` (the plausible charge
      candidates; the full file is saved regardless, so an
      unexpected charged product is still caught by diffing).

## A.2 Unknowns register

| ID | Unknown | Resolved by |
|---|---|---|
| A-U1 | Does placement create a **construction site** or a **finished entity**? | A2–A4 (response, storages row, unpause window, eyeball) |
| A-U2 | `TileTransform.position` semantics: centre tile (H-A-pos-1) vs footprint origin (H-A-pos-2); `z` = tile height level as topology reports it | A3 readback diff |
| A-U3 | Which rotation serialization the channel accepts (`rotation{dx,dy}` vs `rotFlip`); whether rotation observably takes effect | A2 / A2-fallback; effect: A8 (optional, may stay INCONCLUSIVE) |
| A-U4 | Are materials **charged**? If so: at placement (reservation), on delivery during construction, or never (free)? Is Unity involved? | A3/A4 products diffs + HUD eyeball |
| A-U5 | Validation with `allowValidationSuppression:false` at a deliberately invalid (overlapping) spot: rejected cleanly? With `true` at the **same** spot: still rejected, or placed anyway? | A5, A6 |
| A-U6 | What the `/command` response actually carries (`result` = new `EntityId`?), and whether census/topology/storages deltas match it | A2–A3 |

## A.3 Steps

### A1 — Pick and eyeball the spot (no POST)

1. Choose `(X, Y, Z)` by the location strategy (A.0) from
   `A0-topology.json`.
2. In-game, open the build menu, select the unit storage, and hover
   the ghost over the candidate tiles. Record: the footprint size
   the ghost shows (tiles × tiles), whether the ghost is green
   (valid) at the candidate, and the material cost the build panel
   lists for one unit storage (verbatim — this is the expected
   charge A-U4 is checked against).
3. Screenshot the ghost/panel into the run folder.
4. If the ghost is red at every candidate the strategy produces
   (slope, water, hidden obstruction), pick the nearest spot where
   the ghost *is* green and record the deviation — the test needs
   one known-good spot, and the ghost is the ground truth for
   "good".

### A2 — Fire the valid placement (suppression off, rotation form 1)

Write this exact body to `<run>/A2-body.json`, substituting the
chosen `X, Y, Z` (field names and nesting are verbatim from the
`live/commands.json` schema; only the coordinate *values* are
run-specific). `allowValidationSuppression:false` is the schema
default, stated explicitly so the record shows it was off.

```json
{"type":"CreateStaticEntityCmd","args":{"protoId":"StorageUnit","transform":{"position":{"x":X,"y":Y,"z":Z},"rotation":{"dx":1,"dy":0}},"allowValidationSuppression":false}}
```

```sh
curl -s -i -X POST $BASE/command -H "Content-Type: application/json" -d @<run>/A2-body.json > <run>/A2-response.txt
```

Record: HTTP status, full response body verbatim, wall-clock time.
**Do not retry on any error** — an error here is data (proceed to
A2-fallback only for the specific case below).

**A2-fallback (rotation form 2) — only if** A2's response is a
clean client error (HTTP 4xx, or `ok:false`) whose error text
concerns the `transform` / `rotation` / deserialization — not
validation, not placement. Then, at the *same* `(X, Y, Z)`:

```json
{"type":"CreateStaticEntityCmd","args":{"protoId":"StorageUnit","transform":{"position":{"x":X,"y":Y,"z":Z},"rotFlip":0},"allowValidationSuppression":false}}
```

Save as `A2b-body.json` / `A2b-response.txt`. If A2-fallback also
fails, or A2 failed for any other reason: skip to A.5 (abort
conditions) — record A-U3/A-U6 from the error text and end the
placement arm; A5/A6 are meaningless without a known-good success
shape.

### A3 — Immediate readback (game still paused)

Within a minute of A2, capture and save with prefix `A3-`:
census, topology, storages, products/stats, status, game-info.
Then record:

1. **New entity:** diff topology `entities[]` against `A0`. The new
   row (expected: exactly one) gives `id` (call it `E1`) and `pos`.
   Compare `pos` to `[X, Y, Z]` (→ A-U2 verdict).
2. **Response cross-check:** if A2's response `result` is an
   integer, is it `E1`? (→ A-U6.) If `result` is `true`/`null`/a
   string, record verbatim — the id then comes only from the
   topology diff, which is also a valid A-U6 answer.
3. **Census:** `byProto["StorageUnit"]` — expect `N0 + 1`;
   `totalEntities` — expect baseline + 1.
4. **Storages row:** is there a row with `id == E1` already? Its
   `capacity` (0 vs the proto's real capacity) is a site-vs-finished
   signal for A-U1.
5. **Products (paused):** diff every product's `quantity`/`stored`
   against `A0`. Any change *while paused* = charge-at-placement
   (reservation) evidence for A-U4.
6. **Eyeball:** Philip looks at the spot in-game and records what
   is rendered — construction-site scaffold/ghost with a progress
   or materials panel, or a finished building — plus the site's
   demanded-materials list if a panel shows one, and the HUD Unity
   figure (before value from the A1 screenshot).

### A4 — Controlled unpause window (site vs finished, delivery charge)

1. Note the HUD Unity figure and the in-game clock.
2. Unpause at speed 1 for **exactly 60 seconds of wall time**, then
   re-pause. (In-game pause toggle, or the verified
   `SetSimPauseStateCmd` pair via `/command` — either is fine;
   record which.)
3. Capture with prefix `A4-`: census, topology, storages,
   products/stats, game-info. Eyeball again.
4. Record: did trucks move toward `E1` / did a construction
   progress bar advance / did the entity complete? Did any
   product's `stored` drop during the window (→ charge-on-delivery,
   A-U4)? Did the storages row for `E1` appear or change capacity?
   Did Unity change?

*If A3 already proved a finished entity beyond doubt (full-capacity
storages row + finished rendering + no construction panel), A4 may
be shortened to a 10-second unpause sanity check — record that the
short form was used.*

### A5 — Invalid placement, suppression OFF

Target: the anchor's own `pos` `[ax, ay, az]` from `A0-topology`
— a tile-for-tile overlap with a standing entity, invalid by
construction while every other property (terrain, z, proto) is
known-good.

```json
{"type":"CreateStaticEntityCmd","args":{"protoId":"StorageUnit","transform":{"position":{"x":ax,"y":ay,"z":az},"rotation":{"dx":1,"dy":0}},"allowValidationSuppression":false}}
```

(Use whichever rotation form A2 proved the channel accepts.)
Save body/response as `A5-body.json` / `A5-response.txt`, then
immediately capture census + topology (`A5-` prefix) and record:
HTTP status, `ok`, `error` text verbatim, and whether **any** new
entity appeared anywhere (full diff, not just at the target).

### A6 — Invalid placement, suppression ON (same spot)

Identical to A5 except `"allowValidationSuppression":true`:

```json
{"type":"CreateStaticEntityCmd","args":{"protoId":"StorageUnit","transform":{"position":{"x":ax,"y":ay,"z":az},"rotation":{"dx":1,"dy":0}},"allowValidationSuppression":true}}
```

Save as `A6-*`, capture census + topology immediately, full-diff.
If an entity was created, note its id (`E2`) and readback `pos` —
it goes on the revert list (A.4) alongside `E1`.

### A7 — OPTIONAL sub-test: `FinishBuildOfStaticEntityCmd`

*Only if A-U1's verdict is "construction site", the site is still
unfinished after A4, and Philip explicitly agrees to spend the
Unity — the command's own inventory description is "Quick build
or quick removal of an entity, paid with Unity." Skip by default.*

Note the argument name: the schema's parameter is **`id`**, not
`entityId` (verbatim, Appendix).

```json
{"type":"FinishBuildOfStaticEntityCmd","args":{"id":E1}}
```

Record: HUD Unity before/after (eyeball — no API read exists),
response verbatim, and post-capture (`A7-` census/topology/
storages) showing whether `E1` became a finished, full-capacity
storage. A failure here does **not** fail A-U1; it records the
finish path as unproven and the site is left to complete or be
reverted normally.

### A8 — OPTIONAL sub-test: rotation effect probe

*Only if A2 succeeded and time permits.* Repeat A2 at a second
clear spot `(X2, Y2, Z)` (location strategy again) with the
rotation changed to `{"dx":0,"dy":1}` (or `rotFlip` +1 step if
form 2 won). Record response + topology readback, and — decisively
— the **in-game rendering**: does the building's facing/ports point
the other way, or is the footprint indistinguishable (square
proto)? Revert this entity (`E3`) with the others. If the rendering
is indistinguishable, A-U3's effect leg stays INCONCLUSIVE by
design and needs a non-square proto in a future round — say so in
the results record rather than guessing.

## A.4 Observation table

Record one row per step, in the run notes, at the time of the step
(not from memory afterwards):

| Step | Record verbatim / note |
|---|---|
| A0 | gameId, sessionId, date, tick, isPaused; `N0` (census StorageUnit count); baseline Construction/Concrete `quantity`+`stored`; CoiBridge version |
| A1 | anchor id/proto/pos; chosen X,Y,Z; nearest occupied tile + distance; ghost footprint (tiles×tiles); ghost valid?; build-panel material cost (verbatim); HUD Unity |
| A2 (A2b) | request body (file name); HTTP status; response body verbatim; wall time |
| A3 | new topology row (id, proto, pos) or "none"; pos vs [X,Y,Z] delta; census StorageUnit count + totalEntities; storages row for E1 (present? capacity); products diff while paused (product: Δquantity/Δstored) or "no change"; eyeball: site vs finished, demanded materials, HUD Unity |
| A4 | unpause method + duration; truck/progress activity at E1; completed?; products diff over the window; storages row change; HUD Unity |
| A5 | request body; HTTP status; ok/error verbatim; any new entity anywhere (id/pos) or "none" |
| A6 | same as A5; if created: E2 id + readback pos |
| A7 (opt) | HUD Unity before/after; response verbatim; E1 finished? capacity now |
| A8 (opt) | second spot coords; rotation form used; response; readback pos; eyeball facing difference or "indistinguishable" |
| Revert | per entity: command used, response verbatim, post-revert census counts vs baseline |

## A.5 PASS / FAIL / INCONCLUSIVE criteria (per unknown)

- **A-U1 (site vs finished)**
  - PASS "construction site": any two of — in-game rendering/panel
    shows a construction site for E1; storages row absent or
    capacity 0 at A3 and becomes real only after completion;
    A4 shows deliveries/progress/completion over time.
  - PASS "finished entity": any two of — finished building rendered
    at A3 with no construction panel; storages row present at A3
    with the proto's full capacity; A4 shows no construction
    activity because there is nothing to construct.
  - INCONCLUSIVE: rendering and read surfaces disagree (e.g.
    finished rendering but no storages row ever) — record both
    halves verbatim; do not force a verdict.
- **A-U2 (position semantics)**
  - PASS H-A-pos-1: readback `pos == [X, Y, Z]`.
  - PASS H-A-pos-2: readback `pos` offset by a constant vector
    consistent with half the A1 ghost footprint.
  - FAIL (both): no new entity, or the new entity's `pos` matches
    neither pattern (record the actual pos — a third semantics
    exists; that is a finding, not a mess).
- **A-U3 (rotation)**
  - PASS (acceptance leg): exactly one of the two forms is accepted
    and the other cleanly rejected, or form 1 is accepted at A2
    (form 2 then untested — record as "form 1 accepted; form 2
    untested", still a PASS for form 1).
  - PASS (effect leg): only via A8 with a visible facing/footprint
    difference.
  - INCONCLUSIVE (effect leg): no A8, or A8 rendering
    indistinguishable. Expected outcome for a square proto — plan
    on it.
- **A-U4 (charging)**
  - PASS "charged at placement": a products diff exists already at
    A3 (paused), matching the A1 build-panel cost.
  - PASS "charged on delivery": no A3 diff; A4 diff accumulates to
    the panel cost as deliveries land (allow partial if the site
    hasn't completed — record delivered-so-far vs panel cost).
  - PASS "not charged": site completes (or entity is finished)
    with zero products diff across A3+A4 and zero Unity movement.
  - INCONCLUSIVE: diffs exist but don't reconcile with the panel
    cost (background activity in the sandbox polluted the totals)
    — record the raw diffs.
- **A-U5 (validation / suppression)**
  - PASS: A5 is rejected (HTTP error or `ok:false`) with **no** new
    entity, **and** A6 gives a determinate outcome at the same
    spot: rejected-identically (suppression does not cover
    overlap) or accepted-with-entity (suppression bypasses this
    validation class). Either A6 outcome is a PASS — the unknown
    was *what it does*, not a hoped-for answer.
  - FAIL: A5 is *accepted* (an overlapping entity appears with
    suppression off) — validation did not run where the schema
    implies it should; record and treat any A6 entity as E2 for
    revert.
  - INCONCLUSIVE: A5/A6 responses are transport-level failures
    (connection drop, HTTP 500 without a parseable error) — the
    channel's answer is unknowable from that run.
  - *Scope note:* this resolves the **overlap** class only. Slope,
    water, and research-locked-proto validation are separate
    classes, untested by this protocol (Appendix).
- **A-U6 (response & deltas)**
  - PASS: A2's response is captured verbatim and either (a)
    `result` equals the topology-diff id E1, or (b) `result`
    demonstrably carries no id (record its actual type/value) and
    E1 was recovered by diff — both are complete answers.
  - FAIL: a new entity exists but neither the response nor any
    read surface can attribute it (should be impossible given
    topology; if it happens, the diff procedure is the finding).

## A.6 Revert steps

Revert **every** entity this protocol created (E1, plus E2/E3 if
A6/A8 created any), in reverse creation order. Wait for the game to
be paused-or-idle; the sandbox is disposable, but a clean revert is
what proves the demolish path the agent will rely on.

1. **Primary — start deconstruction** (the normal demolish path;
   schema verbatim in the Appendix). For entity `E`:

   ```json
   {"type":"StartDeconstructionOfStaticEntityCmd","args":{"entityId":E,"reason":"Remove","isAreaRemove":false}}
   ```

   POST to `$BASE/command`; save the response (`R<n>-response.txt`).
   Then unpause briefly and let trucks carry out the deconstruction
   (a construction *site* may vanish immediately; a finished storage
   takes a few in-game minutes). Re-pause, capture census +
   topology: the entity must be gone and
   `byProto["StorageUnit"]` back to the count implied by the
   remaining test entities.
2. **Fallback — quick remove with refund**, only if step 1 is
   rejected or the entity is still present after ~5 in-game
   minutes:

   ```json
   {"type":"QuickRemoveEntityRefundCmd","args":{"entityId":E}}
   ```

   Record the response and any products diff (refund behaviour is
   bonus data — note it, don't chase it).
   ⚠️ Do **not** confuse this with `QuickRemoveFromEntityCmd`
   (same inventory, different command — it removes something *from*
   an entity and returns an `EntityId`). Check the `type` string
   character-for-character before sending.
3. **Last resort — in-game demolish:** use the game's own demolish
   tool on the entity and record that both command paths failed
   (itself a catalogue finding).
4. **Final verification:** census + topology captures (`R-final-`)
   diffed against `A0`: `totalEntities` and `byProto` counts back
   to baseline; no test entity remains. Record any products delta
   that did **not** return to baseline (spent/refunded amounts) in
   the results record — "net cost of the test was X" is part of the
   finding.

## A.7 Abort conditions

Stop immediately, revert whatever exists (A.6), and record the run
as aborted if any of these occur:

- The Shared-setup guard trips (Expanding `gameId`, or the loaded
  save is not the sandbox save) — abort *before* any POST.
- A2 creates an entity anywhere other than the target area, or
  more than one entity (census `totalEntities` delta > 1).
- Any products diff at A3/A4 is larger than the A1 build-panel cost
  by an order of magnitude (something else is being charged).
- The game crashes, the web server stops answering, or census /
  topology stop updating (bridge down) mid-protocol.
- Philip's eyeball sees the placement land on a transport, road,
  or another building (readback pos notwithstanding).
- Any response body suggests save corruption or contains a stack
  trace — do not fire further commands at a wounded session; note
  it and close the game without saving if Philip agrees.

---

# PROTOCOL A — REVISION A-R (2026-11-05, after recon round 9)

*Static recon (`qol-recon/09-placement.md`, decompiles of the shipped
0.8.8 assemblies) settled most of the A.2 unknowns register from
code. The original Protocol A text above is preserved unchanged —
this revision supersedes its unknowns register and step list with
the minimal live sequence below. **The Shared-setup Expanding-save
guard (gameId ≠ `5583213724730880438`) applies unchanged and aborts
the run before any POST.***

## A-R.1 Unknowns register — verdicts from code

| ID | Verdict (KNOWN FROM CODE — do not re-test) |
|---|---|
| A-U1 | **Construction site.** Placement auto-starts construction (`StaticEntity.OnAddedToWorld` → `ConstructionManager.StartConstruction`); nothing is deducted at placement — cost is per-product delivery buffers trucks must fill. |
| A-U2 | **Centre tile.** `LayoutEntityBase.SetTransform`: `CenterTile = transform.Position`. Readback `pos` must equal `[X, Y, Z]` exactly. The footprint-origin hypothesis is dead. |
| A-U3 | **Acceptance settled; effect still live-only.** `rotation:{dx,dy}` takes the four unit axis vectors (Deg0=(1,0), Deg90=(0,1), Deg180=(−1,0), Deg270=(0,−1)); never send (0,0). Use this form, not `rotFlip` (bit packing unread). Whether rotation *took effect* still needs the eyeball leg (R4) — topology has no rotation field. |
| A-U4 | **Charged on delivery, never at placement.** A products diff while paused would *contradict* the code — if one appears, stop and record it as a finding. Unity is involved only via FinishBuild (A7's subject; semantics now code-known, price still HUD-only). |
| A-U5 | **Mechanics settled; one proto fact open.** Suppression only acts on protos implementing `IProtoWithValidationSuppressFlag` (flags are the proto's, not the caller's); a suppressed error adds the entity anyway and files it in the invalid-entities registry. Whether `StorageUnit` carries such flags is the only live question (R5). |
| A-U6 | **Response carries the id.** Success = `SetResultSuccess(resultEntity.Id)`; failure = result-error with the validator's message and the instance destroyed. The topology diff is now a cross-check, not the source. |

`FinishBuildOfStaticEntityCmd` (old A7) no longer needs a test to
be *understood*: ConstructionManager runs it as Unity-paid quick
build (in construction) or quick removal (in deconstruction), and
it result-errors on a finished, non-upgradable entity. Run it only
if Philip wants its Unity price observed on the HUD.

## A-R.2 The minimal live sequence

Prerequisites: the original A.1 checklist, unchanged (sandbox save,
guard passed, baseline capture `A0-`, game paused). Location: the
A.0 strategy, unchanged — but the candidate is now known to be a
**centre tile**, so target it as one.

### R1 — One valid placement (channel + response + position)

Fire the original A2 body unchanged (protoId `StorageUnit`,
candidate centre `(X, Y, Z)`, `rotation:{"dx":1,"dy":0}`,
`allowValidationSuppression:false`).
*Readbacks:* response verbatim (expect success, `result` = new id
`E1`); topology diff (exactly one new `StorageUnit` row, `pos` ==
`[X, Y, Z]`); census `StorageUnit` = N0+1; products diff while
paused (**expect none** — a diff is a code-contradicting finding,
record and stop); storages row for `E1` may exist with the site —
its capacity behaviour is observation, not a verdict input.
*Settles:* live unknowns 1 (channel) and the readback half of 3.

### R2 — Revert the untouched site (instant-removal leg)

Game still paused, nothing delivered. Fire the original A.6.1 body:
`StartDeconstructionOfStaticEntityCmd` {entityId: E1,
reason:"Remove", isAreaRemove:false}.
*Readbacks:* response verbatim; immediate census/topology capture —
code predicts the entity is **already gone** (0 steps + empty
buffers = instant removal, no refund, nothing spent).
*Settles:* the cheap-revert leg of the deconstruction state machine.

### R3 — Delivery + completion (the construction flow, observed once)

Repeat R1 at a second clear centre `(X2, Y2, Z)` → `E2`. Note the
build panel's material cost (A1 eyeball). Unpause at speed 1 until
the site completes (trucks deliver; allow a few in-game days),
re-pause.
*Readbacks:* products/stats diff over the window (deliveries should
accumulate to the panel cost as stock drops); storages row for `E2`
at full proto capacity once constructed; eyeball: finished building
rendered, no construction panel.
*Revert:* `StartDeconstructionOfStaticEntityCmd` on `E2`, unpause
until the deconstruction completes (trucks haul the refund out),
re-pause; final census/topology vs baseline (counts restored).
Record the products delta that does **not** return to baseline —
code predicts it equals (1 − DeconstructionRefundMultiplier) ×
cost; that diff is how the multiplier's value gets measured
(live unknown 5).
*Settles:* live unknowns 3 (behavioural half) and 5.

### R4 — Rotation effect (eyeball only)

Place a third `StorageUnit` at `(X3, Y3, Z)` with
`rotation:{"dx":0,"dy":1}`. *Readback:* the in-game rendering /
screenshot vs the R1/R3 buildings — do ports/facing point the other
way? (A square footprint may still read as indistinguishable; if
so, record INCONCLUSIVE as the original A8 did and move on — the
acceptance question is already settled.) Revert as in R2 (paused,
undelivered → instant).

### R5 — Validation legs (failure shape + suppression fact)

Fire the R1 body at the anchor's own occupied centre
`[ax, ay, az]`, suppression **false**. *Expect (code):* result-error
carrying the validator's message, `result` invalid, **no** new
entity on full topology diff. Then — only if Philip agrees — the
identical body with suppression **true**: rejection-identical means
`StorageUnit` has no suppress flags (the code-predicted default);
an entity appearing means the proto carries flags and the entity
is in the invalid registry — **revert it immediately** (R2 body)
and record that suppression on this proto is a real hazard, not a
no-op.
*Settles:* live unknown 4 and the failure-shape half of 1.

## A-R.3 What this revision removes

The original A2-fallback (rotation form 2), A4's separate
site-vs-finished unpause window (folded into R3), A7 (FinishBuild
understanding — code-known; optional price eyeball only), and A8 as
a separate optional arm (folded into R4) are no longer steps. The
abort conditions (A.7) and the revert discipline (A.6) apply to
R1–R5 unchanged, with one addition: **a products diff while paused
at R1 aborts the run** (it contradicts the decompiled cost model —
something else is happening).

**Not covered here, deliberately:** belts/pipes and blueprint
placement. Their creation commands (`BuildTransportCmd`,
`BatchCreateStaticEntitiesCmd`) are **absent from the web
registry** (recon 09 §4) — they belong to the Protocol B channel
question, and no placement result in R1–R5 changes that.

---

# PROTOCOL B — `ToggleEdictEnabledCmd` via `POST /command`

*Estimated run time once the sandbox save is loaded and a qualifying
edict exists: ~5 minutes.*

## ⚖️ VERDICT — SETTLED FROM CODE, LIVE TEST RETIRED (recon round 10, 2026-11-05)

**Do not run B1–B5. The channel question this protocol existed to
test is answered deterministically by the shipped code**
(`qol-recon/10-transports-blueprints.md` §Q1):

- `WebCommandRoutes.command` resolves `type` by dictionary lookup in
  a prebuilt `WebCommandIndex`. A miss is HTTP 400: *"Command type
  '\<name\>' is unknown or cannot be built from JSON yet."* There is
  no reflection/activation fallback.
- The index only contains types with ≥1 constructor that
  `JsonObjectBuilder.CanBuild` accepts. The builder structurally
  blocks collection parameters and any non-command reference type.
- `ToggleEdictEnabledCmd`'s only constructor takes the **live
  `EdictProto` object** → unbuildable from JSON → absent from the
  index. **H-B-env is false at the type level, not the key level** —
  no argument-key spelling (the B3 ladder) could ever have worked.
  B-U1: NO. B-U2/B-U3: moot (the rejection text above is the
  deterministic answer). B-U4: untestable via this channel.
- The same verdict covers `BuildTransportCmd` and
  `BatchCreateStaticEntitiesCmd` (collection constructor parameters
  → blocked). Belts, pipes, blueprints and edict toggles are **all**
  unreachable via `POST /command` in this build.

**The abort guard survives the retirement:** nothing in this verdict
licenses sending these types anywhere else. Any future live probe of
the replacement channel (below) still requires a throwaway save and
`game-info.gameId` ≠ `5583213724730880438` before anything fires.

**Replacement channel:** the QoS builder-mod design note in recon 10
§Q5 — a suite mod scheduling the shipped commands via
`IInputScheduler` from single-use, expiring, gameId-pinned job files
the companion writes only at the §12 confirm step, results reported
through the QoSEventLog feed. **Proposal only — needs Philip's
explicit posture decision before anything is built.** The original
protocol text below is preserved for the record; its steps are
retired, not deleted.

## B.0 What is actually being tested

Recon (SPEC Addendum VII; `BridgeRoutesV6.cs` class comment):
`EdictsManager` (`Mafi.Core.Population.Edicts`) processes a command
named `ToggleEdictEnabledCmd` — an `InputCommand` with a **single
field carrying an `EdictProtoId`** — but the type is **absent from
the web API's `/commands` inventory** (the captured
`live/commands.json` contains zero edict commands; grep confirms no
`Edict` type among the 240). The web registry is therefore not the
game's full command set, and the open question is purely about the
**channel**: does the `/command` endpoint resolve types beyond its
own published inventory, or is the inventory the allowlist?

Verification does not depend on the channel's say-so alone:
CoiBridge v0.6+ `GET /coi/v1/edicts` reports `isEnabled` per edict
from `EdictsManager` directly, so an executed toggle is provable
independently of whatever `/command` claims.

### The hypothesis under test

**HYPOTHESIS H-B-env (envelope + argument name):** the channel
takes the standard envelope with the argument key spelled
`edictProtoId`:

```json
{"type":"ToggleEdictEnabledCmd","args":{"edictProtoId":"<id>"}}
```

The envelope shape itself is verified (Shared setup). The type
name is verified to exist in the game (recon above). The argument
**key** `edictProtoId` is the hypothesis — it follows the game's
own naming pattern in the captured inventory (e.g.
`PinToggleCmd.productToToggle`, `StorageSetProductCmd.productId`,
`QuickTradeCmd` matching on a proto id), and the recon statement
that the command's single field carries an `EdictProtoId`. If the
channel answers with a parameter error, the bounded variant ladder
in B3 tests the plausible alternate keys — and **only** those.

**HYPOTHESIS H-B-toggle (toggle semantics):** with a single
proto-id field and no boolean, the command *flips* the edict's
enabled state — sending it a second time disables what the first
send enabled. The revert step (B5) is therefore also a test: if
the second send does not flip back, the command is enable-only
(or set-to-a-default), which is a finding, and the fallback revert
is the in-game edicts panel.

## B.1 Prerequisites checklist

- [ ] Fresh throwaway save for testing (sandbox mode if offered) —
      **not** the Expanding save, not a copy of it. Named
      distinctly, e.g. `SANDBOX-proto-B` (one shared sandbox save
      for A+B is fine if Protocol A finished reverted; separate
      saves keep the records cleaner).
- [ ] CoiBridge **v0.6 or newer** installed and enabled on this
      save — `GET /coi/v1/edicts` must answer. (v0.5 has no edicts
      route; without the route there is no independent
      verification and this protocol must not run.)
- [ ] Mods enabled; `web_api_start` run; game on `127.0.0.1:3073`.
- [ ] Curl pointed at the sandbox session (Shared setup). **Curl
      only** — the companion's prepare flow will refuse this type
      as unknown (by design, see Shared setup).
- [ ] **Guard passed:** `game-info.gameId` ≠
      `5583213724730880438`; loaded save is the sandbox save.
- [ ] Game **paused** (an enabled edict's effects then cannot bite
      during the on/off window; the toggle pair in B4/B5 is in any
      case minutes apart at most).
- [ ] Baseline capture saved (Shared setup, prefix `B0-`),
      **including `B0-edicts.json`**.
- [ ] From `B0-commands.json`: confirm `ToggleEdictEnabledCmd` is
      **absent**. (If a future game build lists it, this protocol
      is moot — record that and stop; the channel question has
      been answered by the inventory itself.)
- [ ] A qualifying test edict selected per B.2 and its `id`
      recorded.

## B.2 Test-subject selection rule (runtime, from `/coi/v1/edicts`)

The repo contains **no edict list** — no names, no costs — so the
subject is chosen from the live `B0-edicts.json` at runtime.
A qualifying edict must satisfy **all** of:

1. `isEnabled == false` — currently off, so the first toggle is an
   enable (the direction whose effect is easiest to attribute).
2. `canBeEnabled == true` (with `reason` empty) — the game's own
   gate says it *can* be switched on in this save; without this, a
   no-op result would be ambiguous between "channel rejected" and
   "game refused".
3. **Lowest Unity cost among the qualifiers, read in-game:** the
   edicts route exposes no cost field, so Philip opens the edicts
   panel, reads the monthly Unity cost of each qualifier, and
   picks the cheapest — preferring a zero-cost edict if one
   qualifies. **Reject** any qualifier whose panel shows a one-off
   activation charge (a sunk cost the revert cannot recover), and
   any that the panel marks as conflicting with / replacing an
   already-enabled edict.
4. **Boring effect:** among the cheapest, prefer the edict whose
   panel description touches nothing the sandbox is actively using
   (in a fresh sandbox: almost anything qualifies; when in doubt,
   pick the one whose description sounds most administrative).

Record the chosen `id`, its panel cost, and the two runner-up
qualifiers (for the B3 ladder / a re-run). **Precondition-fail
exit:** if *no* edict in `B0-edicts.json` satisfies 1+2 — plausible
in a brand-new save before a settlement exists or edict research
lands — stop here and record "precondition fail: no qualifying
edict in this save yet", with the full edicts list attached. Grow
the sandbox in-game until one qualifies (settlement + edict
unlocks), or accept the INCONCLUSIVE-precondition result; do
**not** relax rule 2 to force a run — a toggle fired at an edict
the game would refuse anyway proves nothing about the channel.

## B.3 Unknowns register

| ID | Unknown | Resolved by |
|---|---|---|
| B-U1 | Does the web `/command` channel accept a command type **absent from its `/commands` inventory** at all? | B4 response + B4 readback |
| B-U2 | Exact argument key: `edictProtoId` (H-B-env), or an alternate key? | B4, variant ladder if needed |
| B-U3 | How does the response distinguish **unknown-type rejection** from **execution** (status code, `ok`, `error` text) — i.e. can a future caller tell them apart without a readback? | B2 control vs B4 response diff; ladder error texts |
| B-U4 | Effect and reversibility: does `isEnabled` actually flip (independently verified), does `isActive` follow, and does a second identical send flip it back (H-B-toggle)? | B4 readback, B5 |

## B.4 Steps

### B0 — Baseline (done in prerequisites)

`B0-edicts.json` saved; chosen edict `id` = `<EDICT_ID>` below,
with `isEnabled:false, canBeEnabled:true` in the baseline.
Also note the baseline `isEnabled` of **every other** edict — B4's
readback is a full-list diff, not a single-row check.

### B1 — Control: capture this session's known-good success shape

Fire one verified, effectively no-op command — re-asserting the
pause the game is already in — and save the response as the
session's canonical success envelope:

```json
{"type":"SetSimPauseStateCmd","args":{"isPaused":true}}
```

```sh
curl -s -i -X POST $BASE/command -H "Content-Type: application/json" -d @<run>/B1-body.json > <run>/B1-response.txt
```

Record: HTTP status and the body verbatim (expected shape per
SPEC: `{"ok":true,"result":true,"error":"","processedAtStep":…}` —
if this control does *not* answer in that shape, the session's
channel behaves differently from every verified session; stop and
record, do not proceed to B4 on a miscalibrated control).

### B2 — (merged into B1; numbering kept so run notes match the template)

### B3 — Variant ladder (pre-committed, only used if B4 demands it)

Do not improvise keys. If B4's answer is a clean parameter/argument
error (see B.5 criteria for what "clean" means), retry the send
with **one** change at a time, in this order, saving each body and
response as `B4v<n>-*`:

1. `{"type":"ToggleEdictEnabledCmd","args":{"protoId":"<EDICT_ID>"}}`
2. `{"type":"ToggleEdictEnabledCmd","args":{"id":"<EDICT_ID>"}}`
3. `{"type":"ToggleEdictEnabledCmd","args":{"edictId":"<EDICT_ID>"}}`

**HYPOTHESIS** labels apply to all three keys (they are naming
conventions from the captured inventory, nothing more). Hard rules
for the ladder: after **each** variant, re-read `/coi/v1/edicts`
before sending another — a variant that executed must not be
followed by another send (that would toggle it back off and poison
the record); stop the ladder at the first variant that flips
`isEnabled`; never climb the ladder on a transport failure or HTTP
500 (see abort conditions).

### B4 — Fire the toggle (hypothesis body)

```json
{"type":"ToggleEdictEnabledCmd","args":{"edictProtoId":"<EDICT_ID>"}}
```

```sh
curl -s -i -X POST $BASE/command -H "Content-Type: application/json" -d @<run>/B4-body.json > <run>/B4-response.txt
```

Record HTTP status + body verbatim + wall time. Then,
**regardless of what the response claims**, within a minute
capture:

```sh
curl -s $BASE/coi/v1/edicts > <run>/B4-edicts.json
curl -s $BASE/api/v1/game-info > <run>/B4-game-info.json
```

and diff the full edicts list against `B0-edicts.json`: the target
edict's `isEnabled` / `isActive` / `canBeEnabled` / reasons, and
confirmation that **no other edict changed**. Also note the HUD
Unity figure if Philip is watching it (expected movement: none
while paused, beyond the panel's stated upkeep once time runs —
record whatever is seen, don't chase it).

Decision tree after B4 (+ readback):

- `isEnabled` flipped → B-U1/B-U2 resolved by this body; go to B5.
- Response is a clean parameter error naming an argument problem →
  climb the B3 ladder (one rung at a time, readback between).
- Response claims success (`ok:true`) but nothing flipped → the
  channel accepted-and-dropped it, or the argument was silently
  ignored: climb the ladder once through (each rung with readback);
  if no rung flips it, that's the determinate negative result for
  B-U1/B-U2 — record all responses verbatim and go to B6.
- Response is an explicit unknown-type rejection → B-U1 resolved
  (negative) and B-U3 resolved by the verbatim error; skip B5,
  go to B6.
- Anything else (500, timeout, crash) → abort conditions, B.7.

### B5 — Revert: send the identical body again

```json
{"type":"ToggleEdictEnabledCmd","args":{"edictProtoId":"<EDICT_ID>"}}
```

(Or the ladder variant that worked in B4 — revert uses the *proven*
body, byte-identical to the successful send.) Save as
`B5-body.json` / `B5-response.txt`, then capture
`B5-edicts.json` and diff against `B0`:

- Target edict back to `isEnabled:false` → H-B-toggle holds;
  revert complete via the channel.
- Still enabled → H-B-toggle fails: record it, and revert through
  the **in-game edicts panel** (click the edict off). Record that
  the panel revert was needed — this is a catalogue-level finding
  (actuation would be one-way via `/command`).
- Either way, confirm no other edict differs from baseline.

### B6 — Close-out captures

Save final `B6-edicts.json` + `B6-game-info.json`. The edicts list
must equal baseline exactly; if it does not and B5's in-game
fallback was already used, stop and hand the discrepancy to Philip
in-game rather than firing further commands.

## B.5 Observation table

| Step | Record verbatim / note |
|---|---|
| B0 | gameId, sessionId; full edicts baseline (file); chosen `<EDICT_ID>` + why it qualified (isEnabled, canBeEnabled, panel Unity cost); runner-up qualifiers |
| B1 | control request body; HTTP status; response body verbatim |
| B4 | request body (file); HTTP status; response body verbatim; wall time; post readback: target `isEnabled`/`isActive` before→after; any other edict changed? (must be "none"); HUD Unity note |
| B4v<n> | the one changed key; response verbatim; readback flip? (per rung) |
| B5 | request body (= proven B4 body); response verbatim; target back to baseline?; other edicts unchanged?; if panel revert used, note it |
| B6 | final edicts diff vs B0: "identical" or the exact discrepancy |

## B.6 PASS / FAIL / INCONCLUSIVE criteria (per unknown)

- **B-U1 (channel accepts unlisted type)**
  - PASS (yes): any send (B4 or a ladder rung) flips `isEnabled`
    in the independent bridge readback.
  - PASS (no — a determinate negative is still a pass for the
    *unknown*): every well-formed body is refused with an
    explicit, parseable unknown-type error, and the readback never
    moves. The catalogue then records edicts as "bridge-write or
    in-game only", which is the decision this test exists to make.
  - INCONCLUSIVE: responses are transport failures / 500s /
    contradictory (success claimed, ladder exhausted, no flip,
    no error text to explain it). Record all bodies verbatim.
- **B-U2 (argument key)**
  - PASS: the exact body that flipped the edict is identified
    (H-B-env's `edictProtoId`, or a named ladder rung).
  - INCONCLUSIVE: B-U1 ends INCONCLUSIVE, or a flip is observed
    but cannot be attributed to one body (only possible if the
    ladder's readback discipline was broken — record how).
- **B-U3 (rejection vs execution, distinguishable?)**
  - PASS: the B1 control response and the B4(/ladder) responses
    differ in a stable, nameable way for the two outcomes —
    e.g. success = HTTP 200 + `ok:true` + a `processedAtStep`,
    rejection = a distinct status or an `error` string naming the
    type/parameter problem. Quote both verbatim in the results
    record; the quoted pair *is* the criterion.
  - FAIL (the distinction does not exist): rejection and success
    are indistinguishable in the response (e.g. both `ok:true`),
    with only the bridge readback telling them apart — a real
    finding: the channel would then require readback-verification
    for every unlisted type, forever.
  - INCONCLUSIVE: B4 never produced a clean rejection to compare
    (e.g. first body executed; ladder unused) — in that case
    B-U3 is *partially* resolved: execution's shape is known,
    rejection's is not. Record it as such, not as PASS.
- **B-U4 (effect + reversibility)**
  - PASS: B4 flips `isEnabled` false→true; B5 flips it back
    true→false; full-list diffs show no other edict moved at any
    point. `isActive` is recorded at each readback; if `isActive`
    lags `isEnabled` (plausible — an enabled edict may only become
    active when its conditions hold), that is noted, not failed.
  - FAIL: a flip happens but the second send does not reverse it
    (one-way actuation), or a *different* edict flips (wrong-target
    actuation — also an abort trigger if seen mid-run).
  - INCONCLUSIVE: no flip ever occurs (rides on B-U1's negative /
    inconclusive outcome).

## B.7 Abort conditions

Stop, do not climb the ladder further, and record as aborted if:

- The Shared-setup guard trips (Expanding `gameId` / wrong save) —
  abort before any POST.
- Any readback shows a **non-target** edict changed state.
- Any response is an HTTP 500 with a stack trace, the connection
  drops mid-send (outcome unknowable — the send may or may not
  have executed; resolve by readback only, never by re-send), or
  the game crashes.
- The HUD Unity figure drops by materially more than the panel
  cost recorded at selection (unexpected charging) — leave the
  edict state as-is, note it, and let Philip decide in-game.
- `/coi/v1/edicts` stops answering (bridge down): without the
  independent readback, further sends are blind — stop.

**Standing rule for both protocols:** a dropped/ambiguous send is
resolved by *reading*, never by re-sending. A duplicated toggle is
a state change, not a retry.

---

# Results-record template (both protocols)

Paste the completed record back to Claudius (chat is fine) together
with the run folder's file names — the verbatim responses are what
turn the run into SPEC/skill findings, so **do not paraphrase
response bodies**. One record per protocol run.

```
SANDBOX PROTOCOL RESULT — <A | B>
Date/time (Paris):            <YYYY-MM-DD HH:MM>
Operator:                     Philip (in-game) / <other>
Game version:                 <e.g. 0.8.8>
CoiBridge version (/coi/v1/hello): <...>
Save name:                    <sandbox save name — NOT Expanding>
gameId:                       <from game-info; must ≠ 5583213724730880438>
sessionId:                    <from game-info>
Run folder:                   <path / file prefix used>

SETUP
  Baseline captures saved:    <list, e.g. A0-game-info.json, A0-census.json, ...>
  Guard check passed:         <yes — how verified>

[Protocol A only]
  Anchor (id/proto/pos):      <...>
  Chosen position (X,Y,Z):    <...>
  Ghost footprint / cost:     <tiles×tiles / verbatim panel cost>
  Rotation form attempted:    <rotation{dx,dy} | rotFlip>

[Protocol B only]
  Chosen edict id:            <EDICT_ID>
  Qualifier evidence:         <isEnabled / canBeEnabled from B0 + panel Unity cost>
  Control (B1) response:      <verbatim body + HTTP status>

STEPS (one block per POST fired)
  Step:        <A2 | A2b | A5 | A6 | A7 | A8 | B4 | B4v1..3 | B5 | revert R…>
  Request:     <file name, or the exact body inline>
  HTTP status: <...>
  Response:    <verbatim body>
  Readback:    <files + the one-line diff summary>

UNKNOWNS — verdict per ID with the evidence pointer
  <A-U1..A-U6 | B-U1..B-U4>:  <PASS (answer: …) | FAIL | INCONCLUSIVE> — <which step/file proves it>

EYEBALL NOTES (things no API reports)
  <site vs finished rendering / edict panel text / HUD Unity before→after / anything odd>

REVERT
  Entities/edicts reverted:   <ids + method per item (deconstruction cmd / quick-remove / panel)>
  Final diff vs baseline:     <"identical" | the exact residual deltas, incl. net product cost>

ANOMALIES / ABORTS
  <none | what happened, at which step, what was done>
```

## How the finding lands (for Claudius, on receipt)

- Protocol A verdicts update SPEC Addendum VI's open sentence
  ("what remains untested is its semantics") and the
  `CreateStaticEntityCmd` catalogue note in
  `coi/src/catalogue.rs` (generated — edit via
  `tools/gen_catalogue.py` inputs, not by hand): site-vs-finished,
  transform semantics, charging, suppression behaviour.
- Protocol B verdicts update SPEC Addendum VII's edict paragraph
  and the companion Trade page's "toggling edicts is not wired
  yet" note (`coi/src/serve.rs`): if B-U1 passes, edicts join the
  §12 prepare→confirm flow like every other command; if it fails
  determinately, edict actuation is recorded as a CoiBridge write
  route decision for Philip.
- Either protocol's verbatim rejection texts get quoted into the
  relevant skill (`local-coi-playing` / `local-coi-bridge-dev`) —
  error strings are the part future sessions cannot reconstruct.

---

# Appendix — verbatim schemas and scope limits

## Schemas copied from `live/commands.json` (v0.8.8 capture)

`CreateStaticEntityCmd` — namespace
`Mafi.Core.Entities.Static.Commands`, result `EntityId`:

```json
{"name":"protoId","type":"StaticEntityProto.ID","json":"string"}
{"name":"transform","type":"TileTransform","json":"{ position: { x: int, y: int, z: int } | ..., rotation?: { dx: int, dy: int } | ..., isReflected?: bool } | { position: { x: int, y: int, z: int } | ..., rotFlip: int | ... }"}
{"name":"allowValidationSuppression","type":"bool","default":false,"description":"Whether this entity can suppress validation, if other conditions are met."}
```

`FinishBuildOfStaticEntityCmd` — namespace
`Mafi.Core.Entities.Static.Commands`, result `EntityId`,
description "Quick build or quick removal of an entity, paid with
Unity." Single parameter: `{"name":"id","type":"EntityId",
"json":"int"}` — note `id`, not `entityId`.

`StartDeconstructionOfStaticEntityCmd` — namespace
`Mafi.Core.Entities.Static.Commands`, result `bool`:

```json
{"name":"entityId","type":"EntityId","json":"int"}
{"name":"reason","type":"EntityRemoveReason","json":"string","values":["Remove","Collapse"]}
{"name":"isAreaRemove","type":"bool","default":false}
```

`QuickRemoveEntityRefundCmd` — namespace `Mafi.Core.Entities`,
result `bool`. Single parameter `entityId` (`EntityId`, int).
⚠️ Distinct from `QuickRemoveFromEntityCmd` (namespace
`Mafi.Core.Entities.Static`, result `EntityId`, also single
`entityId`) — see Protocol A.6 step 2.

`SetSimPauseStateCmd` — namespace `Mafi.Core.Simulation`, result
`bool`, description "Pauses or resumes the game." Single parameter
`{"name":"isPaused","type":"bool"}`.

`ToggleEdictEnabledCmd` — **not in `live/commands.json`.** Known
from recon only (SPEC Addendum VII; `BridgeRoutesV6.cs`): an
`InputCommand` processed by `EdictsManager`
(`Mafi.Core.Population.Edicts`) with a single field carrying an
`EdictProtoId`. The argument key used in Protocol B
(`edictProtoId`) and the ladder alternates (`protoId`, `id`,
`edictId`) are **HYPOTHESIS** — that is what the protocol tests.

Also catalogued but deliberately **not used** by these protocols:
`ToggleStaticEntityConstructionCmd{entityId}` and
`SetConstructionPausedCmd{entityId, isPaused?}` /
`SetConstructionPriorityCmd{entityId, priority}` (construction
controls that become interesting only once A-U1 is known).

## What these protocols cannot resolve (stated in advance)

1. **Rotation's effect on a square-footprint proto, from API data
   alone.** Topology carries no rotation field; only Philip's
   eyeball (A8) or a future non-square-proto round can settle the
   effect leg of A-U3.
2. **Validation classes other than overlap.** A5/A6 test one
   invalid class (standing-entity overlap). Slope, water,
   off-terrain, and research-locked-proto placements each need
   their own deliberate-invalid test; `allowValidationSuppression`
   may behave differently per class.
3. **Per-proto construction costs in general.** Protocol A
   measures the charge for `StorageUnit` only, as a products diff
   reconciled against the build panel. Sandbox background activity
   can pollute the diff (hence the pause discipline); the game's
   per-proto cost table is not exposed by any route in the repo.
4. **Unity amounts via API.** No route reports the Unity balance
   (bridge `status` covers population/housing/food; vanilla
   `products` has no Unity entry). The Unity legs of A-U4 and the
   `FinishBuildOfStaticEntityCmd` sub-test are eyeball-only.
5. **Transport placement.** Whether `CreateStaticEntityCmd` (or
   any sibling) can place pipes/belts — entities the topology
   models with `from`/`to` endpoints rather than a single centre
   tile — is untouched here; `CreateTrainTrackEntityWithDirectionCmd`
   (same `TileTransform` + a `direction` parameter) hints rails
   have their own path. A separate protocol, once A lands.
6. **Edict `isActive` timing.** B-U4 records `isActive` at each
   readback, but an enabled edict becoming *active* may depend on
   in-game conditions/time in ways a minutes-long paused test
   cannot force; the `isActive` leg may legitimately end
   INCONCLUSIVE while `isEnabled` resolves cleanly.
7. **Edict costs via API.** `/coi/v1/edicts` exposes no cost
   field, so B.2's "low Unity cost" leg depends on the in-game
   panel until a future bridge version exposes costs — a bridge
   gap the run should note if it bites.
8. **Whether an unlisted type that *executes* is safe to
   catalogue.** B-U1 passing proves the channel accepts
   `ToggleEdictEnabledCmd`; it does not prove the channel accepts
   *arbitrary* unlisted types — each further unlisted type is its
   own sandbox question.

---

## Protocol A — RESULT (Revision A-R minimal live test, 2026-11-05)

Run on Philip's REAL save ("Expanding", gameId 5583213724730880438) at his
explicit in-the-moment direction, as the minimal variant: R1 + R2 only,
game PAUSED throughout (tick 2787522 never moved; commands process while
paused, trucks cannot deliver). Both commands went through the companion's
§12 prepare -> confirm -> apply flow (scratch serve + scratch DB) and are
audit/activity logged there.

- R1 PLACE: CreateStaticEntityCmd {protoId: "StorageUnit", transform:
  {position: {x:3167, y:762, z:4}, rotation: {dx:1, dy:0}},
  allowValidationSuppression: false}
  -> {"ok":true, "result":38208, "error":""} — result IS the new EntityId.
  Readback: census 12503 -> 12504; topology shows StorageUnit id 38208 at
  exactly [3167, 762, 4]. CENTRE-TILE SEMANTICS CONFIRMED LIVE.
- R2 REMOVE: StartDeconstructionOfStaticEntityCmd {entityId: 38208,
  reason: "Remove", isAreaRemove: false}
  -> {"ok":true, "result":true}. Readback: census back to 12503,
  StorageUnit rows back to 8, entity 38208 absent. Construction Parts
  quantities unchanged (763 / 760 / 89 before and after): ZERO COST.
- Verdict: channel acceptance, centre-tile transform, and the instant
  untouched-site revert are all VERIFIED. CreateStaticEntityCmd +
  StartDeconstructionOfStaticEntityCmd moved to the catalogue's VERIFIED
  set (15 verified / 224 wired / 1 excluded). Still unproven (full A-R on
  a sandbox save): delivery + completion of a web-placed site (R3),
  rotation's visible effect (R4), validation legs + refund measurement
  (R5), and the unlisted-command question for BuildTransportCmd /
  BatchCreateStaticEntitiesCmd (Protocol B) — **that last item is
  since SETTLED FROM CODE by recon round 10 (2026-11-05): unlisted
  types can never be built from POST /command; see the Protocol B
  verdict block and qol-recon/10-transports-blueprints.md.**
