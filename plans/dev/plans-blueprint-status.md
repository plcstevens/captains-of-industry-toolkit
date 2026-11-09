# Plans ↔ blueprint-pack status — Round 8 chunk 2 (2026-11-04)

**Method: map-first, author-only-if-honest.** Each of the 20 plan
`blueprint_ref`s was read against its plan order's note (machines, recipes,
counts) and the pack's module JSONs. A ref was **mapped** only where an
existing pack slug genuinely matches the spec — same machines, same
recipes, same scale class (a starter block was never stretched into a
district block). A ref was **authored** only where the missing design is a
terrain-independent machine cluster fully determined by dataset ratios
(coi-data-0.8.7d) plus the plan note, and every machine in it has a genuine
harvested prototype in the pack's sources. Everything else stays
`not-yet-authored` with the precise blocker named — terrain/deposit shape,
a core prototype that exists nowhere in the pack's sources, a design the
plan note leaves state-contingent, or a chain leg the data does not size.

**Audit totals after this chunk**

- Pack: `audit_final.py` — **82 slugs, ISSUE COUNT 0** (was 80). New
  modules render with **0 unassigned** transports each.
- Plans: `audit_blueprints.py` — 20 refs: **9 AUTHORED, 11
  MISSING-DECLARED, 0 MISSING-UNDECLARED**. (The audit tool gained a
  minimal `pack_slug` check this chunk: an `authored` ref counts only if
  its `pack_slug` — and `pack_slug_overlay` when present — exists in the
  pack; otherwise it reports MISSING-UNDECLARED. All pre-existing
  verdicts are unchanged.)
- `validate_plan.py`: **PASS, 0 errors** on both plans.
  `check_plan_parity.py`: **PARITY PASS** on both plans.
- Plan versions: fresh-start **1.2.0 → 1.2.1**; dragon-isle-continuation
  **1.1.0 → 1.1.1** (patch bumps; only asset fields changed —
  `blueprint_ref` untouched everywhere).

Entity counts below are the audit's decoded counts (audit_final per-slug
`n=`), not the older manifest fields.

## The 20 refs

| # | Ref | Plan / order | Status | Detail |
|---|-----|--------------|--------|--------|
| 1 | starter-cluster/scrap-smelter-1furnace-2caster-plus-coal-maker | fresh-start / p1-build-starter-cluster | **mapped: iron-v1** | iron-v1 is the exact smelter core: 1× SmeltingFurnaceT1 on IronSmeltingT1Scrap + 2× Caster = 24 iron/min (26 entities, pack audit PASS). The order's other items — wood-burning coal maker, diesel generator, 2 research labs — are single-building manual placements: **no genuine prototype for any of them exists in any pack source** (verified across all hub-source + harvest files), so no paste can contain them honestly; the coal maker's output lands in iron-v1's coal input buffer. |
| 2 | starter-cluster/manual-assemblies-x4 | fresh-start / p2-build-assemblies | **authored: starter-assemblies-v1** | NEW — 64 entities, pack audit PASS, render 0 unassigned. 4× AssemblyManual on cparts-v1's proven lanes (shared iron/wood/slab buffers); A1 preset CpAssembly, A2–A4 recipes cleared for in-game selection (dataset confirms AssemblyManual runs mech-parts/electronics recipes, but their in-blueprint strings were never harvested — never guessed); per-assembly output buffers so mixed recipes cannot sushi-mix. cparts-v1+v2 would also reach 4 manual assemblers, but as a dedicated CP-only pair-of-pairs — not this mixed-recipe starter bank. |
| 3 | starter-cluster/maintenance-depot | fresh-start / p2-build-maintenance-depot | still-missing | No genuine MaintenanceDepot prototype exists in any pack source (mechparts-v1/v3 carry only depot *feed* infrastructure with the depots themselves as manual slots). The order's verify requires a real MaintenanceDepotT1 census count ≥ 1 — a feed-stub paste cannot produce it. Blocked on a genuine depot harvest. |
| 4 | starter-cluster/first-farm | fresh-start / p2-build-first-farm | still-missing | FARM_GUIDANCE.md is guidance-only by its own status line ("no drop-in blueprint strings in this pack yet" — the nested folder format is unparsed). The only genuine farm prototype anywhere is FarmT2 (irrigated; its gate is not in the starter research-15), which is the wrong tier for a phase-2 base farm; no genuine base-Farm prototype exists. Plus settlement-edge siting and crop proto ids unsourced in the plan. |
| 5 | mining/coal-mine-first | fresh-start / p3-build-coal-mine | still-missing | Terrain/deposit-bound: Mine Control Tower + excavator faces must sit on the coal deposit (veins run below sea level; setback rules in the order note). No mining prototypes exist in the pack. Not a paste-able cluster. |
| 6 | smelting-district/ore-smelter-anchor | fresh-start / p3-build-iron-mine-and-ore-smelter | **mapped: iron-v1 + overlay iron-v2** | The iron-steel skill's Stage-1 permanent anchor build is exactly this: 1× Blast Furnace + 2× Metal Caster on the ore recipe (30 ore + 12 coal → 24 iron/min). iron-v1 pasted fresh at the permanent site + iron-v2 (6 entities) equips it for the ore/coal recipe with the slag export that recipe needs. The iron mine itself is deposit-sited (as #5) and is not part of the paste. |
| 7 | concrete-district/kiln-plus-mixer-first | fresh-start / p4-build-limestone-kiln-concrete | **mapped: concrete-v1** | Exact machine spec: 1× Rotary Kiln (gas) + 1× Concrete Mixer II = 24 slabs/min, buffered (39 entities, pack audit PASS). The limestone mine is deposit-sited (as #5), not part of the paste. |
| 8 | oil-district/basic-diesel-3pumps-distiller | fresh-start / p4-build-basic-diesel-chain | **mapped: diesel-v1** | diesel-v1 is the Basic Diesel block: 1× BasicDieselDistiller, 27 diesel/min, crude + coal buffered, wastewater to a shore-dump stub — the plan note's "wastewater dumped" (25 entities, pack audit PASS). The 3 oil pumps sit on oil-patch tiles (deposit-bound; no genuine OilPump prototype exists in any source) and are placed at the patch, feeding this block's crude buffer. diesel-refining-v1 is an equivalent duplicate family; diesel-v1 chosen as the starter-family original (its v3 sibling is the same harvest's rubber tower). |
| 9 | oil-district/rubber-maker | fresh-start / p4-build-rubber-maker | **mapped: rubber-v1** | Exact: 1× VacuumDistillationTower, diesel + sulfur → 18 rubber/min, buffered (16 entities, pack audit PASS). |
| 10 | copper-district/mine-smelter-electrolysis | fresh-start / p5-build-copper-district | still-missing | The pack's standalone copper family (copper-v1/v2/v3) is **scrap-fed** — its manual steps prescribe the scrap recipes for the ship-stock stopgap. The plan's permanent district is **ore-fed** from its own mine (copper skill stages 1–2: 1 furnace + 2 casters + 1 electrolysis on ore); the only ore-fed instance of that block in the pack is embedded inside electronics-v1, a different module whose assembly lines are its purpose. The mine is additionally deposit-sited. Needs an ore-fed copper block authored around the electronics-v1 cell — not a mapping. |
| 11 | assembly-district/permanent-conveyor-ready-v1 | fresh-start / p5-build-permanent-assembly-area | still-missing | District-scale layout, and the design is not determined: the order note specifies no machine counts, recipes, or product mix ("start the permanent conveyor-ready assembly area with storage buffers"). The pack's assembly families (cparts, cparts-advanced) are dedicated single-product lines, not a multi-line district. Nothing honest to map or author until the district's line list is specified. |
| 12 | logistics/fuel-station-first | fresh-start / p6-build-fuel-station | still-missing | No genuine FuelStation prototype exists in any pack source (searched all hub + harvest files). It is also a single building whose siting is bound to the vehicle depot / dock position — site placement, not a cluster. |
| 13 | spine/north-south-trunk-with-district-sidings | dragon-isle / p1-formalise-spine | still-missing | Map-length infrastructure, not a paste: a full-island N-S trunk + parallel rail gap + district junction stubs must follow this island's coastline and ridge foot. Length and junction positions exist only on the map. |
| 14 | concrete-district/kiln-plus-mixer2-block | dragon-isle / p2-build-one-concrete-block | **mapped: concrete-v1** | The relocation §5 block is 1× Rotary Kiln + 1× Concrete Mixer II = 24 slabs/min — exactly concrete-v1 (39 entities). The pack kiln is the gas variant, which is §5's own named coal-free swap; the mixer's crushed-slag gravel route is §5's default aggregate route. |
| 15 | mining/ne-iron-excavation | dragon-isle / p3-build-new-iron-mines | still-missing | Deposit-shaped: excavation faces and mega-excavator working faces follow the NE ore body's shape at the ridge-foot flat spot. Terrain work, not a paste-able cluster. |
| 16 | steel-district/caster-pair-block | dragon-isle / p4-build-big-steel | **mapped: steel-v1 + overlay steel-v2** | steel-v1 = one T2 line (24 steel/min, 55 entities); steel-v2 = the second line overlay (46 entities) — combined 48 steel/min, the §5 caster pair: 2× Oxygen Furnace II + 2× Caster (Cooled) II, buffered. Documented divergence: the pack line feeds each oxygen furnace from 2× Smelting Furnace II at its harvested 24-molten rate, where §5's dataset rate (48 molten per furnace) needs one — the output spec (48 steel/min, slag to concrete) is identical; the pack's furnace-stage count is the conservative one. |
| 17 | chemical-district/fertilizer-60-block | dragon-isle / p5-build-fertilizer-capacity | still-missing | Two blockers. (1) The core machine (Chemical Plant ×6 at §5 sizing: 2 fertilizer + 4 ammonia) has **no genuine prototype** in any pack source — chemicals-v1..v3 carry it only as manual drop-zone stubs, and at Fertilizer II completion scale (36/min), not 60 Fertilizer I/min. (2) The order is explicitly gap-filling: what must be built depends on the p5 audit of the existing plants' recipe assignments, so no fixed paste can be "the" block. The order's verify (fertilizer storage capacity ≥ 2,000) is a storage-designation UI act, not a paste. |
| 18 | farm-district/20-farm-mix-with-tofu-bread | dragon-isle / p5-build-farm-district | still-missing | Terrain + irrigation layout: 20 FarmT2 (7 soya / 5 wheat / 2 potato / 3 veg / 3 corn) at ~40×30 tiles each plus a ~168/min seawater supply that must land with the earthworks (relocation §8.4). The crop mix is verifiable only at placement; the district's shape follows district-5 ground. |
| 19 | settlement-supply/household-goods-line | dragon-isle / p6-build-household-goods-line | **authored: household-goods-v1** | NEW — 21 entities, pack audit PASS, render 0 unassigned. 1× AssemblyElectrifiedT2 running HouseholdGoodsAssemblyT2 (dataset dictionary 30: 4 glass + 2 steel + 4 wood / 30 s → 16/min — exactly the relocation §5 per-unit rate), glass/steel/wood input buffers + HG output buffer, consumer-goods-v1's geometry with the machine tier corrected. NOT mapped to consumer-goods-v1: that line is AssemblyRoboticT2 (Assembly IV) at 32/min — wrong tier, double rate. Caveat carried in the module's manual steps: the ElectrifiedT2 prototype comes from a 0.5.5d harvest (electronics-v3 precedent) — confirm placement on U4; consumer-goods-v1 is the documented fallback. |
| 20 | settlement-supply/medical-chain | dragon-isle / p6-build-medical-chain | still-missing | The assembly legs are dataset-grounded (MedicalSuppliesAssemblyT1 on Electrified II = 24/min, matching §5; equipment leg 6/min), but the chain's binding leg — disinfectant — is Chemical-Plant-only (ethanol + plastic → disinfectant), and Chemical Plant has no genuine prototype (as #17). Ethanol/plastic sub-chain sizing is not in the plan note, and the order itself prescribes assignment/flow repair on the existing chemical plants (disinfectant already among their recipes, output 0) before new builds. A paste of only the assembly leg would duplicate the ×33 Electrified II assemblies the base already has starved. |

## New pack modules (this chunk)

| Slug | Entities (decoded) | Pack audit | Render | Registered |
|------|--------------------|------------|--------|------------|
| starter-assemblies-v1 | 64 | PASS (audit_final: 82 slugs, ISSUE COUNT 0) | 0 unassigned | NEW_MODULES_plan_linkage.json + PACK_MANIFEST phase 7 |
| household-goods-v1 | 21 | PASS (same run) | 0 unassigned | NEW_MODULES_plan_linkage.json + PACK_MANIFEST phase 7 |

Built with `rebuild_plan_linkage_v1.py` (run as `__main__` only; no
`build_*.py` was imported). `audit_final.py`'s expected-slug count was
updated 80 → 82 with the increment documented in its comment, following
the Round-7 pattern.

## Addendum — 2026-11-05 (fresh-start v1.2.2): /place placeability

With QoSBuilder built (SPEC Addendum XVII), "authored" now also means
**placeable through the companion /place flow**: the layout must be in
the embedded library (`coi/data/blueprint-library.json`, generated by
`tools/gen_blueprint_library.py`, 82 entries, decode-verified). Checked
for every fresh-start authored ref — all present:

| pack_slug | ref | in embedded library |
|---|---|---|
| iron-v1 | #1 starter-cluster (p1) + #6 smelting anchor (p3) | yes |
| iron-v2 (overlay) | #6 smelting anchor (p3) | yes |
| starter-assemblies-v1 | #2 manual-assemblies (p2) | yes |
| concrete-v1 | #7 kiln-plus-mixer (p4) | yes |
| diesel-v1 | #8 basic-diesel (p4) | yes |
| rubber-v1 | #9 rubber-maker (p4) | yes |

The 6 fresh-start not-yet-authored refs (#3 maintenance-depot,
\#4 first-farm, #5 coal-mine-first, #10 copper-district,
\#11 permanent-assembly-area, #12 fuel-station-first) have no library
asset and remain manual builds — the /place flow cannot offer them.
Honesty line unchanged: the pack's strings are NOT in-game tested;
first placement happens on Philip's new save only under his confirm
(see plans/WEEKEND-RUNBOOK.md step 4).
