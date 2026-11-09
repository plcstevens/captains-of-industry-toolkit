# Superintendent — Earthworks Guides: Dump, Flatten, Mine

How to move ground effectively in Captains of Industry — dumping,
flattening and mining — written against the companion's own sensing:
the bridge v0.6.5 terrain/deposits/designations routes and the
earthworks estimator (SPEC Addendum XIX). Compiled 2026-11-06 for
Philip's v0.8.8 (Update 4.3) game.

Source discipline: every mechanic and number below is traced to the
shipped 0.8.8 assemblies (decompiled with ilspycmd; class names
given), to the official wiki, or is explicitly labelled
**RULE-OF-THUMB** / **DERIVED** (an inference drawn from traced
numbers, stated as such). Where the wiki's older tables and the
0.8.8 code disagree, the code wins and the disagreement is noted.

---

## 0. Shared foundations — the ground rules all three guides stand on

### 0.1 Units (code: `TerrainTile`, recon 11)

- One tile is **2 m × 2 m**. One height unit is **1 tile (2 m)**.
  Heights are Fix32, raw 65536 = 1.0 tile.
- One tile-cubed of earthworks is exactly **8 m³**
  (`TILE_VOLUME_M = 8`). The estimator speaks tiles³ and m³; both
  are geometry, not estimates.

### 0.2 The designation is the unit of work (code

`TerrainDesignation`, `DesignationDataFactory`)

- Work is ordered in **designation cells of 4×4 tiles**, snapped to
  the global 4-tile grid: cell origins are multiples of 4
  (`DesignationDataFactory.GetOrigin(v) = v * 4`;
  `TerrainDesignation.SIZE_TILES = 4`). A dragged area is quantised
  to whole cells — **partial tiles do not exist; every designation
  edge is a tile edge, and every cell corner is a grid vertex**.
- Each cell carries **four corner target heights** (origin, +X, +Y,
  +XY). The target inside the cell is the bilinear plane through
  those corners. All four equal = a flat cell; a 1-tile difference
  across one axis = a **ramp cell**. Shared vertices are literally
  shared: neighbouring cells reference the same corner heights,
  which is what makes large areas continuous.
- One cell serves both directions: tiles **above** the target plane
  are mining work, tiles **below** it are dumping work. Fulfilment
  is tracked per vertex (a 5×5 bitmap per cell) and separately for
  mining and dumping (`IsMiningFulfilled` / `IsDumpingFulfilled`;
  `IsFulfilled` = all 25 vertices done). A designation is finished
  when the game says so, not when it looks flat.
- **Neighbour snapping is a hard constraint, not a courtesy**
  (`DesignationDataFactory.TryCreateSnapToNeighbors`): a new cell
  placed against existing ones must match their shared corner
  heights exactly, and across any shared edge the height step must
  be **0 or exactly ±1 tile**. Anything else — a 2-tile step, a
  non-planar twist — makes the snap fail and the placement is
  refused. This single rule shapes every layout decision in these
  guides: **levels in a connected earthworks area form a staircase
  of at most 1 tile per 4-tile cell.**

### 0.3 Terrain collapses — the physics budget (code

`TerrainPhysicsSimulator`, `TerrainMaterialsData`)

Dumped or exposed material does not hold any slope you draw. Each
material has a **collapse band** `[MinCollapseHeightDiff,
MaxCollapseHeightDiff]` (height difference between adjacent tiles,
in tiles):

- Below the min: the edge is stable, forever.
- Between min and max: each physics pass may slide material
  downhill with probability ramping linearly from 0 at min to
  certain at max; each slide moves **half the excess difference**
  from the high tile to the low tile.
- Global floor: no collapse under 0.125 tiles of difference
  (`MIN_HEIGHT_DIFF_FOR_COLLAPSE`).

Bands for the materials that matter (tiles of height difference
per tile of run; code, `TerrainMaterialsData`):

| Material | Min | Max | Reading |
|---|---|---|---|
| Landfill (dumped waste) | 0.4 | 1.5 | slumps early, keeps creeping |
| Dirt | 0.6 | 1.2 | a 1-tile dirt step is already in the slide zone |
| Gravel / crushed rock | 0.7 | 1.6 | |
| Sand | 0.8 | 1.0 | narrow band — sand finds its angle fast |
| Rock (disrupted), ores (disrupted) | 0.8 | 2.0 | what a dug face is actually made of |
| Grass | 0.9 | 1.8 | |
| Forest floor | 1.0 | 2.0 | |
| Coal (in ground) | 1.4 | 4.0 | |
| Quartz | 1.6 | 5.0 | |
| Rock, bedrock, iron/copper/gold ore (in ground) | 2.0 | 5.0 | a 2-tile rock face is guaranteed stable |
| Hardened rock | 4.1 | 5.0 | |
| Titanium / uranium ore (in ground) | 2.5 | 6.0 | |

Cross-check: the wiki's older slope table (v0.7.8) lists Dirt at
min 0.6 / max 1.2 — identical. The wiki table is the same physics
in older dress; the code values above are the 0.8.8 truth.
(<https://wiki.coigame.com/Designations>)

**DERIVED consequence used throughout:** a bench face is
guaranteed stable only up to the material's *min* band value —
≈0.6 tiles in dirt/soil, ≈2 tiles in undisturbed rock and ore,
≈0.8 in the disrupted material a dug face actually exposes. Faces
drawn steeper than that don't fail catastrophically; they *relax*,
slide by slide, toward the min — and every slide is material your
trucks handle twice.

### 0.4 Retaining walls (code: `RetainingWallsData`

`RetainingWallProto`; costs: wiki)

- A wall **holds at most 5 tiles of height difference**
  (`RETAINED_HEIGHT_TILES = 5`). The in-game description: "Walls
  will collapse if they hold more than {0} units of height or if
  they are overfilled."
- Wall tiles **disable terrain physics** on the tiles they occupy
  (`LayoutTileConstraint.DisableTerrainPhysics`) — that is the
  whole mechanism: the face behind a wall cannot start a slide.
- Walls are placed *submerged*: their layout requires terrain
  between −5 and 0 tiles relative to the wall's elevation, and the
  placement elevation is adjustable ("placed below the surface to
  prevent terrain collapse during mining, or above terrain to aid
  with dumping operations").
- Variants (wiki, <https://wiki.coigame.com/Retaining_Wall>):
  Short 1×2 — 4 iron + 10 concrete slabs; Corner 2×2 — 6 + 12;
  Long 2×4 — 12 + 40. **The Long wall is exactly one designation
  cell edge (4 tiles)** — walls and designations share the grid.
  Research: Retaining Walls.
- A wall whose foundation tiles are dug below its bottom or filled
  above its top takes damage and collapses (wiki; the code's
  collapse-vertex thresholds say the same). Digging out *under* a
  wall is the classic way to lose one (community, Steam guide
  discussion).

### 0.5 The vehicles (code: `ExcavatorsData`, `DumpingJob`

`ExcavatorJobProvider`)

| Excavator | Bucket | Dig reach (tiles) | Thickness removed per dig, near→far |
|---|---|---|---|
| Small excavator (T1) | 6 | 2–5 | 1.5 / 1.0 / 0.5 |
| Excavator (T2) | 20 | 3–7 | 2.0 / 1.5 / 1.0 / 0.5 |
| Mega excavator (T3) | 60 | 4–9 | five steps (exact array not traced — see notes) |
| Amphibious | 16 | 4–9 | 2.0 / 1.5 / 1.0 / 0.5; max mining depth in water **5 tiles** |

- A dig removes a *thickness* at the target tile; thickness falls
  with distance. An excavator chews a face fastest standing close
  under it, slowest at full stretch.
- **Dumping designations are worked by trucks**: `DumpingJob` is a
  truck job (`Mafi.Core.Vehicles.Jobs.DumpingJob`, built for
  `Truck`). Excavators mine and load; trucks haul and dump.
- **Mining designations are worked by excavators assigned to a
  Mine Control Tower**, and only inside that tower's control area
  (official tutorial, translation hub: "mining designations must
  be within an area managed by a {Mine Control Tower}", while
  "dumping designations can be placed anywhere on the island").
  Tower control area max size: **49 designations (196 tiles) per
  side** (wiki, <https://wiki.coigame.com/Mine_Control_Tower>).
- **One excavator per designation cell at a time** (official
  tutorial: "Each designation square can be mined by only one
  excavator at a time. Be sure that there are enough available
  designations if you have multiple excavators.").
- Designations must be *reachable*: "Mining designations have to
  start at or above the terrain, otherwise, they won't be
  reachable by your excavators. If an edge of designation is not
  reachable, a red line is displayed." (official tutorial).
- All excavators path under `SteepnessPathability.SlightSlopeAllowed`
  (code, proto path-finding params). The game's own sanctioned
  vehicle grade is the ramp cell: **1 tile up per 4 tiles of run
  (25%)**.

### 0.6 What dumped material becomes (code

`UnstableTerrainValidator`, `TerrainMaterialsData`)

- Dumped product becomes terrain material of its own kind (dirt →
  dirt layers, rock → rock, and so on) — **except waste, which
  becomes Landfill material**, and Landfill (and weathered Landfill)
  are the only materials flagged `UnstableTerrainMaterialParam`.
- **You cannot build on landfill**: a tile counts as unstable
  ground for building placement if more than **0.25 tiles** of
  unstable material sits in its top layers before **0.75 tiles**
  of stable cover accumulates above it (validator constants
  `MAX_LANDFILL_IGNORE = 0.25`, `MIN_LANDFILL_COVER = 0.75`).
  Placement on such a tile is refused ("Not stable"). Fresh waste
  fill is therefore *not* building land until capped with ≥0.75
  tiles of stable material (or weathered — landfill recovers over
  years, `recoveryTime: 4.Years()` to its weathered form).
- Dumping into the ocean is land reclamation with a hard ceiling:
  `TerrainDesignation.MAX_DUMP_HEIGHT_IN_OCEAN = 0` — an ocean
  designation's target tops out at **sea level (height 0)**.
  Reclaimed land for ports needs **+2 to +5** above sea level
  (wiki, Designations page) — so a port pad is two jobs: reclaim
  to 0, then lift on the dry side.

---

## 1. Dump effectively

### 1.1 The mechanic

You designate a volume (cells × target plane); assigned trucks
drive in, tip their loose product, and the terrain rises toward
the target, vertex by vertex. What may be dumped is governed by
the **allowed-dumping product settings**: global settings by
default, but a designation inside a Mine Control Tower's control
area is governed by *that tower's* settings (community-tested,
Steam: tower settings take precedence inside the area, global
outside it — <https://steamcommunity.com/app/1594320/discussions/0/603025705548131897>).
With the Ore Sorting rule on, a truck will never dump a mixed load
directly — it routes to a sorter first (same thread).

Dumping is how you: dispose of mining waste close to the pit,
build ramps and pads without digging them out of somewhere else,
and reclaim coast. It is also the only sink that never fills a
storage — but it spends truck cycles and it makes *landfill* of
waste, with the building ban that follows (§0.6).

### 1.2 Placement & snapping — siting the pad (CORE)

Dumping is a placement problem before it is a hauling problem.

1. **Snap the pad to the 4-grid, on district seams.** The pad is
   a whole number of 4×4 cells; align its edges with the edges of
   the flattened pads, roads and belt runs it will neighbour
   (all of which live on the same tile grid). A pad offset by 2
   tiles from the district grid wastes a half-cell strip along
   every seam forever.
2. **Site it against the cut it receives.** A dump pad's cost is
   truck cycles: cut → load → haul → tip → return. The estimator
   prices the *volume*; the *distance* is your placement choice.
   Put the waste pad in the same district as the pit it serves —
   ideally on ground that is itself useless (over a worked-out
   deposit, on rock you will never mine, on coast you intend to
   reclaim). **Never site a dump over an unmined deposit region**
   (see §1.4): you are burying ore under your own waste and will
   pay to mine it twice.
3. **Build in lifts, one tile at a time.** Each lift is one flat
   designation pass at current-grade +1. One cell × 1 tile of
   height is 16 tiles³ = 128 m³ of geometry (the loose-product
   unit count varies by material density — wiki quotes 24–42
   units per cell-height from its older density table). Lifts
   respect the physics: a fresh 1-tile exposed edge in dirt is
   already above dirt's 0.6 min band, so an unretained pad edge
   *will* relax outward until its steps are ≤ the band. Either
   accept a battered (sloped) pad edge — leave a sacrificial
   margin cell around the pad — or hold crisp edges with
   retaining walls (≤5 tiles of held height per wall line).
4. **Terrace tall dumps.** For a pad many tiles high, stack lifts
   as terraces: each lift inset by at least one cell on the
   exposed sides, so no single face exceeds what the material
   holds (dirt ≈0.6 guaranteed, dumped rock/disrupted ≈0.8,
   landfill only 0.4). A wedding-cake pad is stable; a sheer
   6-tile dirt cliff is a slow landslide with a truck route at
   the bottom.
5. **Shoreline reclamation placement.** Straddle the coast: the
   designation's landward cells give trucks drivable ground at
   grade; the seaward cells (over `IsOcean` tiles) fill toward
   target 0 (the ocean ceiling, §0.6). A fully-ocean designation
   whose target is ≤ 0 is **not ready for non-amphibious trucks**
   (`TerrainDesignation.IsReadyToDump`: fully-in-ocean + target
   ≤ 0 → false) — work must grow outward from a shore-connected
   edge. Once the new land is at 0 and connected, a second,
   ordinary lift pass raises the building pad to +2 or above.

### 1.3 The effective method

1. Decide the product and confirm it is in the allowed-dumping
   set that governs the site (tower settings if inside a tower
   area, else global).
2. Survey the site with `/terrain` (heights) and `/deposits`
   (nothing worth mining underneath). Run the estimator for the
   full pad volume so the truck commitment is known before the
   first load moves.
3. Designate lift 1 (flat cells, target = lowest current grade
   +1). Keep the active front small — a couple of cells — so
   trucks tip at the face instead of touring the pad.
4. Watch `/designations`: `NumberOfJobsAssigned` > 0 and dumping
   fulfilment advancing. Trucks idle at the tower with full beds
   means the dump is unreachable or the product isn't allowed —
   not "more trucks needed".
5. Step the target up lift by lift. On the final lift of a pad
   meant for buildings, cap with stable material (dirt/rock)
   ≥0.75 tiles thick if anything below is landfill.
6. Close the designation out (remove fulfilled cells) so idle
   trucks stop reserving trips to a finished pad.

### 1.4 Failure modes

**Failure mode** — Trucks loaded with waste idle at the tower or
circle back full.
**Symptoms** — Excavators wait on trucks, trucks wait on nothing
visible; the mine's output decays to zero with every vehicle
"busy".
**Prevention & fix** — The dump path is broken, not the fleet:
the product isn't in the governing allowed set, the designation
is fully-ocean with no shore-connected start, or the only dump
is far side of the island. Wiki states the base symptom directly:
"Trucks will return to the tower and idle if they are unable to
unload their cargo" (Mine Control Tower page). Fix the
designation/settings; fleet size is the last suspect.
**Source** — <https://wiki.coigame.com/Mine_Control_Tower> ;
`TerrainDesignation.IsReadyToDump` (code).

**Failure mode** — Pad edges creep and the pad "loses" height.
**Symptoms** — A finished lift reads a tile lower at the rim on
re-survey; material accumulates in a skirt around the pad.
**Prevention & fix** — Working as coded: rim diffs above the
material's min band (dirt 0.6, landfill 0.4) slide with rising
probability to the max band. Design for it — terrace, margin
cells, or retaining walls — instead of re-dumping the same rim.
**Source** — `TerrainPhysicsSimulator.simulateTerrainFall` (code).

**Failure mode** — Buildings refuse the new land: "Not stable".
**Symptoms** — Placement ghost red on freshly filled ground that
looks perfectly flat.
**Prevention & fix** — The fill contains landfill material
(dumped waste) within the top layers beyond 0.25 tiles without
0.75 tiles of stable cover. Cap it, or wait out weathering
(years). Never plan a district directly on a waste lift.
**Source** — `UnstableTerrainValidator` (code).

**Failure mode** — Retaining wall collapses at the pad face.
**Symptoms** — Wall segments crumble; held fill spills through
the gap and takes the next segment's footing with it.
**Prevention & fix** — The wall was asked to hold > 5 tiles, was
overfilled above its top, or its foundation was undercut. Keep
held height ≤ 5 tiles per wall line, step wall lines down the
face in ≤5-tile drops (community pattern: wall line at the face,
dig/dump 5 levels, next wall line at the new grade, ~2 tiles out
— <https://steamcommunity.com/app/1594320/discussions/0/4355618325413445009/>),
and never designate digging under a wall's own tiles.
**Source** — `RetainingWallsData` (code); wiki Retaining Wall page.

**Failure mode** — The dump eats the fleet (cross-ref).
**Symptoms** — A waste flow with an export set captures every
truck; production starves elsewhere.
**Prevention & fix** — As in the transport guide's slag case:
split flows physically and give the dump its own assigned
vehicles via the tower instead of the general pool.
**Source** — guides-research.md §1 (Steam, slag storage thread).

### 1.5 With our data

- **Before designating:** `coi terrain region` over the pad
  footprint + `/deposits` regions — if the pad rectangle
  intersects a deposit region with remaining quantity, move the
  pad. This check is mechanical and absolute.
- **Price the pad:** estimator fill at each lift height gives
  tiles³/m³ per lift; multiply by nothing — geometry is the
  commitment. Truck-count sanity stays RULE-OF-THUMB: keep the
  haul inside the district and the active front at 1–2 cells per
  assigned truck pair.
- **During:** `/designations` shows the pad's cells, target band
  and `NumberOfJobsAssigned`; a cell at 0 jobs with unfulfilled
  dumping vertices is a stuck dump, diagnosed per §1.4.
- **After:** re-fetch the region; fulfilled cells should read at
  target. Rim tiles reading low = collapse skirt — record the
  pad's *effective* height, don't just re-dump blindly.

---

## 2. Flatten effectively

### 2.1 The mechanic

Flattening is one designation plane doing both jobs at once:
above-plane tiles are mined, below-plane tiles are dumped, and
the cell is fulfilled when every vertex sits at the plane
(§0.2). The game's flatten/level tooling creates flat cells; a
fresh flat cell's starting target is derived from the terrain it
covers — the factory contract (`CreateFlatNoSnapping`) takes the
**maximum** sampled terrain height (ceiled) when the tool is
fill-biased, or the **minimum** (floored) when dig-biased. In
practice: a level designation dropped on rough ground starts by
mostly filling or mostly cutting, not by averaging — you choose
the actual height, the default just shows the bias.

The levelling-specific trap is named in the game's own strings:
`DesignationWarning__CannotStartLeveling` — a level area whose
edges can't be reached can't start (the tutorial's red-line rule:
designations must start at or above terrain; an unreachable edge
shows red, an area ringed in red cannot be worked at all).

### 2.2 Placement & snapping — the pad is a grid decision (CORE)

1. **Choose the pad height from the terrain, not from hope.**
   Run the estimator at each candidate target height across the
   footprint; the right height is the one that minimises
   cut + fill (balanced earthworks — neither importing fill
   across the island nor exporting a mountain of cut). This is
   a **DERIVED** procedure: the estimator makes it a five-minute
   computation instead of a guess, and it is the single highest-
   value habit in this guide.
2. **Snap the rectangle outward to whole cells.** Round the pad
   bounds *out* to the 4-grid. A pad that ends mid-cell is
   impossible (the game quantises the drag), and a pad rounded
   *in* leaves a rough fringe strip that no later designation
   can clean up without re-cutting the seam.
3. **Land the pad on the district's height standard.** Adjacent
   pads at different heights must connect through the only
   transitions the grid allows: a **ramp cell** (±1 tile across
   4 tiles) wherever vehicles cross, or a **retaining wall** at
   a held edge where they don't. Two pads 2 tiles apart in height
   with no ramp row between them are two islands — the neighbour
   snap (§0.2) forbids the step, and trucks will path around the
   long way or not at all. Fix pad heights across a district to
   a small set of standard levels, one tile apart at most
   between neighbours, exactly like floor levels in a building.
4. **Align pad edges to the layout seams.** Belts, roads and
   building rows all live on the tile grid; pad edges on cell
   boundaries (multiples of 4) keep every later placement
   trivially aligned. Where a trunk road or rail line will run,
   flatten its corridor in the same pass and at the same height
   — a corridor re-levelled later re-cuts both its neighbours.
5. **Mind what the pad buries and what it exposes.** The
   estimator is material-agnostic (SPEC XIX): pair it with
   `/deposits` — the cut side's composition is the deposits
   table. Flattening *through* a deposit is mining without a
   sorter plan; flattening *over* one is §1.4's buried-ore
   failure in a new hat.

### 2.3 The effective method

1. Survey: `/terrain` summary for the district, region fetch for
   the exact footprint.
2. Estimator sweep: cut/fill at target heights across the
   plausible band (min terrain height … max). Pick the balance
   point; if cut ≫ fill even at the lowest sensible height, the
   pad is a quarry — plan the cut's destination (a dump pad from
   Guide 1) *before* designating.
3. Designate the flat area at the chosen height, snapped outward
   (§2.2.2), inside a tower's control area wherever mining work
   is involved (flattening's cut side obeys the mining rules —
   excavators only work tower-managed designations).
4. Let both fulfilments run; monitor via `/designations`
   (mining and dumping fulfilment are separate bitmaps — a pad
   can be dump-complete and mine-stalled if the cut side lost
   its destination for mined material).
5. Acceptance: re-fetch the region. Every tile at target (the
   designation's own `IsFulfilled` is the game's verdict; the
   region fetch is yours). Only then release the pad to building
   placement — and check §0.6 first if any fill was waste.

### 2.4 Failure modes

**Failure mode** — Levelling never starts.
**Symptoms** — Designation placed, no jobs assigned, red edges
in-game; `NumberOfJobsAssigned` stays 0 in `/designations`.
**Prevention & fix** — The plane starts below the terrain on
every approach edge, so no excavator can reach a first diggable
tile (tutorial red-line rule). Re-cut the entry: start the pad
with a ramp row from the approach side at terrain height, or
raise the first cell row to start at/above grade.
**Source** — official tutorial (translation hub);
`DesignationWarning__CannotStartLeveling` (code string).

**Failure mode** — The seam step: two pads, one cliff.
**Symptoms** — A 2-tile vertical between neighbouring pads;
vehicles detour or refuse; belts can't cross.
**Prevention & fix** — The neighbour-snap rule (§0.2) allows
only 0/±1-tile steps between cells. Insert a ramp cell row, or
re-level the lower pad up a tile. Never draw two flat areas at
heights ≥2 apart sharing an edge and expect the game to sort it
out — it will refuse or leave the step.
**Source** — `DesignationDataFactory.TryCreateSnapToNeighbors` (code).

**Failure mode** — The pad sags after acceptance.
**Symptoms** — Rim and corner tiles drift below target over the
following weeks; buildings near the edge report terrain errors.
**Prevention & fix** — Post-fulfilment collapse: exposed pad
edges steeper than the fill material's min band (dirt 0.6,
landfill 0.4) keep relaxing. Hold working edges with retaining
walls, or batter the edge (one sacrificial cell stepped down
0.5–1 tile — inside the band) at designation time.
**Source** — `TerrainPhysicsSimulator` (code).

**Failure mode** — Flat, finished, and unbuildable.
**Symptoms** — "Not stable" refusals on a pad that passed its
height check.
**Prevention & fix** — §0.6: the fill was waste-bearing. The
height data can't see it; the material layers can (the bridge
deposits walk reads layers — a pad over landfill material is
visible in layer data). Cap with ≥0.75 tiles of stable material.
**Source** — `UnstableTerrainValidator` (code).

### 2.5 With our data

- **Estimator sweep is the method** (§2.2.1): `coi terrain
  estimate X0 Y0 X1 Y1 TARGET` per candidate height; tabulate
  cut/fill; choose balance. The JSON shape is the documented one
  (SPEC XIX), so the play layer can run the sweep itself.
- **Grid check before designating:** bounds ≡ 0 mod 4 on all
  four sides; target height ∈ the district's standard levels;
  every boundary step to existing designations ∈ {0, ±1}.
- **Acceptance:** region re-fetch + `IsFulfilled` from
  `/designations` — both, not either.

---

## 3. Mine effectively

### 3.1 The mechanic

On-map deposits are **terrain material layers**, not objects
(recon 11): each tile carries a stack of material layers
(top → bottom, bedrock last and unminable-in-practice — it is
excluded from the deposits walk). An excavator digs the tile's
**current top layer**; the mined product and quantity come from
that layer's material (`ToPartialProductQuantity` — the game's
own conversion). Consequences that drive everything below:

- **You mine a deposit from the top down, through whatever lies
  above the ore.** The layers above the pay layer are the
  *overburden* — dirt, rock, other materials — and every tile of
  it must be dug, hauled and put somewhere before the ore under
  it is reachable.
- **Remaining ore is exact, not estimated**: mining removes layer
  thickness, so today's layers *are* the reserve (recon 11;
  `/deposits` sums them per material and per connected region).
- Excavators work tower-assigned designations (§0.5), one
  excavator per cell at a time, digging within reach (2–5 / 3–7 /
  4–9 tiles by tier) and removing the per-distance thickness of
  §0.5 per dig. A dig face is worked from a drivable bench the
  excavator can stand on; unreachable designations (starting
  below grade on all edges) never start.
- Excavators carry a **preferred-product** setting
  (`ExcavatorTogglePreferredProductCmd`; wiki: "a preference can
  be specified inside the UI of individual excavators") — the
  only material-priority lever; there is no tower-level material
  priority (wiki, Mine Control Tower).

### 3.2 Placement & snapping — the pit is a grid laid on a geology (CORE)

1. **Snap the pit grid to the deposit region, outward.** Take
   the deposit's connected-region bounds from `/deposits` and
   round the pit rectangle *outward* to whole cells. Ore left in
   a fringe strip outside the designation — under the pit rim,
   under the access ramp — is ore you will never legally dig:
   the rim can't be re-designated once the pit is open below it
   without an unreachable-edge fight.
2. **The thin-tail exception (decision rule).** Snap outward
   *unless* a fringe cell's content doesn't pay for its own
   overburden. Both numbers are in the data: the cell's ore
   quantity (region/layer sums) vs the overburden tiles³ above
   the ore in that cell (estimator cut to the ore layer's top,
   from region heights + layer data). **RULE-OF-THUMB:** include
   the fringe cell if its ore quantity is at least comparable to
   its overburden volume in tiles³; skip knife-edge tails
   (a tile or two of thin ore smeared over many cells) and note
   them as sterilised in the mine plan rather than discovering
   them at the end.
3. **Margin and access.** Reserve one cell row on the access
   side between the pit rim and the district grade: that row is
   the ramp (ramp cells stepping ±1 per cell down into the pit)
   and the truck turning/haul strip. Excavators enter and leave
   along it; as benches deepen, extend the ramp down the pit
   wall in the same staircase rhythm (1 per cell — the snap rule
   leaves no other option).
4. **Bench geometry — follow the physics, then the contours.**
   A bench is a set of flat cells at one target height, worked
   as a strip. Two layouts:
   - **Straight rectangle benches** (deposit on flat ground):
     simplest grid, every cell identical; on a draped deposit
     some cells dig air-thin overburden and others bury the face.
     Best when the surface over the deposit is within ~1 tile of
     level.
   - **Contour benches** (deposit draped over a slope): each
     strip's target height follows the terrain — strips are
     still rectangles of cells, but successive strips step down
     with the hillside (steps of exactly 1 tile between adjacent
     strips keep the neighbour snap valid). More designations to
     manage; far less wasted cut, and the working face stays
     within the excavator's comfortable reach instead of
     alternating between buried and perched.
   Bench *height* (the drop between successive bench floors) is
   set by §0.3, not by the bucket: **≈2 tiles in undisturbed
   rock/ore** (the guaranteed-stable diff), **≈0.8 in disrupted
   material**, **≈0.5–0.6 in soil/overburden** — soil doesn't
   bench, it batters, so overburden comes off in wide shallow
   passes. With retaining walls at the face, a bench drop of up
   to **5 tiles** can be held (wall limit) — the standard
   deep-pit pattern: wall line at the face, dig 5 down, next
   wall line at the new floor set ~2 tiles out, repeat
   (community pattern, Steam thread in §1.4). **DERIVED** from
   the collapse bands + wall limit; it is the arithmetic the
   game will enforce whether you plan it or not.
5. **Tower placement.** The tower stands at the pit rim on the
   district side, never in the pit. Its control area must cover:
   the pit grid, the waste dump (§3.3 step 1), and the sorter/
   truck apron — one tower area means one governing set of
   dumping rules for the whole operation and one fleet pool.
   Keep the tower's own tiles out of every designation: a tower
   inside its own pit is a tower you demolish at bench two.

### 3.3 The effective method

1. **Designate the waste destination first.** No dump = trucks
   with nowhere to unload = the mine stops at the first bench
   (Guide 1). Site it per §1.2 (same district, not over ore).
2. **Read the deposit before touching it.** From `/deposits`:
   region bounds, per-material totals, and the layer stack —
   compute the **overburden ratio**: overburden tiles³ above the
   pay layer ÷ pay quantity. A deposit whose overburden is
   several times its ore is a quarry that happens to contain
   ore; decide with the ratio in view, and record it.
3. **Snap the pit** per §3.2 (outward to the region, thin-tail
   rule applied, ramp row reserved).
4. **Strip the overburden bench by bench, top-down.** First
   bench at rim height; each next bench one physics-step down
   (§3.2.4). Overburden goes to the waste pad; if the overburden
   *is* a usable material (dirt for farms/compost chains, rock
   for crushing), route it to storage instead of the dump —
   mixed loads are what the Ore Sorting Plant is for, and with
   the sorting rule on, unsorted mixed loads can't be dumped at
   all (§1.1).
5. **Work the pay layer with enough fronts.** One excavator per
   cell (§0.5): N excavators need ≥N active cells in reach.
   Set each excavator's preferred product to the pay material so
   its cell choice biases to ore-bearing cells over barren ones.
6. **Keep the haul honest.** Trucks assigned to the tower (and
   only those) serve its excavators (wiki). If excavators wait
   on trucks, add trucks or shorten the haul (move the sorter/
   storage apron to the rim) — never respond by adding
   excavators to a truck-bound pit.
7. **Close out by the numbers.** Re-read `/deposits` between
   benches: the region's remaining quantity is the scoreboard.
   When the pay layer is gone, the pit's remaining designations
   are just flattening work — reclassify them (Guide 2) or
   remove them so the fleet releases.

### 3.4 Failure modes

**Failure mode** — The mine stops with every truck full.
**Symptoms** — Excavators idle with full buckets' worth of work
in front of them; trucks parked full at the tower.
**Prevention & fix** — No legal unload: dump product not in the
governing allowed set, dump designation fulfilled/removed, or
storages full. The mine's sink failed, not its source. (Wiki:
"Trucks will return to the tower and idle if they are unable to
unload their cargo".)
**Source** — <https://wiki.coigame.com/Mine_Control_Tower>

**Failure mode** — Designated, assigned, and never dug.
**Symptoms** — `NumberOfJobsAssigned` = 0 on fresh cells;
excavators parked at the tower; red edges in-game.
**Prevention & fix** — Unreachable designation: target plane
starts below terrain on all edges (tutorial). Open the pit with
an entry bench at rim height on the access side and step down
via ramp cells (§3.2.3) instead of designating the deep floor
directly.
**Source** — official tutorial (translation hub);
`TerrainDesignation.IsReadyToBeMined` (code).

**Failure mode** — The face keeps eating the bench.
**Symptoms** — Slumped material at the toe re-dug every pass;
the bench floor widens while the face retreats; ore at the face
edge dilutes into disrupted mixed material.
**Prevention & fix** — Bench drop exceeds the material's stable
diff (rock 2.0 guaranteed; disrupted 0.8; soil ~0.6). Cut bench
height to the band, or hold the face with retaining walls (≤5).
Slumped material is not lost — it is re-handling cost, paid per
slide (each slide moves half the excess diff).
**Source** — `TerrainPhysicsSimulator`, `TerrainMaterialsData` (code).

**Failure mode** — Rich-looking pit, poor yield.
**Symptoms** — Weeks of digging, mostly dirt/rock in the trucks;
ore trickles.
**Prevention & fix** — The overburden ratio was never computed:
the surface layer showed ore colour while the pay layer is thin
and deep. `/deposits` totals + layer stack answer this *before*
the first designation — region quantity ÷ tiles tells the grade;
skip or defer deposits whose ratio fails §3.3 step 2.
**Source** — recon 11 layer model; `/deposits` route (SPEC XIX).

**Failure mode** — Ore stranded under the rim.
**Symptoms** — Late in the pit's life, `/deposits` still shows a
ring of ore quantity around the worked area that no designation
can legally reach.
**Prevention & fix** — The pit grid was snapped *inside* the
region bounds (§3.2.1). Prevention is the outward snap; the cure
— re-benching the rim — costs more than the ore is usually
worth. Write stranded fringes off deliberately, in the plan.
**Source** — DERIVED from `/deposits` region geometry + the
unreachable-edge rule.

**Failure mode** — Excavators queue, cells idle.
**Symptoms** — Two excavators contest one cell while
neighbouring cells sit unworked; throughput ≈ one excavator.
**Prevention & fix** — One excavator per cell at a time
(tutorial). Open more cells (widen the active bench) rather
than stacking machines; match excavator count to active fronts.
**Source** — official tutorial (translation hub).

### 3.5 With our data

The full pre-flight, all from bridge v0.6.5, before any
designation is placed:

1. `/deposits` → pick the region: bounds, totals, per-material
   quantities (depletion-exact).
2. `coi terrain region` over the snapped pit rectangle →
   surface heights; estimator cut to the pay layer's top =
   overburden tiles³; **ratio = overburden tiles³ ÷ pay
   quantity** recorded in the plan.
3. `/designations` → confirm the waste dump exists, is
   unfulfilled, and has jobs assigned; confirm no older
   designation overlaps the pit rectangle (overlapping areas are
   how contradictory targets happen).
4. During the dig: `/deposits` deltas between snapshots
   (store schema v4 keeps `deposit_state` per snapshot) are the
   mine's production ledger — ore out and overburden out, per
   material, with no counters to trust on faith.
5. The one thing still not sensed (SPEC XIX): an individual
   excavator's *current* digging target. Job counts per
   designation are visible; per-vehicle assignment is inferred
   from machine states, not read.

---

## Community practice (from the 2026-11-06 mod survey)

**This section is community practice, NOT code-verified.**
Everything above it in these guides was traced to game code;
what follows is the distilled method of the community's most
used mining mod — Kayser's **Auto Terrain Designations** (ATD,
49.5k Hub downloads / 89%, MIT source) — and of the classic
manual guide tradition (SamuraiProgrammer's "Retaining Walls:
An Exhaustive Guide" and the Steam mining threads). Full
citations and the survey itself: `MOD-SURVEY.md` §5.

- **Follow the contour, in cells.** Plan the pit as a connected
  region of 4×4 cells shaped by the deposit, not a rectangle
  dropped on it: scan, filter, connect, then ramp, then place.
  Fill holes and corridor-connect separated regions, with
  corridor clearance sized to the largest vehicle (2 for
  Mega-class). Keep two independent ramps with turning room.
- **Filter before you dig.** Thin "ore spikes" — isolated
  columns reaching far below the deposit — must never set the
  pit floor; gate cells by ore quality (purity, ore height,
  bottom density) and set both a relative depth cap and an
  absolute floor elevation.
- **Flatten the floor to the median**, not the deepest tile,
  and cap the height difference between neighbouring cells:
  lower caps give smoother, safer benches; higher caps follow
  rougher deposits more aggressively.
- **Bench in 5-level terraces.** One retaining-wall ring per
  5 levels (or part thereof), stepping one section inward per
  terrace; setback = ⌈depth ÷ 5⌉ + 1 sections from the
  outermost ore column. Centre walls in their section; fewer
  corners, squarer pits. (The 5 matches the code-verified
  retained-height limit in §0.4 — community practice and code
  agree here.)
- **Ramps are run-length, not gradient**: one section of run
  per level of depth, budgeted against the ore the ramp
  sterilises, and reusable as the next deposit's access.
- **Sequence top-down and evenly**: surface pass → complete
  wall ring → next bench with a two-section berth inside the
  ring above. Never mine beside an unfinished wall; never sink
  one side first. Backfill mistakes with rock, not dirt.
- **Expose the face in shallow passes** — dig flat, find the
  shape, ramp down 3–4, dig out, repeat — running overburden
  and ore as separate priorities (ideally separate towers) so
  ore excavators stay on high-yield ground.
- **Pair every pit with a local sink.** Dump overburden into
  the adjacent worked-out hole or a levelling designation
  inside the same tower zone before hauling to storage (the
  policy the Smart Mining Dump mod automates).
- **Strip mining (Bagger 288 doctrine):** a continuous miner's
  stance is its floor — it never digs below the level it stands
  on — so pre-level the strip and leave room ahead of the
  cutting face and behind the discharge side.

Where community practice and the code traced in §§0–3 agree
(5-tile retained height, 49-cell tower areas, one excavator
per square), the code remains the authority; where they
differ, treat the community figure as a starting heuristic to
validate against /terrain and /deposits, not as fact.

---

## Notes on sources and gaps

- Traced from code in this round: designation cell geometry and
  snapping (`TerrainDesignation`, `DesignationDataFactory`),
  collapse model + per-material bands (`TerrainPhysicsSimulator`,
  `TerrainMaterialsData`), building-stability on landfill
  (`UnstableTerrainValidator`), retaining-wall limit and
  placement window (`RetainingWallsData`), excavator capacities /
  reaches / dig thicknesses / ocean depth (`ExcavatorsData`),
  dumping-is-a-truck-job (`DumpingJob`), tower assignment model
  (`ExcavatorJobProvider`, `MineTower`, tutorial strings),
  ocean dump ceiling (`TerrainDesignation.MAX_DUMP_HEIGHT_IN_OCEAN`).
- Wiki-sourced: tower control-area max (49 designations/side),
  retaining-wall variant costs, port pad heights (+2/+5),
  one-cell-one-height loose-unit range (24–42, older density
  table — treat as orientation, geometry from code is exact).
- Community-sourced and labelled in place: tower-vs-global
  dumping-settings precedence; the 5-down wall staircase pattern.
- **Not verified** (named, not papered over): the Mega
  excavator's exact per-distance dig-thickness array (runtime-
  initialised in the proto; the T1/T2 pattern suggests five
  steps descending to 0.5); the numeric grade behind
  `SteepnessPathability.SlightSlopeAllowed`; the limestone
  collapse band (not individually read); which UI tool direction
  maps to the factory's `preferInitialBelowTerrain` flag (the
  factory contract itself is code-verified); the exact trigger
  for an excavator dumping directly rather than loading a truck
  (its dump animations exist; the job path traced in this round
  assigns excavators mining jobs only).
