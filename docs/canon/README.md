# The canon — one place where the rules live

This folder is the single source of truth for the numbers and rules the
Superintendent website uses to judge your factory: what counts as a stall,
how the issues board orders findings, when a machine counts as unproductive,
how the earthworks planner judges an ore deposit, and what the bundled plans
promise at each phase.

Everything else renders **from** here:

- The site's **Why it suggests** pages (`/why`) render these files.
- Issue cards and issue detail pages quote the entry that fired.
- The agent skills that help you play point at entries by `id`.
- `tools/check_canon.py` fails the build if a coded constant disagrees with
  the number written here. Canon drift is a bug, not a style choice.

## Entry shape

```yaml
- id: board.stall-confirmation        # stable — skills and links depend on it
  title: "…"                            # player-facing heading
  player: "…"                           # plain-words explanation for players
  rule: "…"                             # the precise rule, for agents and auditors
  numbers:                              # named constants, each independently checked
    - { name: cover_max_months, value: 36, unit: months,
        code: { file: coi/src/main.rs, contains: "cover < 36.0" } }
  source: code | docs | plan | mixed   # where the number was verified
  status: verified | wired | heuristic | deliberate-none
```

`status` meanings: **verified** — read back from the running game or the
assemblies; **wired** — the number is in the engine and checked by
`check_canon.py`; **heuristic** — judgement, not measurement; the entry says
so and names its origin; **deliberate-none** — a considered decision *not*
to adopt a rule, recorded so it is not re-litigated by accident.

## The provenance rule

No canon entry — and no guide — may present a specific run's factory as the
reader's factory. A real run's numbers may appear only as a clearly framed
worked example ("for example, in one mature run…"), never as the default
expectation. This rule exists because an earlier storage guide taught a
war-chest target taken from one player's retired run as if it were general
advice.

## Vocabulary

The issues board's four quadrants — **Fix now**, **Fix later**, **Watch**,
**Ignore for now** — are *the* player vocabulary for severity in this
product. Other severity schemes (including the four-level scale the hosting
agent uses in its own strategy notes) are advisory only and are never shown
to players. New player-facing severity or verdict words must be registered
here before they ship.

## Files

| File | Covers |
|---|---|
| `board.yaml` | The issues board: quadrants, importance, stall/deficit/seasonal/storage rules |
| `forecasts.yaml` | The forecast rows: runways, fills, slope outlooks — all horizons stated together |
| `productivity.yaml` | Machine productivity verdicts and their evidence classes |
| `priorities.yaml` | The priority scale, manual groups, and the QoS actuator's rules |
| `earthworks.yaml` | Designation cells, spike filter, ore-purity gates, collapse bands |
| `plans.yaml` | The bundled plans' gates and the runner's hold rules |
| `philosophy.yaml` | Play philosophy: triage order, the death spiral, §12, research discipline, recorded non-adoptions |
