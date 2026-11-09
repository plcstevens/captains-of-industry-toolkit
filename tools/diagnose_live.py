#!/usr/bin/env python3
"""Offline design-fault diagnosis over captured CoI live data.

Reads the JSON captures in ~/workspace/coi-companion/live/ (bridge
census/topology/storages + vanilla product stats, pulled 2026-11-02)
and emits a markdown diagnosis with evidence numbers. No network,
no game access — pure analysis of the captures.

Fault classes (strong evidence only):
  1. Transports the game itself reports as not fully connected.
  2. Zero-length transports (from == to) — degenerate segments.
  3. Storages at >=99.9% full with output logistics DISABLED
     (stuck by configuration: nothing can ever leave by truck).
  4. Products backed up: total stored >= total storage capacity.
  5. Machine clusters starved (NotEnoughInput majority) or blocked
     (OutputFull majority) per census state splits.
  6. Products whose production stopped this month (produced TM == 0,
     LM > 0) while consumption continues — corroboration layer.

Forward-compat (Round 6): when a capture carries CoiBridge v0.6.x
fields, the diagnosis uses them —
  7. Storage slider/priority fields (v0.6.1): a §3 "stuck" storage
     whose ImportUntil slider is ~100% is full BY CONFIGURATION
     (trucks are told to keep it full) and is reported separately,
     not as stuck; census `priorities` map / storage priority fields
     / a bridge-priorities.json capture get their own section.
  8. A bridge-power.json capture (v0.6.2 route, v0.6.3 averages):
     a negative balanceKw is a power-deficit finding.
  9. A bridge-machines.json capture (v0.6.4 route): §5's per-proto
     clusters become per-instance — every starved / output-full
     machine with its id, position, recipes and connected neighbours
     traced over the topology port edges (storages resolved to their
     product + fill). Also: capture-consistency check (instance rows
     vs census aggregates) and paused/disabled instance lists.
Captures without those fields produce exactly the v1 output. A
capture dir containing SYNTHETIC.txt is labelled synthetic in the
report (hand-enriched test data, not a real game state).

Usage: python3 diagnose_live.py [live_dir] [out_md]
"""

import json
import sys
from pathlib import Path

LIVE = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent.parent / "live"
OUT = Path(sys.argv[2]) if len(sys.argv) > 2 else LIVE / "diagnosis-v1.md"


def load(name):
    with open(LIVE / name) as f:
        return json.load(f)


def load_opt(name):
    path = LIVE / name
    if not path.exists():
        return None
    with open(path) as f:
        return json.load(f)


def pct_raw(raw):
    return raw / 1000.0


def main():
    topo = load("bridge-topology.json")
    census = load("bridge-census.json")
    storages = load("bridge-storages.json")["storages"]
    stats = load("stats.json")["products"]
    power = load_opt("bridge-power.json")
    custom_prios = load_opt("bridge-priorities.json")
    machines_cap = load_opt("bridge-machines.json")
    synthetic = (LIVE / "SYNTHETIC.txt").exists()

    lines = []
    lines.append(
        '# Design diagnosis v1 — Philip\'s "Expanding" base (Dragon Isle)'
        + (" (SYNTHETIC enriched capture)" if synthetic else "")
    )
    lines.append("")
    if synthetic:
        lines.append("⚠ **SYNTHETIC CAPTURE** — this directory is the real 2026-11-02")
        lines.append("capture plus hand-added CoiBridge v0.6.x fields, built to test the")
        lines.append("diagnosis tool's forward-compat. It is NOT a real game state; do")
        lines.append("not act on its v0.6.x findings.")
        lines.append("")
    lines.append("Source data: bridge census/topology/storages + vanilla product stats,")
    lines.append("captured live 2026-11-02 (in-game 388-2-27, game PAUSED at capture).")
    lines.append("Offline analysis only — no game access used. Findings are limited to")
    lines.append("what the captures prove; inferences are labelled [inference].")
    lines.append("")

    # --- 1. Unconnected transports -------------------------------------
    bad = [t for t in topo["transports"] if not t["fullyConnected"]]
    lines.append(f"## 1. Transports not fully connected: {len(bad)} of {len(topo['transports'])}")
    lines.append("")
    if bad:
        lines.append("| id | proto | from | to | moving |")
        lines.append("|---|---|---|---|---|")
        for t in bad:
            lines.append(
                f"| {t['id']} | {t['proto']} | {t['from']} | {t['to']} | {t['isMoving']} |"
            )
        lines.append("")
        lines.append("The game's own connection flag says these segments do not form a")
        lines.append("complete path. Anything fed exclusively through one of them cannot flow.")
    else:
        lines.append("None — every transport reports fullyConnected.")
    lines.append("")

    # --- 2. Single-tile transports --------------------------------------
    # NOT a fault class: belts and pipes are placed per tile, so a
    # one-tile segment legitimately has from == to. Recorded here only
    # because the §1 overlap (single-tile AND unconnected) is a real
    # dead end.
    zero = [t for t in topo["transports"] if t["from"] == t["to"]]
    both = [t for t in zero if not t["fullyConnected"]]
    lines.append(f"## 2. Single-tile transports: {len(zero)} (normal — not a fault)")
    lines.append("")
    lines.append("Belts/pipes are placed per tile, so from == to is the ordinary shape of")
    lines.append("a one-tile segment. The only fault-relevant subset is the overlap with")
    lines.append(
        f"§1 — single-tile AND not fully connected: **{len(both)}**"
        + (f" (ids {', '.join(str(t['id']) for t in both)})" if both else "")
        + ". Those are dead ends with nothing on either side."
    )
    lines.append("")

    # --- 3. Full storages with output disabled ---------------------------
    stuck_all = [
        s for s in storages if pct_raw(s["percentFullRaw"]) >= 99.9 and s["logisticsOutDisabled"]
    ]

    def keep_full_explained(s):
        # v0.6.1 slider fields: ImportUntil ~100% means trucks are
        # configured to keep this storage full — the fill is the
        # configuration working as set, not a stuck buffer.
        if "importUntilPercentRaw" in s:
            return pct_raw(s["importUntilPercentRaw"]) >= 99.9
        if "importStep" in s:
            return s["importStep"] >= 10
        return False

    explained = [s for s in stuck_all if keep_full_explained(s)]
    stuck = [s for s in stuck_all if not keep_full_explained(s)]
    full_open = [
        s
        for s in storages
        if pct_raw(s["percentFullRaw"]) >= 99.9 and not s["logisticsOutDisabled"]
    ]
    empty_in_off = [s for s in storages if s["quantity"] == 0 and s["logisticsInDisabled"]]
    lines.append("## 3. Storages full and stuck")
    lines.append("")
    lines.append(f"**Full (≥99.9%) with output logistics DISABLED: {len(stuck)}** — these can")
    lines.append("never be emptied by trucks; only a belt/pipe out (if any) can drain them.")
    lines.append("")
    if stuck:
        lines.append("| id | proto | product | qty / cap |")
        lines.append("|---|---|---|---|")
        for s in sorted(stuck, key=lambda s: s["product"]):
            lines.append(
                f"| {s['id']} | {s['proto']} | {s['product']} | {s['quantity']} / {s['capacity']} |"
            )
        lines.append("")
    if explained:
        lines.append(
            f"**Full with output disabled but EXPLAINED by a keep-full slider: "
            f"{len(explained)}** — their ImportUntil is ~100% (v0.6.1 slider"
        )
        lines.append("fields): trucks are configured to keep them full, so the fill is")
        lines.append("the configuration working as set. Not counted as stuck above.")
        lines.append("")
        lines.append("| id | proto | product | qty / cap | ImportUntil |")
        lines.append("|---|---|---|---|---|")
        for s in sorted(explained, key=lambda s: s["product"]):
            iu = (
                f"{pct_raw(s['importUntilPercentRaw']):.1f}%"
                if "importUntilPercentRaw" in s
                else f"step {s.get('importStep')}"
            )
            lines.append(
                f"| {s['id']} | {s['proto']} | {s['product']} | "
                f"{s['quantity']} / {s['capacity']} | {iu} |"
            )
        lines.append("")
    backed_products = {
        pid
        for pid, p in stats.items()
        if p["storageCapacity"] > 0 and p["stored"] >= p["storageCapacity"]
    }
    stuck_backed = sorted({s["product"] for s in stuck} & backed_products)
    lines.append("Caveat: output-disabled is a legitimate pattern for belt/pipe-drained")
    lines.append("buffers — a full, output-disabled storage is only a PROVEN fault when its")
    lines.append(
        "product is also backed up at product level (§4). That subset here: "
        + (", ".join(stuck_backed) if stuck_backed else "none")
        + ". The rest are configuration facts to eyeball, not proven faults."
    )
    lines.append("")
    lines.append(f"Full (≥99.9%) with output enabled: {len(full_open)} — normal buffers or")
    lines.append("backed-up products; see §4 for the product-level view.")
    if full_open:
        prods = {}
        for s in full_open:
            prods[s["product"]] = prods.get(s["product"], 0) + 1
        lines.append(
            "By product: " + ", ".join(f"{p} ×{n}" for p, n in sorted(prods.items())) + "."
        )
    lines.append("")
    lines.append(
        f"Empty storages with INPUT logistics disabled: {len(empty_in_off)} — "
        "they can never be filled by trucks either."
    )
    lines.append("")

    # --- 4. Products backed up (stored >= capacity) ----------------------
    over = [
        (pid, p)
        for pid, p in stats.items()
        if p["storageCapacity"] > 0 and p["stored"] > p["storageCapacity"]
    ]
    backed = [
        (pid, p)
        for pid, p in stats.items()
        if p["storageCapacity"] > 0 and p["stored"] == p["storageCapacity"]
    ]
    lines.append(f"## 4. Products backed up: {len(over)} OVER capacity, {len(backed)} at capacity")
    lines.append("")
    if over:
        lines.append("**Stored EXCEEDS capacity** — the surplus can only be sitting in")
        lines.append("machine output buffers; producers will read OutputFull (§5):")
        lines.append("")
        lines.append("| product | stored | capacity | surplus | quantity |")
        lines.append("|---|---|---|---|---|")
        for pid, p in sorted(over):
            lines.append(
                f"| {pid} | {p['stored']} | {p['storageCapacity']} | "
                f"{p['stored'] - p['storageCapacity']} | {p['quantity']} |"
            )
        lines.append("")
    if backed:
        lines.append("Stored == capacity (storage exactly full). For bulk products this is")
        lines.append("a hard stop on production; for low-capacity intermediates (molten")
        lines.append("products, steam, exhaust, gases) a full buffer is close to normal")
        lines.append("operating state — weak evidence on its own:")
        lines.append("")
        lines.append("| product | stored = capacity | quantity |")
        lines.append("|---|---|---|")
        for pid, p in sorted(backed):
            lines.append(f"| {pid} | {p['stored']} | {p['quantity']} |")
        lines.append("")
        lines.append(
            "Bulk rows that matter most: "
            + ", ".join(pid for pid, p in sorted(backed) if p["storageCapacity"] >= 500)
            + "."
        )
    lines.append("")

    # --- 5. Machine clusters: starved / blocked ---------------------------
    lines.append("## 5. Machine clusters by state (census)")
    lines.append("")
    lines.append(
        "Totals across all machine protos: "
        + ", ".join(
            f"{s} {n}"
            for s, n in sorted(
                {
                    st: sum(m["states"].get(st, 0) for m in census["machines"].values())
                    for st in ("Working", "NotEnoughInput", "OutputFull", "Paused")
                }.items()
            )
        )
        + "."
    )
    lines.append("")
    starved, blocked = [], []
    for proto, m in census["machines"].items():
        total = sum(m["states"].values())
        if total < 2:
            continue
        nei = m["states"].get("NotEnoughInput", 0)
        of = m["states"].get("OutputFull", 0)
        if nei * 2 >= total:
            starved.append((proto, total, nei, m))
        if of * 2 >= total:
            blocked.append((proto, total, of, m))
    lines.append(f"### Starved clusters (≥half the machines lack input): {len(starved)}")
    lines.append("")
    if starved:
        lines.append("| proto | machines | NotEnoughInput | recipes assigned |")
        lines.append("|---|---|---|---|")
        for proto, total, nei, m in sorted(starved, key=lambda x: -x[2]):
            lines.append(f"| {proto} | {total} | {nei} | {', '.join(m['recipes'])} |")
        lines.append("")
    lines.append(f"### Blocked clusters (≥half the machines are output-full): {len(blocked)}")
    lines.append("")
    if blocked:
        lines.append("| proto | machines | OutputFull | recipes assigned |")
        lines.append("|---|---|---|---|")
        for proto, total, of, m in sorted(blocked, key=lambda x: -x[2]):
            lines.append(f"| {proto} | {total} | {of} | {', '.join(m['recipes'])} |")
        lines.append("")
    lines.append("[inference] A starved cluster next to a backed-up product (§4) usually")
    lines.append("means the missing input is stuck behind a full buffer elsewhere, not")
    lines.append("that the input isn't produced. A blocked cluster means the downstream")
    lines.append("consumer or its storage path is the constraint.")
    lines.append("")

    # --- 6. Production stopped this month --------------------------------
    stopped = [
        (pid, p)
        for pid, p in stats.items()
        if p["produced"][0] == 0 and p["produced"][1] > 0 and p["consumed"][0] > 0
    ]
    lines.append(f"## 6. Production stopped this month, consumption continues: {len(stopped)}")
    lines.append("")
    lines.append("produced[thisMonth] == 0 while produced[lastMonth] > 0 and the product is")
    lines.append("still being consumed. (Seasonal/bursty producers — farms, ships — can")
    lines.append("legitimately show this pattern; treat those rows as weak evidence.)")
    lines.append("")
    if stopped:
        lines.append("| product | produced LM | consumed TM | quantity |")
        lines.append("|---|---|---|---|")
        for pid, p in sorted(stopped):
            lines.append(f"| {pid} | {p['produced'][1]} | {p['consumed'][0]} | {p['quantity']} |")
        lines.append("")

    # --- 7. Priorities (v0.6.1+ fields, only when captured) -------------
    census_prios = census.get("priorities")
    prio_rows = [s for s in storages if "generalPriority" in s]
    if census_prios or prio_rows or custom_prios:
        lines.append("## 7. Priorities (CoiBridge v0.6.1+ fields)")
        lines.append("")
        if census_prios:
            lines.append(
                f"Census non-default general priorities: "
                f"**{len(census_prios)}** entities differ from their "
                "proto default:"
            )
            lines.append("")
            lines.append("| entity | general priority |")
            lines.append("|---|---|")
            for eid, val in sorted(census_prios.items(), key=lambda kv: int(kv[0])):
                lines.append(f"| {eid} | {val} |")
            lines.append("")
        if prio_rows:
            dev = [
                s
                for s in prio_rows
                if s.get("importPriority", s["generalPriority"]) != s["generalPriority"]
                or s.get("exportPriority", s["generalPriority"]) != s["generalPriority"]
            ]
            lines.append(
                f"Storage rows carrying priority/slider fields: "
                f"{len(prio_rows)}; with import/export priority differing "
                f"from their general priority: {len(dev)}."
            )
            if dev:
                lines.append("")
                lines.append("| id | product | general | import | export |")
                lines.append("|---|---|---|---|---|")
                for s in sorted(dev, key=lambda s: s["id"]):
                    lines.append(
                        f"| {s['id']} | {s['product']} | "
                        f"{s['generalPriority']} | "
                        f"{s.get('importPriority')} | "
                        f"{s.get('exportPriority')} |"
                    )
            lines.append("")
        if custom_prios:
            ents = custom_prios.get("entities", [])
            lines.append(
                f"Custom priorities (v0.6.3 route): {len(ents)} entities "
                f"carry at least one visible custom priority id "
                f"(known ids: {', '.join(custom_prios.get('ids', []))})."
            )
            lines.append("")

    # --- 8. Power (v0.6.2+ route, only when captured) --------------------
    if power:
        lines.append("## 8. Power (CoiBridge /coi/v1/power)")
        lines.append("")
        gen, con = power.get("generatedKw"), power.get("consumedKw")
        bal = power.get("balanceKw")
        lines.append(
            f"Generated {gen} kW · consumed {con} kW · demanded "
            f"{power.get('demandedKw')} kW · capacity "
            f"{power.get('generationCapacityKw')} kW "
            f"(max {power.get('maxGenerationCapacityKw')}) · wasted "
            f"{power.get('wastedKw')} kW · **balance {bal} kW** · "
            f"{power.get('generators')} generators / "
            f"{power.get('consumers')} consumers."
        )
        lines.append("")
        if bal is not None and bal < 0:
            lines.append(
                f"**POWER DEFICIT: consumption exceeds generation by "
                f"{-bal} kW** this tick — machines throttle or starve "
                "for power until generation catches up"
                + (
                    f"; a further {power['surplusDemandedKw']} kW of demand is queued unsupplied."
                    if power.get("surplusDemandedKw")
                    else "."
                )
            )
            lines.append("")
        averages = power.get("averages")
        if averages:
            lines.append("Averages (kW):")
            lines.append("")
            lines.append("| series | last day | last month | last year |")
            lines.append("|---|---|---|---|")
            for name in ("production", "consumption", "generationCapacity"):
                a = averages.get(name)
                if a:
                    lines.append(
                        f"| {name} | {a.get('lastDayKw')} | "
                        f"{a.get('lastMonthKw')} | {a.get('lastYearKw')} |"
                    )
            lines.append("")
            if power.get("averagesUnverified"):
                lines.append(
                    "[unverified decoding] The bridge flags these "
                    "averages `averagesUnverified` — the layout is "
                    "assembly-verified but has had no live cross-check "
                    "against the in-game Statistics window yet."
                )
                lines.append("")

    # --- 9. Machine instances (v0.6.4 route, only when captured) --------
    if machines_cap:
        rows = machines_cap.get("machines", [])
        by_id = {r["id"]: r for r in rows}
        ent_proto = {e["id"]: e["proto"] for e in topo["entities"]}
        stor_by_id = {s["id"]: s for s in storages}
        neighbours = {}
        for a, b in topo["edges"]:
            neighbours.setdefault(a, []).append(b)
            neighbours.setdefault(b, []).append(a)

        def neighbour_desc(nid):
            s = stor_by_id.get(nid)
            if s:
                return f"storage {nid} ({s['product']}, {s['quantity']}/{s['capacity']})"
            if nid in by_id:
                r = by_id[nid]
                return f"machine {nid} ({r['proto']}, {r['state']})"
            if nid in ent_proto:
                return f"{ent_proto[nid]} {nid}"
            return f"entity {nid} (port owner with no topology row)"

        lines.append("## 9. Machine instances (CoiBridge v0.6.4 /coi/v1/machines)")
        lines.append("")
        # Consistency: instance rows vs the census aggregates (§5's
        # source). A mismatch means the two captures disagree — a
        # capture-integrity finding, not a game fault.
        mismatches = []
        row_protos = {}
        for r in rows:
            row_protos.setdefault(r["proto"], []).append(r)
        for proto, m in sorted(census["machines"].items()):
            agg_total = sum(m["states"].values())
            inst = row_protos.get(proto, [])
            if len(inst) != agg_total:
                mismatches.append(f"{proto}: census {agg_total} vs {len(inst)} rows")
                continue
            for st, n in m["states"].items():
                got = sum(1 for r in inst if r["state"] == st)
                if got != n:
                    mismatches.append(f"{proto}/{st}: census {n} vs rows {got}")
        extra = sorted(set(row_protos) - set(census["machines"]))
        if extra:
            mismatches.append("protos in rows but not census: " + ", ".join(extra))
        lines.append(
            f"Instance rows: **{len(rows)}**"
            + (
                f" · skipped at capture: {machines_cap['skippedRows']}"
                if machines_cap.get("skippedRows")
                else ""
            )
            + ". Consistency vs census aggregates: "
            + (
                "**MATCH** — every proto's instance states reproduce the census splits."
                if not mismatches
                else "**MISMATCH** — " + "; ".join(mismatches) + "."
            )
        )
        lines.append("")
        paused = [r for r in rows if r.get("isPaused")]
        disabled = [r for r in rows if r.get("isEnabled") is False]
        if paused or disabled:
            lines.append(
                f"Paused instances: {len(paused)}; disabled (isEnabled=false): {len(disabled)}."
            )
            for label, lst in (("paused", paused), ("disabled", disabled)):
                if lst:
                    by_proto = {}
                    for r in lst:
                        by_proto.setdefault(r["proto"], []).append(str(r["id"]))
                    lines.append(
                        f"- {label}: "
                        + "; ".join(
                            f"{p} ids {', '.join(ids)}" for p, ids in sorted(by_proto.items())
                        )
                    )
            lines.append("")
            lines.append("A paused/disabled machine inside a starved or blocked")
            lines.append("cluster is a settings fact, not a supply fault — check it")
            lines.append("before redesigning anything around the cluster.")
            lines.append("")
        for title, state in (("Starved", "NotEnoughInput"), ("Output-full", "OutputFull")):
            flagged = [
                p
                for p, m in census["machines"].items()
                if sum(m["states"].values()) >= 2
                and m["states"].get(state, 0) * 2 >= sum(m["states"].values())
            ]
            inst_rows = [r for p in flagged for r in row_protos.get(p, []) if r["state"] == state]
            lines.append(f"### {title} instances (from §5's flagged clusters): {len(inst_rows)}")
            lines.append("")
            if inst_rows:
                lines.append("| id | proto | pos | recipes | connected neighbours |")
                lines.append("|---|---|---|---|---|")
                for r in sorted(inst_rows, key=lambda r: (r["proto"], r["id"])):
                    nb = "; ".join(neighbour_desc(n) for n in sorted(neighbours.get(r["id"], [])))
                    lines.append(
                        f"| {r['id']} | {r['proto']} | {r['pos']} | "
                        f"{', '.join(r['recipes'])} | {nb or '— none —'} |"
                    )
                lines.append("")
                lines.append("[inference] Topology edges are undirected port")
                lines.append("connections: a neighbour storage holding a product is a")
                lines.append("candidate feeder/drain, not a proven direction of flow.")
                lines.append("")

    lines.append("---")
    lines.append("Generated by tools/diagnose_live.py from the 2026-11-02 captures in live/.")
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT} ({len(lines)} lines)")


if __name__ == "__main__":
    main()
