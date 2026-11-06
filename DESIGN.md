# CoI Companion — Design System v1 (IMPLEMENTED 2026-11-05)

> Status: implemented in `coi/src/serve.rs` the same evening (SPEC
> Addendum XVIII records the as-built state, the adopted defaults —
> dark only, comfortable density, blue accent — and every deviation
> from this document). Screenshots: design-mock/after-overview.png,
> after-issues.png, after-confirm.png.

Philip's brief: *"the coi companion UI is fucking awful, we should adopt a
good design system and apply it for how we want to structure the pages,
navigation, content boxes, highlight, put graphs etc."*

This document is the system. It is written to be **executed page by page**
by an implementation round without re-deciding anything: tokens first,
then the shell, then components, then chart patterns, then a page map
that restates every existing page in the new structure.

**Constraints the system is designed inside (non-negotiable):**

- One self-contained Rust binary. All CSS is the single embedded `CSS`
  const in `serve.rs`; all JS is vanilla and minimal (today: table
  filter/sort only). **No external assets, no webfonts, no CDN, no chart
  library.** Charts are inline SVG generated server-side as strings.
- Server-rendered HTML, thread-per-connection std HTTP. No build step.
- Dark-first: the companion sits next to a dark game UI on the same
  screen, often at night. (Light theme is a recorded fork — §9.)
- The §12 prepare → confirm → audit flow is a **safety surface**. Its
  design must stay unmistakable; prettiness never outranks it.
- Honest labelling is a design feature, not copy: UNVERIFIED data,
  INSUFFICIENT evidence and UNEVALUABLE gates get *visual* treatments,
  so a number's trust level is visible before it is read.

**What is wrong today (the diagnosis this system fixes):** a 40-line
ad-hoc CSS const; a top bar with 12 undifferentiated links; one grey
badge for everything; status shown only as a 4 px left border on issue
cards and two text colours (paused/running); numbers in proportional
font, right-aligned by blanket rule; Log source badges hand-coloured
with inline `style=` hex values that match nothing else; raw error
strings as whole pages; no focus states, no hover states on rows, no
empty-state pattern; the confirm page visually identical to every other
panel, so the one dangerous screen in the app looks like all the safe
ones.

---

## 1. Foundations

### 1.1 Colour tokens

All colour in the app comes from these custom properties. **No hex
value may appear in page code** (the Log page's inline badge styles are
the first thing the implementation removes). Names are semantic
(Radix/Carbon-style), never descriptive (`--blue-3` is banned;
`--accent` is not).

```css
:root{
  /* surfaces — five layers, darkest app floor to lightest inset */
  --bg-0:#0b0e14;        /* app background */
  --bg-1:#10141d;        /* sidebar, page header strip */
  --surface:#151a26;     /* card / panel */
  --surface-2:#1b2130;   /* nested card, table header, hover */
  --surface-3:#232b3e;   /* inset wells: code, inputs, wells */
  /* borders */
  --border:#263048;      /* default card/table border */
  --border-strong:#37466a;
  --border-subtle:#1a2132;
  /* text */
  --text-1:#e9edf6;      /* primary */
  --text-2:#a9b3c7;      /* secondary, captions with content */
  --text-3:#6d7990;      /* meta, placeholders, disabled labels */
  --text-disabled:#49536a;
  /* accent (links, primary actions, focus) */
  --accent:#5b9dff;
  --accent-strong:#3b78e0;   /* primary button fill */
  --accent-soft:rgba(91,157,255,.13); /* tinted bg for info chips */
  --on-accent:#ffffff;
  /* status — see the binding table below */
  --ok:#4cc38a;        --ok-soft:rgba(76,195,138,.13);
  --starved:#e8933c;   --starved-soft:rgba(232,147,60,.14);
  --blocked:#e5534b;   --blocked-soft:rgba(229,83,75,.13);
  --paused:#8494ab;    --paused-soft:rgba(132,148,171,.14);
  --warning:#e5c33b;   --warning-soft:rgba(229,195,59,.12);
  --critical:#f85149;  --critical-soft:rgba(248,81,73,.12);
  --info:#5b9dff;      --info-soft:var(--accent-soft);
  --unverified:#a371f7;--unverified-soft:rgba(163,113,247,.12);
  /* chart-only */
  --chart-grid:#232c42;
  --chart-track:#232b3e;   /* unfilled bar track */
}
```

**Status palette — binding table.** Every status colour is bound to
vocabulary the app already uses. A status is *never* shown by colour
alone: chip text or an adjacent label always names it (colour-blind
safety, and the app's own honesty rule).

| Token | Meaning in this app | Bound to (existing concepts) |
|---|---|---|
| `--ok` | working / healthy / success | machine state **Working**; gate **TRUE**; game **RUNNING**; forecast with comfortable runway; Log outcome success; catalogue **verified**; productivity verdict MEASURED-and-fine |
| `--starved` | input missing | machine state **NotEnoughInput**; "production stalled" forecasts; stall issues; a deficit net flow *when presented as a finding* |
| `--blocked` | output stuck | machine state **OutputFull**; storage-full / fill forecasts; blocked-by-downstream issues |
| `--paused` | deliberately off / parked | machine **Paused** state and `isPaused`; game **PAUSED**; plan phases HELD; productivity PARKED |
| `--warning` | important, not yet urgent | issue quadrant **Fix later**; forecast warnings; degraded-but-working banners (bridge older than ideal) |
| `--critical` | act now / error / destructive | issue quadrant **Fix now**; `.err` callouts; irreversible-action buttons (A6-class); failed applies; QoS feed import failure |
| `--info` | neutral signal | issue quadrant **Watch**; catalogue **wired**; Log source **companion**; ESTIMATED evidence class |
| `--paused` (neutral) also covers | no signal | issue quadrant **Ignore for now**; INSUFFICIENT evidence; gate **UNEVALUABLE** (with dashed border — unknown is *not* bad, so it must not borrow warning's colour) |
| `--unverified` | data present, decoding unproven | power **averages** line (bridge-flagged UNVERIFIED); train raw counters; anything the bridge/companion flags unverified. Always dashed outline + the word UNVERIFIED on the chip — never a bare number |

Log source badges (replacing today's inline hex): companion → `--info`,
qos → `--ok`, system → `--paused`. Catalogue: verified → `--ok`,
wired → `--info`, excluded → `--critical` outline.
Evidence classes (productivity/plans): MEASURED → `--ok` text,
ESTIMATED → `--info`, INSUFFICIENT → `--paused` dashed.

### 1.2 Typography

System stack only (no downloads, identical rendering contract on
Philip's Windows PC):

```css
--font-ui: system-ui,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;
--font-mono: ui-monospace,"Cascadia Mono",Consolas,"SF Mono",Menlo,monospace;
```

| Role | Size / line | Weight | Used for |
|---|---|---|---|
| Page title | 22 / 28 | 650 | the one `<h1>` per page |
| Stat value | 24 / 28 | 650, tabular | stat tiles |
| Section title | 15 / 20 | 650 | `<h2>` card/section heads |
| Card title | 14 / 20 | 600 | issue/product names in cards |
| Body | 14 / 21 | 400 | default text |
| Small | 13 / 18 | 400 | table cells, card evidence |
| Caption | 12 / 16 | 400, `--text-2/3` | meta lines, units, hints |
| Mono | 12.5 / 18 | 400 | ids, JSON, ticks, positions, payloads |

Rules: **one h1 per page**; section titles are never uppercase-shouted
(the game's UI does that; we don't); letter-spacing 0 except captions
(+0.02em). Every numeric context sets
`font-variant-numeric: tabular-nums` — tables, stat tiles, tiles, chart
labels. Ids (`Product_Steel`, entity `#23639`, ticks, `[3167, 762, 4]`)
are always `--font-mono`.

### 1.3 Spacing, layout grid, shape

- Spacing scale (4 px base): `4 · 8 · 12 · 16 · 20 · 24 · 32 · 40`.
  Card padding 16; section gap 24; page gutter 24 (16 under 720 px).
- App shell: **232 px sidebar** + fluid main, content max-width
  **1280 px**, left-aligned (today's centred 1200 px column goes —
  a sidebar app reads from the left edge).
- Radius: `--r-sm:6px` (chips, inputs), `--r-md:10px` (cards),
  `--r-lg:14px` (confirm sheet, login card).
- Elevation: borders first. Shadows only on the confirm sheet and
  menus: `0 8px 28px rgba(0,0,0,.45)`. Cards never shadow.
- Focus: every interactive element gets
  `outline:2px solid var(--accent); outline-offset:1px` on
  `:focus-visible`. Non-negotiable (keyboard is a real input here).
- Motion: none beyond `transition: background .12s` on buttons/rows.
  No animations; the app is a tool.

### 1.4 Number formatting

- Thousands separators on all quantities (today's `fmt_num` behaviour
  is the rule, extended everywhere — power kW included).
- Numeric table columns right-align (class `.num`); text columns left;
  **mixed columns (Qty / cap) right-align as one unit.** The current
  blanket `th,td{text-align:right}` with first-column exception is
  replaced by per-column classes.
- Units are caption-grey suffixes, never part of the number:
  `22,667 <span class=u>kW</span>`, `84 <span class=u>/mo</span>`.
- Rates always carry their window: TM/LM pairs stay labelled
  "this month / last month" in headers, never bare.
- Signs: deficits in findings contexts use `−` and `--starved` text;
  in neutral tables the number stays `--text-1` (colour is for
  findings, not data).
- Percentages: one decimal, `%` as suffix; shares in stacked bars get
  whole-% labels only when the segment is wide enough (§4 rule).
- Game dates render as the game gives them (`27 Feb 0388`); wall-clock
  times on Log/Audit stay UTC and say so.

---

## 2. Structure — the app shell

### 2.1 Shell layout

```
┌──────────┬────────────────────────────────────────────┐
│ SIDEBAR  │  PAGE HEADER (title · summary · actions)   │
│ brand    ├────────────────────────────────────────────┤
│ status   │                                            │
│ strip    │  page content (cards, tables, charts)      │
│          │                                            │
│ NAV      │                                            │
│ groups   │                                            │
│          │                                            │
│ user /   │                                            │
│ logout   │                                            │
└──────────┴────────────────────────────────────────────┘
```

- **Brand block**: `🏛️ CoI Companion` + version line (binary/bridge
  versions when known) in caption.
- **Game status strip** (always visible, replaces today's per-page
  `game_header`): game date (title size), then a meta row —
  PAUSED/RUNNING badge · speed · tick (mono) · population.
  When the game is unreachable the strip shows the `--critical`
  "Game unreachable" state instead of vanishing, and every page falls
  back to its degraded pattern (§2.4) *inside the shell* — the bare
  502 page is retired except for login-time failures.
- **Logout** moves to the sidebar foot with the username.

### 2.2 Navigation model

Twelve pages, four groups. Group labels are captions in the sidebar,
not links.

| Group | Pages (in order) | Why here |
|---|---|---|
| **Monitor** | Overview · Issues · Products · Entities | "what is my factory doing" — the read loop |
| **Game** | Research · Trade · Plans · Actions | "change the game" — state + the §12 write surface |
| **Library** | Guides · Catalogue | reference: failure-mode knowledge + the actions-understood registry |
| **System** | Snapshots · Log | the companion's own records: frozen states + the unified activity feed |

Nav item: 32 px row, 6 px radius, `--text-2`; hover `--surface-2`;
**active**: `--accent-soft` fill, `--text-1`, 2 px accent bar on the
left edge, weight 600. A nav item may carry a count pill (Issues →
open "Fix now" count in `--critical` soft; Log → none). Counts must be
cheap: only pages already computed for Overview may feed them;
otherwise no pill. **Under 900 px** the sidebar collapses to a top bar:
brand + status strip on one row, nav as a horizontally scrolling row
of the same items (no hamburger, no JS).

### 2.3 Page header pattern

Every page opens identically:

```
[Group eyebrow — caption, --text-3]
H1 Title                                    [page actions →]
Summary line — one sentence: what this page is, — text-2
or the live state it shows.
[optional: stat tile row / segmented control / filter bar]
```

The summary line replaces today's long explanatory paragraphs (the
Issues page's five-line preamble becomes one line; the explanation
moves into an info callout *below* the content for whoever wants it).

### 2.4 Content patterns

- **Stat tile row**: 3–5 tiles, grid `repeat(auto-fit,minmax(180px,1fr))`.
  Tile = caption label, stat value, one delta/context line (badge or
  caption). A tile may be a link (Issues tile → /issues).
- **Card**: `--surface`, 1 px `--border`, `--r-md`, 16 px padding.
  Card header row = title + optional right-aligned badge/action.
  Cards group in a 12-col mental grid: full, half (`cols-2`), third.
- **Table card**: a card whose body is a table edge-to-edge (padding 0),
  with the filter bar as its header. All big tables live in table cards.
- **Detail layout** (Issue, Product, Guide): main column (max 760 px)
  + right rail (320 px) for meta/actions/related. Under 900 px they
  stack, rail after main.
- **Form layout**: label-above fields, max field width 420 px, hint
  text under the field, errors inline in `--critical` — never a
  full-page error for a bad form value.
- **Confirm layout**: §3.9 — its own pattern, deliberately unlike the
  rest.
- **Empty state**: dashed-border card, centred caption text naming
  what would appear and why it's empty ("No events recorded yet —
  actions you confirm and QoS mod activity land here."). Never a bare
  `<p>` and never the word "—" alone in a coloured column (today's
  Issues board does exactly that).
- **Degraded states** — three named patterns, one visual language
  (`--warning`/`--critical` callout with an action line):
  - **D1 Game unreachable**: status strip flips; page body = callout
    with the three checks (game running · save loaded ·
    `web_api_start`) + the raw error in a collapsed `<details>`.
    Pages that can show stored data (Snapshots, Log, Plans verdicts)
    still render it below the callout, labelled "stored data".
  - **D2 Bridge absent/old**: section-level placeholder card (the
    Entities "update the mod" pattern, generalised): what the section
    would show, which bridge version unlocks it, nothing else faked.
  - **D3 Partial data**: the section renders, with an `--unverified`
    or `--paused` chip naming the gap (e.g. power averages).

### 2.5 The Log feed pattern

Log is a *feed*, not a table, at reading width: rows grouped by day,
each row = time (mono, UTC) · source badge · actor · action ·
summary · outcome badge, payload in an expandable mono block.
Filters (source segmented control + text search) sit in a filter bar
above it; the import report (feed path, new/skipped/malformed counts)
is a caption line under the header, not a paragraph block; a failed
import is a `--critical` callout. Pager = secondary buttons, bottom.

---

## 3. Components

Class names are the contract for the implementation round.

### 3.1 `.btn` — buttons, four tiers

| Class | Look | For |
|---|---|---|
| `.btn` (primary) | `--accent-strong` fill, white text | the page's one main action; Prepare… |
| `.btn--secondary` | `--surface-2` fill, `--border-strong` | Cancel, Back, pager, Unpin |
| `.btn--danger` | `--critical` fill | irreversible applies only (see §3.9) |
| `.btn--ghost` | transparent, `--text-2` | nav-adjacent, logout, minor row actions |
| `.btn--sm` | 4×10 px padding, 13 px font | in-table actions (Pin, Prepare per row) |

States: hover = one step lighter; disabled = `--surface-3` +
`--text-disabled`, `cursor:not-allowed`, and the *reason* in the
title/summary line (today's disabled speed button says nothing).
Inline prepare-forms in tables render as `.btn--sm`.

### 3.2 `.badge` — status pills

Pill: 11.5 px font, 600, 1×8 px padding, radius 999, tinted-soft bg +
token-coloured text, 1 px transparent border. Variants:
`.badge--ok .badge--starved .badge--blocked .badge--paused
.badge--warning .badge--critical .badge--info` and
`.badge--unverified` (dashed `--unverified` border, transparent bg).
Solid variant `.badge--solid` (token bg, dark text) reserved for the
status strip's PAUSED/RUNNING and the confirm sheet.
**Rule:** badge text is the status word itself — "Starved", "TRUE",
"UNEVALUABLE", "UNVERIFIED" — never an icon or dot alone.

### 3.3 `.table` — tables

- Inside a table card; header row `--surface-2`, caption-size
  `--text-2` labels, bottom `--border-strong`.
- Sortable headers (existing TABLE_JS behaviour): pointer cursor,
  `▲/▼` indicator in `--accent` once sorted, `aria-sort` set by the
  same JS. Sort affordance appears on hover for unsorted columns.
- Row: 1 px `--border-subtle` separators, hover `--surface-2`,
  no zebra striping (borders + hover carry it).
- Numeric columns `.num` (right, tabular); id columns `.mono`.
- **Row highlight rules**: a row whose subject is in a finding state
  gets a 2 px left inset bar in the state token (box-shadow inset —
  the disciplined descendant of today's card borders) *plus* its
  status badge; colour-only rows are banned. Full/stalled storages
  and Fix-now products are the first users.
- Sticky header within the card when a table exceeds the viewport
  (`position:sticky` on `th`, pure CSS).

### 3.4 `.callout` — callouts

Bordered tinted panel, icon glyph + bold first line + body:
`.callout--info` (accent), `.callout--warn` (`--warning`),
`.callout--danger` (`--critical`, replaces `.err`),
`.callout--ok` (`--ok`, replaces `.ok`). The raw-error `<details>`
pattern lives inside danger callouts. Callouts are for *states of
the world*, not decoration: max one per section.

### 3.5 Forms — `.field`

Label (13 px, 600) above; input = `--surface-3` bg, `--border-strong`,
`--r-sm`, 8×10 px padding, full width of its container; focus ring per
§1.3. Hint = caption `--text-3` below. Error = caption `--critical`
below + `--critical` border. Selects and checkboxes use the same well.
Search inputs get `type=search` and the 🔍 glyph in the placeholder
only (today's idiom, kept).

### 3.6 `.seg` — segmented control / tabs

One control, two uses: view switches (Log source filter) and period
switches (Product sources TM/LM — today two stacked sections; becomes
a segmented switch *only if* the implementation keeps both datasets
server-rendered; otherwise it stays stacked — the pattern is specified,
its use is optional per page). Pill container `--surface-3`, active
segment `--surface` + `--text-1` + 600.

### 3.7 `.filterbar`

Sticky-under-header bar inside a table card: search input (flex-grow)
+ optional state `<select>` (Machines page idiom) + result count
caption right ("223 machines · 59 starved"). Replaces today's
"Filter: …" caption paragraphs.

### 3.8 `.gate-chip` and `.phase-track` (Plans)

Gate verdicts are badges with fixed words: TRUE (`--ok`),
FALSE (`--critical`), UNEVALUABLE (`--paused`, dashed). A phase row =
phase name + entry/exit gate chips + status word (COMPLETE `--ok` ·
ACTIVE `--info` · HELD `--paused` · NOT ENTERED `--text-3`) + the
evidence string in caption mono beneath. Never a bare letter code
(today's dry-run table compresses statuses to strings like
"HELD (U)" — the words come back in full).

### 3.9 The confirm sheet — §12's safety surface

Deliberately *not* a normal panel. Anatomy, top to bottom:

1. **Warning band**: full-width strip across the sheet top —
   `--warning-soft` bg (or `--critical-soft` for irreversible
   commands), bold line: **"This will change your game."** /
   **"This cannot be undone."** for the irreversible set.
2. **Plain-language summary** at 17 px: what will happen, naming the
   target ("Set Steel Furnace II priority 5 → 8").
3. **Meta row**: current game state (date · PAUSED/RUNNING · speed) —
   today's `state_line`, kept verbatim in function.
4. **Exact JSON block**: labelled caption "Exact command sent to the
   game", mono `<pre>` in `--surface-3`. Unchanged content — this is
   the audit anchor.
5. **Action row**: **Run it** (primary; `--danger` when irreversible)
   + **Cancel** (secondary, same size — never a grey afterthought).
6. **Expiry caption**: "Single-use · expires in 2 minutes · the
   result is written to the audit log." If the apply failed, the
   result page uses `.callout--danger` + the game's error verbatim +
   a link to the Log; success uses `.callout--ok` + Log link.

The sheet is centred, max-width 640 px, `--r-lg`, shadow (§1.3), on
the normal shell background — recognisable at a glance from across
the room, which is the point.

### 3.10 Small parts

- `.fillbar`: 6 px track (`--chart-track`) + fill in a status token;
  used for storage fullness, research progress, power headroom.
  Always paired with the numeric value as text.
- `.payload`: `<details>` with mono `<pre>` (Log payloads, snapshot
  JSON) — summary line is a caption "Payload".
- `.pager`: secondary buttons + "page n of m" caption.
- `.kv`: definition rows for detail rails (label caption / value).
- Screenshot (Overview): full-width image in a card with a caption
  ("Live screenshot — refreshed on page load"), `--r-md`, never bare.

---

## 4. Data visualisation — inline SVG, server-generated

**General rules.** Every chart is an inline `<svg>` string built by a
small Rust helper per pattern (no JS, no library). Colours come only
from §1.1 tokens. Every chart has: a text alternative in the adjacent
caption or table (the numbers are always *also* on the page as text —
charts summarise, they never hold data hostage), a caption naming the
window/basis, and no axes beyond a baseline + end labels. Minimum
touch: no tooltips (no JS), no legends with more than 4 entries.
**Honest-labelling rule:** any series whose decoding is unverified
renders in `--unverified` with the UNVERIFIED chip in its caption
(currently: power averages, train counters). Estimated series are
solid but their caption carries the basis string — the app's existing
forecast-basis honesty, made visual.

### 4.1 Sparkline — product quantity trend

- **Data (exists):** `/api/v1/products/history` per product —
  `quantity[]` monthly points (count+1 semantics: leading point =
  window start); fallback: snapshot-store trend rows (quantity per
  snapshot) on Product detail / Snapshots.
- **Geometry:** 96×28 inline in tables; 100%×120 on Product detail.
  Polyline in `--accent`; last point dot; min/max as caption
  ("1,042 → 1,108 over 8 mo"). Area fill `--accent-soft`.
- **Rules:** no y-axis; if all points equal, render a flat line +
  caption "flat". Seasonal products: same sparkline, caption notes
  "seasonal — judged on long-window balance" (the detector rule,
  surfaced).

### 4.2 Flow bars — produced vs consumed

- **Data (exists):** `products/stats` flows arrays
  `[thisMonth, lastMonth, thisYear, lastYear, lifetime]` produced &
  consumed per product.
- **Geometry:** paired horizontal bars per period (TM, LM): produced
  `--ok`-family neutral? No — produced = `--accent`, consumed =
  `--starved` is wrong (consumption isn't bad). Produced `--info`,
  consumed `--paused` (neutral slate), **net** marker line; the
  deficit only colours when a forecast/issue already flags it.
  In tables: a compact 64 px version in the Net column's cell.
- **Rules:** shared scale per product row across TM/LM; values as
  text beside bars (bars never replace the TM/LM numbers).

### 4.3 Runway bar — forecast horizon

- **Data (exists):** `compute_forecasts` — months to empty/full,
  kind (`runout` / `fill` / `stall-runway`), basis string, seasonal
  flag.
- **Geometry:** horizontal bar, scale capped at 12 months ("12+"
  label beyond), fill length ∝ months; colour by kind: runout
  `--starved`, fill `--blocked`, stall-runway `--critical` when
  < 3 months else `--starved`; a 3-month tick mark labelled
  "quarter" (the app's own urgency horizon from the quadrant logic).
- **Rules:** the basis string prints verbatim under the bar —
  non-negotiable; seasonal forecasts get the long-window variant
  (bar hatched via SVG pattern in `--paused`) + "seasonal" caption.

### 4.4 Machine-state stacked bar — working share

- **Data (exists):** bridge `/coi/v1/machines` per-instance states
  (Working / NotEnoughInput / OutputFull / Paused) aggregated per
  proto and globally; persisted `machine_state_proto` rows for the
  per-snapshot history variant.
- **Geometry:** one 100% bar, height 14 px, segments in state tokens
  (Working `--ok`, NotEnoughInput `--starved`, OutputFull
  `--blocked`, Paused `--paused`, other `--surface-3` w/ border).
  Segment labels inside when ≥ 12% wide, else in the caption line
  ("94 working · 59 starved · 59 blocked · 11 paused" — the numbers
  always exist as text).
- **Uses:** Overview stat area (global), Entities per-proto rows
  (compact 120 px), Issues productivity section (per-proto, with the
  evidence-class badge beside it — INSUFFICIENT protos show the
  dashed empty track, not a bar).

### 4.5 Power balance bar

- **Data (exists):** bridge `/coi/v1/power` — generatedKw,
  consumedKw, generationCapacityKw, maxGenerationCapacityKw,
  demandedKw, wastedKw; averages object (UNVERIFIED).
- **Geometry:** generated vs consumed as two aligned bars against
  capacity scale; balance figure as a stat; headroom = capacity −
  consumed shown as track. Averages, when shown as mini-bars, render
  in `--unverified` with the chip (today's prose line becomes three
  labelled mini-bars + the same words in the caption).

### 4.6 Storage fill bar

- **Data (exists):** bridge `/coi/v1/storages` qty/capacity per row.
- **Geometry:** `.fillbar` 64 px in the Qty/cap cell; ≥ 90% fill =
  `--blocked`, ≤ 10% on an import storage = `--starved` *only when*
  an issue flags it, else `--accent`. (Mirrors the diagnosis rule:
  fullness alone isn't a finding.)

### 4.7 Research progress

- **Data (exists):** bridge `/coi/v1/research` — current node,
  progress (Percent raw ÷ 1000 via existing `pct_raw`), queue.
- **Geometry:** `.fillbar` at panel width + % text + queue as a
  chip list. Completed count as a stat tile.

---

## 5. Page map — every page restated

Format per page: **group · top of page · components · what changes**.
Detail/utility screens follow the twelve.

### Overview — Monitor
- Top: status strip (shell) → stat tile row: Population ·
  Power balance (kW, headroom caption) · Machines working (x/y +
  stacked bar) · Fix now count (critical tile when > 0).
- Then: Issues board (compact, §5 card grid) · Power card (balance
  bar + averages) · Outlook card (forecast table with runway bars) ·
  Pinned products table card (sparkline + flow bars per row) ·
  Screenshot card.
- Changes: controls (pause/speed) shrink to a compact control card
  in the right column — they stop being the page's second element;
  `game_header` retires into the shell strip.

### Issues — Monitor
- Top: header + one-line summary + counts per quadrant as badges.
- Board: four quadrant columns become four **stacked sections** on
  narrow / 2×2 grid on wide, each section headed by its quadrant
  badge + the existing "why this quadrant" text as a collapsible
  info callout (the copy exists — `quadrant_why` — it gets designed,
  not rewritten). Issue cards: product (card title), title (link),
  evidence (small), fix line, forecast runway mini-bar when present.
- Productivity section: per-proto stacked bars + evidence badges,
  replacing the bare `<ul>` of sentences.
- Footer links (report.md / snapshot.json) become secondary buttons.

### Issue detail — Monitor (detail layout)
- Main: why-card (quadrant badge + reasoning) · numbers panel as
  `.kv` rows · monthly history table + sparkline · evidence ·
  outlook (runway bar + basis) · fix panel with per-entity prepare
  buttons (`.btn--sm`, unchanged targets).
- Rail: product link card · guide buttons · forecast advice callout.
- "No longer present" state → D-pattern empty card, in shell.

### Products — Monitor
- Table card + filter bar (count caption). Columns unchanged;
  Net LM gains the compact flow-bar; pinned state = badge, Pin/Unpin
  = `.btn--sm`. Virtual products stay excluded (existing rule).

### Product detail — Monitor (detail layout)
- Header: name + mono id + pin badge/button.
- Main: stat row (Quantity · Stored/cap · Prod TM/LM · Cons TM/LM)
  → history chart (large sparkline + flow bars per month) →
  sources tables with `.seg` only if both periods server-rendered.
- Rail: stored-snapshots sparkline (replaces the "1 → 2 → 3" text
  arrow series), related issues if any.

### Entities — Monitor
- Sections as cards: Storages table card (fill bars, slider/priority
  controls as `.btn--sm` + compact selects, state badges per row) ·
  Set-a-storage's-product form card · Machines table card (filter
  bar with state select; State column becomes badges bound to §1.1;
  per-proto stacked bars in the summary) · Non-default priorities
  table · Machines & transports summary card (counts + not-fully-
  connected table) · Train lines detail table.
- Degraded variants (no game / no bridge / old bridge) = D1/D2
  patterns per section — the page keeps its skeleton, sections
  degrade independently (today's three render paths map 1:1).

### Research — Game
- Current research panel → card with progress fillbar, % and queue
  chips + Stop prepare. Completed = stat + collapsible list.
  Catalogue = table card + filter bar (Start/Queue/Dequeue as
  `.btn--sm`). The "API does not report research" caveat becomes a
  caption naming the bridge as source.

### Trade — Game
- Four cards: Contracts (active + available tables, action buttons)
  · Quick trades table · Loans (active table + overpay fields as
  `.field`s; borrow table) · Edicts read-only table + info callout
  (the "not wired" explanation, kept and designed). Absent bridge
  data per section = D2 placeholder cards (today's `None =>` arms).

### Plans — Game
- Plan header card: plan name/version, save-version honesty note as
  an info callout, overall verdict badge row (TRUE/FALSE/UNEVALUABLE
  counts as gate chips).
- Phase table → phase-track list (§3.8) with evidence captions;
  per-phase detail cards with gate chips + prepare-next-order forms
  (buttons unchanged in behaviour); end-state as `.callout--ok`.

### Actions — Game
- Header + compact game-controls card (shared with Overview) ·
  Global priorities form card (`.field`s; the "ids not published"
  honesty text as an info callout) · Fix-actions card (the two
  bridge-present/absent variants become one card + D2 callout when
  absent; locked rows keep the 🔒 caption style) · Audit log table
  card (last 20; outcome badges; link to full Log).

### Catalogue — Library
- Table card + filter bar. Status column = badges (verified/wired/
  excluded); domain as caption text under the command name; notes as
  caption rows. Prepare links = `.btn--sm` "Prepare…".

### Prepare (generic form) — Game (utility)
- Form card: command purpose as title, mono name + domain + status
  badge in the header; schema fields as `.field`s (type + optional
  shown as hint text, not parentheses soup); note callout when the
  catalogue has one; excluded commands render the policy callout
  (today's panel, designed).

### Guides — Library
- Card grid (today's `.cols`, tokenised): title, blurb, section
  count caption; a guide tied to a live issue gets an `--warning`
  badge "live now" when the implementation can know cheaply —
  optional, default off.

### Guide detail — Library (detail layout)
- Main: sections as cards; "In your factory right now" live panel
  first (it's the differentiator — designed as a stat/`kv` card,
  not a paragraph). Rail: back link + related guides.

### Snapshots — System
- Store stats as stat tiles · Take-snapshot primary button ·
  snapshots table card · trends table with real sparklines
  (replaces text series) · message banners via callouts
  (`.callout--ok` for the `?msg=` line).

### Log — System
- Per §2.5. Source badges tokenised (kills the inline styles);
  outcome = badges; payload = `.payload`; import report = caption;
  pager = `.pager`.

### Login / setup — (bare)
- Centred card on `--bg-0`, `--r-lg`, brand mark; fields per §3.5;
  errors as inline callout. The only page without the shell.

---

## 6. Implementation notes (for the next round, not this one)

- The whole system lands as **one replacement `CSS` const** plus
  class-name changes in the render helpers; no architecture change.
  Suggested helper order in `serve.rs`: `badge(status, word)`,
  `stat_tile()`, `callout()`, `gate_chip()`, then per-page passes in
  the page-map order (Overview first — it's the mock).
- Keep the JS contract: `#ptable/#tfilter`, `#mtable/#mfilter/
  #mstate`, `#ctable` ids and `TABLE_JS`/`MACHINES_JS` behaviour are
  load-bearing; the design changes their styling, not their wiring.
- SVG helpers belong beside `fmt_num`/`flow_pair` in `serve.rs`:
  `svg_sparkline(&[f64])`, `svg_stacked(&[(token, value, label)])`,
  `svg_runway(months, kind)`, `svg_flow_pair(...)`. Each returns a
  string; each is unit-testable without the server.
- The confirm sheet (§3.9) is a `finish_prepare` restyle + the
  irreversibility flag from the catalogue — no flow change: token,
  120 s expiry, single use, exact JSON, audit all untouched.
- Verify per page with the existing stub server
  (`tools/stub_game_server.py`, captures `live-synthetic-v064/` and
  `live/`) — populated and degraded paths both, as today.

## 7. Reference points (discipline borrowed, assets not copied)

- **IBM Carbon** — token layering (global → alias → component) and
  the rule that status colours are semantic aliases, not raw hues.
- **GitHub Primer** — dark surface stair (canvas → subtle → inset),
  border-first elevation, tabular numbers in data tables.
- **Radix Colors** — soft-tint pairs (bg tint + readable text on the
  same hue) used for every badge/callout here.
- **GOV.UK Design System** — the confirm/check pattern's plain-
  language summary and the insistence that the dangerous action look
  different from navigation; also its empty/error state wording
  discipline (say what happened + what to do).

## 8. What the mock shows

`design-mock/overview.html` implements §§1–4 for the Overview page
with realistic placeholder data shaped from the real run's last live
capture (27 Feb 0388, paused 3×; power 22,667 vs 22,666 kW of
38,000 kW capacity; machines 94 working / 59 starved / 59 blocked /
11 paused of 223; steel and diesel forecasts). It is a static file —
no server, no JS frameworks; the small inline script does nothing
the real pages don't already do. `overview.png` is a headless-
Chromium screenshot of the same file.

## 9. Decisions left to Philip (real forks only)

1. **Dark only, or a light theme too?** The system is dark-first and
   complete as dark-only. A light theme is a second token block
   (cheap to specify, real work to verify on every page). Default
   proposed: dark only.
2. **Density**: the system is "comfortable" (14 px body, 16 px card
   padding). A compact density toggle (13 px, tighter tables) suits
   the Entities/Products firehose pages. Default proposed:
   comfortable only; revisit after use.
3. **Accent hue**: blue (`#5b9dff`) is proposed as the single accent.
   If Philip wants the companion to sit visually closer to the game's
   own amber/industrial UI, the accent + info tokens swap to amber
   and status `--starved` shifts to a deeper orange to stay distinct.
   One-line token change, but it is a taste call, so it's his.
