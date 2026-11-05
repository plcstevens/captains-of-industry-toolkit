//! Built-in guides for the companion website.
//!
//! Each guide is organized around the failure modes it prevents —
//! the same failures the triage detectors watch for — rather than
//! around game systems in the abstract. Content rules: no invented
//! numbers. Mechanics and figures come from the official wiki,
//! Captain's Diary posts, the official translation hub (exact
//! in-game strings), convergent community practice, the verified
//! planner dataset (0.8.7d), the relocation plan, and this run's
//! live data. Player-established mechanics are labelled as such;
//! disputed figures are flagged, not hidden. Full digest with a
//! source per claim: guides-research.md in the project folder.

pub(crate) struct GuideSection {
    pub heading: &'static str,
    pub body: &'static str,
}

pub(crate) struct Guide {
    pub id: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    pub sections: &'static [GuideSection],
}

pub(crate) fn guide_by_id(id: &str) -> Option<&'static Guide> {
    GUIDES.iter().find(|g| g.id == id)
}

pub(crate) const GUIDES: &[Guide] = &[
    Guide {
        id: "transport",
        title: "Transport — belts, pipes & trucks",
        blurb: "What transport failures look like before they cascade: saturated trunks, balancer-free merges, contradictory truck settings, and ship bursts hitting pipe-sized drains.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: the silent backup",
                body: "<p>A transport link that cannot carry what a line produces does not announce itself. The producer's output buffer fills, the machine flips to <b>OutputFull</b>, its inputs stop being consumed, and the backup walks upstream one machine at a time. By the time a shortage shows at the end of the chain, the fault is several buildings away. The census on the Entities page shows the state split per machine type — a cluster of OutputFull is a transport verdict, not a production one.</p>",
            },
            GuideSection {
                heading: "Know the ladder: 60 / 200 / 450",
                body: "<p>Flat conveyors (unit products) and U-shape conveyors (loose) carry <b>60 / 200 / 450 per minute</b> at tiers I / II / III; pipes carry 60 / 200 at tiers I / II (tiers III–IV exist in Update 4; the tier-III figure is disputed between wiki revisions — trust the in-game tooltip). The classic failure is arithmetic, not connectivity: several blocks merged onto a trunk whose tier can't carry their sum. Sum the recipe rates merging onto each segment, upgrade the <i>trunk</i> rather than the last segment, and remember storage input/output rates cap too — a faster belt into a slow storage changes nothing.</p>",
            },
            GuideSection {
                heading: "Merges and splits need a balancer — and a decision",
                body: "<p>Junctions without a balancer take from branches unevenly: one line jams, the other starves, two identical lines run at unequal load. Balancers (flat, U-shape, pipe) have per-port <b>Prioritization</b> — a prioritized port beats a non-prioritized one — and 'enforce strictly even' toggles for fair splits (which cancel port priorities). The canonical uses: priority <i>output</i> so coal feeds the boiler before the distiller on the same line; strictly-even outputs to share one product identically across parallel machines. One product per belt and per pipe network, always — pipes carry exactly one fluid at a time, and molten metal/glass refuse pipes entirely (Molten Channels only).</p>",
            },
            GuideSection {
                heading: "Trucks: the settings contradict each other quietly",
                body: "<p>Full storages next to a starving plant, with idle trucks in standby, is usually a settings contradiction (dev-confirmed diagnosis): a <b>'Trucks keep full' slider on the source storage</b> stops trucks exporting from it; vehicle import/export set to <b>off</b> blocks delivery entirely ('auto' = vehicles work it unless conveyors already meet the need); and once specific trucks/routes are assigned to a storage, <i>only those</i> serve it — the general fleet won't help. Rule of thumb: source storage = red 'keep empty', destination = green 'keep full', vehicle logistics on auto unless you know why not. For remote outposts, assign vehicles to the route — free-roaming trucks path across the whole island and haul from the farthest storage, not the nearest buffer.</p>",
            },
            GuideSection {
                heading: "Ship supply arrives in bursts",
                body: "<p>A dock is not a stream. The small Fluid Module unloads at 125/min — matched by a single Pipe II — and then the ship is gone for a round trip measured in months. Buffer at the dock for the <i>round-trip gap</i>, not the unload rate, and match the pipe away from the module to the unload rate or the ship sits full while the refinery starves.</p><p class=\"sub\">Sources: wiki transport pages (Flat Conveyor II/III, Pipe), Pipe Balancer page; Steam dev-answered logistics thread; Update 4 refinery analysis (mw-stanley). Full links in the project research digest.</p>",
            },
        ],
    },
    Guide {
        id: "trains",
        title: "Trains — networks that don't jam",
        blurb: "Lines for continuous bulk, the Network for intermittent freight, stations that never block the through route — and the dispatch arithmetic that decides how many trains actually move.",
        sections: &[
            GuideSection {
                heading: "The dispatch gate: full load, full unload, or no departure",
                body: "<p>On a Train <b>Network</b>, a train is only dispatched when the source can <i>fully load</i> it and the destination can <i>fully unload</i> it — with cargo already in transit counted against the destination's space (player-established, with converging worked examples). The consequence is buffer arithmetic: concurrent trains ≈ station capacity ÷ train capacity. A 4,000-unit station pair with 1,500-unit trains runs at most 2–3 trains in motion no matter how many wait in the bay. If trains sit idle, the fix is more linked storage at both ends, not more trains.</p>",
            },
            GuideSection {
                heading: "Lines for bulk, Network for the rest — this run's split",
                body: "<p><b>Train Lines</b> run a fixed schedule and suit continuous single-purpose flows; <b>Networks</b> dispatch jobs dynamically, one resource per job, from a shared pool — excellent for varied, intermittent freight, wrong for mine-to-smelter volume. That is exactly this run's architecture (relocation plan): dedicated consists on lines for <b>oil</b>, <b>coal</b> and the new <b>iron-ore shuttle</b>; everything else on the general network. Bulk mixed into the network starves the small urgent jobs behind it.</p>",
            },
            GuideSection {
                heading: "Stations: sidings, sizing, and the 4.3 arrival rules",
                body: "<p>Stations sit on sidings off the through route: trains reserve their path, so one train waiting at a station on the main line freezes everything behind it. Size stations for the longest consist using them, set train limits deliberately, and group parallel stations into one stop (a train picks highest priority, then nearest, at departure — it does not re-check en route). <b>Update 4.3 changed arrival conditions</b> to per-product rules with percentage thresholds — and a station can no longer load <i>and</i> unload the same product, so combined stations from older saves need splitting (this run's plan already splits them). Link storages to stations: linked stock counts toward dispatch decisions, an unlinked silo beside the station does not.</p>",
            },
            GuideSection {
                heading: "Fuel is a network role, not an afterthought",
                body: "<p>In a Network, fuel stations are an explicit role — no assigned fuel station, no refuelling visits, and locomotives run dry away from the depot. On Lines, build refuelling stops into the schedule. On a big map, rail fuel out to satellite fuel stations from one central complex so no train crosses the island just to refuel. Universal wagons (Update 4.2, ¾ of a dedicated wagon's capacity) are the right default for the shared network fleet.</p><p class=\"sub\">Sources: wiki Train Station page; Update 4.1/4.2/4.3 posts and patch-note summaries; Steam train-network threads. The detailed schedules mechanics summary lives on an unofficial wiki draft — directionally consistent, treated as provisional in the digest.</p>",
            },
        ],
    },
    Guide {
        id: "priorities",
        title: "Priorities — telling the factory what matters first",
        blurb: "Three different systems share one word. What each actually steers, the spiral that makes them useless, and the small set of exceptions that makes them work.",
        sections: &[
            GuideSection {
                heading: "First, the map: three systems, one word",
                body: "<p><b>Entity Priority</b> (per building) sets that building's claim on <b>power, workers, maintenance and unity</b> during shortages, and also affects its cargo priority for logistics (Captain's Diary #12, verbatim). <b>Electricity generation priority</b> is a separate, generator-only system — players call its levels P1/P2/P3 — deciding which plants run first. <b>Storage control is not a priority number at all</b>: it is sliders — green 'Trucks keep full if under', red 'Trucks keep empty if above', plus belt/pipe gates ('Receive from belts/pipes only if below', 'Output … only if above') — and per-storage vehicle logistics (on / auto / off). Balancers have their own port prioritization (see Transport). Named <i>custom</i> priorities exist but are the least publicly documented system in the game — treat specifics as unverified.</p>",
            },
            GuideSection {
                heading: "The failure this prevents: the priority spiral",
                body: "<p>When a chain starves, the instinct is to raise its priority. Each raise is locally sensible and globally corrosive: claims get pulled from chains you didn't touch, which starve and get raised in turn — and a slightly-lower-priority consumer of a shared input 'tends to get ignored' entirely while the higher one eats everything. The end state, everything re-prioritized, works worse than the defaults and exists only in the player's head. The fix is discipline: a <i>small, fixed set of tiers with a written rule</i>, applied by category, not per building.</p>",
            },
            GuideSection {
                heading: "The working baseline",
                body: "<p>Leave most buildings at default. The widely used exception set: <b>settlement/worker services, power production and its fuel chain, and maintenance production</b> get high entity priority, so during any shortage the systems that fix shortages are served first. Add the current bottleneck's <i>supply line</i> while it lasts — priority belongs on the mine, storage and producer feeding a starved machine, not on the starving machine itself, which cannot pull what is not being sent. Review after every expansion: last month's exception is this month's distortion.</p>",
            },
            GuideSection {
                heading: "Two cheap wins people miss",
                body: "<p><b>Balancers:</b> they draw only 5–20 kW but sit mid-chain, and an unpowered balancer during a brownout breaks everything behind it — the wiki's advice is to keep balancer priority fairly high (they consume no workers or maintenance). <b>Sliders:</b> a slider left at the wrong end inverts a storage's role — 'keep full' on a source storage stops trucks exporting from it; audit sliders whenever a storage changes job. The companion's Entities page sets per-machine general priority and storage logistics directly; every change goes through the confirmation step and lands in the audit log.</p><p class=\"sub\">Sources: Captain's Diary #12; official translation hub (exact slider strings); wiki Pipe Balancer page; Steam priority complaint/baseline threads.</p>",
            },
        ],
    },
    Guide {
        id: "storage",
        title: "Storage & buffers — decoupling the chain",
        blurb: "Buffers are what let bursty producers and steady consumers coexist. Sizing them to the burst, parking them deliberately, and reading the 'storage full' cascade correctly.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: full storage blocks the chain",
                body: "<p>When a storage fills, its producer does not slow down politely — it stops, then backs up its own inputs. The lethal version is a <i>byproduct</i> with nowhere to go: furnaces halted on slag, a refinery stopped by a full heavy-oil tank while diesel falls and crude sits full. 'Storage full while still producing' on the issue board means the draw or the sink has failed, not the supply. Every byproduct needs a designed sink sized like a product — a consumer with priority routing, a dump/stacker overflow, a flare — built <i>before</i> the tank fills. Adding storage treats the symptom and buys days.</p>",
            },
            GuideSection {
                heading: "Size to the burst, place at both ends",
                body: "<p>A buffer decouples two rhythms, so size it to the gap between them: at least one full delivery (a shipload, a harvest, a train consist) plus consumption cover until the next one. Placement matters as much as size — buffer the <i>receiving</i> end for supply security (that's the stock the consumer draws during a gap) and the <i>sending</i> end for blockage protection. Two half-buffers at the two ends beat one huge one in the middle, and for train stations only <i>linked</i> storage counts toward dispatch.</p>",
            },
            GuideSection {
                heading: "Gates and sliders: the storage that pins at one level",
                body: "<p>A storage hovering at a fixed partial level forever is usually fenced by its own belt/pipe sliders: 'receive only if below' set low stops a belt-fed storage accepting long before it's full; 'output only if above' set high starves the belt it feeds. Used deliberately these gates protect a reserve belts can't touch — but audit them whenever a storage 'mysteriously' stops at the same number. Same discipline for the truck sliders: source = red/keep-empty, destination = green/keep-full (see Priorities).</p>",
            },
            GuideSection {
                heading: "Parked storage is a strategy (the war chest)",
                body: "<p>A full storage with no production and no draw is not automatically a problem: this run's Construction Parts I & II stockpiles are a deliberate war chest for the relocation phases, parked so nothing drains them early. The issue board files exactly that pattern under 'Ignore for now'. The logistics toggles (Entities page, per storage) are how you park — and unpark — a buffer on purpose. And set the game's storage <i>alerts</i> on the handful of buffers whose failure cascades (fuel, food, the current bottleneck's input): a slow drain should page you months before the tank is empty.</p><p class=\"sub\">Sources: wiki Pipe/storage pages; translation hub (slider strings); Steam slag/byproduct thread; train dispatch arithmetic (see Trains).</p>",
            },
        ],
    },
    Guide {
        id: "power",
        title: "Power — keeping the grid ahead of demand",
        blurb: "Brownouts cascade worse than any product shortage, and the cause is usually shaft arithmetic or spin-up inertia, not nameplate capacity. Dispatch order makes the reserve visible.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: the brownout spiral",
                body: "<p>When generation falls short, machines across the factory flip to <b>NotEnoughPower</b> — including, eventually, the machines producing the fuel and parts the power plants need. A power deficit feeds on itself in a way a product deficit never does. The census state split (Entities page) is the early warning: any NotEnoughPower count above zero wants a same-day answer. But first diagnose <i>which</i> power failure you have — the next three sections are the usual suspects, and their fixes don't substitute for each other.</p>",
            },
            GuideSection {
                heading: "Shaft arithmetic: turbines drive generators, not wishes",
                body: "<p>The grid's capacity figure sums every generator's rating, but each shaft is capped by its turbines' mechanical output — a Steam (High) turbine reliably drives about 2 generators, a Steam (Low) about 1. 'Capacity 196 MW, demand 24.5 MW, shortage anyway' is this mismatch: too many generators hung on too little turbine. Balance each shaft like a production chain, put a plant's steam generation on one shaft where possible, and add turbines before generators.</p>",
            },
            GuideSection {
                heading: "Inertia: flywheels cover seconds, buffers cover supply",
                body: "<p>Turbines take time to spin up, so a demand spike can brown out a grid whose nameplate exceeds demand — '90 MW produced, shortage at 75 MW consumed'. Flywheels on the shaft store mechanical power and donate it instantly while turbines spool (community sizing: a few per boiler/turbine block in auto-balance). But no flywheel covers a supply gap: if the boiler is out of coal, the fix is the fuel chain — high priority end-to-end, a priority balancer output on the final feed, a fuel buffer at the plant on alert.</p>",
            },
            GuideSection {
                heading: "Stagger the dispatch order; run nuclear flat",
                body: "<p>With all generators at equal generation priority, the game splits output across them 'in some odd random way' and every shaft runs part-loaded and inefficient. <b>Stagger deliberately</b>: baseload P1, next block P2, and so on, so the first block runs at 100% and later blocks trim. This also makes your reserve <i>visible</i>: if the last block runs flat-out, you are already short — build generation before the next industry block, not after its brownout. Nuclear is the exception to flexible operation: run the reactor at a fixed level with its turbines flat-out, flexible plants trimming around it, and never let the reactor's water loop be the thing that backs up.</p><p class=\"sub\">Sources: Steam power threads (shaft sizing, staggered priorities, flywheel practice); wiki Flywheel page (capacity figures differ between revisions — check in-game).</p>",
            },
        ],
    },
    Guide {
        id: "farming",
        title: "Farming — plan by the season, not the month",
        blurb: "Farms produce in bursts, yields drift with rotation and fertility, and belts jam by crop type. Judged monthly, farms always look broken; judged on the cycle, the truth comes out.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: fixing a farm that isn't broken",
                body: "<p>A farm produces nothing for most of its cycle and a whole cycle's yield in one harvest event, so monthly production swings between zero and a huge spike. Any monthly detector — 'production stalled', 'net deficit' — fires falsely on that pattern, and acting on it spends real resources on an imaginary fault. This run's potatoes are the textbook case: one 81-unit harvest across 7 finished months against ~33/month of steady consumption. The companion classifies bursty producers from their history and judges them on the long window only; food security is <i>stock months of consumption</i>, never this month's production.</p>",
            },
            GuideSection {
                heading: "Rotation: never follow a crop with itself — including around the wrap",
                body: "<p>After Crop Rotation is researched, a crop that follows <i>itself</i> in the rotation takes a <b>50% fertility penalty</b> — and the calendar wraps, so the last slot and the first slot are adjacent. Always run at least two different crops per farm; if only one food crop is unlocked, alternate it with Green Manure as a stopgap (but count Green Manure slots as fertility tools, not food — they produce no calories that cycle). The companion's seasonal runway for this run's potatoes — ≈38 months at the long-window rate — is the right shape of answer: a planting decision, not an emergency.</p>",
            },
            GuideSection {
                heading: "Fertility equilibrium: recipe yields are the ceiling, not the plan",
                body: "<p>Every crop consumes fertility as it grows, and natural replenishment only balances that at an <i>equilibrium</i> — corn's is ≈58% fertility, and yield scales linearly with fertility, so unfertilized corn plans at ~58% of recipe forever. Either budget food production at equilibrium yields, or pipe fertilizer and set a fertility target — knowing that holding a target above 100% costs disproportionately more (a 200% demand multiplier, and natural replenishment turns negative up there). High-need crops (soybean) gain most from fertilizer; potatoes least. This is why the relocation plan builds the fertilizer chain <i>before</i> the new farm district.</p>",
            },
            GuideSection {
                heading: "Pair crops by belt type; prefer two small farms",
                body: "<p>Crops sharing a belt type back each other up — vegetables and corn both ride flat belts, potatoes ride loose — so pair crops with <i>different</i> belt types (potato + vegetables is the canonical pair) and one full output can't jam the other's harvest. Two 2-crop farms beat one 3–4-crop farm: same output per cycle, harvests twice as often (a smoother sawtooth into storage), simpler belts. Whatever the layout, size food storage to the longest gap between harvests plus margin — consumption is continuous, harvests are discrete.</p><p class=\"sub\">Sources: wiki Farming and Fertility pages (penalty, equilibrium and belt-pairing rules verbatim from there).</p>",
            },
        ],
    },
    Guide {
        id: "oil-diesel",
        title: "Oil & diesel — closing the silent deficit",
        blurb: "Diesel rarely fails loudly. It drains a few percent a month for quarters while every month looks explainable — and the refinery itself is often the biggest consumer.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: the structural drain",
                body: "<p>This run's diesel is the archetype: production 84–120/month against consumption 140–195/month, seven straight months, stock sliding 6,432 → 5,992. No single month is a crisis and the runway is years — which is precisely why it never gets fixed. Track diesel as a <i>balance</i>, not a level: produced vs consumed per month, and the multi-month quantity slope. The Outlook's slope forecast exists for this shape — ≈110 months at the 7-month slope is a planning item today, not a surprise in year 397. Beware 'peak diesel' builds: output sized via cracking can silently depend on byproduct streams, and starves when those products idle.</p>",
            },
            GuideSection {
                heading: "Find the real consumer first — it may be the refinery",
                body: "<p>Before adding capacity, split the consumption (the product page's Sources table, this month and last). In this run, of 161 diesel consumed in a month, <b>112 was the hydrocracker's DieselReforming</b> — diesel converted onward to naphtha and fuel gas — and only 49 was vehicle fuel. Cracking <i>into</i> diesel (naphtha/heavy oil + hydrogen → diesel) is the intended direction; reforming <i>out of</i> diesel is a tool for byproduct-heavy states, not a default. A 'diesel shortage' that is really a refining decision is fixed at the cracker — pause it, deprioritize it, or keep it with eyes open — not at the distillery.</p>",
            },
            GuideSection {
                heading: "Every fraction needs a sink, in the right order",
                body: "<p>Distillation makes a fixed slate, not a product: if heavy oil (or naphtha, or fuel gas) has no sink, the column stops and crude backs up behind it — this run's crude sits <i>above</i> nominal capacity while diesel drains, so supply was never the constraint. Default sink order for heavy oil: <b>burn for steam first</b>; crack to diesel when diesel is the goal; crack to naphtha almost never (the Update 4 refinery analysis is blunt: it costs hydrogen and is 'almost never worth it'). Use priority routing as a pressure valve — naphtha and fuel gas flow to diesel-making only when their other destinations back up — rather than as a fixed route. Judge the oil chain by all its outputs' runways together, never one product's.</p>",
            },
            GuideSection {
                heading: "Diesel generators: the hidden second fleet",
                body: "<p>Diesel is the lifeblood — ship, cargo ships, vehicles, rubber production and diesel generators all drink from the same tank. Generators added as 'backup' quietly become a leading consumer, coupling the grid to the fuel logistics needs: power trouble eats the fuel, fuel shortage slows logistics, slower logistics deepens the power trouble. Keep diesel generators <b>last in generation priority</b> (true peaking units), their buffer separate and alarmed, and treat any month where generators lead diesel consumption as a power-design bug, not a fuel-supply bug. Longer term, this run's hydrogen backbone (fertilizer, Electronics II, long-haul trains) shrinks diesel to vehicle and ship duty — trucks, excavators and the ship have no hydrogen option, so a diesel floor never goes away.</p><p class=\"sub\">Sources: wiki Diesel and Heavy Oil pages; Update 4 refinery analysis (mw-stanley) for sink discipline and cracking verdicts; consumption split from this run's live game data.</p>",
            },
        ],
    },
    Guide {
        id: "contracts-trade",
        title: "Contracts & trade — the world market without surprises",
        blurb: "Contracts are standing ship-borne commitments paid in a product you must keep producing, and quick trades get more expensive every time you repeat them. Read the terms the companion can actually see before either one surprises you.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: a contract that eats its payment line",
                body: "<p>Establishing a contract is not a one-off sale. A contract names a <b>productToBuy</b> (what you receive) and a <b>productToPayWith</b> with a <b>quantityToPayWith</b> (what you pay, in kind) — so the classic failure is signing for a payment product your factory cannot sustain: a surplus you signed away turns into a structural deficit, and the chain that used to consume that product now competes with the contract. Before establishing anything, ask the Storage question in reverse: is the payment product a true surplus with a sink to spare, or is it a buffer you were counting on? If the honest answer is the second, the contract is a loan against a stockpile, not an income.</p>",
            },
            GuideSection {
                heading: "Read the offer in the fields the game actually exposes",
                body: "<p>The bridge route <b>/coi/v1/contracts</b> lists active contracts and unlocked-but-not-established offers, and each one carries the same verified fields: the two products and the quantity above, <b>upointsPerMonth</b> and <b>upointsToEstablish</b> (unity, carried as raw values), and <b>minReputationRequired</b> — a hard gate: below that reputation the offer is not yours to take, whatever the products look like. The route also returns a <b>profit multiplier as a raw value</b>; the Trade page renders it as a percentage, but it is a multiplier on the contract terms, not a promise about your factory's margin — your margin depends on what the payment product costs <i>you</i> to make. <b>ToggleContractCmd</b> both establishes and cancels (it takes just the contract id), so check which list — active or available — a contract is in before preparing it: the same command does opposite things in the two lists.</p>",
            },
            GuideSection {
                heading: "Fulfilment is a ship, not an inventory swap",
                body: "<p>Contracts are fulfilled by ship through the cargo dock, which makes a contract a <i>logistics</i> commitment as much as a production one: the payment product has to keep reaching the dock, and what you receive arrives in shipload rhythm, not as a stream (see Storage on buffering bursts). For active contracts the route computes a live <b>toBuy</b> figure — the amount still to buy under that contract, read from the game, not estimated — so judge an active contract by that number and by whether the payment line is still in surplus, not by how good the deal looked on the day you signed it. Cancelling is the same ToggleContractCmd through the usual prepare → confirm → audit flow; cancelling late is still cheaper than letting a payment line starve the base for a product you no longer need.</p>",
            },
            GuideSection {
                heading: "Quick trades: instant, escalating, and finite",
                body: "<p>A quick trade is the opposite shape: one immediate exchange, executed by <b>QuickTradeCmd</b> with just the trade's id. Each provider on the world map works in <b>steps</b> — the route <b>/coi/v1/quick-trades</b> shows <b>currentStep</b> against the prototype's <b>maxSteps</b>, an <b>isSoldOut</b> flag (sold out is exactly currentStep reaching maxSteps), and a <b>hasPriceIncreased</b> flag: the price rises as the steps climb, so the trade you repeat is not the trade you priced the first time. Every trade also carries a <b>unity cost</b> (the route's upointsCost, shown raw on the Trade page). The failure mode is death by convenience: a string of 'cheap' emergency trades escalating in product price while each one also spends unity, until the provider is sold out precisely when the emergency is real. Read the live buy/pay quantities on the Trade page — they come from the game's own getters with its volume multiplier already applied — never from memory of the last trade.</p>",
            },
            GuideSection {
                heading: "The discipline: surplus first, emergencies last",
                body: "<p>Contracts for flows you can prove are surplus on a long window (the same long-window judgement as Farming and Oil & diesel — a good month is not a surplus); quick trades for genuine one-off gaps, checked against their current step before you fire; and a periodic audit of active contracts against the toBuy figure and the payment product's current balance. If a payment product appears on the issue board as draining or in deficit, the contract paying with it is the first suspect, ahead of any production fault.</p><p class=\"sub\">Sources: CoiBridge BridgeRoutesV6.cs (contracts and quick-trades routes, surfaces read from the shipped assemblies); bridge MODDING-NOTES recon round 2 (QuickTradeProvider step/sold-out/price fields and the QuickTradeCmd matching rule); SPEC §12.7 and Addendum VII (ToggleContractCmd / QuickTradeCmd schemas); companion Trade page rendering.</p>",
            },
        ],
    },
    Guide {
        id: "loans",
        title: "Loans — borrowed product, repaid product",
        blurb: "A loan delivers a product now and takes scheduled payments in that same product for years. The failures are timing and kind: a payment landing in a crisis month, and owing a product you no longer produce.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: the payment that lands in the crisis month",
                body: "<p>A loan's danger is not the total owed, it is the schedule. Each active loan carries an <b>annualPayment</b> and a <b>nextPaymentDate</b>, and that payment falls due on its date whether or not the month is convenient — the same month as a breakdown, a fuel crunch or a food gap is exactly when a large in-kind payment hurts most. The companion's loans route (<b>/coi/v1/loans</b>) exposes both fields per loan, so there is no excuse for being surprised: know every active loan's next payment date and hold the payment product in stock ahead of it, the way you would buffer any other burst — except this burst flows <i>out</i>.</p>",
            },
            GuideSection {
                heading: "The terms the companion can see — and one number it cannot decode",
                body: "<p>The route reports the LoansManager terms directly: an <b>interest rate</b> and a <b>fee</b>, both carried as raw fixed-point values which the Trade page renders as percentages, and <b>maxActiveLoans</b> — a cap on how many loans can run at once, so a loan slot is itself a resource not to spend on a trivial gap. It also lists the products the game will lend at all (the borrowable-products list); if a product is not on it, there is no loan for it at any terms. One warning, stated plainly: the route also exposes a <b>credit score as a raw Fix32 value, and its human-scale decoding is unverified</b>. The companion shows it raw for that reason. Do not read it as a percentage, a familiar real-world credit score, or any other scale — nobody has verified what range is good, bad or normal, and planning around an invented scale is worse than ignoring the number until it is decoded.</p>",
            },
            GuideSection {
                heading: "Borrowing in kind means repaying in kind",
                body: "<p><b>AcceptLoanCmd</b> takes exactly three things: a <b>product</b>, an amount (<b>toBorrow</b>) and a term (<b>yearsTotal</b>). What arrives is that product; what you owe is that product — each active loan records its product alongside <b>leftToPay</b>, <b>annualPayment</b>, <b>yearsLeft</b> (and yearsPaid), plus bookkeeping fields such as interestPaid and startingDebt. So the second failure mode is borrowing a product you will not be able to repay in kind: a product from a chain you plan to demolish, replace or let idle becomes an obligation to keep that chain alive — or to buy back its output some other way — for yearsLeft more years. Borrow against a product with a durable production line and a surplus on the long window, never against a chain already on the relocation plan's demolition list.</p>",
            },
            GuideSection {
                heading: "Overpay deliberately; set the buffer priority like you mean it",
                body: "<p>Two actuation levers exist beyond taking the loan. <b>MakeLoanOverpaymentCmd</b> (product, loanId, toPay) pays down a loan ahead of schedule — the tool for a genuine surplus month, shrinking leftToPay before a tight year rather than after a missed-feeling one. <b>SetLoanBufferPriorityCmd</b> (loanId, priority) sets the loan's payment-buffer priority — treat it as part of the Priorities discipline: the payment buffer's claim should be decided by the same small fixed rule as everything else, not fiddled per crisis, because a buffer that outranks the chains that refill it wins once and starves thereafter. All three loan commands go through the companion's prepare → confirm → audit flow; the catalogue flags AcceptLoanCmd itself as financially consequential and never autonomous — borrowing is a decision, not a detector response.</p><p class=\"sub\">Sources: CoiBridge BridgeRoutesV6.cs (loans route, LoansManager and ActiveLoan surfaces read from the shipped assemblies); companion command catalogue (AcceptLoanCmd / MakeLoanOverpaymentCmd / SetLoanBufferPriorityCmd schemas and the AcceptLoanCmd caution note); companion Trade page rendering of the terms.</p>",
            },
        ],
    },
    Guide {
        id: "edicts",
        title: "Edicts — enabled is not the same as active",
        blurb: "The companion can see every edict's enabled and active state and the game's own reasons when something is off — but it cannot switch an edict. Toggling stays in-game, for a stated reason.",
        sections: &[
            GuideSection {
                heading: "The failure this prevents: trusting the toggle you set",
                body: "<p>An edict has two separate states, and conflating them is the failure: <b>isEnabled</b> (you switched it on) and <b>isActive</b> (it is actually in effect right now). An edict can sit enabled-but-not-active while the player assumes its effect is running and plans around a benefit — or a cost — that is not happening. The bridge route <b>/coi/v1/edicts</b> reports both flags for <i>every</i> edict, so the check is cheap: on the Trade page, an edict you rely on should show both ticked. If it does not, the next section is where the answer lives — in the game's own words, not in guesswork.</p>",
            },
            GuideSection {
                heading: "Read the game's own reasons, not folklore",
                body: "<p>For each edict the route also returns <b>canBeEnabled</b> with the explanation from the game's own enable check (<b>reason</b>), and, for an edict that is not currently active, its recorded reason for not being active (<b>inactiveReason</b>). Those strings are the game's verdict on its own state — why this edict cannot be enabled here, or why an enabled one is not in effect — and they outrank any community summary, including this guide. The companion's job is to surface them verbatim next to the flags; the player's job is to fix the condition the string names rather than toggling hopefully and re-checking.</p>",
            },
            GuideSection {
                heading: "What the route does not show: cost, tiers, effects",
                body: "<p>The verified surface is state and reasons only. The edicts route carries <b>no cost field</b> — an edict's unity cost is read in the in-game edicts panel, not here (the sandbox protocol for edict actuation treats cost exactly that way) — and no tier or next-tier fields appear in the route either, so this guide describes no tier behaviour: nothing in the digest, the routes or the command schemas verifies any, and inventing it would be exactly the failure these guides exist to prevent. Judge an edict from its in-game panel description and cost together with the live enabled/active state here; neither half alone is the whole picture.</p>",
            },
            GuideSection {
                heading: "The actuation caveat: the companion cannot toggle edicts",
                body: "<p><b>Read this before looking for a button that is not there.</b> A toggle command does exist inside the game — <b>ToggleEdictEnabledCmd</b>, an input command processed by the EdictsManager — but it is <b>not in the web API's command registry</b>: the captured inventory of 240 commands contains zero edict commands, so the companion's prepare → confirm → audit flow cannot prepare it, and the Trade page's edicts table is deliberately read-only. Toggling therefore stays <b>in-game</b> until that channel is proven. The proof path already exists and is deliberately conservative: <b>SANDBOX-PROTOCOLS.md Protocol B</b> — run only on a throwaway sandbox save (never the live one; it carries a hard game-id abort guard), it tests whether the /command endpoint accepts a type absent from its own inventory, captures the exact request/response envelope, and verifies any flip independently through /coi/v1/edicts before the result is trusted. Until Protocol B has been run and passed, any edict change you want happens in the game's own panel — the companion will show you the resulting state, which is the half of the loop that is already verified.</p><p class=\"sub\">Sources: CoiBridge BridgeRoutesV6.cs (edicts route and its class comment on ToggleEdictEnabledCmd's absence from the web API inventory); SPEC Addendum VII (the inventory finding and the sandbox-first ruling); SANDBOX-PROTOCOLS.md Protocol B (test design, selection rule, abort guard); companion Trade page (read-only edicts table and its not-wired note).</p>",
            },
        ],
    },
];
