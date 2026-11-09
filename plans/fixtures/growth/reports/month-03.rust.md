# DRY RUN — nothing was executed

> **DRY RUN — nothing was executed.** This report is a read-only evaluation of a plan file against local state captures. No command was prepared, confirmed, applied or sent anywhere; no game was contacted. Orders below are what a supervised executor *would* prepare next.

# Plan dry-run: fresh-start v1.1.0

- Title: Fresh start — New Haven (normal, Update 4) to self-sufficiency + first rail
- Map/difficulty (plan): {"difficulty":"normal","name":"New Haven"} — executor map/difficulty/version match is a live check (§3.1) and cannot be settled from captures; not asserted here.
- Plan game_version: 0.8.8
- Requires bridge: >=0.5.0
- State source: `plans/fixtures/growth/month-03` capture files — game-info date **1-3-15**, tick **216000**, gameId `fixture-growth-0001`, paused=True, speed=1
- Population 100, housing 160 (free 60, homeless 0), monthsOfFood 3, starving False; researchedCount 1, current research ResearchPowerAndMaintenance

## State-source mapping used by this evaluator

- `product.*` quantity/stored/capacity/flows ← `stats.json` (`Product_`-prefixed ids, exactly as plans name them); windows thisMonth=0, lastMonth=1, thisYear=2, lastYear=3, lifetime=4; `net_flow` = produced − consumed.
- `product.unlocked` ← `products.json` catalogue; `product.pinned` ← membership of `pinned.json`'s product keys (the pinned=true view, captured in stats shape).
- `game.*` ← `game-info.json`; `settlement.*` ← `bridge-status.json`; `research.*` ← `bridge-research.json` (progressPerc ÷ 1000); `census.*` ← `bridge-census.json`; `storage.*` ← `bridge-storages.json` (percentFullRaw ÷ 1000, capacity-weighted over matching rows); `trains.line_count` ← `bridge-trains.json`.
- **UNEVALUABLE by design:** `research.completed` reads the capture's v0.6.2 `researched` list (1 nodes), format G1 resolved, all `power.*` (no power capture, G2), `contracts/loans/edicts` (no v0.6 captures), `time.*` (no executor run anchor), and any `for:` duration whose instantaneous value is TRUE (a single snapshot cannot prove a duration, §4.3 — see interpretation note).

## Globals

### Invariants (§5.2 — evaluated every iteration)

- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)
- **TRUE** — `product.quantity(product=Product_Diesel) > 0` — Product_Diesel.quantity = 190

### Global checks

- `global-food-floor` (critical): **FALSE** — not firing — monthsOfFood = 3 (bridge-status.json)

## Active-phase determination (§9 play-loop rules)

- `phase-0-scout-bootstrap` — **COMPLETE** — exit gate TRUE ([TRUE] population = 100 (bridge-status.json); [TRUE] freeHousingCapacity = 60 (bridge-status.json))
- `phase-1-scrap-starter` — **COMPLETE** — exit gate TRUE ([TRUE] Product_Iron.quantity = 123; [TRUE] hasActiveLab = True; [TRUE] [TRUE] research.current = ResearchPowerAndMaintenance; [TRUE] len(queue) = 1 ["ResearchTradeDock"])
- `phase-2-assembly-power-maintenance` — **HELD AT ENTRY GATE (UNEVALUABLE)** — first non-complete phase; entry gate UNEVALUABLE — loop holds here, orders do not run (§4.4/§9 step 4)
- `phase-3-mining-vehicles` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-4-concrete-diesel-rubber` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-5-copper-construction2-food` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-6-automation-expansion` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-7-rail-introduction` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)

**Active phase: phase-2-assembly-power-maintenance** — HELD AT ENTRY GATE (UNEVALUABLE)

## Phase-by-phase evaluation

### phase-0-scout-bootstrap — Scout & bootstrap [COMPLETE]

Intent: Pause, look, verify the plan's hypotheses against the actual save, pin the vital products, and queue nothing yet. No building happens in this phase; it exists so every later gate reads observed state, not the layout-policy hypotheses.

**Entry gate: TRUE** — empty gate — enters immediately (§7)

**Exit gate: TRUE**
- **TRUE** — `settlement.population() > 0` — population = 100 (bridge-status.json)
- **TRUE** — `settlement.free_housing() >= 0` — freeHousingCapacity = 60 (bridge-status.json)

Checks (evaluated against current state):
- `p0-diesel-assumption` (critical, action=pause-and-report): **FALSE** — not firing — Product_Diesel.quantity = 190

Fallbacks: {"on_gate_stall":"pause-and-report","stall_after":{"months":1}}

### phase-1-scrap-starter — Scrap-fed starter cluster [COMPLETE]

Intent: Consume the comms-station scrap pile in a DISPOSABLE starter cluster: scrap smelter (1 blast furnace + 2 casters), wood-burning coal maker, diesel generator, two research labs, tree-clearing designations, controlled ocean dumping. Start the research queue. Nothing built here is permanent (layout policy).

**Entry gate: TRUE**
- **TRUE** — `product.quantity(product=Product_IronScrap) > 0` — Product_IronScrap.quantity = 880

**Exit gate: TRUE**
- **TRUE** — `product.quantity(product=Product_Iron) >= 100` — Product_Iron.quantity = 123
- **TRUE** — `research.has_active_lab() == True` — hasActiveLab = True
- **TRUE** — `any(research.current() != 'none'; research.queue_length() > 0)` — [TRUE] research.current = ResearchPowerAndMaintenance; [TRUE] len(queue) = 1 ["ResearchTradeDock"]

Checks (evaluated against current state):
- `p1-scrap-runway` (warning, action=pause-and-report): **FALSE** — not firing — Product_IronScrap.quantity = 880
- `p1-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 190

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-2-assembly-power-maintenance — Assembly + power & maintenance stability [HELD AT ENTRY GATE (UNEVALUABLE)]

Intent: Construction researched → ~4 manual assemblies near the smelter (construction parts priority 1, then mechanical parts, then electronics), maintenance depot online, one farm near the settlement. This is the power+maintenance stability phase: maintenance must never stall from here on.

**Entry gate: UNEVALUABLE**
- **UNEVALUABLE** — `research.completed(node=<placeholder Research<Construction>>) == True (implicit)` — node is a §4.5 placeholder (Research<Construction>) — manual-only, never auto-evaluated
- **TRUE** — `product.quantity(product=Product_Iron) >= 100` — Product_Iron.quantity = 123

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchPowerAndMaintenance) == True (implicit)` — ResearchPowerAndMaintenance in researched list = False
- **FALSE** — `census.proto_count(proto=MaintenanceDepotT1) >= 1` — byProto[MaintenanceDepotT1] = 0 (absent proto counts 0)
- **FALSE** — `all(any(product.produced(product=Product_ConstructionParts, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts, window=lastMonth) >= 0)  for: {"months":1}` — [TRUE] [FALSE] Product_ConstructionParts produced[lastMonth] = 0; [TRUE] Product_ConstructionParts consumed[lastMonth] = 6; [FALSE] Product_ConstructionParts window=lastMonth: produced 0 - consumed 6 = net -6 [for: {"months":1} — currently false, so the duration cannot hold]
- **TRUE** — `product.quantity(product=Product_MechanicalParts) > 0` — Product_MechanicalParts.quantity = 5

Checks (evaluated against current state):
- `p2-construction-parts-starvation` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_ConstructionParts.quantity = 14; [FALSE] Product_ConstructionParts window=thisMonth: produced 12 - consumed 10 = net 2
- `p2-maintenance-proxy` (critical, action=crisis-protocol): **FALSE** — not firing — all machine protos: NotEnoughInput 2/9 = 0.222
- `p2-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 190

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-3-mining-vehicles — Mining & vehicles [NOT REACHED]

Intent: Vehicles & Mining ends reliance on the scrap pile: coal mine first (retiring the coal maker smoothly), then iron mine + ore smelter anchoring the PERMANENT smelting district, truck fleet grown aggressively for the long iron haul.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchPowerAndMaintenance) == True (implicit)` — ResearchPowerAndMaintenance in researched list = False
- **FALSE** — `all(any(product.produced(product=Product_ConstructionParts, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_ConstructionParts produced[lastMonth] = 0; [TRUE] Product_ConstructionParts consumed[lastMonth] = 6; [FALSE] Product_ConstructionParts window=lastMonth: produced 0 - consumed 6 = net -6

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchVehicleAndMining) == True (implicit)` — ResearchVehicleAndMining in researched list = False
- **FALSE** — `product.quantity(product=Product_Coal) >= 200` — Product_Coal.quantity = 101
- **UNEVALUABLE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) > 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows)
- **TRUE** — `all(any(product.produced(product=Product_Iron, window=lastMonth) > 0; product.consumed(product=Product_Iron, window=lastMonth) > 0); product.net_flow(product=Product_Iron, window=lastMonth) >= 0)` — [TRUE] [TRUE] Product_Iron produced[lastMonth] = 75; [TRUE] Product_Iron consumed[lastMonth] = 15; [TRUE] Product_Iron window=lastMonth: produced 75 - consumed 15 = net 60

Checks (evaluated against current state):
- `p3-beacon-housing-guard` (warning, action=pause-and-report): **FALSE** — not firing — freeHousingCapacity = 60 (bridge-status.json)
- `p3-truck-congestion-signal` (warning, action=pause-and-report): **UNEVALUABLE** — check blind (UNEVALUABLE) — Product_IronOre not in stats.json products (selector matched zero rows)
- `p3-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 190

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-4-concrete-diesel-rubber — Concrete, diesel & rubber [NOT REACHED]

Intent: The transitional industry phase: limestone mine + kiln for concrete, oil pumps + basic distiller + rubber maker at the oil patch. Rubber and copper cannot be postponed — this phase ends with a REAL diesel chain replacing the starting stock, and rubber flowing.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchVehicleAndMining) == True (implicit)` — ResearchVehicleAndMining in researched list = False
- **FALSE** — `product.quantity(product=Product_Coal) >= 200` — Product_Coal.quantity = 101

**Exit gate: FALSE**
- **TRUE** — `product.quantity(product=Product_ConcreteSlab) >= 100` — Product_ConcreteSlab.quantity = 165
- **FALSE** — `all(any(product.produced(product=Product_Diesel, window=lastMonth) > 0; product.consumed(product=Product_Diesel, window=lastMonth) > 0); product.net_flow(product=Product_Diesel, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_Diesel produced[lastMonth] = 0; [TRUE] Product_Diesel consumed[lastMonth] = 20; [FALSE] Product_Diesel window=lastMonth: produced 0 - consumed 20 = net -20
- **UNEVALUABLE** — `product.quantity(product=Product_Rubber) > 0` — Product_Rubber not in stats.json products (selector matched zero rows)
- **FALSE** — `product.quantity(product=Product_Diesel) >= 300` — Product_Diesel.quantity = 190

Checks (evaluated against current state):
- `p4-diesel-net-flow` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_Diesel produced[lifetime] = 0; [TRUE] Product_Diesel window=thisMonth: produced 0 - consumed 10 = net -10
- `p4-rubber-deadline` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] ResearchBasicDiesel in researched list = False; [UNEVALUABLE] Product_Rubber not in stats.json products (selector matched zero rows)

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-5-copper-construction2-food — Copper, Construction II & food security [NOT REACHED]

Intent: Spend the ship's copper wisely while the permanent copper district (mine + smelter + electrolysis, near power — it is thirsty for water and power) comes online BEFORE the ship stock runs out. Construction II unlocks electric assemblies and the permanent conveyor-ready assembly area. Food security is gated explicitly here: the farm base must carry the settlement with margin.

**Entry gate: FALSE**
- **FALSE** — `all(any(product.produced(product=Product_Diesel, window=lastMonth) > 0; product.consumed(product=Product_Diesel, window=lastMonth) > 0); product.net_flow(product=Product_Diesel, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_Diesel produced[lastMonth] = 0; [TRUE] Product_Diesel consumed[lastMonth] = 20; [FALSE] Product_Diesel window=lastMonth: produced 0 - consumed 20 = net -20
- **UNEVALUABLE** — `product.quantity(product=Product_Rubber) > 0` — Product_Rubber not in stats.json products (selector matched zero rows)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchCopperRefinement) == True (implicit)` — ResearchCopperRefinement in researched list = False
- **FALSE** — `all(any(product.produced(product=Product_Copper, window=lastMonth) > 0; product.consumed(product=Product_Copper, window=lastMonth) > 0); product.net_flow(product=Product_Copper, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_Copper produced[lastMonth] = 0; [FALSE] Product_Copper consumed[lastMonth] = 0; [TRUE] Product_Copper window=lastMonth: produced 0 - consumed 0 = net 0
- **UNEVALUABLE** — `settlement.months_of_food() >= 3  for: {"months":1}` — monthsOfFood = 3 (bridge-status.json) [for: {"months":1} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]
- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)

Checks (evaluated against current state):
- `p5-ship-copper-runway` (critical, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_Copper.quantity = 300; [UNEVALUABLE] Product_CopperOre not in stats.json products (selector matched zero rows)
- `p5-food-floor` (critical, action=pause-and-report): **FALSE** — not firing — monthsOfFood = 3 (bridge-status.json)
- `p5-power-proxy` (warning, action=pause-and-report): **FALSE** — not firing — all machine protos: NotEnoughInput 2/9 = 0.222

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-6-automation-expansion — Automation & expansion districts [NOT REACHED]

Intent: Conveyor Belts kill the truck chaos: electric assemblies, belt-fed lines, unit storages with keep-full import rules — one dedicated line per product, no sushi belts. Ship repaired, groundwater and fuel station round out the first-15. Districts from the layout policy become the map's permanent shape.

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchCopperRefinement) == True (implicit)` — ResearchCopperRefinement in researched list = False
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 3 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchConveyorBelts) == True (implicit)` — ResearchConveyorBelts in researched list = False
- **FALSE** — `research.completed(node=ResearchShipDockRepair) == True (implicit)` — ResearchShipDockRepair in researched list = False
- **FALSE** — `research.completed(node=ResearchFuelStation) == True (implicit)` — ResearchFuelStation in researched list = False
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConstructionParts2, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts2, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts2, window=lastMonth) >= 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows)
- **TRUE** — `census.transports_fully_connected_ratio() >= 0.95` — fullyConnected 6/6 = 1.0000

Checks (evaluated against current state):
- `p6-transport-connectivity` (warning, action=pause-and-report): **FALSE** — not firing — fullyConnected 6/6 = 1.0000
- `p6-diesel-floor` (critical, action=crisis-protocol): **TRUE** — WOULD FIRE — Product_Diesel.quantity = 190

Fallbacks: {"on_check_fail":"crisis-protocol","on_gate_stall":"pause-and-report","stall_after":{"months":2}}

### phase-7-rail-introduction — Rail introduction (Trains Expansion DLC) [NOT REACHED]

Intent: First rail, honestly scoped: the starter skill hands rail off to the coi-trains sibling skill and notes the in-game train tutorials are new in Update 4 — use them. This phase stands up ONE network and ONE line on the longest truck haul (iron), built by hand, with the command-channel train primitives kept sandbox-only until their semantics are validated (format G3/G10).

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchFuelStation) == True (implicit)` — ResearchFuelStation in researched list = False
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConstructionParts2, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts2, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts2, window=lastMonth) >= 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows)
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 3 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchTrains) == True (implicit)` — ResearchTrains in researched list = False
- **FALSE** — `trains.line_count() >= 1` — len(bridge-trains lines) = 0
- **UNEVALUABLE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) > 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_IronOre not in stats.json products (selector matched zero rows)
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 3 (bridge-status.json)

Checks (evaluated against current state):
- `p7-rail-not-starving-the-base` (warning, action=pause-and-report): **FALSE** — not firing — Product_Iron window=thisMonth: produced 38 - consumed 17 = net 21

Fallbacks: {"on_check_fail":"pause-and-report","on_gate_stall":"skip-phase","stall_after":{"months":3}}

## Next orders (DRY RUN — would be prepared, nothing sent)

For active phase `phase-2-assembly-power-maintenance` [HELD AT ENTRY GATE (UNEVALUABLE)], in plan order:

- **p2-research-power-and-maintenance** — HELD — phase not entered (HELD AT ENTRY GATE (UNEVALUABLE))
  - command `ResearchStartCmd` args={"nodeId":"ResearchPowerAndMaintenance"}
  - note: Research order #2. Display name 'Power & Maintenance' is U4-verified (research-tree.md, wiki Required Research on Diesel Generator + Maintenance Depot). ID ambiguity flagged: research_ids.txt contains BOTH ResearchPowerAndMaintenance and ResearchPowerMaintenance — confirm on the research screen which is the live early node before applying; if it is the other, amend the plan (version bump) rather than guessing.
  - verify (current state): **TRUE** — [TRUE] [TRUE] research.current = ResearchPowerAndMaintenance; [FALSE] queue = ["ResearchTradeDock"]; contains ResearchPowerAndMaintenance = False
- **p2-build-assemblies** — HELD — phase not entered (HELD AT ENTRY GATE (UNEVALUABLE))
  - place-blueprint `starter-cluster/manual-assemblies-x4` district=starter-cluster asset_status=not-yet-authored
  - note: Manual build until authored: ~4 manual assemblies near the smelter. Recipe priority: construction parts FIRST (the eternal early bottleneck), then mechanical parts, then electronics (starter phase 2).
- **p2-enable-construction-parts-recipe** — HELD — phase not entered (HELD AT ENTRY GATE (UNEVALUABLE))
  - manual-step: Set one manual assembly to the Construction Parts I recipe and confirm it is producing; set a second to Mechanical Parts once construction parts flow.
- **p2-build-maintenance-depot** — HELD — phase not entered (HELD AT ENTRY GATE (UNEVALUABLE))
  - place-blueprint `starter-cluster/maintenance-depot` district=starter-cluster asset_status=not-yet-authored
  - note: Maintenance depot online in this phase (starter phase 2). Temporary siting is acceptable — it moves with the districts later.
  - verify (current state): **FALSE** — [FALSE] byProto[MaintenanceDepotT1] = 0 (absent proto counts 0)
- **p2-build-first-farm** — HELD — phase not entered (HELD AT ENTRY GATE (UNEVALUABLE))
  - place-blueprint `starter-cluster/first-farm` district=settlement-edge asset_status=not-yet-authored
  - note: One farm near the settlement (starter phase 2). Crop assignment is manual: FarmAssignCropCmd exists (wired) but crop proto ids are not sourced in the starter material, so no crop id is written here.
- **p2-pin-construction-parts** — HELD — phase not entered (HELD AT ENTRY GATE (UNEVALUABLE))
  - command `PinToggleCmd` args={"productToToggle":"Product_ConstructionParts"}
  - note: Construction parts are the phase-spanning bottleneck; keep them pinned.
  - preconditions: **TRUE** — [TRUE] Product_ConstructionParts.isUnlocked = True (products.json)
  - verify (current state): **TRUE** — [TRUE] Product_ConstructionParts in pinned set = True (pinned.json lists 3 pinned products)

All orders above are held: the phase has not been entered, so no order is due (§9 step 5).

## Summary

- Predicates evaluated (globals invariants/checks plus phase entry/exit gates and phase checks; composites count once): TRUE 19, FALSE 37, UNEVALUABLE 9 (leaf evidence inside composites is in the per-phase tables via the combined evidence strings).
- Plan status: loop position: phase 3 of 8 — phase-2-assembly-power-maintenance [HELD AT ENTRY GATE (UNEVALUABLE)].
- Interpretation notes: (1) `for:` durations — instantaneous FALSE stays FALSE; instantaneous TRUE becomes UNEVALUABLE because one snapshot cannot prove the predicate held for the whole duration (§4.3). (2) A phase counts COMPLETE purely when its exit gate is TRUE (§7/§9 re-derivation from state); human-confirm manual orders inside a gate-complete phase are not re-litigated by the evaluator. (3) Unknown / out-of-vocabulary metrics evaluate UNEVALUABLE with a closed-vocabulary note, never a crash and never a guess.

---
DRY RUN — nothing was executed. Generated by `coi plan-check` (Rust port of `tools/plan_check.py`; read-only over local files).
