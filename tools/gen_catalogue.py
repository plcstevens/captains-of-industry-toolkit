#!/usr/bin/env python3
"""Generate src/catalogue.rs from live/commands.json.

The catalogue is the companion's "actions understood" registry:
every command the game exposes over POST /command, classified by
domain, with its argument schema, entity targeting, verification
status, and hand notes for the commands that matter. Purposes are
read from names + schemas (labelled as such in the UI); only
commands marked VERIFIED have actually been executed and observed
through the website's prepare -> confirm -> audit flow.

Run from the coi crate root:  python3 ../tools/gen_catalogue.py
"""
import json
import re
import sys

SRC = "/home/hatch/workspace/coi-companion/live/commands.json"
OUT = "/home/hatch/workspace/coi-companion/coi/src/catalogue.rs"

VERIFIED = {
    "SetSimPauseStateCmd", "GameSpeedChangeCmd", "ResearchStartCmd",
    "ResearchStopCmd", "ResearchQueueDequeueCmd", "PinToggleCmd",
    "SetGlobalPriorityCmd", "ToggleEnabledCmd",
    "MachineToggleRecipeActiveCmd", "StorageSetProductCmd",
    "StorageDisableLogisticsToggleCmd", "SetGeneralPriorityCmd",
    # Via the generic schema form, executed + reverted live 2026-11-02:
    "StorageAlertSetEnabledCmd",
    # Protocol A-R minimal live test, Philip's real save, while paused,
    # 2026-11-05: placed StorageUnit (result = new EntityId 38208 at the
    # requested centre tile) and removed the untouched site; net zero,
    # zero cost, census 12503 -> 12504 -> 12503.
    "CreateStaticEntityCmd", "StartDeconstructionOfStaticEntityCmd",
}

EXCLUDED = {
    "GameConsoleCmd": "Arbitrary console access. Deliberately NOT exposed: the website's allowlist is the safety boundary, and this command would erase it.",
}

NOTES = {
    "CreateStaticEntityCmd": "THE construction primitive: places a construction SITE for a static entity from a prototype. VERIFIED live 2026-11-05 (Protocol A-R): transform.position is the CENTRE tile in bridge topology space; rotation {dx,dy} maps Deg0=(1,0) Deg90=(0,1) Deg180=(-1,0) Deg270=(0,-1); nothing is charged at placement (cost arrives as truck-delivered construction materials); the game's layout validators reject bad spots with a clean error and nothing placed; result = the new EntityId. allowValidationSuppression stays OFF. Semantics: qol-recon/09-placement.md. Belts/pipes are NOT placed by this command (BuildTransportCmd, absent from the web registry).",
    "FinishBuildOfStaticEntityCmd": "Unity-paid quick build of a placed construction site (arg is 'id', not 'entityId'); the free Cheat variant is absent from the web registry. Price formula unread in code — HUD eyeball prices it. Untested live.",
    "CreateTrainTrackEntityWithDirectionCmd": "Places rail track pieces with a travel direction — the rail-building primitive.",
    "CreateNewTrainLineCmd": "Creates an empty train line; schedule items and consists are separate commands (the Trains namespace is the deepest in the game: 68 commands).",
    "CreateNewTrainNetworkCmd": "Creates an empty train network (Update 4.1 system); stations join it via separate commands.",
    "AddVehicleToBuildQueueCmd": "Queues vehicle construction at a depot — how an agent grows the truck fleet.",
    "BuildVehicleForeverCmd": "Keeps a depot producing a vehicle type indefinitely.",
    "SetConstructionPausedCmd": "Pauses/resumes construction of one entity — construction flow control without placement.",
    "AcceptLoanCmd": "Borrows product against future repayment. Financially consequential; never autonomous without a policy from Philip.",
    "ResearchStartCmd": "Starts a research node (verified via prepare flow).",
    "StorageSetProductCmd": "Assigns/clears a storage's product (verified via prepare flow; a non-empty storage warns first).",
    "MachineToggleRecipeActiveCmd": "Enables/disables one recipe on one machine (verified via prepare flow).",
    "ToggleEnabledCmd": "Pauses/resumes any entity (verified: pause/speed executed live 2026-11-02).",
}

DOMAIN_BY_NS = [
    ("Mafi.Core.Trains", "Trains & rail"),
    ("Mafi.Base.Prototypes.Trains", "Trains & rail"),
    ("Mafi.Core.Vehicles", "Vehicles & logistics"),
    ("Mafi.Core.Fleet", "Vehicles & logistics"),
    ("Mafi.Core.Entities.Static", "Entities & construction"),
    ("Mafi.Core.Entities", "Entities & construction"),
    ("Mafi.Base.Prototypes.Buildings", "Entities & construction"),
    ("Mafi.Core.Bridges", "Entities & construction"),
    ("Mafi.Core.Factory.Lifts", "Entities & construction"),
    ("Mafi.Core.Buildings.RuinedBuildings", "Entities & construction"),
    ("Mafi.Core.Factory.Machines", "Machines & production"),
    ("Mafi.Core.Factory.Datacenters", "Machines & production"),
    ("Mafi.Base.Prototypes.Machines.PowerGenerators", "Machines & production"),
    ("Mafi.Core.Factory.Sorters", "Machines & production"),
    ("Mafi.Core.Buildings.OreSorting", "Machines & production"),
    ("Mafi.Core.Buildings.Storages", "Storages"),
    ("Mafi.Core.Factory.Transports", "Transports"),
    ("Mafi.Core.Factory.Zippers", "Transports"),
    ("Mafi.Core.Factory.ElectricPower", "Power"),
    ("Mafi.Core.Factory.NuclearReactors", "Power"),
    ("Mafi.Core.Buildings.Farms", "Farms & forestry"),
    ("Mafi.Core.Buildings.Forestry", "Farms & forestry"),
    ("Mafi.Core.Terrain.Trees", "Farms & forestry"),
    ("Mafi.Core.Terrain", "Terrain & designation"),
    ("Mafi.Core.World", "World, trade & ships"),
    ("Mafi.Core.Buildings.Shipyard", "World, trade & ships"),
    ("Mafi.Core.Buildings.Cargo", "World, trade & ships"),
    ("Mafi.Core.Entities.Ships", "World, trade & ships"),
    ("Mafi.Core.SpaceProgram", "Space program"),
    ("Mafi.Core.Research", "Research"),
    ("Mafi.Core.Products", "Simulation & system"),
    ("Mafi.Core.Simulation", "Simulation & system"),
    ("Mafi.Core.Console", "Simulation & system"),
    ("Mafi.Core.Input", "Simulation & system"),
    ("Mafi.Core.Messages", "Simulation & system"),
    ("Mafi.Core.MessageNotifications", "Simulation & system"),
    ("Mafi.Core.Maintenance", "Simulation & system"),
    ("Mafi.Core.Buildings.Offices", "Simulation & system"),
    ("Mafi.Core.Buildings.FuelStations", "Simulation & system"),
]


def domain(ns: str) -> str:
    for prefix, dom in DOMAIN_BY_NS:
        if ns == prefix or ns.startswith(prefix + "."):
            return dom
    return "Simulation & system"


def humanize(name: str) -> str:
    n = re.sub(r"Cmd$", "", name)
    n = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", n)
    return n[0].upper() + n[1:]


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> None:
    cmds = json.load(open(SRC))
    if isinstance(cmds, dict):
        cmds = cmds.get("commands", [])
    rows = []
    for c in cmds:
        name = c["type"]
        ns = c.get("namespace", "")
        params = []
        entity = False
        if c.get("constructors"):
            for p in c["constructors"][0].get("params", []):
                params.append(f"{p['name']}: {p.get('json') or p['type']}")
                if p["type"] == "EntityId":
                    entity = True
        status = "verified" if name in VERIFIED else (
            "excluded" if name in EXCLUDED else "wired")
        note = NOTES.get(name) or EXCLUDED.get(name) or ""
        spec = []
        if c.get("constructors"):
            for p in c["constructors"][0].get("params", []):
                spec.append((p["name"], p.get("json") or p["type"]))
        rows.append({
            "name": name,
            "domain": domain(ns),
            "purpose": humanize(name),
            "args": ", ".join(params),
            "spec": spec,
            "entity": entity,
            "status": status,
            "note": note,
        })
    rows.sort(key=lambda r: (r["domain"], r["name"]))
    with open(OUT, "w") as f:
        f.write("//! GENERATED by tools/gen_catalogue.py from live/commands.json\n")
        f.write("//! (the /commands inventory captured from Philip's game,\n")
        f.write("//! v0.8.8). Do not edit by hand; edit the generator.\n\n")
        f.write("pub(crate) struct CommandInfo {\n")
        f.write("    pub name: &'static str,\n    pub domain: &'static str,\n")
        f.write("    pub purpose: &'static str,\n    pub args: &'static str,\n")
        f.write("    pub spec: &'static [(&'static str, &'static str)],\n")
        f.write("    pub entity: bool,\n    pub status: &'static str,\n")
        f.write("    pub note: &'static str,\n}\n\n")
        f.write("pub(crate) const CATALOGUE: &[CommandInfo] = &[\n")
        for r in rows:
            spec = ", ".join(
                "(%s, %s)" % (rust_str(n), rust_str(t)) for n, t in r["spec"])
            f.write("    CommandInfo { name: %s, domain: %s, purpose: %s, args: %s, spec: &[%s], entity: %s, status: %s, note: %s },\n" % (
                rust_str(r["name"]), rust_str(r["domain"]),
                rust_str(r["purpose"]), rust_str(r["args"]), spec,
                "true" if r["entity"] else "false",
                rust_str(r["status"]), rust_str(r["note"])))
        f.write("];\n")
    import collections
    print("wrote", OUT, "entries:", len(rows))
    print("by status:", collections.Counter(r["status"] for r in rows))
    print("by domain:", collections.Counter(r["domain"] for r in rows))


if __name__ == "__main__":
    sys.exit(main())
