# AGENTS.md — the agent operator guide for this toolkit

You are an LLM agent working with the Captains of Industry
toolkit. This file is your entry point and your operating
contract: read it first, before any other document in this repo.
It tells you what the components are, how to reach a running
game, the loop you work in, and the rails you never cross.
Depth lives in the documents it points at — this file is the
map, not the territory.

The game this toolkit serves is **Captains of Industry, Update
4.3 (v0.8.8)**. Mechanics, ids and ratios from other versions
may be wrong here.

## What this repo is

| Component | What it is | Canonical depth |
| --- | --- | --- |
| Superintendent (`coi/`) | A local website served by one Rust binary (`superintendent`), running next to the game. Dashboards, an issues board with stated reasoning, forecasts, a command catalogue, game plans, and the only sanctioned write path into the game. No login; binds loopback. | [coi/README.md](coi/README.md), [dev/SPEC.md](dev/SPEC.md) |
| CoiBridge (`bridge/`) | A read-only C# mod that exposes extra game state (population, research, census, storages, topology, machines, power, terrain, designations) as `/coi/v1/*` HTTP routes on the game's own web server. It never writes to the game. | [bridge/README.md](bridge/README.md) |
| QoS suite (`qos/`) | Small quality-of-life mods on a shared QoSCore foundation. All ship disabled by default; the player enables each feature deliberately. | [qos/README.md](qos/README.md), `qos/SUITE-README.txt` |
| QoSBuilder (`qos/QoSBuilder/`) | The one mod that creates things: it executes sealed job files (blueprint placement, terrain designations) that the companion prepared and a human confirmed. | [dev/SPEC.md](dev/SPEC.md) Addenda XVII and XX |
| Plans (`plans/`) | Machine-executable game plans: phases, typed orders, exit gates. The bundled reference is `fresh-start-v1`. | [docs/GAME-PLAN-FORMAT.md](docs/GAME-PLAN-FORMAT.md), [plans/README.md](plans/README.md) |
| The canon (`docs/canon/`) | The single source of truth for every threshold and rule the guidance layer uses — and the tactics layer you play by. | [docs/canon/README.md](docs/canon/README.md) |

Player-facing entry points are [README.md](README.md),
[INSTALL.md](INSTALL.md) and [REMOTE-ACCESS.md](REMOTE-ACCESS.md).
Agent-facing material is indexed by
[.llms/llms.txt](.llms/llms.txt); this guide is its first entry.

## The runtime stack

Four layers, in the order data flows:

1. **The game** serves a built-in web API (below) and hosts the
   mods.
2. **CoiBridge** adds read-only routes to that same server.
3. **Superintendent** reads both, keeps snapshots and history in
   its local database, reasons over them (issues, forecasts,
   productivity), and is the surface through which every write
   is prepared, confirmed and audited.
4. **You** work through Superintendent's surfaces — its web
   pages, its plan runner, and its CLI debug shell — never
   around them.

## Connecting to a running game

- The player loads a save and runs `web_api_start` in the
  in-game console. The game then serves its API on loopback,
  port 3073. It must be re-run after **every** game start; if
  the API does not answer, that is the first thing to check.
- The game rejects any request whose `Host` header is not
  `127.0.0.1:3073` or `localhost:3073` (403). This is deliberate
  origin protection. Never suggest enabling the game's
  `allowOtherOriginsPost` option — it is unnecessary and unsafe.
- Vanilla routes you will use most: `GET /api/v1/game-info`
  (date, tick, paused, speed, `gameId`), `GET /api/v1/products`,
  `/api/v1/products/stats` (flow windows: this month, last
  month, this year, last year, lifetime), `/api/v1/products/history`,
  `GET /commands` (the command catalogue), `POST /command`.
  Product ids are `Product_`-prefixed everywhere; display names
  are not ids.
- Bridge routes live under `/coi/v1/` on the same server:
  `hello`, `status`, `research`, `census`, `topology`,
  `storages`, `machines`, `power`, `priorities`, `trains`,
  `contracts`, `loans`, `edicts`, `quick-trades`, plus the
  terrain family (`terrain`, `deposits`, `designations`). If a
  bridge route 404s, the mod is absent or older — the companion
  degrades and says so; treat the data as missing, never as
  zero. Full table: [bridge/README.md](bridge/README.md).
- **Superintendent is your primary surface.** Started bare (or
  with `serve`) it serves the site at `http://127.0.0.1:8484`.
  There is no login to perform or bypass: per-form tokens and a
  Host check are the plumbing, and they apply to your requests
  too. The CLI subcommands (`status`, `snapshot`, `products`,
  `forecast`, `report`, `play`, `watch`, `saveinfo`, and the
  rest) are a debug shell over the same engine — same data,
  same rails.
- Topology: by default everything is on one machine. If the
  companion runs elsewhere, it reaches the game only over a
  private tunnel the player controls, configured three ways
  (precedence: `coi.toml` < environment < CLI):
  `[game]` section in `coi.toml`, `COI_GAME_URL` /
  `COI_HOST_HEADER` / `COI_PROXY`, or `--game-url` /
  `--host-header` / `--proxy`. Setup and the security rules are
  in [REMOTE-ACCESS.md](REMOTE-ACCESS.md). Never expose the
  game API or the companion site publicly.

## Trust order

When sources disagree, believe them in this order:

1. **Live game state** — what the API and bridge return now.
2. **The canon** — [docs/canon/](docs/canon/README.md), for what
   the live numbers mean and what to do about them.
3. **The guides** — [docs/](docs/README.md) and the site's
   Guidance pages, for worked numbers and procedures.
4. **Your training memory** — last, and suspect. This game
   changes between versions; if memory and the canon disagree,
   the canon wins and memory loses.

Two id traps that follow from this: never invent an id that
merely parses. Research node ids come from the game's data
(`coi/data/`, the research catalogue) — goal ids look similar
and are **not** research nodes. Storage and machine targets are
addressed by selector or by the ids a live read just returned,
never from memory.

## The operating loop

Work in this loop, in this order:

1. **Observe.** Take a snapshot; read game-info, products,
   census, storages. Note the `gameId` and the in-game date —
   every later claim cites the state it came from.
2. **Diagnose.** Read the Issues board and the Why pages
   (`/why`). Every finding names the canon entry that fired,
   the numbers behind it, and the fix it suggests.
3. **Plan.** Decide the next moves from the canon and, when a
   plan is active, from the plan's current phase and its gates.
   State what you intend and what evidence will prove it worked
   **before** you act.
4. **Act** — only through the rails in the next section.
5. **Verify.** Re-read the live state and compare it against
   what the action claimed. A command's success response is
   **not** evidence: the game's own placement commands report
   success for cells they silently refused, which is why the
   designation flow renders a verdict (LANDED / PARTIALLY
   LANDED / SCHEDULED) from a fresh read-back. Apply that lesson
   everywhere: no read-back, no claim.
6. **Record.** Everything you do through the sanctioned flows
   lands in the audit trail and the Log. If an action is not in
   the Log afterwards, treat it as not done.

## Hard rails

These are not defaults. They are the contract.

1. **Never edit save files.** Not to fix, not to inspect-write,
   not ever. Reading a save offline (`saveinfo`) is fine.
2. **Game commands run only through the §12 flow**:
   prepare → confirm → audit ([dev/SPEC.md](dev/SPEC.md) §12).
   Prepare validates and prices the command and returns a plan
   with a confirm token (120 seconds, single-use). Apply
   executes exactly what was prepared and writes an audit row.
   If the audit write fails, the action is reported as
   applied-but-unaudited — never as clean success.
3. **No ad-hoc `POST /command`.** The endpoint existing does not
   make it a surface. The catalogue marks one command
   (`GameConsoleCmd`) excluded by policy; it stays excluded.
4. **Placement and terrain designations happen only via sealed
   QoSBuilder job files** — prepared by the companion, confirmed
   by a human, single-use, expiring, pinned to one `gameId`,
   hash-sealed. Belts, pipes and blueprints cannot go through
   `/command` at all (the server blocks collection arguments);
   the job file is the only route.
5. **Plans run through the session runner**
   (`superintendent play <plan>`), which is prepare-only by
   construction: it prepares orders in sequence and has no
   apply path. A plan's autonomy setting decides *who confirms*,
   never *whether* the §12 flow runs.
6. **Mod behaviour is enabled per feature, by the player, on a
   scratch save first.** Every QoS feature ships off. A new or
   changed behaviour earns a scratch-save shakedown (place,
   verify, save, reload) before it touches a real save. Never
   keep an old mod folder alongside its replacement.
7. **Stop conditions.** Stop and report — do not improvise —
   when: the game is unreachable; the live `gameId` differs from
   the one a plan or job was prepared against; a confirm token
   has expired (re-prepare; never force or reuse one); a bridge
   route a decision depends on is missing; or a read-back
   contradicts the action's claim.
8. **No network surprises.** The toolkit makes exactly one kind
   of outbound connection: to the configured game endpoint. No
   telemetry, no update checks, no hosted assets. Do not add
   any.

## The canon is your tactics layer

The canon ([docs/canon/](docs/canon/README.md)) is one
structured source — YAML families covering the issues board,
forecasts, productivity, priorities, earthworks, plans and
play philosophy — rendered three ways: the site's Why pages
for players, `docs/canon/canon.json` for the server, and
[.llms/canon-for-agents.md](.llms/canon-for-agents.md) for you.

- Every entry has a stable `id`, a player-facing explanation, a
  precise `rule`, its numbers, and a `status`: **verified**
  (read back from the game), **wired** (in the engine, drift-
  checked), **heuristic** (judgement, labelled as such), or
  **deliberate-none** (a recorded decision not to adopt a rule).
  Know which kind you are standing on before you act on it.
- The board's severity vocabulary — **Fix now**, **Fix later**,
  **Watch**, **Ignore for now** — is the only player-facing
  severity language in this product. Use exactly those words
  with players.
- The provenance rule: no canon entry or guide presents a
  specific run's factory as the reader's factory. Real numbers
  appear only as framed worked examples. Keep it that way in
  anything you write.
- Executable thresholds in the Rust code cite the canon entry
  they implement (`// canon: <id>` comments), and
  `tools/check_canon.py` fails when a wired number drifts from
  the canon. Your moves should be auditable the same way: be
  able to name the canon entry or plan gate behind every
  recommendation and every action.

## Plans

Plans are data, not code: YAML documents in the format of
[docs/GAME-PLAN-FORMAT.md](docs/GAME-PLAN-FORMAT.md) — phases
of typed orders (`command`, `place-blueprint`, `manual-step`,
`observe-only`) with exit gates.

- A gate must hold **over history**, not at a single instant,
  before the runner advances. A gate that fails means the plan
  halts there and says why — that is a finding about the run or
  the plan, not an obstacle to route around.
- The bundled reference plan is
  [plans/fresh-start-v1.yaml](plans/fresh-start-v1.yaml) (its
  `.json` twin is generated — never hand-edit it). Validate any
  plan before use: `python3 tools/validate_plan.py <plan>`.
- A plan describes one run: one map, one difficulty, one game
  version. It contains no geometry — placement lives in
  blueprints and job files, referenced by name.
- Anything a gate would need that the state model cannot supply
  is a known gap, recorded as such — never a value to guess.

## The player's preferences are the player's

This guide is deliberately generic: it ships to every player
whose agent opens this repo. A specific player's preferences —
their map, difficulty, layout taste, how much autonomy they
grant you, what "confirm each step" means for them — live in
**that player's** local layer: their active plan, their
`coi.toml`, and whatever local notes they keep beside the game.

Read those before you play for someone. Never assume them,
never import another player's, and never write a player's
preferences, save details or network setup into this repo's
shared documents — they would ship to everyone.

## Working in this repo

If you are changing the toolkit itself, not just playing
through it:

- Build and test: `cargo build --release` and `cargo test` in
  `coi/`; the mods build with `bridge/vm-build/build.sh` and
  `qos/build/build.sh` against a local `COI_REFS_DIR` of game
  assemblies (never committed).
- The definition of done is the standard check sequence in
  [tools/README.md](tools/README.md) — unit tests, the canon
  drift gate, plan parity, the serializer-pair gate, the
  structure linter, the mod-package linter, and the page checks
  against the stub game server — plus the lint stack
  (`cargo fmt`, `cargo clippy -- -D warnings`, `ruff`,
  `shellcheck`, `markdownlint`, `yamllint`; configs at the
  root, wired by `.pre-commit-config.yaml`).
- Layout rules are enforced by
  `tools/check_repo_structure.py`: a lean root (only the files
  and folders it allowlists — this file and `CLAUDE.md` are on
  it), a README at every component's top level, `manifest.json`
  and `readme.txt` at every mod root, and every relative markdown
  link resolving. New top-level files need the allowlist
  updated in the same change.
- Generated files are never hand-edited: the command catalogue
  (`tools/gen_catalogue.py`), plan JSON twins
  (`tools/plan_to_json.py`), and the canon renderings
  (`tools/canon_to_json.py`). Edit the source, regenerate,
  commit both.
- If you change behaviour, update the canon entry it implements
  and add a SPEC addendum in [dev/SPEC.md](dev/SPEC.md) in the
  same change. Behaviour that exists only in code is a bug in
  this repo's terms.
- Unproven capabilities are tested only under
  [.llms/SANDBOX-PROTOCOLS.md](.llms/SANDBOX-PROTOCOLS.md):
  throwaway saves, game-id guards, verdicts recorded. Distilled
  behaviour policies for play live in
  [.llms/BEHAVIOURS.md](.llms/BEHAVIOURS.md).

## First session checklist

1. Confirm the game answers: `game-info` returns a `gameId`,
   and the bridge `hello` route answers if the mods are
   expected. If not, the fix is on the player's side (game not
   started, `web_api_start` not run, mods not installed) — say
   which, plainly.
2. Confirm Superintendent is running and take a snapshot.
3. Read the Issues board and the Why page behind anything you
   plan to raise.
4. Read the player's local layer: active plan, `coi.toml`, any
   local notes. Note the autonomy posture they have set.
5. State your plan for the session — moves, gates, what proof
   you will collect — before the first action.
6. Work the loop: observe → diagnose → plan → act via the
   rails → verify by read-back → record.
7. Close by reporting what changed, what the read-backs showed,
   and what is still open. Numbers with provenance, gaps named
   as gaps.
