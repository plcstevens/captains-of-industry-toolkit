# DRY RUN — nothing was executed

> **DRY RUN — nothing was executed.** This report is a read-only evaluation of a plan file against local state captures. No command was prepared, confirmed, applied or sent anywhere; no game was contacted. Orders below are what a supervised executor *would* prepare next.

# Plan dry-run: dragon-isle-continuation v1.1.0

- Title: Dragon Isle continuation — stop the bleeding, then the relocation plan (Expanding run)
- Map/difficulty (plan): {'name': 'Dragon Isle', 'difficulty': 'captain'} — executor map/difficulty/version match is a live check (§3.1) and cannot be settled from captures; not asserted here.
- Plan game_version: 0.8.7d
- Requires bridge: >=0.6.2
- State source: `/home/hatch/workspace/coi-companion/live` capture files — game-info date **388-2-27**, tick **2787522**, gameId `5583213724730880438`, paused=True, speed=3
- Population 2320, housing 2320 (free 0, homeless 0), monthsOfFood 26, starving False; researchedCount 127, current research ResearchMoltenMaterialTrains

## State-source mapping used by this evaluator

- `product.*` quantity/stored/capacity/flows ← `stats.json` (`Product_`-prefixed ids, exactly as plans name them); windows thisMonth=0, lastMonth=1, thisYear=2, lastYear=3, lifetime=4; `net_flow` = produced − consumed.
- `product.unlocked` ← `products.json` catalogue; `product.pinned` ← membership of `pinned.json`'s product keys (the pinned=true view, captured in stats shape).
- `game.*` ← `game-info.json`; `settlement.*` ← `bridge-status.json`; `research.*` ← `bridge-research.json` (progressPerc ÷ 1000); `census.*` ← `bridge-census.json`; `storage.*` ← `bridge-storages.json` (percentFullRaw ÷ 1000, capacity-weighted over matching rows); `trains.line_count` ← `bridge-trains.json`.
- **UNEVALUABLE by design:** `research.completed` (captures predate CoiBridge v0.6.2 — no `researched` list in `bridge-research.json`, format G1), all `power.*` (no power capture, G2), `contracts/loans/edicts` (no v0.6 captures), `time.*` (no executor run anchor), and any `for:` duration whose instantaneous value is TRUE (a single snapshot cannot prove a duration, §4.3 — see interpretation note).

## Globals

### Invariants (§5.2 — evaluated every iteration)

- **TRUE** — `settlement.starving() == False` — arePeopleStarving = False (bridge-status.json)
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)
- **TRUE** — `product.quantity(product=Product_Diesel) > 0` — Product_Diesel.quantity = 5992
- **TRUE** — `product.quantity(product=Product_Steel) > 0` — Product_Steel.quantity = 1108

### Global checks

- `global-food-floor` (critical): **FALSE** — not firing — monthsOfFood = 26 (bridge-status.json)

## Active-phase determination (§9 play-loop rules)

- `phase-0-stop-the-bleeding` — **COMPLETE** — exit gate TRUE ([TRUE] Product_Steel.quantity = 1108; [TRUE] Product_Diesel.quantity = 5992)
- `phase-1-war-chest-spine` — **ACTIVE** — first non-complete phase; entry gate TRUE, exit gate UNEVALUABLE
- `phase-2-concrete-scale-up` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-3-iron-mines-ore-shuttle` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-4-big-steel-cutover` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-5-fertilizer-then-farms` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-6-housing3-waves` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)
- `phase-7-mega-vehicles-starter-cleared` — **NOT REACHED** — an earlier phase is not complete; phases never overlap (§7)

**Active phase: phase-1-war-chest-spine** — ACTIVE

## Phase-by-phase evaluation

### phase-0-stop-the-bleeding — Stop the bleeding — steel stall + diesel loop [COMPLETE]

Intent: NEW phase (not in the relocation plan; forced by the live data). Two proven structural faults get diagnosed and decided BEFORE any relocation build spends the buffers they drain: (1) steel production ~0 for ~6 months, stock 1,108 falling 20-46/month; (2) the diesel deficit, whose largest single consumer is the hydrocracker DieselReforming loop (112 of 161 consumed this month). The diesel loop is carried as an investigate/DECIDE order with the open question explicit — this plan does not assume it is a leftover.

**Entry gate: TRUE** — empty gate — enters immediately (§7)

**Exit gate: TRUE**
- **TRUE** — `product.quantity(product=Product_Steel) >= 800` — Product_Steel.quantity = 1108
- **TRUE** — `product.quantity(product=Product_Diesel) >= 4000` — Product_Diesel.quantity = 5992

Checks (evaluated against current state):
- `p0-steel-still-falling` (warning, action=pause-and-report): **TRUE** — WOULD FIRE — Product_Steel window=thisMonth: produced 8 - consumed 20 = net -12
- `p0-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 5992
- `p0-power-balance` (warning, action=pause-and-report): **UNEVALUABLE** — check blind (UNEVALUABLE) — power.balance_kw: no power capture exists — the captures predate CoiBridge v0.6.2's /coi/v1/power route (format G2); field power.balance_kw is absent from every capture file

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-1-war-chest-spine — War chest + spine skeleton (relocation Phase 0) [ACTIVE]

Intent: Relocation Phase 0: confirm the war chest (§4) and formalise the N-S rail spine (sidings + district junctions stubbed, central fuel station on its own siding). NO demolitions. The chest's CP rows are already satisfied by live state — the entry gate below evaluates TRUE against the captures, and says so.

**Entry gate: TRUE**
- **TRUE** — `storage.percent_full(product=Product_ConstructionParts) >= 99` — 1 matching row(s): quantity 500/500 capacity, capacity-weighted percentFull = 100.0%; cross-ref stats.json: stored 741/741 (stats totals include stock outside the bridge storage rows)
- **TRUE** — `storage.percent_full(product=Product_ConstructionParts2) >= 99` — 1 matching row(s): quantity 500/500 capacity, capacity-weighted percentFull = 100.0%; cross-ref stats.json: stored 507/507 (stats totals include stock outside the bridge storage rows)
- **TRUE** — `product.stored(product=Product_ConcreteSlab) >= 2900` — Product_ConcreteSlab.stored = 7205 (storageCapacity 7526)
- **TRUE** — `product.quantity(product=Product_Coal) >= 4000` — Product_Coal.quantity = 19811

**Exit gate: UNEVALUABLE**
- **UNEVALUABLE** — `census.transports_fully_connected_ratio() >= 0.99  for: {'evaluations': 5}` — fullyConnected 1015/1025 = 0.9902 [for: {'evaluations': 5} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]
- **UNEVALUABLE** — `storage.percent_full(product=Product_ConstructionParts) >= 99  for: {'months': 1}` — 1 matching row(s): quantity 500/500 capacity, capacity-weighted percentFull = 100.0%; cross-ref stats.json: stored 741/741 (stats totals include stock outside the bridge storage rows) [for: {'months': 1} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]
- **UNEVALUABLE** — `storage.percent_full(product=Product_ConstructionParts2) >= 99  for: {'months': 1}` — 1 matching row(s): quantity 500/500 capacity, capacity-weighted percentFull = 100.0%; cross-ref stats.json: stored 507/507 (stats totals include stock outside the bridge storage rows) [for: {'months': 1} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]
- **TRUE** — `product.quantity(product=Product_Steel) >= 1000` — Product_Steel.quantity = 1108
- **TRUE** — `product.quantity(product=Product_Diesel) >= 4000` — Product_Diesel.quantity = 5992
- **TRUE** — `product.stored(product=Product_ConcreteSlab) >= 2900` — Product_ConcreteSlab.stored = 7205 (storageCapacity 7526)

Checks (evaluated against current state):
- `p1-chest-not-draining` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] 1 matching row(s): quantity 500/500 capacity, capacity-weighted percentFull = 100.0%; cross-ref stats.json: stored 741/741 (stats totals include stock outside the bridge storage rows); [FALSE] 1 matching row(s): quantity 500/500 capacity, capacity-weighted percentFull = 100.0%; cross-ref stats.json: stored 507/507 (stats totals include stock outside the bridge storage rows)
- `p1-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 5992

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-2-concrete-scale-up — Concrete scale-up (relocation Phase 1) [NOT REACHED]

Intent: Relocation Phase 1: the concrete district at §5 sizing (2 blocks = 2 kiln + 2 Mixer II = 48 slab/min, slag-aggregate route). Live state complicates the naive read: 4 Mixer II and 2 kilns ALREADY exist, but at capture 2 mixers are Paused and 2 OutputFull and slab production is 0 this month — so this phase is first a restart/diagnosis of what exists, then ONE additional block, then cutover. Starter concrete is demolished only after the checkpoint (human act; deconstruction is banned for the agent).

**Entry gate: TRUE**
- **TRUE** — `product.stored(product=Product_ConcreteSlab) >= 2900` — Product_ConcreteSlab.stored = 7205 (storageCapacity 7526)
- **TRUE** — `census.transports_fully_connected_ratio() >= 0.99` — fullyConnected 1015/1025 = 0.9902

**Exit gate: FALSE**
- **FALSE** — `census.proto_count(proto=ConcreteMixerT2) >= 5` — byProto[ConcreteMixerT2] = 4 (absent proto counts 0)
- **FALSE** — `census.proto_count(proto=RotaryKiln) >= 3` — byProto[RotaryKiln] = 2 (absent proto counts 0)
- **UNEVALUABLE** — `all(any(product.produced(product=Product_ConcreteSlab, window=lastMonth) > 0; product.consumed(product=Product_ConcreteSlab, window=lastMonth) > 0); product.net_flow(product=Product_ConcreteSlab, window=lastMonth) >= 0)  for: {'months': 1}` — [TRUE] [TRUE] Product_ConcreteSlab produced[lastMonth] = 8; [TRUE] Product_ConcreteSlab consumed[lastMonth] = 4; [TRUE] Product_ConcreteSlab window=lastMonth: produced 8 - consumed 4 = net 4 [for: {'months': 1} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]

Checks (evaluated against current state):
- `p2-slab-buffer-falling` (warning, action=pause-and-report): **FALSE** — not firing — Product_ConcreteSlab window=thisMonth: produced 0 - consumed 0 = net 0
- `p2-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 5992

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 2}, "on_check_fail": "crisis-protocol"}

### phase-3-iron-mines-ore-shuttle — New iron mines + ore shuttle (relocation Phase 2) [NOT REACHED]

Intent: Relocation Phase 2: excavation at the new NE ore body, a dedicated load-only/unload-only iron-ore shuttle to the steel site (district 1 → 2), starter mines left to run out (no demolition — abandonment). Iron ore is net-negative at capture (produced 42 vs consumed 192 this month) — the dying mines are already failing, which is why this phase precedes big steel.

**Entry gate: TRUE**
- **TRUE** — `census.proto_count(proto=TrainDepot) >= 1` — byProto[TrainDepot] = 1 (absent proto counts 0)
- **TRUE** — `census.proto_count(proto=LocomotiveT1Hydrogen) >= 1` — byProto[LocomotiveT1Hydrogen] = 30 (absent proto counts 0)
- **TRUE** — `product.quantity(product=Product_Diesel) >= 4000` — Product_Diesel.quantity = 5992

**Exit gate: FALSE**
- **FALSE** — `trains.line_count() >= 1` — len(bridge-trains lines) = 0
- **FALSE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) >= 0)  for: {'months': 1}` — [TRUE] [TRUE] Product_IronOre produced[lastMonth] = 67; [TRUE] Product_IronOre consumed[lastMonth] = 96; [FALSE] Product_IronOre window=lastMonth: produced 67 - consumed 96 = net -29 [for: {'months': 1} — currently false, so the duration cannot hold]

Checks (evaluated against current state):
- `p3-ore-still-negative` (warning, action=pause-and-report): **TRUE** — WOULD FIRE — Product_IronOre window=thisMonth: produced 42 - consumed 192 = net -150
- `p3-diesel-excavation-draw` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 5992
- `p3-spine-blockage-proxy` (warning, action=pause-and-report): **FALSE** — not firing — fullyConnected 1015/1025 = 0.9902

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 3}, "on_check_fail": "crisis-protocol"}

### phase-4-big-steel-cutover — Big steel plant + cutover trial (relocation Phase 3) [NOT REACHED]

Intent: Relocation Phase 3 — the highest-risk phase, and the definitive fix for the phase-0 wound IF the wound is capacity/input at the starter site: big steel at district 2 (adjacent to the new mines), built in caster pairs (§5/§8.1: casters are the unit, molten steel + oxygen are the constraints), cut over under a pause-don't-demolish trial of the starter steel.

**Entry gate: FALSE**
- **FALSE** — `trains.line_count() >= 1` — len(bridge-trains lines) = 0
- **FALSE** — `all(any(product.produced(product=Product_IronOre, window=lastMonth) > 0; product.consumed(product=Product_IronOre, window=lastMonth) > 0); product.net_flow(product=Product_IronOre, window=lastMonth) >= 0)` — [TRUE] [TRUE] Product_IronOre produced[lastMonth] = 67; [TRUE] Product_IronOre consumed[lastMonth] = 96; [FALSE] Product_IronOre window=lastMonth: produced 67 - consumed 96 = net -29
- **TRUE** — `product.quantity(product=Product_Steel) >= 800` — Product_Steel.quantity = 1108

**Exit gate: FALSE**
- **FALSE** — `census.proto_count(proto=CasterCooledT2) >= 6` — byProto[CasterCooledT2] = 4 (absent proto counts 0)
- **FALSE** — `all(any(product.produced(product=Product_Steel, window=lastMonth) > 0; product.consumed(product=Product_Steel, window=lastMonth) > 0); product.net_flow(product=Product_Steel, window=lastMonth) > 0)  for: {'months': 1}` — [TRUE] [FALSE] Product_Steel produced[lastMonth] = 0; [TRUE] Product_Steel consumed[lastMonth] = 46; [FALSE] Product_Steel window=lastMonth: produced 0 - consumed 46 = net -46 [for: {'months': 1} — currently false, so the duration cannot hold]
- **FALSE** — `product.quantity(product=Product_Steel) >= 1500` — Product_Steel.quantity = 1108

Checks (evaluated against current state):
- `p4-steel-buffer-falling-in-trial` (critical, action=pause-and-report): **TRUE** — WOULD FIRE — Product_Steel window=thisMonth: produced 8 - consumed 20 = net -12
- `p4-coal-buffer-trend` (warning, action=pause-and-report): **TRUE** — WOULD FIRE — Product_Coal window=thisMonth: produced 0 - consumed 218 = net -218

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 3}, "on_check_fail": "crisis-protocol"}

### phase-5-fertilizer-then-farms — Fertilizer chain, THEN farm district (relocation Phase 4) [NOT REACHED]

Intent: Relocation Phase 4, order enforced by preconditions, not by hope: Air Separators → Hydrogen Reformer (fuel gas) → ammonia → Fertilizer I at §5 sizing (60/min capacity against a ~37/min full-district draw), fertilizer buffer FULL, and only then the 20-farm district (7 soya / 5 wheat / 2 potato / 3 veg / 3 corn + tofu processor + half-rate bakery) with its seawater supply. Live state: the chemical seed exists (ChemicalPlant ×4 incl. FertilizerProduction recipe, AirSeparator ×3, HydrogenReformer ×14) but Product_Fertilizer quantity is 10 with ZERO stored and only 500 storage capacity — the chain is built-ish and the buffer/flow are not. Farms: FarmT2 ×8 exist (the starter farms, retired LAST, after new output flows).

**Entry gate: FALSE**
- **TRUE** — `settlement.months_of_food() >= 6` — monthsOfFood = 26 (bridge-status.json)
- **FALSE** — `all(any(product.produced(product=Product_Steel, window=lastMonth) > 0; product.consumed(product=Product_Steel, window=lastMonth) > 0); product.net_flow(product=Product_Steel, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_Steel produced[lastMonth] = 0; [TRUE] Product_Steel consumed[lastMonth] = 46; [FALSE] Product_Steel window=lastMonth: produced 0 - consumed 46 = net -46

**Exit gate: FALSE**
- **FALSE** — `product.storage_capacity(product=Product_Fertilizer) >= 2000` — Product_Fertilizer.storageCapacity = 500
- **FALSE** — `product.stored(product=Product_Fertilizer) >= 1500` — Product_Fertilizer.stored = 0 (storageCapacity 500)
- **FALSE** — `census.proto_count(proto=FarmT2) >= 20` — byProto[FarmT2] = 8 (absent proto counts 0)
- **TRUE** — `settlement.months_of_food() >= 6` — monthsOfFood = 26 (bridge-status.json)
- **UNEVALUABLE** — `all(any(product.produced(product=Product_Fertilizer, window=lastMonth) > 0; product.consumed(product=Product_Fertilizer, window=lastMonth) > 0); product.net_flow(product=Product_Fertilizer, window=lastMonth) >= 0)  for: {'months': 1}` — [TRUE] [TRUE] Product_Fertilizer produced[lastMonth] = 30; [TRUE] Product_Fertilizer consumed[lastMonth] = 15; [TRUE] Product_Fertilizer window=lastMonth: produced 30 - consumed 15 = net 15 [for: {'months': 1} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]

Checks (evaluated against current state):
- `p5-fertilizer-before-farms-guard` (critical, action=pause-and-report): **FALSE** — not firing — [FALSE] byProto[FarmT2] = 8 (absent proto counts 0); [TRUE] Product_Fertilizer.stored = 0 (storageCapacity 500)
- `p5-food-floor-transition` (critical, action=pause-and-report): **FALSE** — not firing — monthsOfFood = 26 (bridge-status.json)
- `p5-soya-margin` (warning, action=pause-and-report): **TRUE** — WOULD FIRE — Product_Soybean.quantity = 28

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 3}, "on_check_fail": "crisis-protocol"}

### phase-6-housing3-waves — Settlement solidification — Housing III waves (relocation Phase 5) [NOT REACHED]

Intent: Relocation Phase 5: supply chains FIRST, housing upgrades in waves. Live state: HousingT3 ×5 already stand beside HousingT2 ×8, but the Housing III supply chains DO NOT EXIST — HouseholdGoods, MedicalSupplies, MedicalEquipment and Disinfectant all read quantity 0 with lifetime production 0. This phase builds the household-goods line (glass + steel + wood → 16/min per Assembly Electrified II) and the medical chain (medical equipment + disinfectant → Medical Supplies, 24/min per unit), fills both buffers, and only then rolls upgrade waves. Coal import is stood up/confirmed here if not already (import buffer doctrine from §4/Phase 5).

**Entry gate: FALSE**
- **FALSE** — `all(any(product.produced(product=Product_Steel, window=lastMonth) > 0; product.consumed(product=Product_Steel, window=lastMonth) > 0); product.net_flow(product=Product_Steel, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_Steel produced[lastMonth] = 0; [TRUE] Product_Steel consumed[lastMonth] = 46; [FALSE] Product_Steel window=lastMonth: produced 0 - consumed 46 = net -46
- **TRUE** — `settlement.months_of_food() >= 6` — monthsOfFood = 26 (bridge-status.json)
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)

**Exit gate: FALSE**
- **FALSE** — `product.quantity(product=Product_HouseholdGoods) >= 100` — Product_HouseholdGoods.quantity = 0
- **FALSE** — `product.quantity(product=Product_MedicalSupplies) >= 50` — Product_MedicalSupplies.quantity = 0
- **FALSE** — `all(any(product.produced(product=Product_HouseholdGoods, window=lastMonth) > 0; product.consumed(product=Product_HouseholdGoods, window=lastMonth) > 0); product.net_flow(product=Product_HouseholdGoods, window=lastMonth) >= 0)  for: {'months': 1}` — [FALSE] [FALSE] Product_HouseholdGoods produced[lastMonth] = 0; [FALSE] Product_HouseholdGoods consumed[lastMonth] = 0; [TRUE] Product_HouseholdGoods window=lastMonth: produced 0 - consumed 0 = net 0 [for: {'months': 1} — currently false, so the duration cannot hold]
- **FALSE** — `census.proto_count(proto=HousingT3) >= 6` — byProto[HousingT3] = 5 (absent proto counts 0)
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)

Checks (evaluated against current state):
- `p6-supply-before-upgrades-guard` (warning, action=pause-and-report): **FALSE** — not firing — [FALSE] byProto[HousingT3] = 5 (absent proto counts 0); [TRUE] [TRUE] Product_HouseholdGoods.quantity = 0; [TRUE] Product_MedicalSupplies.quantity = 0
- `p6-coal-import-buffer` (warning, action=pause-and-report): **FALSE** — not firing — Product_Coal.quantity = 19811

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 3}, "on_check_fail": "crisis-protocol"}

### phase-7-mega-vehicles-starter-cleared — Mega Vehicles + starter-space cleanup (relocation Phase 6) [NOT REACHED]

Intent: Relocation Phase 6: research Mega Vehicles (late tree — the source plan places it just before Nuclear Reactor, exact prerequisites unpublished), replace the mine/construction fleets progressively (oldest first, never a whole fleet at once), backfill the two worked-out pits (iron + coal) with spoil from the phase-3 excavation, and clear the starter space — which the island survey shows sits INSIDE the only flat corridor band, so clearing it removes the spine's pinch point, not just a leftover.

**Entry gate: FALSE**
- **FALSE** — `product.quantity(product=Product_HouseholdGoods) > 0` — Product_HouseholdGoods.quantity = 0
- **FALSE** — `all(any(product.produced(product=Product_Steel, window=lastMonth) > 0; product.consumed(product=Product_Steel, window=lastMonth) > 0); product.net_flow(product=Product_Steel, window=lastMonth) >= 0)` — [TRUE] [FALSE] Product_Steel produced[lastMonth] = 0; [TRUE] Product_Steel consumed[lastMonth] = 46; [FALSE] Product_Steel window=lastMonth: produced 0 - consumed 46 = net -46

**Exit gate: FALSE**
- **UNEVALUABLE** — `research.completed(node=<placeholder Research<MegaVehicles>>) == True (implicit)` — node is a §4.5 placeholder (Research<MegaVehicles>) — manual-only, never auto-evaluated
- **FALSE** — `all(any(product.produced(product=Product_VehicleParts2, window=lastMonth) > 0; product.consumed(product=Product_VehicleParts2, window=lastMonth) > 0); product.net_flow(product=Product_VehicleParts2, window=lastMonth) >= 0)` — [FALSE] [FALSE] Product_VehicleParts2 produced[lastMonth] = 0; [FALSE] Product_VehicleParts2 consumed[lastMonth] = 0; [TRUE] Product_VehicleParts2 window=lastMonth: produced 0 - consumed 0 = net 0
- **TRUE** — `settlement.homeless() == 0` — numberOfHomeless = 0 (bridge-status.json)
- **TRUE** — `settlement.months_of_food() >= 6` — monthsOfFood = 26 (bridge-status.json)

Checks (evaluated against current state):
- `p7-vehicle-parts-strain` (warning, action=pause-and-report): **FALSE** — not firing — Product_VehicleParts2.quantity = 196
- `p7-diesel-floor` (critical, action=crisis-protocol): **FALSE** — not firing — Product_Diesel.quantity = 5992

Fallbacks: {"on_gate_stall": "pause-and-report", "stall_after": {"months": 3}, "on_check_fail": "crisis-protocol"}

## Next orders (DRY RUN — would be prepared, nothing sent)

For active phase `phase-1-war-chest-spine` [ACTIVE], in plan order:

- **p1-war-chest-audit** — WOULD PRESENT (human/agent task — not executed)
  - observe-only: Audit the remaining §4 rows against live state and record each as MET / NOT MET / UNEVALUABLE: steel >=120 min of total downstream draw (UNEVALUABLE as minutes — the quantity proxy is phase 0's exit floor, already gated); diesel and maintenance >=120 min of fleet+plant draw (UNEVALUABLE — maintenance has no product, format G6; diesel draw is not a recipe rate); fertilizer row belongs to phase 5, not here; food row is [pop]-marked in the source plan and the capture now supplies the figure (population 2,320, monthsOfFood 26 — record MET with that evidence). Do not start phase 2 builds on a chest this audit calls NOT MET without a human decision recorded in the run report.
- **p1-formalise-spine** — WOULD PRESENT (human/agent task — not executed)
  - place-blueprint `spine/north-south-trunk-with-district-sidings` district=spine asset_status=not-yet-authored
  - note: Degrades to a manual build: formalise the N-S spine in the west coastal plain (trunk + parallel rail gap), sidings and junction stubs for districts 1-5, NO industry on the trunk, districts set back behind the gap (Dragon Isle layout doctrine + relocation §2/§6 audit: stations on sidings never the through route, no reversing through junctions, crossings out of no-stop zones).
- **p1-central-fuel-station** — WOULD PRESENT (human/agent task — not executed)
  - manual-step: Stand up the central fuel station ON the spine, on its own siding, with the train depot co-located on the same siding (relocation §8.2): fuel in at the fuel station, nothing loaded where it is unloaded, depot moves never occupy the through route. Census evidence: FuelStationHydrogenT1 ×6 and TrainDepot ×1 already exist — verify in the UI whether they already meet this spec before building anything new.
- **p1-steel-buffer-keep-full** — WOULD PREPARE (command, §12 prepare step — not sent)
  - command `StorageSetSliderStepCmd` args={"storageId": {"$entity": {"proto": "StorageUnitT2", "product": "Product_Steel", "match": "first"}}, "importStep": 10}
  - note: Set the steel buffer's import slider to full (step 10 = 100%; slider steps are Percent/10000 per the v0.6.1 recon). Wired command — first live use is human-confirmed. Before/after slider values ARE readable once bridge v0.6.1+ is installed (format G7 resolved: storage rows carry importStep), so the executor verifies by re-reading the storages route, not by this predicate list.

## Summary

- Predicates evaluated (globals invariants/checks plus phase entry/exit gates and phase checks; composites count once): TRUE 31, FALSE 35, UNEVALUABLE 7 (leaf evidence inside composites is in the per-phase tables via the combined evidence strings).
- Plan status: loop position: phase 2 of 8 — phase-1-war-chest-spine [ACTIVE].
- Interpretation notes: (1) `for:` durations — instantaneous FALSE stays FALSE; instantaneous TRUE becomes UNEVALUABLE because one snapshot cannot prove the predicate held for the whole duration (§4.3). (2) A phase counts COMPLETE purely when its exit gate is TRUE (§7/§9 re-derivation from state); human-confirm manual orders inside a gate-complete phase are not re-litigated by the evaluator. (3) Unknown / out-of-vocabulary metrics evaluate UNEVALUABLE with a closed-vocabulary note, never a crash and never a guess.

---
DRY RUN — nothing was executed. Generated by `tools/plan_check.py` (read-only over local files).
