<!-- markdownlint-disable MD032 -->
<!-- this paper-trail file wraps prose lines that begin with "- " (sentence continuations, not lists); MD032 reads them as unblanked lists. -->
# Hub submission pack — the toolkit mods on the CoI Mod Hub

Prepared 2026-11-09. The submission itself is Philip's account
action (his Hub account, his clicks); this pack makes it a
copy-paste exercise: audited packages, per-mod listing copy,
the submission order, and the verification steps afterwards.

Everything in §2 marked VERIFIED was read off a live source in
this round or the 2026-11-06 survey (dev/MOD-SURVEY.md).
Everything marked UNVERIFIED is a gap this round could not
close from here — the pack states it rather than guessing, and
the checklist tells Philip what to look at on the form itself.

---

## 1. The packages, audited

Gate: `tools/lint_mod_packages.py` (source mode) and
`tools/lint_mod_packages.py --zips` (the shipped zips).

**Result after this round's fixes: PASS, 0 warnings, both
modes.** Before the fixes the gate passed with 6 warnings
(the description_short lengths below).

| Listing | Package to upload | Version | Depends on |
|---|---|---|---|
| CoI Bridge | `dist/CoiBridge-mod-0.6.6.zip` | 0.6.6 | — |
| QoS Core | `dist/QoSCore-mod-0.2.0.zip` | 0.2.0 | — |
| QoS Builder | `dist/QoSBuilder-mod-0.2.0.zip` | 0.2.0 | QoSCore >= 0.2.0 |
| QoS Chickens | `dist/QoSChickens-mod-0.1.2.zip` | 0.1.2 | QoSCore >= 0.2.0 |
| QoS Vehicle Recovery | `dist/QoSVehicleRecovery-mod-0.1.2.zip` | 0.1.2 | QoSCore >= 0.2.0 |
| QoS Priority Actuator | `dist/QoSPriorityActuator-mod-0.1.2.zip` | 0.1.2 | QoSCore >= 0.2.0 |
| QoS World Map | `dist/QoSWorldMap-mod-0.1.1.zip` | 0.1.1 | QoSCore >= 0.2.0 |

All seven declare game compatibility **v0.8.8 only**
(`min_game_version` = `max_verified_game_version` = 0.8.8,
Update 4.3). Dependencies are declared in the manifests
(`mod_dependencies`), so the game itself enforces them.

### Defects found and fixed in this round (text only)

1. **The 6 description warnings — judged real, fixed.** The
   linter flags any manifest `description_short` over 200
   chars as a card-truncation risk; all six QoS mods were
   over (288–370 chars). The 200-char figure is our own house
   rule, not a documented Hub limit (UNVERIFIED either way —
   see §2), but the defect underneath it was real: on
   QoSChickens the words "Requires QoSCore" sat at the very
   end of the text, exactly where a truncated card would cut.
   All six shorts were rewritten to 187–199 chars, each
   self-contained (what it does, its posture, its dependency).
   The full wording survives untouched in each `readme.txt`,
   which is where the long text belongs. CoiBridge's short
   (144 chars) needed no change.
2. **CoiBridge readme carried an in-house name.** The
   troubleshooting section said "tell Claudius which route" —
   meaningless to a stranger. Now: note the route and report
   it on the mod's Hub page.
3. **CoiBridge readme's expected hello reply was stale** —
   it showed `"version":"0.6.3"` in the 0.6.6 readme. Now
   0.6.6. (The "What's new in v0.6.3" changelog section is
   history and stays.)
4. **Four QoS readmes disagreed with their manifests** —
   Chickens, Vehicle Recovery, Priority Actuator and World
   Map said "Requires: QoSCore >= 0.1.0" while their
   manifests declare `QoSCore >= 0.2.0` (and QoSCore ships
   at 0.2.0). Readmes aligned to >= 0.2.0.

All three zips were rebuilt via their build scripts and
restaged into `dist/weekend-bundle/` (hashes updated in its
BUNDLE-README). **Every DLL inside the rebuilt zips is
byte-identical to the previously shipped build** — verified
by hashing each DLL in old and new zips — so nothing about
mod behaviour changed; the serializer-pair gate ran as part
of every build, as always.

### Suite shape: submit per-mod, not as one listing

**Recommendation: seven per-mod listings.** Reasons, all
structural:

- The packages are already one folder + one manifest per
  mod, and per-mod zips exist in `dist/` for every component.
- A live Hub page renders dependency relationships as
  first-class listing content ("This mod depends on" /
  "Mods that depend on this mod" sections — VERIFIED on a
  public mod page, §2), so QoSCore-first per-mod listings
  present the dependency honestly instead of hiding it in
  prose.
- The game enforces manifest dependencies at load (the
  readmes' "the game will report an error without it" is
  shipped behaviour), so a player who installs a feature
  without Core gets a clear refusal, not a broken save.
- Per-mod listings version independently: a QoSChickens fix
  should not force a re-download of the whole suite.
- Whether the Hub accepts a multi-mod zip under one listing
  at all is UNVERIFIED from here; per-mod is the shape every
  surveyed listing uses.

The combined `QoS-suite-0.5.1.zip` stays what it is today:
the manual-install convenience pack for the GitHub channel.

---

## 2. What the Hub expects — evidence and gaps

VERIFIED (survey, 2026-11-06):

- The Hub (coigame.com/Mods) is the game's one real mod
  channel; since Update 4.2 mods are browsed, downloaded and
  updated from inside the game via the Mod Manager, filtered
  by game version.
- The Modding Policy (effective 2026-05-03,
  coigame.com/modding-policy) governs listings and defines
  the licence templates **CoI-Open** / **CoI-Keep**; a
  repository-wide MIT/GPL licence is not compatible *unless
  MaFi material is excluded*. Our packages ship no MaFi
  material (built against the game's assemblies; none are
  included), and the repo licence is MIT — see the licence
  note in §4.
- Hub pages carry explicit **Save-game Add / Remove**
  markers — ours come from the manifests' `can_add_to_saved_game`
  / `can_remove_from_saved_game` flags, which are set
  honestly per mod (§3 copy repeats them in words).

VERIFIED (one public Hub mod page read as text, 2026-11-09 —
a listing's public face, not the submission form):

- A listing shows: display name, current version, a one-line
  short description under the title, a long description,
  tabs for Information / Announcements / Versions / Forum /
  Changelog / Dependencies, a **Source code** link, and
  author-chosen **Tags** (e.g. "Quality of Life", "Vehicles").
- The version block shows: game-version range, release date,
  file size, and a **License** label per version.
- A **Dependencies** section lists "This mod depends on" and
  "Mods that depend on this mod".
- A banner appears when the game has moved past the mod's
  verified version ("Not verified for 0.8.8d" on a mod
  verified to 0.8.7). Consequence for us: the game series
  has a 0.8.8d now; our mods are verified to 0.8.8 exactly,
  so our listings may carry that banner until a build is
  actually verified against 0.8.8d. Do **not** bump
  `max_verified_game_version` to silence it — verified
  means tested.
- Listings carry a thumbnail image.

UNVERIFIED — not established from here, check on the form:

- The submission form's exact fields, and whether uploading
  requires a coigame.com account / game ownership proof.
- Any Hub-side limit on description length (our 200-char
  card rule is a house rule, not a Hub citation).
- Whether a thumbnail is mandatory, and its size/format
  rules. (We have no per-mod thumbnails yet — if the form
  demands one, that is new work, not a paste.)
- The licence field's exact options (CoI-Open / CoI-Keep /
  free-form), and how it interacts with the repo's MIT.
- Whether the Hub reads `description_short` from the
  manifest itself or takes the short text from the form.
- Moderation/review: whether listings go live immediately.
- Update mechanics: whether a new version under the same
  listing is a fresh upload form, and whether the Mod
  Manager auto-updates players.
- Whether dependency declarations must also be entered on
  the form, or are read from the manifest alone.

---

## 3. Listing copy — paste-ready

Conventions for every listing: **Author** Lewis Cameron.
**Source code field** (observed on live listings):
`https://github.com/plcstevens/captains-of-industry-toolkit`.
**Compatibility line** where the form asks in words: Captain
of Industry v0.8.8 (Update 4.3). **Tags** are suggestions —
pick the nearest the form offers (tag vocabulary UNVERIFIED;
"Quality of Life" and "Vehicles" were observed live).

### 3.1 CoI Bridge — CoiBridge 0.6.6

- **Display name:** CoI Bridge
- **One-line summary:** Read-only web API bridge for the
  Superintendent tool. Adds /coi/v1/* routes to the in-game
  web server. Registers no content.
- **Suggested tags:** (tooling/utility if offered)

**Full description:**

> Read-only web API bridge for Superintendent, the local
> companion website for Captains of Industry.
>
> The game's built-in web API (the one `web_api_start`
> enables) does not expose trade, contracts, loans, edicts,
> per-machine state, terrain or designations. This mod adds
> read-only GET routes for exactly that state, under
> /coi/v1/* on the game's own web server: research (current,
> queue, completed), population and housing status, entity
> census and topology, per-storage contents, per-machine
> instances with live state and recipes, power in kW, custom
> priorities, trains and train lines, contracts, loans,
> edicts, quick trades, terrain heights, deposits with
> remaining quantities, and the current mining / dumping /
> levelling designations.
>
> It never writes to the game. It registers no products,
> machines, recipes or research, and only answers requests
> on the game's own loopback server (127.0.0.1:3073). On its
> own it simply serves JSON; its purpose is feeding the
> Superintendent website, which turns that state into
> dashboards, an issues board with stated reasoning,
> forecasts and a strictly two-phase (prepare, confirm,
> audit) action flow.
>
> Save-game: can be added to and removed from existing
> saves freely; removing the mod folder returns everything
> to vanilla and existing saves load unchanged.
>
> Requires: Captain of Industry v0.8.8 (Update 4.3), with
> `web_api_start` run in the in-game console after each
> game start.

- **Changelog seed:** Initial Hub release. v0.6.6 — terrain
  routes plus corner-exact designation read-back; verified
  for game v0.8.8 (Update 4.3).

### 3.2 QoS Core — QoSCore 0.2.0 (submit before all other QoS mods)

- **Display name:** QoS Core
- **One-line summary:** Shared core for the QoS suite of
  small quality-of-life mods: logging, feature registry,
  config and the activity recorder for every QoS action.
  Changes no gameplay; the feature mods depend on it.
- **Suggested tags:** Quality of Life

**Full description:**

> The foundation of the QoS suite — a family of small,
> single-purpose quality-of-life mods that share one core
> instead of duplicating it.
>
> On its own QoS Core changes no gameplay. It provides the
> suite's shared logging, the registry feature mods announce
> themselves to, the suite's config conventions, and the
> unified activity recorder: every action any QoS mod
> actually takes is appended as one line to a shared feed
> file (%APPDATA%\Captain of Industry\QoS\events-v1.jsonl),
> which the Superintendent website imports and shows on its
> Log page — one chronological replay of everything the
> toolkit did. Recording covers actions only, never
> diagnostics, and is controlled by the
> `activity_log_enabled` setting (default on).
>
> Every QoS feature mod declares a dependency on QoSCore
> >= 0.2.0 in its manifest and will not load without it.
> Install Core first (or install a feature and let the Mod
> Manager pull its dependency).
>
> Save-game: can be added to and removed from existing
> saves freely; it registers no content and stores nothing
> in your save beyond its own settings. If you remove Core
> while feature mods are still installed, those mods refuse
> to load with a clear message — your save is not harmed;
> remove the features first.
>
> Settings: verbose_logging (default off), activity_log_enabled
> (default on). Requires Captain of Industry v0.8.8
> (Update 4.3).

- **Changelog seed:** Initial Hub release. v0.2.0 — adds the
  unified activity recorder; verified for game v0.8.8
  (Update 4.3).

### 3.3 QoS Builder — QoSBuilder 0.2.0

- **Display name:** QoS Builder — companion placement + designations
- **One-line summary:** Executes confirmed jobs for the
  Superintendent website: validates each sealed job file
  (untampered, unexpired, pinned to this game, capped),
  then places blueprints and designations. Requires QoSCore.
- **Suggested tags:** Quality of Life (building/utility if offered)

**Full description:**

> The executor half of the Superintendent companion. The
> game's web API can never carry the commands that place
> belts, pipes and blueprints, or that add terrain
> designations — they take collection arguments and are
> excluded from its command index by construction. QoS
> Builder is the website's pair of hands inside the game:
> the website's Place and Designate flows end in a human
> confirmation, and only that confirmation writes a job
> file. This mod executes job files — nothing else can
> make it place or designate anything.
>
> Two job families: blueprint jobs (place one blueprint —
> buildings, belts and pipes — as ordinary construction
> sites; nothing is charged at placement) and designation
> jobs (add or remove a batch of mining / dumping /
> levelling designation cells as one command).
>
> The guarantees, enforced by the mod on every job:
> confirmed-only (no file, no action); single-use (a job is
> claimed and its id recorded before anything is scheduled,
> so even a crash cannot run it twice); expiring (a job
> dies 30 minutes after confirmation); pinned to one game
> (a job confirmed against one save's gameId is refused on
> any other); untampered (SHA-256 of the confirmed
> contents; any later edit fails the check); capped (500
> items / 200 cells per job from the website, and the
> mod's own settings can only lower those caps). Every
> claimed, scheduled and refused job is written to the QoS
> activity feed. Jobs run at in-game day end.
>
> Without the Superintendent website this mod does nothing
> at all — no job files exist for it to execute. It ships
> DISABLED; enable it in Settings -> QoS Builder when you
> use the companion's Place / Designate flows.
>
> Requires QoSCore >= 0.2.0. Requires Captain of Industry
> v0.8.8 (Update 4.3).
>
> Save-game: can be added to existing saves. It cannot be
> removed from a save written while it was installed: like
> the other calendar-driven QoS mods, its day-end
> subscription is written into such saves, so the game
> will refuse to load them without the mod rather than
> risk damage. It registers no content, and everything it
> places or designates is ordinary game content. Disabled,
> it does nothing — leaving the folder in place costs
> nothing.

- **Changelog seed:** Initial Hub release. v0.2.0 — adds
  designation jobs (schema 2) alongside blueprint jobs;
  verified for game v0.8.8 (Update 4.3).

### 3.4 QoS Chickens — QoSChickens 0.1.2

- **Display name:** QoS Chickens — animal farm distribution
- **One-line summary:** Evens out same-species animal farms
  once a day with the game's own move command, under hard
  guards: never past a slaughter limit, growth-paused farms
  untouched. Requires QoSCore. Ships disabled.
- **Suggested tags:** Quality of Life

**Full description:**

> Keeps your animal farms evenly stocked, once a day,
> using the game's own animal-move command — the same
> MoveAnimalsCmd the UI's +50/-50 buttons issue. The mod
> adds policy (when, where, how much, and when NOT to),
> not a new movement mechanism.
>
> Why: the game's breeding is a percentage of each farm's
> own count, and quick trades add animals to whichever
> farm has room — so over time the fattest farm grows
> fastest and can slam into its slaughter limit while a
> sister farm sits nearly empty. At day end (after that
> day's births and culls) the mod evens out same-species
> farms, or fills them in an order you choose.
>
> Hard guards, always on and not configurable away: never
> move animals into a farm at or above its slaughter
> limit; never touch a farm that is not fully constructed
> and enabled; if any farm of a species is growth-paused,
> that species is skipped for the day (a farm you told to
> stay put is never moved behind your back); never drain
> a farm below the breeding-nucleus floor (default 10).
> Every candidate move is simulated with the game's own
> round-robin first and only scheduled if the predicted
> outcome satisfies all guards. Daily cap across all
> species: 50 animals by default.
>
> Modes: equalise (converge farms to within +/-1) or
> fill_first (fill farms you list, in order). Every move
> scheduled is written to the QoS activity feed.
>
> Ships DISABLED — enable it in Settings -> QoS Chickens
> when ready; balancing starts at the next day end.
> Requires QoSCore >= 0.2.0 and Captain of Industry
> v0.8.8 (Update 4.3).
>
> Save-game: can be added to existing saves. It cannot be
> removed from a save written while it was installed —
> its daily calendar subscription is written into such
> saves, and the game will refuse to load them without
> the mod rather than risk damage. Disabled, it does
> nothing; animal counts in your save are untouched
> vanilla state either way.
>
> Updating from the GitHub bundle: if you installed
> v0.1.1, REPLACE its folder with this one — 0.1.1
> prevented the game from saving (its service could not
> be serialized); 0.1.2 fixes that with the game's own
> sanctioned serializer pattern. Your saves were never
> harmed: the failed save wrote nothing.

- **Changelog seed:** Initial Hub release. v0.1.2 — save
  fix (serializer pattern) over 0.1.1; verified for game
  v0.8.8 (Update 4.3).

### 3.5 QoS Vehicle Recovery — QoSVehicleRecovery 0.1.2

- **Display name:** QoS Vehicle Recovery — stuck truck cargo rescue
- **One-line summary:** Rescues trucks stuck with their
  load via the game's own recovery (flat 0.2 Unity, cargo
  stays aboard), under hard guards: Unity reserve floor,
  daily cap. Requires QoSCore. Ships disabled.
- **Suggested tags:** Quality of Life, Vehicles

**Full description:**

> Rescues trucks that are stuck WITH their load, once a
> day, using the game's own commands — the same
> RecoverVehicleCmd and DiscardVehicleCargoCmd the game
> itself uses. The mod adds policy (when, which trucks,
> and when NOT to), never a new mechanism.
>
> Stage 1 — RECOVER (default): a truck flagged
> cannot-deliver, or stuck on navigation by the game's own
> sustained signals, is recovered: the game teleports it
> to the closest accepting depot for a flat 0.2 Unity.
> Vanilla recovery never touches the cargo — the truck
> drives out of the depot with its load exactly as it was,
> its assignment restored and its jobs re-planned. Your
> goods are rescued with the truck.
>
> Stage 2 — EMPTY (optional, default OFF): if a truck this
> mod already recovered is flagged cannot-deliver again on
> a later day, still carrying the same load, the load is
> the problem — the mod empties it through the game's own
> cargo discard, which routes the goods into the game's
> storage network (real storages in global-priority order,
> the shipyard as overflow; only discardable products are
> actually destroyed). The game charges 0.1 Unity on
> success.
>
> Hard guards, always on: the game's own recovery gate is
> checked BEFORE scheduling (a recovery the game would
> refuse still cancels the truck's jobs on its way in, so
> the mod never schedules one); a Unity reserve floor
> (default 5.0) is never crossed; a daily cap (default 5)
> bounds scheduled recoveries; trucks being recovered,
> scrapped or replaced are never touched; stuck must be
> sustained — a momentary pathing hiccup never triggers a
> paid recovery. Stage 2 only ever touches a load this mod
> itself rescued.
>
> Ships DISABLED — enable it in Settings -> QoS Vehicle
> Recovery when ready; rescue passes start at the next
> day end. Every recovery and empty is written to the QoS
> activity feed. Requires QoSCore >= 0.2.0 and Captain of
> Industry v0.8.8 (Update 4.3).
>
> Save-game: can be added to existing saves. It cannot be
> removed from a save written while it was installed —
> its daily calendar subscription is written into such
> saves, and the game will refuse to load them without
> the mod rather than risk damage. Disabled, it does
> nothing; vehicle and cargo state in your save is
> untouched vanilla state either way.
>
> Updating from the GitHub bundle: if you installed
> v0.1.1, REPLACE its folder with this one — 0.1.1
> prevented the game from saving; 0.1.2 fixes that with
> the game's own sanctioned serializer pattern. Your
> saves were never harmed: the failed save wrote nothing.

- **Changelog seed:** Initial Hub release. v0.1.2 — save
  fix (serializer pattern) over 0.1.1; verified for game
  v0.8.8 (Update 4.3).

### 3.6 QoS Priority Actuator — QoSPriorityActuator 0.1.2

- **Display name:** QoS Priority Actuator — starved-machine priority nudges
- **One-line summary:** Steps up the priority of machines
  starved for days so trucks serve them sooner, and back
  when they work again. Bounded nudges; your settings are
  never fought. Requires QoSCore. Ships disabled.
- **Suggested tags:** Quality of Life

**Full description:**

> Acts on exactly one disease: the machine that sits at
> NotEnoughInput for days because trucks serve everything
> else first. Once a day, a machine that has stayed
> input-starved for several days running gets its general
> priority stepped up one bounded step — via the game's
> own SetGeneralPriorityCmd, the same command behind the
> priority control in a machine's inspector — so trucks
> serve its cargo jobs sooner. When the machine works
> again, its priority steps back toward its proto default,
> one step per day.
>
> What it never does: BLOCKED machines (OutputFull) are
> never adjusted — a full output is a downstream problem
> priority cannot fix. PARKED machines (paused or disabled
> by you) are never touched or counted. Machines short of
> workers, power or computing, broken, missing a recipe
> or invalidly placed are never touched.
>
> The ownership rule is how it avoids fighting you: the
> mod keeps no memory of what it set. Safety is computed
> from the live value each day — it may only adjust a
> machine whose current priority is one its own rule
> could have produced (the proto default, down to a
> configured band of steps below it). Anything else is
> treated as set by you and left alone forever, in both
> directions. Bounds (all configurable): 3 starved days
> before the first nudge, at most 2 steps below default,
> 2 days between a machine's nudges, at most 10 actions
> per day. Every action is logged and written to the QoS
> activity feed.
>
> Ships DISABLED — enable it in Settings -> QoS Priority
> Actuator when ready; passes start at the next day end.
> Requires QoSCore >= 0.2.0 and Captain of Industry
> v0.8.8 (Update 4.3).
>
> Save-game: can be added to existing saves. It cannot be
> removed from a save written while it was installed —
> its daily calendar subscription is written into such
> saves, and the game will refuse to load them without
> the mod rather than risk damage. Disabled, it does
> nothing. Priority values it changed remain ordinary
> machine settings afterwards, visible and editable in
> each machine's inspector.
>
> Updating from the GitHub bundle: if you installed
> v0.1.1, REPLACE its folder with this one — 0.1.1
> prevented the game from saving; 0.1.2 fixes that with
> the game's own sanctioned serializer pattern. Your
> saves were never harmed: the failed save wrote nothing.

- **Changelog seed:** Initial Hub release. v0.1.2 — save
  fix (serializer pattern) over 0.1.1; verified for game
  v0.8.8 (Update 4.3).

### 3.7 QoS World Map — QoSWorldMap 0.1.1

- **Display name:** QoS World Map — mine & rig index
- **One-line summary:** A navigation-only index of
  world-map mines and oil rigs: product, state, level,
  quantity left and buffer per location, filterable and
  sortable. A click centres the map on it. Requires QoSCore.
- **Suggested tags:** Quality of Life

**Full description:**

> One window that lists EVERY world-map mining location —
> mines and oil rigs alike (in the game's data a rig is a
> mine whose product is crude oil, so one list covers
> both). Each row shows the location's title, product,
> state, level and production step, remaining quantity (or
> infinity for unlimited reserves) and its buffer fill. A
> text filter, a state filter (All / Working / Needs
> attention / Unrepaired) and sorting (Product, Level,
> Quantity left, State, Name) help you find the one you
> want.
>
> Clicking a row navigates: the world map opens centred
> on the location and the location's own inspector opens
> with it. That is all. Everything you can DO to a
> location — shifts, repair, upgrade, ship orders — stays
> in that inspector, exactly as in the unmodded game.
>
> Inert by construction: no game commands are constructed
> or scheduled, ever; no Harmony patches; no prototypes;
> no save state of its own beyond its settings. The window
> only reads public game state and calls two public UI
> navigation methods. There is deliberately no enable
> switch — the mod does nothing unless you open the window
> and click. (Known cosmetic gap: the toolbar button
> currently uses the game's generic priority icon.)
>
> Save-game: can be added to and removed from existing
> saves freely; it stores nothing in your save beyond its
> own settings values. Delete the folder to remove it.
> Requires QoSCore >= 0.2.0 and Captain of Industry
> v0.8.8 (Update 4.3). Every navigation is written to the
> QoS activity feed.

- **Changelog seed:** Initial Hub release. v0.1.1 —
  verified for game v0.8.8 (Update 4.3).

---

## 4. Licence note (read before the form)

The toolkit repo is MIT-licensed (LICENSE at the repo
root, Copyright (c) 2026 Lewis Cameron), and the mods'
code is original work that includes no MaFi material (the
game's assemblies are referenced at build time, never
shipped). Per the survey, the Hub's Modding Policy speaks
in its own templates — **CoI-Open** / **CoI-Keep** — and a
repo-wide MIT/GPL is compatible only where MaFi material
is excluded, which is our case. What the submission form
actually offers in its License field is UNVERIFIED from
here. At the form: choose the option that corresponds to
open, source-available terms (CoI-Open, if that is the
choice); if the form wants something else or the mapping
is unclear, stop and note what it shows rather than
guessing — the pack gets updated with the real options.

---

## 5. Name-collision check — do this at submission time

**This check has NOT been performed against the live Hub
from here** (no Hub account access in this environment;
public listing pages were read as text only, which is not
the in-Hub search). It takes about two minutes in the
in-game Mod Manager (or the Hub website's search box if it
offers one to signed-in users):

1. Open the Mod Manager (main menu / pause menu -> Mods)
   or hub.coigame.com/Mods signed in.
2. Search, one at a time: `CoiBridge`, `QoSBuilder`,
   `QoS`, `Superintendent`.
3. For each search, note any existing listing that is not
   ours: its exact display name and author.
4. Expected: `CoiBridge`, `QoSBuilder` and the `QoS*`
   names return nothing of ours yet (this is the first
   submission) and — per the 2026-11-06 survey's ecosystem
   read — no same-named mod was observed then. That is a
   survey observation, not a search result; treat any
   surprise hit as real.
5. If a name collides: the manifest `id` is the load-time
   identity and should not change casually (renaming an id
   orphans existing installs); the *display name* is free
   text — disambiguate there (e.g. suffix the toolkit
   name) and note the collision in this file before
   uploading. `Superintendent` is the companion app's name,
   not a mod name; searching it checks that no mod listing
   already trades on it.

---

## 6. Submission order

Dependencies first, so every listing's dependency section
resolves the moment it goes live:

1. **CoI Bridge** (standalone — no dependencies; can go
   any time, listed first because Superintendent's page
   will point at it).
2. **QoS Core** (the dependency of everything below).
3. **QoS Builder** (depends on Core).
4. **QoS Chickens** (depends on Core).
5. **QoS Vehicle Recovery** (depends on Core).
6. **QoS Priority Actuator** (depends on Core).
7. **QoS World Map** (depends on Core).

Run the §5 name check before step 1.

---

## 7. Philip's upload checklist (per listing)

1. §5 name check done for this mod's name; surprises noted.
2. Open the Hub submission form (signed in — account
   requirements UNVERIFIED; note what it asks).
3. Upload the zip from the §1 table (per-mod zip from
   `dist/`, not the suite pack).
4. Paste: display name, one-line summary, full description
   (§3 for this mod).
5. Source code field: the toolkit repo URL (§3 preamble).
6. Tags: nearest available to the §3 suggestions.
7. Licence: per §4 — open terms; if unclear, stop and
   record the options shown.
8. Thumbnail: if the form requires one and none exists
   yet, stop — that is new work (UNVERIFIED requirement).
9. Changelog field: the §3 changelog seed for this mod.
10. Confirm the form's game-version / save-game markers
    against the manifest (v0.8.8; Add yes; Remove per §3 —
    Core, World Map and Bridge yes; Builder, Chickens,
    Vehicle Recovery, Priority Actuator no) — if the form
    lets you edit markers the manifest contradicts, trust
    the manifest and note the discrepancy here.
11. Submit. Record the listing URL in this file.

Listing URLs (fill in as they go live):

- CoI Bridge:
- QoS Core:
- QoS Builder:
- QoS Chickens:
- QoS Vehicle Recovery:
- QoS Priority Actuator:
- QoS World Map:

---

## 8. Post-submission verification

Do these in the in-game Mod Manager, on a **scratch save**
first — the house rule for every mod, ours included:

1. Each listing is findable by its §5 search terms, shows
   the right version, and is offered for game v0.8.8 (not
   filtered out).
2. Each feature listing's Dependencies section names QoS
   Core; Core's page lists the dependents back.
3. Save-game markers on each page match the manifests
   (Add everywhere; Remove only on Bridge / Core / World
   Map).
4. Install all seven via the Mod Manager onto the scratch
   save. Confirm the game loads, the
   `CoiBridge: BridgeRoutes constructed` line appears in
   the log, and `http://127.0.0.1:3073/coi/v1/hello`
   answers after `web_api_start`.
5. Enable ONE acting feature (QoSChickens is the gentlest),
   run to a day end, confirm its log line, then **save and
   reload** — the gate-zero proof for the 0.1.2 serializer
   fix in its Hub-distributed form.
6. QoSBuilder: leave disabled unless a companion Place /
   Designate shakedown is planned — that flow has its own
   scratch-save protocol (SPEC Addendum XX).
7. Note any "Not verified for 0.8.8d" banner (§2) — expected
   until a build is genuinely verified on 0.8.8d; it is
   information, not a defect to paper over.
