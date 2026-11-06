QoS Suite v0.5.1 — a suite of small quality-of-life mods for
Captain of Industry v0.8.8 (Update 4.3). Author: Lewis Cameron.

*** SAVE-BLOCKING BUG IN SUITE v0.5.0 AND EARLIER — READ FIRST ***
QoSChickens, QoSVehicleRecovery and QoSPriorityActuator at
version 0.1.1 (suite packs up to v0.5.0) MAKE THE GAME FAIL TO
SAVE, with an error like: Failed to create generic serializer
for 'ChickenBalancer'. Cause: each feature's daily service
subscribes to the game's calendar, that subscription is itself
written into save files, and the 0.1.1 service classes could
not be written. Your game and your existing saves are NOT
harmed — the save fails before anything is written, and no
save on disk was ever produced by the broken versions.
FIX: replace the three feature folders with the ones in THIS
pack (0.1.2 — the services now carry the game's own sanctioned
serializer pattern). Do not keep the old 0.1.1 folders
alongside. QoSCore and QoSWorldMap were never affected.

THE IDEA
One QoS, many small mods. Each feature is its own tiny mod in its
own folder; they link into a suite through one shared foundation,
QoSCore. You can install the whole pack (this zip) and update
features individually. Removal: QoSCore and QoSWorldMap come out
freely; the three daily-pass features register no content, but
their calendar subscriptions are serialized into saves written
while they are installed, so their manifests now set
can_remove_from_saved_game=false — the game will refuse to load
such a save without the mod rather than risk damaging it (they
ship disabled and do nothing while disabled, so leaving the
folders in place costs nothing).

CONTENTS OF THIS PACK
  QoSCore/        the shared foundation (logging, suite registry,
                  config conventions, and — since v0.2.0 — the
                  unified activity recorder; see THE LOG below).
                  Changes no gameplay alone. Every feature mod
                  depends on it — keep it.
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
  QoSPriorityActuator/  starved-machine priority nudges: once a
                  day, machines stuck input-starved for days get
                  their general priority stepped up one bounded
                  step (trucks serve them sooner), and stepped
                  back to their proto default when they work
                  again — under a stateless ownership rule: only
                  values its own rule could have produced are
                  ever adjusted, so anything you set yourself is
                  never fought. Blocked and parked machines are
                  never touched. SHIPS DISABLED.
  QoSWorldMap/    world-map mine & rig index: a navigation-only
                  window listing every world-map mine and rig with
                  product, state, level, remaining quantity and
                  buffer; clicking a row centres the map on the
                  location and opens its inspector. Issues no
                  commands, changes no game state.

THE LOG (new in suite v0.5.0)
Every action these mods actually take — each command scheduled,
each world-map navigation — is recorded by QoSCore to one feed
file:

  %APPDATA%\Captain of Industry\QoS\events-v1.jsonl

The CoI Companion website imports that feed and shows it on its
Log page, next to the commands applied through the website
itself and the snapshots taken there: one chronological replay
of everything the toolkit did. Recording is on by default and
is controlled by QoSCore's "activity_log_enabled" setting (not
by the features' log verbosity settings). Only actual actions
are recorded — never diagnostics or skipped passes. Actions
from older mod versions were never recorded and cannot be
backfilled; the record starts with this version.

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
      %APPDATA%\Captain of Industry\Mods\QoSPriorityActuator\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSWorldMap\manifest.json
      (plus this readme, which you can delete)
3. Launch the game; enable mods in Settings -> Miscellaneous if
   not already enabled; load your save and tick the QoS mods in
   the save's mod list if asked.
4. Per-mod readmes (inside each folder) cover behaviour,
   settings and defaults.

REMOVING PART OF THE SUITE
QoSWorldMap and QoSCore delete freely — saves are unaffected
(they register no content and store nothing in saves). The
three daily-pass features (Chickens, VehicleRecovery,
PriorityActuator) are different since 0.1.2: a save WRITTEN
while they are installed contains their calendar subscriptions,
so those saves need the folders present to load — the game
enforces this (can_remove_from_saved_game=false) with a clear
message. Saves written before you installed the suite never
contain them and are unaffected. If you want the whole suite
gone: load a pre-suite save (or start fresh), then delete the
folders — features first, then Core (deleting Core while
features remain makes them refuse to load; the game will tell
you why, and your save is not harmed). The activity feed file
lives outside the mod folders (see THE LOG), so removing mods
does not delete the record; delete the feed file itself if you
want it gone.
