# Canon — agent rendering

GENERATED from docs/canon/*.yaml by tools/canon_to_json.py.
Do not edit. Cite entries by id; the YAML files are the source.

## The issues board (`board`)

### board.quadrants — The four quadrants

Rule: Quadrant = f(importance, urgency) for every deficit-class finding: importance HIGH + urgent -> Fix now; importance HIGH -> Fix later; urgent -> Watch; otherwise Ignore for now. These four labels are THE player vocabulary for severity (canon/README.md); agent-side severity scales are never shown to players.

Source: code · Status: wired

### board.importance — What counts as important

Rule: importance(product): HIGH (3) for the listed backbone products, LOW (1) for the listed waste/fill products, otherwise 2. Matching is by substring on the product id, evaluated in code order.

Numbers: high_count = 20 products; low_count = 11 products

Source: code · Status: wired

### board.urgency — What counts as urgent

Rule: urgent := cover_months < 3.0, where cover = quantity / last-month consumption.

Numbers: urgent_cover_months = 3.0 months

Source: code · Status: wired

### board.stall-confirmation — Stalled production

Rule: stalled := produced(last month) == 0 AND consumed(last month) > 0 AND NOT import-fed AND NOT seasonal AND cover < 36 months. Confirmed when production was zero in >= 2 of the last 3 finished months; with no monthly history yet, an unconfirmed stall is shown rather than hidden.

Numbers: cover_max_months = 36 months; confirm_zero_months = 2 of last 3 finished months

Source: code · Status: wired

### board.import-fed-products — Products you import on purpose

Rule: import_fed(P) := an ACTIVE contract in the CoiBridge /coi/v1/contracts payload has productToBuy == P, derived at fetch time into the import-fed set. Exempt from: the stall detector, the seasonal long-window gate (same predicate), and the forecasts slope outlook (a stock trend under ship deliveries is delivery rhythm, not decline). NOT exempt from deficit findings or the blended runway: a draining stock is a planning fact whatever its source. Only active contracts count — available offers are not a feed, and quick trades and loans are one-off deliveries, not standing feeds. When the contracts payload is absent (no bridge / pre-v0.6 bridge) the set is empty and no product is exempt: the exemption applies only on positive evidence. (Pre-C-3 the engine hardcoded this exemption to coal; that hardcode is gone — coal is exempt exactly when a contract establishes it, like any product.)

Source: code · Status: wired

### board.deficit-horizon — Running a deficit

Rule: deficit := net(last month) < 0 AND quantity > 0 AND NOT seasonal AND runway < 24 months.

Numbers: runway_max_months = 24 months

Source: code · Status: wired

### board.structural-horizon — A slow structural deficit

Rule: structural := runway in [24, 120) months AND long-window trend declining AND net(this month) < 0 AND NOT seasonal. Quadrant: Fix later if importance HIGH, else Watch.

Numbers: runway_max_months = 120 months

Source: code · Status: wired

### board.seasonal-window — Seasonal products

Rule: is_seasonal := >= 4 finished months AND >= half of them zero-production AND total production > 0 AND peak month >= max(mean consumption, 1) AND peak >= 2x max(mean production, 1). Fallback without history: produced last year > 0 with this and last month both zero. Seasonal deficit := long-window net < 0 AND runway < 36 months -> Fix later (HIGH) or Watch.

Numbers: seasonal_runway_max_months = 36 months; min_finished_months = 4 months; peak_vs_mean_factor = 2 x

Source: code · Status: wired

### board.storage-full — Storage full — a finding, not a warning colour

Rule: Finding (storage-full) := stored >= capacity AND capacity >= 200 AND the product is still produced. Parked variant := full AND no production and no draw -> Ignore for now, explained as a filled buffer / war chest. Flag (Entities) := stored >= 90% of capacity. The 90% flag and the 100% finding are different things on purpose: flag vs finding.

Numbers: finding_fill = 100 percent; flag_fill = 90 percent; capacity_floor = 200 units

Source: code · Status: wired

### board.overcap — Stored above capacity

Rule: overcap := stored > capacity AND capacity >= 200 -> Watch.

Numbers: capacity_floor = 200 units

Source: code · Status: wired

## Forecasts (`forecasts`)

### forecasts.runway-blend — Runway: when a stock runs out

Rule: runway := quantity / -net where net blends this-month and last-month net (mean of the two); listed when net < 0 AND runway < 36 months.

Numbers: horizon_months = 36 months

Source: code · Status: wired

### forecasts.fill — Fill: when a storage fills up

Rule: fill := headroom / net for capacity >= 200; listed when net > 0 AND months-to-full < 12.

Numbers: horizon_months = 12 months; capacity_floor = 200 units

Source: code · Status: wired

### forecasts.slope — Slope outlook for key products

Rule: Slope runway := least-squares trend of quantity over history, evaluated for importance-HIGH products only; listed when the projected runway is < 120 months. Excludes import-fed products (stock follows delivery rhythm, not trend — see board.import-fed-products and philosophy.coal-buffer) and seasonal products (own rule).

Numbers: horizon_months = 120 months

Source: code · Status: wired

### forecasts.seasonal — Seasonal runway

Rule: seasonal runway := quantity / -(long-window monthly net); listed when < 120 months.

Numbers: horizon_months = 120 months

Source: code · Status: wired

### forecasts.horizons-together — All the horizons, in one place

Rule: Horizon register: urgent 3 / fill 12 / deficit 24 / runway 36 / seasonal-deficit 36 / structural 120 / slope 120 / seasonal-runway 120 (months). The CLI forecast summary prints this same register; the two must not diverge (previously the summary line omitted the 120-month paths — reconciled in v3).

Source: code · Status: wired

## Machine productivity (`productivity`)

### productivity.working-share — When a machine type counts as unproductive

Rule: Working share per machine prototype over the trailing 30 days of snapshots, Wilson 95% confidence interval. FIRE := CI upper < 0.50 AND point estimate < 0.45. SEVERE := point < 0.25 AND CI upper < 0.35. CLEAR := CI lower > 0.60. A borderline verdict must repeat in the second half of the window (second-half CI upper < 0.55) before it is confirmed.

Numbers: fire_ci_upper = 0.5 share; fire_point = 0.45 share; severe_point = 0.25 share; severe_ci_upper = 0.35 share; clear_ci_lower = 0.6 share; confirm_second_half_ci_upper = 0.55 share; window_days = 30 days

Source: code · Status: wired

### productivity.causes — Why it's idle matters more than that it's idle

Rule: Cause shares over non-working observations: starved >= 0.30 -> INPUT-STARVED; blocked >= 0.30 -> OUTPUT-BLOCKED; parked >= 0.50 -> PARKED (suppresses the alert entirely); mixed >= 0.20 -> MIXED; else OTHER. Verdicts aggregate >= 80% of a prototype's instances (population share) with at least 3 instances before firing.

Numbers: parked_share = 0.5 share; starved_share = 0.3 share; blocked_share = 0.3 share; mixed_share = 0.2 share; min_instances = 3 machines; population_share = 0.8 share

Source: code · Status: wired

### productivity.evidence-classes — Measured, estimated, or not enough data

Rule: MEASURED := >= 96 samples AND span >= 27 days AND max gap <= 3 days. ESTIMATED := >= 30 samples AND span >= 24 days AND max gap <= 7 days. Otherwise INSUFFICIENT (no verdict). A second, stricter bar applies before a low-working-share verdict fires on estimated evidence alone.

Numbers: measured_min_samples = 96 samples; estimated_min_samples = 30 samples; measured_span_days = 27 days; estimated_span_days = 24 days

Source: code · Status: wired

### productivity.ramping — New machines get time to ramp

Rule: ramping := instance count grew by factor >= 1.5 across the window.

Numbers: ramp_growth = 1.5 x

Source: code · Status: wired

### productivity.configurable — These thresholds are defaults, and they're yours

Rule: Thresholds load from coi.toml [productivity], falling back to the defaults in this file. Renderings of these rules must read the effective values live (the Why page does); the numbers here are the shipped defaults that check_canon.py pins to the code defaults.

Source: code · Status: wired

## Priorities (`priorities`)

### priorities.scale — The priority scale

Rule: Internal scale 0..14, lower = served sooner. Named points: HIGHEST = 0, HIGH = 8, DEFAULT = 9 (the game's GeneralPriorities table, verified by recon and recorded in the QoS actuator's source notes). The companion displays and writes internal values; the game's own UI displays internal + 1.

Numbers: highest = 0 priority; high = 8 priority; default = 9 priority; scale_max = 14 priority

Source: code · Status: wired

### priorities.manual-groups — The four manual groups (a house convention)

Rule: Group defaults (internal): General 9, Import 8, Export 8, Generator 9. Applying a group writes BOTH the global priority and the per-machine / per-storage priority (the game's per-entity value only takes effect when it differs from the global one for its class). Status note: the groups are a documented convention (BEHAVIOURS, the QoS policy doc); they are applied through the QoS tooling, not a website control.

Numbers: general = 9 priority; import = 8 priority; export = 8 priority; generator = 9 priority

Source: docs · Status: verified

### priorities.actuator — The QoS actuator's rules (if you enable it)

Rule: Actuator (ships disabled): after persistence_days = 3 consecutive starved daily passes, step priority -1 (toward higher precedence), at most max_nudge_steps = 2 steps below the prototype default; cooldown 2 days between nudges of the same machine; at most 10 actions per in-game day; restores step back toward the default while the machine works, only for values the actuator itself could have set. BLOCKED machines are never priority problems and are never nudged; PARKED machines are never touched.

Numbers: persistence_days = 3 days; max_nudge_steps = 2 steps; cooldown_days = 2 days; max_actions_per_day = 10 actions

Source: code · Status: wired

### priorities.groups-xor-actuator — Manual groups or the actuator — never both on one field

Rule: Per entity field, exactly one of {manual groups, QoS actuator} may own priorities. Mixed ownership is a configuration error, not a strategy.

Source: docs · Status: verified

## Earthworks & designations (`earthworks`)

### earthworks.cells-and-steps — Designation cells and the one-step rule

Rule: Designations snap to a global 4x4-tile cell grid. Adjacent cell heights differ by 0 or +/-1 tile. Heightmaps sampled per tile from the bridge are exact (CoiBridge 0.6.5+); verdicts after applying a job are exact per-cell read-backs (0.6.6+).

Numbers: cell_size = 4 tiles; max_neighbour_step = 1 tiles

Source: code · Status: verified

### earthworks.spike-filter — The spike filter

Rule: A centre cell is a spike when its bedrock-top height is more than 4.0 tiles below the median of its 8 neighbours' bedrock tops, with all 8 neighbours present. Pinned definition (used everywhere in this canon): for an even count, the median is the mean of the two middle values.

Numbers: spike_residual = 4.0 tiles; spike_neighbours = 8 neighbours

Source: code · Status: wired

### earthworks.purity-gates — How pure is pure enough — the L0–L4 gates

Rule: Purity gates (min ore height in tiles / min bottom-layer density / min component size in cells): L0 0 / 0 / 0 (permissive), L1 0.25 / 0.05 / 2, L2 0.75 / 0.20 / 6, L3 2.0 / 0.50 / 20, L4 3.0 / 0.75 / 40. The component floor height is a percentile depth into the seam; strength 5 reads the median depth.

Numbers: l1_min_ore_height = 0.25 tiles; l2_min_ore_height = 0.75 tiles; l3_min_ore_height = 2.0 tiles; l4_min_ore_height = 3.0 tiles; l4_component = 40 cells

Source: code · Status: wired

### earthworks.smoothing — Smoothing and deepen-only

Rule: Plans clamp inter-cell height differences to MaxHeightDiff = 1 tile, applied deepen-only: planned floors never exceed the current terrain.

Numbers: max_height_diff = 1 tiles

Source: code · Status: verified

### earthworks.collapse-bands — How dumped material settles

Rule: Retaining-wall-free collapse rates (horizontal per vertical), from the game's assemblies: landfill 0.4–1.5 band by material — Dirt 0.4, Sand/Gravel/Compost 0.5, Rock/Slag/ore 0.6–0.8, molten and liquid fills higher. Ocean dumping tops out at height 0. The guide's table is the player rendering; this entry pins the shape of the rule.

Source: docs · Status: verified

## Plans (`plans`)

### plans.gates-fresh-start — The fresh-start plan's phase gates

Rule: fresh-start-v1 exit gates (plans/fresh-start-v1.yaml): product.quantity Product_Iron >= 100; Product_Coal >= 200; Product_ConcreteSlab >= 100; Product_Diesel >= 300 (with diesel produced last month > 0); product.net_flow Product_ConstructionParts (lastMonth) >= 0; settlement.months_of_food >= 3 sustained for 1 month. Gates are checked against observed state only — never against projections.

Numbers: iron_stock = 100 units; coal_stock = 200 units; concrete_stock = 100 units; diesel_stock = 300 units; months_of_food = 3 months

Source: plan · Status: wired

### plans.food-floor — Three months of food is the security line

Rule: settlement.months_of_food >= 3 := secure; < 2 triggers plan warnings; < 1 is an emergency entry condition (fresh-start-v1 settlement guards).

Numbers: secure_months = 3 months; emergency_months = 1 months

Source: plan · Status: verified

### plans.runner-holds — The runner holds what it cannot prove

Rule: Research orders whose node is already researched at prepare time are HELD with the reason recorded (there is no idempotent research-start command in the game). All other orders prepare normally; nothing is applied without the §12 confirm step.

Source: plan · Status: verified

## Play philosophy (`philosophy`)

### philosophy.triage-order — The order to fix things in

Rule: Agent triage legs map onto the board, they do not compete with it: SURVIVE (food/water/power deficits) -> board Fix now on those products; STRANGLERS (diesel incl. reforming draw, maintenance, power capacity) -> Fix now / Fix later plus forecasts; SLOPES -> the 120-month slope and structural entries. The board quadrant is what the player sees; the triage leg explains why a class of finding outranks another.

Source: mixed · Status: verified

### philosophy.death-spiral — The death spiral: fuel, maintenance, power

Rule: The engine detects fuel/product legs (board + forecasts). There is deliberately NO maintenance detector: maintenance has no stockpile product to trend. The gap is documented here and in the Maintenance guide rather than papered over with a proxy metric.

Source: mixed · Status: verified

### philosophy.maintenance-floor — Maintenance below 75% starts breaking machines

Rule: Breakdown risk begins below 75% maintenance and rises as it falls (~50% monthly breakdown chance at 0%). The three maintenance tiers are not interchangeable — a depot only repairs its own tier. Figures from the game's data; player chapter = the Maintenance guide.

Numbers: breakdown_threshold = 75 percent; breakdown_chance_at_zero = 50 percent per month

Source: docs · Status: verified

### philosophy.section12 — Nothing changes without prepare, confirm, audit

Rule: §12 invariant: prepare -> confirm -> apply -> audit-log, for every mutating route, no exceptions in v3. Command catalogue posture at ship time: 15 verified live / 224 wired-but-unverified / 1 excluded (GameConsoleCmd, policy exclusion).

Numbers: verified_commands = 15 commands; excluded_commands = 1 commands

Source: code · Status: verified

### philosophy.research-queue — Research queue discipline

Rule: Queue discipline: prefer research that removes a current blocker; defer nodes whose inputs the factory cannot yet supply. The site's Research page shows researched/current/queued from the bridge only — there is no snapshot fallback for research state.

Source: docs · Status: verified

### philosophy.steel-sizing — Sizing steel: the 24-per-minute rule of thumb

Rule: The steel expansion hint in issue copy assumes ~24 steel/min per furnace+caster line. Status: HEURISTIC — inherited from layout practice; no assembly-verified ratio has been recorded for it yet. Recorded here so the number has a stated provenance (it previously existed only inside fix-hint text).

Numbers: steel_per_line_per_min = 24 steel/min

Source: code · Status: heuristic

### philosophy.coal-buffer — The coal buffer: two hours of draw

Rule: Coal import buffer >= 120 minutes of draw at plant scale (issue-copy heuristic, previously unsourced inside fix-hint text; recorded here with its status). Related: forecasts exclude import-fed products from the slope outlook because buffer-managed products follow delivery rhythm, not trend (canon: board.import-fed-products).

Numbers: buffer_minutes = 120 minutes

Source: code · Status: heuristic

### philosophy.unma-not-adopted — One severity vocabulary, deliberately

Rule: DELIBERATE NON-ADOPTION: the four-level advisory severity scale used in the hosting agent's internal UNMA audit (CRITICAL/HIGH/MEDIUM/LOW) is not a canon input and is never rendered to players. The board quadrants (board.quadrants) are the only severity vocabulary.

Source: mixed · Status: deliberate-none

### philosophy.guide-gaps-closed — Coverage: maintenance, research, earthworks

Rule: Closes the v3 audit's coverage gap (no Maintenance guide; the research and earthworks guide documents existed in the repo but were never rendered into the site). Canon entries philosophy.maintenance-floor and philosophy.research-queue are the auditable cores of those chapters.

Source: mixed · Status: verified
