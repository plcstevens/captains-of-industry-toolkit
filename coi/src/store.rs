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

impl Store {
    pub fn open(path: &str) -> Result<Store, String> {
        let conn = Connection::open(path).map_err(|e| format!("open db {path}: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("db pragmas: {e}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("db schema: {e}"))?;
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
