# Superintendent — Guide Research Digest

Failure-mode-first guide material for the companion website. Researched 2026-11-02 from the official wiki, Captain's Diary / update posts, the official translation hub (exact in-game strings), and community practice (Steam discussions, one LP analysis). Every entry carries its source. Where sources disagree or a claim is player folklore, that is stated.

Version context: Philip plays v0.8.8 (Update 4.3). Trains were reworked across Update 4.x — Train Networks arrived in 4.1, universal wagons/waypoint rules in 4.2, and per-product station arrival rules in 4.3. Pre-4.1 train advice is largely obsolete.

---

## Terminology check (verified names)

**Priority systems — there is no single "priority":**

- **Entity "Priority"** (per machine/building, on the right side of its UI): sets the priority for that building's consumption of **power, workers (manpower), maintenance and unity**, and also affects **cargo priority for logistics**. Source: Captain's Diary #12 — "The player can now set a general priority for each machine & building to affect their consumption of electricity, workers, maintenance, and unity. This also affects cargo priority for logistics." Wiki balancer pages repeat: "On the right side of the UI is 'Priority', which sets the power/manpower/maintenance priority for this building."
  - <https://coigame.com/Blog/captain-s-diary-12-beta-release-date>
  - <https://wiki.coigame.com/index.php?title=Pipe_Balancer&curid=798&oldid=12653>
- **Electricity generation priority**: a *separate* priority set that exists only for generators, so players can build backup generation that only runs when higher-priority generation can't cover demand (Diary #12: "A dedicated set of priorities for electricity generation. With this system players can now build backup electricity generation."). Players refer to its levels as P1, P2, P3… (Steam power thread below).
- **Storage "priority" is sliders, not a number.** Exact in-game strings (official translation hub):
  - Green slider: **"Trucks keep full if under:"** — trucks prioritize deliveries to fill the storage up to the marker.
  - Red slider: **"Trucks keep empty if above:"** — trucks actively remove stock above the marker.
  - Belt/pipe sliders: **"Receive from belts/pipes only if below:"** and **"Output to belts/pipes only if above:"**
  - Presets labelled **"Keep full"** and **"Empty"**.
  - Source: <https://hub.coigame.com/Translate?projectId=1&stringKey=StoredProduct__ExportSliderLabel&languageId=34> and <https://hub.coigame.com/Translate?projectId=1&stringKey=StoredProduct__InputFromBeltSliderLabel&languageId=23>
  - Storages additionally have per-storage **vehicle logistics toggles (on / auto / off)** for import and export — "On" = vehicles import/export no matter what; "Auto" = same, except when the requirement is met by conveyors; "Off" = no vehicle import/export (dev-confirmed in the Steam thread under Storage below). Philip's build also exposes explicit "disable logistics input/output" toggles per storage.
- **Balancer prioritization**: balancers (Flat / U-shape / Pipe) have their own **"Prioritization"** per port (a prioritized port beats a non-prioritized one; all prioritized ports are equal to each other) and **"Enforce strictly even inputs / outputs"** toggles. Enforcing strictly-even resets any port priorities. Source: wiki Pipe Balancer page (URL above).
- **Custom priorities**: the game also exposes named *custom* priority channels (Philip's build: `SetCustomPriorityCmd` with a priorityId string, alongside `SetGlobalPriorityCmd`). These are the least publicly documented of the systems — no solid wiki page was found; treat any specifics as unverified and prefer the three systems above in guide copy.
- **Train-side priority**: within a station group, stations carry a priority (train picks highest priority, then nearest, among stations whose arrival conditions allow); Train Networks dispatch by source/destination station priority with round-robin. Source: wiki Train Schedules draft (unofficial user sandbox page — flag as such): <https://wiki.coigame.com/index.php?title=User:Timj11dude/sandbox&diff=12555&oldid=12537>

**Transport tiers (per-minute throughput, current wiki revisions, June 2025):**

- **Flat Conveyor** (unit products): I = **60/min**, II = **200/min**, III = **450/min**. (III was 400 in older revisions; the June 2025 revision raised it to 450.)
  - <https://wiki.coigame.com/index.php?title=Flat_Conveyor_II&curid=769&oldid=11624>
  - <https://wiki.coigame.com/index.php?title=Flat_Conveyor_III&diff=cur&oldid=6665>
- **U-shape Conveyor** (loose products): same ladder; III = **450/min**.
  - <https://wiki.coigame.com/index.php?title=U-shape_Conveyor_III&diff=cur&oldid=2051>
- **Pipe** (fluids/gases): I = **60/min**, II = **200/min**; **Pipe IV exists in Update 4** (listed among Pipe variants on the wiki Pipe page). Pipe III is the one disputed number: one wiki revision still shows **400/min**, while the Update 4 LP analysis's math (a T3 pipe supports at most 7.5 distillers drawing 60/min each) implies **450/min**. Flagged as unresolved between sources — the companion's game-extracted dataset should win.
  - <https://wiki.coigame.com/index.php?title=Pipe&diff=11618&oldid=9000>
  - <https://wiki.coigame.com/index.php?title=Pipe_III&oldid=2529>
  - <https://gist.github.com/mw-stanley/d24c4ae4b1a86f390087333c3f5d236a>
- **Molten Channel** is the only transport for molten metal/glass — pipes refuse them (wiki Pipe page, URL above).
- Pipes carry **only one product type at a time**, and can double as storage for un-storable fluids such as Steam (High) (wiki Pipe page).
- Update 4.x raised **T4 storage throughput** "to accommodate 4 T4 pipes" (patch note quoted in the Steam arrival-rules discussion below) — storage I/O rate, not just belt/pipe rate, can be the binding cap.

---

## 1. Transport — belts, pipes, trucks

**Failure mode** — A trunk line is asked to carry more than one segment's tier allows (e.g. several production blocks merged onto a single T1 belt at 60/min).
**Symptoms** — The belt is visibly saturated end-to-end, upstream machines cycle on/off with "output full", downstream machines starve despite healthy production further up. Everything looks "connected" — the cap is arithmetic, not connectivity.
**Prevention & fix** — Sum the recipe rates merging onto each segment and compare against the tier ladder (60 / 200 / 450 per min). Upgrade the *trunk*, not just the last segment; or split the flow across parallel lines with a balancer. The Update 4 refinery LP analysis sizes whole builds around this: "6 distillation columns, because that's the max consistent throughput of a Tier 3 pipe."
**Source** — <https://gist.github.com/mw-stanley/d24c4ae4b1a86f390087333c3f5d236a> ; tier figures: <https://wiki.coigame.com/index.php?title=Flat_Conveyor_II&curid=769&oldid=11624>

**Failure mode** — Merge/split without a balancer: junctions take from branches unevenly, so one branch backs up while another starves.
**Symptoms** — One input line permanently jammed, the other empty; two identical production lines utilized unequally; a boiler intermittently fuel-starved while a less important consumer on the same line eats first.
**Prevention & fix** — Use the tier's Balancer at merges/splits. Port prioritization: any prioritized port beats any non-prioritized port. Wiki use cases: priority *input* so a storage is emptied before accepting stock from a main line; priority *output* "to ensure that sufficient Coal is used to continuously run a Boiler making Electricity before allowing a Basic Distiller to be fed"; strictly-even outputs to split one product evenly across identical machines (e.g. cement from a kiln across four mixers).
**Source** — <https://wiki.coigame.com/index.php?title=Pipe_Balancer&curid=798&oldid=12653>

**Failure mode** — Wrong transport family for the product: molten metal/glass pushed toward pipes, or a second fluid product introduced into an already-committed pipe.
**Symptoms** — Nothing flows at all, or a pipe network silently stops when a different fluid reaches a junction.
**Prevention & fix** — Molten metal and glass move only in Molten Channels; pipes carry exactly one product type at a time — one product per pipe network, no exceptions.
**Source** — <https://wiki.coigame.com/index.php?title=Pipe&diff=11618&oldid=9000>

**Failure mode** — A pipe run is used as an accidental bottleneck at a dock: ship unloading outruns the pipe away from the module.
**Symptoms** — Cargo ship sits docked and full, module storage full, while the refinery at the other end starves — supply arrives in shipload bursts, not a stream.
**Prevention & fix** — Match the pipe tier to the module's unload rate and buffer the burst in storage at the dock: the small Fluid Module unloads at 125/min, "matched with one Pipe II" — then budget for ship round-trip time (3–6 months in the wiki's table) in the buffer, not just the unload rate.
**Source** — <https://wiki.coigame.com/Fluid_Module_(S)>

**Failure mode** — Trucks idle while a building starves: the storage/vehicle logistics settings contradict each other.
**Symptoms** — Full storages sit untouched next to a starving plant; a dozen unassigned trucks in standby do nothing. Dev-answered thread diagnosis: a "Keep Full" slider on the *source* storage prevents trucks from exporting from it; vehicle import/export set to "off" blocks all delivery; and once trucks/routes are assigned to a storage, *only* those assigned trucks work it — unassigned trucks won't help.
**Prevention & fix** — Source storage: red slider ("Empty"); destination storage: green slider ("Keep full"); vehicle settings on "auto" unless you know why not; remove stray truck/route assignments from storages you want the general fleet to serve.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/4358998116401262394/>

**Failure mode** — Truck gridlock and cross-map nonsense trips: free-roaming trucks path over the whole island, get obstructed by new construction, and haul from the farthest storage instead of the nearest buffer.
**Symptoms** — Planters driving to the far end of the map for 8 saplings while a full silo sits next to them; trucks needing manual guidance around freshly built belts; remote outposts (mines, overseas construction) starved because no vehicle is *assigned* to the route.
**Prevention & fix** — Put a buffer storage next to the consumer and pin the flow: import/export routes between paired storages (the community "two storages back to back" pattern), assigned vehicles for remote routes, logistics zones (Update 4) to fence fleets in, and per-storage vehicle-type filters so e.g. mega trucks stay out of the base. Note the reported quirk: chains of 3+ export-linked storages make trucks skip the middle one — chain at most two, or belt between hops. Player-reported, not dev-confirmed — flag as folklore-grade.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/5911590080723456336/?ctp=2> ; <https://steamcommunity.com/app/1594320/discussions/0/603031621173530219/?l=hungarian>

**Failure mode** — A waste/byproduct storage set to export captures all truck attention and can never be dumped (slag case).
**Symptoms** — Furnaces clog on slag because "when you set an export on a storage, trucks don't dare touch the resources anymore for anything else"; or the concrete plant starves because the dump route eats the slag first.
**Prevention & fix** — Split the flows physically: balancer with priority output to the consumer (concrete mixers) and the non-priority output to a stacker/dump storage; or truck-side: source storage "keep empty", consumer storage "keep full", plus a designated dump area so excess has somewhere legal to go.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/664989013663492531>

**Failure mode** — Remote/overseas outpost vehicles run dry: amphibious trucks are not allowed to collect/deliver at storages by default, and nobody is assigned to bring fuel.
**Symptoms** — Vehicles stop dead on the far shore with a full diesel tank built and stocked on the mainland.
**Prevention & fix** — On the outpost storage, open the vehicle-type filter (top-right of the storage panel) and allow amphibious trucks; set up the import route and assign a vehicle to it; keep a fuel station + buffer at the outpost itself.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/833872731749570145>

---

## 2. Trains (Update 4 / Trains Expansion)

**Failure mode** — Using a Train Network for a continuous bulk flow it wasn't designed for: networks dispatch a train only when the source can *fully load* it AND the destination can *fully unload* it, with in-transit cargo already counted against destination space.
**Symptoms** — Most trains sit in the waiting bay while one or two work; "5 trains just sit in the staging yard… zero congestion on the line"; adding trains changes nothing.
**Prevention & fix** — The gate is buffer arithmetic: concurrent trains ≈ station capacity ÷ train capacity (worked example in-thread: 4,000 capacity ÷ 1,500/train → at most 2–3 trains in motion). Link more storage at both ends to raise parallelism, or switch continuous mine→smelter flows to **Train Lines** — community consensus: "networks are excellent for occasional supplies… if you want continuous transport (mines to smelters), lines are better." This maps directly onto Philip's design: dedicated consists on Lines for oil and coal, the shared Network for intermittent general freight.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/809101443875392392>

**Failure mode** — One network asked to do everything: a Train Network services **one resource per job**, and every cargo station competes for the same train pool.
**Symptoms** — Urgent small jobs queue behind bulk work; a fuel delivery waits behind ore jobs; throughput collapses when many products share one network.
**Prevention & fix** — Networks are three roles — cargo stations, fuel stations, waiting bays — and dispatch picks source/destination by station priority with round-robin. Keep the general network for varied, intermittent jobs; give constant single-product flows their own Lines (lines, unlike networks, can carry multiple resources simultaneously on one schedule). Both systems are the F7 Train Schedules interface; Lines date from Update 3, Networks from Update 4.1.
**Source** — <https://wiki.coigame.com/index.php?title=User:Timj11dude/sandbox&diff=12555&oldid=12537> (unofficial draft page — mechanics summary, treat details as provisional)

**Failure mode** — Station too small for its trains: station length/module count and the station's **train limit** cap how many trains may be at or en route to it.
**Symptoms** — Trains wait with "Arrival Conditions not met"; a train refuses to depart the previous station because the destination group is at its limit and skipping isn't enabled.
**Prevention & fix** — Size stations for the longest consist using them; set train limits deliberately; group parallel stations into one stop (the train chooses highest-priority, then nearest, at departure — it does not re-check en route); enable station skipping in the schedule where a full destination is routine.
**Source** — <https://wiki.coigame.com/Train_Station>

**Failure mode** — Stale arrival-condition thinking after Update 4.3: the old "wait until modules empty/full" conditions were replaced by **per-product arrival rules** with percentage thresholds, a "require full service" toggle, and fuel-type filters; trains with no actionable cargo are now refused entry and trains that can't interact with any product skip the station automatically.
**Symptoms** — Migrated saves behave differently than the player remembers ("migration is best effort, please verify your setups"); a station can no longer load *and* unload the same product, breaking old combined-station designs.
**Prevention & fix** — After 4.3, re-audit every station's per-product rules; split load and unload of the same product into separate stations; use "force no skip" only where the station is a terminus/turnaround.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/809100133028344772> (patch-note summary in thread)

**Failure mode** — Shared-track blockage: a train waiting at a station or waypoint holds the only path, and following trains can't pass — CoI trains reserve their route rather than running on Factorio-style signals, so a stopped train on a single-track section blocks everything behind it.
**Symptoms** — A second train won't depart its waypoint until the first has *finished unloading* and cleared to a siding; whole line frozen with no visible crash or error.
**Prevention & fix** — Keep stations on sidings/turnouts off the through route (exactly Philip's layout rule), allow more than one train to stop at busy stations, use waypoints with allow/deny rules (Update 4.2) to keep wrong trains out of holding tracks, and keep "no stop" zones off the main track. Player-reported in the Diary #57 thread; the dual-track + dedicated-turnout layout described there is the standard cure.
**Source** — <https://steamcommunity.com/app/1594320/eventcomments/570426739071729033/?ctp=4> ; <https://coigame.com/Blog/update42-is-out>

**Failure mode** — Fuel starvation of the fleet: trains run dry away from the depot because refueling wasn't designed into the schedule/network.
**Symptoms** — Locomotives stranded or crawling; network trains never visiting a fuel station because none was assigned the fuel role.
**Prevention & fix** — In Networks, explicitly assign fuel stations (and set the refueling threshold in schedules); in Lines, add refueling stops. Player pattern worth copying: one central refueling complex at the main hub, with fuel railed out to satellite fuel stations on big maps so no train deadheads across the island to refuel.
**Source** — <https://wiki.coigame.com/index.php?title=User:Timj11dude/sandbox&diff=12555&oldid=12537> ; pattern: <https://steamcommunity.com/app/1594320/eventcomments/570426739071729033/?ctp=4>

**Failure mode** — Designing for Update 3 trains in an Update 4 game: no universal wagons, no waypoint filters, no linked storages in the mental model.
**Symptoms** — Over-specialized wagon fleets that can't be shared across network jobs; station buffers too small because storages aren't linked to the station (linked storages count toward the station's stock for arrival/dispatch decisions).
**Prevention & fix** — Update 4.2's universal wagons (any product, ¾ of a dedicated wagon's capacity) are the right default for the shared network; link storages to stations so dispatch sees the real buffer; adjacent station modules handling the same product now auto-balance their inventories.
**Source** — <https://coigame.com/Blog/update42-is-out> ; <https://steamcommunity.com/app/1594320/discussions/0/809100133028344772>

---

## 3. Priorities

**Failure mode** — Never touching priorities at all: everything defaults to middling priority, so during a shortage the game shares pain evenly — including to the power plant's fuel supply and the maintenance chain.
**Symptoms** — Death-spiral shortages where the systems that would fix the problem (workers, power, maintenance) are starved at the same rate as everything else.
**Prevention & fix** — The widely-cited baseline: set the settlement/worker services, power production and its fuel chain, and maintenance production to high entity priority so they get first claim on electricity, workers, maintenance and logistics during any shortage; leave bulk industry at default.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/3425564314024806156/>

**Failure mode** — Priority inflation / spiral: because defaults are uneven, players start re-prioritizing everything; with a dozen ad-hoc levels nobody remembers the scheme, and a low-priority consumer of a shared input (e.g. coal) "tends to get ignored" entirely while a slightly-higher one eats everything.
**Symptoms** — Brickworks never gets coal after the power plant was made priority 1 and it priority 2; every new building needs a priority decision; the scheme exists only in the player's head.
**Prevention & fix** — Use a *small, fixed* set of tiers with a written rule (e.g. survival chain / production / everything else), apply it by category rather than per building, and re-check when adding a new consumer of any shared resource. This complaint thread is the canonical description of the failure — the fix is discipline, not more levels.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/5911590080723456336/?ctp=2>

**Failure mode** — Confusing the systems: expecting the entity Priority to control truck deliveries, or the storage sliders to control power draw.
**Symptoms** — Endless slider fiddling while the actual constraint was the building's priority for workers/power, or vice versa.
**Prevention & fix** — Keep the map straight: entity Priority = claim on power/workers/maintenance/unity + cargo priority; storage sliders = truck and belt/pipe fill/empty targets for that storage only; generation priority = which generators run first; balancer prioritization = which port wins at that balancer only.
**Source** — <https://coigame.com/Blog/captain-s-diary-12-beta-release-date> ; <https://hub.coigame.com/Translate?projectId=1&stringKey=StoredProduct__ExportSliderLabel&languageId=34>

**Failure mode** — Generator priorities left equal: with all generators at the same generation priority, the game splits output across them "in some odd random way", so every shaft runs part-loaded and inefficient.
**Symptoms** — Rows of generators each producing a fraction of rating despite full steam; total output far below nameplate while consumption is still short.
**Prevention & fix** — Stagger generation priority deliberately: baseload P1, next block P2, and so on, so the first block runs at 100% and later blocks trim (and the last can sit at zero until needed or charging a flywheel).
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/5841781924536477244/?l=norwegian&ctp=1>

**Failure mode** — The balancer itself starved of power: balancers consume a small amount of electricity (5–20 kW) but sit in the middle of critical chains.
**Symptoms** — A whole resource chain "severely damaged" by one unpowered balancer during a brownout.
**Prevention & fix** — Wiki advice verbatim in effect: keep balancer entity priority fairly high — they consume no workers or maintenance, only a few kW, and an out-of-order balancer breaks the chain it serves.
**Source** — <https://wiki.coigame.com/index.php?title=Pipe_Balancer&curid=798&oldid=12653>

**Failure mode** — Storage slider set once and forgotten at the wrong end: "Trucks keep full" left on a *source* storage (or "keep empty" on a destination) inverts the intended flow.
**Symptoms** — Trucks refuse to take product out of a keep-full storage; a destination with a red slider is actively emptied by your own fleet.
**Prevention & fix** — Rule of thumb from the dev-answered thread: exporting storage = red/Empty, importing storage = green/Keep full; audit sliders whenever a storage changes role.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/4358998116401262394/>

---

## 4. Storage & buffers

**Failure mode** — No buffer between a bursty producer and a steady consumer (or vice versa): the two machines are coupled tick-by-tick, so every pause propagates instantly.
**Symptoms** — A machine alternates "no input" / "output full" many times a minute; average throughput far below either machine's rating.
**Prevention & fix** — Interpose storage sized to the burst, not the average: ship deliveries (months apart, shipload-sized), farm harvests (one lump per cycle) and train loads (one consist-sized lump) all need a buffer at least as large as one delivery, plus consumption cover for the gap between deliveries. The train-network arithmetic is the same rule: station buffer ÷ train capacity = how many trains can be in motion.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/809101443875392392> ; <https://wiki.coigame.com/Fluid_Module_(S)>

**Failure mode** — "Storage full" cascade at the *output* of a chain: a byproduct with nowhere to go stops the machine that makes it, which stops everything downstream of that machine's main product.
**Symptoms** — Furnaces halted on slag; a refinery stopped not by crude supply but by a full heavy-oil or sour-water tank; the companion's "storage full while still producing" pattern — output blocked, chain behind it backing up in sequence.
**Prevention & fix** — Every byproduct needs a designed sink sized like a product: a consumer with priority (slag → concrete via priority balancer output), a dump/stacker overflow path, or a flare/liquid dump for fluids. Overflow routing must exist *before* the tank fills, not as a reaction.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/664989013663492531>

**Failure mode** — Buffer at the wrong end: huge storage at the producer, none at the consumer (or the reverse), so the buffer never absorbs the variance it was built for.
**Symptoms** — Consumer starves while the producer's silo is full (transport cap in between); or consumer's buffer is always empty because the producer's bursts never arrive in time.
**Prevention & fix** — Buffer the *receiving* end for supply security (that's the stock the consumer actually draws during a gap) and the *sending* end for blockage protection; for train stations, link storages at both ends — dispatch counts linked stock, so an unlinked silo next to the station does nothing for train flow.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/809101443875392392>

**Failure mode** — Belt/pipe slider gates left at extremes, silently fencing a storage off from its belts: "Receive from belts/pipes only if below" set low means a belt-fed storage stops accepting long before it's full; "Output … only if above" set high starves the belt it feeds.
**Symptoms** — Storage hovers at a fixed partial level forever; machines downstream starve with product visibly sitting in the tank.
**Prevention & fix** — Treat the two belt/pipe sliders as deliberate gates (useful to keep a reserve that belts can't touch, or to stop belts draining a truck-fed buffer) and audit them whenever a storage "mysteriously" pins at one level.
**Source** — <https://hub.coigame.com/Translate?projectId=1&stringKey=StoredProduct__InputFromBeltSliderLabel&languageId=23>

**Failure mode** — No alerts on critical buffers: a slow structural drain (see Diesel) is invisible until the tank is empty because nothing watches the level.
**Symptoms** — "Sudden" shortages that were actually months in the making.
**Prevention & fix** — Set storage alerts (low-stock alerts on inputs like diesel/coal/food ingredients, high-stock alerts on byproducts) on the handful of buffers whose failure cascades; alerts are per-storage in the current UI. Where an alert fires repeatedly, the fix is capacity or rate, not a bigger silo by default — a buffer that always trends one way is a rate problem wearing a storage costume.
**Source** — storage alert commands/UI verified in-game (Philip's build exposes StorageAlertSetEnabledCmd / SetThreshold); general principle per the deficit pattern in <https://gist.github.com/mw-stanley/d24c4ae4b1a86f390087333c3f5d236a>

**Failure mode** — Using pipes as accidental storage for steam: steam can't go in a Fluid Storage, so players either vent it or discover pipe volume by accident, then can't reason about their buffer.
**Symptoms** — Turbine behaviour that changes when pipe layouts are edited; steam "disappearing" after a rebuild.
**Prevention & fix** — Do it deliberately if at all: pipes legitimately store fluids including Steam (High) — that's a feature (wiki) — but for real thermal buffering use Thermal Storage, and treat pipe volume as a side effect, not the design.
**Source** — <https://wiki.coigame.com/index.php?title=Pipe&diff=11618&oldid=9000>

---

## 5. Power

**Failure mode** — Brownout from inertia, not capacity: nameplate generation exceeds demand, but turbines take time to spin up, so a demand spike browns out the grid before steam generation responds.
**Symptoms** — "I produce 90 MW and get power shortage at 70–75 MW consumption"; red lightning bolts during spikes while the graphs show spare capacity.
**Prevention & fix** — Flywheels on the shaft: they store mechanical power and donate it instantly while turbines spool. Community sizing: ~2–3 flywheels per boiler/turbine block that runs in auto-balance; one early-game flywheel for a small plant. (Wiki revisions disagree on capacity — 80 MW·s in one, 500 MW·s in a newer revision; treat the number as version-dependent and check in-game.)
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/5841781924536477244/?l=norwegian&ctp=1> ; <https://wiki.coigame.com/index.php?title=Flywheel&direction=prev&oldid=2473>

**Failure mode** — Mechanical-power mismatch on a shaft: too many generators hung on too little turbine output.
**Symptoms** — "Consumption 15.5 MW, demand 24.5 MW, max capacity 196 MW": the capacity figure sums every generator's rating, but each Steam (High) turbine reliably drives only ~2 generators and a Steam (Low) ~1 — the shaft, not the generator count, is the cap.
**Prevention & fix** — Balance shafts like any other production chain: turbine mechanical output ≥ generator draw; put a plant's steam generation on **one shaft** where possible so generation priority and auto-balance behave coherently; add turbines before adding generators.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/554627937323769614>

**Failure mode** — Fuel starvation of the power plant itself: the plant's fuel (coal/oil) competes with industry for the same logistics and loses.
**Symptoms** — Brownouts that trace back to an empty boiler, not a broken turbine; "no amount of flywheels will cover you for minutes" — storage buys seconds-to-reaction-time, not supply.
**Prevention & fix** — Make the plant's fuel chain high priority end-to-end (mine/import → storage → boiler), give the final feed a priority balancer output, and keep a fuel buffer at the plant. Flywheels are for spin-up gaps; buffers and priority are for supply gaps. Different failures, different tools.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/3811787298089839352/?l=koreana>

**Failure mode** — Auto-balance on the wrong machines: turbines set to auto-balance cycle on/off chasing demand; applied to nuclear, where the reactor wants a fixed power point, output fluctuates wildly.
**Symptoms** — Nuclear steam output swinging; depleted-steam/water loop backing up; generation graphs saw-toothing.
**Prevention & fix** — Run nuclear at a fixed level sized so its turbines run flat-out ("full beans"), generators set to a *lower* generation priority (P2/P3 in player terms) as steady baseload-behind, and let flexible generation (boilers/diesel) trim with auto-balance + flywheels. Never let the reactor's water loop be the thing that backs up: give recycled water priority at its pipe balancer and guarantee the depleted-steam sink.
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/603032578938691063>

**Failure mode** — Demand growth silently eating the reserve: every new production block is "only a few MW", until the grid runs at ~100% and any single failure cascades.
**Symptoms** — First brownouts coincide with unrelated breakdowns (a turbine under maintenance = instant shortage); flywheel always empty.
**Prevention & fix** — Keep deliberate headroom: staggered generation priorities make the reserve *visible* — the highest-priority-numbered block should normally sit idle or part-loaded. If your last block runs flat-out, you're already short; build generation *before* the next industry block, not after its brownout. (Structuring advice synthesized from the two threads above; the "idle last block = reserve" reading follows directly from the staggered-priority behaviour they describe.)
**Source** — <https://steamcommunity.com/app/1594320/discussions/0/5841781924536477244/?l=norwegian&ctp=1> ; <https://steamcommunity.com/app/1594320/discussions/0/554627937323769614>

---

## 6. Farming

**Failure mode** — Same crop following itself in the rotation (including last slot wrapping to first): a **50% fertility penalty** on the repeated crop.
**Symptoms** — A farm yields roughly half its recipe output forever, and no amount of fertilizer fully compensates — the penalty multiplies fertility consumption (P = 1.5 in the wiki's formula).
**Prevention & fix** — After researching Crop Rotation, always run ≥2 *different* crops per farm; remember the calendar wraps — first and last slots are adjacent. Single-crop farms are an early-game-only expedient; if only one food crop is unlocked, alternate it with Green Manure as a stopgap.
**Source** — <https://wiki.coigame.com/Farming>

**Failure mode** — Judging a farm by one month: a farm produces **nothing for most of its cycle and a whole cycle's yield in one harvest event**, so monthly production swings between zero and a huge spike.
**Symptoms** — "Farm broken / food production = 0" alarms in harvest-gap months; panic-building farms during a gap, then drowning in surplus after harvest; food stock graphs saw-toothing.
**Prevention & fix** — Judge farms over a full rotation cycle (and food security by *stock months of consumption*, never by this month's production). Two-crop rotations harvest twice as often as four-crop ones, smoothing the sawtooth; splitting crops across two farms with offset cycles smooths it further. This is exactly why the companion's seasonal classification exists: bursty producers get long-window balances, not monthly stall flags.
**Source** — <https://wiki.coigame.com/Farming> (2-crop vs 4-crop harvest frequency); harvest-burst behaviour is inherent to the growth-calendar system described there

**Failure mode** — Fertility equilibrium quietly below plan: every crop consumes fertility as it grows (0.2–0.5%/day by crop); natural replenishment only balances it at an equilibrium that may be ~58% (corn's is 57.9%) — and yield scales *linearly* with fertility.
**Symptoms** — Farms permanently yielding 55–70% of recipe with no warning anywhere; food math that "should work" on recipe numbers but doesn't.
**Prevention & fix** — Either plan food production at the *equilibrium* yield, or pipe fertilizer and set the farm's fertility target — budgeting fertilizer with the wiki formula (holding a target above 100% costs disproportionately more: a 200% demand multiplier applies above 100%, and natural replenishment turns negative there). High-need crops (soybean 0.5%/day) gain most from fertilizer; potatoes (0.35%/day) least.
**Source** — <https://wiki.coigame.com/Farming> ; <https://wiki.coigame.com/Fertility>

**Failure mode** — Rotation chosen for yield on paper, broken by logistics: crops sharing a belt type back each other up (vegetables and corn both use flat belts; potatoes use loose belts).
**Symptoms** — One crop's full output buffer blocks the other's harvest; a 3–4 crop farm's shared output jams in rotation order.
**Prevention & fix** — Wiki rule: pair crops with *different* belt types (potato + vegetables is the canonical pair) so one can flow when the other's line is full; prefer two 2-crop farms over one 3–4-crop farm — same output per cycle, more frequent harvests, simpler belts.
**Source** — <https://wiki.coigame.com/Farming>

**Failure mode** — Food buffer sized to average consumption with no harvest-gap cover: consumption is continuous, harvests are discrete.
**Symptoms** — Settlement runs out of a food type days before the next harvest even though annual production exceeds annual consumption.
**Prevention & fix** — Size food storage to cover the longest gap between harvests of that food *plus* a safety margin, and watch *months of food* (the settlement-level figure) rather than production rates. Green Manure slots are fertility tools, not food — a farm slot spent on it produces no calories that cycle, so food plans must count only food-bearing slots.
**Source** — <https://wiki.coigame.com/Farming>

---

## 7. Oil & diesel

**Failure mode** — The silent structural diesel deficit: total diesel *consumption* (vehicles + ship + rubber + generators) creeps above refinery output as the base grows, while the tank is big enough that the drain takes months to notice.
**Symptoms** — Diesel quantity declining a little every month for many months (in Philip's run: ≈ −55/month slope, ≈110 months of cover — slow enough to ignore until it isn't); no single breakdown to blame.
**Prevention & fix** — Track diesel as a *balance*, not a level: produced vs consumed per month from the products stats, and the multi-month quantity slope. The LP analysis's warning generalizes: builds sized for "peak diesel" via cracking can silently depend on byproduct streams — "too much byproduct consumption might starve you of diesel if your base was relying on that extra peak diesel when the other products are idle."
**Source** — <https://gist.github.com/mw-stanley/d24c4ae4b1a86f390087333c3f5d236a>

**Failure mode** — The hydrocracker eating the diesel it was meant to protect: the Cracking Unit's diesel-reforming path consumes diesel + steam to make naphtha/fuel gas — a leftover or mis-prioritized reforming recipe turns the refinery's biggest diesel consumer into the refinery itself.
**Symptoms** — Diesel consumption dominated by an internal recipe rather than vehicles (in Philip's run, 112 of 161 units consumed in a month were DieselReforming vs 49 of vehicle fuel) while diesel output looks healthy.
**Prevention & fix** — Audit *who consumes the diesel* (products/sources) before adding capacity — if the cracker is the top consumer, decide deliberately whether that conversion is wanted (it trades diesel for naphtha/fuel gas/hydrogen chains); disable or deprioritize the reforming recipe if it was a leftover from a different refinery balance. Cracking *into* diesel (naphtha/heavy oil + hydrogen → diesel) is the intended direction; reforming *out of* diesel is a tool for byproduct-heavy states, not a default.
**Source** — recipe direction per the game's own data (companion live pull) and <https://wiki.coigame.com/Heavy_Oil> ("Heavy Oil is sometimes combined with Hydrogen in a Cracking Unit to produce Diesel and Fuel Gas"); consumption-split pattern from Philip's live game data

**Failure mode** — Refinery deadlock on a byproduct: distillation makes fixed fractions; if heavy oil (or naphtha, or fuel gas) has no sink, the whole column stops — crude intake halts behind it.
**Symptoms** — Diesel falls while crude storage is *full*; distillers idle with full output tanks.
**Prevention & fix** — Give every fraction a sink sized for the full-throughput case: heavy oil → boilers (steam) or cracking; naphtha → diesel/rubber/plastic or boilers; fuel gas → hydrogen reforming or boilers. The LP guide's starting block (per 6 distillers: 3 heavy→diesel cracking, 6 naphtha→diesel, 3 naphtha→fuel gas/H₂, 4 fuel gas→hydrogen, 2 fuel gas→diesel, plus boilers as sinks for heavy/naphtha/H₂/fuel gas) uses **priority to push naphtha and fuel gas toward diesel only when their other destinations back up** — priority as a pressure valve, not a fixed route.
**Source** — <https://gist.github.com/mw-stanley/d24c4ae4b1a86f390087333c3f5d236a>

**Failure mode** — Cracking heavy oil to naphtha because it *can* be done: the LP analysis is blunt — "Cracking Heavy Oil to Naphtha is almost never worth it — even if you value the byproducts more than diesel, it's better to boil it for steam, because that costs Hydrogen."
**Symptoms** — Hydrogen chronically short, refinery complexity growing, diesel no better than before.
**Prevention & fix** — Default heavy-oil sink order: burn for steam first; crack to diesel when diesel is the goal; crack to naphtha only with a specific naphtha shortage and hydrogen to spare.
**Source** — <https://gist.github.com/mw-stanley/d24c4ae4b1a86f390087333c3f5d236a>

**Failure mode** — Diesel generation as a hidden second fleet: diesel generators added as "backup" quietly become a top diesel consumer, coupling the power grid to the fuel the trucks and ship need.
**Symptoms** — Diesel drain spikes exactly during power trouble; logistics slows (fuel) because power ate the fuel, causing more power trouble — a coupled death spiral. Diesel "is the lifeblood of your island: the Ship, Cargo Ships, Vehicles, Rubber Maker, and Diesel Generator all run on Diesel."
**Prevention & fix** — Put diesel generators last in generation priority so they're true peaking/backup units, keep their fuel buffer separate and alarmed, and treat any month where generators are a leading diesel consumer as a power-design bug, not a fuel-supply bug.
**Source** — <https://wiki.coigame.com/index.php?title=Diesel&direction=next&oldid=7249> ; <https://coigame.com/Blog/captain-s-diary-12-beta-release-date>

---

## Source-quality notes

- The **wiki is uneven**: several transport pages are stubs with numbers that changed between revisions (Pipe III 400 vs 450; Flywheel 80 vs 500 MW·s; Flat Conveyor III 400 → 450). Figures above use the *latest* revision found and flag conflicts; the companion's game-extracted dataset should override the wiki wherever they differ.
- The detailed **Train Schedules** mechanics summary lives on a wiki *user sandbox* page (a draft, marked version 0.8.3 [tbc]) — directionally consistent with the Train Station page and the 4.1/4.2 update posts, but not an official page.
- The **train-network dispatch gating** (full load + full unload + in-transit counted) comes from player threads, with multiple independent posters converging on the same arithmetic and a worked example — high-confidence folklore, not dev-confirmed.
- The **priority-spiral** material is a complaint thread; it reliably documents the *failure mode*, and the "few fixed tiers" fix is the thread's/community's conclusion rather than official guidance.
- Nothing found contradicts Philip's two design rules for trains (dedicated oil/coal consists + one general network; stations on sidings off the through line) — the network-vs-line evidence actively supports the split.
