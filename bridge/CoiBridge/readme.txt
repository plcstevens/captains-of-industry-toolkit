CoI Bridge v0.6.4 — read-only web API bridge for the CoI Companion tool.
Author: Lewis Cameron. For Captain of Industry v0.8.8 (Update 4.3).

INSTALL (prebuilt — no building needed)
1. Close the game completely.
2. Unzip CoiBridge-mod-0.6.4.zip. You get a folder named CoiBridge
   containing manifest.json, CoiBridge.dll, readme.txt.
3. If an older CoiBridge folder already exists in the Mods folder,
   delete it first. Then move the new CoiBridge folder into:
      %APPDATA%\Captain of Industry\Mods\
   The final layout must be exactly:
      %APPDATA%\Captain of Industry\Mods\CoiBridge\manifest.json
      %APPDATA%\Captain of Industry\Mods\CoiBridge\CoiBridge.dll
   (To open that folder: press Win+R, paste the Mods path above, Enter.
   %APPDATA% is the Roaming tree — the same tree as your Saves folder.)
4. Launch the game. If mods are not enabled yet:
   Settings -> Miscellaneous -> enable mods, then restart the game.
5. Load your save. The manifest allows adding this mod to an existing
   save; if the game shows a mod list for the save, tick CoiBridge.
WHAT'S NEW IN v0.6.4
Two read additions:
   GET /coi/v1/machines      one row per machine INSTANCE: id, proto,
                             position, live state, assigned recipes,
                             paused / enabled. The census aggregates
                             states per proto; this is the per-entity
                             view (closes the diagnosis gap where a
                             state could not be mapped to an entity).
   /coi/v1/trains            now also returns RAW per-train counters
                             (activity + travel ProductivityCounter
                             buckets: category report counts per
                             600-report bucket, monthly/yearly rings,
                             ongoing bucket) plus "counterLabels" and
                             "unverifiedDecoding": true. The numbers
                             are the game's stored buckets verbatim —
                             how the in-game stats panel turns them
                             into percentages is NOT live-verified yet,
                             so do not read them as percentages. On a
                             save with no trains the list is empty,
                             which is correct.

WHAT'S NEW IN v0.6.3
Two read additions from the recon-round-4 decoder pack:
   /coi/v1/power             now also returns an "averages" block
                             (production / consumption / generation
                             capacity, each lastDayKw / lastMonthKw /
                             lastYearKw) plus "averagesUnverified":
                             true — the decoding follows the recon
                             layout but has NOT been cross-checked
                             against a running game yet
   GET /coi/v1/priorities    per-entity custom priorities: only
                             entities with at least one visible id,
                             and only the visible ids per entity.
                             Safety note: ids are probed only through
                             IsCustomPriorityVisible (which never
                             asserts); GetCustomPriority is called
                             only for ids that returned visible —
                             never with an unprobed id.
Everything else is unchanged from v0.6.2.

WHAT'S NEW IN v0.6.2
Two small additions that close gaps the game-plan format found:
   GET /coi/v1/power         electric power in kW: generated,
                             consumed, demanded, surplus demand,
                             capacity, wasted, balance, plus
                             generator / consumer counts
   /coi/v1/research          now also lists the completed
                             ("researched") node ids, not just
                             the count
Everything else is unchanged from v0.6.1.

WHAT'S NEW IN v0.6.1
One new read route, plus richer census/storage data:
   GET /coi/v1/quick-trades  every quick-trade provider: live buy/pay
                             quantities, unity cost, step / sold-out
                             state
   /coi/v1/census            now includes a "priorities" map of
                             entityId -> general priority, only for
                             entities changed from their default
   /coi/v1/storages          rows now include generalPriority; storage
                             rows also include import/export custom
                             priorities and the four logistics slider
                             steps (import/export/transport from/until)
The v0.6 routes are unchanged. If a v0.6.1 manager cannot be
resolved, only the V6 routes are lost — the mod logs the failure and
the rest keeps working.

WHAT'S NEW IN v0.6.0
Four new read routes for state the vanilla web API does not expose
at all (its route table has nothing for trade, contracts, loans or
edicts):
   GET /coi/v1/contracts     active contracts + unlocked offers
   GET /coi/v1/loans         credit terms + active loans
   GET /coi/v1/edicts        every edict: enabled / active / why not
   GET /coi/v1/trains-detail train lines with ids, networks, schedules
The v0.5 routes are unchanged. If a v0.6 manager cannot be resolved,
only the new routes are lost — the mod logs the failure and the rest
keeps working.

6. Open the in-game console and run:
      web_api_start
7. Test in a browser on the same PC:
      http://127.0.0.1:3073/coi/v1/hello
   Expected reply:
      {"mod":"CoiBridge","version":"0.6.3","gameApi":"v1"}

ROUTES (all GET, all read-only)
   /coi/v1/hello            liveness probe
   /coi/v1/research         current research, progress, queue,
                            completed node ids + count
   /coi/v1/status           population, housing, months of food
   /coi/v1/census           entity counts, machine states, recipes
   /coi/v1/topology         entities + positions, transports, port edges
   /coi/v1/storages         per-storage product, quantity, capacity
   /coi/v1/trains           train lines (names, trains, stations)
                            + raw per-train counters (unverified)
   /coi/v1/machines         per-machine instances: state, recipes,
                            paused / enabled, per entity id
   /coi/v1/contracts        active contracts + unlocked offers
   /coi/v1/loans            credit terms + active loans
   /coi/v1/edicts           every edict: enabled / active / why not
   /coi/v1/trains-detail    train lines with ids, networks, schedules
   /coi/v1/quick-trades     quick-trade providers and live quantities
   /coi/v1/power            electric power: generation vs demand (kW),
                            plus unverified last day/month/year averages
   /coi/v1/priorities       per-entity custom priorities (visible ids only)
   /coi/v1/debug/services   discovery report (reflection metadata only)

TROUBLESHOOTING
- /coi/v1/hello returns 404: the mod is not loaded. Check the folder
  nesting (Mods\CoiBridge\manifest.json — NOT
  Mods\CoiBridge\CoiBridge\manifest.json), that mods are enabled, and
  that you restarted the game after installing.
- A data route returns {"error":"bridgeError",...}: tell Claudius which
  route. The detail is in the game log at
  %APPDATA%\Captain of Industry\Logs — search for "CoiBridge:" lines.
  "CoiBridge: BridgeRoutes constructed (M2 managers injected)" in the
  log means the mod loaded correctly.

SAFETY
The mod registers no products, machines, recipes or research and never
writes to game state. It only answers requests on the game's own web
server (127.0.0.1:3073). Removing the CoiBridge folder and restarting
the game returns everything to vanilla; existing saves load unchanged.
