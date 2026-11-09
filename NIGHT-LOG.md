# NIGHT-LOG — overnight build session 2026-11-02 → 2026-11-03

Philip's mandate (2026-11-02 21:07 Paris): build all night, do not stop,
write self-LLM skills/context rules as you go, for (1) the Rust companion,
(2) the C# CoiBridge mod, (3) playing the game as an agent.

HARD CONSTRAINTS for the whole night:
- NO file uploads of any kind. Deliverables stage in dist/ only.
- NO command applies to Philip's live game. Read-only GETs + prepare-only
  tests via the tailnet recipe are allowed. Never POST /actions/apply.
- No messages to Philip. All work stays in the workspace.

Queue: (1) three skills · (2) bridge recon round 2 + possible v0.6.1 ·
(3) game-plan format + fresh-start plan · (4) design-diagnosis v1 ·
(5) README/SPEC polish if time.

---

## [21:10 Paris] Session start — context loaded
- Read SPEC.md Addenda V–VIII, local-coi-starter skill shape.
- Coordinator plan: fan out three subagents in parallel — (A) the three
  skills, (B) bridge recon round 2, (C) game-plan format + fresh-start
  plan. Diagnosis v1 and polish after, in the coordinator.

## [21:08 Paris] Skills written
- `~/workspace/custom-skills/local-coi-companion-dev/` (SKILL.md + skill.yaml) — building/extending the Rust companion: architecture, catalogue generation, generic vs curated prepares, §12 discipline, status semantics, Linux/Windows build recipes, scratch live-test recipe, seasonal detectors, delivery/no-upload rule.
- `~/workspace/custom-skills/local-coi-bridge-dev/` (SKILL.md + skill.yaml) — building CoiBridge: Tier-1 read-only posture, DI/route patterns, isolated route classes, verified manager surfaces, manifest/version discipline, VM net48 build, recon workflow (harness + ilspycmd env), origin/chat-link traps, leak rule.
- `~/workspace/custom-skills/local-coi-playing/` (SKILL.md + skill.yaml) — playing through the companion: observe→plan→act→verify loop, vanilla + bridge route reading (incl. Product_-prefix vs display-name trap), Dragon Isle doctrine, relocation-plan phase gates, hydrogen backbone policy, snapshot habits, live-state caveats, game-plan format pointer.
- Flagged as unverified in the skills: ⚠️ LoansManager.CreditScore Fix32 /65536 decoding is an assumption (bridge skill); ⚠️ ToggleEdictEnabledCmd and CreateStaticEntityCmd actuation/semantics untested, sandbox only (playing + bridge skills); ⚠️ GAME-PLAN-FORMAT.md and plans/ did not exist yet when the playing skill was written — it points at them as the contract being written tonight and falls back to the relocation plan's phase structure.
- No uploads made; no commands applied to the live save; files only, per the overnight constraints.

## [21:14 Paris] Design-diagnosis v1 (coordinator, offline)
- Built tools/diagnose_live.py — pure offline analysis over the
  2026-11-02 captures in live/ (bridge census/topology/storages +
  vanilla stats). Output: live/diagnosis-v1.md (194 lines).
- Headline findings (strong evidence only):
  * 10 of 1,025 transports flagged not-fully-connected by the game
    itself; 5 of them are single-tile dead ends.
  * Crude oil is the one cross-validated storage fault: stored
    9,713 vs capacity 9,000 (+713 surplus in machine buffers), with
    two 3,000/3,000 crude tanks output-disabled.
  * Construction Parts I (741/741) and II (507/507) at capacity —
    matches the relocation plan's war-chest gate state.
  * Starved clusters corroborate the known steel stall
    (CasterCooledT2 3/4, OxygenFurnaceT2 2/4 NotEnoughInput) and
    AssemblyElectrifiedT2 at 18/33 starved.
  * Blocked: Crusher 8/8 OutputFull, MaintenanceDepotT1 4/4.
- Honesty fixes made during the pass: single-tile transports
  (from == to) are NORMAL per-tile construction, not stubs — the
  script now says so and only counts the unconnected overlap;
  full+output-disabled storages are labelled proven faults only
  where the product is also backed up product-level (crude only);
  §4 splits stored>capacity (1) from stored==capacity (30, mostly
  low-capacity intermediates = weak evidence).

## [21:20 Paris] README + SPEC polish (chunk 5, coordinator)
- coi/README.md: Trade page added to the page list; Catalogue copy
  now verified/wired/excluded with Prepare buttons; new "Generic
  prepare" paragraph (all 239 non-excluded commands executable);
  writes-policy paragraph updated from the old allowlist wording;
  exe size corrected to ~4.9 MB; Verified section gained the generic
  flow proof + Trade degradation notes.
- SPEC.md: Addendum VIII results subsection appended (skills, recon
  round 2 pointer, game-plan format, diagnosis v1 headlines).
- Status: skills DONE (21:08), diagnosis v1 DONE (21:14), polish
  DONE. Still running: bridge recon round 2, game-plan format.

## [21:12 Paris] Game-plan format + fresh-start plan
- Files written: `GAME-PLAN-FORMAT.md` (format v1: closed predicate
  vocabulary over vanilla stats/history/game-info + bridge
  status/research/census/storages/trains + v0.6 contracts/loans/edicts;
  three-valued TRUE/FALSE/UNEVALUABLE evaluation; order objects for
  catalogue commands + honest place-blueprint / manual-step /
  observe-only / wait kinds; entity selectors instead of raw ids;
  phase object with entry/exit gates, cadenced checks, closed fallback
  set incl. crisis protocol; autonomy levels per action class, phases
  may only tighten; supervised play loop + lint rules; worked phase
  example) and `plans/fresh-start-v1.yaml` (8 phases, 0–7: scout →
  scrap starter → assembly/power/maintenance → mining → concrete/
  diesel/rubber → copper/Construction II/food security → automation/
  expansion → rail introduction, DLC; 53 orders total).
- Validation (scripted, python3 + PyYAML 6.0.1 — already installed, no
  pip needed): YAML parses clean; 9 distinct catalogue commands used
  across 26 command orders, EVERY name found in live/commands.json and
  every arg name checked against that command's constructor schema —
  zero errors; GameConsoleCmd absent (banned in globals); every bare
  research node id (11 distinct) found in coi/data/research_ids.txt;
  every product id (12 distinct) found in live/products.json; every
  phase has entry gate, exit gate, checks and fallbacks.
- Uncertainty carried, not laundered (23 flagged elements, 5 distinct
  placeholder research nodes): Construction, Concrete Production,
  Synthetic Rubber, Construction II, Groundwater Pump have NO sourced
  game id in research_ids.txt (candidates listed per placeholder —
  e.g. ResearchConcreteAdvanced explicitly rejected as a substitute for
  Concrete Production). Id-ambiguity flags on ResearchPowerAndMaintenance
  (vs ResearchPowerMaintenance), ConveyorBelts, ShipDockRepair;
  TradeDock/VehicleAndMining id-vs-display spelling flagged; starter
  [single-source] items flagged (Basic Diesel form, Power & Maintenance
  order variant, Cargo Depot ship-discovery gate). Starting-area
  details kept as plan assumptions with status: hypothesis.
- Known gaps listed in the format doc (§11, G1–G10): no completed-node
  list in /coi/v1/research (research.completed = executor-observed
  only); no power route (no MW predicates — census proxies only); no
  Product_Maintenance (maintenance proxied); construction placement
  semantics untested (all builds are place-blueprint/manual-step;
  CreateNewTrain{Network,Line}Cmd orders carry
  requires_sandbox_validation); edicts read-only (ToggleEdictEnabledCmd
  not in the web catalogue); no Unity read/spend; priority/slider
  values write-only; quick-trade providers unenumerated; refugee
  acceptance manual-only.
- Nothing run against any game; files only, per the overnight mandate.

## [21:13 Paris] Bridge recon round 2
- (a) Per-entity priorities — RESOLVED. General priority lives on
  `LayoutEntity.GeneralPriority` behind `IEntityWithGeneralPriority`
  (default = `Prototype.Costs.DefaultPriority`, per-proto); the census
  now carries a non-default-only `"priorities"` map and storage rows
  carry `generalPriority`. Custom priorities have no id-enumeration
  surface (Storage's `GetCustomPriority` Assert.Fails on unknown ids),
  so only Storage's typed `ImportPriority`/`ExportPriority` are read,
  onto storage rows. Full trail in bridge/MODDING-NOTES.md.
- (b) Storage sliders — RESOLVED. No stored step integers exist; the
  concrete `Storage` class exposes `ImportUntilPercent` /
  `ExportFromPercent` / `TransportFromPercent` / `TransportUntilPercent`
  and each step is exactly 10% (`step = RawValue / 10000`). Storage rows
  gain `importStep`/`exportStep`/`transportFrom`/`transportUntil` plus
  the four `*PercentRaw` values. (`StorageBase`, which the scan walked
  before, has none of this — the cmd processor also resolves concrete
  `Storage`.)
- (c) Quick trades — RESOLVED. `WorldMapManager.AllQuickTrades`
  (`IIndexable<QuickTradeProvider>`, manager is a global dependency)
  is the owner; new Guard-wrapped read-only route
  `GET /coi/v1/quick-trades` in BridgeRoutesV6 (WorldMapManager
  constructor-injected) returns id, live buy/pay product+quantity,
  `upointsCostRaw` (`UpointsCost.Value.RawValue`, Fix32 raw verified),
  currentStep, maxSteps, isSoldOut, hasPriceIncreased. CanAfford not
  called (out-param signature in this build; not needed).
- v0.6.1 BUILT AND STAGED: `bash bridge/vm-build/build.sh` finished
  0 errors / 0 warnings; `dist/CoiBridge-mod-0.6.1.zip` contains
  CoiBridge/{CoiBridge.dll, manifest.json (version 0.6.1), readme.txt
  with WHAT'S NEW IN v0.6.1}; new route/field strings verified present
  in the compiled DLL. ModVersion const + manifest + readme all bumped
  to 0.6.1; route table lists /coi/v1/quick-trades. NOT uploaded
  anywhere; nothing run against the live game (offline recon + compile
  only). All bridge code remains read-only and Guard-wrapped.

## [21:16 Paris] Final reconciliation (coordinator)
- Verification pass over ALL deliverables: PASS on every check —
  3 skills present with valid skill.yaml (names + include_in_prompt);
  fresh-start-v1.yaml parses (8 phases, command references valid,
  no GameConsoleCmd); GAME-PLAN-FORMAT.md 38.9 KB;
  dist/CoiBridge-mod-0.6.1.zip staged with manifest version 0.6.1;
  live/diagnosis-v1.md present; coi.exe in dist/ untouched tonight
  (no companion rebuild was in the queue).
- Cross-chunk unblocking: recon round 2 resolved TWO of the
  game-plan format's known gaps. G7 (priority/slider values
  write-only) and G8 (quick-trade providers unenumerated) are now
  marked RESOLVED in GAME-PLAN-FORMAT.md with the v0.6.1 routes
  named. Remaining executor work when Philip returns: companion
  predicate support for the new bridge fields.
- Still genuinely open (nothing more tonight can close them):
  construction placement semantics + edict actuation (both need a
  sandbox save and Philip awake); G1 completed-research list (a
  small bridge addition — ResearchedNodes enumeration — candidate
  for v0.6.2 with Philip's say-so); G2 power route (same).
- NO uploads made. NO applies to the live game. Deliverables staged:
  dist/CoiBridge-mod-0.6.1.zip (new), plans/, GAME-PLAN-FORMAT.md,
  live/diagnosis-v1.md, three skills in ~/workspace/custom-skills/.
  Upload window opens ~2026-11-03 21:07 Paris or on Philip's word.

## [06:52 Paris] CoiBridge v0.6.2 — G1 + G2 closed (post-overnight follow-up)
- G1 completed-research list: `/coi/v1/research` now also returns
  `"researched":[...]` — the completed node ids enumerated from
  `ResearchManager.ResearchedNodes` (`IIndexable<ResearchNode>`,
  harness-verified; same element type the route already iterates
  for the queue). `researchedCount` unchanged.
- G2 power route: NEW `GET /coi/v1/power` in BridgeRoutesV6
  (Guard-wrapped, read-only, resolves in the same own-try/catch V6
  isolation). Backing surface `ElectricityManager`
  (Mafi.Core.Factory.ElectricPower, [GlobalDependency AsEverything],
  constructor-injected). Fields (this-tick, exact kW —
  `Electricity.Value` is kW by construction: `OneKw = new
  Electricity(1)`, ilspy-verified): generatedKw, consumedKw,
  demandedKw, surplusDemandedKw, generationCapacityKw,
  maxGenerationCapacityKw, wastedKw, balanceKw (generated −
  consumed), generators, consumers.
- Battery/storage: NO surface exists — name recon over both
  assemblies found no Battery/ElectricityStorage type; thermal
  storage is a product buffer (already visible via product stats).
  Route omits it; documented in MODDING-NOTES recon round 3.
  ElectricityAvgStats averages deliberately not decoded (same
  discipline as the v0.5 train counters).
- Version discipline: manifest.json + ModVersion const + readme
  all at 0.6.2 (readme gained a WHAT'S NEW IN v0.6.2 section, the
  /coi/v1/power route line, and the research completed-ids note).
  Route table (debug/services) now lists GET /coi/v1/power.
- Build: ensure-toolchain TOOLCHAIN_OK; `bash bridge/vm-build/
  build.sh` finished 0 errors / 0 warnings; staged
  dist/CoiBridge-mod-0.6.2.zip (18,535 B) with root layout
  CoiBridge/{CoiBridge.dll, manifest.json (0.6.2), readme.txt}.
  Compiled-DLL string check (UTF-16): `/coi/v1/power`,
  `"researched":[`, generatedKw, balanceKw, surplusDemandedKw,
  maxGenerationCapacityKw, `0.6.2` all present.
- GAME-PLAN-FORMAT.md: G1 and G2 rows now carry RESOLVED
  blockquotes in the G7/G8 style. MODDING-NOTES.md gained the
  recon round 3 section (surfaces, kW calibration, battery
  finding, undecoded-averages note).
- Constraints kept: NO upload (zip staged in dist/ only, window
  runs to ~21:07 Paris today); nothing run against Philip's live
  game (offline recon + compile only); no messages sent.
  Remaining companion-side work when Philip returns: predicate
  support for `researched` + the power fields in the executor.

## [15:55 Paris] Round 3 kicked off (Philip in London, no PC — "do much more")
- New mandate ~15:54 Paris: much more building, PC still unavailable.
  Constraints unchanged: NO uploads (window to ~21:07 Paris), NO live
  applies (PC off — tailnet GETs expected to fail; work from live/
  captures).
- Queue: (1) companion UI for all bridge data — power panel, quick-
  trades panel, storage sliders/priorities, researched list — + exe
  rebuild; (2) supervised play-loop engine, dry-run only (after 3a);
  (3a) dragon-isle-continuation-v1.yaml; (3b) diagnosis-v1 fixes;
  (4) SANDBOX-PROTOCOLS.md; (5) playing-skill update + SPEC addendum.
- Wave 1 fanned out to four subagents (chunks 1, 3a, 3b, 4). Chunk 2
  sequenced after 3a so it can dry-run BOTH plans; it also avoids
  coi-crate contention with chunk 1.

## [16:00 Paris] Round 3 chunk 1 DONE — companion UI for all bridge data
- serve.rs: Overview power panel (/coi/v1/power, exact kW fields);
  Trade quick-trades panel with QuickTradeCmd Prepare buttons
  (arg "tradeId" verified in commands.json; upointsCostRaw shown
  raw, no speculative Fix32 decoding); Entities storage rows gain
  conditional slider steps (importStep/exportStep/transportFrom/
  transportUntil + *PercentRaw) and priorities (keys are camelCase
  importPriority/exportPriority per the C# emitter); new Non-default
  priorities panel (census priorities map = entityId -> general
  priority, resolved via topology); Research page shows the
  v0.6.2 "researched" list via pretty_research().
- Degradation fix: Overview/Entities/Issues now return 200 with a
  notice when the game API is down (was 502) — verified live in the
  scratch test: /, /trade, /entities, /research all 200, /issue?x=
  303->200, no panics (fresh-DB login is mode=setup + password2).
- Builds: Linux debug 0 errors/0 warnings; Windows release staged
  dist/coi.exe = 4,972,701 B (was 4,946,629). TOOLCHAIN_OK.
- Caveat: populated rendering against a live v0.6.2 bridge is NOT
  yet exercised (PC off) — eyeball on the next live session.

## [16:01 Paris] Round 3 chunk 4 DONE — SANDBOX-PROTOCOLS.md
- 1,099 lines / ~54 KB, documentation only; nothing fired anywhere.
- Protocol A (CreateStaticEntityCmd, 447 lines): test proto
  StorageUnit (id verified by 8 placed instances in the topology
  capture; cheapest tier, no research gate, observable on census +
  topology + storages). Distinguishes centre-tile vs footprint-
  origin and both transform rotation forms from the commands.json
  union schema; charging split into placement vs delivery reads;
  suppression tested as a false/true pair on a guaranteed overlap;
  revert via StartDeconstructionOfStaticEntityCmd (fallback
  QuickRemoveEntityRefundCmd) — names copied from commands.json.
- Protocol B (ToggleEdictEnabledCmd, 323 lines): runtime selection
  rule (isEnabled=false AND canBeEnabled=true, lowest Unity cost
  read in-game — the v0.6 edicts route has no cost field); a control
  send first captures the canonical success envelope so rejection
  vs execution is distinguishable; hypothesis body labelled, with a
  pre-committed arg-name variant ladder and readbacks between rungs
  (never double-toggle); revert = identical re-send.
- Both protocols abort if game-info.gameId == 5583213724730880438
  (the Expanding guard). Unresolvable-by-design unknowns listed in
  the doc's appendix (rotation effect via API, non-overlap
  validation classes, Unity balance has no route, transport
  placement, isActive lag, per-type generality).

## [16:01 Paris] Round 3 chunk 3b DONE — diagnosis-v1-fixes.md
- 1,115 lines / ~54.6 KB; diagnosis-v1.md untouched. Every id,
  quantity and layout name copied from the 2026-11-02 captures.
- 13 fault sections, classed: COMPANION ACTION x2 (F2 crude —
  StorageDisableLogisticsToggleCmd re-enable output on tanks 23639
  + 23640, input left disabled by design; F4 glass store 23447,
  causal step labelled [inference]); BLUEPRINT REPLACEMENT x1 (F5
  steel — steel-v1 from the pack, pause-don't-demolish, gated on a
  manual furnace input-buffer pre-check; pack layouts are untested
  in-game, caveat carried); MANUAL x2 (F1 demolish/inspect the 10
  unconnected transports by id+coords; F6 mixer states not mappable
  to ids from captures — blind toggles explicitly rejected);
  OBSERVE x7 with named watch fields + escalation triggers.
- F3 diesel/DieselReforming stays Philip's open decision: in-game
  recipe check across the 19 HydroCrackerT1 positions, then branch
  A (disable recipe per machine via verified commands) or branch B.
- No honest fix path from captures: F10 MaintenanceDepotT1 (no
  maintenance product exists in the vanilla data at all) and the
  oxygen branch of F5. Closing section lists what deliberately gets
  NO fix (112 single-tile transports, 36 other full output-disabled
  storages — unproven, 38 empty input-disabled, war-chest stores,
  seasonal corn, 9 unexplained pauses).

## [16:01 Paris] Round 3 chunk 3a DONE — dragon-isle-continuation-v1.yaml
- 8 phases, 36 orders, mirroring the relocation plan's phases but
  continuing from live state; new Phase 0 "stop the bleeding" for
  the steel stall + the DieselReforming DECIDE order (both cases
  evidenced, never assumed). Phase 1 war-chest entry gate evaluates
  TRUE against the captures (CP I 741/741, CP II 507/507, slabs
  7,205, coal 19,811) and says so in-line. Steel/diesel got NO
  PinToggle orders — both already pinned in the capture; toggling
  would unpin them.
- Validation: NEW tools/validate_plan.py; continuation PASS 0
  errors (10 command orders / 7 distinct commands, args vs
  commands.json schemas, 2 research ids, 12 product ids, no
  GameConsoleCmd); fresh-start re-validated PASS 0 errors (1 lint
  advisory on its AddVehicleToBuildQueueCmd sandbox flag).
- 21 uncertainty flags + 1 placeholder research id; top opens:
  DieselReforming decision; steel's binding link; save version
  0.8.7d (relocation plan) vs 0.8.8 (SPEC) conflict — executor
  refuses mismatch, human confirms; trains route-vs-census
  mismatch flagged (lines [] vs TrainDepot x1 + 30 H2 locos);
  Mega Vehicles research id does not exist (placeholder, manual);
  v0.6.2-only predicates (research.completed, power) UNEVALUABLE
  until the mod is installed — phases hold by design.

## [16:05 Paris] Round 3 chunk 2 DONE — play-loop engine (dry-run)
- Implementation: tools/plan_check.py (Python fallback — the coi
  crate has no YAML dependency and serde_yaml is NOT in the local
  cargo cache, so a Rust `coi plan-check` cannot be built offline;
  PyYAML 6.0.1 used). Imports are argparse/json/pathlib/sys/yaml
  only — no HTTP/socket/subprocess path exists. Reports carry the
  DRY RUN banner head and foot.
- Semantics per GAME-PLAN-FORMAT.md: state view assembled from the
  individual captures (the bundled snapshot lacks bridge routes);
  field mapping documented in-script (stats windows, percent
  raw/1000, net_flow = produced - consumed, etc.); three-valued
  Kleene logic; active phase per S9; next orders rendered as exact
  command + args (would-prepare) or kind + description, with held
  orders shown with reasons.
- Continuation dry-run: Phase 0 COMPLETE (exit TRUE — steel 1,108,
  diesel 5,992); ACTIVE = phase-1-war-chest-spine; next orders
  include StorageSetSliderStepCmd keep-full on the steel buffer.
  Fresh-start vs the mature state: coherent, Phase 1 ACTIVE with
  orders held on S4.5 placeholders — no panics, as expected.
- Sanity: war-chest entry TRUE (CP I/II 100%, slabs 7,205, coal
  19,811 — with a noted nuance: bridge storage rows aggregate
  500/500 per product while stats totals read 741/741 + 507/507;
  both 100%, gate unaffected). ALL research.completed predicates
  UNEVALUABLE (captures predate v0.6.2) and the power.balance_kw
  check UNEVALUABLE — correct blindness, not errors. Negative test
  (bogus metric/product/field) -> clean UNEVALUABLE lines, exit 0.
- Interpretations recorded in both reports: `for:` durations make
  an instantaneous TRUE read UNEVALUABLE on a single snapshot;
  phase COMPLETE is purely exit-gate TRUE (S7/S9 re-derivation);
  out-of-vocabulary metrics -> UNEVALUABLE + lint note.
- Reports: plans/dragon-isle-continuation-v1.dryrun.md,
  plans/fresh-start-v1.dryrun.md. validate_plan.py still passes
  both plans (files unmodified). The CLI addition rides the next
  exe build — dist/coi.exe (chunk 1) does not contain plan_check
  (it is a tools/ script, run on the VM).

## [16:10 Paris] Round 3 chunk 5 DONE + final reconciliation (coordinator)
- local-coi-playing skill updated: bridge route list gains
  quick-trades + power (+ researched note + website coverage);
  Game Plans section now documents both plan instances,
  tools/validate_plan.py, the plan_check.py dry-run engine (usage,
  UNEVALUABLE discipline, the three recorded interpretations), the
  diagnosis assets, and SANDBOX-PROTOCOLS.md; Operating Rule 3
  points at the protocols.
- SPEC.md gained Addendum IX (Round 3 summary).
- FINAL VERIFICATION: all Round-3 deliverables on disk —
  dist/coi.exe 4,972,701 B; plans/dragon-isle-continuation-v1.yaml
  + both .dryrun.md reports; tools/plan_check.py +
  tools/validate_plan.py; live/diagnosis-v1-fixes.md (diagnosis-v1
  untouched); SANDBOX-PROTOCOLS.md (54,319 B).
- ROUND 3 COMPLETE: chunks 1, 2, 3a, 3b, 4, 5 all done, nothing
  blocked. Standing opens unchanged: populated-UI eyeball + mod
  v0.6.2 install + sandbox protocols all need Philip's PC;
  DieselReforming decision is Philip's; uploads wait for ~21:07
  Paris or his word.

## [16:52 Paris] Round 4 kicked off (Philip in London, PC off — "keep going")
- Queue: (1) Rust plan engine `coi plan-check` + parity gate ·
  (2) website /plans page · (3) early-game fixture + fresh-start gate
  tests on BOTH engines · (4) decoder recon pack (docs only) ·
  (5) blueprint reference audit · (6) SPEC addendum X + playing-skill
  update. Constraints unchanged: NO uploads (window to ~21:07 Paris),
  NO live contact (PC off), NO applies ever this round.
- Fan-out: three subagents (decoder recon, blueprint audit, fixture);
  the Rust engine + /plans page stayed with the coordinator (parity-
  sensitive, same-crate files).

## [16:56 Paris] Round 4 chunk 1 DONE — Rust plan engine + PARITY PASS
- tools/plan_to_json.py (new): canonical JSON form of a plan —
  PyYAML safe_load (the same parser both Python tools use) -> sorted
  keys, 2-space indent; dates -> ISO strings, non-string keys
  stringified. Emitted plans/fresh-start-v1.json (65,045 B) +
  plans/dragon-isle-continuation-v1.json (75,492 B).
- coi/src/plan.rs (new, ~1,050 lines) + main.rs wiring:
  `coi plan-check <plan.json> [--state-dir DIR] [--out report.md]`,
  default state dir ./live; .yaml input refused with a converter
  hint. A faithful port of tools/plan_check.py: same State sources,
  same three-valued Kleene semantics, same `for:` handling, same
  counting rules, same active-phase derivation, same would-prepare
  rendering. Python's numeric tower is honoured (100 == 100.0) —
  serde_json's raw Number equality would have diverged there.
- PARITY GATE: new tools/check_plan_parity.py extracts the verdict
  skeleton (phase statuses, combined + per-item gate verdicts, phase
  and global check verdicts, invariants, summary counts, next-order
  statuses) from the stored Python reports and a fresh Rust run.
  Result: PARITY PASS on both plans — fresh-start 44 gate items,
  counts (30 T / 21 F / 14 U); continuation 49 gate items, counts
  (32 / 33 / 8). Zero divergences, so nothing to document beyond two
  cosmetic ones: plan meta maps render as JSON (Python: dict repr)
  and the generator footer names `coi plan-check` instead of
  tools/plan_check.py. Verdicts are unaffected by both.

## [16:58 Paris] Round 4 chunk 2 DONE — website /plans page
- coi/src/serve.rs: GET /plans (+ nav link). The two canonical plan
  JSONs and plans/verdicts.json are baked in via include_str! — no
  runtime file dependencies in the single-file exe. New generator
  tools/gen_verdicts.py computes the verdicts with plan_check.py's
  own machinery (imported as a module): per-plan phase statuses,
  active phase, and its orders with held reasons + args split into
  concrete vs §6.2 selector args.
- The page renders each plan's phase table + plan status and, for
  the active phase's WOULD-PREPARE command orders, a form posting
  to the EXISTING generic prepare flow (POST /actions/prepare-
  generic): concrete args as hidden fields, selector args as
  labelled text inputs (selector shown; human types the entity id
  from Entities — selectors are NOT auto-resolved, and the page
  says so), then the usual exact-JSON confirm step. Both plans
  have exactly one preparable order each: fresh-start p1-resume-
  time (GameSpeedChangeCmd, speed 1) and continuation
  p1-steel-buffer-keep-full (StorageSetSliderStepCmd, importStep
  10 + storageId selector). HELD orders render with reasons, no form.
- The verdict panel states its provenance: verdicts are as of the
  2026-11-02 captures, not live state.
- Verified on a scratch server (port 8486, scratch DB, game
  unreachable): /plans 200 (11,605 B), both forms present, selector
  input + hidden importStep=10 present; submitting the slider form
  reached the Confirm-command page with the exact JSON and the
  "could not read current game state" warning — finish_prepare's
  unreachable-game tolerance holds. Nothing was applied (no game).

## [17:00 Paris] Round 4 chunk 3 DONE — early-game fixture, both engines
- plans/fixtures/early-game/ (subagent): 9 capture-shaped files +
  README. A plausible month-1 New Haven game at date 1-1-18:
  population 100 / housing 160, monthsOfFood 4 (300 food vs 80/mo
  burn), researchedCount 0 with a v0.6.2-style `researched: []`
  (so research.completed reads FALSE honestly, not UNEVALUABLE),
  starter census only (smelter/casters/coal maker/generator/labs,
  no maintenance depot or assemblies yet — phase-2 builds correctly
  absent), iron 42 as the deliberate phase-1 blocker, diesel 220,
  trains {"lines": []}, 4 storage rows, internally consistent
  (stored <= capacity; lifetime produced-consumed = quantity).
- Python verdict (plans/fresh-start-v1.early-game.dryrun.md, exit
  0, DRY RUN banner): phase 0 COMPLETE, phase 1 ACTIVE, phases 2-7
  NOT REACHED with every late exit gate FALSE — no nonsense TRUEs,
  no crashes. Counts (21 T / 34 F / 10 U).
- Rust verdict (plans/fresh-start-v1.early-game.rust.dryrun.md):
  FULL parity with the Python report on every extracted category
  (statuses, combined + item gates, checks, invariants, orders,
  counts) — a third parity surface beyond the two live/ plans.
- Findings (documented, semantics left as-is — changing gate
  semantics is a format decision, not an engine fix):
  (1) phase-4's p4-diesel-net-flow check reads WOULD FIRE in month
  1: the predicate has no "diesel chain exists" guard, so ordinary
  starting-stock drawdown (-30 thisMonth) trips it. Checks only run
  in their own phase per §7.1, so in a real run this is report
  noise, not a false action — noted for plan authoring.
  (2) `net_flow lastMonth >= 0` predicates read TRUE on 0-0 empty
  history (phases 3/4/5 gates). No composite gate passed because of
  it (research/stock siblings block; phase-5 entry reads
  UNEVALUABLE via a sibling, not FALSE) — a zero-history caveat
  plan authors must know: lastMonth windows are vacuous in month 1.
  (3) REAL engine bug found + fixed in BOTH engines: the reports'
  hardcoded state-mapping note claimed research.completed is
  UNEVALUABLE (pre-v0.6.2 wording) even when the capture HAS a
  `researched` list. The note is now conditional (names the list
  and its length when present). Prose-only: all verdicts/counts
  re-verified unchanged after the fix (live parity PASS, fixture
  counts still 21/34/10, validate_plan still PASS 0 errors).
- Both plans re-validated (tools/validate_plan.py): PASS, 0 errors.

## [17:00 Paris] Round 4 chunk 5 DONE — blueprint reference audit
- tools/audit_blueprints.py (new sibling; validate_plan.py
  untouched): extracts every blueprint_ref from both plans and
  checks it against the pack's id set (79 ids: layout file stems +
  PACK_MANIFEST/module ids; "exists" defined in the docstring).
- Result: 20 refs audited — ALL 20 are logical district names with
  asset_status: not-yet-authored (MISSING-DECLARED, the honest
  expected state: the plans' place-blueprint orders degrade to
  manual steps until assets are authored). ZERO missing-undeclared,
  zero exact/leaf matches, exit 0. No plan edits were needed or
  made — there was no unambiguous wrong reference to fix.

## [17:02 Paris] Round 4 chunk 4 DONE — decoder recon pack
- bridge/MODDING-NOTES.md gained "Recon round 4" (~13.4 KB), header
  states PREPARATION ONLY — NOT VERIFIED DECODING; every claim
  marked VERIFIED-FROM-ASSEMBLY or INFERRED. Offline recon only.
- (a) ElectricityAvgStats (real home: Mafi.Core.Stats): sealed,
  inherits ItemAvgStats<Electricity>; raw long IS kW, unscaled
  (OneKw = new Electricity(1)); four RleSequence history rings
  (55-bit value + repeat count, newest-first); Avg semantics: the
  scalars collapse to the latest sample, monthly = integer avg of
  last 30 daily, yearly = avg of 12 monthly, 25-year = MAX of 25.
  Key open question for the live cross-check: a daily bar is the
  last tick sample before the boundary, NOT a time average.
  Public read surface enumerated (GetLatestData etc.).
- (b) Trains: NO ActivityStats/TravelStats types exist — the
  counters are the shared ProductivityCounter family on Train.
  One bucket = 600 reports; Data = 4x ushort unioned with ulong;
  activity A/B/C/D = Working/Idle/Refueling/CannotWork, travel
  A/B/C/D = FullSpeed/Accel+Brake/SpeedLimit/BlockedByTrains;
  classify rules + constants recovered; panel uses TWO different
  denominators (categorised total for headlines, 600 for charts).
  6-point live cross-check list written for the future decode.
- (c) Custom priorities: the hoped-for *Priority*Proto does NOT
  exist (zero proto types with "Prio" in three assemblies). The id
  universe is 7 hardcoded strings across exactly 6 implementer
  classes (Storage pair ImportPrio/ExportPrio, depot fuel pair,
  CargoExportPrio, shipyard's five). GetCustomPriority AND
  SetCustomPriority Assert.Fail on unknown ids in all five direct
  implementers; IsCustomPriorityVisible never asserts (false =
  n/a) — the only safe probe. Future enumeration = compile-time
  table keyed by concrete class, as v0.6.1 did for Storage.
  (Global pair ConstructionPriority/DeconstructionPriority is a
  separate manager and errors cleanly instead of asserting.)

## [17:04 Paris] Round 4 chunk 6 DONE + final reconciliation
- SPEC.md gained Addendum X (Round 4 summary).
- local-coi-playing skill updated: the "Rust plan-check was not
  built" line replaced with the engine + converter + parity gate;
  new bullets for the /plans page, the early-game fixture and its
  two authoring caveats, the blueprint audit, and the decoder
  prep (incl. the IsCustomPriorityVisible-only probing rule).
- FINAL VERIFICATION: dist/coi.exe 5,306,248 B (release, embeds
  plan-check + /plans + baked plans/verdicts); parity gate PASS;
  validate_plan PASS (0 errors, both plans); audit_blueprints
  exit 0; fixture + both-engine reports in plans/; MODDING-NOTES
  recon round 4 in place; tools/ gained plan_to_json.py,
  gen_verdicts.py, check_plan_parity.py, audit_blueprints.py.
- ROUND 4 COMPLETE: chunks 1-6 all done, nothing blocked.
  Standing opens unchanged: everything live (mod install, UI
  eyeball, sandbox protocols, DieselReforming decision) waits for
  Philip's PC; uploads wait for ~21:07 Paris or his word.

## [17:56 Paris] Round 5 chunk 3 DONE — CoiBridge v0.6.3 (bridge side)
- Power(): /coi/v1/power gains "averages" (production / consumption /
  generationCapacity, each lastDayKw/lastMonthKw/lastYearKw from the
  ElectricityAvgStats typed props LastDay/LastMonth/LastYear, raw
  .Value = kW) plus sibling "averagesUnverified": true. Decoding
  follows recon round 4 layout (monthly = rollup avg of daily
  samples; daily = last-tick sample) and is UNVERIFIED — no live
  cross-check has been possible; recon round 4 (a) check list stands.
- NEW route GET /coi/v1/priorities in BridgeRoutesV6 (IEntitiesManager
  added as 7th ctor param; route added to s_routeTable): walks
  non-destroyed entities implementing IEntityWithCustomPriority,
  probes the 7 ids ONLY via IsCustomPriorityVisible (never asserts),
  calls GetCustomPriority ONLY for visible=true ids. Output
  {"ids":[7 ids],"entities":[{"id","proto","priorities":{visible
  ids only}}]} — entities with >=1 visible id only. No setters.
- Version discipline: ModVersion const + manifest.json both 0.6.3;
  readme header / What's-new v0.6.3 / routes list updated;
  MODDING-NOTES.md gained "v0.6.3 implementation" note.
- BUILD: bridge/vm-build/build.sh — 0 errors / 0 warnings.
  Staged dist/CoiBridge-mod-0.6.3.zip (19,656 B), layout verified
  CoiBridge/{CoiBridge.dll, manifest.json, readme.txt}, manifest
  inside reads 0.6.3. No uploads, no live-game contact.

## [17:56 Paris] Round 5 chunks 2 + 3-companion DONE — bare-exe launch + power averages display
- Chunk 2 (coi/src/main.rs): bare `coi` with NO subcommand no longer
  defaults to the one-shot `status`. New `cmd_bare_launch`: probe-
  binds 127.0.0.1:8484 first — on AddrInUse it prints "CoI Companion
  appears to be already running at http://127.0.0.1:8484/", still
  opens the browser, and exits 0; on any other probe error it falls
  through to serve so serve's own bind error surfaces. Normal path
  prints a friendly banner (website starting, where, set-password
  on first visit, keep window open), best-effort opens the default
  browser (new `open_browser`: Windows `cmd /C start "" <url>`,
  macOS `open`, Linux `xdg-open`; spawn-and-ignore, failure never
  stops the server), then runs `serve::cmd` with default options
  (global flags such as --db already parsed are respected).
  Explicit subcommands unchanged. usage() updated: USAGE now shows
  `[command]`, a no-command behaviour line + `(none)` entry added,
  and `status` no longer marked "(default)".
- README.md quickstart updated to the real flow: double-click
  coi.exe → browser opens to the login page → set password on first
  visit; `coi serve` kept as the terminal alternative; already-
  running behaviour noted.
- Chunk 3-companion (coi/src/serve.rs `power_panel`): when the
  bridge power payload carries an `averages` object (CoiBridge
  v0.6.3), a second sub-line is appended showing produced /
  consumed / generation-capacity averages for last day / last
  month / last year in kW, worded "Averages via CoiBridge v0.6.3 —
  decoding UNVERIFIED: produced ≈X kW last day · …" (missing values
  render as —). When `averages` is absent (older mod) the panel
  renders byte-for-byte as before (empty extra string).
- Build: `cargo build` (debug, stable toolchain PATH) — 0 errors,
  0 warnings in the tail output.
- Bare-launch test on this VM (scratch DB /tmp/coi-bare-test.db,
  cleaned up after): `timeout 6 ./target/debug/coi --db
  /tmp/coi-bare-test.db` with no subcommand printed the banner, then
  serve's own start line, and kept serving (timeout had to kill it,
  wrapper exit 124 — NOT one-shot status output); `curl
  http://127.0.0.1:8484/login` during the window returned 200 with
  the login page HTML. xdg-open was attempted and failed gracefully
  on this headless VM ("no method available") without affecting the
  server, as designed. Port-taken path: a second bare instance
  while the first was serving printed the already-running message,
  attempted the browser open, and exited 0; the first instance
  still returned 200 on /login afterwards. Explicit `status` with
  the game off still takes the old one-shot path (exit 1, game
  connection refused) — unchanged. No Windows exe built (coordinator
  does the final build), no uploads, guides.rs and bridge/ untouched.

## [17:57 Paris] Round 5 chunk 5 DONE — three new guides (contracts-trade, loans, edicts)
- coi/src/guides.rs: GUIDES 7 → 10 entries, appended only, existing
  guides untouched. contracts-trade "Contracts & trade — the world
  market without surprises" (5 sections), loans "Loans — borrowed
  product, repaid product" (4 sections), edicts "Edicts — enabled is
  not the same as active" (4 sections). Failure-mode framing and
  closing Sources lines match the existing style.
- Grounding discipline held: no invented numbers. Contracts from
  BridgeRoutesV6 ContractProto fields + ToggleContractCmd toggle
  semantics + live toBuy + raw profit multiplier; quick trades from
  QuickTradeProvider step/maxSteps/isSoldOut/hasPriceIncreased +
  raw unity cost + QuickTradeCmd by id. Loans: interest/fee as
  percents via the Trade page rendering, maxActiveLoans, active-loan
  fields, AcceptLoanCmd / MakeLoanOverpaymentCmd /
  SetLoanBufferPriorityCmd schemas — and credit score stated plainly
  as a RAW Fix32 whose human-scale decoding is UNVERIFIED. Edicts:
  route state + the game's own reason strings only; no cost/tier
  claims (route carries neither); actuation caveat prominent —
  ToggleEdictEnabledCmd exists in-game but is absent from the web
  API's 240-command registry, toggling stays in-game until
  SANDBOX-PROTOCOLS.md Protocol B passes on a sandbox save.
- guides_for_issue lives in serve.rs (NOT guides.rs) — serve.rs not
  edited (another worker owns it). Suggested mappings for its owner:
  issue titles containing "contract" -> contracts-trade; "loan" /
  "payment due" -> loans; "edict" -> edicts; product-deficit/draining
  issues whose product matches an active contract's payment product
  or an active loan's product -> also push the matching trade guide.
- VERIFY: cargo check in coi/ PASS, zero errors/warnings in the
  tail (guides.rs, serve.rs and main.rs all clean at check time).
- No uploads, no live-game contact.

## [17:58 Paris] Round 5 chunk 1 DONE — plan-semantics guards applied (net_flow vacuity + diesel chain)
- THE GUARDS (Round 4 documented findings 1+2 in plans/fixtures/early-game/README.md; this chunk APPLIES them):
  (b) HISTORY/vacuity guard on every `product.net_flow` leaf with `window: lastMonth` in BOTH plans (no `lastYear` net_flow leaf exists in either plan): leaf L wrapped as `all: [ {any: [produced(P,lastMonth)>0, consumed(P,lastMonth)>0]}, L ]`; a leaf's `for:` was MOVED onto the composite (§4.3); each item gained/extended a `note:` naming the vacuous-window guard and why.
  (a) DIESEL CHAIN guard: fresh-start check `p4-diesel-net-flow` — its `when:` is now `all: [ {product.produced Product_Diesel lifetime > 0}, {net_flow thisMonth < 0} ]`, note extended. Quantity-based diesel floors/invariants were NOT touched (see below).
- ITEMS GUARDED — fresh-start (10× (b), all gate leaves): phase-2 exit ConstructionParts >=0 (for:{months:1} moved to composite); phase-3 entry ConstructionParts >=0; phase-3 exit IronOre >0; phase-3 exit Iron >=0; phase-4 exit Diesel >=0; phase-5 entry Diesel >=0; phase-5 exit Copper >=0; phase-6 exit ConstructionParts2 >=0; phase-7 entry ConstructionParts2 >=0; phase-7 exit IronOre >0. Plus 1× (a): check p4-diesel-net-flow. The two fresh diesel lastMonth gates got (b) only: (a) is redundant there — activity-guard TRUE plus net_flow >= 0 mathematically entails produced > 0 in the window, so the chain-exists condition is already implied when the composite passes.
- ITEMS GUARDED — dragon-isle continuation (10× (b), all gate leaves): phase-2 exit ConcreteSlab >=0 (for moved); phase-3 exit IronOre >=0 (for moved); phase-4 entry IronOre >=0; phase-4 exit Steel >0 (for moved); phase-5 entry Steel >=0; phase-5 exit Fertilizer >=0 (for moved); phase-6 entry Steel >=0; phase-6 exit HouseholdGoods >=0 (for moved); phase-7 entry Steel >=0; phase-7 exit VehicleParts2 >=0. Guard (a): NONE applied — the continuation plan contains NO diesel net_flow predicate at all (its chain provably exists; every diesel predicate there is a product.quantity floor/invariant). Existing notes on continuation items were preserved verbatim with the guard sentence appended.
- DIESEL INVARIANT CONFIRMATION: fresh-start `globals.invariants` diesel entry is quantity-based — `{metric: product.quantity, product: Product_Diesel, op: ">", value: 0}` (a stock floor, a real signal at any game age) — confirmed and left untouched, as were all diesel quantity floors/checks in both plans.
- VERSIONS: both plans 1.0.0 -> 1.1.0 (format §3.1: a gate edit is a version bump; predicate additions, no threshold or phase-structure change). All ids preserved. Plan JSONs regenerated via tools/plan_to_json.py (fresh 74,124 B; continuation 83,852 B).
- VALIDATION: tools/validate_plan.py PASS, 0 errors, both plans (only the pre-existing placeholder/lint-advisory warnings).
- PARITY: tools/check_plan_parity.py PARITY PASS on live/ for both plans (fresh: 44 gate items, counts 26 T / 26 F / 13 U; continuation: 49 gate items, 31 / 35 / 7). Fixture parity (extract-compare, Python report vs Rust report): PARITY OK, counts 15 / 41 / 9. Engines run: Python + Rust on fresh×live, fresh×fixture, continuation×live (continuation×fixture not sensible — different run). Dry-run reports regenerated: plans/fresh-start-v1.dryrun.md, plans/fresh-start-v1.early-game.dryrun.md, plans/fresh-start-v1.early-game.rust.dryrun.md, plans/dragon-isle-continuation-v1.dryrun.md; plans/verdicts.json regenerated via tools/gen_verdicts.py (only change vs Round 4: the two version strings; active phases unchanged).
- VERDICT CHANGES vs Round-3/4 reports — every flip traces to a guarded leaf whose window was exactly 0-0 (or duration-masked 0-0) in the source stats; no unexpected flips, no phase-status changes anywhere:
  FRESH x FIXTURE (counts 21/34/10 -> 15/41/9):
  1. phase-2 exit ConstructionParts (for:{months:1}): UNEVALUABLE -> FALSE — guard fails (0 produced / 0 consumed lastMonth); was duration-masked TRUE-on-0-0. Exit combined stays FALSE.
  2. phase-3 entry ConstructionParts: TRUE -> FALSE (0-0 vacuity). Combined stays FALSE.
  3. phase-3 exit Iron: TRUE -> FALSE (0-0). Combined stays FALSE.
  4. phase-4 exit Diesel: TRUE -> FALSE (0-0). Combined stays FALSE.
  5. phase-5 entry Diesel: TRUE -> FALSE (0-0); entry COMBINED UNEVALUABLE -> FALSE (the fixture README finding-2 knock-on is fixed: the vacuous TRUE no longer masks the sibling).
  6. phase-5 exit Copper: TRUE -> FALSE (0-0). Combined stays FALSE.
  7. check p4-diesel-net-flow: TRUE / WOULD FIRE -> FALSE / not firing — guard (a): lifetime diesel produced = 0 on the fixture, no chain exists; thisMonth -30 is starting-stock drawdown (fixture README finding 1 fixed).
  Guarded but unchanged on the fixture: phase-3 exit IronOre, phase-6 exit ConstructionParts2, phase-7 entry ConstructionParts2, phase-7 exit IronOre all stay UNEVALUABLE (products absent from fixture stats — the guard leaves are honestly UNEVALUABLE too).
  FRESH x LIVE (counts 30/21/14 -> 26/26/13):
  8. phase-2 exit ConstructionParts (for): UNEVALUABLE -> FALSE — live lastMonth is literally 0-0 for ConstructionParts; exit COMBINED UNEVALUABLE -> FALSE.
  9. phase-3 entry ConstructionParts: TRUE -> FALSE (0-0); entry COMBINED UNEVALUABLE -> FALSE.
  10. phase-3 exit Iron: TRUE -> FALSE (Iron lastMonth 0-0 on the live capture). Combined stays FALSE.
  11. phase-6 exit ConstructionParts2: TRUE -> FALSE (lastMonth 0-0); exit COMBINED UNEVALUABLE -> FALSE.
  12. phase-7 entry ConstructionParts2: TRUE -> FALSE (0-0); entry COMBINED UNEVALUABLE -> FALSE.
  Guarded but unchanged on live: phase-3 exit IronOre FALSE (real activity, net -29), phase-4 exit Diesel FALSE (net -28), phase-5 entry Diesel FALSE, phase-5 exit Copper TRUE (78-72, real activity), phase-7 exit IronOre FALSE; check p4-diesel-net-flow stays TRUE / WOULD FIRE on live — correct: lifetime diesel produced 423,589, the chain exists, thisMonth net -77 is a real deficit.
  CONTINUATION x LIVE (counts 32/33/8 -> 31/35/7):
  13. phase-6 exit HouseholdGoods (for): UNEVALUABLE -> FALSE — lastMonth 0-0 (lifetime 0-0 too); the duration wrapper no longer masks the vacuity. Combined stays FALSE.
  14. phase-7 exit VehicleParts2: TRUE -> FALSE — lastMonth 0-0 (its activity is lastYear only); exit COMBINED UNEVALUABLE -> FALSE.
  Guarded but unchanged: phase-2 exit ConcreteSlab stays UNEVALUABLE (real 8-4 activity, instantaneous TRUE, for-duration unprovable from one snapshot), phase-3 exit / phase-4 entry IronOre FALSE (net -29, real), all Steel items FALSE (net -46, real consumption), phase-5 exit Fertilizer stays UNEVALUABLE (30-15, for).
- NOTE: plans/fixtures/early-game/README.md's "Python engine verdict" section still quotes the pre-guard counts/findings — left untouched by this chunk; the regenerated reports + this entry are the current record (coordinator may refresh the README at reconciliation).
- Constraints held: offline data only, no live-game contact, no uploads, no messages.

## 2026-11-03 18:03 CET — Round 5 chunk 4: growth-sequence test (12 monthly fixtures)

- GENERATOR + FIXTURES: tools/gen_growth_fixtures.py written; produces plans/fixtures/growth/month-01/ … month-12/ in the exact early-game shape (9 files each). Month 1 = early-game world relabelled; months 2–12 from scripted full-month flow tables with derived quantities (start stock + lifetime net), mid-month snapshots (thisMonth ≈ ½ month, lastMonth = prior full month, thisYear = lifetime in year 1). Generator self-checks (non-negative quantities, stored ≤ capacity ≤ consistent, storage rows ≤ stats stored, percentFullRaw exact) PASS on all 12 months first run — no fixture bugs found, no regeneration needed.
- STORY FOLLOWED: plan's own build order — scrap starter → iron ≥100 (m3) → Construction done (m2, as ResearchCp1Production) → assemblies + maintenance depot (m3–4), CP balance non-negative on real lastMonth activity from m4 (+5, then +4/mo) → Power & Maintenance (m4) → coal mine (m5) → Vehicles & Mining (m6) → first iron ore + ore smelting (m7) → concrete chain (m9) → Basic Diesel (m10, chain online; diesel stock bottoms at 65 in m9, never breaching the >0 invariant) → rubber (m11) → Copper Refinement + copper district (m12). Pop 100 → 116 (m7) → 132 (m10); monthsOfFood 3–4 throughout.
- RUNS: Rust `coi plan-check` (plans/fresh-start-v1.json) on all 12 months → plans/fixtures/growth/reports/month-NN.rust.md. Python spot-checks months 1/6/12 → identical phase statuses AND per-phase entry/exit verdicts (parity PASS).
- FLIP SUMMARY: phase 0 COMPLETE m1; phase 1 COMPLETE m3; phase 2 HELD AT ENTRY (UNEVALUABLE) m3–12; phases 3–7 NOT REACHED all 12 months (status level). Gate level: phase-3 entry TRUE m4, phase-3 exit TRUE m8–12; phase-4 entry TRUE m6; phase-5 entry TRUE m11; phase-6 entry TRUE m12.
- VERIFICATION: (i) strict order PASS (status level); (ii) no COMPLETE regressions PASS; (iii) no TRUE on absent/UNEVALUABLE evidence PASS (phase-3 exit UNEVALUABLE m6 solely because IronOre absent; FALSE m7 on the 0−0 guard; CP2 never TRUE); (iv) firing checks explainable PASS (diesel floors during the 220→65→220 drawdown/rebuild; p6-transport-connectivity m9, 8/9 belts while the kiln line is built).
- FINDINGS (full text in plans/growth-sequence-timeline.md): (1) phases ≥2 can never flip in single-snapshot dry runs — phase-2 exit's CP leaf carries for:{months:1} (instantaneous TRUE → UNEVALUABLE in both engines) and phase-5 exit has the same cap; a snapshot sequence is the history a duration needs but neither engine consumes cross-snapshot history. (2) Phase-2 entry UNEVALUABLE all 12 months by plan design (§4.5 placeholder Research<Construction>; fixtures record completion under the candidate id ResearchCp1Production, which the gate never reads). (3) Phase 4 realistically needs ~14 months: at m12 its exit fails on exactly one leaf (diesel stock 220 < 300; concrete/diesel-flow/rubber all TRUE). (4) Fixture-choice disclosures: no concrete node in `researched` (game id unsourced; no gate reads it), Synthetic Rubber recorded as ResearchRubber (plan candidate, real id), census proto labels beyond early-game fixture are plausible labels (only gate-read one, MaintenanceDepotT1, is plan-flagged unconfirmed). (5) Diesel is the first year's binding constraint — crisis checks fire through the middle sequence exactly as the plan's bottleneck watch predicts.
- Constraints held: fixtures + generator + timeline + this log only; no plan/engine edits, no uploads, no live-game contact.

## [18:07 Paris] Round 5 chunk 6 DONE — reconciliation + final build + docs (coordinator)
- serve.rs guides_for_issue: added the Round-5 title-keyword
  mappings the guides worker proposed (contract / quick trade ->
  contracts-trade; loan / payment due -> loans; edict -> edicts),
  matched on the lowercased title; product-matching mappings
  deliberately not done (need bridge plumbing in the issue page,
  not a keyword rule).
- plans/fixtures/early-game/README.md verdict section refreshed
  for plan v1.1.0: totals 15/41/9 (was 21/34/10), findings 1-2
  marked RESOLVED (guards), finding 3 marked RESOLVED (Round 4),
  phase statuses noted unchanged.
- FINAL BUILD: ensure-toolchain OK; Windows release build clean;
  dist/coi.exe = 5,369,011 B (was 5,306,248) — embeds bare-exe
  launch, power-averages panel, 10 guides + mappings, plan
  v1.1.0 verdicts. dist/CoiBridge-mod-0.6.3.zip = 19,656 B.
- SPEC.md Addendum XI written (Round 5 summary incl. the
  growth-sequence duration finding + executor-history next step).
- Skills updated: local-coi-playing (v0.6.3 routes + UNVERIFIED
  averages, bare-exe launch line, parity counts at v1.1.0 +
  guard discipline, growth finding, decoder-prep status);
  local-coi-companion-dev (bare-launch default corrected).
- ROUND 5 COMPLETE: chunks 1-6 all done, nothing blocked.
  Constraints held: zero uploads, zero live-game contact, zero
  applies. Standing opens unchanged (mod install, populated-UI
  eyeball, sandbox protocols, DieselReforming decision) + new
  engine item: executor/cross-snapshot history for duration
  gates.

## Round 6 — 2026-11-03 evening (Philip in London, PC off; coordinator session)

Constraints held throughout: zero uploads, zero live-game contact,
zero applies, no messages to Philip. Deliveries wait for Thursday.

- ~18:32 CET — Chunk 1 (format semantics) DONE: GAME-PLAN-FORMAT.md
  §4.3.1 defines `for:`-duration evaluation over a state sequence
  (CoI calendar 12×30 days, 2880 ticks/day — fixture-verified;
  anchor-at-window-start rule; FALSE anywhere in window → FALSE;
  no anchor / inner UNEVALUABLE → UNEVALUABLE, never false-by-
  default; gaps never falsify; `for:{evaluations:n}` = last n
  snapshots, same rule). §9 cross-reference added.
- ~18:33 CET — Chunk 2 (Python engine) DONE: tools/plan_check.py
  gains sequence mode (`--state-seq DIR`, per-state subdirs sorted
  by in-game date; per-prefix phase derivation + "Sequence phase
  history" report section). Single-state regression: outputs on
  live (both plans) and the month-1 fixture are BYTE-IDENTICAL to
  the committed Round-5 reports.
- ~18:34 CET — Chunk 5 (fresh-start v1.2) DONE as the documented-
  candidates outcome: the phase-2 entry placeholder research id
  cannot be sourced from any project data (starter skill gives
  display names only; research_ids.txt gives ids only; no mapping
  exists anywhere in the repo). Plan bumped to v1.2.0 with the
  investigation recorded as a plan-level assumption and the sole
  candidate `ResearchCp1Production` documented with reasoning on
  both placeholder sites; NOT substituted (§4.5). validate_plan
  PASS; JSON regenerated; live verdicts unchanged (26/26/13).
- ~18:35 CET — Chunk 3 (Rust port) DONE: coi/src/plan.rs mirrors
  §4.3.1 exactly; grows `coi plan-check --state-seq`. Parity:
  live PARITY PASS (both plans); growth sequence — identical
  per-prefix phase tables, gate verdicts and counts (36/26/3)
  between engines. Edge fixtures verified in both engines:
  evaluations form (UNEVALUABLE below n snapshots, flips at n)
  and insufficient-history (UNEVALUABLE, never defaulted).
- ~18:36 CET — Chunk 4 (acceptance) DONE: on the growth sequence,
  phase 2's duration exit COMPLETES at month 5 (window reaches
  month 3's FALSE at m4; holds {m4,m5} at m5); phase 3 COMPLETE
  at month 8; phase 4 ACTIVE, blocked on ONE leaf (diesel stock
  220 < 300 — fixture-horizon limit, ~month 13-14); phases 5-7
  evaluate on their own gates, blockers named leaf-by-leaf.
  plans/growth-sequence-timeline-v2.md written; v1 file untouched.
  Sequence reports saved: plans/fresh-start-v1.growth-sequence
  .dryrun.md + .rust.dryrun.md.
- ~18:37 CET — Chunk 6 (diagnosis forward-compat) DONE:
  tools/diagnose_live.py uses v0.6.x fields when present —
  §3 splits out full-by-configuration storages (ImportUntil
  ~100%), §7 priorities (census map / storage fields / v0.6.3
  route capture), §8 power with a deficit finding. Real-capture
  output BYTE-IDENTICAL to live/diagnosis-v1.md. Synthetic
  enriched capture built at live-synthetic-v06/ (SYNTHETIC.txt
  marker + in-report banner): stuck 39 -> 38 with crude tank
  23639 explained, synthetic 900 kW deficit reported;
  live-synthetic-v06/diagnosis.md saved alongside.
- ~18:38 CET — Chunk 7 (docs) DONE: plans/verdicts.json refreshed
  (fresh-start v1.2.0; live verdicts unchanged) so the /plans page
  bakes current data; SPEC.md Addendum XII written; local-coi-
  playing skill updated (executor usage, sequence sources incl.
  the product-history non-qualification, parity gate extended to
  sequence mode, v2 flip sequence).
- ~18:39 CET — FINAL BUILD: toolchain OK; Windows release build
  clean; dist/coi.exe = 5,442,879 B (was 5,369,011) — embeds
  sequence-mode plan-check; verified the new code strings are in
  the staged binary; live parity re-run against the final build:
  PARITY PASS. dist/CoiBridge-mod-0.6.3.zip unchanged (19,656 B).
- ROUND 6 COMPLETE: chunks 1-7 all done, nothing blocked.
  Standing opens unchanged (mod v0.6.3 + new exe install and
  populated-UI eyeball on Philip's return Thursday, sandbox
  protocols, DieselReforming decision).

## Round 7 (2026-11-04, Philip in London, PC off — staged only)
- 14:18 CET — Chunk 1 (CoiBridge v0.6.4) DONE: NEW GET
  /coi/v1/machines — one row per machine instance {id, proto, pos,
  state, recipes[], isPaused, isEnabled, canBePaused} built inside
  the shared entity scan (same ~5 s cache as census/topology/
  storages); per-row try/catch with a "skippedRows" counter so a bad
  row can never poison the scan. /coi/v1/trains gains RAW per-train
  ProductivityCounter blocks (activity + travel: version, monthly
  ring in stored order, yearly ring, yearlyRolling, totalRollingHalf
  [halved per recon], ongoing bucket + ticks), "counterLabels" from
  the recon-round-4 label tables, and top-level
  "unverifiedDecoding": true; the counters block sits behind its own
  try/catch so a failure degrades to "trainsError" and never takes
  the lines payload. Surfaces verified by fresh recon before coding:
  TrainsManager [GlobalDependency(AsEverything)] + Trains
  IIndexable<Train>; Train.{Name, TrainId.Value (ushort), TrainLine,
  IsPaused, IsSpawned, TrainCarsCount, ActivityStats/TravelStats
  trios}; ProductivityCounterHistory publics + Data {CategoryA-D
  ushort fields, RawData, Total}; Entity.{IsPaused, IsEnabled,
  CanBePaused}. Build: 0 errors / 0 warnings; dist/
  CoiBridge-mod-0.6.4.zip = 21,767 B, manifest 0.6.4 in-zip, DLL
  strings confirm /coi/v1/machines + unverifiedDecoding + 0.6.4.
  Supersedes 0.6.3 for Thursday's install. Live behaviour pending
  Philip's PC (empty trains list on his save is CORRECT).
- 14:2x CET — Chunk 2 (diagnosis consumes machines) DONE:
  tools/diagnose_live.py gains §9, active only when a capture
  carries bridge-machines.json (v0.6.4 shape): per-proto instance
  rows vs census aggregates consistency check (MATCH/MISMATCH with
  per-state diffs), paused/disabled instance lists, and §5's
  flagged clusters resolved to instances — id, proto, pos, recipes,
  connected neighbours traced over the topology port edges
  (storages resolved to product + fill; direction labelled
  [inference], edges are undirected). Real capture: output
  BYTE-IDENTICAL to live/diagnosis-v1.md (route absent → zero
  change). Synthetic test: live-synthetic-v064/ = real capture +
  SYNTHETIC.txt + bridge-machines.json generated FROM the real
  census + topology (223 rows; census machine counts matched
  topology entities exactly for every proto, zero inconsistencies;
  one AssemblyT1 marked paused to exercise the paused path) —
  §9 reports MATCH, 46 starved + output-full instances listed with
  neighbours (e.g. Flares → naphtha storage 21022 at 248/500).
- 14:31 CET — Chunk 3 (diagnosis → blueprint linkage) DONE:
  two SIZED layouts authored through packlib in
  ~/workspace/coi-blueprint-pack (builder
  rebuild_oil_linkage_v1.py; NEVER imported, run as __main__ only):
  (1) crude-refining-v1 (77 entities) — Layout 1, REPLACEMENT role
  for the half-OutputFull tower cluster (F2+F9): 2×T1 + 2×T2 +
  2×T3 + 1 stripper (exact 36 sour/min match) + 1 BoilerGas on
  heavy oil; 120 crude/min nameplate → 72 diesel + 48 naphtha +
  24 fuel gas/min; steam demand 27/min on 20.25 of 24 heavy/min,
  excess ~3.75/min → the base's FIRST heavy-oil buffer + flare.
  Counter-scale at the full measured stream (480 TM / 554 LM):
  diesel 288/332, naphtha 192/222 per month.
  (2) vehicle-diesel-v1 (45 entities) — Layout 2, sized EXCLUDING
  DieselReforming (F3): 1 advanced chain (Advanced over Basic:
  0.60 vs 0.45 crude→diesel, Basic is throwaway), target =
  vehicle fuel 49/mo × ~2 = ~98/mo → 163 crude/mo at counter
  scale; two chained diesel buffers (fuel-buffer rule); no T3 —
  light oil is a buffered export.
  ASSUMPTION SPLIT (full maths in linkage doc): Case A (reforming
  OFF) — existing 84/mo already covers vehicles (+35); Layout 2
  is a prepared paste only, NOT a supply fix, do not build for
  capacity. Case B (reforming KEPT) — demand 161/mo; existing 84
  + Layout 2's 98 = 182 ✓ (+21 spare); its 163 crude/mo fits the
  +340/+354 headroom leaving +177/+191 crude still accumulating
  (Layout 2 alone does NOT fix crude). Combined: Layout 1 alone
  closes Case B diesel too (288–332/mo); both at named targets
  would need 643–717 crude/mo > production 480–554 — alternatives,
  not additives; sequence is Layout 1 first.
  LINKAGE DOC: live/diagnosis-blueprint-linkage.md — per layout:
  fault → evidence (capture numbers + source files) → layout id →
  sizing maths → expected effect + verification fields → what it
  does NOT fix; §3 honest gaps.
  HONEST GAPS: no per-minute ↔ counter/month conversion exists in
  the captures (sizing is fraction-exact — counters verified to
  obey dataset ratios exactly: 140 crude → 112 medium → 84 diesel
  + 70 light, 28 heavy, 42 sour — plus complete nameplate blocks);
  Layout 1's 120/min nameplate is half the existing cluster's
  240/min (its constraint was routing, not capacity; instantaneous
  inflow rate unknown — input buffer + pause-don't-demolish hedge
  stated); medium/heavy storage product IDs exist in no harvest
  or live storage (heavy capacity 0 base-wide) → those buffers
  placed UNASSIGNED with manual in-game set steps (diesel-
  refining-v3 precedent); all other buffer IDs live-verified in
  bridge-storages (light/naphtha/fuel-gas assignments newly
  possible); PipeT1 ~200/min cap remains reported/unverified
  (manifold peaks 120/min); brief's dataset path omitted
  reference/daxfb/ and the pack has no README.md — noted in doc.
  AUDIT: packlib.validate PASS both, footprint warnings 0 both;
  render_clean 0 unassigned both, assigned products verified
  46/46 + 25/25 correct (needed no_merge_points at 6 L1 / 3 L2
  foreign endpoints — renderer's storage-through union box is
  offset from storage origins and merges neighbouring fluids);
  audit_final.py PASS — 80 slugs, ISSUE COUNT: 0 (script's
  hard-coded expected count updated 78 → 80 for exactly these
  two additions, comment in file). Registered in
  NEW_MODULES_oil_linkage.json + PACK_MANIFEST.json Phase 6.
  Constraints held: no uploads, no live-game contact, no
  messages to Philip; NOT in-game tested (pack global status).
  Addendum (14:3x CET): audit_pack.py also run — 80 modules,
  9 failures, ALL on pre-existing slugs (chemicals-v3,
  cparts-v1/v2/v3, cparts-advanced-v2/v3, nuclear-v1/v2/v3 —
  the pack's known generic-color / documented-unassigned
  exceptions); ZERO failures on crude-refining-v1 or
  vehicle-diesel-v1.
- 14:3x CET — Chunk 4 (docs) DONE + ROUND 7 COMPLETE: SPEC
  Addendum XIII written (v0.6.4 routes, diagnosis §9, linkage
  results incl. the Case A/B assumption split and the
  alternatives-not-additives crude maths); local-coi-bridge-dev
  updated (TrainsManager/Train/Entity surfaces, the two deliberate
  rule-5 exceptions for v0.6.4, unverified-decoding discipline);
  local-coi-playing updated (machines route, raw-counter posture,
  diagnosis §9); MODDING-NOTES gained recon round 5. Coordinator
  independently re-ran audit_final.py: ISSUE COUNT 0 over 80
  slugs. Nothing blocked. Standing opens unchanged: install
  CoiBridge v0.6.4 (supersedes 0.6.3) + populated eyeball Thursday,
  sandbox protocols, DieselReforming decision — which the linkage
  doc now prices out in both directions.

## Round 8 Chunk 3 — Machines (per-instance) UI — 14:51 CET

- serve.rs: new `machines_section(state)` helper + `MACHINES_JS` const,
  rendered on the Entities page. Fetches bridge_json "/coi/v1/machines"
  (CoiBridge v0.6.4). Absent route -> house degradation panel ("Per-
  instance machine states arrive with CoiBridge v0.6.4 — update the
  mod to see them here."). Present -> summary line (total + per-state
  counts, busiest first, plus bridge skippedRows when > 0) and a table
  Id | Proto | Pos (x, y, z) | State | Recipe(s) | Paused | Enabled.
  Filters follow the TABLE_JS idiom scoped to #mtable: text input
  (proto/id/recipe/state via textContent) + explicit state <select>
  matched exactly against each row's data-state. All interpolated
  strings via esc(); missing/malformed fields render "—", no panics.
  The section HTML is computed once at the top of page_entities and
  inserted in all three render paths (normal flow after Storages;
  game-API-unreachable page; bridge-absent page) so the degradation
  note shows even when the game is off. No new deps; plan.rs/main.rs
  untouched.
- Build: Linux debug `cargo build` clean, zero warnings/errors.
- Verify A (game unreachable, scratch coi on :8485, fresh DB):
  POST /login setup+login, GET /entities -> 200, "Machines
  (per-instance)" heading + v0.6.4 degradation note present, no
  #mtable. PASS.
- Verify B (populated, via new stub): tools/stub_game_server.py
  (stdlib http.server) serves the game API from live/ + bridge routes
  from live-synthetic-v064/ on :3999; scratch coi --game-url
  http://127.0.0.1:3999 on :8486. GET /entities -> 200; #mtable row
  count == 223 (== capture rows); 4 distinct states (Working 94,
  OutputFull 59, NotEnoughInput 59, Paused 11) matching the summary
  line; #mfilter + #mstate controls and all 4 state options present;
  sample row #2484 AirSeparator at "3200, 486, 4" and a 12-recipe
  joined cell verified; storages table unaffected. PASS.
- Verify C (bonus, old bridge): stub --capture live/ (no machines
  file -> 404): page fully populated (Storages (283)) with the
  machines degradation note and no table. PASS.
- Stub saved for reuse: tools/stub_game_server.py (--port/--capture).
  No uploads, no live-game contact, no /command POSTs. Windows
  release build left to the coordinator.
- 14:51 CET — Round 8 Chunk 1 (supervised session runner `coi play`)
  DONE: new subcommand implemented INSIDE coi/src/plan.rs as
  `pub fn play_cmd` (uses the private engine internals: load_sequence,
  eval_gate/eval_predicate, derive_statuses via a new eval_prefix
  helper; build_report untouched), dispatched in main.rs + one usage
  line. Interface: `coi play <plan.json> --state-seq DIR
  [--session-out session.json] [--transcript-out transcript.md]`.
  Semantics: walks the sequence prefix by prefix (step k =
  states[0..=k], statuses re-derived per prefix); loop position =
  first non-COMPLETE phase; at a phase's first ACTIVE step its orders
  are recorded once, in plan order — PREPARED with the exact command
  envelope {"type","args"} when `do` is a catalogue command with
  concrete args (catalogue membership checked against
  crate::catalogue::CATALOGUE; GameConsoleCmd excluded by policy),
  SKIPPED with reason for manual-step/place-blueprint/observe-only/
  wait kinds or class manual (reason names the kind), placeholder
  research id ("placeholder research id (unsourced)"), any $entity
  selector ("entity selector requires human resolution (§6.2)"), or
  any other placeholder. Phase completions recorded at the first
  step the exit gate is TRUE, with the engine's exit evidence.
  Final state: complete, or blocked on the loop phase with every
  exit-gate predicate's rendered description, verdict and verbatim
  evidence. Outputs follow schema coi-play-session/1 (steps with
  per-step statuses/loop/events, orders with confirmation:null,
  phaseHistory, final). HARD SAFETY held: prepare-only, NO apply
  path — no sockets, no client, no §12 calls; files read + the two
  named outputs written only; stated in the module doc comment, the
  play section comment, the session `runner` field and the
  transcript banner (human confirms in the website).
  EXERCISE: `coi play plans/fresh-start-v1.json --state-seq
  plans/fixtures/growth` → plans/fresh-start-growth-session.json +
  plans/fresh-start-growth-session.md. Walk AGREES with
  growth-sequence-timeline-v2.md exactly: step 0 (m1, 1-1-18) phase 0
  COMPLETE + phase 1 ACTIVE (orders recorded: 1 PREPARED
  p1-resume-time GameSpeedChangeCmd, 5 SKIPPED incl. placeholder
  p1-research-construction); step 2 (m3, 1-3-15) phase 1 COMPLETE,
  phase 2 entered HELD (UNEVALUABLE entry — the §4.5 placeholder, as
  in v2); step 4 (m5, 1-5-15) phase 2 COMPLETE + phase 3 ACTIVE
  (orders: 4 PREPARED research starts, p3-grow-truck-fleet SKIPPED
  on its $entity selector, 3 SKIPPED blueprint/manual); step 7
  (m8, 1-8-15) phase 3 COMPLETE + phase 4 ACTIVE (orders: 1 PREPARED
  p4-research-basic-diesel, p4-diesel-storage-alert SKIPPED on
  $entity, 2 placeholder-research SKIPPED, 3 blueprint SKIPPED);
  steps 8–11 unchanged, phase 4 ACTIVE to m12. FINAL: blocked on
  phase-4-concrete-diesel-rubber — blockers at final prefix: concrete
  TRUE (226 ≥ 100), diesel lastMonth net-flow TRUE (+82 = 110 − 28),
  rubber TRUE (17 > 0), diesel stock FALSE — engine evidence
  verbatim: "Product_Diesel.quantity = 220" vs >= 300, exactly the
  v2 timeline's single binding leaf.
  ONE RULE CONSEQUENCE, flagged honestly: phases 0 and 2 never have
  an ACTIVE step in this sequence (0 is already COMPLETE at m1; 2
  goes HELD → COMPLETE at m5), so under the specified
  record-at-first-ACTIVE-step rule their orders are NOT recorded;
  phaseHistory still records their entry/completion, and the
  transcript carries an explicit note per such phase.
  REGRESSION GATE: `python3 tools/check_plan_parity.py` → PARITY
  PASS (fresh-start-v1 OK 44 gate items, dragon-isle-continuation-v1
  OK 49 gate items); Linux debug `cargo build` clean, no warnings.
  serve.rs untouched; no uploads, no live-game contact.

## [14:54 CET] Round 8 chunk 4 DONE — history → states adapter + continuation over history sequence
- TOOL: tools/history_to_states.py (stdlib only). Input: the
  388-2-27 snapshot's `history` block (36 pinned products × 8 monthly
  points) + live/ for catalogue/pinned/gameId. Output:
  history-states/month-01-387-07 … month-08-388-02 (dir names carry
  the calendar month) + generated README.md with full provenance.
  The tool REFUSES to build if its alignment premises fail (series
  lengths, gameId match, the live-stats identities, cross-check tail).
- ALIGNMENT DECISION (evidence in history-states/README.md): point i
  = calendar month 387-7 + i; point 7 = 388-2 (in progress). Decisive:
  for 36/36 products, produced AND consumed, history[-1] ==
  live-stats thisMonth, history[-2] == lastMonth, [-2]+[-1] ==
  thisYear; quantity[-1] == live quantity 36/36. Corroborated by
  history.start 387-7-1 (snapshot) vs 387-8-1 (hist-keys) with
  snapshot[1:] == hist-keys exactly on all 9 shared series (3
  products × 3 series) — the offset-by-one is a one-month-wider
  window, and fetch_live's drain-to-trailing-8 (coi/src/main.rs)
  explains why the stored series starts at its first month, not at
  a pre-window state point. Quantity opening/closing-boundary
  convention is NOT provable from flow conservation (exact in only
  ~half the finished transitions under either reading) — disclosed
  in the README, never corrected by inventing numbers.
- ENCODINGS (each TESTED on both engines with a scratch plan first):
  stats rows hold ONLY the 36 history products; produced/consumed
  are SHORT arrays [thisMonth, lastMonth] (month 1: length 1 — June
  387 is in no capture), so thisYear/lastYear/lifetime read
  UNEVALUABLE in both engines; stored/storageCapacity are
  present-but-NULL (reads UNEVALUABLE; OMISSION was tested and
  REJECTED — it makes Python's storage.percent_full cross-ref raise
  KeyError and blinds that metric even with genuine bridge rows).
  Ticks: derived by anchoring the documented 2,880/day rate to the
  real tick (spacing-faithful; the API tick's epoch is not the
  calendar epoch — documented). Months 1–7 have NO bridge files;
  month 8 copies live game-info verbatim + the five genuine bridge
  captures.
- RUNS: both engines over the continuation plan with
  --state-seq history-states → plans/continuation-history-states
  .python.dryrun.md + .rust.dryrun.md. PARITY PASS (verdict skeleton
  identical incl. per-prefix rows; counts TRUE 27 / FALSE 26 /
  UNEVALUABLE 20; diffs are the known cosmetic classes only).
- RESULT (writeup: plans/continuation-history-states.md): phase 0
  COMPLETE at all 8 prefixes (Steel 1,379→1,108 ≥ 800, Diesel
  6,461→5,992 ≥ 4,000 every month — buffers held, bleed slow);
  phase 1 HELD (U) at ALL 8 prefixes and phases 2–7 NOT REACHED
  throughout. Headline limitation, stated plainly in the writeup:
  phase 1's entry gate CANNOT be judged from product history alone —
  its ConcreteSlab `product.stored >= 2900` leaf is underivable in
  every month (history carries no stored field), even though the
  out-of-sequence live capture shows 7,205 stored. UNEVALUABLE
  groups enumerated: (A) bridge-only metrics blind months 1–7 +
  inside every for: window reaching them (power.balance_kw blind
  everywhere; research.completed blind even at month 8 — pre-v0.6.2
  capture); (B) non-pinned products (IronOre, Fertilizer,
  HouseholdGoods, MedicalSupplies, VehicleParts2) blind at every
  prefix; (C) underivable stored/capacity + thisYear/lastYear/
  lifetime slots (the slots are never read by this plan — all its
  flow predicates are thisMonth/lastMonth); (D) the §4.5 Mega
  Vehicles placeholder (manual-only by design, not a data gap).
  Late gates that CAN be read honestly evaluate: phase-2 exit FALSE,
  phase-4 steel balance FALSE — the sequence refutes, not just
  abstains. No uploads, no live-game contact, no messages.
- 14:5x CET — Chunk 6 (strategy skill) DONE (coordinator, after
  chunk 1 landed per Philip's ordering — runner first, strategy on
  top): ~/workspace/custom-skills/local-coi-strategy/SKILL.md —
  the mid-loop decision layer. Contents: triage order (death-spiral
  triangle fuel/maintenance/electricity + starvation, per starter
  bottlenecks.md; then bottleneck-chain starvation — steel/CP;
  blockages/backups; configured-full hygiene; rest), each leg
  grounded in the real base's diagnosis evidence; expand-vs-fix
  heuristics (never scale a chain whose binding leaf is failing;
  crude-cluster routing case from the linkage doc as canonical;
  settings→recipe→replacement→construction ladder; capacity-only
  expansion criterion in plan block sizes); stall interpretation
  (WAIT on §4.3.1 duration/seasonal/refill evidence, REDESIGN via
  the diagnosis→linkage pattern, ESCALATE on irreversibles,
  plan-flagged DECIDEs — DieselReforming p0 order named — and
  plan-vs-base contradictions); experimentation policy (guides
  first, sandbox protocols before trials, verified/wired/excluded
  + unverified-decoding labelling incl. train counters);
  runner consumption (coi-play-session/1: blockers first,
  prepared≠intended, never-activated phases leave no orders by
  design, decisions quote evidence verbatim). Every rule cites a
  guide, plan, diagnosis finding or sibling skill — no invented
  mechanics. Shape mirrors local-coi-playing (Purpose / sections /
  Operating Rules).
- 14:55 CET — Round 8 chunk 2 (blueprint assets for the game plans)
  DONE: map-first pass over all 20 blueprint_refs (12 fresh-start +
  8 dragon-isle), author-only-if-honest. MAPPED 7 to existing pack
  slugs: fresh p1 starter cluster -> iron-v1 (exact smelter core,
  1 furnace + 2 casters scrap; coal maker / diesel generator / labs
  have NO genuine prototype in any pack source - manual singles,
  named in the status doc); fresh p3 ore-smelter anchor -> iron-v1
  + iron-v2 overlay (the iron-steel skill's Stage-1 build verbatim);
  fresh p4 kiln+mixer -> concrete-v1 and dragon p2 §5 concrete
  block -> concrete-v1 (1 kiln + 1 Mixer II = 24 slabs/min, gas kiln
  = §5's own coal-free swap); fresh p4 basic diesel -> diesel-v1
  (pumps are deposit-sited, no OilPump prototype exists); fresh p4
  rubber -> rubber-v1; dragon p4 caster pair -> steel-v1 + steel-v2
  overlay (48 steel/min; pack's 2-furnace-per-line feed vs §5's
  dataset 1-furnace rate documented as the one divergence).
  AUTHORED 2 new modules via rebuild_plan_linkage_v1.py (packlib,
  __main__ only, no build_*.py imported): starter-assemblies-v1
  (64 entities - 4x AssemblyManual bank, cparts-v1 lanes extended,
  A1 CpAssembly preset, A2-A4 recipes cleared rather than guessed,
  per-assembly output buffers) for fresh p2; household-goods-v1
  (21 entities - AssemblyElectrifiedT2 HouseholdGoodsAssemblyT2,
  dataset-verified 16/min, consumer-goods-v1 geometry with the tier
  corrected; NOT a mapping to the robotic 32/min line) for dragon
  p6. Registered in NEW_MODULES_plan_linkage.json + PACK_MANIFEST
  phase 7; renders 0 unassigned each; audit_final updated 80 -> 82
  slugs, ISSUE COUNT 0. STILL MISSING 11, each with a precise
  blocker in plans/plans-blueprint-status.md (the full 20-row
  table): terrain/deposit (coal mine, spine trunk, NE excavation,
  20-farm district), no genuine prototype (maintenance depot,
  first-farm - FARM_GUIDANCE is guidance-only and FarmT2 is the
  wrong tier, fuel station, fertilizer-60 + medical chain -
  Chemical Plant exists only as manual stubs and both orders are
  audit/flow-contingent), scrap-vs-ore mismatch (copper district),
  unspecified design (permanent assembly area). Plans updated in
  place: asset_status authored + pack_slug (+ pack_slug_overlay
  where layered) on the 9 resolved refs, blueprint_ref untouched,
  versions fresh 1.2.0 -> 1.2.1, dragon 1.1.0 -> 1.1.1; canonical
  JSON regenerated. audit_blueprints.py gained a minimal pack_slug
  check (authored counts only if the slug exists in the pack;
  prior verdicts unchanged): 9 AUTHORED / 11 MISSING-DECLARED /
  0 undeclared. validate_plan PASS 0 errors both plans;
  check_plan_parity PARITY PASS both. No uploads, no live-game
  contact, no messages to Philip.
- 15:0x CET — Chunk 5 (docs + final build) DONE + ROUND 8 COMPLETE
  (coordinator): SPEC Addendum XIV written (runner, machines UI,
  blueprint 9/20, history adapter + its proven alignment and HELD
  caveat, strategy skill, staged sizes). local-coi-playing updated
  (runner + adapter paragraphs, blueprint audit state 9/20);
  local-coi-companion-dev updated (play subcommand in CLI surface,
  Machines section + stub_game_server.py in serve shape; setup-mode
  first-POST note). FINAL BUILD: Windows release rebuilt clean,
  dist/coi.exe = 5,609,920 B (was 5,442,879); strings verified in
  the staged binary: coi-play-session/1, "Machines (per-instance)",
  "prepare-only". Coordinator independently verified the session
  file (final: blocked on phase-4, diesel leaf 220 FALSE vs >= 300,
  other three leaves TRUE; phaseHistory 0@0/0, 1@0/2, 2@2/4, 3@4/7,
  4@7/—; 6 prepared / 15 skipped) and the history-states layout
  (month-01-387-07 … month-08-388-02 + README). Round 8 totals:
  chunks 1–6 all DONE, nothing blocked; parity PASS maintained
  across every engine change; zero uploads, zero live-game contact,
  zero applies. Thursday opens unchanged: install CoiBridge v0.6.4
  + this exe, populated eyeball (machines section first), sandbox
  protocols A/B, DieselReforming decision — now with the runner,
  the history sequence and the strategy skill ready to run the
  session for real.

## QoL mod prep round (2026-11-04 ~21:35 CET)
- Philip (on the train home) filed five QoL mod ideas in QOL-MODS.md
  (chicken distribution, world-map dropdown, train cargo for Unity,
  train network advisor, productivity alerts) and ordered:
  "Challenge and refine these, to prepare mods."
- Prep-only round: five recon spikes fanned out (animals,
  world-map UI feasibility, train cargo + Upoints, advisor metrics,
  productivity measurement). Deliverable: QOL-MOD-PREP.md —
  challenge / refined design / recon / readiness per idea + a
  recommended build order. No gameplay-mod code, no uploads, no
  live-game contact; each mod still needs Philip's own go-ahead.
- QoL prep round DONE: five recon spikes landed (qol-recon/01–05,
  ~175 KB of evidence) and QOL-MOD-PREP.md synthesises them —
  per idea: challenge / refined design / recon / readiness.
  Verdicts: chickens READY (vanilla already round-robins moves;
  the mod is a policy layer over MoveAnimalsCmd with
  slaughter-limit guards); world-map dropdown READY as a
  PriorityManager-pattern Unity UI mod, no Harmony (4 live
  checks pending); train cargo NEEDS 2 decisions + the
  custom-command serializer sandbox test (DestroyCargo exists
  but routes to storage; the TrainNetworkJob accounting is the
  real hazard); advisor SPLIT (L0 ready today, L1 ready as a
  bridge round S-STATIONS/S-SCHEDULE/S-NETWORKS/S-WAGONS,
  L2 blocked on live trains for counter verification);
  productivity READY after the snapshot-store prerequisite +
  κ calibration (today's store: exactly 1 paused snapshot,
  zero machine states — coi watch was never built). Build
  order: chickens → productivity → world map → train cargo →
  advisor. QOL-MODS.md status lines point at the prep doc.

## 2026-11-04 (late evening, subagent) — QoS suite round 1: foundation + chickens BUILT
Philip's architecture call (22:03) landed: small individual mods
linking into one QoS suite, sub-mod dependencies explicitly OK.
Built under qos/ (ARCHITECTURE.md has the layout + dependency
rules; BUILD-NOTES.md has built-vs-prep + every deviation):

- Dependency + config mechanics VERIFIED from the official
  modding README + shipped manifests before designing:
  mod_dependencies = string[] of ids with optional ">= x.y.z"
  constraints, topological load; config = per-mod config.json
  (snake_case params) surfaced in the game's settings UI and
  persisted in saves, read via ModJsonConfig.Get*(name, default).
- QoSCore 0.1.0: QoSLog (prefixed suite logging), SuiteRegistry
  (features Announce themselves), QoSCoreService (verbose_logging
  config). Nothing speculative beyond what chickens needs.
- QoSChickens 0.1.0: prep SS1 implemented as designed - daily
  NewDayEnd pass, vanilla MoveAnimalsCmd via IInputScheduler,
  guards enforced by EXACT simulation of the shipped round-robin
  (receiver cap = min(capacity, SlaughterLimit); no drain below
  the nucleus floor; per-day cap). Ships DISABLED (enabled=false,
  mode pre-set to equalise) - the prep left the default open;
  conservative reading taken, recorded in BUILD-NOTES.
  Growth-paused guard = species skip for the day (the shipped
  peer set cannot exclude a farm - stricter than prep text,
  only faithful option). Fill-first order from config ids.
- Builds: both projects 0 warnings / 0 errors (net48, qos/build/,
  one csproj per subfolder - shared obj/project.assets.json
  gotcha found + documented in the bridge-dev skill, which also
  gained the verified manifest-dependency / config.json /
  plain-IMod / cross-assembly mechanics).
- Staged in dist/ (NO uploads): QoSCore-mod-0.1.0.zip (9,951 B),
  QoSChickens-mod-0.1.0.zip (20,050 B, no QoSCore.dll inside),
  QoS-suite-0.1.0.zip (32,199 B, both folders + suite readme).
- Unverified until first live install: cross-assembly QoSCore
  resolution via manifest dependency; peer enumeration order ==
  simulation order (guards bound outcomes regardless).
  CoiBridge untouched; no live-game contact (PC off).

## 2026-11-05 (day) — QoS pattern survey
Philip asked what QoSChickens' structure generalises to. Close-read the shipped balancer + core, surveyed the catalogue/managers, ran fresh assembly recon (VehiclesManager decompile, Truck/Vehicle dumps, ToggleEdictEnabledCmd dump, ICalendar dump). Deliverable: qos/PATTERN-SURVEY.md — pattern anatomy (8 elements) + limits (command owns its target set; simulation needs closed deterministic semantics; calendar-granular cadence; no per-entity memory → stateless ownership rules), 10 candidates with verified surfaces and verdicts, rejections included (quick-trade repetition rejected on the §12 resource-spend discipline despite the cleanest mechanical fit). Recommended next: QoSStoragePolicy (alerts + static slider presets), QoSPriorityActuator (starved-machine counterpart to §5), QoSVehicleRecovery. New verified surfaces this survey: Truck.IsStuck / IsCannotDeliverNotificationActive / NavigationFailedStreak; IVehiclesManager.CanRecoverVehicle decompiled (depot+Unity gate only, NOT a stuck detector); ToggleEdictEnabledCmd is a shipped InputCommand with shipped serializer (EdictProtoId field) — schedulable from a mod despite web-inventory absence; ICalendar cadence menu (NewDay/NewDayEnd/NewMonth*/NewYear*). ResearchManager.ResearchQueue confirmed public via bridge v0.5 code; vanilla auto-advance-on-completion left UNVERIFIED with the settling check named. No code built, no uploads, no live contact.

## 2026-11-05 (day) — QoS round 3: vehicle content recovery + preferred drop-off
Philip's asks 6-8 (QOL-MODS): stuck vehicles' CARGO recovered; trains/depot pick a preferred drop-off for overload; same routing for auto-empty. Recon first (qol-recon/06 + 07, MODDING-NOTES round 6): vanilla vehicle recovery (0.2 Upoints flat, VEHICLE_RECOVERY_COST public) TELEPORTS the truck to the closest accepting depot and NEVER touches its cargo — content preserved aboard by construction; CanRecoverVehicle is depot-availability + affordability only, cargo never consulted; a failed enqueue still cancels the truck's jobs, so the mod pre-checks the gate before scheduling. Every cargo clear in the game routes through AssetTransactionManager.StoreBuffer with NO destination parameter (real storage buffers in global-priority order, shipyard overflow; no abstract pool) and no public door steers a truck's cargo into a NAMED storage (Truck.CargoBuffer internal; TakeCargo+StoreAsMuchAs composite = unverified research lead). Trains: NO preferred-overflow seam anywhere — jobs sized to fit at dispatch; overload = wait, 5-day force-depart still loaded, Finalize(false) with no emptiness check; leftover handled by dispatchLostTrains (whole-quantity, lowest NetworkPriority wins, else parked loaded at a waiting bay); dispatcher scoring is closed, depots never unload wagons, RecoverTrainCmd preserves cargo. Design (BUILD-NOTES round 3, written before code): "preferred drop-off" = an ordered destination LADDER (aboard -> the economy -> the dispatcher), no fake destination config. BUILT: QoSVehicleRecovery 0.1.0 (daily NewDayEnd pass; stage 1 RecoverVehicleCmd under CanRecoverVehicle pre-check + Unity reserve floor (default 5.0) + per-day cap (default 5) + sustained-stuck triggers; stage 2, default OFF, DiscardVehicleCargoCmd for a mod-rescued truck whose same load is cannot-deliver again — marks are session memory, per truck+product, 7-pass expiry). Ships DISABLED (enabled=false). Train leg + named-storage drop-off scoped OUT on evidence (future: companion advisory for leftover-cargo trains; bridge does not expose lost-train state). Builds: all three projects 0 warnings / 0 errors. Staged in dist/ ONLY (no uploads, no push): QoSVehicleRecovery-mod-0.1.0.zip (21,926 B), QoS-suite-0.2.0.zip (54,677 B; suite pack now has its own SUITE_VER decoupled from QoSCore). QoS-suite-0.1.0.zip verified untouched (sha256). Unverified until live install: IVehiclesManager/IUpointsManager injection, cross-assembly QoSCore resolution (suite-wide).

## 2026-11-05 (day) — Recon spike 8: priority re-fill for ATM deposits
Philip asked about updating AssetTransactionManager to add a priority re-fill concept (deposits refilling a preferred storage first). Full decompiles of Mafi.Core + Mafi.dll read. Ordering truth: GlobalBuffer.Priority is an immutable per-buffer int set at buffer construction (machine inputs 5, every storage 10, outputs/depot 15; named scale GlobalInputPriority 5/10/15 exists), sorted ascending per StoreProduct/StoreBuffer call with unspecified tie order (Array.Sort); registrations are public IAssetTransactionManager Add/Remove calls, duplicate-guarded, and SERIALIZED INTO SAVES with their priority values. Three rungs priced in qol-recon/08-priority-refill.md: A) steer via input-side-only remove+re-add at priority 9 on a policy pass — POSSIBLE, recommended (CargoDepotBuffer.SetIsInput is the shipped re-registration precedent; draws/quick-trade/paving untouched because output side never re-pinned; costs = save-persistent pins with restore-on-disable, first direct-service-API mutation in the suite). B) DI substitution — door verified OPEN (mods register last, last-impl-wins, EntityContext is interface-typed) but strictly dominated: concrete-typed consumer on the terrain path + a mod type inside every save's economy graph kills removal safety. C) Harmony — only rung with true custom distribution; first-Harmony discipline change + per-patch re-verification of a core economy method; reserve. Pull reading of "re-fill" (trucks topping up a designated storage) needs no manager change: Storage.ImportPriority/ExportPriority (default 8, SetCustomPriorityCmd ImportPrio/ExportPrio) + sliders already feed VehicleBuffersRegistry — folds into QoSStoragePolicy. No code built; rung choice is Philip's.

## 2026-11-05 (day) — Round 4 (QoS BUILD-NOTES): productivity prerequisite built; QoSWorldMap sources complete, compile blocked on UnityEngine refs
Productivity prerequisite (prep §5) BUILT in the companion: store schema v2 via PRAGMA user_version (machine_state_proto aggregates + machine_state_instance rows + productivity_state hysteresis; v1 upgrade verified against a copy of the real coi.db, its 388-2-27 snapshot intact with zero machine rows); fetch_live captures /coi/v1/census + /coi/v1/machines best-effort into snapshots (bridgeCensus/bridgeMachines keys); `coi watch` built per SPEC §9 (tick-change snapshots, --every/--interval, --count; poll failures retry, never exit); κ-free estimators in coi/src/productivity.rs — (a) machine working share with prep-exact MEASURED/ESTIMATED/INSUFFICIENT discipline (silence below the line, never "healthy"), Wilson firing + second-half confirmation + hysteresis + cause separation + PARKED-before-class-gate, (b) self-relative finished-month output (FLOW-EXACT, composes into stall/deficit issues). Thresholds = prep defaults, coi.toml [productivity] overrides via a dependency-free mini-parser. Surfaces: snapshot-time board issues (CLI + website button share the path), `coi report` section, website Issues-page section. Verification: Linux + Windows builds 0 warnings/0 errors, 12 unit tests, plan parity PASS, diagnose_live.py byte-identical, fake-server end-to-end demo (3 snapshots → all INSUFFICIENT with missing ingredients named, (b) fired; 31 → FurnaceT1 MEASURED FIRE starved, MillT1 PARKED, PressT1 running hot; 93 proto + 806 instance rows). One real bug caught by the demo (8-for-9 placeholder INSERT rolling back every snapshot insert) — fixed + regression test added. dist/coi.exe = 5,828,998 B. QoSWorldMap v0.1.0: full sources written to prep §2 (PriorityManager-pattern Window over WorldMapManager.Mines, navigation-only, four live-check seams isolated: MineNavigator.Navigate / renderer-init TryResolve degradation / navigate_opens_map config / ToolbarOrder=1500f) — but NO ZIP: compiling any UiToolkit Window subclass needs UnityEngine.dll + UnityEngine.CoreModule.dll + UnityEngine.UIElementsModule.dll from the game install, absent from refs/ and the whole VM (probe: single CS0012 naming UIElementsModule; OnClick's EventCallback<> likewise). Mod/navigator/controller sources compile 0/0 against the real Mafi set via a stub-window probe (which also caught a real Option<KbHandle>-null bug — fixed by omitting the defaulted argument); every window component API probe-verified except the two UIElementsModule points. build.sh now detects the three DLLs: until they land in refs/ it skips QoSWorldMap + the 0.3.0 suite pack loudly (SUITE_VER already 0.3.0, suite readme already updated inside the future pack); this run rebuilt the three existing mods 0/0 with all prior zips sha256-identical. UNBLOCK = copy the three UnityEngine*.dll from the game's Managed folder into refs/ and re-run qos/build/build.sh. No uploads, no push, no live contact.

## 2026-11-05 (day) — QoS round 5: QoSPriorityActuator built
Philip's "gogogo whats next" — the pattern survey's recommendation #2 (QoSPriorityActuator, §2.3: FITS-WITH-EXTENSION) built as v0.1.0. It is the actuator counterpart to the §5 productivity estimators: companion detects with evidence classes, the mod acts with bounded steps, both share §5's cause vocabulary (STARVED/BLOCKED/PARKED). Design note written into BUILD-NOTES round 5 BEFORE code: daily NewDayEnd sensing from Machine.CurrentState (nested Machine.State enum — recon round 7: full value list dumped; no "disabled" state, that arrives via IsEnabled); STARVED (NotEnoughInput) persisted 3 consecutive passes -> SetGeneralPriorityCmd at current-1 (GeneralPriorities constants settle direction: HIGHEST 0, HIGH 8, DEFAULT 9, assignable 0-14 — lower = served sooner); BLOCKED never adjusted (named sign decision: survey says "never raised"; a down-step would also deprioritise the pickups that empty the machine — IsCargoAffectedByGeneralPriority is a single per-entity flag, no input-only priority exists); PARKED (paused/disabled/Paused state) never touched; Working restores +1/pass toward Prototype.Costs.DefaultPriority under the STATELESS restore test (current == default-k, 1<=k<=band). Ownership rule is the heart: only values in {D-k: 0<=k<=N} are ever adjusted (defaults: band N=2, cooldown 2d, cap 10 actions/day); anything else is player-set and never fought — safety needs no memory, survives reloads; persistence/cooldown bookkeeping is session-only (recovery-marks pattern). Extra gate the survey didn't name: IsCargoAffectedByGeneralPriority == false -> skip both directions (no no-op actions). Actuation ONLY via the shipped command through IInputScheduler (ctors verified by ilspycmd: (IEntity,int) and (EntityId,int)). Builds: all five compilable projects 0 warnings / 0 errors (QoSWorldMap still skipped loudly — UnityEngine refs absent). Staged in dist/ ONLY (no uploads, no push): QoSPriorityActuator-mod-0.1.0.zip (23,267 B; no QoSCore.dll inside), QoS-suite-0.4.0.zip (79,201 B; four mods + suite readme). Process deviation, named: round 4 withheld the suite pack while WorldMap couldn't compile; this round ships the pack WITHOUT WorldMap, membership declared in the in-pack readme (declared absence, not silence). No 0.3.0 pack was ever written. All prior zips sha256-verified byte-identical. Ships DISABLED (enabled=false). Unverified until live: IEntitiesManager injection in-mod, cross-assembly QoSCore resolution (suite-wide), first-enabled-days eyeball of predicted steps vs inspector values.

## 2026-11-05 (day) — Round 6: the unified Log (companion schema v3 + QoSCore 0.2.0 + suite 0.5.0)
Philip's ask: "add a LOG tab which is an exact replay of things we did in the COI exe or in the mod usage and stored that way" — built as a persistent chronological activity record (a replay to read, never a re-execution; nothing on the page can re-apply anything). SPEC Addendum XVI is the contract; qos/BUILD-NOTES round 6 is the build trail; bridge/MODDING-NOTES round 8 holds the new recon. COMPANION: store schema v3 (PRAGMA user_version; activity_events with UNIQUE event_key); pre-v3 audit rows backfilled once (legacy-audit-<id> keys, detail text verbatim in payload, game context left NULL — never reconstructed); do_apply writes the legacy audit row AND a rich event (exact envelope, HTTP status + response body, session user as actor, game date/tick/id fetched around the apply); website snapshot creation is a system event (stored/duplicate); never-applied prepares, page views and logins are NOT events. GET /log + nav: source badges, actor/mod, action, summary, outcome, game date/tick, exact payload in <details><pre>, source + free-text filters, 200-row pagination, honest provenance header (QoS events exist only from the updated mods onward; no backfill of pre-upgrade mod actions). QoS: QoSCore 0.2.0 gains QoSEventLog — static, fail-safe (never throws into the sim), thread-safe, append-only JSONL at %APPDATA%\Captain of Industry\QoS\events-v1.jsonl (outside mod folders, outside saves), hand-rolled JSON, line schema v1 (GUID eventId = import dedupe key). Gate = QoSCore's activity_log_enabled (default true), NOT the features' verbosity flags. Instrumented after ScheduleInputCmd succeeds: Chickens MoveAnimalsCmd (farm/signed amount/species/mode/predicted spread/eligible count), VehicleRecovery RecoverVehicleCmd + DiscardVehicleCargoCmd (truck, reason/streak or mark product, cargo, cost as Fix32-decoded numbers), PriorityActuator SetGeneralPriorityCmd nudge+restore (machine/proto/cause/current→target/default/band floor/streak), WorldMap navigations (kind navigation, after Navigate() returns). Clock recon: ICalendar.CurrentDate is public (Mafi.GameDate Year/Month/Day) → QoSCoreService wires it as the recorder's date provider, date rides in data.gameDate and imports into the game_date column; ICalendar has NO tick, so QoS events carry no tick (named limit). QoSCoreService gained an ICalendar ctor parameter (feature-service pattern; flagged as the round's one new live unknown). Import runs on every /log load: idempotent (INSERT OR IGNORE on qos:<eventId>), counts + feed path shown on the page, malformed counted never fatal, 64 KiB line cap, COI_QOS_EVENTS_PATH override. Versions: features 0.1.1 with dep "QoSCore >= 0.2.0"; suite pack 0.5.0 (116,830 B, five mods + readme — the in-pack readme also finally matches reality on WorldMap membership); dist/coi.exe rebuilt 5,885,166 B. Verification: Rust 15/15 tests (v2→v3 migration + backfill idempotency, import counts, ISO parser); tools/check_log_page.sh PASS (scratch serve: legacy row renders, QoS rows import, source filter separates, re-import idempotent); all five C# projects 0 warnings / 0 errors; prior suite packs + individual zips sha256 byte-identical. Staged in dist/ ONLY (no uploads, no push, no live contact). Unverified until live: the feed being written in-game, QoSCoreService's ICalendar injection, /log against a real feed.

## 2026-11-05 (afternoon) — Recon round 9: placement semantics settled from code
Philip: "go build and place shit" / "go find that shit — maybe learn more?" Static-only round (ilspycmd on the shipped 0.8.8 assemblies; NO game contact, nothing prepared or sent). Full trail: qol-recon/09-placement.md; summary in bridge/MODDING-NOTES round 9; Protocol A gained Revision A-R (original text + Expanding-save abort guard preserved). Headlines: (1) `TileTransform.Position` IS the centre tile (LayoutEntityBase.SetTransform assigns CenterTile = Position) — the coordinate question that blocked placement is closed, and topology readback is an exact cross-check. (2) CreateStaticEntityCmd places a CONSTRUCTION SITE through the game's own validator pipeline (unknown proto / research-lock / validator error = clean result-error, instance destroyed, result on success = the new EntityId); cost is truck-delivered construction buffers — nothing is charged at placement and the IsFree field is ignored, so there is no free test placement on a live save. (3) Rotation fully mapped (Deg0/90/180/270 = (1,0)/(0,1)/(−1,0)/(0,−1)); only its visible effect still needs an eyeball. (4) FinishBuild = Unity quick build/removal, errors on finished entities. (5) Deconstruction is a state machine: untouched site vanishes instantly (the cheap sandbox revert), finished buildings refund sell-value × DeconstructionRefundMultiplier as physical hauled stock. (6) THE structural finding: belts/pipes and blueprints are created by BuildTransportCmd / BatchCreateStaticEntitiesCmd, and BOTH are absent from the web registry — the Protocol B unlisted-type question now gates all transport work, while plain building placement (Create is listed) does not depend on it. Minimal live set is now five small steps (SANDBOX-PROTOCOLS Revision A-R: R1 placement+readback, R2 instant revert, R3 delivery/completion/refund measurement, R4 rotation eyeball, R5 validation legs) on a throwaway save whenever Philip loads one.

## 2026-11-05 (evening) — Protocol A-R minimal live test PASSED

At Philip's explicit direction, on his real save while paused: placed a
StorageUnit construction site via CreateStaticEntityCmd through the §12
flow — result EntityId 38208 at exactly the requested centre tile
(3167,762,4); removed the untouched site via
StartDeconstructionOfStaticEntityCmd; census 12503→12504→12503,
construction-parts quantities unchanged. Zero cost, zero residual state.
Placement is a verified capability for buildings; belts/pipes/blueprints
still hinge on Protocol B (unlisted BuildTransportCmd /
BatchCreateStaticEntitiesCmd). Catalogue: 15 verified / 224 wired /
1 excluded.

## 2026-11-05 (evening) — Recon round 10: transports + blueprints; the web channel is closed by construction

Philip: "We need to get belts and pipes and generate and use blueprints." Static-only round (ilspycmd on the shipped 0.8.8 assemblies; NO game contact, nothing prepared or sent). Full trail: qol-recon/10-transports-blueprints.md; summary in bridge/MODDING-NOTES round 10; Protocol B updated with the verdict (live test retired, real-save abort guard preserved). Headlines: (1) **Q1 settled from code — an unlisted type can NEVER be constructed from POST /command.** `WebCommandRoutes.command` is a dictionary lookup in a prebuilt `WebCommandIndex` (miss = HTTP 400 "unknown or cannot be built from JSON yet"); the index keeps only types with ≥1 `JsonObjectBuilder.CanBuild` constructor, and the builder structurally blocks collection parameters and any non-command reference type. `BuildTransportCmd`/`BatchCreateStaticEntitiesCmd` die on `ImmutableArray<…>` ctor params; `ToggleEdictEnabledCmd` dies because its only ctor takes the live `EdictProto` object — the Protocol B key ladder was testing the wrong layer. (2) BuildTransportCmd fully mapped (pivots = absolute Tile3i with heights, pillar hints = XY of supported tiles per the TransportBuildController call site; build-or-JOIN; result = transport or mini-zipper id). (3) BatchCreateStaticEntitiesCmd is per-item, NOT atomic, bool + last-error result only (ids via census diff), no size limit; a blueprint IS its `ImmutableArray<EntityConfigData>` — the same five-dict serialization our codec decodes, trajectories rebuilt from pivots on read. (4) The game's own flow (StaticEntityMassPlacer) = transform items about the anchor, then ONE batch command with applyConfiguration:true — the recipe the placement layer mirrors. Codec gaps are small: no payload gap (game deserializer is authoritative; internal-access is the build round's open probe), transport rotation math is the one real new piece. (5) Since Q1 kills the direct route, recon 10 §Q5 carries a DESIGN NOTE (not built): a QoSBuilder mod executing single-use, expiring, gameId-pinned job files written only at the §12 confirm step, reporting through the QoSEventLog feed — §12 discipline preserved, but the suite's first entity-creating mod, so it needs Philip's explicit posture decision. Minimal next live step: none for the web channel; if the mod channel is approved, the owed sandbox import test of the 82-string pack comes first.

## 2026-11-05 (evening) — QoS SAVE BUG fixed: suite 0.5.1 (three features 0.1.2)

Philip hit a live save failure with suite 0.5.0 installed ("Failed to
create generic serializer for 'ChickenBalancer'" — the game itself
unharmed, still paused). Root cause decompiled: ICalendar events are
SAVED events; Add(this, ...) serializes the callback OWNER into every
save, and our three daily services (Chickens / VehicleRecovery /
PriorityActuator) had no serializer pair — shipped subscribers get
theirs from the game's [GenerateSerializer] generator. Fix = the
game's sanctioned hand-written pattern: [ManuallyWrittenSerialization]
+ public static Serialize/Deserialize pair in the generated
(StatsManager) shape; SerializeData writes only the calendar ref;
DeserializeData restores it and re-resolves all other deps via
RegisterResolvedMember; services now hold the mod instance instead of
a bare ModJsonConfig; session state re-initialised on load (documented
reset semantics). Manifests: three features can_remove_from_saved_game
-> false (saves written with the fix contain the subscription; removal
is now game-blocked with a clear message). Audit: QoSCore, QoSWorldMap
(renderer-init lists are [DoNotSave] in shipped GameLoopEvents) and
CoiBridge all CLEAR. Builds: 5 projects 0/0; pack QoS-suite-0.5.1.zip
126,268 B; all prior packs sha256-verified untouched; compiled DLLs
inspected — attribute + pair present. Docs: qos/BUILD-NOTES.md round 7,
bridge/MODDING-NOTES.md round 11, local-coi-bridge-dev SKILL corrected
at the source. Verification is CODE-LEVEL ONLY — final proof is
Philip saving + reloading successfully on 0.5.1. Incident bonus: the
failure proves the suite loads in-game and QoSCore resolution works.
QoSBuilder build (approved 19:31) NOT started — the save bug took the
slot; it remains next.

## 2026-11-05 (evening) — Companion design system v1: DESIGN.md + Overview mock (design phase only)

Philip: "the coi companion UI is fucking awful, we should adopt a good
design system…" Phase 1 = design only (a QoSBuilder coordinator is
editing serve.rs concurrently; no Rust touched, SPEC.md untouched —
the design addendum lands with implementation, after Philip reacts).
Deliverables: DESIGN.md (36 KB) + design-mock/overview.html +
design-mock/overview.png (headless-Chromium screenshot, verified
visually). System summary: dark-first token set (5 surface layers,
semantic status palette bound to the app's own vocabulary —
Working/Starved/Blocked/Paused, quadrant severities, gate
TRUE/FALSE/UNEVALUABLE, plus a violet dashed UNVERIFIED treatment for
unproven data), system font stack + tabular numbers discipline,
232 px sidebar shell with the 12 pages grouped Monitor / Game /
Library / System and an always-visible game status strip (retires
the per-page game_header and the bare 502 page), component contracts
(.btn tiers, .badge set, .table in table-cards, .callout replacing
.err/.ok, .filterbar, .gate-chip, .fillbar), six server-side inline-
SVG chart patterns using only data that already exists (history
sparklines, TM/LM flow bars, forecast runway bars, machine-state
stacked bar, power balance bar, storage fill bars) with the
honest-labelling rule made visual, a deliberately distinct confirm
sheet for the §12 safety surface (warning band, plain-language
summary, exact JSON, danger variant for irreversible commands), and
a full page map restating all 12 pages + detail/utility screens so
the implementation round executes page by page. Reference discipline
borrowed from Carbon / Primer / Radix / GOV.UK (named, not copied).
Open forks left for Philip in DESIGN.md §9: dark-only vs light
theme, comfortable vs compact density, blue vs amber accent.

## 2026-11-05 (evening) — QoSBuilder 0.1.0 built: placement via confirmed job files (SPEC Addendum XVII)

Philip approved the suite's first entity-creating mod; built the same evening. Probe first (MODDING-NOTES round 12): the blueprint STRING route is reachable through fully public API — inject the game's own [GlobalDependency] BlueprintsLibrary, TryParseFromString is a pure parse, IBlueprint.Items is exactly the batch command's input; gameId via IGameIdProvider (the game-info handler's own surface). The internal DeserializeForBlueprints path is not mod-reachable and not needed; the items[] fallback stays documented, unbuilt.

The mod: day-end passes only while `enabled` (ships OFF); a job file is claimed (rename + processed.txt record) BEFORE scheduling, so single-use survives crashes; refusals (bad hash / expired / wrong game / over cap / unparseable / …) are consumed with a QoSEventLog JobRefused and zero action; placement mirrors StaticEntityMassPlacer (skip rule, OriginalEntityId cleared, rotate-then-anchor, trajectories rebuilt from pivots) as ONE BatchCreateStaticEntitiesCmd. Serializer pair from birth (round-7 lesson), can_remove_from_saved_game: false. The companion gained /place (library of the 82-layout pack embedded via tools/gen_blueprint_library.py — pack manifest counts found stale, codec-decoded counts used instead), prepare pinned to the live gameId (refuses when the game is unreachable), confirm = the only write, 30-minute job TTL, 120 s single-use token. Canonical-hash interop pinned three ways (C# builder, Rust builder, independent Python vector + stub check re-verifying a real written file).

Verified: C# 0/0, Rust 17/17, check_place_page.sh PASS, QoSBuilder-mod-0.1.0.zip 35,184 B standalone (suite packs untouched — integration after live proof), coi.exe restaged 6,062,808 B. Owed live: DI resolution of BlueprintsLibrary/IGameIdProvider in-game, first real job end-to-end with the census-diff verdict, save+reload with the mod loaded. Suggested first live step: sandbox save, one concrete-v1 confirm, watch Log + census.

## 2026-11-05 (evening) — Fresh-start plan 4.3 pass: v1.2.1 → v1.2.2, runner re-proven

The fresh-start plan is now the operative plan for Philip's new 4.3 run (he starts fresh this weekend and wants the supervised play session on it). Pass contents: (1) Version reconciliation — `game_version` confirmed 0.8.8 per format §3.1 (the plan targets the version Philip will run; the 0.8.7d figure is the sibling continuation plan's save provenance and never applied here); `requires.companion` refreshed from the stale 13 verified / 226 wired / 1 excluded to the current **15 / 224 / 1** plus the /place flow; bridge floor stays >=0.5.0 with the note that >=0.6.2 makes `research.completed` directly evaluable (weekend stack is 0.6.4). The plan gains a `plan.changelog` (backfilled 1.2.0 / 1.2.1 from the blueprint-status record). (2) Order audit against the generated catalogue: 53 orders total — 26 command orders (23 verified / 3 wired: AddVehicleToBuildQueueCmd, held by its placeholder args; CreateNewTrainLineCmd + CreateNewTrainNetworkCmd, held by requires_sandbox_validation), 12 place-blueprint, 12 manual-step, 3 observe-only; every arg name matches the catalogue spec; no command changed status in a way that touches the plan; the Research<Construction> placeholder stays (genuinely unsourceable) and stays non-wedging — completion derives from exit gates. (3) Blueprint refs: audit unchanged (9 AUTHORED / 11 MISSING-DECLARED / 0 undeclared across both plans); all 6 fresh-start authored slugs (iron-v1, iron-v2, starter-assemblies-v1, concrete-v1, diesel-v1, rubber-v1) confirmed present in the embedded /place library; the 6 authored place-blueprint orders' notes now name the QoSBuilder job route as the placement mechanism (kind/completion/autonomy untouched). (4) 4.3 substance verdict: NO phase, gate, order or threshold changed — mixed-materials support touches storage/cargo buffer semantics, but the plan's gates read aggregate product stats, it uses no `storage.*` metric anywhere, and its storage orders target dedicated single-product buffers by doctrine; overlays / radiation-shielded trains / train stats are surfaces no phase references; recipe ratios unchanged in 0.8.8. (5) Runner proof on v1.2.2: validate_plan PASS (0 errors), check_plan_parity PARITY PASS (both plans), Python+Rust sequence dry-runs regenerated and in verdict agreement; `coi play` over the 12-month growth fixtures reproduces timeline v2 exactly (phases 0–3 COMPLETE at months 1/3/5/8 — phase 2 completes via its exit gate despite its UNEVALUABLE placeholder entry, the non-wedge property in action — phase 4 ACTIVE, blocked on diesel stock 220 < 300 with the other three exit leaves TRUE; 21 orders recorded: 6 PREPARED with exact payloads, 15 SKIPPED with reasons) and over a single-state early-game sequence (phase 0 COMPLETE, phase 1 ACTIVE, 1 prepared / 5 skipped — the first-pass picture for the weekend). New: plans/WEEKEND-RUNBOOK.md (gate zero = save+reload proof on suite 0.5.1 before any play; first builder job = iron-v1 on the new save under Philip's confirm, optional concrete-v1 scratch-save rehearsal first). GAME-PLAN-FORMAT.md untouched — the format itself did not change.

## 2026-11-05 (evening) — Design system v1 implemented across the companion website

Philip called the UI "fucking awful" and delegated the last design decisions; adopted defaults: dark only, comfortable density, blue accent #5b9dff. The whole of serve.rs is restated on DESIGN.md: sidebar shell with the four nav groups + always-visible status strip (fetches game-info per render; OFFLINE is a state, never a fabricated "last known"), token stylesheet, token badges (UNVERIFIED/UNEVALUABLE dashed), the §3.9 confirm sheet verbatim (warning/danger band, summary, exact JSON, Run + Cancel peers), server-SVG charts (sparklines, flow pairs, runway bars with basis verbatim, machine-state stacked bars, power bars with UNVERIFIED average mini-bars, storage fill bars), the Log as a day-grouped feed, degraded states in-shell (D1–D3). Functionality frozen throughout — routes, fields, CSRF, tokens, §12 semantics, audit/activity writes untouched; layout() now takes &State for the strip, which was the one structural change. Deviations are listed in SPEC Addendum XVIII (Place in the Game nav group, no nav pills, productivity verdict lines kept as text under the bars, issue Act tables full-width, no runway hatching — no seasonal flag exists on Forecast). Verified: build clean 0 warnings, tests 17/17, check_place_page.sh + check_log_page.sh PASS, stub sweep all-200 populated + degraded in-shell, screenshots inspected (after-overview / after-issues / after-confirm in design-mock/). Stub gained a /coi/v1/power route (v06 synthetic capture) so the power panel is sweep-verifiable. dist/coi.exe restaged 6,160,589 B (sha256 9976089f…) — install alongside suite 0.5.1 + QoSBuilder when Philip's PC is back.

## 2026-11-06 (terrain sensing round) — CoiBridge 0.6.5 + companion Terrain page

Philip: "work towards that then" — closing the layout blind spot
(heights / earthworks / on-map ore). Recon round 13
(qol-recon/11-terrain-deposits.md, MODDING-NOTES round 13) found:
TerrainManager holds per-tile Fix32 heights (index = y*w + x,
65536 = 1 tile; tile = 2 m, TILE_VOLUME_M = 8 m3); entity Z IS
the floored terrain height in the same grid (WithHeightFloored ->
ExtendHeight) — terrain and topology are one coordinate space.
On-map deposits are terrain MATERIAL LAYERS, not entities:
remaining quantity = the game's own ToPartialProductQuantity
over today's layers, depletion-exact. Designations partition
cleanly via the mining/dumping managers.

Built: CoiBridge 0.6.5 (BridgeRoutesTerrain.cs, own DI class,
own try/catch; routes /terrain summary ~96x96 max,
/terrain/region capped 16,384 tiles, /deposits with 30 s scan
cache + connected regions, /designations; 0 warnings / 0 errors;
zip 27,032 B, sha256
3f26d817943e69244fe083bc176db848f76cb76ea0f8687b2a369ec96c238cf5).
Companion: fetch_live best-effort terrain+deposits; store schema
v4 deposit_state; Terrain page (Monitor) with SVG heatmap,
on-demand region fetch, earthworks estimator (cut/fill in
tiles-cubed and m3 x8; pure, 5 unit tests; CLI `coi terrain
deposits|region|estimate` + documented JSON shape). 22/22 cargo
tests; page verified against a synthetic v0.6.5 stub capture
(live-synthetic-v065; estimator numbers cross-checked
independently) and degrades cleanly on the v0.6.4 capture;
check_log_page + check_place_page PASS. SPEC Addendum XIX.
dist/coi.exe restaged (see SPEC addendum for size/sha).
Outstanding (weekend checklist): first live payloads from all
four routes on a real save; TerrainManager/designation DI in
game; region query binding live.

## [2026-11-06, morning] Earthworks guidance round (dump / flatten / mine)

Philip: "we need to write guidance on how to dump, how to flatten
and how to mine effectively" — plus his refinement that PLACEMENT /
snapping of designation grids is core, not garnish. Documentation
only; no code changed.

Verified from the 0.8.8 assemblies (ilspycmd) before writing:
- Designations are 4x4-tile cells on the global 4-grid
  (TerrainDesignation.SIZE_TILES=4, DesignationDataFactory
  GetOrigin = v*4); target = bilinear plane over 4 corner heights;
  mining + dumping fulfilment are separate per-vertex bitmaps on
  the same cell.
- Neighbour snapping is a HARD rule (TryCreateSnapToNeighbors):
  shared corners must match exactly, edge steps only 0 or ±1 tile,
  else placement refused. Ramp cells step exactly 1 tile per cell
  (CreateRampArea) — the game's sanctioned vehicle grade.
- Collapse physics (TerrainPhysicsSimulator): per-material
  [Min,Max]CollapseHeightDiff, probability ramps min->max, each
  slide moves half the excess diff; floor 0.125. Bands from
  TerrainMaterialsData: landfill 0.4/1.5, dirt 0.6/1.2, gravel
  0.7/1.6, sand 0.8/1.0, disrupted rock/ore 0.8/2.0, grass 0.9/1.8,
  coal 1.4/4.0, rock+bedrock+Fe/Cu/Au ore 2.0/5.0, hardened rock
  4.1/5.0, Ti/U ore 2.5/6.0. (Wiki's old slope table agrees on
  dirt — same physics.)
- Retaining walls (RetainingWallsData): RETAINED_HEIGHT_TILES=5,
  wall tiles DisableTerrainPhysics, placement window -5..0;
  Long = one cell edge. Costs from wiki (Short 4Fe+10, Corner
  6+12, Long 12+40).
- Buildings on landfill (UnstableTerrainValidator): only Landfill
  materials carry UnstableTerrainMaterialParam; >0.25 tiles of it
  in top layers without 0.75 stable cover => placement refused.
- Excavators (ExcavatorsData): T1 bucket 6, reach 2-5, dig
  thickness 1.5/1.0/0.5; T2 20, 3-7, 2.0/1.5/1.0/0.5; T3 60, 4-9
  (thickness array runtime-initialised — NOT traced, named as a
  gap); Amphibious 16, 4-9, MaxMiningOceanDepth 5.
- Dumping is a TRUCK job (DumpingJob.Factory builds for Truck);
  mining is excavators on tower-managed designations only;
  MAX_DUMP_HEIGHT_IN_OCEAN = 0 (TerrainDesignation static ctor).
- Wiki/community (labelled in the guides): tower control area max
  49 designations/side; tower-vs-global dumping-settings
  precedence; 5-down wall staircase pattern; port pads +2..+5.

Deliverables: guides-earthworks.md (three guides, placement as a
first-class section in each, "with our data" checks throughout);
new skill ~/workspace/custom-skills/local-coi-earthworks/SKILL.md
(decision rules, dump/flatten/mine checklists, and the agent's
9-step snapping procedure); one cross-pointer section appended to
local-coi-strategy ("Earthworks in the triage order").

## [11:06 Paris] Wiki Knowledge Programme — offline library + 7 new player skills

Philip's direction (2026-11-06): the whole CoI wiki readable at any time, especially patch notes, with skills and guidance written up for the Muse player. Built by a coordinator + 15 workers; documentation only — no game contact, no uploads, no GitHub pushes.

- **Mirror:** `~/workspace/coi-wiki-mirror/` — the full official wiki (wiki.coigame.com, MediaWiki 1.45.6) via its API (wikitext → markdown, tables/numbers preserved, images NOT mirrored — `[image: …]` text refs only). **683/683 main-namespace titles** (602 articles + 71 redirects = the wiki's own siteinfo counts; 670 files — 13 documented case-only slug collisions), proven complete by a final `refresh.py --dry-run` reconciliation: 0 missing, 0 stale, 0 extra. Per-page provenance in `manifest.tsv` (source URL, revision ts, words, status). `INDEX.md` = topic map grouped by game system + honest gaps section (pre-v0.7.1c patch notes absent from the source index; two verbatim wikitext tables; nested-recipe bullet artifact on a few machine pages).
- **Patch notes:** `changelog/` — 43 files, v0.7.1c (2025-05-18) → v0.8.8b (2026-11-05), + `changelog/INDEX.md` (version/date/headline). Current version from coigame.com (Changelog page + Update 4.3 blog post); v0.8.3 full notes recovered from the official Update 4.1 blog after the Steam-mirror page proved announcement-only; older versions from the Steam news archive, cross-checked.
- **Refresh tool:** `~/workspace/coi-wiki-mirror/refresh.py` — idempotent, re-fetches only pages whose wiki revision changed, `--dry-run`/`--limit`/`--titles`; removed pages are reported, never deleted. Recommendation (in `SKILLS-PLAN.md`, NOT applied): fold the weekly refresh into the existing `coi-patch-watch` cron.
- **Gap inventory:** `~/workspace/coi-wiki-mirror/SKILLS-PLAN.md` — wiki topic space vs the 14 existing skills; 7 P1 gaps, 5 P2 deferrals (water, forestry, nuclear deep-dive, silicon/computing, smelting-alloys).
- **7 new skills** in `~/workspace/custom-skills/` (family shape: SKILL.md + skill.yaml + references/, numbers sourced from mirror pages and cited, judgement labelled inline): `local-coi-farming-food`, `local-coi-maintenance`, `local-coi-population-housing`, `local-coi-logistics-vehicles`, `local-coi-trade-shipping`, `local-coi-pollution-health`, `local-coi-research-strategy`.
- **Wire-up:** `local-coi-playing` SKILL.md gained a "The Offline Library" section (mirror location, grep patterns, changelog location, refresh tool, trust order: live game > mirror snapshot > skill > memory). No other existing skill touched; `local-coi-earthworks` untouched.
- Notable worker findings: wiki has no "Ground Pollution" page (channel = Landfill Pollution); air-pollution 0.044 is a *Health* effect (growth is indirect); several wiki pages carry pre-U4 data flagged stale in the skills' sources; one mirror worker retried a rate-limited (429) request once after a ~95 s pause and recovered the batch fully — no content lost.

---

## 2026-11-06 — Mod ecosystem survey (MOD-SURVEY.md)

Philip: "explore all major popular mods and see what we want to
use / re-use / change", with mine-structuring mods flagged.

- Ecosystem = the official CoI Hub, full stop (~230+ mods,
  in-game Mod Manager since 4.2). Steam Workshop/Nexus/mod.io
  are empty for this game; GitHub is source hosting only.
  COI:Extended's Modding Exception was revoked 2026-04 — it is
  banned, not abandoned; all pre-2026 "top mods" lists are
  obsolete.
- Headline: Kayser's Auto Terrain Designations (49.5k / 89%,
  MIT) is the mine-structuring mod Philip sensed — contour-
  following designation generation, spike/quality filtering,
  median floor flattening, generated ramps, collapse overlay.
  BUT its manifest calls 4.3 support experimental (Build 625+,
  limited validation) — Hub "Stable" badge overstates it.
  Verdict: install (scratch-save shakedown first) + port its
  planner parameters into the companion/QoSBuilder side (MIT
  permits it; zero-Harmony posture means algorithms, not its
  Harmony inspector layer).
- Overlap audit: CoI_PriorityManager (iScripters, MIT, zero-
  Harmony) is MANUAL group management — complement, not rival,
  to QoSPriorityActuator; never both on the same fields in one
  save. Smart Mining Dump (Unlicense) is QOL-MODS #7/#8 in
  miniature → QoS feature sketch. UNMA's alarm model (MIT) is
  the reference for companion alerting (#5). Farm Optimiser's
  solver is companion-shaped, but no LICENSE file → ideas only.
- Save-safety red flag of the survey: Multi-Priority Balancers
  has a documented save-load crash (with ++ mods) and its
  hotfix shipped only via Discord pins. MZ Mining Fleet and
  Concrete Retaining Cube are not-removable-after-use. Our
  serializer paranoia is ecosystem-validated.
- Mine-structuring principles distilled (MOD-SURVEY §5) and a
  clearly-labelled community-practice addendum added to
  guides-earthworks.md: 5-level wall terraces, setback =
  ceil(depth/5)+1 sections, ramps = 1 section per level,
  spike filtering, median floors, local overburden sinks.
- Deliverable: MOD-SURVEY.md (21 mods categorised, top-5
  recommendations, UNVERIFIED register). Research only —
  nothing installed, no code changed.

---

## 2026-11-06 — Behaviour Distillation round (BEHAVIOURS.md)

Philip's framing: avoid mods that crash; the good mods are TEACHERS,
not dependencies — "we just want to figure out how to train YOU the
AI to behave similarly." Coordinator + 3 source-extraction workers;
documentation + one recon only — no builds, no installs, no game
contact, no pushes.

- **Deliverable:** `~/workspace/coi-companion/BEHAVIOURS.md` — six
  distilled behaviours, each as an agent-executable policy (inputs
  mapped to our bridge/companion surfaces, numbered decision rules
  with real numbers + file:symbol provenance, action channel,
  explicit LIMITS): (1) ATD pit planning (MIT, full source read);
  (2) Smart Mining Dump prefer-local-sink (Unlicense) — policy only,
  its reflection truck-decorator is NOT reproduced and no channel
  in our stack routes truck jobs, so it lands as a planning rule +
  an unbuilt QoS sketch; (3) PriorityManager manual groups (MIT) —
  group values 9/8/8/9 on the internal 0–14 scale, manual change
  ejects the building and wins, removal restores native defaults,
  plus the standing XOR rule vs QoSPriorityActuator; actuation =
  §12 prepares of the three shipped priority commands; (4) UNMA
  alarm model (MIT, targets 0.8.6c — not 0.8.8-verified) — 4
  severities, flat AND/OR, per-condition hysteresis,
  K/KQ/KG/KGQ acknowledgement lifecycle, shipped default
  thresholds; advisory, companion Issues layer (QOL-MODS #5);
  (5) Auto Forestry Designations (MIT) — tower-area scan,
  all-16-subtiles eligibility, fail-closed sustainable-yield
  sizing floor(capacity/yr ÷ 12), equal-split truck quota;
  (6) Research Queue — prerequisite-clamped ordering as a
  plan/runner policy; vanilla silently discards out-of-order
  completions and vanilla cancel wipes the queue; reordering has
  no shipped-command channel (advisory).
- **ATD headline parameters (all CODE-verified):** spike filter =
  bedrock top >4 below the median of all 8 neighbours (all 8 must
  have bedrock; NO thickness criterion in the shipped detector —
  the survey's open question is settled: there isn't one); purity
  arrays L1–L4 (0.25/0.05/0.05/2 → 3.0/0.75/0.75/40), bottom-density
  walk stops at the first failing seam; MaxLayers default 30 from
  the cell-minimum surface; floor = component percentile, strength
  default 5 = median, lower-only at purity Off (the MOD-SURVEY
  "5/10 median" figure is verified); MaxHeightDiff default 1
  (1–3), smoothing deepens only; corridor clearance effective
  default 1 under Auto (source carries two stale "2" defaults in
  dead paths — flagged in BEHAVIOURS.md, not papered over); safety
  presets Min(0.8,0)…Max(1.1,4), Med(0.9,2) default, building
  buffer constant 3 tiles, iterative fail-closed removal loop;
  legacy ramp = exactly 1 level per 4-tile cell; routed-ramp
  width/slope NOT PUBLISHED (runtime-derived from vanilla
  pathfinding params); mining body = ONE AddTerrainDesignationsCmd
  batch + per-origin read-back verification (BATCH_SIZE=30 does
  NOT chunk it).
- **Earthworks skill v2:** `local-coi-earthworks` snapping
  procedure upgraded 9 → 13 steps, geology-aware (deposit columns
  → spike filter → purity gates → depth caps + median floor →
  corner smoothing → iterative landslide safety → estimator →
  local-sink-first → single-batch placement + read-back).
  Distilled content in a clearly-marked community-derived section
  with ATD citations; all prior code-verified content preserved.
- **Recon round 14 (MODDING-NOTES):** designations are
  EXECUTABLE-BY-AGENT via the QoS scheduler route —
  `AddTerrainDesignationsCmd` / `RemoveDesignationsCmd` ship in
  the 0.8.8 assemblies (modify = Add at same origin);
  `QuickFulfillDesignationsCmd` excluded by posture; web §12
  channel blocked by the round-10 collection rule; AFD + ATD
  both ship this exact channel in the wild. Executor (QoSBuilder
  job-type extension) is Philip's build decision — not built.
- **Playing skill:** new "Learned Behaviours" section pointing at
  BEHAVIOURS.md + earthworks v2 + the round-14 verdict.
- **Not distillable (named in BEHAVIOURS.md):** vanilla sink
  selection inside DumpingJob.Factory (not in SMD's repo); ATD
  routed-ramp numeric width/slope, material slope numbers,
  flattening blends, batch ordering (runtime-derived or
  nonexistent); RQ queue cap / paused-research handling (absent);
  UNMA 0.8.8 verification (teacher targets 0.8.6c).

## 2026-11-06 (evening) — Code Quality Audit (QUALITY-AUDIT.md)

Read-only audit at Philip's ask ("are we effective, do we have
equiv tests, are we writing high quality?"): 3 parallel workers —
C# style/practices, Rust style/practices (incl. a live
`cargo test` run: 22/22 pass, 0 build warnings), effectiveness +
test-equivalence matrix — synthesised into
`QUALITY-AUDIT.md`; `qos/ARCHITECTURE.md` gains a tight
"Coding standards" section (incl. the saved-state law superseding
its own stale blanket `can_remove: true` claim). No code changed,
no artefact restaged.

Headline: discipline is high and uniform (DI pattern, serializer
pairs 4/4 correct, game-constants-only money, zero TODO/FIXME),
but **verification is inversely correlated with blast radius** —
plan evaluation/productivity (advisory) carry dual engines +
parity + fixtures, while the save/mutation tier (QoS guard
arithmetic, serializer pairs, Builder's C# hash) is
compile+inspection only, with zero C# tests repo-wide. Two real
defects in serve.rs: §12 audit writes are best-effort
(`serve.rs:2513-2528` — an applied command can leave no audit
row), and 7 `.lock().unwrap()`s + no `catch_unwind` make a
mutex-poison cascade a full-website wedge (R1). ARCHITECTURE's
stale `can_remove` rule is a doc DEFECT (code is right).

Remediation (12 items, sequenced in the audit): before the
weekend session — (1) serializer-pair lint in build.sh,
(2) §12 audit fails loudly, (3) execute the C# canonical-hash
vector via the probe harness, (4) poison-tolerant locking.
Guard-arithmetic extraction+tests gate *unsupervised* enabling;
CI wiring, clean-build proof, DiscoveryReport retirement and
serve.rs helper extraction follow. QoSBuilder findings are
provisional — designation round was editing it concurrently.

## 2026-11-06 (evening) — Weekend self-sufficiency pack: placeholder retired, runbook v2, bundle

Philip: "You do it all yourself — the point is you play" + "have the
stuff updated for all of it". Documentation/prep only; no game
contact.

**1. The "Construction" placeholder is RETIRED — from the shipped
assemblies.** Decompiled `refs/Mafi.Base.dll` / `Mafi.Core.dll`
(ilspycmd, ~/.tmp/coibase + coicore). Every research node registers
via `ResearchNodeProtoBuilder.Start(displayName, Ids.Research.X)` —
the display→id map is primary source. "Construction" =
**ResearchCpPacking** (ConstructionResearchData.cs: Start("Construction",
Ids.Research.CpPacking), cost 4 RP; unlocks AssemblyManual + the
CpAssembly/MechPartsIronAssembly/ElectronicsAssembly recipes — the
plan's exact unlock list). The October candidate ResearchCp1Production
is a GOAL id (GoalsData.cs: GoalToResearchNode "ResearchCp1Production"
targets Ids.Research.CpPacking) — that is why it sat among goal
strings in the extracted list behind coi/data/research_ids.txt.
Live cross-check: Philip's bridge-research.json (127 researched)
contains the exact string "ResearchCpPacking". The wiki mirror and
the dataset JSON carry no node ids — assemblies were the only
source that could settle it. Five siblings retired from the same
registrations + one asserted id CORRECTED:
Construction II = ResearchCp2Packing · Synthetic Rubber =
ResearchRubberProduction · "Concrete Production" =
ResearchConcreteAdvanced (shipped display "Advanced concrete";
unlocks RotaryKiln + ConcreteMixer + Crusher) ·
ResearchPowerAndMaintenance confirmed (display "Maintenance";
"ResearchPowerMaintenance" is not a node) · Groundwater Pump =
ResearchUndergroundWater · **ResearchShipDockRepair → ResearchRepairDock**
(the phase-6 order named another goal id and would have failed at
prepare; the plan's old note had the relationship backwards).

**2. Plan fresh-start-v1 → v1.3.0.** All seven ids written into the
orders/gates with sourced notes + verify blocks; assumptions
rewritten; JSON regenerated (84,316 B). Fixtures repaired:
gen_growth_fixtures.py RESEARCH table + honesty notes (goal ids
ResearchCp1Production / ResearchRubber / ResearchCp2 →
ResearchCpPacking / ResearchRubberProduction / ResearchCp2Packing),
early-game fixture + README, all 12 growth months regenerated.
Verification: validate_plan PASS, 0 errors, **placeholder research
ids: 0** (16 concrete). Python + Rust walks agree: phases complete
at months 1/3/5/8; phase 2's entry gate is EVALUABLE for the first
time (was UNEVALUABLE by design under the placeholder); phase-4
ACTIVE, halt unchanged on diesel stock 220 < 300 (scripted fixture
pace — a plan finding, not a defect). Sessions + all four dryrun
reports regenerated at v1.3.0; p1-research-construction now
PREPARES `{"nodeId":"ResearchCpPacking"}` (growth walk: 11
prepared / 16 skipped).
**Parity caveat (engine finding, not a plan defect):**
check_plan_parity now FAILS on exactly one order against the live/
capture: with ResearchCpPacking already researched, the Python
reference HELDs p1-research-construction while the Rust runner
would still PREPARE it (the placeholder previously masked this by
holding both). Gates all match; dragon-isle parity OK. Aligning
the already-researched hold into the Rust runner is a future code
round — deliberately not touched here.

**3. Runbook v2** (plans/WEEKEND-RUNBOOK.md): PHILIP section is the
physical minimum only (files across + wholesale folder replace with
the 0.1.1 warning, Hub installs, game+mods on, `web_api_start`, the
exact tailscale serve line from ACCESS.md, start coi.exe, say
"up"). Everything else is Claudius's, including the NEW pre-run
sequence: Hub-mod shakedown on a scratch save (ATD is
self-declared experimental on 4.3 — designation read-back + queue
smoke tests, drop-on-doubt) BEFORE gate zero (save+reload proof on
the fresh New Haven run). The in-game Construction-id reading step
is deleted — obsolete. §12 confirm-each-step posture, Log watch
list and stop conditions carried over.

**4. Bundle** dist/weekend-bundle/ + BUNDLE-README.md (install
card: order, destinations, quit-game-first, 0.1.1 warning):
coi.exe 6,215,930 B e71ff3de… · CoiBridge-mod-0.6.5.zip 27,032 B
3f26d817… · QoS-suite-0.5.1.zip 126,268 B a2ed9232… ·
QoSBuilder-mod-0.1.0.zip 35,184 B 1e5aa745…. Exe + bridge verified
against the SPEC.md packaging record; suite + builder sizes match
their NIGHT-LOG records (shas first recorded in the bundle readme,
measured from the files). **QoSBuilder is 0.1.0 — no 0.2.0 zip
exists in dist/, so designation jobs are NOT in this bundle**
(flagged in the readme). INSTALL.md checked: already current
(6,215,930 B / bridge 0.6.5) — no edit needed.

Still owed (unchanged): suite 0.5.1 save+reload live proof, bridge
0.6.5 terrain routes live, first QoSBuilder job end-to-end — all
weekend checklist items in runbook v2, sections A/B/D.

QoSBuilder designation jobs — STAGE 1 of 4 done (read-only
scoping; no code changed, nothing built). Design note:
qos/DESIGNATION-DESIGN-NOTE.md. Assembly-verified (0.8.8
Mafi.Core.dll, ilspycmd): DesignationData = origin + 4 integer
corner heights (HeightTilesI) + Fix32 centre; CTOR ORDER is
Origin, PlusX, PlusXY, PlusY (PlusXY before PlusY — field
declarations disagree); centre is the corner mean, so the job
file derives it, never carries it. Kind = the Add command's
single Proto.ID per batch (MiningDesignator / DumpingDesignator
/ LevelDesignator; Forestry excluded v1). Manager semantics:
Add result reports SUCCESS WITH ALL ORIGINS even when cells are
silently refused (not allowed / out of limits / blocking
building) — read-back is the only verdict; Add replaces any
kind + deletes surface designations at the origin; duplicate
origins in a batch = last wins; Remove is proto-agnostic and
its result is always empty (game-side Lyst never populated).
Bridge gap: /designations emits min/max band only, no corners —
flat-cell verdicts exact, sloped-cell verdicts partial until a
0.6.6 corners addition (flagged decision). The v065 synthetic
designations capture disagrees with the route code (merged
areas, "MiningDesignation" vs code's "MiningDesignator") —
correct in stage 4. Planner reality: pads fully plannable from
/terrain/region today; ATD pit generation is NOT computable
from /deposits (no per-column layers served) — pit core is
specified fixture-fed, generation gated on a layers route
(flagged decision). Schema v2 + cell cap 200 proposed; mapping
reuses the v1 ladder/folder/events/token ceremony wholesale,
with NO new BuilderService fields (serializer pair untouched).
Stages 2–4 planned in the note (mod C# / designate.rs / serve
+ checks). Nine open questions recorded, incl. shipped-build
assert behaviour (unprovable from the DLL).
2026-11-06 19:24 UTC

## 2026-11-06 (evening) — Bridge 0.6.6: designation read-back made exact

Philip approved the design note's flagged decision 1 ("yes fix
more", 20:20 Paris). Micro-fix, one route: /coi/v1/designations
rows now also carry `corners` (four corner target heights,
integer tiles, DesignationData ctor order origin/+X/+XY/+Y)
and `centerRaw` (centre target, raw Fix32) — read from
TerrainDesignation.Data, which the runtime object retains in
full. Additive only; every existing field unchanged. With
corners served, the companion's before/after verdict for
QoSBuilder designation jobs is exact for sloped cells too,
not just band-level. Design-note open question 4 settled from
the code path: proto emits "MiningDesignator" (IdsCore
constants via RegisterData), NOT the v065 synthetic's
"MiningDesignation" — code-derived until a live capture.
SPEC Addendum XXI + MODDING-NOTES round 15 written; the v065
synthetic designations capture predates the change and stage
4 must regenerate it. Build 0 warnings / 0 errors.
dist/CoiBridge-mod-0.6.6.zip 27,399 B, sha256
806b7a9e33559ce15e8f53728b33a73a98a69e184af7b1d3094aac92ebcffe10;
0.6.5 zip re-verified untouched (3f26d817…38cf5). Live
verification joins the weekend checklist.
2026-11-06 19:23 UTC

## 2026-11-06 (evening) — QoSBuilder 0.2.0: designation jobs (stage 2 of 4)

Stage 2 of the designation programme built, per the stage-1 design
note: the mod gains the schema-2 designation job family alongside
blueprints (dispatch on `jobType`; v1 path behaviour-identical).
ONE AddTerrainDesignationsCmd (kind = the command's Proto.ID,
allowlist Mining/Dumping/LevelDesignator) or ONE
RemoveDesignationsCmd per job; cells built via the 5-arg ctor —
corners only, centre game-derived. Same claim-before-schedule /
single-use / expiry / gameId / sha256 discipline; events kind
"designation"; refusal ladder per the note (structural
malformed-cells pre-hash by construction — the canonical is
rebuilt from the parsed cells; remove duplicates deduped, add
duplicates refused). Config gains max_cells_per_job (default
200). NO new fields on BuilderService — serializer pair verified
untouched in the compiled DLL (ilspycmd: five fields, calendar
only). Canonical v2 vectors EXECUTED on the C# leg this time
(qos/build/canonprobe, source-compiled AND the packaged net48 DLL
by reflection) against independent Python expectations — v1
a8b4498e…cbf, v2-add e4166dd3…7f7, v2-remove 78fb273c…fa4, all
MATCH (the v1 C# leg's first execution closes that audit
finding). Build 0 warnings / 0 errors.
dist/QoSBuilder-mod-0.2.0.zip 47,373 B, sha256
680d81eac4e68afa5d8e7a1354f5548891b63e56b33dffe07c4a74aa03005131;
all 31 prior dist artefacts sha-verified untouched. Live proof
(first designation job + read-back diff + save/reload) joins the
stage-4 shakedown. Detail: qos/BUILD-NOTES.md round 9.
2026-11-06 19:31 UTC

## 2026-11-06 (evening) — Designation planner: stage 3 of 4 (coi/src/designate.rs)

Stage 3 built standalone, per the design note: `coi/src/designate.rs`
(std + serde_json + sha2 only; NOT wired into the crate — stage 4
adds `mod designate;` + the serve routes once the remediation round
clears main.rs/serve.rs; no dist restage). Canonical v2 acceptance
gate PASSED: the Rust mirror of BuilderJson.CanonicalDesignation
reproduces both published round-9 hashes exactly (add e4166dd3…7f7,
remove 78fb273c…fa4) via the low-level function and the typed
build_add_job / build_remove_job builders (job file = canonical +
trailing sha256 field, the v1 assembly). Pad planner complete:
outward snap, level cells, terrain::estimate volume arithmetic,
boundary-vertex edge-step warnings, typed refusal ladder
(EmptyRectangle → UnsupportedProto → ImplausibleTarget →
OutsideGrid → OverCellCap). Pit core as pure functions over a
Column/ColumnGrid input type: spike cutoffs (residual 4, all-8
neighbours), visible-thickness intervals, L0–L4 purity gates +
evaluate_origin, the bottom-density dig walk (first failure stops),
rule-4 depth caps, components + size filter, percentile component
floors (strength default 5 = median), corner build + MaxHeightDiff
smoothing (deepen-only). 13 tests, all passing, zero warnings, run
standalone in a scratch cargo project by #[path] include
(cargo test --offline) — the corner test caught a real off-by-one
in build_corners' vertex-grid bounds on first run; fixed in code.
Four interpretation points the distilled ATD text doesn't pin are
recorded in the note's new §5 (even-count median convention,
density-walk visible-thickness basis, per-origin depth =
deepest column dig, true mean vs ATD's running average for
CornerInit::Mean). Stage 4's wiring list is written out in full
in DESIGNATION-DESIGN-NOTE §5. Detail: that note.
2026-11-06 19:31 UTC

## 2026-11-06 (evening) — Audit remediation: items 1/2/4 + runner parity fix

Philip approved the fix round ("yes fix more"). Four items landed,
all verified; QUALITY-AUDIT.md gained a "Remediation status"
addendum with the full evidence table.

(1) SERIALIZER-PAIR LINT (audit #1 / T1): new
tools/check_serializer_pairs.py discovers calendar subscribers by
the `.Add(this, ...)` pattern — no hardcoded list; attribution to
the outermost class (nested RecoveryMark/Observation helpers tried
to steal it during development; fixed) and the attribute check
requires a real attribute line (the first red-proof attempt passed
on a doc-comment mention; fixed — a lint that passes on prose is
worse than none). Asserts attribute + static Serialize/Deserialize
pair + owning manifest can_remove_from_saved_game:false. Wired
into qos/build/build.sh, qos/build/build-builder.sh and
bridge/vm-build/build.sh. Green on the real tree (4/4), red on a
scratch copy for each break class (attribute / Serialize /
Deserialize / manifest flag), scratch deleted. Suite build
re-run live with the lint: C# 0/0; every pre-existing dist zip
byte-identical afterwards (the one transient diff — suite 0.5.1
re-packed around the since-updated SUITE-README — was restored
from backup; DLL payloads were identical throughout).

(2) FAIL-LOUD §12 AUDIT WRITES (audit #2 / D1): do_apply and
do_place_confirm in serve.rs no longer swallow store errors.
Failures collect into an audit_err surfaced as an AUDIT WRITE
FAILED danger banner ("this action is missing from the audit
trail"), the apply headline degrades to "Command executed — NOT
audited", the place write-failed page's audit sentence is now
conditional (it previously asserted a record that might not
exist), and the server logs "left NO audit record". Healthy path
unchanged: check_log_page + check_place_page PASS; new
tools/check_audit_fail_loud.sh proves the failure path (stub
game, DB file swapped for a directory after prepare — chmod does
not bite as root — both pages surface the error). The snapshot
activity write (old serve.rs:5007) was deliberately left: not a
§12 apply.

(3) POISON-TOLERANT LOCKING (audit #4 / R1): one lock_map helper
in serve.rs recovers the guard from PoisonError; all 7 former
.lock().unwrap() sites (sessions/prepared/prepared_jobs) use it.
No unwrap remains outside the helper's doc comment.

(4) RUNNER PARITY FIX (self-sufficiency pack caveat): decide_order
now takes the recording step's State and SKIPs a ResearchStartCmd
whose node already appears in the capture's `researched` list —
positive evidence only, so pre-0.6.2 captures without the list can
never hold an order (mirrors research.completed's UNEVALUABLE
discipline). 3 new unit tests; cargo test 25/25. Two corrections
to the pack's account, established from the tree: the parity
failure's mechanism was the committed Python reference
plans/fresh-start-v1.dryrun.md being STALE (header: plan v1.1.0,
placeholder args; the pack regenerated every other report) — the
fix regenerates that reference from the current plan with the
Python engine; no render_order semantics changed in either
engine. And the order the hold actually changes is not
p1-research-construction (recorded at step 0, before CpPacking
completes — correctly still PREPARED) but growth-walk
p3-research-trading-dock: prepared → skipped, the single changed
outcome across all 33 walked orders; phase histories/finals
identical; sessions + transcripts regenerated.
check_plan_parity: PARITY PASS on both plans (fresh 44 gate items
26/26/13; continuation 49 items 31/35/7).

RESTAGE: dist/coi.exe = 6,219,897 B, sha256 01397c50…ae2924
(Windows release build 0 warnings); weekend-bundle exe copy +
BUNDLE-README row updated to match. Audit items 3 (C# hash
vector) and 5 (QoS guard test extraction) remain open — owned by
the QoSBuilder stage-2 round and the next wave respectively.

## 2026-11-06 (evening) — Designation serve integration: stage 4 of 4 (round complete)

The final designation stage. coi/src/designate.rs is wired
into the crate (mod designate in main.rs; the pit core's
production dead code is allowed at module level with the
reason in the file header — it is fixture-fed until the
bridge per-column layers route exists).

SERVE: routes GET /designate, POST /designate/prepare,
POST /designate/confirm; nav entry in the Game group beside
Place (NAV 14 -> 15) + nav_active titles. Pad prepare pins
the gameId like /place (502 refusals), fetches
/coi/v1/terrain/region as rect + 1-cell margin, runs
plan_pad, and renders the section-12 confirm sheet: cells,
cut/fill volumes, EVERY edge-step warning verbatim, EVERY
replaced designation (Add replaces any kind + deletes
surface designations), blocked-cell expectation stated.
Remove prepare takes origins ONLY as sel_<i> checkbox
values rendered from the live /designations read-back; a
selection that no longer matches is refused (Stale
selection) — no free-typed origin path. Confirm reuses
PreparedJob (120 s single-use), writes ONE file
tmp-then-rename via build_add_job / build_remove_job, and
writes audit + activity rows (PlaceDesignationJob /
category designation) under the remediation round's
fail-loud pattern — no if-let-Ok swallowing reintroduced.

VERDICT: designation_verdict_panel diffs the job against
the CURRENT /designations (new designate.rs helpers:
existing_from_json, cell_match, diff_add, diff_remove;
corner-exact on bridge 0.6.6, band-only counted + labelled
partial on older bridges). Rendered on the confirm outcome
page and as the latest-job panel on every /designate visit
(reconstructed from the activity payload). The write alone
never claims placement: statuses LANDED / PARTIALLY LANDED
/ SCHEDULED — verdict pending.

SYNTHETIC FIRST: live-synthetic-v065/bridge-designations
.json regenerated to the 0.6.6 route shape (per-cell rows,
proto "MiningDesignator", corners [6, 9, 9, 6], centerRaw
491520); correction noted in SYNTHETIC.txt + SPEC XX.

VERIFY: cargo build 0 warnings; cargo test 41/41 (25
pre-existing + 13 designate stage-3 + 3 new read-back
verdict tests). tools/check_designate_page.sh PASS
(gameId pin, 6-cell pad with edge warnings, exactly one
add file re-verified under an independent Python v2
canonical, replay refused, latest-job panel, remove flow
+ remove file verified, /log carries PlaceDesignationJob,
bridge-down prepare 502, /designate 200-with-notice).
check_place_page.sh, check_log_page.sh and
check_audit_fail_loud.sh still PASS.

DOCS: SPEC Addendum XX (contract as built);
DESIGNATION-DESIGN-NOTE.md section 6 (as-built + the
weekend shakedown that closes the round: scratch-save pad
add, remove, save+reload, one replace case — see there);
BEHAVIOURS.md recon-verdict status updated (executor
built; pads executable, pit generation stays advisory
until the layers route); INSTALL.md refreshed (builder
0.2.0 + /designate, bridge 0.6.6, exe size).

RESTAGE + BUNDLE: dist/coi.exe = 6,345,284 B, sha256
33f063cea7ee5e6b485e42bc3aa0e52fbee0e8a992d2c9350c
259e171306dd1d (Windows release build 0 warnings).
weekend-bundle refreshed: CoiBridge-mod-0.6.6.zip
(27,399 B, 806b7a9e...ffe10) replaces 0.6.5,
QoSBuilder-mod-0.2.0.zip (47,373 B, 680d81ea...5131)
replaces 0.1.0, exe replaced; BUNDLE-README rows
re-measured from the files + the designation install
line (builder 0.2.0 includes designation jobs, ships
disabled). The follow-up push carries all of it.

## 2026-11-06 (late evening) — Design V2 implemented + login removed + must-fix badge

DESIGN.md V2 addendum (§§V2.1–V2.9) implemented in the addendum's
own step order: chrome (mega-nav top bar, left rail, crumbs) →
tokens/components/icons (20-glyph inline sprite, stat2/figstrip/
delta components, compact tables, contrast lifts) → number-
semantics sweep (§V2.5: sign colour only on differences with
mandatory arrows, measured zero = muted flat chip, absolutes
neutral, amber only on findings) → Overview + Issues (issue2
cards with per-class figure strips and the §V2.7 CTA table;
parked class gets the no-action treatment) → directory pages
(Products facets + 60/page server pagination, Research tabs +
/research/node, Entities ?view= sub-views, Catalogue facets,
Log 50/page) → remaining pages → verification. Philip's two
additions landed in the same round: the top-bar must-fix badge
(GET /api/v1/must-fix-count, server-side cache fed by page loads,
never fetches the game; "last known" state when unreachable) and
the complete removal of the login concept (login page, sessions
and username deleted; per-form CSRF tokens kept; Host-header
check added; web_users table vestigial; reset-password moot).

VERIFICATION: cargo build 0 warnings; cargo test 47/47 (6 new:
delta semantics, signed formatting, Host check, badge
endpoint/states, power panel). All check scripts PASS —
check_badge.sh (new), check_place_page, check_log_page,
check_designate_page, check_audit_fail_loud (the fail-loud §12
pattern intact), check_plan_parity, check_serializer_pairs.
Check scripts moved to the no-login flow (companionUser is now
"local"); their token extractions hardened against SIGPIPE on
the larger V2 pages. One real rendering bug found by screenshot
and fixed: the fig label class collided with v1's fix-later
quadrant class (inset warning shadow ate the labels' first
letter) — renamed to figl/figv. A pre-existing flaky plan.rs
test (temp-dir race) fixed in its helper. §V2.5 examples
verified in rendered markup (diesel −77 TM / −28 LM red ▼,
stored 5,992 neutral, bar 5,188/7,800; hydrogen +295 LM green ▲;
coal −211; bread −15; steel 1,108 / 24.1 months neutral;
broken glass 2.0 months amber; crude 9,713/9,000 full amber
bar). Heights at 1440×900 (shot2-*.png in design-mock/):
Products 2,690 · Research 949 · Entities 1,306 · Issues 3,394 ·
Overview 3,422 (over the ≈2,700 target with all spec caps
applied — the capture is issue-dense; recorded in DESIGN.md).

RESTAGE + BUNDLE: dist/coi.exe = 6,504,511 B, sha256
c0039faeb1ef092d251babcf55ba2d3b1e8b5a98935d060b66c83aadb05537ee
(Windows release build, 0 warnings), replacing 6,345,284 B /
33f063ce...dd1d. weekend-bundle exe + BUNDLE-README row updated;
the three mod zips verified byte-identical (806b7a9e...ffe10,
a2ed9232...cc52626, 680d81ea...05131). INSTALL.md and
plans/WEEKEND-RUNBOOK.md login wording updated to match (no
login; build size line refreshed). DESIGN.md carries the
"Implemented" closing note with the full numbers.
