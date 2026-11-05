# QoS suite — architecture

Philip's decision (2026-11-04 22:03): the quality-of-life features
ship as **small individual mods** that link into a **single QoS
suite**. Sub-mod dependencies are explicitly OK where that's how a
feature is best achieved. CoiBridge stays separate and read-only;
the QoS suite is the gameplay/UI side of the programme.

## Layout

```
qos/
  ARCHITECTURE.md          this file
  BUILD-NOTES.md           what was built vs the prep designs
  QoSCore/                 the shared sub-mod (mod id "QoSCore")
    QoSCoreMod.cs          IMod shell (PriorityManager shape)
    QoSLog.cs              prefixed logging for the whole suite
    SuiteRegistry.cs       feature mods announce themselves here
    QoSCoreService.cs      core's own global dependency
    manifest.json  config.json  readme.txt
  QoSChickens/             feature mod (mod id "QoSChickens")
    QoSChickensMod.cs      IMod shell
    ChickenBalancer.cs     the policy/scheduler service
    manifest.json  config.json  readme.txt
  QoSVehicleRecovery/      feature mod (mod id "QoSVehicleRecovery")
    QoSVehicleRecoveryMod.cs  IMod shell
    VehicleRecoveryService.cs the policy/scheduler service
    manifest.json  config.json  readme.txt
  QoSWorldMap/             feature mod (mod id "QoSWorldMap"; sources
    QoSWorldMapMod.cs        complete round 4 — compile awaits the
    WorldMinesWindow.cs      game's UnityEngine*.dll refs, see
    WorldMinesController.cs  BUILD-NOTES round 4)
    MineNavigator.cs
    manifest.json  config.json  readme.txt
  QoSTrainCargo/           (future — QOL-MOD-PREP §3)
  build/                   VM build scaffold (mirrors bridge/vm-build)
    QoSCore.csproj  QoSChickens.csproj  build.sh
```

One folder per mod, one DLL per mod, one manifest per mod. Sources
live here; the build scaffold in `build/` compiles them against the
game assemblies in `../../refs/` (net48, Microsoft.NETFramework
.ReferenceAssemblies) exactly like the bridge build.

## How the suite presents as "a single QoS"

Three artefacts are staged in `dist/` per release:

| Artefact | Contents |
|---|---|
| `QoSCore-mod-<v>.zip` | `QoSCore/` folder only |
| `QoS<Feature>-mod-<v>.zip` | that feature's folder only |
| `QoS-suite-<v>.zip` | **all** suite mod folders side by side at the zip root |

Extracting the suite zip into `%APPDATA%\Captain of Industry\Mods\`
yields `Mods\QoSCore\…`, `Mods\QoSChickens\…`, etc. — the suite is
the pack; the individual zips exist so a feature can be updated or
removed without touching the rest. A suite readme in the pack
explains the composition and that new feature mods slot in as
siblings. In-game, every suite log line carries a `QoS.` prefix
and the suite registry logs the loaded composition at startup, so
the suite also reads as one thing in the log.

## Dependency rules (the game's real mechanics — verified)

From the official modding README + manifest schema +
On-Demand-Contracts' shipped manifest (vendor copies):

- `mod_dependencies`: **string array of mod ids**, each optionally
  carrying a `>=` version constraint (`"QoSCore >= 0.1.0"`). The
  game loads mods in dependency (topological) order and reports an
  error if a mandatory dependency is missing.
- `optional_mod_dependencies`: same syntax; loaded first if
  present, no error if absent. `incompatible_mods` also exists.
- Manifest `id` must match `[a-zA-Z0-9][a-zA-Z0-9_-]*` and must not
  start with `COI-`. Required manifest fields: `id`, `version`,
  `primary_dlls`.

Suite rules built on that:

1. **Every feature mod declares `mod_dependencies: ["QoSCore >=
   0.1.0"]`** and references `QoSCore.dll` at compile time with
   copy-local OFF — the DLL is provided by the QoSCore mod folder
   at runtime, never duplicated inside a feature folder (two
   copies would be two different `SuiteRegistry` types).
2. **Features never depend on each other.** A feature that needs
   another feature's capability is a design smell: the shared part
   moves down into QoSCore, or the features stay independent.
3. **QoSCore depends on nothing** (beyond the game).
4. Cross-assembly resolution through the declared dependency is
   the documented mechanism; it is flagged as *to be confirmed at
   first live install* (first-hour check, like the world-map
   mod's four unknowns) — the build cannot prove the loader's
   behaviour from the VM.

## Config (the game's real mechanism — verified)

Per the official README and shipped mods: a mod ships a
`config.json` declaring snake_case parameters
(`{ "name": { "default": …, "description": "…" } }`, with
`min`/`max`/`is_integer` for numbers). The game exposes them in its
settings UI and **persists values in save files**. Code reads them
through the mod's own `ModJsonConfig`:
`GetBool/GetInt/GetDouble/GetString(name, default)`.

Services reach config the PriorityManager way: the concrete mod
class is constructor-injected into its services
(`PriorityGroupStore(PriorityManagerMod mod)` reads
`mod.JsonConfig`). Suite convention: a service re-reads its
feature's config values on every evaluation pass — no caching, no
`OnValueChanged` plumbing — so a settings change takes effect on
the next pass with zero extra machinery.

QoSCore does **not** centralise feature config: config is per-mod
by the game's own design (each mod's values persist under its own
id). Core provides the *pattern*, not a store.

## QoSCore contents (deliberately minimal)

Only what the first feature (QoSChickens) actually needs, plus
the suite-identity plumbing:

- **`QoSLog`** — static logger; every suite line is prefixed
  `QoS.<Feature>:` so suite output is greppable as one body.
  A `Verbose` flag (set by core from its own
  `verbose_logging` config) gates chatty diagnostics.
- **`SuiteRegistry`** — a `[GlobalDependency]` service with one
  job: feature mods `Announce(featureId, version)` from their
  `Initialize`; the registry logs the suite composition. It holds
  no state any feature depends on for behaviour.
- **`QoSCoreService`** — core's own global dependency: reads
  core config (verbose flag), announces QoSCore itself.

Deliberately NOT in core (would be speculation): shared UI
windows, a command bus, cross-feature state, persistence helpers.
§3 (train cargo) may one day want a shared "confirm + charge
Unity" helper; it goes into core *when that feature is built*,
not before.

## Removal safety

Every suite mod registers **no prototypes, no recipes, no
entities** — behaviour only, over shipped game systems. Manifests
carry `can_add_to_saved_game: true` and
`can_remove_from_saved_game: true`. Removing a feature folder
removes the behaviour; removing QoSCore makes dependent features
fail the game's dependency check cleanly (reported error, not a
corrupted save). The one residue the game itself creates: config
values are persisted in save files by the game's config system —
with the mod gone they are inert data, read by nothing.

## Relation to CoiBridge and the companion features

- **CoiBridge** is *not* part of the suite: separate mod, separate
  posture (read-only, Tier 1), no dependency in either direction.
  A QoS feature may one day *read* bridge routes via the
  companion, never via a mod dependency.
- **Train advisor (§4) and productivity alerts (§5)** are
  **companion-side features, not mods**: they live in the Rust
  companion + bridge read routes. They link to the suite by
  *data* (the companion reads game state; QoS mods change game
  behaviour) — there is no mod-dependency relationship, and
  forcing one would couple a read-only tool to gameplay mods for
  no benefit. If the advisor ever wants an in-game home, §2's
  window pattern is the route, and *that* window mod would join
  the suite as another sibling feature mod depending on QoSCore.
