QoS Priority Actuator v0.1.2 — bounded priority nudges for
starved machines, for the QoS suite. Author: Lewis Cameron.
For Captain of Industry v0.8.8 (Update 4.3).
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
Once per in-game day (at day end), the mod reads every
machine's own state and acts on exactly one disease:

  STARVED — a machine stuck at NotEnoughInput (its inputs are
  not arriving) for several days running gets its general
  priority stepped UP one step, so trucks serve its cargo
  jobs sooner. In the game's values a LOWER number is served
  sooner (highest 0, default 9), so "stepped up" means the
  number goes down by one. If it stays starved, later days
  may step it once more, down to a configured floor. When
  the machine is Working again, its priority is stepped
  back toward its proto default, one step per day.

All actuation is the game's OWN SetGeneralPriorityCmd, the
same command behind the priority control in a machine's
inspector, scheduled through the game's input scheduler.
The mod adds timing and restraint, never a new mechanism.
It is the actuator counterpart to the companion's
productivity alerts: the companion names causes with
evidence; this mod applies the one remedy priority can
actually deliver, in bounded steps, with a log line per
action you can audit against the following days.

WHAT IT NEVER DOES
- BLOCKED machines (OutputFull) are never adjusted. A full
  output is a downstream problem; priority cannot fix it,
  and because a machine's general priority steers ALL its
  cargo jobs, stepping a blocked machine down would also
  deprioritise the trucks that empty it. Blocked machines
  are only observed (verbose log).
- PARKED machines (paused by you, disabled, or reporting
  the Paused state) are never touched and never counted.
  Your pause is a decision, not a disease.
- Machines short of workers, power or computing, broken,
  missing a recipe, or invalidly placed are never touched —
  cargo priority cannot fix any of those.

THE OWNERSHIP RULE (how it avoids fighting you)
The mod keeps NO memory of what it set, and does not need
any: safety is computed from the live value every day.
Let D be a machine's proto default priority (shown in its
inspector; 9 for most machines) and N the step band
(max_nudge_steps, default 2). The mod may adjust a machine
only while its current priority is one of D, D-1, ..., D-N
— values its own rule could have produced. Anything else
(a lower value than D-N, or anything above D) is treated
as set by you and is left alone forever, in both
directions: it will not nudge it, and it will not
"restore" it either.

One honest imperfection: if YOU set a machine to exactly
D-1, the mod cannot tell that apart from its own nudge —
if that machine then works, it may step your value back
toward D by one. The small band bounds the worst case to
N steps, every step is in the log, and setting any value
outside the band (say D-3 at the default band, or anything
above D) takes the machine out of the mod's reach entirely.

BOUNDS (all in Settings -> QoS Priority Actuator)
  persistence_days   3   consecutive starved days observed
                         before the first nudge
  max_nudge_steps    2   band below the proto default
  cooldown_days      2   minimum days between nudges of
                         the same machine
  max_actions_per_day 10 nudges + restores per day, total
No exact simulation of the logistics response exists (it
is systemic), so the safety class is bounded nudges +
stateless ownership + restore — not prediction. Every
scheduled action logs its prediction so the effect can be
judged from the following days.

INSTALL
1. Close the game completely.
2. QoSCore must be installed (see its readme) — this mod
   declares a dependency on it and the game will report an
   error without it.
3. Unzip so the QoSPriorityActuator folder lands in:
      %APPDATA%\Captain of Industry\Mods\
   Final layout:
      %APPDATA%\Captain of Industry\Mods\QoSPriorityActuator\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSPriorityActuator\QoSPriorityActuator.dll
      %APPDATA%\Captain of Industry\Mods\QoSPriorityActuator\config.json
4. Launch the game, load your save, tick QoSPriorityActuator
   in the save's mod list if asked.
5. The mod ships DISABLED. When ready: Settings -> QoS
   Priority Actuator, set 'enabled' on. Passes start at
   the next day end.

DEFAULTS (change them in Settings -> QoS Priority Actuator)
  enabled             false  master switch (ships OFF)
  nudge_starved       true   step up persistently starved
                             machines
  restore_when_working true  step back toward proto default
                             while a machine works
  persistence_days    3      starved days before a nudge
  max_nudge_steps     2      band below the proto default
  cooldown_days       2      days between a machine's nudges
  max_actions_per_day 10     nudges + restores per day
  log_actions         true   one log line per scheduled
                             action, prefixed
                             QoS.PriorityActuator:

ACTIVITY RECORD
Every priority nudge or restore this mod schedules is recorded to the QoS activity feed
(%APPDATA%\Captain of Industry\QoS\events-v1.jsonl) by QoSCore,
and shows on the CoI Companion website's Log page. Controlled by
QoSCore's activity_log_enabled setting (default on).

REMOVAL SAFETY
The mod registers no prototypes, recipes or entities. It does
leave the priority values it changed through the game's own
command — those are ordinary machine settings afterwards,
visible and editable in each machine's inspector, and any
machine still inside the band simply keeps the value it had.
One honest exception since 0.1.2: the service's calendar
subscription is serialized into saves written while the mod is
installed (that is how the game's saveable calendar events
work), so the manifest declares can_remove_from_saved_game =
false — if a save made with this mod installed is loaded
without it, the game will refuse with a clear message instead
of risking a damaged save. To remove the mod entirely, delete
its folder while playing a save that predates installing it
(or just leave the folder in place: it ships disabled and does
nothing until enabled). Its observation bookkeeping is session
memory only.

FIRST LIVE CHECKS (unverified until first install)
- The "actuator armed" line appears in the log at load, and
  QoSCore resolves across the manifest dependency (the
  suite-wide first-install check).
- On the first enabled days, compare the log's predicted
  steps against the machines' inspector values the next
  day, and confirm a machine you set OUTSIDE the band is
  never touched.
