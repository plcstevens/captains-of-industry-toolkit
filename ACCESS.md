# Access recipe — Claudius → Philip's game (WORKING, 2026-11-02)

Verified live end-to-end on 2026-11-02. All steps are Philip-controlled
and revocable.

## Path

1. Philip's PC joined his own Tailscale tailnet (account:
   cokofreedom@gmail.com). PC machine name: **philip-gaming-home**
   (100.115.158.100). This VM is enrolled as device **`muse`**
   (100.92.93.50) — Philip approved the join link.
2. Game running, save loaded, console: `web_api_start`
   → game listens on PC loopback `127.0.0.1:3073`.
3. PC forwards loopback to the tailnet (raw TCP, no HTTPS certs):
   `tailscale serve --bg --tcp=3073 tcp://localhost:3073`
   (`tailscale serve --tcp=3073 off` removes it.)
4. Claudius fetches through the Tailscale tunnel proxy (port 3130),
   forcing CONNECT (`--proxytunnel`) and setting the Host header the
   game demands — without it the game replies
   `403 {"error":"forbidden","message":"Host must be 127.0.0.1:3073 or localhost:3073."}`:

```sh
tunnel_proxy="${HTTPS_PROXY%:*}:3130"
curl -s --proxytunnel --proxy "$tunnel_proxy" \
     -H "Host: localhost:3073" \
     "http://philip-gaming-home:3073/api/v1/game-info"
```

Notes:
- First request to each exact name/IP needs Philip's approval in the
  Muse app; approval then covers other ports on that address.
- Use the machine NAME consistently (a different name/IP = new approval).
- Origin rules don't bite: curl sends no Origin. Do NOT enable
  `allowOtherOriginsPost` — unnecessary, and it would let any web page
  send game commands.
- Kill switches (any one ends access): remove device `muse` from the
  tailnet · `tailscale serve --tcp=3073 off` · `web_api_stop` ·
  `tailscale down` on the VM.
- Claudius discipline: GET reads only. `/command` POSTs happen only
  through the §12 prepare/confirm flow, never ad hoc.

## First live pull (2026-11-02 ~14:47)

Files in `live/`: game-info, server-info, products, stats, pinned,
hist-keys (Steel/Diesel/Coal monthly history), src-steel, src-diesel,
commands (all 240), merged as `snapshot-388-2-27.json`.
State at pull: date **388-2-27**, tick 2,787,522, paused, speed 3,
gameId 5583213724730880438 (same playthrough as the morning capture),
sessionId 991004233207764232 (new session after restart).
