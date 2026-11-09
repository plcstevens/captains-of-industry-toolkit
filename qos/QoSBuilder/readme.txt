QoS Builder — companion placement + designation jobs (v0.2.0)
Author: Lewis Cameron. For Captain of Industry v0.8.8 (Update 4.3).
Requires: QoSCore >= 0.2.0 (install QoSCore first; this mod
ships as its own zip, not inside the suite pack).

WHAT THIS MOD IS
The game's web API can never carry the commands that place
belts, pipes and blueprints, or that add terrain designations
(they are excluded from its JSON command index by construction:
they take collection arguments). QoS Builder is the companion
website's pair of hands inside the game instead: the website's
placement and designation flows end in a confirmation, and ONLY
that confirmation writes a job file. This mod executes job
files — nothing else can make it place or designate anything.

Two job families:
- BLUEPRINT JOBS (schema 1, since 0.1.0): place one blueprint —
  buildings, belts and pipes — as construction sites.
- DESIGNATION JOBS (schema 2, new in 0.2.0): add or remove a
  batch of terrain designations (mining / dumping / levelling)
  in ONE command — the pit, the dump pad, the flatten plan you
  confirmed on the website's Designate page.

THE JOBS FOLDER (exact path)
  %APPDATA%\Captain of Industry\QoS\jobs\
(%APPDATA% is your per-user roaming folder — typically
  C:\Users\<your-name>\AppData\Roaming.)
The folder sits next to the QoS activity feed (..\QoS\
events-v1.jsonl). The companion writes <jobId>.json files into
it at confirm time. Executed jobs are renamed .done, refused
jobs .failed, and every consumed job id is appended to
processed.txt in the same folder.

THE GUARANTEES, IN PLAIN WORDS (both families)
- Confirmed-only. No file, no action. The file is written by
  the companion at the moment you click Confirm on the website,
  and by nothing else in the toolkit.
- Single-use. A job is claimed (renamed) and its id recorded in
  processed.txt BEFORE anything is scheduled, so even a crash
  mid-execution can never run it twice. Re-delivered copies of
  a consumed job are filed away untouched.
- Expiring. A job dies 30 minutes after it was confirmed. If the
  game is paused or closed past that, confirm again — a stale
  job can never fire days later.
- Pinned to one game. Every job carries the gameId it was
  confirmed against (the id the game's own API reports). If the
  running game's id differs, the job is refused and consumed —
  a job written for a sandbox can never fire on your real base,
  or the other way round.
- Untampered. Each job carries a SHA-256 of its contents as
  confirmed. Any later edit — a moved cell, a swapped
  blueprint — fails the check and the job is refused.
- Capped. A job names its own caps (500 items / 200 cells from
  the website); the mod's max_items_per_job / max_cells_per_job
  settings can only lower them.
- Honest reporting. The mod records claimed / scheduled /
  refused (with the reason) to the QoS activity feed, which the
  companion shows on its Log page. "Scheduled" means the game's
  command was handed to the game. The VERDICT is always the
  companion's before/after read-back, never the mod's word
  alone — see the designation note below for why that matters
  even more there.

HOW PLACEMENT WORKS
At every in-game day end (if enabled), the mod scans the jobs
folder. A valid blueprint job becomes ONE batch placement:
buildings, belts and pipes appear as ordinary construction
sites — nothing is charged at placement, trucks deliver the
construction materials as usual, and sites you don't want can
be removed like any other. Blueprint configuration (recipes,
storage products, priorities) is applied, as with a normal
in-game paste. Surfaces and decals inside a blueprint string
are NOT placed (they are counted in the scheduled event
instead).

Limits of job schema 1: rotations are quarter-turns
(0/90/180/270) about the blueprint's own origin; mirrored
placement is refused; train-track items place only at
rotation 0; a string that decodes to a folder is refused
(single blueprints only).

HOW DESIGNATION JOBS WORK
A designation job names ONE kind — mining, dumping or
levelling (the kind belongs to the command, not to each cell,
so a mixed plan is one job file per kind) — and a list of
4x4-tile cells: each cell is its origin tile plus the four
corner target heights in whole tiles. (The centre height is
never in the file: the game derives it from the corners, the
same as for designations you paint by hand.)
- ADD jobs run as one command. Adding at an origin REPLACES
  whatever designation is already there, of any kind — the
  website's confirm page lists every replacement before you
  confirm; the mod executes the confirmed plan as written.
- REMOVE jobs clear whatever designation sits at each listed
  origin, any kind. Duplicate origins in a remove job are
  simply deduplicated; in an ADD job they are refused (the
  game would silently let the last one win, and a sealed order
  must not contain that ambiguity).
- The mod checks cell sanity itself: origins must sit on the
  4-tile grid, corner heights within +/-1000 tiles (the game's
  own add path bounds nothing — this is the mod's discipline).
- READ-BACK IS THE VERDICT, MORE THAN EVER: the game's add
  command reports success even for individual cells it quietly
  refuses (for example a cell over certain buildings). The mod
  therefore attests only "scheduled"; whether each cell landed
  is the companion's before/after designation diff on the
  website.

INSTALL
1. Quit the game.
2. Unzip so this folder lands at:
     %APPDATA%\Captain of Industry\Mods\QoSBuilder\
   (manifest.json, QoSBuilder.dll, config.json, readme.txt)
3. Make sure QoSCore is installed the same way.
4. Launch the game, load your save, tick QoSBuilder in the
   save's mod list if asked.
5. The mod ships DISABLED. When ready: Settings -> QoS
   Builder, set 'enabled' on. Jobs run at the next day end.

DEFAULTS (change them in Settings -> QoS Builder)
  enabled             false   master switch (ships OFF)
  max_items_per_job   500     hard cap for blueprint jobs
  max_cells_per_job   200     hard cap for designation jobs
  log_jobs            true    one log line per job outcome

ACTIVITY RECORD
Every claimed, scheduled and refused job is recorded to the QoS
activity feed (%APPDATA%\Captain of Industry\QoS\events-v1.jsonl)
by QoSCore, and shows on the Superintendent website's Log page.
Controlled by QoSCore's activity_log_enabled setting (default
on).

REMOVAL — READ THIS ONE
The mod registers no prototypes, recipes or entities, and what
it places/designates is ordinary game content. BUT: like the
other calendar-driven QoS mods it subscribes to the game's
day-end event, and that subscription is written into any save
made while the mod is loaded. Such a save needs this mod
present to load again — that is why the manifest says
can_remove_from_saved_game: false. Keep the folder (disabled
is fine — while disabled it does nothing at all). Saves made
without the mod never reference it and are unaffected.
