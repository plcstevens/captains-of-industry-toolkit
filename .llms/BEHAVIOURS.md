# BEHAVIOURS.md — Distilled Teacher-Mod Behaviours for the Muse Player

Philip's framing (2026-11-06): avoid mods that crash; the good mods are
TEACHERS, not dependencies — "we just want to figure out how to train
YOU the AI to behave similarly."

This file distils decision behaviour from surveyed mods into
agent-executable policies that run through OUR stack only: companion +
CoiBridge (read-only sensing) + QoS suite + QoSBuilder job files,
zero-Harmony, shipped commands only. Nothing here installs, requires,
or reproduces a teacher mod at runtime.

**Provenance classes used throughout:**

- **CODE** — read from the teacher's published source, file:symbol cited.
- **DOCS** — stated in the teacher's published docs/readme.
- **GAME-CODE** — verified from the 0.8.8 game assemblies (our recon).
- **INFERENCE** — our reading beyond the source; labelled, never a number.
- **NOT PUBLISHED** — checked and absent from source/docs; do not invent.

Companion document: `MOD-SURVEY.md` (ecosystem + verdicts).
Execution discipline: `local-coi-playing` (§12 prepare→confirm→audit),
`local-coi-strategy` (triage / XOR rules), `local-coi-earthworks` v2
(geology-aware snapping procedure).

---

## RECON VERDICT — Can our stack create/modify terrain designations?

**Verdict: EXECUTABLE-BY-AGENT via the QoS-side scheduler route;
NOT reachable via the web §12 channel today. QuickFulfill excluded.**

Evidence (GAME-CODE, recon round 14 — see bridge/MODDING-NOTES.md):

1. `live/commands.json` (240 web commands) contains **zero**
   designation/terrain commands. Expected, not decisive — see 4.
2. The 0.8.8 assemblies DO contain shipped InputCommands in
   `Mafi.Core.Terrain.Designation`:
   - `AddTerrainDesignationsCmd(Proto.ID protoId,
     ImmutableArray<DesignationData> data)` —
     `InputCommand<ImmutableArray<Tile2i>>`, result = origins placed.
   - `RemoveDesignationsCmd(ImmutableArray<Tile2i> origins)` —
     same result type.
   - `QuickFulfillDesignationsCmd(RectangleTerrainArea2i area,
     Option<ProductProto> product)` — **EXCLUDED BY POSTURE**
     (instant/Unity fulfil bypasses the economy; never use).
3. `TerrainDesignationsManager` implements `ICommandProcessor` +
   `IAction` for all three. Add's Invoke: proto lookup (error result
   if unknown) → per-item `AddOrReplaceDesignation(proto, data)` →
   result = all origins. **Modify = Add at the same origin (replace
   semantics). Delete = Remove by origins.** No separate modify
   command exists or is needed.
4. `DesignationData` (readonly struct): `OriginTile` (Tile2i) + four
   corner `HeightTilesI` targets (Origin, PlusX, PlusXY, PlusY) +
   `CenterTargetHeight` (HeightTilesF). [LoadCtor] order: originTile,
   originTarget, plusXTarget, plusXYTarget, plusYTarget, centerTarget.
   Protos (GAME-CODE, `TerrainDesignationsData.RegisterData`):
   `IdsCore.TerrainDesignators.MiningDesignator /
   DumpingDesignator / LevelDesignator / ForestryDesignator`
   (bridge readback labels: "MiningDesignation"/"DumpingDesignation").
   Mining: preferInitialBelowTerrain=false, no-tower warning on;
   Dumping: preferInitialBelowTerrain=true; Leveling: both fulfilment
   fns; Forestry: isTerraforming=false.
5. Why the web channel cannot carry them: the `ImmutableArray<…>`
   ctor params hit the same JsonObjectBuilder collection block proven
   in recon 10 for `BatchCreateStaticEntitiesCmd`. The QoS scheduler
   route does not need JSON-buildability:
   `IInputScheduler.ScheduleInputCmd<T>(T) -> T` is the proven
   QoS/QoSBuilder channel.
6. **In-the-wild precedent (CODE):** Auto Forestry Designations
   commits its whole plan as ONE
   `IInputScheduler.ScheduleInputCmd(new AddTerrainDesignationsCmd(
   s_forestryProto.Id, designations.ToImmutableArray()))` and waits
   for `command.IsProcessed` (afd `src/AFD.Scan.cs:
   CommitDesignationsCoroutine`). ATD does the same for its mining
   body (atd `src/ATD.Scan.cs:CreateDesignationsCoroutine`).
   Shipping mods execute designations through exactly our channel.

**Channel decision:** a QoSBuilder job-type extension (or sibling QoS
executor) carrying Add/Remove designation batches — single-use,
expiring, gameId-pinned, sha256-sealed job files, written by the
companion only at a §12-style confirm step — is the sanctioned route.
**BUILT 2026-11-06** (designation stages 1–4; SPEC Addendum XX):
QoSBuilder 0.2.0 executes schema-2 designation jobs, and the
companion's /designate page plans from live bridge data with a
read-back verdict. **Pads are executable end-to-end** (pending
the live scratch-save shakedown that closes the round);
**pit generation below stays ADVISORY / plan-producing** — the
planner core in `coi/src/designate.rs` implements the rules
over per-column inputs, but the bridge serves no per-column
layer stacks yet, so the full ATD set is computed only once
that route exists (the agent computes origins + corner heights
and Philip — or ATD itself, if installed — places them until
then). Per-action-class authorisation still applies.

---

## 1. ATD Pit Planning — geology-aware mine designation generation

**Teacher:** Auto Terrain Designations (ATD), Kayser1444 — **MIT**.
Source: `github.com/Kayser1444/CoI_AutoTerrainDesignations`.
Pipeline (CODE, `src/Mining/MiningPlanner.cs:Run`;
`docs/dev/done/ore-spike-filter.md`): spike-filtered ore
interpretation → ore-quality gates → depth caps → isolated-component
filter → connect/hull → bottom flattening → direct safety removal →
corner build + slope smoothing → exterior safety iterations →
single-batch placement → access/ramp phase.

### INPUTS (our surfaces)

- `/deposits` (bridge v0.6.5): per-tile material layer stacks +
  remaining quantities — supplies ore intervals, bedrock tops,
  thickness per column (the planner's captured columns).
- `/terrain` + `/terrain/region`: surface heights per tile.
- `/designations`: existing designations (no-op fingerprint,
  obsolete-origin removal, access providers).
- Machines/census: tower position + area, assigned excavator class
  (access mode), building footprints (safety exclusion).
- Products: target product selection (a forced product, or AUTO =
  all loose products with `CanBeOnTerrain || TerrainMaterial != null`,
  excluding Phantom and any product whose ID contains "rock" —
  CODE, `src/ATD.Scan.cs:GetCandidateScanProducts, IsRockProduct`).

### DECISION RULES

1. **Grid primitive.** One designation = one 4×4-tile cell; origins
   step by 4; a cell's 16 columns are enumerated rows-first
   (CODE, `MiningPlanner.Cells`; `MiningPlanner.Geometry.cs`).
2. **Spike filter FIRST, before any quality/connect/flatten step**
   (CODE, `src/Mining/MiningSpikeFilter.cs`; per-world setting
   `FilterOreSpikes`, default **true**). Per single terrain column —
   hardcoded parameters, not user settings:
   `VanillaCorrection = (AllowedBedrockResidual = 4,
   MinimumBedrockNeighbors = 8)`.
   Rule: the centre column must contain both a target-product layer
   and a bedrock layer; collect bedrock tops of all 8 neighbours —
   **if fewer than 8 have bedrock, skip** (i.e. all 8 required);
   `median` = median of the 8 neighbour bedrock tops;
   `raise = median − centerBedrockTop − 4`; if `raise > 0.0001`,
   record cutoff = deepestTargetBottom + raise. Effect: target-layer
   thickness below the cutoff is **invisible to the planner**
   (`VisibleTargetThickness`: clamp(Top − cutoff, 0, Thickness));
   terrain, captured layers and existing designations are NOT
   modified. **Critical caveat (CODE): the shipped detector has NO
   thickness criterion** — a column is corrected solely because its
   bedrock top sits >4 levels below the median of all 8 neighbours,
   regardless of ore thickness. The "ultra-thin spike" phrasing is
   the player-facing description of the vanilla defect, not the
   predicate. Any thickness-based spike rule is **NOT PUBLISHED**;
   the prototype detectors in `docs/test/ore-spike-filter-prototype-01.md`
   are marked superseded and must not be used. Measured effect
   (DOCS, `docs/player/mining-designations.md`): planned bedrock
   excavation reduced "by up to 99% locally and 36% across entire
   designation plans"; strongest capture ≈4.79M Rock avoided for
   ≈0.002% target-ore change.
3. **Ore-quality gates** — indexed by `PurityLevel` 0–4, default
   **0 = Off** (CODE, `src/ATD.Settings.cs:Default*ByLevel`;
   revision 5 arrays):

   | Level | MinOreHeight | MinBottomDensity | MinOrePurity | MinComponentSize |
   |---|---|---|---|---|
   | 0 Off | 0 | 0 | 0 | 0 |
   | 1 Low | 0.25 | 0.05 | 0.05 | 2 |
   | 2 Med | 0.75 | 0.20 | 0.20 | 6 |
   | 3 High | 2.0 | 0.50 | 0.50 | 20 |
   | 4 Max | 3.0 | 0.75 | 0.75 | 40 |

   A gate value of 0 disables that gate. Definitions differ — do
   not conflate:
   - **Purity ratio** (per 4×4 origin, 16 columns): walk layers
     top-down, **stop at first bedrock**; total = all non-bedrock
     thickness; ore = spike-filtered target thickness.
     Purity = ore ÷ total (bedrock excluded both sides). Reject
     origin if ratio < MinPurity.
   - **Min ore height** (per origin): sum of spike-filtered target
     interval heights; reject if sum < MinOreHeight. (The constant
     `MIN_ORE_HEIGHT_THRESHOLD = 1.0f` in ATD.Settings.cs is dead —
     not used by the current planner path. Do not port it.)
   - **Bottom density** (`TryGetPurityAdjustedDepth`, CODE
     `MiningPlanner.Depth.cs`): intervals sorted shallow→deep;
     shallowest always qualifies (density 1); for each later
     interval, localDensity = thickness ÷ (this.bottom −
     previous.bottom) — the waste gap plus the seam; dig extends
     while localDensity ≥ MinBottomDensity; **first failure stops
     the walk** — nothing deeper is included even if a deeper seam
     would pass alone. If MinBottomDensity = 0, depth = deepest
     target bottom instead.
   - **Min component size** (after per-origin depths,
     `FilterIsolatedDesignations`): 4-connected components;
     **the largest component is always kept**; other components
     removed if count < MinComponentSize, else removed only if
     EVERY tile's target amount < MinOreHeight (one tile at/above
     saves the component). Runs again after safety removals.
4. **Depth caps, in order** (CODE, `MiningPlanner.cs:Run`):
   per-origin surface = MIN surface height over its 16 columns;
   depth from rule 3; then relative cap — if `MaxLayers > 0`
   (setting `MaxLayersToExcavate`, default **30**, clamp ≥0,
   0 = no limit): `depth = max(depth, surface − MaxLayers)`;
   then absolute floor — if `MinElevation` set (setting
   `MaxDepthToDigTo`, default **null = no limit**):
   `depth = max(depth, MinElevation)`.
5. **Connect / contour** (`FillRectilinearHull`, CODE
   `MiningPlanner.Geometry.cs`). Skipped entirely when corridor
   clearance = 0. Otherwise: recompute components, largest first;
   connect each secondary component to the growing set by closest
   tile pair, drawn as an **L-shaped path, X-align first then
   Y-align**, single-tile wide; corridor tiles take `NearestDepth`
   (nearest existing tile by squared distance). Then fill interior
   holes (bounding-box flood fill from boundary; unreached interior
   cells are enclosed → added at NearestDepth). Clearance ≥2 adds:
   junction squaring (2×2 anchor with exactly 1 missing tile → add
   it) and `EnforceMinimumClearanceTwo` — every tile traversable
   by a 2×2 brush of designation cells (= 8×8 terrain tiles), in
   three phases (coverage → adjacency until stable → Dijkstra
   connectivity over anchors, cost = missing-tile count); widened
   edge tiles take the shallowest neighbour depth, others
   NearestDepth; hole-fill runs again afterwards.
   **Corridor clearance by vehicle class** (CODE,
   `ATD.State.cs:CorridorClearanceForMode`): derived from the
   `VehicleClearance` mode setting (default `Auto`): Off→0,
   Auto/T1/T2→**1** (single-wide spine, small/medium vehicles),
   T3 or legacy widths→**2** (mega vehicles).
   **Discrepancy flagged, not papered over:** three "defaults"
   exist — effective derived value under Auto is **1**; a
   `SetMinCorridorClearance(2)` call in ResetGlobalDefaults is a
   no-op (superseded by VehicleClearance); the JSON loader's
   generated-default sentinel is 2. Port **1 (Auto)** as the
   effective default. INFERENCE: the 2s are pre-VehicleClearance
   leftovers.
6. **Bottom flattening — floor = percentile, default median**
   (CODE, `FlattenDesignationBottom`). Only if `FlattenBottom`
   (default true). Per connected component; **components with
   <4 tiles are skipped**. Sort component depths ascending
   (deepest first); `targetIndex = clamp(Count × (10 − strength)
   / 10, 0, Count−1)` with strength clamped 1–10, **default 5**;
   strength 10 → deepest tile, **5 → median**, 1 → 90th-percentile
   shallow. Blend: purity level ≤0 = **lower-only** (tiles
   shallower than target are pulled down to it; nothing is ever
   raised); purity >0 = **full levelling** (every tile set to
   target). No weighted blending exists — tiles snap exactly to
   target; any smoother blend is **NOT PUBLISHED**.
   Survey cross-check: the MOD-SURVEY figure "strength default
   5/10, floor = median" is **verified correct** in source.
7. **Corner construction + slope smoothing** (CODE,
   `BuildAndSmoothCornerHeights`): initial corner height =
   deepest contributing tile depth (purity ≤0) or running integer
   average (purity >0). Then iterate over all cardinally adjacent
   corner pairs (4 tiles apart) until no pair differs by more than
   **`MaxHeightDiff`** (setting, default **1**, clamp **1–3**):
   the shallower corner is **lowered** to other + MaxHeightDiff —
   smoothing only ever deepens, never raises. Iteration cap 1,000.
8. **Landslide / protected-area safety — iterative, fail-closed**
   (CODE, `src/Mining/MiningSafety.cs`). Loop: direct removals →
   build corners → trace exterior → remove rejected → re-filter
   isolated components → rebuild corners → repeat with **no
   arbitrary iteration limit** until a full pass rejects nothing
   (each unsuccessful pass removes ≥1 origin, so it converges).
   All removed ⇒ empty plan, outcome `SafetyRemovedAll`.
   - **Stage A, direct protection:** reject an origin if any
     terrain tile in its 4×4 footprint expanded by `RayBuffer`
     on all sides is ocean (if AvoidOcean, default true), or any
     building-occupied tile lies within footprint expanded by
     `RayBuffer + BuildingBuffer`, where **BuildingBuffer = 3
     tiles, a constant, not a user setting** (CODE,
     `ATD.State.cs:BuildingSafetyBufferTiles = 3`), if
     AvoidBuildings (default true).
   - **Stage B, exterior rays** (`TraceExterior`): 8 directions
     (4 cardinal + 4 diagonal); a direction is skipped if the
     neighbouring origin in that direction is in-plan (interior
     face); a diagonal is additionally skipped unless BOTH its
     cardinal neighbours are out-of-plan (true convex corner
     only). Cardinal faces sample 5 points; diagonal samples the
     single outer corner; planned height = bilinear interpolation
     of the 4 corner heights. Ray marches 1 tile at a time **to
     the map edge** (no distance cap in this pass), height
     advancing by the per-column material slope for cuts
     (fallback `FallbackMiningSlope`) and `DumpingSlope` for
     fills. Slope = `InterpolateRaySlopeBounds`:
     conservative = minCollapse × 2/3, aggressive = maxCollapse
     × 2/3, interpolated by the conservatism factor; the 2/3 is
     the vanilla approximate-slope conversion, preserved
     deliberately. Diagonal steps use the SAME height increment
     as cardinal (no √2 scaling — vanilla compares the same
     bounds; √2 would be less safe). Hazard: a building at any
     ray tile (with BuildingBuffer expansion), or ocean on a
     CUT ray while ray height < 1 (fill rays never trigger the
     ocean condition). Passing requires surface at/beyond the
     projected height (and height ≥1 for cuts) **plus a tail
     check of `RayBuffer` further tiles** beyond the passing
     tile — ocean/building there still rejects the origin.
   - **Safety policy presets** (CODE, `ATD.Mod.cs:
     GetSafetyPolicyParameters`) — (slope conservatism, ray end
     buffer): **Min (0.8, 0), Low (0.85, 1), Med (0.9, 2),
     High (1.0, 3), Max (1.1, 4)**. Default preset **Med**:
     conservatism 0.9 (expert setting, clamp 0–1.5), RayBuffer
     **2 tiles** (clamp 0–16). Material slope NUMBERS are
     **NOT PUBLISHED** — derived per material from game protos
     at capture; fallback 0.5 only when no protos resolve. Our
     port uses the GAME-CODE collapse bands in
     guides-earthworks.md instead — same source of truth ATD
     derives from.
9. **Ramps / access.** Two disjoint generators, chosen by mode:
   - **Legacy straight ramp (exact rule, CODE
     `TryPlaceRampSliced`): slope = exactly 1 terrain level per
     designation row (4 tiles)**; each step every boundary corner
     moves +1/−1/0 toward its reference height, never more;
     adjacent boundary corners clamped to differ by ≤1;
     `RelaxRampVertexHeights` enforces |Δ| ≤1 between every pair
     of vertices sharing an edge (≤512 iterations). Portable
     rule: **1 level per 4-tile cell along the ramp, ≤1 level
     between laterally adjacent corners** — hardcoded for ramps
     regardless of MaxHeightDiff.
   - **Routed accessway (default modes Auto/T1/T2/T3):** A* over
     a captured snapshot (worker thread default; limits CODE:
     max visited nodes 250,000 clamp 1k–2M; timeout 60 s clamp
     5–600; snapshot ceiling 1,024 MiB; margin 48 tiles). Width
     and slope are **NOT PUBLISHED as numbers** — width =
     max(1, vehicle clearance from vanilla
     `VehiclePathFindingParams` for the tower's excavator class),
     collapsing to 1 or 2 lanes; steepness governed by vanilla
     pathability (`VehicleMaxSteepnessDelta`, snapshot default
     0.5) + material slopes + useful-height envelope (V1 ±1.0,
     V2 ±1.5). Port the DECISION (route with the assigned
     excavator class's own pathability; T3 = two-lane), not an
     invented constant.
   - **When:** clusters evaluated for reachability; if every
     cluster is already tower-reachable, ramp generation is
     skipped entirely (outcome `Crested`). Otherwise clusters in
     `NeedsAccessway` processed closest-to-tower first,
     re-checking connectivity after each placement. If the
     in-area search exhausts and `AllowRampsOutsideTowerAreas`
     (default true), retry once with the domain expanded by
     **16 tiles**. Outcomes: Crested / Truncated / NotAccessible
     / Failed; failure raises a tower warning.
10. **Execution shape (CODE, `ATD.Scan.cs`):** requests queued per
    tower in click order; re-click replaces the pending request.
    A policy fingerprint (bounds, ore, MaxHeightDiff, MaxLayers,
    MinElevation, clearance, purity level + arrays, flattening,
    spike filter, access fingerprint) gates a no-op if unchanged.
    Obsolete self-owned origins are removed first, then **the
    entire mining body is submitted as ONE
    `AddTerrainDesignationsCmd`** via `IInputScheduler`; the
    coroutine waits for IsProcessed, checks HasError, then
    **verifies every origin by reading it back**
    (`GetDesignationAt` + proto + `Data.Equals`) before
    registering it as generated; partial placement aborts the
    flow. Access/ramp tiles are placed separately, one at a
    time via `AddOrReplaceDesignation`. `BATCH_SIZE = 30`
    (clamp 1–200) governs per-frame coroutine yields in OTHER
    loops — **the mining batch is NOT chunked by it**.
    Ownership discipline: Clear removes only self-recognised
    designations; a player edit to a generated tile revokes
    ownership. **Port both: single-batch commit + read-back
    verification + only-clear-what-you-placed.**

### ACTIONS

- **ADVISORY / plan-producing today:** the agent runs rules 1–9
  over bridge `/deposits` + `/terrain` data and emits the full
  designation set (origins, corner heights, ramp rows) as a plan
  artefact for Philip's review.
- **Future channel (verdict above):** QoSBuilder-style designation
  job → ONE `AddTerrainDesignationsCmd` batch; removals via
  `RemoveDesignationsCmd`; verification by `/designations`
  read-back (proto + data equality per origin), census-independent.
- Never `QuickFulfillDesignationsCmd`. Never per-tile drip
  placement of a mine body (batch or nothing — partial bodies
  break the safety loop's assumptions).

### LIMITS — deliberately NOT reproduced

- The entire Harmony/UI layer: inspector panels, Mod Settings
  tabs, `atd_*` console commands, Corner Designations tool mode,
  hazard overlay, notifications, settings JSON migration
  machinery (CODE list, ATD worker §9). Decision behaviour only.
- Ore Sorting Plant export UI + its Harmony logistics patch set;
  Default Mine (unassigned-excavator) patches; vehicle/idle and
  pre-allocation patches.
- The worker-thread/A* access search implementation itself —
  we port its decision rules and limits, not its engine.
- Superseded prototype spike detectors (see rule 2).
- Per-designation ordering inside the batch: **none exists in
  source — NOT PUBLISHED because there is none.** Batch iteration
  order (dictionary order over maxOreDepths) is treated as
  unspecified. INFERENCE (labelled in source report).
- ATD's per-tower excavator priority and dumping priority
  (default Passive; active range 1–15, farmland fill temporarily
  14) are vanilla-preference settings, not geometry — noted
  here, governed by Behaviour 3's priority discipline, not
  duplicated as an earthworks rule.

---

## 2. Prefer-Local-Sink — dump in-zone before hauling to storage

**Teacher:** Smart Mining Dump, CesarM-Git — **Unlicense**.
Clone examined: v1.0.3, game 0.8.7 (CODE,
`SmartMiningDumpMod.cs:DumpFirstWrapper.TryGetJobFor /
TryEnqueueDumpJob`).

### INPUTS

- Machines/census: tower, its trucks (cargo, enabled), assigned
  excavators.
- `/designations`: tower's managed dumping designations
  (count > 0 gate), tower area + assigned input towers.
- Tower dumpable-products set; truck cargo (first/phantom product
  only — see rule 3).

### DECISION RULES

1. **Per-tower opt-in, default OFF.** Policy name in the teacher:
   "Prefer Dumping over Storage". No tower gets the behaviour
   unless explicitly enabled for that tower.
2. **Decision order, per truck job request:** (1) destroyed-tower
   self-heal (drop stale state); (2) gates — toggle on, tower
   enabled, `ManagedDumpingDesignations.Count > 0`, truck cargo
   non-empty, truck enabled; any gate fails → vanilla untouched;
   (3) cargo filter (rule 3); (4) dump attempt (rule 4);
   (5) any failure → **unmodified vanilla fallback**.
3. **Cargo filter:** only `Cargo.FirstOrPhantom` is inspected;
   the product must be in the tower's `DumpableProducts`.
   (Mixed-load handling beyond first-product inference is not
   implemented in the teacher — INFERENCE in the worker report;
   our port inherits the same limitation and labels it.)
4. **Dump attempt:** call the **vanilla**
   `DumpingJob.Factory.TryCreateAndEnqueueJob` scoped to
   [this tower] + its `AssignedInputTowers` — wider than the
   teacher's README ("own zone") claims; code wins. Success sets
   `DumpingOfAllCargoPending` and short-circuits vanilla
   entirely. Why the mod exists: vanilla ranks
   AssignedInputStorages ABOVE the local-dump path, so without
   the inversion, local levelling sinks starve while trucks haul
   to storage. Sink selection inside the factory is vanilla code,
   **not published in the repo** — we do not specify it.
5. **Persistence (teacher's):** only ON entries, hand-built JSON
   `{"<EntityId.Value>": true}` in
   `dump_prefs_<sanitised GameName>.json` **in the mod folder,
   never in the save**.

### ACTIONS

- **ADVISORY / planning rule (our stack, today):** pair every pit
  with a local sink at DESIGN time — when planning a mine or
  flatten (Behaviour 1 / earthworks v2 step 6), designate the
  waste/levelling sink **inside the same tower area first**,
  before any storage routing is considered; prefer in-zone
  dumping/levelling designations over storage for overburden and
  dirt. The agent enforces this in plan review, not in truck
  routing.
- **Future QoS feature sketch (requires Philip's go-ahead, NOT
  built):** a per-tower opt-in flag + the rule 2–4 decision order
  implemented QoS-side. Flagged here as a sketch only.

### LIMITS — deliberately NOT reproduced

- The mechanism: a per-truck `IJobProvider<Truck>` decorator
  installed via reflection, with sim-event lifecycle management
  (it unwraps all trucks before save precisely because the
  wrapper type would otherwise serialize into the save — the
  same failure class as our 0.1.1 incident), plus a
  Reflection.Emit inspector subclass. **No channel in our
  current stack actuates truck job routing.** We distil the
  POLICY, never the interception.
- Edge cases the teacher handles that our planning rule must
  respect as assumptions, not guarantees: full/fulfilled/absent
  sink all collapse to factory=false → vanilla; reassignment /
  despawn / destroyed-tower stale-wrapper sweeps; orphan-wrapper
  sweep after load. In planning terms: a local sink that is
  fulfilled or removed silently reverts behaviour to vanilla
  hauling — re-check sink state at every observe step.

---

## 3. Manual Priority Groups — discipline, override ejection, XOR rule

**Teacher:** CoI_PriorityManager, iScripters — **MIT**.
Clone examined: v0.9.1, game 0.8.8 (CODE,
`PriorityGroupStore.cs:PriorityGroup`).

### INPUTS

- Bridge `/priorities` (v0.6.3+): per-entity custom priorities +
  visibility probes.
- Machines/census: entity proto (native default priority),
  entity class (support rules below).

### DECISION RULES

1. **Group model:** a group carries four channel values —
   `GeneralPriority` (creation default **9**), `ImportPriority`
   (**8**), `ExportPriority` (**8**), `GeneratorPriority` (**9**).
   **Internal values are 0–14** (the game UI displays 1–15;
   `BuildingPriorityService.Set` silently no-ops outside 0–14).
   Channels: General / Import / Export / Generator.
2. **Support rules:** General needs `IEntityWithGeneralPriority`
   - visible; Import/Export need a visible custom-priority id
   (`ImportPrio`/`FuelImportPrio`, `ExportPrio`/`FuelExportPrio`);
   Generator group control applies ONLY when a native default is
   obtainable. **Transports are never eligible.**
3. **Single-membership invariant:** assigning a building to a
   group removes it from every other group first. A building is
   in at most one group, ever.
4. **Override semantics — the discipline to distil:** a manual
   change to ANY channel **ejects the whole building from its
   group, preserving the manually chosen value** — synchronously
   in the teacher's own UI; by a polled monitor for changes made
   anywhere else. The monitor's anti-race grace
   (`HasMatchedGroupValue`): a divergence counts as an override
   only after the group value has actually been observed applied
   once — async command application is tolerated, not fought.
5. **Removal semantics differ from override:** deliberate removal,
   group deletion, or reset **restores native defaults** —
   General → `Prototype.Costs.DefaultPriority` (read live),
   Import/Export → hardcoded **8**, Generator → the proto's
   `GenerationPriority` field (via reflection) if present, else
   no-op.
6. **Application is event-driven only:** on assignment and on
   group edit (single channel re-applied to all members), via
   `IInputScheduler` shipped commands (rule: ACTIONS). **No
   load-time re-application, no enforcement loop** — on load,
   whatever the save stored stands.
7. **Conflict policy: ejection wins, never fight back.**
   Detection is writer-agnostic. Blind spots (teacher's own):
   unsupported controls, and the pre-first-match grace window
   (INFERENCE in the worker report).
8. **XOR RULE (standing, from `local-coi-strategy`):** per save,
   QoSPriorityActuator's automatic nudges OR manual groups on
   the same fields — **never both**. Choosing manual groups for
   a save means the actuator stays off those fields for that
   save, and vice versa.

### ACTIONS

- **Companion §12 prepares** of the three shipped commands, all
  in our §12 catalogue: `SetGeneralPriorityCmd`,
  `SetCustomPriorityCmd`, `SetElectricityGenerationPriorityCmd`.
  Group assignment/edit = a prepared batch of per-member
  prepares, each confirmed under §12 discipline; the "group" is
  the player's bookkeeping (plan/companion record), not a game
  object.
- The ejection discipline is a **player rule**: when Philip (or
  the agent, under direction) manually changes any channel of a
  grouped building, the agent removes it from its group in the
  bookkeeping in the same step and preserves the manual value —
  and never re-applies a group value over a manual one.

### LIMITS — deliberately NOT reproduced

- The teacher's UI, its `ModJsonConfig` persistence
  (`groups_state`, DataContract JSON, state Version=1), and its
  polled monitor machinery. Our monitor is the observe step:
  divergence spotted on a `/priorities` read is surfaced, with
  the grace rule (rule 4) applied by judgement before calling
  anything an override.
- No automatic group enforcement of any kind. If a group value
  and a manual value conflict, the manual value stands
  (rule 7) — the agent reports, it does not correct.

---

## 4. Alarm Model — composable thresholds, hysteresis, acknowledgement lifecycle

**Teacher:** UNMA (Unified Notification & Monitoring Architecture),
max2605 — **MIT**. Clone examined: UNMA 0.10.5, targets game
**0.8.6c — NOT verified against 0.8.8b** (DOCS + CODE from the
UNMA worker report). This is the reference model for QOL-MODS #5
(companion-side alerting).

### INPUTS (all suppliable from our surfaces)

- Companion/bridge metrics — UNMA has **no closed alarm-type
  enum**: any numeric/bool readable is a metric. Global metrics
  include `health.*`, `workers.*` (reserve_percent, free_or_missing…),
  `food.months`, `food.starving`, `food.spiral`, `population.*`,
  plus per-product `product.stored/capacity/fill.<id>` and
  per-type `maintenance.quantity/capacity/fill/delta_month/
  needed_month.<id>` — every one maps to a products/stats,
  bridge status/power/machines, or snapshot field we already read.

### DECISION RULES

1. **Severities: exactly 4** — Notice=0, Warning=1, Critical=2,
   Emergency=3. Default colours: Warning `#F0C541`, Critical
   `#F05A32`, Emergency `#E51B23`, Notice fallback `#83C5BE`.
   Default sounds: Emergency→siren, Critical→horn, Warning→bell,
   Notice→sine (display/audio only — see LIMITS). Factory policy
   (DOCS): **Emergency is reserved for an active hunger/health
   death spiral; a pure worker shortage caps at Critical.**
2. **Conditions:** six operators (<, <=, =, !=, >=, >), equality
   tolerance 1e-6. Value modes: Absolute and
   **PercentOfReference** — a missing/zero/negative reference
   makes the condition **FALSE, never true**; >100% is not
   clamped. Composition is **flat** `All` (AND) / `Any` (OR) over
   the rule's whole condition list — **no nesting**.
3. **Hysteresis (per condition, Schmitt):** the activation
   boundary is the threshold exactly; only the RELEASE boundary
   shifts by h. Poll default 500 ms (clamp 100–5000) — in our
   companion the poll is the snapshot/observe cadence, not a
   game tick loop.
4. **Timing in game ticks** (SimTicksPerDay=20, Month=600,
   Year=7200): `ActivationDelayTicks` (condition must hold
   continuously before firing); `ResetDelayTicks` and
   `MinimumActiveTicks` run **concurrently** before a clear.
5. **Acknowledgement lifecycle — the K/KQ/KG/KGQ model:**
   K = active, unacknowledged; KQ = active, acknowledged
   (silent, colour stays); KG = cleared but unacknowledged
   (**latched** — the "it happened while you weren't looking"
   state); KGQ = cleared + acknowledged (**the only deletable
   state**). A **new occurrence** = reactivation, severity
   increase, or escalation — it **resets acknowledgement**; ack
   never carries across occurrences. `AutoAcknowledgeOnClear`
   is opt-in, **default false** everywhere. Ack scopes: slot /
   panel / area / master — **ack belongs to the occurrence, not
   the view**.
6. **Escalation:** a single sticky step, only if AfterTicks > 0
   AND target severity > base AND base < Emergency; escalation
   creates a new occurrence (re-ack required).
7. **Auto-pause:** only on a NEW unacknowledged occurrence with
   severity ≥ configured minimum; shipped defaults
   `autoPauseEnabled=false`, `autoPauseMinimumSeverity=2`
   (Critical).
8. **Shipped default system alarms (exact thresholds, DOCS/CODE):**
   - Health — Warning `health.value < 10`; Critical
     `health.value <= -10`; Critical
     `health.pollution_penalty <= -5`; Emergency
     `health.disease_active >= 1 AND
     health.worker_spiral_margin <= 0`; Emergency
     `health.structural_value < 0 AND
     health.pollution_penalty < 0`.
   - Food — Warning `food.months <= 12`; Critical
     `food.months <= 3`; Emergency `food.spiral >= 1`.
   - Workers — Warning `workers.reserve_percent < 5`; Critical
     `workers.reserve_percent < 0`.
9. **Lifecycle guard:** if an entity a rule references is
   permanently destroyed, the **whole rule is deleted** (so
   AND/OR logic can't silently change meaning); 10-second grace
   for static-entity missing checks.

### ACTIONS

- **ADVISORY-ONLY:** the companion Issues/alerts layer. Alarms
  surface, latch (KG), and require acknowledgement in the
  companion; they never actuate the game. Auto-pause, if ever
  adopted, would be a §12-class act needing its own
  authorisation — the shipped default is OFF and stays off.

### LIMITS — deliberately NOT reproduced

- In-game panels, sounds, and the external-provider API.
- **Snooze ≠ ack:** UNMA's snooze is audio-only, 1 game month.
  Keep the concepts separate; our model has acknowledgement
  only.
- No nested condition logic (rule 2) — a rule that seems to need
  nesting is two rules.
- Version caveat: thresholds above are as shipped for 0.8.6c;
  metric availability on 0.8.8 is by our own bridge mapping,
  not by UNMA compatibility.

---

## 5. Forestry Designations — scan, eligibility, sustainable-yield sizing

**Teacher:** Auto Forestry Designations (AFD), Kayser1444 — **MIT**.
Requires game 0.8.5+ (CODE, `AFDTowerSettings` and scan/commit
sources in the AFD worker report).

### INPUTS

- `/terrain` + `/deposits`-equivalent fertility/tree data over the
  tower's painted Area polygon; tower config (tree types, target
  harvest percent); machines/census for the truck-pool quota
  (rule 6).

### DECISION RULES

1. **Scan scope:** one click scans the tower's OWN painted Area
   polygon only — no radius, no whole-forest detection. Step =
   4 tiles; each candidate is one 4×4 **ForestryDesignator** cell,
   eligible only if **all 16 sub-tiles are in-area**. Heights come
   from the four corners into a DesignationData.
2. **Eligibility defaults:** `onlyFertileTiles=true` (every
   sub-tile fertile + unblocked); `avoidTilesWithTrees=false`;
   `avoidFlatTiles=false` (flat = all four corners within
   **0.0625 tiles** of one integer height);
   `onlyReachableTiles=true`. World setting
   `overrideTerrainDesignations=false` — if true, forestry
   replaces mining/dumping/levelling at commit. Per-tower
   settings store only diffs from globals.
3. **Ordering:** BFS driving distance over pathable tiles from
   the nearest pathable tile to the tower (margin 96 tiles,
   ≤250,000 visited; candidate centre = origin+(2,2); match
   square `pathabilityTargetSize` default 3, clamp 1–9).
   Unreachable candidates sort last; interior-hole backfill only
   in unlimited mode. Planning is time-sliced (10 ms/frame
   playing, 30 ms paused).
4. **Sizing modes are mutually exclusive:** `targetYield > 0`
   XOR legacy `maxTiles > 0`; neither = unlimited.
   **Target-yield model:** spacing = weighted average of
   `TreeProto.SpacingToOtherTree` over the tower's configured
   tree types; capacity = live managed trees +
   spacing-respecting future trees; sustainable wood/month =
   **floor(capacityPerYear / 12)** with yield/tree/year at the
   tower's TargetHarvestPercent (clamped to 1.0; the 200%
   NO_CUT_AT sentinel reads as 100% / harvest-disabled).
   Candidates are added in distance order until projected ≥
   target — **the final cell may overshoot**. **Fail-closed:**
   if the yield estimate fails, no tree type is configured, or
   spacing is unavailable → **place nothing** that run.
5. **Planting only.** Optional post-step
   `markHarvestReadyForHarvest=false` (default): marks ready
   trees via `TreesManager.AddToHarvest` (a direct manager call,
   creates no designations). Clearing removes ONLY forestry
   designations (direct `TerrainDesignationsManager.
   RemoveDesignation` per origin).
6. **Truck pooling (default ON) — distil the quota rule, not
   the machinery:** pool = trucks assigned to the tower + trucks
   adopted from its harvesters (prototypes `KPIE_Forager` /
   `KPIE_Miner` always excluded); quota = equal split of N pool
   trucks over M target harvesters: `base = N / M`, remainder
   +1 to the first `remainder` harvesters in sort order
   (footprint size desc → current pool count desc → EntityId
   desc); disabled harvesters get quota 0 and are drained (if
   none enabled, all are targets); transfers are
   stability-preserving (unassigned first; donors surrender
   from the end of their list). Rebalance triggers: assignment
   changes, destruction, tower events, save-time reconcile,
   pooling toggle.

### ACTIONS

- **Designation channel = the recon verdict's channel:** create
  = ONE bulk `AddTerrainDesignationsCmd` via `IInputScheduler`
  (the teacher's own execution, CODE `AFD.Scan.cs:
  CommitDesignationsCoroutine` — the precedent for our
  QoSBuilder-style designation jobs). ADVISORY / plan-producing
  until that executor is built (see verdict section).
- Clear + harvest-marking in the teacher are direct manager
  calls, NOT commands — our stack has no equivalent channel;
  clearing maps to `RemoveDesignationsCmd` (origins list), and
  harvest-marking has **no channel** (advisory only).

### LIMITS — deliberately NOT reproduced

- The Harmony vehicle-optimisation layer (field-wait parking
  suppression, future harvest/planting claims) and the Harmony
  depot-ordering patches.
- Doc-vs-code catch, recorded so we don't inherit the doc's
  error: the AFD readme claims depot choice is "closest by
  driving distance" — the source uses **straight-line**
  distance. Code wins.
- The truck-pool machinery (adoption, transfer execution) —
  we apply the quota arithmetic in planning/advice; assignment
  actuation stays with Philip / vanilla.

---

## 6. Research Queue Discipline — prerequisite-clamped ordering, queue-preserving cancel

**Teacher:** COI Research Queue (RQ), Jagg111 — licence as
published in repo (worker report: clone v1.0.1, max verified game
**0.8.4b**). Vanilla facts below are GAME-CODE / 0.8.8-observed
per the worker report.

### INPUTS

- Bridge `/research`: completed nodes, current research +
  progress, queue contents/order.

### DECISION RULES

1. **Know the vanilla substrate (0.8.8):** vanilla HAS a research
   queue; vanilla has **no reorder UI** (the player's only fix
   is clear-and-rebuild); vanilla `StopResearch()` **clears the
   entire queue**; on natural completion the vanilla manager
   dequeues the next item and **silently discards it** if its
   prerequisite is unresearched and sits later in the queue.
   RQ changes nothing about natural completion.
2. **Prerequisite-clamped ordering (the policy):** with the moved
   item removed, earliest legal slot = max over its unresearched
   parents present in the queue of (parent index + 1); latest
   slot = min over queued dependents of their index; append-at-
   end is always allowed when there are no dependents. A queue
   built under this clamp can never trigger vanilla's silent
   discard.
3. **Promote:** refuses when `IsLocked`; otherwise cancels the
   current node (**progress preserved**), reinserts the old
   current at the queue FRONT, and starts the promoted node.
4. **Queue-preserving cancel:** cancel current → auto-start the
   next queued item. This is the teacher's ONLY automation, and
   it fires only on explicit cancel/promote — never on natural
   completion.
5. **Surface, don't strand:** out-of-order rows are flagged with
   a "Move below: <prereq>" warning rather than left for the
   silent discard.
6. **No queue-length cap exists in the teacher's source** — any
   cap in our plans is our own plan discipline, not a distilled
   number. Paused research is not addressed by the teacher
   beyond a no-active-lab warning display.

### ACTIONS

- **ADVISORY / policy for the plan + runner:** plan research
  orders are authored in prerequisite order (rule 2's clamp
  applied at authoring time); the runner treats out-of-order
  queue state read from `/research` as a **defect to surface**,
  never as a state to rely on; cancel-in-vanilla is treated as
  **destructive (queue wipe)** in every plan that mentions
  stopping research.
- Actuation through our stack is limited to the catalogue's
  research commands via §12. **Reordering itself has no
  shipped-command channel found** — advisory only.

### LIMITS — deliberately NOT reproduced

- The mechanism: direct reflection mutation of
  `ResearchManager.m_researchQueue`, the `CurrentResearch`
  private setter, and `refreshQueueValues()` — no Harmony, no
  commands, and no mod-owned save data, but private-state
  mutation all the same. Outside our posture.
- Any assumption that RQ's behaviour holds on 0.8.8: the clone's
  max verified game version is 0.8.4b; the vanilla facts in
  rule 1 are the load-bearing part and are stated for 0.8.8.

---

## NOT DISTILLABLE FROM PUBLISHED EVIDENCE (named, with reason)

- **Smart Mining Dump — sink selection.** Which sink the vanilla
  `DumpingJob.Factory` picks among eligible designations is
  vanilla code, not in the teacher's repo. The policy (prefer
  local) is distilled; the selection algorithm is not, and is
  not invented here.
- **ATD — routed-accessway numeric width per tier / slope per
  cell; material slope numbers; flattening blend beyond
  percentile; per-designation batch ordering.** All checked in
  source: runtime-derived from vanilla pathfinding params /
  game protos, or nonexistent (see Behaviour 1 LIMITS and the
  worker report's "not published" register).
- **Research Queue — any queue-length cap; paused-research
  handling.** Absent from source (Behaviour 6, rule 6).
- **UNMA on 0.8.8.** The model and shipped thresholds are
  distilled as published for 0.8.6c; no 0.8.8 verification exists
  in the teacher's materials, so metric mapping is ours and
  labelled as such.
- **PriorityManager — pre-first-match override detection.**
  The grace window's exact blind spot is INFERENCE from the
  monitor's design, not a stated guarantee.

---

*Round: Behaviour Distillation, 2026-11-06. Sources: ATD worker
extraction (full source read, file:symbol provenance), Smart
Mining Dump + PriorityManager worker report, UNMA + AFD + RQ
worker report, recon round 14 (this round), MOD-SURVEY.md.
No mods built, installed, or contacted the game in this round.*
