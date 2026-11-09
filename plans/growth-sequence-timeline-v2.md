# Growth-sequence timeline v2 — fresh-start (plan v1.2.0) with the history-aware executor

Generated 2026-11-03 (Round 6) by running BOTH engines in §4.3.1
sequence mode over the same 12 monthly fixtures
(`plans/fixtures/growth/month-01/` … `month-12/`):

- Python: `tools/plan_check.py plans/fresh-start-v1.yaml --state-seq plans/fixtures/growth`
  → `plans/fresh-start-v1.growth-sequence.dryrun.md`
- Rust: `coi plan-check plans/fresh-start-v1.json --state-seq plans/fixtures/growth`
  → `plans/fresh-start-v1.growth-sequence.rust.dryrun.md`

**Parity: PASS.** The two engines produce identical per-prefix phase
tables, identical entry/exit verdicts for all 8 phases, identical
summary counts (TRUE 36 / FALSE 26 / UNEVALUABLE 3) and the same
active phase. Single-snapshot behaviour is unchanged (regression:
Round-5 single-state reports reproduce byte-for-byte on live + the
month-1 fixture).

This document supersedes the *status narrative* of
`growth-sequence-timeline.md` (v1). **The v1 file is kept intact** —
its Finding 1 (single-snapshot `for:` cap wedging phases ≥ 2) is the
problem this round fixed, and its month-by-month raw observations
(flows, stocks, research) remain the fixture record. What changes in
v2 is only what the history-aware executor can now prove.

## The new flip sequence (per-prefix derivation, §4.3.1)

| Prefix (date) | Phase statuses after this snapshot |
|---|---|
| m1 · 1-1-18 | 0 COMPLETE · 1 ACTIVE |
| m2 · 1-2-15 | 0 COMPLETE · 1 ACTIVE |
| m3 · 1-3-15 | 0, 1 COMPLETE · 2 HELD AT ENTRY (UNEVALUABLE) |
| m4 · 1-4-15 | 0, 1 COMPLETE · 2 HELD AT ENTRY (UNEVALUABLE) |
| m5 · 1-5-15 | 0, 1, **2 COMPLETE** · 3 ACTIVE |
| m6 · 1-6-15 | … · 3 ACTIVE |
| m7 · 1-7-15 | … · 3 ACTIVE |
| m8 · 1-8-15 | 0–2 COMPLETE · **3 COMPLETE** · 4 ACTIVE |
| m9–m12 | unchanged: 4 ACTIVE, 5–7 NOT REACHED |

**Final verdict at month 12: loop position = phase 4
(`phase-4-concrete-diesel-rubber`), ACTIVE.** Phases 0–3 COMPLETE.

### Why phase 2 completes at month 5, exactly

Its exit's binding leaf is the ConstructionParts lastMonth balance
guarded composite under `for: {months: 1}`. Inner values: FALSE at m3
(net −6), TRUE from m4 (net +5, then +4/mo). At prefix m4 the trailing
1-month window reaches back to m3's FALSE → the duration had not held
(exit not yet TRUE). At prefix m5 the window is {anchor m4, m5}, both
TRUE, span 1.00 months → duration TRUE → exit TRUE → phase COMPLETE
(§7: exit governs completing; its entry gate remains UNEVALUABLE by
design — see blockers). In v1 this exit read UNEVALUABLE at every
month from a single snapshot.

Phase 5's `settlement.months_of_food >= 3 for:{months:1}` leaf — the
second duration cap named in v1 Finding 1 — likewise resolves TRUE in
sequence mode (anchor 1-11-15, span 1.00 months).

## Phases 3–7 on their own gates (final prefix, month 12)

Un-wedged means their gates evaluate with full history; statuses
still derive in order, so 5–7 read NOT REACHED only because phase 4
is legitimately incomplete — not because evaluation is capped.

| Phase | Entry | Exit | Blocking leaf (evidence at month 12) |
|---|---|---|---|
| 3 mining & vehicles | TRUE | **TRUE** (from m8) | — COMPLETE |
| 4 concrete/diesel/rubber | TRUE | FALSE | `product.quantity Product_Diesel >= 300` — actual **220**. Every other exit leaf TRUE (concrete 226 ≥ 100; diesel lastMonth net +82 on real activity; rubber 17 > 0). Same single leaf as v1 Finding 3: at the m12 net rate the 300 target lands ~month 13–14 — a fixture-horizon limit, not an executor limit |
| 5 copper/CP2/food | TRUE (from m11) | FALSE | Copper balance leaf FALSE: lastMonth produced 0 − consumed 6 = **net −6** (copper district started m12; needs its first full month). monthsOfFood duration leaf TRUE (above); starving FALSE as required |
| 6 automation | TRUE (from m12) | FALSE | `research.completed` FALSE for **ResearchConveyorBelts**, **ResearchShipDockRepair**, **ResearchFuelStation** (none in `researched`); plus the ConstructionParts2 balance leaf UNEVALUABLE — `Product_ConstructionParts2` absent from the fixture stats entirely (zero rows matched, honestly blind rather than false) |
| 7 rail | FALSE | FALSE | Entry: ConstructionParts2 flow absent (same absence as phase 6's leaf). Exit: `research.completed ResearchTrains` FALSE and `trains.line_count >= 1` — actual **0** lines |

## What is still blocked, and on what

- **Phase 2's entry gate** stays UNEVALUABLE in every prefix: its
  first leaf is `research.completed` on the §4.5 placeholder
  `Research<Construction>`. Plan v1.2.0 documents the identification
  attempt and the sole candidate (`ResearchCp1Production`) with
  reasoning, and deliberately does not substitute it (no project
  source maps research ids to display names). This no longer wedges
  anything: completion derives from the exit gate, and a live run
  resolves the entry by the human-confirm path the placeholder
  declares.
- **Phase 4's exit** needs ~2 more months of fixture (diesel stock
  220 → 300 at the scripted net rate), and phase 5's exit one full
  month of copper production — both are properties of the 12-month
  fixture horizon (v1 Finding 3), now the *only* things standing
  between this sequence and phases 5–7 flipping.
- Everything else from v1 stands: diesel is the first year's binding
  constraint (crisis checks fire through the middle months exactly
  as the plan predicts), and no gate anywhere reads TRUE on absent
  evidence.
