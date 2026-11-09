mod catalogue;
mod client;
mod designate;
mod guides;
mod plan;
mod productivity;
mod saveinfo;
mod serve;
mod store;
mod terrain;

use client::Client;
use serde_json::Value;
use std::env;
use std::process::exit;

pub(crate) struct Config {
    pub(crate) authority: String,
    pub(crate) host_header: String,
    pub(crate) proxy: Option<client::ProxyCfg>,
}

fn main() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let args: Vec<String> = env::args().collect();
    // Game topology resolves in layers, lowest precedence first:
    // built-in default < [game] section of coi.toml < env < CLI flags.
    // (db_path resolves first because the coi.toml search order looks
    // next to the db.)
    let mut cli_game_url: Option<String> = None;
    let mut cli_proxy: Option<String> = None;
    let mut cli_host_header: Option<String> = None;
    let mut db_path = env::var("COI_DB").unwrap_or_else(|_| default_db_path());
    let mut rest: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--db" => {
                i += 1;
                db_path = args
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| die("--db needs a value"));
            }
            "--game-url" => {
                i += 1;
                cli_game_url = Some(
                    args.get(i)
                        .cloned()
                        .unwrap_or_else(|| die("--game-url needs a value")),
                );
            }
            "--proxy" => {
                i += 1;
                cli_proxy = args.get(i).cloned();
            }
            "--host-header" => {
                i += 1;
                cli_host_header = args.get(i).cloned();
            }
            "-h" | "--help" => {
                usage();
                return;
            }
            other => rest.push(other.to_string()),
        }
        i += 1;
    }

    let (toml_url, toml_host_header, toml_proxy) = load_game_section(&db_path);
    let game_url = cli_game_url
        .or_else(|| env::var("COI_GAME_URL").ok())
        .or(toml_url)
        .unwrap_or_else(|| "http://127.0.0.1:3073".into());
    let proxy = cli_proxy
        .or_else(|| env::var("COI_PROXY").ok())
        .or(toml_proxy);
    let host_header = cli_host_header
        .or_else(|| env::var("COI_HOST_HEADER").ok())
        .or(toml_host_header);

    let authority = parse_authority(&game_url);
    let host_header = host_header.unwrap_or_else(|| authority.clone());
    let proxy = proxy.and_then(|p| resolve_proxy(&p));
    let cfg = Config {
        authority,
        host_header,
        proxy,
    };

    if rest.is_empty() {
        exit(cmd_bare_launch(&cfg, &db_path));
    }

    let cmd = rest.first().map(|s| s.as_str()).unwrap_or("status");
    let arg1 = rest.get(1).cloned();
    let code = match cmd {
        "probe" => cmd_probe(&cfg),
        "status" => cmd_status(&cfg),
        "snapshot" => cmd_snapshot(&cfg, arg1, &db_path),
        "watch" => cmd_watch(&cfg, &db_path, &rest[1..]),
        "trend" => cmd_trend(&db_path, arg1),
        "db" => cmd_db(&db_path, &rest[1..]),
        "products" => cmd_products(&cfg, arg1),
        "report" => cmd_report(&cfg, arg1, &db_path),
        "forecast" => cmd_forecast(&cfg, arg1),
        "saveinfo" => saveinfo::cmd(&rest[1..]),
        "plan-check" => plan::cmd(&rest[1..]),
        "play" => plan::play_cmd(&rest[1..]),
        "terrain" => terrain::cmd(&cfg, &rest[1..]),
        "serve" => serve::cmd(&cfg, &db_path, &rest[1..]),
        _ => {
            eprintln!("unknown command: {cmd}");
            usage();
            2
        }
    };
    exit(code);
}

/// Bare launch — no subcommand (e.g. double-clicking `superintendent.exe`):
/// start the website with default serve options and open
/// it in the default browser. Explicit subcommands never come here.
/// If port 8484 is already in use the companion is almost certainly
/// already running: say so, still open the browser, and exit 0
/// instead of failing on the bind. Any other probe-bind failure
/// falls through to `serve` so its own bind error surfaces.
fn cmd_bare_launch(cfg: &Config, db_path: &str) -> i32 {
    const URL: &str = "http://127.0.0.1:8484/";
    match std::net::TcpListener::bind("127.0.0.1:8484") {
        Ok(listener) => drop(listener),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            println!("Superintendent appears to be already running at {URL}");
            println!("Opening it in your browser instead of starting a second copy.");
            open_browser(URL);
            return 0;
        }
        Err(_) => {
            // Not a port-in-use failure — fall through to serve so
            // its bind error is the one that surfaces.
        }
    }
    println!("Superintendent — starting the website at {URL}");
    println!("Opening it in your browser (no login — the site is loopback-only).");
    println!("Keep this window open while you use it (Ctrl-C to stop).");
    open_browser(URL);
    serve::cmd(cfg, db_path, &[])
}

/// Best-effort open of `url` in the default browser. Spawned and
/// fire-and-forget: a missing/failed browser must never stop the
/// server, so every error is ignored.
fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    let _ = command.spawn();
}

fn usage() {
    eprintln!(
        "superintendent — Superintendent, the Captain of Industry companion\n\n\
         USAGE:\n  superintendent [--game-url URL] [--proxy host:port|env] [--host-header H] [command]\n\n\
         With no command, superintendent starts the website at http://127.0.0.1:8484/ and opens it in your browser (if it is already running, it just opens the browser).\n\n\
         COMMANDS:\n  (none)   Start the companion website and open it in your browser\n  probe    Check the game web API and print server/game info\n  status   One-screen summary of the running game\n  snapshot Write a full snapshot JSON (arg: output path); also stored in the DB
  watch    Poll the game clock and snapshot on every tick change (opts: --every 60s|5m, --count N)
  trend    Quantity/flow trend from the DB (arg: product name filter)
  db       Database tools: `db info`, `db import <snapshot.json>`\n  products Table of all products (arg: name filter)\n  report   Triage board (Eisenhower) over live data (arg: output .md path)
  forecast Outlook at current rates: stock runways + storage fill ETAs (arg: saved raw-stats JSON bundle, offline)
  serve    Local dashboard at http://127.0.0.1:8484/ (opts: --port, --bind)
  saveinfo Inspect a .save file offline (arg: path; flags: --json, --extract-screenshot)\n  plan-check DRY-RUN plan evaluation vs a state dir (arg: plan.json; flags: --state-dir, --out)\n  play     PREPARE-ONLY session walk over a state sequence (arg: plan.json; flags: --state-seq, --session-out, --transcript-out)\n  terrain  Terrain sensing (CoiBridge 0.6.5): `terrain deposits`, `terrain region X0 Y0 X1 Y1`, `terrain estimate X0 Y0 X1 Y1 TARGET`\n\n\
         ENV: COI_GAME_URL, COI_PROXY, COI_HOST_HEADER, COI_DB (default: ./coi.db on Windows; on Linux/macOS the OS data folder — see INSTALL.md)\n\
         Config: [game] url/host_header/proxy in coi.toml (env and CLI override it).\n\
         Default game URL: http://127.0.0.1:3073 (direct, on the game PC).\n\
         Through any TCP tunnel from another machine (SSH, WireGuard, Tailscale, ...): --game-url http://<pc-name>:3073 --host-header localhost:3073, plus --proxy env or --proxy host:port if the tunnel needs a CONNECT proxy. See REMOTE-ACCESS.md."
    );
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    exit(2);
}

// ---------------------------------------------------------------------------
// Per-OS data/config locations (daemon round, SPEC Addendum XXIV)
// ---------------------------------------------------------------------------

/// The OS-conventional folder this app keeps its data in, for the
/// platforms where a bare relative default is the wrong answer:
/// macOS `~/Library/Application Support/Superintendent`, Linux
/// `$XDG_DATA_HOME/superintendent` (or `~/.local/share/...`).
/// Windows returns None: there the established behaviour — the
/// database beside the program — is the convention, and existing
/// installs must see zero change.
pub(crate) fn os_data_dir() -> Option<std::path::PathBuf> {
    let home = env::var_os("HOME").map(std::path::PathBuf::from);
    os_data_dir_from(env::var("XDG_DATA_HOME").ok().as_deref(), home.as_deref())
}

fn os_data_dir_from(
    xdg_data_home: Option<&str>,
    home: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let _ = xdg_data_home;
        home.map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Superintendent")
        })
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(x) = xdg_data_home {
            if !x.is_empty() {
                return Some(std::path::PathBuf::from(x).join("superintendent"));
            }
        }
        home.map(|h| h.join(".local").join("share").join("superintendent"))
    }
    #[cfg(not(unix))]
    {
        let _ = (xdg_data_home, home);
        None
    }
}

/// The OS-conventional config folder, same shapes as
/// [`os_data_dir`] but on the config side: macOS shares the
/// Application Support folder; Linux uses `$XDG_CONFIG_HOME`
/// (or `~/.config`). Windows returns None (zero change).
pub(crate) fn os_config_dir() -> Option<std::path::PathBuf> {
    let home = env::var_os("HOME").map(std::path::PathBuf::from);
    os_config_dir_from(env::var("XDG_CONFIG_HOME").ok().as_deref(), home.as_deref())
}

fn os_config_dir_from(
    xdg_config_home: Option<&str>,
    home: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let _ = xdg_config_home;
        home.map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Superintendent")
        })
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(x) = xdg_config_home {
            if !x.is_empty() {
                return Some(std::path::PathBuf::from(x).join("superintendent"));
            }
        }
        home.map(|h| h.join(".config").join("superintendent"))
    }
    #[cfg(not(unix))]
    {
        let _ = (xdg_config_home, home);
        None
    }
}

/// Where the database lives when neither COI_DB nor --db says
/// otherwise. Windows: `coi.db` beside the program / in the working
/// folder — exactly what existing installs already use. macOS and
/// Linux: a `coi.db` already sitting in the working folder still
/// wins (a folder install keeps working); otherwise the OS data
/// dir, created on demand — a daemon started by launchd/systemd
/// from an arbitrary working folder must not scatter (or lose) its
/// database there.
fn default_db_path() -> String {
    #[cfg(not(unix))]
    {
        "coi.db".to_string()
    }
    #[cfg(unix)]
    {
        if std::path::Path::new("coi.db").exists() {
            return "coi.db".to_string();
        }
        match os_data_dir() {
            Some(dir) => {
                let _ = std::fs::create_dir_all(&dir);
                dir.join("coi.db").to_string_lossy().into_owned()
            }
            None => "coi.db".to_string(),
        }
    }
}

/// Every place a coi.toml may live, in precedence order — shared by
/// the [game], [productivity] and [builder] readers so all three
/// sections resolve identically: $COI_CONFIG, ./coi.toml, coi.toml
/// next to the db, and finally the OS config dir (the daemon case:
/// a service's working folder is not the install folder). First
/// file that exists wins, as each reader has always done.
pub(crate) fn config_candidates(db_path: &str) -> Vec<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = env::var("COI_CONFIG") {
        candidates.push(p.into());
    }
    candidates.push("coi.toml".into());
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if !parent.as_os_str().is_empty() {
            candidates.push(parent.join("coi.toml"));
        }
    }
    if let Some(dir) = os_config_dir() {
        let p = dir.join("coi.toml");
        if !candidates.contains(&p) {
            candidates.push(p);
        }
    }
    candidates
}

fn parse_authority(url: &str) -> String {
    let stripped = url.strip_prefix("http://").unwrap_or_else(|| {
        die("only http:// game URLs are supported (the game API is plain HTTP)");
    });
    stripped.trim_end_matches('/').to_string()
}

/// The `[game]` section of coi.toml: `url`, `host_header`, `proxy` —
/// the topology a self-hoster on a second machine needs, expressible
/// in the config file so even the bare double-click launch picks it
/// up. Search order (shared by [productivity] and [builder] via
/// `config_candidates`): $COI_CONFIG, ./coi.toml, coi.toml next to
/// the db, the OS config dir. Parser is the same deliberately
/// minimal flat `key = value` style; unknown keys and other
/// sections are ignored.
fn load_game_section(db_path: &str) -> (Option<String>, Option<String>, Option<String>) {
    for path in config_candidates(db_path) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            return parse_game_toml(&text);
        }
    }
    (None, None, None)
}

fn parse_game_toml(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let (mut url, mut host_header, mut proxy) = (None, None, None);
    let mut in_game = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_game = line == "[game]";
            continue;
        }
        if in_game {
            if let Some((key, val)) = line.split_once('=') {
                let v = val.trim().trim_matches('"');
                if !v.is_empty() {
                    match key.trim() {
                        "url" => url = Some(v.to_string()),
                        "host_header" => host_header = Some(v.to_string()),
                        "proxy" => proxy = Some(v.to_string()),
                        _ => {}
                    }
                }
            }
        }
    }
    (url, host_header, proxy)
}

/// `--proxy env` derives a CONNECT tunnel proxy from HTTPS_PROXY with
/// its port replaced by 3130 (see client::env_proxy). `tailnet` is a
/// deprecated alias kept so older invocations keep working. Any other
/// value is parsed as a proxy URL or host:port.
fn resolve_proxy(p: &str) -> Option<client::ProxyCfg> {
    match p {
        "env" => client::env_proxy(),
        "tailnet" => client::tailnet_proxy(),
        _ => Some(client::parse_proxy(p)),
    }
}

pub(crate) fn make_client(cfg: &Config) -> Client {
    Client {
        authority: cfg.authority.clone(),
        host_header: cfg.host_header.clone(),
        proxy: cfg.proxy.clone(),
        timeout_secs: 60,
    }
}

pub(crate) fn get_json(c: &Client, path: &str) -> Result<Value, String> {
    let r = c.get(path)?;
    if r.status != 200 {
        return Err(format!(
            "GET {path} -> HTTP {}: {}",
            r.status,
            String::from_utf8_lossy(&r.body)
        ));
    }
    serde_json::from_slice(&r.body).map_err(|e| format!("GET {path}: bad JSON: {e}"))
}

fn cmd_probe(cfg: &Config) -> i32 {
    let c = make_client(cfg);
    match probe_all(&c) {
        Ok((server, game, routes)) => {
            println!("game API reachable at {}", cfg.authority);
            println!(
                "  server: port {}, otherOrigins {}",
                server["port"],
                sv(&server["otherOrigins"])
            );
            println!(
                "  game:   date {}, tick {}, paused {}, speed {}x",
                sv(&game["date"]),
                game["tick"],
                game["isPaused"],
                game["speed"]
            );
            println!(
                "  ids:    gameId {}, sessionId {}",
                sv(&game["gameId"]),
                sv(&game["sessionId"])
            );
            println!(
                "  routes: {} routes registered",
                routes.as_array().map(|a| a.len()).unwrap_or(0)
            );
            0
        }
        Err(e) => {
            eprintln!("probe failed: {e}");
            1
        }
    }
}

fn probe_all(c: &Client) -> Result<(Value, Value, Value), String> {
    let server = get_json(c, "/api/v1/server-info")?;
    let game = get_json(c, "/api/v1/game-info")?;
    let routes = get_json(c, "/routes")?;
    Ok((server, game, routes))
}

fn cmd_status(cfg: &Config) -> i32 {
    let c = make_client(cfg);
    let game = match get_json(&c, "/api/v1/game-info") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("status failed: {e}");
            return 1;
        }
    };
    let catalog = get_json(&c, "/api/v1/products").unwrap_or(Value::Null);
    let pinned = get_json(&c, "/api/v1/products/stats?pinned=true").unwrap_or(Value::Null);

    let mut names: std::collections::HashMap<String, String> = Default::default();
    if let Some(arr) = catalog["products"].as_array() {
        for p in arr {
            if let (Some(id), Some(n)) = (p["id"].as_str(), p["name"].as_str()) {
                names.insert(id.to_string(), n.to_string());
            }
        }
    }

    println!(
        "Date {} · tick {} · {} · speed {}x · game {}",
        sv(&game["date"]),
        game["tick"],
        if game["isPaused"].as_bool().unwrap_or(false) {
            "PAUSED"
        } else {
            "running"
        },
        game["speed"],
        sv(&game["gameId"])
    );
    println!();
    println!(
        "{:<24} {:>8} {:>15} {:>13} {:>13}",
        "Pinned product", "Quantity", "Stored / cap", "Prod TM/LM", "Cons TM/LM"
    );
    if let Some(map) = pinned["products"].as_object() {
        let mut rows: Vec<(&String, &Value)> = map.iter().collect();
        rows.sort_by_key(|(id, _)| names.get(*id).cloned().unwrap_or_else(|| id.to_string()));
        for (id, s) in rows {
            let name = names.get(id).cloned().unwrap_or_else(|| id.to_string());
            let prod = flow_pair(&s["produced"]);
            let cons = flow_pair(&s["consumed"]);
            println!(
                "{:<24} {:>8} {:>7} /{:<7} {:>13} {:>13}",
                name,
                fmt_num(&s["quantity"]),
                fmt_num(&s["stored"]),
                fmt_num(&s["storageCapacity"]),
                prod,
                cons
            );
        }
    } else {
        println!("(no pinned stats returned)");
    }
    0
}

/// String value without JSON quotes; other values via Display.
pub(crate) fn sv(v: &Value) -> String {
    v.as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| v.to_string())
}

/// Flow arrays are [thisMonth, lastMonth, thisYear, lastYear, lifetime].
pub(crate) fn flow_pair(v: &Value) -> String {
    match v.as_array() {
        Some(a) if a.len() >= 2 => format!("{}/{}", fmt_num(&a[0]), fmt_num(&a[1])),
        _ => "-".into(),
    }
}

pub(crate) fn fmt_num(v: &Value) -> String {
    match v.as_i64() {
        Some(n) => {
            let neg = n < 0;
            let digits = n.abs().to_string();
            let mut out = String::new();
            for (i, ch) in digits.chars().enumerate() {
                if i > 0 && (digits.len() - i) % 3 == 0 {
                    out.push(',');
                }
                out.push(ch);
            }
            if neg {
                format!("-{out}")
            } else {
                out
            }
        }
        None => v.to_string(),
    }
}

// ---------------------------------------------------------------------------
// snapshot / products / report
// ---------------------------------------------------------------------------

pub(crate) struct Live {
    pub(crate) game: Value,
    pub(crate) server: Value,
    pub(crate) names: std::collections::HashMap<String, String>,
    pub(crate) kinds: std::collections::HashMap<String, String>,
    pub(crate) stats: Value,            // /api/v1/products/stats (all)
    pub(crate) pinned_ids: Vec<String>, // order as pinned in game
    pub(crate) history: Value,          // monthly history for pinned ids (may be Null)
    pub(crate) census: Value,           // CoiBridge /coi/v1/census (Null when bridge absent)
    pub(crate) machines: Value,         // CoiBridge /coi/v1/machines (Null when bridge absent)
    pub(crate) terrain: Value,          // CoiBridge /coi/v1/terrain summary (Null pre-0.6.5)
    pub(crate) deposits: Value,         // CoiBridge /coi/v1/deposits (Null pre-0.6.5)
}

pub(crate) fn fetch_live(c: &Client) -> Result<Live, String> {
    let game = get_json(c, "/api/v1/game-info")?;
    let server = get_json(c, "/api/v1/server-info")?;
    let catalog = get_json(c, "/api/v1/products?includeLocked=true")?;
    let stats = get_json(c, "/api/v1/products/stats")?;
    let pinned_stats = get_json(c, "/api/v1/products/stats?pinned=true")?;

    let mut names = std::collections::HashMap::new();
    let mut kinds = std::collections::HashMap::new();
    if let Some(arr) = catalog["products"].as_array() {
        for p in arr {
            if let Some(id) = p["id"].as_str() {
                if let Some(n) = p["name"].as_str() {
                    names.insert(id.to_string(), n.to_string());
                }
                if let Some(k) = p["kind"].as_str() {
                    kinds.insert(id.to_string(), k.to_string());
                }
            }
        }
    }
    let mut pinned_ids = Vec::new();
    if let Some(map) = pinned_stats["products"].as_object() {
        pinned_ids = map.keys().cloned().collect();
    }
    // The history endpoint returns count+1 points: the leading point is
    // the state at the start of the window, then one point per month with
    // the last being the month in progress. Normalize to the trailing
    // count+0 series here so every consumer (trend, stall detection,
    // forecasts) sees exactly `count` monthly entries, oldest first.
    let history = if pinned_ids.is_empty() {
        Value::Null
    } else {
        let ids = pinned_ids.join(",");
        let mut h = get_json(
            c,
            &format!("/api/v1/products/history?ids={ids}&resolution=month&series=produced,consumed,quantity&count=8"),
        )
        .unwrap_or(Value::Null);
        if let Some(products) = h["products"].as_object_mut() {
            for (_, series) in products.iter_mut() {
                if let Some(obj) = series.as_object_mut() {
                    for (_, arr_v) in obj.iter_mut() {
                        if let Some(a) = arr_v.as_array_mut() {
                            if a.len() > 8 {
                                let drop = a.len() - 8;
                                a.drain(..drop);
                            }
                        }
                    }
                }
            }
        }
        h
    };
    // CoiBridge payloads, best-effort: the bridge is a separate mod
    // and may not be installed or the game may predate it. Absence is
    // normal and must never fail the vanilla fetch — the snapshot
    // simply carries no machine-state data, and the productivity
    // layer reports INSUFFICIENT for it (prep §5).
    let census = get_json(c, "/coi/v1/census").unwrap_or(Value::Null);
    let machines = get_json(c, "/coi/v1/machines").unwrap_or(Value::Null);
    // Terrain sensing (CoiBridge v0.6.5): the height SUMMARY and
    // the deposits payload ride along best-effort like the other
    // bridge payloads. Exact region heights are bulky and are
    // fetched on demand (Terrain page / `coi terrain`), never
    // per snapshot.
    let terrain = get_json(c, "/coi/v1/terrain").unwrap_or(Value::Null);
    let deposits = get_json(c, "/coi/v1/deposits").unwrap_or(Value::Null);
    Ok(Live {
        game,
        server,
        names,
        kinds,
        stats,
        pinned_ids,
        history,
        census,
        machines,
        terrain,
        deposits,
    })
}

pub(crate) fn snapshot_json(live: &Live) -> Value {
    let mut products = Vec::new();
    if let Some(map) = live.stats["products"].as_object() {
        for (id, s) in map {
            products.push(serde_json::json!({
                "id": id,
                "name": live.names.get(id).cloned().unwrap_or_else(|| id.clone()),
                "kind": live.kinds.get(id).cloned().unwrap_or_default(),
                "quantity": s["quantity"],
                "stored": s["stored"],
                "storageCapacity": s["storageCapacity"],
                "produced": s["produced"],
                "consumed": s["consumed"],
            }));
        }
    }
    products.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_string());
    let taken = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let snap = serde_json::json!({
        "schema": "coi-snapshot/1",
        "takenAtUnix": taken,
        "game": {
            "date": live.game["date"],
            "tick": live.game["tick"],
            "isPaused": live.game["isPaused"],
            "speed": live.game["speed"],
            "gameId": live.game["gameId"],
            "sessionId": live.game["sessionId"],
        },
        "server": live.server,
        "pinnedIds": live.pinned_ids,
        "products": products,
        "history": live.history,
        // Bridge payloads (schema v2 store support): persisted as
        // per-proto aggregates + per-instance rows by insert_snapshot.
        // Null when the bridge was unreachable at capture time.
        "bridgeCensus": live.census,
        "bridgeMachines": live.machines,
        // Terrain sensing (schema v4 store support): deposit
        // totals are derived into deposit_state rows by
        // insert_snapshot; the terrain summary is carried for
        // consumers of the snapshot JSON (heights themselves are
        // re-fetched on demand, they are not stored per snapshot).
        "bridgeTerrain": live.terrain,
        "bridgeDeposits": live.deposits,
    });
    snap
}

fn cmd_snapshot(cfg: &Config, out: Option<String>, db_path: &str) -> i32 {
    let c = make_client(cfg);
    let live = match fetch_live(&c) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("snapshot failed: {e}");
            return 1;
        }
    };
    take_snapshot(&live, out, db_path)
}

/// Shared snapshot path for `snapshot` and `watch`: write the JSON
/// file, compute the issue board, run the §5 productivity evaluation
/// against the store (it extends the board and records hysteresis
/// state), then insert everything in one transaction.
fn take_snapshot(live: &Live, out: Option<String>, db_path: &str) -> i32 {
    let snap = snapshot_json(live);
    let path = out.unwrap_or_else(|| {
        format!(
            "snapshots/{}-{}-{}.json",
            sv(&live.game["gameId"]),
            sv(&live.game["date"]),
            live.game["tick"]
        )
    });
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    match std::fs::write(&path, serde_json::to_string_pretty(&snap).unwrap()) {
        Ok(()) => {
            println!(
                "snapshot written: {path} ({} products, date {}, tick {})",
                snap["products"].as_array().map(|a| a.len()).unwrap_or(0),
                sv(&live.game["date"]),
                live.game["tick"]
            );
            let mut issues = compute_issues(live);
            match store::Store::open(db_path) {
                Ok(mut st) => {
                    let th = productivity::Thresholds::load(db_path);
                    for line in
                        productivity::evaluate_snapshot(&mut st, &snap, live, &mut issues, &th)
                    {
                        println!("{line}");
                    }
                    match st.insert_snapshot(&snap, &issues) {
                        Ok(Some(id)) => println!("db: stored as snapshot #{id} in {db_path}"),
                        Ok(None) => println!("db: this game tick is already stored in {db_path}"),
                        Err(e) => eprintln!("db warning: {e}"),
                    }
                }
                Err(e) => eprintln!("db warning: {e}"),
            }
            0
        }
        Err(e) => {
            eprintln!("snapshot write failed: {e}");
            1
        }
    }
}

/// SPEC §9: poll the game clock and snapshot whenever the tick
/// changes. This is the cadence path that lets the §5 productivity
/// windows accumulate honest evidence — roughly one snapshot per
/// in-game day at speed 1 is plenty (the estimator thins to ≥720
/// ticks itself, so denser polling costs disk, not correctness).
/// Robust by design: a failed poll (game off, save loading) is a
/// printed line and a retry, never an exit. Ctrl-C stops it.
fn cmd_watch(cfg: &Config, db_path: &str, args: &[String]) -> i32 {
    let mut interval_secs: u64 = 300;
    let mut max_count: usize = 0; // 0 = forever
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            // --every is the house spelling; --interval is accepted
            // as an alias because SPEC §9 writes it that way.
            "--every" | "--interval" => {
                i += 1;
                let raw = args
                    .get(i)
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| die("--every needs a value (e.g. 60s, 5m)"));
                interval_secs = parse_duration_secs(raw);
            }
            "--count" => {
                i += 1;
                max_count = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| die("--count needs a number"));
            }
            other => {
                eprintln!("watch: unknown option {other}");
                return 2;
            }
        }
        i += 1;
    }
    let c = make_client(cfg);
    println!("watch: polling every {interval_secs}s, snapshotting on tick change (Ctrl-C to stop)");
    let mut last_tick: Option<i64> = None;
    let mut taken: usize = 0;
    loop {
        match fetch_live(&c) {
            Ok(live) => {
                let tick = live.game["tick"].as_i64().unwrap_or(-1);
                if last_tick != Some(tick) {
                    take_snapshot(&live, None, db_path);
                    last_tick = Some(tick);
                    taken += 1;
                    if max_count > 0 && taken >= max_count {
                        println!("watch: --count {max_count} reached, stopping");
                        return 0;
                    }
                }
            }
            Err(e) => {
                eprintln!("watch: game API not answering ({e}); retrying in {interval_secs}s");
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// "60s" / "5m" / bare seconds → seconds. Bad input dies loudly
/// (a typo'd interval silently becoming 300s would waste a night).
fn parse_duration_secs(raw: &str) -> u64 {
    let (num, mult) = if let Some(n) = raw.strip_suffix('s') {
        (n, 1)
    } else if let Some(n) = raw.strip_suffix('m') {
        (n, 60)
    } else {
        (raw, 1)
    };
    match num.parse::<u64>().map(|n| n * mult) {
        Ok(n) if n > 0 => n,
        _ => die("--every needs a positive duration like 60s or 5m"),
    }
}

fn cmd_products(cfg: &Config, filter: Option<String>) -> i32 {
    let c = make_client(cfg);
    let live = match fetch_live(&c) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("products failed: {e}");
            return 1;
        }
    };
    let filter = filter.map(|f| f.to_lowercase());
    println!(
        "{:<24} {:<10} {:>8} {:>15} {:>13} {:>13} {:>8}",
        "Product", "Kind", "Quantity", "Stored / cap", "Prod TM/LM", "Cons TM/LM", "Net LM"
    );
    if let Some(map) = live.stats["products"].as_object() {
        let mut ids: Vec<&String> = map.keys().collect();
        ids.sort_by_key(|id| {
            live.names
                .get(*id)
                .cloned()
                .unwrap_or_else(|| id.to_string())
        });
        for id in ids {
            let name = live
                .names
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let kind = live.kinds.get(id).cloned().unwrap_or_default();
            if kind == "virtual" && filter.is_none() {
                continue;
            }
            if let Some(f) = &filter {
                if !name.to_lowercase().contains(f) {
                    continue;
                }
            }
            let s = &map[id];
            let net_lm = flow_at(&s["produced"], 1) - flow_at(&s["consumed"], 1);
            println!(
                "{:<24} {:<10} {:>8} {:>7} /{:<7} {:>13} {:>13} {:>8}",
                name,
                kind,
                fmt_num(&s["quantity"]),
                fmt_num(&s["stored"]),
                fmt_num(&s["storageCapacity"]),
                flow_pair(&s["produced"]),
                flow_pair(&s["consumed"]),
                net_lm
            );
        }
    }
    0
}

pub(crate) fn flow_at(v: &Value, idx: usize) -> i64 {
    v.as_array()
        .and_then(|a| a.get(idx))
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
}

// canon: board.importance
pub(crate) fn importance(id: &str) -> u8 {
    const HIGH: &[&str] = &[
        "Steel",
        "Diesel",
        "Coal",
        "CrudeOil",
        "FuelGas",
        "Hydrogen",
        "Fertilizer",
        "Bread",
        "FoodPack",
        "ConstructionParts",
        "ConcreteSlab",
        "IronOre",
        "Iron",
        "Wheat",
        "Soybean",
        "Potato",
        "Vegetables",
        "Tofu",
        "Electronics",
        "Water",
    ];
    const LOW: &[&str] = &[
        "Dirt",
        "Rock",
        "Sand",
        "Gravel",
        "Slag",
        "Waste",
        "Biomass",
        "Woodchips",
        "Compost",
        "Acid",
        "Exhaust",
    ];
    if HIGH.iter().any(|h| id.contains(h)) {
        3
    } else if LOW.iter().any(|l| id.contains(l)) {
        1
    } else {
        2
    }
}

pub(crate) struct Issue {
    pub(crate) quadrant: &'static str,
    pub(crate) product: String,
    pub(crate) title: String,
    pub(crate) evidence: String,
    pub(crate) fix: String,
    /// The generator class (DESIGN V2.7): drives the card's figure
    /// strip and its call-to-action. Set at construction; the web
    /// layer never re-derives it from the title string.
    pub(crate) class: &'static str,
}

/// Production rhythm. Continuous producers (most machines) make
/// monthly flows meaningful. Seasonal / bursty producers — farm
/// harvests land in one or a few months, ship deliveries in single
/// lumps — make a single zero or negative month meaningless: they
/// are judged on the finished-month history balance (or last year's
/// flows) instead. (Philip, 2026-11-02: "farms or boats are
/// different".)
pub(crate) fn is_seasonal(live: &Live, id: &str, s: &Value) -> bool {
    let prod = &live.history["products"][id]["produced"];
    if let Some(a) = prod.as_array() {
        let finished = &a[..a.len().saturating_sub(1)];
        if finished.len() >= 4 {
            let zeros = finished
                .iter()
                .filter(|v| v.as_i64().unwrap_or(0) == 0)
                .count();
            let total: i64 = finished.iter().map(|v| v.as_i64().unwrap_or(0)).sum();
            let peak = finished
                .iter()
                .map(|v| v.as_i64().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let mean = total as f64 / finished.len() as f64;
            let cons_avg = live.history["products"][id]["consumed"]
                .as_array()
                .map(|ca| {
                    let fin = &ca[..ca.len().saturating_sub(1)];
                    if fin.is_empty() {
                        0.0
                    } else {
                        fin.iter().map(|v| v.as_i64().unwrap_or(0)).sum::<i64>() as f64
                            / fin.len() as f64
                    }
                })
                .unwrap_or(0.0);
            // Bursts: at least half the finished months produce
            // nothing, yet the window's production is concentrated —
            // the peak month covers at least a month of consumption
            // and at least twice the average month's production.
            // (A dead line's last blip fails the consumption test;
            // a steady producer fails the zero-share test.)
            return zeros * 2 >= finished.len()
                && total > 0
                && (peak as f64) >= cons_avg.max(1.0)
                && (peak as f64) >= 2.0 * mean.max(1.0);
        }
    }
    // No usable history (product not pinned): produced over the last
    // year but nothing this or last month → intermittent; judge it
    // on the long window rather than flagging a stall.
    flow_at(&s["produced"], 3) > 0
        && flow_at(&s["produced"], 0) == 0
        && flow_at(&s["produced"], 1) == 0
}

/// Net flow per month for a seasonal product, from the steadiest
/// window available: finished-month history when present, else last
/// year's flows. Returns (net_per_month, basis description).
pub(crate) fn seasonal_net(live: &Live, id: &str, s: &Value) -> (f64, String) {
    let hp = &live.history["products"][id]["produced"];
    let hc = &live.history["products"][id]["consumed"];
    if let (Some(p), Some(c)) = (hp.as_array(), hc.as_array()) {
        let n = p.len().min(c.len()).saturating_sub(1);
        if n >= 3 {
            let ps: i64 = p[..n].iter().map(|v| v.as_i64().unwrap_or(0)).sum();
            let cs: i64 = c[..n].iter().map(|v| v.as_i64().unwrap_or(0)).sum();
            return (
                (ps - cs) as f64 / n as f64,
                format!(
                    "seasonal producer — balance over the last {n} finished months: produced {ps}, consumed {cs} (harvests/deliveries land in bursts, so monthly rates mislead)"
                ),
            );
        }
    }
    let p = flow_at(&s["produced"], 3);
    let c = flow_at(&s["consumed"], 3);
    (
        (p - c) as f64 / 12.0,
        format!(
            "seasonal producer — last year's balance: produced {p}, consumed {c} (bursts make monthly rates mislead)"
        ),
    )
}

pub(crate) fn compute_issues(live: &Live) -> Vec<Issue> {
    let mut issues: Vec<Issue> = Vec::new();
    if let Some(map) = live.stats["products"].as_object() {
        for (id, s) in map {
            let kind = live.kinds.get(id).cloned().unwrap_or_default();
            if kind == "virtual" {
                continue;
            }
            let name = live
                .names
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let qty = s["quantity"].as_i64().unwrap_or(0);
            let stored = s["stored"].as_i64().unwrap_or(0);
            let cap = s["storageCapacity"].as_i64().unwrap_or(0);
            let prod_lm = flow_at(&s["produced"], 1);
            let cons_lm = flow_at(&s["consumed"], 1);
            let prod_tm = flow_at(&s["produced"], 0);
            let cons_tm = flow_at(&s["consumed"], 0);
            let net_lm = prod_lm - cons_lm;
            let imp = importance(id);

            // Trend from monthly history (pinned products only).
            let trend = history_trend(&live.history, id);
            let trend_declining = {
                let q = &live.history["products"][id]["quantity"];
                match q.as_array() {
                    Some(a) if a.len() >= 3 => {
                        let f = a.first().and_then(|v| v.as_i64()).unwrap_or(0);
                        let l = a.last().and_then(|v| v.as_i64()).unwrap_or(0);
                        l < f
                    }
                    _ => false,
                }
            };
            // Finished-month zeros in production (last element of a
            // history series is the period in progress, so drop it).
            let prod_zeros_recent = {
                let p = &live.history["products"][id]["produced"];
                p.as_array().map(|a| {
                    let finished = &a[..a.len().saturating_sub(1)];
                    finished
                        .iter()
                        .rev()
                        .take(3)
                        .filter(|v| v.as_i64().unwrap_or(0) == 0)
                        .count()
                })
            };
            // Import-fed products are exempt from the stall detector:
            // zero production is the plan, not a failure. canon:
            // board.import-fed-products — the rule is explicit there,
            // including its current limitation: the exemption list is
            // exactly one product (coal). Widening it is a canon change
            // first, a code change second.
            let import_fed = id == "Product_Coal";
            // Farms and ship-borne supply produce in bursts; they get
            // the yearly-balance treatment below, never the monthly
            // stall/deficit detectors.
            let seasonal = is_seasonal(live, id, s);

            // canon: board.overcap
            if cap >= 200 && stored > cap {
                issues.push(Issue {
                    quadrant: "Watch",
                    product: name.clone(),
                    title: "Stored above nominal capacity".into(),
                    class: "overcap",
                    evidence: format!("stored {stored} vs capacity {cap} (quantity {qty})"),
                    fix: "Check what holds the excess (vehicles/machine buffers?) and whether capacity accounting changed in 0.8.8.".into(),
                });
            }
            if prod_lm == 0 && cons_lm > 0 && !import_fed && !seasonal {
                let cover = qty as f64 / cons_lm as f64;
                // Require the stall to be real: production zero in at
                // least 2 of the last 3 finished months (farms harvest
                // in bursts, so one zero month is not a stall), and
                // less than ~3 years of cover.
                // canon: board.stall-confirmation
                let confirmed = prod_zeros_recent.map(|z| z >= 2).unwrap_or(true);
                if confirmed && cover < 36.0 {
                    let urgent = cover < 3.0;
                    issues.push(Issue {
                    quadrant: if imp == 3 && urgent {
                        "Fix now"
                    } else if imp == 3 {
                        "Fix later"
                    } else if urgent {
                        "Watch"
                    } else {
                        "Ignore for now"
                    },
                    product: name.clone(),
                    title: "Production stalled, consumption continues".into(),
                    class: "stalled",
                    evidence: format!(
                        "produced 0 last month ({} this month), consumed {cons_lm}/mo, stock {qty} ≈ {cover:.1} months of cover{trend}",
                        prod_tm
                    ),
                    fix: fix_hint(id, "stalled"),
                });
                }
            } else if net_lm < 0 && qty > 0 && !seasonal {
                let months = qty as f64 / (-net_lm) as f64;
                // canon: board.deficit-horizon / board.urgency
                if months < 24.0 {
                    let urgent = months < 3.0;
                    issues.push(Issue {
                        quadrant: if imp == 3 && urgent {
                            "Fix now"
                        } else if imp == 3 {
                            "Fix later"
                        } else if urgent {
                            "Watch"
                        } else {
                            "Ignore for now"
                        },
                        product: name.clone(),
                        title: "Net deficit — buffer draining".into(),
                    class: "deficit",
                        evidence: format!(
                            "net {net_lm}/mo last month, stock {qty} ≈ {months:.1} months at that rate{trend}"
                        ),
                        fix: fix_hint(id, "deficit"),
                    });
                } else if trend_declining && prod_tm - cons_tm < 0 && months < 120.0 {
                    // canon: board.structural-horizon
                    // Structural deficit: no near-term cliff, but the
                    // stock falls every month and this month is also
                    // negative — a slow leak worth planning around.
                    issues.push(Issue {
                        quadrant: if imp == 3 { "Fix later" } else { "Watch" },
                        product: name.clone(),
                        title: "Structural deficit — stock falls every month".into(),
                    class: "structural",
                        evidence: format!(
                            "net {net_lm}/mo last month and {} this month, stock {qty} ≈ {months:.1} months at last month's rate{trend}",
                            prod_tm - cons_tm
                        ),
                        fix: fix_hint(id, "deficit"),
                    });
                }
            }
            if seasonal && !import_fed && qty > 0 {
                // Judged on the long window only: a seasonal producer
                // in real deficit still matters, but as a planning
                // item, never a monthly "stall".
                let (net_m, basis) = seasonal_net(live, id, s);
                if net_m < 0.0 {
                    let months = qty as f64 / (-net_m);
                    // canon: board.seasonal-window
                    if months < 36.0 {
                        issues.push(Issue {
                            quadrant: if imp == 3 { "Fix later" } else { "Watch" },
                            product: name.clone(),
                            title: "Seasonal producer in deficit on the long-window balance".into(),
                            class: "seasonal",
                            evidence: format!(
                                "{basis}; stock {qty} ≈ {months:.1} months at that average rate"
                            ),
                            fix: fix_hint(id, "deficit"),
                        });
                    }
                }
            }
            // canon: board.storage-full
            if cap >= 200 && stored >= cap {
                if prod_lm > 0 {
                    issues.push(Issue {
                        quadrant: if imp == 3 { "Fix later" } else { "Watch" },
                        product: name.clone(),
                        title: "Storage full while still producing — output will block".into(),
                    class: "storage-full",
                        evidence: format!("stored {stored} / capacity {cap}, produced {prod_lm} last month"),
                        fix: "Add storage, raise draw, or pause the producer before it backs up the chain.".into(),
                    });
                } else if cons_lm == 0 && prod_lm == 0 {
                    issues.push(Issue {
                        quadrant: "Ignore for now",
                        product: name.clone(),
                        title: "Storage full, chain parked (no production or draw)".into(),
                    class: "parked",
                        evidence: format!("stored {stored} / capacity {cap}, no movement last month"),
                        fix: "Consistent with a filled buffer / war chest. No action unless draw resumes.".into(),
                    });
                }
            }
        }
    }

    issues
}

/// One forward-looking estimate for a single product, computed from the
/// vanilla API only: last month's net flow (LM), this month's net flow
/// (TM), the 7-month quantity slope (pinned products only), current stock
/// and storage headroom. `months` is the runway (deficit) or time-to-full
/// (surplus) at the blended recent rate; `basis` names the rates used so
/// the number is never presented as more precise than it is.
pub(crate) struct Forecast {
    pub(crate) product: String,
    pub(crate) kind: &'static str, // "runway" | "fill" | "stall-runway"
    pub(crate) months: f64,
    pub(crate) basis: String,
    pub(crate) advice: String,
    /// The measured flows behind the forecast (DESIGN V2.3: the
    /// Outlook table renders these as delta chips instead of
    /// burying them in the basis sentence; the basis string itself
    /// is kept verbatim as the caption).
    pub(crate) net_tm: i64,
    pub(crate) net_lm: i64,
    pub(crate) stored: i64,
}

// canon: forecasts.runway-blend, forecasts.fill, forecasts.seasonal
// (all horizons stated together in forecasts.horizons-together)
pub(crate) fn compute_forecasts(live: &Live) -> Vec<Forecast> {
    let mut out: Vec<Forecast> = Vec::new();
    // Stats payloads come in two shapes: /api/v1/products/stats nests the
    // product map under "products", but some saved bundles keep the map at
    // the top level with date/tick siblings. Accept both.
    let stats_map = if live.stats["products"].is_object() {
        live.stats["products"].as_object()
    } else {
        live.stats.as_object()
    };
    if let Some(map) = stats_map {
        for (id, s) in map {
            let kind_of = live.kinds.get(id).cloned().unwrap_or_default();
            if kind_of == "virtual" {
                continue;
            }
            let name = live
                .names
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let qty = s["quantity"].as_i64().unwrap_or(0);
            let stored = s["stored"].as_i64().unwrap_or(0);
            let cap = s["storageCapacity"].as_i64().unwrap_or(0);
            // Stats flow order is [thisMonth, lastMonth, ...] — the same
            // indices flow_pair renders as "TM/LM" and compute_issues uses.
            let net_tm = flow_at(&s["produced"], 0) - flow_at(&s["consumed"], 0);
            let net_lm = flow_at(&s["produced"], 1) - flow_at(&s["consumed"], 1);
            // Blend LM + TM for the headline rate; keep the history slope
            // as the cross-check folded into the basis string.
            let net = net_lm + net_tm;
            let slope = {
                let q = &live.history["products"][id]["quantity"];
                q.as_array().and_then(|a| {
                    if a.len() >= 3 {
                        let f = a.first().and_then(|v| v.as_i64()).unwrap_or(0);
                        let l = a.last().and_then(|v| v.as_i64()).unwrap_or(0);
                        Some((l - f) as f64 / (a.len() - 1) as f64)
                    } else {
                        None
                    }
                })
            };
            let imp = importance(id);
            // History forecast: the monthly quantity slope (finished
            // months only — the last point is the period in progress).
            // When the blended TM/LM flows and the slope disagree on the
            // sign (a partial month can flip either), the slope over the
            // full window is the steadier signal for importance-3
            // products, so they always get a runway line inside 120
            // months — the slow structural drains are the ones worth
            // planning around.
            let slope_finished = {
                let q = &live.history["products"][id]["quantity"];
                q.as_array().and_then(|a| {
                    if a.len() >= 4 {
                        let fin = &a[..a.len() - 1];
                        let f = fin.first().and_then(|v| v.as_i64()).unwrap_or(0);
                        let l = fin.last().and_then(|v| v.as_i64()).unwrap_or(0);
                        Some(((l - f) as f64 / (fin.len() - 1) as f64, fin.len()))
                    } else {
                        None
                    }
                })
            };
            let seasonal = is_seasonal(live, id, s);
            if let Some((sl, n_fin)) = slope_finished {
                if sl < 0.0
                    && qty > 0
                    && net <= 0
                    && net > -120
                    && id != "Product_Coal"
                    && !seasonal
                {
                    let months = qty as f64 / (-sl);
                    // canon: forecasts.slope
                    if months < 120.0 && imp == 3 {
                        out.push(Forecast {
                            product: name.clone(),
                            kind: "runway",
                            months,
                            basis: format!(
                                "monthly stock slope {sl:.0}/mo over {n_fin} finished months; recent flows: net {net_tm}/mo this month, {net_lm}/mo last month"
                            ),
                            advice: fix_hint(id, "deficit"),
                            net_tm,
                            net_lm,
                            stored,
                        });
                    }
                }
            }
            if seasonal {
                // Bursts make the TM/LM blend and the sawtooth slope
                // meaningless; the long-window balance is the rate.
                let (net_m, basis) = seasonal_net(live, id, s);
                if net_m < 0.0 && qty > 0 {
                    let months = qty as f64 / (-net_m);
                    if months < 120.0 {
                        out.push(Forecast {
                            product: name.clone(),
                            kind: "runway",
                            months,
                            basis,
                            advice: fix_hint(id, "deficit"),
                            net_tm,
                            net_lm,
                            stored,
                        });
                    }
                }
                continue;
            }
            if net < 0 && qty > 0 {
                let rate = -(net as f64) / 2.0;
                let months = qty as f64 / rate;
                if months < 36.0 {
                    let mut basis = format!("net {net_lm}/mo last month, {net_tm}/mo this month");
                    if let Some(sl) = slope {
                        basis.push_str(&format!("; history slope {sl:.0}/mo"));
                    }
                    out.push(Forecast {
                        product: name.clone(),
                        kind: if flow_at(&s["produced"], 0) == 0 && flow_at(&s["consumed"], 0) > 0 {
                            "stall-runway"
                        } else {
                            "runway"
                        },
                        months,
                        basis,
                        advice: if imp == 3 {
                            fix_hint(id, "deficit")
                        } else {
                            "Plan a supply increase or a draw cut before the runway ends.".into()
                        },
                        net_tm,
                        net_lm,
                        stored,
                    });
                }
            } else if net > 0 && cap >= 200 && stored < cap {
                // canon: forecasts.fill
                let headroom = (cap - stored) as f64;
                let months = headroom / (net as f64 / 2.0);
                if months < 12.0 {
                    out.push(Forecast {
                        product: name.clone(),
                        kind: "fill",
                        months,
                        basis: format!(
                            "net +{net_lm}/mo last month, +{net_tm}/mo this month; stored {stored} of {cap}"
                        ),
                        advice: "Add storage, raise the draw, or plan a pause point before the output blocks.".into(),
                        net_tm,
                        net_lm,
                        stored,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.months
            .partial_cmp(&b.months)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub(crate) fn render_forecasts_md(forecasts: &[Forecast]) -> String {
    if forecasts.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Outlook — forecasts at current rates\n\n");
    md.push_str("Assumes production/consumption rates stay as measured; any build, pause or trade change moves these dates.\n\n");
    for f in forecasts {
        let (label, unit) = match f.kind {
            "fill" => ("storage full in", "months"),
            "stall-runway" => ("stock lasts (production stalled)", "months"),
            _ => ("stock lasts", "months"),
        };
        md.push_str(&format!(
            "- **{}** — {} ≈ {:.1} {}\n  - Basis: {}\n  - Advice: {}\n",
            f.product, label, f.months, unit, f.basis, f.advice
        ));
    }
    md.push('\n');
    md
}

pub(crate) fn render_report_md(live: &Live, issues: &[Issue]) -> String {
    let order = ["Fix now", "Fix later", "Watch", "Ignore for now"];
    let mut md = String::new();
    md.push_str(&format!(
        "# CoI triage — {} (tick {}, {})\n\n",
        sv(&live.game["date"]),
        live.game["tick"],
        if live.game["isPaused"].as_bool().unwrap_or(false) {
            "paused"
        } else {
            "running"
        }
    ));
    for q in order {
        let items: Vec<&Issue> = issues.iter().filter(|i| i.quadrant == q).collect();
        md.push_str(&format!("## {q} ({})\n\n", items.len()));
        for i in items {
            md.push_str(&format!(
                "- **{}** — {}\n  - Evidence: {}\n  - Suggested fix: {}\n",
                i.product, i.title, i.evidence, i.fix
            ));
        }
        md.push('\n');
    }
    md.push_str(&render_forecasts_md(&compute_forecasts(live)));
    md
}

/// `coi forecast [bundle.json]` — the Outlook section on its own.
/// With a bundle file (the raw stats shape saved under live/), runs fully
/// offline; history series are unavailable in that shape, so those
/// forecasts carry flows-only bases. Without a file, fetches live.
fn cmd_forecast(cfg: &Config, file: Option<String>) -> i32 {
    let live = match file {
        Some(path) => {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("forecast: cannot read {path}: {e}");
                    return 1;
                }
            };
            let bundle: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("forecast: bad JSON in {path}: {e}");
                    return 1;
                }
            };
            // Saved bundles (live/*.json) hold: "stats" = the raw
            // /products/stats payload (map under its own "products"),
            // "products" = the raw catalog payload (array under ITS
            // "products"), "game-info". The bare stats shape (map at top
            // level) is also accepted.
            let stats = if bundle["stats"]["products"].is_object() {
                bundle["stats"].clone()
            } else if bundle["products"].is_object() {
                bundle.clone()
            } else {
                eprintln!("forecast: {path} has no products stats object");
                return 1;
            };
            let mut names = std::collections::HashMap::new();
            let mut kinds = std::collections::HashMap::new();
            if let Some(arr) = bundle["products"]["products"].as_array() {
                for p in arr {
                    if let Some(id) = p["id"].as_str() {
                        if let Some(n) = p["name"].as_str() {
                            names.insert(id.to_string(), n.to_string());
                        }
                        if let Some(k) = p["kind"].as_str() {
                            kinds.insert(id.to_string(), k.to_string());
                        }
                    }
                }
            }
            let gi = bundle["game-info"].clone();
            // Merge any sibling raw history payload (live/ folders keep it
            // as a separate capture with the same date/tick).
            let mut history = bundle["history"].clone();
            if !history["products"].is_object() {
                if let Some(dir) = std::path::Path::new(&path).parent() {
                    for cand in ["hist-keys.json", "history.json"] {
                        if let Ok(t) = std::fs::read_to_string(dir.join(cand)) {
                            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                                if v["products"].is_object() && v["tick"] == stats["tick"] {
                                    history = v;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Live {
                game: if gi.is_object() {
                    gi
                } else {
                    serde_json::json!({"date": stats["date"], "tick": stats["tick"]})
                },
                server: Value::Null,
                names,
                kinds,
                stats,
                pinned_ids: Vec::new(),
                history,
                census: Value::Null,
                machines: Value::Null,
                terrain: Value::Null,
                deposits: Value::Null,
            }
        }
        None => {
            let c = make_client(cfg);
            match fetch_live(&c) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("forecast failed: {e}");
                    return 1;
                }
            }
        }
    };
    let forecasts = compute_forecasts(&live);
    if forecasts.is_empty() {
        println!("forecast: nothing inside the outlook horizons (runways < 36 months, fills < 12 months, slope/seasonal outlooks < 120 months — canon: forecasts.horizons-together)");
    } else {
        print!("{}", render_forecasts_md(&forecasts));
    }
    0
}

fn cmd_report(cfg: &Config, out: Option<String>, db_path: &str) -> i32 {
    let c = make_client(cfg);
    let live = match fetch_live(&c) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("report failed: {e}");
            return 1;
        }
    };
    let issues = compute_issues(&live);
    let mut md = render_report_md(&live, &issues);
    // Productivity evidence (prep §5) is store-based: append it when
    // a database with snapshots exists. Absence is silent — a report
    // on a fresh machine must not sprout an empty section.
    if std::path::Path::new(db_path).exists() {
        if let Ok(st) = store::Store::open(db_path) {
            let th = productivity::Thresholds::load(db_path);
            if let Ok(lines) = productivity::report_lines(&st, &th) {
                if !lines.is_empty() {
                    md.push_str("\n## Productivity evidence (QOL-MOD-PREP §5)\n\n");
                    for line in lines {
                        md.push_str(&format!("- {line}\n"));
                    }
                }
            }
        }
    }
    match out {
        Some(path) => match std::fs::write(&path, &md) {
            Ok(()) => {
                println!("report written: {path}");
                print!("{}", fix_now_section(&md));
                0
            }
            Err(e) => {
                eprintln!("report write failed: {e}");
                1
            }
        },
        None => {
            print!("{md}");
            0
        }
    }
}

fn fix_now_section(md: &str) -> String {
    let mut out = String::new();
    let mut in_fix_now = false;
    for line in md.lines() {
        if line.starts_with("## Fix now") {
            in_fix_now = true;
        } else if line.starts_with("## ") && in_fix_now {
            break;
        }
        if in_fix_now {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// " · quantity fell 1,347 → 1,108 over the last 7 months" style suffix,
/// or empty when no history exists for the product.
fn history_trend(history: &Value, id: &str) -> String {
    let q = &history["products"][id]["quantity"];
    if let Some(a) = q.as_array() {
        if a.len() >= 3 {
            let first = a.first().and_then(|v| v.as_i64()).unwrap_or(0);
            let last = a.last().and_then(|v| v.as_i64()).unwrap_or(0);
            if last < first {
                return format!(
                    " · quantity fell {} → {} over {} months",
                    fmt_num(&Value::from(first)),
                    fmt_num(&Value::from(last)),
                    a.len()
                );
            } else if last > first {
                return format!(
                    " · quantity rose {} → {} over {} months",
                    fmt_num(&Value::from(first)),
                    fmt_num(&Value::from(last)),
                    a.len()
                );
            }
        }
    }
    String::new()
}

// canon: philosophy.steel-sizing (24/min per caster line, heuristic),
// philosophy.coal-buffer (>=120 min import buffer, heuristic)
fn fix_hint(id: &str, kind: &str) -> String {
    match (id, kind) {
        ("Product_Steel", "stalled") => "Steel line is not running: check furnace charge (iron/coal), oxygen supply and caster state; size the line from the casters (24 steel/min per caster line).".into(),
        ("Product_Diesel", "deficit") => "Check the hydrocracker DieselReforming draw first (it can exceed vehicle fuel use); then distillation throughput vs crude supply.".into(),
        ("Product_Coal", _) => "If coal is import-fed, check ship/trade throughput against the burn rate and keep the import buffer at ≥120 min of draw at plant scale; if it is mined, check mine output and haulage.".into(),
        ("Product_Steel", _) => "Steel line is the constraint: check furnace charge (iron/coal), oxygen and caster state; size from the casters (24 steel/min per line).".into(),
        (_, "stalled") => "Producer is idle while demand continues: check its inputs, power, workers and recipe state.".into(),
        (_, "deficit") => "Raise production or cut the largest consumer (see products/sources for who draws it).".into(),
        _ => "Investigate using products/sources for the largest producer/consumer rows.".into(),
    }
}

fn cmd_trend(db_path: &str, filter: Option<String>) -> i32 {
    let filter = match filter {
        Some(f) => f,
        None => {
            eprintln!("usage: superintendent trend <product name filter>");
            return 2;
        }
    };
    if !std::path::Path::new(db_path).exists() {
        eprintln!("no database at {db_path} yet — take a snapshot first");
        return 1;
    }
    let st = match store::Store::open(db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let rows = match st.trend(&filter) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if rows.is_empty() {
        println!("no stored data matches '{filter}'");
        return 0;
    }
    let mut current = String::new();
    let mut prev_qty: Option<i64> = None;
    for (date, tick, qty, ptm, plm, ctm, clm, name) in &rows {
        if *name != current {
            current = name.clone();
            prev_qty = None;
            println!("\n{name}");
            println!(
                "{:<10} {:>9} {:>9} {:>7} {:>13} {:>13}",
                "Date", "Tick", "Quantity", "Δ qty", "Prod TM/LM", "Cons TM/LM"
            );
        }
        let delta = prev_qty.map(|p| qty - p);
        prev_qty = Some(*qty);
        println!(
            "{:<10} {:>9} {:>9} {:>7} {:>13} {:>13}",
            date,
            tick,
            qty,
            delta.map(|d| d.to_string()).unwrap_or_else(|| "-".into()),
            format!("{ptm}/{plm}"),
            format!("{ctm}/{clm}")
        );
    }
    0
}

fn cmd_db(db_path: &str, args: &[String]) -> i32 {
    match args.first().map(|s| s.as_str()) {
        Some("info") => {
            if !std::path::Path::new(db_path).exists() {
                println!("no database at {db_path} yet");
                return 0;
            }
            match store::Store::open(db_path) {
                Ok(st) => match st.counts() {
                    Ok((snaps, stats, issues)) => {
                        println!("db {db_path}: {snaps} snapshots, {stats} product rows, {issues} issues");
                        0
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        1
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        Some("import") => {
            let file = match args.get(1) {
                Some(f) => f.clone(),
                None => {
                    eprintln!("usage: superintendent db import <snapshot.json>");
                    return 2;
                }
            };
            let text = match std::fs::read_to_string(&file) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("import: cannot read {file}: {e}");
                    return 1;
                }
            };
            let snap: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("import: bad JSON in {file}: {e}");
                    return 1;
                }
            };
            if snap["schema"].as_str() != Some("coi-snapshot/1") {
                eprintln!("import: {file} is not a coi-snapshot/1 file");
                return 1;
            }
            match store::Store::open(db_path) {
                Ok(mut st) => {
                    match st.insert_snapshot(&snap, &[]) {
                        Ok(Some(id)) => {
                            println!("imported {file} as snapshot #{id} (issues not recomputed for imports)");
                            0
                        }
                        Ok(None) => {
                            println!("import: this game tick is already stored");
                            0
                        }
                        Err(e) => {
                            eprintln!("import failed: {e}");
                            1
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        _ => {
            eprintln!("usage: superintendent db <info|import <snapshot.json>>");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_toml_section_parsed() {
        let (url, host, proxy) = parse_game_toml(
            "# comment\n[productivity]\nurl = \"http://wrong:1\"\n[game]\nurl = \"http://pc-name:3073\"\nhost_header = localhost:3073\nproxy = \"env\" # trailing comment\nunknown = 3\n[builder]\nproxy = \"http://wrong:2\"\n",
        );
        assert_eq!(url.as_deref(), Some("http://pc-name:3073"));
        assert_eq!(host.as_deref(), Some("localhost:3073"));
        assert_eq!(proxy.as_deref(), Some("env"));
    }

    #[test]
    fn game_toml_absent_section_is_none() {
        let (url, host, proxy) = parse_game_toml("[productivity]\nfire_point = 0.4\n");
        assert!(url.is_none() && host.is_none() && proxy.is_none());
    }

    /// Per-OS dirs (daemon round): Linux honours XDG first, then
    /// the ~/.local/share resp. ~/.config fallbacks; an empty XDG
    /// value counts as unset; no home means no dir (the caller
    /// falls back to the working folder).
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn os_dirs_follow_xdg() {
        let home = std::path::Path::new("/home/tester");
        assert_eq!(
            os_data_dir_from(Some("/xdg/data"), Some(home)),
            Some(std::path::PathBuf::from("/xdg/data/superintendent"))
        );
        assert_eq!(
            os_data_dir_from(None, Some(home)),
            Some(std::path::PathBuf::from(
                "/home/tester/.local/share/superintendent"
            ))
        );
        assert_eq!(
            os_data_dir_from(Some(""), Some(home)),
            Some(std::path::PathBuf::from(
                "/home/tester/.local/share/superintendent"
            ))
        );
        assert_eq!(
            os_config_dir_from(Some("/xdg/cfg"), Some(home)),
            Some(std::path::PathBuf::from("/xdg/cfg/superintendent"))
        );
        assert_eq!(
            os_config_dir_from(None, Some(home)),
            Some(std::path::PathBuf::from(
                "/home/tester/.config/superintendent"
            ))
        );
        assert_eq!(os_data_dir_from(None, None), None);
        assert_eq!(os_config_dir_from(None, None), None);
    }

    /// macOS: both dirs are the one Application Support folder;
    /// XDG variables are not a Mac convention and are ignored.
    #[cfg(target_os = "macos")]
    #[test]
    fn os_dirs_follow_application_support() {
        let home = std::path::Path::new("/Users/tester");
        let want = Some(std::path::PathBuf::from(
            "/Users/tester/Library/Application Support/Superintendent",
        ));
        assert_eq!(os_data_dir_from(Some("/xdg/data"), Some(home)), want);
        assert_eq!(os_config_dir_from(None, Some(home)), want);
        assert_eq!(os_data_dir_from(None, None), None);
    }

    /// The config search keeps its established head (COI_CONFIG is
    /// exercised elsewhere; here: ./coi.toml before the db-adjacent
    /// file) and appends at most one OS-config-dir entry, last.
    #[test]
    fn config_candidates_keep_order_and_append_os_dir() {
        let c = config_candidates("some/dir/coi.db");
        assert_eq!(c[0], std::path::PathBuf::from("coi.toml"));
        assert_eq!(c[1], std::path::PathBuf::from("some/dir/coi.toml"));
        if let Some(dir) = os_config_dir() {
            assert_eq!(c.last(), Some(&dir.join("coi.toml")));
            assert_eq!(c.len(), 3);
        } else {
            assert_eq!(c.len(), 2);
        }
    }
}
