# DRY RUN — nothing was executed

> **DRY RUN — nothing was executed.** This report is a read-only evaluation of a plan file against local state captures. No command was prepared, confirmed, applied or sent anywhere; no game was contacted. Orders below are what a supervised executor *would* prepare next.

# Plan dry-run: fresh-start v1.2.2

- Title: Fresh start — New Haven (normal, Update 4) to self-sufficiency + first rail
- Map/difficulty (plan): {'name': 'New Haven', 'difficulty': 'normal'} — executor map/difficulty/version match is a live check (§3.1) and cannot be settled from captures; not asserted here.
- Plan game_version: 0.8.8
- Requires bridge: >=0.5.0
- State source: `plans/fixtures/growth/month-12` capture files — game-info date **1-12-15**, tick **993600**, gameId `fixture-growth-0001`, paused=True, speed=1
- **Sequence mode (§4.3.1):** 12 snapshots, 1-1-18 .. 1-12-15, one gameId `fixture-growth-0001`. `for:` durations evaluate over this history; all other predicates read the latest snapshot. Per-prefix phase history is tabulated below.
- Population 132, housing 224 (free 92, homeless 0), monthsOfFood 4, starving False; researchedCount 9, current research ResearchCp2

## State-source mapping used by this evaluator

- `product.*` quantity/stored/capacity/flows ← `stats.json` (`Product_`-prefixed ids, exactly as plans name them); windows thisMonth=0, lastMonth=1, thisYear=2, lastYear=3, lifetime=4; `net_flow` = produced − consumed.
- `product.unlocked` ← `products.json` catalogue; `product.pinned` ← membership of `pinned.json`'s product keys (the pinned=true view, captured in stats shape).
- `game.*` ← `game-info.json`; `settlement.*` ← `bridge-status.json`; `research.*` ← `bridge-research.json` (progressPerc ÷ 1000); `census.*` ← `bridge-census.json`; `storage.*` ← `bridge-storages.json` (percentFullRaw ÷ 1000, capacity-weighted over matching rows); `trains.line_count` ← `bridge-trains.json`.
- **UNEVALUABLE by design:** `research.completed` reads the capture's v0.6.2 `researched` list (9 nodes), format G1 resolved, all `power.*` (no power capture, G2), `contracts/loans/edicts` (no v0.6 captures), `time.*` (no executor run anchor), and any `for:` duration the sequence history cannot prove — insufficient history or a blind snapshot inside the window (§4.3.1).

## Globals

### Invariants (§5.2 — evaluated every iteration)

- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)
- **TRUE** — `product.quantity(product=Product_Diesel) > 0` — Product_Diesel.quantity = 220

### Global checks

- `global-food-floor` (critical): **FALSE** — not firing — monthsOfFood = 4 (bridge-status.json)

## Active-phase determination (§9 play-loop rules)

- `phase-0-scout-bootstrap` — **COMPLETE** — exit gate TRUE ([TRUE] population = 132 (bridge-status.json); [TRUE] freeHousingCapacity = 92 (bridge-status.json))
- `phase-1-scrap-starter` — **COMPLETE** — exit gate TRUE ([TRUE] Product_Iron.quantity = 534; [TRUE] hasActiveLab = True; [TRUE] [TRUE] research.current = ResearchCp2; [FALSE] len(queue) = 0 [])
- `phase-2-assembly-power-maintenance` — **COMPLETE** — exit gate TRUE ([TRUE] ResearchPowerAndMaintenance in researched list = True; [TRUE] byProto[MaintenanceDepotT1] = 1 (absent proto counts 0); [TRUE] inner predicate TRUE at every snapshot in the trailing 1-month window (anchor 1-11-15, 2 snapshot(s), span 1.00 months) [for: {'months': 1}]; [TRUE] Product_MechanicalParts.quantity = 23)
- `phase-3-mining-vehicles` — **COMPLETE** — exit gate TRUE ([TRUE] ResearchVehicleAndMining in researched list = True; [TRUE] Product_Coal.quantity = 604; [TRUE] [TRUE] [TRUE] Product_IronOre produced[lastMonth] = 155; [TRUE] Product_IronOre consumed[lastMonth] = 126; [TRUE] Product_IronOre window=lastMonth: produced 155 - consumed 126 = net 29; [TRUE] [TRUE] [TRUE] Product_Iron produced[lastMonth] = 130; [TRUE] Product_Iron consumed[lastMonth] = 72; [TRUE] Product_Iron window=lastMonth: produced 130 - consumed 72 = net 58)
- `phase-4-concrete-diesel-rubber` — **ACTIVE** — first non-complete phase; entry gate TRUE, exit gate FALSE
- `phase-5-copper-construction2-food` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-6-automation-expansion` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-7-rail-introduction` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)

**Active phase: phase-4-concrete-diesel-rubber** — ACTIVE

## Sequence phase history (per prefix, §4.3.1)

Each row re-derives phase statuses using only the history up to that snapshot — the way a live executor's history grows. The last row is this report's verdict.

| snapshot | phase 0 | phase 1 | phase 2 | phase 3 | phase 4 | phase 5 | phase 6 | phase 7 |
|---|---|---|---|---|---|---|---|---|
| 1-1-18 | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-2-15 | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-3-15 | COMPLETE | COMPLETE | HELD (U) | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-4-15 | COMPLETE | COMPLETE | HELD (U) | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-5-15 | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-6-15 | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-7-15 | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-8-15 | COMPLETE | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-9-15 | COMPLETE | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-10-15 | COMPLETE | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-11-15 | COMPLETE | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED |
| 1-12-15 | COMPLETE | COMPLETE | COMPLETE | COMPLETE | ACTIVE | NOT REACHED | NOT REACHED | NOT REACHED |

Phase key: phase 0 = `phase-0-scout-bootstrap`; phase 1 = `phase-1-scrap-starter`; phase 2 = `phase-2-assembly-power-maintenance`; phase 3 = `phase-3-mining-vehicles`; phase 4 = `phase-4-concrete-diesel-rubber`; phase 5 = `phase-5-copper-construction2-food`; phase 6 = `phase-6-automation-expansion`; phase 7 = `phase-7-rail-introduction`

## Phase-by-phase evaluation

### phase-0-scout-bootstrap — Scout & bootstrap [COMPLETE]

Intent: Pause, look, verify the plan's hypotheses against the actual save, pin the vital products, and queue nothing yet. No building happens in this phase; it exists so every later gate reads observed state, not the layout-policy hypotheses.

**Entry gate: TRUE** — empty gate — enters immediately (§7)

**Exit gate: TRUE**
- **TRUE** — `settlement.population() > 0` — population = 132 (bridge-status.json)
- **TRUE** — `settlement.free_housing() >= 0` — freeHousingCapacity = 92 (bridge-status.json)

Checks (evaluated against current state):
- `p0-diesel-assumption` (critical, action=pause-and-report): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 1}}

### phase-1-scrap-starter — Scrap-fed starter cluster [COMPLETE]

Intent: Consume the comms-station scrap pile in a DISPOSABLE starter cluster: scrap smelter (1 blast furnace + 2 casters), wood-burning coal maker, diesel generator, two research labs, tree-clearing designations, controlled ocean dumping. Start the research queue. Nothing built here is permanent (layout policy).

**Entry gate: TRUE**
- **TRUE** — `product.quantity(product=Product_IronScrap) > 0` — Product_IronScrap.quantity = 730

**Exit gate: TRUE**
- **TRUE** — `product.quantity(product=Product_Iron) >= 100` — Product_Iron.quantity = 534
- **TRUE** — `research.has_active_lab() == True` — hasActiveLab = True
- **TRUE** — `any(research.current() != 'none'; research.queue_length() > 0)` — [TRUE] research.current = ResearchCp2; [FALSE] len(queue) = 0 []

Checks (evaluated against current state):
- `p1-scrap-runway` (warning, action=pause-and-report): **FALSE** — not firing — Product_IronScrap.quantity = 730
- `p1-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-2-assembly-power-maintenance — Assembly + power & maintenance stability [COMPLETE]

Intent: Construction researched → ~4 manual assemblies near the smelter (construction parts priority 1, then mechanical parts, then electronics), maintenance depot online, one farm near the settlement. This is the power+maintenance stability phase: maintenance must never stall from here on.

**Entry gate: UNEVALUABLE**
- **UNEVALUABLE** — `research.completed(node=<placeholder Research<Construction>>) == True (implicit)` — node is a §4.5 placeholder (Research<Construction>) — manual-only, never auto-evaluated
- **TRUE** — `product.quantity(product=Product_Iron) >= 100` — Product_Iron.quantity = 534

**Exit gate: TRUE**
- **TRUE** — `research.completed(node=ResearchPowerAndMaintenance) == True (implicit)` — ResearchPowerAndMaintenance in researched list = True
- **TRUE** — `census.proto_count(proto=MaintenanceDepotT1) >= 1` — byProto[MaintenanceDepotT1] = 1 (absent proto counts 0)
- **TRUE** — `all(any(product.produced(product=Product_ConstructionParts, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts, window=lastMonth) >= 0)  for: {'months': 1}` — inner predicate TRUE at every snapshot in the trailing 1-month window (anchor 1-11-15, 2 snapshot(s), span 1.00 months) [for: {'months': 1}]
- **TRUE** — `product.quantity(product=Product_MechanicalParts) > 0` — Product_MechanicalParts.quantity = 23

Checks (evaluated against current state):
- `p2-construction-parts-starvation` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_ConstructionParts.quantity = 51; [FALSE] Product_ConstructionParts window=thisMonth: produced 33 - consumed 31 = net 2
- `p2-maintenance-proxy` (critical, action=crisis-protocol): **FALSE** — not firing — all machine protos: NotEnoughInput 1/15 = 0.067
- `p2-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-3-mining-vehicles — Mining & vehicles [COMPLETE]

Intent: Vehicles & Mining ends reliance on the scrap pile: coal mine first (retiring the coal maker smoothly), then iron mine + ore smelter anchoring the PERMANENT smelting district, truck fleet grown aggressively for the long iron haul.

**Entry gate: TRUE**
- **TRUE** — `research.completed(node=ResearchPowerAndMaintenance) == True (implicit)` — ResearchPowerAndMaintenance in researched list = True
- **TRUE** — `all(any(product.produced(product=Product_ConstructionParts, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts, window=lastMonth) >= 0)` — [TRUE] [TRUE] Product_ConstructionParts produced[lastMonth] = 62; [TRUE] Product_ConstructionParts consumed[lastMonth] = 58; [TRUE] Product_ConstructionParts window=lastMonth: produced 62 - consumed 58 = net 4

**Exit gate: TRUE**
- **TRUE** — `research.completed(node=ResearchVehicleAndMining) == True (implicit)` — ResearchVehicleAndMining in researched list = True
- **TRUE** — `product.quantity(product=Product_Coal) >= 200` — Product_Coal.quantity = 604
- **TRUE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) > 0)` — [TRUE] [TRUE] Product_IronOre produced[lastMonth] = 155; [TRUE] Product_IronOre consumed[lastMonth] = 126; [TRUE] Product_IronOre window=lastMonth: produced 155 - consumed 126 = net 29
- **TRUE** — `all(any(product.produced(product=Product_Iron, window=lastMonth) > 0; product.consumed(product=Product_Iron, window=lastMonth) > 0); product.net_flow(product=Product_Iron, window=lastMonth) >= 0)` — [TRUE] [TRUE] Product_Iron produced[lastMonth] = 130; [TRUE] Product_Iron consumed[lastMonth] = 72; [TRUE] Product_Iron window=lastMonth: produced 130 - consumed 72 = net 58

Checks (evaluated against current state):
- `p3-beacon-housing-guard` (warning, action=pause-and-report): **FALSE** — not firing — freeHousingCapacity = 92 (bridge-status.json)
- `p3-truck-congestion-signal` (warning, action=pause-and-report): **FALSE** — not firing — Product_IronOre window=thisMonth: produced 80 - consumed 65 = net 15
- `p3-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-4-concrete-diesel-rubber — Concrete, diesel & rubber [ACTIVE]

Intent: The transitional industry phase: limestone mine + kiln for concrete, oil pumps + basic distiller + rubber maker at the oil patch. Rubber and copper cannot be postponed — this phase ends with a REAL diesel chain replacing the starting stock, and rubber flowing.

**Entry gate: TRUE**
- **TRUE** — `research.completed(node=ResearchVehicleAndMining) == True (implicit)` — ResearchVehicleAndMining in researched list = True
- **TRUE** — `product.quantity(product=Product_Coal) >= 200` — Product_Coal.quantity = 604

**Exit gate: FALSE**
- **TRUE** — `product.quantity(product=Product_ConcreteSlab) >= 100` — Product_ConcreteSlab.quantity = 226
- **TRUE** — `all(any(product.produced(product=Product_Diesel, window=lastMonth) > 0; product.consumed(product=Product_Diesel, window=lastMonth) > 0); product.net_flow(product=Product_Diesel, window=lastMonth) >= 0)` — [TRUE] [TRUE] Product_Diesel produced[lastMonth] = 110; [TRUE] Product_Diesel consumed[lastMonth] = 28; [TRUE] Product_Diesel window=lastMonth: produced 110 - consumed 28 = net 82
- **TRUE** — `product.quantity(product=Product_Rubber) > 0` — Product_Rubber.quantity = 17
- **FALSE** — `product.quantity(product=Product_Diesel) >= 300` — Product_Diesel.quantity = 220

Checks (evaluated against current state):
- `p4-diesel-net-flow` (warning, action=pause-and-report): **FALSE** — not firing — [TRUE] Product_Diesel produced[lifetime] = 235; [FALSE] Product_Diesel window=thisMonth: produced 70 - consumed 15 = net 55
- `p4-rubber-deadline` (warning, action=pause-and-report): **FALSE** — not firing — [TRUE] ResearchBasicDiesel in researched list = True; [FALSE] Product_Rubber.quantity = 17

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-5-copper-construction2-food — Copper, Construction II & food security [NOT REACHED]

Intent: Spend the ship's copper wisely while the permanent copper district (mine + smelter + electrolysis, near power — it is thirsty for water and power) comes online BEFORE the ship stock runs out. Construction II unlocks electric assemblies and the permanent conveyor-ready assembly area. Food security is gated explicitly here: the farm base must carry the settlement with margin.

**Entry gate: TRUE**
- **TRUE** — `all(any(product.produced(product=Product_Diesel, window=lastMonth) > 0; product.consumed(product=Product_Diesel, window=lastMonth) > 0); product.net_flow(product=Product_Diesel, window=lastMonth) >= 0)` — [TRUE] [TRUE] Product_Diesel produced[lastMonth] = 110; [TRUE] Product_Diesel consumed[lastMonth] = 28; [TRUE] Product_Diesel window=lastMonth: produced 110 - consumed 28 = net 82
- **TRUE** — `product.quantity(product=Product_Rubber) > 0` — Product_Rubber.quantity = 17

**Exit gate: FALSE**
- **TRUE** — `research.completed(node=ResearchCopperRefinement) == True (implicit)` — ResearchCopperRefinement in researched list = True
- **FALSE** — `all(any(product.produced(product=Product_Copper, window=lastMonth) > 0; product.consumed(product=Product_Copper, window=lastMonth) > 0); product.net_flow(product=Product_Copper, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_Copper produced[lastMonth] = 0; [TRUE] Product_Copper consumed[lastMonth] = 6; [FALSE] Product_Copper window=lastMonth: produced 0 - consumed 6 = net -6
- **TRUE** — `settlement.months_of_food() >= 3  for: {'months': 1}` — inner predicate TRUE at every snapshot in the trailing 1-month window (anchor 1-11-15, 2 snapshot(s), span 1.00 months) [for: {'months': 1}]
- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)

Checks (evaluated against current state):
- `p5-ship-copper-runway` (critical, action=pause-and-report): **FALSE** — not firing — [FALSE] Product_Copper.quantity = 255; [FALSE] Product_CopperOre window=thisMonth: produced 22 - consumed 18 = net 4
- `p5-food-floor` (critical, action=pause-and-report): **FALSE** — not firing — monthsOfFood = 4 (bridge-status.json)
- `p5-power-proxy` (warning, action=pause-and-report): **FALSE** — not firing — all machine protos: NotEnoughInput 1/15 = 0.067

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-6-automation-expansion — Automation & expansion districts [NOT REACHED]

Intent: Conveyor Belts kill the truck chaos: electric assemblies, belt-fed lines, unit storages with keep-full import rules — one dedicated line per product, no sushi belts. Ship repaired, groundwater and fuel station round out the first-15. Districts from the layout policy become the map's permanent shape.

**Entry gate: TRUE**
- **TRUE** — `research.completed(node=ResearchCopperRefinement) == True (implicit)` — ResearchCopperRefinement in researched list = True
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 4 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchConveyorBelts) == True (implicit)` — ResearchConveyorBelts in researched list = False
- **FALSE** — `research.completed(node=ResearchShipDockRepair) == True (implicit)` — ResearchShipDockRepair in researched list = False
- **FALSE** — `research.completed(node=ResearchFuelStation) == True (implicit)` — ResearchFuelStation in researched list = False
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConstructionParts2, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts2, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts2, window=lastMonth) >= 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows)
- **TRUE** — `census.transports_fully_connected_ratio() >= 0.95` — fullyConnected 11/11 = 1.0000

Checks (evaluated against current state):
- `p6-transport-connectivity` (warning, action=pause-and-report): **FALSE** — not firing — fullyConnected 11/11 = 1.0000
- `p6-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 220

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-7-rail-introduction — Rail introduction (Trains Expansion DLC) [NOT REACHED]

Intent: First rail, honestly scoped: the starter skill hands rail off to the coi-trains sibling skill and notes the in-game train tutorials are new in Update 4 — use them. This phase stands up ONE network and ONE line on the longest truck haul (iron), built by hand, with the command-channel train primitives kept sandbox-only until their semantics are validated (format G3/G10).

**Entry gate: FALSE**
- **FALSE** — `research.completed(node=ResearchFuelStation) == True (implicit)` — ResearchFuelStation in researched list = False
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConstructionParts2, window=lastMonth) > 0; product.consumed(product=Product_ConstructionParts2, window=lastMonth) > 0); product.net_flow(product=Product_ConstructionParts2, window=lastMonth) >= 0)` — [UNEVALUABLE] [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows); [UNEVALUABLE] Product_ConstructionParts2 not in stats.json products (selector matched zero rows)
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 4 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `research.completed(node=ResearchTrains) == True (implicit)` — ResearchTrains in researched list = False
- **FALSE** — `trains.line_count() >= 1` — len(bridge-trains lines) = 0
- **TRUE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) > 0)` — [TRUE] [TRUE] Product_IronOre produced[lastMonth] = 155; [TRUE] Product_IronOre consumed[lastMonth] = 126; [TRUE] Product_IronOre window=lastMonth: produced 155 - consumed 126 = net 29
- **TRUE** — `settlement.months_of_food() >= 3` — monthsOfFood = 4 (bridge-status.json)

Checks (evaluated against current state):
- `p7-rail-not-starving-the-base` (warning, action=pause-and-report): **FALSE** — not firing — Product_Iron window=thisMonth: produced 68 - consumed 38 = net 30

Fallbacks: {"on_gate_stall": "skip-phase", "stall_after": {"months": 3}, "on_check_fail": "pause-and-report"}

## Next orders (DRY RUN — would be prepared, nothing sent)

For active phase `phase-4-concrete-diesel-rubber` [ACTIVE], in plan order:

- **p4-research-concrete-production** — HELD — §4.5 placeholder id in args — manual-only until a human sources the id
  - command `ResearchStartCmd` args={"nodeId": {"placeholder": "Research<ConcreteProduction>", "display_name": "Concrete Production", "status": "unsourced-id", "candidates": ["ResearchConcreteAdvanced (exists in research_ids.txt but is a LATER 'advanced' node per its name \u2014 do NOT substitute it for this early node)"], "confirm": "Read the node id off the in-game research screen before first use."}}
  - note: Research order #7. Display name 'Concrete Production' is U4-verified (research-tree.md: wiki Required Research on Crusher, Rotary Kiln, Concrete Mixer) — but no matching id exists in the extracted id list, so this stays a placeholder and the order is manual-only until sourced. Never write 'Basic Concrete' — that name is explicitly superseded.
- **p4-build-limestone-kiln-concrete** — WOULD PRESENT (human/agent task — not executed)
  - place-blueprint `concrete-district/kiln-plus-mixer-first` district=concrete-district asset_status=authored
  - note: Limestone mine + rotary kiln + concrete mixer (starter phase 4), using the phase-0 scout's limestone position. Transitional siting: concrete production will be rebuilt at scale later; do not overbuild it here. Placement route (v1.2.2): the kiln+mixer block (pack slug concrete-v1) goes through the companion /place flow — human confirm, QoSBuilder job; the limestone mine is deposit-sited and placed manually.
- **p4-research-basic-diesel** — WOULD PREPARE (command, §12 prepare step — not sent)
  - command `ResearchStartCmd` args={"nodeId": "ResearchBasicDiesel"}
  - note: Research order #8. Id exists in research_ids.txt; the exact display form 'Basic Diesel' is single-source in research-tree.md (Charles says 'diesel'; Medium guide corroborates the progression) — flag carried.
  - verify (current state): **FALSE** — [FALSE] [FALSE] research.current = ResearchCp2; [FALSE] queue = []; contains ResearchBasicDiesel = False
- **p4-build-basic-diesel-chain** — WOULD PRESENT (human/agent task — not executed)
  - place-blueprint `oil-district/basic-diesel-3pumps-distiller` district=oil-district asset_status=authored
  - note: At the oil patch (phase-0 scout): 3 oil pumps + basic distiller, wastewater dumped — layout policy explicitly calls this design 'not the most scalable' and temporary. Diesel generators stay on the output side of fuel storage (bottlenecks.md structural rule). Placement route (v1.2.2): the distiller block (pack slug diesel-v1) goes through the companion /place flow — human confirm, QoSBuilder job; the 3 pumps sit on oil-patch tiles and are placed manually (plans-blueprint-status.md ref #8).
- **p4-diesel-storage-alert** — WOULD PREPARE (command, §12 prepare step — not sent)
  - command `StorageAlertSetEnabledCmd` args={"storageId": {"$entity": {"proto": "StorageFluidT2", "product": "Product_Diesel", "match": "first"}, "flags": ["proto-id-unconfirmed"]}, "isEnabled": true, "isAbove": false}
  - note: Turn on the game's own storage alert for the diesel buffer, firing when the level falls BELOW its threshold (SPEC §12.7: align with native alerts rather than duplicating them). Arg names verified against live/commands.json 2026-11-02 (storageId / isEnabled / isAbove). StorageAlertSetThresholdCmd (wired) may set the level in-session.
- **p4-research-synthetic-rubber** — HELD — §4.5 placeholder id in args — manual-only until a human sources the id
  - command `ResearchStartCmd` args={"nodeId": {"placeholder": "Research<SyntheticRubber>", "display_name": "Synthetic Rubber", "status": "unsourced-id", "candidates": ["ResearchRubber", "ResearchRubberProduction"], "confirm": "Two candidate ids exist in research_ids.txt; pick the one the research screen shows for the rubber-maker unlock, then amend the plan."}}
  - note: Research order #9. 'Cannot be postponed: no rubber → no vehicles/electronics → no maintenance → death spiral' (research-order.md). Display name is single-source (Steam list; Charles says 'rubber'). Placeholder because two candidate ids exist and choosing silently would be inventing.
- **p4-build-rubber-maker** — WOULD PRESENT (human/agent task — not executed)
  - place-blueprint `oil-district/rubber-maker` district=oil-district asset_status=authored
  - note: Rubber maker next to the basic diesel chain (starter phase 4). Placement route (v1.2.2): pack slug rubber-v1 — companion /place flow, human confirm, QoSBuilder job.

## Summary

- Predicates evaluated (globals invariants/checks plus phase entry/exit gates and phase checks; composites count once): TRUE 36, FALSE 26, UNEVALUABLE 3 (leaf evidence inside composites is in the per-phase tables via the combined evidence strings).
- Plan status: loop position: phase 5 of 8 — phase-4-concrete-diesel-rubber [ACTIVE].
- Interpretation notes: (1) `for:` durations evaluate over the state sequence per §4.3.1 — TRUE only with an anchor snapshot at/before the window start and the inner predicate TRUE at every snapshot in the window; FALSE if it broke anywhere in the window; UNEVALUABLE on insufficient history, never by default. (2) A phase counts COMPLETE purely when its exit gate is TRUE (§7/§9 re-derivation from state); human-confirm manual orders inside a gate-complete phase are not re-litigated by the evaluator. (3) Unknown / out-of-vocabulary metrics evaluate UNEVALUABLE with a closed-vocabulary note, never a crash and never a guess.

---
DRY RUN — nothing was executed. Generated by `tools/plan_check.py` (read-only over local files).
