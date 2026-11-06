//! Local storage for the companion: a single-file SQLite database
//! (rusqlite with bundled SQLite — no server, no system dependency).
//!
//! Why not MySQL: this is a single-user tool on one machine; a database
//! *server* would be pure operational overhead. SQLite gives real SQL
//! (trends, diffs, joins) in one file that can be copied, backed up, or
//! deleted. The JSON snapshot files remain the interchange format; the
//! DB is the queryable store on top.
//!
//! Schema v1:
//!   games(game_id, first_seen, last_seen)
//!   snapshots(id, game_id, session_id, tick, game_date, taken_unix,
//!             is_paused, speed)          UNIQUE(game_id, tick)
//!   product_stats(snapshot_id, product_id, name, kind, quantity,
//!                 stored, capacity, prod_tm, prod_lm, cons_tm, cons_lm,
//!                 prod_json, cons_json)
//!   product_history(snapshot_id, product_id, idx, produced, consumed,
//!                   quantity)            -- monthly series, oldest first
//!   issues(snapshot_id, quadrant, product_id, product, title,
//!          evidence, fix)
//!   audit(id, ts_unix, action, detail) -- Tier 2 write log (SPEC §12)
//!
//! Schema v2 (PRAGMA user_version = 2; QOL-MOD-PREP §5 prerequisite):
//!   machine_state_proto(snapshot_id, proto, working, not_enough_input,
//!                       output_full, paused, other, instances, source)
//!     -- per-snapshot machine-state aggregates per proto. `source` is
//!     -- 'instances' when derived from the bridge machines route
//!     -- (per-instance state/isPaused/isEnabled) or 'census' when
//!     -- derived from the census per-proto state counts.
//!   machine_state_instance(snapshot_id, entity_id, proto, state,
//!                          is_paused, is_enabled)
//!     -- per-entity rows where the bridge machines route provides them.
//!   productivity_state(game_id, target, kind, status, cause,
//!                      since_tick, value, evidence, updated_tick)
//!     -- hysteresis state for the §5 productivity estimators:
//!     -- status 'fired' | 'cleared', since_tick = tick the current
//!     -- status began (for 'cleared', the tick of the fire it cleared,
//!     -- so the re-fire cooldown can still be measured from the fire).
//! v1 databases upgrade in place; snapshots stored before v2 simply
//! have no machine-state rows, and the productivity layer reports
//! INSUFFICIENT evidence for them rather than guessing.
//!
//! Schema v3 (PRAGMA user_version = 3; SPEC Addendum XVI — the
//! unified Log):
//!   activity_events(id, event_key UNIQUE, recorded_ts_unix, source,
//!                   actor, category, action, summary, game_id,
//!                   game_date, game_tick, outcome, payload_json)
//!     -- the one chronological activity record behind the website's
//!     -- Log page. Sources: 'companion' (§12 applies recorded by
//!     -- do_apply), 'system' (companion housekeeping worth replaying,
//!     -- currently website snapshot creation), 'qos' (QoS mod
//!     -- actions imported from the mods' JSONL feed). Pre-v3 `audit`
//!     -- rows are backfilled once at migration with event_key
//!     -- 'legacy-audit-<id>' and the original detail text carried in
//!     -- the payload; their game context was never stored and stays
//!     -- NULL — it is not reconstructed. The legacy `audit` table is
//!     -- kept and still written by do_apply (Actions page unchanged).
//!
//! Schema v4 (PRAGMA user_version = 4; terrain sensing, CoiBridge
//! v0.6.5, recon round 13):
//!   deposit_state(snapshot_id, material, product, tiles,
//!                 volume_raw, quantity_raw)
//!     -- per-snapshot ON-MAP resource totals derived from the
//!     -- snapshot's bridgeDeposits payload (the /coi/v1/deposits
//!     -- materials list): one row per terrain material with its
//!     -- mined product, tile count, summed layer thickness
//!     -- (volume_raw, Fix32 tiles-cubed) and remaining quantity
//!     -- (quantity_raw, Fix32 product units — the game's own
//!     -- mined-quantity conversion, depletion-exact). Snapshots
//!     -- taken without the 0.6.5 bridge have no rows; absence is
//!     -- data, never backfilled. Terrain HEIGHTS are not stored
//!     -- per snapshot (bulky, and re-fetchable on demand); only
//!     -- the deposit totals are queryable history.

use rusqlite::{params, Connection};
use serde_json::Value;

pub struct Store {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS games (
    game_id TEXT PRIMARY KEY,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS snapshots (
    id INTEGER PRIMARY KEY,
    game_id TEXT NOT NULL,
    session_id TEXT,
    tick INTEGER NOT NULL,
    game_date TEXT,
    taken_unix INTEGER NOT NULL,
    is_paused INTEGER,
    speed INTEGER,
    UNIQUE(game_id, tick)
);
CREATE TABLE IF NOT EXISTS product_stats (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    product_id TEXT NOT NULL,
    name TEXT,
    kind TEXT,
    quantity INTEGER,
    stored INTEGER,
    capacity INTEGER,
    prod_tm INTEGER, prod_lm INTEGER,
    cons_tm INTEGER, cons_lm INTEGER,
    prod_json TEXT, cons_json TEXT,
    PRIMARY KEY (snapshot_id, product_id)
);
CREATE TABLE IF NOT EXISTS product_history (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    product_id TEXT NOT NULL,
    idx INTEGER NOT NULL,
    produced INTEGER, consumed INTEGER, quantity INTEGER,
    PRIMARY KEY (snapshot_id, product_id, idx)
);
CREATE TABLE IF NOT EXISTS issues (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    quadrant TEXT, product_id TEXT, product TEXT,
    title TEXT, evidence TEXT, fix TEXT
);
CREATE TABLE IF NOT EXISTS audit (
    id INTEGER PRIMARY KEY,
    ts_unix INTEGER NOT NULL,
    action TEXT NOT NULL,
    detail TEXT
);
CREATE TABLE IF NOT EXISTS web_users (
    username TEXT PRIMARY KEY,
    salt TEXT NOT NULL,
    pass_hash TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_stats_product ON product_stats(product_id);
CREATE INDEX IF NOT EXISTS idx_snap_game ON snapshots(game_id, tick);
";

/// Schema v2 additions (see module docs). Applied by `Store::open`
/// when `PRAGMA user_version` < 2; every statement is IF NOT EXISTS so
/// the migration is idempotent and safe on partially-migrated files.
const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS machine_state_proto (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    proto TEXT NOT NULL,
    working INTEGER NOT NULL,
    not_enough_input INTEGER NOT NULL,
    output_full INTEGER NOT NULL,
    paused INTEGER NOT NULL,
    other INTEGER NOT NULL,
    instances INTEGER NOT NULL,
    source TEXT NOT NULL,
    PRIMARY KEY (snapshot_id, proto)
);
CREATE TABLE IF NOT EXISTS machine_state_instance (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    entity_id TEXT NOT NULL,
    proto TEXT NOT NULL,
    state TEXT,
    is_paused INTEGER,
    is_enabled INTEGER,
    PRIMARY KEY (snapshot_id, entity_id)
);
CREATE TABLE IF NOT EXISTS productivity_state (
    game_id TEXT NOT NULL,
    target TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    cause TEXT,
    since_tick INTEGER NOT NULL,
    value REAL,
    evidence TEXT,
    updated_tick INTEGER NOT NULL,
    PRIMARY KEY (game_id, target)
);
CREATE INDEX IF NOT EXISTS idx_msp_snap ON machine_state_proto(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_msi_snap ON machine_state_instance(snapshot_id);
";

/// Schema v3 additions (see module docs). Applied by `Store::open`
/// when `PRAGMA user_version` < 3; IF NOT EXISTS like the earlier
/// migrations. The legacy-audit backfill runs in the same step (in
/// Rust, so the payload JSON is built with serde_json, not SQL
/// string surgery) and is idempotent via the UNIQUE event_key.
const SCHEMA_V3: &str = "
CREATE TABLE IF NOT EXISTS activity_events (
    id INTEGER PRIMARY KEY,
    event_key TEXT NOT NULL UNIQUE,
    recorded_ts_unix INTEGER NOT NULL,
    source TEXT NOT NULL,
    actor TEXT,
    category TEXT,
    action TEXT NOT NULL,
    summary TEXT,
    game_id TEXT,
    game_date TEXT,
    game_tick INTEGER,
    outcome TEXT,
    payload_json TEXT
);
CREATE INDEX IF NOT EXISTS idx_activity_ts ON activity_events(recorded_ts_unix DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_source ON activity_events(source);
";

/// Schema v4 additions (see module docs). Applied by `Store::open`
/// when `PRAGMA user_version` < 4; IF NOT EXISTS like the earlier
/// migrations.
const SCHEMA_V4: &str = "
CREATE TABLE IF NOT EXISTS deposit_state (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    material TEXT NOT NULL,
    product TEXT,
    tiles INTEGER NOT NULL,
    volume_raw INTEGER NOT NULL,
    quantity_raw INTEGER NOT NULL,
    PRIMARY KEY (snapshot_id, material)
);
CREATE INDEX IF NOT EXISTS idx_deposit_snap ON deposit_state(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_deposit_material ON deposit_state(material);
";

impl Store {
    pub fn open(path: &str) -> Result<Store, String> {
        let conn = Connection::open(path).map_err(|e| format!("open db {path}: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("db pragmas: {e}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("db schema: {e}"))?;
        // Schema versioning: v1 files carry user_version 0. Apply the
        // v2 additions and stamp the version; old snapshots stay
        // readable (they simply have no machine-state rows).
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| format!("db user_version: {e}"))?;
        if version < 2 {
            conn.execute_batch(SCHEMA_V2)
                .map_err(|e| format!("db schema v2: {e}"))?;
            conn.execute_batch("PRAGMA user_version = 2")
                .map_err(|e| format!("db user_version set: {e}"))?;
        }
        if version < 3 {
            conn.execute_batch(SCHEMA_V3)
                .map_err(|e| format!("db schema v3: {e}"))?;
            backfill_legacy_audit(&conn)?;
            conn.execute_batch("PRAGMA user_version = 3")
                .map_err(|e| format!("db user_version set: {e}"))?;
        }
        if version < 4 {
            conn.execute_batch(SCHEMA_V4)
                .map_err(|e| format!("db schema v4: {e}"))?;
            conn.execute_batch("PRAGMA user_version = 4")
                .map_err(|e| format!("db user_version set: {e}"))?;
        }
        Ok(Store { conn })
    }

    /// Insert one `coi-snapshot/1` JSON value plus its triage issues.
    /// Returns Ok(Some(id)) for a new snapshot, Ok(None) when this
    /// (game_id, tick) is already stored.
    pub fn insert_snapshot(
        &mut self,
        snap: &Value,
        issues: &[crate::Issue],
    ) -> Result<Option<i64>, String> {
        let game = &snap["game"];
        let game_id = game["gameId"].as_str().unwrap_or("");
        let tick = game["tick"].as_i64().unwrap_or(0);
        let taken = snap["takenAtUnix"].as_i64().unwrap_or(0);
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("db tx: {e}"))?;
        tx.execute(
            "INSERT INTO games(game_id, first_seen, last_seen) VALUES (?1, ?2, ?2)
             ON CONFLICT(game_id) DO UPDATE SET last_seen = MAX(last_seen, ?2)",
            params![game_id, taken],
        )
        .map_err(|e| format!("db games: {e}"))?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO snapshots(game_id, session_id, tick, game_date, taken_unix, is_paused, speed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    game_id,
                    game["sessionId"].as_str(),
                    tick,
                    game["date"].as_str(),
                    taken,
                    game["isPaused"].as_bool().map(|b| b as i64),
                    game["speed"].as_i64(),
                ],
            )
            .map_err(|e| format!("db snapshot: {e}"))?;
        if inserted == 0 {
            return Ok(None);
        }
        let sid = tx.last_insert_rowid();

        if let Some(products) = snap["products"].as_array() {
            for p in products {
                tx.execute(
                    "INSERT INTO product_stats(snapshot_id, product_id, name, kind, quantity, stored, capacity,
                                                prod_tm, prod_lm, cons_tm, cons_lm, prod_json, cons_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        sid,
                        p["id"].as_str(),
                        p["name"].as_str(),
                        p["kind"].as_str(),
                        p["quantity"].as_i64(),
                        p["stored"].as_i64(),
                        p["storageCapacity"].as_i64(),
                        flow(p, "produced", 0),
                        flow(p, "produced", 1),
                        flow(p, "consumed", 0),
                        flow(p, "consumed", 1),
                        p["produced"].to_string(),
                        p["consumed"].to_string(),
                    ],
                )
                .map_err(|e| format!("db product_stats: {e}"))?;
            }
        }
        if let Some(map) = snap["history"]["products"].as_object() {
            for (pid, series) in map {
                let n = series["quantity"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0);
                for idx in 0..n {
                    tx.execute(
                        "INSERT INTO product_history(snapshot_id, product_id, idx, produced, consumed, quantity)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            sid,
                            pid,
                            idx as i64,
                            series["produced"][idx].as_i64(),
                            series["consumed"][idx].as_i64(),
                            series["quantity"][idx].as_i64(),
                        ],
                    )
                    .map_err(|e| format!("db product_history: {e}"))?;
                }
            }
        }
        // Schema v2: machine-state aggregates + per-instance rows
        // from the snapshot's bridge payloads (absent when the bridge
        // was not reachable at capture time — no rows, and the
        // productivity layer will say INSUFFICIENT for this snapshot).
        for (proto, working, nei, of, paused, other, instances, source) in snapshot_machine_rows(snap) {
            tx.execute(
                "INSERT INTO machine_state_proto(snapshot_id, proto, working, not_enough_input, output_full, paused, other, instances, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![sid, proto, working, nei, of, paused, other, instances, source],
            )
            .map_err(|e| format!("db machine_state_proto: {e}"))?;
        }
        for (entity_id, proto, state, is_paused, is_enabled) in snapshot_machine_instances(snap) {
            tx.execute(
                "INSERT INTO machine_state_instance(snapshot_id, entity_id, proto, state, is_paused, is_enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    sid,
                    entity_id,
                    proto,
                    state,
                    is_paused.map(|b| b as i64),
                    is_enabled.map(|b| b as i64)
                ],
            )
            .map_err(|e| format!("db machine_state_instance: {e}"))?;
        }
        // Schema v4: per-material deposit totals from the
        // snapshot's bridgeDeposits payload (absent pre-0.6.5 —
        // no rows, like the machine-state tables).
        for (material, product, tiles, volume_raw, quantity_raw) in snapshot_deposit_rows(snap) {
            tx.execute(
                "INSERT INTO deposit_state(snapshot_id, material, product, tiles, volume_raw, quantity_raw)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![sid, material, product, tiles, volume_raw, quantity_raw],
            )
            .map_err(|e| format!("db deposit_state: {e}"))?;
        }
        for issue in issues {
            tx.execute(
                "INSERT INTO issues(snapshot_id, quadrant, product_id, product, title, evidence, fix)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    sid,
                    issue.quadrant,
                    None::<String>,
                    issue.product,
                    issue.title,
                    issue.evidence,
                    issue.fix
                ],
            )
            .map_err(|e| format!("db issues: {e}"))?;
        }
        tx.commit().map_err(|e| format!("db commit: {e}"))?;
        Ok(Some(sid))
    }

    pub fn counts(&self) -> Result<(i64, i64, i64), String> {        let q = |sql: &str| self.conn.query_row(sql, [], |r| r.get::<_, i64>(0));
        Ok((
            q("SELECT COUNT(*) FROM snapshots").map_err(|e| e.to_string())?,
            q("SELECT COUNT(*) FROM product_stats").map_err(|e| e.to_string())?,
            q("SELECT COUNT(*) FROM issues").map_err(|e| e.to_string())?,
        ))
    }

    /// (game_date, tick, quantity, prod_tm, prod_lm, cons_tm, cons_lm)
    /// for products whose name contains `filter`, oldest first.
    pub fn trend(&self, filter: &str) -> Result<Vec<(String, i64, i64, i64, i64, i64, i64, String)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.game_date, s.tick, ps.quantity, ps.prod_tm, ps.prod_lm, ps.cons_tm, ps.cons_lm, ps.name
                 FROM product_stats ps JOIN snapshots s ON s.id = ps.snapshot_id
                 WHERE ps.name LIKE ?1
                 ORDER BY s.id",
            )
            .map_err(|e| format!("db trend prepare: {e}"))?;
        let rows = stmt
            .query_map(params![format!("%{filter}%")], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                ))
            })
            .map_err(|e| format!("db trend: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db trend row: {e}"))?);
        }
        Ok(out)
    }
}

fn flow(p: &Value, key: &str, idx: usize) -> Option<i64> {
    p[key].as_array().and_then(|a| a.get(idx)).and_then(|v| v.as_i64())
}

// ---------------------------------------------------------------------------
// Schema v3: the unified activity log (SPEC Addendum XVI)
// ---------------------------------------------------------------------------

/// One row of `activity_events`, as written. `event_key` is the
/// idempotency key (UNIQUE): companion-generated keys are
/// `<kind>-<unix>-<random hex>`, imported QoS keys are
/// `qos:<eventId>`, backfilled legacy rows are `legacy-audit-<id>`.
pub struct ActivityEvent {
    pub event_key: String,
    pub recorded_ts_unix: i64,
    pub source: String,
    pub actor: Option<String>,
    pub category: Option<String>,
    pub action: String,
    pub summary: Option<String>,
    pub game_id: Option<String>,
    pub game_date: Option<String>,
    pub game_tick: Option<i64>,
    pub outcome: Option<String>,
    pub payload_json: Option<String>,
}

/// One row of `activity_events`, as read for the Log page.
pub struct ActivityRow {
    pub recorded_ts_unix: i64,
    pub source: String,
    pub actor: Option<String>,
    pub category: Option<String>,
    pub action: String,
    pub summary: Option<String>,
    pub game_date: Option<String>,
    pub game_tick: Option<i64>,
    pub outcome: Option<String>,
    pub payload_json: Option<String>,
}

/// Outcome of one QoS feed import pass.
#[derive(Default, Debug, PartialEq)]
pub struct QosImportStats {
    /// New rows inserted.
    pub imported: usize,
    /// Valid lines whose eventId was already stored.
    pub skipped: usize,
    /// Lines that were not usable events (bad JSON, missing fields,
    /// oversized).
    pub malformed: usize,
}

/// v2 → v3 migration step: carry every pre-existing `audit` row into
/// `activity_events` exactly once. The original detail text is
/// preserved verbatim inside the payload; game context was never
/// recorded for those rows and is left NULL. Idempotent: the
/// `legacy-audit-<id>` keys collide on re-run and are ignored.
fn backfill_legacy_audit(conn: &Connection) -> Result<(), String> {
    let rows: Vec<(i64, i64, String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, ts_unix, action, COALESCE(detail, '') FROM audit ORDER BY id")
            .map_err(|e| format!("db backfill prepare: {e}"))?;
        let mapped = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| format!("db backfill: {e}"))?;
        let mut out = Vec::new();
        for r in mapped {
            out.push(r.map_err(|e| format!("db backfill row: {e}"))?);
        }
        out
    };
    for (id, ts, action, detail) in rows {
        let payload = serde_json::json!({
            "detail": detail,
            "legacyAuditId": id,
        })
        .to_string();
        conn.execute(
            "INSERT OR IGNORE INTO activity_events(event_key, recorded_ts_unix, source, actor,
                                                    category, action, summary, game_id, game_date,
                                                    game_tick, outcome, payload_json)
             VALUES (?1, ?2, 'companion', NULL, 'command', ?3, ?4, NULL, NULL, NULL, NULL, ?5)",
            params![format!("legacy-audit-{id}"), ts, action, detail, payload],
        )
        .map_err(|e| format!("db backfill insert: {e}"))?;
    }
    Ok(())
}

/// Parse an ISO-8601 UTC timestamp ("2026-11-05T14:03:22.1234567Z"
/// or with an explicit +00:00 offset) into unix seconds. Returns
/// None for anything else — the caller treats the line as malformed
/// rather than guessing a time for it.
fn parse_iso8601_utc(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let num = |from: usize, len: usize| -> Option<i64> {
        s.get(from..from + len)?.parse::<i64>().ok()
    };
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-')
        || (bytes.get(10) != Some(&b'T') && bytes.get(10) != Some(&b' '))
        || bytes.get(13) != Some(&b':') || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, min, sec) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day)
        || hour > 23 || min > 59 || sec > 60
    {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm, inverse of the
    // one in serve::fmt_clock).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

impl Store {
    /// Insert one activity event. Returns true when the row was
    /// new, false when its event_key was already stored (imports
    /// and retries are idempotent by construction).
    pub fn activity_add(&self, ev: &ActivityEvent) -> Result<bool, String> {
        let n = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO activity_events(event_key, recorded_ts_unix, source, actor,
                                                        category, action, summary, game_id, game_date,
                                                        game_tick, outcome, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    ev.event_key,
                    ev.recorded_ts_unix,
                    ev.source,
                    ev.actor,
                    ev.category,
                    ev.action,
                    ev.summary,
                    ev.game_id,
                    ev.game_date,
                    ev.game_tick,
                    ev.outcome,
                    ev.payload_json,
                ],
            )
            .map_err(|e| format!("db activity: {e}"))?;
        Ok(n > 0)
    }

    /// Import one QoS JSONL feed's full text into `activity_events`.
    /// Line schema v1 is written by QoSCore's QoSEventLog: each line
    /// is one JSON object with eventId / recordedAtUtc / modId /
    /// modVersion / kind / action / summary / data. The mod's
    /// gameDate rides inside `data` (the feed has no game tick —
    /// ICalendar exposes a date, not a tick) and is promoted to the
    /// game_date column here. Bad lines are counted, never fatal;
    /// blank lines are ignored silently.
    pub fn import_qos_feed(&self, text: &str) -> Result<QosImportStats, String> {
        const MAX_LINE: usize = 64 * 1024;
        let mut stats = QosImportStats::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.len() > MAX_LINE {
                stats.malformed += 1;
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => {
                    stats.malformed += 1;
                    continue;
                }
            };
            let event_id = v["eventId"].as_str().unwrap_or("");
            let action = v["action"].as_str().unwrap_or("");
            let ts = v["recordedAtUtc"]
                .as_str()
                .and_then(parse_iso8601_utc);
            if event_id.is_empty() || action.is_empty() || ts.is_none() {
                stats.malformed += 1;
                continue;
            }
            let data = if v["data"].is_object() {
                v["data"].clone()
            } else {
                Value::Null
            };
            let payload = serde_json::json!({
                "modId": v["modId"],
                "modVersion": v["modVersion"],
                "kind": v["kind"],
                "recordedAtUtc": v["recordedAtUtc"],
                "data": data,
            })
            .to_string();
            let ev = ActivityEvent {
                event_key: format!("qos:{event_id}"),
                recorded_ts_unix: ts.unwrap(),
                source: "qos".to_string(),
                actor: v["modId"].as_str().map(|s| s.to_string()),
                category: v["kind"].as_str().map(|s| s.to_string()),
                action: action.to_string(),
                summary: v["summary"].as_str().map(|s| s.to_string()),
                game_id: None,
                game_date: data["gameDate"].as_str().map(|s| s.to_string()),
                game_tick: None,
                outcome: Some("recorded".to_string()),
                payload_json: Some(payload),
            };
            if self.activity_add(&ev)? {
                stats.imported += 1;
            } else {
                stats.skipped += 1;
            }
        }
        Ok(stats)
    }

    /// Count activity rows matching the Log page's filters.
    pub fn activity_count(&self, source: Option<&str>, search: Option<&str>) -> Result<i64, String> {
        let (where_sql, like) = activity_filter(source, search);
        let sql = format!("SELECT COUNT(*) FROM activity_events{where_sql}");
        self.conn
            .query_row(&sql, params![source, like], |r| r.get(0))
            .map_err(|e| format!("db activity count: {e}"))
    }

    /// Activity rows matching the Log page's filters, newest first.
    pub fn activity_recent(
        &self,
        source: Option<&str>,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ActivityRow>, String> {
        let (where_sql, like) = activity_filter(source, search);
        let sql = format!(
            "SELECT recorded_ts_unix, source, actor, category, action, summary,
                    game_date, game_tick, outcome, payload_json
             FROM activity_events{where_sql}
             ORDER BY recorded_ts_unix DESC, id DESC LIMIT ?3 OFFSET ?4"
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| format!("db activity prepare: {e}"))?;
        let rows = stmt
            .query_map(params![source, like, limit, offset], |r| {
                Ok(ActivityRow {
                    recorded_ts_unix: r.get(0)?,
                    source: r.get(1)?,
                    actor: r.get(2)?,
                    category: r.get(3)?,
                    action: r.get(4)?,
                    summary: r.get(5)?,
                    game_date: r.get(6)?,
                    game_tick: r.get(7)?,
                    outcome: r.get(8)?,
                    payload_json: r.get(9)?,
                })
            })
            .map_err(|e| format!("db activity list: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db activity row: {e}"))?);
        }
        Ok(out)
    }
}

/// Shared WHERE clause for the Log page queries. Positional params:
/// ?1 = source (NULL = any), ?2 = LIKE pattern for the free-text
/// search over action/summary. Both params are always referenced
/// and always bound — with no search the pattern is a bare "%"
/// (match all) — so count and list queries share one shape.
fn activity_filter(_source: Option<&str>, search: Option<&str>) -> (String, String) {
    let like = match search.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(s) => format!(
            "%{}%",
            s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        ),
        None => "%".to_string(),
    };
    let sql = " WHERE (?1 IS NULL OR source = ?1) \
               AND (action LIKE ?2 ESCAPE '\\' OR COALESCE(summary, '') LIKE ?2 ESCAPE '\\')"
        .to_string();
    (sql, like)
}

/// Per-material deposit totals parsed from a snapshot's
/// bridgeDeposits payload: (material, product, tiles, volume_raw,
/// quantity_raw) — raw values are Fix32 (65536 = 1), volume in
/// tiles-cubed, quantity in product units (the game's own
/// conversion, summed by the bridge). Materials with no mined
/// product or a malformed row are skipped; a Null payload (bridge
/// absent or pre-0.6.5) yields no rows.
pub fn snapshot_deposit_rows(snap: &Value) -> Vec<(String, String, i64, i64, i64)> {
    let mut out = Vec::new();
    if let Some(materials) = snap["bridgeDeposits"]["materials"].as_array() {
        for m in materials {
            let Some(material) = m["material"].as_str() else {
                continue;
            };
            out.push((
                material.to_string(),
                m["product"].as_str().unwrap_or("").to_string(),
                m["tiles"].as_i64().unwrap_or(0),
                m["volumeTilesCubedRaw"].as_i64().unwrap_or(0),
                m["quantityRaw"].as_i64().unwrap_or(0),
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Schema v2: machine-state parsing + productivity support
// ---------------------------------------------------------------------------

/// Per-proto machine-state aggregates parsed from a snapshot's bridge
/// payloads: (proto, working, not_enough_input, output_full, paused,
/// other, instances, source). The bridge machines route (per-instance
/// state / isPaused / isEnabled) takes precedence over the census
/// per-proto state counts for the same proto. Unknown state names are
/// summed into `other` — the in-game state vocabulary is not closed
/// (QOL recon §5.2.5), so nothing is ever silently dropped or folded
/// into the working/eligible buckets.
pub fn snapshot_machine_rows(
    snap: &Value,
) -> Vec<(String, i64, i64, i64, i64, i64, i64, String)> {
    use std::collections::BTreeMap;
    // (working, nei, output_full, paused, other, instances, source)
    let mut by_proto: BTreeMap<String, (i64, i64, i64, i64, i64, i64, String)> = BTreeMap::new();

    if let Some(machines) = snap["bridgeCensus"]["machines"].as_object() {
        for (proto, entry) in machines {
            let states = &entry["states"];
            let mut total = 0i64;
            if let Some(obj) = states.as_object() {
                for v in obj.values() {
                    total += v.as_i64().unwrap_or(0);
                }
            }
            let working = states["Working"].as_i64().unwrap_or(0);
            let nei = states["NotEnoughInput"].as_i64().unwrap_or(0);
            let of = states["OutputFull"].as_i64().unwrap_or(0);
            let paused = states["Paused"].as_i64().unwrap_or(0);
            let other = (total - working - nei - of - paused).max(0);
            by_proto.insert(
                proto.clone(),
                (working, nei, of, paused, other, total, "census".to_string()),
            );
        }
    }

    if let Some(list) = snap["bridgeMachines"]["machines"].as_array() {
        let mut agg: BTreeMap<String, (i64, i64, i64, i64, i64, i64)> = BTreeMap::new();
        for m in list {
            let proto = match m["proto"].as_str() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let e = agg.entry(proto).or_insert((0, 0, 0, 0, 0, 0));
            e.5 += 1; // instances
            let state = m["state"].as_str().unwrap_or("");
            let paused_like = m["isPaused"].as_bool() == Some(true)
                || m["isEnabled"].as_bool() == Some(false)
                || state == "Paused";
            if paused_like {
                e.3 += 1;
            } else {
                match state {
                    "Working" => e.0 += 1,
                    "NotEnoughInput" => e.1 += 1,
                    "OutputFull" => e.2 += 1,
                    _ => e.4 += 1,
                }
            }
        }
        for (proto, (w, n, o, p, ot, inst)) in agg {
            by_proto.insert(proto, (w, n, o, p, ot, inst, "instances".to_string()));
        }
    }

    by_proto
        .into_iter()
        .map(|(proto, (w, n, o, p, ot, inst, src))| (proto, w, n, o, p, ot, inst, src))
        .collect()
}

/// Per-instance machine rows from the bridge machines route:
/// (entity_id, proto, state, is_paused, is_enabled). Empty when the
/// snapshot carries no machines-route payload.
pub fn snapshot_machine_instances(
    snap: &Value,
) -> Vec<(String, String, Option<String>, Option<bool>, Option<bool>)> {
    let mut out = Vec::new();
    if let Some(list) = snap["bridgeMachines"]["machines"].as_array() {
        for m in list {
            let proto = match m["proto"].as_str() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let entity_id = match &m["id"] {
                Value::String(s) => s.clone(),
                Value::Null => continue,
                v => v.to_string(),
            };
            out.push((
                entity_id,
                proto,
                m["state"].as_str().map(|s| s.to_string()),
                m["isPaused"].as_bool(),
                m["isEnabled"].as_bool(),
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Web app support: single user, audit log, snapshot listing
// ---------------------------------------------------------------------------

impl Store {
    /// The single web user, if one has been created:
    /// (username, salt_hex, pass_hash_hex).
    pub fn web_user(&self) -> Result<Option<(String, String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT username, salt, pass_hash FROM web_users LIMIT 1")
            .map_err(|e| format!("db web_user prepare: {e}"))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| format!("db web_user: {e}"))?;
        match rows.next().map_err(|e| format!("db web_user row: {e}"))? {
            Some(r) => Ok(Some((
                r.get(0).map_err(|e| e.to_string())?,
                r.get(1).map_err(|e| e.to_string())?,
                r.get(2).map_err(|e| e.to_string())?,
            ))),
            None => Ok(None),
        }
    }

    pub fn create_web_user(&self, username: &str, salt_hex: &str, pass_hash_hex: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO web_users(username, salt, pass_hash) VALUES (?1, ?2, ?3)",
                params![username, salt_hex, pass_hash_hex],
            )
            .map_err(|e| format!("db create user: {e}"))?;
        Ok(())
    }

    pub fn audit_add(&self, ts_unix: i64, action: &str, detail: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO audit(ts_unix, action, detail) VALUES (?1, ?2, ?3)",
                params![ts_unix, action, detail],
            )
            .map_err(|e| format!("db audit: {e}"))?;
        Ok(())
    }

    /// (ts_unix, action, detail), newest first.
    pub fn audit_recent(&self, limit: i64) -> Result<Vec<(i64, String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT ts_unix, action, detail FROM audit ORDER BY id DESC LIMIT ?1")
            .map_err(|e| format!("db audit prepare: {e}"))?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .map_err(|e| format!("db audit list: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db audit row: {e}"))?);
        }
        Ok(out)
    }

    /// The game with the most recent activity (productivity reports
    /// evaluate one game at a time; the companion is single-game in
    /// practice, but the store does not assume it).
    pub fn latest_game_id(&self) -> Result<Option<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT game_id FROM games ORDER BY last_seen DESC LIMIT 1")
            .map_err(|e| format!("db latest game prepare: {e}"))?;
        let mut rows = stmt.query([]).map_err(|e| format!("db latest game: {e}"))?;
        match rows.next().map_err(|e| format!("db latest game row: {e}"))? {
            Some(r) => Ok(Some(r.get(0).map_err(|e| e.to_string())?)),
            None => Ok(None),
        }
    }

    /// All stored snapshot ticks for a game, ascending.
    pub fn snapshot_ticks(&self, game_id: &str) -> Result<Vec<i64>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT tick FROM snapshots WHERE game_id = ?1 ORDER BY tick")
            .map_err(|e| format!("db ticks prepare: {e}"))?;
        let rows = stmt
            .query_map(params![game_id], |r| r.get::<_, i64>(0))
            .map_err(|e| format!("db ticks: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db ticks row: {e}"))?);
        }
        Ok(out)
    }

    /// (tick, date) of the newest snapshot for a game.
    pub fn latest_snapshot_info(&self, game_id: &str) -> Result<Option<(i64, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT tick, game_date FROM snapshots WHERE game_id = ?1 ORDER BY tick DESC LIMIT 1")
            .map_err(|e| format!("db latest snap prepare: {e}"))?;
        let mut rows = stmt
            .query(params![game_id])
            .map_err(|e| format!("db latest snap: {e}"))?;
        match rows.next().map_err(|e| format!("db latest snap row: {e}"))? {
            Some(r) => Ok(Some((
                r.get(0).map_err(|e| e.to_string())?,
                r.get::<_, Option<String>>(1)
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default(),
            ))),
            None => Ok(None),
        }
    }

    /// Per-snapshot machine-state aggregates for a game, oldest first:
    /// (tick, proto, working, not_enough_input, output_full, paused,
    /// other, instances, source).
    pub fn machine_state_series(
        &self,
        game_id: &str,
    ) -> Result<Vec<(i64, String, i64, i64, i64, i64, i64, i64, String)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.tick, m.proto, m.working, m.not_enough_input, m.output_full,
                        m.paused, m.other, m.instances, m.source
                 FROM machine_state_proto m JOIN snapshots s ON s.id = m.snapshot_id
                 WHERE s.game_id = ?1
                 ORDER BY s.tick, m.proto",
            )
            .map_err(|e| format!("db machine series prepare: {e}"))?;
        let rows = stmt
            .query_map(params![game_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, String>(8)?,
                ))
            })
            .map_err(|e| format!("db machine series: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db machine series row: {e}"))?);
        }
        Ok(out)
    }

    /// Hysteresis state for one productivity target:
    /// (status, cause, since_tick, value).
    pub fn prod_state_get(
        &self,
        game_id: &str,
        target: &str,
    ) -> Result<Option<(String, Option<String>, i64, Option<f64>)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT status, cause, since_tick, value FROM productivity_state
                 WHERE game_id = ?1 AND target = ?2",
            )
            .map_err(|e| format!("db prod state prepare: {e}"))?;
        let mut rows = stmt
            .query(params![game_id, target])
            .map_err(|e| format!("db prod state: {e}"))?;
        match rows.next().map_err(|e| format!("db prod state row: {e}"))? {
            Some(r) => Ok(Some((
                r.get(0).map_err(|e| e.to_string())?,
                r.get(1).map_err(|e| e.to_string())?,
                r.get(2).map_err(|e| e.to_string())?,
                r.get(3).map_err(|e| e.to_string())?,
            ))),
            None => Ok(None),
        }
    }

    /// All hysteresis rows for a game: (target, kind, status, cause,
    /// since_tick, value, evidence).
    pub fn prod_states(
        &self,
        game_id: &str,
    ) -> Result<Vec<(String, String, String, Option<String>, i64, Option<f64>, Option<String>)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT target, kind, status, cause, since_tick, value, evidence
                 FROM productivity_state WHERE game_id = ?1 ORDER BY target",
            )
            .map_err(|e| format!("db prod states prepare: {e}"))?;
        let rows = stmt
            .query_map(params![game_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<f64>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| format!("db prod states: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db prod states row: {e}"))?);
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prod_state_put(
        &self,
        game_id: &str,
        target: &str,
        kind: &str,
        status: &str,
        cause: Option<&str>,
        since_tick: i64,
        value: Option<f64>,
        evidence: &str,
        updated_tick: i64,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO productivity_state(game_id, target, kind, status, cause, since_tick, value, evidence, updated_tick)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(game_id, target) DO UPDATE SET
                     kind = excluded.kind, status = excluded.status, cause = excluded.cause,
                     since_tick = excluded.since_tick, value = excluded.value,
                     evidence = excluded.evidence, updated_tick = excluded.updated_tick",
                params![game_id, target, kind, status, cause, since_tick, value, evidence, updated_tick],
            )
            .map_err(|e| format!("db prod state put: {e}"))?;
        Ok(())
    }

    /// (id, game_date, tick, taken_unix, product_rows, issue_rows), newest first.
    pub fn recent_snapshots(&self, limit: i64) -> Result<Vec<(i64, String, i64, i64, i64, i64)>, String> {        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.game_date, s.tick, s.taken_unix,
                        (SELECT COUNT(*) FROM product_stats ps WHERE ps.snapshot_id = s.id),
                        (SELECT COUNT(*) FROM issues i WHERE i.snapshot_id = s.id)
                 FROM snapshots s ORDER BY s.id DESC LIMIT ?1",
            )
            .map_err(|e| format!("db snaps prepare: {e}"))?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(|e| format!("db snaps list: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("db snaps row: {e}"))?);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Tests: schema v3 migration/backfill + QoS feed import (Addendum XVI)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> String {
        let p = std::env::temp_dir().join(format!(
            "coi-store-test-{}-{tag}.db",
            std::process::id()
        ));
        let s = p.to_string_lossy().to_string();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{s}{suffix}"));
        }
        s
    }

    fn cleanup(path: &str) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path}{suffix}"));
        }
    }

    #[test]
    fn iso8601_utc_parsing() {
        // 2026-11-05T13:00:00Z, computed independently.
        assert_eq!(parse_iso8601_utc("2026-11-05T13:00:00Z"), Some(1793883600));
        assert_eq!(parse_iso8601_utc("2026-11-05T13:00:00.1234567Z"), Some(1793883600));
        assert_eq!(parse_iso8601_utc("2026-11-05T13:00:00+00:00"), Some(1793883600));
        assert_eq!(parse_iso8601_utc("not a date"), None);
        assert_eq!(parse_iso8601_utc("2026-13-05T13:00:00Z"), None);
        assert_eq!(parse_iso8601_utc(""), None);
    }

    #[test]
    fn v2_to_v3_migration_backfills_legacy_audit_idempotently() {
        let path = temp_db("migrate");
        // Build a genuine v2 database: v1 + v2 schemas, stamped 2,
        // with two audit rows and nothing in activity_events.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute_batch(SCHEMA_V2).unwrap();
            conn.execute_batch("PRAGMA user_version = 2").unwrap();
            conn.execute(
                "INSERT INTO audit(ts_unix, action, detail) VALUES (1762000000, 'ToggleEnabledCmd', 'Pause steel · HTTP 200 · success')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO audit(ts_unix, action, detail) VALUES (1762000100, 'SetGeneralPriorityCmd', 'Raise furnace · HTTP 500 · FAILED')",
                [],
            )
            .unwrap();
        }
        {
            let st = Store::open(&path).unwrap();
            let version: i64 = st
                .conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            // The v2 file migrates through v3 (backfill below) to
            // the current head version (v4, deposit_state).
            assert_eq!(version, 4);
            assert_eq!(st.activity_count(None, None).unwrap(), 2);
            let rows = st.activity_recent(None, None, 10, 0).unwrap();
            assert_eq!(rows.len(), 2);
            // Newest first: the priority row (later ts) leads.
            assert_eq!(rows[0].action, "SetGeneralPriorityCmd");
            assert_eq!(rows[0].source, "companion");
            assert_eq!(rows[0].recorded_ts_unix, 1762000100);
            assert!(rows[0].game_date.is_none());
            assert!(rows[0].game_tick.is_none());
            let payload = rows[0].payload_json.clone().unwrap();
            assert!(payload.contains("Raise furnace"));
            assert!(payload.contains("\"legacyAuditId\":2"));
            // The legacy table itself is untouched.
            let legacy: i64 = st
                .conn
                .query_row("SELECT COUNT(*) FROM audit", [], |r| r.get(0))
                .unwrap();
            assert_eq!(legacy, 2);
        }
        // Re-opening must not duplicate the backfill.
        {
            let st = Store::open(&path).unwrap();
            assert_eq!(st.activity_count(None, None).unwrap(), 2);
        }
        cleanup(&path);
    }

    const FEED_LINE_A: &str = r#"{"schema":1,"eventId":"11111111-2222-3333-4444-555555555555","recordedAtUtc":"2026-11-05T13:00:00.0000000Z","source":"qos","modId":"QoSChickens","modVersion":"0.1.1","kind":"command","action":"MoveAnimalsCmd","summary":"equalise: moved +12 into farm #101","data":{"farmId":101,"amount":12,"species":"Chicken","gameDate":"388-2-27"}}"#;
    const FEED_LINE_B: &str = r#"{"schema":1,"eventId":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","recordedAtUtc":"2026-11-05T13:05:00.0000000Z","source":"qos","modId":"QoSWorldMap","modVersion":"0.1.1","kind":"navigation","action":"NavigateToMine","summary":"Navigated to Iron mine","data":{"mine":"Iron mine","product":"Product_IronOre"}}"#;

    #[test]
    fn qos_feed_import_counts_and_promotes_game_date() {
        let path = temp_db("import");
        let feed = [
            FEED_LINE_A,
            FEED_LINE_B,
            FEED_LINE_A, // duplicate eventId
            "this is not json",
            r#"{"schema":1,"recordedAtUtc":"2026-11-05T13:06:00Z","action":"NoIdCmd"}"#, // no eventId
            "",
        ]
        .join("\n");
        let st = Store::open(&path).unwrap();
        let stats = st.import_qos_feed(&feed).unwrap();
        assert_eq!(
            stats,
            QosImportStats { imported: 2, skipped: 1, malformed: 2 }
        );
        // Second pass: everything valid is a duplicate now.
        let stats2 = st.import_qos_feed(&feed).unwrap();
        assert_eq!(
            stats2,
            QosImportStats { imported: 0, skipped: 3, malformed: 2 }
        );
        assert_eq!(st.activity_count(Some("qos"), None).unwrap(), 2);
        assert_eq!(st.activity_count(Some("companion"), None).unwrap(), 0);
        assert_eq!(st.activity_count(None, Some("Navigated")).unwrap(), 1);
        let rows = st.activity_recent(Some("qos"), Some("MoveAnimals"), 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].actor.as_deref(), Some("QoSChickens"));
        assert_eq!(rows[0].category.as_deref(), Some("command"));
        assert_eq!(rows[0].game_date.as_deref(), Some("388-2-27"));
        assert!(rows[0].game_tick.is_none());
        assert_eq!(rows[0].outcome.as_deref(), Some("recorded"));
        assert_eq!(rows[0].recorded_ts_unix, 1793883600);
        cleanup(&path);
    }
}
