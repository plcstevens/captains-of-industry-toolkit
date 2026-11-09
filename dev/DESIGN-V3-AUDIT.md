# Design V3 — Audit (2026-11-09)

Audit phase only: no production code was changed for this document.
Graded against Philip's six principles in `DESIGN-V3-BRIEF.md`.
State audited: the site as built by Design V2 (DESIGN.md addendum,
Implemented 2026-11-06) plus the 2026-11-09 privacy cleanup (SPEC
Addendum XXII, repo head `ed4d133c`).

**Method / evidence.** Every page was rendered fresh for this audit
(debug build of the current crate against `tools/stub_game_server.py`
serving the `live-synthetic-v065` capture — bridge 0.6.5-era payloads,
223 machines, 283 storages, 132 products, 15 board issues, game paused
at 388-2-27) and measured at 1440×900 with headless Chromium
(`document.documentElement.scrollHeight`). The v2 3-viewport rule is
≈2,700 px. Pages measuring **949 px** are at the shell's minimum
height, i.e. ≤ 1 viewport. Degraded behaviour is graded from the
render paths in `coi/src/serve.rs` plus the populated/empty renders
above (empty `coi.db`, stub capture with several bridge routes
absent). Canon evidence is file-referenced throughout §3.

---

## 0. Headline findings

### Top 5 problems

1. **Two directory pages silently escaped the v2 height discipline.**
   The Command Catalogue renders all 240 commands on one page:
   **15,264 px** (17 viewports). The Research catalogue tab renders
   all 250 nodes on one page: **10,786 px**. Entities' Machines
   sub-view renders 110 proto rows unpaginated: **6,401 px**.
   Products and Storages *are* paginated at 60/page — the pattern
   exists in the codebase and was simply never applied to these
   three. (Principle 5, and v2's own rule.)
2. **Overview (3,422 px) and Issues (3,394 px) are still over the
   3-viewport rule on an issue-dense run**, unchanged from the v2
   implementation note. Every v2 cap is applied; the board itself
   is the weight. The mitigation (rail anchors) helps a returning
   user, not a first-time one. (Principle 5.)
3. **No page states its intent as a contract — the summary lines
   exist but describe, they don't declare.** Every page does carry
   a one-line `page_head` summary (a genuine v1/v2 win), but the
   lines mix intent, mechanism and provenance ("Read through
   CoiBridge — the vanilla API has no routes for any of this"),
   none states the page's actionable/informational balance, and
   developer vocabulary leaks into player surfaces (Plans: "Machine-
   executable plans (GAME-PLAN-FORMAT v1)… run `coi plan-check`;
   the supervised session walk is `coi play`"). A stranger cannot
   tell, from any page, whether they are there to *look* or to *do*.
   (Principles 1 and 6.)
4. **The reasoning layer is invisible where it matters.** The
   engine's triage logic — the importance tiers that decide Fix
   now vs Ignore, the quadrant rule, the stall-confirmation rule,
   the forecast horizons — exists only in code (`coi/src/main.rs`,
   `productivity.rs`). The player rendering shows outcomes (a
   quadrant badge) with a one-line `quadrant_why`, never the rule
   that produced them; the agent rendering (strategy skill) states
   a *different-looking* ordering (death-spiral triage) that the
   player cannot reconcile with the board. This is the canon drift
   of §3, and it is exactly what the brief's guidance domain exists
   to fix. (Principles 1, 4, 6 + the canon workstream.)
5. **Data-state honesty is good at page level, patchy at section
   level.** The D1/D2/D3 patterns exist and mostly fire (Trade's
   four "arrives with CoiBridge v0.6…" placeholders, Entities'
   three render paths, Terrain's `degraded_bridge`). But: section
   placeholders name a *version floor* as the cause even when the
   real cause is "route absent/empty", Product detail's sources
   panels render raw red 404 boxes against any capture lacking the
   sources route (stub artefact today, a real older-game shape
   tomorrow), and the must-fix badge's red state has never been
   seen against live data (both captures have zero Fix-now issues;
   it is unit-tested only). (Principle 4.)

### Top 3 structural proposals

S1. **A fifth top-level domain — Guidance — as a peer of Monitor /
Game / Library / System**, with the brief's three classes as its
sub-pages: *Learn the game* (the 10 existing guides, re-homed, with
wiki link-outs), *Learn the tool* (a Superintendent manual that does
not exist yet), *Why it suggests* (the reasoning layer: per-class
why pages generated from the canon, §3). Guides leave Library;
Library shrinks to the Catalogue alone and is renamed accordingly
(see §2 — proposal: Library dissolves; Catalogue moves under a
renamed "Reference" or into Game; the tree in §2.3 decides).

S2. **Page contracts.** Each page gets a declared contract, rendered
in the page head and enforced by the audit's checklist: *intent*
(one line, player vocabulary), *domain* (what it owns), *balance*
(informational / actionable / mixed, with the primary action named
when actionable), *data contract* (what it needs; what full /
partial / none look like). The contract replaces today's descriptive
summary line. This is Philip's principle 1+6 made mechanical.

S3. **One canon, one file.** A structured canon in the repo
(`canon/` — §3.4) from which (a) the Why-it-suggests pages render,
(b) the issue `fix`/evidence copy is generated or checked, and
(c) the executable thresholds cite their canon id in code comments
and in a generated constants cross-check. The skills then *point
at* canon entries instead of restating numbers. §3 inventories
what exists and every disagreement found; two are material
(C-2, C-3).

---

## 1. Per-page audit

Heights are the fresh measurements above (v065 stub capture,
1440×900) unless marked *(v2 note)*. "Intent" quotes the line the
page *should* carry under the v3 contract, then grades today's.

### 1.1 Overview — `/` — 3,422 px ✗

- **Intent it should carry:** "Your whole run on one screen: what
  needs you, what happens next, and the products you pinned."
  Today's summary — "The run at a glance — what needs attention,
  what happens next, and the products you pinned" — is the best in
  the app and already earns it. Keep verbatim.
- **Domain purity:** Mostly clean. Foreign content: the game
  controls (pause/speed) live here as a compact card *and* on
  Actions — a deliberate duplication, defensible, but it makes
  Overview partly an action page while presenting as a monitor
  page. The live screenshot is Monitor-domain but is the page's
  single largest visual element and answers no question the stats
  don't; it is Overview's "poster", not its content.
- **Data states:** Full: complete. Game-off: `degraded_game`
  callout + power panel absent + stored sections — good D1.
  Partial (no bridge): power/machines figures vanish silently —
  the tiles render nothing rather than a labelled gap (the v2
  "absent data renders nothing" rule, which principle 4 now
  overrides: absence should be *named* where the tile was
  expected). No-history: sparklines absent without note.
- **Glanceability:** First screen = stat tiles + top of Attention.
  Good. The failure is below: Attention (15 issues on this capture)
  - Outlook + Pinned push the page to 3,422 px despite caps.
- **Actionable vs informational:** Mixed, undeclared. Take-snapshot
  button in the header, pause/speed card, issue CTAs — inside an
  informational page. The CTAs are §V2.7-correct (they land on
  detail/prepare), so the sin is only the missing declaration.
- **V3 changes (ordered):** (1) Declare the balance in the contract
  ("informational; two embedded actions"). (2) Cap Attention to
  Fix-now + first Fix-later on Overview with the counts as the
  link (the board is Issues' domain — Overview should *summarise*
  the board, not host a second copy of it; today it renders the
  same `.issue2` cards). (3) Move the screenshot below Pinned or
  behind the rail anchor it already has; it currently interrupts
  the read. (4) Name absent bridge figures with a one-line D2
  strip in the stats row instead of silent omission.

### 1.2 Issues — `/issues` — 3,394 px ✗

- **Intent:** "What to fix, in what order — and why it ranks there."
  Today: "What to fix, in what order — generated live from flows
  and storage levels." The mechanism clause should move to the
  how-it-works details; the *why it ranks* clause is missing and is
  the page's actual promise.
- **Domain purity:** Clean — the board, productivity findings, and
  nothing else. The markdown/JSON export buttons in the header are
  System-domain (records) squatting in a Monitor header; minor.
- **Data states:** Game-off → degraded callout, board absent
  (correct: no board without flows). No bridge: productivity
  section degrades (machine data is bridge-only) — the section
  renders its INSUFFICIENT treatment; acceptable. Empty board:
  proper empty states per quadrant (v2 fix, verified in code).
- **Glanceability:** Quadrant sections stack; with 15 issues the
  first screen shows Fix now + part of Fix later — the right
  order. 3,394 px total: the cards are at their ≤96 px target;
  the weight is count, not padding.
- **Balance:** Informational with embedded safe CTAs — the model
  page for principle 6, except the balance is nowhere declared.
  The `?only=fix-now` filter (badge target) is a second, hidden
  view of the page with its own caption — fine, but undiscoverable
  except via the badge.
- **V3 changes:** (1) Intent line per above; move the export
  buttons to the page foot or Snapshots. (2) Each quadrant header
  gains its canon link ("Why this order →" into the Guidance
  domain's Why section) — the board is where the reasoning layer
  earns its keep. (3) A "filtered" chip whenever `?only=` is
  active, so the badge's landing view names itself. (4) Do **not**
  paginate the board (see §5, rejected).

### 1.3 Issue detail — `/issue?p=&t=` — ≤949 px ✓ (gone-state measured)

- **Intent:** "Why this finding fired, the numbers behind it, and
  the safest next step." Today's head substitutes product +
  quadrant for an intent line — the detail page is the one page
  whose head does *not* carry a summary of purpose.
- **Domain purity:** Clean; the fix panel (per-entity prepares) is
  the domain's action surface, correctly placed.
- **Data states:** The "Issue gone" state is exemplary D-pattern:
  named, explained, in-shell (measured: renders at shell minimum
  with "This finding is not on the live board any more."). Live
  state requires the issue to still exist — there is no stored
  fallback, so a finding can never be reviewed after it clears.
  That is a real gap for the audit-trail story (the Log records
  applies, not findings).
- **Glanceability:** Why-card → numbers → history → fix: the right
  order (v1 page map, kept).
- **Balance:** Mixed and correctly so; the fix panel is visually
  distinct. Undeclared, as everywhere.
- **V3 changes:** (1) Real intent line in the head. (2) A canon
  strip on the why-card: the class's threshold stated in words
  ("Fires when a high-importance product has < 3 months of cover
  and production confirmed zero in 2 of the last 3 finished
  months") with a link to the Why page — generated from canon,
  not hand-typed (§3). (3) Record findings into the store at
  snapshot time already happens for the board (snapshot issue
  rows exist); surface a "last seen in snapshot" line on the
  gone-state page instead of a dead end.

### 1.4 Products — `/products` — 2,690 px ✓ (facets/pagination working)

- **Intent:** "Every product: how much there is, where it's held,
  and whether it's gaining or draining." Today: "Every product in
  the run — quantities, storage and monthly flows. Virtual
  products are excluded." Earns it; the virtual-products clause is
  a data-contract note and should move to the contract line.
- **Domain purity:** Clean. Pin/Unpin is a game-write living on a
  Monitor page — but it goes through §12 and is the product
  domain's own action; keep, declare it.
- **Data states:** Game-off → the page cannot render (no stored
  product table offered, though snapshots hold the rows — same
  stored-fallback gap as 1.3). Facets (`?f=`) + 60/page pagination
  verified in the render (61 `<tr>` incl. header).
- **Glanceability:** Table-first page; the filter bar + count
  caption is the model directory header. Sort remains client-side
  *within the rendered 60* — the recorded v2 wart, still live:
  sorting by Net on page 1 does not sort the run's products.
- **Balance:** Informational + one declared action (pin). Fine.
- **V3 changes:** (1) Server-side `?sort=` — close the wart; it is
  now the oldest recorded follow-up in the design docs and v3 is
  where follow-ups go to be finished. (2) Contract line naming the
  virtual-product exclusion + the pinned-history caveat (history
  exists only for pinned products — a data-state fact a stranger
  cannot currently discover from this page).

### 1.5 Product detail — `/product?id=` — ≤949 px ✓

- **Intent:** "One product, end to end: stock, flows, who makes
  it, who draws it, and its trend." Today the head carries id +
  pinned state instead of intent — same gap as Issue detail.
- **Domain purity:** Clean. Sources tables are this domain's depth
  and correctly placed here (Issues CTAs land on `#sources`).
- **Data states:** The known flaw stands: against any source
  lacking `/api/v1/products/sources`, the two sources panels
  render red 404 boxes (the stub lacks the route; fixtures
  `src-diesel.json`/`src-steel.json` exist unused). On a real
  older game build this is what a player would see — a raw error
  where a D3 placeholder belongs. History exists only when the
  product is pinned (game-side), and the page does not say so —
  an unpinned product's empty chart reads as "no data" rather
  than "pin it to start history".
- **Glanceability:** Stat tiles → chart → sources: right order,
  compact page.
- **Balance:** Informational + pin action. Fine.
- **V3 changes:** (1) Sources panels get the D3 treatment (named
  gap + remedy, no raw 404). (2) Intent line. (3) Unpinned-history
  empty state naming the remedy ("Pin this product in-game and
  history starts collecting"). (4) Wire the existing source
  fixtures into the stub so the demo/screenshot path shows the
  real panels.

### 1.6 Entities — `/entities` — Summary 1,306 ✓ · Storages 2,730 ~ · Machines 6,401 ✗ · Network ≤949 ✓

- **Intent:** "Every storage and machine the companion can
  address, with their controls." Today's line earns it ("The
  CoiBridge directory — …with controls" — though "directory" is
  our vocabulary, not a player's).
- **Domain purity:** The Summary sub-view is a *second Overview*
  in embryo: population/housing/food/research figures + flagged
  tables. Those figures are settlement domain — they live here
  because the bridge status route lands here, not because a
  player would look for population under "Entities". Research's
  figure on Summary duplicates Research's own landing data.
  Network sub-view (transports + trains + non-default priorities)
  is three domains in one sub-view, held together by "not the
  other two".
- **Data states:** The app's best degraded story: three render
  paths (no game / no bridge / bridge) degrade per section; the
  no-bridge path names the remedy (install mod, load save,
  `web_api_start`) in a danger callout. Verified in code and in
  the v064-capture check scripts.
- **Glanceability:** Summary is genuinely glanceable (1,306 px).
  Machines is not: 110 proto rows, state filter client-side,
  6,401 px — the v2 pagination applied to Storages (61 rows ✓)
  but not to the proto table.
- **Balance:** The most action-dense page in the app (slider
  steps, logistics toggles, priority prepares, set-product form)
  wearing a Monitor badge. It belongs to the action half of the
  product in everything but nav placement — a first-time player
  will not predict that "the page where I *change* storage
  settings" sits under the "what is my factory doing" group.
- **V3 changes:** (1) Paginate Machines like Storages (60/page,
  server-side, keeping `?proto=` deep links landing pre-filtered).
  (2) Move the settlement figures off Summary — population/food
  belong on Overview (which already shows population in the
  status strip) or as their own rail anchor; Summary keeps
  flagged storages/machines only, which is its actual job.
  (3) Split Network: trains stay (transport network is entity
  domain), non-default priorities move to Actions (they are the
  *result* of priority actions) or to the Priorities why-page.
  (4) Rename the summary line's "directory" → "every storage and
  machine, with controls".

### 1.7 Terrain — `/terrain` — 1,747 px ✓

- **Intent:** "The land: heights, what's in it, and what's
  designated on it." Today: "The land itself: heights, on-map
  deposits and designations — the same tile grid as everything
  else (an entity's Z is the floored terrain height)." The
  parenthetical is a lovely fact in the wrong place — it belongs
  in Learn-the-game, not in a page head.
- **Domain purity:** Clean — the tightest page in the app. The
  estimator form is a tool, not a write; correctly unbadged.
- **Data states:** Game-off → degraded callout (measured path in
  code). Bridge < 0.6.5 → `degraded_bridge` naming the missing
  trio. Bridge 0.6.5 without 0.6.6: designations render band-only
  and are *labelled* partial (SPEC XX discipline) — the model
  D3. One stale string: the placeholder cites "0.6.5" as the
  requirement while the shipped bridge is 0.6.6 — the floor is
  correct, but a player on 0.6.5 reading "update the mod" on the
  Designate verdict (which needs 0.6.6 corners) gets one message
  for two floors. Split the two floors in text.
- **Glanceability:** Stat figures → heatmap → region → deposits →
  designations: right order, 1,747 px.
- **Balance:** Informational + estimator (a calculator, not an
  action). Declare as informational.
- **V3 changes:** (1) Move the Z-fact to the Terrain guide/why
  page; intent line shortens. (2) Two-floor wording for the
  bridge requirement (0.6.5 terrain, 0.6.6 exact verdicts).

### 1.8 Trade — `/trade` — ≤949 px ✓ (on this capture: all-placeholder)

- **Intent:** "Your deals with the world: contracts, quick
  trades, loans — and the edicts that bend the rules." Today:
  "Read through CoiBridge — the vanilla API has no routes for
  any of this. Actions go through the usual prepare → confirm →
  audit flow." All provenance, no intent. Rewrite.
- **Domain purity:** Edicts are the foreign body — they are
  settlement policy, not trade; they live here because the bridge
  route group landed here. A player looking for "where do I see
  my edicts" will not predict Trade. (They are read-only here;
  the write path is the catalogue/mod gap, already documented.)
- **Data states:** On the v065 capture all four sections render
  their "arrives with CoiBridge v0.6(.1) — update the mod"
  placeholders: correct D2 behaviour for absent routes — but the
  message asserts a *cause* (old mod) that the page cannot
  verify; a route can 404 for other reasons. And with a current
  bridge + zero contracts, the same section shows an empty table
  — the two states are distinguishable, good.
- **Glanceability:** Four anchored sections, short page. Fine.
- **Balance:** Mixed (contract/trade/loan actions via §12) —
  undeclared.
- **V3 changes:** (1) Intent line. (2) Placeholder wording:
  "needs CoiBridge 0.6+; this route isn't answering" (cause-
  neutral) with the version as the remedy, not the diagnosis.
  (3) Move Edicts out — to the Guidance domain's game class as a
  read-only status panel, or to Actions beside the other
  settlement levers; §2.3 places it under Guidance ▸ Learn the
  game ▸ Edicts *status* with a cross-link from Trade until the
  move is safe. (Lowest-value item on this page; keep the link
  either way.)

### 1.9 Research — `/research` + `/research/node` — Landing ≤949 ✓ · Catalogue tab 10,786 ✗ · Completed ≤949 ✓ · Node ≤949 ✓

- **Intent:** "What your labs are doing, what's queued, and what
  you could start." Today: "Current node, queue and the full node
  catalogue — Start / Queue / Unqueue all go through prepare →
  confirm." Good; drop the second clause to the contract line.
- **Domain purity:** Clean. The catalogue tab is a directory and
  behaves like one — except for size.
- **Data states:** Bridge-only page: no bridge → the whole page's
  data is absent (the "API does not report research" fact is
  carried in the how-it-works details — it belongs in the data
  contract, visible). Node count honesty check: the how-it text
  says 250 nodes; `coi/data/research_ids.txt` has exactly 250 —
  the claim is *correct* (recorded because it was worth
  verifying, not because it drifted).
- **Glanceability:** Landing tab is the model sub-view (current
  node card + queue + completed count, ≤1 viewport). Catalogue
  tab: 250 rows, letter-grouped with sticky heads — findable by
  letter, but 10,786 px of scroll is not "grouped", it's
  "long". Completed as a chip grid: right.
- **Balance:** Actionable (Start/Queue/Unqueue prepares correctly
  exiled to node detail in v2 — the discipline held).
- **V3 changes:** (1) Catalogue tab: collapse letter groups by
  default (only the group containing the current node's letter
  open) or paginate by letter — one mechanism, pick collapse
  (keeps the rail letter index meaningful). (2) Data-contract
  line: research is bridge-only data. (3) Node detail gains the
  canon link for research philosophy (queue discipline — the
  vanilla queue traps in BEHAVIOURS §6 are player-relevant and
  currently agent-only knowledge).

### 1.10 Actions — `/actions` — 1,096 px ✓

- **Intent:** "The write surface: every change to your game goes
  prepare → confirm → audit, and starts here or on a detail
  page." Today: "Commands the companion can run in your game —
  every one goes prepare → confirm → audit." Earns it.
- **Domain purity:** The audit-log tail (last 20) is System
  domain visiting Game — justified as "the receipt drawer next
  to the till", and it links to the full Log. Keep, declare.
  Global priorities form lives here while per-entity priority
  controls live on Entities — the priority *domain* is split
  across two pages by scope (global vs per-entity); a player
  cannot predict which. The page's own prose already apologises
  for this ("The controls live where they're useful…") — an
  apology is a smell; the contract should state the split as a
  rule instead.
- **Data states:** Bridge-absent variant collapses the fix-
  actions card to a D2 callout — good. Game-off: prepares refuse
  at prepare-time (502 pattern), the page itself renders.
- **Glanceability:** Short, carded, anchored. Fine.
- **Balance:** The one honestly actionable page — and the only
  one whose head doesn't need to say so because its content does.
- **V3 changes:** (1) State the priority split as a rule in the
  contract line ("Global priorities here; per-building priority
  lives on Entities ▸ Machines"). (2) Absorb Entities' non-
  default priorities table here (see 1.6) so "current priority
  state" and "change priority" share a page.

### 1.11 Catalogue — `/catalogue` — 15,264 px ✗✗ (the worst page in the app)

- **Intent:** "Every command the game exposes, what it does, and
  whether the companion will run it for you." Today: "Every
  command the game exposes over POST /command (inventory
  captured from this game, v0.8.8)." Earns the first half; the
  parenthetical is provenance for the contract line.
- **Domain purity:** Clean — it *is* a reference directory. Its
  problem is purely mechanical.
- **Data states:** Static (captured inventory) — the one page
  that works fully with the game off, and it doesn't say so.
  Statuses (15 verified / 224 wired / 1 excluded) are live-ish
  facts rendered as badges; the counts live in the how-it-works
  text, not in figures.
- **Glanceability:** None. 241 rows incl. header, no pagination,
  facets narrow but don't page. The v2 nesting table promised
  "compact rows; rail facets" and stopped there.
- **Balance:** Informational with Prepare exits per row —
  correct and safe.
- **V3 changes:** (1) Paginate 60/page exactly like Products
  (same code shape, `?domain=` facet preserved across pages).
  (2) Status counts as figures in the head (Verified · Wired ·
  Excluded). (3) Contract line: "Works without the game — this
  is the captured inventory from v0.8.8."

### 1.12 Guides — `/guides` + `/guide?id=` — List 1,024 ✓ · Detail 2,828 ~

- **Intent:** "Failure-mode playbooks: what goes wrong, how it
  presents, how to stop it." Today: "Failure-mode knowledge:
  what goes wrong, how it presents, and how to keep it from
  happening." Earns it — for the *game* class. In v3 this page
  becomes one class inside the Guidance domain (§2); its intent
  line survives as that class's line.
- **Domain purity:** The 10 guides are game-domain knowledge.
  Two contaminations, both load-bearing for §3: the Storage
  guide's war-chest section teaches from *one specific retired
  run* ("this run's Construction Parts I & II stockpiles are a
  deliberate war chest for the relocation phases") — presented
  to any player as if it were their factory; and the "In your
  factory right now" live panel makes each guide partly a
  Monitor page (a good feature — but it means a guide's data
  state is a live thing, see next).
- **Data states:** Guide text is static (works game-off — not
  stated). The live panel degrades when the game/bridge is
  absent; the panel's absence is not separately labelled.
- **Glanceability:** List is a clean card grid. Detail at
  2,828 px for the longest guide is borderline-over but it is
  *reading* — the 3-viewport rule was written for data pages;
  §5 proposes the rule be amended to exempt long-form reading
  pages (with the live panel kept first, as now).
- **Balance:** Informational, correctly action-free.
- **V3 changes:** (1) Re-home into Guidance ▸ Learn the game
  (§2.3), URLs preserved by redirect or alias. (2) De-run the
  Storage guide (canon C-2). (3) Label the live panel's data
  state. (4) Add the wiki link-out block per guide (brief's
  policy: link where the wiki does it well — farming/food and
  maintenance are the first candidates, §3 C-7).

### 1.13 Plans — `/plans` — 1,245 px ✓

- **Intent (player vocabulary):** "Step-by-step game plans the
  companion can follow with you — what each phase builds, and
  the evidence that gates it." Today: "Machine-executable plans
  (GAME-PLAN-FORMAT v1) bundled with the companion." — format-
  spec vocabulary aimed at the tool's authors, on the page a
  player reads. The body doubles down ("run `coi plan-check`;
  the supervised session walk is `coi play`") — CLI instructions
  on the website of a product whose whole point is that the
  website is the interface.
- **Domain purity:** Post-cleanup the page is reference-only
  (fresh-start plan, phases/intents/gates; "Nothing here has
  been executed" — admirably honest). But that makes its nav
  placement wrong: a read-only reference sits in Game ("Change
  the game") between Research and Actions. Its phases rail is
  the page's own index — fine.
- **Data states:** Static by design now; live evaluation happens
  through the CLI/runner, invisible here. A player cannot see a
  gate's current verdict on this page at all — the *evidence*
  half of the domain lives in runner output files. That is the
  page's real gap, and v3 should name it even if the fix
  (surface last runner verdicts from the store) is scheduled
  later.
- **Glanceability:** 1,245 px, phased, anchored. Fine.
- **Balance:** Informational wearing an actionable group's
  badge. Declare informational; move per §2.3.
- **V3 changes:** (1) Player-vocabulary intent + contract
  ("reference — verdicts appear here after a supervised
  session"). (2) Remove CLI instructions from the body; the
  tool-manual class of Guidance is where `coi play` belongs.
  (3) Move to Guidance ▸ Learn the tool *or* keep in Game with
  the balance declared — §2.3 recommends Guidance, with a
  cross-link from Actions.

### 1.14 Place — `/place` — 2,200 px ✓

- **Intent:** "Put a whole layout down as one confirmed job."
  Today: "Place a whole layout — buildings, belts and pipes —
  as one confirmed job, executed in-game by QoSBuilder." Earns
  it. The five-sentence explainer paragraph under the head is
  the v2 how-it pattern *not applied* — this page kept its prose
  (the flow deserves the words; they belong in the collapsed
  details with the rest, summary line first).
- **Domain purity:** Clean (blueprint library is its own
  domain). Industry collapse groups + rail facets work.
- **Data states:** Prepare refuses when game/gameId is
  unconfirmable (502 path, check-script-verified) — the model
  refusal. The library itself is static and doesn't say so.
- **Glanceability:** 2,200 px with groups collapsed — fine.
- **Balance:** Actionable, honestly so; confirm sheet is the
  §3.9 safety surface, untouched by every round so far.
- **V3 changes:** (1) Explainer → how-it details, one-line
  summary up. (2) Contract line: library is bundled/static;
  jobs need QoSBuilder enabled in-game (already in the prose —
  promote it).

### 1.15 Designate — `/designate` — ≤949 px ✓

- **Intent:** "Plan a terrain pad — mining, dumping, levelling
  — and remove designations, as confirmed jobs." Today earns it
  nearly verbatim.
- **Domain purity:** Clean; the live designations list is
  Terrain-domain data *used* here — a read, not a copy (it links
  the domain rather than duplicating it). The verdict panel is
  this page's own invention and its best feature: LANDED /
  PARTIALLY LANDED / SCHEDULED — verdict pending, never claimed
  from the write alone.
- **Data states:** Prepare needs live terrain (bridge 0.6.5+);
  exact verdicts need 0.6.6 corners, band-only is labelled
  partial — the two-floor wording issue of 1.7 applies here
  with more force, since this page *consumes* the verdict.
- **Glanceability:** Form + list + verdict in one viewport on
  this capture. Fine.
- **Balance:** Actionable; the remove flow's stale-selection
  refusal (origins only from live read-back) is principle-4
  honesty applied to writes. Model behaviour.
- **V3 changes:** (1) Two-floor bridge wording (with 1.7).
  (2) The pit planner's fixture-fed status (core ready, no
  bridge per-column route yet) is invisible to players — the
  pad-only scope should be stated in the contract line before a
  player wonders where "dig a pit" is.

### 1.16 Snapshots — `/snapshots` — ≤949 px ✓ (empty store measured)

- **Intent:** "Frozen states of your run, and how it moved
  between them." Today: "Snapshots freeze the whole run — every
  product, the monthly history, and the triage board — into the
  local SQLite store." Earns it ("SQLite" is our word, not a
  player's — minor).
- **Domain purity:** Clean. Trends tables are Products-domain
  data in stored form — legitimate (the domain here is *time*).
- **Data states:** Empty store renders true zeros as figures
  (0 snapshots / 0 rows / 0 issues) with the take form present —
  honest. Game-off: stored data still renders (the D1 rule's
  "stored data below the callout" — implemented here by
  construction since the page is store-first).
- **Glanceability:** Fine at this size; a long store history
  will need the same pagination question as Log (page size
  exists for snapshots table? — the table is unpaginated in the
  render path; flag for the build round to verify at scale).
- **Balance:** Informational + one action (Take). Declared by
  content.
- **V3 changes:** (1) Drop "SQLite" from the intent line.
  (2) Verify snapshot-table pagination against a populated
  store during the build round (the demo-data round will
  provide one).

### 1.17 Log — `/log` — ≤949 px ✓ (empty store measured)

- **Intent:** "Everything that was done, in order — by you,
  the companion, and the mods." Today earns it nearly verbatim.
- **Domain purity:** Clean; it is the union record and says so.
  The five-line explainer is now a callout + details (v2
  applied). Import report as figures (v2 §V2.3) — verified in
  the check scripts' updated assertions.
- **Data states:** Empty state is the model empty state ("No
  events recorded yet — actions you confirm and QoS mod
  activity land here."). QoS feed absence is reported with its
  *reason* (path unknown / no file) in the header — principle 4
  done right (the Linux-stub wording "(no APPDATA and no
  COI_QOS_EVENTS_PATH…)" is environment leakage of the same
  species as 1.13's CLI text; on Windows it reads as a path —
  acceptable, watch it).
- **Glanceability:** Page size 50 (v2), source facets in rail.
  Fine.
- **Balance:** Informational. The JobRefused rows' "Re-prepare"
  ghost (§V2.7 row 14) is the one action exit — correct.
- **V3 changes:** None structural. (1) Feed-path wording passes
  the stranger test. (2) Findings (not just applies) join the
  record per 1.3 — Log is where that lands.

### 1.18 Utility surfaces (no separate grade)

- **Prepare / Confirm / Result** (`/prepare`, `/actions/apply`
  results): the §3.9 safety surface. Out of v3's scope to
  redesign; the audit confirms the v2 state (warning band,
  plain-language summary, exact JSON, expiry caption) is intact
  and the fail-loud audit banner (2026-11-06 remediation) fires.
  One contract nit: `/prepare` with missing params redirects to
  the Catalogue silently — a stranger deep-linking gets moved
  without a word. Add a flash line on the landing page.
- **Must-fix badge:** behaviour verified by `check_badge.sh`;
  red state still never observed on live-shaped data (see
  problem 5). The demo-data round (stale-save captures) is the
  first chance to see it red — schedule the eyeball there.

---

## 2. Navigation audit

### 2.1 The current structure, graded

Current top level (mega-nav, DESIGN V2.1): **Monitor** (Overview,
Issues, Products, Entities, Terrain) · **Game** (Research, Trade,
Plans, Actions, Place, Designate) · **Library** (Guides,
Catalogue) · **System** (Snapshots, Log). Group descriptors:
Monitor "Live game state, refreshed on every page load", Game
"Everything mutating goes prepare → confirm → audit", Library
"Guidance first — the catalogue is the whole command surface",
System "The local record — nothing leaves this machine". The
group footers are genuinely good — Game's footer *is* its safety
contract. Keep that pattern.

Where a first-time player predicts a fact vs where it lives:

| The player wants… | Predicts | Actually lives | Verdict |
|---|---|---|---|
| "What needs fixing?" | Overview / Issues | Both (badge → Issues?only=fix-now) | ✓ |
| "How much diesel do I have?" | Products | Products / Product detail | ✓ |
| "Change a storage's contents" | Products? Settings? | **Entities ▸ Storages** | ✗ — "Entities" is engine vocabulary; nothing on the path says "storage settings live here" until the mega descriptor |
| "Pause the game / speed" | Overview or Actions | Both | ✓ (duplication declared nowhere) |
| "My edicts" | Settlement? Trade? | **Trade** (read-only table) | ✗ |
| "Priorities" | Actions | Split: global on Actions, per-building on Entities | ✗ (page prose apologises for it) |
| "Why did it suggest this?" | Guides? the issue itself? | Nowhere — a one-line fix string | ✗✗ — the brief's core gap |
| "How does this tool work?" | Help / About | Nowhere (INSTALL.md is a repo file) | ✗ |
| "What did the tool do yesterday?" | Log | Log ✓ (System is a cold name for it, but the descriptor saves it) | ~ |
| "Is my plan on track?" | Plans | Plans shows the *text*; verdicts live in CLI runner output | ✗ |
| "Dig/flatten terrain" | Terrain | **Designate** (Terrain is read-only) | ~ — Terrain ▸ Designate cross-link exists via domain, not via an explicit "do it →" |
| "The wiki's take on farming" | Guides | Our guide only; no link-out exists | ✗ (brief requires the policy) |

The pattern in the failures: **Monitor/Game are verbs the tool
uses about itself, not nouns the player thinks in.** "Entities"
and "System" are the two worst offenders; "Library" holds two
unrelated things (playbooks + a command directory) united only by
"reference".

### 2.2 The Guidance domain (new peer)

Per the brief, Guidance is a top-level peer with three classes,
clearly separated inside the domain:

- **Learn the game** — the 10 existing guides, re-homed.
  *Wiki link-out policy (proposed, for the build round to apply
  per guide):* link out to the official wiki when ALL of these
  hold — the wiki page is current for 0.8.8, the topic is
  reference-shaped (recipes, numbers tables), and our guide adds
  no verified correction. Keep our text where the guide's value
  is the *failure mode* (our differentiator), where we have
  code-verified mechanics the wiki lags on (earthworks collapse
  bands, designation snapping), or where the wiki is known-thin
  (the Ship's initial repair cost is absent from the wiki
  entirely — recorded in the farming skill's source notes).
  Every link-out is labelled external and sits in a standard
  "On the wiki" block at the guide's foot — never inline
  mid-argument. First link-out candidates: Farming (wiki crop
  tables), Power (plant reference numbers). Never link out:
  Priorities, Storage, Oil & diesel (failure-mode content).
- **Learn the tool** — new. The Superintendent manual: what each
  domain/page is for (the page contracts, §0 S2, rendered for
  players), how prepare → confirm → audit works and why, how to
  run it (daemon guidance — lands with the daemon round), what
  the data means (flows windows TM/LM/TY/LY, Percent, evidence
  classes MEASURED/ESTIMATED/INSUFFICIENT in player words), the
  CLI's existence and its two commands that matter (`plan-check`,
  `play`) — this class absorbs every developer-vocabulary leak
  found in §1.
- **Why it suggests** — new. One page per reasoning family,
  rendered from the canon (§3): the board's ordering (quadrants,
  importance, urgency), each issue class (what fired, the
  threshold in words, what the action achieves, what ignoring it
  costs), the productivity verdicts (working share, evidence
  classes, why PARKED is not a finding), forecasts & horizons,
  plan gates (what a gate is, the fresh-start gates explained),
  and the play philosophy (survival first, buffers, never scale
  a failing leaf — the strategy skill's rules, player-rendered).

### 2.3 Proposed v3 top-level tree (complete — every page placed)

```
SUPERINTENDENT
├─ MONITOR — "What is my factory doing"            (unchanged name; it earned it)
│  ├─ Overview            /            (contract: informational + 2 embedded actions)
│  ├─ Issues              /issues      (+ Why-this-order canon link per quadrant)
│  │   └─ Issue detail    /issue?…     (+ canon strip)
│  ├─ Products            /products    (server-side sort)
│  │   └─ Product detail  /product?…   (D3 sources, pin-to-history note)
│  └─ Terrain             /terrain     (read-only land view; "Designate →" exit)
├─ WORKSHOP — "Change the game"                    (renamed from Game: a player noun,
│  │                                                  and it frees "Game" for nothing)
│  ├─ Entities            /entities    (moved from Monitor — it is the control surface;
│  │   └─ sub-views: Summary · Storages · Machines (paginated) · Network (trains only)
│  ├─ Research            /research    (tabs: Current & queue · Catalogue (collapsed) · Completed)
│  │   └─ Node detail     /research/node?…
│  ├─ Trade               /trade       (Contracts · Quick trades · Loans; Edicts leaves)
│  ├─ Actions             /actions     (+ non-default priorities absorbed; audit tail stays)
│  ├─ Place               /place
│  └─ Designate           /designate
├─ GUIDANCE — "Understand"                          (NEW peer domain)
│  ├─ Learn the game      /guides      (the 10 guides; wiki link-out blocks)
│  │   └─ Guide detail    /guide?…
│  ├─ Learn the tool      /manual      (NEW: contracts, §12 explained, data meanings, CLI)
│  └─ Why it suggests     /why         (NEW: canon-rendered reasoning pages)
│      └─ Why detail      /why?topic=… (per family/class)
├─ PLANS — "Step-by-step, with evidence"            (promoted out of Game: its balance is
│  └─ Plans               /plans        informational; verdict surface when the store has them)
└─ RECORDS — "What happened"                        (renamed from System)
   ├─ Snapshots           /snapshots
   └─ Log                 /log         (+ findings join the record)
```

Decisions embedded above, stated plainly:

- **Library dissolves.** Its two occupants were never kin:
  Guides → Guidance; Catalogue → Records-adjacent reference is
  wrong too — the Catalogue is the *index of the write surface*,
  so it goes to **Workshop** as a peer of Actions (not listed
  above to keep the tree readable — it belongs between Actions
  and Place: `Catalogue /catalogue` with Prepare as its utility
  route, as today). Final Workshop order: Entities · Research ·
  Trade · Actions · Catalogue · Place · Designate.
- **Entities moves to Workshop.** The §1.6 finding (most
  action-dense page wearing a Monitor badge) is an IA error,
  and v3 is the round that can afford to fix it. The mega
  descriptor must say "storages & machines, with controls" so
  the storage-settings path (2.1's worst row) becomes
  predictable: Workshop ▸ Entities ▸ Storages.
- **Plans is promoted to a top-level domain of one page** rather
  than buried in Guidance: it is the product's north-star
  surface (agent plays to a plan), it will grow a verdict
  surface, and a one-page domain is honest when the page is a
  destination, not a directory. If Philip prefers five domains
  maximum, the fallback is Guidance ▸ Learn the tool ▸ Plans —
  recorded here as the rejected alternative, with the reason.
- **Renames:** Game → Workshop (player noun, no ambiguity with
  "the game" itself), System → Records (what it holds, in player
  words). Monitor survives — it is the one group name a player
  would invent. All renames are chrome-only; **no URL changes**
  (routes are already page-shaped, not group-shaped — the v2
  additive-routes discipline pays off here).
- **Nothing is orphaned:** favicon/screenshot/report/snapshot
  utility endpoints are not pages and stay as-is; `/prepare`
  stays Workshop's utility route; the must-fix badge still
  targets Issues.

---

## 3. The canon inventory — one canon, three renderings

### 3.1 What exists today, by rendering

**Player rendering** — `coi/src/guides.rs` (10 guides, ~270
lines, failure-mode format: failure → presentation → prevention),
the issue `fix`/`evidence` strings and `fix_hint` table
(`coi/src/main.rs:1469ff`), `quadrant_why` (`serve.rs:5680`),
forecast `advice` strings (`main.rs` `compute_forecasts`), the
Guides' "In your factory right now" live panels, plus the two
standalone guide docs that never made it into the site:
`guides-research.md` and `guides-earthworks.md` (repo-root —
earthworks was later also rendered as a skill, research was
folded toward the plan work; **neither is reachable from the
Guides page today** — the player rendering is missing two of its
own chapters).

**Agent rendering** — 21 `local-coi-*` skills
(`~/workspace/custom-skills/`), of which the reasoning-bearing
ones are: `local-coi-strategy` (triage order, expand-vs-fix,
WAIT/REDESIGN/ESCALATE), `local-coi-playing` (the loop, §12
posture, sandbox rules), `local-coi-earthworks` (snapping
procedure v2, 13 steps), `local-coi-starter` (early-game order),
`local-coi-research-strategy`, and the production-chain skills
(iron-steel, concrete, copper-electronics, oil-diesel-rubber,
power, farming-food, maintenance, population-housing,
logistics-vehicles, trade-shipping, pollution-health, trains).
Plus `BEHAVIOURS.md` (831 lines, six distilled community
behaviours), `GAME-PLAN-FORMAT.md`, `SANDBOX-PROTOCOLS.md`, and
the offline wiki mirror (`~/workspace/coi-wiki-mirror/`, the
evidence base under several skills).

**Executable rendering** — `coi/src/main.rs` (`compute_issues`,
`compute_forecasts`, `is_seasonal`, `seasonal_net`, `importance`,
`fix_hint`), `coi/src/productivity.rs` (Thresholds struct, all
estimator constants, TOML-overridable via `[productivity]`),
`coi/src/designate.rs` (spike filter, purity gates, corner
smoothing), `coi/src/plan.rs` + `plans/fresh-start-v1.yaml`
(gates, orders, DECIDE markers), `coi/src/catalogue.rs` (the
§12 policy notes), and the QoS mods' own guard constants
(`qos/` — actuator step/cooldown, chicken caps, recovery Unity
reserve).

### 3.2 Rules that exist in ≥2 renderings (the inventory)

| # | Rule / content | Player | Agent | Executable |
|---|---|---|---|---|
| R1 | Quadrant ordering & what each quadrant means | `quadrant_why` + board | strategy triage legs | `compute_issues` quadrant assignment |
| R2 | Urgency = cover/runway < 3 months | runway bar "quarter" tick; amber rule in DESIGN V2.5 | — (not stated as a number) | `urgent = months < 3.0` in stall & deficit classes |
| R3 | Stall = production zero, confirmed 2 of last 3 finished months | evidence string only | playing/strategy "seasonal caveat" (adjacent) | `prod_zeros_recent >= 2`, cover < 36 |
| R4 | Seasonal/bursty producers judged on long window | evidence basis strings; farming guide's thesis | strategy seasonal caveat; playing rule 6 | `is_seasonal` (≥half zero months, peak ≥ 2× mean & ≥ consumption avg) + `seasonal_net` |
| R5 | Storage full = producer blocks; parked full = strategy | storage guide (2 sections) + issue classes storage-full/parked | strategy leg 4 ("full by configuration is a goal state") | `stored >= cap` (+ `cap >= 200` floor) |
| R6 | "Full" as a *flag* at ≥ 90% | fill bars amber ≥ 90% (`serve.rs:2684,5859,6920`); DESIGN V2.5 rule 4 | — | Entities Summary flagged set ≥ 90% |
| R7 | Deficit horizons (issue < 24 mo; structural < 120; forecast slope < 120; fill < 12; stall-runway < 36) | Outlook table (no horizon stated) | — | five separate constants in `compute_issues`/`compute_forecasts` |
| R8 | Importance: which products matter most | invisible (badge outcome only) | strategy: death-spiral triangle + bottleneck chains | `importance()`: 21 named HIGH products → imp 3; 11 LOW → 1; else 2 |
| R9 | Priority discipline: small fixed tiers, don't spiral; manual groups 9/8/8/9 internal (UI +1) | priorities guide (qualitative, no numbers) | BEHAVIOURS §3 (full model + XOR rule vs actuator) | catalogue §12 notes; QoS actuator constants (step −1, max 2, cooldown 2 d) |
| R10 | Earthworks: neighbour step ∈ {0,±1}; bench bands; spike filter (>4 below 8-neighbour median); purity gates L0–L4; floor = component percentile (5 = median); MaxHeightDiff 1 | guides-earthworks.md (not in the site) | earthworks skill (13-step procedure) + BEHAVIOURS §1 | `designate.rs`: SPIKE residual 4.0, 8 neighbours; PURITY arrays; smoothing clamp |
| R11 | Productivity: working-share verdicts w/ Wilson CI, cause split, PARKED at ≥ 50% paused | Issues productivity section (verdict + evidence class) | SPEC XV (design record); companion-dev skill | `productivity.rs` Thresholds: fire CI-upper 0.50, point 0.45, severe 0.25/0.35, clear CI-lower 0.60, confirm 0.55, cause shares 0.30/0.30/0.20 |
| R12 | Plan gates as evidence (fresh-start: iron ≥ 100, coal ≥ 200, concrete ≥ 100, diesel ≥ 300 + net ≥ 0, months_of_food ≥ 3, CP net ≥ 0…) | Plans page (reference text) | plan file itself + strategy stall rules | `plan.rs` engine + `tools/plan_check.py` (parity-tested) |
| R13 | §12 posture: prepare → confirm → audit; verified/wired/excluded; GameConsoleCmd excluded forever | Actions/Guides prose; catalogue badges | playing skill rules; strategy §5 | catalogue.rs statuses (15/224/1) + serve flow |
| R14 | Maintenance: breakdowns rise below 75%, 50%/mo at 0 | **no player rendering** (no maintenance guide among the 10) | maintenance skill (full) | — (no executable detector; plan uses starved-share proxy, plan note G6) |
| R15 | Research discipline: queue up-front, prerequisite traps (StopResearch clears the whole queue; a dequeued item whose prereq sits later is silently discarded) | **no player rendering** (research guide doc exists at repo root, not in the site) | BEHAVIOURS §6 + research-strategy skill + starter skill | runner holds already-researched orders (parity rule) |
| R16 | Death-spiral triangle (fuel/maintenance/power) + "crisis beats plan" | oil-diesel & power guides (pieces) | strategy leg 1; starter rule 5 | partially: importance HIGH covers fuel/power products; **maintenance has no product** → engine cannot see the third leg (plan note G6 proxy) |
| R17 | Steel sizing: 24 steel/min per caster line | `fix_hint` steel entries (×2) | iron-steel skill ratios | — (advice constant, not a detector) |
| R18 | Coal buffer: "≥ 120 min of draw at plant scale" | `fix_hint` coal entry — **only** rendering | — | — (and see C-3: the engine's coal assumption) |
| R19 | UNMA alarm model (4 severities, ack lifecycle) | — | BEHAVIOURS §4 (advisory only) | — (deliberately not implemented; the board's 4 quadrants are a *different* 4-way split — see C-6) |
| R20 | Food security floor: months_of_food ≥ 3 | status strip shows months via Entities figures | population-housing skill; plan gate R12 | fresh-start gate `settlement.months_of_food >= 3` (×2 phases, one with `for: 1 month`) |

### 3.3 Disagreements and drift (flagged, NOT reconciled)

- **C-1 — "Full" means three things.** The storage-full *issue*
  fires at `stored >= cap` (100%); the *flag* (amber fill bar,
  Entities flagged set) fires at ≥ 90%; the DESIGN V2.5 text
  names "storage ≥ 90% full" as "a threshold the app itself
  defines" without distinguishing flag from finding. No two
  renderings state both numbers together. A player told "storage
  full" at 100% and shown amber at 90% gets no page explaining
  the difference. *Severity: real, cosmetic-grade — fix by
  canon wording (flag vs finding), no number needs to change.*
- **C-2 — MATERIAL: the player canon teaches from a retired
  run.** `guides.rs` Storage guide: "this run's Construction
  Parts I & II stockpiles are a deliberate war chest for the
  relocation phases" — Philip's retired Dragon Isle run,
  presented as the reader's own factory, in the shipped binary,
  *after* a privacy cleanup whose scrub rule was run-specific
  shipped text (it survived because it contains no name — the
  scrub grepped identifiers, not provenance). The agent
  rendering (strategy leg 4) states the identical rule
  generically and correctly. The player rendering must be
  de-run'd; more importantly the canon needs a provenance rule
  ("no canon entry may cite a specific run as its example
  without framing it as an example").
- **C-3 — MATERIAL: a run-specific policy is compiled into the
  detector, invisibly.** `main.rs`: `let import_fed = id ==
  "Product_Coal";` with the comment "Coal is import-fed by
  design in this run" — the stall detector silently exempts
  coal for *every* player, forever, because one run imported
  coal. No player or agent rendering mentions the exemption;
  the strategy skill, by contrast, treats the analogous diesel
  case (DieselReforming) as an ESCALATE/DECIDE that no rule may
  silently pick. The two cases are handled by opposite
  philosophies in the same codebase. Canon must own this: either
  the exemption becomes a stated, configurable canon rule
  ("products you import by design") or it comes out.
- **C-4 — The horizon thicket.** R7's five constants (12 / 24 /
  36 / 36 / 120 months by class) are each defensible; nowhere
  are they stated together, and the CLI's own summary line
  ("runways < 36 months, fills < 12 months", `main.rs:1371`)
  *contradicts* the engine for slope forecasts (< 120). The
  player rendering (Outlook) states no horizon at all, so a
  forecast appearing at 100 months' runway looks arbitrary
  next to an issue that fires at 24.
- **C-5 — Quadrant logic vs triage logic read as two systems.**
  The board's ordering (importance × urgency → 4 quadrants)
  and the agent's ordering (death-spiral legs → starvation →
  blockage → hygiene) are reconcilable — the strategy skill
  even says "in the board's own priority order" for its leg 5 —
  but no rendering *shows* the reconciliation. An agent that
  drops plan work for a diesel crisis (strategy leg 1) while
  the board shows diesel as "Watch" (importance 3 but cover
  ≥ 3 months → not Fix now) looks, to the player, like it is
  disobeying the board. The Why domain must publish the mapping.
- **C-6 — Two four-way vocabularies.** Board quadrants (Fix
  now / Fix later / Watch / Ignore for now) vs UNMA severities
  (BEHAVIOURS §4, advisory). Only the agent rendering knows
  both exist; if the UNMA model is ever adopted (QOL-MODS #5),
  the collision lands. Canon should register the quadrant
  vocabulary as *the* player vocabulary now.
- **C-7 — Missing player chapters.** R14/R15: maintenance and
  research-discipline reasoning exist in agent + (for research)
  repo-doc form only. The player rendering has no maintenance
  guide at all despite maintenance being a death-spiral leg —
  the single biggest content gap in the Guides list. (The
  earthworks guide doc has the same not-in-the-site status.)
- **C-8 — Priority numbers never reach the player.** R9: the
  guide teaches the discipline qualitatively; the 0–14 internal
  vs 1–15 UI offset and the 9/8/8/9 group defaults exist only
  in BEHAVIOURS + mod code. A player using Actions' priority
  prepares sees raw numbers the guide never grounded. (The
  guide's silence was originally honesty — custom priorities
  were "least publicly documented" — but the bridge now
  publishes them, so the canon can state them as *our*
  convention, labelled as such.)
- **C-9 — Even-count median, recorded divergence of
  *specification*, not of behaviour.** `designate.rs:705ff`
  notes BEHAVIOURS §1 says "median of the 8" without pinning
  the even-count convention; the code chose mean-of-two-middle
  (and the earthworks skill says "median of all 8 neighbours"
  equally unpinned). Behaviour is consistent everywhere it is
  *executed* (only the code executes it); the texts are
  under-specified. Canon pins the definition once.
- **C-10 — Productivity thresholds are configurable in exactly
  one rendering.** `productivity.rs` Thresholds can be
  overridden per-user via `[productivity]` in coi.toml — so
  "the" fire threshold (CI-upper 0.50) is a *default*, a fact
  no player or agent text states. If the Why page prints 0.50
  as the rule while a user's config moved it, the canon has
  produced a confident falsehood. The Why rendering must read
  the *effective* thresholds (they are already loaded server-
  side) — a rendering requirement, recorded here.
- **C-11 — Steel 24/min and coal 120-min (R17/R18) are
  single-rendering numbers.** Each appears in exactly one
  place (fix_hint), with no source in any skill or guide. They
  may be right; canon cannot certify what it cannot trace.
  Inventory flags them as *unsourced advice constants* — the
  canon round either sources them (iron-steel skill ratios;
  trade-shipping) or marks them as heuristics in the text.

### 3.4 Canon format and consumption (proposal)

- **Location:** `canon/` at the repo root, beside `plans/`
  (which is already the executable-rendering precedent: YAML
  authored, JSON generated, parity-checked by
  `tools/plan_check.py` + `check_plan_parity.py` — reuse that
  exact discipline rather than inventing a second one).
- **Format:** one YAML file per reasoning family
  (`canon/board.yaml`, `canon/forecasts.yaml`,
  `canon/productivity.yaml`, `canon/priorities.yaml`,
  `canon/earthworks.yaml`, `canon/plans.yaml`,
  `canon/philosophy.yaml`), each entry:
  `id` (stable, e.g. `board.stall-confirmation`), `title`,
  `player` (the Why-page text, player vocabulary), `rule`
  (the prose rule for agent rendering), `numbers` (named
  constants with values + units), `source` (where the number
  was verified: assembly / wiki page + revision / dataset /
  "heuristic — unsourced"), `status` (verified / wired /
  heuristic). A `canon/README.md` states the provenance rule
  from C-2 and the vocabulary registration from C-6.
- **Consumption:** (a) *Player* — the companion gains a build-
  time include of a generated `canon.json` (same
  `plan_to_json.py` pattern as plans) and renders `/why` +
  the per-issue canon strips from it, reading *effective*
  productivity thresholds live (C-10). (b) *Agent* — skills
  stop restating numbers: each reasoning-bearing skill gains a
  "Canon" section listing the ids it implements and defers to
  the repo files (skills already cite sources this way — the
  strategy skill's first paragraph is the model). (c)
  *Executable* — constants in `main.rs` / `productivity.rs` /
  `designate.rs` gain `// canon: board.stall-confirmation`
  comments, and a new check (`tools/check_canon.py`, modelled
  on `check_plan_parity.py`) fails the build round if a coded
  constant disagrees with its canon entry. The check is the
  point: drift becomes a red test, not a discovery.
- **Not proposed:** generating Rust constants *from* canon
  (codegen) in v3 — citation + parity check first; codegen is
  a v3.1 decision once the parity check has caught its first
  real drift.

---

## 4. Rename inventory — "Superintendent"

### 4.1 Collision check (read this first)

**No collision found.** Three web searches were run
(2026-11-09): `Captains of Industry mod "Superintendent"`;
`site:coigame.com mods Superintendent` (which returned the
official hub's mod listings — hub.coigame.com / coigame.com/Mods
— across ~40 visible mod entries); and `"Superintendent" mod
hub.coigame.com OR coigame.com/Mod`. **No Captains of Industry
mod, tool, or hub entry named "Superintendent" appeared in any
result.** The general-web results were entirely unrelated
(historical "captains of industry" texts, school-district and
industrial job postings, other games' characters). Caveats,
stated plainly: CoI mods live on the official hub (there is no
Steam Workshop for CoI — the hub is the channel; the earlier
MOD-SURVEY established this), and this check is search-index
based, not an exhaustive scrape of the hub's full catalogue —
a final in-hub search-box check for "Superintendent" at
submission time is a two-minute belt-and-braces step, recorded
here rather than claimed as done. One adjacent-name note: the
hub *does* host UNMA, whose tagline is "industrial control
room" monitoring — a different name and a different (in-game)
surface; no conflict with "Superintendent" as the name of an
external companion.

### 4.2 Everything the rename touches

**Binary / crate (code round can do all of this):**

- Crate package name `coi` (`coi/Cargo.toml`: `name = "coi"`,
  description "Companion CLI for Captain of Industry…") →
  package/binary `superintendent`. The binary *file* name is
  the user-facing artefact: `coi.exe` → `superintendent.exe`.
- CLI strings: `serve.rs:411` startup banner ("CoI Companion
  web app at…"), `main.rs:126,136` ("CoI Companion appears to
  be already running…", "CoI Companion — starting the companion
  website…"), usage/help text throughout `main.rs` (subcommand
  names `coi serve`, `coi play`, `coi plan-check` appear in
  player-visible text removed from Plans per §1.13 — in the
  manual they become `superintendent …`).
- **Deliberately NOT renamed:** the on-disk data names a
  stranger never sees but an existing user depends on —
  `coi.db` (renaming breaks every existing install's history;
  keep the filename, it is invisible in the UI), `coi.toml`
  (same reasoning; the `[game]`/`[builder]`/`[productivity]`
  sections are name-free), env vars `COI_GAME_URL` etc. (keep
  as the documented names; add `SUPERINTENDENT_*` aliases only
  if a user ever asks — recorded as a non-goal), the QoS jobs
  directory and `QoSEventLog` paths (mod-side, save-compat).

**Chrome branding (serve.rs):**

- `serve.rs:1233` page title suffix "— CoI Companion" →
  "— Superintendent"; `serve.rs:1234` top-bar brand
  "🏛️ CoI Companion" → the new brand line (the 🏛️ is
  Claudius's mark, not the product's — v3 should give the
  product its own glyph or none; the icon set has no
  superintendent glyph, and §V2.4's rule admits no unused
  glyphs: either commission `i-superintendent` (hard hat /
  clipboard are the honest candidates) or ship a wordmark-only
  brand. Flagged as a design decision inside the rename stage,
  not a blocker.)
- Mega-nav group footers and rail text that say "the
  companion" (e.g. Log's explainer "stored in this companion's
  local database", Snapshots' summary) → "Superintendent"
  where the product is meant, "this tool" where generic.
- `layout` footer line "local companion · design v2" →
  "Superintendent · design v3".

**Workflow / release (NOTE ONLY — Philip's web-UI domain):**

- `.github/workflows/release.yml`: workflow `name:
  companion-release`; artifact names `coi.exe`,
  `coi.exe.sha256`, upload-artifact name `coi-exe`; tag glob
  `coi-v*`. A rename here means: new artefact names
  (`superintendent.exe` + `.sha256`), artifact name
  `superintendent-exe`, and a *tag-prefix decision* — keep
  `coi-v*` triggers working for continuity or move to
  `sup-v*`/`superintendent-v*` and orphan the old glob.
  Recommendation to hand Philip: keep the `coi-v*` glob
  (tags are plumbing, not branding) and rename only the
  artefacts. Also the first release still has not been
  published — renaming artefacts *before* coi-v0.1.0 means no
  published asset ever carries the old name. Sequence in §5
  exploits this.

**Docs (repo):** root `README.md` (title "Captains of
Industry Toolkit" — the *repo* name is a separate question,
see below; the "Companion website" bullet and `coi.exe`
build instructions), `INSTALL.md`, `coi/README.md`,
`RELEASE-NOTES-DRAFT.md` (title/body — must be re-drafted
under the new name before Philip publishes), `RELEASE-
RUNBOOK.md` (asset names + sizes), `REMOTE-ACCESS.md`,
`DESIGN.md`/`SPEC.md` (historical — **do not rewrite history
in the spec trail**; add a rename addendum and let old text
stand as the record), `BUNDLE-README.md` + weekend bundle
artefact names in `dist/` (local staging names; the bundle
itself is superseded by releases).

**Mods (adjacent, mostly untouched):** CoiBridge and the QoS
suite keep their names — they are components, not the
product. Their readmes reference "the CoI Companion website"
(`bridge/CoiBridge/readme.txt`, `qos/*/readme.txt`,
`SUITE-README.txt`) → "the Superintendent website" at their
next rebuild; not worth a rebuild of their own (zip churn +
hash changes for a string). The QoS *event feed* path and
`companionUser: "local"` job field are data formats — frozen.

**Repo identity (NOTE ONLY — Philip's call, GitHub settings):**
repo `plcstevens/captains-of-industry-toolkit` and its
description. The repo name can stay (it describes the
toolkit: mods + companion) even as the product becomes
Superintendent; the description line should gain
"Superintendent — the companion website" so the product is
findable. Renaming the repo itself would break every link
already shared and is **not** recommended (§5).

---

## 5. Implementation plan (staged, each stage independently verifiable)

Verification instruments throughout: the existing check
scripts (`check_place_page.sh`, `check_designate_page.sh`,
`check_log_page.sh`, `check_audit_fail_loud.sh`,
`check_badge.sh`, `check_plan_parity.py`,
`check_serializer_pairs.py`), cargo tests, and a screenshot
pass against the stub (v065 capture + a populated store from
the demo-data round's stale-save captures, when it exists).
Heights re-measured per stage against the ≈2,700 px rule.

- **Stage 0 — Canon skeleton (no UI change).** Create
  `canon/` with the seven family files; enter R1–R20 with
  their *current* numbers and a `status` per entry; write
  `tools/check_canon.py` in report-only mode (prints drift,
  never fails). Resolve nothing yet — but C-2's de-run and
  C-3's coal decision are *content* decisions Philip should
  see before Stage 3 renders them; put both in front of him
  with this audit. *Verify:* check_canon runs; every §3.2 row
  has an entry or an explicit "no canon — deliberate" note
  (R19).
- **Stage 1 — Directory discipline (pure mechanics).**
  Catalogue pagination (60/page, facet-preserving);
  Research catalogue collapse-by-letter; Entities Machines
  pagination; Products server-side `?sort=`. No visual
  change beyond pagers/collapse. *Verify:* heights —
  Catalogue ≤ ~2,700 at default facet, Research catalogue
  ≤ ~2,700, Machines ≤ ~2,700; deep links (`?proto=`,
  `?domain=`, `?f=`) still land pre-filtered; check scripts
  green.
- **Stage 2 — Page contracts + intent lines.** The contract
  block (intent / domain / balance / data contract) specified
  once in DESIGN-V3 (a short addendum), then applied page by
  page in §1's order of pain: Trade, Plans, Prepare-redirect
  flash, Product/Issue detail intent lines, Terrain Z-fact
  out, Snapshots "SQLite" out, priority-split rule on
  Actions, Trade placeholder cause-neutral wording, Product
  sources D3, unpinned-history empty state, two-floor bridge
  wording (Terrain + Designate). *Verify:* screenshot pass;
  every page head carries the contract; no page's first screen
  regresses in height.
- **Stage 3 — Navigation restructure + Guidance domain.**
  Chrome changes: Game → Workshop, System → Records, Guides
  out of Library, Library dissolved (Catalogue → Workshop),
  Entities Monitor → Workshop, Plans promoted, Guidance
  domain added with Learn-the-game (existing guides, URLs
  aliased) + skeleton Learn-the-tool + Why index. Why pages
  render from canon (Stage 0 data); issue canon strips land
  on Issue detail + quadrant canon links on Issues. *Verify:*
  every §2.1 prediction-table row re-tested by hand against
  the new tree; old deep links (bookmarks to `/guides`,
  `/guide?…`) still resolve; check scripts green.
- **Stage 4 — Rename (placed here deliberately).** After the
  chrome is final (Stage 3 touches every branding string
  anyway — renaming before it would re-skin twice, against
  the brief) and *before* any release: crate/binary rename,
  brand strings, title suffix, docs pass, bundle restage,
  RELEASE-NOTES-DRAFT re-draft. Philip's parts (workflow
  artefact names, repo description) are handed to him as one
  stacked list with this stage. *Verify:* strings scan of the
  new binary (the XXII discipline) for "CoI Companion" —
  target zero outside the frozen data names (`coi.db`,
  `coi.toml`, env vars, which are *supposed* to remain);
  fresh install + upgrade-over-old-db smoke test (the db
  filename continuity is the upgrade path — prove it).
- **Stage 5 — Overview/Issues weight (the hard one, last).**
  Overview Attention summarisation (§1.1 change 2) and any
  residual height work, done with the demo-data populated
  store available so the judgement is made against realistic
  density, not the stub alone. *Verify:* Overview ≤ 2,700 on
  the v065 capture *and* on a stale-save capture; must-fix
  badge red state eyeballed on whichever capture produces a
  Fix-now issue (closing problem 5's open item).

### Explicitly NOT worth doing (and why)

- **Paginating or quadrant-collapsing the Issues board.**
  The board's value is the whole ordering on one screen; its
  height is issue-count, not waste. The fix is Overview
  summarising it (Stage 5), not fragmenting it.
- **A light theme / density toggle.** Already deferred by
  v1 §9 / V2.8 with Philip's defaults (dark only, comfortable
  tightened); nothing in this audit re-opens them — the v2.6
  contrast lifts resolved the actual complaint.
- **Code-generating Rust constants from canon in v3.**
  Citation + parity check (Stage 0) catches drift at a
  fraction of the cost; codegen couples the build to the
  canon tooling before the canon has survived one drift.
- **Renaming the GitHub repo.** Link breakage for zero
  player-visible gain; the description line carries the new
  name instead (§4.2).
- **Renaming `coi.db` / `coi.toml` / env vars.** Invisible to
  players, load-bearing for existing installs. The rename is
  a product surface, not a filesystem purge.
- **A global search index.** The top-bar jump (product/page)
  covers the recorded need; a content index is a feature
  looking for a problem at this page count, and Guidance's
  tree (Stage 3) reduces the discovery pressure that would
  justify it.
- **Moving Snapshots/Log into Guidance or Monitor.** Records
  (System) is a coherent domain once named in player words;
  churning it would spend the rename budget where no audit
  finding points.
- **Amending the 3-viewport rule for reading pages is
  proposed, not assumed:** Guide detail at 2,828 px is
  long-form reading with its live panel first; the build
  round should adopt "data pages ≤ 3 viewports; reading
  pages: live/actionable content in the first viewport, no
  height cap" as a DESIGN-V3 line for Philip to strike if he
  disagrees — listed here so it is a decision, not a drift.
