# Continuation plan over the history-derived state sequence

DRY RUN analysis — nothing was executed, no game was contacted.
Companion to `tools/history_to_states.py` and `history-states/README.md`
(adapter construction + month-alignment evidence live there; this doc
is the executor result).

- Plan: `plans/dragon-isle-continuation-v1.yaml` / `.json` (v1.1.0)
- Sequence: `history-states/` — 8 monthly states, 387-7-30 … 388-2-27,
  built from the snapshot's pinned-product history (36 products);
  only the final state carries bridge captures.
- Engine reports (full evidence strings):
  `plans/continuation-history-states.python.dryrun.md`,
  `plans/continuation-history-states.rust.dryrun.md`

## Engine parity

**PARITY PASS.** Both engines were run over the sequence
(`--state-seq history-states`). The extracted verdict skeletons are
identical on every category the parity gate compares: per-phase
statuses, combined + per-item gate verdicts, phase and global check
verdicts, invariants, next-order statuses, and the summary counts —
**TRUE 27 / FALSE 26 / UNEVALUABLE 20** (final state) — and the 8
per-prefix phase-history rows are byte-identical between the reports.
Remaining textual diffs are the known cosmetic classes only (plan-meta
maps rendered as Python dict repr vs JSON; `for:` duration dicts
likewise; one table separator row). Python is the reference; no
verdict divergence exists to reconcile.

## Derived phase history (per prefix)

Each row re-derives phase statuses from the history up to that month
only — the way a live executor's history grows. Statuses are identical
at every prefix; the loop never moves.

| prefix (state date) | phase 0 | phase 1 | phases 2–7 | loop position |
|---|---|---|---|---|
| 387-7-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 387-8-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 387-9-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 387-10-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 387-11-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 387-12-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 388-1-30 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |
| 388-2-27 | COMPLETE | HELD (U) | NOT REACHED | phase 1, held at entry |

Why phase 0 is COMPLETE at every prefix — its exit gate reads only
history-carried quantities, and both hold in all eight months:

| month | Steel quantity (gate ≥ 800) | Diesel quantity (gate ≥ 4,000) |
|---|---|---|
| 387-7 | 1,379 | 6,461 |
| 387-8 | 1,347 | 6,432 |
| 387-9 | 1,315 | 6,374 |
| 387-10 | 1,315 | 6,293 |
| 387-11 | 1,279 | 6,241 |
| 387-12 | 1,251 | 6,177 |
| 388-1 | 1,219 | 6,133 |
| 388-2 | 1,108 | 5,992 |

Caveat on that COMPLETE: phase 0's real deliverables are two
human-confirmed investigations (steel-stall trace, DieselReforming
decision). The executor derives COMPLETE purely from the exit gate
(§7/§9 re-derivation); a quantity-only history can prove the buffers
never bled out, it cannot prove the investigations happened.

## Final-state (388-2-27) gate detail

The last prefix is the only one whose latest state carries bridge
captures, so it is the only prefix where bridge-sourced leaves
evaluate at all. Per phase (full evidence in the engine reports):

- **Phase 0 — COMPLETE.** Exit TRUE (Steel 1,108 ≥ 800; Diesel
  5,992 ≥ 4,000). Checks: `p0-steel-still-falling` WOULD FIRE (thisMonth
  net −12: produced 8, consumed 20 — the wound monitor doing its job);
  `p0-diesel-floor` not firing; `p0-power-balance` UNEVALUABLE (below).
- **Phase 1 — HELD AT ENTRY (UNEVALUABLE).** Entry: CP percent_full
  TRUE ×2 (bridge rows 500/500 = 100%), Coal quantity TRUE (19,811),
  but ConcreteSlab `product.stored` UNEVALUABLE → combined
  UNEVALUABLE. Exit likewise UNEVALUABLE (three `for:` items blind —
  below — against Steel/Diesel TRUE). This is the phase the whole
  sequence cannot judge: see "The headline limitation".
- **Phase 2 — NOT REACHED** (gates still evaluated in the report):
  entry UNEVALUABLE (same ConcreteSlab stored leaf; transports ratio
  TRUE, 1,015/1,025 = 0.9902); exit **FALSE** — a genuinely judged
  gate: mixer/kiln counts short (4 < 5, 2 < 3) and the slab-balance
  `for: {months: 1}` composite FALSE inside the window (lastMonth
  activity 0/0 at 387-12-30 fails the Round-5 activity guard).
- **Phase 3 — NOT REACHED:** entry TRUE (TrainDepot ×1, hydrogen
  locos ×30, Diesel 5,992); exit FALSE (trains.line_count = 0) with
  the IronOre composite UNEVALUABLE inside it.
- **Phase 4 — NOT REACHED:** entry FALSE (trains 0; IronOre composite
  UNEVALUABLE; Steel TRUE); exit FALSE (casters 4 < 6; steel
  `for: {months: 1}` balance FALSE — broke at 387-12-30 with lastMonth
  net −20; Steel 1,108 < 1,500). Checks `p4-steel-buffer-falling-in-trial`
  and `p4-coal-buffer-trend` WOULD FIRE (−12 and −218 thisMonth).
- **Phase 5 — NOT REACHED:** entry FALSE (steel lastMonth net −46;
  months_of_food 26 TRUE); exit FALSE (FarmT2 8 < 20) with all three
  Fertilizer items UNEVALUABLE. `p5-soya-margin` WOULD FIRE (soybean
  quantity 28 < 50 — a history-carried product, genuinely read).
- **Phase 6 — NOT REACHED:** entry FALSE (steel net −46; food and
  homeless TRUE); exit FALSE (HousingT3 5 < 6; homeless TRUE) with
  HouseholdGoods/MedicalSupplies items UNEVALUABLE.
- **Phase 7 — NOT REACHED:** entry FALSE (steel net −46;
  HouseholdGoods UNEVALUABLE); exit UNEVALUABLE (placeholder
  research item + VehicleParts2 composite; homeless/food TRUE).
- **Globals at final state:** all four invariants TRUE (starving
  false, homeless 0, Diesel 5,992 > 0, Steel 1,108 > 0);
  `global-food-floor` not firing (26 months). At prefixes 1–7 the
  two settlement invariants are UNEVALUABLE (no bridge-status) and
  the two quantity invariants TRUE throughout — invariants are
  reported at the final state only; the per-prefix table above is
  phase statuses.

Net reading: the history sequence proves exactly one phase
transition-relevant fact — phase 0's buffer floors held for eight
straight months while steel and diesel both declined monotonically —
and it proves the run never legitimately entered phase 1 *on this
evidence*, because phase 1's entry cannot be fully read from product
history. Everything downstream is report-only evaluation, not a
phase history.

## UNEVALUABLE enumeration

Final-state count: 20 UNEVALUABLE top-level predicates (of 73).
Grouped by cause:

### A. Bridge-only metrics — blind at months 1–7, and inside every `for:` window that reaches them

No bridge capture exists before 388-2-27, so at prefixes 1–7 every
`settlement.*`, `research.*` (except none), `census.*`, `storage.*`
and `trains.*` leaf is UNEVALUABLE. At the final prefix the
instantaneous leaves evaluate from the genuine month-8 captures —
but `for:` durations still cannot be proven, because their windows
reach back into blind months (§4.3.1: an UNEVALUABLE snapshot inside
the window makes the hold unprovable, never TRUE):

- phase-1 exit: `census.transports_fully_connected_ratio`
  `for: {evaluations: 5}` — window = months 04–08; months 04–07 have
  no census.
- phase-1 exit: `storage.percent_full` (CP and CP2)
  `for: {months: 1}` — window includes 387-12-30, which has no
  bridge-storages.
- `p0-power-balance` check: `power.balance_kw` — UNEVALUABLE at
  *every* prefix including the last: no power capture exists in any
  state (the captures predate CoiBridge v0.6.2's power route; the
  engines have no power source at all).
- `research.completed` for sourced nodes (order verifies in phases
  5/6): UNEVALUABLE even at month 8 — the genuine bridge-research
  capture predates v0.6.2 and carries no `researched` list (format
  G1). (Not a gate leaf in this plan, so it appears in no count; the
  blindness is recorded here because the orders' verifies depend on
  it.)

### B. Non-pinned products — absent from the history payload, UNEVALUABLE at every prefix

The history endpoint was fetched for the 36 pinned products only.
Every `product.*` leaf naming another product reads "not in
stats.json products" in all eight states:

- `Product_IronOre` — phase-3 exit composite, phase-4 entry
  composite, check `p3-ore-still-negative` (blind).
- `Product_Fertilizer` — phase-5 exit: storage_capacity, stored,
  and the net-flow `for:` composite.
- `Product_HouseholdGoods` — phase-6 exit quantity + net-flow
  composite; phase-7 entry quantity.
- `Product_MedicalSupplies` — phase-6 exit quantity.
- `Product_VehicleParts2` — phase-7 exit composite; check
  `p7-vehicle-parts-strain` (blind).

### C. Underivable stats fields and flow slots

- `product.stored` / `product.storage_capacity` — the history
  payload carries neither field, for any product. Encoded as
  present-but-null, which both engines read UNEVALUABLE (tested).
  Instances that bite: `Product_ConcreteSlab` stored ≥ 2,900 in
  **phase-1 entry, phase-1 exit and phase-2 entry** — the single
  leaf that holds phase 1 (below). (Fertilizer's stored/capacity
  leaves are also underivable but are masked by group B: the
  product row itself is absent.)
  Note the asymmetry this creates at month 8: `storage.percent_full`
  *does* evaluate there (genuine bridge-storages rows), while
  `product.stored` for the same product does not — the genuine
  final-month stored values exist only in `live/stats.json`, which
  the adapter deliberately does not substitute into the sequence
  (uniform history-derived construction; see adapter README).
- Flow windows `thisYear` / `lastYear` / `lifetime` — not derivable
  from an 8-month window; the adapter's short produced/consumed
  arrays make both engines read them UNEVALUABLE (tested on both
  engines with a scratch plan before the encoding was chosen).
  **Instances in this plan: none** — every flow predicate in the
  continuation plan reads `thisMonth` (5) or `lastMonth` (30), so
  this blind spot is real but untriggered here. Month 1's
  `lastMonth` is likewise absent (the window starts at 387-7-1);
  it is read only inside `for:` windows, where it contributes
  blindness, never a verdict.

### D. Placeholder research (not missing data — manual-only by design)

- Phase-7 exit `research.completed` on the §4.5 placeholder
  `Research<MegaVehicles>`: UNEVALUABLE because the node id is
  unsourced, per format §4.5 — never auto-evaluated, on any data.
  Listed separately so it is not mistaken for a capture gap.

## The headline limitation — phase 1 cannot be judged from product history alone

Phase 1's entry gate is the war-chest audit: CP percent-full ×2,
coal quantity, and **ConcreteSlab stored ≥ 2,900**. The first three
evaluate (TRUE at month 8; the percent-full pair only there). The
fourth never can: `stored` is a storage-accounting field that the
history endpoint does not carry, in any month, including the last.
So on product history alone the executor's honest position for the
entire eight months is *phase 0 complete, phase 1 held at an
unreadable entry gate* — even though the genuine live captures
outside this sequence show the slab stockpile at 7,205 stored. That
verdict is a property of the evidence source, not of the game state,
and it is exactly the §4.3.1 warning made concrete: the vanilla
history series carries per-product flows, not full per-snapshot
state. A sequence that could judge phase 1 needs monthly full
captures (the companion's snapshot habit), not a wider history
window.

## What the sequence does prove

- Phase 0's exit floors held at all eight month-ends (table above) —
  the steel/diesel bleed was slow (Steel −271, Diesel −469 over the
  window), never a breach.
- Steel's flow story is fully readable from history and matches the
  plan's premise: produced [0, 0, 32, 0, 0, 0, 0, 8] against consumed
  [28, 36, 32, 36, 20, 36, 46, 20] — ~zero production for eight
  months, not six as the plan's hist-keys-based assumption said
  (the snapshot window is one month wider).
- Several late-phase gates that depend only on history products +
  month-8 census evaluate to honest FALSE (phase-2 exit, phase-4
  steel balance, FarmT2/HousingT3 counts) — the sequence refutes
  progress claims; it does not merely abstain.
