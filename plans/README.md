# Game plans

Machine-executable game plans for the companion: a plan says what to
build, research and check, in what order, and what evidence proves
each phase is done. The companion's session runner walks a plan and
*prepares* its orders — a human still confirms every consequential
step.

## What's here

- **`fresh-start-v1.yaml`** — the bundled plan: a full game plan for
  a fresh Update 4.3 (v0.8.8) start, in 8 phases. This is the plan
  the companion ships with.
- **`fresh-start-v1.json`** — the same plan in JSON, **generated**
  from the YAML (`tools/plan_to_json.py`). Never hand-edit it.
- **`fixtures/`** — recorded game-state sequences (early-game states
  and a 12-month growth set) used to exercise the runner and to keep
  the Python reference checker and the Rust engine in verdict
  parity.
- **Session and dry-run reports** — the recorded output of walking
  the plan over the fixture states.

## The format

Plans follow `GAME-PLAN-FORMAT.md` (repo root): phases, typed orders
(command / place-blueprint / manual-step / observe-only), and exit
gates that must hold over history — not just at a single instant —
before the runner advances.

Validate a plan before use:

```
python3 tools/validate_plan.py plans/<plan>.yaml
python3 tools/check_plan_parity.py   # Python vs Rust verdict parity
```
