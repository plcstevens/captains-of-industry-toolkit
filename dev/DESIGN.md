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
  - right rail (320 px) for meta/actions/related. Under 900 px they
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

- optional state `<select>` (Machines page idiom) + result count
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
   - **Cancel** (secondary, same size — never a grey afterthought).
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

---

# Addendum — Design System V2 (feedback round, 2026-11-06)

> Philip's feedback on the v1 build, verbatim substance: *"how much
> wasted space there is at times, or a ton of text, or how dark it is
> in parts"* · *"how we structure all those stats bits in the
> paragraphs — break that down better to illustrate the numbers, find
> icons for things"* · *"red for negative numbers, green for
> positive, something else for not pos/negat"* · *"have more CTAs for
> those issues — what is the user meant to do?"* — plus the IA brief:
> *"more NESTING to kill super-long pages… top bar = mega navigation…
> left bar = in-depth navigation."*
>
> This addendum **amends** v1; where they conflict, V2 wins (each
> amendment names the v1 section it replaces). Everything not touched
> here stands: tokens (§1.1 surfaces/accent), the confirm sheet
> (§3.9), honest labelling, no external assets, server-rendered.
> Mocks: `design-mock/overview-v2.html` / `issues-v2.html` (+ PNGs).
> Icons: `design-mock/icons-v2.svg`. Implementation is sequenced in
> §V2.8, **after** the stage-4 round; nothing in this addendum
> changes §12 flow semantics, existing routes, or the accent/surface
> tokens.

## V2.1 Information architecture — nesting, mega bar, depth rail

### V2.1.1 The tree

Three levels everywhere: **section → sub-page → detail**. A page may
not be a scroll of unrelated sections: if v1 stacked N sections on
one URL, V2 gives them sub-pages (rail switcher) or a detail route.

```
CoI Companion
├─ MONITOR  (mega: "What is my factory doing")
│  ├─ Overview            /            anchors: Attention · Power · Machines · Outlook · Pinned
│  ├─ Issues              /issues      anchors: Fix now · Fix later · Watch · Ignore · Productivity
│  │   └─ Issue detail    /issue?product=…&title=…   (fix panel = #fix)
│  ├─ Products            /products?f=…  facets: All · Pinned · In issues · Fluid · Loose · Countable
│  │   └─ Product detail  /product?id=…  (sources = #sources)
│  ├─ Entities            /entities?view=…  sub-views: Summary · Storages · Machines · Network
│  └─ Terrain             /terrain     anchors: Height map · Region · Deposits · Designations
├─ GAME  (mega: "Change the game — every write goes through prepare → confirm")
│  ├─ Research            /research?tab=…  tabs: Current & queue · Catalogue · Completed
│  │   └─ Node detail     /research/node?id=…   (Start / Queue / Unqueue prepares live here)
│  ├─ Trade               /trade       anchors: Contracts · Quick trades · Loans · Edicts
│  ├─ Plans               /plans       rail: phase anchors per plan
│  ├─ Actions             /actions     anchors: Game controls · Global priorities · Fix actions · Audit
│  ├─ Place               /place?ind=…  groups: 25 industries, collapsible, rail facets
│  └─ Designate           /designate   anchors: New pad · Live designations · Remove
├─ LIBRARY  (mega: "Reference — failure modes and the command catalogue")
│  ├─ Guides              /guides      rail: the 10 guide titles
│  │   └─ Guide detail    /guide?id=…
│  └─ Catalogue           /catalogue?domain=…  rail facets: domains + status (verified/wired/excluded)
│     └─ Prepare          /prepare?type=…  (utility, unchanged flow)
└─ SYSTEM  (mega: "The companion's own records")
   ├─ Snapshots           /snapshots   anchors: Store · Take · Snapshots · Trends
   └─ Log                 /log?source=…  rail facets: Companion · QoS · System
```

New routes are **additive only**: `?view=`, `?f=`, `?tab=`, `?ind=`,
`?domain=`, `?source=` params on existing paths, plus one new detail
route `/research/node`. No existing URL changes meaning; existing
deep links (`/entities?proto=…`, `/issue?…`, `/product?id=…`) keep
working — `?proto=` lands on `view=machines` pre-filtered.

### V2.1.2 Top bar = mega navigation (replaces the sidebar brand/status block)

A 48 px full-width bar on `--bg-1`, bottom border `--border`, sticky
at top. Left → right:

1. **Brand**: `🏛️ CoI Companion` (15 px, 650) — version line moves
   to the bar's right end as a caption (one line, ellipsised; the
   v1 sidebar version wrapped to three lines).
2. **Mega entries** — the four groups, each a link to the group's
   landing page (Monitor→`/`, Game→`/research`, Library→`/guides`,
   System→`/snapshots`) with a ▾ caret. Panel on hover/focus (below).
3. **Search**: one input, placeholder "Jump to product, page…".
   Submit → `/products?q=…`; the Products page seeds its filter from
   `q`. (Honest scope: this is a product/page jump, not a global
   content index — no search route exists and V2 does not invent
   one.)
4. **Game status strip** — the v1 sidebar status box, flattened to
   one line: game date · PAUSED/RUNNING solid badge · speed · tick
   (mono) · pop with the users icon. Unreachable game → the strip
   shows the `--critical` "Game unreachable" state (v1 §2.4 D1,
   unchanged in substance).
5. **User**: username + Log out (ghost button) — moved from the
   sidebar foot.

**Mega panel**: drops from its entry, `--surface` bg, `--border`,
`--r-md`, menu shadow (allowed by v1 §1.3), width 560 px, two-column
list of the group's pages. Each row: 16 px icon · page name (600) ·
one-line descriptor · optional count pill (Issues: Fix-now count,
`--critical`; Log: none — cheap-counts rule from v1 §2.2 stands).
Panel footer: one group quick line (Monitor → "Take a snapshot";
Game → "All writes pass a confirmation page"; Library → "240
commands, 15 verified"; System → "Times are UTC").

| Group | Rows (name — descriptor) |
|---|---|
| Monitor | Overview — the run at a glance · Issues — what to fix, in what order · Products — every product, its flows and storage · Entities — every storage and machine, with controls · Terrain — heights, deposits and designations |
| Game | Research — current node, queue, all 250 nodes · Trade — contracts, quick trades, loans, edicts · Plans — phased game plans and their gates · Actions — controls, priorities, the audit trail · Place — put a blueprint down (job file) · Designate — mining / dumping / levelling pads (job file) |
| Library | Guides — failure-mode playbooks · Catalogue — every game command, and what is wired |
| System | Snapshots — frozen states and trends · Log — applies, snapshots and QoS events, in order |

**Behaviour — CSS only, no JS** (v1 constraint stands): panels open
on `:hover` and `:focus-within` of the entry wrapper, close when the
pointer leaves. The entry itself is a real link, so click-through
always works even if the panel never opens; keyboard users tab into
the panel via focus-within. There is no Esc-to-close without JS —
recorded as an accepted limitation, not a defect. **Under 900 px**:
the bar wraps to two rows (brand + status on row 1, mega entries as
a horizontal scroll row on row 2), panels are disabled, and the
left rail becomes a horizontal strip under the bar (the v1 §2.2
collapse pattern, kept).

### V2.1.3 Left bar = the section's depth (replaces v1 §2.2 nav)

The 232 px rail no longer lists all 14 pages. It renders, top to
bottom:

1. **Section label** (caption, e.g. `MONITOR`).
2. **Sub-page switcher** — the current section's pages (same rows/
   active treatment as v1 nav items), with count pills where cheap.
3. **Context block** — one of, per page (the "depth"):
   - *anchors*: in-page section links (Overview, Issues quadrants,
     Trade, Actions, Terrain, Designate, Snapshots);
   - *facets*: filter links that re-render the page server-side
     (Products kinds/pinned/issues; Entities is the sub-view
     switcher itself; Catalogue domains/status; Log sources; Place
     industries with counts);
   - *index*: Research catalogue letter index (A–Z jump links) when
     the Catalogue tab is active;
   - *list*: Guides titles; Plans phases.
4. Nothing else. Brand, status and logout are gone from the rail
   (they live in the top bar).

Per-page rail contents are fixed in the tree (V2.1.1) and the
nesting table (V2.1.4). Detail pages (Issue, Product, Guide,
Research node) keep the rail of their parent sub-page **and** their
v1 content right rail (meta/actions); under 1100 px the content rail
folds below the main column (v1 detail rule, kept).

### V2.1.4 Per-page nesting table + the height discipline

**The 3-viewport rule.** A primary view targets ≤ ≈2,700 px tall at
1440×900 (≈3 viewports). Anything longer must use at least one of:
pagination (directory tables: **60 rows/page**, server-side,
`.pager`), tabs/sub-views, collapse groups, or in-page nav (rail
anchors / facets / letter index). Comfortable density is *not* an
excuse: firehose tables additionally take compact density (§V2.2).

| Page | v1 height (measured) | V2 nesting | What moves where |
|---|---|---|---|
| Overview | ≈6,400 px | one page, capped content | Pinned table → max 6 rows + "All products →" (full table lives on Products); board → max 3 cards/quadrant + count link; Outlook → max 5 rows; rail anchors = in-page nav |
| Issues | ≈3,900 px | one page + detail | Cards shrink (§V2.2/V2.3); quadrant anchors in rail; explainer → collapsed `<details>`; footer buttons move to the page header |
| Issue detail | — | detail (exists) | Fix panel gets `id="fix"` — it is the CTA target for the whole board |
| Products | 4,680 px, 132 rows | list → detail (exists) | Compact rows; rail facets (`?f=`); paginate 60/page; sort stays client-side *within the rendered page* (wart recorded in §V2.8) |
| Product detail | — | detail (exists) | Sources get `id="sources"`; figures per §V2.3 |
| Entities | **27,099 px** | 4 sub-views via `?view=` | Default **Summary**: stat blocks (population/housing/food/research as figures, not the v1 prose strip) + flagged-only mini-tables (full storages, starved/blocked machines, 10 rows each + "see all") + the set-product form as a horizontal form card. **Storages** (283), **Machines** (223): own sub-views, compact, paginate 60/page, rail facets (product / state). **Network**: transports summary + train lines + non-default priorities |
| Terrain | ≈1,950 px | one page | Region form → 3-column form grid (§V2.2); anchors in rail; summary sentence → figures |
| Research | **10,883 px**, 250 rows | 3 tabs via `?tab=` + node detail | **Current & queue** is the landing: current node card + queue chips + completed count figure. **Catalogue**: compact rows (name + status chip + Queue ghost button only — Start/Unqueue move to node detail), grouped by initial letter with sticky letter heads + rail letter index. **Completed**: chip grid in a collapsed card. (Area/tier grouping is *not* specified: the bridge publishes no tier/area per node — grouping by data we don't have would be invention. State tabs + list→detail carry the load instead.) |
| Trade | — | one page | Rail anchors; absent-bridge sections stay D2 cards |
| Plans | — | one page per plan view | Rail = phase anchors; phase cards unchanged in substance |
| Actions | — | one page | Rail anchors; the "ids not published" prose → collapsed details |
| Place | ≈ tall (82 layouts) | grouped list | Industries become collapse groups (first group open), rail facets with counts (`?ind=` jumps + opens); rows compact |
| Designate | — | one page | Rail anchors (New pad · Live designations · Remove); read-back verdict block stays |
| Guides | — | list → detail (exists) | Rail = guide titles |
| Catalogue | 240 rows | list → prepare (exists) | Compact rows; rail facets by domain + status (`?domain=`) |
| Snapshots | — | one page | Rail anchors; trends keep sparklines |
| Log | paged already | one page | Page size 200 → **50**; the 5-line explainer callout → one-line summary + collapsed `<details>` "How the Log works"; rail = source facets (`?source=`) |

## V2.2 Spacing & density — where v1 wastes room (amends §1.3, §3.3)

Named waste from the v1 screenshots, and the correction for each:

| # | Waste (evidence) | V2 rule |
|---|---|---|
| 1 | Card padding 16 + section gap 24 + card margin 16 stack up: the Issues page spends ≈90 px of air per issue card before content | Card padding **12**; card/section gap **16**; the 4 px scale gains a stop: working scale is `4 · 8 · 12 · 16 · 24 · 32` (20 and 40 retire from page layout) |
| 2 | Stat tiles render an empty context line and stand ≈110 px tall for one digit (Overview "FIX NOW 0" tile) | `.stat2` (§V2.3) is content-height: padding 10×12, value 22/26, and **an absent delta/context renders nothing** — no empty line, no placeholder dash |
| 3 | Overview's two columns are unbalanced (attention column ≈4,000 px vs right column ≈1,500 px, then full-width sections) | Overview grid: `2fr 1fr`, right rail (Power, Controls, Machine states) is `position:sticky` under the top bar; the freed left column carries Attention + Outlook; Pinned drops below, capped (§V2.1.4) |
| 4 | Forms stack one field per line in full-width cards (Entities set-product card: two fields in a 1,150 px card; Terrain: five stacked fields) | `.form-grid`: fields flow in a grid — `repeat(auto-fit,minmax(150px,1fr))` for coordinate-style forms (Terrain region: x/y from/to + target in one row of five), label-above kept, submit at the row's end. Max field width 420 applies to single-field forms only |
| 5 | In-row actions are full primary buttons: 283 storage rows × 2 blue buttons, 250 research rows × 3, 132 product rows × 1 — ≈1,600 blue rectangles shouting over the data | Row actions become `.btn--sm btn--ghost` (text-only, `--surface-2` on hover). **One** primary button per page region, maximum. Research rows lose Start/Unqueue entirely (moved to node detail); storage rows keep both toggles as ghost text buttons; Pin/Unpin becomes a pin ghost button with the pinned state shown by a badge |
| 6 | Explainer callouts run 4–5 lines before any content (Log, Research headers) | Every explainer becomes: one-line page summary (v1 §2.3, enforced) + a collapsed `<details class="howit">` holding the long text. Nothing is deleted — it is demoted below the content it explains |
| 7 | Log feed rows are airy (12 px padding + payload indent) and the page size is 200 | Feed row padding 8×12; page size 50 (§V2.1.4) |
| 8 | Issue cards: 4 lines of prose ≈110–130 px each | The v2 issue card (§V2.3/V2.7) targets **≤ 96 px**: title row, figure strip, one guidance line, CTA row — all single-line, nothing wraps past 2 lines |

**Compact table density** — new `.table--compact` (amends §3.3):
cell padding `5px 10px`, font 13/18, row ≈29 px (vs ≈36 px
comfortable). Applies to: Products, Research catalogue, Entities
Storages/Machines, Catalogue, Log-adjacent tables. Short tables
(Outlook, deposits, sources) stay comfortable. Sticky headers
(v1 §3.3) apply to every paginated/grouped table, and grouped
tables get sticky group heads (Research letters, Place industries).

**What does not change**: page gutter 24 (16 under 720 px), content
max-width 1280, radii, the 232 px rail width, body 14 px. V2 is a
tightening, not a re-layout.

## V2.3 The stat block — numbers leave prose (new component §3.11)

**Rule.** A *measurement* never appears inside a sentence. Sentences
may carry reasoning, advice and basis strings; the numbers inside
them move into figures. Evidence/basis text survives verbatim — as
a caption under the figures, not as the container of them.

### Anatomy — `.stat2`, `.figstrip`, `.fig`, `.delta`

```
┌──────────────────────────────────────────────┐
│ [▣ icon] LABEL                        (chip) │   .stat2 (tile form)
│ 22,667 kW                                    │   value: 22/26, 650, tabular
│ ▲ +1 vs consumed · cap 38,000                │   context: ONE line, caption
└──────────────────────────────────────────────┘
```

- **`.stat2`** — tile form (grids): icon tile (28×28, `--surface-2`,
  `--r-sm`, icon 16 px in `--text-2` — or the status token when the
  tile *is* a finding, e.g. Fix now in `--critical`) + label caption
  - value + optional delta chip + optional context line. Tiles in a
  row share one grid (`repeat(auto-fit,minmax(170px,1fr))`, gap 12).
- **`.figstrip`** — the in-card form: a horizontal strip of 2–5
  `.fig`s divided by 1 px `--border-subtle`, used inside issue
  cards, the power card, detail panels. **`.fig`** = label caption
  over value (15/20, 600, tabular) + unit suffix; a `.fig` may carry
  a `.fillbar` under its value (storage figures) or a `.delta` chip
  beside it.
- **`.delta`** — the signed-change chip (§V2.5): arrow icon
  (up/down/flat) + signed value + window caption (`/mo`, `LM`,
  `vs last snapshot`). Chip bg is the soft tint of its sign token.
- **Sparkline slot**: a `.stat2`/`.fig` may end with the v1 §4.1
  sparkline (96×28) — product tiles on Product detail and Pinned
  rows use it; the anatomy does not change.

### Mapping — every prose-stat site, and what it becomes

| Where (v1) | Prose today (example) | V2 treatment |
|---|---|---|
| Overview — Electric power card | "Generated 8200 kW · Consumed 9100 kW · Balance −900 kW" as a sentence; "Demanded 9600 kW (surplus demand 500 kW) · wasted 0 kW · 6 generator(s), 412 consumer(s)" as another; averages as a paragraph | `.figstrip` ×4: Generated · Consumed · **Balance** (delta chip, sign-coloured) · Headroom (capacity − consumed, with balance bar §4.5). Demanded/wasted/generators/consumers become a second figstrip (4 small figs). Averages keep their 3 mini-bars (§4.5) + values as figs; the paragraph is deleted, its basis sentence stays as the caption |
| Issues board + Overview board — issue evidence | "stored 9713 / capacity 9000, produced 554 last month" · "net −20/mo last month, stock 290 ≈ 14.5 months at that rate" | Per-class figure strips (templates in §V2.7): storage classes → Stored (fillbar) · Capacity · Produced LM (delta). Deficit classes → Net LM (delta) · Stock · Cover (fig; amber when the issue's own flag says < 3 months). Stall → Produced LM · Consumed LM · Stock · Cover. The original evidence string remains as a caption line under the strip — verbatim, `--text-2`, prefixed by nothing |
| Issue detail — numbers panel | `.kv` rows of label/value text | Headline `.stat2` row (Stock · Stored/Cap · Net LM as delta · Cover) + the `.kv` rows that are genuinely pairs (ids, positions) stay `.kv` |
| Outlook table — Basis column | "Basis: net +37/mo last month, +156/mo this month; stored 400 of 501" as a cell of prose | Basis cell becomes: Net TM delta chip · Net LM delta chip · Stored fig. The basis *string* moves to a caption row under the runway bar (v1 §4.3's verbatim rule is kept — as caption, not cell) |
| Entities — summary strip | "Population 2320 · housing 0 free of 2320 · months of food 26" + a research sentence with a bar squeezed in | 4 `.stat2` tiles on the Summary sub-view: Population (users icon) · Housing free · Food cover (wheat) · Research (flask + progress fillbar + % fig). The sentence is deleted |
| Product detail — stat tiles | Tiles exist, but flows render as "0/0" pairs ("PRODUCED TM/LM 0/0") | Tiles gain icons; Produced/Consumed become paired figs (TM fig + LM fig); **Net is a delta chip** (TM and LM), never a "0/0" string. Stored/Cap tile gains the fillbar |
| Guide detail — "In your factory right now" | A paragraph of numbers per guide | The panel becomes `.fig` rows for its measurements; the surrounding reasoning stays prose (it is prose) |
| Terrain — region summary | "Height range shown: 5.00 → 17.00 tiles. North (y=0) is up" inside a sentence | Min height fig · Max height fig · Tiles fig; the north note stays a caption |
| Plans — phase evidence | Caption-mono evidence strings | **Exempt, deliberately**: gate evidence is an argument, not a measurement. It stays caption text (§3.8 unchanged); only the phase *counts* on the plan header become figs |
| Log — import report | "imported 2 new, skipped 0…, 0 malformed" inside the feed header prose | Three small figs (New · Skipped · Malformed) right-aligned in the filter bar; malformed > 0 colours its fig `--critical` (a count with a finding attached — §V2.5 rule 4) |

## V2.4 Icon set (new §3.12)

Twenty glyphs — `design-mock/icons-v2.svg` holds the path data as
`<symbol>` defs; implementation pastes the sprite into the page
shell once and references `<svg class="ic"><use href="#i-…"/></svg>`.
**Style contract**: 24×24 grid, `fill:none; stroke:currentColor;
stroke-width:2`, round caps and joins, rendered at **16 px**
(`.ic{width:16px;height:16px}`) — 20 px inside `.stat2` icon tiles
is the only other size. Colour is inherited from `color:`; a glyph
is never coloured directly, and never appears without an adjacent
text label (v1's colour-alone ban extends to icons). Exceptions:
`i-play`/`i-pause` are filled glyphs — strokes vanish at control
size. No emoji, no icon font, no external file (v1 constraint).

| Id | Glyph | Used for |
|---|---|---|
| `i-power` | lightning bolt | Power figures, power card, Actions controls |
| `i-users` | two figures | Population, housing |
| `i-wheat` | wheat head | Food cover, farming guide |
| `i-flask` | conical flask | Research figures, Research nav |
| `i-box` | package | Storage figures, stored/cap, Place |
| `i-factory` | factory | Machines working, machine states |
| `i-alert` | warning triangle | Fix now tile, issue state icon (finding classes) |
| `i-check` | check mark | OK states, fulfilled designations, gate TRUE rows |
| `i-clock` | clock | Cover/runway figures, Log times |
| `i-truck` | truck | Logistics/transport guide, vehicle rows |
| `i-anchor` | anchor | Trade, ship/quick-trade rows |
| `i-train` | train front | Train lines (Entities Network), trains guide |
| `i-chart` | bar chart | Outlook, forecast, trends |
| `i-up` / `i-down` / `i-flat` | arrows | Delta chips only (§V2.5) — nowhere else |
| `i-play` / `i-pause` | media pair | Game controls (Resume/Pause prepares) |
| `i-layers` | stacked layers | Terrain, designations, deposits |
| `i-search` | magnifier | Top-bar search, filter inputs |

(The brief asked for ~16; the set is 20 because the chrome itself —
search, the media pair, the three delta arrows — spends five. Every
glyph has a named use above; unused glyphs are not admitted.)

## V2.5 Number semantics — colour encodes sign, only for differences (amends §1.4)

Philip's rule: *"red for negative numbers, green for positive,
something else for not pos/negat."* Stated exactly:

**Tokens** (aliases over §1.1 — no new hues): `--pos: var(--ok)`
(`#4cc38a`), `--neg: var(--critical)` (`#f85149`),
`--flat: var(--text-2)`, chip tints `--pos-soft`/`--neg-soft` =
the corresponding `-soft` tokens.

1. **A value is coloured by sign only if it is a difference**: a
   net flow (produced − consumed over a window), a balance
   (generated − consumed), a delta against a previous snapshot or
   period, a trend direction. Positive → `--pos` with `i-up`;
   negative → `--neg` with `i-down`. The arrow is mandatory — sign
   is never carried by colour alone (v1 rule).
2. **Zero is a difference too**: a net/delta of exactly 0 renders
   as `0` with `i-flat` in `--flat` on a `--surface-3` chip —
   visibly "measured, and it is nothing", distinct from both signs
   and from *missing* data (missing renders the v1 UNEVALUABLE/
   INSUFFICIENT treatment, never a flat chip).
3. **Absolute values are never sign-coloured**: quantities, stored,
   capacities, counts, stock, cover months, percentages. They stay
   `--text-1`, tabular; magnitude is carried by fill bars (§4.5–4.7)
   and by the numbers themselves. A stock of 5,992 is a fact, not a
   verdict.
4. **Amber is for flags, not signs**: an absolute value takes
   `--warning` only when a *finding* attaches to it — an issue, a
   forecast, a threshold the app itself defines (storage ≥ 90%
   full, cover < 3 months, runway < 3 months). The amber belongs to
   the flag; the same number elsewhere on the page stays neutral.
5. Produced and consumed are absolutes: they keep the v1 §4.2
   treatment (produced `--info`, consumed `--paused` when charted;
   `--text-1` as table figures). Only their **difference** is
   sign-coloured. This paragraph replaces v1 §1.4's "Signs" bullet:
   the old habit — a deficit number in `--starved` wherever it
   appeared — is retired; `--starved`/`--blocked` remain *status*
   colours (machine states, badges, bars), not number colours.

**Worked examples** (the acceptance test for any page):

| Value | Kind | Rendering |
|---|---|---|
| Diesel net TM **−77** | difference | `--neg`, ▼ chip "−77 /mo TM" |
| Diesel net LM −28 | difference | `--neg`, ▼ chip |
| Coal net LM −211 | difference | `--neg`, ▼ chip |
| Hydrogen net LM +295 | difference | `--pos`, ▲ chip |
| Power balance +1 kW | difference | `--pos`, ▲ chip |
| Power balance −900 kW | difference | `--neg`, ▼ chip |
| Bread net LM −15, no issue raised | difference | `--neg`, ▼ chip (sign colouring does not need a finding — rule 1) |
| Any net/delta = 0 | difference | `--flat`, ↔ chip "0" |
| Diesel stored **5,992** | absolute | `--text-1`, neutral; fill bar shows 5,188/7,800 stored |
| Capacity **38,000** kW | absolute | `--text-1`, neutral |
| Steel stock 1,108 | absolute | `--text-1`, neutral |
| Steel cover 24.1 months | absolute | `--text-1` — no flag at 24 months |
| Broken glass cover 2.0 months | absolute + flag | `--warning` — the deficit issue flags < 3 months |
| Crude stored 9,713 / cap 9,000 (108%) | absolute + flag | fill bar full + `--warning` fig — the storage-full issue flags it |

## V2.6 Contrast lifts — the murk, named and fixed (amends §1.1 text tokens)

The system stays dark, the accent stays `#5b9dff`, no surface token
changes. The murk is in the *text* tokens and three usage habits
(ratios are WCAG approximations against `--bg-0`/`--surface`):

| Token / habit | v1 | Problem (evidence) | V2 |
|---|---|---|---|
| `--text-3` | `#6d7990` ≈ 3.8:1 | Captions, tile labels, sidebar group labels, stat context lines all sit at sub-AA contrast — the "dark in parts" Philip named | **`#8494ab` ≈ 5.4:1** (passes AA at 12 px). `--paused` keeps `#8494ab` too — they were already near-twins; status chips carry soft-bg + word, so the collision is harmless and recorded |
| `--text-disabled` | `#49536a` ≈ 2.6:1 | Disabled button labels unreadable (the v1 speed buttons) | **`#5b6a84` ≈ 3.4:1** (disabled text is WCAG-exempt; lifted for legibility, still visibly dimmer than `--text-3`) |
| Mono ids in tables | node ids, entity ids rendered in `--text-3` (Research catalogue column) | Data masquerading as meta — 250 rows of murk | Rule: **`.mono` data uses `--text-2`**; `--text-3` is for labels/hints only, never for values a user might need to read or copy |
| Code wells (`--surface-3`) | JSON/payload text in `--text-2` on the lightest surface reads washed | Confirm-sheet JSON and Log payloads | Well text → `--text-1` for the JSON block itself (it is evidence, read closely); payload `<pre>` → `--text-2` on `--surface-2` (one step darker than v1) |
| Log rows | meta line ("QoSWorldMap ·", game date) in `--text-3`; day heads in `--text-3` | The feed's connective tissue is the murkiest text in the app | Meta → `--text-2`; day heads → `--text-1` 13 px 600 |
| Empty states | dashed card text in `--text-3` | "No issues in Fix now" reads as *absent*, not *stated* | Empty-state text → `--text-2`; the dashed border stays |
| Chart tracks | `--chart-track #232b3e` on `--surface` | Fine — recorded as checked, unchanged | — |

## V2.7 Issue CTAs — every finding names its next action (new §3.13)

Today an issue card ends with a prose "Fix:" sentence and no
action; the only buttons live buried in the issue *detail* page.
V2 gives every card a **CTA row**: one primary `.btn--sm` (the
single most useful next step), at most one ghost link, and the
guidance line above them (the existing `fix` / `fix_hint` /
`fix_for` copy, trimmed to one line — the words already exist in
the code; V2 changes their *form*, not their content).

**Discipline (unchanged §12)**: a CTA never applies anything. It
lands on a detail page, a sources table, a filtered directory, or a
prepare form — the confirmation page remains the only place a
command is confirmed. Where no command exists for a class, the CTA
is an honest navigation ("See who draws it", "See machines") —
never a fake "Fix it" button.

Classes are enumerated from the code: `compute_issues`
(`main.rs`), the productivity estimators (`productivity.rs`), the
forecast engine, and the Overview power card. (There is **no**
research-idle issue class in the code — the board generates none,
so the table invents none; if a lab-idle detector is ever added, it
attaches to Research's Current tab, not to this table.)

| # | Class (generator) | Guidance line (existing copy, one line) | Primary CTA → destination | Ghost |
|---|---|---|---|---|
| 1 | Production stalled, consumption continues (`compute_issues`) | "Producer is idle while demand continues: check its inputs, power, workers and recipe state." (Steel: its own furnace/caster line) | **Inspect producers…** → `/issue?…#fix` (the detail fix panel: per-machine ToggleEnabled + recipe-active prepares) | See machines → `/entities?view=machines` |
| 2 | Net deficit — buffer draining | "Raise production or cut the largest consumer." | **See who draws it** → `/product?id=…#sources` | View product → `/product?id=…` |
| 3 | Structural deficit — stock falls every month | "No cliff, but the stock falls every month — plan capacity, don't firefight." | **See who draws it** → `/product?id=…#sources` | View product |
| 4 | Seasonal producer in deficit (long-window) | "Judged on the long window — plan next season's capacity; don't chase one month." | **View product** → `/product?id=…` (history chart shows the seasons) | — |
| 5 | Storage full while still producing | "Add storage, raise draw, or pause the producer before it backs up the chain." | **Open fix panel…** → `/issue?…#fix` (pause-producer prepares) | See storages → `/entities?view=storages&product=…` |
| 6 | Storage full, chain parked | "Consistent with a filled buffer / war chest. No action unless draw resumes." | *No primary CTA* — the row renders a muted "No action needed" with `i-check`; inventing one would be a fake action. Its stored figure and fill bar render neutral with an accent bar — the fullness is the finding itself, and a red bar would contradict the no-action row | View storages → `/entities?view=storages&product=…` |
| 7 | Stored above nominal capacity | "Check what holds the excess (vehicles/machine buffers?)." | **See storages** → `/entities?view=storages&product=…` | View product |
| 8 | Low machine productivity — input-starved (productivity (a)) | "Input is the constraint: check the supply chain feeding these machines before adding more." | **See machines** → `/entities?view=machines&proto=…` | — |
| 9 | Low machine productivity — output-blocked | "Output is the constraint: clear the downstream blockage before adding machines." | **See machines** → `/entities?view=machines&proto=…` | See storages → `/entities?view=storages` |
| 10 | Low machine productivity — mixed / unclassified / idle | Per-cause copy from `fix_for` (mixed: relieve output first; unclassified: check power and upkeep) | **See machines** → `/entities?view=machines&proto=…` | Open Actions → `/actions` (unclassified only — power priorities live there) |
| 11 | Output below its own baseline (productivity (b)) | "Output fell against its own best month — check what changed around the drop." | **View product** → `/product?id=…` (the history chart *is* the evidence) | See machines → `/entities?view=machines` |
| 12 | Power deficit (Overview power card — a card state, not a board issue) | "Consumption exceeds generation — raise generation priority or add capacity." | **Open Actions…** → `/actions` (generation-priority prepares via the catalogue) | See generators → `/entities?view=machines` |
| 13 | Forecast rows (Outlook: runout / fill / stall-runway) | The forecast's own `advice` string, one line | **View product** → `/product?id=…` | See the issue → `/issue?…` when a matching board issue exists |
| 14 | QoS job refused (Log rows only — `JobRefused` events) | The refusal reason from the event payload | **Re-prepare** → `/place` or `/designate` (by job family) | — |

**Card anatomy with CTA** (`.issue2`): inset state bar (v1 rule) ·
row 1: state icon + product link + title + quadrant badge ·
figure strip (§V2.3) · guidance line · CTA row. The Overview board
uses the same card, capped (§V2.1.4).

## V2.8 Implementation plan (sequenced AFTER the stage-4 round)

Order is load-bearing: chrome first (every page re-renders inside
it), tokens/components second, pages after. Sizes: S ≈ an evening,
M ≈ a day, L ≈ several days. **Untouched throughout**: §12 flow
semantics and the confirm sheet's content, all existing routes and
their meanings, the accent/surface tokens, `TABLE_JS`/`MACHINES_JS`
element ids (extended, never renamed), the audit/log writes.

| Step | Work | Size | Notes |
|---|---|---|---|
| 0 | **Chrome**: `layout()` gains the top bar (brand, mega entries + panels as pure CSS, search form, status strip, user) and the per-section rail renderer (rail content = data tables keyed by section/page, mirroring §V2.1.3); `page_head` gains the breadcrumb + slimmer header; sidebar brand/status/logout code removed | L | Verify every page in the shell before touching any page body; degraded D1 state must render in the top-bar strip |
| 1 | **Tokens & components**: CSS const — `--text-3`/`--text-disabled` lifts, `--pos/--neg/--flat` aliases, spacing tightening, `.table--compact`, `.stat2`/`.figstrip`/`.fig`/`.delta`, `.issue2`, `.form-grid`, `.howit` details style; icon sprite const + `icon(name)` helper in `serve.rs` | M | Pure presentation; snapshot the stub-server pages before/after |
| 2 | **Number semantics sweep**: `delta_chip(value, window)` + `signed()` helpers; apply at the difference call sites (Products Net columns, power balance, Outlook, snapshot diffs, product detail Net) and *remove* sign colour from absolutes (the §1.4-era `--starved` deficit numbers in tables) | S–M | The worked-example table (§V2.5) is the checklist |
| 3 | **Overview + Issues**: stat-block tiles; power figstrips; issue-card renderer gains figure-strip templates per class + `cta_for(issue)` (title-class match → §V2.7 row) + CTA row; Outlook figures + capped rows; board caps; explainer collapses | M | `cta_for` is a match on the generator's title strings — brittle by construction; record the class in `Issue` as a field instead if the type is touched anyway (preferred, small) |
| 4 | **Directory pages**: Entities `?view=` split + Summary sub-view + pagination; Products facets + pagination; Research tabs + letter-grouped catalogue + `/research/node` detail (Start/Unqueue prepares move there); Log page size 50 + explainer collapse | L | Products sort stays client-side within the rendered page — the pagination wart: sorting a 60-row page is not sorting the whole set. Accepted for V2 (rail facets narrow the set first); a server-side `?sort=` is the recorded follow-up, not hidden |
| 5 | **Remaining pages**: Trade/Plans/Actions/Terrain/Place/Designate/Catalogue/Guides/Snapshots — rail anchors/facets, form grids, figure conversions per §V2.3 mapping, explainer collapses | M | Place industry collapse groups ride on `<details>` — no JS |
| 6 | **Verification**: stub server (`tools/stub_game_server.py`, `live-synthetic-v065/` + `live/`) — every page populated + degraded; screenshot pass against design-mock v2 PNGs; the §V2.5 worked examples checked value by value | S | No page ships on code review alone — the v1 lesson |

**Deferred, explicitly**: light theme (v1 §9 fork, still Philip's
call); density *toggle* (V2 tightens the one density instead);
Research area/tier grouping (blocked on bridge data that does not
exist); server-side table sorting (follow-up recorded in step 4).

---

**Implemented — 2026-11-06 (Design V2, all six steps, plus
Philip's two same-evening additions).** The must-fix alert badge
sits in the top bar (count from the same `compute_issues` data as
the board; `GET /api/v1/must-fix-count` answers from a server-side
cache fed by page loads and never fetches the game; unreachable
renders the count greyed as "last known"). The login concept is
deleted entirely — no login page, no sessions, no username in the
UI; the per-form CSRF tokens stay, and a Host-header check
(127.0.0.1/localhost on the configured port only) replaces the
session as the gate. The store's `web_users` table is left in
place for schema stability and is now **vestigial** — nothing
reads or writes it; the once-promised `reset-password` command is
moot. Verification: cargo build 0 warnings; cargo test 47/47
(41 pre-existing + 6 new covering delta-chip semantics, signed
formatting, the Host check, the badge endpoint/states, and the
power panel); all check scripts pass (check_place_page,
check_log_page, check_designate_page, check_audit_fail_loud,
check_plan_parity, check_serializer_pairs, and the new
check_badge.sh, which proves endpoint count == filtered-board
cards, badge markup on two pages, the last-known flip, and the
403 on a foreign Host). The check scripts were moved to the
no-login flow; their Log import assertions now read the §V2.3
figure markup instead of the retired sentence. §V2.5 worked
examples verified in rendered markup against the v065 stub
capture: diesel net TM −77 red ▼ / net LM −28 red ▼, stored 5,992
neutral with fill bar 5,188/7,800; hydrogen net LM +295 green ▲;
coal net LM −211 red ▼; bread net LM −15 red ▼; measured Δ0 →
muted flat chips (Products, Outlook); steel stock 1,108 neutral,
cover 24.1 months neutral; broken-glass cover 2.0 months amber;
crude stored 9,713 / cap 9,000 → full bar + amber figure; power
+1 kW green / −900 kW red / 38,000 kW capacity neutral via unit
test (the stub capture carries no `/coi/v1/power` payload, so the
panel renders nothing against the stub — v1 behaviour identical).
Page heights at 1440×900 vs the 3-viewport rule (≈2,700 px):
Products 2,690 ✓ · Research 949 ✓ (v1: 10,883) · Entities
Summary 1,306 ✓ (v1: 27,099) · Issues 3,394 (its own mock:
3,626; v1 ≈3,900) · Overview 3,422 — over target despite every
spec cap applied (board 3/quadrant, Outlook 5, pinned 6): the
v065 capture carries 15 board issues against the mock's sparser
sample, and the page uses rail anchors per the rule's mitigation
list (v1: ≈6,400). Deviations: mega-panels render without count
pills (a count would need a live fetch per render; the panels
stay data-free). One pre-existing flaky test (plan.rs
decide-order temp-dir race under parallel execution) was fixed
in its test helper. Left for v2.1: Products server-side sort;
machine-states stacked-bar labels overlap on narrow segments;
Overview height on issue-dense runs.

---

## DESIGN V3 ADDENDUM (2026-11-09, as built)

V3 is a restructure of meaning, not a repaint: the V2 design system
(tokens, stat blocks, §V2.5 number semantics, dark-only, blue
accent, no external assets) is unchanged. What changed is what
pages declare, how the site is organised, and where its reasoning
lives. Specified by DESIGN-V3-BRIEF.md (the owner's six principles)
and DESIGN-V3-AUDIT.md (per-page audit); this addendum records the
as-built state and every deviation.

### Page contracts (audit §5 stage 2)

Every page now opens with the same two-part head: the page_head
summary line is the page's INTENT (one player-language sentence),
and a `.contract` strip under it states, in fixed order: the
DOMAIN (which top-level domain owns the page), the BALANCE (Acts
on the game / Read-only / Local records — mutating pages add
"Nothing applies without your confirm"), and the DATA contract
(what the page needs to be truthful, and what it does when that
is missing — degraded, never invented). Developer vocabulary was
removed from player surfaces (route floors, §-references, engine
jargon, dev-tool nouns); where a bridge version floor matters it
is stated as a player fact ("needs CoiBridge 0.6.5+").

Data-state honesty fixes in the same pass: the bridge-degradation
note is now cause-neutral everywhere ("that route isn't answering
right now — if the installed mod is older than X, updating it is
the fix; if it's current, the bridge may not be loaded with this
save") instead of asserting a version cause; the Overview names
whichever vital-sign tiles are ABSENT rather than letting a
missing tile read as zero; the product page's missing
producer/consumer data is a stated gap with its reason, not a raw
error; an issue that has left the board reports when it was last
seen; the catalogue's command-less prepare link explains itself
via a flash message instead of redirecting silently.

### Navigation (audit §2.3)

Five top-level domains: **Monitor** (Overview, Issues, Products,
Terrain), **Workshop** (Entities, Research, Trade, Actions,
Catalogue, Place, Designate), **Guidance** (Learn the game,
Learn the tool, Why it suggests), **Plans**, **Records**
(Snapshots, Log). Guidance is a peer domain with the brief's three
classes. The old Library section dissolved: Place/Designate moved
to Workshop, Guides became Guidance. The edicts table moved from
Trade to the Edicts guide's live panel (Trade keeps a cross-link;
old `#edicts` anchors still land). Non-default priorities moved
from Entities/Network to Actions (Network is captioned as the
trains view). The Entities Summary shed its settlement tiles
(they live on Overview/Research). NO URL changed; /manual and
/why are the only new routes.

### The canon

One canon, three renderings: source YAML in `docs/canon/`
(41 entries, 7 families), the website's Why pages and issue-copy
strips render from the generated `docs/canon/canon.json`, and an
agent rendering is generated into `.llms/canon-for-agents.md`.
`tools/check_canon.py` holds code and canon together (53 wired
numbers verified against the source at build time; unwired
entries are listed, not silent). Flagged disagreements were
reconciled into the canon with their history: the Storage guide's
war-chest passage is now a framed worked example and the canon
carries a provenance rule (no guide may present a specific run's
factory as the reader's); the coal stall exemption is an explicit
canon rule (`board.import-fed-products`) with its
hardcoded-to-coal limitation stated on the Why page — engine
behaviour deliberately NOT changed this round (a flagged decision
for the owner); the forecast horizons are stated together in one
entry and the command-line report was fixed to match them; the
productivity thresholds render on the Why page from the EFFECTIVE
config, not from prose.

### Height discipline (measured 1440×900, v065 stub capture)

Directory pages: Catalogue 15,264 → 2,631 (server-side pagination,
30/page — deviation from the audit's 60: catalogue rows carry
purpose+args prose at ~2× product-row height, and the binding
rule is the 3-viewport cap), Research catalogue 10,786 → 1,776
(letter groups collapsed, only the current node's group open),
Entities Machines 6,401 → 2,579 (instance recipes truncated to
3 + "+N more"; census states as compact counts; census collapsed
and paginated 60/page on its own parameter), Products gained
server-side sort. Overview 3,422 → 1,835 (Attention capped to all
of Fix now + the first Fix later, Watch/Ignore as a counts strip;
the screenshot sits below Pinned, last on the page). The Issues
BOARD is not paginated or collapsed — by the owner's rule every
finding stays visible; it is therefore a reading page under the
proposed rule (actionable content in the first viewport, no
height cap) and measures 3,516 on the issue-dense capture, while
its Fix-now filtered view is 949. The productivity evidence under
the board is collapsed behind a summary line. Products (2,800)
and Entities/Storages (2,816) sit ~100 px over the nominal
2,700 line — entirely the new contract strip, accepted: the cap's
purpose (no endless directories) is met. Guides are reading
pages; the longest (Transport, 3,231) is unchanged prose.

### Rename

The product is **Superintendent** (crate package, binary
`superintendent.exe`, page titles, top-bar brandmark — a clipboard
glyph, the superintendent's board; the 🏛️ mark was the agent's
personal glyph and is retired from the product). Frozen by
design: the `coi/` crate FOLDER, `coi.db`, `coi.toml`, the COI_*
env vars, the QoS data paths, the mod names, and the `coi-v*`
release tag scheme. The release workflow text (with the new lint
gate) is a draft in dev/RELEASE-WORKFLOW-DRAFT.md for the owner to
paste; the in-Hub product-name check at submission time remains
an open residual.

### Repository restructure (same round)

The repo root is lean (README/INSTALL/REMOTE-ACCESS + component
folders), with three documentation destinations: `docs/`
(player-facing, incl. the canon), `dev/` (the paper trail, incl.
this file and the synthetic captures under `dev/captures/`), and
`.llms/` (agent-facing: BEHAVIOURS, SANDBOX-PROTOCOLS, the canon's
agent rendering, and the reading index). The lean rule is
recursive inside components (loose notes moved to each component's
own dev/; mod packaging files stay at mod roots — the shipped
layout the game expects). `tools/check_repo_structure.py` enforces
the layout (root allowlist, per-folder readmes, markdown reference
integrity, the mod invariant, .llms index freshness) and is part
of the standard check sequence. The standard lint stack
(rustfmt/clippy, ruff, shellcheck, markdownlint, yamllint,
actionlint) plus the mod-package linter
(`tools/lint_mod_packages.py`) were adopted in the same round;
configs live at the root (.editorconfig, .pre-commit-config.yaml,
.markdownlint.json, pyproject.toml).

# Addendum — UX / JavaScript performance examination (2026-11-09)

Philip's brief: examine the UX and think about the JavaScript
performance. Discipline: measure first, fix only what the evidence
condemns; every claim carries a number. Harness: Playwright +
headless Chromium in the VM, the stub game server against both
committed captures (`live-synthetic-v065`, `live-synthetic-fixnow`),
1440×900 primary viewport, a 390×844 pass for reachability.
Materiality bar, set before measuring: an interaction over
100 ms on the synthetic data, a poll that burns cycles unseen, a
paint that janks, wire size dominated by avoidable duplication,
or a control that is dead or unreachable at 390 px.

## Baseline measurements (before, v065 capture, 1440×900)

Every page carries the same fixed overhead: inline CSS 25,686 B,
icon sprite 6,321 B, badge script 769 B — 32,776 B before any
content. All JavaScript is inline at the end of `<body>` (there
is no `client.js`); the largest script total on any page is
2,084 B (Entities/Machines). First contentful paint across all
34 routes × 2 captures: 100–260 ms. Longtasks were sporadic
(57–86 ms on about a third of loads, including the smallest
pages — the VM's noise floor, not page-attributable).

| Page | Wire | DOM nodes | FCP (ms) | Notes |
| --- | --- | --- | --- | --- |
| Overview | 54.6 KB | 687 | 260 | |
| Issues | 59.4 KB | 772 | 188 | board, uncollapsed by rule |
| Products | 96.5 KB | 1,957 | 216 | 60 rows, 62 forms (pin per row) |
| Entities/Storages | 127.1 KB | 1,884 | 200 | 60 rows of 283 |
| Entities/Machines | 74.2 KB | 1,170 | 216 | 60 rows of 223 |
| Catalogue | 55.6 KB | 754 | 180 | filter dead — see verdicts |
| Research catalogue | 188.2 KB | 2,934 | 172 | 251 forms (one §12 Queue form per node) |
| Place | 118.0 KB | 2,423 | 188 | 83 forms (per-blueprint prepare) |
| Terrain | 490.0 KB | 6,595 | 232 | 6,144 SVG rects = ~457 KB of the page |
| Terrain, region at the bridge cap (16,384 cells, adversarial noise grid) | 1,268,579 B | 16,865 | 292 | responseEnd 90 ms but domInteractive 649 ms — ~560 ms of client parse; longtask 109 ms |
| Smallest page (Shutdown) | 38.7 KB | 298 | 100 | fixed overhead is 85% of it |

Interaction timings (median of 7, synchronous handler time):
table filters 0.1–0.4 ms (Storages 0.2, Machines 0.1–0.2,
Research over all 250 rows 0.3–0.4); table sorts 0.7–3.2 ms
(Storages 0.7–1.3, Products 1.0–3.2). Every client-filtered
table is server-paginated to at most 60 rows except the
research catalogue's 250.

## Verdicts per suspicion

- **(a) Badge poller runs hidden — CONFIRMED, fixed.** The 60 s
  `setInterval` had no visibility handling; a backgrounded page
  polled once per 65 s window, same as the visible control
  (headless Chromium does not flip `document.hidden` for
  inactive pages, so the code fact plus the not-in-front poll
  are the evidence). Cost per poll is small — a ~57 B JSON from
  the badge cache (the endpoint never touches the game),
  11.6–72.6 ms round trip — but it is invisible work, and the
  badge could sit stale for up to 60 s after returning. Fix:
  `poll()` returns early when `document.hidden`, and a
  `visibilitychange` listener polls immediately on return.
  Verified: forced-hidden → 0 polls; return → 1 poll at once.
- **(b) Table sorting/filtering cost — CLEARED.** All medians
  above are two orders of magnitude under the 100 ms bar. No
  debounce added: the handlers themselves are sub-millisecond,
  so debouncing would only add latency. Sort comparators do
  regex work per comparison and still land at ≤3.2 ms on
  60 rows.
- **(c) Per-row handlers vs delegation — CLEARED.** No script
  attaches a listener to a row. Listeners exist only per `<th>`
  (≤9 per table) plus one global filter function per page,
  invoked from the filter input.
- **(d) Terrain rendering — CONFIRMED, fixed (it is SVG, not
  canvas).** Per-cell `<rect>` markup was 93% of the Terrain
  page and ~94% of the capped-region page, with the parse cost
  measured above. Fix in `svg_heatmap`: the colour ramp is
  quantized to 64 buckets; each bucket renders as ONE `<path>`
  of horizontal same-bucket runs. No JavaScript, same inline
  SVG, same `role="img"` contract, colour error ≤ 1/64 of the
  ramp. Pixel-verified: 10 sampled cells (corners + interior)
  rasterize exactly to their bucket colours. After, capture
  grid (fixnow pair): wire 501,714 → 130,372 B, DOM
  6,595 → 508, FCP 224 → 88 ms, page height unchanged
  (1,917 px). After,
  capped region (noise grid, the worst case for run-merging):
  wire 1,268,579 → 462,395 B, DOM 16,865 → 545,
  domInteractive 649 → 98 ms. A smooth-terrain 16,384-cell
  grid models to ~165 KB of path data (Python model of the
  same algorithm); the noise figure is the measured one.
- **(e) Script parse/exec blocking first paint — CLEARED.**
  No script in `<head>`; totals above; FCP range above. The
  only head payload is the inline CSS, a founding constraint
  (single binary, no external assets) whose measured cost is
  the FCP range itself.
- **(f) Repeated inline SVG payload — MOSTLY CLEARED, one bug
  fixed.** The sprite is already deduplicated (`<symbol>` +
  `<use>`, 6,321 B once per page). But it defined
  `id="i-power"` TWICE: the first definition was the Feather
  "zap" glyph, the second the real power glyph, and browsers
  resolve `<use>` to the first — so Shutdown and Actions have
  been showing a lightning bolt (visible in the 390 px
  screenshots). The zap definition is renamed `i-zap`; the
  power glyph now renders. Duplicate ids after: none.
- **Catalogue filter — DEAD CONTROL, found by the crawl,
  fixed.** `/catalogue` called `wireTable('ctable','tfilter')`,
  a function that existed nowhere: a console `ReferenceError`
  on every load (both captures) and a filter input that changed
  nothing (typed "steel": 30 of 30 rows still visible).
  `TABLE_JS` now defines a real `wireTable(tableId, filterId)`
  — filter listener (skipped when the input already carries an
  inline `oninput`) plus the same header-click sort, scoped to
  the named table — and self-wires `#ptable` as before, so
  Products/Entities behaviour is unchanged and Trade's inert
  include stays an inert no-op. After: no console errors;
  filter "clone" → exactly 1 of 30 rows (the Clone command),
  cleared → 30; catalogue sort click works (22.1 ms first
  click, 30 rows).
- **Confirm-page countdown — NO JAVASCRIPT EXISTS.** The
  confirm sheets carry static "expires in 2 minutes" text;
  expiry is the server-side 120 s token. Nothing to optimise.
  (A live countdown would be a new feature, not a fix; not
  built.)
- **Fixed overhead (CSS + sprite + badge = 32,776 B/page) —
  REPORTED, not changed.** It dominates small pages by
  construction of the single-file architecture. Measured
  impact is the FCP range above; splitting assets would break
  the no-external-assets constraint for no measured gain on
  loopback.

## The 390 px pass — findings and dispositions

The product is desktop-first by contract (DESIGN §0); this pass
separates "desktop-shaped" from "broken":

- **Layout width expands to 439–841 px** on most pages (widest
  unbreakable content wins) and the user pans horizontally.
  Desktop-shaped; reported, not redesigned. All content probed
  was reachable by that pan.
- **Mega-nav Records section unreachable — FIXED.** On pages
  whose layout stays 390 wide (Issues, Log, Snapshots, Manual),
  the Records section sat at right edge 466 px with no page
  scroll covering it — absent from the visible nav row in
  screenshots. At ≤900 px `.meganav` now scrolls horizontally
  (`overflow-x:auto`): verified scrollWidth 450 > clientWidth
  358, and Records scrolls into view at right edge 374 ≤ 390.
  Mega panels are already `display:none` at ≤900 by V3 design;
  sections navigate by their landing links there.
- **Contract strip clipped mid-word — FIXED.** At ≤900 px the
  three contract cells stack full-width; verified unclipped
  (scrollWidth == clientWidth per cell). Desktop strip
  unchanged (media query only).
- **Wide-table hardening — added, honestly inert in this
  emulation.** `.tablecard` and `details.indgroup` get
  `overflow-x:auto` at ≤900 px. In the 390 emulation the boxes
  sized to their content (scrollWidth == clientWidth), so
  reachability there came from page pan, not these rules; the
  rules only bite when an ancestor constrains the box. Kept:
  strictly safer, zero desktop effect.
- Stat tiles, rail and forms reflow acceptably (rail goes
  horizontal per the existing ≤900 rules). No unreadable text
  found; no other dead controls at 390.

## Cost of the fixes

Badge visibility lines + the ≤900 rules add 333 B to every
page; the generalised `TABLE_JS` adds ~400 B more on the four
table pages. Every page's scrollHeight is byte-identical
before/after on the fixnow capture — the v3 height discipline
holds (Terrain 1,917, Products 2,843, Issues 3,695 on fixnow).
Full standard check sequence green after the round (cargo test
60/60, canon, plan parity, serializer pairs, QoS guards
216/216, structure, mod packages, place/designate/log page
checks, badge on both captures, audit-fail-loud, shutdown;
clippy `-D warnings` and fmt clean). SPEC Addendum XXIX records
the behaviour changes.
