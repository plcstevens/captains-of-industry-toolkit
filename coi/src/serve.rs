//! `coi serve` — the CoI Companion **web application**.
//!
//! This is the product (SPEC, as corrected 2026-11-02): a functional
//! website you log into, served by the single Rust binary running on
//! the game PC next to the game — no proxy/tailnet needed there, the
//! server talks to the game API on localhost. (The client config still
//! supports the tailnet path, which is how Claudius tests it from the
//! VM.) The CLI subcommands are a debug shell; this is the interface.
//!
//! Features:
//! - Login with a single local user (first visit = set a password;
//!   salted SHA-256 in the SQLite store, session cookie, CSRF tokens).
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
//! require a session + CSRF token.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::client::Client;
use crate::store::{ActivityEvent, Store};
use crate::catalogue::CATALOGUE;
use crate::guides::{guide_by_id, Guide, GUIDES};
use crate::{
    compute_forecasts, compute_issues, fetch_live, flow_pair, fmt_num, get_json,
    render_report_md, snapshot_json, sv, Config, Forecast, Issue, Live,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct Session {
    user: String,
    csrf: String,
    expires: u64,
}

struct Prepared {
    cmd_type: String,
    args: Value,
    summary: String,
    created: u64,
}

struct State {
    cfg: Config,
    db_path: String,
    sessions: Mutex<HashMap<String, Session>>,
    prepared: Mutex<HashMap<String, Prepared>>,
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

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(s.get(i..i + 2).unwrap_or(""), 16).ok())
        .collect()
}

fn password_hash(salt_hex: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(hex_bytes(salt_hex));
    h.update(password.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// HTTP plumbing
// ---------------------------------------------------------------------------

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    form: HashMap<String, String>,
    cookies: HashMap<String, String>,
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
    let mut cookie_header = String::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
        if lower.starts_with("cookie:") {
            cookie_header = line[7..].trim().to_string();
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
    let mut cookies = HashMap::new();
    for pair in cookie_header.split(';') {
        let mut it = pair.trim().splitn(2, '=');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            cookies.insert(k.to_string(), v.to_string());
        }
    }
    Some(Request {
        method,
        path,
        query,
        form,
        cookies,
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
    fn with_cookie(mut self, cookie: String) -> Response {
        self.headers.push(("Set-Cookie".into(), cookie));
        self
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
    println!("CoI Companion web app at http://{bind}:{port}/  (Ctrl-C to stop)");
    println!("First visit asks you to set a password; it lives in the local DB.");
    let state = Arc::new(State {
        cfg: Config {
            authority: cfg.authority.clone(),
            host_header: cfg.host_header.clone(),
            proxy: cfg.proxy.clone(),
        },
        db_path: db_path.to_string(),
        sessions: Mutex::new(HashMap::new()),
        prepared: Mutex::new(HashMap::new()),
    });
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let state = Arc::clone(&state);
                std::thread::spawn(move || {
                    if let Some(req) = read_request(&mut s) {
                        let resp = route(&state, &req);
                        send(&mut s, resp);
                    }
                });
            }
            Err(e) => eprintln!("serve: accept failed: {e}"),
        }
    }
    0
}

fn client(state: &State) -> Client {
    crate::make_client(&state.cfg)
}

fn store(state: &State) -> Result<Store, String> {
    Store::open(&state.db_path)
}

fn session_of(state: &State, req: &Request) -> Option<(String, Session)> {
    let token = req.cookies.get("coi_session")?;
    let sessions = state.sessions.lock().unwrap();
    let s = sessions.get(token)?;
    if s.expires < now_unix() {
        return None;
    }
    Some((
        token.clone(),
        // Rebuild an owned snapshot (Session isn't Clone; fields are).
        Session {
            user: s.user.clone(),
            csrf: s.csrf.clone(),
            expires: s.expires,
        },
    ))
}

fn route(state: &State, req: &Request) -> Response {
    let path = req.path.as_str();
    match (req.method.as_str(), path) {
        ("GET", "/login") => page_login(state, req),
        ("POST", "/login") => do_login(state, req),
        ("POST", "/logout") => {
            if let Some(token) = req.cookies.get("coi_session") {
                state.sessions.lock().unwrap().remove(token);
            }
            Response::redirect("/login")
                .with_cookie("coi_session=; Path=/; Max-Age=0; HttpOnly".into())
        }
        _ => {
            let Some((_, session)) = session_of(state, req) else {
                return Response::redirect("/login");
            };
            match (req.method.as_str(), path) {
                ("GET", "/") | ("GET", "/index.html") => page_overview(state, &session),
                ("GET", "/issues") => page_issues(state, &session),
                ("GET", "/issue") => page_issue(state, &session, req),
                ("GET", "/guides") => page_guides(state, &session),
                ("GET", "/guide") => page_guide(state, &session, req),
                ("GET", "/catalogue") => page_catalogue(state, &session),
                ("GET", "/products") => page_products(state, &session),
                ("GET", "/product") => page_product(state, &session, req),
                ("GET", "/entities") => page_entities(state, &session, req),
                ("GET", "/trade") => page_trade(state, &session, req),
                ("GET", "/plans") => page_plans(state, &session, req),
                ("GET", "/research") => page_research(state, &session),
                ("GET", "/actions") => page_actions(state, &session),
                ("POST", "/actions/prepare") => do_prepare(state, &session, req),
                ("GET", "/prepare") => page_prepare(state, &session, req),
                ("POST", "/actions/prepare-generic") => do_prepare_generic(state, &session, req),
                ("POST", "/actions/prepare-priority") => do_prepare_priority(state, &session, req),
                ("POST", "/actions/prepare-storage-product") => do_prepare_storage_product(state, &session, req),
                ("POST", "/actions/prepare-entity-priority") => do_prepare_entity_priority(state, &session, req),
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
                ("GET", "/screenshot.jpg") => match client(state).get("/screenshot?width=1920&height=1080") {
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
    }
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
body{background:#12141a;color:#e8eaf0;font:14px/1.5 system-ui,sans-serif;margin:0}
header{background:#171b25;padding:10px 20px;display:flex;gap:16px;align-items:center;flex-wrap:wrap}
header .brand{font-weight:700;font-size:16px}
nav a{color:#a8c7fa;text-decoration:none;margin-right:14px}
nav a:hover{text-decoration:underline}
header form{margin-left:auto}
main{padding:20px;max-width:1200px;margin:0 auto}
h1{font-size:20px;margin:0 0 4px} h2{font-size:16px;margin:22px 0 8px}
.sub{color:#9aa3b2} .badge{padding:1px 8px;border-radius:8px;background:#2a2f3a}
.paused{color:#ffb020}.running{color:#4ade80}
.cols{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:14px}
.col{background:#1a1e28;border-radius:10px;padding:12px 14px}
.col h2{margin-top:0}
.card{background:#222836;border-radius:8px;padding:9px 11px;margin-bottom:9px}
.card .p{font-weight:600}.card .t{color:#c7cede}.card .e{color:#9aa3b2;font-size:13px}
.card .f{color:#a8c7fa;font-size:13px}
.fn{border-left:4px solid #f44336}.fl{border-left:4px solid #ff9800}
.wa{border-left:4px solid #ffc107}.ig{border-left:4px solid #5b6472}
table{border-collapse:collapse;width:100%}
th,td{padding:5px 8px;text-align:right;border-bottom:1px solid #262c3a;vertical-align:top}
th:first-child,td:first-child{text-align:left}
button,.btn{background:#2b4acb;color:#fff;border:0;border-radius:6px;padding:6px 14px;cursor:pointer;font-size:14px;text-decoration:none;display:inline-block}
button:hover,.btn:hover{background:#3a5ce0}
button:disabled{background:#39405a;color:#8b93a5;cursor:not-allowed}
button.ghost{background:#2a2f3a}
input[type=password],input[type=text]{background:#0e1015;border:1px solid #39405a;border-radius:6px;color:#e8eaf0;padding:7px 10px;width:260px}
.panel{background:#1a1e28;border-radius:10px;padding:16px 18px;margin-bottom:16px}
.lock{color:#8b93a5}
pre{background:#0e1015;border-radius:8px;padding:12px;overflow:auto;font-size:13px}
img.shot{max-width:100%;border-radius:10px;margin-top:14px}
.err{background:#4a2020;border:1px solid #a33;border-radius:8px;padding:10px 14px;margin-bottom:14px}
.ok{background:#1e4028;border:1px solid #2e7d32;border-radius:8px;padding:10px 14px;margin-bottom:14px}
a{color:#a8c7fa}
.login-wrap{max-width:420px;margin:60px auto}
.spark{color:#9aa3b2;font-size:13px}
";

fn layout(session: &Session, title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} — CoI Companion</title><style>{CSS}</style></head><body>\
         <header><span class=\"brand\">🏛️ CoI Companion</span>\
         <nav><a href=\"/\">Overview</a><a href=\"/issues\">Issues</a><a href=\"/guides\">Guides</a>\
         <a href=\"/products\">Products</a><a href=\"/entities\">Entities</a><a href=\"/trade\">Trade</a><a href=\"/research\">Research</a><a href=\"/plans\">Plans</a><a href=\"/actions\">Actions</a><a href=\"/catalogue\">Catalogue</a>\
         <a href=\"/snapshots\">Snapshots</a><a href=\"/log\">Log</a></nav>\
         <form method=\"post\" action=\"/logout\"><button class=\"ghost\" type=\"submit\">Log out ({})</button></form></header>\
         <main>{body}</main></body></html>",
        esc(&session.user)
    )
}

fn bare_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} — CoI Companion</title><style>{CSS}</style></head><body>{body}</body></html>"
    )
}

fn page_login(state: &State, req: &Request) -> Response {
    let err = req
        .query
        .get("e")
        .map(|e| format!("<div class=\"err\">{}</div>", esc(e)))
        .unwrap_or_default();
    let user = match store(state) {
        Ok(st) => st.web_user().unwrap_or(None),
        Err(_) => None,
    };
    let body = if user.is_none() {
        format!(
            "<div class=\"login-wrap\"><div class=\"panel\">\
             <h1>Welcome to CoI Companion</h1>\
             <p class=\"sub\">No account exists yet on this machine. Choose the password for the local account — it is stored salted + hashed in the local database, never in the open.</p>{err}\
             <form method=\"post\" action=\"/login\">\
             <input type=\"hidden\" name=\"mode\" value=\"setup\">\
             <p><label>Username<br><input type=\"text\" name=\"username\" value=\"captain\" required></label></p>\
             <p><label>Password (min 8 chars)<br><input type=\"password\" name=\"password\" required></label></p>\
             <p><label>Repeat password<br><input type=\"password\" name=\"password2\" required></label></p>\
             <button type=\"submit\">Create account & log in</button></form></div></div>"
        )
    } else {
        format!(
            "<div class=\"login-wrap\"><div class=\"panel\">\
             <h1>CoI Companion — log in</h1>{err}\
             <form method=\"post\" action=\"/login\">\
             <input type=\"hidden\" name=\"mode\" value=\"login\">\
             <p><label>Username<br><input type=\"text\" name=\"username\" required></label></p>\
             <p><label>Password<br><input type=\"password\" name=\"password\" required></label></p>\
             <button type=\"submit\">Log in</button></form></div></div>"
        )
    };
    Response::html(200, bare_page("Log in", &body))
}

fn do_login(state: &State, req: &Request) -> Response {
    let mode = req.form.get("mode").map(|s| s.as_str()).unwrap_or("");
    let username = req.form.get("username").cloned().unwrap_or_default();
    let password = req.form.get("password").cloned().unwrap_or_default();
    let st = match store(state) {
        Ok(s) => s,
        Err(e) => return Response::text(502, &e),
    };
    let fail = |msg: &str| {
        Response::redirect(&format!("/login?e={}", pct_encode(msg)))
    };
    let user = st.web_user().unwrap_or(None);
    let ok = match (mode, &user) {
        ("setup", None) => {
            let password2 = req.form.get("password2").cloned().unwrap_or_default();
            if password.len() < 8 {
                return fail("Password must be at least 8 characters.");
            }
            if password != password2 {
                return fail("Passwords do not match.");
            }
            if username.trim().is_empty() {
                return fail("Username cannot be empty.");
            }
            let salt = rand_hex(16);
            let hash = password_hash(&salt, &password);
            match st.create_web_user(username.trim(), &salt, &hash) {
                Ok(()) => true,
                Err(e) => return Response::text(502, &e),
            }
        }
        ("login", Some((name, salt, hash))) => {
            username == *name && password_hash(salt, &password) == *hash
        }
        ("setup", Some(_)) => return fail("An account already exists — log in instead."),
        _ => false,
    };
    if !ok {
        return fail("Login failed — check the username and password.");
    }
    let token = rand_hex(32);
    let session = Session {
        user: username.trim().to_string(),
        csrf: rand_hex(16),
        expires: now_unix() + 12 * 3600,
    };
    state.sessions.lock().unwrap().insert(token.clone(), session);
    Response::redirect("/").with_cookie(format!(
        "coi_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        12 * 3600
    ))
}

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
        _ => Err(Response::text(403, "CSRF token missing or wrong — go back and retry.")),
    }
}

fn game_unreachable(e: &str) -> Response {
    Response::html(
        502,
        bare_page(
            "Game unreachable",
            &format!(
                "<main><div class=\"err\">The game API did not answer: {}</div>\
                 <p>Is the game running with a save loaded and <code>web_api_start</code> active?</p></main>",
                esc(e)
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

fn game_header(live: &Live) -> String {
    let g = &live.game;
    let paused = g["isPaused"].as_bool().unwrap_or(false);
    format!(
        "<h1>{}</h1><div class=\"sub\">tick {} · <span class=\"badge {}\">{}</span> · speed {}x · game {}</div>",
        esc(&sv(&g["date"])),
        g["tick"],
        if paused { "paused" } else { "running" },
        if paused { "PAUSED" } else { "RUNNING" },
        g["speed"],
        esc(&sv(&g["gameId"])),
    )
}

/// Prepare-form whose confirmation page carries a human-written
/// summary (used by the entity pages, where the label names the exact
/// machine/storage the command addresses). The args JSON itself is
/// still shown verbatim on the confirmation page.
fn prepare_form_labeled(session: &Session, cmd_type: &str, args: Value, summary: &str, label: &str) -> String {
    format!(
        "<form method=\"post\" action=\"/actions/prepare\" style=\"display:inline\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"{}\">\
         <input type=\"hidden\" name=\"args\" value=\"{}\">\
         <input type=\"hidden\" name=\"label\" value=\"{}\">\
         <button type=\"submit\">{}</button></form>",
        esc(&session.csrf),
        esc(cmd_type),
        esc(&args.to_string()),
        esc(summary),
        esc(label),
    )
}

fn prepare_form(session: &Session, cmd_type: &str, args: Value, label: &str, disabled: bool) -> String {
    format!(
        "<form method=\"post\" action=\"/actions/prepare\" style=\"display:inline\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"{}\">\
         <input type=\"hidden\" name=\"args\" value=\"{}\">\
         <button type=\"submit\" {}>{}</button></form>",
        esc(&session.csrf),
        esc(cmd_type),
        esc(&args.to_string()),
        if disabled { "disabled" } else { "" },
        esc(label),
    )
}

fn controls_html(session: &Session, live: &Live) -> String {
    let g = &live.game;
    let paused = g["isPaused"].as_bool().unwrap_or(false);
    let speed = g["speed"].as_i64().unwrap_or(1);
    let mut h = String::from("<div class=\"panel\"><h2 style=\"margin-top:0\">Game controls</h2><p>");
    if paused {
        h.push_str(&prepare_form(session, "SetSimPauseStateCmd", json!({"isPaused": false}), "▶ Resume game", false));
    } else {
        h.push_str(&prepare_form(session, "SetSimPauseStateCmd", json!({"isPaused": true}), "⏸ Pause game", false));
    }
    h.push_str(" &nbsp; Speed: ");
    for s in [1, 2, 3] {
        let label = if s == speed { format!("{s}× (current)") } else { format!("{s}×") };
        h.push_str(&prepare_form(session, "GameSpeedChangeCmd", json!({"newSpeedMultiplier": s}), &label, s == speed));
        h.push(' ');
    }
    h.push_str("</p><p class=\"sub\">Every command goes through a confirmation page showing the exact JSON, and is written to the audit log.</p></div>");
    h
}

fn issues_html(issues: &[Issue], compact: bool) -> String {
    let cols: [(&str, &str); 4] = [
        ("Fix now", "fn"),
        ("Fix later", "fl"),
        ("Watch", "wa"),
        ("Ignore for now", "ig"),
    ];
    let mut h = String::from("<div class=\"cols\">");
    for (q, cls) in cols {
        let items: Vec<&Issue> = issues.iter().filter(|i| i.quadrant == q).collect();
        h.push_str(&format!("<div class=\"col\"><h2>{} ({})</h2>", q, items.len()));
        let show = if compact { 4 } else { usize::MAX };
        for i in items.iter().take(show) {
            h.push_str(&format!(
                "<div class=\"card {cls}\"><div class=\"p\">{}</div><div class=\"t\"><a href=\"/issue?p={}&t={}\">{}</a></div>\
                 <div class=\"e\">{}</div><div class=\"f\">Fix: {}</div></div>",
                esc(&i.product),
                url_enc(&i.product),
                url_enc(&i.title),
                esc(&i.title),
                esc(&i.evidence),
                esc(&i.fix),
            ));
        }
        if compact && items.len() > show {
            h.push_str(&format!(
                "<div class=\"e\"><a href=\"/issues\">+ {} more…</a></div>",
                items.len() - show
            ));
        }
        if items.is_empty() {
            h.push_str("<div class=\"e\">—</div>");
        }
        h.push_str("</div>");
    }
    h.push_str("</div>");
    h
}

fn pinned_table(session: &Session, live: &Live) -> String {
    let mut h = String::from(
        "<table><tr><th>Pinned product</th><th>Quantity</th><th>Stored / cap</th><th>Prod TM/LM</th><th>Cons TM/LM</th><th></th></tr>",
    );
    let mut ids = live.pinned_ids.clone();
    ids.sort_by_key(|id| live.names.get(id).cloned().unwrap_or_else(|| id.clone()));
    for id in &ids {
        let s = &live.stats["products"][id];
        if s.is_null() {
            continue;
        }
        let name = live.names.get(id).cloned().unwrap_or_else(|| id.clone());
        h.push_str(&format!(
            "<tr><td><a href=\"/product?id={}\">{}</a></td><td>{}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(id),
            esc(&name),
            fmt_num(&s["quantity"]),
            fmt_num(&s["stored"]),
            fmt_num(&s["storageCapacity"]),
            flow_pair(&s["produced"]),
            flow_pair(&s["consumed"]),
            prepare_form(session, "PinToggleCmd", json!({"productToToggle": id}), "Unpin", false),
        ));
    }
    h.push_str("</table>");
    h
}

fn forecasts_html(forecasts: &[Forecast]) -> String {
    if forecasts.is_empty() {
        return String::new();
    }
    let mut h = String::from("<p class=\"sub\">At current measured rates — any build, pause or trade change moves these dates.</p><table><tr><th>Product</th><th>Outlook</th><th>Advice</th></tr>");
    for f in forecasts.iter().take(8) {
        let label = match f.kind {
            "fill" => format!("storage full in ≈ {:.1} months", f.months),
            "stall-runway" => format!("stock lasts ≈ {:.1} months (production stalled)", f.months),
            _ => format!("stock lasts ≈ {:.1} months", f.months),
        };
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}<br><span class=\"sub\">{}</span></td><td>{}</td></tr>",
            esc(&f.product),
            esc(&label),
            esc(&f.basis),
            esc(&f.advice),
        ));
    }
    h.push_str("</table>");
    h
}

fn page_overview(state: &State, session: &Session) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            // Game off / API down: still a 200 page (the power panel
            // renders only if the bridge answers, which it won't).
            let mut body = format!(
                "<div class=\"err\">The game API did not answer: {}</div>\
                 <p>Is the game running with a save loaded and <code>web_api_start</code> active? \
                 The live panels appear here once it is.</p>",
                esc(&e)
            );
            body.push_str(&power_panel(state));
            return Response::html(200, layout(session, "Overview", &body));
        }
    };
    let issues = compute_issues(&live);
    let forecasts = compute_forecasts(&live);
    let mut body = game_header(&live);
    body.push_str(&controls_html(session, &live));
    body.push_str(&power_panel(state));
    body.push_str("<h2>What needs attention</h2>");
    body.push_str(&issues_html(&issues, true));
    if !forecasts.is_empty() {
        body.push_str("<h2>Outlook — what happens next at current rates</h2>");
        body.push_str(&forecasts_html(&forecasts));
    }
    body.push_str("<h2>Pinned products</h2>");
    body.push_str(&pinned_table(session, &live));
    body.push_str("<img class=\"shot\" src=\"/screenshot.jpg\" alt=\"live screenshot\">");
    Response::html(200, layout(session, "Overview", &body))
}

fn page_issues(state: &State, session: &Session) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            return Response::html(
                200,
                layout(
                    session,
                    "Issues",
                    &format!(
                        "<div class=\"err\">The game API did not answer: {}</div>\
                         <p>Is the game running with a save loaded and <code>web_api_start</code> active? \
                         The issue board appears here once it is.</p>",
                        esc(&e)
                    ),
                ),
            )
        }
    };
    let issues = compute_issues(&live);
    let mut body = game_header(&live);
    body.push_str("<h2>Issue board — what to fix, in what order</h2>");
    body.push_str("<p class=\"sub\">Generated live from production/consumption flows and storage levels. \
        Urgency comes from how soon a buffer runs dry or blocks; importance from how much of the \
        factory depends on the product. <b>Click any issue title</b> to open it in depth: why it sits \
        in its quadrant (with the numbers), the full evidence, the forecast, how to fix it — and, \
        where a fix is a game command, buttons that prepare it for the specific storages and \
        machines involved.</p>");
    body.push_str(&issues_html(&issues, false));
    body.push_str(&productivity_section(state));
    body.push_str("<p class=\"sub\">Also available as <a href=\"/api/report.md\">markdown</a> or <a href=\"/api/snapshot.json\">raw snapshot JSON</a>.</p>");
    Response::html(200, layout(session, "Issues", &body))
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
        "<h2>Productivity evidence (§5)</h2>\
         <p class=\"sub\">Machine working-share from stored snapshots (bridge census/machines), \
         judged only when the evidence class reaches ESTIMATED — below that, a line reads \
         INSUFFICIENT and means <b>unjudged</b>, not healthy. κ-free self-relative output \
         checks ride along with snapshots.</p><ul>",
    );
    for line in lines {
        body.push_str(&format!("<li>{}</li>", esc(&line)));
    }
    body.push_str("</ul>");
    body
}

fn page_products(state: &State, session: &Session) -> Response {
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => return game_unreachable(&e),
    };
    let mut body = game_header(&live);
    body.push_str("<h2>All products</h2>");
    body.push_str("<p class=\"sub\">Filter: <input type=\"text\" id=\"tfilter\" placeholder=\"name, kind, id…\" oninput=\"filterTable()\"> · click a column header to sort, click again to reverse.</p>");
    body.push_str("<table id=\"ptable\"><thead><tr><th>Product</th><th>Kind</th><th>Quantity</th><th>Stored / cap</th><th>Prod TM/LM</th><th>Cons TM/LM</th><th>Net LM</th><th>Pin</th></tr></thead><tbody>");
    if let Some(map) = live.stats["products"].as_object() {
        let mut ids: Vec<&String> = map.keys().collect();
        ids.sort_by_key(|id| live.names.get(*id).cloned().unwrap_or_else(|| id.to_string()));
        for id in ids {
            let kind = live.kinds.get(id).cloned().unwrap_or_default();
            if kind == "virtual" {
                continue;
            }
            let s = &map[id];
            let name = live.names.get(id).cloned().unwrap_or_else(|| id.to_string());
            let net_lm = crate::flow_at(&s["produced"], 1) - crate::flow_at(&s["consumed"], 1);
            let pinned = live.pinned_ids.iter().any(|p| p == id);
            body.push_str(&format!(
                "<tr><td><a href=\"/product?id={}\">{}</a></td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(id),
                esc(&name),
                esc(&kind),
                fmt_num(&s["quantity"]),
                fmt_num(&s["stored"]),
                fmt_num(&s["storageCapacity"]),
                flow_pair(&s["produced"]),
                flow_pair(&s["consumed"]),
                net_lm,
                prepare_form(session, "PinToggleCmd", json!({"productToToggle": id}), if pinned { "Unpin" } else { "Pin" }, false),
            ));
        }
    }
    body.push_str("</tbody></table>");
    body.push_str(TABLE_JS);
    Response::html(200, layout(session, "Products", &body))
}

fn sources_table(src: &Value, pid: &str) -> String {
    let mut h = String::new();
    let names = &src["names"];
    for side in ["produced", "consumed"] {
        let rows = &src["products"][pid][side]["rows"];
        h.push_str(&format!("<h3>{}</h3>", if side == "produced" { "Produced by" } else { "Consumed by" }));
        if let Some(arr) = rows.as_array() {
            if arr.is_empty() {
                h.push_str("<p class=\"sub\">— none this period —</p>");
            }
            h.push_str("<table><tr><th>Source</th><th>Detail</th><th>Quantity</th></tr>");
            for r in arr {
                let (a, b) = match r["type"].as_str() {
                    Some("recipe") => (
                        names[sv(&r["machine"])].as_str().map(|s| s.to_string()).unwrap_or_else(|| sv(&r["machine"])),
                        names[sv(&r["recipe"])].as_str().map(|s| s.to_string()).unwrap_or_else(|| sv(&r["recipe"])),
                    ),
                    Some("entity") => (sv(&r["entity"]), "entity (aggregate)".to_string()),
                    Some("fuel") => ("Vehicles & ships".to_string(), "fuel use".to_string()),
                    _ => (sv(&r["type"]), String::new()),
                };
                h.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    esc(&a),
                    esc(&b),
                    fmt_num(&r["quantity"])
                ));
            }
            h.push_str("</table>");
        } else {
            h.push_str("<p class=\"sub\">— no data —</p>");
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
        Err(e) => return game_unreachable(&e),
    };
    let name = live.names.get(&pid).cloned().unwrap_or_else(|| pid.clone());
    let s = &live.stats["products"][&pid];
    let mut body = format!("<h1>{}</h1><div class=\"sub\">{}</div>", esc(&name), esc(&pid));
    if !s.is_null() {
        body.push_str(&format!(
            "<div class=\"panel\">Quantity <b>{}</b> · stored {} / {} · produced TM/LM {} · consumed TM/LM {}</div>",
            fmt_num(&s["quantity"]),
            fmt_num(&s["stored"]),
            fmt_num(&s["storageCapacity"]),
            flow_pair(&s["produced"]),
            flow_pair(&s["consumed"]),
        ));
    }
    {
        let pinned = live.pinned_ids.iter().any(|p| p == &pid);
        body.push_str(&format!(
            "<div class=\"panel\">{} on the game's pinned list. {}</div>",
            if pinned { "Currently pinned" } else { "Not pinned" },
            prepare_form(session, "PinToggleCmd", json!({"productToToggle": pid}), if pinned { "Unpin it" } else { "Pin it" }, false),
        ));
    }
    // Monthly history
    let hist = &live.history["products"][&pid];
    if let Some(q) = hist["quantity"].as_array() {
        body.push_str("<h2>Monthly history</h2><table><tr><th>Month (oldest → now)</th><th>Produced</th><th>Consumed</th><th>Quantity</th></tr>");
        for (i, qty) in q.iter().enumerate() {
            body.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                if i + 1 == q.len() { "current".to_string() } else { format!("-{}", q.len() - 1 - i) },
                fmt_num(&hist["produced"][i]),
                fmt_num(&hist["consumed"][i]),
                fmt_num(qty),
            ));
        }
        body.push_str("</table>");
    }
    // Stored-snapshot trend from the DB
    if let Ok(st) = store(state) {
        if let Ok(rows) = st.trend(&name) {
            let mine: Vec<_> = rows.iter().filter(|r| r.7 == name).collect();
            if !mine.is_empty() {
                let series: Vec<String> = mine.iter().map(|r| r.2.to_string()).collect();
                body.push_str(&format!(
                    "<h2>Stored snapshots</h2><p>Quantity across {} snapshot(s): <span class=\"spark\">{}</span></p>",
                    mine.len(),
                    esc(&series.join(" → "))
                ));
            }
        }
    }
    // Sources, this month and last month
    for period in ["thisMonth", "lastMonth"] {
        match get_json(&c, &format!("/api/v1/products/sources?ids={pid}&period={period}")) {
            Ok(src) => {
                body.push_str(&format!(
                    "<h2>Sources — {}</h2>",
                    if period == "thisMonth" { "this month" } else { "last month" }
                ));
                body.push_str(&sources_table(&src, &pid));
            }
            Err(e) => body.push_str(&format!("<p class=\"err\">sources ({period}): {}</p>", esc(&e))),
        }
    }
    Response::html(200, layout(session, &name, &body))
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
            let side = if args["isInput"].as_bool().unwrap_or(false) { "input" } else { "output" };
            let verb = if args["isDisabled"].as_bool().unwrap_or(false) { "Disable" } else { "Enable" };
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

fn page_actions(state: &State, session: &Session) -> Response {
    let live = fetch_live(&client(state)).ok();
    let mut body = String::from("<h1>Actions</h1>");
    body.push_str("<p class=\"sub\">Commands the companion can run in your game. Every command goes: \
        prepare (you see the exact JSON) → confirm → the game executes it → the result lands in the \
        audit log below. Nothing is ever sent without the confirmation step.</p>");
    match &live {
        Some(l) => {
            body.push_str(&game_header(l));
            body.push_str(&controls_html(session, l));
        }
        None => body.push_str("<div class=\"err\">Game API unreachable — controls unavailable.</div>"),
    }
    body.push_str(&format!(
        "<div class=\"panel\"><h2 style=\"margin-top:0\">Global priorities</h2>\
         <p class=\"sub\">Sets one of the game's global priorities by id (SetGlobalPriorityCmd). \
         The API does not publish the valid ids, so this is a plain entry form: a wrong id makes the \
         game return failure and changes nothing — attempts are audit-logged either way, and \
         confirmed ids will be listed here as they're verified.</p>\
         <form method=\"post\" action=\"/actions/prepare-priority\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <label>Priority id <input type=\"text\" name=\"priorityId\" required></label> \
         <label>Value <input type=\"text\" name=\"priority\" value=\"0\" size=\"5\" required></label> \
         <button type=\"submit\">Prepare…</button></form></div>",
        esc(&session.csrf)
    ));
    if bridge_present(state) {
        body.push_str("<div class=\"panel\"><h2 style=\"margin-top:0\">Fix actions — entity-targeted ✅ unlocked by CoiBridge</h2>\
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
        body.push_str("<div class=\"panel\"><h2 style=\"margin-top:0\">Fix actions — locked for now</h2>\
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
    body.push_str("<h2>Audit log</h2>");
    match store(state) {
        Ok(st) => match st.audit_recent(20) {
            Ok(rows) if !rows.is_empty() => {
                body.push_str("<table><tr><th>When</th><th>Action</th><th>Detail</th></tr>");
                for (ts, action, detail) in rows {
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                        fmt_clock(ts as u64),
                        esc(&action),
                        esc(&detail)
                    ));
                }
                body.push_str("</table>");
            }
            _ => body.push_str("<p class=\"sub\">No commands have been run yet.</p>"),
        },
        Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
    }
    Response::html(200, layout(session, "Actions", &body))
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
        return Response::text(400, "That command is excluded by policy (see the Catalogue).");
    }
    let args: Value = match req.form.get("args").and_then(|a| serde_json::from_str(a).ok()) {
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
                    let name = live.names.get(pid).cloned().unwrap_or_else(|| pid.to_string());
                    summary = if live.pinned_ids.iter().any(|p| p == pid) {
                        format!("Unpin {name} from the pinned list")
                    } else {
                        format!("Pin {name} to the pinned list")
                    };
                }
                Err(e) => return game_unreachable(&e),
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
                Err(e) => return game_unreachable(&e),
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
fn finish_prepare(state: &State, session: &Session, cmd_type: &str, args: &Value, summary: &str) -> Response {
    let token = rand_hex(16);
    state.prepared.lock().unwrap().insert(
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
            if g["isPaused"].as_bool().unwrap_or(false) { "PAUSED" } else { "running" },
            g["speed"]
        ),
        Err(e) => format!("Warning: could not read current game state ({e})"),
    };
    let body = format!(
        "<h1>Confirm command</h1>\
         <div class=\"panel\"><p style=\"font-size:16px\"><b>{}</b></p>\
         <p class=\"sub\">{}</p>\
         <p>This exact JSON will be sent to the game's <code>POST /command</code>:</p>\
         <pre>{}</pre>\
         <form method=\"post\" action=\"/actions/apply\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"token\" value=\"{}\">\
         <button type=\"submit\">Run it</button> \
         <a class=\"btn\" href=\"/actions\" style=\"background:#2a2f3a\">Cancel</a></form>\
         <p class=\"sub\">This confirmation expires in 2 minutes and can be used once. The result will be written to the audit log.</p></div>",
        esc(summary),
        esc(&state_line),
        esc(&serde_json::to_string_pretty(&envelope).unwrap()),
        esc(&session.csrf),
        esc(&token),
    );
    Response::html(200, layout(session, "Confirm command", &body))
}

/// Generic prepare form for ANY catalogue command, generated from
/// its captured argument schema. This is what turns the catalogue
/// from documentation into coverage: a command family does not
/// need a hand-built page before it can be run through the §12
/// flow — the exact JSON is shown at the confirm step either way.
fn page_prepare(_state: &State, session: &Session, req: &Request) -> Response {
    let Some(name) = req.query.get("cmd").cloned() else {
        return Response::redirect("/catalogue");
    };
    let Some(info) = CATALOGUE.iter().find(|c| c.name == name) else {
        return Response::html(
            404,
            layout(session, "Unknown command", "<h1>Unknown command</h1><p><a href=\"/catalogue\">Catalogue</a></p>"),
        );
    };
    if info.status == "excluded" {
        return Response::html(
            200,
            layout(
                session,
                info.name,
                &format!(
                    "<h1>{}</h1><div class=\"panel\"><p><b>Excluded by policy.</b> {}</p><p><a href=\"/catalogue\">← Catalogue</a></p></div>",
                    esc(info.name),
                    esc(info.note),
                ),
            ),
        );
    }
    let mut body = format!(
        "<h1>{}</h1><p class=\"sub\">{} · {} · status: {} — generic form generated from the captured schema. \
         Nothing is sent yet: the next step shows the exact JSON for confirmation.</p>",
        esc(info.purpose),
        esc(info.name),
        esc(info.domain),
        esc(info.status),
    );
    if !info.note.is_empty() {
        body.push_str(&format!("<div class=\"panel\"><p>{}</p></div>", esc(info.note)));
    }
    body.push_str(&format!(
        "<form method=\"post\" action=\"/actions/prepare-generic\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <input type=\"hidden\" name=\"type\" value=\"{}\">\
         <table>",
        esc(&session.csrf),
        esc(info.name),
    ));
    if info.spec.is_empty() {
        body.push_str("<tr><td colspan=\"2\" class=\"sub\">This command takes no arguments.</td></tr>");
    }
    for (an, at) in info.spec {
        let optional = at.contains("or null");
        body.push_str(&format!(
            "<tr><td><label for=\"a_{}\">{} <span class=\"sub\">({}{})</span></label></td>\
             <td><input id=\"a_{}\" name=\"arg_{}\" style=\"width:320px\"></td></tr>",
            esc(an),
            esc(an),
            esc(at),
            if optional { ", optional" } else { "" },
            esc(an),
            esc(an),
        ));
    }
    body.push_str(&format!(
        "<tr><td><label for=\"plabel\">Summary label <span class=\"sub\">(optional)</span></label></td>\
         <td><input id=\"plabel\" name=\"label\" maxlength=\"240\" style=\"width:320px\" placeholder=\"{}\"></td></tr>\
         </table><p><button type=\"submit\">Prepare…</button> <a class=\"btn\" href=\"/catalogue\" style=\"background:#2a2f3a\">Back to catalogue</a></p></form>",
        esc(info.purpose),
    ));
    Response::html(200, layout(session, info.purpose, &body))
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
        return Response::text(400, "That command is excluded by policy (see the Catalogue).");
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
        Err(e) => return game_unreachable(&e),
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
    let prepared = state.prepared.lock().unwrap().remove(&token);
    let prepared = match prepared {
        Some(p) if now_unix() - p.created <= 120 => p,
        _ => {
            return Response::html(
                400,
                layout(
                    session,
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
    if let Ok(st) = store(state) {
        let _ = st.audit_add(
            now_unix() as i64,
            &prepared.cmd_type,
            &format!(
                "{} · args {} · HTTP {} · {}",
                prepared.summary,
                prepared.args,
                http,
                if ok { "success" } else { "FAILED" }
            ),
        );
        // Addendum XVI: the same attempt as a rich activity event —
        // exact envelope, game response, session user, game context.
        let ctx = if before.is_object() { &before } else { &after_g };
        let _ = st.activity_add(&ActivityEvent {
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
        });
    }
    let after = if after_g.is_object() {
        let g = &after_g;
        format!(
            "Game now: {} · {} · speed {}×",
            sv(&g["date"]),
            if g["isPaused"].as_bool().unwrap_or(false) { "PAUSED" } else { "running" },
            g["speed"]
        )
    } else {
        String::new()
    };
    let body = format!(
        "<h1>{}</h1>\
         <div class=\"{}\"><b>{}</b> — {}</div>\
         <div class=\"panel\"><p>Game response (HTTP {}):</p><pre>{}</pre><p class=\"sub\">{}</p>\
         <p><a class=\"btn\" href=\"/actions\">Back to Actions</a> <a class=\"btn\" href=\"/\" style=\"background:#2a2f3a\">Overview</a></p></div>",
        esc(&prepared.summary),
        if ok { "ok" } else { "err" },
        if ok { "Command executed" } else { "Command failed" },
        esc(&prepared.summary),
        http,
        esc(&result_text),
        esc(&after),
    );
    Response::html(200, layout(session, "Command result", &body))
}

// ---------------------------------------------------------------------------
// Catalogue — every command the game exposes, and what we know about it
// ---------------------------------------------------------------------------

fn page_catalogue(_state: &State, session: &Session) -> Response {
    let verified = CATALOGUE.iter().filter(|c| c.status == "verified").count();
    let excluded = CATALOGUE.iter().filter(|c| c.status == "excluded").count();
    let entity = CATALOGUE.iter().filter(|c| c.entity).count();
    let mut body = format!(
        "<h1>Actions understood — the command catalogue</h1>\
         <p class=\"sub\">Every command the game exposes over POST /command (inventory captured \
         from this game, v0.8.8). Purposes are read from command names and argument schemas; \
         only commands marked <b>verified</b> have actually been executed and observed through \
         this website's confirm flow. This registry is step A of the solo-play road: an agent \
         can only play with hands that are understood.</p>\
         <div class=\"cols\">\
         <div class=\"col\"><h2>{} commands</h2><p>{} target a specific entity (addressable via the bridge census); {} are global.</p></div>\
         <div class=\"col\"><h2>{} verified live</h2><p>Executed through the prepare → confirm → audit flow on the real game. {} excluded by policy (see its note). Every other command is <b>wired</b>: preparable from its row below through a generic schema form — the exact JSON is always shown before anything is sent.</p></div>\
         <div class=\"col\"><h2>Construction: the primitive exists</h2><p><b>CreateStaticEntityCmd</b> places a building/transport from a prototype at a tile transform, with <b>FinishBuildOfStaticEntityCmd</b> beside it, <b>CreateTrainTrackEntityWithDirectionCmd</b> for rails, and full train-line / train-network / vehicle-build-queue creation commands. Whether placement creates a construction site or a finished building — and how TileTransform encodes position — is untested: sandbox-save experiments are the next step, never the live save.</p></div>\
         </div>",
        CATALOGUE.len(),
        entity,
        CATALOGUE.len() - entity,
        verified,
        excluded,
    );
    body.push_str("<input id=\"tfilter\" type=\"search\" placeholder=\"Filter commands…\" style=\"width:100%;padding:8px;margin:10px 0\">");
    body.push_str("<table id=\"ctable\"><thead><tr><th>Domain</th><th>Command</th><th>Purpose & args</th><th>Targets</th><th>Status</th></tr></thead><tbody>");
    for c in CATALOGUE {
        let status = match c.status {
            "verified" => "✅ verified",
            "excluded" => "⛔ excluded",
            _ => "wired",
        };
        body.push_str(&format!(
            "<tr><td>{}</td><td><b>{}</b>{}</td><td>{}<br><span class=\"sub\">{}</span>",
            esc(c.domain),
            esc(c.name),
            if c.status == "excluded" {
                String::new()
            } else {
                format!("<br><a class=\"btn\" href=\"/prepare?cmd={}\">Prepare…</a>", esc(c.name))
            },
            esc(c.purpose),
            esc(c.args),
        ));
        if !c.note.is_empty() {
            body.push_str(&format!("<br><span class=\"sub\">Note: {}</span>", esc(c.note)));
        }
        body.push_str(&format!(
            "</td><td>{}</td><td>{}</td></tr>",
            if c.entity { "entity" } else { "global" },
            status,
        ));
    }
    body.push_str("</tbody></table>");
    body.push_str(TABLE_JS);
    body.push_str("<script>wireTable('ctable','tfilter');</script>");
    Response::html(200, layout(session, "Command catalogue", &body))
}

// ---------------------------------------------------------------------------
// Guides — failure-mode library, linked from issues
// ---------------------------------------------------------------------------

/// Which guides speak to a given issue. Rules are deliberately
/// simple and visible: storage failures → Storage + Transport;
/// seasonal producers → Farming; oil-chain products → Oil & diesel;
/// stalls → Priorities + Transport (a stall is usually a delivery
/// or attention problem before it is a production problem).
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

/// Trade, loans & edicts — the state families the vanilla web API
/// never exposes, read through CoiBridge v0.6 routes. Each panel
/// degrades to an "update the mod" note when its route is missing,
/// so this page is safe to ship ahead of the mod update.
// ---------------------------------------------------------------------------
// Plans — baked-in game plans + their latest dry-run verdicts (Round 4).
// The canonical plan JSONs and plans/verdicts.json are compiled into
// the binary (include_str!), so the page works in the single-file exe
// distribution with no runtime file dependencies. Verdicts are baked
// from the captured state in live/ by tools/gen_verdicts.py — they are
// as of that snapshot, NOT live state, and the page says so.
// ---------------------------------------------------------------------------

const PLAN_JSON_FRESH: &str = include_str!("../../plans/fresh-start-v1.json");
const PLAN_JSON_CONT: &str = include_str!("../../plans/dragon-isle-continuation-v1.json");
const PLANS_VERDICTS: &str = include_str!("../../plans/verdicts.json");

fn arg_form_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn page_plans(_state: &State, session: &Session, _req: &Request) -> Response {
    let verdicts: Value = serde_json::from_str(PLANS_VERDICTS).unwrap_or(Value::Null);
    let plan_docs: Vec<Value> = [PLAN_JSON_FRESH, PLAN_JSON_CONT]
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect();
    let mut body = String::from(
        "<h1>Game plans</h1>\
         <p class=\"sub\">Machine-executable plans (GAME-PLAN-FORMAT v1) with their latest \
         <b>dry-run</b> verdicts — gates evaluated against captured state, orders rendered as \
         would-prepare. Nothing here has been executed. A Prepare button sends the order through \
         the standard generic prepare flow; you still confirm the exact JSON before anything runs.</p>",
    );
    if let Some(src) = verdicts["state"]["source"].as_str() {
        body.push_str(&format!(
            "<div class=\"panel\"><p>Verdict state source: {}<br>Captured: date {}, tick {}, gameId {}.</p></div>",
            esc(src),
            esc(&verdicts["state"]["date"].to_string()),
            esc(&verdicts["state"]["tick"].to_string()),
            esc(&verdicts["state"]["gameId"].to_string()),
        ));
    }
    let Some(plans) = verdicts["plans"].as_array() else {
        body.push_str("<div class=\"err\">Baked-in verdict data failed to parse.</div>");
        return Response::html(200, layout(session, "Plans", &body));
    };
    for (i, p) in plans.iter().enumerate() {
        let pid = p["id"].as_str().unwrap_or("?");
        let doc = plan_docs.get(i);
        body.push_str(&format!(
            "<h2>{}</h2><p class=\"sub\">Plan <code>{}</code> v{} · game {} · {}</p>",
            esc(p["title"].as_str().unwrap_or(pid)),
            esc(pid),
            esc(&p["version"].to_string()),
            esc(&p["game_version"].to_string()),
            esc(p["plan_status"].as_str().unwrap_or("")),
        ));
        if let Some(d) = doc {
            if let Some(end) = d["plan"]["end_state"].as_str() {
                body.push_str(&format!("<p class=\"sub\">End state: {}</p>", esc(end)));
            }
        }
        body.push_str("<table><thead><tr><th>Phase</th><th>Name</th><th>Dry-run status</th></tr></thead><tbody>");
        if let Some(phases) = p["phases"].as_array() {
            for ph in phases {
                body.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    esc(ph["id"].as_str().unwrap_or("")),
                    esc(ph["name"].as_str().unwrap_or("")),
                    esc(ph["status"].as_str().unwrap_or("")),
                ));
            }
        }
        body.push_str("</tbody></table>");
        // Active phase's orders.
        if let Some(active) = p["active_phase"].as_str() {
            body.push_str(&format!(
                "<h3>Next orders — active phase <code>{}</code> (dry run)</h3>",
                esc(active)
            ));
            if let Some(orders) = p["orders"].as_array() {
                for o in orders {
                    let oid = o["id"].as_str().unwrap_or("?");
                    let do_ = o["do"].as_str().unwrap_or("?");
                    let head = o["status_head"].as_str().unwrap_or("");
                    body.push_str(&format!(
                        "<div class=\"panel\"><p><b>{}</b> — {} · <code>{}</code></p>",
                        esc(oid),
                        esc(head),
                        esc(do_),
                    ));
                    if let Some(note) = o["note"].as_str() {
                        if !note.is_empty() {
                            body.push_str(&format!("<p class=\"sub\">{}</p>", esc(note)));
                        }
                    }
                    if let Some(reasons) = o["held_reasons"].as_array() {
                        for r in reasons {
                            body.push_str(&format!(
                                "<p class=\"sub\">Held: {}</p>",
                                esc(r.as_str().unwrap_or(""))
                            ));
                        }
                    }
                    // A Prepare form only for command orders the dry run
                    // marks WOULD PREPARE (not held, not a human-task kind).
                    let is_command = !matches!(
                        do_,
                        "place-blueprint" | "manual-step" | "observe-only" | "wait"
                    );
                    if head == "WOULD PREPARE" && is_command {
                        body.push_str(&format!(
                            "<form method=\"post\" action=\"/actions/prepare-generic\">\
                             <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
                             <input type=\"hidden\" name=\"type\" value=\"{}\">",
                            esc(&session.csrf),
                            esc(do_),
                        ));
                        if let Some(conc) = o["concrete_args"].as_object() {
                            for (k, v) in conc {
                                body.push_str(&format!(
                                    "<input type=\"hidden\" name=\"arg_{}\" value=\"{}\">",
                                    esc(k),
                                    esc(&arg_form_value(v)),
                                ));
                            }
                        }
                        if let Some(sel) = o["selector_args"].as_object() {
                            for (k, v) in sel {
                                body.push_str(&format!(
                                    "<p><label>{} — entity selector (proto {}, product {}, match {}): \
                                     type the entity id from the Entities page \
                                     <input name=\"arg_{}\" size=\"8\" placeholder=\"entity id\"></label></p>",
                                    esc(k),
                                    esc(v["proto"].as_str().unwrap_or("?")),
                                    esc(v["product"].as_str().unwrap_or("?")),
                                    esc(v["match"].as_str().unwrap_or("?")),
                                    esc(k),
                                ));
                            }
                            body.push_str("<p class=\"sub\">Selectors are not auto-resolved yet — the confirm step shows the exact JSON before anything is sent.</p>");
                        }
                        body.push_str(&format!(
                            "<p><label>Summary label <input name=\"label\" maxlength=\"240\" style=\"width:320px\" value=\"{}\"></label> \
                             <button type=\"submit\">Prepare…</button></p></form>",
                            esc(&format!("{oid} (from Plans)")),
                        ));
                    }
                    body.push_str("</div>");
                }
            }
        } else {
            body.push_str("<p>Plan complete by its gates — no active phase, no next orders.</p>");
        }
    }
    Response::html(200, layout(session, "Plans", &body))
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
    let mut body = String::from(
        "<h1>Trade, loans &amp; edicts</h1>\
         <p class=\"sub\">Read through CoiBridge — the vanilla API has no routes for any of this. \
         Actions go through the usual prepare → confirm → audit flow.</p>",
    );

    // ---- Contracts ----
    body.push_str("<h2>Contracts</h2>");
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
                            if key == "active" { vi(&c["toBuy"]).to_string() } else { "—".to_string() },
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
        None => body.push_str(
            "<div class=\"panel\"><p>Contracts arrive with <b>CoiBridge v0.6</b> — update the mod to see them here.</p></div>",
        ),
    }

    // ---- Quick trades (CoiBridge v0.6.1) ----
    body.push_str("<h2>Quick trades</h2>");
    match bridge_json(state, "/coi/v1/quick-trades") {
        Some(v) => {
            body.push_str("<table class=\"sortable\"><thead><tr><th>Trade</th><th>You receive</th><th>You pay</th><th>Unity cost (raw)</th><th>Step</th><th>State</th><th></th></tr></thead><tbody>");
            if let Some(arr) = v["trades"].as_array() {
                for t in arr {
                    let id = sv(&t["id"]);
                    let sold_out = t["isSoldOut"].as_bool().unwrap_or(false);
                    let state_txt = if sold_out {
                        "sold out"
                    } else if t["hasPriceIncreased"].as_bool().unwrap_or(false) {
                        "price increased"
                    } else {
                        "available"
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
        None => body.push_str(
            "<div class=\"panel\"><p>Quick trades arrive with <b>CoiBridge v0.6.1</b> — update the mod to see them here.</p></div>",
        ),
    }

    // ---- Loans ----
    body.push_str("<h2>Loans</h2>");
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
        None => body.push_str(
            "<div class=\"panel\"><p>Loans arrive with <b>CoiBridge v0.6</b> — update the mod to see them here.</p></div>",
        ),
    }

    // ---- Edicts ----
    body.push_str("<h2>Edicts</h2>");
    match bridge_json(state, "/coi/v1/edicts") {
        Some(v) => {
            body.push_str("<table class=\"sortable\"><thead><tr><th>Edict</th><th>Enabled</th><th>Active</th><th>Can enable</th><th>Why / why not</th></tr></thead><tbody>");
            if let Some(arr) = v["edicts"].as_array() {
                for e in arr {
                    let why = if sv(&e["reason"]).is_empty() {
                        sv(&e["inactiveReason"]).to_string()
                    } else {
                        sv(&e["reason"]).to_string()
                    };
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        esc(&sv(&e["id"])),
                        if e["isEnabled"].as_bool().unwrap_or(false) { "✅" } else { "—" },
                        if e["isActive"].as_bool().unwrap_or(false) { "✅" } else { "—" },
                        if e["canBeEnabled"].as_bool().unwrap_or(false) { "yes" } else { "no" },
                        esc(&why),
                    ));
                }
            }
            body.push_str("</tbody></table><p class=\"sub\">Toggling edicts is not wired yet: the game's ToggleEdictEnabledCmd exists inside the game but is not in the web API's command list, so it needs a sandbox test before it can go through the confirm flow. Toggle in-game for now.</p>");
        }
        None => body.push_str(
            "<div class=\"panel\"><p>Edicts arrive with <b>CoiBridge v0.6</b> — update the mod to see them here.</p></div>",
        ),
    }
    body.push_str(TABLE_JS);
    Response::html(200, layout(session, "Trade", &body))
}

fn page_guides(_state: &State, session: &Session) -> Response {
    let mut body = String::from(
        "<h1>Guides</h1><p class=\"sub\">Organized around the failures the triage board watches \
         for: what goes wrong, how it presents, and how to keep it from happening. Researched \
         from the official wiki and community practice, and grounded in this run's own numbers — \
         no invented figures.</p><div class=\"cols\">",
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
    Response::html(200, layout(session, "Guides", &body))
}

fn page_guide(state: &State, session: &Session, req: &Request) -> Response {
    let Some(id) = req.query.get("id").cloned() else {
        return Response::redirect("/guides");
    };
    let Some(g) = guide_by_id(&id) else {
        return Response::html(
            404,
            layout(session, "No such guide", "<h1>No such guide</h1><p><a href=\"/guides\">All guides</a></p>"),
        );
    };
    let mut body = format!("<h1>{}</h1><p class=\"sub\">{}</p>", esc(g.title), esc(g.blurb));
    // Open issues right now that this guide speaks to.
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
            body.push_str("<h2>In your factory right now</h2>");
            for i in rel {
                body.push_str(&format!(
                    "<div class=\"card\"><div class=\"p\">{}</div><div class=\"t\"><a href=\"/issue?p={}&t={}\">{}</a></div><div class=\"e\">{}</div></div>",
                    esc(&i.product),
                    url_enc(&i.product),
                    url_enc(&i.title),
                    esc(&i.title),
                    esc(&i.evidence),
                ));
            }
        }
    }
    for s in g.sections {
        body.push_str(&format!("<h2>{}</h2><div class=\"panel\">{}</div>", esc(s.heading), s.body));
    }
    body.push_str("<p class=\"sub\"><a href=\"/guides\">← All guides</a></p>");
    Response::html(200, layout(session, g.title, &body))
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
fn power_panel(state: &State) -> String {
    let Some(v) = bridge_json(state, "/coi/v1/power") else {
        return String::new();
    };
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
        format!(
            "<br><span class=\"sub\">Averages via CoiBridge v0.6.3 — decoding UNVERIFIED: \
             produced ≈{} kW last day · ≈{} kW last month · ≈{} kW last year; \
             consumed ≈{} kW last day · ≈{} kW last month · ≈{} kW last year; \
             generation capacity ≈{} kW last day · ≈{} kW last month · ≈{} kW last year.</span>",
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
    format!(
        "<h2>Electric power</h2><div class=\"panel\">Generated <b>{} kW</b> · consumed <b>{} kW</b> · \
         balance <b>{} kW</b> · capacity {} / {} kW<br>\
         <span class=\"sub\">Demanded {} kW (surplus demand {} kW) · wasted {} kW · \
         {} generator(s), {} consumer(s) — this tick, via CoiBridge.</span>{}</div>",
        vi(&v["generatedKw"]),
        vi(&v["consumedKw"]),
        vi(&v["balanceKw"]),
        vi(&v["generationCapacityKw"]),
        vi(&v["maxGenerationCapacityKw"]),
        vi(&v["demandedKw"]),
        vi(&v["surplusDemandedKw"]),
        vi(&v["wastedKw"]),
        vi(&v["generators"]),
        vi(&v["consumers"]),
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
        let (verb, target) = if disabled { ("Enable", false) } else { ("Disable", true) };
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
        "<tr><td>#{sid}</td><td>{}</td><td><a href=\"/product?id={}\">{}</a></td><td>{} / {}</td><td>{:.1}%</td><td>{}</td><td>{}</td><td>{}{}</td></tr>",
        esc(&proto),
        esc(&pid),
        esc(&pname),
        qty,
        cap,
        pct,
        if in_dis { "input OFF" } else { "input on" },
        if out_dis { "output OFF" } else { "output on" },
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
        Err(e) => return game_unreachable(&e),
    };
    let issues = compute_issues(&live);
    let Some(issue) = issues.iter().find(|i| i.product == p && i.title == t) else {
        return Response::html(
            200,
            layout(
                session,
                "Issue gone",
                "<h1>That issue is no longer present</h1><p>The live numbers have moved since the board was rendered — it may have resolved, or changed quadrant. <a href=\"/issues\">Back to the board</a></p>",
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
    let mut body = format!(
        "<h1>{}</h1><div class=\"sub\">{} · <span class=\"badge\">{}</span></div>",
        esc(&issue.title),
        match &pid {
            Some(id) => format!("<a href=\"/product?id={}\">{}</a>", esc(id), esc(&issue.product)),
            None => esc(&issue.product),
        },
        esc(issue.quadrant),
    );
    body.push_str(&format!(
        "<div class=\"card {cls}\" style=\"margin-top:14px\"><div class=\"t\">Why \u{2018}{}\u{2019}?</div><div class=\"e\">{}</div></div>",
        esc(issue.quadrant),
        quadrant_why(issue.quadrant),
    ));
    // The numbers behind it.
    if let Some(id) = &pid {
        let s = &live.stats["products"][id];
        if !s.is_null() {
            body.push_str(&format!(
                "<h2>The numbers</h2><div class=\"panel\">Quantity <b>{}</b> · stored {} / {} · produced TM/LM {} · consumed TM/LM {} · net last month <b>{}</b>/mo</div>",
                fmt_num(&s["quantity"]),
                fmt_num(&s["stored"]),
                fmt_num(&s["storageCapacity"]),
                flow_pair(&s["produced"]),
                flow_pair(&s["consumed"]),
                crate::flow_at(&s["produced"], 1) - crate::flow_at(&s["consumed"], 1),
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
                body.push_str("<h2>Monthly history</h2><table><tr><th>Month</th><th>Produced</th><th>Consumed</th><th>Quantity</th></tr>");
                for (i, qty) in q.iter().enumerate().skip(keep) {
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        if i + 1 == n { "current (partial)".to_string() } else { format!("-{}", n - 1 - i) },
                        fmt_num(&h["products"][id]["produced"][i]),
                        fmt_num(&h["products"][id]["consumed"][i]),
                        fmt_num(qty),
                    ));
                }
                body.push_str("</table>");
            }
        }
    }
    body.push_str(&format!(
        "<h2>Evidence</h2><div class=\"panel\">{}</div>",
        esc(&issue.evidence)
    ));
    if let Some(f) = fc {
        let label = match f.kind {
            "fill" => format!("storage full in ≈ {:.1} months", f.months),
            "stall-runway" => format!("stock lasts ≈ {:.1} months (production stalled)", f.months),
            _ => format!("stock lasts ≈ {:.1} months", f.months),
        };
        body.push_str(&format!(
            "<h2>Outlook</h2><div class=\"panel\"><b>{}</b><br><span class=\"sub\">Basis: {}</span></div>",
            esc(&label),
            esc(&f.basis),
        ));
    }
    body.push_str(&format!(
        "<h2>How to fix it</h2><div class=\"panel\">{}</div>",
        esc(&issue.fix)
    ));
    if let Some(f) = fc {
        if f.advice != issue.fix {
            body.push_str(&format!(
                "<div class=\"panel\">Forecast advice: {}</div>",
                esc(&f.advice)
            ));
        }
    }
    // Guides that speak to this failure mode.
    let gl = guides_for_issue(issue, pid.as_deref());
    if !gl.is_empty() {
        body.push_str("<h2>Guides for this failure mode</h2><p>");
        for g in gl {
            body.push_str(&format!(
                "<a class=\"btn\" href=\"/guide?id={}\">{}</a> ",
                g.id,
                esc(g.title),
            ));
        }
        body.push_str("</p>");
    }
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
            if let Ok(src) = get_json(&c, &format!("/api/v1/products/sources?ids={id}&period=thisMonth")) {
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
                            .map(|a| a.iter().map(|v| sv(v)).collect())
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
                                    "⏯ Toggle",
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
    Response::html(200, layout(session, "Issue detail", &body))
}

/// Machines per instance (CoiBridge v0.6.4's GET /coi/v1/machines):
/// one row per machine instance with its position, live state,
/// recipe(s) and paused/enabled flags. Degrades to an "update the
/// mod" note when the route is absent (older mod, or the bridge is
/// off). The section is rendered on every /entities path — including
/// when the game API itself did not answer — so it is computed once
/// at the top of page_entities. Malformed rows never panic: missing
/// fields render as "—".
fn machines_section(state: &State) -> String {
    let mut s = String::from("<h2>Machines (per-instance)</h2>");
    let Some(v) = bridge_json(state, "/coi/v1/machines") else {
        s.push_str("<div class=\"panel\"><p>Per-instance machine states arrive with <b>CoiBridge v0.6.4</b> — update the mod to see them here.</p></div>");
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
    s.push_str(&format!("<p class=\"sub\">{}</p>", summary));
    if rows.is_empty() {
        s.push_str("<p class=\"sub\">No machines reported by the bridge.</p>");
        return s;
    }
    // State dropdown options: the distinct states, alphabetical.
    let mut states: Vec<&String> = counts.iter().map(|(st, _)| st).collect();
    states.sort();
    let mut opts = String::from("<option value=\"\">all states</option>");
    for st in states {
        opts.push_str(&format!("<option value=\"{}\">{}</option>", esc(st), esc(st)));
    }
    s.push_str(&format!(
        "<p class=\"sub\">Filter: <input type=\"text\" id=\"mfilter\" placeholder=\"proto, id, recipe…\" oninput=\"filterMachines()\"> · \
         State: <select id=\"mstate\" onchange=\"filterMachines()\">{}</select></p>\
         <table id=\"mtable\"><thead><tr><th>Id</th><th>Proto</th><th>Pos</th><th>State</th><th>Recipe(s)</th><th>Paused</th><th>Enabled</th></tr></thead><tbody>",
        opts
    ));
    let flag = |v: &Value| -> &'static str {
        match v.as_bool() {
            Some(true) => "yes",
            Some(false) => "no",
            None => "—",
        }
    };
    for r in &rows {
        let id = match r["id"].as_i64() {
            Some(n) => format!("#{n}"),
            None => "—".to_string(),
        };
        let pos = match r["pos"].as_array() {
            Some(a) if !a.is_empty() => a
                .iter()
                .map(|c| c.as_i64().map(|n| n.to_string()).unwrap_or_else(|| "—".to_string()))
                .collect::<Vec<_>>()
                .join(", "),
            _ => "—".to_string(),
        };
        let recipes = match r["recipes"].as_array() {
            Some(a) if !a.is_empty() => a
                .iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            _ => "—".to_string(),
        };
        let state_txt = txt(&r["state"]);
        s.push_str(&format!(
            "<tr data-state=\"{}\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&state_txt),
            esc(&id),
            esc(&txt(&r["proto"])),
            esc(&pos),
            esc(&state_txt),
            esc(&recipes),
            flag(&r["isPaused"]),
            flag(&r["isEnabled"]),
        ));
    }
    s.push_str("</tbody></table>");
    s.push_str(MACHINES_JS);
    s
}

/// Machines-table filter: the same idiom as TABLE_JS's filterTable,
/// scoped to #mtable (the storages table owns #ptable/#tfilter),
/// plus an exact-match state dropdown reading each row's data-state.
const MACHINES_JS: &str = "<script>
function filterMachines(){var q=document.getElementById('mfilter').value.toLowerCase();var st=document.getElementById('mstate').value;document.querySelectorAll('#mtable tbody tr').forEach(function(r){var ok=r.textContent.toLowerCase().indexOf(q)>=0;if(ok&&st!==''){ok=r.getAttribute('data-state')===st}r.style.display=ok?'':'none'})}
</script>";

fn page_entities(state: &State, session: &Session, req: &Request) -> Response {
    let machines_html = machines_section(state);
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => {
            let mut html = format!(
                "<h1>Entities — the CoiBridge directory</h1>\
                 <div class=\"err\">The game API did not answer: {}</div>\
                 <p>Is the game running with a save loaded and <code>web_api_start</code> active? \
                 The entity directory appears here once it is.</p>",
                esc(&e)
            );
            html.push_str(&machines_html);
            return Response::html(200, layout(session, "Entities", &html));
        }
    };
    let mut body = game_header(&live);
    body.push_str("<h1>Entities — the CoiBridge directory</h1>");
    if !bridge_present(state) {
        body.push_str("<div class=\"err\">CoiBridge is not answering. Install the mod (Mods\\CoiBridge), load the save with it, and run <code>web_api_start</code> — this page then shows every storage and machine the companion can address, with controls.</div>");
        body.push_str(&machines_html);
        return Response::html(200, layout(session, "Entities", &body));
    }
    if let (Some(st), Some(rs)) = (
        bridge_json(state, "/coi/v1/status"),
        bridge_json(state, "/coi/v1/research"),
    ) {
        let cur = &rs["current"];
        body.push_str(&format!(
            "<div class=\"panel\">Population <b>{}</b> · housing {} free of {} · months of food <b>{}</b>{}<br>\
             Research: <b>{}</b> — {:.1}% ({} of {} steps), {} in queue · {} researched · lab {}</div>",
            st["population"],
            st["freeHousingCapacity"],
            st["totalHousingCapacity"],
            st["monthsOfFood"],
            if st["arePeopleStarving"].as_bool().unwrap_or(false) { " · <b>STARVING</b>" } else { "" },
            esc(&pretty_research(cur["id"].as_str().unwrap_or(""))),
            pct_raw(&cur["progressPerc"]),
            cur["stepsDone"],
            cur["stepsDone"].as_i64().unwrap_or(0) + cur["remainingSteps"].as_i64().unwrap_or(0),
            rs["queue"].as_array().map(|a| a.len()).unwrap_or(0),
            rs["researchedCount"],
            if rs["hasActiveLab"].as_bool().unwrap_or(false) { "active" } else { "idle" },
        ));
    }
    // Set a storage's product — one form for any storage id.
    {
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
            "<div class=\"panel\"><h2 style=\"margin-top:0\">Set a storage's product</h2>\
             <form method=\"post\" action=\"/actions/prepare-storage-product\">\
             <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
             <label>Storage id <input type=\"text\" name=\"storageId\" size=\"7\" required></label> \
             <label>Product <select name=\"productId\">{}</select></label> \
             <button type=\"submit\">Prepare…</button></form>\
             <p class=\"sub\">Goes through the same confirmation page, which warns you if the storage currently holds something else.</p></div>",
            esc(&session.csrf),
            opts,
        ));
    }
    // Train lines in detail (CoiBridge v0.6): ids, networks, schedules.
    if let Some(td) = bridge_json(state, "/coi/v1/trains-detail") {
        if let Some(lines) = td["lines"].as_array() {
            if !lines.is_empty() {
                body.push_str("<h2>Train lines (detail)</h2><table><thead><tr><th>Id</th><th>Line</th><th>Network</th><th>Trains</th><th>Stations</th><th>Schedule</th></tr></thead><tbody>");
                for l in lines {
                    let net = if l["network"].is_null() {
                        "—".to_string()
                    } else {
                        format!("{} (#{})", esc(&sv(&l["network"]["name"])), vi(&l["network"]["id"]))
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
                body.push_str("</tbody></table>");
            }
        }
    }
    // Storages.
    if let Some(storages) = bridge_json(state, "/coi/v1/storages") {
        let rows = storages["storages"].as_array().cloned().unwrap_or_default();
        body.push_str(&format!(
            "<h2>Storages ({})</h2><p class=\"sub\">Filter: <input type=\"text\" id=\"tfilter\" placeholder=\"product, proto, id…\" oninput=\"filterTable()\"></p>\
             <table id=\"ptable\"><thead><tr><th>Id</th><th>Storage</th><th>Product</th><th>Qty / cap</th><th>Full</th><th>Input</th><th>Output</th><th>Logistics</th></tr></thead><tbody>",
            rows.len()
        ));
        for r in &rows {
            body.push_str(&storage_row_html(session, r, &live));
        }
        body.push_str("</tbody></table>");
    }
    // Machines per instance (CoiBridge v0.6.4) — computed at the top
    // of the page so the section also renders on the degraded paths.
    body.push_str(&machines_html);
    // Machines: summary by proto, or one proto's instances.
    if let (Some(census), Some(topo)) = (
        bridge_json(state, "/coi/v1/census"),
        bridge_json(state, "/coi/v1/topology"),
    ) {
        // Non-default priorities (CoiBridge v0.6.1 census): the
        // "priorities" map is entityId → general priority, holding
        // ONLY entities whose value differs from their proto default.
        // Absent on v0.5-era censuses — the panel is then omitted.
        if let Some(prios) = census["priorities"].as_object() {
            if !prios.is_empty() {
                let mut entries: Vec<(&String, &Value)> = prios.iter().collect();
                entries.sort_by_key(|(k, _)| k.parse::<i64>().unwrap_or(0));
                body.push_str(&format!(
                    "<h2>Non-default priorities ({})</h2>\
                     <p class=\"sub\">Entities whose general priority differs from their prototype default (CoiBridge census).</p>\
                     <table><thead><tr><th>Entity</th><th>Type</th><th>Position</th><th>General priority</th></tr></thead><tbody>",
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
                body.push_str("</tbody></table>");
            }
        }
        let total = census["transports"]["total"].as_i64().unwrap_or(0);
        let conn = census["transports"]["fullyConnected"].as_i64().unwrap_or(0);
        body.push_str(&format!(
            "<h2>Machines & transports</h2><p class=\"sub\">{} entities total · transports {} of {} fully connected.</p>",
            census["totalEntities"],
            conn,
            total,
        ));
        let bad: Vec<&Value> = topo["transports"]
            .as_array()
            .map(|a| a.iter().filter(|t| !t["fullyConnected"].as_bool().unwrap_or(false)).collect())
            .unwrap_or_default();
        if !bad.is_empty() {
            body.push_str(&format!("<h3>Not fully connected ({})</h3><table><tr><th>Id</th><th>Type</th><th>From</th><th>To</th></tr>", bad.len()));
            for t in bad.iter().take(40) {
                let fp = |v: &Value| format!("{},{}", v[0].as_i64().unwrap_or(0), v[1].as_i64().unwrap_or(0));
                body.push_str(&format!(
                    "<tr><td>#{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    t["id"].as_i64().unwrap_or(0),
                    esc(&sv(&t["proto"])),
                    fp(&t["from"]),
                    fp(&t["to"]),
                ));
            }
            body.push_str("</table>");
        }
        if let Some(proto) = req.query.get("proto") {
            let instances: Vec<&Value> = topo["entities"]
                .as_array()
                .map(|a| a.iter().filter(|e| sv(&e["proto"]) == *proto).collect())
                .unwrap_or_default();
            let recipes: Vec<String> = census["machines"][proto]["recipes"]
                .as_array()
                .map(|a| a.iter().map(|v| sv(v)).collect())
                .unwrap_or_default();
            body.push_str(&format!(
                "<h3>{} — {} instance(s)</h3><table><tr><th>Id</th><th>Position</th><th>Pause / resume</th><th>Recipes (toggle)</th><th>General priority</th></tr>",
                esc(proto),
                instances.len()
            ));
            for e in instances.iter().take(60) {
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
                     <button type=\"submit\">Set</button></form></td></tr>",
                    eid,
                    esc(&pos),
                    prepare_form_labeled(
                        session,
                        "ToggleEnabledCmd",
                        json!({"entityId": eid}),
                        &format!("Pause / resume {proto} #{eid} (at {pos}) — toggles its enabled state."),
                        "⏯ Toggle",
                    ),
                    btns,
                    esc(&session.csrf),
                    eid,
                    esc(&format!("{proto} #{eid} (at {pos})")),
                ));
            }
            body.push_str("</table>");
            if instances.len() > 60 {
                body.push_str("<p class=\"sub\">Showing the first 60 instances.</p>");
            }
        } else if let Some(machines) = census["machines"].as_object() {
            let mut protos: Vec<&String> = machines.keys().collect();
            protos.sort();
            body.push_str("<table><tr><th>Machine</th><th>Instances</th><th>States</th><th></th></tr>");
            for proto in protos {
                let count = topo["entities"]
                    .as_array()
                    .map(|a| a.iter().filter(|e| sv(&e["proto"]) == *proto).count())
                    .unwrap_or(0);
                body.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td><a class=\"btn\" href=\"/entities?proto={}\">Open</a></td></tr>",
                    esc(proto),
                    count,
                    esc(&machines[proto]["states"].to_string()),
                    url_enc(proto),
                ));
            }
            body.push_str("</table>");
        }
    }
    body.push_str(TABLE_JS);
    Response::html(200, layout(session, "Entities", &body))
}

/// Tiny client-side table filter/sort (no framework, our own pages).
const TABLE_JS: &str = "<script>
function filterTable(){var q=document.getElementById('tfilter').value.toLowerCase();document.querySelectorAll('#ptable tbody tr').forEach(function(r){r.style.display=r.textContent.toLowerCase().indexOf(q)>=0?'':'none'})}
document.querySelectorAll('#ptable thead th').forEach(function(th,i){th.style.cursor='pointer';th.title='Click to sort';th.addEventListener('click',function(){var tb=document.querySelector('#ptable tbody');var rows=Array.prototype.slice.call(tb.rows);var dir=th.getAttribute('data-dir')==='asc'?'desc':'asc';th.setAttribute('data-dir',dir);rows.sort(function(a,b){var x=a.cells[i].textContent.trim(),y=b.cells[i].textContent.trim();var sx=x.trim().charAt(0)==='-'?-1:1,sy=y.trim().charAt(0)==='-'?-1:1;var nx=sx*parseFloat(x.replace(/[^0-9.]/g,'')),ny=sy*parseFloat(y.replace(/[^0-9.]/g,''));var c=(!isNaN(nx)&&!isNaN(ny)&&/[0-9]/.test(x)&&/[0-9]/.test(y))?nx-ny:x.localeCompare(y);return dir==='asc'?c:-c});rows.forEach(function(r){tb.appendChild(r)})})});
</script>";

// ---------------------------------------------------------------------------
// Snapshots
// ---------------------------------------------------------------------------

fn page_snapshots(state: &State, session: &Session, req: &Request) -> Response {
    let mut body = String::from("<h1>Snapshots</h1>");
    if let Some(msg) = req.query.get("msg") {
        body.push_str(&format!("<div class=\"ok\">{}</div>", esc(msg)));
    }
    body.push_str(&format!(
        "<div class=\"panel\"><p>Snapshots freeze the whole run — every product, the monthly \
         history, and the triage board — into the local SQLite store. Take one before and after \
         a big change and the trends below show what it did.</p>\
         <form method=\"post\" action=\"/snapshots/take\">\
         <input type=\"hidden\" name=\"csrf\" value=\"{}\">\
         <button type=\"submit\">Take a snapshot now</button></form></div>",
        esc(&session.csrf)
    ));
    match store(state) {
        Ok(st) => {
            match st.counts() {
                Ok((snaps, rows, issues)) => body.push_str(&format!(
                    "<p class=\"sub\">Store: {snaps} snapshot(s), {rows} product rows, {issues} stored issues.</p>"
                )),
                Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
            }
            if let Ok(list) = st.recent_snapshots(20) {
                if !list.is_empty() {
                    body.push_str("<table><tr><th>#</th><th>Game date</th><th>Tick</th><th>Taken</th><th>Products</th><th>Issues</th></tr>");
                    for (id, date, tick, taken, prods, issues) in list {
                        body.push_str(&format!(
                            "<tr><td>{id}</td><td>{}</td><td>{tick}</td><td>{}</td><td>{prods}</td><td>{issues}</td></tr>",
                            esc(&date),
                            fmt_clock(taken as u64)
                        ));
                    }
                    body.push_str("</table>");
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
                let mut tbl = String::from("<h2>Trends across snapshots</h2><table><tr><th>Product</th><th>Quantity series</th></tr>");
                for name in names {
                    if let Ok(rows) = st.trend(&name) {
                        let mine: Vec<_> = rows.iter().filter(|r| r.7 == name).collect();
                        if mine.len() >= 1 {
                            any = true;
                            let series: Vec<String> = mine.iter().map(|r| r.2.to_string()).collect();
                            tbl.push_str(&format!(
                                "<tr><td>{}</td><td class=\"spark\">{}</td></tr>",
                                esc(&name),
                                esc(&series.join(" → "))
                            ));
                        }
                    }
                }
                tbl.push_str("</table>");
                if any {
                    body.push_str(&tbl);
                }
            }
        }
        Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
    }
    Response::html(200, layout(session, "Snapshots", &body))
}

fn do_take_snapshot(state: &State, session: &Session, req: &Request) -> Response {
    if let Err(r) = check_csrf(session, req) {
        return r;
    }
    let live = match fetch_live(&client(state)) {
        Ok(l) => l,
        Err(e) => return game_unreachable(&e),
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
            if !matches!(result, Err(_)) {
                let (outcome, summary, snap_id) = match &result {
                    Ok(Some(id)) => (
                        "stored",
                        format!("Snapshot #{id} stored"),
                        Some(*id),
                    ),
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
                pct_encode(&format!("Snapshot #{id} stored ({} products).", snap["products"].as_array().map(|a| a.len()).unwrap_or(0)))
            )),
            Ok(None) => Response::redirect("/snapshots?msg=That game tick is already stored — nothing new to save."),
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
    let appdata = std::env::var("APPDATA").ok().filter(|s| !s.trim().is_empty())?;
    Some(format!("{appdata}\\Captain of Industry\\QoS\\events-v1.jsonl"))
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
                Ok(stats) => QosImportReport { path, found: true, stats: Some(stats), error: None },
                Err(e) => QosImportReport { path, found: true, stats: None, error: Some(e) },
            }
        }
        Err(_) => QosImportReport { path, found: false, stats: None, error: None },
    }
}

fn page_log(state: &State, session: &Session, req: &Request) -> Response {
    const PAGE_SIZE: i64 = 200;
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

    let mut body = String::from("<h1>Log — everything that was done, in order</h1>");
    body.push_str(
        "<p class=\"sub\">The unified activity record, stored in this companion's local \
         database (coi.db). <b>Companion</b> rows are written when a command is applied \
         through the Actions flow; <b>system</b> rows when a snapshot is taken here; \
         <b>QoS</b> rows are the mods' own record of what they did inside the game, \
         imported from their feed file each time this page loads. QoS events exist only \
         from the updated mods onward — mod actions from before the update lived only \
         in the game log and are <b>not</b> backfilled. Times are UTC, as everywhere \
         in this companion.</p>",
    );

    match store(state) {
        Ok(st) => {
            let report = import_qos_feed_now(state, &st);
            if report.found {
                match (&report.stats, &report.error) {
                    (Some(s), _) => body.push_str(&format!(
                        "<p class=\"sub\">QoS feed: {} — this load imported {} new, skipped {} already stored, {} malformed line(s).</p>",
                        esc(&report.path), s.imported, s.skipped, s.malformed
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
                    "<p class=\"sub\">QoS feed: no file at {} yet — nothing to import.</p>",
                    esc(&report.path)
                ));
            }

            // Filter form (plain GET, so filters live in the URL).
            let sel = |v: &str| if src_filter == v { " selected" } else { "" };
            body.push_str(&format!(
                "<form method=\"get\" action=\"/log\" style=\"margin:12px 0\">\
                 <label>Source <select name=\"source\">\
                 <option value=\"all\"{}>All sources</option>\
                 <option value=\"companion\"{}>Companion</option>\
                 <option value=\"qos\"{}>QoS mods</option>\
                 <option value=\"system\"{}>System</option></select></label> \
                 <label>Search <input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"action or summary text\"></label> \
                 <button type=\"submit\">Filter</button></form>",
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
                "<p class=\"sub\">{total} event(s) match. Showing {} (newest first).</p>",
                rows.len()
            ));
            if rows.is_empty() {
                body.push_str("<p>No events recorded yet.</p>");
            } else {
                body.push_str("<table><tr><th>Time (UTC)</th><th>Source</th><th>Actor / mod</th><th>Action</th><th>Summary</th><th>Outcome</th><th>Game</th><th>Payload</th></tr>");
                for r in &rows {
                    let badge = match r.source.as_str() {
                        "qos" => "<span class=\"badge\" style=\"background:#274a2a\">QoS</span>",
                        "system" => "<span class=\"badge\" style=\"background:#3a3f4a\">system</span>",
                        _ => "<span class=\"badge\" style=\"background:#2b3a5e\">companion</span>",
                    };
                    let game = match (&r.game_date, r.game_tick) {
                        (Some(d), Some(t)) => format!("{} · tick {}", esc(d), t),
                        (Some(d), None) => esc(d),
                        (None, Some(t)) => format!("tick {t}"),
                        (None, None) => "—".to_string(),
                    };
                    let payload = match &r.payload_json {
                        Some(p) => {
                            let pretty = serde_json::from_str::<Value>(p)
                                .map(|v| {
                                    serde_json::to_string_pretty(&v)
                                        .unwrap_or_else(|_| p.clone())
                                })
                                .unwrap_or_else(|_| p.clone());
                            format!("<details><summary>payload</summary><pre>{}</pre></details>", esc(&pretty))
                        }
                        None => "—".to_string(),
                    };
                    body.push_str(&format!(
                        "<tr><td style=\"white-space:nowrap\">{}</td><td>{}</td><td>{}</td>\
                         <td><b>{}</b>{}</td><td>{}</td><td>{}</td><td style=\"white-space:nowrap\">{}</td><td>{}</td></tr>",
                        esc(&fmt_clock(r.recorded_ts_unix as u64)),
                        badge,
                        esc(r.actor.as_deref().unwrap_or("—")),
                        esc(&r.action),
                        r.category
                            .as_deref()
                            .map(|c| format!("<br><span class=\"sub\">{}</span>", esc(c)))
                            .unwrap_or_default(),
                        esc(r.summary.as_deref().unwrap_or("")),
                        esc(r.outcome.as_deref().unwrap_or("")),
                        game,
                        payload,
                    ));
                }
                body.push_str("</table>");
            }

            // Pagination: simple older/newer links preserving filters.
            let qs = format!(
                "source={}&q={}",
                pct_encode(src_filter),
                pct_encode(search)
            );
            let mut pager = String::new();
            if page > 0 {
                pager.push_str(&format!(
                    "<a class=\"btn\" href=\"/log?{}&page={}\">← Newer</a> ",
                    qs,
                    page - 1
                ));
            }
            if total > (page + 1) * PAGE_SIZE {
                pager.push_str(&format!(
                    "<a class=\"btn\" href=\"/log?{}&page={}\">Older →</a>",
                    qs,
                    page + 1
                ));
            }
            if !pager.is_empty() {
                body.push_str(&format!("<p style=\"margin-top:14px\">{pager}</p>"));
            }
        }
        Err(e) => body.push_str(&format!("<div class=\"err\">{}</div>", esc(&e))),
    }
    Response::html(200, layout(session, "Log", &body))
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

fn page_research(state: &State, session: &Session) -> Response {
    let mut body = String::from("<h1>Research</h1>");
    body.push_str(
        "<p class=\"sub\">The game API does not report research state — what's finished or what's \
         currently running arrives with the CoiBridge census. Until then: commands are validated \
         against the game's own catalogue below (250 nodes), and the game accepts or rejects each \
         one. Starting a node that isn't available yet returns failure and changes nothing. \
         Queueing several nodes and letting the game work through them works the same way. \
         Every attempt is audit-logged.</p>",
    );
    body.push_str("<div class=\"panel\"><h2 style=\"margin-top:0\">Current research</h2><p>");
    body.push_str(&prepare_form(
        session,
        "ResearchStopCmd",
        json!({}),
        "⏹ Stop current research",
        false,
    ));
    body.push_str("</p><p class=\"sub\">Stops whatever is currently being researched (the game keeps the spent progress rules as usual).</p></div>");
    // Completed research (CoiBridge v0.6.2): the bridge research
    // route's "researched" array of completed node ids. Absent on
    // older mods (or when the game is off) — the page is unchanged.
    if let Some(rs) = bridge_json(state, "/coi/v1/research") {
        if let Some(done) = rs["researched"].as_array() {
            let count = if rs["researchedCount"].is_null() {
                done.len() as i64
            } else {
                vi(&rs["researchedCount"])
            };
            let names: Vec<String> = done
                .iter()
                .map(|v| esc(&pretty_research(&sv(v))))
                .collect();
            body.push_str(&format!(
                "<h2>Completed research ({count})</h2><div class=\"panel\">{}</div>",
                if names.is_empty() {
                    "— none yet —".to_string()
                } else {
                    names.join(" · ")
                },
            ));
        }
    }
    body.push_str("<h2>Catalogue</h2>");
    body.push_str(
        "<p><input type=\"text\" id=\"rq\" placeholder=\"Filter research…\" oninput=\"filterResearch()\"></p>\
         <script>function filterResearch(){var q=document.getElementById('rq').value.toLowerCase();\
         document.querySelectorAll('tr[data-r]').forEach(function(r){r.style.display=r.getAttribute('data-r').toLowerCase().includes(q)?'':'none';});}</script>",
    );
    body.push_str("<table><tr><th>Research</th><th>Node id</th><th>Actions</th></tr>");
    for id in research_ids() {
        let pretty = pretty_research(id);
        body.push_str(&format!(
            "<tr data-r=\"{} {}\"><td>{}</td><td class=\"sub\">{}</td><td>{} {} {}</td></tr>",
            esc(&pretty),
            esc(id),
            esc(&pretty),
            esc(id),
            prepare_form(session, "ResearchStartCmd", json!({"nodeId": id}), "Start", false),
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
    }
    body.push_str("</table>");
    Response::html(200, layout(session, "Research", &body))
}

/// The global-priority form posts id+value here; build the command args
/// and reuse the standard prepare flow (validation + confirm page).
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
        cookies: HashMap::new(),
    };
    do_prepare(state, session, &fake)
}
