# First-release draft — tag `coi-v0.1.0`

Re-drafted 2026-11-09 for the Superintendent rename + Design V3 (the
release debuts under the new name — no published asset ever carried
the old one). **Do not publish until the follow-up push has landed
the renamed sources in the repo** — a tag pins the commit it points
at, and a stale-source release is worse than none. Paste the title
and body below into the GitHub release form (steps in
RELEASE-RUNBOOK.md).

## Release title

```
Superintendent coi-v0.1.0 — Designations, Design V3, §12 hardening — first release
```

## Release body

```markdown
First release of **Superintendent**, built by GitHub Actions: the
local website (a single `superintendent.exe`) that sits next to
Captains of Industry's in-game web API and gives you live insight,
planning and a strictly two-phase action flow over your own game.
It runs entirely on your machine — no account, no cloud, no
telemetry; the only thing it ever contacts is your game.

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
- **Design V3 interface** — five domains (Monitor · Workshop ·
  Guidance · Plans · Records) over the V2 design system: nested
  information architecture (top mega-navigation, left in-depth rail),
  stats as icon + label + value + delta blocks (never in prose),
  number colour = sign of change only (green up / red down / muted
  flat; absolutes neutral), a CTA on every issue card, and a
  **must-fix badge** in the top bar that refreshes every minute.
  Every page declares a contract at its head — what it owns, whether
  it acts or only informs, and what data it needs to be truthful.
- **Guidance as a domain** — *Learn the game* (failure-mode guides,
  each opening with what's live in your factory), *Learn the tool*
  (the manual), and *Why it suggests*: the reasoning rules behind
  the issues board, rendered from a single canon that the board's
  own copy quotes — the explanation and the behaviour can't drift
  apart, and a drift check in the repo fails the build if they try.
- **No login** — the site just opens (per-form tokens + a Host check
  remain as invisible request-integrity plumbing).

### Requirements

- Captains of Industry **Update 4.3 (v0.8.8)**, with `web_api_start`
  run in the in-game console after each game start.
- **CoiBridge 0.6.6** for the Terrain page's live data and
  corner-exact designation verification (Superintendent degrades
  cleanly without it).
- Windows: just run `superintendent.exe` and open
  http://127.0.0.1:8484 — no login. Your `coi.db` next to the exe
  carries over between versions (the data files keep their names:
  `coi.db`, `coi.toml`).

### Not in this release

The C# mods (CoiBridge, the QoS suite, QoSBuilder) are **not** built
by Actions and are not attached here: they compile against the game's
own assemblies, which cannot live in this repo. They ship as zips in
the repo's `dist/` weekend bundle, built locally against a game
install. See INSTALL.md.

### Verify your download

`superintendent.exe.sha256` is attached alongside the binary — the
hash inside must match the `superintendent.exe` you downloaded.
```

## Prep notes (not part of the release)

- Tag shape `coi-v0.1.0` matches the workflow trigger glob `coi-v*`
  (the tag scheme deliberately keeps its name; only the product was
  renamed).
- The workflow builds from source on `windows-latest`, so the release
  binary's hash will **not** equal any locally built
  `dist/superintendent.exe`. Verification is internal consistency:
  the attached `.sha256` must match the attached `superintendent.exe`.
- The workflow file itself must be re-pasted by Philip (web UI):
  see RELEASE-WORKFLOW-DRAFT.md (dev/) for the current text —
  artefact names are `superintendent.exe` / `superintendent.exe.sha256`,
  upload-artifact name `superintendent-exe`, workflow name
  `superintendent-release`.
