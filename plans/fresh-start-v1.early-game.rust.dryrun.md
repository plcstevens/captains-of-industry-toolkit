# DRY RUN — nothing was executed

> **DRY RUN — nothing was executed.** This report is a read-only evaluation of a plan file against local state captures. No command was prepared, confirmed, applied or sent anywhere; no game was contacted. Orders below are what a supervised executor *would* prepare next.

# Plan dry-run: fresh-start v1.3.0

- Title: Fresh start — New Haven (normal, Update 4) to self-sufficiency + first rail
- Map/difficulty (plan): {"difficulty":"normal","name":"New Haven"} — executor map/difficulty/version match is a live check (§3.1) and cannot be settled from captures; not asserted here.
- Plan game_version: 0.8.8
- Requires bridge: >=0.5.0
- State source: `plans/fixtures/early-game` capture files — game-info date **1-1-18**, tick **51840**, gameId `fixture-early-game-0001`, paused=True, speed=1
- Population 100, housing 160 (free 60, homeless 0), monthsOfFood 4, starving False; researchedCount 0, current research ResearchCpPacking

## State-source mapping used by this evaluator

- `product.*` quantity/stored/capacity/flows ← `stats.json` (`Product_`-prefixed ids, exactly as plans name them); windows thisMonth=0, lastMonth=1, thisYear=2, lastYear=3, lifetime=4; `net_flow` = produced − consumed.
- `product.unlocked` ← `products.json` catalogue; `product.pinned` ← membership of `pinned.json`'s product keys (the pinned=true view, captured in stats shape).
- `game.*` ← `game-info.json`; `settlement.*` ← `bridge-status.json`; `research.*` ← `bridge-research.json` (progressPerc ÷ 1000); `census.*` ← `bridge-census.json`; `storage.*` ← `bridge-storages.json` (percentFullRaw ÷ 1000, capacity-weighted over matching rows); `trains.line_count` ← `bridge-trains.json`.
- **UNEVALUABLE by design:** `research.completed` reads the capture's v0.6.2 `researched` list (0 nodes), format G1 resolved, all `power.*` (no power capture, G2), `contracts/loans/edicts` (no v0.6 captures), `time.*` (no executor run anchor), and any `for:` duration whose instantaneous value is TRUE (a single snapshot cannot prove a duration, §4.3 — see interpretation note).

## Globals

### Invariants (§5.2 — evaluated every iteration)

- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)
- **TRUE** — `product.quantity(product=Product_Diesel) > 0` — Product_Diesel.quantity = 220

### Global checks

- `global-food-floor` (critical): **FALSE** — not firing — monthsOfFood = 4 (bridge-status.json)

## Active-phase determination (§9 play-loop rules)

- `phase-0-scout-bootstrap` — **COMPLETE** — exit gate TRUE ([TRUE] population = 100 (bridge-status.json); [TRUE] freeHousingCapacity = 60 (bridge-status.json))
- `phase-1-scrap-starter` — **ACTIVE** — first non-complete phase; entry gate TRUE, exit gate FALSE
- `phase-2-assembly-power-maintenance` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-3-mining-vehicles` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-4-concrete-diesel-rubber` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-5-copper-construction2-food` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-6-automation-expansion` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-7-rail-introduction` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)

**Active phase: phase-1-scrap-starter** — ACTIVE

## Phase-by-phase evaluation

### phase-0-scout-bootstrap — Scout & bootstrap [COMPLETE]

Intent: Pause, look, verify the plan's hypotheses against the actual save, pin the vital products, and queue nothing yet. No building happens in this phase; it exists so every later gate reads observed state, not the layout-policy hypotheses.

**Entry gate: TRUE** — empty gate — enters immediately (§7)

**Exit gate: TRUE**
- **TRUE** — `settlement.population() > 0` — population = 100 (bridge-status.json)
- **TRUE** — `settlement.free_housing() >= 0` — freeHousingCapacity = 60 (bridge-status.json)

Checks (evaluated against current state):
- `p0-diesel-assumption` (critical, action=pause-and-report): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_gate_stall":"pause-and-report","stall_after":{"months":1}}

### phase-1-scrap-starter — Scrap-fed starter cluster [ACTIVE]

Intent: Consume the comms-station scrap pile in a DISPOSABLE starter cluster: scrap smelter (1 blast furnace + 2 casters), wood-burning coal maker, diesel generator, two research labs, tree-clearing designations, controlled ocean dumping. Start the research queue. Nothing built here is permanent (layout policy).

**Entry gate: TRUE**
- **TRUE** — `product.quantity(product=Product_IronScrap) > 0` — Product_IronScrap.quantity = 850

**Exit gate: FALSE**
- **FALSE** — `product.quantity(product=Product_Iron) >= 100` — Product_Iron.quantity = 42
- **TRUE** — `research.has_active_lab() == True` — hasActiveLab = True
- **TRUE** — `any(research.current() != 'none'; research.queue_length() > 0)` — [TRUE] research.current = ResearchCpPacking; [TRUE] len(queue) = 1 ["ResearchPowerAndMaintenance"]

Checks (evaluated against current state):
- `p1-scrap-runway` (warning, action=pause-and-report): **FALSE** — not firing — Product_IronScrap.quantity = 850
- `p1-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-2-assembly-power-maintenance — Assembly + power & maintenance stability [NOT REACHED]

Intent: Construction researched → ~4 manual assemblies near the smelter (construction parts priority 1, then mechanical parts, then electronics), maintenance depot online, one farm near the settlement. This is the power+maintenance stability phase: maintenance must never stall from here on.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchCpPacking) == True (implicit)` — ResearchCpPacking in researched list = False
- **FALSE** — `product.quantity(product=Product_Iron) >= 100` — Product_Iron.quantity = 42

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchPowerAndMaintenance) == True (implicit)` — ResearchPowerAndMaintenance in researched list = False
- **FALSE** — `census.proto_count(proto=MaintenanceDepotT1) >= 1` — byProto[MaintenanceDepotT1] = 0 (absent proto counts 0)
- **FALSE** — `all(any(product.produced(product=Product_ConstructionParts, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts, window=lastMonth) >= 0)  for: {"months":1}` — [FALSE] [FALSE] Product_ConstructionParts produced[lastMonth] = 0; [FALSE] Product_ConstructionParts consumed[lastMonth] = 0; [TRUE] Product_ConstructionParts window=lastMonth: produced 0 - consumed 0 = net 0 [for: {"months":1} — currently false, so the duration cannot hold]
- **TRUE** — `product.quantity(product=Product_MechanicalParts) > 0` — Product_MechanicalParts.quantity = 6

Checks (evaluated against current state):
- `p2-construction-parts-starvation` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_ConstructionParts.quantity = 18; [TRUE] Product_ConstructionParts window=thisMonth: produced 0 - consumed 22 = net -22
- `p2-maintenance-proxy` (critical, action=crisis-protocol): **FALSE** — not firing — all machine protos: NotEnoughInput 1/4 = 0.250
- `p2-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-3-mining-vehicles — Mining & vehicles [NOT REACHED]

Intent: Vehicles & Mining ends reliance on the scrap pile: coal mine first (retiring the coal maker smoothly), then iron mine + ore smelter anchoring the PERMANENT smelting district, truck fleet grown aggressively for the long iron haul.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchPowerAndMaintenance) == True (implicit)` — ResearchPowerAndMaintenance in researched list = False
- **FALSE** — `all(any(product.produced(product=Product_ConstructionParts, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_ConstructionParts produced[lastMonth] = 0; [FALSE] Product_ConstructionParts consumed[lastMonth] = 0; [TRUE] Product_ConstructionParts window=lastMonth: produced 0 - consumed 0 = net 0

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchVehicleAndMining) == True (implicit)` — ResearchVehicleAndMining in researched list = False
- **FALSE** — `product.quantity(product=Product_Coal) >= 200` — Product_Coal.quantity = 65
- **UNEVALUABLE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) > 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows)
- **FALSE** — `all(any(product.produced(product=Product_Iron, window=lastMonth) > 0; product.consumed(product=Product_Iron, window=lastMonth) > 0); product.net_flow(product=Product_Iron, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_Iron produced[lastMonth] = 0; [FALSE] Product_Iron consumed[lastMonth] = 0; [TRUE] Product_Iron window=lastMonth: produced 0 - consumed 0 = net 0

Checks (evaluated against current state):
- `p3-beacon-housing-guard` (warning, action=pause-and-report): **FALSE** — not firing — freeHousingCapacity = 60 (bridge-status.json)
- `p3-truck-congestion-signal` (warning, action=pause-and-report): **UNEVALUABLE** — check blind (UNEVALUABLE) — Product_IronOre not in stats.json products (selector matched zero rows)
- `p3-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-4-concrete-diesel-rubber — Concrete, diesel & rubber [NOT REACHED]

Intent: The transitional industry phase: limestone mine + kiln for concrete, oil pumps + basic distiller + rubber maker at the oil patch. Rubber and copper cannot be postponed — this phase ends with a REAL diesel chain replacing the starting stock, and rubber flowing.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchVehicleAndMining) == True (implicit)` — ResearchVehicleAndMining in researched list = False
- **FALSE** — `product.quantity(product=Product_Coal) >= 200` — Product_Coal.quantity = 65

**Exit gate: FALSE**
- **TRUE** — `product.quantity(product=Product_ConcreteSlab) >= 100` — Product_ConcreteSlab.quantity = 180
- **FALSE** — `all(any(product.produced(product=Product_Diesel, window=lastMonth) > 0; product.consumed(product=Product_Diesel, window=lastMonth) > 0); product.net_flow(product=Product_Diesel, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_Diesel produced[lastMonth] = 0; [FALSE] Product_Diesel consumed[lastMonth] = 0; [TRUE] Product_Diesel window=lastMonth: produced 0 - consumed 0 = net 0
- **UNEVALUABLE** — `product.quantity(product=Product_Rubber) > 0` — Product_Rubber not in stats.json products (selector matched zero rows)
- **FALSE** — `product.quantity(product=Product_Diesel) >= 300` — Product_Diesel.quantity = 220

Checks (evaluated against current state):
- `p4-diesel-net-flow` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_Diesel produced[lifetime] = 0; [TRUE] Product_Diesel window=thisMonth: produced 0 - consumed 30 = net -30
- `p4-rubber-deadline` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] ResearchBasicDiesel in researched list = False; [UNEVALUABLE] Product_Rubber not in stats.json products (selector matched zero rows)

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-5-copper-construction2-food — Copper, Construction II & food security [NOT REACHED]

Intent: Spend the ship's copper wisely while the permanent copper district (mine + smelter + electrolysis, near power — it is thirsty for water and power) comes online BEFORE the ship stock runs out. Construction II unlocks electric assemblies and the permanent conveyor-ready assembly area. Food security is gated explicitly here: the farm base must carry the settlement with margin.

**Entry gate: FALSE**
- **FALSE** — `all(any(product.produced(product=Product_Diesel, window=lastMonth) > 0; product.consumed(product=Product_Diesel, window=lastMonth) > 0); product.net_flow(product=Product_Diesel, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_Diesel produced[lastMonth] = 0; [FALSE] Product_Diesel consumed[lastMonth] = 0; [TRUE] Product_Diesel window=lastMonth: produced 0 - consumed 0 = net 0
- **UNEVALUABLE** — `product.quantity(product=Product_Rubber) > 0` — Product_Rubber not in stats.json products (selector matched zero rows)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchCopperRefinement) == True (implicit)` — ResearchCopperRefinement in researched list = False
- **FALSE** — `all(any(product.produced(product=Product_Copper, window=lastMonth) > 0; product.consumed(product=Product_Copper, window=lastMonth) > 0); product.net_flow(product=Product_Copper, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_Copper produced[lastMonth] = 0; [FALSE] Product_Copper consumed[lastMonth] = 0; [TRUE] Product_Copper window=lastMonth: produced 0 - consumed 0 = net 0
- **UNEVALUABLE** — `settlement.months_of_food() >= 3  for: {"months":1}` — monthsOfFood = 4 (bridge-status.json) [for: {"months":1} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]
- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)

Checks (evaluated against current state):
- `p5-ship-copper-runway` (critical, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_Copper.quantity = 300; [UNEVALUABLE] Product_CopperOre not in stats.json products (selector matched zero rows)
- `p5-food-floor` (critical, action=pause-and-report): **FALSE** — not firing — monthsOfFood = 4 (bridge-status.json)
- `p5-power-proxy` (warning, action=pause-and-report): **FALSE** — not firing — all machine protos: NotEnoughInput 1/4 = 0.250

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-6-automation-expansion — Automation & expansion districts [NOT REACHED]

Intent: Conveyor Belts kill the truck chaos: electric assemblies, belt-fed lines, unit storages with keep-full import rules — one dedicated line per product, no sushi belts. Ship repaired, groundwater and fuel station round out the first-15. Districts from the layout policy become the map's permanent shape.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchCopperRefinement) == True (implicit)` — ResearchCopperRefinement in researched list = False
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 4 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchConveyorBelts) == True (implicit)` — ResearchConveyorBelts in researched list = False
- **FALSE** — `research.completed(node=ResearchRepairDock) == True (implicit)` — ResearchRepairDock in researched list = False
- **FALSE** — `research.completed(node=ResearchFuelStation) == True (implicit)` — ResearchFuelStation in researched list = False
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConstructionParts2, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts2, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts2, window=lastMonth) >= 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows)
- **TRUE** — `census.transports_fully_connected_ratio() >= 0.95` — fullyConnected 4/4 = 1.0000

Checks (evaluated against current state):
- `p6-transport-connectivity` (warning, action=pause-and-report): **FALSE** — not firing — fullyConnected 4/4 = 1.0000
- `p6-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-7-rail-introduction — Rail introduction (Trains Expansion DLC) [NOT REACHED]

Intent: First rail, honestly scoped: the starter skill hands rail off to the coi-trains sibling skill and notes the in-game train tutorials are new in Update 4 — use them. This phase stands up ONE network and ONE line on the longest truck haul (iron), built by hand, with the command-channel train primitives kept sandbox-only until their semantics are validated (format G3/G10).

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchFuelStation) == True (implicit)` — ResearchFuelStation in researched list = False
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConstructionParts2, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts2, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts2, window=lastMonth) >= 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows)
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 4 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchTrains) == True (implicit)` — ResearchTrains in researched list = False
- **FALSE** — `trains.line_count() >= 1` — len(bridge-trains lines) = 0
- **UNEVALUABLE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) > 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows)
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 4 (bridge-status.json)

Checks (evaluated against current state):
- `p7-rail-not-starving-the-base` (warning, action=pause-and-report): **FALSE** — not firing — Product_Iron window=thisMonth: produced 58 - consumed 16 = net 42

Fallbacks: {"on_check_fail":"pause-and-report","on_gate_stall":"skip-phase","stall_after":{"months":3}}

## Next orders (DRY RUN — would be prepared, nothing sent)

For active phase `phase-1-scrap-starter` [ACTIVE], in plan order:

- **p1-research-construction** — WOULD PREPARE (command, §12 prepare step — not sent)
  - command `ResearchStartCmd` args={"nodeId":"ResearchCpPacking"}
  - note: Research order #1 (research-order.md): unlocks assemblies, Construction Parts I, Mechanical Parts, Electronics I. Id SOURCED 2026-11-06 (plan v1.3.0): shipped proto registration Start("Construction", Ids.Research.CpPacking) in Mafi.Base (ConstructionResearchData) with exactly these unlocks; the string ResearchCpPacking appears in Philip's live bridge researched list. The tree calls it a starting tech — it may already be complete at t0, in which case this order is skipped by its verify. (The old candidate ResearchCp1Production is a goal id, not a research node — see the plan-level assumption.)
  - verify (current state): **FALSE** — [FALSE] researchedCount = 0
- **p1-research-queue-first15** — WOULD PRESENT (human/agent task — not executed)
  - manual-step: Queue the first-15 research list (research-order.md) up front in the research UI, in the order the phases below consume it: Construction (ResearchCpPacking); Maintenance (listed as 'Power & Maintenance' in older guides; ResearchPowerAndMaintenance); Trading Dock (ResearchTradeDock); Vehicles & Mining (ResearchVehicleAndMining); Beacon; Tools; Advanced concrete (the wiki's 'Concrete Production'; ResearchConcreteAdvanced); Basic Diesel; Synthetic Rubber (ResearchRubberProduction); Copper Refinement; Construction II (ResearchCp2Packing); Conveyor Belts; Ship Dock Repair; Groundwater Pump; Fuel Station. All ids sourced 2026-11-06 (plan v1.3.0). Two labs keep research ahead of building.
- **p1-build-starter-cluster** — WOULD PRESENT (human/agent task — not executed)
  - place-blueprint `starter-cluster/scrap-smelter-1furnace-2caster-plus-coal-maker` district=starter-cluster asset_status=authored
  - note: Degrades to a manual build from layout-policy.md's temporary list: first scrap smelter (1 blast furnace + 2 casters on scrap), wood-burning coal maker, diesel generator placed on the OUTPUT side of fuel storage, two research labs. Starter cluster is disposable — build cheap, near the comms station/docks, never anchor a permanent build here. Placement route (v1.2.2): the smelter core (pack slug iron-v1) goes through the companion /place flow — human confirm, QoSBuilder job; the coal maker, generator and labs are single-building manual placements (no pack asset contains them — plans-blueprint-status.md ref #1).
- **p1-tree-clearing-and-dumping** — WOULD PRESENT (human/agent task — not executed)
  - manual-step: Designate tree clearing around the starter cluster, and set controlled ocean waste dumping on the 'flat' setting in a zone AWAY from the ship's docking spot (dumping extends coastline into future building land; keep the south/east coastline stretch reserved for the future big harbour — do not build or dump on it).
- **p1-resume-time** — WOULD PREPARE (command, §12 prepare step — not sent)
  - command `GameSpeedChangeCmd` args={"newSpeedMultiplier":1}
  - note: Scout is done: run at speed 1 while the starter cluster goes up (a human unpauses — SetSimPauseStateCmd {isPaused: false} — as the companion order to this one; both are verified sim-control commands, confirm class in this plan).
  - verify (current state): **TRUE** — [TRUE] speed = 1
- **p1-research-power-maintenance-early-flag** — WOULD PRESENT (human/agent task — not executed)
  - observe-only: Watch the research queue: Power & Maintenance is order #2 in this plan (maintenance-before-anything-else). The source skill flags an order variant — some guides take it after Trading Dock. The variant is recorded here, not silently resolved.

## Summary

- Predicates evaluated (globals invariants/checks plus phase entry/exit gates and phase checks; composites count once): TRUE 15, FALSE 42, UNEVALUABLE 8 (leaf evidence inside composites is in the per-phase tables via the combined evidence strings).
- Plan status: loop position: phase 2 of 8 — phase-1-scrap-starter [ACTIVE].
- Interpretation notes: (1) `for:` durations — instantaneous FALSE stays FALSE; instantaneous TRUE becomes UNEVALUABLE because one snapshot cannot prove the predicate held for the whole duration (§4.3). (2) A phase counts COMPLETE purely when its exit gate is TRUE (§7/§9 re-derivation from state); human-confirm manual orders inside a gate-complete phase are not re-litigated by the evaluator. (3) Unknown / out-of-vocabulary metrics evaluate UNEVALUABLE with a closed-vocabulary note, never a crash and never a guess.

---
DRY RUN — nothing was executed. Generated by `coi plan-check` (Rust port of `tools/plan_check.py`; read-only over local files).
