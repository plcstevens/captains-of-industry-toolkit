QoS Builder — companion blueprint placement (v0.1.0)
Requires: QoSCore >= 0.2.0 (install QoSCore first, or use the
suite pack when it includes this mod).

WHAT THIS MOD IS
The game's web API can never carry the commands that place
belts, pipes and blueprints (they are excluded from its JSON
command index by construction). QoS Builder is the companion
website's pair of hands inside the game instead: the website's
"Place blueprint" flow ends in a confirmation, and ONLY that
confirmation writes a job file. This mod executes job files —
nothing else can make it place anything.

THE JOBS FOLDER (exact path)
  %APPDATA%\Captain of Industry\QoS\jobs\
On this PC that is:
  C:\Users\cokof\AppData\Roaming\Captain of Industry\QoS\jobs\
The folder sits next to the QoS activity feed (..\QoS\
events-v1.jsonl). The companion writes <jobId>.json files into
it at confirm time. Executed jobs are renamed .done, refused
jobs .failed, and every consumed job id is appended to
processed.txt in the same folder.

THE GUARANTEES, IN PLAIN WORDS
- Confirmed-only. No file, no placement. The file is written by
  the companion at the moment you click Confirm on the website,
  and by nothing else in the toolkit.
- Single-use. A job is claimed (renamed) and its id recorded in
  processed.txt BEFORE anything is scheduled, so even a crash
  mid-placement can never run it twice. Re-delivered copies of a
  consumed job are filed away untouched.
- Expiring. A job dies 30 minutes after it was confirmed. If the
  game is paused or closed past that, confirm again — a stale
  job can never fire days later.
- Pinned to one game. Every job carries the gameId it was
  confirmed against (the id the game's own API reports). If the
  running game's id differs, the job is refused and consumed —
  a job written for a sandbox can never fire on your real base,
  or the other way round.
- Untampered. Each job carries a SHA-256 of its contents as
  confirmed. Any later edit — a changed anchor, a swapped
  blueprint — fails the check and the job is refused.
- Capped. A job names its own item cap (500 from the website);
  the mod's max_items_per_job setting can only lower it.
- Honest reporting. The mod records claimed / scheduled /
  refused (with the reason) to the QoS activity feed, which the
  companion shows on its Log page. "Scheduled" means the game's
  batch command was handed to the game — the batch itself is
  per-item (one bad item errors, the rest still place), so the
  placement VERDICT is the companion's before/after census on
  the next snapshot, never the mod's word alone.

HOW PLACEMENT WORKS
At every in-game day end (if enabled), the mod scans the jobs
folder. A valid job becomes ONE batch placement: buildings,
belts and pipes appear as ordinary construction sites — nothing
is charged at placement, trucks deliver the construction
materials as usual, and sites you don't want can be removed
like any other. Blueprint configuration (recipes, storage
products, priorities) is applied, as with a normal in-game
paste. Surfaces and decals inside a blueprint string are NOT
placed by v1 (they are counted in the scheduled event instead).

Limits of job schema v1: rotations are quarter-turns
(0/90/180/270) about the blueprint's own origin; mirrored
placement is refused; train-track items place only at
rotation 0; a string that decodes to a folder is refused
(single blueprints only).

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
  max_items_per_job   500     hard cap, whatever a job asks
  log_jobs            true    one log line per job outcome

ACTIVITY RECORD
Every claimed, scheduled and refused job is recorded to the QoS
activity feed (%APPDATA%\Captain of Industry\QoS\events-v1.jsonl)
by QoSCore, and shows on the CoI Companion website's Log page.
Controlled by QoSCore's activity_log_enabled setting (default
on).

REMOVAL — READ THIS ONE
The mod registers no prototypes, recipes or entities, and what
it places is ordinary construction sites. BUT: like the other
calendar-driven QoS mods it subscribes to the game's day-end
event, and that subscription is written into any save made
while the mod is loaded. Such a save needs this mod present to
load again — that is why the manifest says
can_remove_from_saved_game: false. Keep the folder (disabled
is fine — while disabled it does nothing at all). Saves made
without the mod never reference it and are unaffected.
