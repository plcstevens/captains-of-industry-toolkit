//! `superintendent serve` — the Superintendent **web application**.
//!
//! This is the product (SPEC, as corrected 2026-11-02): a functional
//! local website (no login), served by the single Rust binary running
//! on the game PC next to the game — no proxy/tunnel needed there, the
//! server talks to the game API on localhost. (The client config also
//! supports reaching the game through a generic TCP tunnel; see
//! REMOTE-ACCESS.md.) The CLI subcommands are a debug shell; this is
//! the interface.
//!
//! Features:
//! - No login (removed 2026-11-06, Philip's call): the site is
//!   loopback-only, single-user, on his machine. Protection is the
//!   loopback bind + a Host-header check (127.0.0.1/localhost on the
//!   configured port only, mirroring the game API) + per-form CSRF
//!   tokens. The store's `web_users` table is vestigial — nothing
//!   reads or writes it any more.
//! - Overview: live game status, pause/speed controls, triage summary,
//!   live screenshot.
//! - Issues: the Eisenhower triage board with evidence + fix guidance.
//! - Products: full table; per-product detail with producer/consumer
//!   sources, monthly history, and stored-snapshot trends.
//! - Actions: game commands through the SPEC §12 two-phase flow —
//!   prepare (exact JSON shown) → confirm → POST /command → audit log.
//!   The allowlist covers the no-entity commands (pause, resume,
//!   speed, research, pins, global priorities) plus, since CoiBridge
//!   went live (2026-11-02), entity-targeted commands addressed
//!   through the bridge's entity directory: machine pause/resume and
//!   recipe toggles, storage product assignment and logistics
//!   toggles, per-entity general priorities (SPEC §3/§12.7).
//! - Issues: every triage card links to a detail page — why this
//!   quadrant (the urgency/importance reasoning with the numbers),
//!   the evidence, the forecast, how to fix it, and buttons that
//!   prepare the fix for the specific storages/machines involved.
//! - Entities: the bridge directory — population/research status,
//!   per-storage state with logistics controls, machine instances
//!   from the topology route with pause/recipe/priority controls,
//!   and the not-fully-connected transports list.
//! - Snapshots: take one now (stores file + DB row), browse history.
//! - Log: the unified activity record (SPEC Addendum XVI) — every
//!   applied command and website snapshot from this companion, plus
//!   the QoS mods' recorded actions imported from their JSONL feed,
//!   in one chronological, filterable replay.
//!
//! Std-only HTTP (thread per connection). All state-changing POSTs
//! require the per-form CSRF token.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::catalogue::CATALOGUE;
use crate::client::Client;
use crate::guides::{guide_by_id, Guide, GUIDES};
use crate::store::{ActivityEvent, Store};
use crate::{
    compute_forecasts, compute_issues, fetch_live, flow_pair, fmt_num, get_json, render_report_md,
    snapshot_json, sv, Config, Forecast, Issue, Live,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The request context handed to page handlers. Since the login
/// concept was removed (2026-11-06) there is exactly one implicit
/// local user; the struct survives as plumbing: it carries the
/// process-lifetime CSRF token every form renders and every POST
/// checks, plus the actor name stamped into activity rows.
struct Session {
    user: String,
    csrf: String,
}

struct Prepared {
    cmd_type: String,
    args: Value,
    summary: String,
    created: u64,
}

/// A prepared blueprint-placement job (QoSBuilder, SPEC Addendum
/// XVII): the exact job-file bytes a confirm will write, plus the
/// game context captured at prepare for the activity record.
struct PreparedJob {
    job_id: String,
    file_json: String,
    summary: String,
    game_id: String,
    game_date: Option<String>,
    game_tick: Option<i64>,
    created: u64,
}

/// Server-side cache behind the top-bar must-fix badge and its
/// polling endpoint (DESIGN V2 addendum). Fed ONLY by page loads
/// that already fetched live game state — the endpoint itself never
/// triggers a game fetch, so N open tabs cannot stampede the API.
/// `count` is None until the first successful page fetch; when the
/// game goes unreachable the last count stays but `reachable` flips
/// false and the badge must render it as last-known, never as live.
struct BadgeCache {
    count: Option<u64>,
    reachable: bool,
    updated: u64,
}

struct State {
    cfg: Config,
    db_path: String,
    /// The single CSRF token for this process run (login removed
    /// 2026-11-06; the token still blocks naive cross-site posts).
    csrf: String,
    /// The port the server is bound to — the Host-header check
    /// accepts only 127.0.0.1/localhost on exactly this port.
    port: u16,
    badge: Mutex<BadgeCache>,
    prepared: Mutex<HashMap<String, Prepared>>,
    prepared_jobs: Mutex<HashMap<String, PreparedJob>>,
    /// Set by the /shutdown POST (daemon round, Addendum XXIV):
    /// the accept loop notices within a poll interval, stops
    /// accepting, drains in-flight requests, and exits 0.
    shutdown: AtomicBool,
    /// Requests currently being handled; the shutdown path waits
    /// for this to reach zero so an in-flight write (or its audit
    /// row) always finishes before the database is checkpointed.
    inflight: Arc<Inflight>,
}

/// In-flight request counter with a drained signal. `begin`
/// returns a guard; the count drops when the guard does — i.e.
/// after the response has been fully written to the socket.
struct Inflight {
    count: Mutex<usize>,
    drained: Condvar,
}

struct InflightGuard(Arc<Inflight>);

impl Inflight {
    fn new() -> Arc<Inflight> {
        Arc::new(Inflight {
            count: Mutex::new(0),
            drained: Condvar::new(),
        })
    }

    fn begin(self: &Arc<Inflight>) -> InflightGuard {
        let mut n = self.count.lock().unwrap_or_else(|p| p.into_inner());
        *n += 1;
        InflightGuard(Arc::clone(self))
    }

    /// Block until every in-flight request has finished.
    fn wait_drained(&self) {
        let mut n = self.count.lock().unwrap_or_else(|p| p.into_inner());
        while *n > 0 {
            n = self.drained.wait(n).unwrap_or_else(|p| p.into_inner());
        }
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let mut n = self.0.count.lock().unwrap_or_else(|p| p.into_inner());
        *n = n.saturating_sub(1);
        if *n == 0 {
            self.0.drained.notify_all();
        }
    }
}

/// Poison-tolerant lock for the session/prepared maps (audit R1):
/// if a request ever panicked while holding one of these mutexes,
/// plain `.lock().unwrap()` would panic in every later request and
/// wedge the whole website until restart. The maps hold per-entry
/// state (single-use tokens with expiry checks), so recovering the
/// guard from the poison error and serving on is safe.
fn lock_map<T>(m: &Mutex<HashMap<String, T>>) -> std::sync::MutexGuard<'_, HashMap<String, T>> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn rand_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("OS randomness unavailable");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// The implicit local session: no login, one user, the process
/// CSRF token. Activity rows keep an actor string for continuity
/// with the pre-removal log.
fn local_session(state: &State) -> Session {
    Session {
        user: "local".to_string(),
        csrf: state.csrf.clone(),
    }
}

/// Record a live fetch outcome into the badge cache. Called by the
/// pages that already fetch live state (Overview, Issues); the
/// badge endpoint only ever reads this.
fn badge_note(state: &State, live: Option<&Live>) {
    let mut b = state.badge.lock().unwrap_or_else(|p| p.into_inner());
    match live {
        Some(l) => {
            let n = compute_issues(l)
                .iter()
                .filter(|i| i.quadrant == "Fix now")
                .count() as u64;
            b.count = Some(n);
            b.reachable = true;
            b.updated = now_unix();
        }
        None => {
            b.reachable = false;
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP plumbing
// ---------------------------------------------------------------------------

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    form: HashMap<String, String>,
    /// The raw Host header ("" when absent) — checked against the
    /// loopback bind before routing (login-removal hardening).
    host: String,
}

fn pct_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_params(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in s.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = pct_decode(it.next().unwrap_or(""));
        let v = pct_decode(it.next().unwrap_or(""));
        map.insert(k, v);
    }
    map
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end;
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = find_seq(&buf, b"\r\n\r\n") {
                    header_end = pos + 4;
                    break;
                }
                if buf.len() > 128 * 1024 {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let raw_path = first.next()?.to_string();
    let mut content_length = 0usize;
    let mut host_header = String::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
        if lower.starts_with("host:") {
            host_header = line[5..].trim().to_string();
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    body.truncate(content_length);
    let (path, query) = match raw_path.split_once('?') {
        Some((p, q)) => (p.to_string(), parse_params(q)),
        None => (raw_path, HashMap::new()),
    };
    let form = if method == "POST" {
        parse_params(&String::from_utf8_lossy(&body))
    } else {
        HashMap::new()
    };
    Some(Request {
        method,
        path,
        query,
        form,
        host: host_header,
    })
}

fn find_seq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

struct Response {
    status: u16,
    content_type: String,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
}

impl Response {
    fn html(status: u16, body: String) -> Response {
        Response {
            status,
            content_type: "text/html; charset=utf-8".into(),
            body: body.into_bytes(),
            headers: Vec::new(),
        }
    }
    fn text(status: u16, body: &str) -> Response {
        Response {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            body: body.as_bytes().to_vec(),
            headers: Vec::new(),
        }
    }
    fn redirect(location: &str) -> Response {
        Response {
            status: 303,
            content_type: "text/plain".into(),
            body: Vec::new(),
            headers: vec![("Location".into(), location.into())],
        }
    }
}

fn send(stream: &mut TcpStream, r: Response) {
    let reason = match r.status {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        502 => "Bad Gateway",
        _ => "Status",
    };
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        r.status, reason, r.content_type,
        r.body.len()
    );
    for (k, v) in &r.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&r.body);
    let _ = stream.flush();
}

// ---------------------------------------------------------------------------
// Server entry
// ---------------------------------------------------------------------------

pub fn cmd(cfg: &Config, db_path: &str, args: &[String]) -> i32 {
    let mut port: u16 = 8484;
    let mut bind = "127.0.0.1".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse().ok()) {
                    Some(p) => port = p,
                    None => {
                        eprintln!("serve: --port needs a number");
                        return 2;
                    }
                }
            }
            "--bind" => {
                i += 1;
                match args.get(i) {
                    Some(b) => bind = b.clone(),
                    None => {
                        eprintln!("serve: --bind needs an address");
                        return 2;
                    }
                }
            }
            other => {
                eprintln!("serve: unexpected argument: {other}");
                return 2;
            }
        }
        i += 1;
    }
    let listener = match TcpListener::bind((bind.as_str(), port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("serve: cannot bind {bind}:{port}: {e}");
            return 1;
        }
    };
    println!("Superintendent web app at http://{bind}:{port}/  (Ctrl-C to stop)");
    println!("No login — loopback only, Host-checked, CSRF tokens on forms.");
    let state = Arc::new(State {
        cfg: Config {
            authority: cfg.authority.clone(),
            host_header: cfg.host_header.clone(),
            proxy: cfg.proxy.clone(),
        },
        db_path: db_path.to_string(),
        csrf: rand_hex(16),
        port,
        badge: Mutex::new(BadgeCache {
            count: None,
            reachable: false,
            updated: 0,
        }),
        prepared: Mutex::new(HashMap::new()),
        prepared_jobs: Mutex::new(HashMap::new()),
        shutdown: AtomicBool::new(false),
        inflight: Inflight::new(),
    });
    // Non-blocking accept with a short poll: the /shutdown POST
    // sets its flag from a handler thread, and this loop is what
    // notices — then stops accepting, drains, and exits cleanly.
    if let Err(e) = listener.set_nonblocking(true) {
        eprintln!("serve: cannot set non-blocking accept: {e}");
        return 1;
    }
    loop {
        if state.shutdown.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept() {
            Ok((mut s, _)) => {
                let state = Arc::clone(&state);
                std::thread::spawn(move || {
                    // The guard drops only after the response has
                    // been written — the shutdown drain measures
                    // real in-flight work, not accepted sockets.
                    let _guard = state.inflight.begin();
                    if let Some(req) = read_request(&mut s) {
                        let resp = route(&state, &req);
                        send(&mut s, resp);
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            Err(e) => {
                eprintln!("serve: accept failed: {e}");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    // Graceful exit (Addendum XXIV): no new requests are accepted
    // (the listener drops with this function); in-flight requests
    // finish — including any write and its audit row — then the
    // database is checkpointed so the WAL is folded back into the
    // main file before the process goes.
    println!("serve: shutdown requested — finishing in-flight requests…");
    state.inflight.wait_drained();
    match Store::open(&state.db_path) {
        Ok(st) => {
            if let Err(e) = st.checkpoint() {
                eprintln!("serve: database checkpoint failed: {e}");
            }
        }
        Err(e) => eprintln!("serve: cannot reopen database to checkpoint it: {e}"),
    }
    println!("serve: stopped cleanly.");
    0
}

fn client(state: &State) -> Client {
    crate::make_client(&state.cfg)
}

fn store(state: &State) -> Result<Store, String> {
    Store::open(&state.db_path)
}

/// Host-header check (added with the login removal, 2026-11-06):
/// with no session gate, the bind alone is not the whole story —
/// a browser on this machine can be driven to POST at any local
/// port by a hostile page (DNS rebinding / CSRF-by-form). Accept
/// only the loopback names on the configured port, exactly like
/// the game API does. The CSRF token remains as the second layer.
fn host_ok(state: &State, req: &Request) -> bool {
    let h = req.host.trim().to_ascii_lowercase();
    h == format!("127.0.0.1:{}", state.port) || h == format!("localhost:{}", state.port)
}

/// The must-fix badge endpoint: answers ONLY from the server-side
/// cache (fed by Overview/Issues page loads). It never fetches the
/// game itself. `count` is null until the first successful page
/// fetch; `reachable:false` means the number is last-known.
fn api_must_fix_count(state: &State) -> Response {
    let b = state.badge.lock().unwrap_or_else(|p| p.into_inner());
    let body = json!({
        "count": b.count,
        "reachable": b.reachable,
        "updatedUnix": if b.updated == 0 { Value::Null } else { json!(b.updated) },
    });
    Response {
        status: 200,
        content_type: "application/json".into(),
        body: body.to_string().into_bytes(),
        headers: Vec::new(),
    }
}

fn route(state: &State, req: &Request) -> Response {
    if !host_ok(state, req) {
        return Response::text(
            403,
            "forbidden: this companion only answers 127.0.0.1 / localhost on its own port",
        );
    }
    let path = req.path.as_str();
    let session = local_session(state);
    match (req.method.as_str(), path) {
        ("GET", "/") | ("GET", "/index.html") => page_overview(state, &session),
        ("GET", "/issues") => page_issues(state, &session, req),
        ("GET", "/issue") => page_issue(state, &session, req),
        ("GET", "/guides") => page_guides(state, &session),
        ("GET", "/guide") => page_guide(state, &session, req),
        ("GET", "/manual") => page_manual(state, &session),
        ("GET", "/why") => page_why(state, &session, req),
        ("GET", "/catalogue") => page_catalogue(state, &session, req),
        ("GET", "/products") => page_products(state, &session, req),
        ("GET", "/product") => page_product(state, &session, req),
        ("GET", "/entities") => page_entities(state, &session, req),
        ("GET", "/terrain") => page_terrain(state, &session, req),
        ("GET", "/trade") => page_trade(state, &session, req),
        ("GET", "/plans") => page_plans(state, &session, req),
        ("GET", "/place") => page_place(state, &session, req),
        ("POST", "/place/prepare") => do_place_prepare(state, &session, req),
        ("POST", "/place/confirm") => do_place_confirm(state, &session, req),
        ("GET", "/designate") => page_designate(state, &session),
        ("POST", "/designate/prepare") => do_designate_prepare(state, &session, req),
        ("POST", "/designate/confirm") => do_designate_confirm(state, &session, req),
        ("GET", "/research") => page_research(state, &session, req),
        ("GET", "/research/node") => page_research_node(state, &session, req),
        ("GET", "/api/v1/must-fix-count") => api_must_fix_count(state),
        ("GET", "/shutdown") => page_shutdown(state, &session),
        ("POST", "/shutdown") => do_shutdown(state, &session, req),
        ("GET", "/actions") => page_actions(state, &session),
        ("POST", "/actions/prepare") => do_prepare(state, &session, req),
        ("GET", "/prepare") => page_prepare(state, &session, req),
        ("POST", "/actions/prepare-generic") => do_prepare_generic(state, &session, req),
        ("POST", "/actions/prepare-priority") => do_prepare_priority(state, &session, req),
        ("POST", "/actions/prepare-storage-product") => {
            do_prepare_storage_product(state, &session, req)
        }
        ("POST", "/actions/prepare-entity-priority") => {
            do_prepare_entity_priority(state, &session, req)
        }
        ("POST", "/actions/apply") => do_apply(state, &session, req),
        ("GET", "/snapshots") => page_snapshots(state, &session, req),
        ("POST", "/snapshots/take") => do_take_snapshot(state, &session, req),
        ("GET", "/log") => page_log(state, &session, req),
        ("GET", "/api/snapshot.json") => match fetch_live(&client(state)) {
            Ok(live) => {
                let body = serde_json::to_string_pretty(&snapshot_json(&live)).unwrap();
                Response {
                    status: 200,
                    content_type: "application/json".into(),
                    body: body.into_bytes(),
                    headers: Vec::new(),
                }
            }
            Err(e) => Response::text(502, &format!("game API unreachable: {e}")),
        },
        ("GET", "/api/report.md") => match fetch_live(&client(state)) {
            Ok(live) => {
                let issues = compute_issues(&live);
                Response {
                    status: 200,
                    content_type: "text/markdown; charset=utf-8".into(),
                    body: render_report_md(&live, &issues).into_bytes(),
                    headers: Vec::new(),
                }
            }
            Err(e) => Response::text(502, &format!("game API unreachable: {e}")),
        },
        ("GET", "/screenshot.jpg") => match client(state).get("/screenshot?width=1920&height=1080")
        {
            Ok(r) if r.status == 200 => Response {
                status: 200,
                content_type: "image/jpeg".into(),
                body: r.body,
                headers: Vec::new(),
            },
            Ok(r) => Response::text(502, &format!("game screenshot returned HTTP {}", r.status)),
            Err(e) => Response::text(502, &e),
        },
        ("GET", "/favicon.ico") => Response::text(204, ""),
        _ => Response::text(404, "not found"),
    }
}

// ---------------------------------------------------------------------------
// Shutdown (daemon round, SPEC Addendum XXIV)
// ---------------------------------------------------------------------------

/// The confirm step of the top-bar Shutdown control. Two-step by
/// design: this page names exactly what stopping means BEFORE the
/// POST exists — after it, this site is gone and there is nothing
/// left to explain anything with.
fn page_shutdown(state: &State, session: &Session) -> Response {
    let mut body = page_head(
        "System",
        "Shut down",
        "Stop the Superintendent website on this machine.",
        "",
    );
    body.push_str(
        "<div class=\"card\"><div class=\"cardhead\"><h2>What happens when you shut down</h2></div>\
         <p><b>This website stops.</b> The program exits; this tab — and any other tab or browser \
         showing the site — will fail to load from then on. There is no page after this one but the \
         confirmation that it stopped.</p>\
         <p><b>Nothing is lost.</b> Anything already in flight finishes first, then the database is \
         closed cleanly. Snapshots, the audit log and your settings stay on disk, ready for the next launch.</p>\
         <p><b>Your game is not touched.</b> Shutting down the companion never changes, pauses or \
         stops the game itself — it only stops this website.</p>\
         <p><b>To use it again</b>, launch the program again the way you started it (double-click, \
         Start menu, or <span class=\"mono\">systemctl --user start superintendent</span> / \
         <span class=\"mono\">launchctl start com.coi.superintendent</span> if you run it as a service). \
         If you set it to start on boot, it also comes back at your next logon.</p></div>",
    );
    body.push_str(&format!(
        "<form method=\"post\" action=\"/shutdown\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <p><button class=\"btn--danger\" type=\"submit\">Shut down Superintendent</button> \
         <a class=\"btn btn--secondary\" href=\"/\">Cancel — keep it running</a></p></form>",
        esc(&session.csrf)
    ));
    Response::html(200, layout(state, session, "Shut down", &body))
}

/// The shutdown itself. CSRF-checked like every POST. Records a
/// system activity event (best-effort — a database problem must
/// never block stopping), sets the flag the accept loop polls,
/// and answers with the landing page: the last thing this server
/// will ever serve in this run.
fn do_shutdown(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    if let Ok(st) = store(state) {
        let _ = st.activity_add(&ActivityEvent {
            event_key: format!("shutdown-{}-{}", now_unix(), rand_hex(4)),
            recorded_ts_unix: now_unix() as i64,
            source: "system".to_string(),
            actor: Some(session.user.clone()),
            category: Some("website".to_string()),
            action: "shutdown".to_string(),
            summary: Some(
                "Website shut down from the top-bar Shutdown control; in-flight requests \
                 drained and the database checkpointed before exit."
                    .to_string(),
            ),
            game_id: None,
            game_date: None,
            game_tick: None,
            outcome: Some("stopped".to_string()),
            payload_json: None,
        });
    }
    state.shutdown.store(true, Ordering::SeqCst);
    Response::html(
        200,
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>Stopped — Superintendent</title><style>{CSS}</style></head><body>\
             <main style=\"max-width:640px;margin:12vh auto 0;padding:0 16px\">\
             <div class=\"card\"><div class=\"cardhead\"><h2>Superintendent has stopped</h2></div>\
             <p><b>You can close this tab.</b> The website is no longer running, so this address \
             will not load again until the program is started.</p>\
             <p>Nothing was lost: in-flight work finished and the database was closed cleanly. \
             Your game was not touched.</p>\
             <p>To use Superintendent again, launch the program again the way you started it — \
             or, if you run it as a service, restart that service.</p></div></main></body></html>"
        ),
    )
}

// ---------------------------------------------------------------------------
// Layout + auth pages
// ---------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CSS: &str = "
:root{
  --bg-0:#0b0e14; --bg-1:#10141d; --surface:#151a26; --surface-2:#1b2130; --surface-3:#232b3e;
  --border:#263048; --border-strong:#37466a; --border-subtle:#1a2132;
  --text-1:#e9edf6; --text-2:#a9b3c7; --text-3:#6d7990; --text-disabled:#49536a;
  --accent:#5b9dff; --accent-strong:#3b78e0; --accent-soft:rgba(91,157,255,.13); --on-accent:#fff;
  --ok:#4cc38a; --ok-soft:rgba(76,195,138,.13);
  --starved:#e8933c; --starved-soft:rgba(232,147,60,.14);
  --blocked:#e5534b; --blocked-soft:rgba(229,83,75,.13);
  --paused:#8494ab; --paused-soft:rgba(132,148,171,.14);
  --warning:#e5c33b; --warning-soft:rgba(229,195,59,.12);
  --critical:#f85149; --critical-soft:rgba(248,81,73,.12);
  --info:#5b9dff; --info-soft:rgba(91,157,255,.13);
  --unverified:#a371f7; --unverified-soft:rgba(163,113,247,.12);
  --chart-grid:#232c42; --chart-track:#232b3e;
  --font-ui:system-ui,\"Segoe UI\",Roboto,\"Helvetica Neue\",Arial,sans-serif;
  --font-mono:ui-monospace,\"Cascadia Mono\",Consolas,\"SF Mono\",Menlo,monospace;
  --r-sm:6px; --r-md:10px; --r-lg:14px;
}
*{box-sizing:border-box}
body{margin:0;background:var(--bg-0);color:var(--text-1);font:14px/1.5 var(--font-ui)}
a{color:var(--accent);text-decoration:none} a:hover{text-decoration:underline}
:focus-visible{outline:2px solid var(--accent);outline-offset:1px}
h1{font-size:22px;line-height:28px;margin:0 0 4px;font-weight:650}
h2{font-size:15px;line-height:20px;font-weight:650;margin:22px 0 8px}
h3{font-size:14px;line-height:20px;font-weight:600;margin:16px 0 6px}
p{margin:0 0 10px}
.mono,code{font-family:var(--font-mono);font-size:12.5px}
.num{font-variant-numeric:tabular-nums;text-align:right}
.u{color:var(--text-3);font-size:12px}
.sub,.caption{color:var(--text-2);font-size:12px}
.caption{margin-top:6px}
.lock{color:var(--text-3)}
/* ---------- shell ---------- */
.shell{display:grid;grid-template-columns:232px 1fr;min-height:100vh}
aside{background:var(--bg-1);border-right:1px solid var(--border-subtle);padding:16px 12px;display:flex;flex-direction:column;gap:16px}
.brand{font-weight:700;font-size:16px;padding:0 8px}
.brand .ver{color:var(--text-3);font-size:12px;font-weight:400;margin-top:2px}
.statusbox{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);padding:10px 12px}
.statusbox .date{font-size:15px;font-weight:650}
.statusbox .meta{color:var(--text-2);font-size:12px;margin-top:4px;display:flex;gap:6px;flex-wrap:wrap;align-items:center}
.statusbox--down{border-color:var(--critical)}
nav .group{color:var(--text-3);font-size:12px;letter-spacing:.02em;padding:10px 8px 4px}
.navitem{display:flex;align-items:center;gap:8px;padding:6px 8px;border-radius:var(--r-sm);color:var(--text-2);margin-bottom:2px}
.navitem:hover{background:var(--surface-2);text-decoration:none}
.navitem.active{background:var(--accent-soft);color:var(--text-1);font-weight:600;box-shadow:inset 2px 0 0 var(--accent)}
.sidefoot{margin-top:auto;border-top:1px solid var(--border-subtle);padding:10px 8px 0;color:var(--text-2);font-size:13px;display:flex;gap:8px;align-items:center}
.sidefoot form{margin-left:auto}
main{padding:24px;max-width:1280px;width:100%}
.eyebrow{color:var(--text-3);font-size:12px;letter-spacing:.02em}
.pagehead{display:flex;align-items:flex-end;gap:16px;margin-bottom:20px}
.pagehead h1{margin:2px 0 4px}
.pagehead .summary{color:var(--text-2)}
.pagehead .actions{margin-left:auto}
@media(max-width:900px){.shell{grid-template-columns:1fr}aside{flex-direction:row;flex-wrap:wrap;align-items:flex-start}nav{display:flex;overflow-x:auto;flex-wrap:nowrap}.sidefoot{margin:0;border:0}main{padding:16px}}
/* ---------- buttons ---------- */
button,.btn{background:var(--accent-strong);color:var(--on-accent);border:1px solid transparent;border-radius:var(--r-sm);padding:6px 14px;font-size:14px;font-family:inherit;cursor:pointer;display:inline-block;text-decoration:none;transition:background .12s}
button:hover,.btn:hover{background:#4a86ec;text-decoration:none}
.btn--secondary,button.ghost,button.secondary{background:var(--surface-2);border-color:var(--border-strong);color:var(--text-1)}
.btn--secondary:hover,button.ghost:hover,button.secondary:hover{background:var(--surface-3)}
.btn--danger,button.danger{background:var(--critical);color:#fff}
.btn--danger:hover,button.danger:hover{background:#ff6b64}
.btn--ghost{background:transparent;color:var(--text-2);border-color:transparent}
.btn--ghost:hover{background:var(--surface-2)}
.btn--sm,button.sm{padding:4px 10px;font-size:13px}
button:disabled,.btn.disabled{background:var(--surface-3);color:var(--text-disabled);cursor:not-allowed;border-color:transparent}
/* ---------- badges ---------- */
.badge{display:inline-block;padding:1px 8px;border-radius:999px;font-size:11.5px;font-weight:600;border:1px solid transparent;white-space:nowrap;background:var(--surface-2);color:var(--text-2)}
.badge--ok{background:var(--ok-soft);color:var(--ok)}
.badge--starved{background:var(--starved-soft);color:var(--starved)}
.badge--blocked{background:var(--blocked-soft);color:var(--blocked)}
.badge--paused{background:var(--paused-soft);color:var(--paused)}
.badge--warning{background:var(--warning-soft);color:var(--warning)}
.badge--critical{background:var(--critical-soft);color:var(--critical)}
.badge--info{background:var(--info-soft);color:var(--info)}
.badge--unverified{background:transparent;color:var(--unverified);border:1px dashed var(--unverified)}
.badge--unev{background:var(--paused-soft);color:var(--paused);border:1px dashed var(--paused)}
.badge--solid-paused{background:var(--paused);color:#0b0e14}
.badge--solid-ok{background:var(--ok);color:#0b0e14}
.badge--src-companion{background:var(--info-soft);color:var(--info)}
.badge--src-qos{background:var(--ok-soft);color:var(--ok)}
.badge--src-system{background:var(--paused-soft);color:var(--paused)}
.paused{color:var(--paused)}.running{color:var(--ok)}
/* ---------- cards / panels ---------- */
.card,.panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);padding:16px;margin-bottom:16px}
.panel h2:first-child,.card h2:first-child{margin-top:0}
.card .cardhead{display:flex;align-items:center;gap:8px;margin-bottom:10px}
.card .cardhead h2{margin:0}.card .cardhead .right{margin-left:auto}
.cols{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:14px;margin-bottom:16px}
.cols2{display:grid;grid-template-columns:1fr 1fr;gap:16px}
@media(max-width:900px){.cols2{grid-template-columns:1fr}}
.col{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);padding:12px 14px}
.col h2{margin-top:0}
.stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin-bottom:16px}
.stat{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);padding:12px 16px}
.stat .label{color:var(--text-2);font-size:12px;letter-spacing:.02em}
.stat .value{font-size:24px;line-height:28px;font-weight:650;font-variant-numeric:tabular-nums;margin:2px 0}
.stat .ctx{color:var(--text-2);font-size:12px}
.stat--critical{border-color:var(--critical)}
.issue{background:var(--surface-2);border:1px solid var(--border-subtle);border-radius:var(--r-md);padding:10px 12px;margin-bottom:8px}
.issue .p{font-weight:600}.issue .t{color:var(--text-1)} .issue .e{color:var(--text-2);font-size:13px}
.issue .f{color:var(--accent);font-size:13px}
.fn,.issue--fn{box-shadow:inset 2px 0 0 var(--critical)} .fl,.issue--fl{box-shadow:inset 2px 0 0 var(--warning)}
.wa,.issue--wa{box-shadow:inset 2px 0 0 var(--info)} .ig,.issue--ig{box-shadow:inset 2px 0 0 var(--paused)}
.quadhead{display:flex;gap:8px;align-items:center;margin:14px 0 8px}
.quadhead:first-child{margin-top:0}
.empty{border:1px dashed var(--border-strong);border-radius:var(--r-md);padding:16px;text-align:center;color:var(--text-2);font-size:12px;margin-bottom:16px}
/* ---------- callouts ---------- */
.callout{border:1px solid var(--border);border-radius:var(--r-md);padding:12px 16px;margin-bottom:16px;background:var(--surface)}
.callout--info{background:var(--info-soft);border-color:var(--accent)}
.callout--warn{background:var(--warning-soft);border-color:var(--warning)}
.callout--danger,.err{background:var(--critical-soft);border:1px solid var(--critical);border-radius:var(--r-md);padding:12px 16px;margin-bottom:16px}
.callout--ok,.ok{background:var(--ok-soft);border:1px solid var(--ok);border-radius:var(--r-md);padding:12px 16px;margin-bottom:16px}
.callout details,.err details{margin-top:8px}
/* ---------- tables ---------- */
table{border-collapse:collapse;width:100%;margin-bottom:16px}
th,td{padding:6px 8px;border-bottom:1px solid var(--border-subtle);vertical-align:middle;text-align:left;font-size:13px}
th{background:var(--surface-2);color:var(--text-2);font-size:12px;font-weight:600}
tbody tr{transition:background .12s}
tbody tr:hover{background:var(--surface-2)}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums}
td.mono,th.mono{font-family:var(--font-mono);font-size:12.5px}
tr.row--critical{box-shadow:inset 2px 0 0 var(--critical)}
tr.row--blocked{box-shadow:inset 2px 0 0 var(--blocked)}
tr.row--starved{box-shadow:inset 2px 0 0 var(--starved)}
.tablecard{padding:0;overflow:hidden}
.tablecard table{margin-bottom:0}
.tablecard table th:first-child,.tablecard table td:first-child{padding-left:16px}
.tablecard table th:last-child,.tablecard table td:last-child{padding-right:16px}
.tablecard .cardhead{padding:16px 16px 0}
.filterbar{display:flex;gap:10px;align-items:center;padding:12px 16px;border-bottom:1px solid var(--border-subtle);flex-wrap:wrap}
.filterbar input[type=text],.filterbar input[type=search]{flex:1;min-width:160px}
.filterbar .count{margin-left:auto;color:var(--text-2);font-size:12px}
/* ---------- forms ---------- */
input[type=text],input[type=password],input[type=search],input[type=number],select,textarea{background:var(--surface-3);border:1px solid var(--border-strong);border-radius:var(--r-sm);color:var(--text-1);padding:8px 10px;font-size:14px;font-family:inherit}
input:focus,select:focus,textarea:focus{outline:2px solid var(--accent);outline-offset:1px}
label{font-size:13px}
.field{margin-bottom:12px;max-width:420px}
.field label{display:block;font-weight:600;font-size:13px;margin-bottom:4px}
.field input,.field select,.field textarea{width:100%}
.field .hint{color:var(--text-3);font-size:12px;margin-top:4px}
.field .ferr{color:var(--critical);font-size:12px;margin-top:4px}
/* ---------- misc components ---------- */
.fillbar{height:6px;background:var(--chart-track);border-radius:3px;overflow:hidden;display:inline-block;vertical-align:middle}
.fillbar>i{display:block;height:100%}
.stackbar{display:flex;height:14px;border-radius:4px;overflow:hidden;background:var(--chart-track)}
.stackbar>i{display:block;height:100%}
.kv{display:flex;gap:8px;flex-wrap:wrap;color:var(--text-2);font-size:13px}
.kvrow{display:flex;gap:12px;padding:4px 0;border-bottom:1px solid var(--border-subtle);font-size:13px}
.kvrow .k{color:var(--text-3);font-size:12px;min-width:150px}
pre{background:var(--surface-3);border-radius:var(--r-sm);padding:12px;overflow:auto;font-family:var(--font-mono);font-size:12.5px}
details.payload summary{cursor:pointer;color:var(--text-2);font-size:12px}
img.shot{max-width:100%;border-radius:var(--r-md);margin-top:0}
.shotwrap{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);padding:16px;margin-bottom:16px}
.seg{display:inline-flex;background:var(--surface-3);border-radius:999px;padding:2px;gap:2px}
.seg a,.seg span{padding:3px 12px;border-radius:999px;color:var(--text-2);font-size:13px}
.seg a:hover{text-decoration:none;background:var(--surface-2)}
.seg .on{background:var(--surface);color:var(--text-1);font-weight:600}
.pager{display:flex;gap:10px;align-items:center;margin-top:14px}
.phase{border:1px solid var(--border-subtle);border-radius:var(--r-md);padding:10px 12px;margin-bottom:8px;background:var(--surface-2)}
.phase .ev{color:var(--text-3);font-family:var(--font-mono);font-size:12px;margin-top:4px}
.feed-day{color:var(--text-3);font-size:12px;letter-spacing:.02em;margin:18px 0 6px}
.spark{color:var(--text-2);font-size:13px}
/* ---------- confirm sheet (SS12 safety surface) ---------- */
.confirm{max-width:640px;margin:24px auto;background:var(--surface);border:1px solid var(--border-strong);border-radius:var(--r-lg);box-shadow:0 8px 28px rgba(0,0,0,.45);overflow:hidden}
.confirm .band{padding:12px 20px;font-weight:650;background:var(--warning-soft);color:var(--warning);border-bottom:1px solid var(--warning)}
.confirm .band--danger{background:var(--critical-soft);color:var(--critical);border-bottom-color:var(--critical)}
.confirm .inner{padding:20px}
.confirm .summary{font-size:17px;line-height:24px;margin-bottom:8px}
.confirm .meta{color:var(--text-2);font-size:13px;margin-bottom:14px}
.confirm .actions{display:flex;gap:12px;margin-top:16px}
.confirm .actions .btn,.confirm .actions button{flex:0 0 auto;padding:8px 20px}
/* ---------- login ---------- */
.login-wrap{max-width:420px;margin:60px auto;padding:0 16px}
.login-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-lg);padding:24px}
.login-card .brand{font-size:18px;padding:0;margin-bottom:12px}
/* detail layout (SS2.4): main column + right rail */
.detail{display:grid;grid-template-columns:minmax(0,1fr) 320px;gap:16px;align-items:start}
@media(max-width:900px){.detail{grid-template-columns:1fr}}
/* =====================================================================
   DESIGN V2 (addendum V2.1–V2.9, implemented 2026-11-06). Everything
   below layers over the v1 rules above: token lifts win by order,
   density overrides restate the v1 selectors, new components are
   new classes. The SS12 confirm-sheet rules above are untouched.
   ===================================================================== */
:root{
  --text-3:#8494ab; --text-disabled:#5b6a84;
  --pos:var(--ok); --neg:var(--critical); --flat:var(--text-2);
}
.ic{width:16px;height:16px;flex:none;vertical-align:-3px}
/* density (V2.2) */
.card,.panel{padding:12px;margin-bottom:16px}
.card .cardhead{margin-bottom:8px}
main{padding:20px 24px}
/* contrast (V2.6): mono data is data, not a hint */
.mono{color:var(--text-2)}
.confirm pre{color:var(--text-1)}
details.payload pre{background:var(--surface-2);color:var(--text-2)}
.feed-day{color:var(--text-1);font-size:13px;font-weight:600}
.feedrow{display:flex;gap:10px;align-items:flex-start;padding:8px 12px;border-bottom:1px solid var(--border-subtle)}
.empty{color:var(--text-2)}
.phase .ev{color:var(--text-2)}
/* ---------- top bar / mega nav (V2.1) ---------- */
.topbar{position:sticky;top:0;z-index:60;display:flex;align-items:center;gap:14px;min-height:48px;padding:0 16px;background:var(--bg-1);border-bottom:1px solid var(--border);flex-wrap:wrap}
.topbar .brand{font-weight:700;font-size:15px;white-space:nowrap;padding:0}
.topbar .brand .brandic{width:19px;height:19px;vertical-align:-4px;color:var(--accent)}
.meganav{display:flex;align-items:stretch;min-height:48px}
.mega{position:relative;display:flex}
.mega>a{display:flex;align-items:center;gap:5px;padding:0 12px;color:var(--text-2);font-weight:600}
.mega>a:hover,.mega:hover>a{color:var(--text-1);text-decoration:none;background:var(--surface-2)}
.mega.active>a{color:var(--text-1);box-shadow:inset 0 -2px 0 var(--accent)}
.mega .caret{font-size:10px;color:var(--text-3)}
.mega-panel{display:none;position:absolute;left:0;top:100%;width:560px;max-width:92vw;background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);box-shadow:0 8px 28px rgba(0,0,0,.45);padding:8px;z-index:70}
.mega:hover .mega-panel,.mega:focus-within .mega-panel{display:block}
.mega-panel .cols{display:grid;grid-template-columns:1fr 1fr;gap:2px 8px}
.mega-row{display:flex;gap:10px;padding:8px;border-radius:var(--r-sm);color:var(--text-1);align-items:flex-start}
.mega-row:hover{background:var(--surface-2);text-decoration:none}
.mega-row .ic{color:var(--text-2);margin-top:2px}
.mega-row .txt{display:flex;flex-direction:column}
.mega-row .nm{font-weight:600;font-size:13.5px}
.mega-row .ds{color:var(--text-2);font-size:12px;line-height:1.35}
.mega-foot{border-top:1px solid var(--border-subtle);margin-top:6px;padding:7px 8px 2px;color:var(--text-3);font-size:12px}
.topsearch{margin-left:auto;display:flex;align-items:center;gap:6px;background:var(--surface-3);border:1px solid var(--border-strong);border-radius:var(--r-sm);padding:5px 9px;color:var(--text-3)}
.topsearch input{background:none;border:0;color:var(--text-1);font:13px var(--font-ui);width:170px;outline:none;margin:0;padding:0}
.topstatus{display:flex;align-items:center;gap:8px;white-space:nowrap;color:var(--text-2);font-size:13px}
.topshutdown{display:inline-flex;align-items:center;gap:6px;white-space:nowrap}
.topstatus .date{color:var(--text-1);font-weight:650}
.topstatus .down{color:var(--critical);font-weight:600}
/* must-fix badge (V2 addition, 2026-11-06) */
.mustfix{display:inline-flex;align-items:center;gap:6px;padding:4px 11px;border-radius:999px;font-size:12.5px;font-weight:650;border:1px solid var(--border-subtle);white-space:nowrap}
.mustfix:hover{text-decoration:none}
.mustfix--alert{background:var(--critical-soft);color:var(--critical);border-color:var(--critical)}
.mustfix--ok{background:var(--ok-soft);color:var(--ok)}
.mustfix--off{background:var(--surface-3);color:var(--text-2)}
/* ---------- rail (V2.1) ---------- */
.shell{display:grid;grid-template-columns:232px 1fr;align-items:start}
aside.rail{background:var(--bg-1);border-right:1px solid var(--border-subtle);padding:14px 12px;position:sticky;top:48px;height:calc(100vh - 48px);overflow:auto}
.rail .sec{color:var(--text-3);font-size:12px;letter-spacing:.02em;padding:2px 8px 6px}
.rail .ctx{color:var(--text-3);font-size:12px;letter-spacing:.02em;padding:14px 8px 6px;border-top:1px solid var(--border-subtle);margin-top:12px}
.rail .navitem{display:flex;align-items:center;gap:8px;padding:6px 8px;border-radius:var(--r-sm);color:var(--text-2);margin-bottom:2px;font-size:13.5px}
.rail .navitem:hover{background:var(--surface-2);text-decoration:none}
.rail .navitem.active{background:var(--accent-soft);color:var(--text-1);font-weight:600;box-shadow:inset 2px 0 0 var(--accent)}
.rail .navitem .count{margin-left:auto}
.rail .navitem.anchor{color:var(--text-3);font-size:13px}
.rail .navitem.anchor:hover{color:var(--text-1)}
.railfoot{color:var(--text-3);font-size:12px;padding:12px 8px 0;border-top:1px solid var(--border-subtle);margin-top:12px}
.crumb{color:var(--text-3);font-size:12px;letter-spacing:.02em}
.pagehead{display:flex;align-items:flex-end;gap:16px;margin-bottom:16px;flex-wrap:wrap}
.pagehead h1{font-size:20px;line-height:26px;margin:2px 0 2px;font-weight:650}
.pagehead .summary{color:var(--text-2);font-size:13.5px}
.pagehead .actions{margin-left:auto;display:flex;gap:8px}
/* ---------- stat blocks + figures (V2.3) ---------- */
.stats2{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:12px;margin-bottom:16px}
.stat2{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);padding:10px 12px}
.stat2 .top{display:flex;align-items:center;gap:8px}
.stat2 .ictile{width:28px;height:28px;border-radius:var(--r-sm);background:var(--surface-2);display:flex;align-items:center;justify-content:center;color:var(--text-2)}
.stat2 .label{color:var(--text-2);font-size:12px;letter-spacing:.02em}
.stat2 .value{font-size:22px;line-height:26px;font-weight:650;font-variant-numeric:tabular-nums;margin:4px 0 2px}
.stat2 .value .u{font-size:13px}
.stat2 .ctx{color:var(--text-2);font-size:12px;display:flex;gap:6px;align-items:center;flex-wrap:wrap}
.stat2--crit{border-color:var(--critical)}
.stat2--crit .ictile{color:var(--critical);background:var(--critical-soft)}
.figstrip{display:flex;flex-wrap:wrap;border:1px solid var(--border-subtle);border-radius:var(--r-sm);background:var(--surface-3);overflow:hidden}
.fig{padding:6px 12px;border-left:1px solid var(--border-subtle);min-width:92px}
.fig:first-child{border-left:0}
.fig .figl{font-size:11px;color:var(--text-3);letter-spacing:.02em}
.fig .fv{font-size:15px;line-height:20px;font-weight:600;font-variant-numeric:tabular-nums}
.fig .figv.warn{color:var(--warning)}
.fig .fv .u{font-size:11px}
/* number semantics (V2.5): a delta is a signed difference, arrowed */
.delta{display:inline-flex;align-items:center;gap:4px;padding:1px 8px;border-radius:999px;font-size:12px;font-weight:600;font-variant-numeric:tabular-nums;white-space:nowrap}
.delta .ic{width:12px;height:12px}
.delta--pos{background:var(--ok-soft);color:var(--pos)}
.delta--neg{background:var(--critical-soft);color:var(--neg)}
.delta--flat{background:var(--surface-2);color:var(--flat);border:1px solid var(--border-subtle)}
.fillbar{display:inline-block;height:6px;background:var(--chart-track);border-radius:3px;overflow:hidden;vertical-align:middle}
.fillbar>i{display:block;height:100%}
.runway{position:relative;height:8px;background:var(--chart-track);border-radius:4px;min-width:140px;display:inline-block;vertical-align:middle}
.runway>i{display:block;height:100%;border-radius:4px}
.runway .tick{position:absolute;left:25%;top:-2px;width:1px;height:12px;background:var(--text-3)}
/* ---------- issue cards (V2.7) ---------- */
.issue2{background:var(--surface-2);border:1px solid var(--border-subtle);border-radius:var(--r-md);padding:10px 12px;margin-bottom:8px}
.issue2--fn{box-shadow:inset 2px 0 0 var(--critical)} .issue2--fl{box-shadow:inset 2px 0 0 var(--warning)}
.issue2--wa{box-shadow:inset 2px 0 0 var(--info)} .issue2--ig{box-shadow:inset 2px 0 0 var(--paused)}
.issue2 .r1{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.issue2 .r1 .p{font-weight:600}
.issue2 .r1 .qb{margin-left:auto}
.issue2 .state-ic{color:var(--text-2);display:flex}
.issue2 .figstrip{margin-top:8px}
.issue2 .guide{color:var(--text-2);font-size:13px;margin-top:7px}
.issue2 .ctas{display:flex;gap:6px;align-items:center;margin-top:8px;flex-wrap:wrap}
.issue2 .evcap{color:var(--text-2);font-size:12px;margin-top:6px}
.issue2 .noact{display:inline-flex;align-items:center;gap:6px;color:var(--text-2);font-size:13px}
.quadhead{display:flex;gap:8px;align-items:center;margin:12px 0 8px;flex-wrap:wrap}
.quadhead:first-child{margin-top:0}
.quadhead .more{margin-left:auto;font-size:13px}
details.quadwhy{display:inline-block}
details.quadwhy summary{cursor:pointer;color:var(--text-3);font-size:12px;list-style:none}
details.quadwhy p{color:var(--text-2);font-size:12.5px;max-width:640px;margin:6px 0 0}
/* ---------- tables (V2.2) ---------- */
.table--compact td{padding:5px 10px;font-size:13px;line-height:18px}
.table--compact th{padding:5px 10px}
.tablecard thead th{position:sticky;top:48px;z-index:5}
.letterhead{position:sticky;top:48px;background:var(--bg-0);padding:10px 2px 4px;font-weight:650;color:var(--text-1);z-index:4}
/* ---------- forms (V2.2) ---------- */
.form-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;align-items:end}
.form-grid .field{max-width:none;margin:0}
.howit{color:var(--text-2);font-size:13px}
.howit summary{cursor:pointer;color:var(--text-3);font-size:12.5px}
.howit p{margin:6px 0 0;max-width:720px}
/* canon strip (DESIGN V3 §3): the rule that fired, quoted on issue detail */
.canonstrip{border-top:1px solid var(--border-subtle);margin-top:12px;padding-top:10px}
.canonstrip .cl{display:block;color:var(--text-3);font-size:10.5px;letter-spacing:.06em;text-transform:uppercase;margin-bottom:2px}
.canonstrip p{margin:2px 0}
/* page contract (DESIGN V3 §2): domain / balance / data, one strip */
.contract{display:flex;gap:0;flex-wrap:wrap;border:1px solid var(--border-subtle);border-radius:var(--r-md);margin:-6px 0 16px;overflow:hidden}
.contract>div{flex:1 1 220px;padding:7px 12px;border-left:1px solid var(--border-subtle)}
.contract>div:first-child{border-left:none}
.contract .cl{display:block;color:var(--text-3);font-size:10.5px;letter-spacing:.06em;text-transform:uppercase;margin-bottom:1px}
.contract .cv{color:var(--text-2);font-size:12.5px}
/* ---------- misc v2 components ---------- */
.ovgrid{display:grid;grid-template-columns:2fr 1fr;gap:16px;align-items:start}
.ovrail{display:flex;flex-direction:column;position:sticky;top:64px}
.minirow{display:flex;align-items:center;gap:8px;font-size:13px;padding:3px 0}
.minirow .bar{flex:1}
.minibar{height:6px;background:var(--chart-track);border-radius:3px;overflow:hidden;display:block}
.minibar>i{display:block;height:100%}
.legend{display:flex;gap:12px;flex-wrap:wrap;font-size:12px;color:var(--text-2);margin-top:6px}
.legend i{width:9px;height:9px;border-radius:2px;display:inline-block;margin-right:4px;font-style:normal}
.chips{display:flex;flex-wrap:wrap;gap:6px}
.chip{background:var(--surface-2);border:1px solid var(--border-subtle);border-radius:999px;padding:2px 10px;font-size:12.5px;color:var(--text-2)}
details.indgroup{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-md);margin-bottom:8px;padding:0 12px}
details.indgroup summary{cursor:pointer;padding:10px 0;font-weight:600;display:flex;gap:8px;align-items:center}
details.indgroup summary .count{margin-left:auto;color:var(--text-3);font-weight:400;font-size:12.5px}
.logfigs{margin-left:auto;display:flex}
@media(max-width:900px){
  .shell{grid-template-columns:1fr}
  aside.rail{position:static;height:auto;display:flex;flex-wrap:wrap;gap:2px;align-items:center}
  .rail .sec,.rail .ctx{width:100%}
  .ovgrid{grid-template-columns:1fr}
  .ovrail{position:static}
  .mega-panel{display:none!important}
  main{padding:16px}
}
";

/// The icon sprite (DESIGN V2.4): the 20 glyphs of
/// dev/design-mock/icons-v2.svg as <symbol> defs, pasted once into the
/// shell. Stroke glyphs on a 24×24 grid; i-play/i-pause are filled.
/// Colour always comes from `color:` at the use site.
const SPRITE: &str = "<svg width=\"0\" height=\"0\" style=\"position:absolute\" aria-hidden=\"true\"><defs>\
<symbol id=\"i-power\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><polygon points=\"13 2 3 14 12 14 11 22 21 10 12 10 13 2\"/></g></symbol>\
<symbol id=\"i-users\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2\"/><circle cx=\"9\" cy=\"7\" r=\"4\"/><path d=\"M23 21v-2a4 4 0 0 0-3-3.87\"/><path d=\"M16 3.13a4 4 0 0 1 0 7.75\"/></g></symbol>\
<symbol id=\"i-wheat\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 10c-3.5 0-5.5-2-5.5-5.5C10 4.5 12 6.5 12 10Z\"/><path d=\"M12 10c3.5 0 5.5-2 5.5-5.5C14 4.5 12 6.5 12 10Z\"/><path d=\"M12 16.5c-2.8 0-4.6-1.4-5-4 2.8 0 4.6 1.4 5 4Z\"/><path d=\"M12 16.5c2.8 0 4.6-1.4 5-4-2.8 0-4.6 1.4-5 4Z\"/><path d=\"M12 10v12\"/></g></symbol>\
<symbol id=\"i-flask\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M9 3h6\"/><path d=\"M10 3v5.5L4.8 18.5A2 2 0 0 0 6.6 21.5h10.8a2 2 0 0 0 1.8-3L14 8.5V3\"/><path d=\"M7.8 15h8.4\"/></g></symbol>\
<symbol id=\"i-box\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z\"/><polyline points=\"3.27 6.96 12 12.01 20.73 6.96\"/><line x1=\"12\" y1=\"22.08\" x2=\"12\" y2=\"12\"/></g></symbol>\
<symbol id=\"i-factory\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M3 21V11l5 3.5V11l5 3.5V7h8v14\"/><path d=\"M3 21h18\"/><path d=\"M16.5 11h.01M16.5 15h.01\"/></g></symbol>\
<symbol id=\"i-alert\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z\"/><line x1=\"12\" y1=\"9\" x2=\"12\" y2=\"13\"/><line x1=\"12\" y1=\"17\" x2=\"12.01\" y2=\"17\"/></g></symbol>\
<symbol id=\"i-check\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><polyline points=\"20 6 9 17 4 12\"/></g></symbol>\
<symbol id=\"i-clock\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><polyline points=\"12 7 12 12 15.5 14\"/></g></symbol>\
<symbol id=\"i-truck\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><rect x=\"1\" y=\"3\" width=\"15\" height=\"13\" rx=\"1\"/><polygon points=\"16 8 20 8 23 11 23 16 16 16 16 8\"/><circle cx=\"5.5\" cy=\"18.5\" r=\"2.5\"/><circle cx=\"18.5\" cy=\"18.5\" r=\"2.5\"/></g></symbol>\
<symbol id=\"i-anchor\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"5\" r=\"3\"/><line x1=\"12\" y1=\"22\" x2=\"12\" y2=\"8\"/><path d=\"M5 12H2a10 10 0 0 0 20 0h-3\"/></g></symbol>\
<symbol id=\"i-train\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><rect x=\"5\" y=\"3\" width=\"14\" height=\"13\" rx=\"2\"/><path d=\"M5 9.5h14\"/><path d=\"M9 13h.01M15 13h.01\"/><path d=\"M9 21l1.8-5M15 21l-1.8-5\"/></g></symbol>\
<symbol id=\"i-chart\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"12\" y1=\"20\" x2=\"12\" y2=\"10\"/><line x1=\"18\" y1=\"20\" x2=\"18\" y2=\"4\"/><line x1=\"6\" y1=\"20\" x2=\"6\" y2=\"16\"/></g></symbol>\
<symbol id=\"i-up\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"12\" y1=\"19\" x2=\"12\" y2=\"5\"/><polyline points=\"5 12 12 5 19 12\"/></g></symbol>\
<symbol id=\"i-down\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"12\" y1=\"5\" x2=\"12\" y2=\"19\"/><polyline points=\"19 12 12 19 5 12\"/></g></symbol>\
<symbol id=\"i-flat\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"4\" y1=\"12\" x2=\"20\" y2=\"12\"/><polyline points=\"8 8 4 12 8 16\"/><polyline points=\"16 8 20 12 16 16\"/></g></symbol>\
<symbol id=\"i-play\" viewBox=\"0 0 24 24\"><polygon points=\"6 3 20 12 6 21 6 3\" fill=\"currentColor\"/></symbol>\
<symbol id=\"i-pause\" viewBox=\"0 0 24 24\"><g fill=\"currentColor\"><rect x=\"6\" y=\"4\" width=\"4\" height=\"16\" rx=\"1\"/><rect x=\"14\" y=\"4\" width=\"4\" height=\"16\" rx=\"1\"/></g></symbol>\
<symbol id=\"i-layers\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><polygon points=\"12 2 2 7 12 12 22 7 12 2\"/><polyline points=\"2 17 12 22 22 17\"/><polyline points=\"2 12 12 17 22 12\"/></g></symbol>\
<symbol id=\"i-search\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"11\" cy=\"11\" r=\"7\"/><line x1=\"21\" y1=\"21\" x2=\"16.5\" y2=\"16.5\"/></g></symbol>\
<symbol id=\"i-clipboard\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2\"/><rect x=\"8\" y=\"2\" width=\"8\" height=\"4\" rx=\"1\"/><path d=\"M9 12h6\"/><path d=\"M9 16h4\"/></g></symbol>\
<symbol id=\"i-power\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M18.36 6.64a9 9 0 1 1-12.73 0\"/><line x1=\"12\" y1=\"2\" x2=\"12\" y2=\"12\"/></g></symbol>\
</defs></svg>";

/// An icon use-site. `name` is one of the sprite ids without the
/// `i-` prefix. Icons never appear without an adjacent text label.
fn icon(name: &str) -> String {
    format!("<svg class=\"ic\"><use href=\"#i-{name}\"/></svg>")
}

/// The flattened one-line game status for the top bar (DESIGN
/// V2.1): date · PAUSED/RUNNING solid badge · speed · tick · pop.
/// When the game does not answer, the strip flips to its "Game
/// unreachable" state instead of vanishing, and the page body
/// carries the D1 callout.
fn status_strip(state: &State) -> String {
    match get_json(&client(state), "/api/v1/game-info") {
        Ok(g) => {
            let paused = g["isPaused"].as_bool().unwrap_or(false);
            let pop = bridge_json(state, "/coi/v1/status")
                .map(|s| {
                    format!(
                        "<span>{} {}</span>",
                        icon("users"),
                        fmt_num(&s["population"])
                    )
                })
                .unwrap_or_default();
            format!(
                "<span class=\"date\">{}</span>\
                 <span class=\"badge {}\">{}</span>\
                 <span>speed {}×</span><span class=\"mono\">tick {}</span>{}",
                esc(&sv(&g["date"])),
                if paused {
                    "badge--solid-paused"
                } else {
                    "badge--solid-ok"
                },
                if paused { "PAUSED" } else { "RUNNING" },
                g["speed"],
                g["tick"],
                pop,
            )
        }
        Err(_) => "<span class=\"down\">Game unreachable</span>\
             <span class=\"badge badge--critical\">OFFLINE</span>"
            .to_string(),
    }
}

/// The top-bar must-fix badge (V2 addition): server-rendered from
/// the badge cache, then kept fresh by BADGE_JS polling
/// /api/v1/must-fix-count every 60 s. Colour is never alone — the
/// words "must fix" are always in the label, and a last-known
/// number is labelled as such, never dressed as live.
fn mustfix_badge(state: &State) -> String {
    let b = state.badge.lock().unwrap_or_else(|p| p.into_inner());
    let (cls, ic, label) = match (b.count, b.reachable) {
        (Some(n), true) if n > 0 => ("mustfix--alert", "alert", format!("{n} must fix")),
        (Some(_), true) => ("mustfix--ok", "check", "0 must fix".to_string()),
        (Some(n), false) => (
            "mustfix--off",
            "alert",
            format!("{n} must fix · last known"),
        ),
        (None, _) => (
            "mustfix--off",
            "alert",
            "must fix · no data yet".to_string(),
        ),
    };
    format!(
        "<a id=\"mustfix\" class=\"mustfix {cls}\" href=\"/issues?only=fix-now\">{}<span>{}</span></a>",
        icon(ic),
        esc(&label)
    )
}

const BADGE_JS: &str = "<script>
(function(){var b=document.getElementById('mustfix');if(!b)return;
function ic(n){return '<svg class=\"ic\"><use href=\"#i-'+n+'\"/></svg>'}
function render(d){var n=d.count;
if(n===null||n===undefined){b.className='mustfix mustfix--off';b.innerHTML=ic('alert')+'<span>must fix · no data yet</span>';return}
if(!d.reachable){b.className='mustfix mustfix--off';b.innerHTML=ic('alert')+'<span>'+n+' must fix · last known</span>';return}
if(n>0){b.className='mustfix mustfix--alert';b.innerHTML=ic('alert')+'<span>'+n+' must fix</span>'}
else{b.className='mustfix mustfix--ok';b.innerHTML=ic('check')+'<span>0 must fix</span>'}}
function poll(){fetch('/api/v1/must-fix-count').then(function(r){return r.json()}).then(render).catch(function(){})}
setInterval(poll,60000)})();
</script>";

/// Which nav item a page title belongs to (DESIGN §2.2). Detail and
/// result pages map back to their section; unknown titles get none.
fn nav_active(title: &str) -> &'static str {
    match title {
        "Overview" => "Overview",
        "Issues" | "Issue detail" | "Issue gone" => "Issues",
        "Learn the game" => "Learn the game",
        "Learn the tool" => "Learn the tool",
        "Why it suggests" => "Why it suggests",
        "Products" => "Products",
        "Entities" => "Entities",
        "Terrain" => "Terrain",
        "Research" => "Research",
        "Trade" => "Trade",
        "Plans" => "Plans",
        "Place a blueprint"
        | "Confirm placement"
        | "Job written"
        | "Job write failed"
        | "Confirmation expired"
        | "Unknown blueprint"
        | "Bad anchor"
        | "Bad placement"
        | "Prepare refused — game unreachable"
        | "Prepare refused — no game id"
        | "Game unreachable"
        | "No game id" => "Place",
        "Designate"
        | "Confirm designation"
        | "Designation job written"
        | "Designation job write failed"
        | "Designation confirmation expired"
        | "Bad pad"
        | "Plan refused"
        | "Nothing selected"
        | "Stale selection"
        | "Designation prepare refused — game unreachable"
        | "Designation prepare refused — no game id"
        | "Designation prepare refused — bridge unavailable" => "Designate",
        "Actions" | "Confirm command" | "Command result" | "Expired" => "Actions",
        "Command catalogue" => "Catalogue",
        "Guides" => "Learn the game",
        "Snapshots" => "Snapshots",
        "Log" => "Log",
        _ => "",
    }
}

/// The five sections and their pages, with the one-line
/// descriptors the mega-nav panels and the rail switcher share
/// (DESIGN V2.1 tree, restructured in V3 §2.3): (section, section
/// landing href, footer line, [(page, href, icon, descriptor)]).
/// V3: Monitor (the run's state) · Workshop (changing the game;
/// was "Game") · Guidance (NEW peer domain: learn the game, learn
/// the tool, why it suggests) · Plans (its own domain) · Records
/// (was "System"). No URL changed — chrome only.
/// (href, icon, label, blurb) — one entry in a section's panel.
type NavLink = (&'static str, &'static str, &'static str, &'static str);
/// (label, home href, footer line, panel links).
type NavSection = (&'static str, &'static str, &'static str, &'static [NavLink]);

const SECTIONS: [NavSection; 5] = [
    (
        "Monitor",
        "/",
        "Live game state, refreshed on every page load.",
        &[
            (
                "Overview",
                "/",
                "chart",
                "Triage, power and pinned products at a glance",
            ),
            (
                "Issues",
                "/issues",
                "alert",
                "The board — fix now, fix later, watch, ignore",
            ),
            (
                "Products",
                "/products",
                "box",
                "Every product: quantities, storage, monthly flows",
            ),
            (
                "Terrain",
                "/terrain",
                "layers",
                "Heights, deposits and designations — Designate is one hop away",
            ),
        ],
    ),
    (
        "Workshop",
        "/entities",
        "Everything mutating goes prepare → confirm → audit.",
        &[
            (
                "Entities",
                "/entities",
                "factory",
                "Storages & machines, with controls",
            ),
            (
                "Research",
                "/research",
                "flask",
                "Current node, queue and the full catalogue",
            ),
            (
                "Trade",
                "/trade",
                "anchor",
                "Contracts, quick trades and loans",
            ),
            (
                "Actions",
                "/actions",
                "power",
                "Commands through prepare → confirm → audit",
            ),
            (
                "Catalogue",
                "/catalogue",
                "search",
                "Every game command, with its status",
            ),
            (
                "Place",
                "/place",
                "box",
                "Blueprint placement jobs via QoSBuilder",
            ),
            (
                "Designate",
                "/designate",
                "layers",
                "Terrain pads and removals via QoSBuilder",
            ),
        ],
    ),
    (
        "Guidance",
        "/guides",
        "The game, the tool, and the reasoning — in plain words.",
        &[
            (
                "Learn the game",
                "/guides",
                "check",
                "Failure-mode guides, live in your factory",
            ),
            (
                "Learn the tool",
                "/manual",
                "clipboard",
                "How this tool works: pages, evidence, commands",
            ),
            (
                "Why it suggests",
                "/why",
                "search",
                "The rules behind every suggestion, from the canon",
            ),
        ],
    ),
    (
        "Plans",
        "/plans",
        "Step-by-step plans, followed with you — never executed silently.",
        &[(
            "Plans",
            "/plans",
            "layers",
            "Phase-gated plans and their gates",
        )],
    ),
    (
        "Records",
        "/snapshots",
        "The local record — nothing leaves this machine.",
        &[
            (
                "Snapshots",
                "/snapshots",
                "clock",
                "Stored snapshots and trends",
            ),
            (
                "Log",
                "/log",
                "chart",
                "Superintendent and QoS activity, in order",
            ),
        ],
    ),
];

fn section_of(active: &str) -> Option<usize> {
    SECTIONS
        .iter()
        .position(|(_, _, _, pages)| pages.iter().any(|(name, _, _, _)| *name == active))
}

/// One rail context entry (anchor or facet link).
fn rail_link(href: &str, label: &str) -> String {
    format!(
        "<a class=\"navitem anchor\" href=\"{}\">{}</a>",
        href,
        esc(label)
    )
}

/// The default rail context for pages whose context block is a
/// fixed anchor list (DESIGN V2.1 tree). Pages with data-driven
/// context (facets, indexes, lists) call layout_ctx directly.
fn default_ctx(active: &str) -> String {
    let items: &[(&str, &str)] = match active {
        "Overview" => &[
            ("#attention", "Attention"),
            ("#power", "Power"),
            ("#machines", "Machines"),
            ("#outlook", "Outlook"),
            ("#pinned", "Pinned products"),
        ],
        "Issues" => &[
            ("#q-fix-now", "Fix now"),
            ("#q-fix-later", "Fix later"),
            ("#q-watch", "Watch"),
            ("#q-ignore", "Ignore for now"),
            ("#prod", "Productivity"),
        ],
        "Terrain" => &[
            ("#heightmap", "Height map"),
            ("#region", "Region & earthworks"),
            ("#deposits", "Deposits"),
            ("#designations", "Designations"),
        ],
        "Trade" => &[
            ("#contracts", "Contracts"),
            ("#quick", "Quick trades"),
            ("#loans", "Loans"),
            ("#edicts", "Edicts"),
        ],
        "Actions" => &[
            ("#controls", "Game controls"),
            ("#prio", "Global priorities"),
            ("#fix", "Fix actions"),
            ("#audit", "Audit"),
        ],
        "Designate" => &[
            ("#pad", "New pad"),
            ("#live", "Live designations"),
            ("#remove", "Remove"),
        ],
        "Snapshots" => &[
            ("#store", "Store"),
            ("#take", "Take"),
            ("#list", "Snapshots"),
            ("#trends", "Trends"),
        ],
        _ => &[],
    };
    if items.is_empty() {
        return String::new();
    }
    let mut h = String::from("<div class=\"ctx\">ON THIS PAGE</div>");
    for (href, label) in items {
        h.push_str(&rail_link(href, label));
    }
    h
}

fn layout(state: &State, session: &Session, title: &str, body: &str) -> String {
    let active = nav_active(title);
    layout_ctx(state, session, title, active, &default_ctx(active), body)
}

fn layout_nav(state: &State, session: &Session, title: &str, active: &str, body: &str) -> String {
    layout_ctx(state, session, title, active, &default_ctx(active), body)
}

/// The V2 shell: sticky top bar (brand · mega-nav · product jump ·
/// must-fix badge · flattened status) over a depth rail (section
/// label · sub-page switcher · one context block). No user area —
/// there is no login (2026-11-06).
fn layout_ctx(
    state: &State,
    session: &Session,
    title: &str,
    active: &str,
    ctx: &str,
    body: &str,
) -> String {
    let _ = session;
    let sec_idx = section_of(active);
    // Mega-nav.
    let mut mega = String::new();
    for (i, (sec, sec_href, foot, pages)) in SECTIONS.iter().enumerate() {
        let mut rows = String::new();
        for (name, href, ic, desc) in *pages {
            rows.push_str(&format!(
                "<a class=\"mega-row\" href=\"{}\">{}<span class=\"txt\"><span class=\"nm\">{}</span><span class=\"ds\">{}</span></span></a>",
                href,
                icon(ic),
                name,
                desc
            ));
        }
        mega.push_str(&format!(
            "<div class=\"mega{}\"><a href=\"{}\">{}<span class=\"caret\">▾</span></a>\
             <div class=\"mega-panel\"><div class=\"cols\">{}</div><div class=\"mega-foot\">{}</div></div></div>",
            if sec_idx == Some(i) { " active" } else { "" },
            sec_href,
            sec,
            rows,
            foot
        ));
    }
    // Rail: section label + this section's pages + context block.
    let mut rail = String::new();
    if let Some(i) = sec_idx {
        let (sec, _, _, pages) = &SECTIONS[i];
        rail.push_str(&format!("<div class=\"sec\">{}</div>", sec.to_uppercase()));
        for (name, href, _, _) in *pages {
            rail.push_str(&format!(
                "<a class=\"navitem{}\" href=\"{}\">{}</a>",
                if *name == active { " active" } else { "" },
                href,
                name
            ));
        }
    }
    rail.push_str(ctx);
    rail.push_str("<div class=\"railfoot\">Superintendent · design v3</div>");
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} — Superintendent</title><style>{CSS}</style></head><body>{SPRITE}\
         <header class=\"topbar\"><div class=\"brand\"><svg class=\"ic brandic\"><use href=\"#i-clipboard\"/></svg> Superintendent</div>\
         <nav class=\"meganav\">{}</nav>\
         <form class=\"topsearch\" method=\"get\" action=\"/products\">{}<input type=\"text\" name=\"q\" placeholder=\"Jump to product, page…\" aria-label=\"Jump to product\"></form>\
         {}\
         <div class=\"topstatus\">{}</div>\
         <a class=\"btn btn--ghost btn--sm topshutdown\" href=\"/shutdown\">{} Shutdown</a></header>\
         <div class=\"shell\"><aside class=\"rail\">{}</aside><main>{body}</main></div>{BADGE_JS}</body></html>",
        mega,
        icon("search"),
        mustfix_badge(state),
        status_strip(state),
        icon("power"),
        rail
    )
}

// ---------------------------------------------------------------------------
// Design-system component helpers (DESIGN §3)
// ---------------------------------------------------------------------------

/// Status pill (DESIGN §3.2): the word is the status; colour is never
/// the only carrier. `variant` is one of the token-bound names in
/// the CSS badge set (ok/starved/blocked/paused/warning/critical/
/// info/unverified/unev).
fn badge(variant: &str, word: &str) -> String {
    format!(
        "<span class=\"badge badge--{}\">{}</span>",
        variant,
        esc(word)
    )
}

/// Page header pattern (DESIGN V2.1): breadcrumb, H1, one-line
/// summary, optional right-aligned actions.
fn page_head(group: &str, title: &str, summary: &str, actions: &str) -> String {
    format!(
        "<div class=\"crumb\">{} / <b>{}</b></div>\
         <div class=\"pagehead\"><div><h1>{}</h1><div class=\"summary\">{}</div></div><div class=\"actions\">{}</div></div>",
        esc(group),
        esc(title),
        esc(title),
        esc(summary),
        actions,
    )
}

/// The page contract (DESIGN V3 §2): every main page declares, in
/// one standard strip under its head, what territory it owns
/// (domain), whether it acts or only informs (balance), and what
/// data it needs to be truthful (data contract). The head summary
/// carries the fourth element — intent — in player words.
fn contract_strip(domain: &str, balance: &str, data: &str) -> String {
    format!(
        "<div class=\"contract\"><div><span class=\"cl\">Domain</span><span class=\"cv\">{}</span></div>\
         <div><span class=\"cl\">Balance</span><span class=\"cv\">{}</span></div>\
         <div><span class=\"cl\">Data</span><span class=\"cv\">{}</span></div></div>",
        esc(domain),
        esc(balance),
        esc(data)
    )
}

/// A V2 stat tile (DESIGN V2.3): icon tile + label + big tabular
/// value + at most one context line. Absent context renders
/// nothing — the tile is content-height.
fn stat2_tile(
    icon_name: &str,
    label: &str,
    value_html: &str,
    ctx_html: &str,
    critical: bool,
) -> String {
    format!(
        "<div class=\"stat2{}\"><div class=\"top\"><span class=\"ictile\">{}</span><span class=\"label\">{}</span></div><div class=\"value\">{}</div>{}</div>",
        if critical { " stat2--crit" } else { "" },
        icon(icon_name),
        esc(label),
        value_html,
        if ctx_html.is_empty() {
            String::new()
        } else {
            format!("<div class=\"ctx\">{}</div>", ctx_html)
        },
    )
}

/// Signed rendering for a DIFFERENCE (DESIGN V2.5 rule 1): ASCII
/// digits with an explicit + for positives and the Unicode minus
/// for negatives, thousands-separated like fmt_num.
fn signed(n: i64) -> String {
    if n > 0 {
        format!("+{}", fmt_num(&json!(n)))
    } else if n < 0 {
        format!("\u{2212}{}", fmt_num(&json!(-n)))
    } else {
        "0".to_string()
    }
}

/// The delta chip (DESIGN V2.5): the ONLY way a signed difference
/// is rendered. Positive → green + up arrow; negative → red + down
/// arrow; a MEASURED zero → the muted flat chip. Callers must never
/// build a chip from missing data — absence is rendered as text
/// (UNEVALUABLE / no data), never as a flat chip.
fn delta_chip(value: i64, window: &str) -> String {
    let (cls, ic) = if value > 0 {
        ("delta--pos", "up")
    } else if value < 0 {
        ("delta--neg", "down")
    } else {
        ("delta--flat", "flat")
    };
    if window.is_empty() {
        format!(
            "<span class=\"delta {cls}\">{} {}</span>",
            icon(ic),
            signed(value)
        )
    } else {
        format!(
            "<span class=\"delta {cls}\">{} {} <span class=\"u\">{}</span></span>",
            icon(ic),
            signed(value),
            esc(window)
        )
    }
}

/// One figure in a figstrip (DESIGN V2.3): caption label over a
/// tabular value. `warn` paints the value amber — only when a
/// FINDING attaches to the figure (V2.5 rule 4), never for sign.
fn fig(label: &str, value_html: &str, warn: bool) -> String {
    format!(
        "<div class=\"fig\"><div class=\"figl\">{}</div><div class=\"figv{}\">{}</div></div>",
        esc(label),
        if warn { " warn" } else { "" },
        value_html
    )
}

fn figstrip(figs: &[String]) -> String {
    format!("<div class=\"figstrip\">{}</div>", figs.concat())
}

/// D1 degraded state (DESIGN §2.4): the game did not answer — the
/// three checks plus the raw error in a collapsed details block.
fn degraded_game(e: &str) -> String {
    format!(
        "<div class=\"callout callout--danger\"><b>Game unreachable.</b> The game API did not answer. \
         Check: the game is running · a save is loaded · <code>web_api_start</code> is active.\
         <details><summary>Raw error</summary><pre>{}</pre></details></div>",
        esc(e)
    )
}

/// D2 degraded state: a section that needs a newer CoiBridge.
/// D2 placeholder (DESIGN V3, principle 4 — data-state honesty):
/// cause-neutral. The old text asserted "update the mod" as the
/// cause; the route can equally be silent because the bridge isn't
/// loaded with this save. State the floor AND the alternative.
fn degraded_bridge(what: &str, version: &str) -> String {
    format!(
        "<div class=\"empty\">{} need <b>CoiBridge {}+</b> — that route isn't answering right now. \
         If the installed mod is older than {}, updating it is the fix; if it's current, the bridge may not be loaded with this save.</div>",
        esc(what),
        esc(version),
        esc(version),
    )
}

/// §4.6 fill bar: `pct` is 0..=100, `color` a CSS colour expression
/// (usually a var(--token)). Always paired with the number as text
/// by the caller.
fn fillbar(pct: f64, color: &str, width_px: u32) -> String {
    let pct = pct.clamp(0.0, 100.0);
    format!(
        "<span class=\"fillbar\" style=\"width:{}px\"><i style=\"width:{:.1}%;background:{}\"></i></span>",
        width_px, pct, color
    )
}

/// §4.1 sparkline: quantity points → inline SVG polyline. Flat
/// series render as a flat line; the caller adds the min → max
/// caption (the numbers always also exist as text).
fn svg_sparkline(points: &[f64], color: &str) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let (w, h, pad) = (96.0_f64, 28.0_f64, 3.0_f64);
    let min = points.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = points.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = if (max - min).abs() < f64::EPSILON {
        1.0
    } else {
        max - min
    };
    let step = (w - 2.0 * pad) / (points.len() - 1) as f64;
    let mut pts = String::new();
    let mut last = (0.0, 0.0);
    for (i, v) in points.iter().enumerate() {
        let x = pad + i as f64 * step;
        let y = pad + (1.0 - (v - min) / span) * (h - 2.0 * pad);
        last = (x, y);
        pts.push_str(&format!("{:.1},{:.1} ", x, y));
    }
    format!(
        "<svg width=\"96\" height=\"28\" viewBox=\"0 0 96 28\" role=\"img\"><polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/><circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"{}\"/></svg>",
        pts.trim(),
        color,
        last.0,
        last.1,
        color
    )
}

/// §4.4 machine-state stacked bar: (token colour, label, count).
/// Segments ≥12% get an inside label; the caller prints the full
/// counts line as text beside it.
fn svg_stacked(segments: &[(&str, &str, i64)]) -> String {
    let total: i64 = segments.iter().map(|s| s.2).sum();
    if total <= 0 {
        return "<div class=\"stackbar\"></div>".to_string();
    }
    let mut h = String::from("<div class=\"stackbar\">");
    for (color, label, n) in segments {
        if *n <= 0 {
            continue;
        }
        let pct = *n as f64 / total as f64 * 100.0;
        let inside = if pct >= 12.0 {
            format!("<span style=\"color:#0b0e14;font-size:11px;font-weight:600;padding-left:6px;line-height:14px\">{}</span>", esc(label))
        } else {
            String::new()
        };
        h.push_str(&format!(
            "<i style=\"width:{:.1}%;background:{}\">{}</i>",
            pct, color, inside
        ));
    }
    h.push_str("</div>");
    h
}

/// §4.3 runway bar: months of runway against the 12-month cap with
/// the quarter (3-month) tick. Colour by forecast kind per DESIGN.
fn svg_runway(months: f64, kind: &str) -> String {
    let color = match kind {
        "fill" => "var(--blocked)",
        "stall-runway" if months < 3.0 => "var(--critical)",
        "stall-runway" => "var(--starved)",
        _ => "var(--starved)",
    };
    let pct = (months / 12.0 * 100.0).clamp(0.0, 100.0);
    format!(
        "<span style=\"position:relative;display:inline-block;vertical-align:middle\">{}\
         <span style=\"position:absolute;left:25%;top:-2px;bottom:-2px;width:1px;background:var(--text-3)\" title=\"quarter (3 months)\"></span></span>",
        fillbar(pct, color, 110)
    )
}

/// §4.1 large variant for the Product detail chart (100% × 120).
fn svg_sparkline_big(points: &[f64], color: &str) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let (w, h, pad) = (720.0_f64, 120.0_f64, 6.0_f64);
    let min = points.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = points.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = if (max - min).abs() < f64::EPSILON {
        1.0
    } else {
        max - min
    };
    let step = (w - 2.0 * pad) / (points.len() - 1) as f64;
    let mut pts = String::new();
    let mut last = (0.0, 0.0);
    for (i, v) in points.iter().enumerate() {
        let x = pad + i as f64 * step;
        let y = pad + (1.0 - (v - min) / span) * (h - 2.0 * pad);
        last = (x, y);
        pts.push_str(&format!("{:.1},{:.1} ", x, y));
    }
    format!(
        "<svg width=\"100%\" height=\"120\" viewBox=\"0 0 720 120\" preserveAspectRatio=\"none\" role=\"img\"><polyline points=\"720,{h} {pts} {lx},{h}\" fill=\"var(--accent-soft)\" stroke=\"none\"/><polyline points=\"{pts}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"/><circle cx=\"{lx:.1}\" cy=\"{ly:.1}\" r=\"3.5\" fill=\"{color}\"/></svg>",
        h = h,
        pts = pts.trim(),
        lx = last.0,
        ly = last.1,
        color = color,
    )
}

/// Terrain heatmap: a height grid (tiles, row-major) as inline
/// SVG rects on a low→high ramp (deep water blue → green → tan →
/// pale → white). Colour is never alone: the caller prints the
/// height range as text beside it. Cell size scales so the SVG
/// stays ~720 px wide; north (y = 0) is up, matching the game's
/// own map orientation in bridge coordinates.
fn svg_heatmap(heights: &[f64], w: usize, h: usize, min: f64, max: f64) -> String {
    if w == 0 || h == 0 || heights.len() < w * h {
        return String::new();
    }
    const STOPS: [(u8, u8, u8); 5] = [
        (31, 64, 110),
        (46, 125, 79),
        (150, 128, 80),
        (205, 198, 182),
        (246, 246, 242),
    ];
    let span = if (max - min).abs() < f64::EPSILON {
        1.0
    } else {
        max - min
    };
    let cell = (720.0 / w as f64).clamp(1.0, 24.0);
    let mut rects = String::new();
    for y in 0..h {
        for x in 0..w {
            let t = ((heights[y * w + x] - min) / span).clamp(0.0, 1.0);
            let seg = (t * (STOPS.len() - 1) as f64).min((STOPS.len() - 2) as f64);
            let i = seg as usize;
            let f = seg - i as f64;
            let (r0, g0, b0) = STOPS[i];
            let (r1, g1, b1) = STOPS[i + 1];
            let (r, g, b) = (
                (r0 as f64 + (r1 as f64 - r0 as f64) * f) as u8,
                (g0 as f64 + (g1 as f64 - g0 as f64) * f) as u8,
                (b0 as f64 + (b1 as f64 - b0 as f64) * f) as u8,
            );
            rects.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"rgb({},{},{})\"/>",
                x as f64 * cell,
                y as f64 * cell,
                cell,
                cell,
                r,
                g,
                b
            ));
        }
    }
    format!(
        "<svg width=\"{:.0}\" height=\"{:.0}\" viewBox=\"0 0 {:.1} {:.1}\" role=\"img\" style=\"max-width:100%;height:auto\">{}</svg>",
        w as f64 * cell,
        h as f64 * cell,
        w as f64 * cell,
        h as f64 * cell,
        rects
    )
}

/// Human tiles value from a bridge raw Fix32 height/quantity.
fn fmt_tiles(raw: i64) -> String {
    format!("{:.2}", raw as f64 / crate::terrain::RAW_PER_TILE)
}

/// f64 view of a stats flow array entry (values are i64 in the API).
fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn check_csrf(session: &Session, req: &Request) -> Result<(), Response> {
    match req.form.get("csrf") {
        Some(t) if *t == session.csrf => Ok(()),
        _ => Err(Response::text(
            403,
            "CSRF token missing or wrong — go back and retry.",
        )),
    }
}

fn game_unreachable(state: &State, session: &Session, e: &str) -> Response {
    // D1 (DESIGN §2.4): the bare error page is retired — the shell
    // renders, the status strip shows OFFLINE, and the body carries
    // the checks + the raw error in a collapsed details block.
    Response::html(
        502,
        layout(
            state,
            session,
            "Game unreachable",
            &format!(
                "{}<div class=\"pagehead\"><div><div class=\"eyebrow\">DEGRADED</div><h1>Game unreachable</h1>\
                 <div class=\"summary\">This page needs live game data; stored pages (Snapshots, Log, Plans) still work.</div></div></div>",
                degraded_game(e)
            ),
        ),
    )
}

fn fmt_clock(unix: u64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm), UTC.
    let days = (unix / 86400) as i64;
    let secs = unix % 86400;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}

// ---------------------------------------------------------------------------
// Content pages
// ---------------------------------------------------------------------------

fn prepare_form_labeled(
    session: &Session,
    cmd_type: &str,
    args: Value,
    summary: &str,
    label: &str,
) -> String {
    format!(
        "<form method=\"post\" action=\"/actions/prepare\" style=\"display:inline\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"{}\">\
         <input type=\"hidden\" name=\"args\" value=\"{}\">\
         <input type=\"hidden\" name=\"label\" value=\"{}\">\
         <button class=\"btn btn--sm btn--ghost\" type=\"submit\">{}</button></form>",
        esc(&session.csrf),
        esc(cmd_type),
        esc(&args.to_string()),
        esc(summary),
        esc(label),
    )
}

fn prepare_form(
    session: &Session,
    cmd_type: &str,
    args: Value,
    label: &str,
    disabled: bool,
) -> String {
    format!(
        "<form method=\"post\" action=\"/actions/prepare\" style=\"display:inline\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"{}\">\
         <input type=\"hidden\" name=\"args\" value=\"{}\">\
         <button class=\"btn btn--sm btn--ghost\" type=\"submit\" {}>{}</button></form>",
        esc(&session.csrf),
        esc(cmd_type),
        esc(&args.to_string()),
        if disabled {
            "disabled title=\"Already the current setting\""
        } else {
            ""
        },
        esc(label),
    )
}

fn controls_html(session: &Session, live: &Live) -> String {
    let g = &live.game;
    let paused = g["isPaused"].as_bool().unwrap_or(false);
    let speed = g["speed"].as_i64().unwrap_or(1);
    let mut h = String::from("<div class=\"card\" id=\"controls\"><div class=\"cardhead\"><h2>Game controls</h2></div><p>");
    if paused {
        h.push_str(&prepare_form(
            session,
            "SetSimPauseStateCmd",
            json!({"isPaused": false}),
            "Resume game",
            false,
        ));
    } else {
        h.push_str(&prepare_form(
            session,
            "SetSimPauseStateCmd",
            json!({"isPaused": true}),
            "Pause game",
            false,
        ));
    }
    h.push_str(" &nbsp; Speed: ");
    for s in [1, 2, 3] {
        let label = if s == speed {
            format!("{s}× (current)")
        } else {
            format!("{s}×")
        };
        h.push_str(&prepare_form(
            session,
            "GameSpeedChangeCmd",
            json!({"newSpeedMultiplier": s}),
            &label,
            s == speed,
        ));
        h.push(' ');
    }
    h.push_str("</p><p class=\"sub\">Every command goes through a confirmation page showing the exact JSON, and is written to the audit log.</p></div>");
    h
}

/// Quadrant → (card inset class, badge variant) per DESIGN §1.1:
/// Fix now = critical, Fix later = warning, Watch = info, Ignore
/// for now = paused/neutral.
fn quadrant_style(q: &str) -> (&'static str, &'static str) {
    match q {
        "Fix now" => ("issue2--fn", "critical"),
        "Fix later" => ("issue2--fl", "warning"),
        "Watch" => ("issue2--wa", "info"),
        _ => ("issue2--ig", "paused"),
    }
}

/// The state icon at the head of an issue card, by quadrant.
fn quadrant_icon(q: &str) -> &'static str {
    match q {
        "Fix now" => "alert",
        "Fix later" => "clock",
        "Watch" => "chart",
        _ => "check",
    }
}

/// Anchor id for a quadrant section on the Issues board.
fn quadrant_anchor(q: &str) -> &'static str {
    match q {
        "Fix now" => "q-fix-now",
        "Fix later" => "q-fix-later",
        "Watch" => "q-watch",
        _ => "q-ignore",
    }
}

/// Look up a product's stats row by its DISPLAY name (Issue.product
/// and Forecast.product carry names, the stats map is keyed by id).
fn stats_for_name<'a>(live: &'a Live, name: &str) -> Option<(String, &'a Value)> {
    live.names
        .iter()
        .find(|(_, n)| n.as_str() == name)
        .map(|(id, _)| (id.clone(), &live.stats["products"][id]))
}

/// The figure strip for an issue card (DESIGN V2.3 mapping): the
/// measurements behind the issue as figures — never inside the
/// evidence sentence, which survives verbatim as the caption under
/// the strip. Classes without product stats (productivity protos)
/// get no strip; their evidence label is the figure source and
/// stays as the caption alone.
fn issue_figstrip(i: &Issue, live: Option<&Live>) -> String {
    let Some(live) = live else {
        return String::new();
    };
    let Some((_, s)) = stats_for_name(live, &i.product) else {
        return String::new();
    };
    if s.is_null() {
        return String::new();
    }
    let qty = s["quantity"].as_i64().unwrap_or(0);
    let stored = s["stored"].as_i64().unwrap_or(0);
    let cap = s["storageCapacity"].as_i64().unwrap_or(0);
    let prod_lm = crate::flow_at(&s["produced"], 1);
    let cons_lm = crate::flow_at(&s["consumed"], 1);
    let net_lm = prod_lm - cons_lm;
    let pct = if cap > 0 {
        stored as f64 / cap as f64 * 100.0
    } else {
        0.0
    };
    let stored_fig = |bar_color: &str, warn: bool| {
        fig(
            "Stored",
            &format!("{} {}", fmt_num(&s["stored"]), fillbar(pct, bar_color, 90)),
            warn,
        )
    };
    let cover_fig = |months: f64| {
        fig(
            "Cover",
            &format!("{months:.1} <span class=\"u\">months</span>"),
            months < 3.0,
        )
    };
    let figs: Vec<String> = match i.class {
        "stalled" => {
            let mut v = vec![
                fig("Produced LM", &fmt_num(&json!(prod_lm)), false),
                fig("Consumed LM", &fmt_num(&json!(cons_lm)), false),
                fig("Stock", &fmt_num(&s["quantity"]), false),
            ];
            if cons_lm > 0 {
                v.push(cover_fig(qty as f64 / cons_lm as f64));
            }
            v
        }
        "deficit" | "structural" => {
            let mut v = vec![
                fig("Net LM", &delta_chip(net_lm, "LM"), false),
                fig("Stock", &fmt_num(&s["quantity"]), false),
            ];
            if net_lm < 0 {
                v.push(cover_fig(qty as f64 / (-net_lm) as f64));
            }
            v
        }
        "seasonal" => {
            let (net_m, _) = crate::seasonal_net(
                live,
                &stats_for_name(live, &i.product)
                    .map(|(id, _)| id)
                    .unwrap_or_default(),
                s,
            );
            let mut v = vec![
                fig(
                    "Net (long window)",
                    &delta_chip(net_m.round() as i64, "/mo avg"),
                    false,
                ),
                fig("Stock", &fmt_num(&s["quantity"]), false),
            ];
            if net_m < 0.0 {
                v.push(cover_fig(qty as f64 / (-net_m)));
            }
            v
        }
        "storage-full" => vec![
            stored_fig("var(--warning)", pct >= 90.0),
            fig("Capacity", &fmt_num(&s["storageCapacity"]), false),
            fig("Produced LM", &fmt_num(&json!(prod_lm)), false),
        ],
        "parked" => vec![
            // Row 6 of the CTA table: a parked full buffer is NOT a
            // finding — figures neutral, bar in the accent colour.
            stored_fig("var(--accent)", false),
            fig("Capacity", &fmt_num(&s["storageCapacity"]), false),
            fig("Produced LM", &fmt_num(&json!(prod_lm)), false),
        ],
        "overcap" => vec![
            stored_fig("var(--warning)", true),
            fig("Capacity", &fmt_num(&s["storageCapacity"]), false),
            fig("Quantity", &fmt_num(&s["quantity"]), false),
        ],
        "below-baseline" => vec![
            fig("Produced LM", &fmt_num(&json!(prod_lm)), false),
            fig("Stock", &fmt_num(&s["quantity"]), false),
        ],
        _ => return String::new(),
    };
    if figs.is_empty() {
        String::new()
    } else {
        figstrip(&figs)
    }
}

/// The CTA row for an issue card (DESIGN V2.7): ONE primary
/// button + at most one ghost link, keyed by the issue's generator
/// class. A CTA never applies anything — it lands on a detail
/// page, a sources anchor, a filtered directory or a prepare form.
/// The parked class gets the spec'd no-action treatment instead.
fn issue_ctas(i: &Issue, live: Option<&Live>) -> String {
    let pid = live.and_then(|l| stats_for_name(l, &i.product).map(|(id, _)| id));
    let detail = format!("/issue?p={}&t={}", url_enc(&i.product), url_enc(&i.title));
    let product_href = match &pid {
        Some(id) => format!("/product?id={}", esc(id)),
        None => format!("/products?q={}", url_enc(&i.product)),
    };
    let primary = |href: &str, label: &str| {
        format!(
            "<a class=\"btn btn--sm\" href=\"{}\">{}</a>",
            href,
            esc(label)
        )
    };
    let ghost = |href: &str, label: &str| {
        format!(
            "<a class=\"btn btn--sm btn--ghost\" href=\"{}\">{}</a>",
            href,
            esc(label)
        )
    };
    match i.class {
        "stalled" => format!(
            "{}{}",
            primary(&format!("{detail}#fix"), "Inspect producers…"),
            ghost("/entities?view=machines", "See machines")
        ),
        "deficit" | "structural" => format!(
            "{}{}",
            primary(&format!("{product_href}#sources"), "See who draws it"),
            ghost(&product_href, "View product")
        ),
        "seasonal" => primary(&product_href, "View product"),
        "storage-full" => format!(
            "{}{}",
            primary(&format!("{detail}#fix"), "Open fix panel…"),
            ghost(
                &format!(
                    "/entities?view=storages&product={}",
                    pid.as_deref().unwrap_or("")
                ),
                "See storages"
            )
        ),
        "parked" => format!(
            "<span class=\"noact\">{} No action needed</span>{}",
            icon("check"),
            ghost(
                &format!(
                    "/entities?view=storages&product={}",
                    pid.as_deref().unwrap_or("")
                ),
                "View storages"
            )
        ),
        "overcap" => format!(
            "{}{}",
            primary(
                &format!(
                    "/entities?view=storages&product={}",
                    pid.as_deref().unwrap_or("")
                ),
                "See storages"
            ),
            ghost(&product_href, "View product")
        ),
        "prod-starved" => primary(
            &format!("/entities?view=machines&proto={}", url_enc(&i.product)),
            "See machines",
        ),
        "prod-blocked" => format!(
            "{}{}",
            primary(
                &format!("/entities?view=machines&proto={}", url_enc(&i.product)),
                "See machines"
            ),
            ghost("/entities?view=storages", "See storages")
        ),
        "prod-mixed" => primary(
            &format!("/entities?view=machines&proto={}", url_enc(&i.product)),
            "See machines",
        ),
        "prod-unclassified" => format!(
            "{}{}",
            primary(
                &format!("/entities?view=machines&proto={}", url_enc(&i.product)),
                "See machines"
            ),
            ghost("/actions", "Open Actions")
        ),
        "below-baseline" => format!(
            "{}{}",
            primary(&product_href, "View product"),
            ghost("/entities?view=machines", "See machines")
        ),
        _ => primary(&detail, "Open issue…"),
    }
}

/// The V2 issue card (DESIGN V2.7 anatomy): state icon + product +
/// title + quadrant badge · figure strip · evidence verbatim as
/// caption · one-line guidance · CTA row.
fn issue_card(i: &Issue, live: Option<&Live>) -> String {
    let (cls, variant) = quadrant_style(i.quadrant);
    let strip = issue_figstrip(i, live);
    format!(
        "<div class=\"issue2 {cls}\"><div class=\"r1\"><span class=\"state-ic\">{}</span><span class=\"p\">{}</span><span class=\"t\"><a href=\"/issue?p={}&t={}\">{}</a></span><span class=\"qb\">{}</span></div>{}\
         <div class=\"evcap\">{}</div><div class=\"guide\">{}</div><div class=\"ctas\">{}</div></div>",
        icon(quadrant_icon(i.quadrant)),
        esc(&i.product),
        url_enc(&i.product),
        url_enc(&i.title),
        esc(&i.title),
        badge(variant, i.quadrant),
        strip,
        esc(&i.evidence),
        esc(&i.fix),
        issue_ctas(i, live),
    )
}

fn issues_html(issues: &[Issue], compact: bool, live: Option<&Live>, hide_empty: bool) -> String {
    let quads = ["Fix now", "Fix later", "Watch", "Ignore for now"];
    let mut h = String::new();
    if compact {
        // Overview form (DESIGN V3 §1.1 cap): one card — ALL of
        // Fix now (at most 3 cards + a count link), the FIRST
        // Fix later only, and Watch / Ignore for now as a counts
        // strip linking to the board. The Overview is a summary;
        // the board is the board.
        h.push_str("<div class=\"card\"><div class=\"cardhead\"><h2>What needs attention</h2><span class=\"right\"><a href=\"/issues\">All issues →</a></span></div>");
        let mut any = false;
        let mut counts_strip = String::new();
        for q in quads {
            let items: Vec<&Issue> = issues.iter().filter(|i| i.quadrant == q).collect();
            if items.is_empty() {
                continue;
            }
            any = true;
            let (_, variant) = quadrant_style(q);
            match q {
                "Fix now" | "Fix later" => {
                    let take = if q == "Fix now" { 3 } else { 1 };
                    h.push_str(&format!(
                        "<div class=\"quadhead\">{}<span class=\"caption\" style=\"margin:0\">{}</span></div>",
                        badge(variant, q),
                        items.len()
                    ));
                    for i in items.iter().take(take) {
                        h.push_str(&issue_card(i, live));
                    }
                    if items.len() > take {
                        h.push_str(&format!(
                            "<div class=\"evcap\"><a href=\"/issues#{}\">+ {} more in {} →</a></div>",
                            quadrant_anchor(q),
                            items.len() - take,
                            q
                        ));
                    }
                }
                _ => {
                    counts_strip.push_str(&format!("{} {} · ", items.len(), q));
                }
            }
        }
        if !counts_strip.is_empty() {
            h.push_str(&format!(
                "<div class=\"evcap\"><a href=\"/issues\">Also on the board: {}→</a></div>",
                counts_strip
            ));
        }
        if !any {
            h.push_str("<div class=\"empty\">Nothing needs attention — no issues on the board right now.</div>");
        }
        h.push_str("</div>");
        return h;
    }
    // Full board: one anchored section per quadrant, the "why this
    // quadrant" copy as a collapsed details in the head (V2.2:
    // explainers are demoted, never deleted).
    for q in quads {
        let items: Vec<&Issue> = issues.iter().filter(|i| i.quadrant == q).collect();
        if hide_empty && items.is_empty() {
            continue;
        }
        let (_, variant) = quadrant_style(q);
        h.push_str(&format!(
            "<div class=\"quadhead\" id=\"{}\">{}<span class=\"caption\" style=\"margin:0\">{} issue(s)</span>\
             <details class=\"quadwhy\"><summary>Why this quadrant?</summary><p>{}</p><p><a href=\"/why?topic=board\">Why this order — the full rules, from the canon →</a></p></details></div>",
            quadrant_anchor(q),
            badge(variant, q),
            items.len(),
            quadrant_why(q)
        ));
        if items.is_empty() {
            h.push_str(&format!(
                "<div class=\"empty\">No issues in \u{2018}{}\u{2019} right now.</div>",
                q
            ));
        }
        for i in items {
            h.push_str(&issue_card(i, live));
        }
    }
    h
}

fn pinned_table(session: &Session, live: &Live, issues: &[Issue]) -> String {
    let mut h = String::from(
        "<div class=\"card tablecard\" id=\"pinned\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Pinned products</h2><span class=\"right\"><a href=\"/products?f=pinned\">All pinned →</a></span></div>\
         <table class=\"table--compact\"><thead><tr><th>Product</th><th>Trend (8 mo)</th><th class=\"num\">Quantity</th><th>Stored / cap</th><th class=\"num\">Net LM</th><th></th></tr></thead><tbody>",
    );
    let mut ids = live.pinned_ids.clone();
    ids.sort_by_key(|id| live.names.get(id).cloned().unwrap_or_else(|| id.clone()));
    let total = ids
        .iter()
        .filter(|id| !live.stats["products"][*id].is_null())
        .count();
    let mut shown = 0usize;
    for id in &ids {
        let s = &live.stats["products"][id];
        if s.is_null() {
            continue;
        }
        if shown >= 6 {
            break;
        }
        shown += 1;
        let name = live.names.get(id).cloned().unwrap_or_else(|| id.clone());
        // Sparkline from the monthly history the live fetch already
        // carries; colour follows the direction of travel.
        let mut trend = String::new();
        if let Some(q) = live.history["products"][id]["quantity"].as_array() {
            let pts: Vec<f64> = q.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
            if pts.len() >= 2 {
                let (first, last) = (pts[0], pts[pts.len() - 1]);
                let color = if last < first {
                    "var(--starved)"
                } else if last > first {
                    "var(--ok)"
                } else {
                    "var(--accent)"
                };
                trend = svg_sparkline(&pts, color);
            }
        }
        let issue_badge = issues
            .iter()
            .find(|i| i.product == name)
            .map(|i| format!(" {}", badge(quadrant_style(i.quadrant).1, i.quadrant)))
            .unwrap_or_default();
        let stored = s["stored"].as_i64().unwrap_or(0);
        let cap = s["storageCapacity"].as_i64().unwrap_or(0);
        let pct = if cap > 0 {
            stored as f64 / cap as f64 * 100.0
        } else {
            0.0
        };
        // V2.5: stored/capacity are absolutes — neutral figures,
        // magnitude via the fill bar; amber only because ≥90% full
        // is a finding. Net LM is a difference — a delta chip.
        let bar_color = if pct >= 90.0 {
            "var(--warning)"
        } else {
            "var(--accent)"
        };
        let net_lm = crate::flow_at(&s["produced"], 1) - crate::flow_at(&s["consumed"], 1);
        h.push_str(&format!(
            "<tr><td><a href=\"/product?id={}\">{}</a>{}</td><td>{}</td><td class=\"num\">{}</td><td><span class=\"num\">{} / {}</span> {}</td><td class=\"num\">{}</td><td>{}</td></tr>",
            esc(id),
            esc(&name),
            issue_badge,
            trend,
            fmt_num(&s["quantity"]),
            fmt_num(&s["stored"]),
            fmt_num(&s["storageCapacity"]),
            fillbar(pct, bar_color, 90),
            delta_chip(net_lm, "LM"),
            prepare_form(session, "PinToggleCmd", json!({"productToToggle": id}), "Unpin", false),
        ));
    }
    h.push_str("</tbody></table>");
    if total > shown {
        h.push_str(&format!(
            "<p class=\"caption\" style=\"padding:0 12px 10px\">Showing {} of {} pinned — <a href=\"/products?f=pinned\">see all pinned</a></p>",
            shown, total
        ));
    }
    h.push_str("</div>");
    h
}

fn forecasts_html(forecasts: &[Forecast], issues: &[Issue]) -> String {
    if forecasts.is_empty() {
        return String::new();
    }
    let mut h = String::from(
        "<div class=\"card tablecard\" id=\"outlook\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Outlook — what happens next at current rates</h2></div>\
         <p class=\"caption\" style=\"padding:0 12px\">At current measured rates — any build, pause or trade change moves these dates. The bar runs to the 12-month cap; the tick marks one quarter.</p>\
         <table class=\"table--compact\"><thead><tr><th>Product</th><th>Outlook</th><th class=\"num\">Net TM</th><th class=\"num\">Net LM</th><th class=\"num\">Stored</th><th>Guidance</th><th></th></tr></thead><tbody>",
    );
    for f in forecasts.iter().take(5) {
        let label = match f.kind {
            "fill" => format!("storage full in ≈ {:.1} months", f.months),
            "stall-runway" => format!("stock lasts ≈ {:.1} months (production stalled)", f.months),
            _ => format!("stock lasts ≈ {:.1} months", f.months),
        };
        // CTA row 13 (V2.7): View product, plus See the issue when
        // the board carries a matching one.
        let mut ctas = format!(
            "<a class=\"btn btn--sm btn--ghost\" href=\"/products?q={}\">View product</a>",
            url_enc(&f.product)
        );
        if let Some(i) = issues.iter().find(|i| i.product == f.product) {
            ctas.push_str(&format!(
                " <a class=\"btn btn--sm btn--ghost\" href=\"/issue?p={}&t={}\">See the issue</a>",
                url_enc(&i.product),
                url_enc(&i.title)
            ));
        }
        h.push_str(&format!(
            "<tr><td>{}</td><td>{} <span class=\"sub\">{}</span><div class=\"caption\">{}</div></td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
            esc(&f.product),
            svg_runway(f.months, f.kind),
            esc(&label),
            esc(&f.basis),
            delta_chip(f.net_tm, "TM"),
            delta_chip(f.net_lm, "LM"),
            fmt_num(&json!(f.stored)),
            esc(&f.advice),
            ctas,
        ));
    }
    h.push_str("</tbody></table></div>");
    h
}

fn page_overview(state: &State, session: &Session) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            // Game off / API down: still a 200 page, D1 pattern (the
            // power panel renders only if the bridge answers, which
            // it won't).
            let mut body = page_head(
                "Monitor",
                "Overview",
                "One screen for the state of the whole run — the tiles are its vital signs.",
                "",
            );
            body.push_str(&contract_strip(
                "The whole run, summarised",
                "Informational — the only actions here are the game controls and Take snapshot",
                "Live game state + your pins. If the game is off, figures come from the last snapshot and are labelled as such",
            ));
            body.push_str(&degraded_game(&e));
            body.push_str(&power_panel(state));
            badge_note(state, None);
            return Response::html(200, layout(state, session, "Overview", &body));
        }
    };
    let issues = compute_issues(&live);
    badge_note(state, Some(&live));
    let forecasts = compute_forecasts(&live);
    let power = power_data(state);
    // Stat tile row (DESIGN V2.3): Population · Power balance ·
    // Machines working · Fix now — figures, not sentences.
    let mut tiles = String::from("<div class=\"stats2\">");
    match bridge_json(state, "/coi/v1/status") {
        Some(st) => {
            let starving = st["arePeopleStarving"].as_bool().unwrap_or(false);
            tiles.push_str(&stat2_tile(
                "users",
                "Population",
                &fmt_num(&st["population"]),
                &format!(
                    "housing {} free of {} · food ≈ {} months{}",
                    fmt_num(&st["freeHousingCapacity"]),
                    fmt_num(&st["totalHousingCapacity"]),
                    fmt_num(&st["monthsOfFood"]),
                    if starving { " · <b>STARVING</b>" } else { "" },
                ),
                starving,
            ));
        }
        None => tiles.push_str(&stat2_tile(
            "users",
            "Population",
            "—",
            "reported by CoiBridge — not answered right now",
            false,
        )),
    }
    if let Some(p) = &power {
        let bal = vi(&p["balanceKw"]);
        // The balance is a difference (V2.5 rule 1): sign colour +
        // arrow. Generated/consumed/capacity are absolutes and
        // stay neutral in the context line.
        let (cls, ic) = if bal > 0 {
            ("var(--pos)", "up")
        } else if bal < 0 {
            ("var(--neg)", "down")
        } else {
            ("var(--flat)", "flat")
        };
        tiles.push_str(&stat2_tile(
            "power",
            "Power balance",
            &format!(
                "<span style=\"color:{cls}\">{} {}</span> <span class=\"u\">kW</span>",
                icon(ic),
                signed(bal)
            ),
            &format!(
                "{} generated · {} consumed · capacity {} kW",
                fmt_num(&json!(vi(&p["generatedKw"]))),
                fmt_num(&json!(vi(&p["consumedKw"]))),
                fmt_num(&json!(vi(&p["generationCapacityKw"])))
            ),
            bal < 0,
        ));
    }
    let mut machine_rows: Option<Vec<Value>> = None;
    if let Some(m) = bridge_json(state, "/coi/v1/machines") {
        machine_rows = m["machines"].as_array().cloned();
    }
    if let Some(rows) = &machine_rows {
        let count = |s: &str| {
            rows.iter()
                .filter(|r| r["state"].as_str() == Some(s))
                .count() as i64
        };
        tiles.push_str(&stat2_tile(
            "factory",
            "Machines working",
            &format!(
                "{} <span class=\"u\">/ {}</span>",
                count("Working"),
                rows.len()
            ),
            &format!(
                "{} {}",
                badge("starved", &format!("{} starved", count("NotEnoughInput"))),
                badge("blocked", &format!("{} blocked", count("OutputFull")))
            ),
            false,
        ));
    }
    let fix_now: Vec<&Issue> = issues.iter().filter(|i| i.quadrant == "Fix now").collect();
    tiles.push_str(&stat2_tile(
        "alert",
        "Fix now",
        &fix_now.len().to_string(),
        &fix_now
            .iter()
            .map(|i| esc(&i.product))
            .collect::<Vec<_>>()
            .join(" · "),
        !fix_now.is_empty(),
    ));
    tiles.push_str("</div>");

    let mut body = page_head(
        "Monitor",
        "Overview",
        "One screen for the state of the whole run — the tiles are its vital signs.",
        &format!(
            "<form method=\"post\" action=\"/snapshots/take\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><button class=\"btn--secondary\" type=\"submit\">Take snapshot</button></form>",
            esc(&session.csrf)
        ),
    );
    body.push_str(&contract_strip(
        "The whole run, summarised",
        "Informational — the only actions here are the game controls and Take snapshot",
        "Live game state + your pins. If the game is off, figures come from the last snapshot and are labelled as such",
    ));
    body.push_str(&tiles);
    // D2 honesty (DESIGN V3 §1.1): when CoiBridge-derived figures are
    // absent, name them — never let a missing tile read as zero.
    {
        let mut missing: Vec<&str> = Vec::new();
        if power.is_none() {
            missing.push("power balance");
        }
        if machine_rows.is_none() {
            missing.push("machine states");
        }
        if bridge_json(state, "/coi/v1/status").is_none() {
            missing.push("population / housing / food");
        }
        if !missing.is_empty() {
            body.push_str(&format!(
                "<p class=\"caption\">Not shown: {} — CoiBridge isn't answering for those right now; the figures above are the game's own API only.</p>",
                missing.join(", ")
            ));
        }
    }
    body.push_str("<div class=\"ovgrid\"><div id=\"attention\">");
    body.push_str(&issues_html(&issues, true, Some(&live), false));
    if let Some(rows) = &machine_rows {
        let count = |s: &str| {
            rows.iter()
                .filter(|r| r["state"].as_str() == Some(s))
                .count() as i64
        };
        let (w, s, b, p) = (
            count("Working"),
            count("NotEnoughInput"),
            count("OutputFull"),
            count("Paused"),
        );
        let other = rows.len() as i64 - w - s - b - p;
        let pct = |n: i64| {
            if rows.is_empty() {
                0.0
            } else {
                n as f64 / rows.len() as f64 * 100.0
            }
        };
        body.push_str(&format!(
            "<div class=\"card\" id=\"machines\"><div class=\"cardhead\"><h2>Machine states</h2><span class=\"right badge badge--paused\">via CoiBridge</span></div>{}\
             <div class=\"caption\">{} working ({:.0}%) · {} starved ({:.0}%) · {} blocked ({:.0}%) · {} paused ({:.0}%){} — of {} machines, this snapshot.</div></div>",
            svg_stacked(&[
                ("var(--ok)", "Working", w),
                ("var(--starved)", "Starved", s),
                ("var(--blocked)", "Blocked", b),
                ("var(--paused)", "Paused", p),
                ("var(--surface-3)", "Other", other),
            ]),
            w, pct(w), s, pct(s), b, pct(b), p, pct(p),
            if other > 0 { format!(" · {} other", other) } else { String::new() },
            rows.len(),
        ));
    }
    body.push_str("</div><div class=\"ovrail\">");
    if let Some(p) = &power {
        body.push_str("<div id=\"power\">");
        body.push_str(&power_panel_html(p));
        body.push_str("</div>");
    }
    body.push_str(&controls_html(session, &live));
    body.push_str("</div></div>");
    body.push_str(&forecasts_html(&forecasts, &issues));
    body.push_str(&pinned_table(session, &live, &issues));
    // The screenshot is reference material, not content (DESIGN V3
    // §1.1): it sits below Pinned, last on the page.
    body.push_str("<div class=\"shotwrap\"><div style=\"display:flex;align-items:center;margin-bottom:10px\"><h2 style=\"margin:0\">Live screenshot</h2><span class=\"caption\" style=\"margin:0 0 0 auto\">refreshed on page load</span></div><img class=\"shot\" src=\"/screenshot.jpg\" alt=\"live screenshot\"></div>");
    Response::html(200, layout(state, session, "Overview", &body))
}

fn page_issues(state: &State, session: &Session, req: &Request) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            let mut body = page_head(
                "Monitor",
                "Issues",
                "What to fix, in what order — and why it ranks there.",
                "",
            );
            body.push_str(&contract_strip(
                "Findings from live flows, storage levels and machine productivity",
                "Informational — every fix is a link to a safe prepare/detail page; nothing here applies anything",
                "Needs the live game for the board; productivity findings below it need snapshots in the store",
            ));
            body.push_str(&degraded_game(&e));
            badge_note(state, None);
            return Response::html(200, layout(state, session, "Issues", &body));
        }
    };
    let issues = compute_issues(&live);
    badge_note(state, Some(&live));
    // The must-fix badge lands here: ?only=fix-now shows just the
    // Fix now quadrant (same compute_issues data, no second count).
    let only_fix_now = req.query.get("only").map(|s| s.as_str()) == Some("fix-now");
    let mut body = page_head(
        "Monitor",
        "Issues",
        "What to fix, in what order — and why it ranks there.",
        "",
    );
    body.push_str(&contract_strip(
        "Findings from live flows, storage levels and machine productivity",
        "Informational — every fix is a link to a safe prepare/detail page; nothing here applies anything",
        "Needs the live game for the board; productivity findings below it need snapshots in the store",
    ));
    body.push_str("<details class=\"howit\"><summary>How the board is built</summary><p>Generated live from production/consumption flows and storage levels. \
        Urgency comes from how soon a buffer runs dry or blocks; importance from how much of the \
        factory depends on the product. The exact rules live in <a href=\"/why?topic=board\">Why it suggests → The issues board</a> — \
        the board and the explanation can never drift apart, because both render from the same canon. \
        <b>Click any issue title</b> to open it in depth: why it sits \
        in its quadrant (with the numbers), the full evidence, the forecast, how to fix it — and, \
        where a fix is a game command, buttons that prepare it for the specific storages and \
        machines involved.</p></details>");
    if only_fix_now {
        let filtered: Vec<Issue> = issues
            .into_iter()
            .filter(|i| i.quadrant == "Fix now")
            .collect();
        body.push_str(&format!(
            "<p class=\"caption\"><span class=\"badge badge--critical\">Filtered: Fix now only</span> {} issue(s). <a href=\"/issues\">Show the full board</a></p>",
            filtered.len()
        ));
        if filtered.is_empty() {
            body.push_str("<div class=\"empty\">Nothing in ‘Fix now’ right now.</div>");
        } else {
            body.push_str(&issues_html(&filtered, false, Some(&live), true));
        }
    } else {
        body.push_str(&issues_html(&issues, false, Some(&live), false));
        // Depth-on-demand (DESIGN V3 §5): the productivity evidence
        // is a second, heavier system under the board — collapsed
        // behind a summary line so the board keeps the page. The
        // board itself is never collapsed (audit: explicitly not
        // worth doing).
        let prod = productivity_section(state);
        if !prod.is_empty() {
            body.push_str(&format!(
                "<details class=\"indgroup\" id=\"prodwrap\"><summary>Productivity evidence <span class=\"sub\">· machine working-share verdicts from your snapshots — open for the full table</span></summary>{}</details>",
                prod
            ));
        }
    }
    // Exports sit at the foot (DESIGN V3 §1.2): the head of the page
    // is for the board, not for file formats.
    body.push_str("<div class=\"pager\"><span class=\"sub\">Export this board:</span> <a class=\"btn btn--secondary btn--sm\" href=\"/api/report.md\">Report as markdown</a> <a class=\"btn btn--secondary btn--sm\" href=\"/api/snapshot.json\">Raw snapshot JSON</a></div>");
    Response::html(200, layout(state, session, "Issues", &body))
}

/// Productivity evidence section (QOL-MOD-PREP §5): read-only over
/// the persisted machine-state aggregates. Below the evidence line a
/// proto is listed as INSUFFICIENT — unjudged, never "healthy".
/// Renders nothing when no database/snapshots exist yet.
fn productivity_section(state: &State) -> String {
    if !std::path::Path::new(&state.db_path).exists() {
        return String::new();
    }
    let Ok(st) = store(state) else {
        return String::new();
    };
    let th = crate::productivity::Thresholds::load(&state.db_path);
    let Ok(lines) = crate::productivity::report_lines(&st, &th) else {
        return String::new();
    };
    if lines.is_empty() {
        return String::new();
    }
    let mut body = String::from(
        "<div class=\"card\" id=\"prod\"><div class=\"cardhead\"><h2>Productivity evidence</h2><span class=\"right\"><a href=\"/why?topic=productivity\">Why these verdicts →</a></span></div>\
         <p class=\"sub\">Machine working-share from stored snapshots, \
         judged only when the evidence class reaches ESTIMATED — below that, a line reads \
         INSUFFICIENT and means <b>unjudged</b>, not healthy.</p>",
    );
    // Per-proto stacked bars from the latest stored machine-state
    // row per proto (same aggregates the estimator reads).
    if let Ok(Some(gid)) = st.latest_game_id() {
        if let Ok(series) = st.machine_state_series(&gid) {
            let mut latest: std::collections::BTreeMap<String, (i64, i64, i64, i64, i64)> =
                Default::default();
            for (_, proto, w, n, o, p, ot, _, _) in &series {
                latest.insert(proto.clone(), (*w, *n, *o, *p, *ot));
            }
            if !latest.is_empty() {
                body.push_str("<table><thead><tr><th>Machine type</th><th>Working share (latest snapshot)</th><th>Counts</th></tr></thead><tbody>");
                for (proto, (w, n, o, p, ot)) in &latest {
                    body.push_str(&format!(
                        "<tr><td>{}</td><td style=\"min-width:180px\">{}</td><td class=\"sub\">{} working · {} starved · {} blocked · {} paused{}</td></tr>",
                        esc(proto),
                        svg_stacked(&[
                            ("var(--ok)", "Working", *w),
                            ("var(--starved)", "Starved", *n),
                            ("var(--blocked)", "Blocked", *o),
                            ("var(--paused)", "Paused", *p),
                            ("var(--surface-3)", "Other", *ot),
                        ]),
                        w, n, o, p,
                        if *ot > 0 { format!(" · {} other", ot) } else { String::new() },
                    ));
                }
                body.push_str("</tbody></table>");
            }
        }
    }
    for line in lines {
        // Evidence-class badge from the line's own class word; the
        // line text itself is unchanged.
        let class_badge = if line.contains("INSUFFICIENT") {
            badge("unev", "INSUFFICIENT")
        } else if line.contains("MEASURED") {
            badge("ok", "MEASURED")
        } else if line.contains("ESTIMATED") {
            badge("info", "ESTIMATED")
        } else if line.contains("PARKED") {
            badge("paused", "PARKED")
        } else if line.contains("FIRING") {
            badge("critical", "FIRING")
        } else {
            String::new()
        };
        body.push_str(&format!(
            "<div class=\"kvrow\"><span>{}</span><span>{}</span></div>",
            class_badge,
            esc(&line)
        ));
    }
    body.push_str("</div>");
    body
}

/// (id, name, qty, stored, net_tm, net_lm, sort_key, pinned) — one
/// row of the products directory built by page_products.
type ProductTableRow = (String, String, i64, i64, i64, i64, i64, bool);

fn page_products(state: &State, session: &Session, req: &Request) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => return game_unreachable(state, session, &e),
    };
    // Facets (V2.1): All · Pinned · In issues · Fluid · Loose ·
    // Countable, via ?f=. The top-bar jump lands here as ?q= and
    // seeds the filter (server-side, so the landing is already
    // narrowed). Pagination is server-side, 60 rows per page; sort
    // is server-side too (?sort=, DESIGN V3 — the v2.1 follow-up,
    // landed), so a sort orders the whole list, not just the page.
    let facet = req.query.get("f").map(|s| s.as_str()).unwrap_or("all");
    let sort = req.query.get("sort").map(|s| s.as_str()).unwrap_or("name");
    let q = req.query.get("q").cloned().unwrap_or_default();
    let page: usize = req
        .query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let issues = compute_issues(&live);
    let issue_names: std::collections::HashSet<&str> =
        issues.iter().map(|i| i.product.as_str()).collect();
    let mut rows: Vec<ProductTableRow> = Vec::new();
    let mut counts = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    if let Some(map) = live.stats["products"].as_object() {
        let mut ids: Vec<&String> = map.keys().collect();
        ids.sort_by_key(|id| {
            live.names
                .get(*id)
                .cloned()
                .unwrap_or_else(|| id.to_string())
        });
        for id in ids {
            let kind = live.kinds.get(id).cloned().unwrap_or_default();
            if kind == "virtual" {
                continue;
            }
            let s = &map[id];
            let name = live
                .names
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let pinned = live.pinned_ids.iter().any(|p| p == id);
            counts.0 += 1;
            if pinned {
                counts.1 += 1;
            }
            if issue_names.contains(name.as_str()) {
                counts.2 += 1;
            }
            match kind.as_str() {
                "fluid" => counts.3 += 1,
                "loose" => counts.4 += 1,
                "countable" => counts.5 += 1,
                _ => {}
            }
            let in_facet = match facet {
                "pinned" => pinned,
                "issues" => issue_names.contains(name.as_str()),
                "fluid" => kind == "fluid",
                "loose" => kind == "loose",
                "countable" => kind == "countable",
                _ => true,
            };
            if !in_facet {
                continue;
            }
            if !q.is_empty()
                && !name.to_lowercase().contains(&q.to_lowercase())
                && !id.to_lowercase().contains(&q.to_lowercase())
            {
                continue;
            }
            rows.push((
                id.clone(),
                name,
                s["quantity"].as_i64().unwrap_or(0),
                s["stored"].as_i64().unwrap_or(0),
                s["storageCapacity"].as_i64().unwrap_or(0),
                crate::flow_at(&s["produced"], 1) - crate::flow_at(&s["consumed"], 1),
                crate::flow_at(&s["produced"], 0) - crate::flow_at(&s["consumed"], 0),
                pinned,
            ));
        }
    }
    match sort {
        "qty" => rows.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1))),
        "stored" => rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.1.cmp(&b.1))),
        "netlm" => rows.sort_by(|a, b| a.5.cmp(&b.5).then_with(|| a.1.cmp(&b.1))),
        "nettm" => rows.sort_by(|a, b| a.6.cmp(&b.6).then_with(|| a.1.cmp(&b.1))),
        _ => {}
    }
    let _ = session;
    let total_rows = rows.len();
    let pages = total_rows.div_ceil(60).max(1);
    let page = page.min(pages - 1);
    let facet_label = match facet {
        "pinned" => "Pinned",
        "issues" => "In issues",
        "fluid" => "Fluid",
        "loose" => "Loose",
        "countable" => "Countable",
        _ => "All",
    };
    let mut body = page_head(
        "Monitor",
        "Products",
        "Every product in your factory — what you hold, what's flowing, what's running out.",
        "",
    );
    body.push_str(&contract_strip(
        "All products, one directory",
        "Informational, with pin toggles — pinning is the one action, via prepare → confirm",
        "Live game state. Virtual products are excluded, and pinning a product starts its history — a pinned product's charts begin at its first pin",
    ));
    body.push_str("<div class=\"card tablecard\"><div class=\"filterbar\"><form method=\"get\" action=\"/products\" style=\"display:flex;gap:8px;align-items:center;flex:1\">");
    if facet != "all" {
        body.push_str(&format!(
            "<input type=\"hidden\" name=\"f\" value=\"{}\">",
            esc(facet)
        ));
    }
    let sort_opts = [
        ("name", "Name"),
        ("qty", "Quantity"),
        ("stored", "Stored"),
        ("netlm", "Net LM (worst first)"),
        ("nettm", "Net TM (worst first)"),
    ]
    .iter()
    .map(|(v, l)| {
        format!(
            "<option value=\"{v}\"{}>{l}</option>",
            if sort == *v { " selected" } else { "" }
        )
    })
    .collect::<String>();
    body.push_str(&format!(
        "<input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"🔍 name, kind, id…\"><label class=\"sub\">Sort <select name=\"sort\">{}</select></label><button class=\"btn btn--secondary btn--sm\" type=\"submit\">Filter</button></form><span class=\"count\">{} products · {} · page {} of {}</span></div>",
        esc(&q),
        sort_opts,
        total_rows,
        facet_label,
        page + 1,
        pages
    ));
    body.push_str("<table id=\"ptable\" class=\"table--compact\"><thead><tr><th>Product</th><th>Kind</th><th class=\"num\">Quantity</th><th>Stored / cap</th><th class=\"num\">Prod TM/LM</th><th class=\"num\">Cons TM/LM</th><th class=\"num\">Net TM</th><th class=\"num\">Net LM</th><th>Pin</th></tr></thead><tbody>");
    for (id, name, qty, stored, cap, net_lm, net_tm, pinned) in rows.iter().skip(page * 60).take(60)
    {
        let s = &live.stats["products"][id];
        let kind = live.kinds.get(id).cloned().unwrap_or_default();
        let pct = if *cap > 0 {
            *stored as f64 / *cap as f64 * 100.0
        } else {
            0.0
        };
        let bar_color = if pct >= 90.0 {
            "var(--warning)"
        } else {
            "var(--accent)"
        };
        body.push_str(&format!(
            "<tr><td><a href=\"/product?id={}\">{}</a></td><td>{}</td><td class=\"num\">{}</td><td><span class=\"num\">{} / {}</span> {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
            esc(id),
            esc(name),
            esc(&kind),
            fmt_num(&json!(qty)),
            fmt_num(&json!(stored)),
            fmt_num(&json!(cap)),
            fillbar(pct, bar_color, 70),
            flow_pair(&s["produced"]),
            flow_pair(&s["consumed"]),
            delta_chip(*net_tm, "TM"),
            delta_chip(*net_lm, "LM"),
            prepare_form(session, "PinToggleCmd", json!({"productToToggle": id}), if *pinned { "Unpin" } else { "Pin" }, false),
        ));
    }
    body.push_str("</tbody></table>");
    if pages > 1 {
        let mut pager = String::new();
        if page > 0 {
            pager.push_str(&format!(
                "<a class=\"btn btn--secondary btn--sm\" href=\"/products?f={}&q={}&sort={}&page={}\">← Prev</a> ",
                esc(facet),
                url_enc(&q),
                url_enc(sort),
                page - 1
            ));
        }
        if page + 1 < pages {
            pager.push_str(&format!(
                "<a class=\"btn btn--secondary btn--sm\" href=\"/products?f={}&q={}&sort={}&page={}\">Next →</a>",
                esc(facet),
                url_enc(&q),
                url_enc(sort),
                page + 1
            ));
        }
        body.push_str(&format!("<div class=\"pager\">{pager}</div>"));
    }
    body.push_str("</div>");
    body.push_str(TABLE_JS);
    // Rail context: the facet switcher with counts.
    let facet_link = |f: &str, label: &str, n: usize| {
        format!(
            "<a class=\"navitem anchor{}\" href=\"/products?f={}{}\">{}<span class=\"count\">{}</span></a>",
            if facet == f { " active" } else { "" },
            f,
            if sort != "name" { format!("&sort={}", url_enc(sort)) } else { String::new() },
            label,
            n
        )
    };
    let mut ctx = String::from("<div class=\"ctx\">FACETS</div>");
    ctx.push_str(&facet_link("all", "All", counts.0));
    ctx.push_str(&facet_link("pinned", "Pinned", counts.1));
    ctx.push_str(&facet_link("issues", "In issues", counts.2));
    ctx.push_str(&facet_link("fluid", "Fluid", counts.3));
    ctx.push_str(&facet_link("loose", "Loose", counts.4));
    ctx.push_str(&facet_link("countable", "Countable", counts.5));
    Response::html(
        200,
        layout_ctx(state, session, "Products", "Products", &ctx, &body),
    )
}

fn sources_table(src: &Value, pid: &str) -> String {
    let mut h = String::new();
    let names = &src["names"];
    for side in ["produced", "consumed"] {
        let rows = &src["products"][pid][side]["rows"];
        h.push_str(&format!(
            "<h3>{}</h3>",
            if side == "produced" {
                "Produced by"
            } else {
                "Consumed by"
            }
        ));
        if let Some(arr) = rows.as_array() {
            if arr.is_empty() {
                h.push_str("<div class=\"empty\">None this period.</div>");
            }
            h.push_str("<table class=\"table--compact\"><thead><tr><th>Source</th><th>Detail</th><th class=\"num\">Quantity</th></tr></thead><tbody>");
            for r in arr {
                let (a, b) = match r["type"].as_str() {
                    Some("recipe") => (
                        names[sv(&r["machine"])]
                            .as_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| sv(&r["machine"])),
                        names[sv(&r["recipe"])]
                            .as_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| sv(&r["recipe"])),
                    ),
                    Some("entity") => (sv(&r["entity"]), "entity (aggregate)".to_string()),
                    Some("fuel") => ("Vehicles & ships".to_string(), "fuel use".to_string()),
                    _ => (sv(&r["type"]), String::new()),
                };
                h.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                    esc(&a),
                    esc(&b),
                    fmt_num(&r["quantity"])
                ));
            }
            h.push_str("</tbody></table>");
        } else {
            h.push_str("<div class=\"empty\">No data for this period.</div>");
        }
    }
    h
}

fn page_product(state: &State, session: &Session, req: &Request) -> Response {
    let pid = match req.query.get("id") {
        Some(id) => id.clone(),
        None => return Response::redirect("/products"),
    };
    let c = client(state);
    let live = match fetch_live(&c) {
        Ok(l) => l,
        Err(e) => return game_unreachable(state, session, &e),
    };
    let name = live.names.get(&pid).cloned().unwrap_or_else(|| pid.clone());
    let s = &live.stats["products"][&pid];
    let pinned = live.pinned_ids.iter().any(|p| p == &pid);
    let mut body = page_head(
        "Monitor",
        &name,
        "Where this product comes from, where it goes, and how long the stock lasts.",
        &prepare_form(
            session,
            "PinToggleCmd",
            json!({"productToToggle": pid}),
            if pinned { "Unpin it" } else { "Pin it" },
            false,
        ),
    );
    body.push_str(&contract_strip(
        &format!("One product in depth · {}", pid),
        "Mostly informational — the pin toggle is the one action",
        "Live game state; the sources tables come from the game's own API and can be absent for products with no recorded flows",
    ));
    if !s.is_null() {
        // V2.3: tiles carry icons; produced/consumed are absolutes
        // (neutral pairs); Net is a difference — delta chips only,
        // never a "0/0" string; Stored/Cap gains the fill bar.
        let stored = s["stored"].as_i64().unwrap_or(0);
        let cap = s["storageCapacity"].as_i64().unwrap_or(0);
        let pct = if cap > 0 {
            stored as f64 / cap as f64 * 100.0
        } else {
            0.0
        };
        let net_tm = crate::flow_at(&s["produced"], 0) - crate::flow_at(&s["consumed"], 0);
        let net_lm = crate::flow_at(&s["produced"], 1) - crate::flow_at(&s["consumed"], 1);
        body.push_str("<div class=\"stats2\">");
        body.push_str(&stat2_tile(
            "box",
            "Quantity",
            &fmt_num(&s["quantity"]),
            "",
            false,
        ));
        body.push_str(&stat2_tile(
            "layers",
            "Stored / cap",
            &format!(
                "{} <span class=\"u\">/ {}</span>",
                fmt_num(&s["stored"]),
                fmt_num(&s["storageCapacity"])
            ),
            &fillbar(
                pct,
                if pct >= 90.0 {
                    "var(--warning)"
                } else {
                    "var(--accent)"
                },
                140,
            ),
            false,
        ));
        body.push_str(&stat2_tile(
            "chart",
            "Produced TM/LM",
            &flow_pair(&s["produced"]),
            "this month / last month",
            false,
        ));
        body.push_str(&stat2_tile(
            "chart",
            "Consumed TM/LM",
            &flow_pair(&s["consumed"]),
            "this month / last month",
            false,
        ));
        body.push_str(&stat2_tile(
            "chart",
            "Net",
            &format!("{} {}", delta_chip(net_tm, "TM"), delta_chip(net_lm, "LM")),
            "",
            false,
        ));
        body.push_str("</div>");
    }
    // Main column: history chart + table, then sources per period.
    let mut main = String::new();
    let hist = &live.history["products"][&pid];
    if let Some(q) = hist["quantity"].as_array() {
        let pts: Vec<f64> = q.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
        main.push_str("<div class=\"card\" id=\"history\"><div class=\"cardhead\"><h2>Monthly history</h2></div>");
        if pts.len() >= 2 {
            main.push_str(&svg_sparkline_big(&pts, "var(--accent)"));
            main.push_str(&format!(
                "<div class=\"caption\">{} → {} over {} months{}</div>",
                fmt_num(&q[0]),
                fmt_num(&q[q.len() - 1]),
                q.len(),
                if !s.is_null() && crate::is_seasonal(&live, &pid, s) {
                    " · seasonal — judged on long-window balance"
                } else {
                    ""
                },
            ));
        }
        main.push_str("<table class=\"table--compact\"><thead><tr><th>Month (oldest → now)</th><th class=\"num\">Produced</th><th class=\"num\">Consumed</th><th class=\"num\">Quantity</th></tr></thead><tbody>");
        for (i, qty) in q.iter().enumerate() {
            main.push_str(&format!(
                "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
                if i + 1 == q.len() { "current".to_string() } else { format!("-{}", q.len() - 1 - i) },
                fmt_num(&hist["produced"][i]),
                fmt_num(&hist["consumed"][i]),
                fmt_num(qty),
            ));
        }
        main.push_str("</tbody></table></div>");
    }
    if hist["quantity"].as_array().is_none() {
        // No-history empty state (DESIGN V3 §1.3): name the gap and
        // the remedy instead of the section silently not existing.
        main.push_str("<div class=\"card\" id=\"history\"><div class=\"cardhead\"><h2>Monthly history</h2></div>\
            <div class=\"empty\">No monthly history for this product yet. History accrues as the game records month-over-month flows; \
            pinning the product also starts its stored-snapshot trend (right rail) from the next snapshot you take.</div></div>");
    }
    for period in ["thisMonth", "lastMonth"] {
        match get_json(&c, &format!("/api/v1/products/sources?ids={pid}&period={period}")) {
            Ok(src) => {
                main.push_str(&format!(
                    "<div class=\"card\"{}><div class=\"cardhead\"><h2>Sources — {}</h2></div>",
                    if period == "thisMonth" { " id=\"sources\"" } else { "" },
                    if period == "thisMonth" { "this month" } else { "last month" }
                ));
                main.push_str(&sources_table(&src, &pid));
                main.push_str("</div>");
            }
            // D3 honesty (DESIGN V3 §1.3): a named gap with its
            // meaning, never the raw transport error. The game's
            // sources view has no rows for products it doesn't track
            // flows for (Dirt is the standing example).
            Err(_) => main.push_str(&format!(
                "<div class=\"card\"{}><div class=\"cardhead\"><h2>Sources — {}</h2></div>\
                 <div class=\"empty\">Producer / consumer detail isn't available for this product {} — the game's sources view didn't answer for it. \
                 That is normal for products with no recorded machine flows (imported, mined or dumped products among them). \
                 The balances and history on this page are unaffected.</div></div>",
                if period == "thisMonth" { " id=\"sources\"" } else { "" },
                if period == "thisMonth" { "this month" } else { "last month" },
                if period == "thisMonth" { "this month" } else { "last month" }
            )),
        }
    }
    // Rail: stored-snapshot trend as a real sparkline.
    let mut rail = String::new();
    rail.push_str(&format!(
        "<div class=\"card\"><div class=\"cardhead\"><h2>Game pin</h2></div><p>{} on the game's pinned list.</p><p>{}</p></div>",
        if pinned { "Currently pinned" } else { "Not pinned" },
        prepare_form(
            session,
            "PinToggleCmd",
            json!({"productToToggle": pid}),
            if pinned { "Unpin it" } else { "Pin it" },
            false,
        ),
    ));
    if let Ok(st) = store(state) {
        if let Ok(rows) = st.trend(&name) {
            let mine: Vec<_> = rows.iter().filter(|r| r.7 == name).collect();
            if !mine.is_empty() {
                let pts: Vec<f64> = mine.iter().map(|r| r.2 as f64).collect();
                rail.push_str(&format!(
                    "<div class=\"card\"><div class=\"cardhead\"><h2>Stored snapshots</h2></div>{}<div class=\"caption\">Quantity across {} snapshot(s): {} → {}</div></div>",
                    svg_sparkline(&pts, "var(--accent)"),
                    mine.len(),
                    mine.first().map(|r| r.2).unwrap_or(0),
                    mine.last().map(|r| r.2).unwrap_or(0),
                ));
            }
        }
    }
    body.push_str(&format!(
        "<div class=\"detail\"><div>{}</div><div>{}</div></div>",
        main, rail
    ));
    let ctx = "<div class=\"ctx\">ON THIS PAGE</div>".to_string()
        + &rail_link("#sources", "Sources")
        + &rail_link("#history", "Monthly history")
        + &rail_link("/products", "All products");
    Response::html(
        200,
        layout_ctx(state, session, &name, "Products", &ctx, &body),
    )
}

// ---------------------------------------------------------------------------
// Actions — the SPEC §12 two-phase command flow
// ---------------------------------------------------------------------------

// Command policy (2026-11-02, the solo-play build-out): every
// command in the captured catalogue may be prepared — through the
// curated forms or the generic schema form — EXCEPT the commands
// the catalogue marks "excluded" (today: GameConsoleCmd, arbitrary
// console access). The §12 confirm step (exact JSON, single-use
// token, audit log) is the control; the catalogue's per-command
// status records how far each command has been verified.

fn describe_command(cmd_type: &str, args: &Value) -> String {
    match cmd_type {
        "SetSimPauseStateCmd" => {
            if args["isPaused"].as_bool().unwrap_or(false) {
                "Pause the game".into()
            } else {
                "Resume the game".into()
            }
        }
        "GameSpeedChangeCmd" => format!(
            "Set game speed to {}×",
            args["newSpeedMultiplier"].as_i64().unwrap_or(1)
        ),
        "ResearchStartCmd" => format!(
            "Start researching {}",
            pretty_research(args["nodeId"].as_str().unwrap_or(""))
        ),
        "ResearchStopCmd" => "Stop the current research".into(),
        "ResearchQueueDequeueCmd" => {
            let n = pretty_research(args["nodeId"].as_str().unwrap_or(""));
            if args["isEnqueue"].as_bool().unwrap_or(false) {
                format!("Add to the research queue: {n}")
            } else {
                format!("Remove from the research queue: {n}")
            }
        }
        "PinToggleCmd" => format!(
            "Toggle pin for {}",
            args["productToToggle"].as_str().unwrap_or("")
        ),
        "QuickTradeCmd" => format!(
            "Execute quick trade {}",
            args["tradeId"].as_str().unwrap_or("")
        ),
        "SetGlobalPriorityCmd" => format!(
            "Set global priority \u{2018}{}\u{2019} to {}",
            args["priorityId"].as_str().unwrap_or(""),
            args["priority"].as_i64().unwrap_or(0)
        ),
        "ToggleEnabledCmd" => format!(
            "Pause / resume entity #{} (toggles its enabled state)",
            args["entityId"].as_i64().unwrap_or(0)
        ),
        "MachineToggleRecipeActiveCmd" => format!(
            "Toggle recipe {} on machine #{}",
            args["recipeId"].as_str().unwrap_or(""),
            args["machineId"].as_i64().unwrap_or(0)
        ),
        "StorageSetProductCmd" => format!(
            "Set storage #{} to store {}",
            args["storageId"].as_i64().unwrap_or(0),
            args["productId"].as_str().unwrap_or("")
        ),
        "StorageDisableLogisticsToggleCmd" => {
            let side = if args["isInput"].as_bool().unwrap_or(false) {
                "input"
            } else {
                "output"
            };
            let verb = if args["isDisabled"].as_bool().unwrap_or(false) {
                "Disable"
            } else {
                "Enable"
            };
            format!(
                "{verb} {side} logistics on storage #{}",
                args["storageId"].as_i64().unwrap_or(0)
            )
        }
        "SetGeneralPriorityCmd" => format!(
            "Set general priority of entity #{} to {}",
            args["entityId"].as_i64().unwrap_or(0),
            args["priority"].as_i64().unwrap_or(0)
        ),
        _ => format!("Run {cmd_type}"),
    }
}

/// Non-default priorities card (CoiBridge census): entities whose
/// general priority differs from their prototype default. Lives on
/// Actions since DESIGN V3 (the Entities Network view is trains
/// only; priorities are a Workshop concern). Empty string when the
/// bridge is silent or nothing differs.
fn nondefault_priorities_card(state: &State) -> String {
    let (Some(census), Some(topo)) = (
        bridge_json(state, "/coi/v1/census"),
        bridge_json(state, "/coi/v1/topology"),
    ) else {
        return String::new();
    };
    let mut body = String::new();
    // Non-default priorities (CoiBridge v0.6.1 census).
    if let Some(prios) = census["priorities"].as_object() {
        if !prios.is_empty() {
            let mut entries: Vec<(&String, &Value)> = prios.iter().collect();
            entries.sort_by_key(|(k, _)| k.parse::<i64>().unwrap_or(0));
            body.push_str(&format!(
                        "<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Non-default priorities ({})</h2></div>\
                         <p class=\"sub\" style=\"padding:0 12px\">Entities whose general priority differs from their prototype default (CoiBridge census).</p>\
                         <table class=\"table--compact\"><thead><tr><th>Entity</th><th>Type</th><th>Position</th><th>General priority</th></tr></thead><tbody>",
                        entries.len()
                    ));
            for (eid, pv) in entries {
                let idn = eid.parse::<i64>().unwrap_or(0);
                let ent = topo["entities"]
                    .as_array()
                    .and_then(|a| a.iter().find(|e| e["id"].as_i64() == Some(idn)));
                let (proto, pos) = match ent {
                    Some(e) => (
                        sv(&e["proto"]),
                        format!(
                            "{},{}",
                            e["pos"][0].as_i64().unwrap_or(0),
                            e["pos"][1].as_i64().unwrap_or(0)
                        ),
                    ),
                    None => ("—".to_string(), "—".to_string()),
                };
                body.push_str(&format!(
                    "<tr><td>#{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    idn,
                    esc(&proto),
                    esc(&pos),
                    vi(pv),
                ));
            }
            body.push_str("</tbody></table></div>");
        }
    }
    body
}

fn page_actions(state: &State, session: &Session) -> Response {
    let live = fetch_live(&client(state)).ok();
    let mut body = page_head(
        "Workshop",
        "Actions",
        "The commands you reach for most — each one prepared, confirmed by you, then applied and logged.",
        "",
    );
    body.push_str(&contract_strip(
        "Hand-picked commands + your non-default priorities",
        "Actionable — nothing on this page applies without the confirm step",
        "Works from the command catalogue; the priorities table needs CoiBridge's census",
    ));
    // Non-default priorities live here now (DESIGN V3 §2.3): the
    // Network view of Entities is trains-only; priority state is a
    // Workshop concern. Same data, same read as before.
    body.push_str(&nondefault_priorities_card(state));
    body.push_str("<div class=\"callout callout--info\">Every command goes: \
        prepare (you see the exact JSON) → confirm → the game executes it → the result lands in the \
        audit log below. Nothing is ever sent without the confirmation step.</div>");
    match &live {
        Some(l) => {
            body.push_str(&controls_html(session, l));
        }
        None => body.push_str(&degraded_game(
            "controls unavailable — the game API did not answer",
        )),
    }
    body.push_str(&format!(
        "<div class=\"card\" id=\"prio\"><div class=\"cardhead\"><h2>Global priorities</h2></div>\
         <details class=\"howit\"><summary>How this form works</summary><p>Sets one of the game's global priorities by id (SetGlobalPriorityCmd). \
         The API does not publish the valid ids, so this is a plain entry form: a wrong id makes the \
         game return failure and changes nothing — attempts are audit-logged either way, and \
         confirmed ids will be listed here as they're verified.</p></details>\
         <form method=\"post\" action=\"/actions/prepare-priority\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <div class=\"form-grid\">\
         <div class=\"field\"><label for=\"gp-id\">Priority id</label><input id=\"gp-id\" type=\"text\" name=\"priorityId\" required></div>\
         <div class=\"field\"><label for=\"gp-val\">Value</label><input id=\"gp-val\" type=\"text\" name=\"priority\" value=\"0\" required></div>\
         <div class=\"field\"><label>&nbsp;</label><button type=\"submit\">Prepare…</button></div></div></form></div>",
        esc(&session.csrf)
    ));
    if bridge_present(state) {
        body.push_str("<div class=\"panel\" id=\"fix\"><h2 style=\"margin-top:0\">Fix actions — entity-targeted ✅ unlocked by CoiBridge</h2>\
            <p>The bridge's entity directory is live, so the companion can address individual machines \
            and storages. The controls live where they're useful: on each <a href=\"/issues\">issue's detail \
            page</a> (the exact storages and machines involved in that issue) and on the \
            <a href=\"/entities\">Entities page</a> (every storage and machine). Everything still goes \
            through prepare → confirm → audit.</p>\
            <table><tr><th>Action</th><th>Game command</th><th>Status</th></tr>\
            <tr><td>Set a machine's recipe on/off</td><td>MachineToggleRecipeActiveCmd</td><td>✅ issue + Entities pages</td></tr>\
            <tr><td>Pause / resume a machine or building</td><td>ToggleEnabledCmd</td><td>✅ issue + Entities pages</td></tr>\
            <tr><td>Set a machine's general priority</td><td>SetGeneralPriorityCmd</td><td>✅ Entities page</td></tr>\
            <tr><td>Set a storage's product</td><td>StorageSetProductCmd</td><td>✅ Entities page</td></tr>\
            <tr><td>Storage import/export logistics on/off</td><td>StorageDisableLogisticsToggleCmd</td><td>✅ issue + Entities pages</td></tr>\
            <tr><td>Upgrade a belt / pipe</td><td>—</td><td class=\"lock\">🔒 no vanilla command exists (SPEC §12.7) — in-game only</td></tr>\
            <tr><td>Flip a pipe's direction</td><td>—</td><td class=\"lock\">🔒 not a thing in CoI: pipe flow is pressure-driven (SPEC §12 A7)</td></tr>\
            </table></div>");
    } else {
        body.push_str("<div class=\"panel\" id=\"fix\"><h2 style=\"margin-top:0\">Fix actions — locked for now</h2>\
            <p>These need a <b>numeric entity id</b> for the specific machine or storage. The game's web API \
            deliberately serves no entity directory (checked live: sources rows carry machine <i>types</i>, \
            not instances), so the companion cannot address an individual machine yet. The CoiBridge mod's \
            census route is the unlock (SPEC §3). The commands themselves exist and are verified:</p>\
            <table><tr><th>Action</th><th>Game command</th><th>Status</th></tr>\
            <tr><td>Set a machine's recipe on/off</td><td>MachineSetRecipeActiveCmd</td><td class=\"lock\">🔒 needs entity directory</td></tr>\
            <tr><td>Pause / resume a machine or building</td><td>ToggleEnabledCmd</td><td class=\"lock\">🔒 needs entity directory</td></tr>\
            <tr><td>Set general / power priority</td><td>SetGeneralPriorityCmd · SetElectricityGenerationPriorityCmd</td><td class=\"lock\">🔒 needs entity directory</td></tr>\
            <tr><td>Set a storage's product / import-export</td><td>StorageSetProductCmd · StorageSetSliderStepCmd</td><td class=\"lock\">🔒 needs entity directory</td></tr>\
            <tr><td>Upgrade a belt / pipe</td><td>—</td><td class=\"lock\">🔒 no vanilla command exists (SPEC §12.7)</td></tr>\
            </table></div>");
    }
    body.push_str("<div class=\"card tablecard\" id=\"audit\"><div class=\"cardhead\"><h2>Audit log</h2><span class=\"right\"><a href=\"/log\">Full activity log →</a></span></div>");
    match store(state) {
        Ok(st) => match st.audit_recent(20) {
            Ok(rows) if !rows.is_empty() => {
                body.push_str("<table class=\"table--compact\"><thead><tr><th>When</th><th>Action</th><th>Detail</th><th>Outcome</th></tr></thead><tbody>");
                for (ts, action, detail) in rows {
                    let outcome = if detail.contains("FAILED") {
                        badge("critical", "failed")
                    } else if detail.contains("success") {
                        badge("ok", "success")
                    } else {
                        badge("paused", "recorded")
                    };
                    body.push_str(&format!(
                        "<tr><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        fmt_clock(ts as u64),
                        esc(&action),
                        esc(&detail),
                        outcome
                    ));
                }
                body.push_str("</tbody></table>");
            }
            _ => body.push_str("<div class=\"empty\">No commands have been run yet — confirmed actions land here.</div>"),
        },
        Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
    }
    body.push_str("</div>");
    Response::html(200, layout(state, session, "Actions", &body))
}

fn do_prepare(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let cmd_type = req.form.get("type").cloned().unwrap_or_default();
    let Some(info) = CATALOGUE.iter().find(|c| c.name == cmd_type) else {
        return Response::text(400, "Unknown command.");
    };
    if info.status == "excluded" {
        return Response::text(
            400,
            "That command is excluded by policy (see the Catalogue).",
        );
    }
    let args: Value = match req
        .form
        .get("args")
        .and_then(|a| serde_json::from_str(a).ok())
    {
        Some(v) => v,
        None => return Response::text(400, "Bad command arguments."),
    };
    let mut summary = describe_command(&cmd_type, &args);
    match cmd_type.as_str() {
        "ResearchStartCmd" | "ResearchQueueDequeueCmd" => {
            let nid = args["nodeId"].as_str().unwrap_or("");
            if !research_ids().iter().any(|x| x == nid) {
                return Response::text(400, "Unknown research node id.");
            }
            if cmd_type == "ResearchQueueDequeueCmd" && !args["isEnqueue"].is_boolean() {
                return Response::text(400, "isEnqueue must be true or false.");
            }
        }
        "PinToggleCmd" => {
            let pid = args["productToToggle"].as_str().unwrap_or("");
            match fetch_live(&client(state)) {
                Ok(live) => {
                    if !live.names.contains_key(pid) {
                        return Response::text(400, "Unknown product id.");
                    }
                    let name = live
                        .names
                        .get(pid)
                        .cloned()
                        .unwrap_or_else(|| pid.to_string());
                    summary = if live.pinned_ids.iter().any(|p| p == pid) {
                        format!("Unpin {name} from the pinned list")
                    } else {
                        format!("Pin {name} to the pinned list")
                    };
                }
                Err(e) => return game_unreachable(state, session, &e),
            }
        }
        "SetGlobalPriorityCmd" => {
            let idok = args["priorityId"]
                .as_str()
                .map(|s| {
                    !s.is_empty()
                        && s.len() <= 64
                        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
                .unwrap_or(false);
            let pok = args["priority"]
                .as_i64()
                .map(|p| (-100..=100).contains(&p))
                .unwrap_or(false);
            if !idok || !pok {
                return Response::text(
                    400,
                    "Bad priority id or value (id: letters/digits/underscore, max 64; value: -100..100).",
                );
            }
        }
        "ToggleEnabledCmd" => {
            if !matches!(args["entityId"].as_i64(), Some(i) if i > 0) {
                return Response::text(400, "entityId must be a positive integer.");
            }
        }
        "MachineToggleRecipeActiveCmd" => {
            let mid = matches!(args["machineId"].as_i64(), Some(i) if i > 0);
            let rid = args["recipeId"]
                .as_str()
                .map(|s| {
                    !s.is_empty()
                        && s.len() <= 80
                        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
                .unwrap_or(false);
            if !mid || !rid {
                return Response::text(400, "Bad machineId or recipeId.");
            }
        }
        "StorageSetProductCmd" => {
            let sid = matches!(args["storageId"].as_i64(), Some(i) if i > 0);
            let pid = args["productId"].as_str().unwrap_or("");
            match fetch_live(&client(state)) {
                Ok(live) => {
                    if !sid || !live.names.contains_key(pid) {
                        return Response::text(400, "Bad storageId or unknown productId.");
                    }
                }
                Err(e) => return game_unreachable(state, session, &e),
            }
        }
        "StorageDisableLogisticsToggleCmd" => {
            let sid = matches!(args["storageId"].as_i64(), Some(i) if i > 0);
            if !sid || !args["isInput"].is_boolean() || !args["isDisabled"].is_boolean() {
                return Response::text(400, "Bad storage logistics arguments.");
            }
        }
        "SetGeneralPriorityCmd" => {
            let eid = matches!(args["entityId"].as_i64(), Some(i) if i > 0);
            let pok = args["priority"]
                .as_i64()
                .map(|p| (-100..=100).contains(&p))
                .unwrap_or(false);
            if !eid || !pok {
                return Response::text(400, "Bad entityId or priority (value: -100..100).");
            }
        }
        _ => {}
    }
    // Entity pages pass a human summary naming the exact machine or
    // storage; it replaces the generated one (the args JSON is still
    // shown verbatim on the confirmation page).
    if let Some(l) = req.form.get("label") {
        let l = l.trim();
        if !l.is_empty() && l.len() <= 240 {
            summary = l.to_string();
        }
    }
    finish_prepare(state, session, &cmd_type, &args, &summary)
}

/// Second half of the §12 flow, shared by every prepare endpoint:
/// store the single-use confirmation token and render the page that
/// shows the exact JSON before anything is sent.
fn finish_prepare(
    state: &State,
    session: &Session,
    cmd_type: &str,
    args: &Value,
    summary: &str,
) -> Response {
    let token = rand_hex(16);
    lock_map(&state.prepared).insert(
        token.clone(),
        Prepared {
            cmd_type: cmd_type.to_string(),
            args: args.clone(),
            summary: summary.to_string(),
            created: now_unix(),
        },
    );
    let envelope = json!({"type": cmd_type, "args": args});
    let state_line = match get_json(&client(state), "/api/v1/game-info") {
        Ok(g) => format!(
            "Currently: {} · {} · speed {}×",
            sv(&g["date"]),
            if g["isPaused"].as_bool().unwrap_or(false) {
                "PAUSED"
            } else {
                "running"
            },
            g["speed"]
        ),
        Err(e) => format!("Warning: could not read current game state ({e})"),
    };
    // The confirm sheet (DESIGN §3.9): deliberately unlike every
    // other panel — warning band, plain-language summary, current
    // game state, the exact JSON, Run + Cancel as peers, expiry
    // caption. Irreversible commands (deconstruction / destruction
    // class) get the danger band and the danger button.
    let irreversible = ["Deconstruction", "Destroy", "Discard", "Remove"]
        .iter()
        .any(|k| cmd_type.contains(k));
    let body = format!(
        "<div class=\"confirm\"><div class=\"{}\">{}</div><div class=\"inner\">\
         <p class=\"summary\">{}</p>\
         <p class=\"meta\">{}</p>\
         <div class=\"caption\">Exact command sent to the game</div>\
         <pre>{}</pre>\
         <form method=\"post\" action=\"/actions/apply\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"token\" value=\"{}\">\
         <div class=\"actions\"><button class=\"{}\" type=\"submit\">Run it</button>\
         <a class=\"btn btn--secondary\" href=\"/actions\">Cancel</a></div></form>\
         <p class=\"caption\">Single-use · expires in 2 minutes · the result is written to the audit log.</p></div></div>",
        if irreversible { "band band--danger" } else { "band" },
        if irreversible {
            "This cannot be undone."
        } else {
            "This will change your game."
        },
        esc(summary),
        esc(&state_line),
        esc(&serde_json::to_string_pretty(&envelope).unwrap()),
        esc(&session.csrf),
        esc(&token),
        if irreversible { "btn--danger" } else { "" },
    );
    Response::html(200, layout(state, session, "Confirm command", &body))
}

/// Generic prepare form for ANY catalogue command, generated from
/// its captured argument schema. This is what turns the catalogue
/// from documentation into coverage: a command family does not
/// need a hand-built page before it can be run through the §12
/// flow — the exact JSON is shown at the confirm step either way.
fn page_prepare(state: &State, session: &Session, req: &Request) -> Response {
    let Some(name) = req.query.get("cmd").cloned() else {
        // No silent redirects (DESIGN V3 §1.10): the landing page
        // explains what happened via a flash line.
        return Response::redirect("/catalogue?flash=prepare-needs-command");
    };
    let Some(info) = CATALOGUE.iter().find(|c| c.name == name) else {
        return Response::html(
            404,
            layout(
                state,
                session,
                "Unknown command",
                "<h1>Unknown command</h1><p><a href=\"/catalogue\">Catalogue</a></p>",
            ),
        );
    };
    if info.status == "excluded" {
        return Response::html(
            200,
            layout_nav(state, session,
                info.name, "Catalogue",
                &format!(
                    "{}<div class=\"callout callout--danger\"><b>Excluded by policy.</b> {}</div><p><a class=\"btn btn--secondary\" href=\"/catalogue\">← Catalogue</a></p>",
                    page_head("Workshop", info.name, info.purpose, &badge("critical", "excluded")),
                    esc(info.note),
                ),
            ),
        );
    }
    let status_badge = match info.status {
        "verified" => badge("ok", "verified"),
        _ => badge("info", "wired"),
    };
    let mut body = page_head(
        "Workshop",
        info.purpose,
        &format!("{} · {}", info.name, info.domain),
        &status_badge,
    );
    body.push_str(
        "<div class=\"card\"><p class=\"sub\">Generic form generated from the captured schema. \
         Nothing is sent yet: the next step shows the exact JSON for confirmation.</p>",
    );
    if !info.note.is_empty() {
        body.push_str(&format!(
            "<div class=\"callout callout--info\">{}</div>",
            esc(info.note)
        ));
    }
    body.push_str(&format!(
        "<form method=\"post\" action=\"/actions/prepare-generic\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"{}\">",
        esc(&session.csrf),
        esc(info.name),
    ));
    if info.spec.is_empty() {
        body.push_str("<p class=\"sub\">This command takes no arguments.</p>");
    }
    for (an, at) in info.spec {
        let optional = at.contains("or null");
        body.push_str(&format!(
            "<div class=\"field\"><label for=\"a_{}\">{}</label><input id=\"a_{}\" name=\"arg_{}\"><div class=\"hint\">{}{}</div></div>",
            esc(an),
            esc(an),
            esc(an),
            esc(an),
            esc(at),
            if optional { " · optional" } else { "" },
        ));
    }
    body.push_str(&format!(
        "<div class=\"field\"><label for=\"plabel\">Summary label</label><input id=\"plabel\" name=\"label\" maxlength=\"240\" placeholder=\"{}\"><div class=\"hint\">optional — shown on the confirmation page</div></div>\
         <p><button type=\"submit\">Prepare…</button> <a class=\"btn btn--secondary\" href=\"/catalogue\">Back to catalogue</a></p></form></div>",
        esc(info.purpose),
    ));
    Response::html(
        200,
        layout_nav(state, session, info.purpose, "Catalogue", &body),
    )
}

fn coerce_arg(jtype: &str, raw: &str) -> Option<Value> {
    let t = jtype.to_lowercase();
    if t.starts_with("bool") {
        return raw.parse::<bool>().ok().map(Value::Bool);
    }
    if t.starts_with("int") {
        return raw.parse::<i64>().ok().map(|n| json!(n));
    }
    if t.starts_with("number") {
        return raw.parse::<f64>().ok().map(|n| json!(n));
    }
    if t.starts_with("string") {
        return Some(Value::String(raw.to_string()));
    }
    // Structured or proto-typed arguments: JSON first (objects,
    // numbers, quoted strings), else the bare string.
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return Some(v);
    }
    Some(Value::String(raw.to_string()))
}

fn do_prepare_generic(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let cmd_type = req.form.get("type").cloned().unwrap_or_default();
    let Some(info) = CATALOGUE.iter().find(|c| c.name == cmd_type) else {
        return Response::text(400, "Unknown command.");
    };
    if info.status == "excluded" {
        return Response::text(
            400,
            "That command is excluded by policy (see the Catalogue).",
        );
    }
    let mut args = json!({});
    for (an, at) in info.spec {
        let raw = req
            .form
            .get(&format!("arg_{an}"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if raw.is_empty() {
            if at.contains("or null") {
                args[an] = Value::Null;
                continue;
            }
            return Response::text(400, &format!("Missing argument: {an}"));
        }
        let Some(v) = coerce_arg(at, &raw) else {
            return Response::text(400, &format!("Argument {an} must be of type {at}."));
        };
        args[an] = v;
    }
    let label = req.form.get("label").cloned().unwrap_or_default();
    let summary = if label.trim().is_empty() || label.len() > 240 {
        format!("{} ({})", info.purpose, cmd_type)
    } else {
        label.trim().to_string()
    };
    finish_prepare(state, session, &cmd_type, &args, &summary)
}

fn do_prepare_storage_product(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let sid: i64 = match req.form.get("storageId").and_then(|s| s.parse().ok()) {
        Some(i) if i > 0 => i,
        _ => return Response::text(400, "storageId must be a positive integer."),
    };
    let pid = req.form.get("productId").cloned().unwrap_or_default();
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => return game_unreachable(state, session, &e),
    };
    let pname = match live.names.get(&pid) {
        Some(n) => n.clone(),
        None => return Response::text(400, "Unknown product id."),
    };
    // Warn with the storage's current contents when the bridge can see it.
    let mut note = String::new();
    if let Some(storages) = bridge_json(state, "/coi/v1/storages") {
        if let Some(rows) = storages["storages"].as_array() {
            if let Some(row) = rows.iter().find(|r| r["id"].as_i64() == Some(sid)) {
                let qty = row["quantity"].as_i64().unwrap_or(0);
                let cur = row["product"].as_str().unwrap_or("");
                if qty > 0 {
                    note = format!(
                        " Warning: storage #{sid} currently holds {qty} of {cur} — the game will refuse the change or clear it.",
                    );
                }
            }
        }
    }
    let summary = format!("Set storage #{sid} to store {pname} ({pid}).{note}");
    finish_prepare(
        state,
        session,
        "StorageSetProductCmd",
        &json!({"storageId": sid, "productId": pid}),
        &summary,
    )
}

fn do_prepare_entity_priority(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let eid: i64 = match req.form.get("entityId").and_then(|s| s.parse().ok()) {
        Some(i) if i > 0 => i,
        _ => return Response::text(400, "entityId must be a positive integer."),
    };
    let prio: i64 = match req.form.get("priority").and_then(|s| s.parse().ok()) {
        Some(p) if (-100..=100).contains(&p) => p,
        _ => return Response::text(400, "priority must be an integer in -100..100."),
    };
    let label = req
        .form
        .get("label")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() <= 200)
        .unwrap_or_else(|| format!("entity #{eid}"));
    let summary = format!("Set general priority of {label} to {prio}.");
    finish_prepare(
        state,
        session,
        "SetGeneralPriorityCmd",
        &json!({"entityId": eid, "priority": prio}),
        &summary,
    )
}

fn do_apply(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let token = req.form.get("token").cloned().unwrap_or_default();
    let prepared = lock_map(&state.prepared).remove(&token);
    let prepared = match prepared {
        Some(p) if now_unix() - p.created <= 120 => p,
        _ => {
            return Response::html(
                400,
                layout(state, session,
                    "Expired",
                    "<h1>Confirmation expired</h1><p>The prepared command was not run (confirmations last 2 minutes and are single-use). <a href=\"/actions\">Back to Actions</a></p>",
                ),
            )
        }
    };
    let envelope = json!({"type": prepared.cmd_type, "args": prepared.args});
    // Game context for the activity record: fetched before the
    // command goes out (after is the fallback — the game may have
    // stopped answering between the two).
    let before = get_json(&client(state), "/api/v1/game-info").unwrap_or(Value::Null);
    let result = client(state).post_json("/command", &envelope);
    let (http, result_text, ok) = match &result {
        Ok(r) => {
            let txt = String::from_utf8_lossy(&r.body).to_string();
            (r.status, txt, r.status == 200)
        }
        Err(e) => (0, e.clone(), false),
    };
    let after_g = get_json(&client(state), "/api/v1/game-info").unwrap_or(Value::Null);
    // §12's guarantee IS the audit trail: what reached the game must
    // leave a record. Audit/activity write failures are therefore
    // never swallowed (audit D1) — they are collected here and
    // surfaced on the result page as an error state; the user must
    // never see a clean success for an unaudited apply.
    let mut audit_err: Option<String> = None;
    match store(state) {
        Ok(st) => {
            if let Err(e) = st.audit_add(
                now_unix() as i64,
                &prepared.cmd_type,
                &format!(
                    "{} · args {} · HTTP {} · {}",
                    prepared.summary,
                    prepared.args,
                    http,
                    if ok { "success" } else { "FAILED" }
                ),
            ) {
                audit_err = Some(format!("audit row: {e}"));
            }
            // Addendum XVI: the same attempt as a rich activity event —
            // exact envelope, game response, session user, game context.
            let ctx = if before.is_object() {
                &before
            } else {
                &after_g
            };
            if let Err(e) = st.activity_add(&ActivityEvent {
                event_key: format!("apply-{}-{}", now_unix(), rand_hex(8)),
                recorded_ts_unix: now_unix() as i64,
                source: "companion".to_string(),
                actor: Some(session.user.clone()),
                category: Some("command".to_string()),
                action: prepared.cmd_type.clone(),
                summary: Some(prepared.summary.clone()),
                game_id: ctx["gameId"].as_str().map(|s| s.to_string()),
                game_date: ctx["date"].as_str().map(|s| s.to_string()),
                game_tick: ctx["tick"].as_i64(),
                outcome: Some(if ok { "success" } else { "failed" }.to_string()),
                payload_json: Some(
                    json!({
                        "envelope": envelope,
                        "httpStatus": http,
                        "responseBody": result_text,
                    })
                    .to_string(),
                ),
            }) {
                audit_err.get_or_insert_with(|| format!("activity row: {e}"));
            }
        }
        Err(e) => {
            audit_err = Some(format!("audit store unavailable: {e}"));
        }
    }
    if let Some(e) = &audit_err {
        eprintln!(
            "serve: §12 apply of {} by {} left NO audit record: {e}",
            prepared.cmd_type, session.user
        );
    }
    let after = if after_g.is_object() {
        let g = &after_g;
        format!(
            "Game now: {} · {} · speed {}×",
            sv(&g["date"]),
            if g["isPaused"].as_bool().unwrap_or(false) {
                "PAUSED"
            } else {
                "running"
            },
            g["speed"]
        )
    } else {
        String::new()
    };
    let audit_banner = match &audit_err {
        Some(e) => format!(
            "<div class=\"callout callout--danger\"><b>AUDIT WRITE FAILED.</b> {} Recording the attempt in the §12 audit log failed: {} — <b>this action is missing from the audit trail</b>. Do not re-run it blindly; check the game state first.</div>",
            if ok {
                "The command WAS applied to the game."
            } else {
                "The command did not succeed, and the attempt itself could not be recorded either."
            },
            esc(e),
        ),
        None => String::new(),
    };
    let body = format!(
        "{}{}\
         <div class=\"{}\"><b>{}</b> — {}<p style=\"margin-top:8px\"><a href=\"/log\">See it in the Log →</a></p></div>\
         <div class=\"card\"><div class=\"cardhead\"><h2>Game response</h2><span class=\"right badge {}\">HTTP {}</span></div><pre>{}</pre><p class=\"sub\">{}</p>\
         <p><a class=\"btn\" href=\"/actions\">Back to Actions</a> <a class=\"btn btn--secondary\" href=\"/\">Overview</a></p></div>",
        page_head("Game", "Command result", &prepared.summary, ""),
        audit_banner,
        if ok && audit_err.is_none() {
            "callout callout--ok"
        } else {
            "callout callout--danger"
        },
        if ok && audit_err.is_none() {
            "Command executed"
        } else if ok {
            "Command executed — NOT audited"
        } else {
            "Command failed"
        },
        esc(&prepared.summary),
        if ok { "badge--ok" } else { "badge--critical" },
        http,
        esc(&result_text),
        esc(&after),
    );
    Response::html(200, layout(state, session, "Command result", &body))
}

// ---------------------------------------------------------------------------
// Place — blueprint placement via QoSBuilder job files (Addendum XVII)
// ---------------------------------------------------------------------------
//
// The §12 shape, with a file as the apply step: Prepare renders the
// exact job (blueprint, anchor, rotation, target gameId, expiry)
// and Confirm — the only step that writes anything — writes ONE
// job file into the QoS jobs folder. QoSBuilder (the in-game mod,
// ships disabled) validates it and places it as construction sites
// at the next in-game day end. There is deliberately no other
// write path: no file, no placement.

const BLUEPRINT_LIBRARY_JSON: &str = include_str!("../data/blueprint-library.json");

/// How long a confirmed job stays valid (the mod refuses expired
/// jobs): long enough for the day to turn, short enough that a
/// stale job can never fire on another evening or another save.
const JOB_TTL_SECONDS: u64 = 1800;
/// Item cap written into every job (QoSBuilder's own config can
/// only lower its side further).
const JOB_MAX_ITEMS: i64 = 500;

fn blueprint_library() -> Value {
    serde_json::from_str(BLUEPRINT_LIBRARY_JSON)
        .unwrap_or_else(|_| json!({"count": 0, "entries": [], "testing_status": ""}))
}

fn blueprint_entry(slug: &str) -> Option<Value> {
    blueprint_library()["entries"]
        .as_array()?
        .iter()
        .find(|e| e["slug"].as_str() == Some(slug))
        .cloned()
}

/// The QoS jobs folder confirmed jobs are written into. Config:
/// `[builder] jobs_dir` in coi.toml (the shared search order,
/// `crate::config_candidates`); default `%APPDATA%\Captain of
/// Industry\QoS\jobs` on Windows. On Linux/macOS the APPDATA
/// variable is honoured when set (a Proton prefix exports it);
/// otherwise the fallback is a `qos-jobs` folder inside the OS
/// data dir, and `[builder] jobs_dir` is the documented way to
/// point at the game's prefix — see INSTALL.md.
fn builder_jobs_dir(db_path: &str) -> std::path::PathBuf {
    for path in crate::config_candidates(db_path) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let mut in_builder = false;
            for raw in text.lines() {
                let line = raw.split('#').next().unwrap_or("").trim();
                if line.starts_with('[') {
                    in_builder = line == "[builder]";
                    continue;
                }
                if in_builder {
                    if let Some((key, val)) = line.split_once('=') {
                        if key.trim() == "jobs_dir" {
                            let v = val.trim().trim_matches('"');
                            if !v.is_empty() {
                                return v.into();
                            }
                        }
                    }
                }
            }
            break; // first coi.toml found wins, like [productivity]
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return std::path::PathBuf::from(appdata)
                .join("Captain of Industry")
                .join("QoS")
                .join("jobs");
        }
    }
    if let Some(dir) = crate::os_data_dir() {
        return dir.join("qos-jobs");
    }
    std::path::PathBuf::from("qos-jobs")
}

/// ISO-8601 UTC ("2026-11-05T18:30:00Z") — the job file's
/// timestamp format (QoSBuilder parses it back with
/// DateTime.TryParse, RoundtripKind).
fn iso_utc(unix: u64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm), UTC — same as
    // fmt_clock, with seconds and the ISO shape.
    let days = (unix / 86400) as i64;
    let secs = unix % 86400;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Job-file escaping — the ONE rule shared with QoSBuilder's
/// canonical payload (Addendum XVII): '"' and '\' escaped, control
/// chars as \u00xx (lowercase hex), everything else raw UTF-8.
fn job_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The canonical job payload: fixed field order, compact, the
/// sha256 field excluded. QoSBuilder rebuilds exactly this from
/// the parsed job and refuses the job if the hash differs — so
/// this string IS what the user confirms.
#[allow(clippy::too_many_arguments)]
fn job_canonical(
    job_id: &str,
    created_utc: &str,
    expires_utc: &str,
    game_id: &str,
    user: &str,
    name: &str,
    blueprint: &str,
    anchor: (i64, i64, i64),
    rotation_deg: i64,
    apply_configuration: bool,
    max_items: i64,
) -> String {
    format!(
        "{{\"jobId\":\"{}\",\"schema\":1,\"createdUtc\":\"{}\",\"expiresUtc\":\"{}\",\"gameId\":\"{}\",\"companionUser\":\"{}\",\"blueprintName\":\"{}\",\"blueprintString\":\"{}\",\"anchor\":{{\"x\":{},\"y\":{},\"z\":{}}},\"rotationDeg\":{},\"applyConfiguration\":{},\"limits\":{{\"maxItems\":{}}}}}",
        job_escape(job_id),
        job_escape(created_utc),
        job_escape(expires_utc),
        job_escape(game_id),
        job_escape(user),
        job_escape(name),
        job_escape(blueprint),
        anchor.0,
        anchor.1,
        anchor.2,
        rotation_deg,
        apply_configuration,
        max_items
    )
}

fn job_sha256_hex(canonical: &str) -> String {
    let mut h = Sha256::new();
    h.update(canonical.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn new_job_id() -> String {
    let h = rand_hex(16);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn page_place(state: &State, session: &Session, req: &Request) -> Response {
    let lib = blueprint_library();
    let jobs_dir = builder_jobs_dir(&state.db_path);
    let ind_want = req.query.get("ind").cloned().unwrap_or_default();
    let mut h = page_head(
        "Workshop",
        "Place",
        "Place a whole layout — buildings, belts and pipes — as one confirmed job.",
        "",
    );
    h.push_str(&contract_strip(
        "Blueprint placement jobs, executed in-game by QoSBuilder",
        "Actionable — Prepare shows the exact job; only Confirm writes it",
        "The library below is bundled with the tool and works offline. Jobs need the game running and QoSBuilder enabled in the game's mod settings",
    ));
    h.push_str("<details class=\"howit\"><summary>How placement works</summary><p>Pick a layout from the pack library, say where its origin lands (the anchor tile — a blueprint's own coordinates are relative to it), and <b>Prepare</b>. You then see the exact job — blueprint, anchor, rotation, target game, expiry — and only <b>Confirm</b> writes anything: one job file into the QoS jobs folder. <b>QoSBuilder</b> (the in-game mod) validates it — untampered, unexpired, pinned to the running game, item-capped — and places it as ordinary construction sites at the next in-game day end, <b>if the mod is enabled</b> in the game's mod settings. No file, no placement; a stale or foreign job can never fire.</p></details>");
    h.push_str("<div class=\"card\">");
    match get_json(&client(state), "/api/v1/game-info") {
        Ok(g) => h.push_str(&format!(
            "<p class=\"sub\">Game reachable: {} · {} · gameId {}</p>",
            esc(&sv(&g["date"])),
            if g["isPaused"].as_bool() == Some(true) {
                "paused"
            } else {
                "running"
            },
            esc(&sv(&g["gameId"]))
        )),
        Err(e) => h.push_str(&format!(
            "<p class=\"sub\">⚠ The game is unreachable ({}) — the library still lists, but Prepare will refuse until the game answers.</p>",
            esc(&e)
        )),
    }
    h.push_str(&format!(
        "<p class=\"sub\">Jobs folder: <code>{}</code></p><p class=\"sub\">Pack: {}</p></div>",
        esc(&jobs_dir.display().to_string()),
        esc(&sv(&lib["testing_status"]))
    ));
    // Industries as collapse groups (V2.1/V2.2): the first group
    // opens by default, or the one ?ind= names; the rail carries
    // the industry facets.
    let mut ctx = String::from("<div class=\"ctx\">INDUSTRIES</div>");
    if let Some(entries) = lib["entries"].as_array() {
        let mut modules: Vec<String> = Vec::new();
        for e in entries {
            let m = sv(&e["module"]);
            if !modules.contains(&m) {
                modules.push(m);
            }
        }
        for (mi, module) in modules.iter().enumerate() {
            let group: Vec<&Value> = entries
                .iter()
                .filter(|e| sv(&e["module"]) == *module)
                .collect();
            ctx.push_str(&format!(
                "<a class=\"navitem anchor{}\" href=\"/place?ind={}\">{}<span class=\"count\">{}</span></a>",
                if ind_want == *module { " active" } else { "" },
                url_enc(module),
                esc(module),
                group.len()
            ));
            let open = if ind_want.is_empty() {
                mi == 0
            } else {
                ind_want == *module
            };
            h.push_str(&format!(
                "<details class=\"indgroup\" id=\"ind-{}\"{}><summary>{}<span class=\"count\">{} layouts</span></summary><table class=\"table--compact\"><thead><tr><th>Blueprint</th><th class=\"num\">Items</th><th>Research gate</th><th>Place</th></tr></thead><tbody>",
                module
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                    .collect::<String>(),
                if open { " open" } else { "" },
                esc(module),
                group.len()
            ));
            for e in group {
                let kinds = &e["kinds"];
                let breakdown = [
                    ("buildings", kinds["buildings"].as_i64().unwrap_or(0)),
                    ("transports", kinds["transports"].as_i64().unwrap_or(0)),
                    ("trees", kinds["trees"].as_i64().unwrap_or(0)),
                    ("other", kinds["other"].as_i64().unwrap_or(0)),
                ]
                .iter()
                .filter(|(_, n)| *n > 0)
                .map(|(label, n)| format!("{n} {label}"))
                .collect::<Vec<_>>()
                .join(" · ");
                let gate = if e["research_gate"].is_null() {
                    "—".to_string()
                } else {
                    sv(&e["research_gate"])
                };
                h.push_str(&format!(
                    "<tr><td><b>{}</b><br><span class=\"sub mono\">{}</span></td><td class=\"num\">{} <span class=\"sub\">({})</span></td><td>{}</td>\
                     <td><form method=\"post\" action=\"/place/prepare\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><input type=\"hidden\" name=\"slug\" value=\"{}\">\
                     anchor x <input name=\"ax\" size=\"5\" value=\"0\"> y <input name=\"ay\" size=\"5\" value=\"0\"> z <input name=\"az\" size=\"3\" value=\"0\"> \
                     <select name=\"rotation\"><option value=\"0\">0°</option><option value=\"90\">90°</option><option value=\"180\">180°</option><option value=\"270\">270°</option></select> \
                     <button class=\"btn btn--sm btn--ghost\" type=\"submit\">Prepare…</button></form></td></tr>",
                    esc(&sv(&e["name"])),
                    esc(&sv(&e["slug"])),
                    e["item_count"].as_i64().unwrap_or(0),
                    breakdown,
                    esc(&gate),
                    session.csrf,
                    esc(&sv(&e["slug"])),
                ));
            }
            h.push_str("</tbody></table></details>");
        }
    }
    Response::html(
        200,
        layout_ctx(state, session, "Place a blueprint", "Place", &ctx, &h),
    )
}

fn do_place_prepare(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let slug = req.form.get("slug").cloned().unwrap_or_default();
    let Some(entry) = blueprint_entry(&slug) else {
        return Response::html(
            400,
            layout(state, session,
                "Unknown blueprint",
                "<h1>Unknown blueprint</h1><div class=\"panel\"><p>That slug is not in the embedded pack library.</p><p><a href=\"/place\">← Back to the library</a></p></div>",
            ),
        );
    };
    let parse_i = |k: &str| req.form.get(k).and_then(|v| v.trim().parse::<i64>().ok());
    let (Some(ax), Some(ay), Some(az)) = (parse_i("ax"), parse_i("ay"), parse_i("az")) else {
        return Response::html(
            400,
            layout(state, session,
                "Bad anchor",
                "<h1>Bad anchor</h1><div class=\"panel\"><p>Anchor x, y and z must all be whole numbers.</p><p><a href=\"/place\">← Back to the library</a></p></div>",
            ),
        );
    };
    let rotation = parse_i("rotation").unwrap_or(0);
    if ax.abs() > 100_000
        || ay.abs() > 100_000
        || az.abs() > 1_000
        || ![0, 90, 180, 270].contains(&rotation)
    {
        return Response::html(
            400,
            layout(state, session,
                "Bad placement",
                "<h1>Bad placement</h1><div class=\"panel\"><p>Anchor out of sane range, or rotation not a quarter turn (0/90/180/270).</p><p><a href=\"/place\">← Back to the library</a></p></div>",
            ),
        );
    }
    // The job is pinned to the RUNNING game: prepare refuses when
    // the game cannot be reached or its id cannot be confirmed.
    let game = match get_json(&client(state), "/api/v1/game-info") {
        Ok(g) => g,
        Err(e) => {
            return Response::html(
                502,
                layout(state, session,
                    "Game unreachable",
                    &format!(
                        "<h1>Prepare refused — game unreachable</h1><div class=\"panel\"><p>{}</p><p>A placement job must be pinned to the running game's id; without game-info there is nothing safe to prepare. Start the game, run <code>web_api_start</code>, and try again.</p><p><a href=\"/place\">← Back to the library</a></p></div>",
                        esc(&e)
                    ),
                ),
            )
        }
    };
    let Some(game_id) = game["gameId"].as_str().map(|s| s.to_string()) else {
        return Response::html(
            502,
            layout(state, session,
                "No game id",
                "<h1>Prepare refused — no game id</h1><div class=\"panel\"><p>game-info answered without a gameId; a job cannot be pinned. Nothing was prepared.</p><p><a href=\"/place\">← Back to the library</a></p></div>",
            ),
        );
    };
    let now = now_unix();
    let job_id = new_job_id();
    let created = iso_utc(now);
    let expires = iso_utc(now + JOB_TTL_SECONDS);
    let name = entry["name"].as_str().unwrap_or(&slug).to_string();
    let blueprint_string = entry["string"].as_str().unwrap_or_default().to_string();
    let canonical = job_canonical(
        &job_id,
        &created,
        &expires,
        &game_id,
        &session.user,
        &name,
        &blueprint_string,
        (ax, ay, az),
        rotation,
        true,
        JOB_MAX_ITEMS,
    );
    let sha = job_sha256_hex(&canonical);
    let file_json = format!(
        "{},\"sha256\":\"{}\"}}",
        &canonical[..canonical.len() - 1],
        sha
    );
    let kinds = &entry["kinds"];
    let summary = format!(
        "Place '{}' ({} items: {} buildings, {} transports) at ({ax}, {ay}, {az}), rotation {rotation}°",
        name,
        entry["item_count"].as_i64().unwrap_or(0),
        kinds["buildings"].as_i64().unwrap_or(0),
        kinds["transports"].as_i64().unwrap_or(0),
    );
    let token = rand_hex(16);
    lock_map(&state.prepared_jobs).insert(
        token.clone(),
        PreparedJob {
            job_id: job_id.clone(),
            file_json: file_json.clone(),
            summary: summary.clone(),
            game_id: game_id.clone(),
            game_date: game["date"].as_str().map(|s| s.to_string()),
            game_tick: game["tick"].as_i64(),
            created: now,
        },
    );
    let pretty = serde_json::from_str::<Value>(&file_json)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
        .unwrap_or_else(|_| file_json.clone());
    let jobs_dir = builder_jobs_dir(&state.db_path);
    let body = format!(
        "{}\
         <div class=\"confirm\"><div class=\"band\">This will change your game — a whole layout, placed by the QoSBuilder mod.</div><div class=\"inner\">\
         <p class=\"summary\">{}</p>\
         <p class=\"meta\">Job <code>{}</code> · target gameId <code>{}</code> (game date {}) · confirmed jobs are written to <code>{}</code> and <b>expire at {}</b> ({} min) — after that the mod refuses them and you prepare again.</p>\
         <p class=\"sub\">Confirming writes exactly this job file (shown pretty-printed; the file itself is the compact canonical form). Its sha256 is <code>{}</code> — QoSBuilder recomputes it in-game and refuses the job if a single byte differs. Nothing else is sent to the game; the mod places at the <b>next in-game day end</b>, and only while its <code>enabled</code> setting is on. Surfaces and decals in a string are not placed in v1.</p>\
         <div class=\"caption\">Exact job file written on confirm</div>\
         <pre>{}</pre>\
         <form method=\"post\" action=\"/place/confirm\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><input type=\"hidden\" name=\"token\" value=\"{}\">\
         <div class=\"actions\"><button class=\"danger\" type=\"submit\">Write the job file</button> <a class=\"btn btn--secondary\" href=\"/place\">Cancel</a></div></form>\
         <p class=\"caption\">This confirmation expires in two minutes.</p></div></div>",
        page_head("Game", "Confirm placement", "One job file, written only on confirm — the mod does the placing.", ""),
        esc(&summary),
        esc(&job_id),
        esc(&game_id),
        esc(game["date"].as_str().unwrap_or("?")),
        esc(&jobs_dir.display().to_string()),
        esc(&fmt_clock(now + JOB_TTL_SECONDS)),
        JOB_TTL_SECONDS / 60,
        esc(&sha),
        esc(&pretty),
        session.csrf,
        token,
    );
    Response::html(200, layout(state, session, "Confirm placement", &body))
}

fn do_place_confirm(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let token = req.form.get("token").cloned().unwrap_or_default();
    let job = lock_map(&state.prepared_jobs).remove(&token);
    let job = match job {
        Some(j) if now_unix() - j.created <= 120 => j,
        _ => {
            return Response::html(
                400,
                layout(state, session,
                    "Confirmation expired",
                    "<h1>Confirmation expired</h1><div class=\"panel\"><p>That placement confirmation is unknown or older than two minutes, so nothing was written. Prepare the placement again.</p><p><a href=\"/place\">← Back to the library</a></p></div>",
                ),
            )
        }
    };
    let dir = builder_jobs_dir(&state.db_path);
    let path = dir.join(format!("{}.json", job.job_id));
    let write = (|| -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create jobs folder: {e}"))?;
        let tmp = dir.join(format!("{}.json.tmp", job.job_id));
        std::fs::write(&tmp, job.file_json.as_bytes()).map_err(|e| format!("write job: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("rename job: {e}"))?;
        Ok(())
    })();
    // Same §12 discipline as do_apply (audit D1): the confirm's
    // audit/activity rows are part of the guarantee, so a failed
    // write is collected and surfaced on the outcome page — never
    // a silent `let _`, in either outcome branch.
    let outcome = if write.is_ok() {
        "job-written"
    } else {
        "write-failed"
    };
    let mut audit_err: Option<String> = None;
    match store(state) {
        Ok(st) => {
            if let Err(e) = st.audit_add(
                now_unix() as i64,
                "PlaceBlueprintJob",
                &format!(
                    "{} · job {} · game {} · {}",
                    job.summary, job.job_id, job.game_id, outcome
                ),
            ) {
                audit_err = Some(format!("audit row: {e}"));
            }
            if let Err(e) = st.activity_add(&ActivityEvent {
                event_key: format!("place-{}-{}", now_unix(), rand_hex(8)),
                recorded_ts_unix: now_unix() as i64,
                source: "companion".into(),
                actor: Some(session.user.clone()),
                category: Some("placement".into()),
                action: "PlaceBlueprintJob".into(),
                summary: Some(job.summary.clone()),
                game_id: Some(job.game_id.clone()),
                game_date: job.game_date.clone(),
                game_tick: job.game_tick,
                outcome: Some(outcome.into()),
                payload_json: Some(job.file_json.clone()),
            }) {
                audit_err.get_or_insert_with(|| format!("activity row: {e}"));
            }
        }
        Err(e) => {
            audit_err = Some(format!("audit store unavailable: {e}"));
        }
    }
    if let Some(e) = &audit_err {
        eprintln!(
            "serve: place-confirm of job {} left NO audit record: {e}",
            job.job_id
        );
    }
    let audit_banner = match &audit_err {
        Some(e) => format!(
            "<div class=\"callout callout--danger\"><b>AUDIT WRITE FAILED.</b> The job file WAS written — QoSBuilder will act on it at the next in-game day end — but recording it in the audit log failed: {} — <b>this placement is missing from the audit trail</b>.</div>",
            esc(e),
        ),
        None => String::new(),
    };
    let audit_note = match &audit_err {
        Some(e) => format!(
            "Nothing was placed. The audit write failed too ({}) — so this failure is NOT in the audit log either.",
            esc(e)
        ),
        None => "Nothing was placed and the failure is in the audit log.".to_string(),
    };
    match write {
        Ok(()) => Response::html(
            200,
            layout(state, session,
                "Job written",
                &format!(
                    "<h1>Job written</h1>{}<div class=\"panel\"><p><b>{}</b></p>\
                     <p class=\"sub\">File: <code>{}</code> · job <code>{}</code></p>\
                     <p>QoSBuilder picks it up at the <b>next in-game day end</b> — if the mod is installed and its <code>enabled</code> setting is on. Watch the <a href=\"/log\">Log page</a> for <code>JobClaimed</code> / <code>JobScheduled</code> (or a refusal with its reason), then take a snapshot: the census change is the honest placement verdict, since the batch is per-item in-game.</p>\
                     <p><a href=\"/place\">← Place another</a></p></div>",
                    audit_banner,
                    esc(&job.summary),
                    esc(&path.display().to_string()),
                    esc(&job.job_id),
                ),
            ),
        ),
        Err(e) => Response::html(
            500,
            layout(state, session,
                "Job write failed",
                &format!(
                    "<h1>Job write failed</h1><div class=\"panel\"><p>{}</p><p>{} Check the jobs folder path (shown on the Place page; override it with <code>[builder] jobs_dir</code> in coi.toml if the companion runs somewhere other than the game PC).</p><p><a href=\"/place\">← Back to the library</a></p></div>",
                    esc(&e),
                    audit_note,
                ),
            ),
        ),
    }
}

// ---------------------------------------------------------------------------
// Designate — terrain designations via QoSBuilder job files (schema 2)
// ---------------------------------------------------------------------------
//
// The designation sibling of Place (DESIGNATION-DESIGN-NOTE.md,
// SPEC Addendum XX): Prepare plans a flatten/dump pad from the
// bridge's live terrain region — or a removal picked from the
// live /designations read-back, never free-typed origins — and
// renders the exact schema-2 job; Confirm, the only step that
// writes anything, writes ONE sealed job file into the same QoS
// jobs folder. QoSBuilder 0.2.0 executes it as a single
// AddTerrainDesignationsCmd / RemoveDesignationsCmd batch at the
// next in-game day end. The mod attests "scheduled"; the verdict
// is the read-back diff against /designations — exact with
// bridge 0.6.6 corners, band-level (labelled partial) without
// them. No file, no designation.

/// Cell cap written into every designation job (the mod's own
/// config can only lower its side further; design note §2.3).
const DESIGNATE_MAX_CELLS: i64 = 200;

fn designate_proto_label(proto: &str) -> &'static str {
    match proto {
        "MiningDesignator" => "Mining",
        "DumpingDesignator" => "Dumping",
        "LevelDesignator" => "Levelling",
        _ => "Designation",
    }
}

/// The live designation read-back (bridge 0.6.5+; corners with
/// 0.6.6). None = the bridge did not answer.
fn fetch_designations(state: &State) -> Option<Vec<crate::designate::ExistingDesignation>> {
    bridge_json(state, "/coi/v1/designations").map(|v| crate::designate::existing_from_json(&v))
}

/// The read-back verdict for one confirmed designation job file.
/// The mod attests "scheduled", so this diff against the CURRENT
/// /designations is the only landing evidence there is — it is
/// recomputed from the live read-back on every render, and it
/// never claims more than the read-back shows.
fn designation_verdict_panel(
    heading: &str,
    file_json: &str,
    existing: Option<&[crate::designate::ExistingDesignation]>,
) -> String {
    let mut h = String::new();
    h.push_str(&format!(
        "<div class=\"card\"><div class=\"cardhead\"><h2>{}</h2></div>",
        esc(heading)
    ));
    let Ok(job) = serde_json::from_str::<Value>(file_json) else {
        h.push_str("<p class=\"sub\">The recorded job payload could not be parsed; no verdict can be computed.</p></div>");
        return h;
    };
    let Some(existing) = existing else {
        h.push_str("<p class=\"sub\">Read-back unavailable — the bridge did not answer just now, so no verdict can be computed. The job stands as written; revisit when the game is up.</p></div>");
        return h;
    };
    match job["jobType"].as_str() {
        Some("designations-add") => {
            let proto = job["protoId"].as_str().unwrap_or("");
            let cells: Vec<crate::designate::Cell> = job["cells"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| {
                            Some(crate::designate::Cell::new(
                                c["x"].as_i64()?,
                                c["y"].as_i64()?,
                                [
                                    c["origin"].as_i64()?,
                                    c["plusX"].as_i64()?,
                                    c["plusXY"].as_i64()?,
                                    c["plusY"].as_i64()?,
                                ],
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let d = crate::designate::diff_add(&cells, proto, existing);
            let total = d.total();
            let (word, text) = if total == 0 {
                (
                    "paused",
                    "No cells in the job — nothing to verify.".to_string(),
                )
            } else if d.all_landed() && d.band_only == 0 {
                (
                    "ok",
                    "LANDED — every cell reads back exactly as confirmed (origin, proto and all four corners).".to_string(),
                )
            } else if d.all_landed() {
                (
                    "ok",
                    format!(
                        "LANDED (band-level for {} of {}) — every cell reads back, but those cells are verified by height band only: this bridge serves no corners (0.6.6 serves them). Two different corner assignments can share a band, so this verdict is partial.",
                        d.band_only, total
                    ),
                )
            } else if d.landed() > 0 {
                (
                    "warning",
                    format!(
                        "PARTIALLY LANDED — {} of {} cells read back as confirmed.",
                        d.landed(),
                        total
                    ),
                )
            } else {
                (
                    "info",
                    "SCHEDULED — verdict pending. Nothing reads back yet: QoSBuilder acts at the next in-game day end, and only while its <code>enabled</code> setting is on. This is not a placement claim — the panel recomputes from the live read-back on every visit.".to_string(),
                )
            };
            h.push_str(&format!(
                "<p>{} <b>{}</b></p>",
                badge(word, "verdict"),
                text
            ));
            h.push_str(&format!(
                "<p class=\"sub\">Read-back counts: exact {} · band-only {} · different designation there {} · nothing there {} — of {} cells.</p>",
                d.exact, d.band_only, d.different, d.absent, total
            ));
            let mut details: Vec<String> = Vec::new();
            for c in &cells {
                match crate::designate::cell_match(c, proto, existing) {
                    crate::designate::CellMatch::Different => {
                        let e = crate::designate::existing_at(existing, c.x, c.y);
                        details.push(format!(
                            "({}, {}) — a different designation sits there{}",
                            c.x,
                            c.y,
                            e.map(|e| format!(" ({} / {})", esc(&e.kind), esc(&e.proto)))
                                .unwrap_or_default()
                        ));
                    }
                    crate::designate::CellMatch::Absent => {
                        details.push(format!("({}, {}) — nothing designated there yet", c.x, c.y));
                    }
                    _ => {}
                }
                if details.len() >= 12 {
                    break;
                }
            }
            if !details.is_empty() {
                h.push_str("<ul>");
                for line in &details {
                    h.push_str(&format!("<li>{}</li>", line));
                }
                h.push_str("</ul>");
            }
        }
        Some("designations-remove") => {
            let origins: Vec<(i64, i64)> = job["origins"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|o| Some((o["x"].as_i64()?, o["y"].as_i64()?)))
                        .collect()
                })
                .unwrap_or_default();
            let still = crate::designate::diff_remove(&origins, existing);
            let n = origins.len();
            let (word, text) = if n == 0 {
                (
                    "paused",
                    "No origins in the job — nothing to verify.".to_string(),
                )
            } else if still == 0 {
                (
                    "ok",
                    format!(
                        "LANDED — all {} origins are clear; nothing is designated there now.",
                        n
                    ),
                )
            } else if still < n {
                (
                    "warning",
                    format!(
                        "PARTIALLY LANDED — {} of {} origins cleared; {} still designated.",
                        n - still,
                        n,
                        still
                    ),
                )
            } else {
                (
                    "info",
                    "SCHEDULED — verdict pending. Every origin is still designated: QoSBuilder acts at the next in-game day end, and only while its <code>enabled</code> setting is on. This is not a removal claim — the panel recomputes from the live read-back on every visit.".to_string(),
                )
            };
            h.push_str(&format!(
                "<p>{} <b>{}</b></p>",
                badge(word, "verdict"),
                text
            ));
            if still > 0 && still < n.max(1) {
                h.push_str("<ul>");
                for (x, y) in &origins {
                    if crate::designate::existing_at(existing, *x, *y).is_some() {
                        h.push_str(&format!("<li>({}, {}) — still designated</li>", x, y));
                    }
                }
                h.push_str("</ul>");
            }
        }
        _ => {
            h.push_str("<p class=\"sub\">Not a designation job payload.</p>");
        }
    }
    h.push_str("</div>");
    h
}

/// The verdict panel for the most recent confirmed designation
/// job, reconstructed from its activity record (the payload IS
/// the job file). Empty when no designation job was ever
/// confirmed through this companion.
fn latest_designation_panel(
    state: &State,
    existing: Option<&[crate::designate::ExistingDesignation]>,
) -> String {
    let Ok(st) = store(state) else {
        return String::new();
    };
    let Ok(rows) = st.activity_recent(Some("companion"), Some("PlaceDesignationJob"), 10, 0) else {
        return String::new();
    };
    let Some(row) = rows.iter().find(|r| {
        r.category.as_deref() == Some("designation") && r.outcome.as_deref() == Some("job-written")
    }) else {
        return String::new();
    };
    let Some(payload) = row.payload_json.as_deref() else {
        return String::new();
    };
    let is_designation = serde_json::from_str::<Value>(payload)
        .map(|v| v["jobType"].as_str().is_some())
        .unwrap_or(false);
    if !is_designation {
        return String::new();
    }
    let mut panel = designation_verdict_panel(
        "Latest designation job — read-back verdict",
        payload,
        existing,
    );
    if let Some(summary) = &row.summary {
        panel = panel.replacen(
            "</h2></div>",
            &format!("</h2></div><p class=\"sub\">{}</p>", esc(summary)),
            1,
        );
    }
    panel
}

fn page_designate(state: &State, session: &Session) -> Response {
    let mut body = page_head(
        "Workshop",
        "Designate",
        "Plan terrain designations — mining, dumping and levelling pads, and removals — as confirmed QoSBuilder jobs.",
        "",
    );
    body.push_str(&contract_strip(
        "Terrain designation pads and removals, planned from live terrain",
        "Actionable — every job is prepared, confirmed by you, then read back cell by cell for its verdict",
        "Scope, stated plainly: this page plans flat pads and removals. The full pit planner (graded ore benches) is not wired yet — it has no live per-column terrain feed to plan from. Bridge floors: live terrain needs CoiBridge 0.6.5+; exact verdicts need 0.6.6+",
    ));
    let game = match get_json(&client(state), "/api/v1/game-info") {
        Ok(g) => g,
        Err(e) => {
            body.push_str(&degraded_game(&e));
            body.push_str("<div class=\"card\"><p>Designation planning needs the live game: the job is pinned to its id, pads are planned from the bridge's live terrain, and removals are picked from the live read-back. Nothing can be prepared while it is down.</p></div>");
            return Response::html(200, layout(state, session, "Designate", &body));
        }
    };
    let jobs_dir = builder_jobs_dir(&state.db_path);
    body.push_str("<div class=\"card\">");
    body.push_str("<p>Say where a pad goes (a tile rectangle + a target height + a kind) and <b>Prepare</b>: the companion fetches the live terrain, snaps the rectangle outward to whole 4×4 cells, and shows the exact job — cells, volumes, edge-step warnings, replacements — before anything is written. Only <b>Confirm</b> writes: one schema-2 job file into the QoS jobs folder. <b>QoSBuilder</b> (0.2.0, the in-game mod) validates it — untampered, unexpired, pinned to the running game, cell-capped — and executes it as one batch at the next in-game day end, <b>if the mod is enabled</b>. The mod only attests <i>scheduled</i>; whether a designation actually landed is read back from the game and shown below — never assumed.</p>");
    body.push_str(&format!(
        "<p class=\"sub\">Game reachable: {} · {} · gameId {}</p>",
        esc(&sv(&game["date"])),
        if game["isPaused"].as_bool() == Some(true) {
            "paused"
        } else {
            "running"
        },
        esc(&sv(&game["gameId"]))
    ));
    body.push_str(&format!(
        "<p class=\"sub\">Jobs folder: <code>{}</code></p></div>",
        esc(&jobs_dir.display().to_string())
    ));

    let existing = fetch_designations(state);
    body.push_str(&latest_designation_panel(state, existing.as_deref()));

    // Pad planner form.
    body.push_str("<div class=\"card\" id=\"pad\"><div class=\"cardhead\"><h2>New pad</h2></div>");
    body.push_str(&format!(
        "<form method=\"post\" action=\"/designate/prepare\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\"><input type=\"hidden\" name=\"op\" value=\"pad\">\
         <div class=\"form-grid\">\
         <div class=\"field\"><label for=\"dg-x0\">x from</label><input id=\"dg-x0\" type=\"number\" name=\"x0\" required></div>\
         <div class=\"field\"><label for=\"dg-y0\">y from</label><input id=\"dg-y0\" type=\"number\" name=\"y0\" required></div>\
         <div class=\"field\"><label for=\"dg-x1\">x to (exclusive)</label><input id=\"dg-x1\" type=\"number\" name=\"x1\" required></div>\
         <div class=\"field\"><label for=\"dg-y1\">y to (exclusive)</label><input id=\"dg-y1\" type=\"number\" name=\"y1\" required></div>\
         <div class=\"field\"><label for=\"dg-t\">target height (whole tiles)</label><input id=\"dg-t\" type=\"number\" name=\"target\" required></div>\
         <div class=\"field\"><label for=\"dg-proto\">kind</label><select id=\"dg-proto\" name=\"proto\">\
         <option value=\"MiningDesignator\">Mining</option>\
         <option value=\"DumpingDesignator\">Dumping</option>\
         <option value=\"LevelDesignator\">Levelling</option></select></div>\
         <div class=\"field\"><label for=\"dg-name\">plan name</label><input id=\"dg-name\" type=\"text\" name=\"name\" maxlength=\"120\" placeholder=\"optional — shown on the confirmation page\"></div>\
         <div class=\"field\"><label>&nbsp;</label><button type=\"submit\">Prepare…</button></div></div></form>",
        session.csrf
    ));
    body.push_str("<p class=\"sub\">The rectangle snaps OUTWARD to whole cells (origins on the 4-grid). Prepare fetches the rectangle plus a one-cell margin of live terrain, levels every cell to the target, and lists every boundary vertex whose step to the terrain outside breaks the 0/±1 snapping rule — the game accepts such cells silently, so the warning is yours to read before confirming. Pit planning from deposits is not offered yet: the bridge serves no per-column rock layers, and a pit without the spike/purity gates would be a worse plan, not a faster one.</p></div>");

    // Current designations + remove flow (origins from read-back only).
    match &existing {
        None => body.push_str(&degraded_bridge("Designation read-back", "0.6.6")),
        Some(rows) => {
            body.push_str("<div class=\"card tablecard\" id=\"live\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Current designations</h2></div>");
            if rows.is_empty() {
                body.push_str("<div class=\"empty\">No mining, dumping or levelling designations are currently marked.</div>");
            } else {
                body.push_str(&format!(
                    "<form method=\"post\" action=\"/designate/prepare\" id=\"remove\">\
                     <input type=\"hidden\" name=\"csrf\" value=\"{}\"><input type=\"hidden\" name=\"op\" value=\"remove\">\
                     <table class=\"table--compact\"><thead><tr><th>Remove</th><th>Kind</th><th>Origin</th><th>Corners (origin, +X, +XY, +Y)</th><th>Target band</th><th>State</th><th class=\"num\">Jobs</th></tr></thead><tbody>",
                    session.csrf
                ));
                for (i, r) in rows.iter().enumerate() {
                    let kind_badge = match r.kind.as_str() {
                        "mining" => badge("info", "Mining"),
                        "dumping" => badge("warning", "Dumping"),
                        _ => badge("paused", "Other"),
                    };
                    let state_badge = if r.is_fulfilled {
                        badge("ok", "Fulfilled")
                    } else {
                        badge("starved", "Open")
                    };
                    let corners = match r.corners {
                        Some(c) => format!("[{}, {}, {}, {}]", c[0], c[1], c[2], c[3]),
                        None => "— (bridge &lt; 0.6.6)".to_string(),
                    };
                    let band = match r.band {
                        Some((lo, hi)) => format!("{}–{} tiles", lo, hi),
                        None => "—".to_string(),
                    };
                    body.push_str(&format!(
                        "<tr><td><input type=\"checkbox\" name=\"sel_{}\" value=\"{},{}\"></td><td>{} <span class=\"sub mono\">{}</span></td><td>({}, {})</td><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                        i,
                        r.x,
                        r.y,
                        kind_badge,
                        esc(&r.proto),
                        r.x,
                        r.y,
                        corners,
                        band,
                        state_badge,
                        r.jobs_assigned,
                    ));
                }
                body.push_str("</tbody></table>");
                body.push_str("<p style=\"padding:0 16px\"><button type=\"submit\">Prepare removal…</button></p>");
                body.push_str("<p class=\"sub\" style=\"padding:0 16px 12px\">Removal is prepared from this live read-back only — origins are never typed by hand, and a selection that no longer matches the read-back is refused at prepare. Remove clears whatever designation sits at an origin, of any kind.</p></form>");
            }
            body.push_str("</div>");
        }
    }
    Response::html(200, layout(state, session, "Designate", &body))
}

/// The shared confirm sheet for both designation families: the
/// §12 ceremony, identical in shape to /place's — band, summary,
/// meta, family details, the exact job file, one danger button.
#[allow(clippy::too_many_arguments)]
fn designate_confirm_response(
    state: &State,
    session: &Session,
    summary: &str,
    details_html: &str,
    job: &crate::designate::DesignationJob,
    job_id: &str,
    game_id: &str,
    game_date: &str,
    token: &str,
) -> Response {
    let pretty = serde_json::from_str::<Value>(&job.file_json)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
        .unwrap_or_else(|_| job.file_json.clone());
    let jobs_dir = builder_jobs_dir(&state.db_path);
    let expires_unix = now_unix() + JOB_TTL_SECONDS;
    let body = format!(
        "{}\
         <div class=\"confirm\"><div class=\"band\">This will change your game — terrain designations, executed by the QoSBuilder mod.</div><div class=\"inner\">\
         <p class=\"summary\">{}</p>\
         <p class=\"meta\">Job <code>{}</code> · target gameId <code>{}</code> (game date {}) · confirmed jobs are written to <code>{}</code> and <b>expire at {}</b> ({} min) — after that the mod refuses them and you prepare again.</p>\
         {}\
         <p class=\"sub\">Confirming writes exactly this job file (shown pretty-printed; the file itself is the compact canonical form). Its sha256 is <code>{}</code> — QoSBuilder recomputes it in-game and refuses the job if a single byte differs. Nothing else is sent to the game; the mod executes at the <b>next in-game day end</b>, and only while its <code>enabled</code> setting is on.</p>\
         <div class=\"caption\">Exact job file written on confirm</div>\
         <pre>{}</pre>\
         <form method=\"post\" action=\"/designate/confirm\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><input type=\"hidden\" name=\"token\" value=\"{}\">\
         <div class=\"actions\"><button class=\"danger\" type=\"submit\">Write the job file</button> <a class=\"btn btn--secondary\" href=\"/designate\">Cancel</a></div></form>\
         <p class=\"caption\">This confirmation expires in two minutes.</p></div></div>",
        page_head("Game", "Confirm designation", "One job file, written only on confirm — the mod does the designating.", ""),
        esc(summary),
        esc(job_id),
        esc(game_id),
        esc(game_date),
        esc(&jobs_dir.display().to_string()),
        esc(&fmt_clock(expires_unix)),
        JOB_TTL_SECONDS / 60,
        details_html,
        esc(&job.sha256),
        esc(&pretty),
        session.csrf,
        token,
    );
    Response::html(200, layout(state, session, "Confirm designation", &body))
}

fn do_designate_prepare(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    // The job is pinned to the RUNNING game: prepare refuses
    // when the game cannot be reached or its id cannot be
    // confirmed — the /place discipline, unchanged.
    let game = match get_json(&client(state), "/api/v1/game-info") {
        Ok(g) => g,
        Err(e) => {
            return Response::html(
                502,
                layout(state, session,
                    "Designation prepare refused — game unreachable",
                    &format!(
                        "<h1>Prepare refused — game unreachable</h1><div class=\"panel\"><p>{}</p><p>A designation job must be pinned to the running game's id; without game-info there is nothing safe to prepare. Start the game, run <code>web_api_start</code>, and try again.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
                        esc(&e)
                    ),
                ),
            )
        }
    };
    let Some(game_id) = game["gameId"].as_str().map(|s| s.to_string()) else {
        return Response::html(
            502,
            layout(state, session,
                "Designation prepare refused — no game id",
                "<h1>Prepare refused — no game id</h1><div class=\"panel\"><p>game-info answered without a gameId; a job cannot be pinned. Nothing was prepared.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
            ),
        );
    };
    let op = req.form.get("op").map(|s| s.as_str()).unwrap_or("pad");
    if op == "remove" {
        designate_prepare_remove(state, session, req, &game, &game_id)
    } else {
        designate_prepare_pad(state, session, req, &game, &game_id)
    }
}

fn designate_prepare_pad(
    state: &State,
    session: &Session,
    req: &Request,
    game: &Value,
    game_id: &str,
) -> Response {
    let parse_i = |k: &str| req.form.get(k).and_then(|v| v.trim().parse::<i64>().ok());
    let (Some(x0), Some(y0), Some(x1), Some(y1), Some(target)) = (
        parse_i("x0"),
        parse_i("y0"),
        parse_i("x1"),
        parse_i("y1"),
        parse_i("target"),
    ) else {
        return Response::html(
            400,
            layout(state, session,
                "Bad pad",
                "<h1>Bad pad</h1><div class=\"panel\"><p>The rectangle corners and the target height must all be whole numbers (corner heights are integer tiles in the game).</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
            ),
        );
    };
    let proto = req.form.get("proto").cloned().unwrap_or_default();
    if !crate::designate::ALLOWED_PROTOS.contains(&proto.as_str()) {
        return Response::html(
            400,
            layout(state, session,
                "Bad pad",
                "<h1>Bad pad</h1><div class=\"panel\"><p>Kind must be Mining, Dumping or Levelling (forestry designations are not offered).</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
            ),
        );
    }
    // Live terrain for the rectangle + a one-cell margin (the
    // margin is what the edge-step warnings are computed from).
    let path = format!(
        "/coi/v1/terrain/region?x0={}&y0={}&x1={}&y1={}",
        x0 - 4,
        y0 - 4,
        x1 + 4,
        y1 + 4
    );
    let grid =
        bridge_json(state, &path).and_then(|v| crate::designate::HeightGrid::from_region_json(&v));
    let Some(grid) = grid else {
        return Response::html(
            502,
            layout(state, session,
                "Designation prepare refused — bridge unavailable",
                "<h1>Prepare refused — bridge terrain unavailable</h1><div class=\"panel\"><p>The bridge did not serve a usable terrain region for that rectangle, so the pad cannot be planned from live heights. CoiBridge 0.6.5+ serves the region route. Nothing was prepared.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
            ),
        );
    };
    let plan = match crate::designate::plan_pad(
        &grid,
        (x0, y0, x1, y1),
        target,
        &proto,
        DESIGNATE_MAX_CELLS as usize,
    ) {
        Ok(p) => p,
        Err(e) => {
            return Response::html(
                400,
                layout(state, session,
                    "Plan refused",
                    &format!(
                        "<h1>Plan refused</h1><div class=\"panel\"><p>The pad planner refused: <b>{}</b></p><p>Nothing was prepared. Adjust the rectangle or target and prepare again.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
                        esc(&e.to_string())
                    ),
                ),
            )
        }
    };
    let name = req
        .form
        .get("name")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            format!(
                "Pad ({},{})-({},{}) @ {}",
                plan.footprint.0, plan.footprint.1, plan.footprint.2, plan.footprint.3, target
            )
        });
    // Replace warnings: Add replaces ANY designation at an
    // origin (and deletes a surface designation there).
    let existing = fetch_designations(state);
    let mut details = String::new();
    details.push_str(&format!(
        "<p><b>{} cells</b> of {} over tiles ({},{})–({},{}) at target height {} — cut {:.1} tiles³ ({:.0} m³), fill {:.1} tiles³ ({:.0} m³) against the live terrain.</p>",
        plan.cells.len(),
        designate_proto_label(&proto),
        plan.footprint.0,
        plan.footprint.1,
        plan.footprint.2,
        plan.footprint.3,
        target,
        plan.cut_tiles3,
        plan.cut_m3(),
        plan.fill_tiles3,
        plan.fill_m3(),
    ));
    if plan.warnings.is_empty() {
        details.push_str("<p class=\"sub\">Edge steps: every boundary vertex is within 0/±1 of the terrain just outside — no snapping warnings.</p>");
    } else {
        details.push_str("<div class=\"callout callout--warning\"><b>Edge-step warnings</b> — the pad's edge breaks the 0/±1 snapping rule at these vertices. The game accepts such cells silently and terrain physics settles the slope; read them before confirming:<ul>");
        for w in &plan.warnings {
            details.push_str(&format!(
                "<li>Vertex ({}, {}): pad target {} vs terrain {:.2} just outside — step {:+.2} tiles</li>",
                w.vertex.0, w.vertex.1, w.target, w.outside_height, w.step
            ));
        }
        details.push_str("</ul></div>");
    }
    match &existing {
        Some(rows) => {
            let replaced: Vec<&crate::designate::ExistingDesignation> = plan
                .cells
                .iter()
                .filter_map(|c| crate::designate::existing_at(rows, c.x, c.y))
                .collect();
            if !replaced.is_empty() {
                details.push_str("<div class=\"callout callout--warning\"><b>This pad replaces existing designations</b> — Add replaces whatever sits at an origin, of any kind, and deletes any surface designation on the cell:<ul>");
                for e in replaced {
                    details.push_str(&format!(
                        "<li>({}, {}) — currently {} (<span class=\"mono\">{}</span>)</li>",
                        e.x,
                        e.y,
                        esc(&e.kind),
                        esc(&e.proto)
                    ));
                }
                details.push_str("</ul></div>");
            }
        }
        None => {
            details.push_str("<p class=\"sub\">The current designations could not be read back just now, so replace warnings are unavailable on this sheet; the after-confirm verdict still diffs against the live read-back.</p>");
        }
    }
    details.push_str("<p class=\"sub\">Cells whose tiles are blocked by buildings are silently skipped by the game — the read-back verdict (shown after confirm and on the Designate page) is the honest check, not this sheet.</p>");

    let now = now_unix();
    let job_id = new_job_id();
    let created = iso_utc(now);
    let expires = iso_utc(now + JOB_TTL_SECONDS);
    let meta = crate::designate::JobMeta {
        job_id: &job_id,
        created_utc: &created,
        expires_utc: &expires,
        game_id,
        companion_user: &session.user,
        plan_name: &name,
    };
    let job = crate::designate::build_add_job(&meta, &proto, &plan.cells, DESIGNATE_MAX_CELLS);
    let summary = format!(
        "Designation pad '{}': {} {} cells → level {} over ({},{})-({},{})",
        name,
        plan.cells.len(),
        designate_proto_label(&proto),
        target,
        plan.footprint.0,
        plan.footprint.1,
        plan.footprint.2,
        plan.footprint.3
    );
    let token = rand_hex(16);
    lock_map(&state.prepared_jobs).insert(
        token.clone(),
        PreparedJob {
            job_id: job_id.clone(),
            file_json: job.file_json.clone(),
            summary: summary.clone(),
            game_id: game_id.to_string(),
            game_date: game["date"].as_str().map(|s| s.to_string()),
            game_tick: game["tick"].as_i64(),
            created: now,
        },
    );
    designate_confirm_response(
        state,
        session,
        &summary,
        &details,
        &job,
        &job_id,
        game_id,
        game["date"].as_str().unwrap_or("?"),
        &token,
    )
}

fn designate_prepare_remove(
    state: &State,
    session: &Session,
    req: &Request,
    game: &Value,
    game_id: &str,
) -> Response {
    // Origins arrive as sel_<i> = "x,y" checkbox values rendered
    // from the live read-back — there is no free-typed path.
    let mut origins: Vec<(i64, i64)> = Vec::new();
    for (k, v) in req.form.iter() {
        if !k.starts_with("sel_") {
            continue;
        }
        let Some((xs, ys)) = v.split_once(',') else {
            continue;
        };
        if let (Ok(x), Ok(y)) = (xs.trim().parse::<i64>(), ys.trim().parse::<i64>()) {
            origins.push((x, y));
        }
    }
    origins.sort();
    origins.dedup();
    if origins.is_empty() {
        return Response::html(
            400,
            layout(state, session,
                "Nothing selected",
                "<h1>Nothing selected</h1><div class=\"panel\"><p>Tick at least one designation in the live list, then prepare the removal.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
            ),
        );
    }
    if origins.len() as i64 > DESIGNATE_MAX_CELLS {
        return Response::html(
            400,
            layout(state, session,
                "Plan refused",
                &format!(
                    "<h1>Plan refused</h1><div class=\"panel\"><p>{} origins is over the {}-cell job cap. Split the removal and prepare again.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
                    origins.len(),
                    DESIGNATE_MAX_CELLS
                ),
            ),
        );
    }
    // The selection must still match the live read-back —
    // removing an origin from memory is how cells get cleared
    // blind; refuse instead.
    let Some(existing) = fetch_designations(state) else {
        return Response::html(
            502,
            layout(state, session,
                "Designation prepare refused — bridge unavailable",
                "<h1>Prepare refused — bridge read-back unavailable</h1><div class=\"panel\"><p>The bridge did not serve the current designations, so the selection cannot be checked against what is actually designated. Nothing was prepared.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
            ),
        );
    };
    let stale: Vec<(i64, i64)> = origins
        .iter()
        .copied()
        .filter(|&(x, y)| crate::designate::existing_at(&existing, x, y).is_none())
        .collect();
    if !stale.is_empty() {
        let list = stale
            .iter()
            .map(|(x, y)| format!("({}, {})", x, y))
            .collect::<Vec<_>>()
            .join(", ");
        return Response::html(
            400,
            layout(state, session,
                "Stale selection",
                &format!(
                    "<h1>Stale selection</h1><div class=\"panel\"><p>These origins are not in the live read-back any more: <b>{}</b>. The list has changed since the page rendered; nothing was prepared. Re-pick from the fresh list.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
                    esc(&list)
                ),
            ),
        );
    }
    let name = format!(
        "Remove {} designation{}",
        origins.len(),
        if origins.len() == 1 { "" } else { "s" }
    );
    let mut details = String::new();
    details.push_str(&format!(
        "<p><b>{} origins</b> will be cleared. Remove is proto-agnostic — whatever designation sits at each origin goes:</p><ul>",
        origins.len()
    ));
    for (x, y) in &origins {
        let e = crate::designate::existing_at(&existing, *x, *y);
        details.push_str(&format!(
            "<li>({}, {}) — {}</li>",
            x,
            y,
            e.map(|e| format!(
                "{} (<span class=\"mono\">{}</span>){}",
                esc(&e.kind),
                esc(&e.proto),
                if e.is_fulfilled {
                    ", fulfilled"
                } else {
                    ", open"
                }
            ))
            .unwrap_or_else(|| "unknown".to_string())
        ));
    }
    details.push_str("</ul>");
    details.push_str("<p class=\"sub\">Origins the game no longer designates when the mod runs are simply not there to remove; the read-back verdict after confirm is the honest check.</p>");

    let now = now_unix();
    let job_id = new_job_id();
    let created = iso_utc(now);
    let expires = iso_utc(now + JOB_TTL_SECONDS);
    let meta = crate::designate::JobMeta {
        job_id: &job_id,
        created_utc: &created,
        expires_utc: &expires,
        game_id,
        companion_user: &session.user,
        plan_name: &name,
    };
    let job = crate::designate::build_remove_job(&meta, &origins, DESIGNATE_MAX_CELLS);
    let summary = format!(
        "Designation removal '{}': {} origins cleared",
        name,
        origins.len()
    );
    let token = rand_hex(16);
    lock_map(&state.prepared_jobs).insert(
        token.clone(),
        PreparedJob {
            job_id: job_id.clone(),
            file_json: job.file_json.clone(),
            summary: summary.clone(),
            game_id: game_id.to_string(),
            game_date: game["date"].as_str().map(|s| s.to_string()),
            game_tick: game["tick"].as_i64(),
            created: now,
        },
    );
    designate_confirm_response(
        state,
        session,
        &summary,
        &details,
        &job,
        &job_id,
        game_id,
        game["date"].as_str().unwrap_or("?"),
        &token,
    )
}

fn do_designate_confirm(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let token = req.form.get("token").cloned().unwrap_or_default();
    let job = lock_map(&state.prepared_jobs).remove(&token);
    let job = match job {
        Some(j) if now_unix() - j.created <= 120 => j,
        _ => {
            return Response::html(
                400,
                layout(state, session,
                    "Designation confirmation expired",
                    "<h1>Confirmation expired</h1><div class=\"panel\"><p>That designation confirmation is unknown or older than two minutes, so nothing was written. Prepare the job again.</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
                ),
            )
        }
    };
    let dir = builder_jobs_dir(&state.db_path);
    let path = dir.join(format!("{}.json", job.job_id));
    let write = (|| -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create jobs folder: {e}"))?;
        let tmp = dir.join(format!("{}.json.tmp", job.job_id));
        std::fs::write(&tmp, job.file_json.as_bytes()).map_err(|e| format!("write job: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("rename job: {e}"))?;
        Ok(())
    })();
    // Same §12 discipline as do_place_confirm (audit D1): the
    // confirm's audit/activity rows are part of the guarantee,
    // so a failed write is collected and surfaced — never a
    // silent `let _`, in either outcome branch.
    let outcome = if write.is_ok() {
        "job-written"
    } else {
        "write-failed"
    };
    let mut audit_err: Option<String> = None;
    match store(state) {
        Ok(st) => {
            if let Err(e) = st.audit_add(
                now_unix() as i64,
                "PlaceDesignationJob",
                &format!(
                    "{} · job {} · game {} · {}",
                    job.summary, job.job_id, job.game_id, outcome
                ),
            ) {
                audit_err = Some(format!("audit row: {e}"));
            }
            if let Err(e) = st.activity_add(&ActivityEvent {
                event_key: format!("designate-{}-{}", now_unix(), rand_hex(8)),
                recorded_ts_unix: now_unix() as i64,
                source: "companion".into(),
                actor: Some(session.user.clone()),
                category: Some("designation".into()),
                action: "PlaceDesignationJob".into(),
                summary: Some(job.summary.clone()),
                game_id: Some(job.game_id.clone()),
                game_date: job.game_date.clone(),
                game_tick: job.game_tick,
                outcome: Some(outcome.into()),
                payload_json: Some(job.file_json.clone()),
            }) {
                audit_err.get_or_insert_with(|| format!("activity row: {e}"));
            }
        }
        Err(e) => {
            audit_err = Some(format!("audit store unavailable: {e}"));
        }
    }
    if let Some(e) = &audit_err {
        eprintln!(
            "serve: designate-confirm of job {} left NO audit record: {e}",
            job.job_id
        );
    }
    let audit_banner = match &audit_err {
        Some(e) => format!(
            "<div class=\"callout callout--danger\"><b>AUDIT WRITE FAILED.</b> The job file WAS written — QoSBuilder will act on it at the next in-game day end — but recording it in the audit log failed: {} — <b>this designation job is missing from the audit trail</b>.</div>",
            esc(e),
        ),
        None => String::new(),
    };
    let audit_note = match &audit_err {
        Some(e) => format!(
            "Nothing was written. The audit write failed too ({}) — so this failure is NOT in the audit log either.",
            esc(e)
        ),
        None => "Nothing was written and the failure is in the audit log.".to_string(),
    };
    match write {
        Ok(()) => {
            // The mod attests "scheduled" only; the verdict is
            // the read-back diff, computed right now (it will
            // normally read "pending" until the day turns) and
            // recomputed on every Designate page visit.
            let existing = fetch_designations(state);
            let panel = designation_verdict_panel(
                "Read-back verdict — right now",
                &job.file_json,
                existing.as_deref(),
            );
            Response::html(
            200,
            layout(state, session,
                "Designation job written",
                &format!(
                    "<h1>Job written</h1>{}<div class=\"panel\"><p><b>{}</b></p>\
                     <p class=\"sub\">File: <code>{}</code> · job <code>{}</code></p>\
                     <p>QoSBuilder picks it up at the <b>next in-game day end</b> — if the mod is installed and its <code>enabled</code> setting is on. Watch the <a href=\"/log\">Log page</a> for <code>JobClaimed</code> / <code>JobScheduled</code> (or a refusal with its reason).</p>\
                     <p><a href=\"/designate\">← Back to Designate</a></p></div>{}",
                    audit_banner,
                    esc(&job.summary),
                    esc(&path.display().to_string()),
                    esc(&job.job_id),
                    panel,
                ),
            ),
            )
        }
        Err(e) => Response::html(
            500,
            layout(state, session,
                "Designation job write failed",
                &format!(
                    "<h1>Job write failed</h1><div class=\"panel\"><p>{}</p><p>{} Check the jobs folder path (shown on the Designate page; override it with <code>[builder] jobs_dir</code> in coi.toml if the companion runs somewhere other than the game PC).</p><p><a href=\"/designate\">← Back to Designate</a></p></div>",
                    esc(&e),
                    audit_note,
                ),
            ),
        ),
    }
}

// ---------------------------------------------------------------------------
// Catalogue — every command the game exposes, and what we know about it
// ---------------------------------------------------------------------------

fn page_catalogue(state: &State, session: &Session, req: &Request) -> Response {
    let _ = session;
    let verified = CATALOGUE.iter().filter(|c| c.status == "verified").count();
    let excluded = CATALOGUE.iter().filter(|c| c.status == "excluded").count();
    let entity = CATALOGUE.iter().filter(|c| c.entity).count();
    // Facets (V2.1): ?domain= and ?status= filter server-side;
    // the client filter then works within the rendered rows.
    let domain_f = req.query.get("domain").cloned().unwrap_or_default();
    let status_f = req.query.get("status").cloned().unwrap_or_default();
    let mut body = page_head(
        "Workshop",
        "Catalogue",
        "Every command the game exposes — what each one does, and whether it has ever been run from here.",
        "",
    );
    body.push_str(&contract_strip(
        "The full command inventory behind every Prepare button on this site",
        "Actionable — every non-excluded row prepares through the same confirm flow",
        "Works without the game: a captured inventory from v0.8.8. Statuses are facts about this site, not the game — verified means executed live through the confirm flow; wired means preparable, not yet run; excluded means policy refusal",
    ));
    if req.query.get("flash").map(|s| s.as_str()) == Some("prepare-needs-command") {
        body.push_str("<div class=\"callout callout--info\">That link opened <b>Prepare</b> without naming a command, so nothing was prepared. Pick the command here — or from any Prepare button on the site — and you'll see its confirm sheet first.</div>");
    }
    body.push_str(
        "<details class=\"howit\"><summary>How to read this catalogue</summary><p>Purposes are read from command names and argument schemas; \
         only commands marked <b>verified</b> have actually been executed and observed through \
         this website's confirm flow.</p><p><b>Construction: the primitive exists.</b> \
         <b>CreateStaticEntityCmd</b> places a building/transport from a prototype at a tile transform, with \
         <b>FinishBuildOfStaticEntityCmd</b> beside it, <b>CreateTrainTrackEntityWithDirectionCmd</b> for rails, \
         and full train-line / train-network / vehicle-build-queue creation commands. Whether placement creates \
         a construction site or a finished building is untested: sandbox-save experiments are the way to find \
         out, never the live save.</p></details>",
    );
    body.push_str("<div class=\"stats2\">");
    body.push_str(&stat2_tile(
        "search",
        "Commands",
        &CATALOGUE.len().to_string(),
        &format!(
            "{} target a specific entity · {} global",
            entity,
            CATALOGUE.len() - entity
        ),
        false,
    ));
    body.push_str(&stat2_tile(
        "check",
        "Verified live",
        &verified.to_string(),
        "executed through prepare → confirm → audit",
        false,
    ));
    body.push_str(&stat2_tile(
        "alert",
        "Excluded by policy",
        &excluded.to_string(),
        "never preparable — see its note",
        excluded > 0,
    ));
    body.push_str("</div>");
    let shown_all: Vec<_> = CATALOGUE
        .iter()
        .filter(|c| {
            (domain_f.is_empty() || c.domain == domain_f)
                && (status_f.is_empty() || c.status == status_f)
        })
        .collect();
    // Directory discipline (DESIGN V3): paginated like the other
    // directories, facets preserved across pages. Page size is 30,
    // not the usual 60: a catalogue row carries purpose + args prose
    // and runs ~2x a product row's height — the binding rule is the
    // 3-viewport page cap, and 30 rows is what fits it.
    const CAT_PAGE: usize = 30;
    let cat_pages = shown_all.len().div_ceil(CAT_PAGE).max(1);
    let cat_page = req
        .query
        .get("page")
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, cat_pages);
    let shown: Vec<_> = shown_all
        .iter()
        .skip((cat_page - 1) * CAT_PAGE)
        .take(CAT_PAGE)
        .cloned()
        .collect();
    let facet_q = {
        let mut q = String::new();
        if !domain_f.is_empty() {
            q.push_str(&format!("&domain={}", url_enc(&domain_f)));
        }
        if !status_f.is_empty() {
            q.push_str(&format!("&status={}", url_enc(&status_f)));
        }
        q
    };
    let pager = if cat_pages > 1 {
        format!(
            "<div class=\"pager\"><span class=\"sub\">Page {} of {}</span>{}{}</div>",
            cat_page,
            cat_pages,
            if cat_page > 1 {
                format!(
                    "<a class=\"btn btn--sm btn--ghost\" href=\"/catalogue?page={}{}\">← Prev</a>",
                    cat_page - 1,
                    facet_q
                )
            } else {
                String::new()
            },
            if cat_page < cat_pages {
                format!(
                    "<a class=\"btn btn--sm btn--ghost\" href=\"/catalogue?page={}{}\">Next →</a>",
                    cat_page + 1,
                    facet_q
                )
            } else {
                String::new()
            }
        )
    } else {
        String::new()
    };
    body.push_str(&format!("<div class=\"card tablecard\"><div class=\"filterbar\"><input id=\"tfilter\" type=\"search\" placeholder=\"🔍 Filter commands…\"><span class=\"count\">{} of {} commands · status: verified = executed live · wired = preparable, unverified · excluded = policy refusal</span></div>", shown_all.len(), CATALOGUE.len()));
    body.push_str("<table id=\"ctable\" class=\"table--compact\"><thead><tr><th>Domain</th><th>Command</th><th>Purpose & args</th><th>Targets</th><th>Status</th></tr></thead><tbody>");
    for c in &shown {
        let status = match c.status {
            "verified" => badge("ok", "verified"),
            "excluded" => badge("critical", "excluded"),
            _ => badge("info", "wired"),
        };
        body.push_str(&format!(
            "<tr><td class=\"sub\">{}</td><td><b>{}</b>{}</td><td>{}<br><span class=\"sub mono\">{}</span>",
            esc(c.domain),
            esc(c.name),
            if c.status == "excluded" {
                String::new()
            } else {
                format!("<br><a class=\"btn btn--sm btn--ghost\" href=\"/prepare?cmd={}\">Prepare…</a>", esc(c.name))
            },
            esc(c.purpose),
            esc(c.args),
        ));
        if !c.note.is_empty() {
            body.push_str(&format!(
                "<br><span class=\"sub\">Note: {}</span>",
                esc(c.note)
            ));
        }
        body.push_str(&format!(
            "</td><td>{}</td><td>{}</td></tr>",
            if c.entity { "entity" } else { "global" },
            status,
        ));
    }
    body.push_str("</tbody></table>");
    body.push_str(&pager);
    body.push_str("</div>");
    body.push_str(TABLE_JS);
    body.push_str("<script>wireTable('ctable','tfilter');</script>");
    // Rail context: domain facets with counts + status facets.
    let mut domains: Vec<&str> = CATALOGUE.iter().map(|c| c.domain).collect();
    domains.sort_unstable();
    domains.dedup();
    let mut ctx = String::from("<div class=\"ctx\">DOMAINS</div>");
    ctx.push_str(&format!(
        "<a class=\"navitem anchor{}\" href=\"/catalogue\">All domains<span class=\"count\">{}</span></a>",
        if domain_f.is_empty() { " active" } else { "" },
        CATALOGUE.len()
    ));
    for d in domains {
        let n = CATALOGUE.iter().filter(|c| c.domain == d).count();
        ctx.push_str(&format!(
            "<a class=\"navitem anchor{}\" href=\"/catalogue?domain={}\">{}<span class=\"count\">{}</span></a>",
            if domain_f == d { " active" } else { "" },
            url_enc(d),
            esc(d),
            n
        ));
    }
    ctx.push_str("<div class=\"ctx\">STATUS</div>");
    for (sv_, label) in [
        ("verified", "Verified"),
        ("wired", "Wired"),
        ("excluded", "Excluded"),
    ] {
        let n = CATALOGUE.iter().filter(|c| c.status == sv_).count();
        ctx.push_str(&format!(
            "<a class=\"navitem anchor{}\" href=\"/catalogue?status={}\">{}<span class=\"count\">{}</span></a>",
            if status_f == sv_ { " active" } else { "" },
            sv_,
            label,
            n
        ));
    }
    Response::html(
        200,
        layout_ctx(
            state,
            session,
            "Command catalogue",
            "Catalogue",
            &ctx,
            &body,
        ),
    )
}

fn guides_for_issue(issue: &Issue, pid: Option<&str>) -> Vec<&'static Guide> {
    let mut out: Vec<&'static Guide> = Vec::new();
    let mut push = |id: &str| {
        if let Some(g) = guide_by_id(id) {
            if !out.iter().any(|x| x.id == g.id) {
                out.push(g);
            }
        }
    };
    let title = issue.title.as_str();
    if title.contains("Storage full") || title.contains("above nominal capacity") {
        push("storage");
        push("transport");
    }
    if title.contains("Seasonal") {
        push("farming");
        push("storage");
    }
    // Round-5 guides (contracts/trade, loans, edicts): title keywords.
    let lower = title.to_lowercase();
    if lower.contains("contract") || lower.contains("quick trade") {
        push("contracts-trade");
    }
    if lower.contains("loan") || lower.contains("payment due") {
        push("loans");
    }
    if lower.contains("edict") {
        push("edicts");
    }
    match pid {
        Some("Product_Diesel")
        | Some("Product_CrudeOil")
        | Some("Product_Naphtha")
        | Some("Product_FuelGas")
        | Some("Product_HeavyOil") => push("oil-diesel"),
        Some("Product_Wheat")
        | Some("Product_Corn")
        | Some("Product_Potato")
        | Some("Product_Soybean")
        | Some("Product_Vegetables")
        | Some("Product_Fruit")
        | Some("Product_SugarCane") => push("farming"),
        _ => {}
    }
    if title.contains("stalled") {
        push("priorities");
        push("transport");
    }
    if title.contains("deficit") || title.contains("draining") {
        push("priorities");
    }
    out
}

// Trade, loans & edicts — the state families the vanilla web API
// never exposes, read through CoiBridge v0.6 routes. Each panel
// degrades to an "update the mod" note when its route is missing,
// so the Trade page is safe to ship ahead of the mod update.
// ---------------------------------------------------------------------------
// Plans — the bundled game plan (Round 4; privacy cleanup 2026-11-09).
// The fresh-start plan JSON is compiled into the binary (include_str!),
// so the page works in the single-file exe distribution with no
// runtime file dependencies. Only generic, run-independent plans are
// bundled: the personalised continuation plan and the baked dry-run
// verdicts (which carried one specific run's gameId and state) were
// un-embedded in the 2026-11-09 privacy cleanup — evaluate plans
// against live state with `coi plan-check` / `coi play` instead.
// No additional plans are bundled.
// ---------------------------------------------------------------------------

const PLAN_JSON_FRESH: &str = include_str!("../../plans/fresh-start-v1.json");

fn page_plans(state: &State, session: &Session, _req: &Request) -> Response {
    // Bundled plans only (privacy cleanup 2026-11-09): the generic
    // fresh-start plan. Personalised plans and baked verdicts are no
    // longer compiled in; evaluate a plan against live state with
    // `coi plan-check` / `coi play`.
    let plan_docs: Vec<Value> = [PLAN_JSON_FRESH]
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect();
    let mut body = page_head(
        "Plans",
        "Plans",
        "Step-by-step plans for a run, phase by phase — each phase ends at a gate your factory must actually reach.",
        "",
    );
    body.push_str(&contract_strip(
        "Bundled plans and their phase gates",
        "Reference — nothing on this page executes anything",
        "Bundled with the tool and readable offline. Verdicts appear here after a supervised session walks a plan against your live factory — until then, a plan is a promise, not a result",
    ));
    body.push_str(
        "<div class=\"callout callout--info\">This page is the plan <b>reference</b>: phases, intents \
         and gates as authored. Nothing here has been executed. A plan gets <i>walked</i> against your \
         live factory in a supervised session — every step prepared, confirmed by you, and recorded — \
         and the verdicts from that walk are what turn a plan from a promise into a result. \
         (How plan evaluation works: <a href=\"/manual\">Learn the tool</a>.) No additional plans are \
         bundled — personalised plans live as files next to the tool, not inside it.</div>",
    );
    let mut ctx = String::from("<div class=\"ctx\">PHASES</div>");
    for d in &plan_docs {
        let plan = &d["plan"];
        let pid = plan["id"].as_str().unwrap_or("?");
        let phases: Vec<&Value> = d["phases"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let n_orders: usize = phases
            .iter()
            .map(|ph| ph["orders"].as_array().map(|o| o.len()).unwrap_or(0))
            .sum();
        body.push_str(&format!(
            "<div class=\"card\"><div class=\"cardhead\"><h2>{}</h2><span class=\"right\">{}</span></div><p class=\"sub\">Plan <code>{}</code> v{} · game {} · map {} ({})</p>",
            esc(plan["title"].as_str().unwrap_or(pid)),
            badge("info", "bundled"),
            esc(pid),
            esc(&plan["version"].to_string()),
            esc(&plan["game_version"].to_string()),
            esc(plan["map"]["name"].as_str().unwrap_or("")),
            esc(plan["map"]["difficulty"].as_str().unwrap_or("")),
        ));
        if let Some(end) = plan["end_state"].as_str() {
            body.push_str(&format!("<p class=\"sub\">End state: {}</p>", esc(end)));
        }
        body.push_str(&figstrip(&[
            fig("Phases", &phases.len().to_string(), false),
            fig("Orders", &n_orders.to_string(), false),
        ]));
        for ph in &phases {
            let phid = ph["id"].as_str().unwrap_or("");
            ctx.push_str(&rail_link(
                &format!("#{phid}"),
                ph["name"].as_str().unwrap_or(""),
            ));
            body.push_str(&format!(
                "<div class=\"phase\" id=\"{}\"><b>{}</b> <span class=\"sub\"><code>{}</code> · {} orders</span>",
                esc(phid),
                esc(ph["name"].as_str().unwrap_or("")),
                esc(phid),
                ph["orders"].as_array().map(|o| o.len()).unwrap_or(0),
            ));
            if let Some(intent) = ph["intent"].as_str() {
                if !intent.is_empty() {
                    body.push_str(&format!("<div class=\"ev\">{}</div>", esc(intent)));
                }
            }
            body.push_str("</div>");
        }
        body.push_str("</div>");
    }
    if plan_docs.is_empty() {
        body.push_str("<div class=\"err\">Bundled plan data failed to parse.</div>");
    }
    Response::html(
        200,
        layout_ctx(state, session, "Plans", "Plans", &ctx, &body),
    )
}

fn page_trade(state: &State, session: &Session, _req: &Request) -> Response {
    let cl = client(state);
    let live = fetch_live(&cl).ok();
    let name_of = |id: String| -> String {
        live.as_ref()
            .and_then(|l| l.names.get(&id).cloned())
            .unwrap_or(id)
    };
    let pcts = |v: &Value| format!("{:.1}%", pct_raw(v));
    let mut body = page_head(
        "Workshop",
        "Trade",
        "Your deals with the world: contracts, quick trades and loans — what you owe, what you're owed, and what's about to come due.",
        "",
    );
    body.push_str(&contract_strip(
        "Deals with the world market",
        "Actionable — contract toggles, trades and loan actions all go prepare → confirm → audit",
        "Read through CoiBridge — the game's own API has no routes for any of this",
    ));

    // ---- Contracts ----
    body.push_str("<div class=\"card tablecard\" id=\"contracts\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Contracts</h2></div>");
    match bridge_json(state, "/coi/v1/contracts") {
        Some(v) => {
            body.push_str(&format!(
                "<p class=\"sub\">Profit multiplier: {}</p>",
                esc(&pcts(&v["profitMultiplierRaw"]))
            ));
            let rows = |key: &str, action: &str| -> String {
                let mut t = String::from(
                    "<table class=\"sortable\"><thead><tr><th>Contract</th><th>You receive</th><th>You pay</th><th>To buy now</th><th></th></tr></thead><tbody>",
                );
                if let Some(arr) = v[key].as_array() {
                    for c in arr {
                        let id = sv(&c["id"]);
                        t.push_str(&format!(
                            "<tr><td>{}</td><td>{}</td><td>{} {}</td><td>{}</td><td>{}</td></tr>",
                            esc(&id),
                            esc(&name_of(sv(&c["productToBuy"]))),
                            vi(&c["quantityToPayWith"]),
                            esc(&name_of(sv(&c["productToPayWith"]))),
                            if key == "active" {
                                vi(&c["toBuy"]).to_string()
                            } else {
                                "—".to_string()
                            },
                            prepare_form(
                                session,
                                "ToggleContractCmd",
                                json!({"contractId": id}),
                                action,
                                false,
                            ),
                        ));
                    }
                }
                t.push_str("</tbody></table>");
                t
            };
            body.push_str("<h3>Active</h3>");
            body.push_str(&rows("active", "Cancel contract"));
            body.push_str("<h3>Available to establish</h3>");
            body.push_str(&rows("available", "Establish contract"));
        }
        None => body.push_str(&degraded_bridge("Contracts", "v0.6")),
    }
    body.push_str("</div>");

    // ---- Quick trades (CoiBridge v0.6.1) ----
    body.push_str(
        "<div class=\"card\" id=\"quick\"><div class=\"cardhead\"><h2>Quick trades</h2></div>",
    );
    match bridge_json(state, "/coi/v1/quick-trades") {
        Some(v) => {
            body.push_str("<table class=\"sortable\"><thead><tr><th>Trade</th><th>You receive</th><th>You pay</th><th>Unity cost (raw)</th><th>Step</th><th>State</th><th></th></tr></thead><tbody>");
            if let Some(arr) = v["trades"].as_array() {
                for t in arr {
                    let id = sv(&t["id"]);
                    let sold_out = t["isSoldOut"].as_bool().unwrap_or(false);
                    let state_txt = if sold_out {
                        badge("paused", "sold out")
                    } else if t["hasPriceIncreased"].as_bool().unwrap_or(false) {
                        badge("warning", "price increased")
                    } else {
                        badge("ok", "available")
                    };
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{} {}</td><td>{} {}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{}</td></tr>",
                        esc(&id),
                        vi(&t["productToBuy"]["quantity"]),
                        esc(&name_of(sv(&t["productToBuy"]["product"]))),
                        vi(&t["productToPayWith"]["quantity"]),
                        esc(&name_of(sv(&t["productToPayWith"]["product"]))),
                        vi(&t["upointsCostRaw"]),
                        vi(&t["currentStep"]),
                        vi(&t["maxSteps"]),
                        state_txt,
                        if sold_out {
                            String::new()
                        } else {
                            prepare_form(
                                session,
                                "QuickTradeCmd",
                                json!({"tradeId": id}),
                                "Prepare trade",
                                false,
                            )
                        },
                    ));
                }
            }
            body.push_str("</tbody></table>");
        }
        None => body.push_str(&degraded_bridge("Quick trades", "v0.6.1")),
    }
    body.push_str("</div>");

    // ---- Loans ----
    body.push_str("<div class=\"card\" id=\"loans\"><div class=\"cardhead\"><h2>Loans</h2></div>");
    match bridge_json(state, "/coi/v1/loans") {
        Some(v) => {
            body.push_str(&format!(
                "<p class=\"sub\">Credit score (raw): {} · Interest rate: {} · Fee: {} · Max active loans: {}</p>",
                vi(&v["creditScoreRaw"]),
                esc(&pcts(&v["interestRateRaw"])),
                esc(&pcts(&v["feeRaw"])),
                vi(&v["maxActiveLoans"]),
            ));
            if let Some(arr) = v["activeLoans"].as_array() {
                if arr.is_empty() {
                    body.push_str("<p>No active loans.</p>");
                } else {
                    body.push_str("<table class=\"sortable\"><thead><tr><th>Product</th><th>Left to pay</th><th>Annual payment</th><th>Rate</th><th>Years left</th><th>Next payment</th><th>Overpay</th></tr></thead><tbody>");
                    for l in arr {
                        body.push_str(&format!(
                            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>\
                             <form method=\"post\" action=\"/actions/prepare-generic\">\
                             <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
                             <input type=\"hidden\" name=\"type\" value=\"MakeLoanOverpaymentCmd\">\
                             <input type=\"hidden\" name=\"arg_product\" value=\"{}\">\
                             <input type=\"hidden\" name=\"arg_loanId\" value=\"{}\">\
                             <input name=\"arg_toPay\" size=\"6\" placeholder=\"amount\"> \
                             <button type=\"submit\">Prepare…</button></form></td></tr>",
                            esc(&name_of(sv(&l["product"]))),
                            vi(&l["leftToPay"]),
                            vi(&l["annualPayment"]),
                            esc(&pcts(&l["interestRateRaw"])),
                            vi(&l["yearsLeft"]),
                            esc(&sv(&l["nextPaymentDate"])),
                            esc(&session.csrf),
                            esc(&sv(&l["product"])),
                            vi(&l["id"]),
                        ));
                    }
                    body.push_str("</tbody></table>");
                }
            }
            if let Some(arr) = v["borrowableProducts"].as_array() {
                if !arr.is_empty() {
                    body.push_str("<h3>Borrow</h3><table><thead><tr><th>Product</th><th>Amount</th><th>Years</th><th></th></tr></thead><tbody>");
                    for p in arr {
                        let pid = sv(p);
                        body.push_str(&format!(
                            "<tr><td>{}</td><td colspan=\"3\">\
                             <form method=\"post\" action=\"/actions/prepare-generic\">\
                             <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
                             <input type=\"hidden\" name=\"type\" value=\"AcceptLoanCmd\">\
                             <input type=\"hidden\" name=\"arg_product\" value=\"{}\">\
                             <input name=\"arg_toBorrow\" size=\"7\" placeholder=\"amount\"> \
                             <input name=\"arg_yearsTotal\" size=\"4\" placeholder=\"years\"> \
                             <button type=\"submit\">Prepare…</button></form></td></tr>",
                            esc(&name_of(pid.clone())),
                            esc(&session.csrf),
                            esc(&pid),
                        ));
                    }
                    body.push_str("</tbody></table>");
                }
            }
        }
        None => body.push_str(&degraded_bridge("Loans", "v0.6")),
    }
    body.push_str("</div>");

    // ---- Edicts live elsewhere now (DESIGN V3 §2.3): the status
    // table is part of Guidance ▸ Learn the game ▸ Edicts, next to
    // the explanation of what enabled/active mean. A cross-link
    // strip stays here so old habits (and #edicts links) land well.
    body.push_str("<div class=\"card\" id=\"edicts\"><div class=\"cardhead\"><h2>Edicts</h2></div>\
        <p class=\"sub\">Edict status — which edicts are enabled, which are actually active, and the game's own reasons when one isn't — \
        now lives with the guide that explains it: <a href=\"/guide?id=edicts\">Learn the game → Edicts</a>. Toggling still happens in-game.</p></div>");
    body.push_str(TABLE_JS);
    Response::html(200, layout(state, session, "Trade", &body))
}

/// The edicts status table (moved from Trade in DESIGN V3): every
/// edict's enabled/active flags plus the game's own reason strings.
/// Rendered as the live panel of the Edicts guide.
fn edicts_status_table(state: &State) -> String {
    match bridge_json(state, "/coi/v1/edicts") {
        Some(v) => {
            let mut t = String::from("<div class=\"card\"><div class=\"cardhead\"><h2>Edict status — right now</h2></div><table class=\"sortable\"><thead><tr><th>Edict</th><th>Enabled</th><th>Active</th><th>Can enable</th><th>Why / why not</th></tr></thead><tbody>");
            if let Some(arr) = v["edicts"].as_array() {
                for e in arr {
                    let why = if sv(&e["reason"]).is_empty() {
                        sv(&e["inactiveReason"]).to_string()
                    } else {
                        sv(&e["reason"]).to_string()
                    };
                    t.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        esc(&sv(&e["id"])),
                        if e["isEnabled"].as_bool().unwrap_or(false) { badge("ok", "enabled") } else { "—".to_string() },
                        if e["isActive"].as_bool().unwrap_or(false) { badge("ok", "active") } else { "—".to_string() },
                        if e["canBeEnabled"].as_bool().unwrap_or(false) { "yes" } else { "no" },
                        esc(&why),
                    ));
                }
            }
            t.push_str("</tbody></table><div class=\"callout callout--info\">Toggling edicts is not wired: the game's ToggleEdictEnabledCmd exists inside the game but is not in the web API's command list, so it needs a sandbox test before it can go through the confirm flow. Toggle in-game for now.</div></div>");
            t
        }
        None => format!("<div class=\"card\"><div class=\"cardhead\"><h2>Edict status — right now</h2></div>{}</div>", degraded_bridge("Edicts", "v0.6")),
    }
}

// ---------------------------------------------------------------------------
// The canon (DESIGN V3 §3): docs/canon/*.yaml compiled to JSON by
// tools/canon_to_json.py and embedded here. The Why pages, the
// issue canon strips and the board's ordering explanation all
// render from this one source; tools/check_canon.py fails the
// build if a coded constant disagrees with it.
// ---------------------------------------------------------------------------

const CANON_JSON: &str = include_str!("../../docs/canon/canon.json");

fn canon() -> &'static Value {
    static CANON: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    CANON.get_or_init(|| serde_json::from_str(CANON_JSON).unwrap_or(json!({})))
}

const CANON_FAMILY_ORDER: [&str; 7] = [
    "board",
    "forecasts",
    "productivity",
    "priorities",
    "earthworks",
    "plans",
    "philosophy",
];

fn canon_entry(id: &str) -> Option<&'static Value> {
    let c = canon();
    for fam in CANON_FAMILY_ORDER {
        if let Some(entries) = c["families"][fam]["entries"].as_array() {
            for e in entries {
                if e["id"].as_str() == Some(id) {
                    return Some(e);
                }
            }
        }
    }
    None
}

/// Whitespace-folded player text of a canon entry, optionally
/// truncated at a sentence boundary for strip use.
fn canon_player(e: &Value, max_chars: usize) -> String {
    let raw = e["player"].as_str().unwrap_or("");
    let folded = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if folded.chars().count() <= max_chars {
        return folded;
    }
    let cut: String = folded.chars().take(max_chars).collect();
    match cut.rfind(". ") {
        Some(i) => format!("{}.", &cut[..i]),
        None => format!("{cut}…"),
    }
}

/// The canon strip on an issue detail's why-card: the rule that
/// fired, in its own words, with the link to the full entry.
fn canon_strip_for_issue(issue: &Issue) -> String {
    let id = match issue.class {
        "stalled" => "board.stall-confirmation",
        "deficit" => "board.deficit-horizon",
        "structural" => "board.structural-horizon",
        "seasonal" => "board.seasonal-window",
        "storage-full" | "parked" => "board.storage-full",
        "overcap" => "board.overcap",
        "prod-starved" | "prod-blocked" | "prod-mixed" | "prod-unclassified" => {
            "productivity.causes"
        }
        "below-baseline" => "productivity.working-share",
        _ => return String::new(),
    };
    let Some(e) = canon_entry(id) else {
        return String::new();
    };
    let fam = id.split('.').next().unwrap_or("board");
    format!(
        "<div class=\"canonstrip\"><span class=\"cl\">The rule — {}</span><p>{}</p>\
         <p><a href=\"/why?topic={}#{}\">Why it suggests → {}</a></p></div>",
        esc(e["title"].as_str().unwrap_or("")),
        esc(&canon_player(e, 300)),
        fam,
        esc(id),
        esc(e["title"].as_str().unwrap_or(""))
    )
}

fn guidance_ctx() -> String {
    let mut ctx = String::from("<div class=\"ctx\">GUIDANCE</div>");
    ctx.push_str(&rail_link("/guides", "Learn the game"));
    ctx.push_str(&rail_link("/manual", "Learn the tool"));
    ctx.push_str(&rail_link("/why", "Why it suggests"));
    ctx
}

/// Why it suggests — the canon, rendered for players. /why is the
/// index; /why?topic=<family> renders one family's entries in
/// full (player text, numbers, status, and the precise rule for
/// auditors). Productivity renders the EFFECTIVE thresholds live
/// (canon C-10: the numbers in effect, not just the defaults).
fn page_why(state: &State, session: &Session, req: &Request) -> Response {
    let topic = req.query.get("topic").cloned().unwrap_or_default();
    let c = canon();
    if topic.is_empty() {
        let mut body = page_head(
            "Guidance",
            "Why it suggests",
            "Why the companion suggests what it suggests — the rules, in plain words.",
            "",
        );
        body.push_str(&contract_strip(
            "The reasoning layer: board rules, forecasts, verdicts, philosophy",
            "Informational — this domain changes nothing; it explains everything",
            "Rendered from the canon (docs/canon in the toolkit) — the same source the board's own copy quotes, so the two can never drift apart",
        ));
        body.push_str("<div class=\"cols\">");
        for fam in CANON_FAMILY_ORDER {
            let f = &c["families"][fam];
            let entries = f["entries"].as_array().cloned().unwrap_or_default();
            let first = entries
                .first()
                .map(|e| canon_player(e, 140))
                .unwrap_or_default();
            body.push_str(&format!(
                "<div class=\"col\"><h2><a href=\"/why?topic={}\">{}</a></h2><p>{}</p><p class=\"sub\">{} rules</p></div>",
                fam,
                esc(f["title"].as_str().unwrap_or(fam)),
                esc(&first),
                entries.len()
            ));
        }
        body.push_str("</div>");
        return Response::html(
            200,
            layout_ctx(
                state,
                session,
                "Why it suggests",
                "Why it suggests",
                &guidance_ctx(),
                &body,
            ),
        );
    }
    let f = &c["families"][&topic];
    if f.is_null() {
        return Response::redirect("/why");
    }
    let fam_title = f["title"].as_str().unwrap_or(&topic).to_string();
    let mut body = page_head(
        "Guidance",
        &fam_title,
        "One family of the canon — the rules as the companion applies them.",
        "<a class=\"btn btn--secondary btn--sm\" href=\"/why\">← All families</a>",
    );
    if topic == "productivity" {
        // C-10: show the thresholds actually in effect for this
        // install (coi.toml [productivity] overrides applied).
        let th = crate::productivity::Thresholds::load(&state.db_path);
        body.push_str(&format!(
            "<div class=\"callout callout--info\"><b>In effect on this install:</b> fire when the working-share interval tops out below {:.2} (point estimate below {:.2}) · severe below {:.2} · clear above {:.2} · parked at ≥ {:.0}% paused · starved/blocked at ≥ {:.0}%. \
             These come from your <code>coi.toml</code> [productivity] section where set, and from the shipped defaults otherwise — the entries below state the defaults and the reasoning.</div>",
            th.fire_ci_upper, th.fire_point, th.severe_point, th.clear_ci_lower,
            th.parked_share * 100.0, th.starved_share * 100.0
        ));
    }
    for e in f["entries"].as_array().cloned().unwrap_or_default() {
        let status = e["status"].as_str().unwrap_or("");
        let status_badge = match status {
            "verified" => badge("ok", "verified"),
            "wired" => badge("info", "wired to the code"),
            "heuristic" => badge("warning", "heuristic"),
            "deliberate-none" => badge("paused", "deliberately not adopted"),
            _ => badge("info", status),
        };
        body.push_str(&format!(
            "<div class=\"card\" id=\"{}\"><div class=\"cardhead\"><h2>{}</h2><span class=\"right\">{}</span></div><p>{}</p>",
            esc(e["id"].as_str().unwrap_or("")),
            esc(e["title"].as_str().unwrap_or("")),
            status_badge,
            esc(&canon_player(&e, usize::MAX))
        ));
        if let Some(nums) = e["numbers"].as_array() {
            if !nums.is_empty() {
                body.push_str("<table class=\"table--compact\"><thead><tr><th>Number</th><th class=\"num\">Value</th><th>Unit</th></tr></thead><tbody>");
                for n in nums {
                    body.push_str(&format!(
                        "<tr><td>{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
                        esc(n["name"].as_str().unwrap_or("")),
                        esc(&n["value"].to_string()),
                        esc(n["unit"].as_str().unwrap_or(""))
                    ));
                }
                body.push_str("</tbody></table>");
            }
        }
        body.push_str(&format!(
            "<details class=\"howit\"><summary>The precise rule (for auditors)</summary><p>{}</p><p class=\"sub\">Source: {} · canon id <code>{}</code></p></details></div>",
            esc(e["rule"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").as_str()),
            esc(e["source"].as_str().unwrap_or("")),
            esc(e["id"].as_str().unwrap_or(""))
        ));
    }
    let mut ctx = guidance_ctx();
    ctx.push_str("<div class=\"ctx\">FAMILIES</div>");
    for fam in CANON_FAMILY_ORDER {
        ctx.push_str(&format!(
            "<a class=\"navitem anchor{}\" href=\"/why?topic={}\">{}</a>",
            if topic == fam { " active" } else { "" },
            fam,
            esc(c["families"][fam]["title"].as_str().unwrap_or(fam))
        ));
    }
    Response::html(
        200,
        layout_ctx(
            state,
            session,
            "Why it suggests",
            "Why it suggests",
            &ctx,
            &body,
        ),
    )
}

/// Learn the tool — the manual. What the tool is, how its pages
/// are organised (the contracts, rendered for players), how
/// changes work, how to read its data, and the command-line
/// surface. Static reference text; no live data except the
/// install's own facts where noted.
fn page_manual(state: &State, session: &Session) -> Response {
    let mut body = page_head(
        "Guidance",
        "Learn the tool",
        "How this tool works — its pages, its evidence, and its one safety rule.",
        "",
    );
    body.push_str(&contract_strip(
        "The tool itself: what it is, how it reads, how it acts",
        "Informational — a manual; the actions it describes live on their own pages",
        "Static reference text, bundled with the tool. Nothing here needs the game",
    ));
    body.push_str(&format!(
        "<div class=\"card\"><div class=\"cardhead\"><h2>What this is</h2></div>\
         <p>Superintendent is a single program that serves this website from your own machine and talks to your running game of Captain of Industry. \
         There is no account, no cloud and no telemetry: the only thing it ever contacts is your game. Its data — snapshots, the audit log, settings — \
         lives in a database file (this install: <code>{}</code>) and an optional <code>coi.toml</code> settings file. Run the program and open the address it prints \
         (by default http://127.0.0.1:8484). To let another machine on your own network reach it, see REMOTE-ACCESS.md in the toolkit — and read its warnings first.</p></div>",
        esc(&state.db_path)
    ));
    body.push_str(
        "<div class=\"card\"><div class=\"cardhead\"><h2>Run it all the time</h2></div>\
         <p>The site only exists while the program runs, so most players leave it running: it is \
         loopback-only and idles at a few megabytes of memory and no measurable CPU. On Windows, put a \
         shortcut in the Startup folder (simplest) or a Task Scheduler \u{201c}at logon\u{201d} task (most reliable); \
         on Linux a <code>systemd --user</code> service; on macOS a launchd agent. Where its data lives: on \
         Windows, <code>coi.db</code> sits beside the program; on Linux it defaults to \
         <code>~/.local/share/superintendent/</code> (or <code>$XDG_DATA_HOME</code>), with settings in \
         <code>~/.config/superintendent/coi.toml</code>; on macOS both live in \
         <code>~/Library/Application Support/Superintendent/</code>. The full recipes, with the exact \
         settings that matter, are in INSTALL.md (\u{201c}Run it all the time\u{201d}).</p>\
         <p><b>Stopping it:</b> the <b>Shutdown</b> control at the right of the top bar (two steps: it \
         tells you what will happen, then you confirm) — in-flight work finishes and the database closes \
         cleanly. From the OS: end the scheduled task, <code>systemctl --user stop superintendent</code>, \
         or <code>launchctl stop com.coi.superintendent</code>. Your game is never touched by stopping.</p></div>",
    );
    body.push_str(
        "<div class=\"card\"><div class=\"cardhead\"><h2>The pages, by domain</h2></div>\
         <p>Every page declares its contract at its head — what it owns, whether it acts or only informs, and what data it needs. The domains:</p>\
         <table class=\"table--compact\"><tbody>\
         <tr><td><b>Monitor</b></td><td>Overview, Issues, Products, Terrain — the state of your run, read live.</td></tr>\
         <tr><td><b>Workshop</b></td><td>Entities, Research, Trade, Actions, Catalogue, Place, Designate — where changes are prepared and confirmed.</td></tr>\
         <tr><td><b>Guidance</b></td><td>Learn the game (failure-mode guides), Learn the tool (this page), Why it suggests (the reasoning rules).</td></tr>\
         <tr><td><b>Plans</b></td><td>Step-by-step run plans with phase gates — reference, until a supervised session walks one.</td></tr>\
         <tr><td><b>Records</b></td><td>Snapshots and the Log — what happened, stored locally.</td></tr>\
         </tbody></table></div>",
    );
    body.push_str(
        "<div class=\"card\"><div class=\"cardhead\"><h2>How changes work — the one safety rule</h2></div>\
         <p>Nothing in your game changes silently. Every change is <b>prepared</b> (you see the exact command and its target), \
         <b>confirmed</b> by you on a dedicated sheet, then <b>applied</b> and written to the audit log with the game's own answer. \
         Command statuses mean: <b>verified</b> — executed live through this flow and observed; <b>wired</b> — preparable, not yet run; \
         <b>excluded</b> — refused by policy. One command family is excluded outright: the raw game console, which can do anything — \
         the tool will not touch it at any setting. The full statement is in the canon: <a href=\"/why?topic=philosophy#philosophy.section12\">Play philosophy → §12</a>.</p></div>",
    );
    body.push_str(
        "<div class=\"card\"><div class=\"cardhead\"><h2>How to read the numbers</h2></div>\
         <p><b>Flow windows.</b> TM = this month, LM = last month, TY/LY = this/last year. Most judgements use LM (a finished month) \
         because TM is still moving. <b>Cover / runway</b> = stock ÷ recent consumption or deficit rate, in months.</p>\
         <p><b>Bridge percentages.</b> Where CoiBridge reports a percentage it arrives as a raw value of percent × 1000; the site divides it back — \
         if a figure ever looks 1000× too large, that decoding is the suspect.</p>\
         <p><b>Evidence classes.</b> Productivity verdicts carry a class: <b>Measured</b> (dense snapshots across most of a month), \
         <b>Estimated</b> (sparser — directionally trustworthy), or <b>Insufficient</b> (no verdict is given at all). Taking snapshots regularly is what upgrades your evidence.</p>\
         <p><b>The must-fix badge</b> (top right) counts the board's Fix now quadrant. When the game is off it shows the last known count, labelled as such — never dressed as live.</p></div>",
    );
    body.push_str(
        "<div class=\"card\"><div class=\"cardhead\"><h2>Beyond the website: the command line</h2></div>\
         <p>The same program has a small command-line surface for scripted checks: <code>plan-check</code> evaluates a bundled plan against a saved state file; \
         <code>play</code> prepares a supervised plan session (prepare-only — it cannot apply anything); <code>snapshot</code> takes a snapshot without the website; \
         <code>serve</code> is the website itself. The website is the primary surface; the CLI never bypasses the confirm rule.</p></div>",
    );
    Response::html(
        200,
        layout_ctx(
            state,
            session,
            "Learn the tool",
            "Learn the tool",
            &guidance_ctx(),
            &body,
        ),
    )
}

fn page_guides(state: &State, session: &Session) -> Response {
    let mut body = page_head(
        "Guidance",
        "Learn the game",
        "Failure-mode knowledge: what goes wrong, how it presents, and how to keep it from happening.",
        "",
    );
    body.push_str(&contract_strip(
        "The game's failure modes, one guide per family",
        "Informational — reading pages; each guide opens with what's live in your factory right now",
        "Guides are bundled reference text. The live panels inside them read your current game; the guidance itself never changes with your data",
    ));
    body.push_str(
        "<p class=\"sub\">Organized around the failures the issues board watches \
         for. Researched from the official wiki and community practice — no invented figures, \
         and worked examples are framed as examples. The other two kinds of guidance: \
         <a href=\"/manual\">Learn the tool</a> (how this site works) and \
         <a href=\"/why\">Why it suggests</a> (the rules behind the board).</p><div class=\"cols\">",
    );
    for g in GUIDES {
        body.push_str(&format!(
            "<div class=\"col\"><h2><a href=\"/guide?id={}\">{}</a></h2><p>{}</p><p class=\"sub\">{} sections</p></div>",
            g.id,
            esc(g.title),
            esc(g.blurb),
            g.sections.len(),
        ));
    }
    body.push_str("</div>");
    Response::html(
        200,
        layout_ctx(
            state,
            session,
            "Learn the game",
            "Learn the game",
            &guides_ctx(),
            &body,
        ),
    )
}

/// The rail context for the Library's guide pages: the guide
/// titles themselves (V2.1 tree).
fn guides_ctx() -> String {
    let mut ctx = String::from("<div class=\"ctx\">GUIDES</div>");
    for g in GUIDES {
        ctx.push_str(&rail_link(&format!("/guide?id={}", g.id), g.title));
    }
    ctx
}

fn page_guide(state: &State, session: &Session, req: &Request) -> Response {
    let Some(id) = req.query.get("id").cloned() else {
        return Response::redirect("/guides");
    };
    let Some(g) = guide_by_id(&id) else {
        return Response::html(
            404,
            layout(
                state,
                session,
                "No such guide",
                "<h1>No such guide</h1><p><a href=\"/guides\">All guides</a></p>",
            ),
        );
    };
    let mut body = page_head("Guidance", g.title, g.blurb, "");
    let mut main = String::new();
    let mut rail = String::new();
    // The Edicts guide carries the live status table (moved from
    // Trade in DESIGN V3 §2.3) as its live panel.
    if g.id == "edicts" {
        main.push_str(&edicts_status_table(state));
    }
    // Open issues right now that this guide speaks to — the "in
    // your factory right now" panel leads the main column.
    if let Ok(live) = fetch_live(&client(state)) {
        let issues = compute_issues(&live);
        let rel: Vec<&Issue> = issues
            .iter()
            .filter(|i| {
                let pid = live
                    .names
                    .iter()
                    .find(|(_, n)| *n == &i.product)
                    .map(|(k, _)| k.as_str());
                guides_for_issue(i, pid).iter().any(|x| x.id == g.id)
            })
            .collect();
        if !rel.is_empty() {
            main.push_str("<div class=\"card\"><div class=\"cardhead\"><h2>In your factory right now</h2></div>");
            for i in rel {
                main.push_str(&issue_card(i, Some(&live)));
            }
            main.push_str("</div>");
        }
    }
    for s in g.sections {
        main.push_str(&format!(
            "<div class=\"card\"><div class=\"cardhead\"><h2>{}</h2></div>{}</div>",
            esc(s.heading),
            s.body
        ));
    }
    // Wiki link-out (DESIGN V3 §2.2 policy): a labelled external
    // block at the foot, only where the guide struct declares it —
    // never inline, never for failure-mode content.
    if let Some((wtitle, wurl)) = g.wiki {
        main.push_str(&format!(
            "<div class=\"card\"><div class=\"cardhead\"><h2>On the wiki</h2></div>\
             <p class=\"sub\">The official wiki's <a href=\"{}\" target=\"_blank\" rel=\"noopener\">{} page</a> carries the full reference tables for this topic. \
             This guide stays focused on what goes wrong and how to prevent it; figures there are the wiki's, and where the two disagree the in-game tooltip wins.</p></div>",
            esc(wurl),
            esc(wtitle)
        ));
    }
    rail.push_str("<div class=\"card\"><p><a class=\"btn btn--secondary\" href=\"/guides\">← All guides</a></p><p class=\"sub\">More failure modes:</p>");
    for other in GUIDES.iter().filter(|x| x.id != g.id).take(5) {
        rail.push_str(&format!(
            "<p><a href=\"/guide?id={}\">{}</a></p>",
            other.id,
            esc(other.title)
        ));
    }
    rail.push_str("</div>");
    body.push_str(&format!(
        "<div class=\"detail\"><div>{}</div><div>{}</div></div>",
        main, rail
    ));
    Response::html(
        200,
        layout_ctx(
            state,
            session,
            g.title,
            "Learn the game",
            &guides_ctx(),
            &body,
        ),
    )
}

// ---------------------------------------------------------------------------
// CoiBridge — entity directory, issue detail, entity actions
// ---------------------------------------------------------------------------

/// Percent-encode a query value (issue titles contain spaces, arrows…).
fn url_enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Tolerant bridge fetch: any failure (mod absent, route missing,
/// bad JSON) is `None`, never an error page — the bridge is optional
/// and every consumer degrades gracefully.
fn vi(v: &Value) -> i64 {
    v.as_i64().unwrap_or(0)
}

fn bridge_json(state: &State, path: &str) -> Option<Value> {
    let r = client(state).get(path).ok()?;
    if r.status != 200 {
        return None;
    }
    serde_json::from_slice(&r.body).ok()
}

fn bridge_present(state: &State) -> bool {
    bridge_json(state, "/coi/v1/hello")
        .map(|v| v["mod"].as_str() == Some("CoiBridge"))
        .unwrap_or(false)
}

/// Bridge Percent fields are raw = percent × 1000 (verified live
/// 2026-11-02 against storage quantities and research step counts).
fn pct_raw(v: &Value) -> f64 {
    v.as_i64().unwrap_or(0) as f64 / 1000.0
}

/// Electric power panel from CoiBridge v0.6.2's GET /coi/v1/power:
/// this-tick generation / consumption / balance in exact kW
/// (Electricity.Value is kW by construction — no scaling). Returns an
/// empty string when the route is absent (older mod, or the game is
/// off): the Overview degrades silently, never an error.
fn power_data(state: &State) -> Option<Value> {
    bridge_json(state, "/coi/v1/power")
}

fn power_panel(state: &State) -> String {
    let Some(v) = power_data(state) else {
        return String::new();
    };
    power_panel_html(&v)
}

fn power_panel_html(v: &Value) -> String {
    // CoiBridge v0.6.3 averages (optional — absent on older mods, in
    // which case the panel renders exactly as before). The bridge
    // flags this decoding UNVERIFIED (`averagesUnverified`), so the
    // line says so in plain words and never presents the numbers as
    // settled fact.
    let averages_line = if v["averages"].is_object() {
        let avg = |x: &Value| -> String {
            if x.is_null() {
                "—".to_string()
            } else {
                fmt_num(x)
            }
        };
        let a = &v["averages"];
        let prod = &a["production"];
        let cons = &a["consumption"];
        let cap = &a["generationCapacity"];
        // §4.5: the averages as labelled mini-bars in the UNVERIFIED
        // colour (last-month values, shared scale), with every word
        // of the old prose line kept in the caption beneath.
        let num = |x: &Value| x.as_f64().unwrap_or(0.0);
        let scale = num(&prod["lastMonthKw"])
            .max(num(&cons["lastMonthKw"]))
            .max(num(&cap["lastMonthKw"]))
            .max(1.0);
        let bar =
            |x: &Value| -> String { fillbar(num(x) / scale * 100.0, "var(--unverified)", 160) };
        format!(
            "<div style=\"margin-top:12px\"><div class=\"kv\"><span>Monthly averages</span>{}</div>\
             <table style=\"margin:6px 0\"><tr><td>Produced</td><td>{}</td><td class=\"num\">≈{} <span class=\"u\">kW</span></td></tr>\
             <tr><td>Consumed</td><td>{}</td><td class=\"num\">≈{} <span class=\"u\">kW</span></td></tr>\
             <tr><td>Capacity</td><td>{}</td><td class=\"num\">≈{} <span class=\"u\">kW</span></td></tr></table>\
             <div class=\"caption\">Averages via CoiBridge v0.6.3 — decoding UNVERIFIED: \
             produced ≈{} kW last day · ≈{} kW last month · ≈{} kW last year; \
             consumed ≈{} kW last day · ≈{} kW last month · ≈{} kW last year; \
             generation capacity ≈{} kW last day · ≈{} kW last month · ≈{} kW last year.</div></div>",
            badge("unverified", "UNVERIFIED"),
            bar(&prod["lastMonthKw"]),
            avg(&prod["lastMonthKw"]),
            bar(&cons["lastMonthKw"]),
            avg(&cons["lastMonthKw"]),
            bar(&cap["lastMonthKw"]),
            avg(&cap["lastMonthKw"]),
            avg(&prod["lastDayKw"]),
            avg(&prod["lastMonthKw"]),
            avg(&prod["lastYearKw"]),
            avg(&cons["lastDayKw"]),
            avg(&cons["lastMonthKw"]),
            avg(&cons["lastYearKw"]),
            avg(&cap["lastDayKw"]),
            avg(&cap["lastMonthKw"]),
            avg(&cap["lastYearKw"]),
        )
    } else {
        String::new()
    };
    let balance = vi(&v["balanceKw"]);
    let cap = vi(&v["generationCapacityKw"]).max(1);
    let gen_pct = vi(&v["generatedKw"]) as f64 / cap as f64 * 100.0;
    let cons_pct = vi(&v["consumedKw"]) as f64 / cap as f64 * 100.0;
    let kw = |n: i64| format!("{} <span class=\"u\">kW</span>", fmt_num(&json!(n)));
    // V2.3 mapping: every measurement is a figure. Generated /
    // consumed / capacity are absolutes (neutral); the balance is
    // the difference and renders as a delta chip. The old
    // demanded/wasted sentence becomes the second figstrip.
    let strip1 = figstrip(&[
        fig("Generated", &kw(vi(&v["generatedKw"])), false),
        fig("Consumed", &kw(vi(&v["consumedKw"])), false),
        fig("Balance", &delta_chip(balance, ""), false),
        fig(
            "Headroom",
            &kw(vi(&v["generationCapacityKw"]) - vi(&v["generatedKw"])),
            false,
        ),
    ]);
    let strip2 = figstrip(&[
        fig("Demanded", &kw(vi(&v["demandedKw"])), false),
        fig("Surplus demand", &kw(vi(&v["surplusDemandedKw"])), false),
        fig("Wasted", &kw(vi(&v["wastedKw"])), false),
        fig("Generators", &fmt_num(&v["generators"]), false),
        fig("Consumers", &fmt_num(&v["consumers"]), false),
    ]);
    let deficit_cta = if balance < 0 {
        "<div class=\"ctas\" style=\"display:flex;gap:6px;margin-top:10px\"><a class=\"btn btn--sm\" href=\"/actions\">Open Actions…</a><a class=\"btn btn--sm btn--ghost\" href=\"/entities?view=machines\">See generators</a></div>"
    } else {
        ""
    };
    format!(
        "<div class=\"card\"><div class=\"cardhead\"><h2>Electric power</h2><span class=\"right\">{}</span></div>\
         {}\
         <div style=\"margin-top:10px\"><div class=\"caption\" style=\"margin:0 0 4px\">Generation vs capacity ({} / {} <span class=\"u\">kW</span>)</div>{}</div>\
         <div style=\"margin-top:6px\"><div class=\"caption\" style=\"margin:0 0 4px\">Consumption vs capacity</div>{}</div>\
         <div style=\"margin-top:10px\">{}</div>\
         <div class=\"caption\">This tick, via CoiBridge.</div>{}{}</div>",
        if balance >= 0 {
            badge("ok", "balanced")
        } else {
            badge("critical", "deficit")
        },
        strip1,
        vi(&v["generationCapacityKw"]),
        vi(&v["maxGenerationCapacityKw"]),
        fillbar(gen_pct, "var(--accent)", 420),
        fillbar(cons_pct, "var(--paused)", 420),
        strip2,
        deficit_cta,
        averages_line,
    )
}

fn quadrant_why(q: &str) -> &'static str {
    match q {
        "Fix now" => "High importance and short time-to-impact: the buffer runs dry (or the chain blocks) within about a quarter at current rates, and a lot downstream depends on this product. This is the quadrant where waiting costs more than acting.",
        "Fix later" => "Important, but the runway is measured in quarters, not weeks. Schedule the fix — a build, a capacity increase, a rebalance — rather than firefighting it today.",
        "Watch" => "Either the impact is small or the evidence is not yet conclusive. No action now; the companion re-checks this on every page load, and it will move quadrants if the numbers worsen.",
        _ => "Low importance and no cliff in sight — typically a deliberately parked buffer (a filled war chest) or a product almost nothing depends on. Listed so you can see it was considered, not missed.",
    }
}

/// One storage row with its logistics controls, shared by the issue
/// detail page and the Entities page. `row` is a bridge storages row.
fn storage_row_html(session: &Session, row: &Value, live: &Live) -> String {
    let sid = row["id"].as_i64().unwrap_or(0);
    let proto = sv(&row["proto"]);
    let pid = sv(&row["product"]);
    let pname = live
        .names
        .get(&pid)
        .cloned()
        .unwrap_or_else(|| pid.trim_start_matches("Product_").to_string());
    let qty = row["quantity"].as_i64().unwrap_or(0);
    let cap = row["capacity"].as_i64().unwrap_or(0);
    let pct = pct_raw(&row["percentFullRaw"]);
    let in_dis = row["logisticsInDisabled"].as_bool().unwrap_or(false);
    let out_dis = row["logisticsOutDisabled"].as_bool().unwrap_or(false);
    let mut ctl = String::new();
    for (is_input, disabled, side) in [(true, in_dis, "input"), (false, out_dis, "output")] {
        let (verb, target) = if disabled {
            ("Enable", false)
        } else {
            ("Disable", true)
        };
        ctl.push_str(&prepare_form_labeled(
            session,
            "StorageDisableLogisticsToggleCmd",
            json!({"storageId": sid, "isInput": is_input, "isDisabled": target}),
            &format!("{verb} {side} logistics on {pname} storage #{sid} ({proto}, {qty}/{cap})."),
            &format!("{verb} {side}"),
        ));
        ctl.push(' ');
    }
    // CoiBridge v0.6.1 storage extras — every key is conditional on
    // presence, so v0.5-era rows render exactly as before. The C#
    // emitter puts generalPriority on every storage row; the slider
    // steps and import/export priorities only on concrete Storage
    // rows (keys: importPriority / exportPriority, steps are 10%
    // each, percent raws decoded with pct_raw).
    let mut extra = String::new();
    if !row["generalPriority"].is_null() {
        extra.push_str(&format!(
            "<div class=\"sub\">General priority: {}</div>",
            vi(&row["generalPriority"])
        ));
    }
    if !row["importPriority"].is_null() {
        extra.push_str(&format!(
            "<div class=\"sub\">Import priority: {} · export priority: {}</div>",
            vi(&row["importPriority"]),
            vi(&row["exportPriority"]),
        ));
    }
    if !row["importStep"].is_null() {
        extra.push_str(&format!(
            "<div class=\"sub\">Import until step {} ({:.1}%) · export from step {} ({:.1}%) · \
             transport {}–{} ({:.1}%–{:.1}%)</div>",
            vi(&row["importStep"]),
            pct_raw(&row["importUntilPercentRaw"]),
            vi(&row["exportStep"]),
            pct_raw(&row["exportFromPercentRaw"]),
            vi(&row["transportFrom"]),
            vi(&row["transportUntil"]),
            pct_raw(&row["transportFromPercentRaw"]),
            pct_raw(&row["transportUntilPercentRaw"]),
        ));
    }
    format!(
        "<tr><td class=\"mono\">#{sid}</td><td>{}</td><td><a href=\"/product?id={}\">{}</a></td><td class=\"num\">{} / {} {}</td><td class=\"num\">{:.1}%</td><td>{}</td><td>{}</td><td>{}{}</td></tr>",
        esc(&proto),
        esc(&pid),
        esc(&pname),
        qty,
        cap,
        fillbar(
            pct,
            if pct >= 90.0 { "var(--blocked)" } else { "var(--accent)" },
            64
        ),
        pct,
        if in_dis {
            badge("warning", "input OFF")
        } else {
            badge("paused", "input on")
        },
        if out_dis {
            badge("warning", "output OFF")
        } else {
            badge("paused", "output on")
        },
        ctl,
        extra,
    )
}

fn page_issue(state: &State, session: &Session, req: &Request) -> Response {
    let (Some(p), Some(t)) = (req.query.get("p").cloned(), req.query.get("t").cloned()) else {
        return Response::redirect("/issues");
    };
    let c = client(state);
    let live = match fetch_live(&c) {
        Ok(l) => l,
        Err(e) => return game_unreachable(state, session, &e),
    };
    let issues = compute_issues(&live);
    let Some(issue) = issues.iter().find(|i| i.product == p && i.title == t) else {
        // Gone-state (DESIGN V3 §1.2): say when it was last seen in
        // a stored snapshot, when the store knows.
        let last_seen = store(state)
            .ok()
            .and_then(|st| st.issue_last_seen(&p, &t).ok().flatten())
            .map(|(sid, taken, date)| {
                format!(
                    " Last seen in snapshot #{} ({} · game date {}).",
                    sid,
                    fmt_clock(taken as u64),
                    esc(&date)
                )
            })
            .unwrap_or_default();
        return Response::html(
            200,
            layout(state, session,
                "Issue gone",
                &format!(
                    "{}<div class=\"empty\">That issue is no longer present — the live numbers have moved since the board was rendered; it may have resolved, or changed quadrant.{}</div><p><a class=\"btn btn--secondary\" href=\"/issues\">Back to the board</a></p>",
                    page_head("Monitor", "Issue gone", "This finding is not on the live board any more.", ""),
                    last_seen
                ),
            ),
        );
    };
    let pid = live
        .names
        .iter()
        .find(|(_, n)| *n == &issue.product)
        .map(|(k, _)| k.clone());
    let forecasts = compute_forecasts(&live);
    let fc = forecasts.iter().find(|f| f.product == issue.product);
    let cls = match issue.quadrant {
        "Fix now" => "fn",
        "Fix later" => "fl",
        "Watch" => "wa",
        _ => "ig",
    };
    let (_, qvariant) = quadrant_style(issue.quadrant);
    let mut body = page_head(
        "Monitor",
        &issue.title,
        "Why this finding fired, the numbers behind it, and the safest next step.",
        &badge(qvariant, issue.quadrant),
    );
    // Detail layout (DESIGN §5): main column + right rail; the wide
    // per-entity Act tables follow full-width below the grid.
    body.push_str("<div class=\"detail\"><div>");
    body.push_str(&format!(
        "<div class=\"card {cls}\" id=\"why\"><div class=\"cardhead\"><h2>Why \u{2018}{}\u{2019}?</h2></div><p>{}</p>{}</div>",
        esc(issue.quadrant),
        quadrant_why(issue.quadrant),
        canon_strip_for_issue(issue),
    ));
    // The numbers behind it.
    if let Some(id) = &pid {
        let s = &live.stats["products"][id];
        if !s.is_null() {
            let qty = s["quantity"].as_i64().unwrap_or(0);
            let stored = s["stored"].as_i64().unwrap_or(0);
            let cap = s["storageCapacity"].as_i64().unwrap_or(0);
            let net_lm = crate::flow_at(&s["produced"], 1) - crate::flow_at(&s["consumed"], 1);
            let cons_lm = crate::flow_at(&s["consumed"], 1);
            // Headline figures (V2.3): Stock · Stored/Cap · Net LM
            // (a difference — delta chip) · Cover. The produced /
            // consumed TM/LM pairs are genuine pairs and stay .kv.
            let cover = if net_lm < 0 {
                Some(qty as f64 / (-net_lm) as f64)
            } else if cons_lm > 0 {
                Some(qty as f64 / cons_lm as f64)
            } else {
                None
            };
            let pct = if cap > 0 {
                stored as f64 / cap as f64 * 100.0
            } else {
                0.0
            };
            body.push_str("<div class=\"stats2\" id=\"numbers\">");
            body.push_str(&stat2_tile(
                "box",
                "Stock",
                &fmt_num(&s["quantity"]),
                "",
                false,
            ));
            body.push_str(&stat2_tile(
                "layers",
                "Stored / cap",
                &format!(
                    "{} <span class=\"u\">/ {}</span>",
                    fmt_num(&s["stored"]),
                    fmt_num(&s["storageCapacity"])
                ),
                &fillbar(
                    pct,
                    if pct >= 90.0 {
                        "var(--warning)"
                    } else {
                        "var(--accent)"
                    },
                    140,
                ),
                false,
            ));
            body.push_str(&stat2_tile(
                "chart",
                "Net LM",
                &delta_chip(net_lm, "LM"),
                "",
                false,
            ));
            body.push_str(&stat2_tile(
                "clock",
                "Cover",
                &match cover {
                    Some(m) => format!("{m:.1} <span class=\"u\">months</span>"),
                    None => "—".to_string(),
                },
                "",
                cover.map(|m| m < 3.0).unwrap_or(false),
            ));
            body.push_str("</div>");
            body.push_str(&format!(
                "<div class=\"card\"><div class=\"cardhead\"><h2>Flows</h2></div>\
                 <div class=\"kvrow\"><span class=\"k\">Produced TM/LM</span><span>{}</span></div>\
                 <div class=\"kvrow\"><span class=\"k\">Consumed TM/LM</span><span>{}</span></div></div>",
                flow_pair(&s["produced"]),
                flow_pair(&s["consumed"]),
            ));
        }
        // On-demand monthly history for this product (pinned or not).
        if let Ok(h) = get_json(
            &c,
            &format!("/api/v1/products/history?ids={id}&resolution=month&series=produced,consumed,quantity&count=8"),
        ) {
            if let Some(q) = h["products"][id]["quantity"].as_array() {
                let n = q.len();
                let keep = n.saturating_sub(8);
                let pts: Vec<f64> = q.iter().skip(keep).map(|v| v.as_f64().unwrap_or(0.0)).collect();
                body.push_str(&format!(
                    "<div class=\"card\"><div class=\"cardhead\"><h2>Monthly history</h2></div>{}",
                    svg_sparkline_big(&pts, "var(--accent)")
                ));
                body.push_str("<table><thead><tr><th>Month</th><th class=\"num\">Produced</th><th class=\"num\">Consumed</th><th class=\"num\">Quantity</th></tr></thead><tbody>");
                for (i, qty) in q.iter().enumerate().skip(keep) {
                    body.push_str(&format!(
                        "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
                        if i + 1 == n { "current (partial)".to_string() } else { format!("-{}", n - 1 - i) },
                        fmt_num(&h["products"][id]["produced"][i]),
                        fmt_num(&h["products"][id]["consumed"][i]),
                        fmt_num(qty),
                    ));
                }
                body.push_str("</tbody></table></div>");
            }
        }
    }
    body.push_str(&format!(
        "<div class=\"card\" id=\"evidence\"><div class=\"cardhead\"><h2>Evidence</h2></div><p>{}</p></div>",
        esc(&issue.evidence)
    ));
    if let Some(f) = fc {
        let label = match f.kind {
            "fill" => format!("storage full in ≈ {:.1} months", f.months),
            "stall-runway" => format!("stock lasts ≈ {:.1} months (production stalled)", f.months),
            _ => format!("stock lasts ≈ {:.1} months", f.months),
        };
        body.push_str(&format!(
            "<div class=\"card\" id=\"outlook\"><div class=\"cardhead\"><h2>Outlook</h2></div><p>{} <b>{}</b></p><div class=\"caption\">{}</div></div>",
            svg_runway(f.months, f.kind),
            esc(&label),
            esc(&f.basis),
        ));
    }
    body.push_str(&format!(
        "<div class=\"card\" id=\"fix\"><div class=\"cardhead\"><h2>How to fix it</h2></div><p>{}</p></div>",
        esc(&issue.fix)
    ));
    body.push_str("</div><div>");
    // Rail: product link card, guide buttons, forecast advice.
    if let Some(id) = &pid {
        body.push_str(&format!(
            "<div class=\"card\"><div class=\"cardhead\"><h2>Product</h2></div><p><a href=\"/product?id={}\">{}</a></p><p class=\"sub mono\">{}</p></div>",
            esc(id),
            esc(&issue.product),
            esc(id),
        ));
    }
    let gl = guides_for_issue(issue, pid.as_deref());
    if !gl.is_empty() {
        body.push_str("<div class=\"card\"><div class=\"cardhead\"><h2>Guides for this failure mode</h2></div><p>");
        for g in gl {
            body.push_str(&format!(
                "<a class=\"btn btn--secondary btn--sm\" href=\"/guide?id={}\">{}</a> ",
                g.id,
                esc(g.title),
            ));
        }
        body.push_str("</p></div>");
    }
    if let Some(f) = fc {
        if f.advice != issue.fix {
            body.push_str(&format!(
                "<div class=\"callout callout--info\"><b>Forecast advice.</b> {}</div>",
                esc(&f.advice)
            ));
        }
    }
    body.push_str("</div></div>");
    // Act on it: pin toggle + bridge-driven controls for the actual
    // storages holding this product and the machines producing it.
    body.push_str("<h2>Act on it</h2>");
    if let Some(id) = &pid {
        let pinned = live.pinned_ids.iter().any(|x| x == id);
        body.push_str(&format!(
            "<div class=\"panel\">{} {} (the game shows pinned products on its own dashboard).</div>",
            prepare_form(
                session,
                "PinToggleCmd",
                json!({"productToToggle": id}),
                if pinned { "Unpin from the game dashboard" } else { "Pin to the game dashboard" },
                false,
            ),
            if pinned { "Currently pinned." } else { "Not currently pinned." },
        ));
        if bridge_present(state) {
            if let Some(storages) = bridge_json(state, "/coi/v1/storages") {
                let rows: Vec<&Value> = storages["storages"]
                    .as_array()
                    .map(|a| a.iter().filter(|r| sv(&r["product"]) == *id).collect())
                    .unwrap_or_default();
                if !rows.is_empty() {
                    body.push_str(&format!(
                        "<h3>Storages holding {} ({})</h3><table><tr><th>Id</th><th>Storage</th><th>Product</th><th>Qty / cap</th><th>Full</th><th>Input</th><th>Output</th><th>Logistics</th></tr>",
                        esc(&issue.product),
                        rows.len()
                    ));
                    for r in rows.iter().take(16) {
                        body.push_str(&storage_row_html(session, r, &live));
                    }
                    body.push_str("</table>");
                    if rows.len() > 16 {
                        body.push_str(&format!(
                            "<p class=\"sub\">…and {} more on the <a href=\"/entities\">Entities page</a>.</p>",
                            rows.len() - 16
                        ));
                    }
                }
            }
            // Machines producing this product this month (vanilla
            // sources give the machine protos; the bridge census +
            // topology give the instances to address).
            let mut machine_protos: Vec<String> = Vec::new();
            if let Ok(src) = get_json(
                &c,
                &format!("/api/v1/products/sources?ids={id}&period=thisMonth"),
            ) {
                if let Some(rows) = src["products"][id]["produced"]["rows"].as_array() {
                    for r in rows {
                        if r["type"].as_str() == Some("recipe") {
                            let m = sv(&r["machine"]);
                            if !m.is_empty() && !machine_protos.contains(&m) {
                                machine_protos.push(m);
                            }
                        }
                    }
                }
            }
            if !machine_protos.is_empty() {
                if let (Some(census), Some(topo)) = (
                    bridge_json(state, "/coi/v1/census"),
                    bridge_json(state, "/coi/v1/topology"),
                ) {
                    for proto in &machine_protos {
                        let recipes: Vec<String> = census["machines"][proto]["recipes"]
                            .as_array()
                            .map(|a| a.iter().map(sv).collect())
                            .unwrap_or_default();
                        let states = &census["machines"][proto]["states"];
                        let instances: Vec<&Value> = topo["entities"]
                            .as_array()
                            .map(|a| a.iter().filter(|e| sv(&e["proto"]) == *proto).collect())
                            .unwrap_or_default();
                        if instances.is_empty() {
                            continue;
                        }
                        body.push_str(&format!(
                            "<h3>{} — {} instance(s)</h3>",
                            esc(proto),
                            instances.len()
                        ));
                        if !states.is_null() {
                            body.push_str(&format!(
                                "<p class=\"sub\">States across all instances: {}</p>",
                                esc(&states.to_string())
                            ));
                        }
                        body.push_str("<table><tr><th>Id</th><th>Position</th><th>Pause / resume</th><th>Recipes (toggle)</th></tr>");
                        for e in instances.iter().take(12) {
                            let eid = e["id"].as_i64().unwrap_or(0);
                            let pos = format!(
                                "{},{}",
                                e["pos"][0].as_i64().unwrap_or(0),
                                e["pos"][1].as_i64().unwrap_or(0)
                            );
                            let mut btns = String::new();
                            for r in &recipes {
                                btns.push_str(&prepare_form_labeled(
                                    session,
                                    "MachineToggleRecipeActiveCmd",
                                    json!({"machineId": eid, "recipeId": r}),
                                    &format!("Toggle recipe {r} on {proto} #{eid} (at {pos})."),
                                    r,
                                ));
                                btns.push(' ');
                            }
                            if btns.is_empty() {
                                btns = "<span class=\"sub\">—</span>".into();
                            }
                            body.push_str(&format!(
                                "<tr><td>#{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                                eid,
                                esc(&pos),
                                prepare_form_labeled(
                                    session,
                                    "ToggleEnabledCmd",
                                    json!({"entityId": eid}),
                                    &format!("Pause / resume {proto} #{eid} (at {pos}) — toggles its enabled state."),
                                    "Toggle",
                                ),
                                btns,
                            ));
                        }
                        body.push_str("</table>");
                        if instances.len() > 12 {
                            body.push_str(&format!(
                                "<p class=\"sub\">Showing 12 of {} — the full list is on the <a href=\"/entities?proto={}\">Entities page</a>.</p>",
                                instances.len(),
                                url_enc(proto)
                            ));
                        }
                    }
                }
            }
        } else {
            body.push_str("<div class=\"panel\">Install the CoiBridge mod and this page gains buttons for the exact storages and machines involved (logistics toggles, pause/resume, recipe toggles) — each still behind the prepare → confirm step.</div>");
        }
    }
    body.push_str("<p class=\"sub\"><a href=\"/issues\">← Back to the board</a></p>");
    let ctx = "<div class=\"ctx\">ON THIS PAGE</div>".to_string()
        + &rail_link("#why", "Why this quadrant")
        + &rail_link("#numbers", "The numbers")
        + &rail_link("#evidence", "Evidence")
        + &rail_link("#fix", "Fix panel");
    Response::html(
        200,
        layout_ctx(state, session, "Issue detail", "Issues", &ctx, &body),
    )
}

/// Machines per instance (CoiBridge v0.6.4's GET /coi/v1/machines):
/// one row per machine instance with its position, live state,
/// recipe(s) and paused/enabled flags. Degrades to an "update the
/// mod" note when the route is absent (older mod, or the bridge is
/// off). The section is rendered on every /entities path — including
/// when the game API itself did not answer — so it is computed once
/// at the top of page_entities. Malformed rows never panic: missing
/// fields render as "—".
/// Machines per-instance table, one server-side page of 60
/// (V2.1 3-viewport rule; the client filter/sort works within the
/// rendered page). `page` is 0-based.
fn machines_section(state: &State, page: usize) -> String {
    let mut s = String::from("<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Machines (per-instance)</h2></div>");
    let Some(v) = bridge_json(state, "/coi/v1/machines") else {
        s.push_str(&degraded_bridge("Per-instance machine states", "v0.6.4"));
        s.push_str("</div>");
        return s;
    };
    let txt = |v: &Value| -> String {
        match v.as_str() {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => "—".to_string(),
        }
    };
    let rows = v["machines"].as_array().cloned().unwrap_or_default();
    // Summary: total + counts per state, busiest state first.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for r in &rows {
        let st = txt(&r["state"]);
        match counts.iter_mut().find(|(name, _)| *name == st) {
            Some((_, n)) => *n += 1,
            None => counts.push((st, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut summary = format!("{} instance(s)", rows.len());
    for (st, n) in &counts {
        summary.push_str(&format!(" · {} {}", esc(st), n));
    }
    if let Some(skipped) = v["skippedRows"].as_i64() {
        if skipped > 0 {
            summary.push_str(&format!(" · {} row(s) skipped by the bridge", skipped));
        }
    }
    if rows.is_empty() {
        s.push_str("<div class=\"empty\">No machines reported by the bridge.</div></div>");
        return s;
    }
    // State dropdown options: the distinct states, alphabetical.
    let mut states: Vec<&String> = counts.iter().map(|(st, _)| st).collect();
    states.sort();
    let mut opts = String::from("<option value=\"\">all states</option>");
    for st in states {
        opts.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            esc(st),
            esc(st)
        ));
    }
    let pages = rows.len().div_ceil(60).max(1);
    let page = page.min(pages - 1);
    s.push_str(&format!(
        "<div class=\"filterbar\"><input type=\"text\" id=\"mfilter\" placeholder=\"🔍 proto, id, recipe…\" oninput=\"filterMachines()\">\
         <label>State <select id=\"mstate\" onchange=\"filterMachines()\">{}</select></label>\
         <span class=\"count\">{} · page {} of {}</span></div>\
         <table id=\"mtable\" class=\"table--compact\"><thead><tr><th>Id</th><th>Proto</th><th>Pos</th><th>State</th><th>Recipe(s)</th><th>Paused</th><th>Enabled</th></tr></thead><tbody>",
        opts,
        summary,
        page + 1,
        pages
    ));
    let flag = |v: &Value| -> &'static str {
        match v.as_bool() {
            Some(true) => "yes",
            Some(false) => "no",
            None => "—",
        }
    };
    for r in rows.iter().skip(page * 60).take(60) {
        let id = match r["id"].as_i64() {
            Some(n) => format!("#{n}"),
            None => "—".to_string(),
        };
        let pos = match r["pos"].as_array() {
            Some(a) if !a.is_empty() => a
                .iter()
                .map(|c| {
                    c.as_i64()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "—".to_string())
                })
                .collect::<Vec<_>>()
                .join(", "),
            _ => "—".to_string(),
        };
        let recipes_full = match r["recipes"].as_array() {
            Some(a) if !a.is_empty() => a
                .iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            _ => "—".to_string(),
        };
        // Row-height discipline (DESIGN V3): long recipe lists wrap a
        // row to 4–5 lines; show the first three and count the rest —
        // the full list is the title text and lives on the proto page.
        let recipes = {
            let parts: Vec<&str> = recipes_full.split(", ").collect();
            if parts.len() > 3 {
                format!("{} +{} more", parts[..3].join(", "), parts.len() - 3)
            } else {
                recipes_full.clone()
            }
        };
        let state_txt = txt(&r["state"]);
        // State cell as a token badge (DESIGN §1.1 binding); the
        // row's data-state keeps the raw state for the filter JS.
        let state_badge = match state_txt.as_str() {
            "Working" => badge("ok", "Working"),
            "NotEnoughInput" => badge("starved", "Starved"),
            "OutputFull" => badge("blocked", "Blocked"),
            "Paused" => badge("paused", "Paused"),
            other => badge("paused", other),
        };
        s.push_str(&format!(
            "<tr data-state=\"{}\"><td class=\"mono\">{}</td><td>{}</td><td class=\"mono\">{}</td><td>{}</td><td title=\"{}\">{}</td><td>{}</td><td>{}</td></tr>",
            esc(&state_txt),
            esc(&id),
            esc(&txt(&r["proto"])),
            esc(&pos),
            state_badge,
            esc(&recipes_full),
            esc(&recipes),
            flag(&r["isPaused"]),
            flag(&r["isEnabled"]),
        ));
    }
    s.push_str("</tbody></table>");
    if pages > 1 {
        let mut pager = String::new();
        if page > 0 {
            pager.push_str(&format!(
                "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=machines&page={}\">← Prev</a> ",
                page - 1
            ));
        }
        if page + 1 < pages {
            pager.push_str(&format!(
                "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=machines&page={}\">Next →</a>",
                page + 1
            ));
        }
        s.push_str(&format!("<div class=\"pager\">{pager}</div>"));
    }
    s.push_str("</div>");
    s.push_str(MACHINES_JS);
    s
}

const MACHINES_JS: &str = "<script>
function filterMachines(){var q=document.getElementById('mfilter').value.toLowerCase();var st=document.getElementById('mstate').value;document.querySelectorAll('#mtable tbody tr').forEach(function(r){var ok=r.textContent.toLowerCase().indexOf(q)>=0;if(ok&&st!==''){ok=r.getAttribute('data-state')===st}r.style.display=ok?'':'none'})}
</script>";

/// Terrain page (CoiBridge v0.6.5): the land itself — height
/// summary + heatmap, an exact-region heatmap with the earthworks
/// estimator, on-map deposit totals and regions, and terrain
/// designations. Terrain (x, y) is the same grid as every entity
/// position the companion serves; heights are in tiles
/// (1 tile = 2 m; one tile-cubed = 8 m3).
fn page_terrain(state: &State, session: &Session, req: &Request) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            let mut html = page_head(
                "Monitor",
                "Terrain",
                "The land: heights, what's in it, and what's designated on it.",
                "",
            );
            html.push_str(&contract_strip(
                "The map itself — read-only",
                "Informational — earthworks are planned and applied on Designate, not here",
                "Bridge floors, stated plainly: terrain heights and deposits need CoiBridge 0.6.5+; exact per-cell verdicts after a designation job need 0.6.6+",
            ));
            html.push_str(&degraded_game(&e));
            return Response::html(200, layout(state, session, "Terrain", &html));
        }
    };
    let mut body = page_head(
        "Monitor",
        "Terrain",
        "The land: heights, what's in it, and what's designated on it.",
        "",
    );
    body.push_str(&contract_strip(
        "The map itself — read-only",
        "Informational — earthworks are planned and applied on Designate, not here",
        "Bridge floors, stated plainly: terrain heights and deposits need CoiBridge 0.6.5+; exact per-cell verdicts after a designation job need 0.6.6+",
    ));
    if live.terrain.is_null() {
        body.push_str(&degraded_bridge(
            "Terrain, deposit and designation data",
            "0.6.5",
        ));
        return Response::html(200, layout(state, session, "Terrain", &body));
    }
    let t = &live.terrain;
    let (map_w, map_h) = (vi(&t["width"]), vi(&t["height"]));
    let q = &req.query;
    let qint = |k: &str| q.get(k).and_then(|s| s.parse::<i64>().ok());
    let (rx0, ry0, rx1, ry1) = (qint("rx0"), qint("ry0"), qint("rx1"), qint("ry1"));
    let target: Option<f64> = q.get("target").and_then(|s| s.parse::<f64>().ok());

    // Headline tiles.
    let mut tiles = String::from("<div class=\"stats2\">");
    tiles.push_str(&stat2_tile(
        "layers",
        "Map",
        &format!("{} × {}", fmt_num(&t["width"]), fmt_num(&t["height"])),
        &format!(
            "playable x {}–{}, y {}–{}",
            vi(&t["limits"]["minX"]),
            vi(&t["limits"]["maxXExcl"]),
            vi(&t["limits"]["minY"]),
            vi(&t["limits"]["maxYExcl"])
        ),
        false,
    ));
    tiles.push_str(&stat2_tile(
        "chart",
        "Height range (tiles)",
        &format!(
            "{} → {}",
            fmt_tiles(vi(&t["minHeightRaw"])),
            fmt_tiles(vi(&t["maxHeightRaw"]))
        ),
        "1 tile = 2 m · a building's Z is the terrain height at its tile (Earthworks guide)",
        false,
    ));
    tiles.push_str(&stat2_tile(
        "anchor",
        "Ocean tiles",
        &fmt_num(&t["oceanTiles"]),
        &format!("of {} tiles", fmt_num(&t["tilesCount"])),
        false,
    ));
    tiles.push_str("</div>");
    body.push_str(&tiles);

    // Region fetch (on demand — exact heights are never pulled
    // per page view without the user asking for a rectangle).
    let region = match (rx0, ry0, rx1, ry1) {
        (Some(x0), Some(y0), Some(x1), Some(y1)) => {
            let path = format!("/coi/v1/terrain/region?x0={x0}&y0={y0}&x1={x1}&y1={y1}");
            bridge_json(state, &path).and_then(|v| crate::terrain::region_from_json(&v))
        }
        _ => None,
    };

    // Heatmap + region/estimator form card.
    {
        let (grid, gw, gh, caption) = match &region {
            Some(r) => (
                r.heights.clone(),
                r.width(),
                r.height(),
                format!(
                    "Exact heights for x {}–{}, y {}–{}{}.",
                    r.x0,
                    r.x1,
                    r.y0,
                    r.y1,
                    if r.truncated {
                        " (bridge capped the rectangle at 16,384 tiles)"
                    } else {
                        ""
                    }
                ),
            ),
            None => {
                let raw = t["summaryHeightsRaw"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let heights: Vec<f64> = raw
                    .iter()
                    .map(|v| v.as_i64().unwrap_or(0) as f64 / crate::terrain::RAW_PER_TILE)
                    .collect();
                (
                    heights,
                    vi(&t["summaryWidth"]) as usize,
                    vi(&t["summaryHeight"]) as usize,
                    format!(
                        "Whole-map summary, point-sampled every {} tiles. Pick a rectangle below for exact heights.",
                        vi(&t["summaryStride"])
                    ),
                )
            }
        };
        let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
        for &v in &grid {
            if v < mn {
                mn = v;
            }
            if v > mx {
                mx = v;
            }
        }
        if !grid.is_empty() {
            body.push_str(&format!(
                "<div class=\"card\" id=\"heightmap\"><div class=\"cardhead\"><h2>Height map</h2></div>\
                 <p class=\"sub\">{}</p>{}<p class=\"caption\">North (y = 0) is up.</p>{}</div>",
                esc(&caption),
                figstrip(&[
                    fig("Min height", &format!("{mn:.2} <span class=\"u\">tiles</span>"), false),
                    fig("Max height", &format!("{mx:.2} <span class=\"u\">tiles</span>"), false),
                    fig("Tiles", &fmt_num(&json!(grid.len())), false),
                ]),
                svg_heatmap(&grid, gw, gh, mn, mx),
            ));
        }
        // Region + target form.
        body.push_str(&format!(
            "<div class=\"card\" id=\"region\"><div class=\"cardhead\"><h2>Region &amp; earthworks</h2></div>\
             <form method=\"get\" action=\"/terrain\">\
             <div class=\"form-grid\">\
             <div class=\"field\"><label for=\"tr-x0\">x from</label><input id=\"tr-x0\" type=\"number\" name=\"rx0\" value=\"{}\" required></div>\
             <div class=\"field\"><label for=\"tr-y0\">y from</label><input id=\"tr-y0\" type=\"number\" name=\"ry0\" value=\"{}\" required></div>\
             <div class=\"field\"><label for=\"tr-x1\">x to (exclusive)</label><input id=\"tr-x1\" type=\"number\" name=\"rx1\" value=\"{}\" required></div>\
             <div class=\"field\"><label for=\"tr-y1\">y to (exclusive)</label><input id=\"tr-y1\" type=\"number\" name=\"ry1\" value=\"{}\" required></div>\
             <div class=\"field\"><label for=\"tr-t\">target height (tiles)</label><input id=\"tr-t\" type=\"text\" name=\"target\" value=\"{}\" placeholder=\"e.g. 12.5\"></div>\
             <div class=\"field\"><label>&nbsp;</label><button type=\"submit\">Show region</button></div></div></form>\
             <p class=\"sub\">Exact heights are fetched on demand; the bridge caps a region at 16,384 tiles. With a target height, the estimator sums the height deltas: cut = above target (mine away), fill = below target (dump). Volumes in tiles-cubed and m³ (1 tile³ = 8 m³).</p></div>",
            rx0.unwrap_or(vi(&t["limits"]["minX"])),
            ry0.unwrap_or(vi(&t["limits"]["minY"])),
            rx1.unwrap_or(map_w),
            ry1.unwrap_or(map_h),
            target.map(|v| v.to_string()).unwrap_or_default(),
        ));
        if let (Some(r), Some(tg)) = (&region, target) {
            let ew = crate::terrain::estimate(&r.heights, tg);
            let mut tiles = String::from("<div class=\"stats2\">");
            tiles.push_str(&stat2_tile(
                "layers",
                "Cut — mine away",
                &format!("{:.0} <span class=\"u\">m³</span>", ew.cut_m3()),
                &format!(
                    "{:.1} tiles³ over {} tiles above target",
                    ew.cut_tiles3, ew.tiles_above
                ),
                false,
            ));
            tiles.push_str(&stat2_tile(
                "layers",
                "Fill — dump",
                &format!("{:.0} <span class=\"u\">m³</span>", ew.fill_m3()),
                &format!(
                    "{:.1} tiles³ over {} tiles below target",
                    ew.fill_tiles3, ew.tiles_below
                ),
                false,
            ));
            tiles.push_str(&stat2_tile(
                "check",
                "Already at target",
                &format!("{}", ew.tiles_at),
                &format!("of {} tiles in the region", ew.tiles),
                false,
            ));
            tiles.push_str("</div>");
            body.push_str(&format!(
                "<div class=\"card\"><div class=\"cardhead\"><h2>Earthworks to flatten at {:.2} tiles</h2></div>{}</div>",
                tg, tiles
            ));
        } else if region.is_some() {
            body.push_str("<div class=\"callout\"><b>Region loaded.</b> Add a target height to run the earthworks estimator over it.</div>");
        } else if target.is_some() {
            body.push_str("<div class=\"callout callout--danger\"><b>No region.</b> The estimator needs a rectangle the bridge could serve — check the bounds against the map size above.</div>");
        }
    }

    // Deposits: on-map resources are terrain material layers.
    if live.deposits.is_null() {
        body.push_str(&degraded_bridge("Deposit data", "0.6.5"));
    } else {
        let d = &live.deposits;
        let name_of = |pid: &str| {
            live.names
                .get(pid)
                .cloned()
                .unwrap_or_else(|| pid.trim_start_matches("Product_").to_string())
        };
        if let Some(mats) = d["materials"].as_array() {
            body.push_str("<div class=\"card tablecard\" id=\"deposits\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>On-map resources — totals by material</h2></div><table class=\"table--compact\"><thead><tr><th>Product</th><th>Material</th><th class=\"num\">Tiles</th><th class=\"num\">Remaining</th></tr></thead><tbody>");
            for m in mats {
                body.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
                    esc(&name_of(m["product"].as_str().unwrap_or(""))),
                    esc(m["material"].as_str().unwrap_or("")),
                    fmt_num(&m["tiles"]),
                    fmt_num(&m["quantity"]),
                ));
            }
            body.push_str("</tbody></table><p class=\"sub\" style=\"padding:0 16px 12px\">Remaining quantities use the game's own mined-quantity conversion over today's layers — mining removes layer thickness, so this is what is left, bedrock excluded.</p></div>");
        }
        if let Some(deps) = d["deposits"].as_array() {
            body.push_str("<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Deposit regions</h2></div><table class=\"table--compact\"><thead><tr><th>Product</th><th>Bounds (x, y)</th><th class=\"num\">Tiles</th><th class=\"num\">Remaining</th></tr></thead><tbody>");
            let shown = deps.len().min(200);
            for dep in &deps[..shown] {
                body.push_str(&format!(
                    "<tr><td>{}</td><td>{}–{}, {}–{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
                    esc(&name_of(dep["product"].as_str().unwrap_or(""))),
                    vi(&dep["minX"]),
                    vi(&dep["maxX"]),
                    vi(&dep["minY"]),
                    vi(&dep["maxY"]),
                    fmt_num(&dep["tiles"]),
                    fmt_num(&dep["quantity"]),
                ));
            }
            body.push_str("</tbody></table>");
            if deps.len() > shown {
                body.push_str(&format!(
                    "<p class=\"sub\" style=\"padding:0 16px 12px\">Showing the first {} of {} regions (bridge order: by material, then discovery).</p>",
                    shown,
                    deps.len()
                ));
            }
            if d["truncated"].as_bool().unwrap_or(false) {
                body.push_str("<p class=\"sub\" style=\"padding:0 16px 12px\">The bridge capped the region list; the totals above stay complete.</p>");
            }
            body.push_str("</div>");
        }
    }

    // Designations (mining / dumping areas the player marked).
    match bridge_json(state, "/coi/v1/designations") {
        Some(dz) => {
            let rows = dz["designations"].as_array().cloned().unwrap_or_default();
            body.push_str("<div class=\"card tablecard\" id=\"designations\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Terrain designations</h2></div>");
            if rows.is_empty() {
                body.push_str("<div class=\"empty\">No mining or dumping designations are currently marked.</div>");
            } else {
                body.push_str("<table class=\"table--compact\"><thead><tr><th>Kind</th><th>Area (x, y)</th><th class=\"num\">Tiles</th><th>Target height</th><th>State</th><th class=\"num\">Jobs</th></tr></thead><tbody>");
                for r in &rows {
                    let kind = r["kind"].as_str().unwrap_or("other");
                    let kind_badge = match kind {
                        "mining" => badge("info", "Mining"),
                        "dumping" => badge("warning", "Dumping"),
                        _ => badge("paused", "Other"),
                    };
                    let state_badge = if r["isFulfilled"].as_bool().unwrap_or(false) {
                        badge("ok", "Fulfilled")
                    } else {
                        badge("starved", "Open")
                    };
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{}–{}, {}–{}</td><td class=\"num\">{}</td><td>{}–{} tiles</td><td>{}</td><td class=\"num\">{}</td></tr>",
                        kind_badge,
                        vi(&r["area"]["minX"]),
                        vi(&r["area"]["maxXExcl"]),
                        vi(&r["area"]["minY"]),
                        vi(&r["area"]["maxYExcl"]),
                        fmt_num(&r["sizeTiles"]),
                        vi(&r["minTargetHeight"]),
                        vi(&r["maxTargetHeight"]),
                        state_badge,
                        fmt_num(&r["jobsAssigned"]),
                    ));
                }
                body.push_str("</tbody></table>");
            }
            body.push_str("</div>");
        }
        None => body.push_str(&degraded_bridge("Designation data", "0.6.5")),
    }

    Response::html(200, layout(state, session, "Terrain", &body))
}

/// The rail context for Entities: the four sub-views (V2.1).
fn entities_ctx(view: &str) -> String {
    let mut ctx = String::from("<div class=\"ctx\">VIEWS</div>");
    for (v, label) in [
        ("summary", "Summary"),
        ("storages", "Storages"),
        ("machines", "Machines"),
        ("network", "Network"),
    ] {
        ctx.push_str(&format!(
            "<a class=\"navitem anchor{}\" href=\"/entities?view={}\">{}</a>",
            if view == v { " active" } else { "" },
            v,
            label
        ));
    }
    ctx
}

fn page_entities(state: &State, session: &Session, req: &Request) -> Response {
    // Sub-views (V2.1): Summary · Storages · Machines · Network
    // via ?view=. Old deep links keep working: /entities?proto=X
    // lands on Machines pre-filtered to that proto.
    let view = req
        .query
        .get("view")
        .map(|s| s.as_str())
        .unwrap_or(if req.query.contains_key("proto") {
            "machines"
        } else {
            "summary"
        })
        .to_string();
    let page: usize = req
        .query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let ctx = entities_ctx(&view);
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            let mut html = page_head(
                "Workshop",
                "Entities",
                "Every storage and machine the companion can address, with their controls.",
                "",
            );
            html.push_str(&contract_strip(
                "The addressable factory: storages, machines, the train network",
                "Actionable — control changes go prepare → confirm → audit",
                "Needs CoiBridge in the game; without it this page shows the install remedy instead of a directory",
            ));
            html.push_str(&degraded_game(&e));
            return Response::html(
                200,
                layout_ctx(state, session, "Entities", "Entities", &ctx, &html),
            );
        }
    };
    let mut body = page_head(
        "Workshop",
        "Entities",
        "Every storage and machine the companion can address, with their controls.",
        "",
    );
    body.push_str(&contract_strip(
        "The addressable factory: storages, machines, the train network",
        "Actionable — control changes go prepare → confirm → audit",
        "Needs CoiBridge in the game; without it this page shows the install remedy instead of a directory",
    ));
    if !bridge_present(state) {
        body.push_str("<div class=\"callout callout--danger\"><b>CoiBridge is not answering.</b> Install the mod (Mods\\CoiBridge), load the save with it, and run <code>web_api_start</code> — this page then shows every storage and machine the companion can address, with controls.</div>");
        return Response::html(
            200,
            layout_ctx(state, session, "Entities", "Entities", &ctx, &body),
        );
    }
    let status = bridge_json(state, "/coi/v1/status");
    let research = bridge_json(state, "/coi/v1/research");
    match view.as_str() {
        "storages" => {
            if let Some(storages) = bridge_json(state, "/coi/v1/storages") {
                let all = storages["storages"].as_array().cloned().unwrap_or_default();
                let product_filter = req.query.get("product").cloned();
                let rows: Vec<&Value> = all
                    .iter()
                    .filter(|r| match &product_filter {
                        Some(pf) => sv(&r["product"]) == *pf,
                        None => true,
                    })
                    .collect();
                let pages = rows.len().div_ceil(60).max(1);
                let page = page.min(pages - 1);
                body.push_str(&format!(
                    "<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Storages</h2></div><div class=\"filterbar\"><input type=\"text\" id=\"tfilter\" placeholder=\"🔍 product, proto, id…\" oninput=\"filterTable()\"><span class=\"count\">{} storages{} · page {} of {}</span></div>\
                     <table id=\"ptable\" class=\"table--compact\"><thead><tr><th>Id</th><th>Storage</th><th>Product</th><th class=\"num\">Qty / cap</th><th class=\"num\">Full</th><th>Input</th><th>Output</th><th>Logistics</th></tr></thead><tbody>",
                    rows.len(),
                    match &product_filter {
                        Some(pf) => format!(
                            " · filtered to {} (<a href=\"/entities?view=storages\">clear</a>)",
                            esc(live.names.get(pf).map(|s| s.as_str()).unwrap_or(pf.as_str()))
                        ),
                        None => String::new(),
                    },
                    page + 1,
                    pages
                ));
                for r in rows.iter().skip(page * 60).take(60) {
                    body.push_str(&storage_row_html(session, r, &live));
                }
                body.push_str("</tbody></table>");
                if pages > 1 {
                    let pf = match &product_filter {
                        Some(pf) => format!("&product={}", url_enc(pf)),
                        None => String::new(),
                    };
                    let mut pager = String::new();
                    if page > 0 {
                        pager.push_str(&format!(
                            "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=storages{}&page={}\">← Prev</a> ",
                            pf,
                            page - 1
                        ));
                    }
                    if page + 1 < pages {
                        pager.push_str(&format!(
                            "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=storages{}&page={}\">Next →</a>",
                            pf,
                            page + 1
                        ));
                    }
                    body.push_str(&format!("<div class=\"pager\">{pager}</div>"));
                }
                body.push_str("</div>");
            } else {
                body.push_str(&degraded_bridge("Storage data", "0.6"));
            }
        }
        "machines" => {
            body.push_str(&machines_section(state, page));
            if let (Some(census), Some(topo)) = (
                bridge_json(state, "/coi/v1/census"),
                bridge_json(state, "/coi/v1/topology"),
            ) {
                if let Some(proto) = req.query.get("proto") {
                    let instances: Vec<&Value> = topo["entities"]
                        .as_array()
                        .map(|a| a.iter().filter(|e| sv(&e["proto"]) == *proto).collect())
                        .unwrap_or_default();
                    let recipes: Vec<String> = census["machines"][proto]["recipes"]
                        .as_array()
                        .map(|a| a.iter().map(sv).collect())
                        .unwrap_or_default();
                    let pages = instances.len().div_ceil(60).max(1);
                    let page = page.min(pages - 1);
                    body.push_str(&format!(
                        "<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>{} — {} instance(s)</h2><span class=\"right\"><a href=\"/entities?view=machines\">All machine types</a></span></div><table class=\"table--compact\"><thead><tr><th>Id</th><th>Position</th><th>Pause / resume</th><th>Recipes (toggle)</th><th>General priority</th></tr></thead><tbody>",
                        esc(proto),
                        instances.len()
                    ));
                    for e in instances.iter().skip(page * 60).take(60) {
                        let eid = e["id"].as_i64().unwrap_or(0);
                        let pos = format!(
                            "{},{}",
                            e["pos"][0].as_i64().unwrap_or(0),
                            e["pos"][1].as_i64().unwrap_or(0)
                        );
                        let mut btns = String::new();
                        for r in &recipes {
                            btns.push_str(&prepare_form_labeled(
                                session,
                                "MachineToggleRecipeActiveCmd",
                                json!({"machineId": eid, "recipeId": r}),
                                &format!("Toggle recipe {r} on {proto} #{eid} (at {pos})."),
                                r,
                            ));
                            btns.push(' ');
                        }
                        if btns.is_empty() {
                            btns = "<span class=\"sub\">—</span>".into();
                        }
                        body.push_str(&format!(
                            "<tr><td>#{}</td><td>{}</td><td>{}</td><td>{}</td><td>\
                             <form method=\"post\" action=\"/actions/prepare-entity-priority\" style=\"display:inline\">\
                             <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
                             <input type=\"hidden\" name=\"entityId\" value=\"{}\">\
                             <input type=\"hidden\" name=\"label\" value=\"{}\">\
                             <input type=\"text\" name=\"priority\" value=\"0\" size=\"4\"> \
                             <button class=\"btn btn--sm btn--ghost\" type=\"submit\">Set</button></form></td></tr>",
                            eid,
                            esc(&pos),
                            prepare_form_labeled(
                                session,
                                "ToggleEnabledCmd",
                                json!({"entityId": eid}),
                                &format!("Pause / resume {proto} #{eid} (at {pos}) — toggles its enabled state."),
                                "Toggle",
                            ),
                            btns,
                            esc(&session.csrf),
                            eid,
                            esc(&format!("{proto} #{eid} (at {pos})")),
                        ));
                    }
                    body.push_str("</tbody></table>");
                    if pages > 1 {
                        let mut pager = String::new();
                        if page > 0 {
                            pager.push_str(&format!(
                                "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=machines&proto={}&page={}\">← Prev</a> ",
                                url_enc(proto),
                                page - 1
                            ));
                        }
                        if page + 1 < pages {
                            pager.push_str(&format!(
                                "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=machines&proto={}&page={}\">Next →</a>",
                                url_enc(proto),
                                page + 1
                            ));
                        }
                        body.push_str(&format!("<div class=\"pager\">{pager}</div>"));
                    }
                    body.push_str("</div>");
                } else if let Some(machines) = census["machines"].as_object() {
                    // Directory discipline (DESIGN V3): the census is a
                    // directory like the others — 60 rows per page,
                    // server-side (?cpage=, distinct from the instance
                    // table's ?page=).
                    let mut protos: Vec<&String> = machines.keys().collect();
                    protos.sort();
                    let cpages = protos.len().div_ceil(60).max(1);
                    let cpage = req
                        .query
                        .get("cpage")
                        .and_then(|p| p.parse::<usize>().ok())
                        .unwrap_or(0)
                        .min(cpages - 1);
                    // The census is the second directory on this view:
                    // collapsed by default (DESIGN V3 depth-on-demand)
                    // so the per-instance table owns the first screens.
                    body.push_str(&format!(
                        "<details class=\"indgroup\"><summary>Machine types <span class=\"sub\">· {} types, A–Z · page {} of {}</span></summary><table class=\"table--compact\"><thead><tr><th>Machine</th><th class=\"num\">Instances</th><th>States</th><th></th></tr></thead><tbody>",
                        protos.len(),
                        cpage + 1,
                        cpages
                    ));
                    for proto in protos.iter().skip(cpage * 60).take(60) {
                        let count = topo["entities"]
                            .as_array()
                            .map(|a| a.iter().filter(|e| sv(&e["proto"]) == **proto).count())
                            .unwrap_or(0);
                        // States rendered as counts, never raw JSON.
                        let states_txt = machines[*proto]["states"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| format!("{} {}", v.as_i64().unwrap_or(0), k))
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            })
                            .unwrap_or_default();
                        body.push_str(&format!(
                            "<tr><td>{}</td><td class=\"num\">{}</td><td>{}</td><td><a class=\"btn btn--ghost btn--sm\" href=\"/entities?view=machines&proto={}\">Open</a></td></tr>",
                            esc(proto),
                            count,
                            esc(&states_txt),
                            url_enc(proto),
                        ));
                    }
                    body.push_str("</tbody></table>");
                    if cpages > 1 {
                        let mut pager = String::new();
                        if cpage > 0 {
                            pager.push_str(&format!(
                                "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=machines&cpage={}\">← Prev</a> ",
                                cpage - 1
                            ));
                        }
                        if cpage + 1 < cpages {
                            pager.push_str(&format!(
                                "<a class=\"btn btn--secondary btn--sm\" href=\"/entities?view=machines&cpage={}\">Next →</a>",
                                cpage + 1
                            ));
                        }
                        body.push_str(&format!("<div class=\"pager\">{pager}</div>"));
                    }
                    body.push_str("</details>");
                }
            }
        }
        "network" => {
            // Trains only since DESIGN V3 §2.3 — non-default
            // priorities moved to Actions (a Workshop concern).
            body.push_str("<p class=\"caption\">Train lines and transport connectivity. Non-default priorities now live on <a href=\"/actions\">Actions</a>.</p>");
            if let Some(td) = bridge_json(state, "/coi/v1/trains-detail") {
                if let Some(lines) = td["lines"].as_array() {
                    if !lines.is_empty() {
                        body.push_str(&format!(
                            "<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>{} Train lines (detail)</h2></div><table class=\"table--compact\"><thead><tr><th>Id</th><th>Line</th><th>Network</th><th>Trains</th><th>Stations</th><th>Schedule</th></tr></thead><tbody>",
                            icon("train")
                        ));
                        for l in lines {
                            let net = if l["network"].is_null() {
                                "—".to_string()
                            } else {
                                format!(
                                    "{} (#{})",
                                    esc(&sv(&l["network"]["name"])),
                                    vi(&l["network"]["id"])
                                )
                            };
                            let sched = l["schedule"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .map(|i| sv(&i["type"]).to_string())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            body.push_str(&format!(
                                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                                vi(&l["id"]),
                                esc(&sv(&l["name"])),
                                net,
                                vi(&l["trains"]),
                                vi(&l["stations"]),
                                esc(&sched),
                            ));
                        }
                        body.push_str("</tbody></table></div>");
                    }
                }
            }
            if let (Some(census), Some(topo)) = (
                bridge_json(state, "/coi/v1/census"),
                bridge_json(state, "/coi/v1/topology"),
            ) {
                let total = census["transports"]["total"].as_i64().unwrap_or(0);
                let conn = census["transports"]["fullyConnected"].as_i64().unwrap_or(0);
                body.push_str(&format!(
                    "<div class=\"stats2\">{}{}{}</div>",
                    stat2_tile(
                        "factory",
                        "Entities total",
                        &fmt_num(&census["totalEntities"]),
                        "",
                        false
                    ),
                    stat2_tile("truck", "Transports", &fmt_num(&json!(total)), "", false),
                    stat2_tile(
                        "check",
                        "Fully connected",
                        &fmt_num(&json!(conn)),
                        &format!("of {} transports", fmt_num(&json!(total))),
                        false
                    ),
                ));
                let bad: Vec<&Value> = topo["transports"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|t| !t["fullyConnected"].as_bool().unwrap_or(false))
                            .collect()
                    })
                    .unwrap_or_default();
                if !bad.is_empty() {
                    body.push_str(&format!("<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Not fully connected ({})</h2></div><table class=\"table--compact\"><thead><tr><th>Id</th><th>Type</th><th>From</th><th>To</th></tr></thead><tbody>", bad.len()));
                    for t in bad.iter().take(40) {
                        let fp = |v: &Value| {
                            format!(
                                "{},{}",
                                v[0].as_i64().unwrap_or(0),
                                v[1].as_i64().unwrap_or(0)
                            )
                        };
                        body.push_str(&format!(
                            "<tr><td>#{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                            t["id"].as_i64().unwrap_or(0),
                            esc(&sv(&t["proto"])),
                            fp(&t["from"]),
                            fp(&t["to"]),
                        ));
                    }
                    body.push_str("</tbody></table></div>");
                }
            }
        }
        _ => {
            // Summary: flagged-only mini-tables + the set-product
            // form. Settlement figures (population / housing / food)
            // live on Overview and research state on Research since
            // DESIGN V3 §1.6 — this page is a directory, and its
            // summary shows only what in the directory needs eyes.
            let _ = (&status, &research);
            // Flagged storages: ≥90% full, first 10.
            if let Some(storages) = bridge_json(state, "/coi/v1/storages") {
                let rows = storages["storages"].as_array().cloned().unwrap_or_default();
                let flagged: Vec<&Value> = rows
                    .iter()
                    .filter(|r| {
                        let cap = r["capacity"].as_i64().unwrap_or(0);
                        cap > 0 && r["quantity"].as_i64().unwrap_or(0) as f64 >= 0.9 * cap as f64
                    })
                    .collect();
                if !flagged.is_empty() {
                    body.push_str("<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Storages nearly full</h2><span class=\"right\"><a href=\"/entities?view=storages\">See all →</a></span></div><table class=\"table--compact\"><thead><tr><th>Id</th><th>Storage</th><th>Product</th><th class=\"num\">Qty / cap</th><th class=\"num\">Full</th></tr></thead><tbody>");
                    for r in flagged.iter().take(10) {
                        let cap = r["capacity"].as_i64().unwrap_or(0);
                        let qty = r["quantity"].as_i64().unwrap_or(0);
                        let pctf = if cap > 0 {
                            qty as f64 / cap as f64 * 100.0
                        } else {
                            0.0
                        };
                        body.push_str(&format!(
                            "<tr><td>#{}</td><td>{}</td><td>{}</td><td class=\"num\">{} / {}</td><td class=\"num\">{:.0}%</td></tr>",
                            r["id"].as_i64().unwrap_or(0),
                            esc(&sv(&r["proto"])),
                            esc(live.names.get(&sv(&r["product"])).map(|s| s.as_str()).unwrap_or("")),
                            fmt_num(&json!(qty)),
                            fmt_num(&json!(cap)),
                            pctf,
                        ));
                    }
                    body.push_str("</tbody></table></div>");
                }
            }
            // Flagged machines: starved or blocked, first 10.
            if let Some(m) = bridge_json(state, "/coi/v1/machines") {
                let rows = m["machines"].as_array().cloned().unwrap_or_default();
                let flagged: Vec<&Value> = rows
                    .iter()
                    .filter(|r| {
                        matches!(
                            r["state"].as_str(),
                            Some("NotEnoughInput") | Some("OutputFull")
                        )
                    })
                    .collect();
                if !flagged.is_empty() {
                    body.push_str("<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Machines starved or blocked</h2><span class=\"right\"><a href=\"/entities?view=machines\">See all →</a></span></div><table class=\"table--compact\"><thead><tr><th>Id</th><th>Proto</th><th>State</th></tr></thead><tbody>");
                    for r in flagged.iter().take(10) {
                        let stt = r["state"].as_str().unwrap_or("");
                        body.push_str(&format!(
                            "<tr><td>#{}</td><td>{}</td><td>{}</td></tr>",
                            r["id"].as_i64().unwrap_or(0),
                            esc(&sv(&r["proto"])),
                            if stt == "NotEnoughInput" {
                                badge("starved", "Starved")
                            } else {
                                badge("blocked", "Blocked")
                            },
                        ));
                    }
                    body.push_str("</tbody></table></div>");
                }
            }
            // Set a storage's product — one form for any storage id.
            let mut ids: Vec<&String> = live.names.keys().collect();
            ids.sort_by_key(|id| live.names.get(*id).cloned().unwrap_or_default());
            let mut opts = String::new();
            for id in ids {
                let k = live.kinds.get(id).cloned().unwrap_or_default();
                if k == "virtual" {
                    continue;
                }
                opts.push_str(&format!(
                    "<option value=\"{}\">{}</option>",
                    esc(id),
                    esc(live.names.get(id).map(|s| s.as_str()).unwrap_or(id))
                ));
            }
            body.push_str(&format!(
                "<div class=\"card\"><div class=\"cardhead\"><h2>Set a storage's product</h2></div>\
                 <form method=\"post\" action=\"/actions/prepare-storage-product\">\
                 <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
                 <div class=\"form-grid\">\
                 <div class=\"field\"><label for=\"ssp-id\">Storage id</label><input id=\"ssp-id\" type=\"text\" name=\"storageId\" required></div>\
                 <div class=\"field\"><label for=\"ssp-prod\">Product</label><select id=\"ssp-prod\" name=\"productId\">{}</select></div>\
                 <div class=\"field\"><label>&nbsp;</label><button type=\"submit\">Prepare…</button></div></div></form>\
                 <p class=\"sub\">Goes through the same confirmation page, which warns you if the storage currently holds something else.</p></div>",
                esc(&session.csrf),
                opts,
            ));
        }
    }
    body.push_str(TABLE_JS);
    Response::html(
        200,
        layout_ctx(state, session, "Entities", "Entities", &ctx, &body),
    )
}

const TABLE_JS: &str = "<script>
function filterTable(){var q=document.getElementById('tfilter').value.toLowerCase();document.querySelectorAll('#ptable tbody tr').forEach(function(r){r.style.display=r.textContent.toLowerCase().indexOf(q)>=0?'':'none'})}
document.querySelectorAll('#ptable thead th').forEach(function(th,i){th.style.cursor='pointer';th.title='Click to sort';th.addEventListener('click',function(){var tb=document.querySelector('#ptable tbody');var rows=Array.prototype.slice.call(tb.rows);var dir=th.getAttribute('data-dir')==='asc'?'desc':'asc';th.setAttribute('data-dir',dir);rows.sort(function(a,b){var x=a.cells[i].textContent.trim(),y=b.cells[i].textContent.trim();var sx=x.trim().charAt(0)==='-'?-1:1,sy=y.trim().charAt(0)==='-'?-1:1;var nx=sx*parseFloat(x.replace(/[^0-9.]/g,'')),ny=sy*parseFloat(y.replace(/[^0-9.]/g,''));var c=(!isNaN(nx)&&!isNaN(ny)&&/[0-9]/.test(x)&&/[0-9]/.test(y))?nx-ny:x.localeCompare(y);return dir==='asc'?c:-c});rows.forEach(function(r){tb.appendChild(r)})})});
</script>";

// ---------------------------------------------------------------------------
// Snapshots
// ---------------------------------------------------------------------------

fn page_snapshots(state: &State, session: &Session, req: &Request) -> Response {
    let take_form = format!(
        "<form method=\"post\" action=\"/snapshots/take\" id=\"take\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><button type=\"submit\">Take a snapshot now</button></form>",
        esc(&session.csrf)
    );
    let mut body = page_head(
        "Records",
        "Snapshots",
        "Point-in-time records of your run — what makes trends, evidence and history possible.",
        &take_form,
    );
    body.push_str(&contract_strip(
        "Stored snapshots and the trends computed from them",
        "Mostly informational — taking a snapshot is the one action",
        "The local store on this machine (coi.db, next to the tool). Snapshots are the input to productivity verdicts and pinned-product trends",
    ));
    if let Some(msg) = req.query.get("msg") {
        body.push_str(&format!(
            "<div class=\"callout callout--ok\">{}</div>",
            esc(msg)
        ));
    }
    match store(state) {
        Ok(st) => {
            match st.counts() {
                Ok((snaps, rows, issues)) => body.push_str(&format!(
                    "<div class=\"stats2\" id=\"store\">{}{}{}</div>",
                    stat2_tile("layers", "Snapshots", &format!("{snaps}"), "", false),
                    stat2_tile("box", "Product rows", &format!("{rows}"), "", false),
                    stat2_tile("alert", "Stored issues", &format!("{issues}"), "", false),
                )),
                Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
            }
            if let Ok(list) = st.recent_snapshots(20) {
                if !list.is_empty() {
                    body.push_str("<div class=\"card tablecard\" id=\"list\"><table class=\"table--compact\"><thead><tr><th>#</th><th>Game date</th><th class=\"num\">Tick</th><th>Taken</th><th class=\"num\">Products</th><th class=\"num\">Issues</th></tr></thead><tbody>");
                    for (id, date, tick, taken, prods, issues) in list {
                        body.push_str(&format!(
                            "<tr><td>{id}</td><td>{}</td><td class=\"num mono\">{tick}</td><td>{}</td><td class=\"num\">{prods}</td><td class=\"num\">{issues}</td></tr>",
                            esc(&date),
                            fmt_clock(taken as u64)
                        ));
                    }
                    body.push_str("</tbody></table></div>");
                }
            }
            // Trends for pinned products (names from a live fetch if possible)
            if let Ok(live) = fetch_live(&client(state)) {
                let mut names: Vec<String> = live
                    .pinned_ids
                    .iter()
                    .filter_map(|id| live.names.get(id).cloned())
                    .collect();
                names.sort();
                names.dedup();
                let mut any = false;
                let mut tbl = String::from("<div class=\"card tablecard\" id=\"trends\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Trends across snapshots</h2></div><table class=\"table--compact\"><thead><tr><th>Product</th><th>Trend</th><th>First → last</th></tr></thead><tbody>");
                for name in names {
                    if let Ok(rows) = st.trend(&name) {
                        let mine: Vec<_> = rows.iter().filter(|r| r.7 == name).collect();
                        if !mine.is_empty() {
                            any = true;
                            let vals: Vec<f64> = mine.iter().map(|r| r.2 as f64).collect();
                            tbl.push_str(&format!(
                                "<tr><td>{}</td><td>{}</td><td class=\"num\">{} → {}</td></tr>",
                                esc(&name),
                                svg_sparkline(&vals, "var(--accent)"),
                                mine.first().map(|r| r.2).unwrap_or(0),
                                mine.last().map(|r| r.2).unwrap_or(0),
                            ));
                        }
                    }
                }
                tbl.push_str("</tbody></table></div>");
                if any {
                    body.push_str(&tbl);
                }
            }
        }
        Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
    }
    Response::html(200, layout(state, session, "Snapshots", &body))
}

fn do_take_snapshot(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => return game_unreachable(state, session, &e),
    };
    let snap = snapshot_json(&live);
    let mut issues = compute_issues(&live);
    match store(state) {
        Ok(mut st) => {
            // §5 productivity evaluation, same as the CLI snapshot
            // path: extends the board + records hysteresis state
            // before the insert persists everything together.
            let th = crate::productivity::Thresholds::load(&state.db_path);
            let _ = crate::productivity::evaluate_snapshot(&mut st, &snap, &live, &mut issues, &th);
            let result = st.insert_snapshot(&snap, &issues);
            // Addendum XVI: snapshot creation is a system event in
            // the unified log — stored and duplicate outcomes both
            // replay (an insert error stays a bare 502, as before).
            if !result.is_err() {
                let (outcome, summary, snap_id) = match &result {
                    Ok(Some(id)) => ("stored", format!("Snapshot #{id} stored"), Some(*id)),
                    _ => (
                        "duplicate",
                        "Snapshot skipped — that game tick is already stored".to_string(),
                        None,
                    ),
                };
                let _ = st.activity_add(&ActivityEvent {
                    event_key: format!("snapshot-{}-{}", now_unix(), rand_hex(8)),
                    recorded_ts_unix: now_unix() as i64,
                    source: "system".to_string(),
                    actor: Some(session.user.clone()),
                    category: Some("snapshot".to_string()),
                    action: "TakeSnapshot".to_string(),
                    summary: Some(summary),
                    game_id: snap["game"]["gameId"].as_str().map(|s| s.to_string()),
                    game_date: snap["game"]["date"].as_str().map(|s| s.to_string()),
                    game_tick: snap["game"]["tick"].as_i64(),
                    outcome: Some(outcome.to_string()),
                    payload_json: Some(
                        json!({
                            "snapshotId": snap_id,
                            "products": snap["products"].as_array().map(|a| a.len()).unwrap_or(0),
                        })
                        .to_string(),
                    ),
                });
            }
            match result {
                Ok(Some(id)) => Response::redirect(&format!(
                    "/snapshots?msg={}",
                    pct_encode(&format!(
                        "Snapshot #{id} stored ({} products).",
                        snap["products"].as_array().map(|a| a.len()).unwrap_or(0)
                    ))
                )),
                Ok(None) => Response::redirect(
                    "/snapshots?msg=That game tick is already stored — nothing new to save.",
                ),
                Err(e) => Response::text(502, &e),
            }
        }
        Err(e) => Response::text(502, &e),
    }
}

// ---------------------------------------------------------------------------
// Log page — the unified activity record (SPEC Addendum XVI)
// ---------------------------------------------------------------------------

/// Where the QoS mods' activity feed lives: the mods append it next
/// to the game's own data, at %APPDATA%\Captain of Industry\QoS\
/// events-v1.jsonl. COI_QOS_EVENTS_PATH overrides the location
/// (tests, unusual installs). None when no path can be derived.
fn qos_feed_path() -> Option<String> {
    if let Ok(p) = std::env::var("COI_QOS_EVENTS_PATH") {
        if !p.trim().is_empty() {
            return Some(p);
        }
    }
    let appdata = std::env::var("APPDATA")
        .ok()
        .filter(|s| !s.trim().is_empty())?;
    Some(format!(
        "{appdata}\\Captain of Industry\\QoS\\events-v1.jsonl"
    ))
}

struct QosImportReport {
    path: String,
    found: bool,
    stats: Option<crate::store::QosImportStats>,
    error: Option<String>,
}

/// Import the QoS feed into the store (idempotent — already-stored
/// eventIds are skipped). A missing feed is a normal state (mods
/// not installed yet, or never ran), never an error for the page.
fn import_qos_feed_now(state: &State, st: &Store) -> QosImportReport {
    let Some(path) = qos_feed_path() else {
        return QosImportReport {
            path: "(no APPDATA and no COI_QOS_EVENTS_PATH — feed location unknown)".to_string(),
            found: false,
            stats: None,
            error: None,
        };
    };
    let _ = state;
    match std::fs::read(&path) {
        Ok(bytes) => {
            // The feed is append-only small JSON lines; cap the
            // read so a pathological file cannot stall the page.
            let cap = bytes.len().min(64 * 1024 * 1024);
            let text = String::from_utf8_lossy(&bytes[..cap]).into_owned();
            match st.import_qos_feed(&text) {
                Ok(stats) => QosImportReport {
                    path,
                    found: true,
                    stats: Some(stats),
                    error: None,
                },
                Err(e) => QosImportReport {
                    path,
                    found: true,
                    stats: None,
                    error: Some(e),
                },
            }
        }
        Err(_) => QosImportReport {
            path,
            found: false,
            stats: None,
            error: None,
        },
    }
}

fn page_log(state: &State, session: &Session, req: &Request) -> Response {
    const PAGE_SIZE: i64 = 50;
    let src_filter = req.query.get("source").map(|s| s.as_str()).unwrap_or("all");
    let source = match src_filter {
        "companion" | "qos" | "system" => Some(src_filter),
        _ => None,
    };
    let search = req.query.get("q").map(|s| s.as_str()).unwrap_or("");
    let page: i64 = req
        .query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0)
        .max(0);

    let mut body = page_head(
        "Records",
        "Log",
        "Everything that was done, in order — applies, snapshot takes and the QoS mods' own feed.",
        "",
    );
    body.push_str(&contract_strip(
        "The audit trail: every applied command, snapshot and mod action",
        "Informational — the record itself; nothing here can be edited or re-applied",
        "The local store on this machine. QoS rows are imported from the mods' own event feed each time this page loads",
    ));
    body.push_str(
        "<div class=\"callout callout--info\">The unified activity record, stored in this tool's local \
         database (coi.db). <b>Companion</b> rows are written when a command is applied \
         through the Actions flow; <b>system</b> rows when a snapshot is taken here; \
         <b>QoS</b> rows are the mods' own record of what they did inside the game, \
         imported from their feed file each time this page loads. QoS events exist only \
         from the updated mods onward — mod actions from before the update lived only \
         in the game log and are <b>not</b> backfilled. Times are UTC, as everywhere \
         on this site.</div>",
    );

    match store(state) {
        Ok(st) => {
            let report = import_qos_feed_now(state, &st);
            if report.found {
                match (&report.stats, &report.error) {
                    (Some(s), _) => body.push_str(&format!(
                        "<div style=\"display:flex;align-items:center;gap:12px;flex-wrap:wrap;margin-bottom:12px\"><p class=\"caption\" style=\"margin:0\">QoS feed: <span class=\"mono\">{}</span> — this load:</p>{}</div>",
                        esc(&report.path),
                        figstrip(&[
                            fig("New", &s.imported.to_string(), false),
                            fig("Skipped", &s.skipped.to_string(), false),
                            if s.malformed > 0 {
                                format!("<div class=\"fig\"><div class=\"figl\">Malformed</div><div class=\"figv\" style=\"color:var(--critical)\">{}</div></div>", s.malformed)
                            } else {
                                fig("Malformed", "0", false)
                            },
                        ])
                    )),
                    (None, Some(e)) => body.push_str(&format!(
                        "<div class=\"err\">QoS feed import failed ({}): {}</div>",
                        esc(&report.path),
                        esc(e)
                    )),
                    _ => {}
                }
            } else {
                body.push_str(&format!(
                    "<p class=\"caption\">QoS feed: no file at <span class=\"mono\">{}</span> yet — nothing to import.</p>",
                    esc(&report.path)
                ));
            }

            // Filter form (plain GET, so filters live in the URL).
            let sel = |v: &str| if src_filter == v { " selected" } else { "" };
            body.push_str(&format!(
                "<div class=\"card\"><form method=\"get\" action=\"/log\" class=\"filterbar\" style=\"margin:0\">\
                 <label>Source <select name=\"source\">\
                 <option value=\"all\"{}>All sources</option>\
                 <option value=\"companion\"{}>Companion</option>\
                 <option value=\"qos\"{}>QoS mods</option>\
                 <option value=\"system\"{}>System</option></select></label>\
                 <input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"🔍 action or summary text\">\
                 <button type=\"submit\">Filter</button></form></div>",
                sel("all"),
                sel("companion"),
                sel("qos"),
                sel("system"),
                esc(search)
            ));

            let total = st.activity_count(source, Some(search)).unwrap_or(0);
            let rows = st
                .activity_recent(source, Some(search), PAGE_SIZE, page * PAGE_SIZE)
                .unwrap_or_default();
            body.push_str(&format!(
                "<p class=\"caption\">{total} event(s) match. Showing {} (newest first).</p>",
                rows.len()
            ));
            if rows.is_empty() {
                let filtered = source.is_some() || !search.is_empty();
                body.push_str(&format!(
                    "<div class=\"empty\">{}</div>",
                    if filtered {
                        "No events match this filter."
                    } else {
                        "No events recorded yet — actions you confirm and QoS mod activity land here."
                    }
                ));
            } else {
                body.push_str("<div class=\"card\">");
                let mut cur_day = String::new();
                for r in &rows {
                    let clock = fmt_clock(r.recorded_ts_unix as u64);
                    let (day, time) = clock
                        .split_once(' ')
                        .map(|(d, t)| (d.to_string(), t.to_string()))
                        .unwrap_or((clock.clone(), String::new()));
                    if day != cur_day {
                        body.push_str(&format!("<div class=\"feed-day\">{}</div>", esc(&day)));
                        cur_day = day;
                    }
                    let src_badge = match r.source.as_str() {
                        "qos" => "<span class=\"badge badge--src-qos\">QoS</span>".to_string(),
                        "system" => {
                            "<span class=\"badge badge--src-system\">system</span>".to_string()
                        }
                        _ => "<span class=\"badge badge--src-companion\">companion</span>"
                            .to_string(),
                    };
                    let outcome_badge = match r.outcome.as_deref() {
                        Some("success") => badge("ok", "success"),
                        Some("failed") => badge("critical", "failed"),
                        Some(o) if !o.is_empty() => badge("paused", o),
                        _ => String::new(),
                    };
                    let game = match (&r.game_date, r.game_tick) {
                        (Some(d), Some(t)) => format!("{} · tick {}", esc(d), t),
                        (Some(d), None) => esc(d),
                        (None, Some(t)) => format!("tick {t}"),
                        (None, None) => String::new(),
                    };
                    let payload = match &r.payload_json {
                        Some(p) => {
                            let pretty = serde_json::from_str::<Value>(p)
                                .map(|v| {
                                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| p.clone())
                                })
                                .unwrap_or_else(|_| p.clone());
                            format!("<details class=\"payload\"><summary>payload</summary><pre>{}</pre></details>", esc(&pretty))
                        }
                        None => String::new(),
                    };
                    // CTA row 14 (V2.7): a refused QoS job gets a
                    // re-prepare link back to its family's page.
                    let reprepare = if r.source == "qos" && r.action.contains("Refused") {
                        let hay = format!(
                            "{} {}",
                            r.summary.as_deref().unwrap_or(""),
                            r.payload_json.as_deref().unwrap_or("")
                        )
                        .to_lowercase();
                        let href = if hay.contains("designat") {
                            "/designate"
                        } else {
                            "/place"
                        };
                        format!(
                            " <a class=\"btn btn--sm btn--ghost\" href=\"{href}\">Re-prepare</a>"
                        )
                    } else {
                        String::new()
                    };
                    body.push_str(&format!(
                        "<div class=\"feedrow\"><span class=\"time mono\">{}</span>{}<div style=\"flex:1;min-width:0\">\
                         <div><b>{}</b> <span class=\"sub\">{}</span> {}{}</div>\
                         <div>{}</div>\
                         <div class=\"caption\">{} · {}</div>{}</div></div>",
                        esc(&time),
                        src_badge,
                        esc(&r.action),
                        esc(r.category.as_deref().unwrap_or("")),
                        outcome_badge,
                        reprepare,
                        esc(r.summary.as_deref().unwrap_or("")),
                        esc(r.actor.as_deref().unwrap_or("—")),
                        game,
                        payload,
                    ));
                }
                body.push_str("</div>");
            }

            // Pagination: simple older/newer links preserving filters.
            let qs = format!("source={}&q={}", pct_encode(src_filter), pct_encode(search));
            let mut pager = String::new();
            if page > 0 {
                pager.push_str(&format!(
                    "<a class=\"btn btn--secondary\" href=\"/log?{}&page={}\">← Newer</a> ",
                    qs,
                    page - 1
                ));
            }
            if total > (page + 1) * PAGE_SIZE {
                pager.push_str(&format!(
                    "<a class=\"btn btn--secondary\" href=\"/log?{}&page={}\">Older →</a>",
                    qs,
                    page + 1
                ));
            }
            if !pager.is_empty() {
                body.push_str(&format!(
                    "<div class=\"pager\">{pager}<span class=\"caption\">page {}</span></div>",
                    page + 1
                ));
            }
        }
        Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
    }
    let mut ctx = String::from("<div class=\"ctx\">SOURCES</div>");
    for (v, label) in [
        ("all", "All sources"),
        ("companion", "Companion"),
        ("qos", "QoS mods"),
        ("system", "System"),
    ] {
        ctx.push_str(&format!(
            "<a class=\"navitem anchor{}\" href=\"/log?source={}&q={}\">{}</a>",
            if src_filter == v { " active" } else { "" },
            v,
            url_enc(search),
            label
        ));
    }
    Response::html(200, layout_ctx(state, session, "Log", "Log", &ctx, &body))
}

// ---------------------------------------------------------------------------
// Research page + global-priority prepare
// ---------------------------------------------------------------------------

/// The game's research catalogue (250 node ids), extracted from the
/// game's own data tables as serialized in saves (same extraction as
/// the verified save reader). The API itself publishes no research
/// data; the game validates every start/queue command.
fn research_ids() -> &'static Vec<String> {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        include_str!("../data/research_ids.txt")
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

/// "ResearchAdvancedDiesel" -> "Advanced Diesel"
fn pretty_research(id: &str) -> String {
    let base = id.strip_prefix("Research").unwrap_or(id);
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in base.chars() {
        if ch.is_ascii_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = ch.is_ascii_lowercase();
        out.push(ch);
    }
    out
}

fn page_research(state: &State, session: &Session, req: &Request) -> Response {
    // Tabs (V2.1): Current & queue · Catalogue · Completed, via
    // ?tab=. The catalogue is letter-grouped with a rail A–Z
    // index; per-node Start/Unqueue moved to the node detail page
    // (/research/node) — rows keep Queue only.
    let tab = req
        .query
        .get("tab")
        .map(|s| s.as_str())
        .unwrap_or("current");
    let rs = bridge_json(state, "/coi/v1/research");
    let mut body = page_head(
        "Workshop",
        "Research",
        "What your labs are doing, what's queued, and what you could start.",
        "",
    );
    body.push_str(&contract_strip(
        "The research tree: current node, queue, catalogue, completed",
        "Actionable — Start / Queue / Unqueue go prepare → confirm → audit",
        "Research is bridge-only data: the game's own API doesn't report it, and there is no snapshot fallback — with the bridge silent, this page shows the remedy, not stale state",
    ));
    body.push_str(
        "<details class=\"howit\"><summary>How research commands work</summary><p>Commands are validated against the game's own catalogue (250 nodes), and the game accepts \
         or rejects each one. Starting a node that isn't available yet returns failure and changes nothing. \
         Queueing several nodes and letting the game work through them works the same way. Every attempt is audit-logged. \
         Queue discipline — what to run first and why — lives in <a href=\"/guide?id=research\">Learn the game → Research</a>.</p></details>",
    );
    let mut ctx = String::from("<div class=\"ctx\">VIEWS</div>");
    for (t, label) in [
        ("current", "Current & queue"),
        ("catalogue", "Catalogue"),
        ("completed", "Completed"),
    ] {
        ctx.push_str(&format!(
            "<a class=\"navitem anchor{}\" href=\"/research?tab={}\">{}</a>",
            if tab == t { " active" } else { "" },
            t,
            label
        ));
    }
    match tab {
        "catalogue" => {
            // Letter index of the pretty names actually present.
            let mut letters: Vec<char> = research_ids()
                .iter()
                .filter_map(|id| pretty_research(id).chars().next())
                .collect();
            letters.sort_unstable();
            letters.dedup();
            ctx.push_str("<div class=\"ctx\">INDEX</div>");
            for l in &letters {
                ctx.push_str(&rail_link(&format!("#L-{l}"), &l.to_string()));
            }
            body.push_str("<div class=\"card tablecard\"><div class=\"cardhead\" style=\"padding:12px 12px 0\"><h2>Catalogue</h2></div>");
            body.push_str(
                "<div class=\"filterbar\"><input type=\"text\" id=\"rq\" placeholder=\"🔍 Filter research…\" oninput=\"filterResearch()\"></div>\
                 <script>function filterResearch(){var q=document.getElementById('rq').value.toLowerCase();\
                 document.querySelectorAll('tr[data-r]').forEach(function(r){r.style.display=r.getAttribute('data-r').toLowerCase().includes(q)?'':'none';});}</script>",
            );
            let mut sorted: Vec<&String> = research_ids().iter().collect();
            sorted.sort_by_key(|id| pretty_research(id));
            // Directory discipline (DESIGN V3): letter groups collapse;
            // only the group containing the current node's letter opens
            // by default. The A–Z rail opens its group via the hash.
            let open_letter = rs
                .as_ref()
                .and_then(|r| r["current"]["id"].as_str().map(|s| s.to_string()))
                .and_then(|cid| pretty_research(&cid).chars().next());
            let mut groups: Vec<(char, Vec<&String>)> = Vec::new();
            for id in sorted {
                let letter = pretty_research(id).chars().next().unwrap_or('?');
                match groups.last_mut() {
                    Some((l, v)) if *l == letter => v.push(id),
                    _ => groups.push((letter, vec![id])),
                }
            }
            for (letter, ids) in &groups {
                body.push_str(&format!(
                    "<details class=\"indgroup\" id=\"L-{letter}\"{}><summary>{letter} <span class=\"sub\">· {} nodes</span></summary><table class=\"table--compact\"><tbody>",
                    if Some(*letter) == open_letter { " open" } else { "" },
                    ids.len()
                ));
                for id in ids {
                    let pretty = pretty_research(id);
                    body.push_str(&format!(
                        "<tr data-r=\"{} {}\"><td><a href=\"/research/node?id={}\">{}</a></td><td class=\"mono\">{}</td><td>{}</td></tr>",
                        esc(&pretty),
                        esc(id),
                        url_enc(id),
                        esc(&pretty),
                        esc(id),
                        prepare_form(
                            session,
                            "ResearchQueueDequeueCmd",
                            json!({"nodeId": id, "isEnqueue": true}),
                            "Queue",
                            false
                        ),
                    ));
                }
                body.push_str("</tbody></table></details>");
            }
            body.push_str(
                "<script>function openLetter(){var h=location.hash;if(h.indexOf('#L-')===0){var d=document.getElementById(h.slice(1));if(d){d.open=true;}}}window.addEventListener('hashchange',openLetter);openLetter();</script>",
            );
            body.push_str("</div>");
        }
        "completed" => match &rs {
            Some(rsv) => match rsv["researched"].as_array() {
                Some(done) => {
                    let count = if rsv["researchedCount"].is_null() {
                        done.len() as i64
                    } else {
                        vi(&rsv["researchedCount"])
                    };
                    let mut chips = String::new();
                    for v in done {
                        let id = sv(v);
                        chips.push_str(&format!(
                            "<a class=\"chip\" href=\"/research/node?id={}\">{}</a>",
                            url_enc(&id),
                            esc(&pretty_research(&id))
                        ));
                    }
                    body.push_str(&format!(
                            "<div class=\"card\"><div class=\"cardhead\"><h2>Completed research</h2><span class=\"right\">{} nodes</span></div><div class=\"chips\">{}</div></div>",
                            count,
                            if chips.is_empty() { "— none yet —".to_string() } else { chips },
                        ));
                }
                None => body.push_str(&degraded_bridge("Completed research", "0.6.2")),
            },
            None => body.push_str(&degraded_bridge("Completed research", "0.6.2")),
        },
        _ => {
            body.push_str(
                "<div class=\"card\"><div class=\"cardhead\"><h2>Current research</h2></div>",
            );
            if let Some(rsv) = &rs {
                let cur = &rsv["current"];
                let cur_id = cur["id"].as_str().unwrap_or("");
                if !cur_id.is_empty() {
                    let pct = pct_raw(&cur["progressPerc"]);
                    let queue_html: String = rsv["queue"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|q| {
                                    format!(
                                        "<a class=\"chip\" href=\"/research/node?id={}\">{}</a>",
                                        url_enc(&sv(q)),
                                        esc(&pretty_research(&sv(q)))
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(" → ")
                        })
                        .unwrap_or_default();
                    body.push_str(&format!(
                        "<p><b><a href=\"/research/node?id={}\">{}</a></b> — {:.1}%</p>{}<p class=\"sub\">Queue: {}</p>",
                        url_enc(cur_id),
                        esc(&pretty_research(cur_id)),
                        pct,
                        fillbar(pct, "var(--accent)", 420),
                        if queue_html.is_empty() { "empty".to_string() } else { queue_html },
                    ));
                } else {
                    body.push_str("<p class=\"sub\">No research is running right now.</p>");
                }
            } else {
                body.push_str(&degraded_bridge("Current research", "0.6.2"));
            }
            body.push_str("<p>");
            body.push_str(&prepare_form(
                session,
                "ResearchStopCmd",
                json!({}),
                "Stop current research",
                false,
            ));
            body.push_str("</p><p class=\"sub\">Stops whatever is currently being researched (the game keeps the spent progress rules as usual).</p></div>");
        }
    }
    Response::html(
        200,
        layout_ctx(state, session, "Research", "Research", &ctx, &body),
    )
}

/// One research node (V2.1): status from the bridge research
/// route, and the Start / Queue / Unqueue prepares that used to
/// crowd every catalogue row.
fn page_research_node(state: &State, session: &Session, req: &Request) -> Response {
    let id = match req.query.get("id") {
        Some(id) if research_ids().iter().any(|r| r == id) => id.clone(),
        _ => return Response::redirect("/research?tab=catalogue"),
    };
    let pretty = pretty_research(&id);
    let rs = bridge_json(state, "/coi/v1/research");
    let (status, status_ctx) = match &rs {
        Some(rsv) => {
            let cur_id = rsv["current"]["id"].as_str().unwrap_or("");
            if cur_id == id {
                (
                    "Researching now".to_string(),
                    format!("{:.1}% complete", pct_raw(&rsv["current"]["progressPerc"])),
                )
            } else if rsv["researched"]
                .as_array()
                .map(|a| a.iter().any(|v| sv(v) == id))
                .unwrap_or(false)
            {
                ("Researched".to_string(), "completed".to_string())
            } else if let Some(pos) = rsv["queue"]
                .as_array()
                .and_then(|a| a.iter().position(|v| sv(v) == id))
            {
                (
                    format!("Queued · position {}", pos + 1),
                    "in the queue".to_string(),
                )
            } else {
                (
                    "Not started".to_string(),
                    "available to start or queue".to_string(),
                )
            }
        }
        None => (
            "Unknown".to_string(),
            "CoiBridge is not answering".to_string(),
        ),
    };
    let mut body = page_head(
        "Workshop",
        &pretty,
        &format!("One research node in depth · {} — what it is, where it stands, and the queue discipline that decides when to run it (canon: philosophy.research-queue).", id),
        "<a class=\"btn btn--secondary btn--sm\" href=\"/research?tab=catalogue\">← Catalogue</a>",
    );
    body.push_str("<div class=\"stats2\">");
    body.push_str(&stat2_tile(
        "flask",
        "Status",
        &esc(&status),
        &esc(&status_ctx),
        false,
    ));
    body.push_str(&stat2_tile(
        "layers",
        "Node id",
        &format!(
            "<span class=\"mono\" style=\"font-size:15px\">{}</span>",
            esc(&id)
        ),
        "",
        false,
    ));
    body.push_str("</div>");
    // Start is this page's one primary; Queue/Unqueue are ghosts.
    body.push_str(&format!(
        "<div class=\"card\"><div class=\"cardhead\"><h2>Actions</h2></div>\
         <form method=\"post\" action=\"/actions/prepare\" style=\"display:inline\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"ResearchStartCmd\">\
         <input type=\"hidden\" name=\"args\" value=\"{}\">\
         <button class=\"btn btn--sm\" type=\"submit\">Start research</button></form> {} {}\
         <p class=\"sub\">Every action goes through the §12 prepare → confirm → audit flow; the game accepts or rejects each one.</p></div>",
        esc(&session.csrf),
        esc(&json!({"nodeId": id}).to_string()),
        prepare_form(
            session,
            "ResearchQueueDequeueCmd",
            json!({"nodeId": id, "isEnqueue": true}),
            "Queue",
            false
        ),
        prepare_form(
            session,
            "ResearchQueueDequeueCmd",
            json!({"nodeId": id, "isEnqueue": false}),
            "Unqueue",
            false
        ),
    ));
    let ctx = "<div class=\"ctx\">VIEWS</div>".to_string()
        + &rail_link("/research?tab=current", "Current & queue")
        + &rail_link("/research?tab=catalogue", "Catalogue")
        + &rail_link("/research?tab=completed", "Completed");
    Response::html(
        200,
        layout_ctx(state, session, &pretty, "Research", &ctx, &body),
    )
}

fn do_prepare_priority(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let pid = req.form.get("priorityId").cloned().unwrap_or_default();
    let pval: i64 = req
        .form
        .get("priority")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let args = json!({"priorityId": pid, "priority": pval});
    let fake = Request {
        method: "POST".into(),
        path: "/actions/prepare".into(),
        query: HashMap::new(),
        form: HashMap::from([
            ("csrf".into(), session.csrf.clone()),
            ("type".into(), "SetGlobalPriorityCmd".into()),
            ("args".into(), args.to_string()),
        ]),
        host: String::new(),
    };
    do_prepare(state, session, &fake)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical job payload is a cross-language contract with
    /// QoSBuilder (SPEC Addendum XVII): this vector was produced by
    /// an independent implementation of the same spec. If either
    /// side's escaping or field order drifts, the mod refuses every
    /// job as tampered — this test must fail first.
    #[test]
    fn job_canonical_vector() {
        let canonical = job_canonical(
            "11111111-2222-3333-4444-555555555555",
            "2026-11-05T18:00:00Z",
            "2026-11-05T18:30:00Z",
            "1234567890123456789",
            "captain",
            "Test \"A\" \\ Béton",
            "B1504:H4sIAAAAAAAC/test+string==",
            (3167, 762, 4),
            90,
            true,
            500,
        );
        assert_eq!(
            canonical,
            "{\"jobId\":\"11111111-2222-3333-4444-555555555555\",\"schema\":1,\"createdUtc\":\"2026-11-05T18:00:00Z\",\"expiresUtc\":\"2026-11-05T18:30:00Z\",\"gameId\":\"1234567890123456789\",\"companionUser\":\"captain\",\"blueprintName\":\"Test \\\"A\\\" \\\\ Béton\",\"blueprintString\":\"B1504:H4sIAAAAAAAC/test+string==\",\"anchor\":{\"x\":3167,\"y\":762,\"z\":4},\"rotationDeg\":90,\"applyConfiguration\":true,\"limits\":{\"maxItems\":500}}"
        );
        assert_eq!(
            job_sha256_hex(&canonical),
            "c84de603240955fd454b24a87a5bb722a586d5de1ec3220320b0e07dc9bfb465"
        );
    }

    #[test]
    fn iso_utc_format() {
        assert_eq!(iso_utc(1793883600), "2026-11-05T13:00:00Z");
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    }

    /// DESIGN V2.5 acceptance semantics: a difference renders
    /// signed with an arrow; a measured zero is the muted flat
    /// chip; absolutes never pass through here at all.
    #[test]
    fn delta_chip_semantics() {
        let pos = delta_chip(295, "LM");
        assert!(
            pos.contains("delta--pos") && pos.contains("#i-up") && pos.contains("+295"),
            "{pos}"
        );
        let neg = delta_chip(-77, "TM");
        assert!(
            neg.contains("delta--neg") && neg.contains("#i-down") && neg.contains("\u{2212}77"),
            "{neg}"
        );
        let flat = delta_chip(0, "LM");
        assert!(
            flat.contains("delta--flat") && flat.contains("#i-flat"),
            "{flat}"
        );
        assert!(flat.contains(">0 <") || flat.contains(" 0 "), "{flat}");
    }

    #[test]
    fn signed_formatting() {
        assert_eq!(signed(1234), "+1,234");
        assert_eq!(signed(-77), "\u{2212}77");
        assert_eq!(signed(0), "0");
    }

    fn test_state() -> State {
        State {
            cfg: Config {
                authority: "127.0.0.1:3073".into(),
                host_header: "127.0.0.1:3073".into(),
                proxy: None,
            },
            db_path: ":memory:".into(),
            csrf: "test-csrf".into(),
            port: 8484,
            badge: Mutex::new(BadgeCache {
                count: None,
                reachable: false,
                updated: 0,
            }),
            prepared: Mutex::new(HashMap::new()),
            prepared_jobs: Mutex::new(HashMap::new()),
            shutdown: AtomicBool::new(false),
            inflight: Inflight::new(),
        }
    }

    fn test_req(host: &str) -> Request {
        Request {
            method: "GET".into(),
            path: "/".into(),
            query: HashMap::new(),
            form: HashMap::new(),
            host: host.into(),
        }
    }

    /// The login-removal hardening: only the loopback names on
    /// the configured port are answered.
    #[test]
    fn host_check_accepts_only_loopback_on_port() {
        let st = test_state();
        assert!(host_ok(&st, &test_req("127.0.0.1:8484")));
        assert!(host_ok(&st, &test_req("localhost:8484")));
        assert!(host_ok(&st, &test_req("LOCALHOST:8484")));
        assert!(!host_ok(&st, &test_req("evil.example:8484")));
        assert!(!host_ok(&st, &test_req("127.0.0.1:9999")));
        assert!(!host_ok(&st, &test_req("")));
    }

    /// The badge endpoint answers from cache only: before any
    /// page fetch the count is null (never a dressed-up stale
    /// number), and the badge markup carries the words "must fix"
    /// in every state.
    #[test]
    fn must_fix_endpoint_starts_null() {
        let st = test_state();
        let r = api_must_fix_count(&st);
        let body = String::from_utf8(r.body).unwrap();
        assert!(body.contains("\"count\":null"), "{body}");
        assert!(body.contains("\"reachable\":false"), "{body}");
        let badge_html = mustfix_badge(&st);
        assert!(badge_html.contains("must fix"), "{badge_html}");
        assert!(badge_html.contains("/issues?only=fix-now"), "{badge_html}");
    }

    /// §V2.5 power examples: a +1 kW balance is a green delta, a
    /// −900 kW balance a red one, and the 38,000 kW capacity is a
    /// neutral absolute (no delta chip on its figure).
    #[test]
    fn power_panel_number_semantics() {
        let pos = power_panel_html(&serde_json::json!({
            "generatedKw": 38001, "consumedKw": 38000, "balanceKw": 1,
            "generationCapacityKw": 38000
        }));
        assert!(pos.contains("delta--pos") && pos.contains("+1"), "{pos}");
        assert!(pos.contains("38,000"), "{pos}");
        let neg = power_panel_html(&serde_json::json!({
            "generatedKw": 37100, "consumedKw": 38000, "balanceKw": -900,
            "generationCapacityKw": 38000
        }));
        assert!(
            neg.contains("delta--neg") && neg.contains("\u{2212}900"),
            "{neg}"
        );
        // The deficit panel carries the spec'd CTA row (V2.7 row 12).
        assert!(neg.contains("Open Actions"), "{neg}");
        assert!(!pos.contains("Open Actions"), "{pos}");
    }

    /// Shutdown (Addendum XXIV): the confirm page names the
    /// consequences and carries the form; a tokenless POST is
    /// refused and does not set the flag.
    #[test]
    fn shutdown_confirm_page_and_token_gate() {
        let st = test_state();
        let s = local_session(&st);
        let r = page_shutdown(&st, &s);
        let body = String::from_utf8(r.body).unwrap();
        assert!(body.contains("What happens when you shut down"), "{body}");
        assert!(body.contains("action=\"/shutdown\""), "{body}");
        assert!(body.contains("name=\"csrf\""), "{body}");
        let r2 = do_shutdown(&st, &s, &test_req("127.0.0.1:8484"));
        assert_eq!(r2.status, 403);
        assert!(!st.shutdown.load(Ordering::SeqCst));
    }

    /// The drain the graceful exit waits on: it must not return
    /// while a request guard is alive, and must return once the
    /// last guard drops.
    #[test]
    fn inflight_drain_waits_for_last_guard() {
        let inf = Inflight::new();
        let guard = inf.begin();
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let waiter = {
            let inf = Arc::clone(&inf);
            let done = Arc::clone(&done);
            std::thread::spawn(move || {
                inf.wait_drained();
                done.store(true, Ordering::SeqCst);
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(!done.load(Ordering::SeqCst));
        drop(guard);
        waiter.join().unwrap();
        assert!(done.load(Ordering::SeqCst));
    }

    /// Badge states: alert with the count when issues exist;
    /// after the game drops, the same count is labelled
    /// "last known" and greyed — never dressed as live.
    #[test]
    fn must_fix_badge_states() {
        let st = test_state();
        {
            let mut b = st.badge.lock().unwrap_or_else(|p| p.into_inner());
            b.count = Some(3);
            b.reachable = true;
        }
        let alert = mustfix_badge(&st);
        assert!(
            alert.contains("mustfix--alert") && alert.contains("3 must fix"),
            "{alert}"
        );
        {
            let mut b = st.badge.lock().unwrap_or_else(|p| p.into_inner());
            b.reachable = false;
        }
        let off = mustfix_badge(&st);
        assert!(
            off.contains("mustfix--off") && off.contains("last known"),
            "{off}"
        );
        let r = api_must_fix_count(&st);
        let body = String::from_utf8(r.body).unwrap();
        assert!(
            body.contains("\"count\":3") && body.contains("\"reachable\":false"),
            "{body}"
        );
    }
}
