# Tools

Development and verification tooling for the toolkit. Nothing here
is needed to *use* the companion or the mods — this is the kit that
keeps them honest. Most scripts are checks: they fail loudly and
exit non-zero when something is off.

| Tool | Purpose |
|---|---|
| `gen_catalogue.py` | Regenerates the command catalogue from the captured `/commands` list. The catalogue source is generated — never hand-edit it. |
| `plan_check.py` / `validate_plan.py` / `plan_to_json.py` | The Python reference plan engine: validation, verdicts, and YAML→JSON generation for game plans. |
| `check_plan_parity.py` | Proves the Python reference engine and the Rust runner return identical verdicts on every fixture state. |
| `check_place_page.sh` / `check_designate_page.sh` / `check_log_page.sh` / `check_badge.sh` | End-to-end page checks against the stub game server (the designation check independently re-verifies the sealed job file's hash in Python). |
| `check_audit_fail_loud.sh` | Breaks the database mid-flow and asserts the UI reports an applied-but-unaudited command instead of claiming success. |
| `check_serializer_pairs.py` | Build gate: every mod service that subscribes to saved calendar events must carry its serializer pair and manifest flag, or the build refuses. |
| `stub_game_server.py` | Serves a captured game state over the game's API shape, so the companion and the checks can run with no game installed. |
| `history_to_states.py` | Rebuilds per-month game states from a `/products/history` capture (the fixture generator). |
| `gen_blueprint_library.py` | Regenerates the companion's embedded blueprint library data. |
| `ensure-toolchain.sh` | Health-checks (and repairs) the local cross-build toolchain. |
| `diagnose_live.py` | One-shot diagnostic dump against a running game's API. |
