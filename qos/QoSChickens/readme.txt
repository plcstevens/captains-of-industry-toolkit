QoS Chickens v0.1.1 — animal farm distribution for the QoS suite.
Author: Lewis Cameron. For Captain of Industry v0.8.8 (Update 4.3).
Requires: QoSCore >= 0.1.0 (install it first, or use the combined
QoS suite pack, which contains both folders).

WHAT IT DOES
Once per in-game day (at day end, after that day's births and
slaughter culls), the mod looks at your animal farms, grouped by
species, and schedules the game's OWN animal-move command to even
out the distribution — the same MoveAnimalsCmd the UI's +50/-50
buttons issue, with the same 1-by-1 round-robin spread inside each
move. The mod adds policy (when, where, how much, and when NOT
to), not a new movement mechanism.

WHY: the game's breeding (a per-farm percentage of ITS OWN count
every 30 days) and quick trades (animals added to whichever farm
has room) skew distribution over time — the fattest farm grows
fastest and can slam into its slaughter limit while a sister farm
sits nearly empty.

HARD GUARDS (always on, cannot be configured away)
- Never move animals INTO a farm at or above its slaughter limit;
  no farm ends a move above min(capacity, slaughter limit).
- Never touch a farm that is not fully constructed and enabled.
- Growth-paused farms: the game's own move command cannot exclude
  a farm from its peer set, so if ANY constructed + enabled farm
  of a species is growth-paused, that species is skipped for the
  day (a note goes in the log). Nothing is moved behind the back
  of a farm you told to stay put.
- Never drain a farm below the breeding-nucleus floor (config).
- Every candidate move is simulated exactly (the same round-robin
  the game will run) and only scheduled if the predicted outcome
  satisfies all guards; the scheduled amount is the simulated
  amount.

INSTALL
1. Close the game completely.
2. QoSCore must be installed (see its readme) — this mod declares
   a dependency on it and the game will report an error without it.
3. Unzip so the QoSChickens folder lands in:
      %APPDATA%\Captain of Industry\Mods\
   Final layout:
      %APPDATA%\Captain of Industry\Mods\QoSChickens\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSChickens\QoSChickens.dll
      %APPDATA%\Captain of Industry\Mods\QoSChickens\config.json
4. Launch the game, load your save, tick QoSChickens in the
   save's mod list if asked.
5. The mod ships DISABLED. When ready: Settings -> QoS Chickens,
   set 'enabled' on. Balancing starts at the next day end.

DEFAULTS (change them in Settings -> QoS Chickens)
  enabled                     false   master switch (ships OFF)
  mode                        equalise
                              equalise  = converge same-species
                                farms to within +/-1 of each other
                              fill_first = fill the farms listed in
                                fill_first_farm_ids, in that order,
                                each to capacity / slaughter limit
                              off       = do nothing
  max_animals_moved_per_day   50      total cap per in-game day,
                                      all species combined (one
                                      vanilla UI button-press)
  breeding_nucleus_floor      10      never drain a farm below this
  fill_first_farm_ids         (empty) entity ids, comma-separated
                                      (fill_first only)
  log_moves                   true    one log line per scheduled
                                      move, prefixed QoS.Chickens:

ACTIVITY RECORD
Every move this mod schedules is recorded to the QoS activity feed
(%APPDATA%\Captain of Industry\QoS\events-v1.jsonl) by QoSCore,
and shows on the CoI Companion website's Log page. Controlled by
QoSCore's activity_log_enabled setting (default on).

REMOVAL SAFETY
The mod registers no prototypes, recipes or entities and stores
nothing in your save beyond its own settings values (inert once
the mod is gone). Delete the QoSChickens folder to remove it;
animal counts in your save are untouched vanilla state.
