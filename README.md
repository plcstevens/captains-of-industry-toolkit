# Captains of Industry Toolkit

A personal toolkit for [Captains of Industry](https://www.captain-of-industry.com/) (Update 4), by Lewis Cameron.

## What's inside

- **Superintendent** (`coi/`, a single `superintendent.exe`) — a
  local website (no login; it binds loopback and just opens) that
  sits next to the game: live status, an issues board with stated
  reasoning, forecasts, products, terrain and earthworks,
  research, trade, a full command catalogue, game plans and a
  supervised session runner, plus a Guidance domain (learn the
  game, learn the tool, and *why it suggests* — rendered from a
  single canon of rules in `docs/canon/`). It reads the game
  through its in-game web API plus the CoiBridge mod, and acts
  only through a prepare → confirm → audit flow. It runs entirely
  on your machine — no account, no cloud, no telemetry.
- **CoiBridge** (`bridge/`) — a read-only mod that exposes extra
  game state (population, research, census, storages, topology,
  per-instance machines, power, terrain) over local HTTP routes
  for Superintendent. It never writes to the game.
- **QoS mod suite** (`qos/`) — small quality-of-life mods on a
  shared QoSCore foundation: QoSChickens, QoSVehicleRecovery,
  QoSPriorityActuator, QoSWorldMap, and QoSBuilder (blueprint
  placement and terrain designations from confirmed job files).
  Player notes in `qos/SUITE-README.txt`; design notes in
  `qos/dev/`.
- **Plans** (`plans/`) — game plans in the format specified by
  `docs/GAME-PLAN-FORMAT.md`, including the fresh-start reference
  plan the session runner walks.

## Where things live

- `README.md` (this file), `INSTALL.md`, `REMOTE-ACCESS.md` — the
  player-facing entry points.
- `docs/` — player-facing deeper documentation, including the
  canon (`docs/canon/`).
- `dev/` — the development paper trail: `SPEC.md`, `DESIGN.md`,
  the build log, audits, release working documents, design mocks
  and the synthetic test captures.
- `.llms/` — agent-facing material and the index of what an agent
  should read, in what order, for what task.

## Not in this repo

- The game's own assemblies (needed as compile-time references for
  the C# mods) — bring your own from your game install; see
  `bridge/` and `qos/` build notes.
- Built binaries — see Releases.
- Live game captures and personal playthrough data. (Synthetic
  fixtures under `dev/captures/` are generated data for the check
  scripts.)

## Status

Personal project, published as-is. The specification lives in
`dev/SPEC.md`; the build log in `dev/NIGHT-LOG.md`. Expect sharp
edges.

## Building

**Superintendent (`superintendent.exe`)** — built by GitHub
Actions: push a tag like `coi-v0.1.0` and the release workflow
builds the Rust crate in `coi/` on a Windows runner and attaches
`superintendent.exe` (plus a SHA256 file) to the GitHub Release.
Local build: `cargo build --release` in `coi/`.

**Mods (CoiBridge, QoS suite)** — built locally, not in CI: they
compile against the game's own assemblies (`Mafi*.dll`), which are
not redistributable and so cannot live in this repo. The build
scripts take a call-out to wherever those DLLs are:

```sh
export COI_REFS_DIR="/path/to/game/Captain of Industry_Data/Managed"
bridge/vm-build/build.sh   # -> dist/CoiBridge-mod-<version>.zip
qos/build/build.sh         # -> dist/QoSCore/QoSChickens/suite zips
```

With `COI_REFS_DIR` unset the scripts fall back to a repo-local
`refs/` folder. Only prerequisite: a .NET 8 SDK.
