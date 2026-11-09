# WEEKEND RUNBOOK v2 — supervised fresh-start session (Update 4.3)

Plan: `fresh-start-v1` **v1.3.0** — New Haven, normal, game 0.8.8.
Bridge: **CoiBridge 0.6.5**. Companion: current staged `coi.exe`.
Posture (Philip, 2026-11-06): "You do it all yourself — the point is
you play." Claudius does everything the VM can reach: proof reads,
captures, snapshots, runner passes, job preparation, the play loop,
Log watching. Philip's part is the physical minimum below, plus the
clicks that are his **by posture, not by necessity**: every §12
confirm in the website (confirm-each-step, decided 2026-11-05) and
the in-game acts no channel can perform for him.

**v2 change:** the plan's last research placeholder is retired.
"Construction" is `ResearchCpPacking` — sourced from the shipped
game's own proto registrations (plan v1.3.0 changelog has the full
evidence, incl. five sibling retirements and one corrected id,
ResearchRepairDock). Nobody reads node ids off the research screen
any more; the first-15 queue below names real ids throughout.

## PHILIP — physical minimum

Do these, in this order. Everything else is Claudius's.

1. **Files onto the PC:** quit the game; copy `dist/weekend-bundle/`
   across; replace `coi.exe` in its existing folder (keep `coi.db`
   beside it); unzip the three mod zips into
   `%APPDATA%\Captain of Industry\Mods\`, replacing the `CoiBridge`,
   `QoS*` and `QoSBuilder` folders wholesale — **never keep the old
   0.1.1 QoS folders alongside the 0.5.1 ones** (they break saving).
2. **Hub installs (in-game Mods browser):** Auto Terrain
   Designations · Auto Forestry Designations · Research Queue ·
   Terrain Grid Only.
3. **Game started, mods on:** launch the game; Settings →
   Miscellaneous → mods enabled.
4. **Web API:** open the console, run `web_api_start` (needed after
   every game start).
5. **Tailscale forward:** `tailscale serve --bg --tcp=3073 tcp://localhost:3073`
6. **Companion:** start `coi.exe` → website at
   <http://127.0.0.1:8484> (no login — the site just opens; existing
   `coi.db` carries over — snapshots and audit history intact).

Then say "up" in chat. From that line on, Claudius drives.

## CLAUDIUS — everything else, in order

### A. Shakedown on a scratch save (BEFORE the fresh run)

The four Hub mods are unproven on this stack; ATD declares its own
4.3 support *experimental*. They earn their place on a throwaway
save first, so a bad interaction costs a scratch colony, never the
real run.

1. Philip starts any throwaway game (any map) and saves it once.
2. Claudius verifies from live reads: all routes answer, census
   sane, no errors in the Log after a game day passes with the
   Hub mods loaded.
3. ATD smoke test: Philip paints one small mining designation in
   the scratch save; Claudius confirms via `/coi/v1/designations`
   that it registers and reads back with sane target heights.
4. Research Queue smoke test: Philip queues two nodes; Claudius
   confirms the queue reads correctly and vanilla research still
   advances.
5. Any misbehaviour → that mod is dropped from the weekend stack;
   the fresh run proceeds without it. Terrain Grid Only is
   cosmetic — drop on any doubt.

### B. Gate zero — prove saving works (fresh run)

The QoS 0.1.1 calendar mods broke saving (serializer bug); suite
0.5.1 is the fix, and its live proof is a save **and a reload**.
This proof is still owed.

1. Philip starts the **new game**: New Haven, normal difficulty
   (the executor refuses a map/difficulty/version mismatch — §3.1).
2. Philip **saves immediately, then reloads that save.** Both must
   succeed before anything else happens. If the reload fails:
   stop, report, do not play on an unproven stack. (The clicks are
   Philip's because only he is at the machine; Claudius verifies
   the reloaded state from live reads the moment the API is back.)
3. QoS features stay **OFF** (they ship off) until section E.
   QoSBuilder's `enabled` stays off until section D.

### C. First capture + phase 0

1. Claudius pulls game-info / status / research / census /
   products / terrain + deposits; takes the first snapshot into
   the companion store. Records the new **gameId** (the run's
   identity — a change mid-run halts the loop) and confirms the
   game reports 0.8.8.
2. Phase-0 verification against the plan's assumptions, reported
   before unpausing: starting diesel > 0 (the globals invariant is
   armed on it), scrap / concrete / copper opening quantities,
   population + housing baseline, researchedCount baseline.
3. **Scout (Philip's eyes, Claudius records):** Layers view — oil
   patch bearing/distance, coal, limestone, iron distance,
   coastline/docks south. Nothing is placed yet.
4. **First-15 queue (Philip, one UI pass):** queue the plan's
   first-15 list in the research UI. Every id is now sourced
   (plan v1.3.0): Tools → Vehicle & Mining → Trading Dock →
   Construction (`ResearchCpPacking`) → Power & Maintenance
   (`ResearchPowerAndMaintenance`) → … — the full annotated list
   is the `p1-research-queue-first15` step in the plan.
5. Claudius prepares phase-0/1 command orders in the website
   (pause, pins, research starts); Philip confirms each (§12);
   Claudius verifies post-state from live reads, never from the
   "ok".

### D. First builder job — a small starter layout

Default (zero-risk) order of experiments:

- **(Optional, 5 min, on the scratch save)** One `concrete-v1`
  job to watch the whole circuit once: /place prepare → confirm →
  job file → day-end claim → Log events → census diff.
- **(The real first job)** Phase 1's starter-cluster core:
  **`iron-v1`** (26 items) via /place, at an anchor Philip picks
  from the scout, rotation as authored. QoSBuilder `enabled` on
  first. The job is single-use, expires in 30 minutes, and is
  pinned to this save's gameId — it cannot fire anywhere else.
- Watch it land: Log tab shows `JobClaimed` then `JobScheduled`;
  the verdict is the **next snapshot's census diff** (the mod
  attests *scheduled*, the census proves *placed*). Sites are
  ordinary construction sites — nothing charged at placement, and
  a misplaced site is deconstructable.
- The rest of phase 1 (coal maker, diesel generator, 2 labs) is
  manual single-building placement by Philip — no pack asset
  contains them.

### E. Only after the above

- Enable **one** QoS feature (Chickens is the gentle start) and let
  a game day pass; Claudius watches its actions in the Log.
- Then settle into the loop: observe → runner pass over the live
  sequence → prepare due orders → Philip confirms → verify →
  snapshot at every phase checkpoint.

## What to watch in the Log (Claudius)

- Every companion apply (exact envelope, outcome) — the audit trail.
- QoS events: `JobClaimed` / `JobScheduled` / `JobRefused` (with
  reason) from QoSBuilder; feature-mod actions once enabled.
- Silence is a signal too: a claimed job with no census change on
  the next snapshot means the batch partially failed — the census
  diff names what is missing.

## Stop conditions (any one halts the session, game left paused)

- A globals invariant goes FALSE (starving / homeless / diesel = 0).
- gameId changes, version mismatch, or a catalogue command missing.
- Save or reload misbehaves in any way.
- Anything on screen that neither of us can explain — pause first,
  diagnose second.
