# Fixture: early-game (synthetic state capture)

A hand-built, plausible **month-1 new game** state for dry-running
`plans/fresh-start-v1.yaml` with `tools/plan_check.py`. It is a fixture —
no game was contacted and none of these numbers come from a real save.
File shapes are copied from the real captures in `live/` (same keys,
same window ordering, `percentFullRaw` = percent × 1000,
`progressPerc` = percent × 1000).

Snapshot moment: in-game date **1-1-18**, game paused at speed 1 —
mid-phase-1. The phase-0 scout is done, the disposable starter cluster
is built and smelting, but the iron stockpile has not yet reached the
phase-1 exit threshold.

## Key numbers and why

- **Settlement** (`bridge-status.json`): population **100**, housing
  capacity **160** (free 60, homeless 0) — the plan's own layout-policy
  assumption (~100 pop in ~160 container housing). `monthsOfFood` **4**:
  food stock is 300 (240 potato + 60 vegetables) against an 80/month
  burn in `stats.json` → 3.75 months, recorded as 4. Not starving.
- **Research** (`bridge-research.json`): `researchedCount` **0**,
  `hasActiveLab` true (2 labs), current node `ResearchCp1Production`
  (the plan's candidate id for "Construction") at 43.750%
  (35/80 steps — stepsDone + remainingSteps = scienceCost), queue
  `[ResearchPowerAndMaintenance]`. This file **does** include a
  v0.6.2-style `researched` list — deliberately the **empty list**, so
  `research.completed` evaluates honestly to FALSE for real node ids
  instead of UNEVALUABLE.
- **Census** (`bridge-census.json`): starter buildings only —
  1× SmeltingFurnaceT1 (scrap, recipe `IronSmeltingT1Scrap`),
  2× CasterT1, 1× CoalMaker, 1× DieselGeneratorT1, 2× ResearchLab1,
  4 trucks, 4 storage buildings, 1 waste dump; **no** MaintenanceDepotT1
  and no AssemblyManual (both are phase-2 builds, correctly absent).
  Machine states: 4 total, 1 NotEnoughInput (one caster waiting on
  molten iron) → starved share 0.250, below both proxy-check thresholds.
  Transports 4/4 fully connected (molten channels; the cluster is
  truck-fed, not belted). Some T1 proto/recipe labels are plausible
  fixture labels where the live (late-game) capture only attests the
  T2/late equivalents.
- **Products** (`stats.json`): starter set only. All flow windows are
  first-month consistent: lastMonth = lastYear = 0 everywhere and
  thisYear = lifetime. For produced goods with no starting stock,
  lifetime produced − consumed = quantity exactly (scrap 1250−400=850,
  iron 58−16=42, coal 90−25=65, wood 260−120=140, water 300−150=150).
  - IronScrap **850** (> 0 → phase-1 entry TRUE; > 100 → scrap-runway
    check quiet).
  - Iron **42** (< 100 → phase-1 exit FALSE; this is the intended
    blocker keeping phase 1 ACTIVE).
  - Diesel **220**: starting stock 300 − 80 burned, none produced yet.
    > 0 (global invariant TRUE), above the phase-1/2 floors (100) and
    the phase-3 floor (150) and phase-6 floor (200), below the phase-4
    exit stock target (300).
  - Copper **300** (untouched ship's-hold stock, per the plan's
    assumption), ConcreteSlab **180** (300 starting/claimed − 120
    spent), ConstructionParts **18** and MechanicalParts **6**
    (dwindling starting stock, no production yet), Steel **10**.
  - Absent from stats on purpose, so their predicates are honestly
    UNEVALUABLE rather than invented: Product_Rubber,
    Product_IronOre, Product_CopperOre, Product_ConstructionParts2.
  - `products.json` catalogue lists the starter set as unlocked plus
    three visibly still-locked near-term products (ConstructionParts2,
    Rubber, Electronics). `pinned.json` pins Diesel + IronScrap, as
    phase 0's pin-vitals order prescribes.
- **Storages** (`bridge-storages.json`): 4 rows (scrap 800/1000, iron
  40/200, diesel 200/250, wood 120/500); every row's quantity ≤ the
  matching product's `stored` in stats.json, and every product's
  `stored ≤ storageCapacity` in stats.
- **Trains** (`bridge-trains.json`): `{"lines": []}` — no rail yet.

## Python engine verdict (2026-11-03 run; refreshed for plan v1.1.0 guards, Round 5)

Command: `python3 tools/plan_check.py plans/fresh-start-v1.yaml
--live-dir plans/fixtures/early-game --out
plans/fresh-start-v1.early-game.dryrun.md` — exit code **0**, report
carries the DRY RUN banner, rerun is byte-identical.

- **Active phase: `phase-1-scrap-starter` — ACTIVE.**
  Phase 0 COMPLETE (exit TRUE: population 100 > 0, free housing 60 ≥ 0);
  phase 1 entry TRUE (IronScrap 850 > 0), exit FALSE (Iron 42 < 100,
  despite hasActiveLab TRUE and research in progress);
  phases 2–7 all **NOT REACHED**, and every late phase's exit gate is
  FALSE — no late phase reads COMPLETE and no gate is TRUE on absurd
  evidence. All 3 global invariants TRUE; global food-floor check not
  firing. Totals: TRUE 15, FALSE 41, UNEVALUABLE 9 (post-guard;
  pre-guard run read 21 / 34 / 10). Phase statuses are unchanged by
  the guards — only individual gate verdicts moved.
- Surprising / noteworthy gates (details in the coordinator report):
  1. `p4-diesel-net-flow` (phase 4, not reached) read **WOULD FIRE**
     pre-guard: diesel net flow thisMonth = −30 — true arithmetic,
     but the check applies "once the basic diesel chain exists" and
     the predicate had no such guard, so it fired on ordinary
     starting-stock drawdown in month 1. **RESOLVED in plan v1.1.0**
     (Round 5): the check now carries a diesel-chain guard
     (`product.produced Product_Diesel lifetime > 0`) and reads
     FALSE on this fixture; it still fires on the live capture,
     where lifetime diesel production is 423,589 and the deficit is
     real.
  2. Every `product.net_flow … window: lastMonth >= 0` predicate
     read **TRUE on 0 − 0** in a first-month game (empty history):
     phase-3 entry, phase-4 exit (diesel), phase-5 entry (diesel)
     and exit (copper). No composite gate went TRUE because of it
     (research / stock gates blocked), but phase 5's entry gate
     consequently read UNEVALUABLE rather than FALSE. **RESOLVED in
     plan v1.1.0** (Round 5): every `lastMonth` net_flow leaf in
     both plans now carries an activity guard (produced > 0 OR
     consumed > 0 in the same window); on this fixture they read
     FALSE, and phase-5's entry reads FALSE.
  3. The report's hardcoded state-mapping note claimed
     `research.completed` is UNEVALUABLE because captures have no
     `researched` list — stale for this v0.6.2-style fixture, where
     the evaluator itself correctly returned FALSE from the empty
     list. **RESOLVED in Round 4**: the note is conditional in both
     engines (names the list and its length when present).
