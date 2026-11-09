# QoS build notes — round 1 (2026-11-04 evening)

Built: suite foundation + QoSChickens v0.1.0, per QOL-MODS.md's
architecture decision (small individual mods, one QoS suite) and
QOL-MOD-PREP.md §1 (chickens). Everything staged in `dist/`, no
uploads, no live-game contact. CoiBridge untouched.

## Artefacts

| File | Bytes | Contents |
|---|---|---|
| `dist/QoSCore-mod-0.1.0.zip` | 9,951 | `QoSCore/` (dll 7,168) |
| `dist/QoSChickens-mod-0.1.0.zip` | 20,050 | `QoSChickens/` (dll 13,824; **no** QoSCore.dll inside, by design) |
| `dist/QoS-suite-0.1.0.zip` | 32,199 | both folders + `QoS-SUITE-README.txt` |

Build: `qos/build/build.sh` — both projects **0 warnings / 0
errors** (net48, same toolchain as the bridge). Sources in
`qos/QoSCore/`, `qos/QoSChickens/`; build scaffolds in
`qos/build/{core,chickens}/` (one csproj per folder — see the
skill note on the shared-`project.assets.json` gotcha).

## QoSCore — as designed

`QoSLog` (prefixed suite logging + verbose flag), `SuiteRegistry`
(feature mods `Announce(id, version)`; logs suite composition),
`QoSCoreService` (applies core config: `verbose_logging`, default
false). Nothing else — no shared UI, no command bus, no state
store; those arrive only when a feature needs them (§3 may).

## QoSChickens — prep §1 refined design, implemented as specified

- Daily pass at `ICalendar.NewDayEnd` (post-birth, post-cull),
  moves issued as vanilla `MoveAnimalsCmd` through
  `IInputScheduler.ScheduleInputCmd` — the shipped path, shipped
  round-robin inside each move.
- Guards enforced by **exact simulation** of that round-robin
  (same peer enumeration order) before scheduling; only a
  predicted outcome passing every guard is scheduled, and the
  scheduled amount is the simulated amount. Guard list: receiver
  cap = min(capacity, `SlaughterLimit`); a farm at/above its cap
  gains zero; eligible = constructed + enabled only; nucleus
  floor on drains; per-day cap.
- One command per species per day; on guard failure the amount
  halves and retries within the pass, then the day is skipped.

### Deviations from the prep design (each with its reason)

1. **Master switch defaults OFF** (`enabled=false`), mode
   pre-selected to Equalise. The prep left the default "for
   Philip to pick" (Equalise vs Off); shipping the switch off
   with the gentlest mode pre-selected is the conservative
   reading of that open choice — first install is observable and
   inert until he enables it. One settings flip activates the
   prep's recommended mode.
2. **Growth-paused guard implemented as a species skip.** The
   prep says "skip such farms as receivers AND donors", but the
   shipped command's peer set is all constructed+enabled
   same-species farms and cannot exclude a peer (recon §1). Any
   move in a species with a growth-paused farm present would
   move that farm's animals against the player's setting. So the
   species is skipped that day (verbose-logged). Stricter than
   the prep text; the only faithful implementation over the
   shipped command path.
3. **Fill-first priority order comes from config**
   (`fill_first_farm_ids`, comma-separated entity ids). The prep
   says "Farm A first, then Farm B" without naming a source for
   the order; there is no in-game farm-naming UI to read, so the
   order is an explicit setting. Unconfigured/unparseable → log
   - no-op rather than an invented order.
4. **"Max per day" capped globally**, summed across species
   (prep implies a per-mod budget; with one shipped species the
   two coincide). Default 50 = one vanilla UI button-press.
5. **Cadence point is `NewDayEnd`**, not `NewDay`: the prep
   wanted "after the day's births/culls"; the day-end event is
   the one that guarantees both have run. No behavioural
   difference beyond ordering.
6. **Config via the game's own `config.json`/`ModJsonConfig`**
   (settings UI + save persistence) instead of a mod-folder JSON
   file — the prep/patterns showed the game mechanism exists
   (official README + PriorityManager's injected-mod pattern), so
   the suite uses it. Consequence: values persist per save; the
   readmes state this.

### NOT verified (needs first live install / first enabled days)

- Cross-assembly resolution of `QoSCore.dll` from QoSChickens
  through the manifest dependency (documented mechanism; the VM
  build cannot prove loader behaviour). If it fails live, the
  fallback is bundling a copy — rejected for now (duplicate-type
  hazard) — or merging chickens into core.
- `GetAllEntitiesOfType<AnimalFarm>()` enumeration order equals
  the order `FarmCommandsProcessor` materialises peers in
  (recon: INFERRED from the same shared API). If it differed, the
  simulation could mispredict by an animal or two at the margins
  — the guards still bound every outcome to safe ranges, but the
  first enabled days should be eyeballed against the log lines
  (each scheduled move logs its predicted spread).
- `IEvent` subscription from a service constructor (pattern
  copied from the game's own StatsManager) — compiled clean;
  confirm the "balancer armed" line appears in the log at load.

---

# QoS build notes — round 3 (2026-11-05): vehicle content recovery + preferred drop-off

Asks (QOL-MODS.md items 6–8): (6) stuck vehicles get their CARGO
recovered, not just the vehicle; (7) a train or its depot can pick
a PREFERRED drop-off for overload; (8) the same preferred-drop-off
routing applies whenever cargo is auto-emptied/cleared.

## Recon answers (full trails: qol-recon/06-vehicle-recovery.md, qol-recon/07-train-overload.md)

### (a) Vanilla vehicle recovery and cargo

- `RecoverVehicleCmd` → `Vehicle.TryRequestRecovery()` →
  `IVehiclesManager.TryEnqueueRecoveryJob`. Cost: flat **0.2
  Upoints** (`VehiclesManager.VEHICLE_RECOVERY_COST`, public),
  charged at enqueue via `TryConsume(UpointsCategories
  .VehicleRecovery)`, never refunded. Gate
  (`CanRecoverVehicle`, public) is exactly two checks: some depot
  `CanAccept(vehicle)`, and Unity affords 0.2. Cargo is never
  consulted.
- The recovery job **teleports**: 1 s wait, despawn in place,
  respawn at the closest accepting depot, drive out. **Cargo is
  completely preserved** — nothing in the chain reads or writes
  `Truck.m_cargoBuffer`; the truck exits the depot with its load
  byte-identical, assignment + config restored, prior jobs
  cancelled (its job provider re-plans). A failed enqueue still
  leaves the truck jobless (jobs are cancelled before the
  attempt) — so a policy mod must pre-check with
  `CanRecoverVehicle` before scheduling, never schedule blind.
- Truck cargo model: one private `ProductBuffer`, surfaced as a
  single `Cargo` `ProductQuantity` — one product per truck
  (a mixed-category product carries its member breakdown inside
  the same buffer). Partial loads are normal.
- Contrast: replacement/scrap call-ins DO empty the truck first
  (`ClearCargoImmediately`, StoreBuffer semantics) — recovery is
  the only depot path that keeps the load aboard.

### (c) Where emptied cargo goes — and why it cannot be steered

Every clear path (truck `TryDiscardCargo` / `ClearCargo
Immediately`, wagon `DestroyCargo()`, replacement/scrap) routes
through `AssetTransactionManager.StoreBuffer` — **no overload
takes a destination**. Non-mixed: discardable products are
destroyed (`DestroyReason.Cleared`, stats); everything else goes
to registered receiver buffers in global-priority order — these
are the real buffers of real `Storage` entities (each registers
itself at priority 10), remainder to the first overflow storage
(the shipyard), destroyed only if no overflow exists. There is
no abstract pool: "into the economy" = physically deposited in
ordinary storages, drawable by ordinary logistics afterwards.
Redirect to a NAMED storage is not buildable on public surfaces:
`IProductBuffer.TransferTo` is public and the target side is
public (`StorageBase.Buffer`), but a truck never exposes its
buffer (`Truck.CargoBuffer` is internal; no interface accessor),
and the wagon equivalent (`SendAsMuchAsTo`) is internal too.
The one composite that could land truck cargo in a chosen
storage — public `Truck.TakeCargo(...)` + the target buffer's
`StoreAsMuchAs(...)` — has no shipped caller using it as a
transfer pair and unverified stats accounting: recorded as a
research lead, NOT built.

### (b) Train overload — no seam exists (evidence)

- Network jobs are sized to fit at dispatch (min of source
  available, destination free capacity, train capacity). Overload
  happens only when the world changes en route: the train then
  WAITS at the demand station (fullness merely throttles the
  per-tick partial transfer), force-departs still loaded after
  `IDLE_TIME_BEFORE_NETWORK_FORCE_DEPART` = 5 days, and
  `TrainNetworkJob.Finalize(false)` completes the job with NO
  emptiness check — partial delivery with leftover aboard is
  vanilla behaviour.
- Leftover makes the train a "lost train": `TrainNetwork
  .dispatchLostTrains` builds a delivery-only job to the single
  destination whose free capacity >= the WHOLE quantity aboard
  (all-or-nothing, no split), lowest per-product
  `NetworkPriority` wins; if none qualifies, the loaded train is
  parked at a waiting bay indefinitely. No timeout, no discard,
  no player prompt.
- Destination choice is a closed dispatcher scoring function
  (NetworkPriority 0–15 per product per station, reservation
  JobPenalty, active/passive thresholds, free capacity/slots).
  There is NO per-train, per-line or per-depot preference input
  anywhere: not on `TrainNetworkJob` (fields are exactly
  Network/Train/Supply/Demand/Product/LoadedSoFar/Quantity/
  State), not on the station root (`ArrivalRules`,
  `LinkedStorages`, `AllowedFuels`, `DoNotSkipThisStation`,
  `TrainLimit` — arrival rules are per-(station, product)
  admissibility thresholds, incl. `RequireFullServiceOnArrival`,
  not routing), not on the network (station sets +
  `RefuelThreshold` only). Manual line schedule items likewise
  offer only pre-departure multi-root choice/balancing and
  skip-if-cannot-arrive — never a divert of a remainder
  (field inventory in recon 07, Q3).
- `TrainDepot` never unloads wagons (its product buffers are
  construction materials), and `RecoverTrainCmd` preserves cargo
  aboard exactly like vehicle recovery. The depot route is a
  dead end for this feature.
- The closest buildable train behaviour — a mod watcher issuing
  a follow-up delivery-only `TrainNetworkJob` to a configured
  station — needs internal/private job-line plumbing
  (`Train.SetNetworkJob` internal, `getOrCreateJobLine`
  private). That is new state + intervention, not a shipped
  surface; it breaks the suite's pattern and is NOT built.

## DESIGN — "preferred drop-off" as one routing concept (written before code)

Since no named destination is steerable on either vehicle or
train side, the concept is defined as an **ordered destination
policy** — a ladder every auto-recovery / auto-empty follows —
rather than a place:

1. **Aboard** — recovery preserves the load on the vehicle
   (strongest form of content recovery; the load re-enters the
   economy when the vehicle's jobs re-plan from the depot).
2. **The economy** — when a load demonstrably cannot be
   delivered, emptying routes it through the game's own
   StoreBuffer distribution: real storages in global-priority
   order, shipyard overflow last, destruction only where vanilla
   itself destroys (discardable products / no receiver at all).
3. **The dispatcher** (trains) — vanilla already owns overflow:
   lost-train re-dispatch by NetworkPriority. The player's only
   preference lever is per-(station, product) `NetworkPriority`
   (lower = preferred; set the intended overflow station lowest
   for that product and it wins any lost-train re-dispatch it can
   wholly accept). Per-train/per-depot designation does not
   exist and is not faked.

Designation granularity, answered honestly: per-truck automatic
(the truck's own state designates which ladder rung applies);
depot choice is vanilla's (closest accepting) and not
configurable; global defaults live in the mod's config. A named
storage/station setting is deliberately NOT shipped, because no
public surface could honour it — a config that pretends
otherwise would be a lie in the settings UI.

### Build scope for this round

- **QoSVehicleRecovery v0.1** (new feature mod on QoSCore):
  the ladder for trucks. Daily NewDayEnd pass (pattern cadence;
  a finer tick is a named extension, not v1). Stage 1 RECOVER:
  trucks flagged cannot-deliver, or navigation-stuck on the
  game's own sustained signals (`IsFailingToNavigateRepeatedly`,
  or `IsStuck` with `NavigationFailedStreak` >= config), are
  scheduled `RecoverVehicleCmd` — only after the prediction
  guards pass: `CanRecoverVehicle` (vanilla's own depot+Unity
  gate, checked first so a doomed attempt never cancels the
  truck's jobs for nothing), a Unity reserve floor
  (`CanConsume(floor + 0.2)`), a per-day recovery cap, and
  skip-while-scrap/replacement/recovering. Stage 2 EMPTY
  (default OFF): a truck THIS mod recovered that is
  cannot-deliver again on a later pass with cargo still aboard
  is scheduled `DiscardVehicleCargoCmd` — the load goes to the
  economy via StoreBuffer (ladder rung 2), the truck is freed.
  Mod-side marks (truck id -> pass) are session memory only;
  they expire when the truck empties (episode over) or after
  7 passes. Every scheduled action logs its prediction
  (vehicle, reason, cargo, cost, expected destination rung).
  Master switch OFF by default.
- **Train leg (items 7+8 for trains): NO CODE this round** —
  scoped down on the recon evidence above. Its honest future
  form is a companion advisory (detect leftover/foreign cargo
  sitting on a train across snapshots; surface it with the
  NetworkPriority lever explained) — the bridge does not expose
  lost-train/job state today, so even the advisory needs a
  data source decision first. Recorded here, not built.
- **Vehicle named-storage drop-off: NO CODE** — scoped down on
  the Q6 evidence (recon 06). Research lead recorded
  (TakeCargo composite) for a future verified build.

## AS BUILT — round 3 (2026-11-05)

Built exactly the scope above: **QoSVehicleRecovery v0.1.0**
(new feature mod on QoSCore) + suite pack **QoS-suite-0.2.0**.
Train leg and named-storage drop-off: no code, per the design's
evidence-based scope-down. Everything staged in `dist/`, no
uploads, no GitHub push, no live-game contact. CoiBridge
untouched.

### Artefacts

| File | Bytes | Contents |
|---|---|---|
| `dist/QoSVehicleRecovery-mod-0.1.0.zip` | 21,926 | `QoSVehicleRecovery/` (dll + manifest + config + readme; **no** QoSCore.dll inside, by design) |
| `dist/QoS-suite-0.2.0.zip` | 54,677 | `QoSCore/` + `QoSChickens/` + `QoSVehicleRecovery/` + `QoS-SUITE-README.txt` |
| `dist/QoS-suite-0.1.0.zip` | 32,199 | previous pack — NOT rewritten (sha256 verified identical before/after this build) |

Build: `qos/build/build.sh` (extended) — all three projects
**0 warnings / 0 errors** (net48; recovery project re-verified
standalone after a touch). Sources in
`qos/QoSVehicleRecovery/`; scaffold in `qos/build/recovery/`
(one csproj per folder, per the suite gotcha).

### QoSVehicleRecovery — as designed

- Daily pass at `ICalendar.NewDayEnd`; both actions issued as
  shipped commands through `IInputScheduler.ScheduleInputCmd`
  (`RecoverVehicleCmd`, `DiscardVehicleCargoCmd`) — no direct
  state mutation anywhere in the mod.
- Stage 1 prediction guards, in order per truck: skip
  destroyed/unspawned/being-recovered/scrap-bound/
  replacement-bound; trigger = cannot-deliver flag, or
  `IsFailingToNavigateRepeatedly`, or `IsStuck` with
  `NavigationFailedStreak` >= config; per-day cap;
  `CanRecoverVehicle` pre-check (so a refused attempt never
  cancels the truck's jobs for nothing); Unity reserve floor
  via `CanConsume(floor + VEHICLE_RECOVERY_COST)` using
  vanilla's public constant, not a hardcoded 0.2.
- Stage 2 (default OFF): marks are (truck id -> pass, product
  id) in session memory; fires only for a marked truck,
  cannot-deliver again, same product aboard, reserve floor
  checked against `VehicleCommandsProcessor
  .COST_TO_DISCARD_CARGO` (the game's own 0.1, read from the
  shipped constant). Stage 2 is evaluated before stage 1 so a
  repeat rescue cannot loop.
- Every scheduled action logs its prediction: truck, reason,
  cargo (product x qty), cost from the shipped constants
  (`Format1Dec`), expected outcome/rung.

### Defaults (all in Settings -> QoS Vehicle Recovery)

| Setting | Default | Note |
|---|---|---|
| `enabled` | **false** | master switch — ships inert |
| `recover_cannot_deliver` | true | stage 1 trigger A |
| `recover_navigation_stuck` | true | stage 1 trigger B |
| `min_navigation_failed_streak` | 3 | when the game's repeated-failure flag is unset |
| `max_recoveries_per_day` | 5 | = at most 1.0 Upoints/day of stage-1 spend |
| `unity_reserve_floor` | 5.0 | Upoints; applies to both stages |
| `empty_cargo_if_still_undeliverable` | **false** | stage 2 — opt-in |
| `log_actions` | true | one line per scheduled action, with prediction |

### Deviations from the design text (each with its reason)

1. **Stage-2 mark is per (truck, product), not per truck.**
   The design said "the SAME load"; implemented literally —
   the mark stores the product id and stage 2 requires the
   truck to still carry that product. A different load is a
   new episode (stage 1 may rescue and re-mark).
2. **Costs are read from the shipped public constants**
   (`VehiclesManager.VEHICLE_RECOVERY_COST`,
   `VehicleCommandsProcessor.COST_TO_DISCARD_CARGO`) instead
   of literals — if a patch re-prices vanilla, the guards and
   log lines follow automatically.
3. **Suite pack versioning decoupled from QoSCore.**
   `build.sh` previously versioned the pack by the core
   version (hence suite 0.1.0). The pack now has its own
   `SUITE_VER` (0.2.0): adding a feature bumps the pack, not
   the core. The script only ever writes the current
   `SUITE_VER` pack, so older packs in `dist/` stay intact.
4. **Empty trucks are also recovered when navigation-stuck**
   (cargo "none" in the log). Item 6 is cargo-focused, but a
   wedged empty truck blocks the same logistics; the
   cannot-deliver trigger is inherently loaded-only. Recorded
   here as a deliberate widening of stage 1's trigger B.

### NOT verified (needs first live install / first enabled days)

- `IVehiclesManager` / `IUpointsManager` constructor injection
  resolving in the service (both are the game's standard
  AsEverything globals, the same pattern class as the
  chickens service's dependencies — compiled clean; a failure
  would surface as one init error log line, mod inert).
- Cross-assembly QoSCore resolution through the manifest
  dependency (still the suite-wide first-install check from
  round 1).
- Behavioural eyeball on first enabled days: predicted vs
  actual recovery outcomes are all in the log lines by design.

# Recon addendum (2026-11-05) — priority re-fill for AssetTransactionManager deposits

Philip's follow-up to round 3: *"What about updating that
manager to introduce a priority re-fill concept?"* Full trail:
`qol-recon/08-priority-refill.md`. Recon + design only; the
rung choice is Philip's. Summary:

- **Ordering truth:** deposit order is an immutable per-buffer
  int fixed at buffer construction (`AssetTransactionManager.
  GlobalBuffer.Priority`, private struct, ascending sort
  recomputed per call, ties unspecified — `Array.Sort`).
  Machine inputs register at 5, **every storage at 10**,
  outputs/depot at 15; the scale is designed
  (`GlobalInputPriority` INCREASED 5 / DEFAULT 10 / LOW 15).
  No command/UI/config surface touches it. The entries are
  **serialized into saves** (priority included).
- **Rung A — steer the existing mechanism: POSSIBLE
  (recommended).** A policy pass re-registers a preferred
  storage's buffer on the **input side only** via the public
  `IAssetTransactionManager.RemoveGlobalInput/AddGlobalInput`
  at priority 9 (config 6–9) — the same public calls buffers
  make themselves; `CargoDepotBuffer.SetIsInput` is the
  shipped runtime re-registration precedent. All deposit
  flows (discards, `DestroyCargo`, refunds, loot, mixed)
  steer; truck logistics and global draws do not (draws stay
  untouched precisely because the output side is never
  re-pinned). Costs: pins persist in saves while active
  (restore-on-disable pass mitigates; first suite feature
  whose settings outlive it in save data), and it is the
  suite's first direct-service-API mutation (vs
  shipped-command scheduling).
- **Rung B — DI substitution: POSSIBLE-WITH-COSTS, strictly
  dominated.** The door is open (mods' `RegisterDependencies`
  runs last; last-registered impl wins absent a preference;
  consumers are interface-typed incl. `EntityContext`) — but
  one core consumer (terrain-surface path) injects the
  concrete class, and substitution puts a mod type inside
  every save's core economy graph: removal orphans saves.
  Zero capability over rung A. Do not take.
- **Rung C — Harmony on `StoreProduct`/`StoreBuffer`:
  POSSIBLE-WITH-COSTS, reserve.** Two patch points cover all
  deposits; Harmony is mod-bundled only (DoubleQoLMod,
  Lib.Harmony 2.2.2). It is the only rung with true custom
  distribution (splits/routing), at the price of the suite's
  first Harmony dependency + per-patch re-verification of a
  core economy method. Not needed for priority order.
- **Second reading (pull):** "re-fill" as trucks keeping a
  designated storage filled is already fully supported —
  `Storage.ImportPriority`/`ExportPriority` (default 8, via
  `SetCustomPriorityCmd` "ImportPrio"/"ExportPrio") + slider
  percents feed `VehicleBuffersRegistry` strategies. That is
  QoSStoragePolicy (survey #1) with priority presets added;
  it composes with rung A (disjoint flow systems).

## AS BUILT — round 4 (2026-11-05): productivity prerequisite + QoSWorldMap

Two work items in one round, per QOL-MOD-PREP §5 and §2.
Everything staged in `dist/`, no uploads, no GitHub push, no
live-game contact. CoiBridge untouched.

### Item 1 — productivity prerequisite (companion, Rust)

Built exactly what prep §5 names as the prerequisite, plus the
κ-free estimator that can ship on it:

- **Store schema v2** (`PRAGMA user_version`; v1 files upgrade in
  place, verified against a copy of the real coi.db — its single
  snapshot, tick 2787522 / 388-2-27, survives intact and honestly
  carries zero machine rows):
  `machine_state_proto` (per-snapshot per-proto counts by state +
  source), `machine_state_instance` (per-entity state/isPaused/
  isEnabled where the bridge machines route provides them),
  `productivity_state` (hysteresis: status, cause, since-tick,
  last value + evidence label).
- **Capture**: `fetch_live` now also fetches the bridge's
  `/coi/v1/census` + `/coi/v1/machines`, best-effort (absence is
  normal — the snapshot simply carries no machine data, and the
  estimator says INSUFFICIENT for it). Snapshots gain
  `bridgeCensus`/`bridgeMachines` keys; `insert_snapshot` parses
  them in the same transaction (machines route takes precedence
  over census aggregates per proto; unknown state names sum into
  `other` — the state vocabulary is not closed, recon §5.2.5).
- **Cadence**: `coi watch` (SPEC §9) — polls the game clock,
  snapshots on tick change only, `--every 60s|5m`, `--count N`;
  a failed poll is a printed line and a retry, never an exit.
  Smoke-tested end-to-end (2 snapshots on 2 tick changes).
- **Estimator** (`coi/src/productivity.rs`): (a) machine working
  share over the trailing 30-day window with prep-exact evidence
  classes (MEASURED N_eff≥96/span≥27d/gap≤3d; ESTIMATED 30/24d/7d;
  else INSUFFICIENT = listed as unjudged, never "healthy"),
  720-tick thinning, population gate (≥3 instances at ≥80% of
  counted snapshots), Wilson-CI firing (CI upper < 0.50 and
  p̂ < 0.45, confirmed by the second-half leg (ii): CI upper
  < 0.55), clear at CI lower > 0.60, hysteresis dead zone between,
  same-cause re-fire cooldown of one month, cause separation
  (starved / blocked / mixed / unclassified-other / idle) and
  PARKED at paused share ≥ 50% (checked before the class gate —
  parked-ness needs no eligible samples). Ramp exemption (first
  appearance < 1 month before window end, or > 50% instance
  growth). (b) κ-free self-relative output per product
  (FLOW-EXACT; U vs own best finished month; fires at U < 0.50
  sustained 2 months or U < 0.35 once; clears at U > 0.65;
  seasonal + virtual exempt, reusing the board's `is_seasonal`
  verbatim; no-baseline products left to the stall detector;
  composes into an existing stall/deficit issue for the same
  product instead of duplicating it). Every verdict carries the
  recon §5.7 label. Thresholds are the prep's defaults,
  overridable via `[productivity]` in `coi.toml` (minimal parser,
  no new dependency). Surfaces: snapshot-time issues + log lines
  (CLI `snapshot`/`watch` and the website's snapshot button),
  a Productivity section in `coi report`, and a read-only
  Productivity section on the website's Issues page.
- **Verification**: cargo build 0 warnings / 0 errors on Linux
  debug AND Windows release; 12 unit tests (Wilson values,
  thinning, class/fire/parked on synthetic windows, hysteresis
  transitions, TOML overrides, store round-trip incl. census
  unknown-state bucketing); plan-engine parity PASS (both plans);
  `diagnose_live.py` output byte-identical; synthetic end-to-end
  demo against a fake game server — phase A (3 daily snapshots):
  all protos INSUFFICIENT with the missing ingredient named,
  (b) already fired; phase B (31 snapshots): FurnaceT1 MEASURED
  FIRE (N_eff=300, p̂=0.300, CI [0.251, 0.354], starved), MillT1
  PARKED, PressT1 running hot; 93 proto rows + 806 instance rows
  persisted. `dist/coi.exe` rebuilt: 5,828,998 B.
- **Deviations / open mechanics (named, per the prep's licence)**:
  (i) confirmation uses leg (ii) (second-half CI) — leg (i) needs
  a proto→product map that does not exist yet; (ii) cause shares
  are judged on point estimates with every share's numbers in the
  label (the prep says "CI-checked" without fixing the rule);
  (iii) year-window classes not implemented — the store has no
  year of cadence; month window only, stated in the module docs;
  (iv) one real bug was caught by the demo, not the tests (an
  8-for-9 placeholder INSERT that rolled back every snapshot
  insert) — a store round-trip test now guards it.

### Item 2 — QoSWorldMap v0.1.0 (sources complete; compile blocked on refs)

Implemented to prep §2's refined design exactly: plain IMod
shell deferring UI to renderer-init (PriorityManager pattern,
zero Harmony), `WorldMinesWindow : Window` over
`WorldMapManager.Mines` (mines AND rigs — there is no OilRig
type), toolbar entry via `ToolbarHud.AddMainMenuButton` +
`IToolbarItemController` (`ControllerConfig.Window`), filters
(text, state), sorts (product / level / quantity / state / name),
rows showing title, product, state, level + step, quantity left
(or ∞), buffer; row click → `MineNavigator.Navigate` →
`WorldMapWindow.Controller.OpenAndCenterOnLocation` +
`InspectorsManager.TryActivateFor`. NAVIGATION ONLY — no
commands exist in the mod, no remote actions; the inertness
argument is in the mod readme. Manifest dependency on
QoSCore >= 0.1.0, QoSCore.dll reference copy-local OFF, same
discipline as QoSChickens. The four prep unknowns are isolated
as seams and listed in the readme as first-hour live checks:
(1) inspector placement → `MineNavigator.Navigate` (fallback =
guarded SelectLocation replication, plugs in there);
(2) `WorldMapWindow.Controller` resolution → resolved once at
renderer-init via TryResolve, degrades to inspector-only + log
if unresolvable; (3) fullscreen stacking → config
`navigate_opens_map` (default true) flips clicks to
inspector-only; (4) toolbar slot → `WorldMinesController.
ToolbarOrder` = 1500f (PriorityManager's proven slot; the game's
map button is 520). No `enabled` switch, deliberately: the mod
does nothing unless its window is opened and a row clicked
(differs from the acting mods; justified in the readme).

**BLOCKER — no zip this round.** Compiling ANY UiToolkit
`Window` subclass requires the game's UnityEngine module
assemblies (`UnityEngine.dll`, `UnityEngine.CoreModule.dll`,
`UnityEngine.UIElementsModule.dll`) as references — the same
set CoI_PriorityManager and the official ExampleMod reference
from the game install. They are not in `refs/` (which holds
only the Mafi set) and nowhere else on this VM. Probe evidence:
a minimal Window subclass against the Mafi-only refs fails with
exactly one error — CS0012 naming
`UnityEngine.UIElementsModule`; `UiComponent.OnClick` likewise
needs `EventCallback<>` from that module. Everything else was
verified as far as the refs allow: QoSWorldMapMod +
MineNavigator + WorldMinesController compile **0 warnings /
0 errors** against the real Mafi assemblies + QoSCore (stub-
window probe), and every component API the window file uses was
exercised in a second probe — all clean except the two
UIElementsModule-touching points above. The probe also caught
two real bugs pre-emptively (passing `null` for the value-type
`Option<KbHandle>` shortcut parameter — now omitted, using its
declared default; `ControllerConfig` lives in
`Mafi.Unity.InputControl`).

**Unblock**: copy the three UnityEngine*.dll from
`<game install>\Captain of Industry_Data\Managed\` into `refs/`
(the same folder the Mafi set came from) and re-run
`qos/build/build.sh` — it detects them, builds QoSWorldMap, and
writes `dist/QoSWorldMap-mod-0.1.0.zip` +
`dist/QoS-suite-0.3.0.zip` (SUITE_VER already bumped; the suite
readme inside already describes v0.3.0). Until then build.sh
skips both with a loud notice rather than shipping a pack that
silently lacks a member. This round's build.sh run: the three
existing projects rebuilt 0/0 and all prior zips verified
byte-identical (sha256) — `QoS-suite-0.1.0.zip`,
`QoS-suite-0.2.0.zip`, and the three mod zips untouched.

---

# QoS build notes — round 5 (2026-11-05): QoSPriorityActuator (DESIGN — written before code)

Source design: PATTERN-SURVEY.md §2.3 (verdict FITS-WITH-EXTENSION,
~1.2x chickens) + QOL-MOD-PREP.md §5 cause discipline. This is the
actuator counterpart to the companion's productivity estimators:
the companion detects with evidence classes (§5), this mod acts
with bounded steps, and the two share §5's cause vocabulary
(STARVED / BLOCKED / PARKED) so a machine is never described two
ways. It reads machine states in-game and does NOT depend on the
companion or on §5's snapshot prerequisite.

## Verified surfaces (assembly recon: MODDING-NOTES recon 2a + round 7)

- Actuation: `SetGeneralPriorityCmd(EntityId, int)` /
  `(IEntity, int)` — shipped InputCommand in
  `Mafi.Core.Entities.Commands`, public ctors, processed by
  `EntitiesCommandsProcessor` → `entity.SetGeneralPriority(v)`
  with `GeneralPriorities.AssertAssignableRange`, range 0–14.
- Value semantics: `GeneralPriorities` constants settle the
  direction — HIGHEST_PRIORITY=0, HIGH=8, DEFAULT=9,
  LOWEST_ACTIONABLE_PRIORITY=14, LOWEST_PRIORITY=15, IGNORE=16.
  **Lower number = served sooner.** "Stepping a machine's
  priority up" (survey wording) therefore means DECREMENTING
  the stored value toward 0.
- Read side: `Machine.CurrentState` is the nested enum
  `Machine.State` { None, Broken, Paused, NotEnoughWorkers,
  NotEnoughPower, NotEnoughComputing, NotEnoughInput,
  InvalidPlacement, OutputFull, NoRecipes, Working };
  `Machine.Prototype` is `MachineProto`;
  `Prototype.Costs.DefaultPriority` is a public readonly
  per-proto int (ctor default 9). Every Machine is a
  LayoutEntity implementing `IEntityWithGeneralPriority`
  (public `GeneralPriority` getter) and exposes
  `IsCargoAffectedByGeneralPriority` — the per-entity flag for
  whether cargo jobs honour the value at all.
- Scheduling: `IInputScheduler.ScheduleInputCmd` (the suite's
  only mutation path); cadence `ICalendar.NewDayEnd` (chickens /
  recovery pattern).

## Sensing cadence

One pass per in-game day at NewDayEnd. A daily pass is enough:
the actuator's whole premise is PERSISTENT states (a machine
starved for days), and daily granularity is the pattern's proven
cadence. No finer tick (the extension vehicle recovery needed)
is required — a same-day transient starve is exactly what the
persistence gate exists to ignore.

## Cause classification (in-mod, from Machine.State + entity flags)

- **PARKED** — `IsPaused` OR NOT `IsEnabled` OR state == Paused.
  The player's own choice; NEVER touched, never even counted
  toward persistence. (§5 checks parked-ness before its class
  gate; same order here.)
- **STARVED** — state == NotEnoughInput (eligible: constructed,
  enabled, not paused, cargo-affected).
- **BLOCKED** — state == OutputFull. Observed, verbose-logged,
  NEVER adjusted (see the sign decision below).
- **Working** — state == Working: not a nudge target; the
  restore candidate class.
- **Unclassified / hands-off** — None, Broken, InvalidPlacement,
  NoRecipes, NotEnoughWorkers, NotEnoughPower,
  NotEnoughComputing. Priority steers cargo jobs; it cannot fix
  workers, power, computing, breakage or a missing recipe, so
  these are never touched (§5's "other" bucket, same reasoning).

## Nudge policy per cause (the sign decision, named)

- **STARVED → step UP**: schedule SetGeneralPriorityCmd at
  current − 1 (toward HIGHEST_PRIORITY), i.e. trucks serve this
  machine's cargo jobs sooner — the survey's policy verbatim
  ("trucks serve its inputs first"). One step per action.
- **BLOCKED → no action, ever.** The survey leaves the sign
  choice open (it says only "never raised"); the conservative
  resolution is never-touch, for two reasons: (1) the survey's
  stated rationale is that priority cannot fix a downstream
  blockage — a down-step is still "fixing with priority";
  (2) mechanically, general priority steers the entity's cargo
  jobs as a whole (`IsCargoAffectedByGeneralPriority` is a
  single per-entity flag — there is no input-only priority for
  machines), so stepping a blocked machine DOWN would also
  deprioritise the pickups that empty it, working against the
  unblocking. Blocked machines are counted and verbose-logged
  so the pass stays auditable.
- **Working → step back toward proto default** (+1 per pass)
  while the value is still a mod-producible one (restore rule
  below). Restoration is the survey's "steps back toward proto
  default" made symmetric with the nudge.

## Ownership + restore rules (STATELESS — the heart of the mod)

The mod keeps no per-entity memory of what it set (pattern
limit: persistence across reload must be derivable from the
value itself). Let D = the machine's proto default
(`Prototype.Costs.DefaultPriority`) and N = `max_nudge_steps`.

- **Owned set** = { D − k : 0 ≤ k ≤ N }. Only a machine whose
  current value is in the owned set may be adjusted at all.
  Anything outside it (D + anything, or below D − N) is treated
  as player-set and left alone — the mod never fights a value it
  could not have produced.
- **Nudge test** (starved, persistence + cooldown passed):
  current ∈ owned set AND current > D − N AND current − 1 ≥ 0
  (assignable floor) → schedule current − 1.
- **Restore test** (Working): current ∈ { D − k : 1 ≤ k ≤ N }
  — exactly "the value this mod's rule would have set" — →
  schedule current + 1 (one step per pass; reaching D ends it).
- Known imperfection (from the survey, accepted + bounded): a
  player's own D − 1 is indistinguishable from the mod's −1;
  after a reload the mod may "restore" a value the player set.
  The band cap (N small) bounds the worst case, every action is
  logged with its prediction, and the player can defeat the
  rule entirely by setting any value outside the owned set.

## Bounds (all config, conservative defaults)

- `persistence_days` = 3: a machine must be observed STARVED on
  3 consecutive daily passes before the first nudge (survey:
  "multi-day persistence required before the first step").
- `max_nudge_steps` (N) = 2: the band is [D − 2, D]; also
  hard-clamped to the assignable range [0, 14].
- `cooldown_days` = 2: minimum passes between two nudges of the
  same machine.
- `max_actions_per_day` = 10: nudges + restores together.
- Persistence counters, cooldowns and last-cause tracking are
  SESSION memory only (the recovery mod's marks pattern): a
  reload resets them, which only delays action — the safety
  properties (ownership, band, restore) are all stateless and
  survive reloads exactly.
- Safety class, stated plainly: NO exact simulation exists for
  logistics response (survey limit), so this mod is the weaker
  safety class — bounded nudges + stateless ownership + restore,
  with every scheduled action logged with its prediction (id,
  proto, cause, from → to, expected effect) for audit against
  the next passes.

## Toggles

`enabled` (master, default false), `nudge_starved` (default
true), `restore_when_working` (default true), `log_actions`
(default true). Config via the game's own settings mechanism,
re-read every pass — the suite convention.

## AS BUILT — round 5 (2026-11-05)

Built exactly the design above: **QoSPriorityActuator v0.1.0**
(new feature mod on QoSCore) + suite pack **QoS-suite-0.4.0**.
Everything staged in `dist/`, no uploads, no GitHub push, no
live-game contact. CoiBridge untouched.

### Artefacts

| File | Bytes | Contents |
|---|---|---|
| `dist/QoSPriorityActuator-mod-0.1.0.zip` | 23,267 | `QoSPriorityActuator/` (dll 13,824; **no** QoSCore.dll inside, by design) |
| `dist/QoS-suite-0.4.0.zip` | 79,201 | `QoSCore/` + `QoSChickens/` + `QoSVehicleRecovery/` + `QoSPriorityActuator/` + `QoS-SUITE-README.txt` |
| `dist/QoS-suite-0.1.0.zip` / `0.2.0.zip` | — | previous packs — NOT rewritten (sha256 verified identical before/after this build, as were all three prior mod zips) |

Build: `qos/build/build.sh` (extended) — all five compilable
projects **0 warnings / 0 errors** (net48). Sources in
`qos/QoSPriorityActuator/`; scaffold in `qos/build/actuator/`
(one csproj per folder, per the suite gotcha). QoSWorldMap
still cannot compile (UnityEngine refs absent) and was skipped
loudly, as before.

### QoSPriorityActuator — as designed

- Daily pass at `ICalendar.NewDayEnd`; the only mutation path
  is `SetGeneralPriorityCmd` via `IInputScheduler`. Cause
  classification from `Machine.CurrentState` + entity flags in
  §5's order: PARKED (paused / disabled / state Paused) first,
  then STARVED / BLOCKED / Working / hands-off states.
- Nudge = current − 1 (lower number = served sooner), gated by
  persistence streak, per-machine cooldown, the stateless
  owned-set test, the band floor, the assignable range [0, 14]
  and the daily cap. Restore = current + 1 toward proto
  default while Working, under the stateless restore test.
- Every scheduled action logs its prediction (machine, proto,
  cause, from → to, proto default, band floor, streak).

### Defaults (all in Settings -> QoS Priority Actuator)

| Setting | Default | Note |
|---|---|---|
| `enabled` | **false** | master switch — ships inert |
| `nudge_starved` | true | the starved-step policy |
| `restore_when_working` | true | step back toward proto default |
| `persistence_days` | 3 | consecutive starved passes before the first nudge |
| `max_nudge_steps` | 2 | band = [proto default − 2, proto default] |
| `cooldown_days` | 2 | between nudges of the same machine |
| `max_actions_per_day` | 10 | nudges + restores together |
| `log_actions` | true | one line per scheduled action, with prediction |

### Deviations from the survey / design text (each named)

1. **BLOCKED sign resolved to never-touch.** The brief's
   "opposite-signed treatment" reading was considered; the
   survey's literal semantics ("never raised") plus the
   single-flag cargo argument (a down-step would deprioritise
   the pickups that empty the machine) make no-touch the
   conservative pick. Rationale in the design section above.
2. **Suite pack ships without QoSWorldMap (process change vs
   round 4).** Round 4 withheld the pack entirely while the
   window mod could not compile. This round's brief required a
   new pack; the pack is written with the four compilable mods
   and the suite readme inside declares the membership (and
   WorldMap's absence + reason) explicitly — declared absence,
   not silence, which was round 4's actual concern. SUITE_VER
   0.3.0 → 0.4.0; no 0.3.0 pack file was ever written (0.3.0
   existed only as a version string + readme state).
3. **Restores are not cooldown-gated.** The cooldown bounds
   nudges; restores step at most once per pass while Working,
   and re-nudging after a restore must re-earn the full
   persistence streak, so oscillation is bounded to ±1 per
   (persistence + 1) days by construction.
4. **Streak counting is gated by `nudge_starved`.** With the
   toggle off, starved observations do not accumulate;
   enabling it starts a fresh persistence count. Conservative
   reading of "observed under the active policy".
5. **Cargo-affected gate added.** A machine whose
   `IsCargoAffectedByGeneralPriority` is false is skipped in
   both directions — its priority value is inert for cargo, so
   adjusting it would be a no-op action with a misleading log
   line. The survey did not name this gate; the surface is
   verified (recon 2a / round 7).
6. **Log cause labels are lowercase** (`cause=starved`,
   `cause=working`) to match the companion estimator's
   evidence-label format (productivity.rs `cause={}`), while
   the design vocabulary stays §5's STARVED / BLOCKED /
   PARKED. Same words, two casings, one meaning.

### NOT verified (needs first live install / first enabled days)

- `IEntitiesManager` injection + `GetAllEntitiesOfType<Machine>`
  enumeration in the service (the chickens service uses the
  same manager surface — compiled clean; a failure would
  surface as one init error log line, mod inert).
- Cross-assembly QoSCore resolution through the manifest
  dependency (still the suite-wide first-install check).
- Behavioural eyeball on first enabled days: predicted steps
  vs the machines' inspector values the next day; confirm a
  machine set OUTSIDE the band is never touched; confirm the
  owned-band imperfection behaves as documented (a player's
  own D−1 inside the band is indistinguishable from the mod's).
- Weaker-than-survey sensing, reported honestly: the survey
  described the read side via the bridge's scan
  (`Machine.CurrentState`, `IsPaused`, `IsEnabled` per
  instance). In-mod all three verified present and public —
  no weakening found. The one nuance: `Machine.State` has no
  separate "disabled" value — disabled arrives via the entity
  `IsEnabled` flag, which is why PARKED is a flag test, not a
  state test.

# QoS build notes — round 6 (2026-11-05): the unified activity log (QoSCore 0.2.0)

Philip's ask: "add a LOG tab which is an exact replay of things we
did in the COI exe or in the mod usage and stored that way" — a
persistent, chronological activity record (a replay to READ, not
a re-execution). Companion half is SPEC Addendum XVI; this note is
the QoS half: what the mods record, where, and in what shape.

## DESIGN (written before code)

- **One recorder in Core.** `QoSEventLog` (new file in QoSCore):
  static, append-only, JSON Lines. Mods must never grow their own
  file formats — the feed is suite infrastructure, so it lives
  with the logging and the registry.
- **Location**: `%APPDATA%\Captain of Industry\QoS\events-v1.jsonl`
  — next to the game's own data (where Mods/ lives), NOT inside
  any mod folder (removing a mod must not delete the record) and
  NOT in a save (the record spans saves; the companion imports it).
- **Line schema v1**: schema / eventId (GUID — the companion's
  import dedupe key) / recordedAtUtc (ISO-8601 UTC wall clock) /
  source "qos" / modId / modVersion / kind / action / summary /
  data{}. Hand-rolled JSON writer (string/bool/integral/double/
  dictionary/list) — no new dependency for a fixed, tiny schema.
- **Fail-safe above all**: Record() wraps everything in try/catch
  and appends under a lock. A full disk or a locked file costs one
  warning line (once), never an exception in the simulation.
- **Actions only.** Instrumentation points are the existing
  ScheduleInputCmd call sites, AFTER the call succeeds, plus
  QoSWorldMap's navigation (its only action). No debug lines, no
  skipped/no-op passes, no per-pass summaries. Recording is NOT
  gated by log_moves/log_actions (those govern game-log noise);
  the single gate is QoSCore's new `activity_log_enabled` config
  (default true), wired by QoSCoreService.
- **Game clock**: recon (MODDING-NOTES round 8) found the verified
  public surface — `ICalendar.CurrentDate` (`Mafi.GameDate`, public
  Year/Month/Day). QoSCoreService — which now also takes ICalendar
  by constructor injection, the feature services' pattern — wires
  it as the recorder's date provider; the date rides in
  `data.gameDate` and the companion promotes it on import.
  ICalendar exposes NO tick, so QoS events carry no game tick.
- **Versions**: QoSCore 0.2.0 (new capability = minor bump per the
  suite's own convention); features that call the recorder bump
  patch → 0.1.1 and their manifest dependency becomes
  "QoSCore >= 0.2.0". Suite pack → 0.5.0.

## AS BUILT — round 6 (2026-11-05)

### Artefacts

All five projects: **0 warnings / 0 errors** (net48). Suite pack
**QoS-suite-0.5.0.zip** (116,830 B — QoSCore 0.2.0 + four features
at 0.1.1 + QoSWorldMap + suite readme). Individual zips:
QoSCore-mod-0.2.0.zip (16,040 B), QoSChickens-mod-0.1.1.zip
(21,341 B), QoSVehicleRecovery-mod-0.1.1.zip (23,748 B),
QoSPriorityActuator-mod-0.1.1.zip (24,579 B),
QoSWorldMap-mod-0.1.1.zip (26,286 B). Prior suite packs
(0.1.0 / 0.2.0 / 0.4.0) and prior individual zips sha256-verified
byte-identical after the build. No feature zip contains
QoSCore.dll (as always).

### What each mod records

- **QoSChickens** — `MoveAnimalsCmd` at the single TryMove
  scheduling site (both modes): farm id, signed amount, species,
  mode, predicted post-move spread (min..max over the eligible
  set — now computed unconditionally; the log line reuses it),
  eligible farm count.
- **QoSVehicleRecovery** — `RecoverVehicleCmd` (truck id, stage 1,
  reason cannot-deliver / navigation-stuck, NavigationFailedStreak,
  cargo product+quantity when loaded, cost) and
  `DiscardVehicleCargoCmd` (truck id, stage 2, the mark's product,
  quantity, cost). Costs are plain numbers via
  `Upoints.Value.RawValue / 65536.0` (Fix32 decoding, recon 8).
- **QoSPriorityActuator** — `SetGeneralPriorityCmd` at both sites:
  nudge (machine id, proto, cause=starved, current→target, proto
  default, band floor, streak) and restore (same minus streak,
  cause=working).
- **QoSWorldMap** — kind `navigation`, action `NavigateToMine`, in
  the window's Go() AFTER `MineNavigator.Navigate` returns (a
  click that actually navigated; the pre-renderer-init early
  return records nothing): mine title, product, state, level.

### Companion half (for the trail; detail in SPEC Addendum XVI)

Store schema v3 + `activity_events`; do_apply writes the legacy
audit row AND the rich event; snapshot creation is a system
event; the feed imports on Log-page load (idempotent, counts
shown on the page); `/log` renders with source/search filters and
200-row pagination. Verified: 15/15 Rust tests (migration +
backfill idempotency, import counts/dedupe, ISO-8601 parsing) and
`tools/check_log_page.sh` against a scratch serve.

### Deviations / limits (named)

1. **No game tick on QoS events** — ICalendar has no tick member
   (recon 8). Dates only; the companion's own events carry both.
2. **QoSCoreService gained an ICalendar constructor parameter.**
   Same injection the feature services already use, so it should
   resolve identically — but Core previously resolved with only
   suite-internal dependencies, so this is a new (if small) live
   unknown: if it failed, Core would log its init error, the
   registry/verbose wiring would be skipped, and the recorder
   would still work (Enabled defaults true) minus game dates.
3. **Feed growth is unbounded** — one small line per action, no
   rotation in v1. At daily-pass cadences this is years of runway;
   deleting the file is safe (the companion keeps its imports).
4. **WorldMap records even if navigation visually no-ops** — the
   record attests the click invoked Navigate(), which is the
   mod's action; whether the game centred as intended is one of
   the round-4 live-check seams, not something the click handler
   can attest.

### NOT verified (needs first live install)

- QoSEventLog writing in-game (path, permissions, append sharing
  with the companion reading the same file).
- QoSCoreService's ICalendar injection (see deviation 2).
- The companion's Log page against a real feed (scratch feed
  verified; real one awaits the mods running).

# QoS build notes — round 7 (2026-11-05): THE SAVE-SERIALIZER INCIDENT — suite 0.5.1 (Chickens / VehicleRecovery / PriorityActuator 0.1.2)

## Symptom (live, Philip's screenshot, 2026-11-05 ~19:32)

Saving ANY game with suite 0.5.0 installed failed:

    Failed to save the game '...Prosperous Border\26-11-05_16-43-autosave-paused.save':
    Failed to create generic serializer for 'ChickenBalancer'.
    Mafi.Serialization.BlobWriter.WriteGeneric -> DynamicDispatchClassSerializer
    -> Mafi.EventBase`1+CallbackSaveData[TAction].Serialize -> Lyst`1.SerializeData

The game itself was unharmed (still paused at the same tick); only
saving was broken. Side effect worth noting: the incident PROVES the
suite loads in-game and QoSCore dependency resolution works —
ChickenBalancer was constructed and subscribed, which was the
long-open live-check item.

## Root cause (decompiled from the 0.8.8 assemblies; evidence in bridge MODDING-NOTES round 11)

1. `ICalendar`'s events are SAVED events (`Mafi.EventBase<Action>`).
   `Add(owner, callback)` stores `CallbackSaveData { Owner, DeclaringType,
   MethodName }` in `m_callbacksSaveData`, and `EventBase.SerializeData`
   writes that list into every save.
2. `CallbackSaveData.Serialize` calls `writer.WriteGeneric(value.Owner)`:
   the callback TARGET OBJECT is generic-serialized by runtime type.
3. The eligibility rule (BlobWriter / GenericSerializersFactory /
   SerializerGenerator, quoted): a concrete class gets a serializer
   only via `DirectCallSerializer<T>`, whose constructor requires
   public static methods found by
   `t.GetMethod("Serialize", BindingFlags 24 /* Public|Static */)`
   (and the matching `Deserialize`). No pair -> the factory returns
   null -> "Failed to create generic serializer for '<Class>'".
4. Shipped subscribers (e.g. `Mafi.Core.Simulation.StatsManager`,
   which subscribes to the SAME `NewDayEnd` with the IDENTICAL call
   shape `calendar.NewDayEnd.Add(this, new Action(onNewDay))`)
   carry the pair because the game's serializer source generator
   emitted it for their `[GenerateSerializer]` classes. Our three
   services had neither the pair nor any marking — the subscription
   call shape was never the problem; the missing serializer was.
5. `ICalendar` exposes NO non-saveable event surface (all seven
   members are saveable `IEvent`s), so "subscribe somewhere
   unsaved" does not exist for calendar cadence. The game's
   actually-unsaved callback surface is `IGameLoopEvents`
   (GameLoopEvents' registration lists are [DoNotSave]) — but it
   is a renderer/new-game lifecycle surface, not a daily hook.

## The fix (the game's own sanctioned pattern)

Each of the three services now carries, mirroring the generator's
output shape for StatsManager:

- `[ManuallyWrittenSerialization]` on the class — the game's marker
  for hand-written serializer pairs. The generator's validation pass
  (generateSerializationSpecs) SKIPS marked types and THROWS for any
  type that has Serialize/Deserialize methods without
  `[GenerateSerializer]` or this mark, so the mark is required, not
  decorative.
- `public static void Serialize(T value, BlobWriter writer)` /
  `public static T Deserialize(BlobReader reader)` built on
  TryStartClassSerialization / TryStartClassDeserialization +
  EnqueueDataSerialization / EnqueueDataDeserialization.
- `SerializeData` writes ONLY the calendar reference
  (`writer.WriteGeneric(m_calendar)` — a global-dependency ref,
  exactly what shipped generated code writes for such members).
- `DeserializeData` restores the calendar (`SetField` +
  `ReadGenericAs<ICalendar>()`), re-resolves every other dependency
  from the game's DependencyResolver at load-finalize
  (`RegisterResolvedMember` — resolution failure there is a
  CorruptedSaveException, and all registered types are ordinary
  global deps / the mod instance, resolvable by construction), and
  re-initialises session-only state empty (VehicleRecovery marks +
  pass index; PriorityActuator observations + pass index — the
  documented "resets on reload" semantics; load materialises the
  instance uninitialised, so the reset MUST live in DeserializeData).
- Field restructure: services now hold the MOD instance (`m_mod`)
  instead of a bare `ModJsonConfig` (`m_config`), because
  ModJsonConfig is not a resolvable dependency while the mod
  instance is (it is already constructor-injected today). Config
  access is `m_mod.JsonConfig.*` — still re-read on every pass,
  behaviour identical.

Load-identity note: `DependencyResolver.DeserializeInto` +
BlobReader's object map guarantee the callback owner, the resolver
section instance and any other reference are ONE object; the event's
lazy `TryBindCallback` rebinds `Delegate.CreateDelegate(type, owner,
method)` against that instance, whose fields DeserializeData +
FinalizeLoading restore before any day-end can fire.

Version/manifest changes: the three features 0.1.1 -> 0.1.2 (Mod
Version consts in step); QoSCore stays 0.2.0 and QoSWorldMap stays
0.1.1 (untouched, audit-clear); suite pack SUITE_VER 0.5.0 -> 0.5.1.
The three feature manifests now set `can_remove_from_saved_game:
false`: saves WRITTEN with the fixed build contain the service
footprint (subscription + resolver entry), so removal must be
blocked by the game with a clear message rather than surfacing as
an unloadable save. Saves written before the suite existed contain
nothing of it and are unaffected. Per-mod readmes + SUITE-README
rewritten accordingly (README carries a top banner: versions
before this fix can prevent saving — remove/disable them).

## Audit of the remaining surface (same exposure?)

- QoSCore (QoSCoreService, SuiteRegistry, QoSEventLog): CLEAR.
  No event subscriptions anywhere (grep). QoSCoreService holds NO
  fields — its ICalendar is used transiently in the constructor to
  wire a STATIC delegate on QoSEventLog; statics are never
  serialized, and nothing saved references the service. Neither
  class has a serializer pair, so BlobWriter.CanSerialize excludes
  them from the resolver's saved-instances section
  (DependencyResolver.serializeData filters on CanSerialize).
  SuiteRegistry's List<string> is runtime-only for the same reason.
- QoSWorldMap: CLEAR. Its only registration is
  `IGameLoopEvents.RegisterRendererInitState` — in the shipped
  GameLoopEvents class every registration list is
  [DoNotSave]/[DoNotSaveCreateNewOnLoad], i.e. never written to a
  save; registration re-happens per session from Mod.Initialize.
  Controller/window are Unity-side runtime objects, referenced by
  nothing saved.
- CoiBridge (route classes, mod Initialize): CLEAR. Zero event
  subscriptions of any kind (grep over bridge sources). Routes are
  discovered at runtime by WebApiServer.ScanObjectForRoutes and
  hold no state; no serializer pair -> excluded from saves by the
  same CanSerialize filter; the bridge writes nothing into saved
  structures.

## Artefacts

- dist/QoS-suite-0.5.1.zip — 126,268 B — sha256 a2ed9232523e401bc7d3d322c40c1fc489339489967f517aedd2bcae5cc52626
- dist/QoSChickens-mod-0.1.2.zip — 23,926 B — sha256 3efff494f714380fa1d45d334fba343c2a07917b2c2b0fec582653be551f5697
- dist/QoSVehicleRecovery-mod-0.1.2.zip — 26,325 B — sha256 81f46b5586698d4a2c98d3c41b1f6c84eed73a2abd6839bacf8565625807ccfc
- dist/QoSPriorityActuator-mod-0.1.2.zip — 27,158 B — sha256 fde6e095668045d7e630397b3a2314f6e7b0d8c6f068368ea4877a9a874d7ffa
- All five projects build 0 warnings / 0 errors. All prior packs
  (suite <= 0.5.0, per-mod <= 0.1.1, Core 0.2.0 zip, WorldMap
  0.1.1 zip) sha256-verified byte-untouched after the rebuild.
- Compiled-DLL check (ilspycmd on the built assemblies): all three
  services show [ManuallyWrittenSerialization] + the exact public
  static Serialize/Deserialize pair.

## Verification status

Code-level ONLY (the coordinator cannot run the game): pattern
equivalence with the shipped StatsManager generated serializer +
eligibility by the decompiled rules + compiled-assembly inspection.
FINAL PROOF = Philip installing suite 0.5.1 and saving successfully,
then loading that save and confirming the daily passes still arm.
Residual risks, named: (1) RegisterResolvedMember resolving the
concrete mod type at load-finalize relies on the mod instance being
registered in the game resolver — strongly evidenced (the same
resolver constructor-injects it into these services today), not
load-tested; a failure would surface as a CorruptedSaveException on
LOAD, so the first live check should be save -> reload, not just
save. (2) The hand-written pair was validated against the
generator's shape, not against the game's runtime serializer-spec
pass over mod assemblies — if that pass dislikes anything, it will
say so at game/mod load, before any save is attempted.

# QoS build notes — round 8 (2026-11-05 evening): QoSBuilder 0.1.0 — companion blueprint placement via confirmed job files

Philip approved the posture step 2026-11-05: the suite gains its
first entity-CREATING mod. Recon round 10 (MODDING-NOTES) proved
the web /command channel can never carry BuildTransportCmd or
BatchCreateStaticEntitiesCmd, so placement runs companion-side
confirmation -> job file -> in-game mod, exactly the §12
prepare -> confirm -> audit discipline with a file as the apply
step. Built standalone (own zip, own build script); suite-pack
integration waits for live verification, same as WorldMap did.

## Probe outcome (MODDING-NOTES round 12 — the string route wins)

The approved design's open question was whether a mod can reach
the game's blueprint deserialization. Verdict: YES, through
fully public API — no reflection, no internals:

- `BlueprintsLibrary` is PUBLIC and `[GlobalDependency]`:
  inject it, call the instance
  `TryParseFromString(code, out error, out IBlueprintItem)` — a
  pure parse on the library's own serialization context; the
  player's library is untouched.
- `IBlueprint.Items` is the public
  `ImmutableArray<EntityConfigData>` the batch command's ctor
  takes; `EntityConfigData` has public setters (Transform /
  Trajectory / Pillars / OriginalEntityId) + `CreateCopy()`;
  the skip rule is public
  (`BlueprintRotationUtils.ShouldSkipForPlacement`); the
  transform math (TileTransform, RotatedAroundZ,
  TransportTrajectory.TryCreateFromPivots) is public.
- gameId: constructor-inject `IGameIdProvider` (the surface the
  vanilla game-info handler itself uses) — `GameId.ToString()`
  is byte-identical to the API's gameId.
- The internal `Blueprint` / `DeserializeForBlueprints` path is
  NOT mod-reachable (InternalsVisibleTo covers only Mafi's own
  assemblies + tests) — and is not needed.

The items[] fallback (per-item CreateStaticEntityCmd /
BuildTransportCmd specs) was therefore NOT built for v1; it
remains the documented fallback in MODDING-NOTES round 12.

## What was built

**QoSBuilder 0.1.0** (`qos/QoSBuilder/`; BuilderService +
BuilderJson + IMod shell; manifest depends on QoSCore >= 0.2.0;
can_remove_from_saved_game: false — the round-7 lesson applied
from birth: the service subscribes to ICalendar.NewDayEnd and
carries the [ManuallyWrittenSerialization] pair mirroring the
0.1.2 fix exactly, calendar field only).

- Watches `%APPDATA%\Captain of Industry\QoS\jobs\` (sibling of
  the QoS events feed). Passes run at day end, ONLY when the
  `enabled` config is on (ships false — while off the folder is
  not even scanned).
- Validation pipeline, in order, each refusal consumed + logged
  with zero partial action: unreadable -> malformed (incl.
  missing/mistyped fields, jobId shape, anchor sanity) ->
  duplicate (jobId already in processed.txt: filed, no event) ->
  unsupported-schema -> bad-hash -> expired -> wrong-game
  (IGameIdProvider) -> unsupported-rotation ->
  reflection-unsupported -> blueprint-unparseable (the game's
  own parser error text rides in the event) -> not-a-blueprint
  -> over-item-cap (min of job cap and config
  max_items_per_job, default 500) -> placement build
  (track-rotation-unsupported / item-transform-failed) ->
  nothing-to-place.
- Placement mirrors the game's StaticEntityMassPlacer: copy
  each item, clear OriginalEntityId, skip
  ShouldSkipForPlacement, rotate about the anchor (anchor added
  AFTER rotation — blueprint coords are origin-relative), rebuild
  trajectories from rotated pivots via TryCreateFromPivots with
  the game's blueprint-mode flags, offset pillars. ONE
  BatchCreateStaticEntitiesCmd(DeferToProto,
  allowValidationSuppression: false, applyConfiguration from the
  job) via IInputScheduler.
- Claim BEFORE schedule: file renamed .claimed, jobId appended
  to processed.txt, JobClaimed event — then schedule; success
  renames .done and records JobScheduled (item counts, skipped
  counts, surfaces/decals ignored counts, anchor, rotation,
  protosThatFailedToLoad when present); schedule exception
  renames .failed + JobScheduleFailed.
- Rotation support: quarter turns for buildings + transports;
  train-track items only at rotation 0 (v1 refusal, honest);
  mirrored jobs refused.
- Surfaces/decals are NOT placed in v1 (the batch command does
  not carry them) — counted in the JobScheduled event.

**Companion (Rust)** — new /place flow (serve.rs):

- Library: `tools/gen_blueprint_library.py` embeds the pack as
  `coi/data/blueprint-library.json` (82 entries, 167,149 B;
  strings decode-verified with the pack codec at generation;
  per-slug metadata from the pack's own JSON sidecars — the
  manifest's entity counts were found stale and are not used;
  includes the pack's honest NOT-in-game-tested status line).
- GET /place: library table (name, module/layer, item count +
  kind breakdown, research gate) with a per-row prepare form
  (anchor x/y/z, rotation select); shows the resolved jobs dir
  and live gameId, or the unreachable warning.
- POST /place/prepare: CSRF; refuses when the game is
  unreachable or gameId is not a string (502, nothing prepared).
  Builds the job (guid jobId, created/expires ISO-8601 UTC with
  a 30-minute TTL, gameId pinned, companionUser = session user,
  applyConfiguration true, limits.maxItems 500), computes the
  canonical payload + sha256, and renders the §12 confirm page
  (summary, target game, expiry, sha256, the exact job JSON).
- POST /place/confirm: 120 s single-use token, exactly like
  §12. Writes `<jobs_dir>/<jobId>.json` (create_dir_all,
  tmp-then-rename) — THE ONLY WRITE PATH — then audit_add +
  activity_add (source companion, category placement, outcome
  job-written / write-failed, payload = the exact job JSON).
- jobs_dir: coi.toml `[builder] jobs_dir` (search order mirrors
  [productivity]: $COI_CONFIG, ./coi.toml, next to the DB);
  default `%APPDATA%\Captain of Industry\QoS\jobs` — on Philip's
  PC that is the mod's exact folder with zero configuration.

**Canonical-hash interop** — the contract's sharp edge, pinned
three ways: BuilderJson.cs builds the canonical payload from
the PARSED job (unknown fields ignored, integer truncation
canonical: 90.0 hashes as 90 — a reformatting JSON writer can
never invalidate a genuine job); serve.rs builds it from the
typed fields; a unit test (`job_canonical_vector`) asserts both
the canonical string and sha256 against a vector computed by an
independent Python implementation, and
`tools/check_place_page.sh` re-verifies a REAL written job file
with that same Python implementation.

## Artefacts

- dist/QoSBuilder-mod-0.1.0.zip — 35,184 B — sha256
  1e5aa745b4029540874cb8f5b919d52a3382c6a4677e31d03eb7a40d7ace3965
  (QoSBuilder.dll 28,160 B + manifest/config/readme; no QoSCore
  DLL inside — the build script guards it)
- dist/coi.exe — 6,062,808 B — sha256
  f7676419573793e1bdf70ef7877d73aba6d048bcb520026f5d2ce35eb9a67d27
  (Windows x86-64 release; was 5,886,159 B — the delta is the
  embedded library + the Place flow)
- Build: QoSBuilder C# 0 warnings / 0 errors (standalone
  `qos/build/build-builder.sh` — suite build.sh and all 0.5.1
  packs untouched); Rust 17/17 tests (15 prior + canonical
  vector + ISO format); tools/check_place_page.sh PASS (stub
  game: prepare pins gameId 5583213724730880438, confirm writes
  one file whose sha verifies independently, token replay
  refused, /log shows PlaceBlueprintJob, game-down prepare =
  502 with nothing written).

## Verification status

Code + stub level ONLY. Unverified until Philip runs it live:
(1) DI resolution of BlueprintsLibrary + IGameIdProvider in
BuilderService's constructor (both are the game's own
[GlobalDependency]-registered surfaces — compile-verified,
resolution itself is runtime); (2) the first real job end to
end (day-end pass -> batch processed -> sites appear);
(3) the census-diff verdict practice on the companion side;
(4) save + reload with QoSBuilder's serializer pair — the same
proof class suite 0.5.1 still owes. Install order when he is
ready: QoSBuilder zip alongside the suite, enable it in the
save's mod list, leave `enabled` OFF until the first supervised
placement, then confirm one small blueprint (concrete-v1, 39
items) in a sandbox and watch the Log page + census.

# QoS build notes — round 9 (2026-11-06 evening): QoSBuilder 0.2.0 — designation jobs (stage 2 of 4)

Stage 2 of the designation programme (spec: DESIGNATION-DESIGN-NOTE.md,
stage 1). MOD SIDE ONLY — bridge, companion Rust, serve.rs, plans and
INSTALL.md untouched (stages 3–4 own them). QoSBuilder gains its second
job family: schema-2 designation jobs, executed under the identical
claim/refusal discipline as blueprint jobs. Built standalone (own zip,
own build script); suite-pack integration still waits for live
verification, as with 0.1.0.

## Assembly surfaces re-verified before coding (qos probe, 0.8.8 refs)

- `DesignationData` 5-arg ctor is `(Tile2i, HeightTilesI origin,
  HeightTilesI plusX, HeightTilesI plusXY, HeightTilesI plusY)` —
  ctor order Origin/PlusX/PlusXY/PlusY, centre derived. ✓ note §1.1.
- `AddTerrainDesignationsCmd(Mafi.Core.Prototypes.Proto.ID,
  ImmutableArray<DesignationData>)`; `RemoveDesignationsCmd(
  ImmutableArray<Tile2i>)`; `Proto.ID(string)` public ctor;
  `HeightTilesI(int)` in namespace `Mafi`. Kind is the command's
  Proto.ID — one kind per batch; `DesignationData` has no kind
  member. ✓ note §1.2.

## What was built

- **Family dispatch** (`BuilderService.ProcessJob`): files with a
  non-empty string `jobType` branch to the new
  `ProcessDesignationJob`; v1 files (no `jobType`) take the existing
  blueprint path — same code, same order, same event payloads
  (`Refuse` now delegates to a shared `RefuseCore` with kind
  "placement" / key "blueprintName", byte-identical output).
- **`BuilderJson`**: `GetList` accessor + `CanonicalDesignation(...)`
  - the `DesignationCell` / `DesignationOrigin` structs. Canonical v2
  field order per note §2.3 (jobId, schema, jobType, createdUtc,
  expiresUtc, gameId, companionUser, planName, protoId [add only],
  cells|origins, limits.maxCells); cells carry corners in ctor
  order; the centre is never a field.
- **Execution**: ONE `AddTerrainDesignationsCmd(new Proto.ID(protoId),
  ImmutableArray<DesignationData>)` per add job, cells built via the
  5-arg ctor (corners only, centre game-derived); ONE
  `RemoveDesignationsCmd` per remove job. No new injected services.
- **Events**: kind `"designation"`, the same four actions
  (JobClaimed / JobScheduled / JobRefused / JobScheduleFailed); data
  adds `jobType`, `protoId` (add jobs), `cells` / `origins` counts.
  Claim-before-schedule, processed.txt, `.claimed`/`.done`/`.failed`
  — identical to blueprints.
- **Config**: `max_cells_per_job` (default 200, declared [1, 2000]).
  Manifest + `QoSBuilderMod.Version` → 0.2.0;
  `can_remove_from_saved_game` stays false. Readme rewritten for
  both families; SUITE-README.txt gained a "NOT IN THIS PACK" note
  (the builder still ships standalone; the note now records the
  designation family).

## The C# canonical leg — EXECUTED (quality-audit finding, closed)

The audit found the v1 C# canonical had never been executed ("pinned
three ways" was really two + visual equivalence). This round executes
it, and pins v2 the same way. Harness: `qos/build/canonprobe/`
(kept, rerunnable) — a net8 console that (a) compiles the exact
shipping `BuilderJson.cs` source in, and (b) in `dll` mode invokes
the methods on the packaged net48 `QoSBuilder.dll` itself by
reflection. Expectations computed INDEPENDENTLY in Python from the
specs (v1: the vector already pinned in serve.rs; v2: a from-the-note
implementation of §2.3).

| Vector | Expected sha256 (Python) | C# source leg | C# DLL leg |
|---|---|---|---|
| v1 blueprint | a8b4498e9edb99e4338bbee339a826f51db9f6ad7a0439c72d8e379343069cbf | MATCH | MATCH |
| v2 designations-add | e4166dd35533a12049ee495a5192358f3a262272cc86475c0a79979158ed57f7 | MATCH | MATCH |
| v2 designations-remove | 78fb273ccdb9d498cae5e05ecc077d1e18c58dd2c406d59c23d1a8617e70cfa4 | MATCH | MATCH |

Canonical strings printed by both legs are identical to the Python
strings, byte for byte (including the escaping exercise: a plan name
with quote, backslash and "Béton"). Stage 3's Rust leg must reproduce
the two v2 hashes exactly — they are the cross-stage contract.

## Refusal ladder — walk-through against the code

Shared rungs (same order as blueprints; events kind "designation"
once the family is known):
unreadable → malformed (missing/wrong-typed shared fields, unknown
jobType, unparseable expiresUtc) → **malformed-cells (structural:**
cells/origins not an array, item not an object, field missing or
non-integer — see decision 1) → duplicate (jobId in processed.txt:
filed `.done`, no event) → unsupported-schema (≠ 2) → bad-hash →
expired → wrong-game.
Family rungs (note §2.2 order): unsupported-proto (allowlist
MiningDesignator / DumpingDesignator / LevelDesignator) →
malformed-cells (semantic: origin off the 4-grid, origin magnitude,
corner outside ±1000) → duplicate-cells (add: refuse; remove:
dedupe) → over-cell-cap (count > min(job `limits.maxCells`, config
`max_cells_per_job`)) → nothing-to-designate (count 0).
Then claim → schedule (exception → JobScheduleFailed + `.failed`)
→ JobScheduled. DI-bound parts (scheduler, IGameIdProvider) cannot
execute on this VM — the ladder is verified by this code review and
by the stage-4 sandbox shakedown live, stated honestly.

### Ladder decisions where the note left a choice

1. **Structural malformed-cells is detected pre-hash**, not in the
   note's post-wrong-game position: the canonical payload is
   REBUILT from the parsed cells, so an unparseable cell has no
   canonical to hash — refusing it later is impossible by
   construction. The note's rung placement is honoured for the
   semantic half (alignment / plausibility), which needs no
   canonical. Detail strings distinguish the two halves.
2. **Unknown jobType → "malformed"** (the note names no rung): a
   jobType this mod doesn't speak is a schema-shape problem, the
   same class as v1's field malformations.
3. **Remove duplicates: silently deduplicated** (first occurrence
   kept); add duplicates refused. Rationale: RemoveDesignation is
   proto-agnostic and idempotent (an empty origin is a no-op), so
   a duplicate remove is unambiguous, while a duplicate add cell
   triggers the game's silent last-wins (§1.3) — exactly what the
   seal exists to prevent. The hash is verified over the file AS
   CONFIRMED (duplicates included); dedupe shapes only execution.
4. **Origin magnitude bound ±100000** added to malformed-cells,
   mirroring the v1 anchor sanity bound; the note names only the
   ±1000 corner bound. Range comparisons, not Math.Abs (see
   observation 2 below).
5. **Config cell cap clamped to [1, 2000]** at read time (the
   note's declared config range), rather than trusting a
   hand-edited stored value.
6. **Designation refusals record the extractable jobId** in the
   processed record + event, where v1's field-malformation refusal
   passes null even when a jobId was readable. Blueprint behaviour
   deliberately left as-is (byte-identical path).

## Serializer discipline — untouched, verified in the compiled DLL

No new fields on BuilderService (the proto allowlist is a static
method, not state; the cell cap is a pass-local, like the item
cap). ilspycmd on the built DLL: `[ManuallyWrittenSerialization]`
present, exactly the five 0.1.0 instance fields (m_mod,
m_scheduler, m_calendar, m_library, m_gameIds), SerializeData
writes only `m_calendar`, the same four RegisterResolvedMember
calls. The save surface of the mod did not change.

## Artefacts

- dist/QoSBuilder-mod-0.2.0.zip — 47,373 B — sha256
  680d81eac4e68afa5d8e7a1354f5548891b63e56b33dffe07c4a74aa03005131
  (QoSBuilder.dll 37,888 B + manifest/config/readme; no QoSCore.dll
  inside — the build script's guard passed)
- Build: `qos/build/build-builder.sh` — 0 warnings / 0 errors.
  Suite build untouched; QoSCore recompiled only as the project
  reference, sources unchanged.
- ALL prior dist artefacts (31 zips + coi.exe, checksums recorded
  before the build) sha256-verified byte-identical afterwards.

## Observations on the existing builder code (not fixed — stage scope)

1. v1's malformed-field refusal passes `jobId: null` even when the
   file's jobId was extractable (the refusal call site passes
   literal nulls), so a malformed v1 job is not added to
   processed.txt under its id. Harmless (a re-delivered copy is
   refused malformed again), but the designation path's decision 6
   diverges deliberately.
2. v1's anchor sanity uses `Math.Abs(long)`, which throws on
   `long.MinValue`; such a job would escape as an unexpected
   pass-level error and be left in place rather than refused.
   Pre-existing; the new code uses range comparisons. One-line
   fix candidate for a future round.

## NOT verified (live proof deferred to the stage-4 shakedown)

- The first real designation job end to end (day-end pass → batch
  processed → cells visible in the /designations read-back diff —
  exact for flat AND sloped cells now that bridge 0.6.6 serves
  corners).
- Manager behaviour the note flagged: blocked-cell skips,
  replace-any-kind semantics, remove-result emptiness — all
  read-back questions, none block execution design.
- Save + reload with QoSBuilder 0.2.0 installed (same proof class
  the suite still owes; the serializer surface is unchanged, so
  the expectation is equivalence with 0.1.0).
