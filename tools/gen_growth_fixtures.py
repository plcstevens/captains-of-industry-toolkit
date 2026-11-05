#!/usr/bin/env python3
"""gen_growth_fixtures.py — generate the 12-month growth-sequence fixtures.

Produces plans/fixtures/growth/month-01/ … month-12/ in the EXACT
early-game fixture shape (same 9 state files, same keys,
percentFullRaw / progressPerc = percent x 1000, stats flow windows
[thisMonth, lastMonth, thisYear, lastYear, lifetime]).

Story: a plausible fresh start FOLLOWING plans/fresh-start-v1.yaml's own
build order. Month 1 IS the early-game fixture (copied, gameId/session
relabelled to the growth run). Months evolve from scripted FULL-MONTH
flow tables (below): a snapshot for month n is taken mid-month (day 15),
so thisMonth = ~half of month n's full flow, lastMonth = month n-1's
full flow, and thisYear == lifetime (all 12 months are in game year 1)
= sum of full months before n + thisMonth. Quantity is derived, never
hand-set: quantity = starting_stock + lifetime_produced -
lifetime_consumed, with the starting stocks implied by the early-game
fixture itself (diesel 300, copper 300, concrete 300, CP 58, MP 15,
potato 300, vegetables 80, steel 10, everything else 0).

HONESTY NOTES (fixture choices, also recorded in
plans/growth-sequence-timeline.md):
- "Construction" completes as ResearchCp1Production — the early-game
  fixture's own in-progress node and the plan's named candidate. The
  plan's phase-2 entry gate references the §4.5 placeholder, NOT this
  id, so that gate stays UNEVALUABLE regardless (by plan design).
- The Concrete Production node's game id is unsourced (plan §4.5
  placeholder; only the later ResearchConcreteAdvanced exists in
  coi/data/research_ids.txt, and the plan forbids substituting it).
  The fixture therefore never records a concrete node in `researched`;
  kiln/mixer builds appear month 9 (in-fiction the node completed
  around then) and NO gate reads that node, so nothing depends on the
  omission. ResearchRubber IS a real id in research_ids.txt (one of
  the plan's two candidates) and is used for Synthetic Rubber.
- Census proto labels beyond the early-game fixture (AssemblyManual,
  MaintenanceDepotT1, FarmT1, MineT1, KilnT1, ConcreteMixerT1,
  OilPumpT1, DistillerT1, RubberMakerT1, ElectrolysisT1) are plausible
  fixture labels in the early-game README's sense; only
  MaintenanceDepotT1 is read by a gate (the plan flags it
  proto-id-unconfirmed itself).
- Numbers follow the plan's build order at plausible early-game rates.
  They were NOT tuned to force late-phase flips: where the plan's own
  pace makes a gate unreachable in 12 months, the fixtures say so and
  the timeline doc records it as a finding.

Read-only over the repo except the output dirs it creates. No game
contact, no network.
"""

import json
import pathlib
import shutil

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "plans" / "fixtures" / "early-game"
OUT = ROOT / "plans" / "fixtures" / "growth"
GAME_ID = "fixture-growth-0001"
SESSION_ID = "fixture-growth-session"

# ------------------------------------------------------------ flow tables
# F[n] = (produced, consumed) for the FULL month n. Month 1 = the
# early-game fixture's lifetime flows (game year 1 starts at month 1).
# A product absent from a month's dict does not exist yet that month
# (absent from stats => predicates on it are honestly UNEVALUABLE).

def flows(**kw):
    return {k: v for k, v in kw.items()}

F = {
    "Product_IronScrap": dict(start=0, cap=1000, buf=50, f={
        1: (1250, 400), 2: (300, 260), 3: (220, 240), 4: (120, 180),
        5: (40, 90), 6: (0, 30), 7: (0, 0), 8: (0, 0), 9: (0, 0),
        10: (0, 0), 11: (0, 0), 12: (0, 0)}),
    "Product_Iron": dict(start=0, cap=200, buf=8, cap_sched={4: 400, 8: 800}, f={
        1: (58, 16), 2: (75, 15), 3: (75, 34), 4: (70, 40), 5: (70, 42),
        6: (75, 45), 7: (105, 55), 8: (115, 60), 9: (120, 65),
        10: (125, 70), 11: (130, 72), 12: (135, 75)}),
    "Product_Steel": dict(start=10, cap=200, buf=0, f={
        n: (0, 0) for n in range(1, 13)}),
    "Product_Coal": dict(start=0, cap=500, buf=5, cap_sched={7: 1000}, f={
        1: (90, 25), 2: (85, 60), 3: (85, 62), 4: (85, 64), 5: (120, 70),
        6: (140, 78), 7: (150, 85), 8: (155, 90), 9: (160, 95),
        10: (165, 100), 11: (170, 105), 12: (175, 110)}),
    "Product_Wood": dict(start=0, cap=500, buf=15, cap_sched={7: 1000}, f={
        1: (260, 120), 2: (160, 90), 3: (150, 88), 4: (140, 86),
        5: (120, 60), 6: (90, 35), 7: (80, 30), 8: (75, 28), 9: (70, 26),
        10: (65, 25), 11: (60, 24), 12: (55, 22)}),
    "Product_Diesel": dict(start=300, cap=250, buf=20, cap_sched={10: 500}, f={
        1: (0, 80), 2: (0, 20), 3: (0, 20), 4: (0, 20), 5: (0, 20),
        6: (0, 20), 7: (0, 22), 8: (0, 22), 9: (0, 22), 10: (55, 26),
        11: (110, 28), 12: (140, 30)}),
    "Product_Copper": dict(start=300, cap=500, buf=20, f={
        1: (0, 0), 2: (0, 0), 3: (0, 0), 4: (0, 6), 5: (0, 6), 6: (0, 6),
        7: (0, 6), 8: (0, 6), 9: (0, 6), 10: (0, 6), 11: (0, 6),
        12: (14, 7)}),
    "Product_ConcreteSlab": dict(start=300, cap=1000, buf=10, f={
        1: (0, 120), 2: (0, 10), 3: (0, 10), 4: (0, 10), 5: (0, 10),
        6: (0, 10), 7: (0, 10), 8: (0, 10), 9: (55, 30), 10: (70, 36),
        11: (75, 38), 12: (80, 40)}),
    "Product_ConstructionParts": dict(start=58, cap=200, buf=3, f={
        1: (0, 40), 2: (0, 6), 3: (25, 20), 4: (34, 30), 5: (38, 34),
        6: (42, 38), 7: (46, 42), 8: (50, 46), 9: (54, 50),
        10: (58, 54), 11: (62, 58), 12: (66, 62)}),
    "Product_MechanicalParts": dict(start=15, cap=200, buf=0, f={
        1: (0, 9), 2: (0, 2), 3: (10, 8), 4: (12, 10), 5: (13, 11),
        6: (14, 12), 7: (15, 13), 8: (16, 14), 9: (17, 15),
        10: (18, 16), 11: (19, 17), 12: (20, 18)}),
    "Product_Potato": dict(start=300, cap=1000, buf=20, f={
        1: (0, 60), 2: (0, 52), 3: (40, 54), 4: (55, 55), 5: (65, 56),
        6: (72, 58), 7: (78, 62), 8: (85, 64), 9: (105, 66),
        10: (110, 70), 11: (115, 72), 12: (120, 74)}),
    "Product_Vegetables": dict(start=80, cap=500, buf=5, f={
        1: (0, 20), 2: (0, 17), 3: (0, 18), 4: (15, 16), 5: (18, 18),
        6: (20, 19), 7: (22, 21), 8: (24, 22), 9: (30, 23),
        10: (32, 24), 11: (34, 25), 12: (36, 26)}),
    "Product_Water": dict(start=0, cap=500, buf=10, cap_sched={5: 1000}, f={
        1: (300, 150), 2: (320, 170), 3: (340, 185), 4: (360, 200),
        5: (380, 215), 6: (400, 230), 7: (430, 250), 8: (460, 270),
        9: (490, 290), 10: (520, 310), 11: (550, 330), 12: (580, 350)}),
    "Product_IronOre": dict(start=0, cap=500, buf=5, first=7, f={
        7: (120, 100), 8: (135, 110), 9: (145, 118), 10: (150, 122),
        11: (155, 126), 12: (160, 130)}),
    "Product_Electronics": dict(start=0, cap=200, buf=0, first=4, f={
        4: (8, 6), 5: (8, 6), 6: (9, 7), 7: (9, 7), 8: (10, 8),
        9: (10, 8), 10: (11, 9), 11: (11, 9), 12: (12, 10)}),
    "Product_Limestone": dict(start=0, cap=500, buf=5, first=9, f={
        9: (85, 72), 10: (90, 76), 11: (95, 80), 12: (100, 84)}),
    "Product_CrudeOil": dict(start=0, cap=500, buf=5, first=10, f={
        10: (130, 118), 11: (140, 126), 12: (150, 134)}),
    "Product_Rubber": dict(start=0, cap=200, buf=0, first=11, f={
        11: (16, 6), 12: (26, 12)}),
    "Product_CopperOre": dict(start=0, cap=500, buf=0, first=12, f={
        12: (45, 35)}),
}

META = {  # catalogue rows (id, name, kind, unlock month)
    "Product_IronScrap": ("Iron scrap", "loose", 1),
    "Product_Iron": ("Iron", "countable", 1),
    "Product_Steel": ("Steel", "countable", 1),
    "Product_Coal": ("Coal", "loose", 1),
    "Product_Wood": ("Wood", "countable", 1),
    "Product_Diesel": ("Diesel", "fluid", 1),
    "Product_Copper": ("Copper", "countable", 1),
    "Product_ConcreteSlab": ("Concrete slab", "countable", 1),
    "Product_ConstructionParts": ("Construction Parts", "countable", 1),
    "Product_MechanicalParts": ("Mechanical Parts", "countable", 1),
    "Product_Potato": ("Potato", "loose", 1),
    "Product_Vegetables": ("Vegetables", "countable", 1),
    "Product_Water": ("Water", "fluid", 1),
    "Product_IronOre": ("Iron ore", "loose", 7),
    "Product_Electronics": ("Electronics", "countable", 2),
    "Product_Limestone": ("Limestone", "loose", 9),
    "Product_CrudeOil": ("Crude oil", "fluid", 10),
    "Product_Rubber": ("Rubber", "countable", 10),
    "Product_CopperOre": ("Copper ore", "loose", 12),
    "Product_ConstructionParts2": ("Construction Parts II", "countable", 99),
}
LOCKED_EXTRA = ["Product_ConstructionParts2"]

# ------------------------------------------------------------ research
# (researched list after this month, current (id, progressPerc, steps,
# cost) or None, queue)
RESEARCH = {
    1: ([], ("ResearchCp1Production", 43750, 35, 80),
        ["ResearchPowerAndMaintenance"]),
    2: (["ResearchCp1Production"],
        ("ResearchPowerAndMaintenance", 55000, 44, 80), ["ResearchTradeDock"]),
    3: (["ResearchCp1Production"],
        ("ResearchPowerAndMaintenance", 86250, 69, 80), ["ResearchTradeDock"]),
    4: (["ResearchCp1Production", "ResearchPowerAndMaintenance"],
        ("ResearchTradeDock", 50000, 30, 60), ["ResearchVehicleAndMining"]),
    5: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
         "ResearchTradeDock"],
        ("ResearchVehicleAndMining", 45000, 36, 80), ["ResearchBeacon"]),
    6: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
         "ResearchTradeDock", "ResearchVehicleAndMining"],
        ("ResearchBeacon", 62500, 25, 40), ["ResearchTools"]),
    7: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
         "ResearchTradeDock", "ResearchVehicleAndMining", "ResearchBeacon"],
        ("ResearchTools", 55000, 33, 60), ["ResearchBasicDiesel"]),
    8: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
         "ResearchTradeDock", "ResearchVehicleAndMining", "ResearchBeacon",
         "ResearchTools"],
        ("ResearchBasicDiesel", 55000, 44, 80), ["ResearchRubber"]),
    9: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
         "ResearchTradeDock", "ResearchVehicleAndMining", "ResearchBeacon",
         "ResearchTools"],
        ("ResearchBasicDiesel", 87500, 70, 80), ["ResearchRubber"]),
    10: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
          "ResearchTradeDock", "ResearchVehicleAndMining", "ResearchBeacon",
          "ResearchTools", "ResearchBasicDiesel"],
         ("ResearchRubber", 60000, 36, 60), ["ResearchCopperRefinement"]),
    11: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
          "ResearchTradeDock", "ResearchVehicleAndMining", "ResearchBeacon",
          "ResearchTools", "ResearchBasicDiesel", "ResearchRubber"],
         ("ResearchCopperRefinement", 55000, 44, 80), ["ResearchCp2"]),
    12: (["ResearchCp1Production", "ResearchPowerAndMaintenance",
          "ResearchTradeDock", "ResearchVehicleAndMining", "ResearchBeacon",
          "ResearchTools", "ResearchBasicDiesel", "ResearchRubber",
          "ResearchCopperRefinement"],
         ("ResearchCp2", 30000, 24, 80), []),
}

# ------------------------------------------------------------ census
BASE_PROTOS = {"Settlement1": 1, "SmeltingFurnaceT1": 1, "CasterT1": 2,
               "CoalMaker": 1, "DieselGeneratorT1": 1, "ResearchLab1": 2,
               "TruckT1": 4, "StorageLoose": 2, "StorageUnit": 1,
               "StorageFluid": 1, "WasteDump": 1}
PROTO_ADDS = {  # month -> {proto: new count}
    2: {"StorageUnit": 2},
    3: {"AssemblyManual": 4, "FarmT1": 1},
    4: {"MaintenanceDepotT1": 1, "TruckT1": 5},
    5: {"MineT1": 1, "StorageLoose": 3},
    6: {"TruckT1": 7},
    7: {"MineT1": 2, "SmeltingFurnaceT1": 2, "CasterT1": 3},
    8: {"TruckT1": 8},
    9: {"MineT1": 3, "KilnT1": 1, "ConcreteMixerT1": 1, "FarmT1": 2},
    10: {"OilPumpT1": 3, "DistillerT1": 1, "DieselGeneratorT1": 2},
    11: {"RubberMakerT1": 1},
    12: {"MineT1": 4, "SmeltingFurnaceT1": 3, "ElectrolysisT1": 1},
}
TRANSPORTS = {1: (4, 4), 2: (4, 4), 3: (6, 6), 4: (6, 6), 5: (7, 7),
              6: (7, 7), 7: (8, 8), 8: (8, 8), 9: (9, 8), 10: (10, 10),
              11: (10, 10), 12: (11, 11)}  # month 9: one kiln belt dangling

POP = {n: 100 for n in range(1, 7)} | {n: 116 for n in range(7, 10)} | \
      {n: 132 for n in range(10, 13)}
HOUSING = {n: 160 for n in range(1, 9)} | {n: 224 for n in range(9, 13)}


def half(x):
    return int(round(x * 0.5))


def product_row(spec, n):
    """Stats row for one product at month n's snapshot, or None."""
    first = spec.get("first", 1)
    if n < first:
        return None
    f = spec["f"]
    if n == 1:
        p1, c1 = f[1]
        tm_p, tm_c = p1, c1  # month-1 handled by verbatim copy; unused
    life_p = sum(f[k][0] for k in range(first, n))
    life_c = sum(f[k][1] for k in range(first, n))
    tm_p, tm_c = half(f[n][0]), half(f[n][1])
    life_p += tm_p
    life_c += tm_c
    last_p, last_c = f[n - 1] if n - 1 >= first else (0, 0)
    qty = spec["start"] + life_p - life_c
    cap = spec["cap"]
    for m, c in sorted(spec.get("cap_sched", {}).items()):
        if n >= m:
            cap = c
    stored = max(0, min(qty - spec["buf"], cap))
    return {"quantity": qty, "stored": stored, "storageCapacity": cap,
            "produced": [tm_p, last_p, life_p, 0, life_p],
            "consumed": [tm_c, last_c, life_c, 0, life_c]}


def build_month(n):
    d = OUT / f"month-{n:02d}"
    d.mkdir(parents=True, exist_ok=True)
    if n == 1:  # verbatim early-game world, relabelled to the growth run
        for name in ("game-info.json", "stats.json", "products.json",
                     "pinned.json", "bridge-status.json",
                     "bridge-research.json", "bridge-census.json",
                     "bridge-storages.json", "bridge-trains.json"):
            doc = json.loads((SRC / name).read_text())
            if isinstance(doc, dict):
                if "gameId" in doc:
                    doc["gameId"] = GAME_ID
                if "sessionId" in doc:
                    doc["sessionId"] = SESSION_ID
            (d / name).write_text(json.dumps(doc, indent=1) + "\n")
        return
    date = f"1-{n}-15"
    tick = ((n - 1) * 30 + 15) * 2880
    stats = {}
    for pid, spec in F.items():
        row = product_row(spec, n)
        if row is not None:
            stats[pid] = row
    (d / "stats.json").write_text(json.dumps(
        {"date": date, "tick": tick, "products": stats, "unknownIds": []},
        indent=1) + "\n")
    # catalogue
    prods = []
    for pid, (name, kind, unlock) in META.items():
        first = F.get(pid, {}).get("first", 1)
        if n >= first or pid in LOCKED_EXTRA:
            prods.append({"id": pid, "name": name, "kind": kind,
                          "isUnlocked": n >= unlock})
    (d / "products.json").write_text(json.dumps(
        {"date": date, "tick": tick, "products": prods, "unknownIds": []},
        indent=1) + "\n")
    # pinned: diesel + scrap from month 1; + construction parts month 3
    pin_ids = ["Product_Diesel", "Product_IronScrap"] + \
        (["Product_ConstructionParts"] if n >= 3 else [])
    (d / "pinned.json").write_text(json.dumps(
        {"date": date, "tick": tick,
         "products": {p: stats[p] for p in pin_ids}, "unknownIds": []},
        indent=1) + "\n")
    (d / "game-info.json").write_text(json.dumps(
        {"date": date, "tick": tick, "isPaused": True, "speed": 1,
         "gameId": GAME_ID, "sessionId": SESSION_ID}, indent=1) + "\n")
    # settlement; monthsOfFood from food stock / last month's food burn
    food_qty = stats["Product_Potato"]["quantity"] + \
        stats["Product_Vegetables"]["quantity"]
    burn = (F["Product_Potato"]["f"][n - 1][1] +
            F["Product_Vegetables"]["f"][n - 1][1]) if n >= 3 else 80
    (d / "bridge-status.json").write_text(json.dumps(
        {"population": POP[n], "totalHousingCapacity": HOUSING[n],
         "freeHousingCapacity": HOUSING[n] - POP[n], "numberOfHomeless": 0,
         "monthsOfFood": int(round(food_qty / burn)),
         "arePeopleStarving": False, "settlements": 1}, indent=1) + "\n")
    # research
    res, cur, queue = RESEARCH[n]
    cur_doc = None
    if cur:
        cid, perc, steps, cost = cur
        cur_doc = {"id": cid, "progressPerc": perc, "stepsDone": steps,
                   "remainingSteps": cost - steps, "scienceCost": cost}
    (d / "bridge-research.json").write_text(json.dumps(
        {"current": cur_doc, "queue": queue, "researchedCount": len(res),
         "hasActiveLab": True, "researched": res}, indent=1) + "\n")
    # census
    protos = dict(BASE_PROTOS)
    for m in range(2, n + 1):
        protos.update(PROTO_ADDS.get(m, {}))
    if n >= 6:
        protos.pop("CoalMaker", None)  # retired smoothly in phase 3
    machines = {
        "SmeltingFurnaceT1": {"states": {"Working": protos["SmeltingFurnaceT1"]},
                              "recipes": ["IronSmeltingT1Scrap"] if n < 7
                              else ["IronSmeltingT1Scrap", "IronSmeltingT1Ore"]},
        "CasterT1": {"states": {"Working": protos["CasterT1"] - 1,
                                "NotEnoughInput": 1},
                     "recipes": ["IronCasting"]},
    }
    if "CoalMaker" in protos:
        machines["CoalMaker"] = {"states": {"Working": 1},
                                 "recipes": ["CoalFromWood"]}
    if n >= 3:
        machines["AssemblyManual"] = {
            "states": {"Working": 3, "NotEnoughInput": 1} if n == 3
            else {"Working": 4},
            "recipes": ["ConstructionParts1", "MechanicalParts"]}
        machines["FarmT1"] = {"states": {"Working": protos["FarmT1"]},
                              "recipes": ["PotatoFarming"]}
    if n >= 9:
        machines["KilnT1"] = {"states": {"Working": 1},
                              "recipes": ["ConcreteKiln"]}
        machines["ConcreteMixerT1"] = {"states": {"Working": 1},
                                       "recipes": ["ConcreteMixing"]}
    if n >= 10:
        machines["DistillerT1"] = {"states": {"Working": 1},
                                   "recipes": ["BasicDieselDistilling"]}
    tt, tf = TRANSPORTS[n]
    (d / "bridge-census.json").write_text(json.dumps(
        {"totalEntities": sum(protos.values()), "byProto": protos,
         "machines": machines,
         "transports": {"total": tt, "fullyConnected": tf}},
        indent=1) + "\n")
    # storages: rows consistent with stats (row qty <= product stored)
    rows = []

    def row(sid, proto, pid, cap):
        q = min(stats[pid]["stored"], cap)
        rows.append({"id": sid, "proto": proto, "product": pid,
                     "quantity": q, "capacity": cap,
                     "percentFullRaw": round(q / cap * 100000),
                     "logisticsInDisabled": False,
                     "logisticsOutDisabled": False})
    row(101, "StorageLoose", "Product_IronScrap", 1000)
    row(102, "StorageUnit", "Product_Iron", 400 if n >= 4 else 200)
    row(103, "StorageFluid", "Product_Diesel", 500 if n >= 10 else 250)
    row(104, "StorageLoose", "Product_Wood", 500)
    if n >= 4:
        row(105, "StorageUnit", "Product_ConstructionParts", 200)
    if n >= 5:
        row(106, "StorageLoose", "Product_Coal", 500)
    (d / "bridge-storages.json").write_text(json.dumps(
        {"storages": rows}, indent=1) + "\n")
    (d / "bridge-trains.json").write_text(json.dumps(
        {"lines": []}, indent=1) + "\n")


def main():
    for n in range(1, 13):
        build_month(n)
    # self-checks: internal consistency of every generated month
    problems = []
    for n in range(1, 13):
        d = OUT / f"month-{n:02d}"
        stats = json.loads((d / "stats.json").read_text())["products"]
        stor = json.loads((d / "bridge-storages.json").read_text())["storages"]
        for pid, r in stats.items():
            if r["quantity"] < 0:
                problems.append(f"m{n} {pid} negative quantity")
            if r["stored"] > r["storageCapacity"]:
                problems.append(f"m{n} {pid} stored > capacity")
            if r["stored"] > r["quantity"]:
                problems.append(f"m{n} {pid} stored > quantity")
            spec = F[pid]
            want = spec["start"] + r["produced"][4] - r["consumed"][4]
            if want != r["quantity"]:
                problems.append(f"m{n} {pid} qty {r['quantity']} != "
                                f"start+life net {want}")
        for row in stor:
            if row["quantity"] > stats[row["product"]]["stored"]:
                problems.append(f"m{n} storage {row['id']} qty > stats stored")
            if row["percentFullRaw"] != round(
                    row["quantity"] / row["capacity"] * 100000):
                problems.append(f"m{n} storage {row['id']} percentFullRaw off")
    if problems:
        print("SELF-CHECK FAILURES:")
        print("\n".join(problems))
        raise SystemExit(1)
    print(f"wrote 12 month dirs under {OUT} — self-checks passed")
    for n in range(1, 13):
        s = json.loads((OUT / f"month-{n:02d}" / "stats.json")
                       .read_text())["products"]
        st = json.loads((OUT / f"month-{n:02d}" / "bridge-status.json")
                        .read_text())
        rs = json.loads((OUT / f"month-{n:02d}" / "bridge-research.json")
                        .read_text())
        cp = s["Product_ConstructionParts"]
        print(f"m{n:02d} pop={st['population']} MoF={st['monthsOfFood']} "
              f"iron={s['Product_Iron']['quantity']} "
              f"CPbal_last={cp['produced'][1]-cp['consumed'][1]} "
              f"diesel={s['Product_Diesel']['quantity']} "
              f"researched={rs['researchedCount']}")


if __name__ == "__main__":
    main()
