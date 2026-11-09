# Release runbook — companion releases via GitHub Actions

Written 2026-11-06 (release pipeline prep). Ground truth verified
against the repo the same evening; re-verify the workflow file before
the first tag if anything below looks stale.

## The pipeline (verified in the repo)

- Workflow: **companion-release**, file `.github/workflows/release.yml`
  (repo blob SHA `7827089392e5dedd25ca90b89912d2b69ca2b6a3` at prep).
- Triggers: push of any tag matching **`coi-v*`**, plus
  `workflow_dispatch` (manual runs).
- Permissions declared in the workflow: `contents: write` — sufficient
  for creating a release and uploading assets. No change needed.
- Job (windows-latest): checkout → Rust stable → `cargo build
  --release` in `coi/` → stage `dist/superintendent.exe` +
  `dist/superintendent.exe.sha256`.
- **Tag runs** attach both files to a GitHub Release via
  `softprops/action-gh-release@v2`. That action **creates the release
  if none exists for the tag, or attaches to the existing one** — so
  both routes below work; no pre-created release object is required.
- **Manual runs** (workflow_dispatch) upload a workflow *artifact*
  named `coi-exe` instead, and never touch Releases. Dispatch ≠
  release.
- Proven green: run **#1**, manually dispatched by plcstevens on
  2026-11-05 10:11 UTC at commit `744f9b0`, **Success**, 1m 34s,
  artifact `coi-exe` 1.65 MB (digest `sha256:83206882…283f`). Run id
  42008622808. (The connector has no workflow-runs tool; run status
  was read off the public Actions page.)

### Known mismatch — action versions (cosmetic, not a blocker)

The workflow **in the repo** pins `actions/checkout@v4` and
`actions/upload-artifact@v4`. The local copy
(`~/workspace/coi-companion/.github/workflows/release.yml`) is the
revised **v5/v5** version Philip was sent on 2026-11-05; its re-upload
was never confirmed and, per the read-back above, **did not land**.
Everything else is byte-identical.

v4 still works — run #1 succeeded with only a Node.js 20 deprecation
warning (those actions are being force-run on Node 24). Fix, when
convenient: Philip re-uploads the local v5 file via the GitHub web UI
(Contents → `.github/workflows/release.yml` → edit/upload). Connector
pushes of workflow files 403 (the Muse app lacks the Workflows
permission) — **never push workflow files via the connector**.

## Preconditions for the first tag

1. **Repo current with the fixed sources.** The follow-up push of
   tonight's rounds (designation stages 2–3, bridge 0.6.6, audit
   remediation restage) must be landed and the head verified. At prep
   time the repo head was `f595b803` (INSTALL.md refresh) — tonight's
   source fixes were **not** yet in. A tag pins its commit: tagging
   early ships a stale binary.
2. Workflow file present as verified above.
3. No existing `coi-v*` tag or release (verified at prep: none).

## Cutting the release

**The connector cannot do this step.** Its tool surface has reads for
tags/releases (`list_tags`, `list_releases`, `get_tag`,
`get_release_by_tag`, `get_latest_release`) and `create_branch` for
branches — but **no** create-release and **no** create-ref/tag call.
So the tag is created by Philip in the web UI, which is also where the
release notes go:

1. Open the repo → **Releases** (right-hand sidebar on the **Code**
   tab) → **Draft a new release**.
2. **Choose a tag** → type `coi-v0.1.0` → **Create new tag:
   coi-v0.1.0 on publish**. Leave the target on **main**.
3. Paste the title and body from `RELEASE-NOTES-DRAFT.md`.
4. **Publish release**.

Publishing creates tag `coi-v0.1.0` on the head of `main`, which fires
the tag trigger; the workflow builds and attaches `superintendent.exe` +
`superintendent.exe.sha256` to the release Philip just published.

(Alternative if a tag ever gets pushed without the UI: the workflow
still fires and softprops creates a bare release named after the tag
with empty notes; edit the notes in afterwards.)

## Post-release verification

1. **Actions** tab → the `companion-release` run for `coi-v0.1.0`
   shows **Success**.
2. The release page lists **Assets (2)**: `superintendent.exe` and
   `superintendent.exe.sha256`. Asset size > 0 — expect `superintendent.exe` in the
   ~6 MB range.
3. Download `superintendent.exe`; its SHA-256 must equal the value inside the
   downloaded `superintendent.exe.sha256`. It will **not** match any locally
   built exe — Actions rebuilds from source; internal consistency is
   the check.
4. Read back with the connector: `get_release_by_tag`
   (`owner: plcstevens`, `repo: captains-of-industry-toolkit`,
   `tag: coi-v0.1.0`) — confirms the release object and its assets.

## Standing rule for future releases

- Tag **after each companion restage batch that Philip will
  install**, named `coi-v<semver>` (patch bumps for fix batches,
  minor for feature batches). Draft the notes from the NIGHT-LOG
  entries since the previous tag.
- **Mods are never in Actions releases.** CoiBridge / QoS /
  QoSBuilder compile against the game's own assemblies, which cannot
  live in this repo; they keep shipping as locally built zips in the
  `dist/` weekend bundle.
- Any workflow-file change is Philip's, via the web UI — the
  connector's 403 on `.github/workflows/*` is structural, not a
  retry candidate.
