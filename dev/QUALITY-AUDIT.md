# Code Quality Audit — CoI Toolkit (2026-11-06)

Read-only audit commissioned by Philip: *"check the coding style and
practices in our mods — are we effective, do we have equiv tests, are
we writing high quality?"*

Scope: `bridge/CoiBridge` (C#), `qos/*` (QoSCore + QoSChickens,
QoSVehicleRecovery, QoSPriorityActuator, QoSWorldMap, QoSBuilder —
C#), `coi/` (Rust companion: main, serve, store, plan, productivity,
terrain, saveinfo, client, guides, catalogue). Evidence is
file:line throughout. No code was changed by this audit.

**⚠️ QoSBuilder is a moving target:** the designation-jobs round was
editing its sources concurrently. QoSBuilder findings describe the
tree as it stood at audit start (BuilderService.cs 572 lines,
BuilderJson.cs 372, no designation code yet) and should be
re-checked for that project when the round lands.

Grades: **DEFECT** (wrong today) / **RISK** (wrong under plausible
conditions) / **STYLE** (consistency/maintainability) / **TEST-GAP**
(no automated net where the risk class demands one).

---

## Overall verdict

The toolkit is **well above hobby-mod quality in discipline, and
uneven in verification**. The C# follows one architectural pattern
rigorously (DI, resolution isolation, simulate-then-schedule,
ship-disabled); the post-incident serializer discipline is applied
uniformly and correctly in all four subscribing services; the Rust
companion's store/terrain/productivity cores are genuinely good
(parameterised SQL, versioned migrations, pure estimators with
hand-computed test vectors). Builds are 0 warnings; `cargo test`
22/22 pass; there is not one TODO/FIXME/HACK in either codebase.

The central failure is **test equivalence**: verification effort is
*inversely correlated with blast radius*. The code that can break a
save or mutate the game (QoS guard arithmetic, serializer pairs,
QoSBuilder's C# job-hash verifier) has the *least* automation —
compile + inspection only — while advisory, read-only logic (plan
evaluation, productivity estimators) has the most (dual engines,
exact-parity gates, 20 fixture state-dirs, 12 hand-checked unit
tests). Two real defects also sit in the companion's §12 path —
the flow whose entire point is its audit guarantee.

Per-component verdicts:

| Component | Verdict |
|---|---|
| CoiBridge (1,940 LOC) | **Good.** Disciplined read-only bridge; two unguarded handlers (D2) and one stale recon leftover shipping as a route (R6). |
| QoSCore (446) | **Good.** Minimal, sound; write-only `Features` API (S5), minor unsynchronised statics (R5). |
| QoSChickens (569) | **Best-structured QoS policy** — exact simulation factored as statics; welded to concrete game types, so untestable as-is (T2). |
| QoSVehicleRecovery (472) | **Correct on money** (game constants only), but the most interleaved guard method in the suite (T2). |
| QoSPriorityActuator (572) | **Sound design** (stateless ownership, overflow care); safety rule is ~10 inline lines, untestable as written (T2). |
| QoSWorldMap (567) | **Good** pure-view mod; structural deviations are deliberate and documented. |
| QoSBuilder (1,034) ⚠️ | **Most defensively coded C#** (per-job refusal records, tamper/expiry checks); its C# hash verifier has never been executed (T3). |
| Rust companion (14,048) | **Good core, two defects in serve.rs** (D1, R1). store.rs is the hygiene exemplar; plan.rs is 2,711 untested lines (T4). |
| Game plans | **Strongest cross-checking in the toolkit** (Python↔Rust parity, fixtures) — for logic that has never executed live. |
| Blueprint pack | Decode-verified, audit-scripted; **explicitly never imported into the game**. |

---

## Findings

### DEFECTS

**D1 — §12 audit trail is best-effort; an applied command can leave
no record. (Rust, serve.rs)**
`do_apply` writes the audit + activity rows under
`if let Ok(st) = store(state)` with `let _ = st.audit_add(..)` /
`let _ = st.activity_add(..)` (`serve.rs:2513-2528`). Same pattern
in place-confirm (`serve.rs:3002-3012`) and snapshot activity
(`serve.rs:5007`). If the DB won't open or the insert fails, a
command *has been applied to the game*, the user sees success, and
the §12 audit row — the flow's core guarantee — silently does not
exist.

**D2 — Two bridge handlers bypass the mandated `Guard()`. (C#)**
SKILL.md mandates every handler wrapped in `BridgeRoutes.Guard`
(exception → JSON 500, never throw into the sim). `Hello()`
(`BridgeRoutes.cs:121-124`) is trivial, but `DebugServices()`
(`BridgeRoutes.cs:128-129`) calls the reflection-heavy
`DiscoveryReport.Build(...)` unguarded — the most failure-prone
handler is the one that can throw toward the web server.
`TerrainRegion` (`BridgeRoutesTerrain.cs:178-233`) additionally
re-implements Guard inline (~8 duplicated lines) instead of
reusing it.

**D3 — ARCHITECTURE.md states a safety rule that is now wrong.
(docs)**
`qos/ARCHITECTURE.md` "Removal safety" states as suite rule:
*"Manifests carry `can_add_to_saved_game: true` and
`can_remove_from_saved_game: true`."* Reality post-incident:
the four calendar-subscribing mods correctly carry
`can_remove_from_saved_game: false` (their manifests; §5 of the
C# audit verified all four pairs + flags). Anyone "fixing"
manifests to match the architecture doc would reintroduce the
0.5.0-class save hazard. The code is right; the doc is stale.
The Coding-standards section added to ARCHITECTURE.md by this
audit states the correct rule and supersedes that sentence.
Related drift: four mod doc-comments cite the superseded
`QoSCore >= 0.1.0` dependency floor (`QoSChickensMod.cs:16`,
`QoSPriorityActuatorMod.cs:31`, `QoSVehicleRecoveryMod.cs:19`,
`QoSWorldMapMod.cs:21`); all manifests say `>= 0.2.0`.

### RISKS

**R1 — Mutex-poison cascade can wedge the whole website. (Rust)**
Thread-per-connection contains a panic to one dropped connection,
but there is no `catch_unwind` anywhere in the crate, and seven
sites lock shared state with `.lock().unwrap()`:
`sessions`/`prepared`/`prepared_jobs` at `serve.rs:403, 426, 1139,
2222, 2486, 2932, 2980`. One panic while a lock is held poisons
the mutex; every subsequent authenticated request then panics too
— full website outage until restart, in the middle of what may be
a supervised play session.

**R2 — Productivity hysteresis state writes are silently dropped.
(Rust)** Five `let _ = store.prod_state_put(..)` at
`productivity.rs:709, 753, 771, 805, 866`, and
`let _ = productivity::evaluate_snapshot(..)` at `serve.rs:4989`.
Failure ⇒ verdicts can re-fire/flap on the next snapshot with no
trace. Advisory-only impact, but it undermines the estimator's
evidence discipline, which is otherwise the crate's best.

**R3 — Config defaults exist twice, with no agreement check.
(C#)** All 28 mod-config keys are declared in `config.json` *and*
as literals in `Get*(name, default)` calls (e.g.
`max_animals_moved_per_day: 50` in config vs `ChickenBalancer.cs:160`).
Spot-check: all 28 currently agree. Nothing enforces it; drift
would be silent. The duplication itself is small (28 one-line
calls, ~10 clamp wrappers) and core-side centralisation is
correctly rejected by ARCHITECTURE (config is per-mod by the
game's design) — the gap is the missing check, not the shape.

**R4 — No clean build has ever been proven; no one-command build.
(build)** No script removes `bin/obj`; every build is incremental
against a dirty tree (`.gitignore` keeps artefacts out of git, but
no from-scratch build is on record). Four build rituals exist
(bridge, suite, builder standalone, cargo) and `README.md:Building`
omits the Builder entirely. Version truth has two exceptions:
`SUITE_VER="0.5.1"` is hardcoded in `qos/build/build.sh:66`
(nowhere else), and the Rust companion's `Cargo.toml` version
(0.1.0) is decorative — its real version lives only in `coi-v*`
git tags. QoS/Builder zips are timestamp-deterministic; the bridge
zip is not.

**R5 — Unsynchronised static flags in QoSCore. (C#)**
`QoSEventLog.Enabled` (`QoSEventLog.cs:50`) and `GameDateProvider`
(`:59`) are public mutable statics read outside `s_lock`;
`Record` checks `Enabled` before locking (`:91-96`); `s_warned`
(`:63`) is touched in the catch path unlocked. Appends themselves
are correctly serialised. Effects are benign (a dropped warning,
one event after disable) — noted because ARCHITECTURE claims
"thread-safe" without qualification. `QoSLog.Verbose`
(`QoSLog.cs:15`) is the same one-shot pattern.

**R6 — DiscoveryReport: 286 lines of fulfilled recon still ship
as a production route. (C#)** `DiscoveryReport.cs` is live at
`/coi/v1/debug/services` (`BridgeRoutes.cs:127`); its header
(`DiscoveryReport.cs:9-14`) still describes an M1.1 workaround for
services "still marked GUESS" — that purpose closed in M2. It is
the largest leftover-recon surface inside a mod whose posture is
"read-only, registers no content", and it carries the scope's
only cluster of silent catches (`DiscoveryReport.cs:161-163`,
`196-243` ×4, `263-265` — metadata-probe placeholders by design,
but invisible as failures).

### STYLE

- **S1 — serve.rs is navigable but drifting from DESIGN.md.**
  104 top-level fns, one dispatch match (28 arms), real shared
  helpers (`layout*` 44×, `badge` 55×, `page_head` 27×); a new
  page costs ~3 touchpoints. But NAV has 14 entries vs DESIGN
  §2.2's 12, one raw hex survives outside the CSS const
  (`serve.rs:912`, against DESIGN §1.1), and 48 `style=` attributes
  remain (most are legitimate dynamic widths).
- **S2 — Duplication clusters in serve.rs.** (i) Live-fetch
  prologue repeated ~17× (`serve.rs:1449, 1593, 1708, 1798, 2420,
  3909, 4325, 4603, 4977` + nested variants); (ii) bridge-section
  fetch+degrade re-implemented per section (24 `bridge_json`
  calls; `page_trade` alone 4× at `:3385, 3429, 3476, 3543`);
  (iii) identical 3-line CSRF prologue in 9 POST handlers
  (`serve.rs:2094, 2373, 2412, 2454, 2482, 2839, 2976, 4974,
  5426`); the `arg_<name>` form convention is implemented three
  ways (`:2333`, `:2387`, hand-written at `:3330-3341`,
  `:3496-3526`). Auth, by contrast, is properly centralised
  (`serve.rs:431-436`).
- **S3 — main.rs is dispatch + engine + reporting in one file**
  (1,518 lines; `cmd_products:647` alone is 532). plan.rs has a
  coherent section skeleton but god-functions inside it:
  `eval_leaf:387` is 547 lines, `play_cmd:2260` 452,
  `build_report:1497` 435.
- **S4 — Naming/tagging nits (C#).** `ChickenBalancer` is the only
  daily service not named `*Service`. QoSLog feature tags differ
  from mod ids (`QoSBuilder`→`"Builder"`, `QoSChickens`→
  `"Chickens"`) while `QoSEventLog.Record` uses full mod ids —
  grepping one name misses the other surface. `QoSCoreMod.cs:60`
  logs init failure via raw `Log.Error`, bypassing QoSLog.
- **S5 — Write-only API + hand-maintained route table (C#).**
  `SuiteRegistry.Features` (`SuiteRegistry.cs:62-65`) has no
  consumer; only `Announce(...)` is ever called. `BridgeRoutes.cs:
  68-89 s_routeTable` is a hand-maintained 20-entry list spanning
  three classes — a new route serves fine while silently absent
  from discovery output.
- **S6 — Bare `65536` (Fix32 raw-per-unit) in ≥5 places**
  (`BridgeRoutesTerrain.cs:143, 214, 424-425`,
  `VehicleRecoveryService.cs:379`); `/ 10000` percent divisors
  unnamed at `BridgeRoutes.cs:588-591`. One shared named constant
  per assembly ends this class.
- **S7 — Credits where earned.** Zero TODO/FIXME/HACK in either
  codebase. `cargo build` 0 warnings; only 3 narrow `#[allow]`s
  in the crate. SQL in store.rs is parameterised throughout (the
  two `format!` SQL strings interpolate a compile-time-constant
  WHERE fragment; values are bound, LIKE is escaped —
  `store.rs:687-756`). Migrations are versioned and idempotent
  (`store.rs:229-255`), though not transactional. Catalogue's
  generated-code boundary is respected (`catalogue.rs:1-3` header;
  editable tables live in `tools/gen_catalogue.py:20-60`).
  `saveinfo.rs:98-107` keeps one dated `static mut` CRC-table
  unsafe that `OnceLock` would eliminate.

### TEST-GAPS

**T1 — Serializer pairs: correct today, guarded by nothing.**
All four subscribing services carry the sanctioned pair and
manifest flag (verified line-by-line in this audit: pairs at
`ChickenBalancer.cs:97-115`, `VehicleRecoveryService.cs:101ff`,
`PriorityActuatorService.cs:120ff`, `BuilderService.cs:97ff`).
But no test, lint, or build step asserts their presence. This
exact omission broke *all saving* in suite 0.5.0, and 0.5.1's
save+reload proof is still owed (WEEKEND-RUNBOOK Gate zero).
A regression here merges silently and detonates in-game.

**T2 — QoS guard arithmetic: zero automated tests, and mostly
untestable as written.** The safety rules of the three daily
mods — slaughter caps / breeding floor / equalisation
(`ChickenBalancer.cs`), Unity reserve 5.0 / daily cap 5 /
7-pass mark expiry (`VehicleRecoveryService.cs`), the owned-set
band `{D−k}` / cooldown / caps (`PriorityActuatorService.cs`) —
have no test of any class. Worse, in Recovery and Actuator the
decision logic is interleaved with live game objects, session
state and `ScheduleInputCmd` side effects in single methods
(`VehicleRecoveryService.cs:236-368`, `PriorityActuatorService.cs:
322-410`); Chickens factors its simulation into statics
(`ChickenBalancer.cs:419, 454, 334`) but welds them to concrete
`AnimalFarm`. The "Nudge test"/"Restore test" entries in
BUILD-NOTES (`:671-674`) are prose design definitions, not
executable tests. Current safety net: compile + "eyeball the
first enabled days".

**T3 — QoSBuilder's C# canonical hash has never been executed.**
The job hash is "pinned three ways" per BUILD-NOTES (`:1219`),
but the Rust test (`serve.rs:5460 job_canonical_vector`) and the
shell check (`tools/check_place_page.sh`) both exercise only the
Rust and Python implementations. The C# verifier
(`BuilderJson.cs:67 Canonical`) — the one that refuses jobs
in-game — has compile + visual equivalence only. A C#-only
divergence surfaces first as a refused (or wrongly accepted)
live job.

**T4 — plan.rs / main.rs / client.rs / saveinfo.rs have zero
unit tests.** plan.rs is 2,711 lines whose own header
(`plan.rs:1-15`) declares Python parity "a release gate"; parity
*is* genuinely enforced by `tools/check_plan_parity.py` — but
nothing in `cargo test` would catch a regression, and the parity
script is not wired into CI either (T5). main.rs's detectors and
forecasts (the oldest logic in the crate) and client.rs's
hand-rolled chunk parser (`client.rs:179-223`) are likewise
untested. The catalogue has no invariant test for its claimed
15 verified / 224 wired / 1 excluded counts.

**T5 — No CI runs any test.** The sole workflow
(`.github/workflows/release.yml`) builds the Rust release binary
on `coi-v*` tags. No `cargo test`, no `check_*.sh`, no parity
run, on any trigger. (C# cannot build in CI — game DLLs aren't
redistributable — but the entire Rust/Python net could run and
doesn't.)

---

## Test-equivalence matrix

Classes: **UT** automated unit tests in-repo · **XV** cross-checked
vectors (independent second implementation) · **SF** stub/fixture
integration · **LV** live-verified in Philip's real game · **PE**
pattern-equivalence by inspection vs shipped game code · **CO**
compile-only for the behaviour in question.

| Component | UT | XV | SF | LV | PE | CO |
|---|---|---|---|---|---|---|
| CoiBridge | ✗ (zero C# tests repo-wide) | ✗ (avg-power decoding explicitly unverified, SPEC:1064) | Partial — consumer-side stub captures only (`live-synthetic-v06/v064/v065`); producer never harness-executed | ✓ ≤0.6.4 (SPEC:775, 806); ✗ 0.6.5 (Add. XIX: live outstanding by design) | ✓ (MODDING-NOTES recon) | — |
| QoSCore | ✗ | ✗ | Partial — `check_log_page.sh` tests the companion import, not the C# writer | Partial — suite load-verified in-game 5 Nov | ✓ | — |
| QoSChickens | ✗ | ✗ | ✗ | Load only; enabled-days behaviour unverified (BUILD-NOTES:84ff) | ✓ | **✓ for guard arithmetic (T2)** |
| QoSVehicleRecovery | ✗ | ✗ | ✗ | Load only | ✓ | **✓ for guards (T2)** |
| QoSPriorityActuator | ✗ | ✗ | ✗ | Load only | ✓ | **✓ for owned-set arithmetic (T2)** |
| QoSWorldMap | ✗ | ✗ | Partial — probe compiled vs real Mafi set 0/0 | Load only; window interaction unchecked | ✓ | Mostly |
| QoSBuilder | ✗ (C#) | **Partial** — Rust↔Python ✓, C# leg ✗ (T3) | ✓ companion half (`check_place_page.sh`) | ✗ — "code + stub level ONLY" (BUILD-NOTES:1246ff) | ✓ | **✓ for in-game execution** |
| Rust companion | ✓ 22 tests, concentrated (productivity 12, terrain 5, store 3, serve 2; **zero** in plan/main/client/saveinfo — T4) | ✓ (saveinfo byte-faithful cross-check; terrain estimator vs independent computation) | ✓ best in toolkit (stub server, 3 bridge generations) | ✓ (M3 live; §12 applies audited net-zero) | n/a | — |
| Game plans | n/a | ✓ **strongest XV in toolkit** (Python plan_check.py ↔ Rust plan.rs, exact verdict parity) | ✓ (12 growth fixtures + 8 history states) | ✗ — runner is prepare-only by construction; never executed live | n/a | — |
| Blueprint pack | Partial (pack audit scripts assert geometry/footprints) | ✓ (decode-verified at generation; plan refs audited) | Partial (embedded library via check_place_page) | ✗ **explicitly never imported into the game** | n/a | — |

**Reading of the matrix — the equivalence answer.** Held to the
same standard for the same risk class, we are **not** equivalent:

- *Read-only / advisory code* (plan evaluation, productivity,
  terrain estimator, store): UT + XV + SF, some LV. **Meets the
  standard, arguably exceeds it** (dual plan engines with exact
  parity is more verification than two unexecuted plans need).
- *Mutation-capable / save-threatening code* (QoS guards,
  serializer pairs, Builder execution + hash): CO + PE only,
  LV = "loads". **Fails the standard** — this tier has the least
  automation and the largest blast radius (0.5.0 already proved
  the failure mode once).
- *Bridge routes* sit between: live-verified at introduction,
  never regression-tested afterwards; the stub freezes
  yesterday's output, so a route regression is only caught when
  the companion degrades — which *is* tested. Half a net.

Best-covered component: **Rust companion**. Worst-covered:
**the QoS feature tier** (Chickens/Recovery/Actuator tied) —
save-class code, pure-ish guard logic, zero automated tests.

---

## Prioritised remediation

Ordered by risk retired ÷ effort. "Before weekend" = must land
before the supervised fresh-run session (or before the build it
gates ships); "Follow" = safe to schedule after.

| # | Item | Effort | Risk retired | When |
|---|---|---|---|---|
| 1 | **Serializer-pair lint in `qos/build/build.sh`**: assert every calendar-subscribing service has `[ManuallyWrittenSerialization]` + static Serialize/Deserialize + manifest `can_remove:false`. ~20 lines of grep. (T1) | XS | The exact bug class that broke all saving in 0.5.0; also covers the in-flight designation code the moment it subscribes. | **Before weekend** (before any new QoS build ships) |
| 2 | **Make §12 audit writes fail loudly**: in `do_apply`/place-confirm, a failed audit/activity insert must surface as an error state to the user (applied-but-unaudited), not `let _ =`. Decide in the fix round whether audit precedes apply for the apply path. (D1) | S | Silent loss of the §12 flow's core guarantee. | **Before weekend** |
| 3 | **Execute the C# canonical-hash vector**: feed the existing `job_canonical_vector` through `BuilderJson.Canonical` via the existing `qos/build/probe` harness. (T3) | S | First live Builder job refused/accepted on an unverified third implementation. | **Before weekend** (Gate zero's first job depends on it) |
| 4 | **Poison-tolerant locking in serve.rs**: replace the 7 `.lock().unwrap()`s with poison-recovering locks (or wrap `route()` in `catch_unwind`). (R1) | S | One bad request permanently wedging the control surface mid-session. | **Before weekend** |
| 5 | **Guard the two bridge handlers + reuse Guard in TerrainRegion.** (D2) | XS | The reflection-heavy debug handler throwing into the web server. | Before next bridge build |
| 6 | **Extract + unit-test QoS guard arithmetic**: decision functions over plain values (counts/caps/bands) for the three daily services, small net8 test runner alongside the probe harness. Chickens needs only a value-object seam; Recovery/Actuator need the decision-function refactor. (T2) | M | First regression net for the mutation-capable tier; converts "eyeball first enabled days" into a checked property. | Before any QoS feature is enabled *unsupervised*; may follow the session's supervised first-enable |
| 7 | **Wire the existing net into CI**: `cargo test` + `check_plan_parity.py` + `check_log_page.sh` + `check_place_page.sh` on push. All already exist and pass locally. (T5) | S | Regressions merging unnoticed between rounds. | Follow |
| 8 | **Config-default agreement check**: script comparing `config.json` defaults to the `Get*` literals, run in `qos/build/build.sh`. (R3) | S | Silent settings drift across 28 keys. | Follow (fold into item 1's build checks) |
| 9 | **`build-all.sh` + one proven clean build**: `rm -rf bin obj` per scaffold, `SUITE_VER` read from a version file, README gains the Builder ritual. (R4) | S–M | Incremental-state and version-drift surprises in every future round. | Follow |
| 10 | **Retire or gate DiscoveryReport**: delete `/coi/v1/debug/services` from the shipped bridge, or keep it behind an explicit debug flag with its header corrected. (R6) | S | 286 lines of stale recon surface + the only silent-catch cluster in the bridge. | Follow |
| 11 | **plan.rs leaf tests + catalogue invariant test**: pin a handful of `eval_leaf` cases in `cargo test` (parity script stays the gate) and assert catalogue status counts. (T4) | M | Silent parity/count drift in the runner's core. | Follow |
| 12 | **serve.rs helper extraction** (fetch prologue, CSRF prologue, bridge-section helper) + DESIGN.md nav/hex reconciliation. (S1, S2) | M | Churn reduction only — deliberately last; not worth destabilising the control surface before the session. | Follow |

### What NOT to fix

- QoSCore's minimalism is correct: ARCHITECTURE's "pattern, not a
  store" call on config is right, and `SuiteRegistry`'s
  behaviour-free design is deliberate (only its unused `Features`
  getter is dead weight, S5).
- The plan dual-engine "over-testing" should not be dismantled:
  parity is the runner's safety case. The imbalance is fixed by
  raising the QoS tier (items 1, 6), not by lowering this one.
- ChickenBalancer's simulate-then-schedule structure and
  VehicleRecovery's game-constants-only money handling are the
  suite's exemplars — the fix round should copy their shape, not
  rewrite them.

---

## Remediation status (2026-11-06, evening — remediation round)

Philip approved ("yes fix more"). Implemented and verified in the
remediation round; the audit above is unchanged (it describes the
tree as found).

| Audit item | Status | Evidence |
|---|---|---|
| **1 — Serializer-pair lint (T1)** | **DONE** | `tools/check_serializer_pairs.py`: discovers subscribers by the `.Add(this, ...)` pattern (no hardcoded list; outermost-class attribution so nested helpers can't steal it), asserts `[ManuallyWrittenSerialization]` as a real attribute line (comment mentions don't count — caught in self-proof), the static Serialize/Deserialize pair, and the owning manifest's `can_remove_from_saved_game: false`. Wired into `qos/build/build.sh`, `qos/build/build-builder.sh` and `bridge/vm-build/build.sh` (build refuses on failure). Proof: PASS on the real tree (4/4 discovered: BuilderService, ChickenBalancer, PriorityActuatorService, VehicleRecoveryService); red on a scratch copy for each break class in turn (attribute removed / Serialize renamed / Deserialize renamed / manifest flag flipped), green again after restore; scratch deleted. Suite build re-run end-to-end with the lint live: C# 0 warnings / 0 errors, all pre-existing dist zips byte-identical afterwards. |
| **2 — §12 audit writes fail loudly (D1)** | **DONE** | `coi/src/serve.rs` `do_apply` + `do_place_confirm`: store-open and per-row insert errors are collected into an `audit_err` and surfaced — a danger banner ("AUDIT WRITE FAILED … this action is missing from the audit trail"), the apply headline degrades to "Command executed — NOT audited", the place write-failed page no longer claims a failed audit is "in the audit log", and the server logs a `left NO audit record` line. Healthy-DB behaviour unchanged. Proof: `tools/check_log_page.sh` PASS, `tools/check_place_page.sh` PASS (healthy path), plus new `tools/check_audit_fail_loud.sh` PASS — serves against the stub, swaps the DB file for a directory after prepare, and asserts both confirm and apply pages surface the failure. Deliberately out of scope: the snapshot activity write at old `serve.rs:5007` (a website snapshot record, not a §12 apply). |
| **4 — Poison-tolerant locking (R1)** | **DONE** | One helper, `lock_map` in `serve.rs` (`lock().unwrap_or_else(\|p\| p.into_inner())`), now used at all 7 former`.lock().unwrap()` sites (sessions ×3, prepared ×2, prepared_jobs ×2). No `.lock().unwrap()` remains outside the helper's own doc comment. Behavioural proof: the two page-check scripts exercise login/prepare/confirm through the helper end-to-end. |
| **Runner parity fix** (not an audit item — from the self-sufficiency pack's caveat) | **DONE** | `decide_order` in `coi/src/plan.rs` now takes the recording step's state and SKIPs a `ResearchStartCmd` whose node is in the capture's `researched` list (positive evidence only; a list-less pre-0.6.2 capture can never hold). 3 new unit tests (25/25 total). Walk diff vs the v1.3.0 sessions: exactly one outcome changed across 33 recorded orders — growth-walk `p3-research-trading-dock` prepared → skipped ("research node ResearchTradeDock already researched"), phase histories and finals identical; sessions/transcripts regenerated. `tools/check_plan_parity.py`: **PARITY PASS** on both plans. Note: the pack's framing named p1-research-construction, but that order is recorded at step 0, before CpPacking completes, so it correctly still prepares; and the parity failure itself traced to the committed Python reference `plans/fresh-start-v1.dryrun.md` being stale (v1.1.0-era, placeholder args — the pack regenerated every report except that one). The reference was regenerated from the current plan + Python engine as part of this fix; no `render_order` semantics changed in either engine. |
| 3 — C# canonical-hash vector (T3) | OPEN — owned by the QoSBuilder stage-2 round (executed there per its brief; not duplicated here). | |
| 5 — QoS guard-arithmetic extraction + tests (T2) | OPEN — next wave, per the remediation brief. | |

Companion restaged after this round: `dist/coi.exe` 6,219,897 B,
sha256 `01397c50758207926fcac7f8bdc6d6122b9aec42b182d45171581ec572ae2924`
(cargo build 0 warnings, cargo test 25/25); the weekend bundle's
exe copy and its BUNDLE-README row were updated to match. All prior
dist zips byte-identical.
