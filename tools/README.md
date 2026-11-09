# Tools

Development and verification tooling for the toolkit. Nothing here
is needed to *use* Superintendent or the mods — this is the kit that
keeps them honest. Most scripts are checks: they fail loudly and
exit non-zero when something is off.

**The standard check sequence** (run before calling a round done;
the page checks need the debug binary built):

```sh
cargo test --manifest-path coi/Cargo.toml
python3 tools/check_canon.py
python3 tools/check_plan_parity.py
python3 tools/check_serializer_pairs.py
python3 tools/check_repo_structure.py
python3 tools/lint_mod_packages.py
for s in place designate log badge; do bash tools/check_${s}_page.sh; done
bash tools/check_audit_fail_loud.sh
CAPTURE=dev/captures/live-synthetic-fixnow bash tools/check_badge.sh
```

**The lint stack** (configs at the repo root; `.pre-commit-config.yaml`
wires it for commits): `cargo fmt` + `cargo clippy -- -D warnings`
(Rust), `ruff check` + `ruff format` (Python, `pyproject.toml`),
`shellcheck` (shell), `markdownlint` (Markdown,
`.markdownlint.json` — pragmatic set: MD013/MD025/MD033/MD036/
MD040/MD041 off by design — long prose lines, multi-H1 addenda
documents, inline HTML, bold lead-ins as house style, unlabelled
fences and non-H1 openings are this repo's conventions, not
defects), `yamllint` (`.yamllint.yml`), `actionlint` against the
workflow draft. Adding the structure check to CI is already in
the draft workflow (dev/RELEASE-WORKFLOW-DRAFT.md) — no separate
step needed when the owner pastes it.

| Tool | Purpose |
|---|---|
| `gen_catalogue.py` | Regenerates the command catalogue from the captured `/commands` list. The catalogue source is generated — never hand-edit it. |
| `plan_check.py` / `validate_plan.py` / `plan_to_json.py` | The Python reference plan engine: validation, verdicts, and YAML→JSON generation for game plans. |
| `check_plan_parity.py` | Proves the Python reference engine and the Rust runner return identical verdicts on every fixture state. |
| `check_place_page.sh` / `check_designate_page.sh` / `check_log_page.sh` / `check_badge.sh` | End-to-end page checks against the stub game server (the designation check independently re-verifies the sealed job file's hash in Python). |
| `check_audit_fail_loud.sh` | Breaks the database mid-flow and asserts the UI reports an applied-but-unaudited command instead of claiming success. |
| `check_serializer_pairs.py` | Build gate: every mod service that subscribes to saved calendar events must carry its serializer pair and manifest flag, or the build refuses. |
| `check_canon.py` | Drift gate: every wired number in the canon (`docs/canon/`) must agree with the code it cites, and the generated `canon.json` / agent rendering must be fresh. |
| `canon_to_json.py` | Generates `docs/canon/canon.json` (embedded by the server) and `.llms/canon-for-agents.md` from the canon YAML source. |
| `check_repo_structure.py` | Structure gate: the lean repo layout (root allowlist, per-folder readmes, markdown reference integrity, the mod invariant, `.llms/` index freshness). |
| `lint_mod_packages.py` | Mod-package lint (the Hub-submission gate): manifest/readme consistency, dependency resolution, the calendar-subscriber rule, stray-file and privacy sweeps, suite consistency. Source mode by default; `--zips` lints the packages as shipped. |
| `stub_game_server.py` | Serves a captured game state over the game's API shape, so Superintendent and the checks can run with no game installed. Captures are self-contained fixtures (game payloads resolve capture-dir first); the committed captures live in `dev/captures/`. |
| `history_to_states.py` | Rebuilds per-month game states from a `/products/history` capture (the fixture generator). |
| `gen_blueprint_library.py` | Regenerates the companion's embedded blueprint library data. |
| `ensure-toolchain.sh` | Health-checks (and repairs) the local cross-build toolchain. |
| `diagnose_live.py` | One-shot diagnostic dump against a running game's API. |
