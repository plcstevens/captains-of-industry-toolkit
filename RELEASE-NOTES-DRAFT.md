# First-release draft — tag `coi-v0.1.0`

Drafted 2026-11-06 (release pipeline prep). **Do not publish until the
follow-up push has landed tonight's fixed sources in the repo** — a tag
pins the commit it points at, and a stale-source release is worse than
none. Paste the title and body below into the GitHub release form
(steps in RELEASE-RUNBOOK.md).

## Release title

```
CoI Companion coi-v0.1.0 — Designations, Design v2, §12 hardening — first release
```

## Release body

```markdown
First release of the **CoI Companion** built by GitHub Actions: the
local website (single `coi.exe`) that sits next to Captains of
Industry's in-game web API and gives you live insight, planning and a
strictly two-phase action flow over your own game.

### What's in this build

- **Terrain page** — whole-map height heatmap from CoiBridge's terrain
  routes, deposit tables with exact remaining quantities, and the
  current mining / dumping / levelling **designations read-back**
  (target-height bands per designation). Includes the **earthworks
  estimator**: pick a rectangle and a target height, get cut / fill in
  tiles³ and m³ before you commit a single truck.
- **Deposit history** — snapshots now record per-material deposit
  state (schema v4), so ore depletion is queryable over time.
- **§12 action flow hardening** — audit writes are fail-loud: if an
  applied command cannot be written to the audit trail, the UI says
  so instead of reporting silent success. Session/prepared-state
  locking is poison-tolerant, so one panic can't wedge the website.
- **Session runner parity** — research orders for already-researched
  nodes are held (skipped) exactly as the Python reference engine
  holds them.
- **Designations** — a /designate page plans flatten / dump / mining
  pads from live terrain data (edge-step warnings, cut/fill volumes,
  replaced-designation warnings) and writes sealed QoSBuilder 0.2.0
  job files at confirm; a read-back verdict panel (landed / partial /
  pending) verifies against CoiBridge 0.6.6's corner-exact route,
  because the game's own Add command cannot distinguish placed from
  silently refused cells.
- **Design v2 interface** — nested information architecture (top
  mega-navigation, left in-depth rail, no page over ~3 viewports),
  stats as icon + label + value + delta blocks (never in prose),
  number colour = sign of change only (green up / red down / muted
  flat; absolutes neutral), a CTA on every issue card, and a
  **must-fix badge** in the top bar that refreshes every minute.
  Pages: Overview / Issues / Products / Entities / Terrain /
  Research / Trade / Plans / Actions / Place / Designate / Guides /
  Catalogue / Snapshots / Log.
- **No login** — the site just opens (per-form tokens + a Host check
  remain as invisible request-integrity plumbing).

### Requirements

- Captains of Industry **Update 4.3 (v0.8.8)**, with `web_api_start`
  run in the in-game console after each game start.
- **CoiBridge 0.6.6** for the Terrain page's live data and
  corner-exact designation verification (the companion degrades
  cleanly without it).
- Windows: just run `coi.exe` and open http://127.0.0.1:8484 — no
  login. Your `coi.db` next to the exe carries over between versions.

### Not in this release

The C# mods (CoiBridge, the QoS suite, QoSBuilder) are **not** built
by Actions and are not attached here: they compile against the game's
own assemblies, which cannot live in this repo. They ship as zips in
the repo's `dist/` weekend bundle, built locally against a game
install. See INSTALL.md.

### Verify your download

`coi.exe.sha256` is attached alongside the binary — the hash inside
must match the `coi.exe` you downloaded.
```

## Prep notes (not part of the release)

- Tag shape `coi-v0.1.0` matches the workflow trigger glob `coi-v*`.
- The workflow builds from source on `windows-latest`, so the release
  binary's hash will **not** equal any locally built `dist/coi.exe`
  (local restage at prep time: 6,219,897 B, sha256 `01397c50…ae2924`).
  Verification is internal consistency: the attached `.sha256` must
  match the attached `coi.exe`.
