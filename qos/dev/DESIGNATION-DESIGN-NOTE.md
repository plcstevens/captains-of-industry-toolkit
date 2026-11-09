# QoSBuilder designation jobs — DESIGN NOTE (stage 1 of 4)

Stage 1 = read-only scoping, per Philip's instruction (2026-11-06):
proceed slowly, one stage at a time, identify exactly where work
stops if it stops. **No code was changed and nothing was built for
this note.** Assembly evidence is from the 0.8.8 `Mafi.Core.dll` in
`~/workspace/coi-companion/refs/`, decompiled with ilspycmd on
2026-11-06; file/type names are given for every claim so any of it
can be re-checked. Companion/mod evidence is from the sources as
they stand (QoSBuilder 0.1.0, bridge 0.6.5, companion schema v4).

The channel verdict itself is recon round 14 (bridge/MODDING-NOTES.md)

+ BEHAVIOURS.md §0: designations are executable via the QoS scheduler
route, never via the web §12 channel (the `ImmutableArray<…>` ctor
params are JSON-blocked). `QuickFulfillDesignationsCmd` remains
**excluded by posture** — it appears below only where the manager's
code forces a mention; no stage may use it.

---

## 1. Assembly evidence — the designation types

### 1.1 `DesignationData` — exact shape

`Mafi.Core.Terrain.Designation.DesignationData` (readonly struct,
`IEquatable<DesignationData>`):

```csharp
public readonly Tile2i       OriginTile;
public readonly HeightTilesI OriginTargetHeight;
public readonly HeightTilesI PlusXTargetHeight;
public readonly HeightTilesI PlusYTargetHeight;    // declared 4th…
public readonly HeightTilesI PlusXyTargetHeight;   // …but see ctor order
public readonly HeightTilesF CenterTargetHeight;

[LoadCtor]
public DesignationData(Tile2i originTile,
    HeightTilesI originTargetHeight, HeightTilesI plusXTargetHeight,
    HeightTilesI plusXyTargetHeight, HeightTilesI plusYTargetHeight,
    HeightTilesF centerTargetHeight);
public DesignationData(Tile2i originTile, HeightTilesI originTargetHeight,
    HeightTilesI plusXTargetHeight, HeightTilesI plusXyTargetHeight,
    HeightTilesI plusYTargetHeight);               // centre derived
public DesignationData(Tile2i originTile, HeightTilesI allHeights); // flat cell
```

Facts that shape the design:

+ **Ctor/serialization order is Origin, PlusX, PlusXY, PlusY,
  Center** — PlusXY comes *before* PlusY in the ctor parameter list
  and in `Serialize`, even though the field declarations list PlusY
  first. Any code (or canonical job form) that orders corners must
  use the ctor order or it will swap the +Y and +XY corners.
+ **Every ctor asserts alignment**: `Assert.That(originTile % 4)
  .IsZero("Given tile is not origin of a designation.")`. Origins
  are 4-grid cell origins by construction; `TerrainDesignation
  .GetOrigin(p)` is `(p.X & -4, p.Y & -4)`.
+ **Corners are integer tiles.** `Mafi.HeightTilesI` is a readonly
  struct over a plain `int Value` (`new HeightTilesI(int)`);
  `Mafi.HeightTilesF` wraps `Fix32`. Fractional corner targets are
  unrepresentable — the planner works in whole levels for corners,
  exactly as snapping procedure v2 already assumes.
+ **The centre is derivable, so the job file should not carry it.**
  The 5-arg ctor computes `CenterTargetHeight` as the mean of the
  four corner ints, as Fix32: `(o + x + y + xy).Over(4)`. If the mod
  constructs via the 5-arg (or flat 2-arg) ctor, the game derives
  the centre identically to any other constructor of the same cell.
  Derived-not-transported removes a whole inconsistency class.
+ Corner tile coords: `PlusXTileCoord = Origin + (4,0)`,
  `PlusYTileCoord = Origin + (0,4)`, `PlusXyTileCoord = Origin +
  (4,4)`, `CenterTileCoord = Origin + (2,2)` — a cell's corners are
  the shared vertices of the 4×4 tile block.
+ `Equals` compares origin + all four corners + centre — the
  in-process equality ATD verifies with.

### 1.2 The commands — and what `Proto.ID` denotes

```csharp
// Mafi.Core.Terrain.Designation
public class AddTerrainDesignationsCmd : InputCommand<ImmutableArray<Tile2i>> {
    public readonly Proto.ID ProtoId;
    public readonly ImmutableArray<DesignationData> Data;
    public AddTerrainDesignationsCmd(Proto.ID protoId, ImmutableArray<DesignationData> data);
}
public class RemoveDesignationsCmd : InputCommand<ImmutableArray<Tile2i>> {
    public readonly ImmutableArray<Tile2i> Origins;
    public RemoveDesignationsCmd(ImmutableArray<Tile2i> origins);
}
```

+ **`Proto.ID` denotes the `TerrainDesignationProto` to create —
  i.e. the KIND.** Kind is a property of the *batch*, not of a cell:
  the command carries exactly one `ProtoId` for the whole array,
  and `DesignationData` has no kind member. A mixed-kind plan is
  therefore multiple batches (→ multiple job files at one confirm,
  §2).
+ The ID strings (`Mafi.Core.IdsCore.TerrainDesignators` static
  ctor): `MiningDesignator = new Proto.ID("MiningDesignator")`,
  `"DumpingDesignator"`, `"LevelDesignator"`,
  `"ForestryDesignator"` (plus surface/decal designators —
  out of scope, excluded).
+ Per-proto parameters (`Mafi.Core.Data.TerrainDesignationsData
  .RegisterData`, ctor args in `TerrainDesignationProto` order):

  | Proto.ID | preferInitial BelowTerrain | maxAssigned Entities | warn when no tower | isTerra forming | fulfilment fn |
  |---|---|---|---|---|---|
  | MiningDesignator | false | 1 | true | true | mining |
  | DumpingDesignator | true | 2 | false | true | dumping |
  | LevelDesignator | true | 1 | true | true | mining + dumping |
  | ForestryDesignator | false | 1 | true | **false** | fertile ∧ unoccupied |

+ Execution channel: `Mafi.Core.Input.IInputScheduler
  .ScheduleInputCmd<T>(T cmd) where T : IInputCommand` returns the
  command; `InputCommand<TResult>` exposes `IsProcessed`,
  `ResultSet`, `HasError`, `ErrorMessage`, `Result`. This is the
  same channel/injection QoSBuilder already uses
  (`BuilderService.cs:366`).

### 1.3 Manager semantics — where the sharp edges are

`Mafi.Core.Terrain.Designation.TerrainDesignationsManager`
implements `ICommandProcessor<>` + `IAction<>` for all three
commands.

**Add** (`IAction<AddTerrainDesignationsCmd>.Invoke`):

+ Unknown proto → `SetResultError(ImmutableArray<Tile2i>.Empty,
  "Failed to find proto '{0}'.")`, nothing placed.
+ Otherwise it loops **every** item through
  `AddOrReplaceDesignation(proto, data)`, collects **all** origins,
  and calls `SetResultSuccess(all origins)` — *regardless of
  whether individual cells were accepted*. **The command result
  cannot distinguish placed from refused cells. Read-back is not a
  nicety; it is the only verdict** (the same honesty rule as
  blueprints, one level up).
+ `AddOrReplaceDesignation`: if `IsDesignationAllowed` fails →
  returns `false`, **no state change** (an existing designation at
  that origin is *not* removed). If allowed → removes the existing
  designation at the origin (**any** proto) **and any surface
  designation at that origin**, inserts the new one, runs the
  no-tower warning check, fires `DesignationAdded`.
+ `IsDesignationAllowed(data, out error)` refuses when:
  1. `origin != TerrainDesignation.GetOrigin(origin)` (misaligned)
     → `DesignationError__Invalid`;
  2. the whole 4×4 cell is not inside terrain limits
     (`origin < TerrainManager.MinOnLimits`, or
     `origin + 4 > MaxOnLimitsIncl`) → `DesignationError__Invalid`;
  3. `AllowDesignationsOverEntities` is false **and** any of the
     16 tiles is occupied by an entity whose proto is
     `ITerrainDesignationBlockingEntityNoEdgeProto` → `Blocked`.
+ **There is no corner-consistency or neighbour-step validation in
  the Add path at all.** The 0/±1 snapping rules are planner
  discipline + terrain physics, not command validation — a job can
  legally encode a cliff; the game will just try to work it.
+ Duplicate origins inside one batch: applied sequentially,
  **last wins**, no error. The planner must never emit them; the
  mod refuses them (§2) because silent last-wins inside a sealed
  order is exactly what the seal exists to prevent.
+ No batch-size limit exists in the code — the cap is ours (§2).

**Remove** (`IAction<RemoveDesignationsCmd>.Invoke`):

+ Proto-agnostic: `RemoveDesignation(origin)` removes whatever
  designation sits at the origin, any kind. (The remove job schema
  therefore needs no proto field; the confirm page shows kinds
  from read-back instead.)
+ `RemoveDesignation` asserts `origin == GetOrigin(origin)`.
+ **Game-side quirk, recorded as fact**: Invoke builds a
  `Lyst<Tile2i>` result list and never adds to it — the command's
  result is **always an empty array** on success. Nothing may
  depend on it; read-back decides.

**Runtime object** (`TerrainDesignation`): ctor validates only
proto non-null. `MinTargetHeight`/`MaxTargetHeight` = corner
min/max; `IsFlat` ⇔ all four corners equal; fulfilment is a
25-bit (5×5 vertex) bitmap, with separate mining/dumping bitmaps;
`IsFulfilled ⇔ bitmap == 0x1FFFFFF`.

**Manager partitions** (this is what the bridge's kind label is
built from): `TerrainMiningManager.MiningDesignations` =
proto ∈ {Mining, **Level**}; `TerrainDumpingManager
.DumpingDesignations` = proto ∈ {Dumping, **Level**}. A Levelling
cell is in *both* sets.

### 1.4 The bridge read-back surface — sufficient for flat, partial for sloped

`GET /coi/v1/designations` (bridge 0.6.5, `BridgeRoutesTerrain.cs`
`Designations()`) emits per designation: `kind` (mining-set first,
so a Levelling cell labels `"mining"`; the `proto` field
disambiguates), `proto`, `origin`, `area`, `sizeTiles`,
`minTargetHeight`, `maxTargetHeight`, `isFulfilled`,
`jobsAssigned`.

**It does not emit the four corner heights or the centre.** Full
`Data.Equals` verification (ATD-grade) is therefore impossible
against 0.6.5:

+ **Flat cells** (all corners equal — flatten pads, and any planner
  output that is per-cell flat): min == max == the corner value,
  so band equality *is* full equality. Verdict: exact.
+ **Sloped cells** (pit benches after corner smoothing): band
  equality is necessary but not sufficient — two different corner
  assignments can share a band. Verdict: partial, and must be
  labelled partial wherever reported.

Closing this is a ~6-line read-only bridge addition (§4, flagged
decision — not silently absorbed into the stages).

**Synthetic-capture discrepancy (must fix before stage-4 checks
mean anything):** `live-synthetic-v065/bridge-designations.json`
shows merged areas (`sizeTiles: 144`, 12×12 area) and proto
strings `"MiningDesignation"`. The route code emits per-cell rows
(`TerrainDesignation.SizeTiles => 4`, `Area` = 4×4) and appends
`d.ProtoId.Value`, which per §1.2 is `"MiningDesignator"`. The
synthetic disagrees with the code on both points; code reading
says the synthetic is wrong (open question 4 — only a live 0.6.5
capture settles the proto string definitively).

### 1.5 Planner data reality — pads yes, ATD pits not yet

+ **Flatten / dump pads: fully plannable today.** Inputs =
  `GET /coi/v1/terrain/region` per-tile `heightsRaw` (exact,
  Fix32→tiles conversion already the single point in
  `coi/src/terrain.rs: region_from_json`). Cells snap outward to
  the 4-grid (`GetOrigin` floor), corner targets are integer
  tiles, edge steps vs the terrain ring outside the rectangle are
  computable from the same fetch (request the rect + 1-cell
  margin), volumes come from `terrain::estimate`.
+ **ATD pit planning: its core inputs are not served.**
  `/coi/v1/deposits` serves per-material totals and connected
  regions (bounds, tiles, quantity) — **no per-column layer
  stacks**. The spike filter (per-column bedrock tops of 8
  neighbours), purity ratio and bottom-density walk
  (BEHAVIOURS.md §1 rules 2–3) all walk per-column layers and are
  **uncomputable from the served payload**. Stage 3 therefore
  specifies the pit planner as a pure function over a column
  input type, unit-tested on synthetic columns, and ships the
  pad planner end-to-end; pit *generation* waits on a bridge
  per-column layers route (the deposits scan already walks
  `TileMaterialLayers` per tile — serving a requested region's
  stacks is a route addition, not new sensing; §4 flagged
  decision). A reduced pit (region bounds → cells, uniform floor
  from surface heights) is possible but drops every ATD gate;
  it is **not** recommended, and if ever shipped its UI copy must
  say "region outline only — no purity/spike filtering".

---

## 2. Mapping onto the existing builder

### 2.1 Carries over unchanged

From the v1 blueprint flow (SPEC Addendum XVII; `BuilderService.cs`,
`BuilderJson.cs`):

+ **Folder + scan discipline**: same jobs folder
  (`%APPDATA%\Captain of Industry\QoS\jobs`), day-end pass gated
  by `enabled` (ships off; folder unscanned while off), ordinal
  filename order, one bad job never stops the pass.
+ **Single-use claim ladder**: `processed.txt` re-read every pass;
  duplicate jobId filed to `.done` untouched; CLAIM = rename
  `.claimed` + append id **before** scheduling; success →
  `.done`; refusal → append id + rename `.failed` + event;
  scheduling exception → `JobScheduleFailed` + `.failed`. No
  crash can re-run a job. (JobIds are GUIDs across both families;
  the shared record is correct as-is.)
+ **Shared refusal rungs, in the existing order**: unreadable →
  malformed → duplicate → unsupported-schema → bad-hash →
  expired → wrong-game → *family-specific rungs* (§2.2).
+ **TTL 30 minutes; gameId pin** via the already-injected
  `IGameIdProvider` (byte-identical comparison to the API value).
+ **No new injected services, no new fields on `BuilderService`.**
  Designation execution lives inside the same day-end pass, so
  the hand-written serializer pair (which writes only
  `m_calendar`) is untouched — the save surface of the mod does
  not change. This is deliberate: the 0.5.0 serializer incident
  class is not re-opened.
+ **Event log**: `QoSEventLog.Record(modId, version, kind,
  action, summary, data)` with kind `"designation"` and the same
  four actions (`JobClaimed`, `JobScheduled`, `JobRefused`,
  `JobScheduleFailed`); data adds `jobType`, `protoId`, `cells`.
  Companion /log imports QoS events generically — no companion
  change needed for events to appear.
+ **Companion ceremony**: `prepared_jobs` token store (120 s,
  single-use), confirm writes tmp-then-rename, audit + activity
  rows at confirm, `builder_jobs_dir` resolution, prepare refuses
  when the game is unreachable or gameId unconfirmable.
+ **Hash discipline**: same escaping rule (one rule, both sides),
  canonical rebuilt from the *parsed* job mod-side, shared test
  vector pinned on all legs — **including executing the C# leg
  through the probe harness** (the quality audit found the v1 C#
  canonical has never been executed; v2 does not repeat that).

### 2.2 What is new in the mod

+ **Family dispatch** in `ProcessJob`: v1 files (no `jobType`)
  take today's blueprint path byte-for-byte; `jobType` files
  branch after the shared rungs.
+ **Designation refusal rungs** (after wrong-game, in order):
  `unsupported-proto` (allowlist: `MiningDesignator`,
  `DumpingDesignator`, `LevelDesignator` — Forestry excluded in
  v1: `isTerraforming=false`, different fulfilment semantics,
  and AFD owns that niche; surface/decal protos excluded
  absolutely) → `malformed-cells` (array shape, integer fields,
  origin/corner plausibility: |corner| ≤ 1000, mirroring the v1
  anchor sanity bound — our discipline, not a game limit,
  open question 7) → `duplicate-cells` (same origin twice) →
  `over-cell-cap` → `nothing-to-designate` (empty plan).
  Misaligned origins are `malformed-cells` — the mod checks
  `x % 4 == 0 && y % 4 == 0` itself and never lets the game's
  ctor assert become the validator.
+ **Execution**: build `List<DesignationData>` via the 5-arg
  ctor (centre game-derived), then ONE
  `ScheduleInputCmd(new AddTerrainDesignationsCmd(
  new Proto.ID(protoId), ImmutableArray.Create(cells)))` —
  or ONE `RemoveDesignationsCmd(origins)` for remove jobs.
  Fire-and-forget exactly like blueprints: the mod attests
  *scheduled*, the companion's /designations diff attests
  *landed*. (The manager resolves the proto; an allowlist miss
  is unreachable, and an unknown proto would surface only as a
  command error the day-end pattern cannot observe — one more
  reason the allowlist lives mod-side.)
+ **`BuilderJson`**: add a `GetList` accessor (the parser already
  produces `List<object>` for arrays; today's accessors cover
  string/long/bool/object only) + `CanonicalDesignation(...)`.
+ **Config/manifest**: `max_cells_per_job` added to `config.json`
  (default 200, min 1, max 2000, integer); manifest +
  `QoSBuilderMod.Version` → 0.2.0; `can_remove_from_saved_game`
  stays `false`; readme updated. QoSCore untouched. Bridge
  untouched (unless §4's flagged decision is taken).

### 2.3 Job schema v2 (designation jobs only; blueprints stay v1)

One JSON object, one file `<jobId>.json`, same folder:

| Field | Meaning |
|---|---|
| `jobId` | GUID, the single-use key |
| `schema` | `2` |
| `jobType` | `"designations-add"` \| `"designations-remove"` |
| `createdUtc` / `expiresUtc` | ISO-8601 UTC; TTL 30 min at issue |
| `gameId` | pinned gameId from `/api/v1/game-info` at prepare |
| `companionUser` | confirming user (attribution) |
| `planName` | display name (mirrors v1's `blueprintName`) |
| `protoId` | **add jobs only**: one of the three allowlisted IDs |
| `cells` | **add jobs only**: `[{x, y, origin, plusX, plusXY, plusY}]` — origin tile coords + the four corner heights in integer tiles; field order inside each cell follows the **ctor order** (plusXY before plusY) so the C# rebuild is auditable against §1.1 |
| `origins` | **remove jobs only**: `[{x, y}]` |
| `limits.maxCells` | the job's own cell cap |
| `sha256` | SHA-256 of the canonical payload |

Canonical v2 field order: `jobId, schema, jobType, createdUtc,
expiresUtc, gameId, companionUser, planName, protoId (add only),
cells|origins, limits.maxCells` — same compact form and escaping
rule as v1. The centre height is **not** a field (§1.1). Kind is
the single `protoId` (§1.2): a mixed-kind confirm writes one job
file per kind.

**Cell cap: propose 200** (job cap and mod config default;
effective = min of the two, as v1). Rationale: the game imposes
no batch limit, so the cap is purely confirm discipline. ATD-
scale pits run to tens of cells; 200 cells = 3,200 tiles is a
very large single earthwork — big enough for any one pit or pad
a human would confirm at once, small enough that the confirm
page's cell list stays auditable. Config ceiling 2000 for a
deliberate large job. Remove jobs share the cap (origins count).

### 2.4 Intent lives at the confirm page (the mod executes blindly)

+ **Replace warnings**: Add replaces an existing designation of
  *any* kind at the origin, and deletes a surface designation
  there (§1.3). Prepare must diff the plan against the current
  `/designations` read-back and list every cell that replaces
  something, with its current kind/proto.
+ **Remove discipline** (ATD's only-clear-what-you-placed,
  adapted): remove jobs are prepared *from* the live read-back —
  origins picked from what is actually designated, kind/proto
  shown per origin on the confirm sheet.
+ **Blocked-cell expectation**: cells over blocking buildings
  are silently skipped by the game (§1.3). Prepare flags what it
  can (entity positions are points, not footprints — partial),
  the confirm copy states the expectation, and the read-back
  diff is the verdict.

---

## 3. Stage plan — stages 2–4

### Stage 2 — QoSBuilder 0.2.0 (mod C#)

Files: `qos/QoSBuilder/BuilderService.cs` (family dispatch,
designation ladder, execution), `BuilderJson.cs` (`GetList`,
`CanonicalDesignation`), `QoSBuilderMod.cs` (Version 0.2.0),
`manifest.json` (0.2.0), `config.json` (+`max_cells_per_job`),
`readme.txt`. QoSCore and the bridge untouched.

Verification:

+ `qos/build/build.sh` (or build-builder.sh) — 0 warnings /
  0 errors, suite builds unaffected.
+ Canonical v2 test vector executed on **both** legs: the C#
  `BuilderJson.CanonicalDesignation` run through the
  bridge/recon probe harness against the pinned expected string
  + hash (the v1 vector rides along if the harness is open —
  audit remediation synergy, not a dependency).
+ The refusal ladder is DI-bound (needs the game) — it is
  verified by code review here and by the stage-4 sandbox
  shakedown live; stated honestly, not dressed as tested.

### Stage 3 — Rust planner (`coi/src/designate.rs`, new)

Files: `coi/src/designate.rs` (+ `mod designate;` in
`coi/src/main.rs`). Nothing in serve.rs yet — the earlier
sequencing constraint (serve.rs held while a GitHub sync was in
flight) applies to stage 4, not here; this module is additive.

Contents:

+ Plan types: cell = {origin (i64,i64), corners [i64; 4] in ctor
  order}; plan = {protoId, cells} / remove plan = {origins}.
+ **Pad planner (ships end-to-end)**: `RegionGrid` + rectangle +
  target → cells snapped outward, per-vertex targets, edge-step
  warnings against the outside ring (0/±1 rule from snapping
  v2), volumes via `terrain::estimate`.
+ **Pit planner core (interface + rules, fixture-fed)**: pure
  functions over a per-column input type implementing
  BEHAVIOURS.md §1 rules 2 (spike filter), 3 (purity gates),
  4 (depth caps), 6 (percentile floor), 7 (corner smoothing) —
  ready to consume a layers route when one exists.
+ **Canonical v2 builder + sha256** (byte-identical contract
  with stage 2) and job-file JSON assembly (mirrors
  `job_canonical`'s usage in serve.rs).

Verification: `cargo test` — pad on flat terrain (uniform
corners, zero warnings); pad on a slope (edge warnings fire);
pit fixtures (spike column excluded, median floor, MaxHeightDiff
clamp); canonical vector cross-checked against an independent
Python implementation; estimator numbers cross-checked against
`terrain::estimate` on the synthetic region.

### Stage 4 — serve integration + page checks + docs

Files: `coi/src/serve.rs` (routes `GET /designate`,
`POST /designate/prepare`, `POST /designate/confirm`; nav entry
in the Game group beside Place; `PreparedJob` reused with a
per-family audit action, e.g. `PlaceDesignationJob`;
read-back status block diffing `/designations` before/after),
`live-synthetic-v065/bridge-designations.json` (**corrected**
to the route-true per-cell shape first — §1.4),
`tools/check_designate_page.sh` (new; mirrors
`check_place_page.sh`: prepare pins gameId, confirm writes
exactly one file whose sha256 re-verifies in Python, token
replay refused, bridge-down prepare refuses, before/after diff
renders; the stub already serves `/designations` + a canned
region), `SPEC.md` (Addendum XX — this contract),
`BEHAVIOURS.md` §1 status line, the earthworks skill's
procedure note, `dist/` restage (QoSBuilder-mod-0.2.0.zip
standalone + coi.exe), NIGHT-LOG.

Precondition: confirm the GitHub sync of the terrain-round
state has cleared before editing serve.rs (check the sync
record — the constraint that shaped the earlier split).

Live proof (weekend, Philip's PC — closes the round, not part
of its code completion): scratch-save shakedown — one pad add
job + one remove job, /designations diff verdict, save +
reload with 0.2.0 installed.

### Flagged decisions (parent/Philip — not absorbed silently)

1. **Bridge 0.6.6 read-back**: add `corners:[origin,plusX,
   plusXY,plusY]` + `centerRaw` to the `/designations` route
   (~6 lines, read-only, `BridgeRoutesTerrain.cs`). With it,
   stage 4 verifies Data-equality for sloped cells too;
   without it, v1 verdicts are exact for flat cells and
   band-level (labelled partial) for sloped. Recommendation:
   do it, in parallel with stage 2.
2. **Bridge per-column layers route** (e.g.
   `/coi/v1/deposits/region?x0&y0&x1&y1` serving layer stacks):
   the gate for pit *generation* (§1.5). Needed before the
   stage-3 pit core can run on live data; not needed for pads.

---

## 4. Open questions — what the assemblies did NOT answer

1. **Assert behaviour in the shipped build.** The DesignationData
   ctors and `RemoveDesignation` call `Assert.That(...)`; whether
   Mafi asserts throw, log, or are compiled out in the release
   build the mod runs against cannot be determined from the DLL
   alone. The design already makes them unreachable (mod-side
   alignment checks, planner emits aligned origins only) — but
   "unreachable" rests on our discipline, not on observed
   runtime behaviour.
2. **`AllowDesignationsOverEntities`**: its default value and
   who calls `SetAllowDesignationsOverEntities` were not traced.
   If anything flips it, the blocked-cell refusal (§1.3) silently
   disappears.
3. **Occupancy predicate semantics.** `IsDesignationAllowed`
   passes `(IStaticEntity p) => p.Prototype is
   ITerrainDesignationBlockingEntityNoEdgeProto` as a filter to
   `TryGetAnyOccupyingEntityAt`; that overload's exact semantics
   were not decompiled, so "blocked only by no-edge protos" is
   the natural reading, not a proven one. Affects only how
   precisely prepare can predict skipped cells — never the
   read-back verdict.
4. **The live proto string.** Route code appends
   `ProtoId.Value` ⇒ `"MiningDesignator"`; the v065 synthetic
   says `"MiningDesignation"`. Only a live bridge 0.6.5 capture
   settles which the game actually serves.
5. **Partition timing.** The mining/dumping managers maintain
   their sets from designation events; whether a just-processed
   command is reflected in the very next `/designations` fetch
   (vs a tick later) is live behaviour, unobserved.
6. **`MaxAssignedEntities` reservation semantics** (Level 1 vs
   Dumping 2, `CanOverflowReservations = MaxAssignedEntities
   > 1`): what a second assigned entity changes in fulfilment
   pacing is gameplay behaviour — not traced, not needed by the
   executor, recorded so nobody assumes it was.
7. **Corner-height range.** The Add path accepts any `int`
   height; no game-side bound was found. The mod's ±1000 sanity
   bound is our discipline. Whether extreme targets upset the
   fulfilment bitmaps or terrain physics is untested.
8. **Remove result emptiness** is source-evident (§1.3) but
   never observed live; nothing in the design depends on it
   either way.
9. **In-mod verification**: ATD verifies in-process
   (`GetDesignationAt` + `Data.Equals`) after `IsProcessed`.
   Whether QoSBuilder should add a next-pass in-process check
   on top of the companion's snapshot diff is a stage-2 design
   choice (current plan: companion diff only, mirroring the
   blueprint honesty rule) — deferred, not decided here.

---

## 5. Stage 3 — as built (2026-11-06)

`coi/src/designate.rs` exists, standalone as specified: std +
serde_json + sha2 only, no crate-module imports (deliberate
local duplication, marked in the file: `job_escape`,
raw→tiles conversion, the estimator sums — stage 4 may unify,
values must never diverge). NOT wired into the crate — no
`mod designate;` yet; stage 4 does that after the remediation
round clears main.rs/serve.rs. No dist restage.

**Canonical v2 — acceptance gate PASSED.** `canonical_designation`
is a byte-level mirror of `BuilderJson.CanonicalDesignation`;
both published vectors reproduce EXACTLY from the note's test
inputs (add `e4166dd3…58ed57f7`, remove `78fb273c…7e70cfa4`),
through the low-level mirror and through the typed builders
(`build_add_job` / `build_remove_job`, which also assemble the
job file as canonical + trailing `sha256` field, the v1 serve.rs
assembly). No vector was adjusted.

**Pad planner — complete.** `plan_pad(grid, rect, target,
proto_id, max_cells)`: snaps the rectangle outward to whole
cells, emits level cells, volumes over the snapped footprint
(terrain::estimate arithmetic), and boundary-vertex edge-step
warnings (|target − outside terrain| > 1, checked against every
outside tile touching the vertex; tiles outside the grid never
warn — fetch rect + margin). Typed refusals in order:
EmptyRectangle → UnsupportedProto → ImplausibleTarget (±1000)
→ OutsideGrid (the SNAPPED footprint must be fully gridded) →
OverCellCap (caller-passed cap).

**Pit core — pure functions, fixture-fed.** `Column` /
`ColumnGrid` input types; `spike_cutoffs` (rule 2, constants
4 / 8, cutoff = deepestTargetBottom + raise); `target_intervals`

+ `visible_target_thickness` (clamp(top − cutoff, 0, thickness));
`PURITY_GATES` L0–L4 + `evaluate_origin` (ore height, purity,
per-column dig depths combined deepest-first) and `dig_depth`
(rule 3's bottom-density walk, first failure stops);
`origin_surface_min` + `apply_depth_caps` (rule 4, in order);
`components` + `filter_components` (rule 3's size filter,
largest always kept) + `component_floor` / `flatten_depths`
(rule 6: percentile index n×(10−strength)/10, <4 tiles skipped,
lower-only = elementwise min); `build_corners` +
`cells_from_corners` (rule 7: initial heights deepest or mean,
smoothing lowers the shallower corner only, cap 1,000,
converged flag; non-live hole vertices never smooth).

**Tests: 13, all passing, zero warnings**, run STANDALONE in a
scratch cargo project (`/tmp/designate-stage3`, lib.rs pulls the
file in by `#[path]`, same serde_json/sha2 versions, `cargo test
--offline`): both canonical vectors; pad flat / snap-outward /
slope (warnings east only, cut 1920 tiles³ exact) / outside-grid
/ over-cap; spike column (cutoff 9, visible 3 of 4); density
walk stop-at-first-failure; origin evaluation vs L1/L4 gates +
caps; two components each taking their own median floor;
component-size filter; corner clamp (14 → 11 beside 10). The
corner test caught a real bug on first run — `build_corners`
undercounted the vertex grid by one row/column (bounds are
origins, vertices extend one cell past); fixed in the code.

**Interpretation points the distilled text did not pin**
(recorded, not hidden):

+ Median of the 8 neighbour bedrock tops = mean of the two
  middle values (standard even-count convention).
+ The bottom-density walk runs on intervals with visible
  (post-spike) thickness > 0, density = visible ÷ (waste gap +
  interval thickness).
+ Per-origin depth = the deepest justified column dig within
  the origin; rule 6 flattening disciplines it afterwards.
+ `CornerInit::Mean` (purity > 0) is the true rounded mean of
  contributing tiles; ATD's running integer average is
  order-dependent and was not reproduced literally.

**Stage 4 must wire (precise list):**

1. `mod designate;` in `coi/src/main.rs`.
2. Fetch `/coi/v1/terrain/region` (rect + 1-cell margin) and
   build `designate::HeightGrid` via `from_region_json` (or
   adapt from `terrain::RegionGrid` — pick one, delete the
   other conversion path).
3. serve.rs routes `GET /designate`, `POST /designate/prepare`,
   `POST /designate/confirm` (nav beside Place, Game group);
   reuse `PreparedJob` (120 s single-use token) with a
   per-family audit action; prepare refuses when the game is
   unreachable or gameId unconfirmable; prepare diffs the plan
   against `/designations` for replace warnings (note §2.4)
   and shows cells + volumes + warnings + exact job JSON on
   the confirm sheet; confirm builds the file with
   `build_add_job` / `build_remove_job` and writes it
   tmp-then-rename into `builder_jobs_dir` (the only write
   path) + audit/activity rows; read-back status block diffing
   `/designations` before/after (corners exact with bridge
   0.6.6; band-level labelled partial without it).
4. Remove-job prepare from the live `/designations` read-back
   only (origins picked from what exists, kind shown per
   origin).
5. Correct `live-synthetic-v065/bridge-designations.json` to
   the route-true per-cell shape (note §1.4) BEFORE writing
   `tools/check_designate_page.sh` (mirror check_place_page.sh:
   prepare pins gameId, confirm writes exactly one file whose
   sha256 re-verifies, token replay refused, bridge-down
   prepare refused, diff renders).
6. Pit *generation* stays fixture-fed until the bridge
   per-column layers route (flagged decision 2) exists; then a
   fetch builds a `designate::ColumnGrid` and the pit pipeline
   (spike → gates → caps → components → floor → corners →
   cells) runs on live data.
7. SPEC Addendum XX, BEHAVIOURS.md §1 status line, earthworks
   skill procedure note, dist restage (coi.exe), and the live
   scratch-save shakedown (pad add + remove, read-back verdict,
   save + reload) that closes the round.

---

## 6. Stage 4 — as built (2026-11-06); round complete, live proof pending

All four stages are built. Stage 4 wired `designate.rs` into
the crate (`mod designate;` in main.rs) and added the serve
integration exactly along §5's wiring list:

+ **Routes** (`coi/src/serve.rs`, nav: Game group beside
  Place): `GET /designate` (pad form + live designation list
  with 0.6.6 corners + latest-job verdict panel),
  `POST /designate/prepare`, `POST /designate/confirm`.
+ **Prepare (pad)**: gameId pin with /place's 502 refusals;
  region fetched as rect + 1-cell margin; `plan_pad` over
  `HeightGrid::from_region_json` (the designate-side parser —
  §5 item 2's "pick one": stage 4 kept BOTH conversions
  because `terrain::RegionGrid` and `designate::HeightGrid`
  have different consumers; the raw→tiles constant is shared
  by value and the planner's volumes were cross-checked
  against `terrain::estimate` arithmetic in stage 3).
  The confirm sheet carries cells, volumes, every edge-step
  warning, every replaced designation, and the blocked-cell
  expectation (§2.4).
+ **Prepare (remove)**: origins only as `sel_<i>` checkbox
  values rendered from the live read-back; a selection that
  no longer matches the read-back is refused ("Stale
  selection") — no free-typed origin path exists.
+ **Confirm**: `PreparedJob` reused (120 s single-use token;
  per-family audit action `PlaceDesignationJob`, category
  `designation`), file built by `build_add_job` /
  `build_remove_job`, tmp-then-rename into
  `builder_jobs_dir`, audit/activity rows under the
  remediation round's fail-loud pattern.
+ **Verdict**: `designation_verdict_panel` diffs the job
  against the CURRENT `/designations` (via the new
  `existing_from_json` / `diff_add` / `diff_remove` in
  designate.rs) on the outcome page and on every /designate
  visit (latest-job panel reconstructed from the activity
  payload). Exact corner equality on bridge 0.6.6; band-only
  matches are counted separately and labelled partial.
  Statuses: LANDED / PARTIALLY LANDED / SCHEDULED — verdict
  pending. The write alone never claims placement.
+ **Synthetic capture corrected first**, as required:
  `live-synthetic-v065/bridge-designations.json` is now the
  0.6.6 route shape (per-cell rows, `"MiningDesignator"`,
  corners `[6, 9, 9, 6]` / centerRaw 491520); SYNTHETIC.txt
  records the correction.
+ **Pit generation stays fixture-fed** (§5 item 6): the
  /designate page says so in its own copy; no reduced pit is
  offered. The pit core's production dead-code is allowed at
  module level with the reason recorded in the file header.

**Verification**: cargo build 0 warnings; cargo test 41/41
(25 pre-existing + stage 3's 13 + 3 new read-back/verdict
tests); `tools/check_designate_page.sh` PASS (gameId pin,
6-cell pad with edge warnings, exactly one add file whose
sha256 re-verifies under an independent Python v2 canonical,
replay refused, latest-job panel, remove flow + file
verified, /log carries PlaceDesignationJob, bridge-down
prepare 502, /designate 200-with-notice);
`check_place_page.sh`, `check_log_page.sh`,
`check_audit_fail_loud.sh` all still PASS.

**Packaging**: dist/coi.exe = 6,345,284 B, sha256
33f063ce…06dd1d (SPEC Addendum XX); weekend bundle refreshed
(bridge 0.6.6, QoSBuilder 0.2.0, new exe; BUNDLE-README rows
re-measured from the files).

**Live proof — the weekend shakedown (closes the round)**,
on a SCRATCH save, in order:

1. Install the weekend bundle; enable QoSBuilder
   (`enabled` on) with the rest of the suite; `web_api_start`.
2. On the scratch save, /designate: prepare a small pad
   (2–4 cells) over empty terrain, confirm, note the job id.
3. Advance to the next in-game day end. EXPECT: /log shows
   the companion's PlaceDesignationJob, then QoS
   JobClaimed/JobScheduled (kind "designation"); the
   /designate verdict panel flips to LANDED with exact
   counts; in-game, the cells show the chosen kind at the
   chosen corners.
4. Prepare a removal of exactly those origins from the
   fresh read-back, confirm, advance a day. EXPECT: verdict
   LANDED — all origins clear, in-game designations gone.
5. Save the game AND reload it (QoSBuilder 0.2.0's
   serializer surface is unchanged from 0.1.0, but the proof
   rides with the round like every suite change).
6. One replace case: re-add a pad over one surviving
   designation of a different kind; EXPECT the confirm
   sheet's replace warning named it, and the read-back shows
   the new kind at that origin after the day turns.
Any step failing = the round reopens with the observed
state recorded here; no step may be "verified" by the
command result (Add's result cannot distinguish placed from
refused cells — §1.3).
