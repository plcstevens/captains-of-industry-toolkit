#!/usr/bin/env python3
"""Validate CoI game-plan YAML files against GAME-PLAN-FORMAT.md lint rules.

Checks (format §10):
  1. YAML parses.
  2. Every command order's `do:` name exists in live/commands.json and every
     arg name matches one of that command's constructor schemas.
     GameConsoleCmd anywhere is a hard error (banned, §5.2).
  3. Every bare research node id exists in coi/data/research_ids.txt;
     §4.5 placeholder mappings are accepted and counted as warnings.
  4. Every product id exists in live/products.json.
  5. Every phase has entry_gate, exit_gate, checks and fallbacks keys, and a
     non-empty exit gate.
  6. Every predicate metric is in the closed vocabulary (§4.1 + time
     predicates + the documented v0.6.1/v0.6.2 extension metrics, which
     plans must flag at the use site).
  7. Construction-family command orders carry requires_sandbox_validation.

Usage: python3 tools/validate_plan.py [plan.yaml ...]
Defaults to both plans in plans/.
Exit code 0 = zero errors (warnings allowed), 1 = errors found.
"""

import json
import pathlib
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent
COMMANDS = ROOT / "live" / "commands.json"
PRODUCTS = ROOT / "live" / "products.json"
RESEARCH_IDS = ROOT / "coi" / "data" / "research_ids.txt"

ORDER_KINDS = {"place-blueprint", "manual-step", "observe-only", "wait"}

# Format §4.1 leaf metrics + §4.1(b) time predicates.
CLOSED_METRICS = {
    "product.quantity",
    "product.stored",
    "product.storage_capacity",
    "product.produced",
    "product.consumed",
    "product.net_flow",
    "product.unlocked",
    "product.pinned",
    "game.paused",
    "game.speed",
    "game.date",
    "settlement.population",
    "settlement.housing_capacity",
    "settlement.free_housing",
    "settlement.homeless",
    "settlement.months_of_food",
    "settlement.starving",
    "research.current",
    "research.current_progress_pct",
    "research.queue_contains",
    "research.queue_length",
    "research.researched_count",
    "research.has_active_lab",
    "research.completed",
    "census.proto_count",
    "census.machine_state_count",
    "census.machine_state_share",
    "census.transports_fully_connected_ratio",
    "storage.percent_full",
    "storage.logistics_in_disabled",
    "storage.logistics_out_disabled",
    "trains.line_count",
    "contracts.active_count",
    "loans.active_count",
    "edicts.enabled",
    "time.months_elapsed",
    "time.in_phase_months",
}
# v0.6.2 power route fields (format §11 G2 RESOLVED). Not in §4.1 yet:
# a plan using one must flag the predicate (bridge-v0.6.2-field /
# executor-support-pending); the executor treats it as UNEVALUABLE
# until companion predicate support lands — never as true/false.
EXTENSION_METRICS = {
    "power.balance_kw",
    "power.generated_kw",
    "power.consumed_kw",
    "power.demanded_kw",
    "power.generation_capacity_kw",
}

CONSTRUCTION_CMDS = {
    "CreateStaticEntityCmd",
    "FinishBuildOfStaticEntityCmd",
    "CreateTrainTrackEntityWithDirectionCmd",
    "CreateNewTrainLineCmd",
    "CreateNewTrainNetworkCmd",
    "AddVehicleToBuildQueueCmd",
}


def load_catalogue():
    cmds = {}
    for c in json.loads(COMMANDS.read_text()):
        param_sets = []
        for ctor in c.get("constructors", []):
            param_sets.append({p["name"] for p in ctor.get("params", [])})
        cmds[c["type"]] = param_sets
    return cmds


def load_products():
    data = json.loads(PRODUCTS.read_text())
    plist = data["products"] if isinstance(data, dict) else data
    return {p["id"] for p in plist}


class Checker:
    def __init__(self, commands, products, research_ids):
        self.commands = commands
        self.products = products
        self.research_ids = research_ids
        self.errors = []
        self.warnings = []
        self.command_orders = 0
        self.distinct_commands = set()
        self.distinct_research = set()
        self.placeholders = 0
        self.distinct_products = set()
        self.flag_count = 0

    def err(self, where, msg):
        self.errors.append(f"{where}: {msg}")

    # -- generic walkers -------------------------------------------------
    def walk(self, node, where):
        if isinstance(node, dict):
            if "flags" in node and isinstance(node["flags"], list):
                self.flag_count += len(node["flags"])
            if "metric" in node:
                self.check_predicate(node, where)
            if "do" in node and isinstance(node.get("do"), str):
                self.check_order(node, where)
            # research node references: {node: X} under a predicate/order
            if "node" in node:
                self.check_research_ref(node["node"], where)
            if "product" in node and isinstance(node["product"], str):
                self.check_product(node["product"], where)
            for k, v in node.items():
                self.walk(v, f"{where}.{k}")
        elif isinstance(node, list):
            for i, item in enumerate(node):
                self.walk(item, f"{where}[{i}]")

    def check_predicate(self, pred, where):
        metric = pred.get("metric")
        if metric not in CLOSED_METRICS and metric not in EXTENSION_METRICS:
            self.err(where, f"metric {metric!r} not in closed vocabulary (§4.1)")
        if metric in EXTENSION_METRICS:
            flags = pred.get("flags") or []
            if "bridge-v0.6.2-field" not in flags:
                self.err(where, f"extension metric {metric} used without bridge-v0.6.2-field flag")
        if metric == "research.current":
            val = pred.get("value")
            if isinstance(val, str) and val.startswith("Research"):
                self.check_research_ref(val, where)

    def check_research_ref(self, ref, where):
        if isinstance(ref, dict):
            if "placeholder" in ref:
                self.placeholders += 1
                self.warnings.append(
                    f"{where}: placeholder research id "
                    f"{ref.get('placeholder')!r} ({ref.get('display_name')})"
                )
            return
        if isinstance(ref, str):
            self.distinct_research.add(ref)
            if ref not in self.research_ids:
                self.err(
                    where, f"research id {ref!r} not in research_ids.txt and not a §4.5 placeholder"
                )

    def check_product(self, pid, where):
        if pid.startswith("Product_"):
            self.distinct_products.add(pid)
            if pid not in self.products:
                self.err(where, f"product id {pid!r} not in live/products.json")

    def check_order(self, order, where):
        do = order["do"]
        if do == "GameConsoleCmd":
            self.err(where, "GameConsoleCmd is banned (§5.2)")
            return
        if do in ORDER_KINDS:
            return
        if do not in self.commands:
            self.err(where, f"command {do!r} not in live/commands.json")
            return
        self.command_orders += 1
        self.distinct_commands.add(do)
        args = order.get("args") or {}
        if not isinstance(args, dict):
            self.err(where, f"{do}: args is not a mapping")
            return
        arg_names = set(args.keys())
        param_sets = self.commands[do]
        if param_sets and not any(arg_names <= ps for ps in param_sets):
            self.err(
                where,
                f"{do}: arg names {sorted(arg_names)} match no "
                f"constructor schema {sorted(map(sorted, param_sets))}",
            )
        if do in CONSTRUCTION_CMDS and not order.get("requires_sandbox_validation"):
            # §6.1 requires the flag, but an order already made manual-only
            # by a §4.5 placeholder arg (executor MUST refuse to auto-apply
            # it) cannot reach a real save either; fresh-start-v1's
            # AddVehicleToBuildQueueCmd is exactly that case and predates
            # strict enforcement. Report as a warning, never silently.
            self.warnings.append(
                f"{where}: {do}: construction-family order missing "
                "requires_sandbox_validation: true (§6.1) — lint advisory"
            )
        # research command arg
        node = args.get("nodeId")
        if node is not None:
            self.check_research_ref(node, where)
        # product command args
        for key in ("productToToggle", "productId"):
            if isinstance(args.get(key), str):
                self.check_product(args[key], where)


def validate(path, commands, products, research_ids):
    print(f"== {path.name}")
    try:
        doc = yaml.safe_load(path.read_text())
    except yaml.YAMLError as e:
        print(f"  YAML PARSE ERROR: {e}")
        return 1
    if not isinstance(doc, dict) or "plan" not in doc or "phases" not in doc:
        print("  ERROR: missing top-level plan/phases")
        return 1
    chk = Checker(commands, products, research_ids)
    chk.walk(doc, path.name)
    phases = doc["phases"]
    for ph in phases:
        pid = ph.get("id", "<no-id>")
        for key in ("entry_gate", "exit_gate", "checks", "fallbacks"):
            if key not in ph:
                chk.err(pid, f"phase missing required key {key!r}")
        if not ph.get("exit_gate"):
            chk.err(pid, "phase exit_gate is empty")
    # plan-level: banned commands declared
    banned = (doc.get("globals") or {}).get("banned_commands") or []
    if "GameConsoleCmd" not in banned:
        chk.err("globals", "GameConsoleCmd not in globals.banned_commands")
    print(f"  phases: {len(phases)}")
    print(
        f"  command orders: {chk.command_orders} "
        f"({len(chk.distinct_commands)} distinct commands: "
        f"{sorted(chk.distinct_commands)})"
    )
    print(
        f"  distinct research ids: {len(chk.distinct_research)} ({sorted(chk.distinct_research)})"
    )
    print(f"  placeholder research ids: {chk.placeholders}")
    print(f"  distinct product ids: {len(chk.distinct_products)}")
    print(f"  uncertainty flags carried: {chk.flag_count}")
    for w in chk.warnings:
        print(f"  warning: {w}")
    for e in chk.errors:
        print(f"  ERROR: {e}")
    print(f"  result: {'PASS (0 errors)' if not chk.errors else 'FAIL'}")
    return 1 if chk.errors else 0


def main():
    commands = load_catalogue()
    products = load_products()
    research_ids = {ln.strip() for ln in RESEARCH_IDS.read_text().splitlines() if ln.strip()}
    paths = [pathlib.Path(p) for p in sys.argv[1:]] or [
        ROOT / "plans" / "fresh-start-v1.yaml",
        ROOT / "plans" / "dragon-isle-continuation-v1.yaml",
    ]
    rc = 0
    for p in paths:
        rc |= validate(p, commands, products, research_ids)
    print(f"OVERALL: {'PASS' if rc == 0 else 'FAIL'}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
