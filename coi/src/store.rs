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
    pub fn recent_snapshots(&self, limit: i64) -> Result<Vec<(i64, String, i64, i64, i64, i64)>, String> {
        let mut stmt = self
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
