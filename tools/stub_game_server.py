#!/usr/bin/env python3
"""Stub game server for scratch-testing the CoI companion website.

Serves canned capture files over the exact HTTP surface the companion
talks to, so `coi serve --game-url http://127.0.0.1:<port>` can be
exercised end-to-end without a live game:

  Game API (vanilla web server):
    /api/v1/game-info                 -> live/game-info.json
    /api/v1/server-info               -> live/server-info.json
    /api/v1/products                  -> live/products.json
    /api/v1/products/stats            -> live/stats.json
    /api/v1/products/stats?pinned=..  -> live/pinned.json
    /api/v1/products/history          -> {"products":{}} (empty series)
    /api/v1/products/sources?ids=..   -> <capture>/src-diesel.json /
                                         src-steel.json when the fixture
                                         exists; 404 for other products
                                         (the game's untracked-flows case)

  CoiBridge routes:
    /coi/v1/hello                     -> <capture>/bridge-hello.json
    /coi/v1/status                    -> <capture>/bridge-status.json
    /coi/v1/research                  -> <capture>/bridge-research.json
    /coi/v1/trains-detail             -> <capture>/bridge-trains.json
    /coi/v1/storages                  -> <capture>/bridge-storages.json
    /coi/v1/census                    -> <capture>/bridge-census.json
    /coi/v1/topology                  -> <capture>/bridge-topology.json
    /coi/v1/machines                  -> <capture>/bridge-machines.json

Anything else gets a 404 (which is exactly how an older CoiBridge
looks to the companion: bridge_json() degrades to None).

Usage:
    python3 stub_game_server.py [--port 3999] [--capture DIR]

--capture defaults to ../dev/captures/live-synthetic-v065 (the synthetic
capture plus the generated v0.6.4 machines payload). Point it at
../live for a pre-v0.6.4 bridge (no /coi/v1/machines file -> 404 ->
the companion shows its "update the mod" degradation note).
Stdlib only.
"""

import argparse
import json
import os
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

GAME_ROUTES = {
    "/api/v1/game-info": "game-info.json",
    "/api/v1/server-info": "server-info.json",
    "/api/v1/products": "products.json",
    "/api/v1/products/stats": "stats.json",
}

BRIDGE_ROUTES = {
    "/coi/v1/hello": "bridge-hello.json",
    "/coi/v1/status": "bridge-status.json",
    "/coi/v1/research": "bridge-research.json",
    "/coi/v1/trains-detail": "bridge-trains.json",
    "/coi/v1/storages": "bridge-storages.json",
    "/coi/v1/census": "bridge-census.json",
    "/coi/v1/topology": "bridge-topology.json",
    "/coi/v1/machines": "bridge-machines.json",
    "/coi/v1/power": "bridge-power.json",
    "/coi/v1/terrain": "bridge-terrain.json",
    "/coi/v1/deposits": "bridge-deposits.json",
    "/coi/v1/designations": "bridge-designations.json",
    # The region route takes query params; the stub serves ONE
    # canned region (bridge-region.json) whatever was asked for.
    "/coi/v1/terrain/region": "bridge-region.json",
}

CAPTURE_DIR = os.path.join(ROOT, "dev", "captures", "live-synthetic-v065")


def load_json(path):
    with open(path, "rb") as f:
        return f.read()


def game_file(name):
    """Game-API payloads resolve capture-dir first, so a capture is
    a self-contained fixture (a variant capture can override stats,
    game-info, ...); ../live is the fallback for captures that only
    carry bridge files."""
    p = os.path.join(CAPTURE_DIR, name)
    if os.path.exists(p):
        return p
    return os.path.join(ROOT, "live", name)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):  # keep stdout clean
        pass

    def _send(self, code, body, ctype="application/json"):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        u = urlparse(self.path)
        path, qs = u.path, parse_qs(u.query)
        try:
            if path == "/api/v1/products/stats" and qs.get("pinned") == ["true"]:
                return self._send(200, load_json(game_file("pinned.json")))
            if path == "/api/v1/products/history":
                return self._send(200, json.dumps({"products": {}}).encode())
            if path == "/api/v1/products/sources":
                # Canned per-product fixtures live in the capture dir
                # (src-diesel.json / src-steel.json). Products without
                # a fixture 404 — exactly what the real game does for
                # products it doesn't track flows for, and what the
                # product page's named-gap state renders for.
                ids = (qs.get("ids") or [""])[0]
                fixture = {
                    "Product_Diesel": "src-diesel.json",
                    "Product_Steel": "src-steel.json",
                }.get(ids)
                if fixture:
                    f = os.path.join(CAPTURE_DIR, fixture)
                    if os.path.exists(f):
                        return self._send(200, load_json(f))
                return self._send(404, b'{"error":"no sources for this product"}')
            if path in GAME_ROUTES:
                return self._send(200, load_json(game_file(GAME_ROUTES[path])))
            if path in BRIDGE_ROUTES:
                f = os.path.join(CAPTURE_DIR, BRIDGE_ROUTES[path])
                if not os.path.exists(f):
                    return self._send(404, b'{"error":"route absent in this capture"}')
                return self._send(200, load_json(f))
            return self._send(404, b'{"error":"no such route"}')
        except FileNotFoundError as e:
            return self._send(500, json.dumps({"error": str(e)}).encode())

    def do_POST(self):
        # The companion never POSTs during page rendering; refuse loudly.
        self._send(405, b'{"error":"stub is read-only"}')


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=3999)
    ap.add_argument(
        "--capture", default=CAPTURE_DIR, help="directory holding bridge-*.json capture files"
    )
    args = ap.parse_args()
    CAPTURE_DIR = args.capture
    print(f"stub game server on 127.0.0.1:{args.port}, bridge capture: {CAPTURE_DIR}")
    HTTPServer(("127.0.0.1", args.port), Handler).serve_forever()
