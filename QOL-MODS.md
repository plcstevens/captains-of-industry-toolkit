# QoL mod ideas — Philip, 2026-11-04 (from London, by phone)

Quality-of-life mods for the game itself (a step beyond the read-only
bridge: these change gameplay/UI, so each needs its own go-ahead).

## 1. Better chicken distribution
Distribute chickens (animals) across animal farms more sensibly —
e.g. even distribution / fill rules instead of manual per-farm
wrangling. Exact pain point to confirm with Philip.
Recon needed: animal farm surfaces, animal assignment mechanics,
the Farms command family (slaughter limits etc. already catalogued).

## 2. World map dropdown for mines/rigs
A dropdown list of world-map mines and oil rigs to select/interact
with, instead of clicking each one around the map individually.
This is a UI mod — a different discipline from the data bridge
(Unity UI layer, not web routes). Biggest of the three.

## 3. Trains: remove cargo with Unity
Pay Unity to instantly clear a train's cargo (the stuck/wrong-product
wagon problem). Gameplay-changing: needs train/wagon cargo surfaces
+ a Unity-cost hook. Recon first: check whether vanilla already has
a cargo-discard path for trains (vehicles have related commands).

Status: ideas filed 2026-11-04; CHALLENGED + REFINED + RECONNED 2026-11-04 → see QOL-MOD-PREP.md (§1–§3) and qol-recon/01–03. No code yet; each mod still needs Philip's go-ahead.

## 4. Train network advisor (Philip, 2026-11-04 21:24)
Networks suggest additions and stations suggest themselves:
- Detect producer/consumer gaps: a station (or product flow) with an
  output that no network serves, or an input with no supply route —
  suggest the line/network that would close the gap, plus other
  matching producer/consumer pairs ("additional matching cases").
- Unassigned stations propose themselves to networks whose cargo
  profile matches what the station handles.
- Utilisation check: is a network heavily used — are its trains
  always busy or mostly idle — factoring train counts relative to
  what they carry; suggest adding/removing trains or lines.
Scoping note (Claudius): the analysis core fits the COMPANION, not
only an in-game mod — topology, stations, lines, flows and the
v0.6.4 raw train counters are already exposed. Presentation could
be a companion page first; in-game UI later if wanted. The
utilisation leg depends on train-counter decoding being verified
live, which needs trains running (his save has zero lines so far).
Status: CHALLENGED + REFINED + RECONNED 2026-11-04 → QOL-MOD-PREP.md
§4 (three layers: L0 ready today, L1 ready as a bridge round,
L2 blocked on live trains) and qol-recon/04-advisor.md.

## 5. Productivity alerts (Philip, 2026-11-04 21:25)
Alert when a factory's productivity is LOW over a month or a year
(high-productivity alerts a bonus). Measurement, not vibes.
Scoping note (Claudius): companion-native — this is the Issues layer
growing a time dimension. Two measures available from existing data:
(a) working-state share across snapshots over the window (machine
census/machines states: Working vs NotEnoughInput/OutputFull/paused),
(b) actual output vs nameplate from product flows + dataset rates.
Dependency: a snapshot cadence — a month/year window is only as good
as the snapshot series inside it (the watch/snapshot habit matters).
Status: CHALLENGED + REFINED + AUDITED 2026-11-04 → QOL-MOD-PREP.md
§5 and qol-recon/05-productivity.md. Headline: the store holds ONE
paused snapshot and no machine states (coi watch was never built);
estimator (b) κ-free form ships first, estimator (a) needs the
store prerequisite + a month of cadence.

## Architecture decision — Philip, 2026-11-04 22:03
Build SMALL INDIVIDUAL MODS that link into a single QoS suite.
Sub-mod dependencies are explicitly OK where that's how a feature
is best achieved (e.g. a shared core mod the feature mods depend
on). CoiBridge stays separate and read-only; the QoS suite is the
gameplay/UI side.

## Philip, 2026-11-05 11:54 — new build asks
6. **QoS vehicle CONTENT recovery** (build): stuck vehicles get their
   CARGO recovered, not just the vehicle — content preserved back
   into the economy (storage / preferred drop-off), Unity-bounded.
7. **Preferred drop-off for overload**: a train (or its depot) can
   name a PREFERRED overflow drop-off for when the primary
   destination cannot take everything.
8. **Same routing for auto-empty**: whenever cargo is auto-emptied
   or cleared, it goes to the preferred drop-off (per #7) rather
   than vanishing or dumping globally.
