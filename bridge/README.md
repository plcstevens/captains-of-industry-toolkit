# CoiBridge — CoI Companion bridge mod

A tiny, **read-only** mod for Captain of Industry v0.8.8 (Update 4.3) that adds
`/coi/v1/*` JSON routes to the game's built-in web server, for use by the
`coi` Rust companion tool. Design: `../SPEC.md` (sections 1, 3, 9, 10).
Author: Lewis Cameron. Current version: **0.5.0**.

| Route | Status |
|---|---|
| `GET /coi/v1/hello` | ✅ live — mod/version liveness probe |
| `GET /coi/v1/research` | ✅ live — current node + progress %, queue, researched count, lab active (ResearchManager) |
| `GET /coi/v1/status` | ✅ live — population, housing capacity/free, homeless, months of food, starving flag (SettlementsManager) |
| `GET /coi/v1/census` | ✅ live — per-proto counts, machine state splits (Working / NotEnoughInput / OutputFull / …) + assigned recipes, transport totals (IEntitiesManager) |
| `GET /coi/v1/topology` | ✅ live — every layout entity `{id, proto, pos}`, every transport `{from, to, fullyConnected, isMoving}`, port-connection edges between entity ids (IoPort.OwnerEntity / ConnectedPort) |
| `GET /coi/v1/storages` | ✅ live — per-storage proto, product, quantity, capacity, fill %, logistics in/out disabled (StorageBase) |
| `GET /coi/v1/trains` | ✅ live (lines level) — line names, train/station counts (TrainLinesManager). Per-train activity/travel counters exist on `Train` (ProductivityCounterHistory) but are deliberately not decoded until they can be cross-checked live against the in-game train stats panel |
| `GET /coi/v1/debug/services` | ✅ live — route table + runtime reflection discovery report (metadata only; nothing invoked) |

All API surfaces above were read from the game's own shipped assemblies
(Mafi.Core.dll, 0.8.8) on 2026-11-02 — see `MODDING-NOTES.md` for the
verified facts (including `[WebRoute]` = `Mafi.Core.WebApi.WebRouteAttribute`
with a required `WebRouteStage` argument, `WebRouteResponse` returns, and
registration via `WebApiServer.scanAllRoutes()` over the resolver's
`AllResolvedInstances`). Census/topology/storages share one entity walk,
cached ~5 s. Every handler is wrapped so a failure returns a JSON 500
(`{"error":"bridgeError",...}`), never an exception into the sim. Vanilla
already covers products, history, game-info and screenshots — the bridge
deliberately does not duplicate those.

## Install & test (prebuilt zip — the normal path)

You need `CoiBridge-mod-0.5.0.zip` (built on Claudius's VM; see below).

1. **Close the game completely.**
2. Unzip the package. You get a folder named `CoiBridge` containing
   `manifest.json`, `CoiBridge.dll`, `readme.txt`.
3. If an older `CoiBridge` folder already exists in the Mods folder,
   delete it first. Then move the new `CoiBridge` folder into
   `%APPDATA%\Captain of Industry\Mods\`. The final layout must be exactly:
   - `%APPDATA%\Captain of Industry\Mods\CoiBridge\manifest.json`
   - `%APPDATA%\Captain of Industry\Mods\CoiBridge\CoiBridge.dll`

   (To open the Mods folder: Win+R, paste `%APPDATA%\Captain of Industry\Mods`,
   Enter. `%APPDATA%` is the **Roaming** tree — the same tree as `Saves`.)
4. Launch the game. If mods are not enabled yet:
   **Settings → Miscellaneous → enable mods**, then **restart the game**
   (the setting only takes effect on restart).
5. Load your save. The manifest declares the mod addable to existing
   saves; if the game shows a mod list for the save, tick **CoiBridge**.
6. Open the in-game console and run: `web_api_start`
   (the game's web server listens on `http://127.0.0.1:3073/`; this must
   be re-run after every game start).
7. In a browser on the same PC, open <http://127.0.0.1:3073/coi/v1/hello> —
   expect: `{"mod":"CoiBridge","version":"0.5.0","gameApi":"v1"}`
8. Spot-check a data route, e.g. <http://127.0.0.1:3073/coi/v1/status> or
   `/coi/v1/census`. Cross-check <http://127.0.0.1:3073/routes>: the
   `/coi/v1/*` routes should be listed alongside the vanilla ones.
9. Leave the game running with the Tailscale forward up
   (`tailscale serve --bg --tcp=3073 tcp://localhost:3073`) and tell
   Claudius — he fetches the routes directly and verifies them.

### Troubleshooting

- **`/coi/v1/hello` 404s** → the mod is not loaded. Check the folder
  nesting (`Mods\CoiBridge\manifest.json`, **not**
  `Mods\CoiBridge\CoiBridge\manifest.json`), that mods are enabled, and
  that you restarted the game after copying the folder in.
- **A data route returns `{"error":"bridgeError",...}`** → note which
  route and tell Claudius. The exception detail is in the game log.
- **Logs**: `%APPDATA%\Captain of Industry\Logs` — search for
  `CoiBridge:` lines (also visible in-game via the `also_log_to_console`
  console command). Healthy signs: `CoiBridge: BridgeRoutes registered
  with the dependency builder`, `CoiBridge: BridgeRoutes constructed
  (M2 managers injected)`, `CoiBridge: BridgeRoutes resolved at Initialize`.

## Safety posture

- **Read-only, Tier 1 only** (SPEC §1): no setters are called anywhere in
  the source — audit it by searching for assignments to game objects;
  there are none.
- **Registers no content**: `RegisterPrototypes` is intentionally empty —
  no products, machines, recipes, research, or config. Nothing is added
  to a save beyond the mod system's own record that the mod exists, so
  deleting the mod folder returns the game to vanilla.
- **Loopback only**: the routes ride on the game's own web server, which
  binds `127.0.0.1:3073`. Nothing in this mod opens a network surface of
  its own.
- **Kill switch**: remove/rename `Mods\CoiBridge`, or run `web_api_stop`
  in-game — either ends all bridge access instantly.
- Tier 2 write actions (SPEC §12) are **not** in this build and will
  never be added to these GET routes; §12.7 moved most of them onto the
  vanilla `/command` channel with the confirm flow living in the Rust tool.

## Building

**Philip does not build this mod** (his call, 2026-11-02). Claudius
builds it on the VM:

- Toolchain: .NET SDK 8 at `~/.dotnet` (net48 via the
  `Microsoft.NETFramework.ReferenceAssemblies.net48` NuGet package).
- Reference assemblies: the game's own `Mafi*.dll` in
  `~/workspace/coi-companion/refs/` (from Philip's install; reference
  only, never redistributed).
- Build + package: `bridge/vm-build/build.sh` → compiles
  `bridge/CoiBridge/*.cs` via `bridge/vm-build/CoiBridge.csproj` and
  produces `dist/CoiBridge-mod-<version>.zip` with `CoiBridge/` at the
  zip root (manifest.json + CoiBridge.dll + readme.txt).
- v0.5.0 build result: **0 warnings, 0 errors.**

A Windows/Visual Studio build from `CoiBridge/CoiBridge.csproj` (the
original ExampleMod-derived project, `COI_ROOT`-based) remains possible
but is no longer the supported path.

## Uninstall

Delete `%APPDATA%\Captain of Industry\Mods\CoiBridge\` and restart the
game. Because the mod registers no content, existing saves load unchanged.
