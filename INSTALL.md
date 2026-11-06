# Installing the toolkit on the game PC

Downloads go to three destinations. Quit the game before swapping any
mod files — relaunch afterwards.

## 1. The companion program

Replace the old companion program file in the folder where you already
keep it — the same folder as its database file. Keep the database file:
it holds your website password and all snapshots, and carries over.
Run the program by double-clicking it; it opens the local website in
your browser. (On this repo, the Actions tab's companion-release
workflow also builds the current program — Run workflow on main, then
download the artifact it produces.)

The current build is 6,345,284 bytes. It includes the /place page
(blueprint placement through the QoSBuilder mod, section 4 below),
the /designate page (terrain designation pads and removals through
the same mod, section 4 below), the redesigned interface — sidebar
navigation, an always-visible game status strip, and server-drawn
charts throughout — and the Terrain page: a whole-map height view
fed by the CoiBridge terrain routes (section 2 below), ore deposit
totals with per-snapshot depletion history, and an earthworks
estimator that works out cut and fill volumes for levelling a
rectangle to a chosen height.

## 2. The CoiBridge mod (archive: CoiBridge-mod-0.6.6)

Unzip into the game's Mods folder:

    %APPDATA%\Captain of Industry\Mods\

The archive contains a single CoiBridge folder. Replace the old
CoiBridge folder there completely — do not merge old and new files.
Version 0.6.5 added read-only terrain, deposits and designations
routes that feed the companion's Terrain page; version 0.6.6 adds
each designation's corner heights and centre to that read-back, so
the /designate page can verify placed designations exactly. The
older routes are unchanged.

## 3. The QoS suite (archive: QoS-suite-0.5.1)

Unzip into the same Mods folder. The archive creates five folders
alongside CoiBridge:

    QoSCore  QoSChickens  QoSVehicleRecovery
    QoSPriorityActuator  QoSWorldMap

QoSCore must be present — the other four depend on it.

**WARNING — replacing the feature folders is not optional.** Suite
0.5.1 carries the three daily feature mods at version 0.1.2, which fix
a save-breaking bug: the 0.1.1 versions of QoSChickens,
QoSVehicleRecovery and QoSPriorityActuator make the game fail to save
(their calendar subscriptions could not be serialized). Delete the old
0.1.1 folders and put the 0.1.2 folders in their place — never keep
both versions side by side. Saves written by the fixed versions need
these mods present to load; the game will refuse removal rather than
corrupt a save.

## 4. The QoSBuilder mod (archive: QoSBuilder-mod-0.2.0)

A separate archive. Unzip into the same Mods folder; it creates one
more folder:

    QoSBuilder

QoSBuilder places whole blueprints (buildings, belts and pipes in one
batch) — but only from job files the companion writes after you
confirm a placement on its /place page. Version 0.2.0 also executes
**designation jobs**: flatten/dump pads and designation removals
planned on the companion's /designate page, run as a single batch in
the game. Job files of both kinds land in:

    %APPDATA%\Captain of Industry\QoS\jobs

The mod ships switched OFF; nothing executes until you enable it under
Settings. Each job is single-use, expires after 30 minutes, is pinned
to one specific saved game, and is hash-sealed — a job edited after
confirmation is refused and logged. For designations the mod only
reports *scheduled*; whether cells actually landed is verified by the
companion reading the designations back from the bridge (the verdict
panel on /designate), because the game itself does not report
silently refused cells.

## 5. Back in the game

1. Relaunch, and confirm mods are enabled under
   Settings → Miscellaneous.
2. Load your save.
3. Start the web API from the in-game console (it must be re-run
   after every game start).
4. Every QoS feature ships switched OFF. Enable them individually
   under Settings when ready — Chickens first is the gentle start.
