# CoI Game-Plan Format v1 — a machine-executable plan for agent play

**Status:** Format v1, authored 2026-11-02 (north-star step B, SPEC Addendum V).
**First instance:** `plans/fresh-start-v1.yaml` (New Haven, normal, Update 4).
**Companion spec:** `SPEC.md` — §0.1 (vanilla API), §3 (bridge routes), §12/§12.7
(write channel + two-phase safety), Addenda V–VII (north star, catalogue, gaps).

Normative language: **MUST / MUST NOT** are executor requirements; **SHOULD** is
a strong default a plan may override explicitly; **MAY** is permitted.

A game plan is a YAML document an executor (the supervised play loop, §9)
runs against a live game through the companion: it **observes** state,
**evaluates** gates and checks, **applies** due orders within the plan's
autonomy posture, **verifies** post-state, and writes everything to the
audit log. A plan is data, not code: it contains no expressions, no
scripts, and no command names outside the captured catalogue (§6.1).

---

## 1. Scope and non-goals

- A plan describes **one run**: one map, one difficulty, one game version,
  from a defined start state (usually a fresh save) to a defined end state
  (its last phase's exit gate).
- A plan does **not** contain geometry. Tile-level placement lives in
  blueprints and manual steps (§6.3); the plan references them by name.
  This is deliberate: the vanilla channel's placement primitive
  (`CreateStaticEntityCmd`) exists but its semantics are **untested**
  (Addendum VI), so no plan may assume coordinates work.
- A plan does **not** override the companion's safety layer. The §12
  two-phase flow (prepare → confirm → apply, 60 s token, audit) is the
  only write path, whatever the plan's autonomy settings say. Autonomy
  (§8) decides *who/what confirms*, never *whether* the flow runs.
- Out of scope: save editing, cheat commands in a non-sandbox run,
  `GameConsoleCmd` (excluded by policy, §5.2), demolitions by default (§5.2).

---

## 2. State model — what gates and checks may read

Every predicate in a plan MUST be evaluable from exactly these sources.
Anything else belongs in §11 (known gaps), never in a gate.

| Source | Route | Fields a plan may reference |
|---|---|---|
| Vanilla game-info | `GET /api/v1/game-info` | `date` (in-game `Y-M-D`, e.g. `388-2-27`), `tick`, `isPaused`, `speed`, `gameId`, `sessionId` |
| Vanilla product stats | `GET /api/v1/products/stats` | per product id: `quantity`, `stored`, `storageCapacity`, and `produced` / `consumed` flow arrays indexed by window: `[0]=thisMonth, [1]=lastMonth, [2]=thisYear, [3]=lastYear, [4]=lifetime` |
| Vanilla product history | `GET /api/v1/products/history` | per-product monthly series (trend predicates, §4.3) |
| Vanilla product catalogue | `GET /api/v1/products` | `id`, `name`, `kind`, `isUnlocked`; `pinned=true` query gives the pinned set (pin-order verification, §6.1) |
| Bridge status | `GET /coi/v1/status` | `population`, `totalHousingCapacity`, `freeHousingCapacity`, `numberOfHomeless`, `monthsOfFood`, `arePeopleStarving`, `settlements` |
| Bridge research | `GET /coi/v1/research` | `current` (`id`, `progressPerc`, `stepsDone`, `remainingSteps`, `scienceCost`), `queue` (ordered node ids), `researchedCount`, `hasActiveLab`. **Percent-raw convention applies:** `progressPerc` is percent × 1000 (48438 ≈ 48.4%). **There is no completed-node list** — see gap G1 |
| Bridge census | `GET /coi/v1/census` | `totalEntities`, `byProto` (proto → count), `machines` (proto → `states` map, e.g. `Working` / `NotEnoughInput` / `OutputFull` / `Paused`, plus `recipes`), `transports` (`total`, `fullyConnected`) |
| Bridge storages | `GET /coi/v1/storages` | rows: `id`, `proto`, `product`, `quantity`, `capacity`, `percentFullRaw`, `logisticsInDisabled`, `logisticsOutDisabled`. **Percent-raw:** real percent = `percentFullRaw / 1000` (99800 = 99.8%) |
| Bridge trains | `GET /coi/v1/trains`, `/coi/v1/trains-detail` | line presence/counts; detail adds line ids, networks, schedules (v0.6) |
| Bridge trade state (v0.6) | `GET /coi/v1/contracts`, `/coi/v1/loans`, `/coi/v1/edicts` | active contracts / offers, loan terms + active loans, edict enabled state. Field-level shapes follow the v0.6 payloads; a predicate naming a field the payload lacks is UNEVALUABLE (§4.4), never false-by-guessing |
| Companion snapshot store | `coi.db` snapshots | the executor's own observation history: prior evaluations, observed research completions (G1 workaround), order outcomes. Plans may rely on it only through the predicates in §4 |

Conventions:

- **Product ids** are the game's `Product_`-prefixed ids (`Product_Diesel`,
  `Product_ConstructionParts`, `Product_ConcreteSlab`). A plan MUST NOT use
  display names ("Diesel") as ids.
- **Research node ids** are the game's `Research…` proto ids as extracted in
  `coi/data/research_ids.txt` (e.g. `ResearchBeacon`, `ResearchBasicDiesel`).
  Display names from guides ("Concrete Production") are **not** ids. Where
  a display name has no sourced id, the plan MUST use the placeholder
  convention of §4.5 and flag it — never invent an id that parses.
- **Entity ids** are runtime ints and may not survive a save/load cycle
  (SPEC §12.3). Orders MUST address entities by selector (§6.2), not by
  literal id, except inside a single session's prepared plan.
- **Freshness:** no order may apply against a snapshot older than 10 s
  (SPEC §12.6). Gate/check evaluations stamp the snapshot tick they used.

---

## 3. Document anatomy

Top level of every plan document (all keys required unless marked optional):

```yaml
plan:            # metadata, §3.1
  id: fresh-start
  version: 1.0.0
  title: …
  map: {name: "New Haven", difficulty: normal}
  game_version: "0.8.8"          # Update 4.3; executor refuses other versions
  dlc: [trains-expansion]        # optional; phases may require entries here
  author: …
  created: 2026-11-02
  requires:                      # minimum substrate versions
    companion: ">=0.1 (website, generic command flow)"
    bridge: ">=0.5.0"            # >=0.6.0 if any phase reads contracts/loans/edicts
  assumptions: [ … ]             # sourced or flagged; see §3.2
  end_state: "…"                 # what "plan complete" means, in words

globals:         # §5 — autonomy defaults + safety invariants
  autonomy: { <action-class>: <level>, … }
  invariants: [ <predicate>, … ] # MUST hold in every phase
  banned_commands: [GameConsoleCmd]
  crisis_protocol: …             # §7.3

phases: [ <phase>, … ]           # §7, executed in list order
```

### 3.1 Metadata rules

- `plan.id` + `plan.version` (semver) identify the plan in every audit
  entry. Any edit to gates, orders or thresholds is a version bump:
  patch = wording/notes, minor = thresholds/order additions,
  major = phase structure or gate changes.
- The executor MUST refuse to start a plan whose `map`, `difficulty` or
  `game_version` does not match the live game (map/difficulty are checked
  at start against the save/`coi saveinfo`; `gameId` from game-info is
  recorded and a change of `gameId` mid-run halts the loop, §9).
- `requires.bridge` is a hard precondition: if a phase reads a v0.6 route
  and the bridge is v0.5, that phase's v0.6 predicates are UNEVALUABLE.

### 3.2 Assumptions

Every assumption is a list entry with a `claim` and a `source`:
`sourced` (with where), `single-source`, or `hypothesis`. Hypotheses may
inform **manual steps** (where to look first) but MUST NOT appear inside
gate predicates — gates read observed state only. This is the starter
skill's rule ("hypotheses stay labelled") made structural.

---

## 4. Gate predicates — the closed vocabulary

A predicate is a YAML mapping. The vocabulary below is **closed**: an
executor MUST reject a plan containing any other predicate form, and MUST
NOT silently coerce an unevaluable predicate to true or false (§4.4).

### 4.1 Leaf predicates

**(a) Metric comparison** — the workhorse.

```yaml
{metric: product.quantity, product: Product_Diesel, op: ">=", value: 500}
```

Fields: `metric` (one row of the table), the metric's selector fields,
`op` ∈ `==, !=, <, <=, >, >=`, `value` (number, bool, or string per row).
For boolean-valued metrics (`research.completed`, `product.unlocked`,
`settlement.starving`, …), `op`/`value` may be omitted, meaning
`op: "==", value: true`.

| `metric` | Selector fields | Source / derivation |
|---|---|---|
| `product.quantity` | `product` | stats `quantity` (total in world incl. machines) |
| `product.stored` | `product` | stats `stored` (in storages only) |
| `product.storage_capacity` | `product` | stats `storageCapacity` |
| `product.produced` / `product.consumed` | `product`, `window` ∈ `thisMonth,lastMonth,thisYear,lastYear,lifetime` | stats flow array at the window index |
| `product.net_flow` | `product`, `window` | derived: `produced[window] − consumed[window]` |
| `product.unlocked` | `product` | catalogue `isUnlocked` (bool; `value: true/false`) |
| `product.pinned` | `product` | catalogue `pinned=true` view: membership of the pinned set (bool) |
| `game.paused` / `game.speed` | — | game-info `isPaused` / `speed` |
| `settlement.population` | — | status `population` |
| `settlement.housing_capacity` | — | status `totalHousingCapacity` |
| `settlement.free_housing` | — | status `freeHousingCapacity` |
| `settlement.homeless` | — | status `numberOfHomeless` |
| `settlement.months_of_food` | — | status `monthsOfFood` |
| `settlement.starving` | — | status `arePeopleStarving` (bool) |
| `research.current` | — | research `current.id`; compare with `==`/`!=` against a node id, or `value: none` when no node is active |
| `research.current_progress_pct` | — | derived: `current.progressPerc / 1000` (0–100) |
| `research.queue_contains` | `node` | research `queue` membership (bool) |
| `research.queue_length` | — | `len(queue)` |
| `research.researched_count` | — | research `researchedCount` |
| `research.has_active_lab` | — | research `hasActiveLab` (bool) |
| `research.completed` | `node` | **see §4.5/G1** — executor-observed completions only |
| `census.proto_count` | `proto` | census `byProto[proto]` (absent proto = 0) |
| `census.machine_state_count` | `proto`, `state` | census `machines[proto].states[state]` (absent = 0). `proto: "*"` aggregates all machine protos |
| `census.machine_state_share` | `proto`, `state` | derived: state count ÷ total state count for the proto(s), 0–1 |
| `census.transports_fully_connected_ratio` | — | derived: `transports.fullyConnected / transports.total` (total 0 ⇒ UNEVALUABLE) |
| `storage.percent_full` | one of `storage_id`, or `product`, or `proto` | storages rows; with `product`/`proto` the executor aggregates **all** matching rows (quantity-weighted). Derived percent = `percentFullRaw / 1000` |
| `storage.logistics_in_disabled` / `storage.logistics_out_disabled` | `storage_id` | storages row flags (bool) |
| `trains.line_count` | — | trains route: number of lines |
| `contracts.active_count` / `loans.active_count` | — | v0.6 routes (count of active entries) |
| `edicts.enabled` | `edict` | v0.6 edicts route (bool) — read-side only; there is no catalogue write command for edicts (G4) |

**(b) Time predicates.**

```yaml
{metric: time.months_elapsed, op: ">=", value: 3}        # since plan start
{metric: time.in_phase_months, op: ">=", value: 2}       # since phase entry
{metric: game.date, op: ">=", value: "1-6-1"}            # in-game Y-M-D
```

Months are in-game months from game-info `date` deltas, not wall clock.
`time.months_elapsed` anchors at the executor's first observation of the
run (recorded in the audit log with the starting `date` and `tick`).

### 4.2 Composite predicates

```yaml
all: [ <predicate>, … ]     # AND — every child TRUE
any: [ <predicate>, … ]     # OR  — at least one child TRUE
not: <predicate>            # negation — only of a TRUE/FALSE child (§4.4)
```

Gate fields (`entry_gate`, `exit_gate`, order `preconditions`/`verify`)
take a **list** of predicates, read as an implicit `all:`.

### 4.3 Duration wrapper

Any leaf or composite may carry `for: {months: 1}` or
`for: {evaluations: 5}`: the predicate is TRUE only after holding TRUE on
every evaluation spanning that duration. Use it for "buffer rising /
stable" gates (the relocation plan's checkpoint style) instead of
single-reading gates wherever a stall could fake a pass.

Trend form (history-based): `{metric: product.net_flow, product: …,
window: lastMonth, op: ">", value: 0, for: {months: 1}}` is the canonical
"sustained positive balance" gate.

Duration semantics over a single snapshot vs a state sequence are
normative in §4.3.1 (added 2026-11-03, Round 6).

### 4.3.1 Duration evaluation over a state sequence (normative)

A `for:` predicate is a claim about an **interval of game time**, so it
can only be fully evaluated against a **state sequence**: snapshots of
the §2 sources for **one** `gameId`, ordered by in-game `date`. The
executor's own snapshot history (live play, §9 step 1) is such a
sequence; so is a directory of per-snapshot capture directories in the
§2 shape (e.g. the growth fixtures). A single state is the degenerate
sequence of length 1. The vanilla product-**history** series does NOT
qualify as a sequence source: it carries per-product flows, not full
per-snapshot state, so leaves other than product flows could not be
evaluated at the earlier points.

**Calendar.** In-game dates convert to an absolute day index
`A(Y-M-D) = ((Y−1)×12 + (M−1))×30 + (D−1)` — the CoI calendar is 12
months of 30 days (verified against captured ticks: 2,880 ticks/day,
monthly captures exactly 30 days apart). The span between two dates is
`(A(b) − A(a)) / 30` months. Only dates ever matter for duration —
never wall clock, never snapshot counts.

**`for: {months: m}`** evaluated at the latest snapshot Sₙ (date dₙ):

- Let the boundary be `B = A(dₙ) − 30m` (in day-index units).
- The **window** W is every snapshot with `A(dᵢ) > B`, **plus the
  anchor**: the latest snapshot with `A(dᵢ) ≤ B`, if one exists. The
  anchor proves the predicate's value at the start of the window.
- Verdict:
  - **FALSE** if the inner predicate is FALSE at any snapshot in W
    (the hold demonstrably broke inside the window);
  - else **UNEVALUABLE** if no anchor exists (history begins strictly
    inside the window — the start of the window is unobserved;
    insufficient history is never FALSE-by-default and never
    TRUE-by-default), or the inner predicate is UNEVALUABLE at any
    snapshot in W;
  - else **TRUE** (anchor exists and the inner predicate is TRUE at
    every snapshot in W).
- **Gaps:** unobserved intervals between snapshots do not falsify and
  do not prove anything — the predicate is asserted only at
  observations, so the snapshot cadence is the resolution of the
  proof. A dip that begins and ends between two snapshots is
  invisible to this rule; plans that care MUST poll tighter (§9
  cadence), not loosen the rule.
- **Irregular spacing** is handled by construction: spacing never
  enters the rule, only dates do.
- At the latest snapshot itself, an instantaneously-FALSE inner
  predicate yields FALSE (it is in W), matching the single-snapshot
  reading.

**`for: {evaluations: n}`** evaluated over the last n snapshots of the
sequence (including the latest): **FALSE** if the inner predicate is
FALSE at any of them; else **UNEVALUABLE** if the sequence holds fewer
than n snapshots or the inner predicate is UNEVALUABLE at any of them;
else **TRUE**. (`n = 1` degenerates to the instantaneous value. No
current plan uses `n = 1`.)

**Single-snapshot mode is the degenerate case** and reproduces the
long-standing readings exactly: a `months` duration with an
instantaneously-TRUE inner predicate is UNEVALUABLE (no anchor — the
preceding window is unobserved), an instantaneously-FALSE one is
FALSE, and an `evaluations` duration with n > 1 is UNEVALUABLE.

**Phase derivation in sequence mode** (§7/§9 unchanged): statuses are
derived per **prefix** of the sequence — the report at prefix k uses
history S₁..Sₖ, exactly as a live executor's history grows — and the
final prefix is the sequence's verdict. A phase is COMPLETE when its
exit gate is TRUE under that prefix's history (§7: entry governs
entering, exit governs completing).

### 4.4 Three-valued evaluation

Every predicate evaluates to **TRUE**, **FALSE**, or **UNEVALUABLE**
(source route down, field absent, selector matched zero or ambiguous
rows, snapshot stale). Rules:

- A gate is satisfied only when **all** its predicates are TRUE.
- UNEVALUABLE is never TRUE and never FALSE: the executor holds position,
  logs the gap, and the phase's `on_unevaluable` fallback applies
  (default: pause-and-report, §7.2). `not:` of UNEVALUABLE is UNEVALUABLE.
- A check (§7.1) whose predicate is UNEVALUABLE reports "check blind",
  which is itself reportable — a blind safety check is not a pass.

### 4.5 Placeholder-with-warning convention (unsourced ids)

When a needed id cannot be sourced (research node whose game id is not in
`coi/data/research_ids.txt`, an unconfirmed crop/proto id), the plan MUST
write it as a mapping, never as a bare string that looks real:

```yaml
node: {placeholder: "Research<Construction>", display_name: "Construction",
       status: unsourced-id, confirm: "in-game research screen before first use"}
```

Any order or predicate touching a placeholder is **manual-only** (§8)
until a human replaces the placeholder with the sourced id and bumps the
plan version. Executors MUST refuse to auto-apply such orders. This is
how single-source guide knowledge enters a plan without being laundered
into fact; elements carrying source uncertainty additionally carry
`flags: [single-source]` (or the relevant flag) copied from their source.

---

## 5. Globals

### 5.1 Autonomy defaults

`globals.autonomy` maps **action class → level** (§8). A phase may
**tighten** a class (lower its level) but MUST NOT loosen it beyond the
global default; loosening is a plan-level decision a human makes by
editing `globals`, not something a phase smuggles in.

### 5.2 Safety invariants

`globals.invariants` is a list of predicates evaluated on **every** loop
iteration, in every phase, before any order applies. On any invariant
going FALSE the executor MUST: pause the game
(`SetSimPauseStateCmd {isPaused: true}`), stop applying orders, and
report — regardless of autonomy. Canonical invariants:

- Diesel never hits zero while anything consumes it:
  `{metric: product.quantity, product: Product_Diesel, op: ">", value: 0}`
  (plans usually add a stronger floor as a *check*, §7.1).
- The settlement is never starving:
  `{metric: settlement.starving, op: "==", value: false}`.
- **Structural invariants** (not predicates, executor-enforced):
  `GameConsoleCmd` is banned everywhere; no deconstruction/demolition
  order (`StartDeconstructionOfStaticEntityCmd`, `DeconstructTransportSegmentCmd`,
  `QuickRemoveEntityRefundCmd`, …) may auto-apply — demolition is a human
  decision, plan or no plan (the relocation doctrine: replacement live
  before demolition; pause-don't-demolish); no order with
  `requires_sandbox_validation: true` (§6.1) may apply to a non-sandbox
  save until its sandbox validation is recorded in the plan.

---

## 6. Orders

An order is one intended change (or one human/agent task) inside a phase.
Common fields:

```yaml
- id: p1-pin-diesel            # unique across the plan, phase-prefixed
  do: PinToggleCmd             # command orders: exact catalogue name (§6.1)
  args: {productToToggle: Product_Diesel}
  note: "Why this order exists, in one line."
  class: pins-alerts           # action class (§8)
  # optional:
  preconditions: [ <predicate>, … ]   # all TRUE before applying
  verify: [ <predicate>, … ]          # all TRUE after applying (§9 step 5)
  on_fail: pause-and-report           # fallback id (§7.2)
  flags: [single-source]              # uncertainty carried from sources
```

### 6.1 Command orders (`do: <catalogue name>`)

- `do` MUST be a command `type` in the captured catalogue
  (`live/commands.json`, 240 commands) with argument names taken from that
  command's constructor schema. Names not in the catalogue are a plan
  lint error — including plausible-sounding ones. (Edicts are the standing
  example: `ToggleEdictEnabledCmd` exists in the game but is **not** in
  the web catalogue, so it MUST NOT appear as a command order; see G4.)
- **Catalogue status semantics** (companion catalogue, Addenda VI–VII):
  - `verified` (13 commands: `SetSimPauseStateCmd`, `GameSpeedChangeCmd`,
    `ResearchStartCmd`, `ResearchStopCmd`, `ResearchQueueDequeueCmd`,
    `PinToggleCmd`, `SetGlobalPriorityCmd`, `ToggleEnabledCmd`,
    `MachineToggleRecipeActiveCmd`, `StorageSetProductCmd`,
    `StorageDisableLogisticsToggleCmd`, `SetGeneralPriorityCmd`,
    `StorageAlertSetEnabledCmd`) — live-verified through the §12 flow.
  - `wired` (226) — preparable through the generic §12 flow, **not**
    individually live-verified. A plan order using a `wired` command MUST
    carry `requires: first-use-confirm` unless its class autonomy says
    otherwise, and the first live use of each wired command in a run is
    always human-confirmed.
  - `excluded` (1: `GameConsoleCmd`) — MUST NOT appear in any plan.
- **Construction orders** (`CreateStaticEntityCmd`,
  `FinishBuildOfStaticEntityCmd`, `CreateTrainTrackEntityWithDirectionCmd`,
  `CreateNewTrainLineCmd`, `CreateNewTrainNetworkCmd`,
  `AddVehicleToBuildQueueCmd`, and family) exist in the catalogue, but
  placement semantics are **untested** (Addendum VI: site-vs-finished,
  `TileTransform` encoding, cost/validation behaviour all unknown). Any
  order using them MUST carry `requires_sandbox_validation: true`, which
  makes it manual-only on a real save until a sandbox experiment is
  recorded in the plan (a `sandbox_validation:` entry naming the save,
  date, and observed semantics). Plans MUST NOT assume them.
- **Toggle commands are not idempotent** (`PinToggleCmd`,
  `ToggleEnabledCmd`, …). Their `preconditions`/`verify` MUST bracket the
  toggle with the observable state (e.g. pin orders verify against the
  catalogue's `pinned=true` set), and re-applying an already-verified
  order is forbidden (§9, order states).

### 6.2 Entity selectors

Where a constructor argument is an `EntityId`, a plan writes a selector
instead of an int:

```yaml
args:
  storageId: {$entity: {proto: StorageFluidT2, product: Product_Diesel, match: first}}
```

`match` ∈ `first | only | all`. Resolution happens at prepare time from
the bridge census/topology/storages (SPEC §12.3): zero matches or an
ambiguous `only`/`first` match **fails the prepare loudly**, never
guesses. `all` expands to one prepared action per entity under one
confirmation. Literal int entity ids are permitted only in ad-hoc
prepared plans generated during a session, never in a committed plan file.

### 6.3 Non-command order kinds

Honest kinds for work the command channel cannot (yet) do. Each has its
own `do:` value and execution semantics; none of them pretends to be a
command.

| Kind | Fields | Execution semantics |
|---|---|---|
| `do: place-blueprint` | `blueprint_ref` (string, logical name in the blueprint library), `district` (layout zone name), `count` (optional) | Agent/human places the named blueprint string in-game (or, post-sandbox-validation, via construction commands). If `blueprint_ref` has no authored asset yet, the order MUST say `asset_status: not-yet-authored` and degrades to a `manual-step` describing the build in words. Completion is detected by its `verify` predicates (census/product state), not by the placement act |
| `do: manual-step` | `step` (what the human does in the UI), `why` | Executor presents the step, waits for human "done" (or for its `verify` predicates to become TRUE, whichever the step declares via `completion: human-confirm \| verify-predicates`). Never auto-applied, never counted as agent action in the audit — recorded as human action |
| `do: observe-only` | `observe` (what to record), `duration` (optional) | Executor records snapshots/notes for the duration and applies nothing. Used for trials (e.g. pause-don't-demolish trials) and scout phases |
| `do: wait` | `until: [<predicate>, …]` | Executor idles the loop (checks still run) until the predicates hold or the phase's stall fallback fires |

### 6.4 What orders may not do

No order may: target `GameConsoleCmd`; set `allowValidationSuppression:
true` on a construction command outside a sandbox save; spend Unity,
accept loans/contracts, or adopt population except as an explicit order
whose class autonomy permits it (§8) — these are never side effects of
another order.

---

## 7. The phase object

```yaml
- id: phase-1-scrap-starter
  name: "Scrap-fed starter"
  intent: "One line: what this phase achieves and why it comes here."
  skippable: false                  # may a fallback skip it? (§7.2)
  entry_gate: [ <predicate>, … ]    # all TRUE to enter (empty = enter immediately)
  orders: [ <order>, … ]            # §6, applied in order as preconditions allow
  checks: [ <check>, … ]            # §7.1 periodic monitors while in phase
  exit_gate: [ <predicate>, … ]     # all TRUE (with any `for:`) to complete
  fallbacks: { … }                  # §7.2
  autonomy: { <class>: <level> }    # optional tightening only (§5.1)
```

Phases execute strictly in list order. A phase is *entered* when its
entry gate passes, *active* while its orders/checks run, and *complete*
when its exit gate passes. The next phase's entry gate is evaluated only
after the previous phase completes — phases never overlap, because the
starter/relocation doctrine is one district at a time, replacement live
before demolition, hard gates before spending (relocation plan §3).

### 7.1 Checks (periodic monitors)

```yaml
checks:
  - id: diesel-floor
    every: {minutes: 5}              # wall-clock cadence of evaluation
    when: {metric: product.quantity, product: Product_Diesel, op: "<", value: 100}
    severity: critical               # info | warning | critical
    action: crisis-protocol          # fallback id (§7.2) or an order id in this phase
    note: "Bottleneck watch: fuel leg of the death-spiral trio."
```

Checks evaluate while their phase is active (global checks may be
declared under `globals.checks` to run in every phase). A check *fires*
when `when` is TRUE; firing runs its `action` once per firing episode
(rising edge), logs evidence (the predicate values), and — for
`critical` — may pause the game if its action says so. Checks are the
plan form of the starter skill's bottleneck watch and the companion's
§5 detectors; where a companion detector already covers a signal, the
check SHOULD reference it in its note rather than duplicate thresholds.

### 7.2 Fallbacks

```yaml
fallbacks:
  on_check_fail: crisis-protocol       # named protocol (below) or inline action
  on_gate_stall: pause-and-report      # exit gate unmet for stall_after
  on_order_fail: pause-and-report
  on_unevaluable: pause-and-report
  stall_after: {months: 2}             # progress = any order verified or any
                                       # exit-gate predicate newly TRUE
```

Fallback actions (closed set):

- `pause-and-report` — pause the game, stop applying orders, surface the
  evidence to the human. The universal default; any fallback not declared
  defaults to this.
- `substitute: <order-id>` — apply the named substitute order (declared in
  the same phase, normally `enabled: on-fallback` so it isn't applied in
  the normal flow), then resume.
- `skip-phase` — mark the phase skipped and advance. Permitted **only**
  if the phase declares `skippable: true` **and** skipping cannot violate
  a global invariant; the skip and its reason go to the audit log and the
  run report. A skipped phase's exit gate is never retro-claimed as met.
- `crisis-protocol` — the named protocol in `globals.crisis_protocol`:
  a fixed order list for existential threats (for a fresh start: the
  fuel/maintenance/electricity death-spiral response from the starter
  skill — pause research, pause non-essential production, divert iron to
  mechanical parts, boost maintenance). Crisis beats plan: while a
  crisis protocol runs, phase progression is suspended.

### 7.3 Crisis protocol shape

`globals.crisis_protocol` is `{trigger_note, orders: [...]}` using the
same order objects as phases, plus `manual-step` entries for the parts
only a human can do (e.g. Unity boosts — no catalogue command spends
Unity; see G5). It is referenced by id (`crisis-protocol`) from checks
and fallbacks.

---

## 8. Autonomy model

Levels, most → least autonomous:

| Level | Meaning |
|---|---|
| `auto` | Executor may apply without per-order human confirm — still through §12 prepare → apply, still audited; the confirm token is issued by the executor's policy engine, and the human sees the audit stream |
| `confirm` | Human confirms each prepared order (the current companion default for everything) |
| `manual-only` | The agent prepares and presents; a human performs or explicitly triggers it in the UI. All placeholder orders (§4.5), all `manual-step`s, and all sandbox-unvalidated construction |
| `banned` | Never in this plan/run, even manually via the agent channel |

Action classes (an order's `class` MUST be one of these):

| Class | Covers (examples) |
|---|---|
| `sim-control` | `SetSimPauseStateCmd`, `GameSpeedChangeCmd` |
| `research` | `ResearchStartCmd`, `ResearchStopCmd`, `ResearchQueueDequeueCmd` |
| `pins-alerts` | `PinToggleCmd`, `PinnedProductReorderCmd`, `StorageAlertSetEnabledCmd`, `StorageAlertSetThresholdCmd` |
| `global-priority` | `SetGlobalPriorityCmd` |
| `entity-settings` | `ToggleEnabledCmd`, `SetGeneralPriorityCmd`, `SetCustomPriorityCmd`, `MachineToggleRecipeActiveCmd`, `MachineSetRecipeActiveCmd`, priority family |
| `storage` | `StorageSetProductCmd`, `StorageDisableLogisticsToggleCmd`, `StorageSetSliderStepCmd`, slot/sorter assignments |
| `trade-contracts-loans` | `ToggleContractCmd`, `QuickTradeCmd`, `AcceptLoanCmd`, `MakeLoanOverpaymentCmd`, `SetLoanBufferPriorityCmd`, dock priorities |
| `vehicles-logistics` | `AddVehicleToBuildQueueCmd`, vehicle assignment/zone commands, `TruckJobsAllowCmd`/`ForbidCmd` |
| `trains` | line/network/schedule commands (DLC) |
| `construction` | `CreateStaticEntityCmd` family, `SetConstructionPausedCmd`, `SetConstructionPriorityCmd` — **and** the `place-blueprint` kind |
| `deconstruction` | demolition/removal commands — default `banned` for auto, `manual-only` at most |
| `manual` | `manual-step` / `observe-only` / `wait` kinds |

Recommended starting posture for a **first supervised run** (what
`fresh-start-v1` adopts): `sim-control: auto` (pause only; speed changes
`confirm`), `research: confirm`, `pins-alerts: auto`, everything else
`confirm` or stricter, `construction: manual-only`,
`deconstruction: banned`. Autonomy expands class by class as verified
coverage grows (Addendum V, step C) — a plan records the posture it was
run under in the audit log.

---

## 9. Execution semantics — the supervised play loop

One loop iteration (cadence: the executor's poll interval, default 5 s
for observation; order application is event-driven, not per-poll):

1. **Observe.** Pull game-info, product stats, and the bridge routes the
   active phase's predicates reference. Stamp the snapshot (tick, date).
   If any required source fails, mark dependent predicates UNEVALUABLE.
   Snapshots accumulate as the run's **state sequence** (§4.3.1) — the
   history `for:` durations evaluate against; the sequence resets if
   `gameId` changes (a stop condition, below).
2. **Invariants.** Evaluate `globals.invariants`. Any FALSE ⇒ pause,
   halt, report (§5.2). Any UNEVALUABLE invariant ⇒ no orders apply this
   iteration; report if it persists beyond one phase check cadence.
3. **Checks.** Evaluate active checks (phase + global) whose cadence is
   due; fire rising-edge actions (§7.1). A firing `critical` check
   pre-empts order application this iteration.
4. **Gates.** If no phase is active, evaluate the next phase's entry
   gate; on pass, enter it (audit: phase, gate evidence). If a phase is
   active, evaluate its exit gate; on pass, complete it (audit) and stop
   — entry of the next phase happens next iteration, never mid-iteration.
5. **Orders.** For the active phase, in list order: an order is *due*
   when it is `pending`, its `preconditions` are all TRUE, and (for
   `enabled: on-fallback` orders) its fallback has fired. For each due
   order, by kind:
   - *command*: build the §12 prepare (resolve selectors, read
     before-state), then by autonomy: `auto` ⇒ apply via policy token;
     `confirm` ⇒ present and wait; `manual-only` ⇒ present as a human
     task; `banned` ⇒ lint error, halt. After apply, re-observe and
     evaluate `verify`: all TRUE ⇒ order *verified*; else order *failed*
     ⇒ its `on_fail` fallback. **A verified order is never re-applied**
     (toggle safety, §6.1).
   - *place-blueprint / manual-step*: present, wait for declared
     completion (§6.3), record as human action.
   - *observe-only / wait*: hold until duration/`until` satisfied.
6. **Stall detection.** Track phase progress (orders verified, exit-gate
   predicates newly TRUE). No progress for `stall_after` ⇒
   `on_gate_stall` fallback.
7. **Audit.** Every evaluation that changes state (gate pass, check fire,
   order prepare/apply/verify/fail, fallback, skip) appends an audit
   entry: wall time, in-game date/tick, plan id+version, phase, evidence
   values, before/after for writes. The audit log is the run's record;
   the run report is generated from it, never from memory.

**Stop conditions** (any ⇒ loop halts, game left paused): invariant
violation; `gameId` change; game version mismatch; catalogue drift (a
`do:` name missing from the live `/commands` inventory); human stop.
**Plan completion:** last phase's exit gate passes ⇒ loop halts, game
paused (unless the plan's `end_state` says otherwise), run report issued.

**Versioning during a run:** a run binds to one plan version. Editing the
plan mid-run starts a *new* run segment: the executor re-anchors
`time.months_elapsed`, re-reads phase state from gates (completed phases
are re-derived from state, not assumed), and records the version change
in the audit. Research-completion observations (G1) carry across segments
of the same `gameId` only.

---

## 10. Plan lint rules (author checklist)

A plan is well-formed only if all hold — the companion SHOULD lint these
mechanically before a run:

1. Every `do:` command name exists in `live/commands.json`; none is
   `GameConsoleCmd`; arg names match a constructor schema.
2. Every product id is `Product_`-prefixed and exists in the product
   catalogue sample; every research id is in
   `coi/data/research_ids.txt` **or** is a §4.5 placeholder mapping.
3. Every predicate uses §4 metrics only; every gate is evaluable from §2
   sources (placeholders and G-listed gaps make their elements
  manual-only, not invalid — but they must be *marked*).
4. Every construction-family order carries
   `requires_sandbox_validation: true`; every phase's autonomy only
   tightens globals; `deconstruction` is never `auto`.
5. Every phase has an exit gate, fallbacks (or inherits the
   pause-and-report default explicitly), and at least the death-spiral
   checks relevant to its stage (fuel / maintenance-proxy / food).
6. Uncertainty from sources is carried as `flags:` on the element it
   belongs to — never silently dropped, never promoted.

---

## 11. Known gaps — state a plan may *want* but cannot have (yet)

| # | Gap | Consequence for plans |
|---|---|---|
| G1 | `/coi/v1/research` returns `current`, `queue`, `researchedCount`, `hasActiveLab` — **no completed-node id list** | `research.completed` is evaluable only from completions the executor itself observed (a node leaving `current`/`queue` while `researchedCount` increments), anchored at run start. A plan started mid-run cannot prove earlier completions from the API; its early research gates must be entered via human confirmation of the in-game research screen. Bridge fix candidate: enumerate `ResearchedNodes` |

> **G1 RESOLVED 2026-11-03 (CoiBridge v0.6.2, staged in dist/):** `/coi/v1/research` now also returns `researched`: the completed node ids, enumerated from the same verified `ResearchManager.ResearchedNodes` (`IIndexable<ResearchNode>`) the route already counted. Once v0.6.2 is installed, `research.completed` is evaluable directly against live state for any run, not just executor-observed completions; the executor-observed anchoring above remains the fallback for older bridge versions. Pending: companion-side predicate support for the new field (executor work, not a format change) |
| G2 | **No power route.** SPEC §3 planned `/coi/v1/power`; bridge v0.5/v0.6 does not ship it, and electricity is not a product in product stats | No `power.generated/consumed/headroom` predicate exists. Power health must be proxied: census machine-state shares (generators `Working` vs starved machines `NotEnoughInput`) + companion detectors. Plans MUST NOT gate on MW numbers |
> **G2 RESOLVED 2026-11-03 (CoiBridge v0.6.2, staged in dist/):** new route `GET /coi/v1/power` in the V6 dependency class, backed by `ElectricityManager` (`Mafi.Core.Factory.ElectricPower`, `[GlobalDependency(AsEverything)]`). It reports this-tick generated / consumed / demanded / surplus-demanded / capacity / max-capacity / wasted in kW plus `balanceKw` (generated − consumed) and generator/consumer counts. `Electricity.Value` is exact kW (`OneKw = new Electricity(1)`, verified in the shipped struct), so values need no scaling. There is NO battery/electricity-storage field: recon found no battery/storage aggregate on the manager (thermal storage holds a product, readable via product stats). The manager's `ElectricityAvgStats` averages are deliberately not decoded (same discipline as the v0.5 train counters). Once installed, plans may gate on kW predicates; pending companion-side predicate support (executor work, not a format change) |
| G3 | **Construction placement semantics untested** (Addendum VI) | All construction is `place-blueprint`/`manual-step` (§6.3) or sandbox-flagged command orders. No plan may promise an agent-built building on a real save today |
| G4 | **Edicts have reads (v0.6) but no catalogue write**: `ToggleEdictEnabledCmd` is not in the web `/commands` inventory; firing it via `/command` is untested (Addendum VII) | Edict changes are `manual-step` only. Edict *state* predicates (§4.1) are fine |
| G5 | **No Unity read or spend** in any §2 source; Unity boosts (a starter-skill crisis lever) have no command | Crisis protocols include Unity actions as `manual-step`s. No predicate may reference Unity |
| G6 | **Maintenance has no product id** in the catalogue (no `Product_Maintenance…` in the live product list) and no bridge route exposes maintenance stock/coverage | Maintenance health is proxied by census machine states + the MechanicalParts/VehicleParts products. Plans MUST NOT write `product.quantity` predicates for "maintenance" |
| G7 | **Per-entity priority/slider values are write-only**: commands exist (`SetGeneralPriorityCmd`, `StorageSetSliderStepCmd`), but no route reads current values back (Addendum VII recon) | `verify:` for priority/slider orders can only check indirect effects, or must declare `verify: human-confirm`. Before-values for §12 prepare come from the prepare flow's own read attempts and may be "unknown" — the audit must say so |
> **G7 RESOLVED 2026-11-03 overnight (CoiBridge v0.6.1, staged in dist/):** recon round 2 found the values — general priority lives on `LayoutEntity.GeneralPriority` (default from proto `Costs.DefaultPriority`); the census now carries a non-default-only `priorities` map and storage rows carry `generalPriority` + `importPriority`/`exportPriority` (concrete `Storage` only; custom-priority ids have no enumeration surface and blind probing is unsafe — `Storage.GetCustomPriority` asserts on unknown ids). Sliders are derived: step = `Percent.RawValue / 10000` (each step = 10%), exposed as `importStep`/`exportStep`/`transportFrom`/`transportUntil` + raw percents on storage rows. Once v0.6.1 is installed, `verify:` on priority/slider orders can read before/after values directly. Pending: companion-side predicate support for the new fields (executor work, not a format change) |
| G8 | **Quick-trade providers not enumerated** (Addendum VII): no manager surface found; `QuickTradeCmd` takes a typed trade id | Trade-by-id orders are manual-only until the provider/id source is pinned. Contract toggles via `/coi/v1/contracts` ids are the supported path |
> **G8 RESOLVED 2026-11-03 overnight (CoiBridge v0.6.1):** the owner is `WorldMapManager.AllQuickTrades` (`IIndexable<QuickTradeProvider>`, matched by `Prototype.Id` — the same collection the game's `QuickTradeCmd` processor uses). New route `GET /coi/v1/quick-trades` returns each provider's id, buy/pay products + quantities, `upointsCostRaw`, step state and sold-out/price flags. Once installed, quick-trade orders can reference provider ids from the route instead of typed-in guesses |
| G9 | **Refugee/beacon acceptance** has no catalogue command (beacon is a building toggle at best; wave acceptance is UI) | Refugee decisions are `manual-step`s gated on `settlement.free_housing` predicates |
| G10 | **Train schedule state** is read via v0.6 trains-detail, but line/track construction semantics are G3-untested | Rail phases gate on `trains.line_count` and product flows, and keep all track/station work manual until sandbox validation lands |

---

## 12. Worked mini-example — one complete phase

Phase 2 of `fresh-start-v1.yaml`, verbatim in structure (the instance
file carries the full run). It shows: an entry gate mixing research,
product and settlement predicates; a verified research order; toggle
bracketing; a manual build; death-spiral checks; and a stall fallback.

```yaml
- id: phase-2-assembly-power-maintenance
  name: "Assembly + power & maintenance stability"
  intent: >-
    Construction researched → manual assemblies online (construction
    parts first), maintenance depot built, power moved behind a fuel
    buffer. This phase ends the fragile scrap-only start.
  skippable: false
  entry_gate:
    - metric: research.completed
      node: {placeholder: "Research<Construction>", display_name: "Construction",
             status: unsourced-id, confirm: "in-game research screen before first use"}
    - {metric: product.quantity, product: Product_Iron, op: ">=", value: 100}
  orders:
    - id: p2-research-power-maintenance
      do: ResearchStartCmd
      args: {nodeId: ResearchPowerAndMaintenance}
      note: "Maintenance before anything else (starter research order #2)."
      class: research
      flags: [single-source]        # order-variant dispute in the source skill
      verify:
        - any:
            - {metric: research.current, op: "==", value: ResearchPowerAndMaintenance}
            - {metric: research.queue_contains, node: ResearchPowerAndMaintenance, op: "==", value: true}
    - id: p2-build-maintenance-depot
      do: place-blueprint
      blueprint_ref: "starter-cluster/maintenance-depot"
      asset_status: not-yet-authored
      district: starter-cluster
      note: "Degrades to manual build until the blueprint asset exists."
      class: construction
      completion: verify-predicates
      verify:
        - {metric: census.proto_count, proto: MaintenanceDepotT1, op: ">=", value: 1, flags: [proto-id-unconfirmed]}
    - id: p2-pin-mechanical-parts
      do: PinToggleCmd
      args: {productToToggle: Product_MechanicalParts}
      class: pins-alerts
      preconditions:
        - {metric: product.unlocked, product: Product_MechanicalParts, op: "==", value: true}
      verify:
        - {metric: product.pinned, product: Product_MechanicalParts, op: "==", value: true}
  checks:
    - id: p2-diesel-floor
      every: {minutes: 5}
      when: {metric: product.quantity, product: Product_Diesel, op: "<", value: 100}
      severity: critical
      action: crisis-protocol
  exit_gate:
    - {metric: research.completed, node: ResearchPowerAndMaintenance}
    - {metric: product.net_flow, product: Product_MechanicalParts, window: lastMonth, op: ">=", value: 0}
  fallbacks:
    on_gate_stall: pause-and-report
    stall_after: {months: 2}
```

(`product.pinned` in the pin verify is read from the product catalogue's
`pinned=true` view, §2 and §4.1. The `flags:` key inside the census
verify predicate above is the §4.5 uncertainty-carrying convention:
extra annotation keys on a predicate are permitted and are copied into
the audit evidence, but an executor MUST ignore unknown keys when
evaluating.)

---

## 13. Provenance

Format decisions trace to: SPEC §0.1/§3 (state sources), §12/§12.7
(two-phase writes, selectors, catalogue mapping), Addendum V (north
star), Addendum VI (construction primitive + untested semantics),
Addendum VII (13 verified / 226 wired / 1 excluded; v0.6 read routes;
edict/quick-trade/priority-read gaps), the starter skill + references
(phase/research/layout/bottleneck conventions), and the relocation plan
(hard gates, pause-don't-demolish, checkpoint style). Where those sources
disagreed with a clean design, the gap is in §11 rather than papered over.
