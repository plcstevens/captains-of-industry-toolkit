QoS World Map v0.1.1 — world-map mine & rig index for the QoS suite.
Author: Lewis Cameron. For Captain of Industry v0.8.8 (Update 4.3).
Requires: QoSCore >= 0.2.0 (install it first, or use the combined
QoS suite pack, which contains both folders).

WHAT IT DOES
Adds one toolbar button, "World mines", opening a window that
lists EVERY world-map mining location — mines and oil rigs alike
(in the game's data a rig is a mine whose product is crude oil, so
one list covers both). Each row shows the location's title,
product, state, level and production step, remaining quantity
(or ∞ for unlimited reserves) and its buffer fill. A text filter,
a state filter (All / Working / Needs attention / Unrepaired) and
sorting (Product, Level, Quantity left, State, Name, with a
Descending toggle) help you find the one you want.

Clicking a row (or its Go button) NAVIGATES: the world map opens
centred on the location and the location's own inspector opens
with it. That is all. Everything you can DO to a location —
shifts, repair, upgrade, ship orders — stays in that inspector,
exactly as in the unmodded game.

WHAT IT NEVER DOES (inert by construction)
- No game commands are constructed or scheduled, ever. There is
  no code path from this mod to any InputCommand.
- No Harmony patches, no prototypes, no save state of its own
  beyond its settings values.
- The window only READS public game state (the mine list and its
  public properties) and calls two public UI navigation methods.
A navigation window cannot alter game state; the worst a bug here
can do is open the wrong inspector.

There is deliberately no 'enabled' master switch (unlike the
suite's acting mods): this mod does nothing unless you open the
window and click. To disable it, delete the folder.

FIRST-HOUR LIVE CHECKS (v0.1.0 was built against the game's
assemblies but these four behaviours can only be settled live;
each is isolated behind a small seam in the code)
1. Inspector placement — the click uses the public
   InspectorsManager.TryActivateFor. If the inspector lands oddly
   without the world map's internal selection bookkeeping, the
   fallback is a guarded replication of that bookkeeping; the
   seam is MineNavigator.Navigate.
2. Controller resolution — the game's WorldMapWindow.Controller
   is resolved from mod DI at startup. If your log shows the
   "not resolvable" warning, clicks degrade to inspector-only
   (no map centring) and the window still works as an index.
3. Fullscreen stacking — the world map is immersive-fullscreen;
   if the inspector or this window stacks badly against it, set
   'navigate_opens_map' off in the settings: clicks then open
   the inspector only.
4. Toolbar slot — the button sits at toolbar order 1500 (the
   slot CoI_PriorityManager ships with). If it collides with a
   vanilla button on your build, that is what the check is for.
Also cosmetic: the button currently uses the generic Priority
icon; a dedicated glyph is a later polish item.

INSTALL
1. Close the game completely.
2. QoSCore must be installed (see its readme) — this mod declares
   a dependency on it and the game will report an error without it.
3. Unzip so the QoSWorldMap folder lands in:
      %APPDATA%\Captain of Industry\Mods\
   Final layout:
      %APPDATA%\Captain of Industry\Mods\QoSWorldMap\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSWorldMap\QoSWorldMap.dll
      %APPDATA%\Captain of Industry\Mods\QoSWorldMap\config.json
4. Launch the game, load your save, tick QoSWorldMap in the
   save's mod list if asked.

DEFAULTS (change them in Settings -> QoS World Map)
  navigate_opens_map   true    row click opens the world map
                               centred on the location, then its
                               inspector; off = inspector only

ACTIVITY RECORD
Every navigation this mod performs is recorded to the QoS activity feed
(%APPDATA%\Captain of Industry\QoS\events-v1.jsonl) by QoSCore,
and shows on the Superintendent website's Log page. Controlled by
QoSCore's activity_log_enabled setting (default on).

REMOVAL SAFETY
The mod registers no prototypes, recipes or entities and stores
nothing in your save beyond its own settings values (inert once
the mod is gone). Delete the QoSWorldMap folder to remove it.
