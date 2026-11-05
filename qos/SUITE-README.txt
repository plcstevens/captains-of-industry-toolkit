QoS Suite v0.3.0 — a suite of small quality-of-life mods for
Captain of Industry v0.8.8 (Update 4.3). Author: Lewis Cameron.

THE IDEA
One QoS, many small mods. Each feature is its own tiny mod in its
own folder; they link into a suite through one shared foundation,
QoSCore. You can install the whole pack (this zip), update or
remove features individually, and every feature stays removable
without touching your save's content.

CONTENTS OF THIS PACK
  QoSCore/        the shared foundation (logging, suite registry,
                  config conventions). Changes no gameplay alone.
                  Every feature mod depends on it — keep it.
  QoSChickens/    animal farm distribution: once a day, evens out
                  same-species animal farms using the game's own
                  move command, with hard guards (never past a
                  slaughter limit, growth-paused farms untouched,
                  breeding-nucleus floor). SHIPS DISABLED — enable
                  it in its settings when ready.
  QoSVehicleRecovery/  stuck truck cargo rescue: once a day,
                  rescues trucks stuck with their load via the
                  game's own recovery (0.2 Unity; the load stays
                  aboard — recovery never touches cargo), under
                  hard guards (vanilla gate pre-checked, Unity
                  reserve floor, daily cap). Optional stage 2
                  empties a still-undeliverable load into the
                  game's storage routing. SHIPS DISABLED.
  QoSWorldMap/    world-map mine & rig index: a window listing
                  every world-map location (product, state, level,
                  quantity left, buffer), filterable and sortable;
                  clicking a row centres the world map on it and
                  opens its inspector. NAVIGATION ONLY — it issues
                  no commands and cannot change game state, so it
                  needs no enable switch. New in v0.3.0; see its
                  readme for the four first-hour live checks.

More feature mods (a safe train-cargo clear) are designed and
will slot in as sibling folders — QoSTrainCargo/ — each
depending on QoSCore, never on each other.

INSTALL
1. Close the game completely.
2. Extract this zip into:
      %APPDATA%\Captain of Industry\Mods\
   You should end up with:
      %APPDATA%\Captain of Industry\Mods\QoSCore\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSChickens\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSVehicleRecovery\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSWorldMap\manifest.json
      (plus this readme, which you can delete)
3. Launch the game; enable mods in Settings -> Miscellaneous if
   not already enabled; load your save and tick the QoS mods in
   the save's mod list if asked.
4. Per-mod readmes (inside each folder) cover behaviour,
   settings and defaults.

REMOVING PART OF THE SUITE
Delete any feature folder on its own — saves are unaffected
(features register no content). If you delete QoSCore, dependent
features will refuse to load and the game will tell you why; your
save is not harmed. Remove features first, then Core, if you want
the whole suite gone.
