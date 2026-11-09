# Installing the toolkit on the game PC

Downloads go to three destinations. Quit the game before swapping any
mod files — relaunch afterwards.

## 1. The companion program

Replace the old companion program file in the folder where you already
keep it — the same folder as its database file. Keep the database file:
it holds all snapshots and the audit history, and carries over.
Run the program by double-clicking it; it opens the local website in
your browser. (On this repo, the Actions tab's superintendent-release
workflow also builds the current program — Run workflow on main, then
download the artifact it produces.)

The current build is 6,610,173 bytes. It includes the /place page
(blueprint placement through the QoSBuilder mod, section 4 below),
the /designate page (terrain designation pads and removals through
the same mod, section 4 below), and the Design V3 interface — five
domains (Monitor / Workshop / Guidance / Plans / Records) over the
V2 design system: mega-nav top bar with the must-fix alert badge,
left-rail in-page navigation, a contract strip at every page head,
figure-based stats throughout, a Guidance domain (learn the game,
learn the tool, and why it suggests — rendered from the canon in
docs/canon/), and no login (the site is loopback-only and just
opens). The program itself is renamed: superintendent.exe (it was
coi.exe); its data files keep their names (coi.db, coi.toml) — and the Terrain page: a whole-map height view
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

## 6. Run it all the time (optional)

The website only exists while the program runs, and the tool's
evidence gets better the more snapshots it holds — so most players
leave it running. That is cheap: it listens on loopback only,
never contacts anything except your game, and at idle it uses
about 8 MB of memory and no measurable CPU (measured on the Linux
build, 2026-11-09; the Windows build is the same program). Every
recipe below starts the program with the `serve` command, which
runs the website *without* popping a browser window open at every
logon.

Where its data lives:

| OS | Database (default) | Settings (`coi.toml`) |
|---|---|---|
| Windows | `coi.db` beside the program | beside the program |
| Linux | `~/.local/share/superintendent/coi.db` (`$XDG_DATA_HOME` if set) | `~/.config/superintendent/coi.toml` (`$XDG_CONFIG_HOME` if set) |
| macOS | `~/Library/Application Support/Superintendent/coi.db` | same folder |

On Linux and macOS a `coi.db` already sitting in the folder you
start it from still wins, so a folder install keeps working. On
all systems `COI_DB` / `--db` and `COI_CONFIG` override these.

Stopping it, whichever recipe you used: the **Shutdown** control
at the right of the website's top bar. It is two steps — the first
click shows exactly what will happen, the second confirms — and
it exits cleanly: in-flight work finishes, the database is closed
properly, and your game is never touched. The OS-level stops
below work too, but the button is the clean one.

Reaching the site from another machine once it runs all the time
is the tunnel topic — see `REMOTE-ACCESS.md`, and read its
warnings first.

### Windows — Startup folder (simple)

1. Keep `superintendent.exe` in its folder, with `coi.db` beside
   it as now.
2. Press Win+R, type `shell:startup`, Enter — the Startup folder
   opens.
3. Right-click the program → Create shortcut, and move the
   shortcut into that Startup folder.
4. Open the shortcut's Properties and make two edits:
   - **Target:** add a space and then `serve` at the end, after
     the program path.
   - **Start in:** the folder the program lives in. Do not skip
     this — the database and `coi.toml` are found relative to it,
     and a shortcut with an empty Start in can start the program
     with a fresh, empty database somewhere else.

It now starts at every logon. To stop it for good, delete the
shortcut; to stop it once, use the Shutdown button.

### Windows — Task Scheduler (reliable)

1. Open Task Scheduler → Create Task (not "Create Basic Task").
2. **General:** name it `Superintendent`; select "Run only when
   user is logged on".
3. **Triggers:** New → "At log on", for your user.
4. **Actions:** New → "Start a program". Program: the full path
   to `superintendent.exe`. Add arguments: `serve`. **Start in:**
   the folder the program lives in — required, for the same
   database/settings reason as the shortcut above.
5. **Conditions:** on a laptop, untick "Start the task only if
   the computer is on AC power" if you want it on battery too.
6. **Settings:** untick "Stop the task if it runs longer than".

To stop it once: the Shutdown button, or Task Scheduler → right-
click the task → End. To stop it for good: disable or delete the
task.

### Linux — systemd user service

Put the program somewhere stable, e.g. `~/superintendent/
superintendent` (make it executable), then save this as
`~/.config/systemd/user/superintendent.service`:

    [Unit]
    Description=Superintendent — Captains of Industry companion
    After=network.target

    [Service]
    ExecStart=%h/superintendent/superintendent serve
    WorkingDirectory=%h/superintendent
    Restart=on-failure
    RestartSec=5

    [Install]
    WantedBy=default.target

Enable and start it:

    systemctl --user daemon-reload
    systemctl --user enable --now superintendent

`Restart=on-failure` is deliberate: a clean exit — the Shutdown
button — stays down until you start it again, while a crash is
restarted. If you want it running before you log in (a headless
box), `loginctl enable-linger $USER` makes user services start
at boot. Stop it with the Shutdown button or
`systemctl --user stop superintendent`.

One Linux note on the QoS mods: the game under Proton keeps its
data inside the game's Proton prefix, so the companion cannot
find the QoS jobs folder by itself. Point it there with
`[builder] jobs_dir` in `coi.toml`, set to the
`Captain of Industry\QoS\jobs` folder inside the prefix's
`drive_c/users/steamuser/AppData/Roaming`.

### macOS — launchd agent (build available, not yet run-tested)

Honest status: the macOS build is produced by the release
workflow, but nobody on this project has a Mac — the build is
**not run-tested**, and this recipe follows Apple's standard
shapes rather than our own scars. Reports welcome.

Save this as `~/Library/LaunchAgents/com.coi.superintendent.plist`
(adjust the program path to where you put the binary):

    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
      "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
      <key>Label</key>
      <string>com.coi.superintendent</string>
      <key>ProgramArguments</key>
      <array>
        <string>/Users/you/Applications/Superintendent/superintendent</string>
        <string>serve</string>
      </array>
      <key>WorkingDirectory</key>
      <string>/Users/you/Applications/Superintendent</string>
      <key>RunAtLoad</key>
      <true/>
      <key>KeepAlive</key>
      <false/>
      <key>StandardOutPath</key>
      <string>/Users/you/Library/Logs/superintendent.log</string>
      <key>StandardErrorPath</key>
      <string>/Users/you/Library/Logs/superintendent.log</string>
    </dict>
    </plist>

Load it once:

    launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.coi.superintendent.plist

`KeepAlive` is deliberately `false`: a clean exit — the Shutdown
button — stays down. Stop it with the Shutdown button or
`launchctl stop com.coi.superintendent`; remove it with
`launchctl bootout gui/$(id -u)/com.coi.superintendent` and
delete the plist.
