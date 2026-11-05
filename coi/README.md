# coi — Captain of Industry companion

**The product is the web app**: a website you log into, served by this
single Rust binary running on the game PC next to the game. It reads
the game's web API (Update 4.3 / v0.8.8+), identifies issues, explains
how to fix them, stores snapshots/trends in SQLite, and can run game
commands for you through a prepare → confirm → audit flow. The CLI
subcommands are a debug shell around the same engine; `saveinfo` is an
offline `.save` inspector. Built per `../SPEC.md`; live access notes
in `../ACCESS.md`.

## Run it (Windows)

1. Copy `coi.exe` anywhere (single file, ~4.9 MB, no installer, no
   runtime dependencies — SQLite and everything else is compiled in).
2. In the game: load a save, open the console, run `web_api_start`.
3. Double-click `coi.exe` — it starts the companion website and opens
   http://127.0.0.1:8484/ in your browser, at the login page. Keep the
   window it opens in open while you use it (Ctrl-C or closing it
   stops the website). Terminal alternative: run `coi.exe serve`
   yourself and open the same address by hand.
4. First visit asks you to set the account password (stored salted +
   hashed in `coi.db` next to the exe).

If the companion is already running, double-clicking `coi.exe` again
does not start a second copy: it says the companion appears to be
already running, opens the browser at the login page, and exits.

Pages: Overview (status, pause/speed controls, electric-power panel
from CoiBridge v0.6.2 — generated/consumed/balance/capacity in kW,
triage summary,
Outlook forecasts, live screenshot) · Issues (triage board; every
issue opens a detail page with the quadrant reasoning, the numbers,
history, the fix — and buttons that act on the exact storages and
machines involved) · Guides (failure-mode library: transport,
trains, priorities, storage, power, farming, oil & diesel — each
guide lists the currently open issues it speaks to) · Products
(filterable/sortable; per-product sources, history, trends, pin)
· Entities (the CoiBridge directory: settlement + research status,
all storages with logistics controls and product assignment —
v0.6.1 rows also show general/import/export priorities and the
logistics slider steps — a Non-default priorities panel from the
census, machine instances with pause/recipe/priority controls, and
the not-fully-connected transports list) · Research (250-node
catalogue + live bridge state: current node, progress, queue, and
the completed-research list from CoiBridge v0.6.2) · Actions
(command centre, global priorities + audit log) · Catalogue (all
240 game commands classified, with argument schemas, a Prepare
button per command, and a verified / wired / excluded status) ·
Snapshots (take + browse) · Trade (CoiBridge v0.6: contracts with
establish/cancel actions, quick trades with a Prepare button per
live trade (v0.6.1), loan terms + active loans with overpay and
borrow forms, the edicts table with the game's own enable/active
reasons — each panel degrades to an "update the mod" note when its
bridge route is missing).

Live commands (prepare → confirm → audit): pause/speed, research
start/stop/queue, product pin/unpin, global priorities — plus, with
the CoiBridge mod installed, the entity-targeted set: per-machine
pause and recipe toggles, per-machine general priority, storage
product assignment and storage logistics toggles. The bridge census
is the entity directory the vanilla API lacks (SPEC §3/§12.7).

**Generic prepare (2026-11-02 evening): every command in the
catalogue is executable, not just the curated set.** Each Catalogue
row has a Prepare button opening a form generated from that
command's captured argument schema (GET /prepare?cmd=… →
POST /actions/prepare-generic); values are type-coerced and
validated against the schema, and the flow then joins the same
confirm page (exact JSON, single-use token) and audit log as the
curated forms. Catalogue statuses: **verified** (13 — executed live
through the flow), **wired** (226 — preparable via the generic form,
not yet executed), **excluded** (1 — GameConsoleCmd, arbitrary
console access, policy). Contracts/trade/loans, the full priority
family, storage sliders and alerts, trains, farms and the space
programme are all reachable this way. What remains locked,
honestly: belt/pipe upgrades (no vanilla command exists) and
pipe-direction flips (not a real mechanic — flow is
pressure-driven).

Detectors and forecasts understand production rhythm: farms and
other bursty producers are judged on their long-window balance
(finished-month history / last-year flows), not on monthly rates,
so a harvest-cycle crop no longer flags as "stalled" between
harvests.

## Build

```sh
cargo build --release   # binary: target/release/coi(.exe)
```

No runtime dependencies; single static-ish binary (~1 MB).

## Commands

| Command | What it does |
|---|---|
| `coi probe` | Check the game API: server info, game date/tick, route count |
| `coi status` | One-screen summary: date, pause state, pinned products |
| `coi snapshot [path]` | Full snapshot JSON (schema `coi-snapshot/1`): all products, monthly history for pinned; also stored in the SQLite DB |
| `coi trend <filter>` | Quantity/flow trend for a product across stored snapshots |
| `coi db info` / `coi db import <file>` | DB counts / backfill the DB from a snapshot JSON |
| `coi products [filter]` | Table of every product: quantity, storage, produced/consumed |
| `coi report [path.md]` | Eisenhower triage board from live data (Fix now / later / Watch / Ignore) |
| `coi serve [--port 8484] [--bind 127.0.0.1]` | Local dashboard: triage board + pinned table + live screenshot; `/api/snapshot.json`, `/api/report.md`, `/screenshot.jpg` |
| `coi saveinfo <file.save> [--json] [--extract-screenshot out.jpg]` | Offline save inspector: header/CRC integrity, game version, map, mods/DLC, difficulty, seed, current research node, embedded screenshot |

Writes to the game happen only through the website's prepare →
confirm → audit flow (SPEC §12). Policy: any catalogue command may
be prepared (curated or generic form) except the ones the Catalogue
marks "excluded". The CLI remains read-only apart from its local
database.

## Local storage

Snapshots are also stored in a single-file **SQLite** database
(bundled SQLite via rusqlite — no server to run, unlike MySQL; the
JSON files remain the interchange format). Default path `./coi.db`,
override with `--db <path>` or `COI_DB`. Tables: `games`, `snapshots`
(deduped on game+tick), `product_stats`, `product_history` (monthly
series), `issues` (the triage board at snapshot time), `audit`
(reserved for the Tier 2 write log).

- `coi trend <name filter>` — quantity/flow trend across stored snapshots
- `coi db info` — store counts
- `coi db import <snapshot.json>` — backfill from a snapshot file

## Connecting

On the game PC (game running, save loaded, console: `web_api_start`):

```sh
coi status            # defaults to http://127.0.0.1:3073
```

From Claudius's VM over Tailscale (game forwarded per ACCESS.md):

```sh
coi --proxy tailnet --game-url http://<pc-name>:3073 --host-header localhost:3073 status
```

Flags can also come from env: `COI_GAME_URL`, `COI_PROXY`, `COI_HOST_HEADER`.
`--proxy` accepts a proxy URL or `host:port`; `tailnet` derives the
Muse runtime's tunnel proxy from `HTTPS_PROXY` (port forced to 3130).
The Host header override exists because the game rejects any Host other
than `127.0.0.1:3073` / `localhost:3073`.

## Verified (2026-11-02)

- All commands live-tested against Philip's running 0.8.8 game
  (Dragon Isle run, gameId 5583213724730880438) through the Tailscale
  forward — including a dashboard screenshot pull at 1920×1080.
- `saveinfo` output checked field-by-field against the browser save
  reader on `Expanding_1_5m2i.save`: identical, and the extracted
  screenshot is byte-identical to the reader's.
- Generic prepare proven end-to-end: StorageAlertSetEnabledCmd was
  executed through the generic form on the live game and reverted,
  both runs audited "success" (net zero on the save).
- Trade page renders against CoiBridge v0.5 with per-panel "update
  the mod" degradation; full data needs CoiBridge v0.6 routes
  (contracts/loans/edicts/trains-detail).
