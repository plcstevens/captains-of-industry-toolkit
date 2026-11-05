# Installing the toolkit on the game PC

Three downloads, three destinations. Quit the game before swapping any
mod files — relaunch afterwards.

## 1. The companion program

Replace the old companion program file in the folder where you already
keep it — the same folder as its database file. Keep the database file:
it holds your website password and all snapshots, and carries over.
Run the program by double-clicking it; it opens the local website in
your browser. (On this repo, the Actions tab's companion-release
workflow also builds the current program — Run workflow on main, then
download the artifact it produces.)

## 2. The CoiBridge mod (archive: CoiBridge-mod-0.6.4)

Unzip into the game's Mods folder:

    %APPDATA%\Captain of Industry\Mods\

The archive contains a single CoiBridge folder. Replace the old
CoiBridge folder there completely — do not merge old and new files.

## 3. The QoS suite (archive: QoS-suite-0.4.0)

Unzip into the same Mods folder. The archive creates five folders
alongside CoiBridge:

    QoSCore  QoSChickens  QoSVehicleRecovery
    QoSPriorityActuator  QoSWorldMap

QoSCore must be present — the other four depend on it.

## 4. Back in the game

1. Relaunch, and confirm mods are enabled under
   Settings → Miscellaneous.
2. Load your save.
3. Start the web API from the in-game console (it must be re-run
   after every game start).
4. Every QoS feature ships switched OFF. Enable them individually
   under Settings when ready — Chickens first is the gentle start.
