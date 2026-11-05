# QoS pattern survey — what QoSChickens proved, and where else it fits

Philip's question (2026-11-05 11:28): review what was actually
built for the chickens, and survey whether other game systems
would benefit from the SAME structure.

Method: close read of the shipped QoSChickens/QoSCore sources;
every candidate grounded in a verified vanilla surface — a
command in `live/commands.json` (the 240-command web inventory,
schemas included), a manager/entity surface from
`bridge/MODDING-NOTES.md` or the bridge-dev skill, or fresh
assembly recon run for this survey (reflection harness +
ilspycmd against `refs/`, 0.8.8). Anything not verified is
labelled UNVERIFIED with the check that would settle it.
Survey and design only — no new mod code.

---

## 1. The pattern, precisely

What QoSChickens actually is (ChickenBalancer.cs, ~400 lines +
a 90-line IMod shell), generalised:

1. **The shipped command is the only mutation path.** The mod
   never touches game state directly. It schedules vanilla
   `MoveAnimalsCmd` through `IInputScheduler.ScheduleInputCmd`
   — the game's own mutation channel, so the move inherits
   vanilla's clamping, validation, and save/replay semantics.
2. **A policy/scheduler service decides WHEN and HOW MUCH.**
   A `[GlobalDependency]` service (mod instance
   constructor-injected for config — the PriorityManager
   shape) evaluates on a calendar event
   (`ICalendar.NewDayEnd`). The game provides the cadence
   menu: `NewDay`, `NewDayEnd`, `NewMonthStart`, `NewMonth`,
   `NewMonthEnd`, `NewYear`, `NewYearEnd` (verified by
   reflection dump of `Mafi.Core.Simulation.ICalendar`).
3. **Exact simulation before scheduling.** The balancer
   re-implements vanilla's 1-by-1 round-robin in
   `SimulatePull`/`SimulatePush` and computes the predicted
   post-move state of every farm. Only a move whose
   *predicted outcome* passes every guard is scheduled, and
   the scheduled amount is the simulated amount. On guard
   failure the amount halves and retries within the pass;
   then the day is skipped.
4. **Guards are predicates over the predicted state**, not
   over the request: receiver cap = min(capacity,
   `SlaughterLimit`); a farm at/above cap gains zero;
   eligible set = constructed + enabled only; drains never
   cross the breeding-nucleus floor; a global per-day budget
   caps total change.
5. **Config is the game's own mechanism** (`config.json` +
   `ModJsonConfig`, shown in the game's settings UI,
   persisted in saves), re-read on every pass — settings
   changes apply next pass, no restart, no caching.
6. **Ships inert.** Master switch `enabled` defaults to
   **false**; the gentlest mode (`equalise`) is pre-selected.
   First install is observable and does nothing until the
   player flips one setting.
7. **Every action logs its prediction** (farm, amount,
   predicted spread across the peer set) — the first enabled
   days are auditable against reality from the log alone.
8. **Zero content, total failure containment.** No protos,
   recipes or entities are registered (removal safety:
   `can_add/remove_from_saved_game: true`); the whole pass
   is wrapped so a failure is a log line, never an exception
   into the sim. Suite plumbing (QoSCore dependency,
   `QoS.<Feature>:` log prefix, registry announce) is shared.

### The pattern's limits (learned, not theorised)

- **The command owns its target set.** `MoveAnimalsCmd`'s
  peer set is all constructed+enabled same-species farms and
  cannot exclude one. When a growth-paused farm is present,
  the only faithful guard is to skip the species for the
  day — stricter than the prep design asked for. General
  form: *if a shipped command's internal targeting is
  coarse, the policy layer's guards degrade to
  all-or-nothing accept/skip.* Any candidate whose command
  retargets internally inherits this.
- **Simulation needs deterministic, fully-known semantics.**
  The round-robin could be simulated exactly because it was
  decompiled and is a closed distribution over a known set.
  Where a command's effect is systemic (truck logistics,
  the power grid), no exact simulation exists — the pattern
  degrades to *bounded nudges* (step limits, cooldowns,
  restore-toward-default), a weaker safety class.
- **Cadence is calendar-granular.** Day/month/year events
  only. A policy that must react inside a day needs a
  different tick source — an extension, not the pattern as
  shipped.
- **No per-entity memory.** A mod's only persistence is the
  game's config (global values, not per-entity). "Did *I*
  set this value, or did the player?" is unanswerable after
  a reload unless the rule is stateless — e.g. *only ever
  adjust entities sitting at their proto default*, which
  needs no memory. This ownership problem recurs in every
  actuator candidate below.
- **One exactness assumption is still unverified live:**
  `GetAllEntitiesOfType<AnimalFarm>()` enumeration order ==
  the order `FarmCommandsProcessor` materialises peers in
  (INFERRED in prep §1). Guards bound outcomes to safe
  ranges regardless, but prediction could be off by an
  animal at the margins until the first enabled days are
  eyeballed.
- **Nothing new can be created.** No new mechanics, no UI
  (windows are the §2 discipline), no content. The pattern
  is strictly *policy over shipped behaviour*.

---

## 2. Candidate survey

Verdict key: **FITS** = buildable in the pattern as shipped ·
**FITS-WITH-EXTENSION** = pattern applies but needs one named
piece of new machinery or a weaker safety class ·
**DOESN'T FIT** = rejected, reason given. Sizes are relative
to QoSChickens (= 1.0, ~490 lines total).

### 2.1 Storage alert policy — FITS · ~0.7×

- **Vanilla surface (verified):** `StorageAlertSetEnabledCmd
  {storageId, isEnabled, isAbove}` and
  `StorageAlertSetThresholdCmd {storageId, value: Percent,
  isAbove}` in commands.json; the game has native per-storage
  fill alerts — but each is set by hand, per storage. His
  save has **283 storages** (bridge census). Sibling alert
  surfaces, same shape: `WellPumpAlertSetEnabledCmd`,
  `StackerTowerSetFullAlertActiveCmd`,
  `SortingPlantSetBlockedProductAlertCmd`, farm
  `ToggleFullBufferNotificationCmd`.
- **Policy added:** a standing alert policy applied for the
  player: e.g. "every storage gets an above-90% alert; 
  storages of configured products also get a below-10%
  alert." Applied on load and when new storages appear —
  a batch-applier variant of the pattern (apply-on-event
  rather than daily scheduler).
- **Guards:** scope by config (all storages / product
  list / proto list); thresholds are exact Percent sets,
  read-back verifiable in-mod and via the bridge storages
  route; master switch off by default. The ownership
  problem is mild here: alerts are notifications, not
  logistics — worst case is a redundant alert.
- **Main risk:** low. Alert fatigue if the policy is
  broader than the player's actual attention; config scope
  is the mitigation.
- **Note:** SPEC §12.7 already says companion detectors
  should *align with* native storage alerts rather than
  duplicate them. This mod is the alignment mechanism — it
  sets the native alerts; §5's companion alerts remain the
  per-proto productivity evidence. Complementary, not
  duplicative.

### 2.2 Storage sliders — static presets FITS · dynamic FITS-WITH-EXTENSION · ~0.8×

- **Vanilla surface (verified, MODDING-NOTES recon 2b):**
  `StorageSetSliderStepCmd {storageId, importStep,
  exportStep, transportFrom, transportUntil}` +
  `StorageDisableLogisticsToggleCmd`. Semantics are
  exactly known: steps are derived (step = Percent/10%),
  setting import > 0 forces export-from back to 100% and
  vice versa, `CleaningMode` zeroes both. Reads verified
  (bridge v0.6.1 serves the four derived steps per
  concrete `Storage`).
- **Policy added:** (static) per-product/per-proto slider
  presets applied like 2.1 — same applier, second payload.
  (dynamic) threshold-driven adjustment, e.g. raise
  import-until on a buffer whose product is globally
  scarce.
- **Guards:** static half inherits 2.1's. Dynamic half
  cannot simulate logistics outcomes (only state
  outcomes), so it needs the weaker safety class:
  one-step moves, multi-day cooldowns, and the
  stateless ownership rule (only touch storages at
  preset/default values).
- **Main risk:** logistics thrash — sliders steer every
  truck decision touching that storage; a fiddly policy
  fights the player's intent invisibly. The static half
  has no such risk.
- **Verdict:** build the static half with 2.1 as one
  storage-policy mod; park the dynamic half until a
  concrete slider pain is named.

### 2.3 Priority actuator for starved machines — FITS-WITH-EXTENSION · ~1.2×

The actuator counterpart to productivity alerts (prep §5).

- **Vanilla surface (verified):** actuation
  `SetGeneralPriorityCmd {entityId, priority}` (commands
  .json; processor `EntitiesCommandsProcessor`, assignable
  range 0–14, MODDING-NOTES recon 2a). Read side is the
  bridge's own scan: `Machine.CurrentState`, `IsPaused`,
  `IsEnabled` per instance via `IEntitiesManager`
  (bridge v0.6.4 /machines). `LayoutEntity.GeneralPriority`
  is publicly readable and its proto default is
  `Prototype.Costs.DefaultPriority` (per-proto, ctor
  default 9) — both verified, which is what makes the
  stateless ownership rule below possible.
- **Policy added:** a machine persistently in
  NotEnoughInput gets its general priority stepped **up**
  (trucks serve its inputs first); when it returns to
  Working, priority steps back toward proto default.
  Cause separation inherited from §5's design: BLOCKED
  (OutputFull) machines are never raised — priority
  cannot fix a downstream blockage; PARKED (player-
  paused) machines are never touched.
- **Guards (the extension):** no simulation is possible
  (logistics response is systemic), so the safety class
  is bounded nudges: ±1 step per machine per day, a
  configured band around proto default, multi-day
  persistence required before the first step, and the
  **stateless ownership rule** — the mod only adjusts
  machines whose current priority is one it could have
  produced (default, or default ± its own step count);
  anything else is treated as player-set and left alone.
  That rule is imperfect after reloads (its own +1 is
  indistinguishable from a player's +1); the band cap
  bounds the worst case.
- **Main risk:** systemic side-effects — raising one
  line's priority starves another's trucks. The band +
  step discipline bounds it; the per-action log makes it
  auditable exactly like chickens.
- **Interaction:** designed to pair with §5 — the
  companion detects with evidence classes, this acts
  with bounded steps. It can also run standalone on its
  own in-game state reads; it does not need the
  companion. Custom priorities (`SetCustomPriorityCmd`)
  are deliberately out of scope: the 7-id universe is
  storage/dock/shipyard logistics (MODDING-NOTES recon
  4c), a different lever, folded into 2.2's domain.

### 2.4 Vehicle recovery — FITS-WITH-EXTENSION · ~0.8×

- **Vanilla surface (verified this survey):** detection —
  `Truck` (Mafi.Core.Vehicles.Trucks) exposes public
  `IsStuck`, `IsCannotDeliverNotificationActive`,
  `NavigationFailed`, `NavigationFailedStreak`.
  Eligibility gate — `IVehiclesManager.CanRecoverVehicle
  (vehicle, out errorReason)`, decompiled for this
  survey: it checks ONLY that a depot can accept the
  vehicle and that Unity affords `VEHICLE_RECOVERY_COST`
  (it is an affordability gate, **not** a stuck detector —
  the two must not be conflated). Actuation —
  `RecoverVehicleCmd {vehicleId}` (commands.json) or
  manager-level `IVehiclesManager.TryEnqueueRecoveryJob
  (vehicle)`; enumeration via `IVehiclesManager.Trucks`
  / `AllVehicles`.
- **Policy added:** a truck stuck / cannot-deliver for a
  sustained period is recovered automatically instead of
  sitting wedged with its cargo frozen until the player
  notices the notification.
- **Guards:** sustained-failure threshold
  (`NavigationFailedStreak` and/or cannot-deliver
  observed on consecutive passes — a transient pathing
  hiccup must never trigger a paid recovery); per-day
  recovery cap; a **Unity reserve floor** (never recover
  if the balance would drop below a configured floor —
  balance readable via `IUpointsManager`, verified in
  prep §3); `IsVehicleBeingRecovered` check to avoid
  duplicates.
- **The extension:** cadence. Calendar events are daily;
  a wedged truck hurts within the day. Either accept
  day-end cadence for v1 (still strictly better than
  unnoticed-for-days) or add a finer evaluation tick —
  the one piece of machinery chickens didn't need.
- **Main risk:** it **spends Unity autonomously** —
  bounded, rare, and priced by vanilla for exactly this
  rescue, but it crosses the line chickens never does
  (chickens spends nothing). Master-off default +
  reserve floor + cap are what make it acceptable; it
  should be a conscious opt-in, and the report should
  say so plainly.

### 2.5 Edicts conditional toggling — FITS · ~0.6×

- **Vanilla surface (verified):** `EdictsManager`
  `[GlobalDependency(AsEverything)]`, `AllEdicts` of
  `Edict {Prototype, IsEnabled, IsActive, CanBeEnabled(),
  LastReasonForNotBeingActive}` (bridge v0.6 read route).
  Actuation: `ToggleEdictEnabledCmd` — **verified this
  survey** as a shipped `InputCommand` in
  `Mafi.Core.Population.Edicts` with a public
  `EdictProtoId` field and the game's own
  Serialize/Deserialize. Its absence from the web
  inventory (the reason the companion can't use it) is
  irrelevant to a mod: the chickens path —
  `IInputScheduler.ScheduleInputCmd` — takes any shipped
  command, with its shipped serializer. No custom-command
  serializer question (contrast prep §3).
- **Policy added:** config-named edicts toggled by
  standing conditions — the honest example class is
  Unity economics: run a Unity-costly edict only while
  the Unity balance/income supports it; drop it before
  it drags the settlement negative.
- **Guards:** per-edict opt-in list in config (the
  ownership problem is solved by declaration — the mod
  manages only edicts the player named); `CanBeEnabled()`
  checked before every enable; minimum on/off durations
  (hysteresis) so an edict never flaps; Unity floor for
  costly edicts.
- **Main risk:** low mechanically. The real gap is
  demand: no specific edict policy has been named yet.
  This is a clean build in search of its first rule —
  build it when a rule is named, not before.

### 2.6 Research wishlist refill — FITS-WITH-EXTENSION (one premise UNVERIFIED) · ~0.8×

- **Vanilla surface (verified):** `ResearchManager`
  (global dependency) exposes public `CurrentResearch`
  and a public enumerable `ResearchQueue` — the bridge's
  v0.5 research route already enumerates
  `m_research.ResearchQueue`, so the read side is proven
  in shipped code. Commands (commands.json):
  `ResearchStartCmd {nodeId}`, `ResearchQueueDequeueCmd
  {nodeId, isEnqueue}`, `ResearchStopCmd {}`.
- **Policy added:** a standing wishlist: whenever the
  queue is empty (or below a configured depth), enqueue
  the next wishlist node whose prerequisites are met.
  The mod adds *refill from a declared priority list*,
  not queue mechanics.
- **UNVERIFIED premise:** whether vanilla auto-starts the
  next queued node on completion is not established by
  any source read for this survey (a community mod,
  COI-ResearchQueue, exists in the vendor set, which
  hints the vanilla behaviour left something to want —
  a hint, not evidence). The check that settles it:
  decompile `ResearchManager`'s completion path in
  `refs/Mafi.Core.dll`. The wishlist-refill policy is
  valuable under either answer, but the design differs
  (refill-only vs refill-and-start), so the check comes
  first at build time.
- **Guards:** wishlist is explicit config (node ids);
  never dequeue or stop anything (refill only — the
  player's queue edits are never fought); enqueue only
  nodes whose prerequisites are researched (readable via
  `ResearchManager.ResearchedNodes`, bridge v0.6.2);
  one enqueue per pass.
- **Main risk:** low — research time is the only cost,
  and every enqueued node was named by the player in
  advance.

### 2.7 Animal farms beyond chickens — ALREADY COVERED (not a candidate)

Answered from the shipped code: `ChickenBalancer.RunPass`
groups **all** `AnimalFarm` entities by
`farm.Prototype.Animal` and runs the same modes/guards
per species; the daily budget is global across species;
one command per species per day. Nothing in the mod is
chicken-specific except the name. The prep's recon (§1c)
found exactly one shipped species in 0.8.8 (Chicken, via
`ChickenFarm`) — so today the generality is latent, and
if a future patch adds species the mod covers them
unchanged. Sibling levers exist
(`AnimalFarmSetSlaughterLimitCmd`,
`AnimalFarmToggleGrowthPauseCmd`) but the balancer
deliberately treats slaughter limit and growth pause as
**player-owned inputs its guards read, never values it
writes** — keep it that way.

### 2.8 Electricity generation priority — FITS-WITH-EXTENSION, evidence-first · ~1.0×

- **Vanilla surface (verified):** commands
  `SetElectricityGenerationPriorityCmd {entityId,
  priority}`, `SetIsElectricitySurplusConsumerCmd`,
  `SetIsElectricitySurplusGeneratorCmd` (commands.json).
  Grid read side verified: `ElectricityManager`
  aggregates (`GeneratedThisTick`, `WastedElectricity
  ThisTick`, `SurplusDemandedThisTick`,
  `GenerationCapacityThisTick`; bridge v0.6.2) plus
  `ElectricityAvgStats` day/month averages (recon 4a).
  **UNVERIFIED:** the per-entity read of a generator's
  current generation-priority value (no processor or
  entity surface for it surfaced in recon) — the check
  that settles it is a reflection dump of the command's
  processor + the generator entity base at build time.
- **Policy added:** merit-order management — demote
  expensive peaker generation while surplus is being
  wasted, promote it under sustained deficit.
- **Why not a straight FITS:** grid response cannot be
  simulated at all, per-entity current values aren't
  verified readable, and whether his pre-nuclear grid
  changes state often enough to reward automation is
  unknown. The honest sequence is companion-first:
  surface the proposed order as an evidence card (data),
  let it prove itself, then consider the actuator.
- **Verdict:** companion feature first (see §3); mod
  only after the suggestion has a track record.

### 2.9 Quick-trade repetition — DOESN'T FIT (rejected)

- **Surfaces are all verified** — `QuickTradeCmd
  {tradeId}`; `WorldMapManager.AllQuickTrades`;
  `QuickTradeProvider {CurrentStep, IsSoldOut,
  HasPriceIncreased, GetCooldownForCurrentStep(),
  CanAfford(out error)}` (MODDING-NOTES recon 2c, prep
  §1c/§3c). Mechanically this is the *cleanest* fit in
  the survey — and it is rejected anyway:
- **Reason:** it spends products **and Unity**
  autonomously, at prices that escalate per step
  (`HasPriceIncreased`), which breaks the programme's
  own standing discipline (SPEC §12: resource-cost
  actions are never autonomous) and the lesson prep §3
  priced for train cargo. Chickens spends nothing; that
  is precisely why its automation is safe to leave
  running. A trade repeater is an economy actor, not a
  QoL policy. Deliberate repetition already has a home:
  the companion's prepare → confirm flow.

### 2.10 Examined and rejected (one line each)

- **Farm crop/fertility automation** (`FarmAssignCropCmd`,
  `FarmSetFertilityTargetCmd`, `FarmForceNextSlotCmd`) —
  crop rotation already runs on the farm's own schedule;
  fertility target is set-once. No recurring manual
  policy gap.
- **Forestry tower cut %** (`ForestryTowerSetCut
  PercentageCmd`) — a set-once tuning value, not a
  policy.
- **Nuclear reactor regulation** — vanilla already ships
  automatic regulation (`NuclearReactorToggleAutomatic
  RegulationCmd`). Duplicate.
- **Mech generator auto-balance** — vanilla ships it
  (`ToggleMechGeneratorAutoBalanceCmd`). Duplicate.
- **Train refuel/schedule conditions** — vanilla ships
  rich depart/skip conditions (`SetNetworkRefuel
  ThresholdCmd`, `SetTrainScheduleSkipIfHighFuelCmd`,
  `SkipIfFuelHigherThan`). The policy layer already
  exists in-game; advisor (§4) stays read-side.
- **Shipyard/fleet automation** — vanilla toggles exist
  (`ShipyardToggleAutoRepairCmd`,
  `FleetToggleAutoReturnCmd`). Duplicate.
- **Zipper even/priority ports** (`ZipperSetForceEven
  Inputs/OutputsCmd`, `ZipperSetPriorityPortsCmd`) —
  static build-time configuration, set once per zipper.
- **Storage logistics re-enable guard** (the diagnosis's
  crude-tank case) — deciding a disabled output *should*
  be re-enabled is a judgement about downstream demand;
  as a one-off fix it already exists as a companion §12
  action. Automating the judgement is how storages get
  silently re-plumbed.
- **Mine shifts by Unity balance**
  (`SetShiftsCountForMineCmd`) — real shape, zero
  recorded pain, and it steers his coal/oil lifeline;
  belongs to §2's window/companion suggestions if
  anywhere. Parked, not recommended.
- **Machine boost toggling** (`MachineBoostToggleCmd`) —
  boost carries a Unity upkeep; no threshold-blind
  manual policy identified. Resource spend without a
  policy is 2.9's failure mode in miniature.
- **Contracts auto-management** (`ToggleContractCmd`) —
  establishing/cancelling is a standing economic
  commitment, not a threshold policy; the companion
  already surfaces contracts read-side (bridge v0.6).
  Suggestions belong there.
- **ClearUnusedMachineBuffersCmd** — one-shot cleanup;
  its exact semantics were not verified and no
  recurring policy attaches to it.

---

## 3. What belongs in the companion instead

The suite's own line (ARCHITECTURE.md): the companion
reads and advises (data); QoS mods act (behaviour). Three
survey items sit on the data side of that line:

- **Electricity merit-order (2.8)** — evidence card
  first, for the reasons given: no simulation, no
  verified per-entity read, unproven demand.
- **Quick-trade repetition (2.9)** — prepare → confirm
  per §12; the companion's trade page already lists
  providers with costs and steps (bridge v0.6.1).
- **Contracts (2.10)** — already companion-surfaced;
  term-change suggestions are analysis, not actuation.

And one hybrid worth naming: **2.3's detection half is
§5** (companion, evidence-classed) while its actuation
half is a mod. They should share §5's cause vocabulary
(STARVED / BLOCKED / PARKED) so the mod's log and the
companion's Issues board never describe the same machine
two ways.

---

## 4. Recommended next 2–3

1. **QoSStoragePolicy (2.1 + static half of 2.2).** The
   purest pattern fit after chickens: exact set-and-read-
   back semantics, no resource cost, no simulation gap,
   immediate pain relief at his scale (283 storages,
   every alert/slider set by hand today). Reuses the
   chickens scaffold verbatim — same DI shape, same
   config mechanism, same build scaffold. ~0.7–0.8×
   chickens. It also doesn't compete with anything in
   the queue: §5 measures productivity, this sets the
   game's own per-storage alarms.
2. **QoSPriorityActuator (2.3).** The strategic one: it
   completes the sense→act pair with productivity
   alerts (§5) and turns the Issues board from diagnosis
   into bounded, audited, reversible remediation — the
   direction the whole programme points. It needs the
   extension machinery (bounded nudges + stateless
   ownership rule), so it goes second, after 2.1 has
   re-proven the scaffold. It does not block on §5's
   snapshot prerequisite — it reads machine states
   in-game — but its thresholds should adopt §5's
   persistence/cause discipline from the start.
3. **QoSVehicleRecovery (2.4).** Small, and after this
   survey's recon its surfaces are the most completely
   verified of any candidate (detection properties,
   decompiled eligibility gate, two actuation paths).
   The pain is real and silent (a wedged truck freezes a
   supply line and its cargo). Its two honest costs —
   a finer-than-daily cadence and autonomous Unity
   spend, bounded by cap + reserve floor — are exactly
   the conversation to have once, explicitly, rather
   than discover per-mod.

**Honourable mention:** edicts (2.5) is the cleanest
unbuilt FITS — the moment Philip names one edict rule
he actually wants, it is a ~0.6× build with fully
verified surfaces. Research refill (2.6) is one
decompile check away from the same status.

**Queue interaction, stated once:** none of these
displaces QoSWorldMap (§2) — it is a different
discipline (Unity UI) and proceeds independently;
the policy-service candidates share the chickens
scaffold, so they are cheap *in sequence with each
other*. Train advisor (§4) is untouched — every
train-side automation examined was either vanilla-
shipped or read-side by design. §5's prerequisite
(snapshot persistence + cadence) remains the shared
infrastructure the companion side is waiting on; 2.3
is the only recommendation that touches §5's domain,
and it consumes §5's vocabulary, not its data.

*All recon against the 0.8.8 assemblies; a game patch
re-opens surface verification, least for the
command-scheduling candidates (2.1–2.6), most for
anything that would touch UI (none recommended here).*
