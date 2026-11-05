# CoI modding — verified API notes (2026-11-02)

Facts below were taken from **shipped, working mods and official MaFi
sources**, not inferred. Collected after Philip pointed out the material
had to exist publicly. Local copies of the community sources are under
`../vendor/community/` (reference only — do not modify or redistribute).

## Sources

- Official modding repo + README: `../vendor/Captain-of-industry-modding/`
  (lifecycle, manifest fields incl. `can_add_to_saved_game`,
  `mod_dependencies`, deploy targets, COI_ROOT setup).
- Captain's Diary #57 (coigame.com/Blog/cd-57): the `[WebRoute]` pattern —
  a method on a global dependency returning string/HTML/JSON/binary.
- Changelog v0.8.8, Modding section (coigame.com/Changelog): "Mods can
  serve web API routes by marking a method of a global dependency with
  [WebRoute], or register routes of other objects with RegisterRoute."
  Also: JsonWriter (fluent) replaces WebJsonWriter; key bindings can be
  added in IMod.Initialize or a global dependency's constructor;
  IPlainProductBuffer / IMixedProductBuffer are the 0.8.8 buffer
  interfaces; [CheatCommand] fails outside sandbox.
- Community mods (GitHub, cloned to `../vendor/community/`):
  iScripters/CoI_PriorityManager, FelixZett/captain-of-industry-data-exporter,
  Jagg111/COI-ResearchQueue, shuntianyifang/coi-balancer,
  DoubleMa/DoubleQoLMod.

## The IMod contract (verified in 4 independent mods)

```csharp
public sealed class MyMod : IMod {
    public MyMod(ModManifest manifest) { Manifest = manifest; JsonConfig = new ModJsonConfig(this); }
    public ModManifest Manifest { get; }
    public bool IsUiOnly => false;
    public Option<IConfig> ModConfig => Option<IConfig>.None;
    public ModJsonConfig JsonConfig { get; }
    public void RegisterPrototypes(ProtoRegistrator registrator) { }
    public void RegisterDependencies(DependencyResolverBuilder depBuilder, ProtosDb protosDb, bool gameWasLoaded)
    {
        depBuilder.RegisterDependency<MyService>().AsSelf();
    }
    public void EarlyInit(DependencyResolver resolver) { }
    public void Initialize(DependencyResolver resolver, bool gameWasLoaded) { }
    public void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) { }
    public void Dispose() { }
}
```

- Namespaces: `IMod`, `ModManifest`, `ModJsonConfig` ∈ `Mafi.Core.Mods`;
  `ProtosDb` ∈ `Mafi.Core.Prototypes`; `DependencyResolver`,
  `DependencyResolverBuilder`, `Option`, `GlobalDependency`,
  `RegistrationMode` ∈ root `Mafi`; `Dict` ∈ `Mafi.Collections`.
- The bool in RegisterDependencies/Initialize is `gameWasLoaded`
  (new game vs loaded save) — initialization must cover both.
- **DataOnlyMod hybrid** (BalancerPlanner): `class X : DataOnlyMod, IMod`
  with *explicit* `IMod.RegisterDependencies` / `IMod.Initialize` /
  `IMod.IsUiOnly` / `IDisposable.Dispose` members — DataOnlyMod alone
  exposes no dependency registration. CoiBridge uses this shape.
- Registration alternatives, both seen in shipped mods:
  `depBuilder.RegisterDependency<T>().AsSelf()` (PriorityManager),
  `depBuilder.RegisterAllTypesImplementing<TInterface>(assembly)`
  (CaptainOfCheats), and the `[GlobalDependency(RegistrationMode.AsEverything)]`
  attribute on the service class itself (PriorityManager controllers —
  which ALSO register explicitly; belt and braces).
- Resolution: `resolver.Resolve<T>()` generic and
  `resolver.Resolve(typeof(T))` non-generic are both used.
  `resolver.Resolve<IGameLoopEvents>().RegisterRendererInitState(this, cb)`
  is the renderer-init hook (IGameLoopEvents ∈ Mafi.Core.GameLoop).

## Verified game services for M2

- **Entities**: `IEntitiesManager` ∈ `Mafi.Core.Entities` — global
  dependency; `.Entities` enumerable of `IEntity`;
  `TryGetEntity(new EntityId(int), out T)`. Concrete `EntitiesManager`
  is directly resolvable and offers `GetAllEntitiesOfType<LayoutEntity>()`
  (coi-balancer reads whole factory snapshots this way). Entities expose
  `Id` (EntityId), `Prototype.Id`, `IsDestroyed`, `IsConstructed`,
  `IsPaused`; `Machine` (Mafi.Core.Factory.Machines) adds `CurrentState`,
  `RecipesAssigned`, `PowerRequired`. → the census + entity directory.
- **Research**: `ResearchManager` ∈ `Mafi.Core.Research` — global
  dependency, constructor-injected in ResearchQueue's controller.
  Public `CurrentResearch` property returns `Option<ResearchNode>`;
  the queue is a private field (`m_researchQueue`), nodes can be cast to
  `IResearchNodeFriend` (CancelResearch etc.). Public queue read surface
  TBD from the bridge discovery report.
- **Protos**: `ProtosDb` resolvable (`resolver.Resolve(typeof(ProtosDb))`);
  `protosDb.Filter<T>()` used for catalogues. Exporter also resolves
  `GameDifficultyConfig` and `AssetsDb` the same way.
- **Not yet found in community code**: a train-network manager surface
  and a population/settlement service. The bridge `/debug/services`
  discovery report is the tool for those two.

## Verified against the shipped assemblies (2026-11-02, game 0.8.8)

Philip supplied `Mafi*.dll` from his install (there is NO
Assembly-CSharp.dll — all game code lives in the Mafi assemblies).
Decompiled with ilspycmd; the assemblies live in
`~/workspace/coi-companion/refs/` (reference only, never shipped).

- **`[WebRoute]` is `Mafi.Core.WebApi.WebRouteAttribute`** — the earlier
  guess was right about the namespace, wrong about the shape: the
  constructor is `(string path, WebRouteStage stage, string method =
  "GET", string description = "")` — **stage is required**. Diary #57's
  one-argument snippet is illustrative, not literal.
- `WebRouteStage` enum: `Background, Sim, Sync, Main`.
- Handlers return **`WebRouteResponse`** (Mafi.Core.WebApi): factories
  `.Json(string, int status = 200)`, `.Text`, `.Html`, `.Bytes`.
  (Plain-string returns also exist per the diary, but game code uses
  WebRouteResponse throughout.)
- The game's own `DemoDashboard` is the reference shape:
  `[GlobalDependency(RegistrationMode.AsSelf, false, false)]` internal
  class, **private** handler methods returning WebRouteResponse.
- **Registration mechanics**: `WebApiServer` (public, itself a global
  dependency AsSelf) keeps a `WebRouter`; on first start,
  `scanAllRoutes()` calls `AddRoutesOf(instance)` for **every instance
  in `DependencyResolver.AllResolvedInstances`**. So a mod's routes go
  live iff an instance of the route class has been resolved before the
  server starts. CoiBridgeMod.Initialize resolves BridgeRoutes; if the
  server is already running, Initialize calls the public
  `WebApiServer.ScanObjectForRoutes(object)` directly (returns count).
  Also public: `RegisterRoute(path, stage, Delegate, method, desc)`,
  `UnregisterRoute`, `UnregisterRoutesOf`, `IsRunning`, `Started` /
  `Stopped` events.
- `GlobalDependencyAttribute(RegistrationMode, bool onlyInDebug =
  false, bool onlyInDevOnly = false)`; `RegistrationMode` is [Flags]:
  AsSelf = 1, AsAllInterfaces = 2, AsEverything = 3.
- `DataOnlyMod` in this build: `RegisterDependencies` and `Initialize`
  are **non-virtual** no-ops; `RegisterPrototypes` is **abstract**;
  `EarlyInit` virtual. A DataOnlyMod + explicit-IMod hybrid (the
  BalancerPlanner shape) is therefore required, as built.
- `DependencyResolver`: `Resolve<T>()`, `Resolve(Type)`,
  `TryResolve<T>(out T)`, `AllResolvedInstances`.
  `DependencyResolverBuilder.RegisterDependency<T>()` returns a
  `DependencyRegistrar` with `.AsSelf()`, `.AsAllInterfaces()`,
  `.As<T>()`. `Mafi.Log` is the logging class (root namespace).
- **Build proof**: CoiBridge v0.4.0 compiled on the VM against these
  assemblies (`bridge/vm-build/build.sh`) — 0 warnings, 0 errors —
  and packaged as `dist/CoiBridge-mod-0.4.0.zip`. First in-game run is
  still Philip's to confirm.

## Recon round 2 (2026-11-03 overnight)

Three read surfaces hunted in the shipped assemblies (reflection harness
`bridge/recon/` + ilspycmd decompiles of `refs/Mafi.Core.dll` /
`refs/Mafi.dll`). All three RESOLVED; implemented in CoiBridge v0.6.1.

### (a) Per-entity priority values — RESOLVED

- The commands live in `Mafi.Core.Entities.Commands`:
  `SetGeneralPriorityCmd{EntityId, Priority}`,
  `SetCustomPriorityCmd{EntityId, PriorityId(string), Priority}`,
  `SetGlobalPriorityCmd{PriorityId, Priority}` (global, not per-entity).
  Both per-entity commands are processed by
  `Mafi.Core.Entities.EntitiesCommandsProcessor`:
  `Invoke(SetGeneralPriorityCmd)` does
  `TryGetEntity<IEntityWithGeneralPriority>(cmd.EntityId)` →
  `entity.SetGeneralPriority(cmd.Priority)` (the extension in
  `Mafi.Core.Entities.Priorities.EntityWithGeneralPriorityExtensions`,
  which forwards to `IEntityWithGeneralPriorityFriend.SetGeneralPriorityInternal`
  after a `GeneralPriorities.AssertAssignableRange` check, range 0–14);
  `Invoke(SetCustomPriorityCmd)` does
  `TryGetEntity<IEntityWithCustomPriority>(...)` →
  `entity.SetCustomPriority(cmd.PriorityId, cmd.Priority)`.
- **General priority storage**: `Mafi.Core.Entities.Priorities.IEntityWithGeneralPriority`
  exposes `int GeneralPriority { get; }` (+ `IsGeneralPriorityVisible`,
  `IsCargoAffectedByGeneralPriority`). The value lives on the base class
  `Mafi.Core.Entities.Static.Layout.LayoutEntity` (implements the
  interface + the Friend interface): public `GeneralPriority` getter,
  private setter, initialised in the ctor from
  `proto.Costs.DefaultPriority` and persisted in `SerializeData`.
  `Mafi.Core.Prototypes.EntityCosts.DefaultPriority` is a public readonly
  int field (ctor default 9; `GeneralPriorities.DEFAULT = 9`,
  `HIGH = 8`, assignable range 0–14, `LOWEST_PRIORITY = 15`,
  `IGNORE = 16`). So "non-default" per entity =
  `GeneralPriority != Prototype.Costs.DefaultPriority` — a per-proto
  default, not one global constant. Every LayoutEntity (machines,
  storages, transports' endpoints, buildings…) carries it; there is no
  PrioritiesManager keyed by entity id.
- **Custom priorities**: `IEntityWithCustomPriority` offers
  `GetCustomPriority(string id)` / `IsCustomPriorityVisible(string id)` /
  `SetCustomPriority(string id, int)`. There is **no enumeration surface
  for the ids** — the id set is per-class knowledge, and `Storage`'s
  implementation calls `Assert.Fail` on an unknown id, so blind probing
  of arbitrary entities is unsafe. The one fully verified implementer is
  `Mafi.Core.Buildings.Storages.Storage`: ids are the consts
  `IMPORT_PRIO_ID = "ImportPrio"` and `EXPORT_PRIO_ID = "ExportPrio"`,
  backed by the public typed properties `int ImportPriority` /
  `int ExportPriority` (both default 8 in the ctor). `TradeDock` also
  implements the interface (`CARGO_EXPORT_PRIO_ID`), not read in v0.6.1.
- **Route shape (v0.6.1)**: census JSON gains a compact `"priorities"`
  object `{entityId: generalPriority}` containing ONLY entities whose
  value differs from their proto default (keeps the 12.5k-entity census
  lean); storage rows gain `generalPriority` on every row plus
  `importPriority` / `exportPriority` for concrete `Storage` rows.

### (b) Storage slider values — RESOLVED

- `StorageSetSliderStepCmd{StorageId, ImportStep, ExportStep,
  TransportFrom, TransportUntil}` — all four are plain `int` fields in
  the shipped build (not nullable); the processor
  `Mafi.Core.Buildings.Storages.StorageCommandsProcessor.Invoke(...)`
  treats a negative value as "leave unchanged" and otherwise calls
  `entity.SetImportStep / SetExportStep / SetTransportFromStep /
  SetTransportUntilStep` with `LogisticsBuffer.ClampStepOfCmd` applied
  (clamps only above 10; steps are 0–10, `LogisticsBuffer.MAX_STEPS = 10`).
- The processor resolves the entity as the **concrete**
  `Mafi.Core.Buildings.Storages.Storage`, NOT `StorageBase` — and indeed
  the readable state lives on `Storage` (the existing bridge scan walks
  `StorageBase`, which has none of it):
  - `Percent ImportUntilPercent` → delegates to
    `Buffer.Value.ImportUntilPercent` (`LogisticsBuffer`);
  - `Percent ExportFromPercent` → delegates to
    `Buffer.Value.ExportFromPercent`;
  - `Percent TransportFromPercent` / `Percent TransportUntilPercent` →
    auto-properties on `Storage` itself (defaults 0% / 100%).
  `LogisticsBuffer` (`Mafi.Core.Buildings.Storages`) stores only the two
  percents; its `SetImportStep(int)` does
  `ImportUntilPercent = step * SingleStep` with
  `SingleStep = Percent.Hundred / 10` (10%), and symmetrically for
  export; `Storage.SetTransportFromStep(int)` does
  `TransportFromPercent = step * Percent.Hundred / 10`. There is **no
  stored step integer anywhere** — the step is exactly
  `percent / 10%`, i.e. `Percent.RawValue / 10000`
  (`Mafi.Percent`: `public readonly int RawValue`, Hundred = 100000 raw).
  Semantics: import step = "trucks keep importing until this full"
  (ImportUntil), export step = "trucks keep emptying from this full"
  (ExportFrom); setting one side resets the other (import > 0 forces
  ExportFrom back to 100%, and vice versa), and `CleaningMode` zeroes
  both — the derived steps remain exact because every write goes
  through a step setter.
- **Route shape (v0.6.1)**: storage rows for concrete `Storage` gain
  `importStep`, `exportStep`, `transportFrom`, `transportUntil`
  (derived steps 0–10) plus the four `*PercentRaw` values
  (`importUntilPercentRaw`, `exportFromPercentRaw`,
  `transportFromPercentRaw`, `transportUntilPercentRaw`) so the
  companion can verify the derivation against the live Percent scale.
  Non-`Storage` `StorageBase` rows omit the slider fields.

### (c) Quick-trade provider enumeration — RESOLVED

- Owner: `Mafi.Core.World.WorldMapManager` —
  `[GlobalDependency(RegistrationMode.AsEverything)]`, public
  `IIndexable<QuickTradeProvider> AllQuickTrades` backed by a private
  `Lyst<QuickTradeProvider> m_allQuickTrades`. The manager also
  processes `QuickTradeCmd` by matching
  `x.Prototype.Id == cmd.TradeId` over that same collection, confirming
  it is the canonical provider set. No separate QuickTradesManager
  exists; settlements do not own providers (each provider holds a
  private `WorldMapVillage` reference, but enumeration is the
  manager's). `IIndexable<T>` ∈
  `Mafi.Collections.ReadonlyCollections` — same shape as
  `TrainLinesManager.Lines`, foreach-able, with `Count` via
  `ICollectionWithCount`.
- Provider: `Mafi.Core.World.QuickTrade.QuickTradeProvider` — public
  field `QuickTradePairProto Prototype` (a `Proto`, so
  `Prototype.Id.Value` is the trade id string), public field
  `Upoints UpointsCost`, `int CurrentStep { get; private set; }`,
  `bool IsSoldOut => CurrentStep >= Prototype.MaxSteps`,
  `bool HasPriceIncreased` (compares this step's pay quantity against
  the proto's), pure getters `GetProductToBuy()` /
  `GetProductToPayWith()` returning `Mafi.Core.ProductQuantity`
  (struct with public fields `ProductProto Product`,
  `Quantity Quantity` → `Product.Id.Value`, `Quantity.Value`), and
  `GetCooldownForCurrentStep()`. `CanAfford` in this build is
  `CanAfford(out LocStrFormatted error)` (not parameterless) and is
  NOT used by the route — the route sticks to the pure value reads.
- Value types: `Mafi.Upoints` is a struct with public field
  `Fix32 Value`; `Mafi.Fix32` has `public readonly int RawValue`
  (FRACTION_RANGE 1024). So the unity cost raw is
  `provider.UpointsCost.Value.RawValue` — the same
  `.Value.RawValue` chain v0.6 already compiles for
  `ContractProto.UpointsPerMonth`.
- **Route shape (v0.6.1)**: `GET /coi/v1/quick-trades` in
  `BridgeRoutesV6` (WorldMapManager constructor-injected alongside the
  existing four managers, so a resolution failure still only costs the
  V6 class), returning per provider: `id` (proto id), `productToBuy`
  / `productToPayWith` as `{product, quantity}`, `upointsCostRaw`,
  `currentStep`, `maxSteps`, `isSoldOut`, `hasPriceIncreased`.

## Recon round 3 (2026-11-03 morning) — research ids + power, CoiBridge v0.6.2

Two small read additions closing the game-plan format's G1/G2 gaps.
Both verified in the shipped assemblies (recon harness + ilspycmd)
before coding; v0.6.2 built 0 errors / 0 warnings and staged in
dist/ (no upload — Addendum VIII window).

### (d) Completed-research id list — RESOLVED (gap G1)

- `ResearchManager.ResearchedNodes` is `IIndexable<ResearchNode>` —
  the same element type and shape as `ResearchQueue`, which the
  v0.5 research route already enumerates. Each node exposes
  `Proto.Id.Value` (the string id the companion's research_ids.txt
  uses).
- **Route shape (v0.6.2)**: the existing `/coi/v1/research` handler
  gains a `"researched":[...]` array of those ids, alongside the
  pre-existing `researchedCount`. Route extended in place (the
  research handler lives in `BridgeRoutes`, the v0.5 class — this
  was an extension of an existing route, not a new family).

### (e) Electric power state — RESOLVED (gap G2); no battery surface exists

- Manager: `Mafi.Core.Factory.ElectricPower.ElectricityManager` —
  `[GlobalDependency(RegistrationMode.AsEverything)]`, sealed,
  implements `IElectricityManager`. Constructor-injectable exactly
  like the other managers; v0.6.2 injects it into `BridgeRoutesV6`
  (so a resolution failure still only costs the V6 class).
- Exact this-tick aggregates (all `Mafi.Electricity`):
  `GeneratedThisTick`, `ConsumedThisTick`, `DemandedThisTick`,
  `SurplusDemandedThisTick`, `GenerationCapacityThisTick`,
  `MaxGenerationCapacity`, `WastedElectricityThisTick`, plus
  `GeneratorsCount` / `ConsumersCount` (int).
- **`Electricity.Value` is exact kW** — `OneKw => new Electricity(1)`
  and `Quantity => new Quantity(Value)` in the shipped struct
  (`Mafi.Electricity` ∈ Mafi.Core.dll). No FRACTION_RANGE, no
  scaling: the route reports `.Value` directly as kW.
- **Battery / electricity storage: NO manager surface exists.**
  Name recon across Mafi.Core.dll + Mafi.Base.dll found no
  `*Battery*`, `*ElectricityStorage*` or electricity
  `*Accumulator*` type (only `RecyclablesAccumulator`, unrelated).
  CoI stores energy as steam in thermal-storage buildings
  (`Mafi.Base.Prototypes.Buildings.ThermalStorages.ThermalStorage`)
  — a product buffer, already visible via vanilla product stats /
  bridge storages, not an electricity aggregate. The route
  therefore has no storage field; do not invent one.
- Deliberately NOT read: `ProductionStats` / `ConsumptionStats` /
  `GenerationCapacityStats` and the per-proto
  `GetProductionStatsPerProto()` / `GetConsumptionStatsPerProto()`
  — all wrap `ElectricityAvgStats`, a time-averaged raw encoding
  that has not been cross-checked against the in-game UI. Same
  discipline as the v0.5 train counters: no undecoded averages.
- **Route shape (v0.6.2)**: `GET /coi/v1/power` in
  `BridgeRoutesV6`, Guard-wrapped, returning `generatedKw`,
  `consumedKw`, `demandedKw`, `surplusDemandedKw`,
  `generationCapacityKw`, `maxGenerationCapacityKw`, `wastedKw`,
  `balanceKw` (generated − consumed, computed in the handler),
  `generators`, `consumers`. Values are this-tick snapshots —
  callers wanting trends should sample, as with all bridge reads.

## Recon round 4 (2026-11-03) — decoder recon pack (ready to decode): ElectricityAvgStats, train activity/travel counters, custom-priority ids

**PREPARATION ONLY — THIS IS NOT VERIFIED DECODING.** Everything in
this round comes from OFFLINE assembly recon alone (ilspycmd
decompiles + a reflection scan over `refs/Mafi.Core.dll`,
`refs/Mafi.Base.dll`, `refs/Mafi.dll`, `refs/Mafi.Unity.dll`; the game
was off and nothing here has been compared against a running game).
This pack exists so a future live cross-check can confirm or refute
each reading quickly. Every claim is marked
**VERIFIED-FROM-ASSEMBLY** (the code was seen) or **INFERRED** (a
reading of that code). No route was built from this round; the
round-3 discipline stands — no undecoded averages in the bridge.

### (a) ElectricityAvgStats — full layout; raw = kW, history lives in four RLE rings

- **Name correction — VERIFIED-FROM-ASSEMBLY**: the type is
  `Mafi.Core.Stats.ElectricityAvgStats` (namespace `Mafi.Core.Stats`,
  NOT `Mafi.Core.Factory.ElectricPower` — that namespace holds the
  consumer, `ElectricityManager`). Sealed class, **no fields or
  constants of its own**; everything is inherited:
  `ElectricityAvgStats : ItemAvgStats<Electricity> :
  ItemStats<Electricity> : ItemStats`.
- **The subclass's only code — VERIFIED-FROM-ASSEMBLY**:
  `ValueToRaw(Electricity v) => (long)v.Value`;
  `RawToValue(long) => new Electricity((int)value)`;
  `FormatValue(long)` formats via
  `ElectricityQuantityFormatter.Instance.FormatNumberAndUnitOnly(new
  QuantityLarge(value))`. Ctor: `(Option<StatsManager> manager, bool
  isMonthlyEvent = false)`; `ElectricityManager` constructs all of
  its instances with the default, so electricity stats are NOT
  monthly events.
- **Units / scaling — VERIFIED-FROM-ASSEMBLY**: **the raw long IS
  kW, unscaled.** `Mafi.Electricity` is a struct over a single
  `public readonly int Value`; `OneKw => new Electricity(1)`,
  `FromMw(int) => Value = mw * 1000`, `Quantity => new
  Quantity(Value)`. No FRACTION_RANGE, no fixed-point. Any kW→MW
  presentation scaling happens in the formatter only (INFERRED: the
  formatter picks the display unit from the `QuantityLarge`
  magnitude; the stored/feed values are never rescaled).
- **`ItemStats` field layout — VERIFIED-FROM-ASSEMBLY** (all values
  are raw `long` in the type's unit, here kW):
  - scalars: `CurrentValue`, `ThisYearValue`, `LifetimeValue`
    (all protected, private set), `YearCollectionStarted` (int,
    internal), `FirstYearValue` (long, protected), `IsMonthlyEvent`
    (bool), private `Option<StatsManager> m_manager`;
  - four history rings, all `protected RleSequence`:
    `LastNDaysData` (cap 120), `LastNMonthsData` (cap 120),
    `LastNYearsData` (cap 100), `QuarterCenturiesData` (25-year
    buckets). Consts on the class: `QUARTER_CENTURY = 25`,
    `MAX_YEARS_OF_ANNUAL_DATA = 100`, `MAX_MONTHS_OF_MONTHLY_DATA =
    120`, `MAX_DAYS_OF_DAILY_DATA = 120`.
- **`RleSequence` packing — VERIFIED-FROM-ASSEMBLY**: struct over a
  private `Option<ulong[]>` ring (initial capacity 8 entries). Each
  entry packs the value in the low 55 bits (mask
  `0x7FFFFFFFFFFFFF`; bit `0x40000000000000` acts as the sign bit and
  is sign-extended on read) and a repeat count in bits 55+, max 512
  reps per entry (`MAX_REPS_PER_ENTRY`). Enumeration walks
  newest → oldest (INFERRED from the enumerator starting at the
  newest index; consistent with the UI chart being
  `rightToLeft: true`).
- **Avg semantics — VERIFIED-FROM-ASSEMBLY**: `ItemAvgStats<T>.Set(T)`
  calls `UpdateCurrentValue(raw, raw, raw)` — for an Avg stats
  object the three scalars `CurrentValue` / `ThisYearValue` /
  `LifetimeValue` ALL collapse to the most recent sample; the
  history is the real payload. Rollups on period boundaries
  (driven by `StatsManager`, which subscribes to `ICalendar`
  `NewDayEnd` / `NewMonthEnd` / `NewYearEnd` and fans out to every
  registered stats object): `OnNewDay` pushes `CurrentValue` into
  the daily ring and zeroes it; the monthly entry is
  `AggregateDaysToLastMonth() = LastNDaysData.GetAvgOfLastNValues(30)`
  (integer average); the yearly entry is
  `AggregateMonthsToLastYear() = GetAvgOfLastNValues(12)`; the
  25-year entry is `AggregateYears(25) =
  GetMaxOfLastNValues(25)` — a MAX, not an average.
- **Who feeds it — VERIFIED-FROM-ASSEMBLY**: `ElectricityManager`
  (public readonly fields `ProductionStats`, `ConsumptionStats`,
  `GenerationCapacityStats`, all `ElectricityAvgStats`) calls, from
  its sim-loop `updateStart`, `ProductionStats.Set(GeneratedThisTick)`,
  `ConsumptionStats.Set(ConsumedThisTick)`,
  `GenerationCapacityStats.Set(GenerationCapacityThisTick)` — i.e.
  each tick the PREVIOUS tick's aggregates become the new sample.
  Per-proto wrappers (`ElectricityManager.ProductionPerProto` /
  `ConsumptionPerProto` structs: public readonly proto +
  `ElectricityAvgStats` + `LastTick` + `EntitiesTotal`) are fed the
  same way (`Set(lastTickData.ProduceAndWasted)` /
  `Set(lastTickData.Consumed)`). `AddToCurrent` exists on
  `ItemAvgStats<T>` but the manager never calls it on these
  (VERIFIED-FROM-ASSEMBLY for the manager's call sites).
  Sampling consequence (INFERRED): a daily ring entry is the last
  tick sample taken before the day boundary, NOT a time-average
  over the day — the "Avg" averaging only appears at the
  day→month and month→year rollups. This is the single most
  important thing the live cross-check must confirm or refute.
- **Public read surface for a future route — VERIFIED-FROM-ASSEMBLY**:
  the rings and `CurrentValue` are protected, so a bridge route can
  only use what `ItemStats` / `ItemStats<T>` expose publicly:
  `GetLatestData(StatsDataRange, Lyst<long>)` (ring entries only;
  does NOT append the current partial period),
  `GetRangeTotal(StatsDataRange)`, `GetLastNDays / GetLastNMonths /
  GetLastNYears(int, Lyst<Electricity>)`, typed props `LastDay`,
  `LastMonth`, `LastYear`, `ThisYear`, `Lifetime` (the last two are
  near-useless for Avg stats per the Set semantics above),
  `HasAnyData` / `HasAnyNonZeroData`. `StatsDataRange` enum:
  `Last120Days, Last12Months, Last120Months, Last100Years,
  Last1000Years, QuarterCenturies`. (The internal
  `ItemStatsRawData` export struct — `long[] Daily/Monthly/Yearly/
  QuarterCenturies` + the scalars — is internal, not route-usable
  without reflection.)
- **How the game UI consumes it — VERIFIED-FROM-ASSEMBLY**:
  `Mafi.Unity.Ui.Statistics.ElectricityStatisticsTab.Overview`
  wraps the manager's three global stats directly in
  `ItemStatsDataSeriesAdapter` (series names: MaxCapacity /
  "+ Production" / "− Consumption") on a `LineChart` with an
  energy formatter; the adapter's `RefreshData(range)` fills its
  series from `GetLatestData(range, …)` with the raw longs
  unmodified. The Overview's range selector exposes only
  `Last120Days`, `Last12Months`, `Last1000Years` (updater default
  `Last120Months`). The breakdown / focus views use the per-proto
  stats objects through the same adapter, but sort and group by
  the per-proto `LastTick` values (this-tick, not the stats).
- **Live cross-check list for a future `/coi/v1/power` extension**:
  (1) bridge-read `GetLatestData` per range vs the Statistics
  window's Electricity tab chart at the same range, bar by bar;
  (2) newest-first ordering of the rings; (3) the daily-bar
  sampling question above (compare a daily bar against the
  this-tick `/power` values sampled across that day); (4) monthly
  bar == integer average of the last 30 daily bars; (5) per-proto
  series vs the tab's focus view for one proto.

### (b) Train activity / travel counters — no dedicated stats types exist; the counters are the shared ProductivityCounter family

- **Exact names — VERIFIED-FROM-ASSEMBLY**: there are NO standalone
  types named `*ActivityStats*` or `*TravelStats*` anywhere in
  `Mafi.Core.dll` / `Mafi.Base.dll` / `Mafi.dll`. What exists:
  - two PRIVATE nested enums in `Mafi.Core.Trains.Train`:
    `Train.TrainActivity { None, Working, Idle, Refueling,
    NotWorking }` and `Train.TrainTravelState { NotTravelling,
    FullSpeed, Accelerating, Braking, TrackSpeedLimit,
    BlockedByTrain }`;
  - public properties on `Train` (VERIFIED-FROM-ASSEMBLY):
    `ProductivityCounterHistory ActivityStats`,
    `ProductivityCounterHistory.Data ActivityStatsOngoingMonth`,
    `int ActivityStatsOngoingMonthTicks`, and the same trio as
    `TravelStats` / `TravelStatsOngoingMonth` /
    `TravelStatsOngoingMonthTicks` — all delegating to two private
    fields `ProductivityCounter m_activityCounter` /
    `m_travelCounter` (persisted in the train's `SerializeData`;
    counters only exist in saves ≥ version 337);
  - the counter types themselves are the SHARED machine
    productivity types in `Mafi.Core.Factory`:
    `ProductivityCounter` (struct), `ProductivityCounterHistory`
    (class), `ProductivityCounterHistory.Data` (struct),
    `ProductivityCounterLabels` / `ProductivityLabelCategory`.
- **`ProductivityCounter` — VERIFIED-FROM-ASSEMBLY**: fields are
  `public ProductivityCounterHistory HistoricData`, private
  `ProductivityCounterHistory.Data m_monthly`, private int
  `m_lastMonthCounter`. `ReportCategoryA/B/C/D()` increments that
  category in `m_monthly` and decrements `m_lastMonthCounter`;
  `ReportNoCategory()` ONLY decrements. When the counter hits 0,
  `saveMonthlyData()` resets it to **600**, pushes `m_monthly`
  into `HistoricData.ReportFullMonth(...)` and zeroes it. So one
  counter "month" bucket = **600 reports**, full stop. The ctor
  takes `initialTickCounter` (= the train's `TrainId.Value` at
  construction) and seeds `m_lastMonthCounter = (byte)(value % 100
  + 1)` (INFERRED: a phase stagger so trains do not all roll
  their first bucket over on the same tick).
  `OngoingMonthTicks`: if `HistoricData.Version != 0` it returns
  `600 - m_lastMonthCounter` (ALL reports this bucket, including
  NoCategory); if `Version == 0` it returns `m_monthly.Total`
  (categorised reports only) — a first-bucket quirk a decoder
  must reproduce or flag.
- **`ProductivityCounterHistory.Data` — VERIFIED-FROM-ASSEMBLY**:
  explicit-layout struct, size 8: four `ushort` fields
  `CategoryA` @0, `CategoryB` @2, `CategoryC` @4, `CategoryD` @6,
  unioned with a single `ulong RawData` @0 (equality/hash are on
  `RawData`). `Total => A + B + C + D`. Counts are report counts
  (per bucket ≤ 600 per category sum), NOT percentages; shares
  are derived at display time.
  `GetProductive(labels) = Σ category × label.IsProductiveMult`.
- **`ProductivityCounterHistory` — VERIFIED-FROM-ASSEMBLY**: consts
  `MONTHLY_RECORDS_COUNT = 12`, `YEARLY_RECORDS_COUNT = 12`,
  `TOTAL_RECORDS_TICK_COUNT = 43200`. Fields: `Data[12]
  m_monthlyRecordsBuffer` (ring; public `MonthlyRecordsBuffer` +
  `MonthlyRecordsBufferIndex`), `Data m_yearlyRolling`, `Data[12]
  m_yearlyRecordsBuffer` (ring + index), `Data
  m_totalRollingHalf`, int `Version` (= number of completed
  buckets; `FullMonthlyRecordsCount = clamp(Version − 1, 0, 12)`,
  `YearlyRecordsCount = min(Version / 12, 12)`).
  `ReportFullMonth` writes the next ring slot and adjusts
  `m_yearlyRolling` by the delta against the evicted slot; when
  the ring wraps (index 0) it calls `reportYearly`, which
  maintains `m_totalRollingHalf` by adding/subtracting
  `category >>> 1` — the stored total is HALVED (INFERRED: ratios
  over it are unaffected since numerator and denominator halve
  together, but an absolute-count decoder must double it).
  `CreateAverageOf(IIndexable<ProductivityCounterHistory>)`
  builds a cross-train average history (per-slot integer averages,
  skipping `RawData == 0` slots) — VERIFIED-FROM-ASSEMBLY it is
  consumed by `Mafi.Unity.Ui.Trains.TrainGroupingStatsCache`
  (INFERRED: the line/grouping panels' average view).
- **Reporting path — VERIFIED-FROM-ASSEMBLY**: `Train.reportStats(
  movedLessThanPlanned, coastingToSpeedLimit)` is called at the
  end of the spawned movement update and from
  `SimUpdateNotSpawned` (via `reportStatsNotSpawned`), i.e.
  (INFERRED) once per sim tick per train. It classifies the
  train's current state twice and reports exactly one category
  (or NoCategory) into each counter:
  - Activity mapping: Working→A, Idle→B, Refueling→C,
    NotWorking→D, None→NoCategory. `classifyActivity()` rules:
    paused / player-driving / `StateForUi.Unknown` → None;
    `NoValidGoals`, `NoPower`, `CannotFindPath`, `SelfIntersect`
    (plus two further error states) → NotWorking; a network
    train with no job → Idle, or Refueling if on a refuel
    schedule; not spawned → Idle; `ArrivalConditionsNotMet`,
    `AllStationsSkipped`, `AtOnlyStationOnLine` → Idle;
    `LoadingOrUnloading` → station activity: Working unless
    `StationInactiveDuration ≥ STATION_INACTIVE_DURATION_FOR_IDLE_STATS`
    (= 1 second, static ctor) → Idle; otherwise Working.
  - Travel mapping: FullSpeed→A, Accelerating→B, Braking→B
    (merged), TrackSpeedLimit→C, BlockedByTrain→D,
    NotTravelling→NoCategory. `classifyTravel()` only engages
    when spawned, unpaused, not player-driven, not at a station,
    and `StateForUi ∈ {Driving, WaitingForFreeTrack,
    WaitingForSuperBlock, WaitingForBidirectionalSuperBlock,
    DrivingToDepot}`; otherwise NotTravelling. Then, in order:
    BlockedByTrain if (`ClearanceThrottle < 100%` OR
    movedLessThanPlanned) AND `LastBlockingTrainIdOrNone` is set
    and ≠ own id; NotTravelling if speed is zero; TrackSpeedLimit
    if `SpeedLimit < MaxSpeed`, `SpeedLimit ≤ TargetSpeed`, and
    speed ≥ limit − tolerance OR coasting to the limit;
    FullSpeed if speed ≥ `MaxSpeed` − tolerance; Braking if
    `TargetSpeed < MaxSpeed` OR braking force > 0 OR accel force
    ≤ 0; Accelerating if not power-limited (`IsPowerLimited`
    uses `TRAVEL_STATS_SETTLED_EFFORT_SHARE` = 5%);
    fallback FullSpeed. Tolerance const:
    `TRAVEL_STATS_SPEED_TOLERANCE` = 0.01 tiles.
- **Labels — VERIFIED-FROM-ASSEMBLY** (Train static ctor,
  `Train.ACTIVITY_STATS_LABELS` / `TRAVEL_STATS_LABELS`):
  activity A = Working (`EntityStatus__Working`, productive),
  B = Idle, C = Refueling (`TrucksStats__RefuelingSelf`),
  D = **CannotWork** (`EntityStatus__CannotWork` — the UI label,
  not the enum name NotWorking), NoCategory = Paused.
  Travel A = FullSpeed (productive), B =
  AcceleratingAndBraking (productive), C = TrackSpeedLimit
  (productive), D = BlockedByTrains, NoCategory = NotTraveling.
- **How the in-game panel consumes it — VERIFIED-FROM-ASSEMBLY**:
  `Mafi.Unity.Ui.Inspectors.TrainInspector` adds two
  `ProductivityCounterUi` panels wired `ConnectTo(() =>
  Train.ActivityStats, () => Train.ActivityStatsOngoingMonth,
  () => Train.ActivityStatsOngoingMonthTicks, () =>
  Train.ACTIVITY_STATS_LABELS)` and identically for travel
  (panel titles `TrainTravelStats__Monthly` / `__Yearly`).
  `ProductivityCounterUi` builds its monthly chart as
  `ChartData(13 points, maxY = 600)` — 12 completed buckets plus
  the ongoing one; the NoCategory series is computed as
  `max(0, totalTicks − Data.Total)`; the headline percentages
  are `Percent.FromRatio(GetProductive(YearlyRolling),
  YearlyRolling.Total)` ("last 12 months") and the same over
  `TotalRollingHalf` ("last 12 years"). So the panel's headline
  share denominator is the CATEGORISED total, while its chart
  denominator is the 600-report bucket — a future decode must
  state which denominator it is using.
- **Live cross-check list for a future `/coi/v1/trains` decode**:
  (1) ongoing-month `Data` + `OngoingMonthTicks` vs the
  inspector's "Now" bar for one train, including the NoCategory
  remainder (600-bucket vs categorised-total denominators);
  (2) activity productive % == Working / Total and travel
  productive % == (A+B+C) / Total against the panel headlines;
  (3) monthly ring order — `MonthlyRecordsBufferIndex` is the
  NEWEST slot, the chart reads backwards from it; (4) the
  `Version == 0` first-bucket `OngoingMonthTicks` quirk on a
  freshly built train; (5) blocked share == travel category D
  for a train observed queueing behind another; (6) the
  grouping average (`CreateAverageOf`) vs a line panel, if the
  route ever serves line-level shares.

### (c) Custom-priority definitions — NOT enumerable from ProtosDb: no *Priority*Proto exists; the ids are per-class hardcoded strings

- **Verdict — VERIFIED-FROM-ASSEMBLY**: the hoped-for proto does
  NOT exist. `SetCustomPriorityCmd.PriorityId` is a plain
  `public readonly string` (same for `SetGlobalPriorityCmd`), and
  a reflection scan of `Mafi.Core.dll` + `Mafi.Base.dll` +
  `Mafi.dll` found **zero** types with "Priority" in the name
  deriving from any `Proto` base, and zero `*Proto` types with
  "Prio" anywhere in the full name. There is therefore no proto
  class to name, no id field to read, and nothing to enumerate —
  the `ProtosDb` enumeration surface itself is real but has no
  target here: `ProtosDb` offers `All<T>()`, `All(Type)`,
  `Filter<T>(Func<T,bool>)`, `Get<T>(ID)`, `GetOrThrow<T>(ID)`
  (all VERIFIED-FROM-ASSEMBLY). Hypothetical, for completeness
  only: had a `*Priority*Proto` existed, instances would have
  been `protosDb.All<T>()` with the id at `Proto.Id.Value` —
  that pattern is how every other catalogue in this file works,
  but it does not apply here.
- **Where the ids actually live — VERIFIED-FROM-ASSEMBLY**: the
  complete implementer set of `IEntityWithCustomPriority`
  (reflection scan, non-interface types assignable to the
  interface) is exactly six classes, and each hardcodes its own
  id strings in its `GetCustomPriority` / `SetCustomPriority` /
  `IsCustomPriorityVisible` if-chains:
  - `Mafi.Core.Buildings.Storages.Storage`: consts
    `IMPORT_PRIO_ID = "ImportPrio"`, `EXPORT_PRIO_ID =
    "ExportPrio"`;
  - `Mafi.Core.Buildings.Storages.NuclearWaste.NuclearWasteStorage`:
    inherits `Storage`, no override — same two ids;
  - `Mafi.Core.Buildings.Cargo.Modules.CargoDepotModule`:
    string LITERALS `"ImportPrio"` / `"ExportPrio"` (no consts);
  - `Mafi.Core.Buildings.Cargo.CargoDepot`: string LITERALS
    `"FuelImportPrio"` / `"FuelExportPrio"` (no consts);
  - `Mafi.Core.Buildings.Cargo.TradeDock`: const
    `CARGO_EXPORT_PRIO_ID = "CargoExportPrio"`;
  - `Mafi.Core.Buildings.Shipyard.Shipyard`: consts
    `FUEL_IMPORT_PRIO_ID = "FuelImportPrio"`,
    `FUEL_EXPORT_PRIO_ID = "FuelExportPrio"`,
    `CARGO_EXPORT_PRIO_ID = "CargoExportPrio"`,
    `SHIP_REPAIR_IMPORT_PRIO_ID = "ShipRepairImportPrio"`,
    `WORLD_CARGO_IMPORT_PRIO_ID = "WorldCargoImportPrio"`.
  The full distinct per-entity id universe is therefore **7
  strings**: `ImportPrio`, `ExportPrio`, `FuelImportPrio`,
  `FuelExportPrio`, `CargoExportPrio`, `ShipRepairImportPrio`,
  `WorldCargoImportPrio`. (A scan of every string const field
  containing "Prio" in all three assemblies found nothing else;
  the only other hits were `Mafi.Base` tutorial asset paths and
  terrain-editor strings, unrelated.)
- **The global pair is separate — VERIFIED-FROM-ASSEMBLY**:
  `Mafi.Core.Entities.Priorities.GlobalPrioritiesManager` holds
  consts `CONSTRUCTION_PRIORITY_ID = "ConstructionPriority"` and
  `DECONSTRUCTION_PRIORITY_ID = "DeconstructionPriority"` (both
  priorities default 7) and processes `SetGlobalPriorityCmd`
  itself; an unknown global id returns a command error
  ("Priority category … not found"), it does NOT assert. These
  are global values, not per-entity custom priorities, and are
  not part of the 7-id universe above.
- **Assert behaviour, pinned down — VERIFIED-FROM-ASSEMBLY**: in
  all five direct implementers (`Storage`, `CargoDepotModule`,
  `CargoDepot`, `TradeDock`, `Shipyard`), BOTH `GetCustomPriority`
  and `SetCustomPriority` end their if-chains with
  `Assert.Fail("Unknown custom priority: " + id)`. But
  `IsCustomPriorityVisible(string)` NEVER asserts — every
  implementation returns `false` for an unknown id (verified in
  all five; `NuclearWasteStorage` inherits `Storage`'s). The
  game's own UI follows the same discipline:
  `Mafi.Unity.Ui.Library.Inspectors.CustomPriorityToggleForBuffer`
  receives its two id strings as constructor parameters from each
  per-building inspector (per-class knowledge again, no central
  registry) and only calls `GetCustomPriority` for ids it was
  handed.
- **Safe decode strategy for a future route (INFERRED from the
  above)**: enumeration is a compile-time table keyed by concrete
  entity type — the 7-id universe above, exactly as v0.6.1
  already hardcoded `Storage`'s pair — and any id not in the
  entity's own row can be probed safely ONLY through
  `IsCustomPriorityVisible(id)` (false = not applicable), never
  through `GetCustomPriority`. No runtime discovery from
  `ProtosDb` is possible; if a future game update adds an
  implementer or an id, only a fresh assembly recon like this
  one will see it.

## v0.6.3 implementation (2026-11-03) — power averages + custom-priorities route

Built from recon round 4 (a) and (c) above. Read-only, Guard-wrapped,
staged as `dist/CoiBridge-mod-0.6.3.zip` (no upload, no live-game
contact — same standing window as v0.6.2).

### Power averages — wired, decoding UNVERIFIED

- `/coi/v1/power` (BridgeRoutesV6.Power) now appends, after the
  pre-existing this-tick fields:
  `"averages":{"production":{"lastDayKw":N,"lastMonthKw":N,"lastYearKw":N},
  "consumption":{...same...},"generationCapacity":{...same...}}`
  plus the sibling flag `"averagesUnverified":true`.
- Sources: the three public readonly `ElectricityAvgStats` fields on
  the already-injected `ElectricityManager` — `ProductionStats`,
  `ConsumptionStats`, `GenerationCapacityStats` — read through the
  typed props `LastDay` / `LastMonth` / `LastYear` on
  `ItemStats<Electricity>`, each returning `Electricity` whose
  `.Value` is exact kW, unscaled (recon round 4 (a)).
- Status: **UNVERIFIED.** The decoding follows the recon-round-4
  layout (monthly scalar = rollup average of daily samples; daily =
  last-tick sample before the day boundary, NOT a time-average),
  taken from offline assembly recon alone. No live cross-check has
  been possible; the recon round 4 (a) live cross-check list still
  applies in full (chart bar-by-bar comparison, newest-first
  ordering, daily-sampling question, monthly == avg of last 30
  daily, per-proto focus view). The `averagesUnverified` flag stays
  in the payload until that check passes.

### Custom priorities route — visibility-gate discipline

- New route `GET /coi/v1/priorities` in `BridgeRoutesV6`, which
  gains `IEntitiesManager` (`Mafi.Core.Entities`) as its 7th
  constructor parameter. Registered in `BridgeRoutes.s_routeTable`.
- Walk: every entity in `IEntitiesManager.Entities`, skipping
  `IsDestroyed`; entities implementing `IEntityWithCustomPriority`
  (`Mafi.Core.Entities.Priorities`) are probed over the 7-id
  universe from recon round 4 (c): `ImportPrio`, `ExportPrio`,
  `FuelImportPrio`, `FuelExportPrio`, `CargoExportPrio`,
  `ShipRepairImportPrio`, `WorldCargoImportPrio`.
- **Visibility-gate discipline (the safety core of this route):**
  each id is probed ONLY through `IsCustomPriorityVisible(id)` —
  it never asserts and returns false for an inapplicable id.
  `GetCustomPriority(id)` is called ONLY for an id that returned
  visible=true — the game's own UI discipline. `GetCustomPriority`
  / `SetCustomPriority` are NEVER called with an unprobed id (both
  `Assert.Fail` on unknown ids in every direct implementer), and
  `SetCustomPriority` is never called at all (read-only bridge).
- Exact fields emitted:
  `{"ids":["ImportPrio","ExportPrio","FuelImportPrio","FuelExportPrio","CargoExportPrio","ShipRepairImportPrio","WorldCargoImportPrio"],
  "entities":[{"id":<entity id int>,"proto":"<proto id>","priorities":{"<visible id>":<int>, ...}}, ...]}`
  — only entities with ≥1 visible id are included, and only the
  visible ids appear in each entity's `priorities` object.

### Version discipline

- `BridgeRoutes.ModVersion` and `manifest.json` version both
  `0.6.3`; readme header + "What's new in v0.6.3" section + routes
  list updated to match.

## Recon round 5 (2026-11-04) — per-instance machines + train counter accessors, CoiBridge v0.6.4

Surfaces verified with the reflection harness + ilspycmd BEFORE
coding (same discipline as round 3):

- `TrainsManager` carries `[GlobalDependency(RegistrationMode
  .AsEverything)]` — constructor-injectable like the other managers.
  `Trains` is `IIndexable<Train>`; `TrainDepots` also exists.
- `Train`: `Name` (string), `TrainId` (struct, `Value` is ushort),
  `TrainLine` (`Option<TrainLine>`), `IsPaused`, `IsSpawned`,
  `TrainCarsCount`, and the counter trios from recon round 4 (b) —
  `ActivityStats` / `ActivityStatsOngoingMonth` /
  `ActivityStatsOngoingMonthTicks`, same for `TravelStats`.
- `ProductivityCounterHistory` PUBLIC surface (the route uses only
  these): `Version`, `FullMonthlyRecordsCount`,
  `YearlyRecordsCount`, `MonthlyRecordsBuffer` +
  `MonthlyRecordsBufferIndex`, `YearlyRecordsBuffer` +
  `YearlyRecordsBufferIndex`, `YearlyRolling`, `TotalRollingHalf`.
  `Data`: public ushort fields `CategoryA–D`, public ulong
  `RawData`, `Total` property. The monthly buffer enumerates in
  STORED order; the recon says the index points at the newest slot —
  the route does not reorder (interpretation is the consumer's,
  post-cross-check).
- `Entity` base class: `IsPaused`, `IsEnabled`, `CanBePaused` are
  public on EVERY entity (machines included) — no interface cast
  needed for the machines route. `Machine.CurrentState` +
  `RecipesAssigned` were already used by the census.
- v0.6.4 placement decisions: `/coi/v1/machines` rows are built
  inside BridgeRoutes' shared scan (per-row try/catch +
  `skippedRows` counter) because the scan already walks every
  machine; the train counters were appended to the existing
  `/coi/v1/trains` handler behind a nested try/catch (`trainsError`
  fallback) so the lines payload can never be taken down by the
  counters block. Both choices bend the "new family = new class"
  rule deliberately — they extend existing routes/scans rather
  than adding a family. The payload flags the counters
  `"unverifiedDecoding": true`; the recon-round-4 (b) cross-check
  list is still the gate for any interpretation.
