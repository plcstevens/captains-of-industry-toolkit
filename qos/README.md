# QoS suite — the mods

Small, single-purpose mods for Captains of Industry (Update 4 / v0.8.x),
built as one suite around a shared core. Where the companion app
*observes and advises*, these mods *act* — carefully, and never by
default.

| Mod | What it does |
|---|---|
| **QoSCore** | Shared services every other QoS mod builds on: logging, the suite registry, and the append-only activity event feed the companion's Log page imports. |
| **QoSChickens** | Rebalances chickens between farms on a daily pass, exactly simulating the game's own distribution rules before it acts. |
| **QoSVehicleRecovery** | Recovers vehicles stuck in a cannot-deliver loop (paying the game's own Unity recovery cost), with an optional second stage for the cargo that caused the loop. |
| **QoSPriorityActuator** | Nudges the general priority of machines that have been starved for days, and restores them when they recover. Blocked machines are never touched. |
| **QoSWorldMap** | A window listing your world-map mines and rigs, with navigation. Read-only; changes nothing. |
| **QoSBuilder** | Executes *sealed job files* written by the companion (blueprint placement and terrain designations) after hash, game and expiry checks. Ships separately — see its own folder and readme. |

## Rules the suite lives by

- **Master switches ship OFF.** Nothing acts until you enable it in
  the mod's config.
- **The game's own commands only.** A mod never edits game state
  directly; it schedules the same commands the game itself uses,
  after simulating the outcome and checking its guards. If the
  guards can't be satisfied, it skips the day.
- **Removal-safe by declaration.** Mods that subscribe to saved
  calendar events declare `can_remove_from_saved_game: false` and
  carry hand-written serializer pairs — the build refuses to compile
  a subscriber without one (`tools/check_serializer_pairs.py`).
- **Everything is logged**, with the prediction it acted on, to the
  QoS event feed.

Start with `SUITE-README.txt` in this folder for install and config,
and `ARCHITECTURE.md` for how the suite is put together. Build
scripts live under `build/`.
