# CoI Mod Ecosystem Survey — 2026-11-06

Philip's ask (2026-11-06): explore all major popular mods, with a
focus on the mine-structuring guidance that exists as mods, and say
what we should **use / re-use / change** against our own stack.

Method: three parallel research passes (ecosystem enumeration,
mining/terrain deep-dive, general/QoL engineering posture), every
number read off a live page or repo on 2026-11-06 and cited.
Popularity = Hub downloads / comments / % positive (the Hub lists
them in that order), GitHub stars/forks where source exists. Where
a fact could not be verified it is marked UNVERIFIED — see the
register at the end. Nothing was installed; no code was changed.

---

## 1. The ecosystem at a glance

**There is exactly one distribution channel that matters: MaFi's
own CoI Hub** (https://coigame.com/Mods, a.k.a. hub.coigame.com).
Since Update 4.2 (2026-07-20) mods, maps and blueprints are
browsed, downloaded and updated **from inside the game** via the
Mod Manager — no account needed
(https://coigame.com/Blog/update42-is-out). Scale observed
2026-11-06: ~230+ mods (12 listing pages), top mod at 60.6k
downloads, page 1 mostly updated within hours. The Hub's Modding
Policy (effective 2026-05-03, https://coigame.com/modding-policy)
defines the Hub, the licence templates (**CoI-Open** / **CoI-Keep**
— repository-wide MIT/GPL is *not* compatible unless MaFi material
is excluded), and a revocation mechanism.

Everything else is empty or secondary:

| Channel | Status (2026-11-06) |
|---|---|
| CoI Hub (coigame.com) | **Dominant.** In-game since 4.2. |
| GitHub | Source hosting only (73 repos match "Captain of Industry mod"; official template MaFi-Games/Captain-of-industry-modding, 66★). Players don't download here. |
| Steam | **No Workshop** (never shipped). Discussions only; has a Modding subforum. |
| Nexus / mod.io / itch.io / CurseForge | **Absent** — no CoI presence found. |
| Official Discord | Real support channel, gated; at least one hotfix (Multi-Priority Balancers v0.2.2) was distributed *only* via Discord pins. Not publicly readable. |
| Official wiki modding page | Thin WIP quick-start; not a catalogue. |

**The COI:Extended event.** Effective 2026-04-03 MaFi permanently
revoked the Modding Exception for COI:Extended (author Keranik)
— distribution, forks and mirrors are all forbidden, the original
GitHub repo is gone, and the mod is absent from the Hub
(https://steamcommunity.com/app/1594320/discussions/0/809099441085669189/).
It was historically the biggest overhaul; **any pre-2026 "top
mods" list is obsolete**, and COI:Extended itself is a hard SKIP
(§4.16).

**Two blocs dominate downloads:** Moriarty's "++" suite
(Gameplay++ 60.6k, Cheat++ 56.1k, Storage++ 43.8k, Tweaks++ 39.6k,
Speed++ 33.2k, Recipes++ 29.1k, WorldGen++ 28.3k, Boost++ 23.1k,
Keybind Framework 10.5k) and Kayser's designation mods (Auto
Terrain Designations 49.5k / 89% — the highest rating in the top
10 — and Auto Forestry Designations 24.4k). Trains and true
priority-management mods are notably *under*-supplied relative to
mining and QoL.

**Spam warning:** GitHub repos `arshil-564/Factory-Overseer-Suite`
(57★) and `Rechardelsambada/industrial-overlord-console` (55★)
are SEO "trainer" pages, not on the Hub, UNVERIFIED — treat as
malware bait, never recommend.

---

## 2. Overlap audit vs our stack

| Our feature | Community counterpart | Verdict |
|---|---|---|
| QoSPriorityActuator (automatic starved-machine nudges via shipped `SetGeneralPriorityCmd`) | **CoI_PriorityManager** (manual groups); **Boost++** (per-entity speed, different lever); **Programable Network / Controllers+** (conditional control) | Different problem. PriorityManager is a *manual* group-management UI — see §3.1. Ours is the only *automatic* priority policy found. Keep ours; do not run PriorityManager groups and the actuator on the same fields in the same save — they manage the same values with opposite assumptions (persistent human groups vs stateless nudging). |
| QoSChickens (animal distribution policy) | **Advanced Farm & Food Optimiser** (crop solver + *chicken flock-size overrides*) | Partial overlap, different layer: theirs is a farm-planning solver that also sets flock sizes; ours moves animals between farms daily under guards. Their rotation solver and shortage detection are the valuable part for us — as companion-side advice, not a mod port (§4.6). |
| QoSVehicleRecovery (+ QOL-MODS #6–8 preferred drop-off asks) | **Smart Mining Dump** (per-tower prefer-dumping into local designations) | Same instinct, mining-scoped: pair a pit with a local sink inside the tower zone instead of hauling spoil to storage. Community validation of Philip's #7/#8 idea — and a cleaner first implementation of it than a global drop-off system (§4.5). |
| QoSWorldMap (mines/rigs window) | No direct counterpart found. **WorldGen++** is pre-game world generation, a different thing. | Ours stands alone; no change. |
| QoSBuilder (blueprint placement via confirmed job files) | **Blueprint Designer Toolkit** (Kayser, 6.22k) — in-game blueprint *authoring/testing* UI | Complementary, not competing: theirs is a human authoring tool; ours is an agent execution channel. No change; BDT is a reasonable install for Philip's own authoring. |
| Companion Issues / productivity alerts (QOL-MODS #5) | **UNMA** (in-game alarm panels), **Zone Analyzer** (zone production balance incl. rail) | UNMA's alarm *model* (severity, thresholds, AND/OR triggers, acknowledgement) is the reference design for our alerting semantics (§4.7). Zone Analyzer overlaps the companion's zone thinking — reference only. |
| Earthworks guides + estimator + snapping procedure | **Auto Terrain Designations** (Kayser) | The big one. ATD *automates* in-game what our guides *teach* and our companion *measures*. Its planning parameters are the community's distilled mine-structuring practice (§5). Install it for the run; port its parameters into our planner (§6). |

---

## 3. The one deep overlap: CoI_PriorityManager

**CoI_PriorityManager** by iScripters
(github.com/iScripters/CoI_PriorityManager per MODDING-NOTES;
vendor copy read in full for this survey). MIT licence
(excludes MaFi material). Targets game 0.8.7. Zero Harmony —
verified from source in our own recon (qol-recon/02): a
`[GlobalDependency]` `Window` subclass, the pattern QoSWorldMap
already clones. "Can be added to and removed from existing saves"
per its README.

What it does: one window to inspect, change, reset and organise
**building priorities manually** — an Overall list of every
building with a priority control (filter/sort/camera-pan), named
**Groups** carrying shared general / import / export / generator
priority values applied to members, an inspector section on every
building for group assignment, and careful reset semantics
(manual override ejects a building from its group; removing from
a group restores native defaults). Priorities 1–15.

How it compares to QoSPriorityActuator:

- **Better than ours at:** deliberate, persistent, human intent.
  Groups are exactly the "this district outranks that one"
  statement our actuator cannot express — it only reacts.
- **Worse / different:** it never *acts*. A starved machine waits
  for a human to notice; our actuator exists precisely to remove
  that loop. It also owns the priority fields persistently, where
  our actuator's stateless band-ownership rule deliberately never
  fights the player.
- **Implication:** keep ours. The two are complements with a
  conflict surface (same fields), so the guidance is *per save*:
  actuator-driven **or** group-driven, not both. If Philip ever
  wants manual groups in the companion, PriorityManager's group
  model (shared 4-channel values, eject-on-manual-override,
  reset-to-native-defaults) is the design to mirror — MIT makes
  even code reuse legal with attribution, though our pattern
  would re-implement it companion-side anyway.
- Popularity on the Hub: UNVERIFIED in this pass (no Hub listing
  captured); it circulates via GitHub releases.

---

## 4. The matrix

Categories: **INSTALL-AS-IS** (put it on the fresh run),
**ADAPT** (port the idea/feature into QoS/companion),
**REFERENCE** (read it; shapes our thinking), **SKIP** (with
reason). Install route for every INSTALL pick is the same: the
in-game Hub Mod Manager (Update 4.2+), which also keeps versions
matched to the game build.

### 4.1 Auto Terrain Designations (ATD) — Kayser — **INSTALL-AS-IS** (+ adapt its planner)

The mod Philip was pointing at, and the survey's headline.
Generates mining designations that follow deposit contours
(Scan → Filter → Connect → Ramp → Place), with per-tower mining
priority, ore-composition estimates, ore-spike and ore-quality
filtering, floor flattening, generated ramps/accessways, a
predicted-collapse overlay, corner designations for hand
shaping, sorting-plant export routing and farmland prep.
**49.5k downloads / 255 comments / 89%** on the Hub, updated 2
days before observation, Hub-stable for 0.8.2–0.8.8a
(https://coigame.com/Mods?page=1). Source public, **MIT**
(https://github.com/Kayser1444/CoI_AutoTerrainDesignations) —
code reuse is legal with attribution. Posture is hybrid: vanilla
`TerrainDesignationsManager.AddOrReplaceDesignation` for
placement, **Harmony only for inspector-UI injection and one
guarded truck-parking prefix** — not the zero-Harmony purity of
our suite, but disciplined about it. Save safety: author states
"can be safely removed from saves", with one caveat — removing
it while unassigned excavation is active may discard transient
cargo in excavator buckets/trucks. No ATD-specific
save-corruption reports found (UNVERIFIED beyond that).
**Caveat that matters:** the Hub "Stable" badge overstates 4.3
readiness — the mod's own manifest (v0.9.7) says verbatim that
Update 4.3 compatibility is *experimental*, needs game Build 625
or newer, and has "only limited gameplay validation". Trust the
manifest: install on the fresh run, but shake it down on a
scratch save first — which is our house rule for everything
anyway. **Adapt follow-on:** its planner parameters
(MaxHeightDiff, median bottom-flattening, spike filter,
ore-quality gates, depth caps, landslide safety policy) should
be ported into the companion's earthworks planner / QoSBuilder
job generation — §5, §6.2.

### 4.2 Auto Forestry Designations — Kayser — **INSTALL-AS-IS**

ATD's sibling: one-click forestry designations with per-tower
truck pooling. **24.4k / 159 / 86%**, updated 2 days before
observation (https://coigame.com/Mods?page=1;
github.com/Kayser1444/CoI_AutoForestryDesignations, 2★). Same
author, same engineering discipline, same designation-cell
model; the truck-pooling-per-tower pattern is also a tidy
reference for vehicle-assignment policy generally. (Deep source
read not performed in this pass — posture inferred from the
author's ATD work; treat as partially verified.)

### 4.3 Research Queue — Jagg111 — **INSTALL-AS-IS**

Drag-and-drop research queue — small, single-purpose, beloved:
**7.38k / 147 / 85%** (https://coigame.com/Mods?page=2). Public
source (github.com/Jagg111/COI-ResearchQueue, 5★). It fixes a
real friction in exactly the phase of the game the fresh run is
entering, and its scope is too small to carry meaningful save
risk. Caveat: Hub listing showed "updated 6 months ago" while
the repo moved 2026-05-16; an explicit 0.8.8 verification was
not captured in this pass — install via the Mod Manager (which
filters by game version) and confirm it loads.

### 4.4 Terrain Grid Only / Show Terrain Grid — Mad_MaN — **INSTALL-AS-IS**

A tool that shows the terrain grid on its own. **1.55k / 33 /
78%**, updated 3 days before observation, Hub-stable across
0.8.0–0.8.8 (https://coigame.com/Mods?page=5). Trivial scope,
directly aligned with the grid-first placement discipline our
earthworks guides teach (designations are 4×4 cells on the
global grid; seeing the grid is the skill). Source/licence
UNVERIFIED in this pass; the mod ID surfaced once in a
multi-mod save-error thread's mod list with no blame attached.

### 4.5 Smart Mining Dump — Nimb (repo: CesarM-Git) — **ADAPT**

Per-tower "Prefer Dumping" checkbox: a tower's trucks dump
dumpable material into dumping/levelling designations *inside
the tower's zone* before falling back to storage delivery
(https://coigame.com/Mod/1170/Smart-Mining-Dump;
github.com/CesarM-Git/SmartMiningDumpMOD). **The Unlicense**
(public domain — reuse unrestricted). Small (19 commits) and
honestly engineered: preferences in a per-save JSON in the mod
folder, "safe to add to and remove", reflection used
defensively (it rebuilds its inspector subclass from the base
constructor's runtime parameter list after a vanilla signature
change bit it). Not an install pick: verified for 0.8.7 only,
Hub banner "Not verified for 0.8.8b". **Port sketch:** this is
Philip's QOL-MODS #7/#8 (preferred drop-off) in its smallest
viable form — re-implement the *policy* as a QoS feature:
per-tower opt-in, shipped dumping-job surfaces where they
exist, QoSCore config + activity-log events, default off. The
underlying principle (pair every pit with a local sink) goes
into the earthworks guides regardless (§5.15).

### 4.6 Advanced Farm & Food Optimiser — iserik — **ADAPT** (ideas only)

Automatic crop-rotation solver, shortage detection, fertility
planning — **9.17k / 29 / 79%**, v0.4.1 released 2026-11-02
(https://coigame.com/Mod/1109/Advanced-Farm-Food-Optimiser;
github.com/iserikba/FarmAndFoodOptimiser). Overlaps QoSChickens
at one point (it can override chicken flock sizes) but its real
value is the solver: rotations and shortage forecasts are
*companion-shaped* problems. **Port sketch:** a companion farm
advisor (rotation plans + shortage warnings from
products/history) modelled on its approach — do not port code:
the repo has **no LICENSE file** (verified via API); the Hub's
CoI-Open label is the only licence statement, so until the
author clarifies, this is ideas-only. Audit note: since v0.4.0
it writes its configuration **into the .coisave file** (target
fertility, priority crops, exclusions, flock sizes) — exactly
the save-resident-data surface our own serializer incident
taught us to scrutinise; no corruption reports were found, but
any install decision should be Philip's with that fact in hand.

### 4.7 UNMA — Arystus (Maxim, TaskEcho) — **ADAPT** (alarm model)

A universal notification/alarm system: customisable annunciator
panels mirroring game messages, user-defined alarms over
production, logistics, storage levels, shortages, health and
more, with severity levels, thresholds, AND/OR trigger logic,
sounds and acknowledgement behaviour; other mods can raise
alarms into it (https://coigame.com/Mod/1146/UNMA-Universelle-Nachrichten-Meldeanlage;
github.com/max2605/UNMA). **MIT** (confirmed at repo root).
Not an install pick for the fresh run yet: its Hub page lists
game 0.8.6c and "Not verified for 0.8.8b". **Adapt value:** its
alarm semantics are the best worked example of the alerting
layer our companion's Issues/productivity roadmap (QOL-MODS #5)
needs — severity tiers, trigger composition, acknowledgement
state. Port the model into the companion's alert design; MIT
would even permit code reuse, though the surfaces (in-game
panels vs our web UI) mean re-implementation regardless.

### 4.8 Programable Network / Controllers+ — DeznekCZ — **REFERENCE**

Conditional machine control and programmable displays/lights —
the "circuit network" direction for CoI. **3.14k / 41 / 82%**,
updated 3 days before observation (https://coigame.com/Mods?page=3).
Relevant as the far end of the spectrum QoSPriorityActuator sits
on: where we nudge one priority field under guards, Controllers+
lets players wire conditions themselves. Read it before growing
the actuator's policy vocabulary; no install or port now — its
generality is the opposite of our deliberately narrow posture.

### 4.9 Grid Battery — LordXaosa — **REFERENCE**

Power-storage content mod with public source under **MIT**
(models/textures CC BY 4.0 per the author's Hub statement)
(github.com/LordXaosa/COI-GridBattery). In this pass only its
licence and posture could be confirmed; its content registers
new protos (a grid-scale battery), which puts it outside the
removal-safe class our long runs prefer. Reference for how a
small content mod is packaged and licensed; install verdict
would need the fuller read we did not complete.

### 4.10 Blueprint Designer Toolkit — Kayser — **REFERENCE** (install optional for Philip)

Blueprint authoring/testing UI with transport visualisation,
**6.22k / 41 / 80%** (https://coigame.com/Mods?page=2). It serves
the human author; QoSBuilder serves the agent. If Philip authors
blueprints by hand for the fresh run, this is a sensible
install; it changes nothing about our stack either way.

### 4.11 Shipping++ — rschurade / Roest — **REFERENCE**

Local cargo shipping between island terminals, with a lines
manager deliberately modelled on the vanilla train-lines UI.
v0.6.6, explicitly requires 0.8.8+, **CoI-Open**, public source
(https://coigame.com/Mod/1134/Shipping;
github.com/rschurade/COI-mods). Reference on two counts: its
lines-manager pattern is directly relevant to our parked train
network advisor (QOL-MODS #4), and it is a worked example of a
content mod (new terminals/ships = removal is not free) done
carefully. Not an install pick while the train advisor is
companion-side and unbuilt.

### 4.12 Zone Analyzer — LofwyrMod — **REFERENCE**

Zone-level production/consumption balance across belts, pipes
and rail, **2.23k downloads** (https://coigame.com/Mod/1151/Zone-Analyzer).
Its zone abstraction overlaps the companion's Issues/forecast
thinking — worth reading before we design any zone view of our
own. Plugs into Moriarty's Keybind Framework, a useful signal
of how the ecosystem standardises on shared infrastructure.

### 4.13 The Moriarty "++" suite — **REFERENCE** (as a bloc)

Gameplay++ (60.6k), Cheat++, Storage++, Tweaks++, Speed++,
Recipes++, WorldGen++, Boost++, Keybind Framework. These define
the community's centre of gravity and the compatibility baseline
other mods test against — but as a bloc they are a *different
game*: content additions, balance changes, speed and boost
levers. Several list "No source provided" under a CoI-Open
label. For a vanilla-plus-QoS fresh run they are reference
material (what players expect, what "QoL" means in this
ecosystem, Keybind Framework as infrastructure done right), not
installs. Individual exceptions can be revisited if Philip
wants a specific one (Tweaks++'s camera/HUD set is the usual
candidate).

### 4.14 Bagger 288 — aneverse — **REFERENCE** (do not install on the long run)

A bucket-wheel excavator for continuous strip mining: digs at
the wheel, buffers internally, discharges through four rear
ports. **16.1k / 81%**, v0.9.2 released 2026-11-05, re-verified
for 0.8.8a–0.8.8b (https://coigame.com/Mod/1137/Bagger-288). Its
layout doctrine feeds §5.16. Not an install pick for Philip's
run: **CoI-Keep licence, no source**, it registers a new vehicle
+ research (the not-freely-removable class), and the author
says plainly "the mod is still in development… back up your
saves". Fine for a sandbox experiment; wrong default for a
hundred-hour save.

### 4.15 Multi-Priority Balancers — X-Mag-X — **REFERENCE** (do not install; cautionary)

Adds balancers whose ports carry P1/P2/P3 input/output
priorities — the ecosystem's only true *priority-management*
mechanic mod. Small (**597 downloads / 71%**) but the concept is
real, and if port priorities ever come to QoS it will be via
this idea. **Do not install:** the Hub forum documents it
**crashing save loads in combination with Moriarty's ++ mods**,
and the v0.2.2 hotfix was distributed only through Discord
pinned messages (https://coigame.com/Forum/System/Post/50808/Goto)
— an un-auditable delivery channel for a mod whose failure mode
is the save file itself. This is the same failure class as our
own 0.1.1 serializer incident, from the other side: §7.1.

### 4.16 COI:Extended — Keranik — **SKIP**

Modding Exception permanently revoked 2026-04-03; distribution,
forks and mirrors prohibited; original repo deleted. Not
"abandoned" — *banned*. Never download, never mirror, never
recommend. Any old "best mods" list featuring it is obsolete.

### 4.17 MZ Mining Fleet & Free Logistics — Mazael et al. — **SKIP**

Oversized mining fleet + free-logistics changes, **3.60k / 63%**
(the weakest rating among major mining mods), public-test build
for 0.8.7, Harmony-based (https://coigame.com/Mod/1150/MZ-Mining-Fleet-Free-Logistics).
Author's own warning: "Do not disable or remove it after using
its custom vehicles, buildings, products, or asphalt surfaces."
That sentence disqualifies it for a long run under our
save-safety posture.

### 4.18 Concrete Retaining Cube — Sirael — **SKIP**

Modular 1×1 retaining blocks + ramp segments — an attractive
idea (fine-grained retaining for stairs/platforms/embankments),
but "the mod is **not marked as safely removable** because the
constructed entities are stored in the save", and the verified
game version in this pass was 0.8.6
(https://coigame.com/Mod/1180/Concrete-Retaining-Cube). Wrong
risk profile for the fresh run; revisit only if it gains a
clean removal story.

### 4.19 Retaining Walls Never Die — Nimb (CesarM-Git) — **SKIP**

Removes retaining walls' terrain-height constraints and
collapse behaviour by rewriting prototype data and sweeping
terrain-interaction state on load (Unlicense, public source,
verified 0.8.7 only; https://coigame.com/Mod/1065/Retaining-Walls-Never-Die).
Whatever its engineering merits, it deletes the mechanic our
earthworks guides exist to teach — collapse bands, retained
height, wall placement discipline. Installing it would make
§5's principles (and our guides' physics) moot in the worst
way: silently.

### 4.20 Legacy GitHub generation — **SKIP**

CaptainOfCheats (deprecated by its author, reflection-heavy,
"most certainly not a best practice" in its own words),
DoubleQoLMod (MIT, 18★, but a legacy personal-use mod whose two
mining features — tower priority field, designation sizing —
are superseded by ATD), CoI.Mod.Better (archived 2025). History
worth knowing; nothing to run on 0.8.8.

### 4.21 Wimma cluster (Mega Mining, Bulldozers I–III, Megafactory Logistics, Cargo Airports, Field Farming, Move Buildings) — **SKIP / UNVERIFIED**

A prolific content author (several listings 2–3k range, mostly
CoI-Keep, no source, Harmony-bundled per Hub pages). Bulldozers
("push earth from high parts of levelling designations to low
parts") is the one idea adjacent to our earthworks work, but
both it and Mega Mining surfaced only as Beta listings in this
pass — licences and save behaviour UNVERIFIED. Move Buildings
(0.8.8-re-verified) is the most tempting single item and still
changes what "placement" means under QoSBuilder's audit model.
Skip as a bloc for the fresh run; individual revisits need a
proper read first.

---

## 5. Mine-structuring principles — the community's distilled practice

Labelled by source class. **[CODE]** = our own verified game-code
facts (guides-earthworks.md). **[ATD]** = ATD's player guide /
docs. **[GUIDE]** = the classic community guides (chiefly
SamuraiProgrammer's "Retaining Walls: An Exhaustive Guide").
**[WIKI]** = official wiki / in-game tutorial strings.
**[COMM]** = community threads. These principles also land in
guides-earthworks.md as a clearly-labelled community addendum.

**Design in cells, follow the geology**
1. [ATD] Plan mines as *connected regions of 4×4 designation
   cells following the deposit contour* — Scan → Filter →
   Connect → Ramp → Place — filling holes and corridor-
   connecting separated regions so vehicles can actually reach
   the work.
2. [ATD] Size access for the largest vehicle: corridor
   clearance 1 for small/medium, 2 for Mega-class; ramp routing
   is a first-class planning step, not an afterthought.
3. [COMM] Two independent ramps with turning room, so one
   broken ramp never strands the fleet.

**Control the floor and the benches**
4. [ATD] Cap the height difference between neighbouring cells
   (MaxHeightDiff): lower = smoother, safer benches; higher =
   steeper, more aggressive following of rough deposits.
5. [ATD] Flatten the pit floor to a *median* target (strength
   default 5/10), not the single deepest tile.
6. [GUIDE] Bench in **5-level terraces**: one retaining-wall
   ring per 5 levels (or part thereof), stepping one section
   inward per terrace — matching the code's retained-height
   limit of 5 tiles [CODE].
7. [GUIDE] Setback = **⌈depth ÷ 5⌉ + 1 sections** from the
   outermost ore column; the +1 covers cell-averaging and
   unseen ore in adjacent sections. Centre walls in their 4×4
   section, facing side one cell off the section edge; don't
   over-corner — fewer corners, squarer pits.
8. [GUIDE] Ramps cost **one section of run per level of depth**
   (16 sections for 16 levels) — budget the ramp's footprint
   against the ore it sterilises, and let one deposit's ramp
   serve the next.

**Filter before you dig**
9. [ATD] Filter ore spikes first: isolated ultra-thin columns
   reaching far below the deposit must not dictate the pit
   floor (the author's captures showed spike filtering cutting
   planned bedrock excavation by up to 99% locally).
10. [ATD] Gate by ore quality (purity ratio, ore height, bottom
    density): don't dig the whole column when only part of it
    is worth moving.
11. [ATD] Set both a relative depth cap (layers from surface)
    and an absolute floor elevation.

**Sequence top-down, evenly, walls first**
12. [GUIDE] Surface pass → complete the wall ring → descend
    one 5-level bench with a two-section berth inside the ring
    above → repeat. **Never mine beside an unfinished wall;
    never sink one side to the bottom first** ("steady wins
    the course"). Backfill mistakes with rock, not dirt —
    dirt slides.
13. [WIKI] Designations must start at or above terrain or they
    are unreachable (all-red edges = cannot be mined); each
    designation square works **one excavator at a time** —
    provide at least as many active squares as excavators.
14. [WIKI] Respect material repose (wiki slope table: dirt
    ~0.90 vs undisrupted ore/rock ~3.50): clear overburden
    first or widen setbacks, because surface dirt slides more
    than twice as far as ore faces.
15. [COMM/ATD] "Expose the face" in shallow passes — dig flat,
    find the shape, ramp down 3–4, dig out, repeat — and run
    overburden and ore as separate priorities, ideally separate
    towers, so ore excavators stay on high-yield ground.

**Pair every pit with a sink**
16. [ATD/SMART-DUMP] Dump overburden into the adjacent
    worked-out hole or a levelling designation inside the same
    tower zone before hauling anything to storage — the
    principle Smart Mining Dump automates and ATD's farmland/
    access helpers assume.
17. [BAGGER] For continuous strip mining, the machine's stance
    is its floor: it never digs below the level it stands on,
    so pre-level the strip, leave room ahead of the wheel and
    behind the discharge ports, and expect collapse in the
    travelled strip to be re-cleared to that pass's lowest
    stance.

*Synthesis (labelled inference, stated by no single source):*
ATD's parameter set is the classic manual system, automated —
MaxHeightDiff ≈ bench smoothness, BottomFlattening ≈ the flat
pit floor, Safety policy ≈ the +1-section setback, RampWidth /
CorridorClearance ≈ the ramp-budget rules. The manual guides
remain the *explanation*; ATD is the *execution*.

---

## 6. Top recommendations (in order)

1. **Install ATD + Auto Forestry Designations for the fresh
   run** — via the in-game Hub Mod Manager, after a scratch-save
   shakedown (the manifest's 4.3-experimental caveat, Build 625+
   requirement). This is the guidance Philip sensed, in
   executable form: 49.5k downloads, 89% positive, MIT source,
   actively maintained, removable per the author. Research Queue
   and Terrain Grid Only ride along as zero-regret small QoL.
2. **Port ATD's planner parameters into the companion's
   earthworks layer.** MIT permits code reuse with attribution;
   our zero-Harmony posture means porting the *algorithms*
   (spike filter, median floor flattening, ore-quality gates,
   safety-policy buffer) to companion-side designation planning
   that feeds QoSBuilder jobs — upgrading the agent's 9-step
   snapping procedure from geometry-only to geology-aware.
3. **Implement "prefer local sink" (Smart Mining Dump's policy)
   as a QoS feature sketch.** The Unlicense makes reuse free;
   the policy is tiny, per-tower, default-off — and it is
   Philip's own QOL-MODS #7/#8 ask, community-validated in its
   smallest form. Design it against shipped surfaces first
   (our pattern), with the mod's per-save-JSON preference store
   as the fallback shape.
4. **Settle the priority story per save: actuator XOR manual
   groups.** Keep QoSPriorityActuator as the automatic layer;
   treat CoI_PriorityManager (MIT, zero-Harmony, removal-safe)
   as the sanctioned manual alternative for saves where Philip
   wants deliberate groups — never both on the same fields. If
   manual groups ever come to the companion, mirror its group
   model (four shared channels, eject-on-manual-override,
   reset-to-native).
5. **Steal UNMA's alarm model for the companion's alerting
   roadmap** (QOL-MODS #5): severity tiers, composable
   thresholds/triggers, acknowledgement state — as design
   input to Issues/productivity alerts. MIT-licensed reference;
   no install until it verifies against 0.8.8b.

Deliberately *not* recommended: any Moriarty ++ content on the
fresh run (a different game, not vanilla+QoS), any new-proto
content mod that isn't freely removable (Bagger 288, MZ Mining
Fleet, Concrete Retaining Cube), Retaining Walls Never Die
(deletes the mechanic we are learning), and COI:Extended under
any circumstances (revoked).

---

## 7. Save-safety red flags (the audit section)

Our own standard, post-0.1.1: the dangerous mods are the ones
whose data or callbacks live **inside the save** and can fail at
load/save time. Against that standard:

1. **Multi-Priority Balancers — documented save-load crash** in
   combination with Moriarty's ++ mods, fix distributed only
   via Discord pins (§4.15). A serializer-class failure in a
   597-download mod, remediated through an un-auditable channel.
   This is the ecosystem's clearest warning, and it validates
   our paranoia.
2. **MZ Mining Fleet** — author's own words: do not remove
   after using its vehicles/buildings/products/surfaces.
3. **Concrete Retaining Cube** — explicitly not marked safely
   removable; constructed entities persist in saves.
4. **Bagger 288** — new vehicle + research protos; author
   advises save backups; "still in development".
5. **Farm & Food Optimiser** — writes mod configuration into
   the .coisave (v0.4.0+). No incident found, but it is
   save-resident mod data by design; removal strands it (author
   presents this as handled — UNVERIFIED by us).
6. **ATD** — the mildest flag in its class: removal during
   active unassigned excavation can discard transient bucket/
   truck cargo. Documented, bounded, and disclosed — the way it
   should be done.
7. **Positive examples worth naming:** Smart Mining Dump
   (preferences outside the save's critical path, add/remove
   safe, defensive reflection), CoI_PriorityManager (no protos,
   add/remove safe, MIT), Research Queue (scope too small to
   hurt a save). And the ecosystem norm has visibly shifted
   post-Hub: Hub pages carry explicit Save-game Add/Remove
   markers — check them before any install, every time.

---

## 8. UNVERIFIED register

- ATD-specific save-corruption reports: none found; absence of
  evidence only (multi-mod crash threads list ATD as present
  but blame other mods).
- CoI_PriorityManager Hub presence/download counts: not
  captured; known via GitHub + vendor source read.
- Auto Forestry Designations internals: not deep-read; posture
  inferred from the author's ATD work.
- Research Queue explicit 0.8.8 verification: not captured.
- Terrain Grid Only source/licence: not captured.
- Grid Battery feature details beyond licence/posture: partial.
- Farm & Food Optimiser Harmony use: code search inconclusive
  (incomplete_results) — not evidence of absence.
- Smart Mining Dump, MZ Mining Fleet, Wimma cluster Hub
  download counts beyond listing snippets.
- Discord-only content (hotfixes, mod threads): gated, not read.
- Reddit r/CaptainOfIndustry: could not be verified as a mod
  channel from public search.

*Survey compiled 2026-11-06 from three research passes plus
local vendor-source reads. All download/star figures are as
observed on that date and will drift.*
