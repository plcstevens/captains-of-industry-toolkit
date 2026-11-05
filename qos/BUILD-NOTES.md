# QoS build notes — round 1 (2026-11-04 evening)

Built: suite foundation + QoSChickens v0.1.0, per QOL-MODS.md's
architecture decision (small individual mods, one QoS suite) and
QOL-MOD-PREP.md §1 (chickens). Everything staged in `dist/`, no
uploads, no live-game contact. CoiBridge untouched.

## Artefacts

| File | Bytes | Contents |
|---|---|---|
| `dist/QoSCore-mod-0.1.0.zip` | 9,951 | `QoSCore/` (dll 7,168) |
| `dist/QoSChickens-mod-0.1.0.zip` | 20,050 | `QoSChickens/` (dll 13,824; **no** QoSCore.dll inside, by design) |
| `dist/QoS-suite-0.1.0.zip` | 32,199 | both folders + `QoS-SUITE-README.txt` |

Build: `qos/build/build.sh` — both projects **0 warnings / 0
errors** (net48, same toolchain as the bridge). Sources in
`qos/QoSCore/`, `qos/QoSChickens/`; build scaffolds in
`qos/build/{core,chickens}/` (one csproj per folder — see the
skill note on the shared-`project.assets.json` gotcha).

## QoSCore — as designed

`QoSLog` (prefixed suite logging + verbose flag), `SuiteRegistry`
(feature mods `Announce(id, version)`; logs suite composition),
`QoSCoreService` (applies core config: `verbose_logging`, default
false). Nothing else — no shared UI, no command bus, no state
store; those arrive only when a feature needs them (§3 may).

## QoSChickens — prep §1 refined design, implemented as specified

- Daily pass at `ICalendar.NewDayEnd` (post-birth, post-cull),
  moves issued as vanilla `MoveAnimalsCmd` through
  `IInputScheduler.ScheduleInputCmd` — the shipped path, shipped
  round-robin inside each move.
- Guards enforced by **exact simulation** of that round-robin
  (same peer enumeration order) before scheduling; only a
  predicted outcome passing every guard is scheduled, and the
  scheduled amount is the simulated amount. Guard list: receiver
  cap = min(capacity, `SlaughterLimit`); a farm at/above its cap
  gains zero; eligible = constructed + enabled only; nucleus
  floor on drains; per-day cap.
- One command per species per day; on guard failure the amount
  halves and retries within the pass, then the day is skipped.

### Deviations from the prep design (each with its reason)

1. **Master switch defaults OFF** (`enabled=false`), mode
   pre-selected to Equalise. The prep left the default "for
   Philip to pick" (Equalise vs Off); shipping the switch off
   with the gentlest mode pre-selected is the conservative
   reading of that open choice — first install is observable and
   inert until he enables it. One settings flip activates the
   prep's recommended mode.
2. **Growth-paused guard implemented as a species skip.** The
   prep says "skip such farms as receivers AND donors", but the
   shipped command's peer set is all constructed+enabled
   same-species farms and cannot exclude a peer (recon §1). Any
   move in a species with a growth-paused farm present would
   move that farm's animals against the player's setting. So the
   species is skipped that day (verbose-logged). Stricter than
   the prep text; the only faithful implementation over the
   shipped command path.
3. **Fill-first priority order comes from config**
   (`fill_first_farm_ids`, comma-separated entity ids). The prep
   says "Farm A first, then Farm B" without naming a source for
   the order; there is no in-game farm-naming UI to read, so the
   order is an explicit setting. Unconfigured/unparseable → log
   + no-op rather than an invented order.
4. **"Max per day" capped globally**, summed across species
   (prep implies a per-mod budget; with one shipped species the
   two coincide). Default 50 = one vanilla UI button-press.
5. **Cadence point is `NewDayEnd`**, not `NewDay`: the prep
   wanted "after the day's births/culls"; the day-end event is
   the one that guarantees both have run. No behavioural
   difference beyond ordering.
6. **Config via the game's own `config.json`/`ModJsonConfig`**
   (settings UI + save persistence) instead of a mod-folder JSON
   file — the prep/patterns showed the game mechanism exists
   (official README + PriorityManager's injected-mod pattern), so
   the suite uses it. Consequence: values persist per save; the
   readmes state this.

### NOT verified (needs first live install / first enabled days)

- Cross-assembly resolution of `QoSCore.dll` from QoSChickens
  through the manifest dependency (documented mechanism; the VM
  build cannot prove loader behaviour). If it fails live, the
  fallback is bundling a copy — rejected for now (duplicate-type
  hazard) — or merging chickens into core.
- `GetAllEntitiesOfType<AnimalFarm>()` enumeration order equals
  the order `FarmCommandsProcessor` materialises peers in
  (recon: INFERRED from the same shared API). If it differed, the
  simulation could mispredict by an animal or two at the margins
  — the guards still bound every outcome to safe ranges, but the
  first enabled days should be eyeballed against the log lines
  (each scheduled move logs its predicted spread).
- `IEvent` subscription from a service constructor (pattern
  copied from the game's own StatsManager) — compiled clean;
  confirm the "balancer armed" line appears in the log at load.
