# QoL mod prep — challenge, refinement, recon, readiness

Philip's five quality-of-life mod ideas (filed 2026-11-04 in
QOL-MODS.md), put through an adversarial pass and grounded in
assembly recon so each is READY to build — or honestly marked with
what it still needs. No gameplay-mod code exists yet; every mod
still gets Philip's own go-ahead before building.

Method: five parallel recon spikes against Philip's shipped 0.8.8
assemblies (ilspycmd + the bridge/recon reflection harness), the
vendor community mod sources, commands.json, and the companion's
own data stores. Full evidence trails in `qol-recon/01–05`
(chickens, worldmap, train-cargo, advisor, productivity).
Claims below cite exact type/method names from those files.

Standing context: CoiBridge stays read-only — all five of these
are separate gameplay/UI mods or companion features; the bridge
only ever gains read routes. SPEC §12's autonomy discipline
(resource-cost actions are never autonomous) applies to anything
the companion might one day trigger on Philip's behalf.

---

## 1. Better chicken distribution

### (a) The challenge

- **The naive version solves a non-problem.** Chickens are a
  `VirtualProductProto` (`Ids.Products.Chicken`, `isStorable:
  false`). No storage, truck, ship, belt or port can ever carry
  one; a chicken exists only as the integer `AnimalFarm.
  AnimalsCount`. Any design framed as "distribution logistics"
  is dead on arrival — there is no transport leg to improve.
- **Vanilla already ships the even-distribution algorithm.**
  `MoveAnimalsCmd {farmId, numberToMove}`, processed by
  `FarmCommandsProcessor`, moves animals 1-by-1 round-robin
  across all other constructed, enabled, same-species farms.
  An "even distribution button" duplicates shipped behaviour.
- **The real skew is dynamic, so one-shot fixes decay.**
  Breeding adds 4 per 100 animals/month (fractional daily
  accumulator in `onNewDay`) — big farms pull away in absolute
  terms; slaughter culls at each farm's `SlaughterLimit`;
  quick-trade arrivals fill farms greedily in entity-enumeration
  order — the exact opposite policy to the move command's
  round-robin. The game itself holds two conflicting policies;
  arrivals will re-skew whatever a rebalance sets.
- **Automation can actively destroy value.** Moving animals
  INTO a farm at its slaughter limit converts them to carcasses
  at 0.5× the next day. Moving them into a farm whose feed
  pipeline isn't running starves them (animals have feed/water
  needs and die). Moving them OUT of a farm the player is
  deliberately consolidating (or about to demolish) fights the
  player's intent. A dumb equaliser is worse than the manual
  ±50 buttons it replaces.
- **No exploit, but no free lunch either.** Moves are instant
  teleports in vanilla, so automation inherits no new balance
  hole; the risk is purely hostile automation, not cheating.

### (b) The refined design

A policy/scheduler layer over shipped commands — not a
transport system, not a re-implementation of moving:

- A small mod service (global dependency) evaluates same-species
  farm counts on a slow cadence (daily) and issues the game's
  own `MoveAnimalsCmd`s to converge toward the configured
  target. Command dispatch keeps it on the game's own mutation
  path (save/replay-safe).
- Modes: **Equalise** (all enabled farms within ±1), **Fill-first**
  (explicit farm priority order: primary farm to target, then
  overflow), **Off** (default until Philip picks one).
- Hard guards, all from recon facts: never move into a farm at
  or above its `SlaughterLimit`; never move into a farm that
  isn't constructed + enabled; respect `IsGrowthPaused` farms
  (excluded unless the mode says otherwise); never drain a farm
  below a configurable breeding-nucleus floor; cap moves per
  day so the player can see it working and intervene.
- Deliberately does NOT: touch quick-trade arrival policy
  (greedy fill is the game's economy; intercepting
  `QuickTradeCmd` processing means replacing a game system — a
  different risk class); add transport; special-case chickens
  (it keys off `AnimalFarmProto.Animal`, so it generalises —
  though chicken is the only species shipped in 0.8.8).

### (c) Recon (full trail: qol-recon/01-chickens.md)

- `AnimalFarm`: `AnimalsCount`, `CapacityLeft`, `SlaughterLimit`,
  `IsGrowthPaused`; public clamping mutators `AddAnimals(int)` /
  `RemoveAnimals(int)`. Only shipped proto: `ChickenFarm`,
  capacity 500, animal = Chicken.
- Arrival path (sole source): `WorldMapManager.Invoke
  (QuickTradeCmd)` → `QuickTradeProvider.QuickBuy()` →
  `AnimalQuickTradeHandler.StoreBoughtProduct` — the sole
  `IVirtualProductQuickTradeHandler` implementer. `CanBuy` sums
  `CapacityLeft` over enabled same-species farms, then fills
  greedily in enumeration order. Instant — no ship, no dock.
- Breeding/death: `AnimalFarmsData` + farm `onNewDay`: 4/100/mo
  growth, ageless animals, fractional accumulator; feed/water
  needs with starvation deaths.
- UI today: `AnimalFarmInspector` with hardcoded ±50 buttons
  and a −10000 "remove all" — the manual wrangling Philip
  wants gone.
- NOT FOUND: any `AnimalsManager`/distribution service; any
  contract, ship, loot or research path granting animals
  (contracts manager decompile has no virtual-product handling;
  the complete `AddAnimals` caller set is the handler + the
  move processor). One INFERRED point: entity enumeration order
  (= quick-trade fill priority) assumed to be creation order;
  not observed live.

### (d) Readiness

**Ready to build** — the smallest and most completely reconned
of the five. One decision for Philip, askable at build time:
default mode + where the toggle lives (in-game setting vs
companion). Also worth confirming with him that the pain is the
post-arrival/birth skew (the automation gap) — the original
idea's "exact pain point" is now precisely framed.

---

## 2. World map dropdown for mines/rigs

### (a) The challenge

- **The spec targets types that don't exist.** There is no
  `OilRig` entity: `WorldMapEntitiesData.addOilRig()` builds a
  plain `WorldMapMineProto` (CrudeOil, reserve 1,000,000, max
  level 16). There is no per-mine "ship access" flag either —
  it's `TravelingFleetManager.CanRequestLocationVisit
  (location)`. A design specced against imagined types dies in
  the first hour; the corrected target is simpler (one entity
  type covers mines AND rigs).
- **The obvious template is the fragile one.** DoubleQoLMod —
  the community's QoL mod — is Harmony postfixes injected into
  legacy `Mafi.Unity.UiFramework` views. That stack is what the
  game is migrating away from; copying it means inheriting the
  highest patch-fragility in the ecosystem.
- **The companion cannot finish this job.** Bridge routes are
  read-only GETs; nothing shipped demonstrates a `[WebRoute]`
  handler touching UI services, and a browser page cannot
  centre the in-game world map or open the mine inspector.
  Anyone who scopes this as "bridge-able" delivers a list that
  can't do the thing the list is for.
- **Fullscreen stacking is unproven.** The world map is an
  immersive fullscreen window; a mod window must coexist with
  it, and whether a bare `TryActivateFor` on a mine places the
  inspector cleanly while the map is open — or needs
  `WorldMapWindow.SelectLocation`'s bookkeeping — is only
  answerable live.
- **Scope creep by design gravity.** "Interact with" will try
  to grow into remote shift-setting/repair/upgrade. Every one
  of those actions already exists — in the inspector, one
  click away once the list has navigated you there.

### (b) The refined design

Clone the shipped, supported pattern (CoI_PriorityManager —
zero Harmony), navigation only:

- IMod shell that defers UI construction to renderer-init;
  a `WorldMinesWindow : Window` listing
  `WorldMapManager.Mines`; opened via
  `ToolbarHud.AddMainMenuButton` + `IToolbarItemController`
  (optional keybind, HarbourParty-style).
- Rows: title, product, state (`CurrentState`: Working /
  FullStorage / ResourceDepleted / NotEnoughUnity / …),
  level, quantity left (or ∞), buffer. Filters by product and
  state; sort by product/level/quantity.
- Row click = `WorldMapWindow.Controller.
  OpenAndCenterOnLocation(mine.Location)` +
  `InspectorsManager.TryActivateFor(mine)` — public calls.
  Fallback, only if live testing shows the inspector misplaces:
  replicate `SelectLocation`'s bookkeeping behind a
  probe-guarded reflection call (the ResearchQueue precedent).
- Deliberately NOT: no remote actions in the list (the
  inspector owns shifts/repair/upgrade/ship orders — all
  `InputCommand`s keyed by `EntityId`); no Harmony; no protos;
  no save state (pure view → add/remove freely, like
  CoiBridge).
- Optional precursor, explicitly not the feature: a
  `GET /coi/v1/world-mines` bridge route (WorldMapManager is
  already constructor-injected in BridgeRoutesV6) giving the
  companion a reference list.

### (c) Recon (full trail: qol-recon/02-worldmap.md)

- `WorldMapManager.Mines`: public `IReadOnlySet<WorldMapMine>`;
  also `AllMinableProducts`, `OnWorldEntityCreated`,
  `Map.Locations`. Concrete `WorldMapEntity` subclasses are
  exactly three: `WorldMapMine`, `WorldMapVillage`,
  `WorldMapCargoShipWreck`.
- `WorldMapMine` row state is all public: `Product`,
  `QuantityAvailable` (null = unlimited), `CurrentState`,
  `Level`, `ProductionStep/MaxProductionSteps`, `Buffer`,
  `IsRepaired` (= `IsOwnedByPlayer`),
  `CustomTitle`/`DefaultTitle`.
- Interaction model today: pin click →
  `WorldMapWindow.SelectLocation(location)` (public) →
  `InspectorsManager.TryActivateFor(entity, out inspector)` →
  `WorldMapMineInspector` (shifts slider issuing
  `SetShiftsCountForMineCmd`; repair / ship-orders / upgrade
  panels; processors: `WorldMapManager`,
  `WorldEntitiesCommandsProcessor`).
- Templates (VERIFIED-FROM-SOURCE, vendor/community):
  CoI_PriorityManager — `[GlobalDependency]` `Window` subclass,
  filter/sort/paginated rows, toolbar button, row click =
  `camera.PanTo(...)` + `inspectors.TryActivateFor(building)`,
  zero Harmony. HarbourParty — custom window, F4 via
  `KeyBindings.FromKey`, toolbar button. On-Demand-Contracts —
  custom `Window` list, Harmony used only to place its launch
  button. DoubleQoLMod — Harmony `APatcher<P>` postfixes into
  legacy views; renders no standalone list window (its one
  custom window was removed upstream).

### (d) Readiness

**Ready to build**, with four live-verification items that are
first-hour checks once Philip is at his PC, none of which block
starting: (1) inspector placement from a list click vs
`SelectLocation` bookkeeping; (2) resolving
`WorldMapWindow.Controller` from mod DI (expected fine,
unproven); (3) window stacking against the fullscreen map;
(4) a free toolbar/keybind slot (the game's map button is
order 520). Biggest of the five in effort — a new discipline
(Unity UI) — smallest in design risk, because the shape is
proven by a shipped mod.

---

## 3. Trains: remove cargo with Unity

### (a) The challenge

- **The name lies about the semantics.** The game's own clear
  method, `CargoWagon.SubCargoWagon.DestroyCargo()`, routes
  the cargo through `AssetTransactionManager.StoreBuffer`:
  products go INTO available global storage where the product
  allows it, and are truly destroyed (`DestroyReason.Cleared`)
  only for discardable products or when no storage takes them.
  (The truck equivalent's tooltip says exactly this.) A mod
  that promises "delete cargo" actually delivers "teleport to
  storage, delete the rest" — unless the result is reported
  per product, the player will think goods vanished that are
  sitting in storage, or watch storage silently fill.
- **The buffer is easy; the JOB is the hazard.** At load time
  `Train.ReportLoadedQuantity` increments
  `TrainNetworkJob.LoadedSoFar`; `DestroyCargo()` does not
  decrement it, and the job holds `ReservedJobs` entries at
  both the supply and demand station groups. Clear mid-job
  naively and you get a phantom delivery (the job "completes"
  at demand with empty wagons) or a stuck job with leaked
  reservations that block stations for every other train.
  Silent network-state corruption is the worst failure class
  of all five ideas.
- **The station has a grip on the wagon.** During
  LoadingOrUnloading the station module holds the wagon
  (`AlignedStation` / `RequestWagonUnload`); clearing under it
  races the station's own unload logic. Depot replacement and
  scrapping flows are the game's own DestroyCargo callers —
  a third clearer must not interleave with them.
- **Flat pricing underprices the fix.** Vanilla's reference
  points: truck discard is a flat 0.1 Upoints — but a truck is
  small, and vanilla gates it behind a cannot-deliver
  notification (it is a rescue, not a convenience).
  `RecoverTrainCmd` prices per car (cars × 0.2);
  `QuickRepairTrainCmd` sums per-loco costs. A train carries
  10–30× a truck; at flat 0.1, "load wrong → clear → reload"
  becomes cheaper than building correct wagon filters, and
  Unity — the game's scarcest meta-resource — leaks value.
- **The trigger has an unverified foundation.** Every shipped
  `InputCommand` carries `[GenerateSerializer]` for replay
  persistence; no community mod in the vendor set defines a
  custom command, so whether mod assemblies get working
  serializers is unverified. Build the feature on a custom
  command without testing that first and the failure mode is
  a save/replay determinism problem discovered late.

### (b) The refined design

Port the truck feature honestly — priced per car, gated on
state, explicit about the job:

- Action: "Clear cargo" on the inspected train. Cost =
  0.2 × number of cargo cars (`RecoverTrainCmd` precedent),
  floor 0.5 Upoints, charged through the exact shipped chain:
  `IUpointsManager.CanConsume` → clear →
  `ConsumeExactly(IdsCore.UpointsCategories.QuickRemove,
  cost)`. (`TryConsume` where a bool flow fits better; never
  call `ConsumeExactly` unaffordably — it asserts.)
- Eligibility, all required: train spawned; not travelling;
  `StateForUi` not LoadingOrUnloading; not in depot
  replacement/scrap flows.
- Job handling is explicit, never implicit: if the train has
  an active `TrainNetworkJob`, the action first finalises it
  as cancelled — `TrainNetworkJob.Finalize(asCancelled:
  true)` is public and removes the reservations and the job
  (it also removes the train from its line; the UI must
  disclose that before confirming) — or schedules the vanilla
  `CancelTrainJobCmd`; only then `DestroyCargo()` per
  sub-wagon. Jobless/manual trains clear directly.
- Feedback: capture `GetMixedBreakdown` before clearing and
  report per product what went to storage vs what was
  destroyed. No silent outcomes.
- Trigger: the custom-`InputCommand` route (a
  `[GlobalDependency(AsEverything)]` `ICommandProcessor<T>` —
  dispatch is resolver-driven via `InputScheduler` →
  `DependencyResolver.InvokeActionHierarchy`, so the shape is
  plausible) ONLY after the serializer question is answered
  by a sandbox persistence test. Fallback triggers (game-loop
  service, keybind) are documented in the recon file with
  their replay caveats.
- Deliberately NOT: no clearing while moving; no other
  vehicle types (trucks already have theirs); no per-wagon
  partial selection in v1 (whole train only); no free tier.

### (c) Recon (full trail: qol-recon/03-train-cargo.md)

- Cargo: `CargoWagon.SubCargoWagon` (Mafi.Core.Trains) —
  `CargoBuffer: Option<IMixedProductBuffer>`; contents via
  `Cargo` → `ProductQuantity`; `GetMixedBreakdown` for mixed
  loads (0.8.8 mixed materials). `DestroyCargo()` is public;
  shipped callers are `TrainsManager.DestroyTrain` and the
  depot replacement transfer (which DestroyCargos whatever
  mixed cargo it cannot transfer).
- Vanilla discard verdict: trains — NONE (no command in the
  240-command inventory; `TrainInspector` has no discard UI).
  Trucks — YES: `DiscardVehicleCargoCmd` →
  `VehicleCommandsProcessor.Invoke` resolves the concrete
  `Truck` only via `TryGetEntity<Truck>` (a Train is not an
  entity and a CargoWagon is not a Truck, so trains are
  unreachable by construction), gates on the truck's
  cannot-deliver notification, charges flat 0.1 Upoints.
- Unity spend: `Mafi.Core.Population.IUpointsManager` —
  balance via `Quantity`; `bool CanConsume(Upoints)`;
  `bool TryConsume(Proto.ID categoryId, Upoints, …)`;
  `void ConsumeExactly(…)` (asserts if unaffordable).
  Shipped chains to imitate: truck discard
  (`CanConsume` → `TryDiscardCargo` → `ConsumeExactly
  (QuickRemove, 0.1)`); QuickTrade (`WorldMapManager.Invoke
  (QuickTradeCmd)` → `QuickTradeProvider.QuickBuy()` →
  `CanAfford` incl. `CanConsume(UpointsCost)` →
  `ConsumeExactly(QuickTrade, UpointsCost)`);
  `QuickRepairTrainCmd`; `RecoverTrainCmd`
  (`TryConsume(VehicleRecovery, cars × 0.2)`).

### (d) Readiness

**Needs-a-decision (×2) + one sandbox experiment**, then
ready:

1. Job-cancellation disclosure: clearing a working train also
   cancels its job and pulls it off its line. Acceptable — or
   restrict v1 to jobless/manual trains only (the safer scope
   cut, recommended for v1)?
2. Price: per-car 0.2 with a 0.5 floor (proposed) vs Philip's
   own number.
Experiment (gating): custom-command serializer persistence
in a sandbox save — the same class of test as SANDBOX-
PROTOCOLS Protocol B. Code size is small; the decisions and
the experiment are the schedule.

---

## 4. Train network advisor

### (a) The challenge

- **The payload can't express the questions.** There are no
  schedule-item types: `ITrainScheduleItem` has exactly ONE
  implementer, `TrainStationScheduleItem`, where load/unload
  lives in `DisableLoad`/`DisableUnload` flags plus
  `LoadOnlyProducts`/`UnloadOnlyProducts` filter sets. The
  bridge's trains-detail per-item `{type}` is therefore
  information-free. And NO existing route serves stations or
  networks at all. An advisor built on today's payloads can
  only repeat "stations exist, zero lines" forever.
- **The utilisation metric has four independent traps.**
  (i) Two denominators — the inspector headline % uses the
  categorised `Data.Total`; the chart uses the 600-report
  bucket — and mixing them manufactures percentages from
  nothing. (ii) `TotalRollingHalf` is halved by the game, so
  absolute counts read off it are 2× wrong. (iii) The ongoing
  bucket is partial by definition. (iv) At `Version == 0` the
  `OngoingMonthTicks` definition itself changes (categorised
  total only). Add rings that enumerate newest-at-index, and
  a careless busy-% is wrong in five ways at once.
- **"Busy" alone gives harmful advice.** Under the game's
  full-load dispatch, a saturated network and a badly
  filtered one can show identical activity shares; a train
  can be 100% Working while hauling a trickle (hence the
  load-factor gate below). And `BlockedByTrain` share
  measures network topology, not train shortage — answering
  congestion with "add trains" makes the actual problem
  worse. Any advisor that can't distinguish saturated /
  congested / misconfigured must not speak.
- **Product-only matching suggests impossible lines.** A
  producer/consumer pair matched on product alone ignores
  wagon class (fluid product, loose wagons = uncarriable),
  station direction profiles, and distance. The
  highest-trust gap signal available anywhere is the game's
  OWN network product net (`TrainNetwork.
  RebuildProductInfo`), not a companion reconstruction —
  the design ranks it first for that reason.
- **Validation debt is real.** Every counter the utilisation
  leg reads is served flagged `unverifiedDecoding: true`
  (v0.6.4 discipline). Shipping add/remove-train advice on
  an unverified decode would break the project's own
  labelling rules.

### (b) The refined design

Companion-first (Philip's direction, confirmed in
QOL-MODS.md), in three layers with independent launch gates:

- **Layer 0 — buildable today, current routes.** A
  cluster-level gap page: producer/consumer clusters from
  products/sources + machines + topology + flows; an
  orphan-station census at proto level (today's truth: 40
  station roots, 167 modules, 6 fuel stations, 0 lines —
  everything unserved), labelled product-blind where it is
  product-blind.
- **Layer 1 — after one bridge round adds S-STATIONS /
  S-SCHEDULE / S-NETWORKS / S-WAGONS (all surfaces verified
  public; shapes in the recon file).** Station-level signals
  G1a–G10, ranked: G7 first (network product-net imbalance —
  the game's own arithmetic: supply with no in-network
  demand, demand with no in-network source), then dead
  visits (G5a filter-excludes-everything, G5b direction
  disabled), uncarriable products (G5c), unassigned-station →
  network matching (G4, ranked by products matched →
  direction complementarity → topology distance), unserved
  producers/consumers (G1a/G2a), empty pickups (G5d, needs
  ≥3 polls of persistence), fuel gaps (G8), missing waiting
  bays (G9), station saturation (G10).
- **Layer 2 — utilisation, gated on live verification.**
  Busy share per network G over member trains t:
  `busy_G = Σ_t A_W(t) / Σ_t Total_W(t)` (activity category
  A = Working over the categorised total A+B+C+D), summed
  over the k ≤ 12 most recent COMPLETED 600-report buckets
  (k = `fullMonthlyRecordsCount`), trains with Version ≥ 1
  only. The ongoing bucket never enters a verdict; the
  paused share is computed on the 600·k bucket denominator;
  `totalRollingHalf` is banned. Congestion:
  `blocked_G = Σ D_travel / Σ Total_travel`. Bands with
  hysteresis — SATURATED ≥0.85 in / <0.80 out; UNDERUSED
  <0.35 / ≥0.40; IDLE <0.15 / ≥0.20; CONGESTED ≥0.10 /
  <0.07. "Heavily used" (→ suggest more capacity) requires
  SATURATED **and** load factor ≥0.50 (load factor from
  S-WAGONS capacities vs the flows the network exists to
  move). CONGESTED routes advice to topology/stations,
  never to "add trains"; UNDERUSED + low load factor routes
  to schedule/filter checks before "remove trains".
- Deliberately NOT: no automatic line creation — suggestions
  are evidence cards; applying anything stays a prepared,
  confirmed act. No percentages from `totalRollingHalf`. No
  station-level claims before Layer 1 routes exist. No
  utilisation verdicts before the cross-check passes.

### (c) Recon (full trail: qol-recon/04-advisor.md)

- Stations: no single `TrainStation` entity — four concrete
  classes over one abstract base, grouped by
  `TrainStationManager.TrainStationGroups` into
  `TrainStationGroup`s. Modules expose `StoredProduct` /
  `IsForLoading`; station roots carry `ArrivalRules`;
  groups expose supply (`GetNetworkSupply`) and
  `GetRemainingTrainSlots()`.
- Schedule: `TrainStationScheduleItem` is the sole
  `ITrainScheduleItem` implementer — `DisableLoad`,
  `DisableUnload`, `LoadOnlyProducts`,
  `UnloadOnlyProducts`, `DepartConditions`,
  `SkipIfFuelHigherThan`; refuelling is implicit via
  fuel-station roots. Line membership: `TrainLine.
  IsStationInSchedule` exists for orphan checks.
- Networks: `TrainNetwork.RebuildProductInfo(CargoStations,
  dict, true)` is public — the same call
  `TrainNetworkProductsTab` makes (the underlying
  `m_products` is private with no getter, so the bridge
  must recompute it the game's way). `HadNoWaitingBay` is
  public. The `TrainNetworksManager` enumeration property
  is the one surface still to pin at build time.
- Capacity: `Train.CargoWagons[]` exposes `Capacity` /
  `Cargo` / `AcceptsProduct(p)` — reachable only via the
  proposed S-WAGONS payload extension.
- Counters: ProductivityCounter family per MODDING-NOTES
  recon round 4 (b) — 600-report buckets, category tables,
  the two denominators, the halved total, the Version==0
  quirk — all reused by the metric definitions above.

### (d) Readiness

**Split verdict.** Layer 0: **ready to build** today.
Layer 1: **ready to build** as one bridge+companion round
(surfaces verified; one enumeration property to pin).
Layer 2: **blocked on live trains** — Philip's save has zero
lines, so the 10-step cross-check protocol in the recon
file (two trains on one shuttle, Version ≥ 2, compare
headline %, ongoing bucket, ring order, blocked share
against the TrainInspector) cannot run until his rail phase
starts. Decision for Philip: ship Layer 0 alone or wait and
build 0+1 together — recommendation: 0+1 together; Layer 0
alone is thin.

---

## 5. Productivity alerts

### (a) The challenge

- **Today there is no time series to measure over.** The
  companion's store (coi.db) holds exactly ONE snapshot —
  tick 2787522, in-game 388-2-27, taken while PAUSED — with
  111 product_stats rows, 288 product_history rows (36
  pinned products × 8 months) and 17 issues, and NO
  machine-state data at all: bridge census/machines are
  fetched live per page render in serve.rs and never
  persisted. SPEC's `coi watch [--interval 5s]` was never
  built — the subcommand is not in the binary's dispatch.
  A "monthly productivity" alert shipped now would be
  arithmetic over a single paused instant; the only honest
  output for every proto today is INSUFFICIENT EVIDENCE.
- **Snapshot sampling is biased sampling.** Instantaneous
  states polled at human-chosen moments misread
  systematically — the one snapshot that exists was taken
  paused, when everything reads idle-ish. Without thinning
  (count samples ≥720 ticks apart) and span/gap rules, a
  big N is vanity, not evidence.
- **The nameplate leg has an uncalibrated unit bridge (κ).**
  Dataset rates are per-minute; game flows are per-month
  counters; naive calendar conversion overshoots the
  observed steel/diesel flows by roughly five orders of
  magnitude. Any absolute utilisation % computed today
  would be confident nonsense. (The dataset is also 0.8.7d
  against game 0.8.8.) κ-free forms exist — self-relative
  (a row vs its own best month) and installed-rate share,
  where κ cancels — and are the only honest outputs until
  κ is calibrated.
- **Attribution is weaker than it looks.** What a machine
  produced is not stored per machine. `products/sources`
  gives period rows only at capture time (today: captured
  for steel + diesel, at one moment). Census `recipes` is a
  UNION of `RecipesAssigned` across instances — assigned ≠
  producing — worst exactly where it matters most:
  AssemblyElectrifiedT2, 33 instances running 12 recipes.
  And 13 of 104 live recipe ids fail the dataset name-join —
  ALL of them Assembly's — so naive joins silently drop the
  biggest proto in the run. Splitting a product's total
  across protos by assumed shares is fabrication; the
  honest fallback is chain-level.
- **"Low productivity" conflates opposite diseases.**
  NotEnoughInput (starved — fix upstream), OutputFull
  (blocked — fix downstream), and paused (the player's own
  choice — not an alert) demand opposite interventions; a
  single number mixing them sends Philip to the wrong end
  of the chain. Whole categories are also structurally
  invisible: FarmT2 ×8, ships and mine towers never appear
  in census `machines` at all. Seasonal/bursty producers
  (farms, boats) read "idle" most of the month BY DESIGN —
  the companion's existing `is_seasonal`/`seasonal_net`
  logic is the precedent — and import-fed products (his
  coal: produced TM = 0, healthy) break any naive
  production-zero rule.
- **Alert fatigue is the shipping risk.** Without
  CI-clearance firing, hysteresis and a confirmation leg,
  this becomes a wall of red that gets muted — the standard
  death of monitoring systems, and of trust in the Issues
  board generally.

### (b) The refined design

Two estimators, one label discipline, composed with the
existing detectors (productivity attaches cause evidence to
existing stall/deficit issues instead of opening duplicates):

- **Evidence classes on every output, no exceptions.**
  MEASURED: N_eff ≥ 96 counted machine-samples per proto
  (95% Wilson half-width ≲ ±0.10 worst case), window span
  ≥ 27/30 days, max sampling gap ≤ 3 days. ESTIMATED:
  N_eff ≥ 30, span ≥ 24/30 days, gap ≤ 7 days.
  INSUFFICIENT: anything less → the proto is listed as
  "insufficient evidence (N_eff=…, span=…)", NEVER as
  healthy. N_eff = snapshots ≥720 ticks apart × eligible
  instances. Year windows use the same thresholds.
- **Estimator (a) — working-state share** (needs the
  prerequisite below). Fire only when the interval clears
  the line: 95% Wilson CI upper bound < 0.50 working share;
  clear on a later window whose CI lower bound > 0.60;
  0.50–0.60 is the hysteresis dead zone. Requires ≥3
  instances and a confirmation leg (second half of the
  window agrees, or estimator (b) chain utilisation
  < 0.60). Cause is named from the state mix with share
  thresholds — STARVED / BLOCKED / MIXED / UNCLASSIFIED —
  except PARKED (≥50% paused share), which is reported as
  parked and is not an alert.
- **Estimator (b) — flow vs nameplate, κ-free first.** The
  retrospective leg works today: game flow aggregates are
  exact counts (finished `lastMonth`; `thisMonth` is
  partial — his capture sits at day 27). Ship only κ-free
  forms, labelled `nameplate-unverified`: self-relative
  utilisation (row vs its own best month) and share of a
  product's installed rate. Absolute % waits for κ
  calibration (sandbox single-machine run, or a
  cross-month fit κ̂ = actual / (rate × n ×
  working-share × months)) and for the recipe join to be
  rebuilt on (proto, output, material-input signature)
  instead of the broken name-join.
- **Exemptions built in**: seasonal/bursty products via
  `is_seasonal`/`seasonal_net`; import-fed products exempt
  from production-zero logic; protos whose machines never
  appear in census (farms/ships/towers) are out of scope
  for (a) and named as such.
- **Prerequisite build (small, companion-side, part of
  this design — not an optional extra):** persist census/
  machines state aggregates into the snapshot store, plus
  a real polling cadence while Philip plays (the `watch`
  habit). Until roughly a month of cadence exists,
  estimator (a) can only ever say INSUFFICIENT — by design.
- **High productivity is not an alert.** A ≥90% "running
  hot" line in a digest note. (An absolute reading >100%
  would indicate a calibration bug, not a triumph — it
  should never be celebrated by the tool that computed it.)

### (c) Recon / data audit (full trail: qol-recon/05-productivity.md)

- coi.db (read directly): 1 snapshot (tick 2787522,
  388-2-27, paused), 111 product_stats, 288 product_history
  (36 pinned × 8 months), 17 issues; schema has games /
  snapshots / product_stats / product_history / issues /
  audit — no census or machine-state tables.
  `snapshot_json`/`fetch_live` pull vanilla product routes
  only; serve.rs fetches bridge census/machines per render
  and discards them.
- Flows: `/api/v1/products/stats` arrays are
  [thisMonth, lastMonth, thisYear, lastYear, lifetime];
  history covers pinned products only.
- Sources: `/api/v1/products/sources` (recipe+machine rows,
  entity rows, fuel rows) is the attribution substrate;
  in the current captures it exists for steel + diesel at
  one moment.
- Census semantics (BridgeRoutes.cs): machine states are
  per-proto aggregates per scan; `recipes` is the union of
  `RecipesAssigned` over instances. The v0.6.4 machines
  route gives per-instance states live, but nothing
  persists them.
- Dataset: `~/workspace/coi-blueprint-pack/reference/daxfb/
  coi-data-0.8.7d.json` — per-minute recipe rates;
  13/104 live recipe ids fail the id/name join (all
  Assembly); version skew 0.8.7d vs game 0.8.8.
- Precedents in coi/src/main.rs: `is_seasonal` /
  `seasonal_net` (finished-history balance or lastYear÷12
  with a basis string); the seasonal gating of the stall /
  deficit detectors and slope forecasts.

### (d) Readiness

**Ready after one prerequisite build + one calibration.**
(1) The store change (persist state aggregates per
snapshot) + a snapshot cadence habit — estimator (b)'s
κ-free form can ship immediately on finished-month flows;
estimator (a) becomes honest roughly one in-game month of
cadence later. (2) κ calibration before any absolute
nameplate %. The only thing needed from Philip is
accepting the cadence habit (snapshots while playing) —
no design decision outstanding.

---

## Recommended build order

1. **Chickens (§1)** — smallest, most completely reconned,
   built entirely on shipped commands; it also exercises the
   policy-layer pattern the other game mods learn from.
   Decision (mode default, toggle location) askable at
   build time.
2. **Productivity alerts (§5)** — companion-side, no game
   mod, immediate value from estimator (b) on finished-month
   flows; and its prerequisite (snapshot state persistence
   - cadence) starts the evidence clock that estimator (a)
   — and the advisor's persistence signals (G5d/G10) —
   both need.
3. **World-map dropdown (§2)** — self-contained Unity UI
   mod on a shipped template; independent of everything
   else; its four unknowns are first-hour live checks.
4. **Train cargo for Unity (§3)** — the code is small, but
   the gating sandbox experiment (custom-command
   serializer) and Philip's job-cancellation decision put
   it behind the others; it also matters more once his
   rail phase is actually running.
5. **Train advisor (§4)** — Layers 0+1 can be built as one
   bridge+companion round and may be pulled forward to
   position 3–4 if Philip wants the rail tooling early
   (it's the idea closest to his stated play direction),
   but Layer 2 is blocked until trains exist to verify
   against. Building the full advisor first would mean
   validating utilisation against a synthetic network —
   the wrong order.

Cross-cutting: §5's snapshot persistence is shared
infrastructure (§4's persistence signals consume it);
§1's command-dispatch pattern de-risks §3's trigger
question; §4's S-WAGONS payload feeds §3's cargo surfaces;
§2's UI-window pattern is the future in-game home if the
advisor ever wants one. CoiBridge's read-only posture is
untouched by all five.

Version note: all recon is against the 0.8.8 assemblies.
A game patch re-opens surface verification — most of all
for the Unity UI mod (§2), least for the companion-side
work (§4, §5).
