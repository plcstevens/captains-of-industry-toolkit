QoS Vehicle Recovery v0.1.2 — stuck truck cargo rescue for the
QoS suite. Author: Lewis Cameron. For Captain of Industry
v0.8.8 (Update 4.3).
Requires: QoSCore >= 0.1.0 (install it first, or use the
combined QoS suite pack, which contains both folders).

VERSION 0.1.2 — SAVE FIX (read this if you had 0.1.1)
Version 0.1.1 of this mod PREVENTED THE GAME FROM SAVING:
saving failed with "Failed to create generic serializer for
'{cls}'". The cause: the mod's daily service subscribes to the
game's calendar, and that subscription is itself written into
save files — 0.1.1's service class could not be written. Your
game and your existing saves were never harmed (the save fails
before anything is written). 0.1.2 fixes it with the game's own
sanctioned serializer pattern. If 0.1.1 is installed, REPLACE
its folder with this one — do not keep both.

WHAT IT DOES
Once per in-game day (at day end), the mod looks for trucks
that are stuck WITH their load and schedules the game's OWN
commands to rescue them — the same RecoverVehicleCmd and
DiscardVehicleCargoCmd the game itself uses. The mod adds
policy (when, which trucks, and when NOT to), never a new
mechanism. Two stages:

Stage 1 — RECOVER (the default behaviour)
A truck flagged cannot-deliver, or stuck on navigation by the
game's own sustained signals, is recovered: the game teleports
it to the closest accepting depot for a flat 0.2 Unity. The
important part, verified from the game assemblies: vanilla
recovery NEVER touches the cargo. The truck drives out of the
depot with its load exactly as it was, its assignment restored
and its jobs re-planned. Your goods are rescued with the
truck, not dumped, not lost.

Stage 2 — EMPTY (optional, default OFF)
If a truck THIS mod already recovered is flagged
cannot-deliver AGAIN on a later day, still carrying the same
load, that load is the problem — so the mod empties it through
the game's own cargo discard. The game routes discarded cargo
into its storage network: real storages in global-priority
order, the shipyard as overflow, and only discardable
products (or a load no storage can take) are actually
destroyed. The game charges 0.1 Unity on success. The freed
truck goes back to work instead of sitting on an
undeliverable load forever.

HARD GUARDS (always on, cannot be configured away)
- Vanilla's own recovery gate is checked BEFORE scheduling:
  a depot must accept the truck and Unity must afford the
  cost. This matters — a recovery request the game refuses
  still cancels the truck's jobs on its way in, so the mod
  never schedules a rescue the game would refuse.
- Unity reserve floor (config): no action is scheduled if
  the balance after its cost would drop below the floor.
- Daily cap (config) on scheduled recoveries.
- Trucks being recovered, scrapped or replaced are never
  touched; neither are unspawned or destroyed trucks.
- Stuck must be sustained (see above) — a momentary pathing
  hiccup never triggers a paid recovery.
- Stage 2 only ever touches a load this mod itself rescued,
  only the same product, and its marks expire after 7 days
  or when the truck delivers the load.

WHAT THIS MOD DELIBERATELY DOES NOT HAVE
There is no "preferred storage" setting. No public game
surface lets a mod steer a truck's cargo into a chosen
storage — the game's discard routing (stage 2) picks the
destination, and recovery (stage 1) keeps the load aboard.
A setting the game cannot honour would be a lie in the
settings UI, so it is not shipped. For TRAINS, the same
applies: vanilla has no per-train or per-depot overflow
destination at all (its lost-train re-dispatch follows the
per-product NetworkPriority you set on stations — lower
wins). See qos/BUILD-NOTES.md round 3 for the full evidence.

INSTALL
1. Close the game completely.
2. QoSCore must be installed (see its readme) — this mod
   declares a dependency on it and the game will report an
   error without it.
3. Unzip so the QoSVehicleRecovery folder lands in:
      %APPDATA%\Captain of Industry\Mods\
   Final layout:
      %APPDATA%\Captain of Industry\Mods\QoSVehicleRecovery\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSVehicleRecovery\QoSVehicleRecovery.dll
      %APPDATA%\Captain of Industry\Mods\QoSVehicleRecovery\config.json
4. Launch the game, load your save, tick QoSVehicleRecovery
   in the save's mod list if asked.
5. The mod ships DISABLED. When ready: Settings -> QoS
   Vehicle Recovery, set 'enabled' on. Rescue passes start
   at the next day end.

DEFAULTS (change them in Settings -> QoS Vehicle Recovery)
  enabled                            false  master switch (ships OFF)
  recover_cannot_deliver             true   stage 1: cannot-deliver trucks
  recover_navigation_stuck           true   stage 1: navigation-stuck trucks
  min_navigation_failed_streak       3      streak needed when the game's
                                            repeated-failure flag is unset
  max_recoveries_per_day             5      scheduled recoveries per day
  unity_reserve_floor                5.0    Upoints kept in reserve, always
  empty_cargo_if_still_undeliverable false  stage 2 (see above)
  log_actions                        true   one log line per scheduled
                                            action, prefixed
                                            QoS.VehicleRecovery:

COSTS (the game's own prices, charged by the game)
  Recovery: 0.2 Unity per truck, charged when the recovery is
  enqueued, never refunded (vanilla behaviour).
  Stage-2 empty: 0.1 Unity, charged by the game only on
  success.

ACTIVITY RECORD
Every recovery or cargo-empty this mod schedules is recorded to the QoS activity feed
(%APPDATA%\Captain of Industry\QoS\events-v1.jsonl) by QoSCore,
and shows on the CoI Companion website's Log page. Controlled by
QoSCore's activity_log_enabled setting (default on).

REMOVAL SAFETY
The mod registers no prototypes, recipes or entities. One
honest exception since 0.1.2: the service's calendar
subscription is serialized into saves written while the mod is
installed (that is how the game's saveable calendar events
work), so the manifest declares can_remove_from_saved_game =
false — if a save made with this mod installed is loaded
without it, the game will refuse with a clear message instead
of risking a damaged save. To remove the mod entirely, delete
its folder while playing a save that predates installing it
(or just leave the folder in place: it ships disabled and does
nothing until enabled). Its stage-2 marks are session memory
only. Vehicle and cargo state in your save is untouched
vanilla state either way.
