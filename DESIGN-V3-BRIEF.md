# Design V3 — Philip's brief (2026-11-09)

The v3 round is a full UX/UI review + implementation, graded against the
principles below (Philip's words, structured). It carries the
Superintendent rename. It runs AFTER the privacy cleanup round and
BEFORE the daemon round and the demo-data screenshots.

## The six principles

1. **Every page has a declared intent.** What the page is FOR, stated
   in the product itself (a one-line purpose a first-time player can
   read), not just in design docs. If a page can't state its job in
   one line, the page is wrong.
2. **Data volume is deliberate; pages own domains.** Each page holds
   the data of ONE domain (stocks, machines, terrain, research…).
   No domain scattered across three pages, no page stuffed with
   three domains. The amount of data on a page is a decision, not
   an accident.
3. **Navigation is obvious to anyone.** Moving between pages — and
   understanding where you are, where a thing lives, and how to get
   back — must need no learning. A player who has never seen the
   tool should predict where a given fact lives.
4. **The UI knows what data it has — and what it doesn't.** Every
   page behaves honestly per data state: full data, partial data
   (bridge missing, game off, no history yet), and no data. Absent
   data is shown as absent with the reason and the remedy — never
   as a fake zero, a silent gap, or a stale number dressed as live.
5. **Glanceable first, depth on demand.** The first screen of any
   page answers its question at reading distance; depth (tables,
   histories, exact figures) is one deliberate step further in,
   never the entry point and never missing.
6. **Actionable vs informational is explicit.** Every element is
   clearly one or the other. Actionable things carry a real action
   (and land somewhere safe — prepare/detail, never one-click
   applies). Informational things don't masquerade as tasks.
   Pages declare their balance: a page that exists to inform
   shouldn't nag; a page that exists to act shouldn't bury the act.

## The guidance layer (new in v3) — its OWN top-level nav domain

Philip (2026-11-09 10:31): guides/tutorials are not a sub-page of
something else — they get a separate top-level navigation domain,
peer to the monitoring/action domains, holding THREE classes of
content, clearly separated inside the domain:

1. **The game itself** — how Captains of Industry works (the
   existing guides' territory). Where the official wiki already
   does a topic well, LINK OUT to it (labelled external) instead of
   duplicating; keep our own text for what we have verified better,
   for worked numbers, and for anything the wiki lags on.
2. **This tool itself** — the Superintendent manual: what each part
   of the tool does, how the two-phase actions work, how to run it,
   what its data means.
3. **Why it suggests stuff** — the reasoning layer (below): why an
   action is recommended, what it is trying to achieve, and the
   play philosophy behind the whole tool.

Build out this domain — in the product, for players — covering:

- WHY each class of action is suggested (per issue class: what the
  companion saw, what threshold fired, what the suggested action
  achieves, what happens if you ignore it).
- WHAT the guidance is trying to do overall: the play philosophy
  behind the recommendations (survival first, buffers, no starving
  chains — the strategy content, written for players, not devs).
- Plan gates explained the same way: what a gate is checking and
  why that number.

This extends the existing Guides pages from game knowledge toward
*the companion's own reasoning*, in the same register.

## One canon, three renderings (Philip, 2026-11-09 10:31)

The guidance is "almost as if it is guidance for THIS agent to
utilise when it is running" — because it is. The same body of
reasoning exists today in three drifting copies:

- **Player-facing**: the Guides pages and the issue fix/hint text.
- **Agent-facing**: the local-coi-* skills, BEHAVIOURS.md, the
  strategy documents — what a Muse uses when it plays a run
  through the companion.
- **Executable**: the issue thresholds, plan gates and runner rules
  compiled into the engine.

V3 treats these as ONE canon with three renderings, authored from
a single structured source in the repo:

- The player reads it as the guidance domain (game / tool / why).
- The agent reads the same canon when it plays — and its moves are
  then auditable by the player against the published page ("the
  agent did X because the guidance says Y, here is the page").
- The executable thresholds cite the canon entry they implement,
  so a number in code traces to a written reason.

Where the three copies disagree today, the round reconciles them
into the canon and notes the disagreement rather than papering
over it. The canon lives in the repo as data/docs; the product
renders it; the skills point at it.

## Working method for the round

- Start from a written per-page audit against principles 1–6
  (intent, domain, data states, balance) BEFORE changing code;
  the audit is a deliverable Philip can read.
- Implement the changes; keep every v2 invariant that still earns
  its place (stat blocks, number semantics, §12 safety surfaces,
  no-external-assets).
- Verify with the page check scripts + a fresh screenshot pass on
  the synthetic captures.
- The Superintendent rename folds into this round (chrome, binary
  name, docs) — do not re-skin twice.
