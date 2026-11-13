# Dynamic Fleet — mining-truck dispatch design

Status: DESIGN (2026-11-13). No code, no live applies.
Philip's idea: an AI can micro-manage far more intensively
than a human — take trucks on and off mining assignments
aggressively as conditions change, instead of a fixed count
per Mine Control Tower. This document establishes what can
be sensed, what can be actuated, the bounded control policy
that makes "aggressive" safe, and the staged plan. Evidence
labels: VERIFIED = read from the shipped 0.8.8 assemblies
(ilspycmd, refs/) or a real capture this round;
UNVERIFIED = plausible surface, recon step named. Assembly
trail: bridge/dev/MODDING-NOTES.md recon round 16.

## 0. The one-paragraph verdict

The actuation channel already exists and is safer than
expected: the type+count commands draw **only from the free
pool** (they cannot steal a truck from another tower, by
construction), and assignment state is ordinary serialized
game state produced by vanilla input commands — the same
channel the tower UI uses. The sensing does **not** exist
yet: CoiBridge has no vehicles/assignments route, so one new
read-only bridge route (`/coi/v1/fleet`) is the gate for
everything. The policy must be a bounded dispatcher, not a
fast toggler, because every move has a real cost the game
imposes: unassign/assign **cancels the vehicle's current
jobs**, and every truck on a tower is a truck removed from
general logistics entirely.

## 1. Sensing map

### 1.1 Available today (no new code)

| Signal | Source today | Notes |
|---|---|---|
| Tower existence / count / position | bridge `/coi/v1/census` (byProto: `MineTower` ×9 in the Dragon Isle capture) + `/coi/v1/topology` positions | No per-tower assignment or state data |
| Deposits remaining (per material, map-wide) | bridge `/coi/v1/deposits` | Not attributed to a tower |
| Mining designations (origins, corners) | bridge `/coi/v1/designations` | Not attributed to a tower |
| Storages + fill levels | bridge `/coi/v1/storages` | Sink headroom computable only if the tower's sinks are known (they are not, today) |
| Product stocks/flows | vanilla `/api/v1/products*` | Economy-level corroboration only |

### 1.2 Needs the new read-only route: `/coi/v1/fleet` (CoiBridge 0.6.7)

One route, three sections. Every field below names its
verified surface (recon round 16) unless marked UNVERIFIED.

**Towers** — enumerate `MineTower` via `IEntitiesManager`
(the bridge's existing pattern; `MineTowersManager` also
exists):

- `id`, `proto`, `pos` — entity basics, as other routes emit
- `area` — `MineTower.Area` (PolygonTerrainArea2i, public).
  Emit its rect/polygon so the companion can attribute
  designations + deposits to this tower by containment
  (companion-side join; attribution is geometric inference,
  labelled as such wherever shown)
- `zoneMask` — `MineTower.ZoneMask` (public)
- `excavatorsTotal` / `trucksTotal` —
  `AssignedExcavatorsTotal` / `AssignedTrucksTotal` (public)
- `assignedOutputs` / `assignedInputs` — entity ids from
  `AssignedOutputStorages` / `AssignedInputStorages`
  (public) → the companion joins `/storages` fills for sink
  headroom
- per-excavator rows (from `AllAssignedExcavators`):
  `id`, `proto` (capacity: Small 6 / T2 20 / Mega 60 —
  wiki, see §6), `state` (`Excavator.State`:
  Idle/DoJob/WaitingForTruck/LoadTruck/WaitingForShovel/
  GettingUnstuck), `stateAgeSteps` (current sim step −
  `StateChangedOnSimStep`; sim-step source inside the
  bridge: UNVERIFIED — recon step: resolve
  `ISimLoopEvents` in the route ctor like other managers,
  else omit age and let the companion diff successive
  polls), `cargoQty` + `cargoProduct` (`Cargo`),
  `isFull` (`IsFull`), monthly productivity
  (`IEntityWithProductivityCounter.OngoingMonthlyData` —
  exact member shape UNVERIFIED; recon: mirror how the
  machines route reads machine counters, or defer to v2)

**Vehicles** — enumerate `IVehiclesManager.AllVehicles`
(`IReadOnlySet<Vehicle>`; the manager is resolvable from
mod context — QoSVehicleRecovery already consumes it):

- `id`, `proto` (e.g. `TruckT2`, `TruckT2H`, `ExcavatorT2`
  — real capture strings), `kind` (truck/excavator/other
  by runtime type)
- `assignedTo` — `Vehicle.AssignedTo` (public Option;
  emit owner entity id or null). This one field yields the
  **type-assignment quotas**: group by (owner, proto) —
  there is no separate quota object in the game; the UI's
  counts ARE the concrete assignments
- `canBeAssigned` — `Vehicle.CanBeAssigned` (free-pool
  membership, exactly the predicate the type-assign
  command will apply)
- `hasTrueJob` — `Vehicle.HasTrueJob` (public); current
  job type name: UNVERIFIED member path (jobs live on the
  DrivingEntity base; recon step: read `Jobs.CurrentJob`
  in a scratch build and confirm accessibility — v1 can
  ship with `hasTrueJob` alone)
- trucks: `cargoQty`, `cargoProduct` (`Truck.Cargo`),
  `capacity` (`Truck.Capacity`; wiki cross-check:
  Pickup 20 / Truck 60 / Haul 180)
- `needsRefueling`, `isFuelTankEmpty` — public Vehicle
  summaries (the tank itself is private; levels beyond
  these two booleans are NOT readable — do not promise a
  fuel gauge)
- `isEnabled`, `isSpawned`, position (entity basics)

**Pool summary** (computed by the companion from the rows,
or emitted by the route): free trucks by proto =
`canBeAssigned && kind=truck`; assigned counts per
competing owner — towers, `TreeHarvester`s and
`FuelStation`s also implement `IEntityAssignedWithVehicles`
(VERIFIED) and draw on the same pool; the route should emit
rows for those owners too (id + kind), even though the
policy only ever *acts* on towers in v1.

### 1.3 What sensing cannot give us (state it, don't fake it)

- No per-tower "desired trucks" setting exists in the
  game — the desired count is the dispatcher's own output.
- No queue-length counter at an excavator: trucks queue
  via `ITruckQueueObserver`; queue pressure must be
  inferred from truck states near/assigned to the tower
  (idle-with-no-true-job assigned trucks ≈ surplus;
  `ExcavatorState.WaitingForTruck` dwell ≈ deficit).
- Deposit/designation → tower attribution is geometric
  (area containment), not a game fact. Where areas overlap
  or a designation sits outside every area, the policy
  treats the tower's remaining-work estimate as unknown.
- Fuel is two booleans, not a level.

## 2. Action map (§12)

All four commands are in the wired catalogue
(`coi/src/catalogue.rs`, generated). VERIFIED semantics
from `VehicleCommandsProcessor` + the entity classes
(recon round 16):

| Command | Args (web names per catalogue) | Semantics that matter |
|---|---|---|
| `AssignVehicleTypeToEntityCmd` | `vehicleId`: **proto id string** (e.g. `"TruckT2"`), `entityId`: tower id, `count`: int (cap 100), `zoneId`: int or null | Assigns up to `count` FREE vehicles of that proto to the tower. The game picks: nearest to the tower, preferring enabled and **empty** trucks (distance² + 10⁷ if disabled + 10⁶ if loaded). **Cannot steal** (`CanBeAssigned` requires unassigned). **Silent partial fulfilment**: success is returned even if 0 were free. Fails only on unknown proto/entity or `CanVehicleBeAssigned(proto)=false` for the tower (e.g. tanker proto — towers accept loose trucks + any excavator proto only) |
| `UnassignVehicleFromEntityCmd` | `vehicleId`: proto id string, `entityId`, `count`, `zoneToAssign`: int or null | Removes up to `count` vehicles of that proto from the tower; **the game chooses which** (last matching in its list — order UNVERIFIED). Each removal cancels that vehicle's jobs; cargo stays aboard. `zoneToAssign` has a decompiled quirk (appears to clear the zone instead of setting it) — **always send null in v1** (UNVERIFIED, live-test item) |
| `AssignVehicleToEntityCmd` | `vehicleId`: concrete vehicle EntityId, `entityId` | **Steals**: if the vehicle is assigned elsewhere it is unassigned from there first; jobs cancelled either way. No `CanVehicleBeAssigned(proto)` check at the processor (the entity's pool add is the only gate). Reserve for supervised, named-vehicle moves — never the autopilot's tool |
| `UnassignVehicleCmd` | `vehicleId`: concrete vehicle EntityId | Frees that one vehicle (jobs cancelled, cargo kept). Idempotent no-op if unassigned. The surgical counterpart — useful to free a *specific* idle truck the type-command might not pick |

**Entity-id meaning**: `entityId` is the tower's EntityId
(the same id space as topology/census); `vehicleId` is a
proto string for the type pair and an EntityId for the
concrete pair — the catalogue's arg names collide, so the
§12 prepare layer must label them unambiguously in the UI
("vehicle type" vs "vehicle").

**Read-back (the only proof)**: command success proves
nothing for the type pair (silent partial fulfilment).
After an apply, re-poll `/coi/v1/fleet` and diff:
(assigned count for (tower, proto)) and the concrete
`assignedTo` of affected vehicles. Verdict vocabulary
reuses the designation pattern: LANDED (delta == requested)
/ PARTIALLY LANDED (0 < delta < requested) / NOT LANDED
(delta == 0). The §12 audit row records requested vs
landed, never just "success".

**Sandbox-first status**: all four are `wired`, none is
`verified`. First live use of each is a scratch-save
shakedown item (§4), including: type-assign partial
fulfilment read-back, type-unassign's vehicle choice,
concrete-assign steal cost (job abandonment), the
`zoneToAssign` quirk, and what a freed loaded truck does
with its load (code says the buffer is untouched; the
delivery behaviour is the game's general logistics —
observe it live before trusting it in policy).

## 3. Control policy — the bounded dispatcher

Design stance: Philip's "aggressively" is about
*responsiveness of the decision loop*, not move frequency.
The game punishes churn (every move cancels a job; a moved
truck drives empty to its new tower). So: evaluate often,
move rarely, explain every move.

### 3.1 Demand estimate per tower (per truck proto class)

Per tower, from the latest `/fleet` poll (poll cadence:
one snapshot per evaluation; evaluations at most every
~30 s of real time, and only while the game is unpaused —
the companion already knows pause state):

- **Starvation evidence**: excavators in `WaitingForTruck`
  with dwell ≥ threshold (default: state age ≥ 20 s of
  game time, or present in 2 consecutive polls) — weight
  by excavator tier (a starving Mega at bucket 60 starves
  10× a Small at 6).
- **Throughput balance**: excavator fill rate vs haul
  rate. Load time per truck ≈ (truck capacity / bucket) ×
  shovel cycle; haul cycle ≈ measured round-trip (the
  companion can estimate from successive polls: time
  between a truck leaving the excavator full and returning
  empty). Target trucks ≈ ceil(haul cycle / load cycle)
  per working excavator — the classic keep-the-shovel-fed
  ratio, computed from proto constants (capacities VERIFIED
  via wiki §6) until live cycle measurement exists.
- **Remaining work**: designations + deposits attributed
  to the tower's area (§1.3 caveat). A tower whose
  attributed remaining is ~0 is a *donor*, not a demander,
  whatever its excavators say (they will go Idle).
- **Sink headroom**: the tower's assigned output storages'
  free capacity (join `/storages`) vs current inflow. No
  headroom → do not add trucks (they would queue at a full
  sink — the tower's own idle-truck symptom). Headroom
  unknown (no assigned outputs + `AllowNonAssignedOutput`
  false) → treat as constrained, recommend only.

### 3.2 Supply evidence

- Free pool by proto (`canBeAssigned`): the only supply
  the type-commands can draw on.
- Surplus evidence at a tower: assigned trucks with
  `!hasTrueJob` sustained across polls (vanilla's own
  `HasUsefulIdleVehicle` notion, per-vehicle), or trucks
  queued while all excavators are Idle/DoJob with no
  WaitingForTruck.
- **Global competition check**: before any add, the free
  pool must stay above the reserve (§3.4) AND no other
  tower may be in starvation with a stronger claim
  (priority: starving-with-work > starving > balance >
  surplus trimming). Forestry/fuel-station assignments are
  observed, never raided (concrete steal is excluded from
  the dispatcher entirely; type commands can't raid
  anyway).

### 3.3 Move shape

- Moves are **type+count quota adjustments only**:
  `UnassignVehicleFromEntityCmd` at the donor,
  `AssignVehicleTypeToEntityCmd` at the receiver, each as
  its own §12 prepare/apply (in stage (b), one click each;
  the pair is presented as one recommendation with two
  applies, donor first so the pool actually contains the
  truck before the receiver asks).
- One truck per move per tower per evaluation in v1
  (count=1). Aggression lives in re-evaluation cadence,
  not in batch size.

### 3.4 Bounds (defaults; all config in coi.toml `[fleet]`)

- `reserve_free_trucks` per proto class: never assign the
  pool below this (default 2) — construction and general
  logistics keep breathing room.
- Hysteresis: add threshold strictly above remove
  threshold (e.g. add when starvation score ≥ 1.0 sustained,
  remove when surplus score ≥ 1.0 sustained AND starvation
  score == 0); a tower inside the dead band is left alone.
- Minimum dwell: a truck moved to a tower cannot be moved
  again for `min_dwell` (default 5 game-days or 10 min
  real time at current speed, whichever the companion can
  measure — game-days preferred) unless the tower's
  remaining work hits zero.
- Cooldown per tower after any move (default: 2
  evaluations skipped).
- Max moves per evaluation across all towers (default 2);
  max moves per game-day (default 8) — a hard churn ceiling.
- Sink gate (§3.1) and remaining-work gate are vetoes, not
  scores.

### 3.5 Manual-ownership rule

The dispatcher NEVER fights Philip:

- If a tower's assigned count *increases* outside the
  dispatcher's own applies (diff between polls not
  explained by its audit log), that tower is marked
  **hand-touched** and the dispatcher stops recommending
  removals from it for the rest of the session (adds still
  allowed if it starves). A page toggle per tower
  ("automation: on/off", default ON in stage (b) display,
  stored companion-side) gives Philip the explicit switch;
  OFF towers are observe-only.
- Philip's assignments via the game UI are indistinguishable
  from dispatcher applies except via the audit log — hence
  the diff rule above, stated plainly in the UI.

### 3.6 Fail-closed

- No `/fleet` payload, stale payload (>2 poll intervals),
  bridge absent, game paused, or tower attribution unknown
  → **no recommendations marked actionable** (stage a/b)
  and no applies prepared. The page shows the staleness,
  exactly like other bridge-dependent pages.
- After an apply whose read-back says NOT LANDED twice in
  a row for the same (tower, proto), the dispatcher stops
  proposing that pair and surfaces why ("pool empty",
  "proto not assignable to towers", …) instead of retrying.

### 3.7 Audit / explanation

Every recommendation and every move writes a companion-side
record (the §12 audit trail for applies; a fleet log for
recommendations): timestamp/tick, tower, proto, direction,
count, the evidence snapshot that justified it
("ExcavatorT2 #n WaitingForTruck for 2 polls; 0 idle
trucks at tower; free pool TruckT2 = 5 (reserve 2);
sink StorageLooseT2 41% free; donor: Tower #m surplus 2
idle"), and the read-back verdict. The page's per-move
"because…" line is rendered from this record — an
unexplained move is a bug, by design rule.

## 4. Implementation plan — smallest shippable stages

### Stage (a) — sensing + recommendations only (no applies)

- **CoiBridge 0.6.7**: new file `bridge/CoiBridge/
  BridgeRoutesFleet.cs` with `[WebRoute("/coi/v1/fleet")]`
  per §1.2; version bump + readme; build via
  `bridge/vm-build/build.sh`; package lint.
- **Stub**: extend `tools/stub_game_server.py` capture set
  with a `bridge-fleet.json` fixture (hand-authored against
  the route's schema, 2 towers, mixed states incl. a
  WaitingForTruck excavator and a loaded free truck) under
  `dev/captures/live-synthetic-v065/` + a fixnow variant.
- **Companion**: `coi/src/fleet.rs` (payload types,
  attribution join, demand/supply scoring per §3, pure
  functions over the payload) + unit tests on fixtures
  (starving tower recommends +1; full sink vetoes; unknown
  attribution → observe-only; hysteresis dead band → no
  recommendation). Page: `/fleet` under Workshop —
  per-tower cards (excavators with states, trucks, quota
  now vs recommended, evidence lines), pool summary,
  recommendations list with "because" lines, and the
  honest absent/stale states. Route + page check script
  `tools/check_fleet_page.sh` in the standard sequence.
- **Shakedown (Philip, scratch save)**: install 0.6.7,
  capture a real `/fleet` payload from a mining-active
  save; diff against schema; settle the UNVERIFIED items
  (sim-step source, job-name member, productivity member)
  from the real payload before stage (b).

### Stage (b) — supervised one-click applies

- **Companion**: §12 action pair in `coi/src/serve.rs`
  following the existing per-command prepare pattern
  (`do_prepare_fleet_assign` / `do_prepare_fleet_unassign`
  + the apply path): prepare shows tower name, proto,
  current → target count, the evidence record, and the
  explicit warnings (jobs cancelled on affected vehicles;
  game chooses which concrete vehicles; partial fulfilment
  possible). Apply → command → **mandatory read-back poll**
  → verdict (LANDED / PARTIALLY / NOT LANDED) written to
  the audit row.
- Catalogue: the four commands' entries already exist
  (`wired`); flip to `verified` only per-command after its
  scratch-save proof, via `tools/gen_catalogue.py` +
  capture, never by hand-editing `catalogue.rs`.
- **Scratch-save shakedown script** (Philip drives, agent
  reads back): (1) type-assign +1 to a tower with a free
  truck → verdict LANDED, truck drives to tower;
  (2) type-assign +5 with only 2 free → PARTIALLY LANDED
  with delta 2; (3) type-unassign 1 → which vehicle left
  (record it — settles the order question); (4) unassign a
  tower whose truck is mid-haul loaded → observe load's
  fate; (5) `zoneToAssign` probe on a zoned save (only if
  Philip uses zones); (6) concrete steal once, deliberately,
  to confirm the job-abandonment cost is what the code says;
  (7) save + reload after moves (expected boring: vanilla
  commands, serialized assignments — proof still owed).

### Stage (c) — bounded autopilot (ONLY on Philip's later, explicit authorisation)

- A companion-side loop behind `[fleet] autopilot = false`
  (default off, per-tower opt-in on the page), running the
  §3 policy inside its bounds, using the same prepare-less
  internal apply path as stage (b) but writing the full
  §3.7 record per move, with a global stop (config +
  page button) and the fail-closed rules of §3.6 as hard
  gates. No new game-side code: if the §12 channel proves
  insufficient for stage (c) cadence, that is a new design
  round — a QoS-side executor is NOT pre-authorised by
  this document (see §5).

## 5. Risks / unknowns

- **Job cancellation is the price of every move**
  (VERIFIED): a mid-haul truck freed or stolen abandons
  its current job; cargo stays aboard (VERIFIED for
  unassign) and re-enters general logistics — its delivery
  then competes with everything else. The dwell/cooldown
  bounds exist precisely because moves are not free.
- **Pathing churn**: trucks reassigned across the map
  drive empty; with several towers this can look like (and
  be) waste motion. Mitigation: nearest-first is the
  game's own pick for type-assigns; the dispatcher's
  donor choice still matters — prefer donors whose surplus
  trucks are nearest the receiver (companion-side estimate
  from positions).
- **Partial loads**: `TogglePartialTrucksLoadCmd` exists
  (wired); a tower's trucks may run part-full by the
  player's own setting. The policy reads capacities, not
  load policy; it must not "fix" partial loading.
- **Fuel/maintenance**: an assigned truck still refuels at
  its fuel station (assignment doesn't change fuel
  logistics; `NeedsRefueling` is sensed so the dispatcher
  doesn't count a refuelling truck as idle-surplus —
  UNVERIFIED that refuelling shows as a true job; settle
  in stage (a) live capture). Maintenance/breakdown states
  arrive via the existing machines-route patterns; a broken
  truck is excluded from surplus by `hasTrueJob`/state
  evidence where visible, else by the dwell rule.
- **Mega-excavator geometry**: Mega (bucket 60, NavTolerance
  radius 3×) loads a Haul truck (180) in 3 cycles but a
  Pickup (20) can never be filled by one bucket cycle
  ratio-wise — proto-class matching matters: the demand
  model is per (tower × truck proto), never a single
  "trucks" number.
- **Zones**: logistics zones filter the free pool by mask
  (§2). A zoned fleet can make a tower's effective pool
  much smaller than the global count; the pool summary
  must be reported per zone-mask intersection, and
  `zoneId`/`zoneToAssign` stay null in v1 until the quirk
  is live-tested.
- **Save/serializer**: assignments via vanilla input
  commands are serialized by the game itself
  (`Vehicle.AssignedTo` is in the save format) — no
  QoS-style serializer exposure, no new write mod needed
  or proposed. The standing rule stands: if the command
  channel were ever proven insufficient, the fallback is a
  NEW design round, not a quiet mod.
- **Attribution risk**: deposit/designation→tower joins
  are geometric; overlapping areas can double-count
  remaining work and make two towers both look like
  donors. The policy treats contested designations as
  unknown-work (never as zero-work).

## 6. Sources

- Assemblies (refs/, game 0.8.8): `VehicleCommandsProcessor`,
  the four command classes, `AssignedVehiclesExtensions`,
  `IEntityAssignedWithVehicles`, `Vehicle`, `Truck`,
  `Excavator`, `MineTower`, `VehiclesManager.GetFreeVehicle`
  — trail in bridge/dev/MODDING-NOTES.md round 16.
- Wiki mirror (fetched 2026-11-06): Mine Control Tower
  (assignment exclusivity, idle behaviour, control area
  ≤49 designations); capacities — Pickup 20, Truck 60,
  Haul Truck (Dump) 180 loose-only; Small Excavator bucket
  6, Excavator 20, Mega Excavator 60.
- Captures: `dev/captures/live-synthetic-v065/bridge-census.json`
  (MineTower ×9, ExcavatorT2 ×23, TruckT2 ×92, TruckT2H ×60).
