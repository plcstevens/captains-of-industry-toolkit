# Growth-sequence timeline — fresh-start-v1 (plan v1.1.0) over 12 monthly fixtures

Generated 2026-11-03 by `tools/gen_growth_fixtures.py` (Round 5, chunk 4).
Fixtures: `plans/fixtures/growth/month-01/` … `month-12/`, in the exact
early-game fixture shape (same 9 state files/keys; percentFullRaw and
progressPerc = percent × 1000; stats windows [thisMonth, lastMonth,
thisYear, lastYear, lifetime] with lifetime = running sums, quantity =
starting stock + lifetime produced − lifetime consumed, stored ≤
storageCapacity, storage rows ≤ stats stored). Month 1 is the
early-game fixture's world, relabelled to the growth run. Snapshots
from month 2 are mid-month (day 15): thisMonth ≈ half the month's full
flow, lastMonth = the prior full month.

Engine reports (Rust `coi plan-check`, JSON plan):
`plans/fixtures/growth/reports/month-NN.rust.md`; Python spot-checks
for months 1, 6, 12 alongside (`month-NN.python.md`).

## Month-by-month

| Month | Date | Engine loop position | Phases completed this month | Pop | Iron | CP net (lastMonth) | Diesel stock | Research done |
|---|---|---|---|---|---|---|---|---|
| 1 | 1-1-18 | phase-1 ACTIVE | phase-0 | 100 | 42 | 0 (no lastMonth yet) | 220 | 0 |
| 2 | 1-2-15 | phase-1 ACTIVE | — | 100 | 72 | −40 (starting-stock drawdown) | 210 | 1 (Construction) |
| 3 | 1-3-15 | phase-2 HELD AT ENTRY (UNEVALUABLE) | **phase-1** (iron 123 ≥ 100) | 100 | 123 | −6 | 190 | 1 |
| 4 | 1-4-15 | phase-2 HELD | — | 100 | 158 | **+5** (CP production live, non-negative on real activity) | 170 | 2 (+Power & Maintenance; maintenance depot built) |
| 5 | 1-5-15 | phase-2 HELD | — | 100 | 187 | +4 | 150 | 3 (+Trading Dock; coal mine opens) |
| 6 | 1-6-15 | phase-2 HELD | — | 100 | 217 | +4 | 130 | 4 (+Vehicles & Mining) |
| 7 | 1-7-15 | phase-2 HELD | — | 116 | 255 | +4 | 109 | 5 (+Beacon; first iron ore, ore smelting) |
| 8 | 1-8-15 | phase-2 HELD | — (phase-3 **exit gate** first TRUE — see Finding 1) | 116 | 309 | +4 | 87 | 6 (+Tools) |
| 9 | 1-9-15 | phase-2 HELD | — | 116 | 364 | +4 | 65 (runway low point) | 6 (Basic Diesel in progress; kiln + concrete mixer built, concrete production starts) |
| 10 | 1-10-15 | phase-2 HELD | — | 132 | 418 | +4 | 69 | 7 (+Basic Diesel; diesel chain online, first produced diesel) |
| 11 | 1-11-15 | phase-2 HELD | — (phase-5 **entry gate** first TRUE) | 132 | 475 | +4 | 124 | 8 (+Synthetic Rubber; first rubber) |
| 12 | 1-12-15 | phase-2 HELD | — (phase-6 **entry gate** first TRUE) | 132 | 534 | +4 | 220 | 9 (+Copper Refinement; copper district starts) |

Gate-level progression (entry/exit, from the Rust reports; T/F/U):

| Phase | Entry first TRUE | Exit first TRUE | State at month 12 |
|---|---|---|---|
| 0 scout | — (empty) | month 1 | COMPLETE |
| 1 scrap starter | month 1 | month 3 | COMPLETE |
| 2 assembly/power/maint. | never (UNEVALUABLE — Finding 2) | never (UNEVALUABLE — Finding 1; every leaf TRUE from month 4) | HELD AT ENTRY |
| 3 mining & vehicles | month 4 | **month 8** | exit TRUE months 8–12 (phase status NOT REACHED — Finding 1) |
| 4 concrete/diesel/rubber | month 6 | never | exit FALSE at 12 on exactly one leaf: diesel stock 220 < 300 (Finding 3) |
| 5 copper/CP2/food | month 11 | never | exit FALSE: copper lastMonth net −6 (district just started) |
| 6 automation | month 12 | never | exit FALSE (belts/CP2 not researched) |
| 7 rail | never | never | entry FALSE (CP2 flow absent) |

## Verification results

- **(i) Strict phase order — PASS at the status level.** Phase 0
  COMPLETE from month 1; phase 1 ACTIVE months 1–2, COMPLETE month 3;
  phase 2 is the loop position from month 3 on; phases 3–7 read NOT
  REACHED in every month. No phase ever reads ACTIVE/COMPLETE while a
  predecessor has not. (Gate-level exception is Finding 1, an engine/
  format interaction, not a fixture ordering violation.)
- **(ii) No COMPLETE regressions — PASS.** Phases 0 and 1, once
  COMPLETE, stay COMPLETE in all later months.
- **(iii) No TRUE on UNEVALUABLE/absent evidence — PASS.** Absent
  products stay UNEVALUABLE: phase-3 exit reads UNEVALUABLE in month 6
  precisely because Product_IronOre does not exist yet (it first
  appears month 7, where the exit reads FALSE on the guard's 0−0
  lastMonth, then TRUE in month 8 on real flow 120−100). No check
  fires on an absent product in any month; ConstructionParts2
  predicates never read TRUE (product absent all 12 months).
- **(iv) Firing checks explainable — PASS.** The only firings are
  diesel-floor checks during the starting-stock drawdown (p6 floor 200:
  months 3–11; p3 floor 150: months 6–11; p1/p2 floor 100: months
  8–10; stock path 220 → 65 at month 9 → 220 at month 12) and
  `p6-transport-connectivity` in month 9 (8/9 transports connected =
  0.889 < 0.95 — one kiln belt left dangling the month the concrete
  district is built). Every firing traces to a fixture number above.
  The global diesel invariant (> 0) and food floor never fire;
  monthsOfFood stays 3–4 throughout.
- **Engine parity — PASS.** Python `tools/plan_check.py` on months 1,
  6, 12 produces phase statuses and per-phase entry/exit verdicts
  identical to the Rust engine.

## Findings

1. **Phases 2+ can never flip in a single-snapshot dry run, however
   good the state gets.** Two independent caps: (a) phase 2's exit
   contains the ConstructionParts balance wrapped in `for:
   {months: 1}` — both engines (§4.3) read an instantaneously-TRUE
   duration predicate as UNEVALUABLE from one snapshot, so phase 2's
   exit is UNEVALUABLE from month 4 onward even though every leaf is
   TRUE (evidence string in month-12 report, verbatim). (b) Phase
   status is derived strictly in order, so phases 3–7 stay NOT
   REACHED even when their own gates pass (phase-3 exit TRUE months
   8–12). A 12-month snapshot sequence is exactly the evaluation
   history a `for:` duration needs — but neither engine consumes
   cross-snapshot history. This is the growth test's main result:
   demonstrating phases ≥2 completing requires an executor with run
   history (or a multi-snapshot mode in plan-check), not better
   fixtures. Phase 5's exit has the same `for:` cap on its
   monthsOfFood leaf, so it is capped the same way whenever reached.
2. **Phase-2 entry is UNEVALUABLE in all 12 months by plan design:**
   its first leaf is `research.completed` on the §4.5 placeholder
   `Research<Construction>` (manual-only, never auto-evaluated). The
   fixtures record Construction completing as ResearchCp1Production
   (the early-game fixture's node, the plan's named candidate), but
   the gate never reads that id. Until a human sources the id and
   bumps the plan, phase 2 can only ever read HELD AT ENTRY in a dry
   run — even in months where its exit leaves are all TRUE.
3. **Phase 4 realistically needs ~14 months, not 12.** At month 12 its
   exit fails on exactly one leaf — diesel stock 220 vs the plan-set
   300 target — with concrete (226 ≥ 100), diesel lastMonth net (+82,
   real activity) and rubber (17 > 0) all TRUE. At the month-12 net
   rate (+110/month and rising as the chain scales) the stock target
   lands around month 13–14. Likewise phase 5's exit needs the copper
   district to have a full month of non-negative copper flow behind
   it (earliest ~month 14–15), and phases 6–7 sit behind Construction
   II / belt research that the plan's own research order places after
   month 12. Phases 0–3 flipping (0–1 as COMPLETE, 3 at gate level) is
   what 12 honest months buys.
4. **Fixture-choice disclosures** (also in the generator docstring):
   the Concrete Production node's game id is unsourced, so no concrete
   node ever appears in `researched` — kiln/mixer builds appear month
   9 in-fiction and no gate reads that node; Synthetic Rubber is
   recorded as ResearchRubber, one of the plan's two candidate ids and
   a real id in `coi/data/research_ids.txt`; census proto labels
   beyond the early-game fixture are plausible fixture labels in that
   fixture README's sense (only MaintenanceDepotT1 is gate-read, and
   the plan flags it proto-id-unconfirmed itself).
5. **Diesel is the binding constraint of the whole first year**, as
   the plan's checks predict: the starting stock (300, 220 at month 1)
   burns at 20–22/month against a research order that puts Basic
   Diesel 8th, bottoming at 65 in month 9 — above the >0 invariant
   but below every phase floor, so the crisis-protocol checks fire
   for most of the middle sequence. A real run following this plan
   would spend months 6–10 one bad month from a fuel crisis; the
   plan's stall fallback (2 months) would also have fired on phase 2
   long before, per Finding 1's caps.

No fixture bugs were found during verification — no generator fix or
regeneration was needed after the first run (the generator's own
self-checks on quantity/stored/capacity/flow accumulation passed on
all 12 months before any engine ran).
