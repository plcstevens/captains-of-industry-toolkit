# Remote access — running the companion on a different machine than the game

The normal setup runs the companion on the game PC, where it talks to
the game's web API on loopback (`http://127.0.0.1:3073`). Nothing in
this document is needed for that setup.

If you run the companion somewhere else — a home server, a small VPS,
a second PC — it still only ever talks to *your* game, over a private
tunnel *you* control. There is no hosted service and no account: the
companion makes exactly one kind of outbound connection, to the game
endpoint you configure.

## What you need

1. **The game API running** on the game PC: load a save, open the
   in-game console, run `web_api_start`. The game then listens on the
   PC's loopback, port 3073. (It must be re-run after every game
   start.)
2. **A raw TCP forward** from the companion's machine to the game PC's
   loopback 3073. Any tool that forwards plain TCP works, for example:
   - an SSH tunnel (`ssh -L` / `-W`-style forwarding),
   - a WireGuard network where the game PC is a peer,
   - Tailscale (`tailscale serve --bg --tcp=3073 tcp://localhost:3073`
     on the game PC exposes its loopback to your tailnet only).
3. **The companion pointed at the tunnel**, with the Host header the
   game demands (next section).

## The Host-header rule

The game rejects any request whose `Host` header is not
`127.0.0.1:3073` or `localhost:3073`:

```
403 {"error":"forbidden","message":"Host must be 127.0.0.1:3073 or localhost:3073."}
```

That is why the forward must be raw TCP (not an HTTP reverse proxy,
which would rewrite Host), and why the companion has a separate
`--host-header` setting: it dials the tunnel address but presents
`Host: localhost:3073` to the game. Do **not** enable the game's
`allowOtherOriginsPost` option — it is unnecessary for this setup,
and it would let any web page send commands to your game.

## Configuring the companion

Three equivalent ways, highest precedence last: `coi.toml`, env, CLI.

**CLI flags:**

```sh
superintendent --game-url http://<game-pc-name>:3073 --host-header localhost:3073 status
superintendent --game-url http://<game-pc-name>:3073 --host-header localhost:3073 serve
```

**Environment:** `COI_GAME_URL`, `COI_HOST_HEADER`, `COI_PROXY`.

**Config file** — a `[game]` section in `coi.toml` (search order:
`$COI_CONFIG`, `./coi.toml`, `coi.toml` next to the database). This
is the convenient option when the companion runs as a service or by
double-click, where flags are awkward:

```toml
[game]
url = "http://<game-pc-name>:3073"
host_header = "localhost:3073"
# proxy = "http://proxy-host:port"   # only if the tunnel needs one
```

## When the tunnel needs a CONNECT proxy

Some environments can only reach the tunnel through an HTTP proxy.
`--proxy` accepts a proxy URL (`http://user:pass@host:port`) or a
plain `host:port`, and the companion tunnels through it with CONNECT.
The special value `--proxy env` derives a proxy from the standard
`HTTPS_PROXY` environment variable (with its port replaced by 3130) —
a recipe some hosted runtimes use; credentials given in a proxy URL
are used only for the Proxy-Authorization header and are never
printed or logged.

## Security notes

- Keep the forward private: a tailnet, a WireGuard network, or an SSH
  tunnel — never a public port-forward of the game API.
- The companion website itself binds to loopback by default
  (`127.0.0.1:8484`) and checks the Host header on every request.
  If you deliberately expose it beyond the machine it runs on
  (`serve --bind`), put it behind your tunnel or a reverse proxy
  with authentication — the site has no login of its own.
- Everything the companion stores (snapshots, history, the audit
  trail) lives in its local `coi.db` on the machine it runs on.
  Nothing is sent anywhere else.
