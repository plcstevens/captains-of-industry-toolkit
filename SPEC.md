# CoI Companion — Full Spec

**Status:** Design spec v1, agreed 2026-11-02 (Philip: "Write the full spec").
**Components:** (1) `CoiBridge` — a small C# mod inside Captain of Industry;
(2) `coi` — a Rust CLI + local server, single static binary for Windows / Linux / macOS.
**Game baseline:** v0.8.8 (Update 4.3). Mod toolchain: official MaFi modding repo
(`MaFi-Games/Captain-of-industry-modding`, ExampleMod), .NET Framework 4.8, `COI_ROOT`
env var pointing at the game install.

---

## 0. Why this exists — the pivot

The browser save-reader hit a hard wall (CRACKING.md §22.6, five RE sessions):
Mafi's serializer gives object bodies no length prefixes and no definition
markers, so body attribution failed. Still LOCKED from save files:

| Wanted data | Save-file status |
|---|---|
| Research completion | LOCKED (UnlockedProtosDb body unaddressed) |
| Per-product stats series | LOCKED (ProductStats are identity shells; series live elsewhere) |
| In-game date / Calendar | LOCKED (Calendar body unaddressed; day counter unverified) |
| Storage contents / entity census | LOCKED (walk-only) |
| Currently-researched node | CRACKED (§17) — the only win |

Update 4.3's web API makes the game its own parser: a mod reads **live objects**
and serves JSON. Every locked row above becomes a route. The save reader stays
as the offline fallback (§7); binary save-copying is retired as the main flow.

---

### 0.1 What the vanilla API already serves (M0 finding, 2026-11-02)

The probe (§8) found the built-in surface is far richer than the dev
diary suggested. Verbatim route inventory: `api-routes-0.8.8.json`;
live samples: `api-samples-0.8.8.md`. Vanilla already provides:

- `game-info` — **date**, tick, pause, speed, `gameId` (stable per
  playthrough — the history store's natural key), `sessionId`.
  → the LOCKED Calendar item is dead. (No population field.)
- `products` catalog (ids, names, kinds, unlocked; `pinned` supported),
  `products/stats` (quantity / stored / storage capacity + produced &
  consumed flows for thisMonth / lastMonth / thisYear / lastYear /
  lifetime), `products/history` (real per-product series at day / month /
  year / quarterCentury resolution) → the LOCKED stats-series item is
  dead, and buffer-cliff detection (§5) needs no mod at all.
- `products/sources` — per-product sources/sinks with recipe+machine
  rows and **entity rows** → bottleneck attribution evidence (§5.1).
- `icons/{id}` — real proto icons for the dashboard and guide cards.
- `/command` + `/commands` — the input-command channel (inventory of
  available commands still to capture; it defines Tier 0's real reach).
- `/screenshot` (JPG, up to 4096px), `/routes`, `/server-info`.

**Consequence:** the Rust CLI's first useful version needs **no mod**
(§9, M3 moved ahead of the bridge). The bridge keeps only what vanilla
lacks: research state, train statistics, entity census, per-storage
detail, population/settlement numbers.

## 1. Access model — read vs write (Philip's question, 2026-11-02)

We are **not** limited to read-only. There are three distinct channels, and the
spec keeps them strictly separate:

### Tier 0 — Built-in API, no mod (available today)
- `web_api_start` / `web_api_stop` console commands start/stop the server.
- Built-in routes: `/demo-dashboard`, `/screenshot`, `/routes`.
- Game commands can be sent through the API — the demo dashboard already
  **controls game speed** this way. So even vanilla, influence exists.
- Limits: cheat commands work **only in sandbox games** (0.8.8 changelog);
  commands reject locked protos and invalid values. Non-cheat commands
  (speed, pause) work in a normal run.

### Tier 1 — Bridge mod, read-only (the default build)
- `[WebRoute]` handlers on a global dependency return HTML/JSON/binary.
- Reads are **live values at request time** from the running simulation —
  not a stale export. The Rust side polls and caches, so dashboards are
  near-real-time (poll interval, default 5 s) rather than per-frame.
- A read-only mod cannot alter the save. Removing it leaves the game vanilla.
  (Per community experience, config-only mods are the removable kind; this
  bridge registers **no protos, no recipes, no entities** — deliberately.)

### Tier 2 — Bridge mod, write actions (opt-in, off by default)
- A mod runs inside the game with the modding API's full access, so writes
  are technically possible wherever the API exposes mutable state, e.g.:
  machine/building priority, recipe selection, storage import/export toggles
  and limits, vehicle/train assignments where exposed, game speed/pause.
- **Rules for Tier 2 (hard):**
  - Allowlist only: every writable action is an individually named endpoint;
    no generic "run this command" passthrough in the mod.
  - Each action is behind a per-action enable flag in the bridge config;
    default all off. Enabling is a deliberate edit on Philip's PC.
  - Every write requires a confirm token issued by the Rust CLI after showing
    the exact change ("Set Steel Furnace II priority 5 → 8?"), and is written
    to an audit log (timestamp, action, before/after) in the companion folder.
  - No writes while the game is paused-state-unknown; bridge reports sim
    state with every response so the CLI can refuse on mismatch.
  - Kill switch: deleting/renaming the bridge DLL or `web_api_stop` ends all
    access instantly. Tier 2 never runs without Tier 1's read path healthy.
- **What we will NOT do:** cheat commands in a non-sandbox run (the game
  refuses anyway), direct save-file editing, memory patching/Harmony hooks.
  If a desired action is only reachable via a hook, it stays out of scope
  until Philip explicitly re-decides.

**Design principle:** v1 ships Tier 0 + Tier 1 only. Tier 2 is specified now
so the architecture doesn't paint us into a corner, and is built only when
Philip names the specific actions he wants.

---

## 2. Architecture

```
┌──────────────────────── Philip's PC ────────────────────────┐
│  Captain of Industry (running, save loaded)                 │
│    └─ CoiBridge mod ── [WebRoute] JSON ──► game web server  │
│                         (localhost:3073 — confirmed M0)     │
│  coi.exe (Rust)                                             │
│    ├─ poller ── HTTP ──► game server (bridge + built-ins)   │
│    ├─ store ── snapshots JSONL + state DB (companion dir)   │
│    ├─ diagnostics engine (triage, bottlenecks, guides)      │
│    ├─ CLI subcommands (snapshot / status / trains / ...)    │
│    └─ serve ──► local dashboard http://127.0.0.1:<port>     │
└─────────────────────────────────────────────────────────────┘
        │ snapshot file (one JSON, hand-off)
        ▼
   Claudius (analysis, planning, blueprint work)
```

- All traffic is loopback. Nothing leaves the PC except snapshot files
  Philip chooses to send.
- The Rust binary embeds its dashboard assets; no Node, no Python, no
  runtime dependencies. `cargo` cross-builds: `x86_64-pc-windows-msvc`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`.

---

## 3. Bridge mod spec (`CoiBridge`)

**Base:** fork of the official `ExampleMod` (net48, Release build,
`COI_ROOT` set). Output: `CoiBridge/CoiBridge.dll` installed to
`%APPDATA%\Captain of Industry\Mods\CoiBridge\` (same Roaming tree as Saves).
Mods enabled in Settings → Miscellaneous; game restart after install.

**Scope (revised at M0):** vanilla serves game-info, the whole products
family, icons and screenshot (§0.1) — the bridge does NOT duplicate
them. Its routes are only what vanilla lacks:

**Route namespace:** `/coi/v1/...` (versioned; breaking changes bump v).

| Route | Returns | Unlocks (ex-LOCKED item) |
|---|---|---|
| `GET /coi/v1/research` | completed node IDs, current node + progress %, research points rate | Research completion |
| `GET /coi/v1/census` | counts by building/vehicle proto, incl. working/idle/starved/blocked splits where exposed | Census |
| `GET /coi/v1/storages` | per storage: proto, product(s) incl. mixed contents, amount, capacity, import/export flags | Storage contents |
| `GET /coi/v1/trains` | per line/train: activity shares (working/idle/refuelling/not working), travel shares (full speed, accel/brake, speed-limited, **blocked by trains**), current job/station | Train diagnostics |
| `GET /coi/v1/power` | generation/consumption by source class, battery/grid state where exposed | — |
| `GET /coi/v1/status` | population + settlement headline numbers (vanilla game-info covers date/tick/speed; population is the gap) | Settlement |
| `GET /coi/v1/debug/services` | list of resolvable global-dependency types + route table | API discovery (below) |
| `GET /screenshot` | built-in route, proxied by the Rust side | Survey images |

**Discovery first (M0 gate):** the exact public surface for product stats,
research state and train stats is not documented publicly (the feature is a
week old; no public WebRoute mod exists yet). The bridge therefore starts
with `/debug/services` + a `/coi/v1/hello` route, and the endpoint list is
finalised against what the running game + the assembly DLLs actually expose.
The assemblies Philip already owes for the serializer lookup are the same
reference set used to confirm these surfaces — one zip, two jobs.

**Performance rules (hard):**
- Handlers never scan the full entity list per request; they read manager
  aggregates the game already maintains. Census/storage detail routes are
  cached in the bridge with a 2 s TTL and computed at most once per TTL.
- Every response carries `simTick`-equivalent + wall timestamp if exposed,
  so the Rust side can detect a paused/stale game.
- Bridge failures return JSON errors, never throw into the sim.

**Safety:** read-only build has zero write calls (auditable: no setters in
source); registers no protos/recipes; adds no persisted config of its own to
the save beyond what the mod system itself records.

---

## 4. Rust CLI spec (`coi`)

```
coi probe                 # find the game server: port scan of the documented
                          # start state, GET /routes, report what's live
coi snapshot [--out F]    # one full pull (all /coi/v1/*) → single JSON file
coi watch [--interval 5s] # poll → append to history store (runs while playing)
coi serve [--port 8777]   # dashboard + diagnostics on 127.0.0.1
coi status                # one-screen summary in the terminal
coi trains                # line table with blocked/limited shares, worst first
coi products [name]       # rates, stored, time-to-empty/full
coi research              # current node, progress, ETA at current points rate
coi diff A.json B.json    # structured snapshot diff (replaces binary diffing)
coi saveinfo FILE.save    # offline mode: existing save parser (header, CRCs,
                          # screenshot extract, meta) — port of the JS parser
coi report                # triage report (see §5) as Markdown to stdout/file
```

- Config: `coi.toml` next to the binary (game port, poll interval, dashboard
  port, watch-list products, alert thresholds). Sensible defaults, zero
  required setup beyond `coi probe` finding the game.
- History store: append-only JSONL of snapshots (simple, diffable, no DB
  dependency); dashboard reads it directly. Rotation by run/save name.
- Snapshot = the hand-off artefact: one self-describing JSON (schema §6)
  Philip sends Claudius instead of an 11 MB binary save.


---

## 5. Diagnostics — triage, bottlenecks, and "around the corner"

This is the point of the tool for Philip's play: not raw numbers, but a
short, ranked list of what to do next, each item with its evidence.

### 5.1 Issue detectors

Each detector consumes snapshot + history and emits issues with evidence:

- **Product bottleneck:** a machine class with high *starved* share
  (input missing) or *blocked* share (output stuck) over the polling window;
  cross-referenced with buffer time-to-empty/full so "blocked" correctly
  blames a full downstream buffer, not the machine.
- **Buffer cliff:** watched product (steel, diesel, fertilizer, coal,
  construction parts, food) trending to empty/full within a set horizon at
  current net rate. Evidence: stored amount, net/min, minutes remaining.
- **Train network issues (Philip's standing ask):** per line and per train —
  *blocked by trains* share, *speed-limit* share (incl. new 25%-grade
  80 km/h zones), refuelling share, idle share; stations where trains queue.
  Detectors flag: blocked share > threshold over a window ("network is
  congested, not under-supplied"), trains never idle + blocked high ("add
  capacity won't help; fix junctions"), one line's blocked share spiking
  after a layout change window.
- **Power:** generation margin, sustained deficit, battery drain trend.
- **Settlement:** food variety/stock vs consumption, housing capacity vs
  population growth, unity trend.
- **Research idle:** no current node, or ETA drift vs labs' output.

### 5.2 Eisenhower triage (Philip's request, 2026-11-02)

Every issue is scored on two axes:

- **Urgency** = time-to-impact from live rates (minutes until the cliff,
  or "already biting" for starved/blocked shares above threshold now).
- **Importance** = blast radius: settlement food/power and chain-head
  inputs (iron ore, coal, crude) score highest; mid-chain buffers next;
  cosmetic/efficiency issues lowest. Weights live in `coi.toml` and are
  shown on each card — no hidden scoring.

Output — the **Recommended** board, four quadrants:

| | Urgent | Not urgent |
|---|---|---|
| **Important** | **Fix now** — act this session | **Fix later** — plan it (goes to the run's phase plan) |
| **Less important** | **Watch** — set a threshold alert, revisit if it worsens | **Ignore for now** — listed, collapsed, with the reason |

Each card carries: the evidence numbers, the detector that raised it, the
suggested fix in one line, and a link into the matching guide (§5.3). When
an issue clears, it leaves the board and is logged (so false alarms are
auditable). The dashboard's home view *is* this board; `coi report`
renders the same thing as Markdown for hand-off to Claudius.

### 5.3 "Around the corner" guides

Forward-looking cards per industry, combining **static guide content**
(built from the verified planner dataset + the CoI skills: ratios,
research gates, layout rules) with **live numbers** from the bridge:

- Each guide answers: *what demand arrives next if you follow your plan,
  and what must exist before it bites?* Examples, keyed to Philip's
  Dragon Isle run and the relocation plan's phases:
  - **Farms/fertilizer:** fertilizer demand ≈37/min for the planned
    20-farm district; nitrogen is the binding constraint (ammonia needs
    48 N₂/min vs 36 per air separator) — "build the second separator
    before the farms, not after."
  - **Steel:** casters are the sizing unit (24 steel/min per line);
    one caster pair draws 30 coal/min — import buffer warning levels.
  - **Electronics II:** poly silicon puts hydrogen on the critical path
    (4 H₂ per reactor batch) — the hydrogen backbone must precede it.
  - **Trains:** before adding a line — station split rule (a station
    cannot load and unload the same product), siding rule, fuel-station
    placement; after adding — what the blocked-share should look like.
- Guides are content files in the repo (Markdown + a small data block of
  thresholds/rates), so they improve without recompiling the binary.
- A guide card surfaces on the board automatically when its trigger
  approaches (e.g. research node X completed → guide for what X unlocks
  and what it will consume appears in *Fix later*).

---

## 6. Snapshot schema v1 (hand-off format)

```json
{
  "schema": "coi-snapshot/1",
  "takenAt": "wall-clock ISO",
  "game": { "version": "0.8.8", "saveName": "...", "mapName": "Dragontail Isle",
            "inGameDate": "...", "population": 0, "speed": 1, "paused": false },
  "research": { "completed": ["ResearchId", "..."], "current": { "id": "...", "progressPct": 0 } },
  "products": [ { "id": "Steel", "stored": 0, "producedPerMin": 0, "consumedPerMin": 0, "netPerMin": 0 } ],
  "census":   [ { "proto": "CooledCasterII", "count": 0, "working": 0, "idle": 0, "starved": 0, "blocked": 0 } ],
  "storages": [ { "proto": "...", "contents": [ { "product": "...", "amount": 0 } ], "capacity": 0 } ],
  "trains":   [ { "line": "...", "activity": { "working": 0, "idle": 0, "refuelling": 0, "notWorking": 0 },
                  "travel": { "fullSpeed": 0, "accelBrake": 0, "speedLimited": 0, "blockedByTrains": 0 } } ],
  "power":    { "generatedMw": 0, "consumedMw": 0 },
  "issues":   [ "the triage board at snapshot time" ]
}
```

Field-for-field, this is the save-reader's LOCKED list (§0) delivered live.
Rates are per-minute, normalised by the Rust side from the game's stat
windows and labelled with the window used — estimates are marked as such,
never silently presented as exact.

---

## 7. Offline mode

`coi saveinfo` ports the existing browser parser (verified 9/9 on the real
corpus: 40-byte header, both CRC32s, format version ↔ game version map,
embedded JPEG screenshot, map/seed/difficulty/mods). Purpose: save hygiene
and survey screenshots when the game isn't running, and as the regression
corpus if a future patch changes the save format again (329=0.8.7b,
331=0.8.7c, 333=0.8.7d lineage).

---

## 8. Probe plan (Philip, ~5 minutes, before any build)

1. Launch the game, load the current Dragon Isle save.
2. Open the console, run `web_api_start`.
3. In a browser: open `/routes` on the address the game reports (note the
   **port** — undocumented publicly) and `/demo-dashboard`; confirm speed
   control works and `/screenshot` returns an image.
4. Report back: the port, the `/routes` list, anything the console printed.
5. (Same sitting, optional) Copy the assembly zip
   (`Captain of Industry_Data\Managed`: `Mafi*.dll` + `Assembly-CSharp.dll`)
   — still the reference for confirming bridge API surfaces.

No mod is installed for the probe; nothing about the save changes.

---

## 9. Build phases

| Phase | Deliverable | Acceptance |
|---|---|---|
| **M0 Probe** | §8 done; port + routes known — **DONE 2026-11-02** (port 3073, loopback; full route inventory + live samples captured; only the `/commands` list is outstanding) | Philip's report; screenshot fetched |
| **M1 Bridge hello** | ExampleMod fork builds; `/coi/v1/hello` + `/debug/services` live in-game | Route visible in `/routes`; JSON in browser |
| **M2 Bridge data** | research/trains/census/per-storage/status(population) routes against confirmed surfaces (products & game-info are vanilla, §0.1) | Values match in-game UI spot-checks (current research, a train line's blocked share, one storage's contents) |
| **M3 Rust v0** | `coi probe`, `coi snapshot`, `coi saveinfo` — **against the vanilla API, no mod required**; snapshot validates against §6 (products/date sections complete; bridge sections added when M2 lands) | Snapshot of the live run answers the date/stats questions the save RE couldn't |
| **M4 Diagnostics** | `coi watch`, `coi serve`, triage board, train detectors, first guide set (fertilizer, steel, trains) | Board reproduces a known issue (e.g. a congested line) with correct evidence |
| **M5 (optional, Philip's call)** | Tier 2 write actions, named one by one | Per-action: confirm flow + audit log verified |

Build note: the bridge compiles on Windows (net48 + game DLLs via
`COI_ROOT`); the Rust CLI builds anywhere. Claudius authors both; Philip
(or a CI runner) compiles the bridge against his install.

---

## 10. Risks & open questions

- **Port/bind RESOLVED at M0 (2026-11-02, Philip's live run):** the server
  listens on `http://127.0.0.1:3073/` — **loopback only**, so nothing is
  exposed to the LAN by default; the Rust side and browser run on the same
  PC, which is exactly our architecture. Demo dashboard at
  `/demo-dashboard`, full route list at `/routes`.
- **Origin protection (confirmed at M0, 2026-11-02):** the game server
  rejects requests from other sites ("Requests from other sites are not
  allowed"); `web_api_start` accepts `allowOtherOrigins` (GET) or
  `allowOtherOriginsPost` flags to relax it. **Design rules:** the Rust
  CLI is a native HTTP client and is unaffected; the browser dashboard
  must never fetch the game directly (different port = different
  origin) — it reads from `coi serve`, which polls server-side. That was
  already the architecture; this confirms it is mandatory, not tidy.
  Do NOT enable `allowOtherOriginsPost` (any website could then send
  game commands); the GET flag is unnecessary for this design.
- **API surface depth — mostly RESOLVED at M0 (§0.1):** products,
  history, sources, game-info and icons are confirmed rich and live.
  Still unknown: the bridge-side surfaces (research state, train stat
  objects, census, per-storage detail, population) — M1/M2 territory,
  assemblies as reference — and the `/commands` inventory, which sets
  Tier 0's real write reach.
- **Patch fragility:** mods break across game updates (routine in this
  ecosystem; one hub mod above already carries "Not verified for 0.8.8").
  Mitigation: thin bridge, manager-aggregate reads only, version check in
  `/coi/v1/status`, patch-watch cron already flags new versions.
- **Performance:** polling a big late-game save must stay cheap — the
  bridge cache rules in §3 are acceptance-tested in M2 on the Expanding
  run (35.9 MB payload class), not on a fresh map.
- **Write tier:** specified but unbuilt; do not let M5 scope creep into
  M1–M4.

## 11. Non-goals

- No public release/sharing of mod, binary or snapshots without Philip's
  explicit say-so. This is a personal playthrough tool.
- No multiplayer/co-op interaction (CoopFoundry exists; not our problem).
- No save editing, no cheat surface in a non-sandbox run, no Harmony hooks.
- The planner (daxfb fork) stays a separate tool; the companion may *link*
  to its rates in guides but does not merge with it.

---

## 12. Tier 2 write API — detailed design

*Commissioned by Philip 2026-11-02: priorities, recipes, storage toggles —
plus the fixes that motivated them (wrong-direction pipes, pipe upgrades,
priority changes aimed at specific bottlenecks). This section supersedes
the outline in §1 Tier 2; the safety rules there still apply.*

### 12.1 Purpose: remediation, not remote control

The write API exists so a triage card (§5) can **carry out its own
suggested fix** after one confirmation, and so Philip can apply known
fixes from the dashboard/CLI without hunting through the factory. It is
not a general scripting channel.

### 12.2 The CoI Action Contract v1 (the "moddable" part)

Any mod — ours or anyone's — exposes actions the same way, so `coi`
can discover and drive them uniformly:

```
GET  {base}/actions                       → action registry (below)
POST {base}/actions/{id}/prepare          → validate + price a change, change nothing
POST {base}/actions/{id}/apply            → execute a prepared plan
GET  {base}/audit                         → append-only audit entries
```

Registry entry (data, not code, on the wire):

```json
{ "id": "set-priority", "title": "Set priority", "risk": "low",
  "reversible": true, "targetKinds": ["machine", "storage", "powerplant"],
  "params": { "scope": ["general", "power"], "value": "int" },
  "handler": "bridge:set-entity-priority" }
```

- **Two-phase, always.** `prepare` resolves the target, reads current
  values, checks preconditions, and returns
  `{planId, summary, before, after, warnings, cost?, confirmToken, expiresAt}`.
  `apply` takes `{planId, confirmToken}`; tokens expire after 60 s and are
  bound to a hash of target+params, so a stale or edited plan cannot apply.
- **Composite actions are config.** The bridge ships primitive handlers;
  named composites (e.g. a "steel cutover" bundle) are declared in JSON
  under `Mods/CoiBridge/actions/` and rendered generically by `coi` —
  new fixes without recompiling either side.
- **Discovery:** `coi probe` scans `/routes`; any base path answering
  `GET {base}/actions` in this contract is listed as an action provider.

### 12.3 Targeting

Every read route (§3) gains a `ref` per entity:
`{ "entity": <runtime id>, "proto": "...", "pos": [x, y] }`.
Runtime ids may not survive a save/load cycle, so `prepare` re-resolves
by proto+position and **fails loudly** on a missing or ambiguous match —
never guesses. Selectors: single ref; proto-class (all entities of a
proto in the last snapshot); rectangle (for transport upgrades).

### 12.4 Action catalogue v1

| ID | Action | Key preconditions / notes | Reversible |
|---|---|---|---|
| A1 `set-priority` | Set an entity's priority, `scope: general \| power`, value in the game's own range (range confirmed at M2) | The bottleneck fix: raise a starved line's priority, demote a hog. Evidence from §5 rides along in the plan summary | Yes |
| A2 `set-recipe` | Switch a machine's recipe | Recipe must be unlocked (game rejects locked protos anyway) and supported by the machine proto. `prepare` reports current recipe + stored inputs; switching may eject in-flight inputs under game rules — that cost is shown before confirm, not discovered after | Setting yes; ejected inputs no |
| A3 `set-storage-flags` | Per storage: allow import / allow export toggles (per product where the game models it that way) | The "storage won't release / won't accept" fix | Yes |
| A4 `set-storage-product` | Assign or clear a storage's product; keep-empty style toggles where the API exposes them (verify at M2) | Clearing a product with contents is refused at `prepare` unless `force` is passed and shown in the plan | Assign yes; forced clear no |
| A5 `set-paused` | Pause / resume a machine or building | Directly implements the relocation plan's steel rule — *pause the starter line, don't demolish* — as a one-click, reversible trial | Yes |
| A6 `upgrade-transport` | Upgrade belt/pipe segments (single, chain, or rectangle) to a named tier, e.g. Pipe I → II → III | Tier must be unlocked; the game charges materials. `prepare` reports segment count and material cost where readable. **Risk: high** | **No** — resource cost is spent |
| A7 `flip-pipe-direction` | **Status: UNVERIFIED — may not exist.** Pipe flow in CoI is pressure-driven and machine ports are fixed; "wrong direction" usually means a segment built against the intended flow. At M2 the assemblies decide: if a direction/flow setter exists, it ships as an action; if not, A7 degrades to **mark & guide** — diagnostics flags the segment, the dashboard pins its position on the survey screenshot, and the fix stays manual. Not promised as a mutation until proven | n/a |

Out of v1: vehicle reassignment, train schedule edits, terraforming,
demolish/build. Each joins only as its own named, designed action.

### 12.5 Fix plans (triage → action)

A Fix-now / Fix-later card may carry a **fix plan**: 1..n prepared
actions shown together (before/after per item, warnings, total material
cost) under one confirmation. Apply runs sequentially and stops at the
first failure; partial results are reported per item, never silently.
For reversible actions the audit entry stores before-values, and
`coi undo <auditId>` prepares the inverse plan (itself confirmed —
undo is a write too).

### 12.6 Safety rules (in addition to §1)

- Bridge config lists **enabled action ids**; default is none. Philip
  enables each action deliberately on his own PC.
- No `apply` without a live `prepare`; no apply if the last snapshot is
  older than 10 s or the game reports paused-state mismatch.
- Audit log is append-only JSONL in the companion folder: timestamp,
  action id, target ref, before, after, plan summary, result.
- The generic command channel (Tier 0) is never proxied through the
  bridge — actions only, no passthrough.

---

## 12.7 M0 addendum — most of this catalogue is VANILLA (2026-11-02)

The `/commands` inventory captured from Philip's live game (240 input
commands, full constructor schemas served per command) maps almost 1:1
onto §12.4:

| Spec action | Vanilla command(s) |
|---|---|
| A1 set-priority | `SetGeneralPriorityCmd`, `SetElectricityGenerationPriorityCmd`, `SetCustomPriorityCmd`, `SetConstructionPriorityCmd`, `SetArrivalRuleNetworkPriorityCmd`, `SetTrainSchedulePriority` |
| A2 set-recipe | `MachineSetRecipeActiveCmd` (machineId: int, recipeId: string, enableRecipe: bool), `MachineToggleRecipeActiveCmd`, `ReorderRecipeCmd` |
| A3 storage flags | `StorageDisableLogisticsToggleCmd`, `StorageSetSliderStepCmd`, `ToggleTrainStationStorageLinkCmd` |
| A4 storage product | `StorageSetProductCmd`, `AssignProductToSlotCmd` (entityId, productId?, slot), `SorterToggleProductCmd`, `TrainStationModuleSetProductCmd` |
| A5 pause/resume | `ToggleEnabledCmd`, `ToggleTrainPausedCmd`, `SetConstructionPausedCmd` |
| A6 upgrade-transport | **Still no direct command** (upgrade commands exist only for space station / world-map entities) — stays bridge-or-manual, as flagged |
| A7 flip-pipe | `DeconstructTransportSegmentCmd` / `ClearTransportCmd` exist; a direction setter does not appear — mark & guide stands |
| (bonus) native alerts | `StorageAlertSetEnabledCmd`, `StorageAlertSetThresholdCmd` — the game has its own storage alert thresholds; §5 detectors should read/align with these rather than duplicate them |

Consequences:
- The Action Contract's two-phase safety (prepare → confirm → apply,
  audit) moves entirely into the Rust CLI: `prepare` = read current
  state via GET routes + render the plan; `apply` = POST the named
  command to `/command`. No bridge code needed for A1–A5.
- Entity ids are plain ints in command args. `products/sources` entity
  rows provide some; a full entity directory (id → proto/position) is
  still bridge territory and is now the bridge's main read job alongside
  research/trains/census/population.
- `/command` is a live write channel into a running game: the §12.6
  rules (allowlist, confirm tokens, audit) apply unchanged, enforced
  client-side in `coi`.

---

## Build status (2026-11-02, afternoon II) — command surface expanded

Per Philip's pick, the website's allowlist grew from 2 to 7 commands —
the no-EntityId families: ResearchStartCmd / ResearchStopCmd /
ResearchQueueDequeueCmd, PinToggleCmd, SetGlobalPriorityCmd. New
/research page (250-node catalogue extracted from the game's own data
tables via the save-vocab method; state not API-readable, stated on the
page), pin/unpin buttons on Overview/Products/Product pages (prepare
summary resolves pin vs unpin from live state), and a global-priority
entry form on /actions with honest "ids not published" labelling.
Live-verified against Philip's game: research page + all prepares
render; the pin toggle was applied twice on Product_Acid (pin → verified
pinned → unpin → verified restored, net-zero, both in the audit log).
Research/priority applies were deliberately NOT executed in testing
(they change his run); the apply path is the one already proven by
pause/speed/pin. dist/coi.exe rebuilt.

## Build status (2026-11-02, evening)

- **M3 Rust v0 DONE and live-verified**: `coi` probe / status / snapshot /
  products / report / serve / saveinfo all working against Philip's
  running game through the tailnet forward (code: `coi/`, git-versioned).
  Report detectors tuned v0.1 (history-confirmed stalls, storage-full
  threshold, structural-deficit detector, import-fed coal exemption).
  `saveinfo` is byte-faithful to the browser save reader (cross-checked
  field-by-field; screenshot extraction byte-identical).
- **M1 bridge skeleton DONE (source-only)**: `bridge/` — csproj, IMod
  entry, [WebRoute] handlers for /coi/v1/hello + /coi/v1/debug/services,
  contract-shaped stubs for research/trains/census/storages/status.
  Compiles only on Windows against the game assemblies; 8 GUESS: items
  await the assembly zip (chief: IGameApiServer DI shape + WebRoute
  attribute namespace).
- Remaining: M2 bridge data services (needs assemblies), M4 detectors
  (trains/power/settlement — need bridge), M5 Tier 2 via the §12.7
  vanilla-command mapping, cross-builds for Windows/macOS binaries.

### Storage decision (2026-11-02, Philip's question)

Philip asked whether the companion should run "MySQL or Rust equiv" for
local storage. Decision: **SQLite** (rusqlite, bundled — compiled in, no
server process, no system dependency) as the local store; JSON snapshot
files remain the interchange format. A MySQL server was rejected:
single-user, single-machine tool; a daemon + credentials + migrations
is pure overhead, and SQLite covers the query load (trends/diffs/audit)
at this scale (hundreds of products × a snapshot every poll interval).
§6's JSONL plan is superseded: snapshots land in `coi.db` (games /
snapshots / product_stats / product_history / issues / audit), deduped
on (game_id, tick). `coi trend` and `coi db import` shipped with it.

---

## Product correction (2026-11-02, Philip — supersedes §0/§9 framing)

Philip, verbatim sense: the deliverable is **a website you log into**,
served by the Rust binary running locally on the game PC (no Tailscale
needed in that topology) — "IT IS NOT A CLI". The website is the
product: issue identification, fix guidance, and commands it can run
(§12 flow), reading in and writing out. The CLI subcommands are demoted
to a debug/admin shell over the same engine. Shipped the same day:
login (salted-hash single user in SQLite, session cookie, CSRF),
Overview / Issues / Products (+detail with sources & trends) /
Actions / Snapshots pages; command execution verified live
(SetSimPauseStateCmd, GameSpeedChangeCmd; game answers
{"ok":true,"result":true,"error":"","processedAtStep":…}); Windows
single-file build `dist/coi.exe` (4.6 MB, system DLLs only).

---

## Entity-id handling — options analysis (2026-11-02)

Question (Philip): how do we run entity-targeted commands when the
vanilla API serves no entity directory, and why the assembly zip?

Scan of the captured `/commands` inventory (240 commands):
**153 take an EntityId; 87 do not.** The 87 take proto-id strings,
domain ids (TrainId/TrainLineId/NetworkId/ZoneId), or nothing.
Immediately usable families: research (ResearchStartCmd /
ResearchStopCmd / ResearchQueueDequeueCmd — nodeId is a proto string),
SetGlobalPriorityCmd (priorityId string), PinToggleCmd /
PinnedProductReorderCmd, ship/fleet globals (FleetToggleAutoReturnCmd
etc.), SetPlanningModeEnabledCmd, TogglePartialTrucksLoadCmd,
ToggleDefaultLogisticsMode, QuickTradeCmd / ToggleContractCmd
(proto ids). GameConsoleCmd(string) exists — a meta-channel, NOT for
the allowlist by default (cheat surface outside sandbox).

Routes to the full 153:
- **A. Non-EntityId surface (above)** — no mod, no zip. Next site build.
- **B. Bridge census** — the mod enumerates entities at runtime and
  serves (id, proto, position). Its /debug/services discovery route
  dumps the game's real service/type surface from inside the running
  game, so census code can be written against reported facts and
  iterated on Philip's PC. No zip required; zip makes it first-try.
- **C. Save-file registry decoding** — research nodes proved the save
  serializes (objId, typeId) directories; a per-manager census may be
  extractable (CRACKING.md programme). Offline; whether save objIds
  equal live runtime EntityIds is plausible but UNVERIFIED.
- **D. The assembly zip's actual role** — Mafi*.dll/Assembly-CSharp.dll
  from Philip's own install, read-only reference for Claudius: (1)
  serializer field order → unlocks C and the save-reader's remaining
  locked items; (2) bridge data-service APIs (entity/research/train
  managers) → B right first time; (3) settle save-vs-runtime id
  identity. It does NOT gate the website, route A, or compiling the
  bridge skeleton (COI_ROOT on Philip's PC covers that).
- **E. Rejected**: probing sequential ids with toggle commands on the
  live save — mutates unknown machines; sandbox mapping doesn't
  transfer between saves.

---

## Build status (2026-11-02, late afternoon) — bridge M1.1 discovery-first

Philip: "keep going" → route B advanced without the assembly zip.
CoiBridge bumped to **0.2.0**: new `DiscoveryReport.cs` turns
`/coi/v1/debug/services` from a route-table stub into a real runtime
discovery report — the resolver object captured in
`RegisterDependencies` (metadata only, never invoked) plus reflection
over loaded Mafi / Assembly-CSharp types filtered to the M2 families
(web API, research, trains, storage, entity managers, population /
settlement), each with public methods/properties/fields, capped and
cached. This is how M2's backing services get pinned from inside
Philip's running game; the zip would only have made it faster.

Validation here: all three .cs files parse clean under a real C#
grammar (tree-sitter). NOT type-checked — no .NET toolchain or game
assemblies on this VM; the first real compile stays on Philip's PC
(the two standing GUESS items — [WebRoute] namespace and the
RegisterDependencies signature — are unchanged and remain the likely
first-build fixes). Source package:
`dist/CoiBridge-0.2.0-source.zip`.

Next step is Philip's: build in Visual Studio with COI_ROOT set
(bridge/README.md), enable mods, `web_api_start`, open
/coi/v1/debug/services, leave the game running with the tailnet
forward up — Claudius fetches the report and writes M2
(research/trains/census/storages/status + the entity directory that
unlocks the website's entity-targeted actions).

---

## Build status (2026-11-02, late afternoon II) — bridge v0.3.0, contract grounded in real mod sources

Philip pushed back: explore how CoI mods are actually written — "surely
there is all the stuff you need on the codex". He was right. The
official template was the wrong place to stop; the working knowledge
lives in shipped community mods and MaFi's own diary/changelog. Full
citations: `bridge/MODDING-NOTES.md`; sources cloned (read-only) to
`vendor/community/` (PriorityManager, data-exporter, ResearchQueue,
coi-balancer, DoubleQoL).

What changed in v0.3.0:
- **The mod contract is no longer a guess.** Our old
  `RegisterDependencies(DependencyResolver)` override +
  `EnsureResolved` call was the WRONG shape. Real mods use
  `RegisterDependencies(DependencyResolverBuilder, ProtosDb, bool
  gameWasLoaded)` with `depBuilder.RegisterDependency<T>().AsSelf()`.
  CoiBridgeMod now follows BalancerPlanner's proven hybrid:
  `DataOnlyMod, IMod` with explicit IMod members; Initialize resolves
  BridgeRoutes and captures the live resolver for the discovery
  report. BridgeRoutes additionally carries
  `[GlobalDependency(RegistrationMode.AsEverything)]` (root Mafi
  namespace) — PriorityManager registers both ways; so do we.
- **M2 backing services verified for census + research**:
  IEntitiesManager (Mafi.Core.Entities; Entities enumerable,
  TryGetEntity(EntityId, out T)) and EntitiesManager
  .GetAllEntitiesOfType<T>() — entity.Id + Prototype.Id IS the entity
  directory; ResearchManager (Mafi.Core.Research), CurrentResearch =
  Option<ResearchNode>, queue private (m_researchQueue). Trains and
  population services were NOT found in community code — the discovery
  report remains the tool for those two.
- **Manifest**: version 0.3.0 + `can_add_to_saved_game` /
  `can_remove_from_saved_game`: true — Philip can add CoiBridge to his
  existing Expanding save, and removal is safe (no content registered).
- **One unknown remains**: the [WebRoute] attribute's namespace.
  Diary #57 + the 0.8.8 changelog's modding section confirm the
  mechanism ("marking a method of a global dependency with [WebRoute],
  or register routes of other objects with RegisterRoute") but never
  name the namespace; GitHub code search finds no community usage of
  the week-old feature. The guess stays confined to one using line in
  BridgeRoutes.cs; a wrong guess surfaces as a compiler error naming
  the fix. The earlier "8 GUESS items / IGameApiServer DI shape" risk
  framing is superseded by this section.

Validation: all .cs parse clean (tree-sitter); still not type-checked
(no .NET/game assemblies here) — first compile remains Philip's, but
it now compiles against patterns copied from mods that ship.
Package: `dist/CoiBridge-0.3.0-source.zip` (supersedes the 0.2.0 one).

---

## Build arrangement (2026-11-02, Philip's instruction)

Philip: "i dont build in VS, you do it all yourself please." Agreed —
the build moved to this VM:

- .NET SDK 8.0.427 installed user-space at `~/.dotnet` (direct tarball
  extract with `tar --no-same-owner`; the dotnet-install script fails
  here — it stages in /tmp, a 512 MB tmpfs, and tar chokes on archive
  ownership). net48 compilation PROVEN: hand-written SDK-style project
  + `Microsoft.NETFramework.ReferenceAssemblies.net48` 1.0.3 → clean
  build, DLL produced.
- VM build scaffold ready: `bridge/vm-build/` (CoiBridge.csproj
  compiling the same sources against `refs/`, build.sh packaging
  `dist/CoiBridge-mod-<version>.zip` as a drop-in Mods folder).
- Alternatives exhausted for getting the game assemblies without
  Philip: public mod repos don't redistribute them (MaFi modding
  policy — that's why every csproj references COI_ROOT); his PC's
  tailnet exposes only the game API port (445/22/3389 closed); the
  API serves no type metadata.
- The ONE remaining step on Philip's side: a one-line PowerShell
  Compress-Archive of `Mafi*.dll` + `Assembly-CSharp.dll` from his
  Managed folder to Desktop\coi-dlls.zip, dropped into the side chat
  (sent 2026-11-02 16:23). On receipt: inspect assemblies (WebRoute
  namespace + M2 surfaces settled by reading, not guessing), fix,
  compile here, return the finished mod zip. His remaining actions
  after that: unzip into Mods, enable CoiBridge on his save,
  `web_api_start`.

## Addendum 2026-11-02 (III) — assemblies received; CoiBridge v0.4.0 BUILT

Philip ran the PowerShell zip (corrected once: CoI ships no
Assembly-CSharp.dll; the Mafi*.dll set is complete) and attached
`coi-dlls.zip` at 16:35. Assemblies extracted to
`~/workspace/coi-companion/refs/` and read with ilspycmd. Settled by
reading, not guessing (full detail in bridge/MODDING-NOTES.md):
[WebRoute] = Mafi.Core.WebApi.WebRouteAttribute with a REQUIRED
WebRouteStage argument; handlers return WebRouteResponse; route
registration happens because WebApiServer.scanAllRoutes() scans the
resolver's AllResolvedInstances on first start. BridgeRoutes +
CoiBridgeMod were fixed to the real signatures (handlers return
WebRouteResponse.Json; stubs answer 501; Initialize resolves
BridgeRoutes and late-scans via the public ScanObjectForRoutes if the
server is already running) and compiled on the VM:
**CoiBridge-mod-0.4.0.zip — build succeeded, 0 warnings, 0 errors.**
Delivered to Philip 2026-11-02 ~16:40. His remaining steps: unzip
into %APPDATA%\Captain of Industry\Mods\, enable CoiBridge on his
Expanding save, `web_api_start`, leave the game running with the
tailnet forward up. Next on receipt of a live game: fetch
/coi/v1/hello + /coi/v1/debug/services, then M2 (census via
IEntitiesManager, research via ResearchManager, trains/population
pinned from the discovery report).

## Addendum 2026-11-02 (IV) — bridge LIVE; companion UI overhaul; guides

CoiBridge v0.5.0 installed by Philip and verified live end-to-end:
all routes answer (hello/research/status/census/storages/trains/
topology). Population 2,320 (the relocation plan's [pop] figure).
Percent.RawValue calibrated = percent × 1000 (two independent
cross-checks). Trains route reports zero lines on the Expanding
save. Companion website gained: issue detail pages (reasoning,
evidence, forecast, fix, guide links, per-issue action buttons),
the Entities page (bridge directory with storage/machine controls),
entity-targeted actions on the §12 flow (ToggleEnabled,
MachineToggleRecipeActive, StorageSetProduct,
StorageDisableLogisticsToggle, SetGeneralPriority — schemas from
the captured /commands inventory), filterable/sortable tables, a
seasonal/bursty production rhythm in the detectors and forecasts
(bursty = ≥half of finished months at zero, peak ≥ monthly
consumption and ≥2× mean; judged on the long-window balance), and
a Guides section (7 failure-mode guides, content researched from
the official wiki/diaries/translation hub + community practice;
digest in guides-research.md).

## Addendum 2026-11-02 (V) — NORTH STAR: an agent that plays

Philip, 2026-11-02 20:33: "our goal is that you can set up a Muse
agent to PLAY THE GAME FOR US by itself sometime… we want to have
all actions understood, a reasonable GAME PLAN FOR A GIVEN MAP
START and play to it."

What that makes the current stack: substrate, not side quests.
- EYES (done): full state reads — vanilla API + bridge census,
  topology, storages, research, settlement status.
- JUDGEMENT (done, v1): triage, forecasts (incl. seasonal rhythm),
  failure-mode guides.
- HANDS (partial): 12 commands live-verified through the §12
  prepare→confirm→audit flow. The other ~228 inventoried commands
  are catalogued but not yet understood one by one.

The road, in order:
A. ACTION CATALOGUE — every command in the /commands inventory
   gets: purpose, argument semantics, preconditions, effect,
   reversibility, and a verification status (docs / tested-safe /
   untested). Surfaced in the companion as the "actions understood"
   registry. Known open question, possibly THE blocker:
   construction — the inventory has construction pause/priority
   verbs; whether any command places/upgrades/demolishes buildings
   or assigns vehicles/schedules trains is unproven. If the vanilla
   channel cannot build, the bridge mod growing a Tier-2 build
   surface is a deliberate, separate decision (it changes the
   mod's read-only posture) — Philip's call when we get there.
B. GAME PLAN FORMAT — a machine-executable plan for a given map
   start: phases with entry gates (stockpiles, research, power
   headroom), build/research orders, and the checks that close a
   phase. Conventions from the starter skill + relocation plan;
   first instance: a fresh-start plan for a defined map/difficulty.
C. SUPERVISED PLAY LOOP — observe → compare to plan → act →
   verify, on a sandbox save first, with per-action-class autonomy
   levels set by Philip and the audit log as the record. Autonomy
   expands class by class as the catalogue's verified coverage
   grows; the §12 confirm step is the training-wheels form of the
   same flow.

## Addendum 2026-11-02 (VI) — action catalogue built; construction primitive FOUND

Step A of Addendum V is underway in the companion: a Catalogue
page generated from the captured /commands inventory
(tools/gen_catalogue.py -> src/catalogue.rs) — all 240 commands
classified into 13 domains with argument schemas, entity
targeting (153 entity / 87 global), and status: 12 verified live,
1 excluded by policy (GameConsoleCmd), 227 schema-known. The open
question from Addendum V is ANSWERED at the schema level:
CreateStaticEntityCmd(protoId, TileTransform,
allowValidationSuppression) exists in Mafi.Core.Entities.Static.
Commands, alongside FinishBuildOfStaticEntityCmd,
CreateTrainTrackEntityWithDirectionCmd, CreateNewTrainLineCmd,
CreateNewTrainNetworkCmd and AddVehicleToBuildQueueCmd. The
vanilla channel therefore DOES contain a placement primitive; what
remains untested is its semantics (construction site vs finished
building, TileTransform encoding, cost/validation behaviour) —
sandbox-save experiments are the next step, never the live save.

## Addendum 2026-11-02 (VII) — the uncovered remainder (Philip's build queue)

Philip, 2026-11-02 20:54 (correcting a stand-down misread): "no
build more shit — like into the C# mod level — to do all the
commands. I want all the things left to build that we hadn't
covered: that's contracts, trade, its priority setting etc etc."
Direction: DEPTH over surface. 228 of 240 commands are still
uncovered (12 verified). The queue:

A. Wire + verify the remaining command families (vanilla channel):
   1. Contracts & trade (6): ToggleContractCmd, QuickTradeCmd,
      AcceptLoanCmd, MakeLoanOverpaymentCmd,
      SetLoanBufferPriorityCmd, TradeDockToggleUnloadPriorityCmd.
   2. The full priority family (10): SetCustomPriorityCmd,
      SetElectricityGenerationPriorityCmd, construction priority
      pair, train schedule priority, arrival-rule network
      priority, ZipperSetPriorityPortsCmd (balancer ports),
      shipyard/trade-dock unload priorities.
   3. Storage sliders & alerts (8): StorageSetSliderStepCmd (the
      keep-full/keep-empty sliders), StorageAlertSetEnabled/
      ThresholdCmd, fuel sliders, stacker/sorting/well alerts.
   4. Trains end-to-end (68 ns): line/network creation, schedule
      items, train assignment, wagon product filters, skip rules.
      (Cosmetic colour commands are lowest priority.)
   5. Vehicles & logistics zones (33): build queue, assignments,
      zones, TruckJobsAllow/Forbid, navigation, scrap/replace.
   6. World/ships/cargo (26): shipyard ops, fleet crew/fuel,
      world-map repair/upgrade, settlement adoption.
   7. Farms & forestry (8): FarmAssignCropCmd, fertility target,
      harvest-now, slaughter limits, forestry tower settings.
   8. Terrain & mining (6): mine shift counts, tree
      designate/plant, excavator preferred product.
   9. Space program (15): launch, station build/upgrade, cargo
      buffers, auto-launch.
   10. Construction semantics: CreateStaticEntityCmd family —
       sandbox placement tests (site vs finished, TileTransform
       encoding, costs).

B. CoiBridge v0.6 — the C# mod level (Philip's phrase). The agent
   cannot play what it cannot see, and the vanilla API does not
   expose: contract offers/active terms, loan state, train
   schedule state, per-entity priority values, storage slider
   values, or EDICTS — the inventory contains ZERO edict commands,
   a genuine hole (investigate the mechanism; likely a bridge
   route into the edicts manager). Bridge read routes for the
   state above; bridge write routes only where the command
   channel provably cannot act. This grows CoiBridge from census
   reader into the full state + actuation layer under the
   companion/agent.

Recon note (2026-11-02 20:56, local files only): the 13
contracts/trade/priority/slider commands all have small clean
schemas (ids + ints/bools; StorageSetSliderStepCmd takes nullable
import/export/transport steps; StorageAlertSetThresholdCmd takes
a 0..1 fraction) — wiring them is mechanical, same pattern as the
existing entity actions. The read side is the real gap, now
evidenced: api-routes-0.8.8.json contains ZERO routes mentioning
trade, contract, loan or edict — no vanilla read exists for that
state at all. CoiBridge v0.6 read routes are therefore a hard
prerequisite for agent play in those families, not an enhancement.

### Addendum VII — build-out status (2026-11-02 ~22:05, after Philip's "do it all now")

- **Generic command execution: BUILT + LIVE-VERIFIED.** The catalogue
  policy changed: every command except GameConsoleCmd (excluded) is
  preparable — curated forms where they exist, otherwise a generic
  form generated from the command's captured schema (GET /prepare,
  POST /actions/prepare-generic), always through prepare → confirm
  (exact JSON) → audit. Statuses: 13 verified, 226 wired, 1 excluded.
  Proof run: StorageAlertSetEnabledCmd executed + reverted live via
  the generic flow, both audited "success", net zero on the save.
  This covers Addendum VII §A families 1–9 functionally; per-family
  curated pages remain polish, not coverage.
- **CoiBridge v0.6.0: BUILT, packaged, delivered (install = Philip's
  step).** New read routes /coi/v1/contracts (ContractsManager),
  /coi/v1/loans (LoansManager), /coi/v1/edicts (EdictsManager),
  /coi/v1/trains-detail (TrainLine id/network/schedule). Separate
  dependency class resolved in its own try/catch — a v0.6 failure
  cannot take the v0.5 routes down. Recon findings: edicts DO have
  a command (ToggleEdictEnabledCmd, an InputCommand processed by
  EdictsManager) but it is NOT in the web API's /commands inventory —
  the "240 commands" are the web API's registry, not the game's full
  command set. Edict actuation via /command is therefore UNTESTED;
  needs a sandbox save before anyone fires it at a real game.
- **Companion Trade page (/trade): BUILT.** Contracts (establish /
  cancel via ToggleContractCmd), loans (terms, active loans,
  overpayment + borrow forms through the generic endpoint), edicts
  (read-only table with the game's own reasons). Panels degrade to
  an "update the mod" note until CoiBridge v0.6 is installed.
  Entities page shows trains-detail when lines exist.
- Still open: §A10 construction semantics (sandbox), quick-trade
  provider enumeration (no manager surface found yet — providers not
  located; QuickTradeCmd works via the generic form with a typed
  trade id), per-entity priority/slider READ routes (values not yet
  exposed; the write side is fully wired), game-plan format
  (north-star step B).

### Addendum VIII — overnight build mandate (Philip, 2026-11-02 ~21:07)

- "Do this ALL NIGHT. Do not stop building. Write lots of self-LLM
  skills and context rules as you go" — for (1) building the Rust
  companion, (2) building the C# mod, (3) PLAYING the game as an
  agent.
- HARD CONSTRAINT: **no uploads for Philip in the next 24 hours**
  (he is asleep, then travelling; he cannot touch them). All
  deliverables stage locally in dist/; upload only after the window
  (from ~2026-11-03 21:07 Paris) or on his word.
- Note: the two uploads made at ~21:06 (coi.exe + CoiBridge 0.6.0
  zip) predate the instruction by seconds; their links expire
  2026-11-04 ~20:06 UTC and are recorded in the chat report.
- Overnight queue: the three skills; bridge recon round 2
  (priorities/sliders/quick-trade providers); game-plan format +
  first plan (north-star step B); design-diagnosis v1 against
  captured live data; read-only snapshots via tailnet allowed;
  NO command applies to the live save overnight (prepares only).
  Progress log: ~/workspace/coi-companion/NIGHT-LOG.md.

### Addendum VIII — overnight session results (2026-11-02 night → 2026-11-03)

Full log: NIGHT-LOG.md in this folder. Summary of the night queue:
- Three skills written (local-coi-companion-dev, local-coi-bridge-dev,
  local-coi-playing) under ~/workspace/custom-skills/.
- Bridge recon round 2: priorities / storage sliders / quick-trade
  provider ownership (results in NIGHT-LOG + MODDING-NOTES; a v0.6.1
  bridge may be staged in dist/ — still NO uploads until the window
  ends ~2026-11-03 21:07 Paris or Philip says otherwise).
- Game-plan format (north-star step B): GAME-PLAN-FORMAT.md +
  plans/fresh-start-v1.yaml (New Haven, normal).
- Design-diagnosis v1: tools/diagnose_live.py over the 2026-11-02
  captures → live/diagnosis-v1.md. Proven faults: 10 unconnected
  transports (5 single-tile dead ends); crude oil over capacity
  (9,713 stored vs 9,000); steel chain starved (corroborates the
  known stall); Crusher 8/8 output-full.
- coi/README.md refreshed for the generic prepare + Trade page.

### Addendum IX — Round 3 (2026-11-03 afternoon; Philip in London, PC off, "do much more")

- **Companion UI now renders every bridge route.** Overview power
  panel (/coi/v1/power kW fields); Trade quick-trades panel with
  QuickTradeCmd prepare buttons (arg tradeId); Entities storage
  rows show v0.6.1 slider steps + general/import/export priorities
  (emitter keys are camelCase) plus a Non-default priorities panel
  (census priorities = entityId -> general priority); Research page
  lists the v0.6.2 "researched" ids. Overview/Entities/Issues now
  degrade to 200-with-notice when the game API is down (was 502).
  Verified in degradation only (PC off); populated rendering awaits
  the next live session. coi.exe rebuilt + staged: 4,972,701 B.
- **Play-loop engine (north-star step C) — DRY-RUN evaluator built.**
  tools/plan_check.py (Python: the coi crate has no YAML dep and
  serde_yaml is absent from the offline cargo cache). Three-valued
  gate evaluation over a state source, active-phase derivation,
  would-prepare order rendering; NO send path exists in the tool.
  Recorded interpretations: `for:` durations -> instantaneous TRUE
  reads UNEVALUABLE on one snapshot; phase COMPLETE = exit gate
  TRUE; out-of-vocabulary metrics -> UNEVALUABLE + lint note.
  First dry-runs: continuation Phase 0 COMPLETE, Phase 1 ACTIVE.
- **Second plan instance:** plans/dragon-isle-continuation-v1.yaml
  (8 phases, 36 orders) + tools/validate_plan.py; both plans pass
  validation with 0 errors. Noted conflicts it carries: save
  version 0.8.7d (relocation plan) vs 0.8.8 (SPEC) — executor
  refuses mismatch, human confirms; trains route-vs-census
  mismatch flagged, not resolved.
- **Diagnosis fixes:** live/diagnosis-v1-fixes.md — 13 fault
  sections classed companion-action (2) / blueprint (1) /
  manual (2) / observe (7) + a deliberate-no-fix list. F10
  (MaintenanceDepotT1) has no honest fix path from captures: no
  maintenance product exists in the vanilla data.
- **SANDBOX-PROTOCOLS.md** — minute-by-minute protocols for
  CreateStaticEntityCmd (Protocol A, test proto StorageUnit) and
  ToggleEdictEnabledCmd (Protocol B, runtime selection rule +
  control-send envelope capture + variant ladder). Both carry the
  Expanding gameId abort guard. Ready when Philip provides a
  sandbox save.
- Constraints held all round: zero uploads (window to ~21:07
  Paris), zero live applies, deliverables staged in dist/ + the
  project tree only.

### Addendum X — Round 4 (2026-11-03 late afternoon; Philip in London, PC off, "keep going")

- **Rust plan engine: BUILT + PARITY-VERIFIED.** `coi plan-check
  <plan.json> [--state-dir DIR] [--out report.md]` (coi/src/plan.rs)
  is a faithful port of tools/plan_check.py — same three-valued
  semantics, counting rules, active-phase derivation and
  would-prepare rendering. It consumes the canonical JSON form from
  tools/plan_to_json.py (PyYAML-based conversion; the crate still
  has no YAML dependency). Parity gate: tools/check_plan_parity.py
  diffs Rust verdicts against the Python reference reports — PARITY
  PASS on both plans (fresh-start 44 gate items, counts 30/21/14;
  continuation 49 items, 32/33/8) and full parity on the early-game
  fixture. The Python tool remains the verified reference; if the
  engines disagree, fix the Rust side. A stale-prose bug the fixture
  exposed (the reports' G1 note ignored captures that DO carry a
  `researched` list) was fixed in BOTH engines, verdicts unchanged.
- **Website /plans page: BUILT + VERIFIED.** Both canonical plans
  and plans/verdicts.json (tools/gen_verdicts.py, computed with the
  reference evaluator's machinery) are baked into the exe via
  include_str!. The page shows phase tables + dry-run statuses and
  wires the active phase's WOULD-PREPARE order into the existing
  generic prepare flow — concrete args hidden, §6.2 selector args
  as labelled inputs (NOT auto-resolved; the confirm step remains
  the control). Verified on a scratch server with the game
  unreachable: page 200, form submit reached the confirm page.
  dist/coi.exe rebuilt + staged: 5,306,248 B (embeds plan-check,
  the /plans page, the plans and the verdicts).
- **Early-game fixture:** plans/fixtures/early-game/ — a synthetic,
  internally consistent month-1 state; fresh-start reads phase 0
  COMPLETE / phase 1 ACTIVE / nothing late COMPLETE under BOTH
  engines. Authoring caveats recorded in the playing skill
  (zero-history lastMonth windows read vacuously TRUE; unguarded
  checks in unreached phases are report noise).
- **Decoder recon pack (bridge/MODDING-NOTES.md, recon round 4) —
  assembly-verified, NOT live-verified:** ElectricityAvgStats raw
  values are unscaled kW with RLE history rings + a documented
  public read surface; train activity/travel shares are the shared
  ProductivityCounter family (600-report buckets, category maps,
  two different panel denominators — a decoder must state which);
  custom priorities have NO proto — the id universe is 7 hardcoded
  strings across 6 entity classes, safely readable only via
  IsCustomPriorityVisible (GetCustomPriority asserts). This is the
  preparation for a future trains-stats route + power history —
  nothing was built from it this round, per the no-undecoded-data
  discipline.
- **Blueprint reference audit:** tools/audit_blueprints.py — all
  20 blueprint_refs across both plans are declared
  not-yet-authored logical names; zero undeclared-missing, no plan
  edits needed. The pack (79 ids) still has no assets under the
  plans' district naming — authoring those blueprints is future
  work, not a reference error.
- Constraints held all round: zero uploads (window to ~21:07
  Paris), zero live-game contact (PC off), zero applies.
  Deliverables staged in dist/ + the project tree only.

### Addendum XI — Round 5 results (2026-11-03 evening, Philip in London, PC off)

- **Plan semantics guards APPLIED (plans → v1.1.0).** The two
  Round-4 authoring caveats are now guards in the plan files, not
  documentation: (b) every `lastMonth` net_flow leaf in both plans
  (21 leaves total across the plans) carries an activity guard —
  `any: [produced > 0, consumed > 0]` in the same window — so a
  0−0 window can no longer read vacuously TRUE; `for:` durations
  moved onto the guarded composites. (a) fresh-start's
  `p4-diesel-net-flow` carries a diesel-chain guard
  (`product.produced Product_Diesel lifetime > 0`); quantity
  floors/invariants deliberately untouched. validate_plan PASS
  (0 errors); Python↔Rust parity PASS on live + fixture. 14
  verdict flips vs the Round-3/4 reports — ALL expected, every
  one traced to a literal 0−0 window, ZERO phase-status changes.
  The diesel check now stays quiet on the month-1 fixture and
  still fires on the live capture (real deficit).
- **Bare-exe launch.** Bare `coi` / double-clicking coi.exe now
  starts the website and best-effort opens the browser at
  http://127.0.0.1:8484/; if the port is already bound it prints
  "already running", opens the browser and exits 0. Explicit
  subcommands unchanged (`status` is a one-shot again only when
  asked for). README quickstart rewritten to match. Verified on
  the VM: /login 200, second instance exits 0.
- **CoiBridge v0.6.3** (staged dist/CoiBridge-mod-0.6.3.zip,
  19,656 B; build 0/0): /coi/v1/power gains an `averages` block
  (production/consumption/generationCapacity × last
  day/month/year, raw kW from ElectricityAvgStats typed props)
  flagged `"averagesUnverified": true` — the recon-round-4
  semantics, NOT yet live-cross-checked; the Overview panel
  renders them with the UNVERIFIED label when present. NEW
  /coi/v1/priorities: per-entity custom priorities for the 7
  hardcoded ids, probed ONLY via IsCustomPriorityVisible before
  any GetCustomPriority call (the game's own UI discipline);
  read-only, Guard-wrapped.
- **Growth-sequence test** (tools/gen_growth_fixtures.py;
  plans/fixtures/growth/month-01..12 + timeline doc): phase 0
  COMPLETE m1, phase 1 COMPLETE m3, all later gates flip in
  order on real activity (phase-3 exit TRUE m8, phase-5 entry
  m11, phase-6 entry m12); verification rules (strict order, no
  regressions, no TRUE on absent evidence, explainable checks)
  all PASS; engines at parity on spot-checks. FINDING (the
  round's most important): single-snapshot dry runs can never
  complete a phase whose exit carries a `for:` duration —
  phase 2's exit is capped at UNEVALUABLE per snapshot, and
  strict sequencing then holds phases 3–7 at NOT REACHED
  despite passing gates. The identified next engine step is
  executor history (persist evaluations — coi.db snapshots —
  and let durations span them). Also re-surfaced: phase-2 entry
  is UNEVALUABLE by plan design (placeholder research id), and
  phase 4 realistically needs ~14 months (diesel stock is the
  binding leaf).
- **Guides ×3** (website now 10): contracts-trade, loans,
  edicts — grounded in the verified route/command surfaces
  only; the loans guide flags the credit-score raw Fix32 as
  UNVERIFIED decoding; the edicts guide states the actuation
  caveat prominently. Issue pages now link them via title
  keywords (contract/quick trade → contracts-trade, loan/
  payment due → loans, edict → edicts).
- **Final exe:** dist/coi.exe 5,369,011 B (bare launch, power
  averages panel, 10 guides, guide mappings, plan v1.1.0
  verdicts baked). Constraints held: zero uploads, zero
  live-game contact, zero applies. Standing opens unchanged
  (mod install + populated-UI eyeball, sandbox protocols,
  DieselReforming decision) plus the new engine-history item
  above.

### Addendum XII — Round 6 results (2026-11-03 evening, Philip in London, PC off)

Constraints held throughout: zero uploads, zero live-game contact,
zero applies; deliveries staged locally for Philip's return Thursday.

- **History-aware executor (the headline).** GAME-PLAN-FORMAT.md
  **§4.3.1** now defines `for:`-duration semantics over a state
  sequence: CoI calendar = 12 × 30-day months (2,880 ticks/day,
  fixture-verified); a duration holds iff the inner predicate is
  TRUE at every snapshot in the trailing window AND an anchor
  snapshot at-or-before the window start exists (proving the span);
  FALSE at any windowed snapshot → FALSE; no anchor or an inner
  UNEVALUABLE → UNEVALUABLE, never false-by-default; gaps between
  snapshots never falsify. `for:{evaluations:n}` = the last n
  snapshots, same three-valued rule. Phase derivation runs per
  prefix, reproducing how a live executor's history grows.
- **Both engines implement it identically**: `tools/plan_check.py`
  and `coi plan-check` both take `--state-seq DIR` (each subdir =
  one capture state, sorted by in-game date from game-info.json).
  Single-state results are byte-identical to the Round-5 reports
  (regression on live + month-1 fixture, both plans). Parity gate
  extended: identical per-prefix phase tables, gate verdicts and
  counts (36/26/3) on the full 12-month growth sequence; the
  evaluations form and the insufficient-history rule verified on
  targeted edge fixtures in both engines.
- **Acceptance met** (`plans/growth-sequence-timeline-v2.md`; the
  v1 timeline file is untouched): fresh-start phases 0–3 COMPLETE;
  **phase 2's duration exit completes at month 5** (window reaches
  month 3's FALSE at m4, holds {m4, m5} at m5), phase 3 completes
  at month 8, phase 4 ACTIVE and blocked on exactly one leaf —
  diesel stock 220 < 300 (a fixture-horizon limit, ~month 13–14).
  Phase 5's monthsOfFood duration leaf resolves TRUE; its exit
  blocker is copper lastMonth net −6 (district's first month).
  Phases 6/7 blockers named leaf-by-leaf in the v2 timeline.
- **Fresh-start plan v1.2.0**: the phase-2 entry placeholder
  research id remains a placeholder — investigated exhaustively;
  no project source maps research ids to display names, so the
  sole candidate (`ResearchCp1Production`, research_ids.txt line
  42) is documented in-plan with reasoning instead of guessed
  (per §4.5). Validation PASS; live/fixture verdicts unchanged.
- **Diagnosis forward-compat**: `tools/diagnose_live.py` now uses
  v0.6.x capture fields when present — a full storage whose
  ImportUntil slider is ~100% is reported as full-by-configuration,
  not stuck (§3); census/storage/route priorities get §7; a
  bridge-power.json capture gets §8 with a power-deficit finding.
  Real-capture output is byte-identical to diagnosis-v1.md;
  verified additionally on `live-synthetic-v06/` (hand-enriched,
  SYNTHETIC-labelled): stuck 39 → 38 with the crude tank explained,
  a synthetic 900 kW deficit reported.
- **Sequence sources (which histories qualify)**: any directory of
  per-state capture subdirs with game-info.json dates — the growth
  fixtures qualify; the companion's live product-history series
  does NOT (per-product rollups, not full state snapshots).
- **Final exe:** dist/coi.exe 5,442,879 B (sequence-mode plan-check,
  plan v1.2.0 verdicts baked via verdicts.json refresh). Standing
  opens unchanged: mod v0.6.3 + new exe install and populated-UI
  eyeball (Thursday), sandbox protocols, DieselReforming decision.

### Addendum XIII — Round 7: per-instance machines, raw train counters, diagnosis→blueprint linkage (2026-11-04)

- **CoiBridge v0.6.4 (built, staged, supersedes v0.6.3 for
  Thursday's install):** `dist/CoiBridge-mod-0.6.4.zip` (21,767 B),
  build 0 errors / 0 warnings, ModVersion + manifest both 0.6.4.
  - NEW `GET /coi/v1/machines`: one row per machine instance
    {id, proto, pos, state, recipes, isPaused, isEnabled,
    canBePaused} — closes the Round-3 gap where census states
    could not be mapped to entity ids. Rows are built inside the
    shared ~5 s entity scan (per-row try/catch, `skippedRows`
    counter) so a bad row can never poison census/topology/
    storages. Surfaces recon-verified first (MODDING-NOTES recon
    round 5): Entity base carries IsPaused/IsEnabled/CanBePaused.
  - `/coi/v1/trains` gains RAW per-train ProductivityCounter
    blocks from TrainsManager.Trains (activity + travel: rings in
    stored order, yearlyRolling, totalRollingHalf, ongoing bucket
    + ticks) with `counterLabels` and top-level
    `"unverifiedDecoding": true`. Counts are 600-report-bucket
    report counts, NOT percentages; interpretation waits for the
    recon-round-4 (b) live cross-check against the train inspector.
    The counters block sits behind its own try/catch (`trainsError`
    fallback) so it can never take the lines payload down. On a
    trainless save the empty list is CORRECT.
- **Diagnosis consumes machines:** `tools/diagnose_live.py` §9 —
  active only when a capture carries bridge-machines.json:
  instance-vs-census consistency check, paused/disabled instance
  lists, and §5's flagged clusters resolved to instances with
  neighbours traced over the topology port edges (storages →
  product + fill; direction labelled inference — edges are
  undirected). Real capture output remains byte-identical to
  diagnosis-v1.md. Synthetic proof in `live-synthetic-v064/`
  (223 rows generated FROM the real census + topology — counts
  matched exactly for every proto; §9 reports MATCH, 46
  starved/output-full instances listed).
- **Diagnosis → blueprint linkage:** `live/diagnosis-blueprint-
  linkage.md` maps fault → evidence → layout → sizing maths →
  expected effect → what it does NOT fix for two new library
  layouts, both authored through the pack tooling, audit PASS
  (80 slugs, ISSUE COUNT 0):
  - `crude-refining-v1` (77 entities) — REPLACEMENT for the
    existing 4+4+4 distillation cluster (half OutputFull in every
    tier; its constraint was output routing, not nameplate):
    2×T1 + 2×T2 + 2×T3 towers + sour-water stripper + gas boiler;
    120 crude/min nameplate → 72 diesel + 48 naphtha + 24 fuel
    gas/min, steam from the heavy-oil fraction with the base's
    first heavy-oil buffer. Old cluster paused, not demolished.
  - `vehicle-diesel-v1` (45 entities) — 60 crude/min → 36
    diesel/min (advanced chain; basic rejected on maths), sized
    EXCLUDING DieselReforming: target = vehicle fuel 49/mo × ~2
    ≈ 98/mo → 163 crude/mo at counter scale. Assumption split is
    written in the linkage doc: Case A (reforming off) it is a
    prepared paste, NOT a supply fix (existing 84/mo already
    covers vehicles); Case B (reforming kept) existing 84 + 98 =
    182/mo closes the 161/mo demand. The two layouts are
    ALTERNATIVES sharing one crude pool (together they'd need
    643–717 crude/mo vs 480–554 produced) — Layout 1 first.
  - Honest gaps recorded in the doc: no per-minute ↔ counter/month
    conversion exists in the captures (sizing is fraction-exact +
    complete nameplate blocks); medium/heavy-oil buffer product
    ids exist in no template or live storage, so those two buffers
    are placed unassigned with manual set steps.
- **Skills:** local-coi-bridge-dev (v0.6.4 surfaces, the two
  deliberate rule-5 exceptions, unverified-decoding discipline)
  and local-coi-playing (machines route, raw counters posture,
  diagnosis §9) updated; MODDING-NOTES gained recon round 5.
- Standing opens unchanged: install + populated eyeball Thursday,
  sandbox protocols, DieselReforming decision.

### Addendum XIV — Round 8: the play loop runs; strategy layer written (2026-11-04)

- **`coi play` — supervised session runner v1 (north-star step C, BUILT
  + EXERCISED).** New subcommand in plan.rs (`play_cmd`), prepare-only
  by construction: no sockets, no client, no §12 calls anywhere in the
  runner. Walks a §4.3.1 state sequence prefix by prefix, re-derives
  phase statuses per prefix, records the ACTIVE phase's orders in plan
  order — PREPARED (catalogue command, concrete args; payload =
  {type, args}) or SKIPPED with reason (manual kinds, placeholder
  research id (§4.5), $entity selectors needing human resolution
  (§6.2)) — records completions at first TRUE exit, and halts
  "blocked" with the loop phase's exit leaves (predicate + verdict +
  verbatim engine evidence). Session schema `coi-play-session/1` +
  markdown transcript. Exercise on the growth fixtures with
  fresh-start v1.2.1: walk matches timeline v2 exactly (P0 complete
  m1, P1 m3, P2 m5, P3 m8; P4 ACTIVE → blocked on the single diesel
  leaf 220 < 300); 6 orders prepared, 15 skipped with reasons.
  Known v1 rule, deliberate: orders are recorded at a phase's first
  ACTIVE step, so a phase already COMPLETE at the first snapshot (or
  HELD → COMPLETE) contributes no orders — its gates are its record.
  Parity gate after the change: PARITY PASS (44 + 49 gate items).
- **Machines UI (website).** Entities page gains a per-instance
  machines section consuming /coi/v1/machines (v0.6.4): summary counts
  per state + filterable table (text + state select, TABLE_JS idiom),
  rendered in all three page paths so the degradation note shows even
  with the game unreachable. Verified BOTH ways: absent-route (200 +
  note) and populated via the new local stub —
  tools/stub_game_server.py serves canned captures (--capture
  live-synthetic-v064 → 223 rows asserted in HTML, 4 states;
  --capture live/ reproduces a pre-v0.6.4 bridge).
- **Blueprint assets: 9 of 20 plan refs now authored.** 7 mapped to
  existing pack slugs (iron-v1 [+iron-v2 overlay], concrete-v1 ×2,
  diesel-v1, rubber-v1, steel-v1 [+steel-v2 overlay]) after
  machine/recipe/scale verification; 2 newly authored through packlib
  (starter-assemblies-v1, 64 entities; household-goods-v1, 21) —
  pack audit PASS at 82 slugs, ISSUE COUNT 0. 11 remain
  not-yet-authored with precise blockers (terrain/deposit-bound,
  no genuine prototype, scrap-vs-ore mismatch, note-underdetermined,
  audit-contingent chains) in plans/plans-blueprint-status.md.
  audit_blueprints.py now verifies pack_slug existence for authored
  refs (9 AUTHORED / 11 MISSING-DECLARED / 0 UNDECLARED). Plans
  bumped: fresh-start 1.2.1, continuation 1.1.1; validate PASS,
  parity PASS.
- **History → states adapter (tools/history_to_states.py).** Rebuilds
  per-month state dirs from a stored snapshot's products/history
  block (36 pinned products × 8 months → history-states/
  month-01-387-07 … month-08-388-02). Month alignment PROVEN, not
  assumed: history[-1]/[-2]/[-2]+[-1] == live thisMonth/lastMonth/
  thisYear on 36/36 products; snapshot[1:] == the hist-keys probe
  exactly (its window is one month narrower). Encodings tested on
  both engines first: short flow arrays make thisYear/lastYear/
  lifetime UNEVALUABLE; stored/capacity present-but-null (omission
  blinds storage.percent_full in Python — rejected). Continuation
  plan over real history, BOTH engines, PARITY PASS (27/26/20):
  phase 0 COMPLETE at all 8 prefixes (floors held throughout),
  phase 1 HELD at all 8 — its entry's ConcreteSlab stored ≥ 2900
  leaf is underivable from product history in every month, an
  evidence-source property stated plainly in
  plans/continuation-history-states.md with the full UNEVALUABLE
  enumeration. Thursday use: a fresh live capture + this sequence
  gives the executor real history from minute one.
- **Strategy skill (Philip's ordering: runner first, strategy on
  top).** ~/workspace/custom-skills/local-coi-strategy/SKILL.md —
  the mid-loop decision layer: triage order (death-spiral triangle
  + starvation → bottleneck starvation → blockages → configured-full
  hygiene), expand-vs-fix (never scale a chain whose binding leaf
  fails; crude-cluster routing case canonical; settings→recipe→
  replacement→construction ladder), stall interpretation (WAIT /
  REDESIGN / ESCALATE — irreversibles, plan DECIDEs incl.
  DieselReforming, plan-vs-base contradictions), experimentation
  policy (guides first, protocols before trials, verified/wired/
  excluded + unverified-decoding labelling), and runner consumption
  (blockers first; decisions quote evidence verbatim). Every rule
  cites a guide, plan, diagnosis finding or sibling skill.
- Staged deliverable: dist/coi.exe REBUILT = 5,609,920 B (runner +
  machines UI inside; strings verified). Bridge unchanged this round
  (v0.6.4 stands). Still Thursday-gated: installs, populated eyeball,
  sandbox protocols, DieselReforming decision.

### Addendum XV — Productivity prerequisite: store schema v2, `coi watch`, §5 κ-free estimators (2026-11-05)

Implements QOL-MOD-PREP §5's prerequisite build (design authority
stays with the prep + qol-recon/05; this addendum records what the
code does):

- **Store schema v2** (`PRAGMA user_version = 2`; v1 files upgrade
  in place). New tables: `machine_state_proto` (per-snapshot
  per-proto machine-state counts + source), `machine_state_instance`
  (per-entity state/isPaused/isEnabled), `productivity_state`
  (estimator hysteresis). Snapshots taken before v2 have no machine
  rows and evaluate as INSUFFICIENT — absence is data, not an error.
- **Capture**: `fetch_live` fetches `/coi/v1/census` +
  `/coi/v1/machines` best-effort alongside the vanilla routes;
  snapshot JSON gains `bridgeCensus`/`bridgeMachines` keys;
  `insert_snapshot` derives the v2 rows in the same transaction
  (machines route beats census per proto; unknown states → `other`).
- **`coi watch`** (§9's polling snapshotter, now real): snapshots
  on game-tick change only; `--every 60s|5m` (SPEC's `--interval`
  spelling accepted as an alias), `--count N` to stop after N.
  Poll failures print and retry; the loop never exits on its own.
- **Estimators** (`coi/src/productivity.rs`): (a) machine working
  share, trailing 30-day window, prep-exact evidence classes
  (MEASURED / ESTIMATED / INSUFFICIENT — silence below the line,
  never "healthy"), Wilson-CI firing with second-half confirmation,
  hysteresis dead zone, one-month same-cause cooldown, cause
  separation, PARKED ≥ 50% paused; (b) self-relative finished-month
  output (FLOW-EXACT, no nameplate, no κ — κ calibration remains
  future live work). Thresholds: prep defaults, overridable in
  `coi.toml [productivity]`. Surfaces: snapshot-time issues on the
  board, `coi report` section, website Issues-page section.
- Verified: builds 0/0 (Linux + Windows), 12 unit tests, plan
  parity PASS, diagnosis reproduction byte-identical, synthetic
  fake-server demo (INSUFFICIENT at 3 snapshots → MEASURED FIRE /
  PARKED / running-hot at 31). dist/coi.exe = 5,828,998 B.

### Addendum XVI — The unified Log: one activity record for the whole toolkit (2026-11-05)

Philip's ask: "add a LOG tab which is an exact replay of things we
did in the COI exe or in the mod usage and stored that way". Built
as a persistent, chronological activity record — a replay in the
sense of re-reading history, NOT a re-execution mechanism. Nothing
on the Log page can re-apply anything.

**Event contract (one shape, two writers).** Every event is:
`recorded_ts_unix` (wall clock of the action), `source`
(`companion` | `system` | `qos`), `actor` (session username, or
the mod id), `category` (companion: `command`; system: `snapshot`;
QoS: the feed line's `kind` — `command` | `navigation`), `action`
(the shipped command type name, or `TakeSnapshot` / `NavigateToMine`),
`summary` (one human line), `game_id` / `game_date` / `game_tick`
(where the writer could know them), `outcome`, and a `payload_json`
carrying the exact detail (for applies: the full command envelope
+ HTTP status + game response body; for QoS: the feed line's data).

**Companion side — store schema v3** (`PRAGMA user_version = 3`).
New table `activity_events(event_key UNIQUE, …)` in coi.db.
`do_apply` still writes the legacy `audit` row (the Actions page
is unchanged) AND a rich activity event: exact envelope JSON,
summary, session username as actor, HTTP status + response body,
and game date/tick/gameId fetched around the apply. Website
snapshot creation (`POST /snapshots/take`) is a `system` event
with the insert outcome (`stored` / `duplicate`). Prepares that
were never applied, page views and logins are NOT events.
Pre-v3 `audit` rows are backfilled once at migration
(`event_key = legacy-audit-<id>`, source `companion`, the original
detail text verbatim in the payload); their game context was never
stored and stays NULL — it is not reconstructed.

**QoS side — the feed.** QoSCore 0.2.0 gains `QoSEventLog`: a
static, fail-safe (never throws into the sim), thread-safe,
append-only JSONL recorder at
`%APPDATA%\Captain of Industry\QoS\events-v1.jsonl`
(`Environment.SpecialFolder.ApplicationData` + fixed subpath).
Line schema v1: `{schema, eventId (GUID), recordedAtUtc (ISO-8601),
source:"qos", modId, modVersion, kind, action, summary, data{}}` —
hand-rolled JSON writer, no new dependencies. QoSCore config
`activity_log_enabled` (default true) is the master switch, wired
by QoSCoreService; recording is NOT gated by features' verbosity
flags. Instrumented actions only (after the shipped command is
actually scheduled — never diagnostics, never skipped passes):
QoSChickens `MoveAnimalsCmd` (farm, signed amount, species, mode,
predicted spread, eligible farms); QoSVehicleRecovery
`RecoverVehicleCmd` (truck, reason, streak, cargo, cost) and
`DiscardVehicleCargoCmd` (truck, product, quantity, cost);
QoSPriorityActuator `SetGeneralPriorityCmd` nudge + restore
(machine, proto, cause, current→target, proto default, band floor,
streak where held); QoSWorldMap navigations (kind `navigation`,
on a row click that actually navigates). Feature mods are 0.1.1
and require `QoSCore >= 0.2.0`.

**Game clock for QoS events — date yes, tick no.** Recon
(MODDING-NOTES round 8): `ICalendar.CurrentDate` is a public
property returning `Mafi.GameDate` (public Year/Month/Day).
QoSCoreService wires it as the recorder's date provider; the date
rides inside the line's `data.gameDate` and the importer promotes
it to the `game_date` column. ICalendar exposes no tick, so QoS
events carry no game tick — import-time context is never stamped
onto events after the fact.

**Import.** The Log page imports the feed on every load
(idempotent via `INSERT OR IGNORE` on `qos:<eventId>`), and
reports imported / skipped / malformed counts + the feed path on
the page. Path: `COI_QOS_EVENTS_PATH` override, else APPDATA.
Malformed lines are counted, never fatal; lines are capped at
64 KiB; the read is capped at 64 MiB. A missing feed is a normal
state, shown as such.

**The Log page** (`GET /log`, nav "Log"): newest-first table —
time (UTC, like the rest of the companion), source badge, actor /
mod, action, summary, outcome, game date/tick when known, and the
exact payload in an expandable details block. Filters: source
(all / companion / qos / system) + free text over action/summary;
pagination at 200 rows. The header states provenance honestly:
QoS events exist only from the updated mods onward; pre-upgrade
mod actions lived only in the game log and are NOT backfilled.

**Versions.** QoSCore 0.2.0; QoSChickens / QoSVehicleRecovery /
QoSPriorityActuator / QoSWorldMap 0.1.1; suite pack 0.5.0;
companion rebuilt (dist/coi.exe = 5,885,166 B). Verified: Rust
15/15 tests (incl. v2→v3 migration + backfill idempotency, feed
import counts), scripted scratch-serve check
(`tools/check_log_page.sh` — legacy row renders, QoS rows import,
source filter separates), all five C# projects 0 warnings /
0 errors, prior suite packs sha256-untouched. Unverified until
live install (the standing suite class): the feed file itself is
only written by the mods in-game; QoSCoreService's new ICalendar
constructor injection resolves by the same DI pattern the feature
services already use, but has not been observed live.

### Addendum XVII — The QoSBuilder job-file contract: placement by confirmed file (2026-11-05)

Recon round 10 closed the web channel for placement: the game's
JSON command index can never construct `BuildTransportCmd` or
`BatchCreateStaticEntitiesCmd` (collection arguments are blocked
by the builder), so blueprints, belts and pipes cannot be placed
through `/command` — ever, by construction. Philip approved the
alternative (2026-11-05): **QoSBuilder**, an in-game QoS mod that
executes placement *job files*. This addendum is the contract
between the companion (writer) and the mod (executor).

**The file IS the confirmed order.** The companion writes a job
file ONLY at the §12 confirm step of the Place flow (below) — the
file is to placement what the POSTed envelope is to a command.
The mod has no other trigger: no chat command, no hot folder
semantics beyond this schema, no network surface.

**Schema v1** (one JSON object, one file `<jobId>.json`):

| Field | Meaning |
|---|---|
| `jobId` | GUID (lowercase, hyphenated). The single-use key. |
| `schema` | `1`. Anything else is refused. |
| `createdUtc` / `expiresUtc` | ISO-8601 UTC (`...Z`). TTL at issue: **30 minutes**. |
| `gameId` | The gameId string from `/api/v1/game-info` at prepare time. The mod compares it to the running game's own `IGameIdProvider` value. |
| `companionUser` | The companion session user who confirmed. Attribution only. |
| `blueprintName` | Display name (attribution + event readability). |
| `blueprintString` | The blueprint string. The mod parses it with the game's own `BlueprintsLibrary.TryParseFromString` — a pure parse; the player's library is untouched. |
| `anchor` | `{x, y, z}` — the world tile the blueprint's origin lands on. |
| `rotationDeg` | 0 / 90 / 180 / 270 — quarter-turn rotation about the origin, applied before the anchor offset. |
| `applyConfiguration` | Passed to the batch command (recipes / storage products / priorities apply, as with an in-game paste). |
| `limits.maxItems` | The job's own item cap (500 from the website). |
| `sha256` | Lowercase hex SHA-256 of the canonical payload (below). |

**Canonical payload + hash.** The hash covers every field above
except `sha256` itself, serialized in the fixed field order
shown, compact (no spaces), integers bare, strings escaped by
ONE rule: `"` → `\"`, `\` → `\\`, control chars (< 0x20) →
`\u00xx` (lowercase hex), everything else raw UTF-8. Hash =
SHA-256 over the UTF-8 bytes. The mod rebuilds the canonical
form from the PARSED job (unknown extra fields are ignored;
numbers canonicalize through integer truncation — a `90.0`
re-serialization hashes as `90`), so any post-confirm edit to a
covered field fails the check while innocent reformatting of
the file cannot. Both writers and the check are pinned by a
shared test vector (companion unit test + the stub check
script's independent Python implementation).

**Claim / consume semantics (single-use, crash-safe).** The mod
scans `%APPDATA%\Captain of Industry\QoS\jobs\` at each in-game
day end, ONLY while its `enabled` config is on (it ships OFF —
while off, the folder is not even scanned and jobs simply
expire). A processed-ids record (`processed.txt`, one jobId per
line, same folder) is re-read every pass. A job whose id is
already recorded is filed to `.done` untouched. Otherwise, in
order: a validation refusal (malformed / unsupported-schema /
bad-hash / expired / wrong-game / unsupported-rotation /
reflection-unsupported / blueprint-unparseable / not-a-blueprint
/ over-item-cap — the cap is min(job cap, mod config
`max_items_per_job`, default 500) / track-rotation-unsupported /
item-transform-failed / nothing-to-place) appends the id to the
record, renames the file `.failed`, records a `JobRefused`
activity event with the reason, and takes NO other action. A
valid job is CLAIMED first (renamed `.claimed`, id appended,
`JobClaimed` event) and only then scheduled — one
`BatchCreateStaticEntitiesCmd` via `IInputScheduler`
(`DeferToProto`, validation suppression always off,
`applyConfiguration` from the job) — then renamed `.done` with a
`JobScheduled` event (item counts, skipped counts,
surfaces/decals-ignored counts, anchor, rotation). A scheduling
exception renames `.failed` + `JobScheduleFailed`. Because the
claim precedes scheduling, no crash can re-run a job.

**Placement semantics** mirror the game's own blueprint placer:
per item — copy, clear `OriginalEntityId`, skip
auto-built-by-parent protos, rotate about the anchor (anchor
added after rotation; blueprint coordinates are origin-relative),
rebuild transport trajectories from rotated pivots, offset
pillars. Buildings, belts and pipes appear as ordinary
construction sites; nothing is charged at placement. Mirroring
is refused in v1; train-track items place only at rotation 0;
surfaces and decals are not placed in v1 (counted in the event).

**Honesty rule.** The mod attests *scheduled*, never *placed*:
the batch is per-item in-game (one bad item errors, the rest
still place), so the placement verdict is the companion's
before/after census (bridge `/coi/v1/census` + topology) on the
next snapshot — the same discipline as §12's read-back rule,
one level up.

**The companion flow** (the writer side, §12 shape): GET /place
lists the embedded pack library (82 layouts; strings
decode-verified at generation time by `tools/
gen_blueprint_library.py`; the page carries the pack's
not-in-game-tested honesty line). POST /place/prepare refuses
when the game is unreachable or its gameId cannot be confirmed —
no pin, no job — and otherwise renders the confirm page: summary,
target gameId, expiry, the sha256, and the exact job JSON.
POST /place/confirm (120 s single-use token, as §12) writes the
file — tmp-then-rename into the jobs folder — and records audit
+ activity rows (action `PlaceBlueprintJob`, outcome
`job-written` / `write-failed`, payload = the exact job JSON).
There is NO other write path. Jobs folder resolution:
`[builder] jobs_dir` in coi.toml (search order as
[productivity]), default `%APPDATA%\Captain of Industry\QoS\
jobs` — zero-configuration on the game PC, where companion and
mod share the machine.

**Versions.** QoSBuilder 0.1.0 (standalone zip — suite-pack
integration after live verification); QoSCore ≥ 0.2.0 required;
companion rebuilt (dist/coi.exe = 6,062,808 B). Verified: C#
0 warnings / 0 errors; Rust 17/17 tests incl. the canonical
vector; `tools/check_place_page.sh` PASS against the stub game
(prepare pins, confirm writes a hash-verified file, replay
refused, game-down prepare refuses). Unverified until live: DI
resolution of the two game surfaces in the mod's constructor,
the first real job end-to-end, and save+reload with the mod's
serializer pair (the round-7 pattern, applied from birth).

---

## Addendum XVIII — Design system v1 implemented (2026-11-05, evening)

The companion website is restyled end-to-end on the design system in
DESIGN.md. Philip delegated the open design decisions ("make and do
the last decisions please"); the defaults adopted and built are:
**dark theme only, comfortable density only, blue accent #5b9dff**.
Presentation only — every route, form field, CSRF/token mechanic, the
§12 prepare → confirm → apply flow, and the audit/activity writes are
behaviourally unchanged.

**As built.** One embedded token stylesheet (five-layer surface
stair, border-first elevation, system fonts); the app shell is a
232 px sidebar with the four nav groups (Monitor / Game / Library /
System) and an always-visible game status strip (date, PAUSED/RUNNING
badge, speed, tick, population; OFFLINE variant when the game does not
answer, fetched per render — there is no fabricated "last known"
state). All 13 pages + login + the generic Prepare form + the /place
flow are restated on the class contracts (.btn tiers, token .badge
set with UNVERIFIED / UNEVALUABLE dashed variants, .table/.num,
.callout, .filterbar, .field, stat tiles, cards, .detail grid,
.feed for the Log). The §12 confirm sheet follows DESIGN §3.9
exactly: warning band ("This will change your game."; danger band
"This cannot be undone." for irreversible command types), 17 px
summary, the state line unchanged, caption "Exact command sent to
the game" over the exact JSON, Run + Cancel as full-size peers,
expiry caption. Charts are server-generated inline SVG per DESIGN
§4: product sparklines (Products, Product detail, Snapshots trends),
TM/LM flow pairs (Products Net LM), forecast runway bars (basis
string verbatim, 12-month cap, quarter tick), machine-state stacked
bars (Overview, Issues productivity), power balance bars with the
averages as UNVERIFIED mini-bars, storage fill bars, research
progress fillbars. Degraded states render in-shell: D1 callout +
checks on game-unreachable pages (HTTP behaviour unchanged — the
pages that returned 200 still do, game_unreachable keeps its 502),
D2 placeholder cards where a bridge section is absent, D3 chips on
partial data.

**Deviations from DESIGN.md as written** (each deliberate, recorded
here): Place sits in the nav's Game group (the page map predates
/place); nav count pills are omitted (nothing is computed in the
shell to source them); the productivity section keeps its verdict
lines as text under the new per-proto stacked bars (the eval structs
are private to productivity.rs — bars come from the store's
machine-state aggregates instead); Issue-detail's Act-on-it tables
render full-width below the detail grid (the per-entity tables do
not fit the 320 px rail); Actions' bridge-absent panel keeps its
richer copy rather than collapsing to a D2 one-liner; runway bars
have no seasonal hatching (Forecast carries no seasonal flag —
seasonality lives in the basis strings, printed verbatim, and in
the seasonal captions); Product sources keep both periods stacked
instead of a .seg switch; the Machines table's state cell shows the
token word (Starved/Blocked) — text search matches the token words,
the state dropdown still filters on the raw state via data-state.

**Verification.** cargo build clean (0 warnings), cargo test 17/17,
tools/check_place_page.sh PASS, tools/check_log_page.sh PASS
(its verbatim import sentence preserved). Full page sweep against
the stub game (live-synthetic-v064 capture + the v06 power capture):
all routes 200 and populated; degraded sweep (stub down) renders
in-shell with the OFFLINE strip. The stub gained a /coi/v1/power
route so the power panel is sweep-verifiable. Screenshots:
design-mock/after-overview.png, after-issues.png, after-confirm.png
(inspected). dist/coi.exe restaged: 6,160,589 B, sha256
9976089fc6d45bd185e2afce759f0d1758752c9fb759e8e976b94c66677c2072.
Not covered by the stub: product-history sparklines against real
history (the stub serves empty series by design) — the code path is
the same one the Snapshots trends render through, which is covered.

## Addendum XIX — Terrain sensing: heights, deposits, designations (CoiBridge v0.6.5, companion Terrain page) — 2026-11-06

Philip's layout question ("how well do we understand layout?")
exposed the blind spot: built things were exact, the land itself
was unsensed. Recon round 13 (`qol-recon/11-terrain-deposits.md`,
bridge MODDING-NOTES round 13) closed the knowledge gap; this
addendum is the built contract.

### The coordinate guarantee

Terrain tiles and entity positions are ONE grid. Terrain height
lives per-tile on `TerrainManager` (`index = y * width + x`);
`TerrainTile.WithHeightFloored` builds its Tile3i from
`TileCoordSlim.ExtendHeight(Height.TilesHeightFloored)` — an
entity's Z IS the floored terrain height at its tile, in the same
units. No transform, no offset, ever. Heights are Fix32
(`heightRawPerTile = 65536` per tile); a tile is 2 m × 2 m and
`TerrainTile.TILE_VOLUME_M = 8` m3 — so one tile-cubed of
earthworks = 8 m3 exactly.

### New bridge routes (read-only, own DI class, own failure domain)

- `GET /coi/v1/terrain` — map meta (width/height/tilesCount,
  playable limits, unit semantics) + full-grid min/max + ocean
  count + a point-sampled summary grid (stride chosen so the grid
  is at most 96x96). Default payload stays ~2-4 MB even at
  640x640.
- `GET /coi/v1/terrain/region?x0&y0&x1&y1` — exact raw heights for
  a rectangle (x1/y1 exclusive, -1 = map edge, clamped to the
  map). Area capped at 16,384 tiles; oversized rectangles shrink
  from the requested origin and carry `"truncated": true`.
  Empty-after-clamp is a 400 JSON error.
- `GET /coi/v1/deposits` — on-map resources ARE terrain material
  layers (no deposit entities exist). Per-material totals
  (tiles, summed layer thickness as `volumeTilesCubedRaw`,
  remaining `quantity`/`quantityRaw` via the game's own
  `ToPartialProductQuantity` conversion — depletion-exact, since
  mining removes layer thickness) + connected deposit regions
  (4-connectivity components with bounds; bedrock and
  product-less materials excluded; list capped at 1024 with
  `"truncated"`). The full-grid scan is cached 30 s in the mod.
- `GET /coi/v1/designations` — mining/dumping/other partition via
  the shipped designation managers (HashSet membership):
  area, size, target-height band, fulfilled, assigned jobs.

### Companion side

- `fetch_live` pulls terrain summary + deposits BEST-EFFORT into
  `Live.terrain` / `Live.deposits` (Null on an older bridge;
  absence never fails the vanilla fetch). Snapshots carry them as
  `bridgeTerrain` / `bridgeDeposits`; store schema v4 derives
  `deposit_state` rows (per snapshot + material: tiles,
  volume_raw, quantity_raw) — deposit history is queryable;
  heights are NOT stored per snapshot (bulky, re-fetchable).
- **Terrain page** (`/terrain`, Monitor group): stat tiles (map,
  height range, ocean), whole-map summary heatmap (server-side
  SVG, range printed as text beside it), a region form (exact
  heights fetched ON DEMAND; capped rectangles say so), the
  deposits tables (totals + regions, product names resolved),
  and the designations table (kind/state always worded badges).
- **Earthworks estimator** (`coi/src/terrain.rs`, pure + unit
  tested): rectangle + target height -> cut (sum of heights above
  target) and fill (sum below), tiles-cubed and m3 (x8), with
  above/below/at tile counts. MATERIAL-AGNOSTIC by design: it
  prices geometry, and the deposits table says what the cut is
  made of. Exposed three ways, one implementation: the Terrain
  page form, the CLI (`coi terrain deposits|region|estimate`),
  and the documented JSON shape the estimator emits
  (`{targetTiles, tiles, tilesAbove, tilesBelow, tilesAt,
  cutTilesCubed, fillTilesCubed, cutCubicMeters,
  fillCubicMeters}`) for the playing layer.
- CLI: `coi terrain deposits`, `coi terrain region X0 Y0 X1 Y1`,
  `coi terrain estimate X0 Y0 X1 Y1 TARGET` (prints the estimator
  JSON plus the region descriptor).

### Still NOT sensed after this round

- An excavator's CURRENT digging target / in-progress terraforming
  job internals (designations expose areas + assigned-job counts,
  not per-job state).
- Nothing else from the 2026-11-06 answer remains open: heights,
  earthworks and on-map ore are all sensed as of v0.6.5.

Verification: bridge builds 0/0; companion 22/22 cargo tests
(5 new terrain tests: flat-at-target, flat fill, ramp split,
single peak, raw->tiles parse); Terrain page rendered against a
synthetic v0.6.5 stub capture (summary, region, estimator —
numbers cross-checked against an independent computation) and
degrades to the standard bridge note at HTTP 200 against the
v0.6.4 capture; check_log_page.sh + check_place_page.sh still
PASS. LIVE verification is outstanding by design (game PC off):
the four routes against a real save join the weekend checklist
with the suite 0.5.1 save+reload proof and the first QoSBuilder
job.

Packaging for this round: dist/CoiBridge-mod-0.6.5.zip 27,032 B,
sha256 3f26d817943e69244fe083bc176db848f76cb76ea0f8687b2a369ec96c238cf5; dist/coi.exe restaged 6,215,930 B, sha256
e71ff3de9225f81cfa51c4b276812aae633a470ec01f04bd4a526a3db911d7e5
(supersedes the 6,160,589 B redesign build).

## Addendum XX — Designation jobs: pads and removals by confirmed file (QoSBuilder 0.2.0) — 2026-11-06

The designation sibling of Addendum XVII, built in four stages
(`qos/DESIGNATION-DESIGN-NOTE.md`): the same sealed job-file
channel now carries terrain designations, which the web §12
channel can never reach (recon round 14 — the commands'
`ImmutableArray` arguments are JSON-blocked by construction;
the mod's `IInputScheduler` route is the only executor).
`QuickFulfillDesignationsCmd` remains excluded by posture.

**Schema v2** (designation jobs only; blueprints stay v1), one
file `<jobId>.json` in the same jobs folder: `jobId`, `schema`
2, `jobType` (`"designations-add"` | `"designations-remove"`),
`createdUtc` / `expiresUtc` (30-min TTL), `gameId` pin,
`companionUser`, `planName`, `protoId` (**add only** — one of
`MiningDesignator` / `DumpingDesignator` / `LevelDesignator`;
the kind is a property of the batch, §1.2 of the note),
`cells` (**add only**: `{x, y, origin, plusX, plusXY, plusY}`
in `DesignationData` ctor order — PlusXY before PlusY; the
centre height is game-derived and never carried) or `origins`
(**remove only**: `[{x, y}]`), `limits.maxCells` (200 from the
website), `sha256` over the canonical payload (field order and
the ONE escaping rule as v1). The canonical vectors are pinned
in `qos/BUILD-NOTES.md` round 9 and were executed on all three
legs (C# probe harness, Rust, Python); the companion's page
check re-verifies every written file with an independent
Python implementation of the v2 canonical form.

**Companion flow** — routes `GET /designate`,
`POST /designate/prepare`, `POST /designate/confirm` (nav:
Game group, beside Place):

- **Pad prepare** pins the gameId exactly as /place (502
  refusal when the game is unreachable or unidentifiable),
  fetches `/coi/v1/terrain/region` for the rectangle plus a
  one-cell margin, and runs `designate::plan_pad`: the
  rectangle snaps OUTWARD to whole 4×4 cells, every cell
  levelled to the target. The confirm sheet shows the cell
  count, cut/fill volumes (`terrain::estimate` arithmetic),
  **every edge-step warning verbatim** (|target − terrain
  just outside| > 1 at a boundary vertex — the game accepts
  such cells silently, so snapping is planner discipline,
  surfaced at confirm), and **every replaced designation**
  (Add replaces any kind at an origin and deletes a surface
  designation there). The sheet also states the blocked-cell
  expectation: cells over blocking buildings are silently
  skipped in-game.
- **Remove prepare** takes origins ONLY from the live
  `/designations` read-back (checkboxes rendered from it;
  there is no free-typed origin path), and refuses a
  selection that no longer matches the read-back ("Stale
  selection").
- **Confirm** reuses the `PreparedJob` token store (120 s,
  single-use) and writes exactly ONE file tmp-then-rename
  into `builder_jobs_dir`, with audit + activity rows
  (`PlaceDesignationJob`, category `designation`) under the
  remediation round's fail-loud pattern — an audit failure
  is surfaced on the outcome page, never swallowed.
- **Verdict**: the mod attests *scheduled* only — its Add
  result cannot distinguish placed from silently refused
  cells (note §1.3). The companion diffs the job against the
  CURRENT `/designations` on the confirm outcome page and on
  every /designate visit (latest-job panel, reconstructed
  from the activity payload): exact per-cell corner equality
  with bridge 0.6.6 (Addendum XXI); band-level, labelled
  partial, on older bridges. Statuses: LANDED / PARTIALLY
  LANDED / SCHEDULED — verdict pending. The page never claims
  placement from the write alone.

**Executor** — QoSBuilder 0.2.0 (stage 2): family dispatch
after the shared v1 refusal ladder, then the designation
rungs (`unsupported-proto` → `malformed-cells` →
`duplicate-cells` → `over-cell-cap` →
`nothing-to-designate`); ONE `AddTerrainDesignationsCmd` /
`RemoveDesignationsCmd` per job; QoSEventLog kind
`"designation"`. No new fields on `BuilderService` — the
hand-written serializer pair and the mod's save surface are
untouched.

**Not in v1**: pit *generation* from live data (the bridge
serves no per-column layer stacks — the planner core in
`coi/src/designate.rs` is fixture-fed and ready; flagged
decision 2 in the note), forestry designations, mixed-kind
single confirms (one job file per kind).

**Synthetic capture**: `live-synthetic-v065/
bridge-designations.json` regenerated to the 0.6.6 route
shape (per-cell rows, proto `"MiningDesignator"`, corners +
centerRaw); the 0.6.5-era file carried merged areas and
display-name proto strings, and predated Addendum XXI.

Verification: cargo build 0 warnings; cargo test 41/41 (25
pre-existing + 13 designate stage-3 + 3 read-back verdict);
`tools/check_designate_page.sh` PASS (prepare pins gameId,
warnings render, confirm writes exactly one file whose hash
re-verifies in Python, replay refused, remove flow verified,
latest-job panel renders, bridge-down prepare 502,
/designate degrades 200-with-notice);
`check_place_page.sh`, `check_log_page.sh` and
`check_audit_fail_loud.sh` still PASS. Live proof is the
weekend shakedown (design note §6) — deliberately not part
of code completion.

Packaging for this round: dist/coi.exe = 6,345,284 B,
sha256 33f063cea7ee5e6b485e42bc3aa0e52fbee0e8a992d2c9350c
259e171306dd1d (Windows release build, 0 warnings);
QoSBuilder-mod-0.2.0.zip = 47,373 B, sha256
680d81eac4e68afa5d8e7a1354f5548891b63e56b33dffe07c4a74aa03005131
(stage 2). The weekend bundle carries both plus CoiBridge
0.6.6 (Addendum XXI).

## Addendum XXI — Designation read-back made exact: corners + centre (CoiBridge v0.6.6) — 2026-11-06

Micro-fix round, approved by Philip 2026-11-06 ("yes fix more"),
closing flagged decision 1 of the designation design note
(`qos/DESIGNATION-DESIGN-NOTE.md` §1.4/§4). ONE route changes;
additive only — every existing field keeps its name and shape.

- `GET /coi/v1/designations` rows now also carry:
  - `corners`: the designation's four corner target heights as
    integer tiles, in `DesignationData` ctor order —
    `[origin, plusX, plusXY, plusY]` (PlusXY before PlusY, the
    standing ctor-order trap; the route reads them from
    `TerrainDesignation.Data`, which the runtime object retains
    in full — verified in the shipped `Mafi.Core.dll`).
  - `centerRaw`: the centre target height as raw Fix32
    (`HeightTilesF.Value.RawValue`; 65536 = 1 tile).
- Why: with only the min/max band (≤0.6.5), a read-back verdict
  was exact for flat cells but merely band-level for sloped
  cells — two different corner assignments can share a band.
  With corners + centre served, the companion can verify
  `DesignationData` equality per origin for sloped cells too,
  which is the verdict the QoSBuilder designation jobs
  (stages 2–4) rely on.
- Design-note open question 4, settled from the code path
  (labelled code-derived until a live capture proves it): the
  route appends `d.ProtoId.Value`, and `TerrainDesignation
  .ProtoId => Prototype.Id`; `TerrainDesignationsData
  .RegisterData` registers the protos under `IdsCore
  .TerrainDesignators.*`, whose string values are
  `"MiningDesignator"`, `"DumpingDesignator"`,
  `"LevelDesignator"`, `"ForestryDesignator"`. So `proto`
  emits e.g. `"MiningDesignator"` — NOT the
  `"MiningDesignation"` of the v065 synthetic capture (that
  string resembles the protos' *display* names, "Mining
  designation"). `kind` is unchanged: partition-derived
  (`"mining"` / `"dumping"` / `"other"`).
- **The v065 synthetic designations capture predates this
  change** (and already disagreed with the route on areas and
  the proto string): stage 4 must regenerate
  `live-synthetic-v065/bridge-designations.json` against the
  0.6.6 route shape before its page checks mean anything.

Verification: bridge builds 0 warnings / 0 errors; the compiled
DLL carries the new field strings (the 0.6.5 DLL does not). LIVE
verification is outstanding by design (game PC off): the first
real /designations capture joins the weekend checklist.

Packaging for this round: dist/CoiBridge-mod-0.6.6.zip
27,399 B, sha256
806b7a9e33559ce15e8f53728b33a73a98a69e184af7b1d3094aac92ebcffe10.
The 0.6.5 zip is untouched (27,032 B, sha256
3f26d817943e69244fe083bc176db848f76cb76ea0f8687b2a369ec96c238cf5,
re-verified after this build).
