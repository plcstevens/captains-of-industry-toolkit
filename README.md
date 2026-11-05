# Captains of Industry Toolkit

A personal toolkit for [Captains of Industry](https://www.captain-of-industry.com/) (Update 4), by Lewis Cameron.

## What's inside

- **Companion website** (`src/`, served by the `coi` binary) — a local website you log into: live game status, issues and forecasts, products, research, trade, a full command catalogue, game plans and a supervised session runner. It reads the game through its in-game web API plus the CoiBridge mod, and acts only through a prepare → confirm → audit flow.
- **CoiBridge** (`bridge/`) — a read-only mod that exposes extra game state (population, research, census, storages, topology, per-instance machines, power) over local HTTP routes for the companion. It never writes to the game.
- **QoS mod suite** (`qos/`) — small quality-of-life mods that link into one suite: a shared **QoSCore** foundation plus one mod per feature (QoSChickens today; more in preparation). Design and challenge notes: `QOL-MODS.md`, `QOL-MOD-PREP.md`.

## Not in this repo

- The game's own assemblies (needed as compile-time references for the C# mods) — bring your own from your game install; see `bridge/` and `qos/` build notes.
- Built binaries — see Releases.
- Live game captures and personal playthrough data.

## Status

Personal project, built for one player's Dragon Isle run. Specifications live in `SPEC.md`; the build log in `NIGHT-LOG.md`. Expect sharp edges.
