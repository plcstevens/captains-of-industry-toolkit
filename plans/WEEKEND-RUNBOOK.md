# WEEKEND RUNBOOK — supervised fresh-start session (Update 4.3)

Plan: `fresh-start-v1` **v1.2.2** — New Haven, normal, game 0.8.8.
Posture: Claudius prepares and reports; **Philip confirms every apply**
in the companion website (§12 flow). The runner is prepare-only by
construction — nothing reaches the game except through a confirmed
website action or a confirmed QoSBuilder job.

## 0. Gate zero — prove saving works before playing

The QoS 0.1.1 calendar mods break saving (serializer bug); suite
**0.5.1** is the fix, and its live proof is a save **and a reload**:

1. Quit the game. Swap mod folders: CoiBridge 0.6.4 (already in),
   QoS suite **0.5.1** (replace the 0.1.1 feature folders wholesale —
   never keep them alongside), QoSBuilder 0.1.0 alongside.
2. Start a **new game**: New Haven, normal difficulty (the executor
   refuses a map/difficulty/version mismatch — §3.1).
3. **Save immediately, then reload that save.** Both must succeed
   before anything else happens. If the reload fails, stop and
   report — do not play on an unproven stack.
4. Settings → Miscellaneous: mods enabled. QoS features stay **OFF**
   (they ship off) until step 5. QoSBuilder's `enabled` stays off
   until step 4.

## 1. Bring the channels up (Philip, ~2 minutes)

- Console: `web_api_start` (needed after every game start).
- Tailscale serve on (the VM reaches the game only through it).
- Start `coi.exe` → website at http://127.0.0.1:8484 (login
  `captain`; the existing `coi.db` next to the exe carries over).

## 2. First capture (Claudius)

- Pull game-info / status / research / census / products; take the
  first snapshot into the companion store. Record the new **gameId**
  (the run's identity — a change mid-run halts the loop) and confirm
  the game reports 0.8.8.
- Phase-0 verification against the plan's assumptions, reported
  before unpausing: starting diesel > 0 (the globals invariant is
  armed on it), scrap / concrete / copper opening quantities,
  population + housing baseline, researchedCount baseline.

## 3. Phase 0 with Philip's eyes

- Philip does the scout manual-step in-game: Layers view — oil patch
  bearing/distance, coal, limestone, iron distance, coastline/docks
  south. Claudius records it; **nothing is placed yet**.
- Philip queues the first-15 research list in the research UI —
  while queueing, read the real node id shown for **"Construction"**
  (the plan's one §4.5 placeholder; sole candidate
  `ResearchCp1Production`). That single reading retires the
  placeholder by plan amendment.
- Claudius prepares phase-0/1 command orders in the website
  (pause, pins); Philip confirms each; Claudius verifies post-state
  from live reads, never from the "ok".

## 4. First builder job — a small starter layout

Default (zero-risk) order of experiments:

- **(Optional, 5 min, throwaway save)** One `concrete-v1` job on a
  scratch save to watch the whole circuit once: /place prepare →
  confirm → job file → day-end claim → Log events → census diff.
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

## 5. Only after the above

- Enable **one** QoS feature (Chickens is the gentle start) and let
  a game day pass; watch its actions in the Log.
- Then settle into the loop: observe → runner pass over the live
  sequence → prepare due orders → Philip confirms → verify →
  snapshot at every phase checkpoint.

## What to watch in the Log

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
