//! Designation planning + QoSBuilder schema-2 job assembly
//! (designation round, stage 3 of 4 — DESIGNATION-DESIGN-NOTE.md).
//!
//! This module is deliberately self-contained: std + serde_json +
//! sha2 only, no other crate modules, so it could be developed
//! and unit-tested standalone before stage 4 wires it into
//! main.rs / serve.rs. Two halves:
//!
//! * **Canonical v2 + job files.** A byte-level mirror of
//!   QoSBuilder's `BuilderJson.CanonicalDesignation` (the mod
//!   rebuilds the canonical payload from the parsed job and
//!   refuses it if the sha256 differs — so this string IS what
//!   the user confirms). Pinned by the executed test vectors in
//!   `qos/BUILD-NOTES.md` round 9; the tests below reproduce
//!   both published hashes exactly.
//!
//! * **Planners.** The pad planner runs end-to-end on a bridge
//!   `/terrain/region` height grid. The pit core is pure
//!   functions over a per-column layer input type (BEHAVIOURS.md
//!   §1 rules 2/3/4/6/7): the bridge serves no per-column layers
//!   yet (note §1.5), so pit *generation* from live data waits
//!   on that route — the rules here are fixture-fed and ready.
//!
//! Local duplication, deliberate (standalone constraint):
//! `job_escape` repeats serve.rs's one escaping rule, the
//! raw→tiles conversion repeats terrain.rs's single point, and
//! the volume sums repeat `terrain::estimate`'s arithmetic.
//! Stage 4 may unify them; the values must never diverge.

// The pit core below is exercised by its unit tests but has no
// production caller yet: pit *generation* from live data waits
// on the bridge per-column layers route (design note §1.5,
// flagged decision 2). Dead-code warnings for it would be
// noise, not signal — the tests are its current consumer.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Fix32 raw units per 1.0 tile of height (bridge payload basis;
/// same constant as terrain.rs).
pub const RAW_PER_TILE: f64 = 65536.0;

/// Cubic metres in one tile-cubed (TerrainTile.TILE_VOLUME_M).
pub const TILE_VOLUME_M3: f64 = 8.0;

/// Tiles per designation cell side. Origins are 4-grid aligned
/// by construction (the game's DesignationData ctor asserts it).
pub const CELL_TILES: i64 = 4;

/// Job schema version for designation jobs (blueprints stay v1).
pub const SCHEMA_V2: i64 = 2;

pub const JOB_TYPE_ADD: &str = "designations-add";
pub const JOB_TYPE_REMOVE: &str = "designations-remove";

/// The three terraforming designation protos in the mod's
/// allowlist (note §2.2). ForestryDesignator is excluded in v1:
/// `isTerraforming = false`, different fulfilment semantics.
pub const PROTO_MINING: &str = "MiningDesignator";
pub const PROTO_DUMPING: &str = "DumpingDesignator";
pub const PROTO_LEVEL: &str = "LevelDesignator";
pub const ALLOWED_PROTOS: [&str; 3] = [PROTO_MINING, PROTO_DUMPING, PROTO_LEVEL];

/// Corner-height sanity bound, mirroring the mod's malformed-cells
/// discipline (|corner| ≤ 1000 — our rule, not a game limit;
/// note open question 7).
pub const MAX_CORNER_ABS: i64 = 1000;

// ------------------------------------------------------------------
// Canonical payload + job files (mirror of BuilderJson.cs)
// ------------------------------------------------------------------

/// Job-file escaping — the ONE rule shared with QoSBuilder's
/// canonical payload (Addendum XVII): '"' and '\' escaped,
/// control chars as \u00xx (lowercase hex), everything else raw
/// UTF-8. Identical to serve.rs's `job_escape` (duplicated for
/// standalone compilation; the rule must never diverge).
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

/// Lowercase hex SHA-256 of a text payload (the canonical form
/// is hashed as UTF-8 bytes).
pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// One designation cell as carried in a schema-2 add job: the
/// origin tile coords, then the four corner target heights in
/// integer tiles, in DesignationData **ctor order** — origin,
/// plusX, plusXY, plusY (plusXY before plusY; the field
/// declarations in the game list plusY first, the ctor and the
/// serializer do not). The centre height is never carried: the
/// game's 5-arg ctor derives it as the corners' mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub x: i64,
    pub y: i64,
    /// Corner heights in ctor order: [origin, plusX, plusXY, plusY].
    pub corners: [i64; 4],
}

impl Cell {
    pub fn new(x: i64, y: i64, corners: [i64; 4]) -> Cell {
        Cell { x, y, corners }
    }

    /// A level cell: all four corners at the same height.
    pub fn flat(x: i64, y: i64, height: i64) -> Cell {
        Cell { x, y, corners: [height; 4] }
    }

    pub fn corner_origin(&self) -> i64 {
        self.corners[0]
    }
    pub fn corner_plus_x(&self) -> i64 {
        self.corners[1]
    }
    pub fn corner_plus_xy(&self) -> i64 {
        self.corners[2]
    }
    pub fn corner_plus_y(&self) -> i64 {
        self.corners[3]
    }

    pub fn is_flat(&self) -> bool {
        self.corners.iter().all(|&c| c == self.corners[0])
    }
}

/// The attribution/header fields every schema-2 job carries
/// (mirrors the v1 header: jobId is the single-use key, the job
/// is pinned to `game_id`, TTL rides in created/expires).
#[derive(Debug, Clone, Copy)]
pub struct JobMeta<'a> {
    pub job_id: &'a str,
    pub created_utc: &'a str,
    pub expires_utc: &'a str,
    pub game_id: &'a str,
    pub companion_user: &'a str,
    pub plan_name: &'a str,
}

/// The canonical payload string for a schema-2 designation job,
/// rebuilt from its fields — a byte-level mirror of
/// `BuilderJson.CanonicalDesignation`. Field order: jobId,
/// schema, jobType, createdUtc, expiresUtc, gameId,
/// companionUser, planName, protoId (add only), cells|origins,
/// limits.maxCells. Compact, integers bare, strings escaped by
/// the one rule; the sha256 field itself is excluded.
///
/// For "designations-add" pass proto_id + cells (origins None);
/// for "designations-remove" pass origins (proto_id/cells
/// None) — the field set itself is part of the contract, so an
/// add-shaped canonical can never collide with a remove-shaped
/// one. Branching on the jobType string mirrors the C# exactly.
#[allow(clippy::too_many_arguments)]
pub fn canonical_designation(
    job_id: &str,
    schema: i64,
    job_type: &str,
    created_utc: &str,
    expires_utc: &str,
    game_id: &str,
    companion_user: &str,
    plan_name: &str,
    proto_id: Option<&str>,
    cells: Option<&[Cell]>,
    origins: Option<&[(i64, i64)]>,
    max_cells: i64,
) -> String {
    let mut sb = String::new();
    sb.push_str(&format!("{{\"jobId\":\"{}\"", job_escape(job_id)));
    sb.push_str(&format!(",\"schema\":{schema}"));
    sb.push_str(&format!(",\"jobType\":\"{}\"", job_escape(job_type)));
    sb.push_str(&format!(",\"createdUtc\":\"{}\"", job_escape(created_utc)));
    sb.push_str(&format!(",\"expiresUtc\":\"{}\"", job_escape(expires_utc)));
    sb.push_str(&format!(",\"gameId\":\"{}\"", job_escape(game_id)));
    sb.push_str(&format!(",\"companionUser\":\"{}\"", job_escape(companion_user)));
    sb.push_str(&format!(",\"planName\":\"{}\"", job_escape(plan_name)));
    if job_type == JOB_TYPE_ADD {
        // C# Escape(null) is ""; a null protoId canonicalises to
        // an empty string there, so mirror that rather than fail.
        sb.push_str(&format!(",\"protoId\":\"{}\"", job_escape(proto_id.unwrap_or(""))));
        sb.push_str(",\"cells\":[");
        let cells = cells.unwrap_or(&[]);
        for (i, c) in cells.iter().enumerate() {
            if i > 0 {
                sb.push(',');
            }
            sb.push_str(&format!(
                "{{\"x\":{},\"y\":{},\"origin\":{},\"plusX\":{},\"plusXY\":{},\"plusY\":{}}}",
                c.x,
                c.y,
                c.corner_origin(),
                c.corner_plus_x(),
                c.corner_plus_xy(),
                c.corner_plus_y()
            ));
        }
        sb.push(']');
    } else {
        sb.push_str(",\"origins\":[");
        let origins = origins.unwrap_or(&[]);
        for (i, o) in origins.iter().enumerate() {
            if i > 0 {
                sb.push(',');
            }
            sb.push_str(&format!("{{\"x\":{},\"y\":{}}}", o.0, o.1));
        }
        sb.push(']');
    }
    sb.push_str(&format!(",\"limits\":{{\"maxCells\":{max_cells}}}}}"));
    sb
}

/// A fully assembled schema-2 job: the canonical payload, its
/// hash, and the exact job-file text (canonical + the sha256
/// field appended last — the same assembly serve.rs uses for
/// v1 job files).
#[derive(Debug, Clone, PartialEq)]
pub struct DesignationJob {
    pub canonical: String,
    pub sha256: String,
    pub file_json: String,
}

fn assemble_job(canonical: String) -> DesignationJob {
    let sha256 = sha256_hex(&canonical);
    // The file is the canonical form with ,"sha256":"…" inserted
    // before the closing brace (v1 assembly, serve.rs).
    let file_json = format!(
        "{},\"sha256\":\"{}\"}}",
        &canonical[..canonical.len() - 1],
        sha256
    );
    DesignationJob { canonical, sha256, file_json }
}

/// Build a "designations-add" job file from a plan's cells.
/// `max_cells` is the job's own cap, agreed at confirm (the mod
/// enforces min(job cap, its config cap)).
pub fn build_add_job(meta: &JobMeta, proto_id: &str, cells: &[Cell], max_cells: i64) -> DesignationJob {
    let canonical = canonical_designation(
        meta.job_id,
        SCHEMA_V2,
        JOB_TYPE_ADD,
        meta.created_utc,
        meta.expires_utc,
        meta.game_id,
        meta.companion_user,
        meta.plan_name,
        Some(proto_id),
        Some(cells),
        None,
        max_cells,
    );
    assemble_job(canonical)
}

/// Build a "designations-remove" job file from origins (remove
/// is proto-agnostic in the game; kinds are shown from read-back
/// at prepare time, never carried in the job).
pub fn build_remove_job(meta: &JobMeta, origins: &[(i64, i64)], max_cells: i64) -> DesignationJob {
    let canonical = canonical_designation(
        meta.job_id,
        SCHEMA_V2,
        JOB_TYPE_REMOVE,
        meta.created_utc,
        meta.expires_utc,
        meta.game_id,
        meta.companion_user,
        meta.plan_name,
        None,
        None,
        Some(origins),
        max_cells,
    );
    assemble_job(canonical)
}

// ------------------------------------------------------------------
// Planner errors
// ------------------------------------------------------------------

/// Why a planner refused to produce a plan. Every refusal is
/// typed so the confirm page (stage 4) can say exactly which
/// rung fired — the planner's mirror of the mod's refusal
/// ladder.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    /// Rectangle with no area (x1 <= x0 or y1 <= y0).
    EmptyRectangle,
    /// Proto outside the three-proto allowlist.
    UnsupportedProto(String),
    /// Target height outside the ±1000 sanity discipline.
    ImplausibleTarget(i64),
    /// The snapped cell footprint is not fully inside the
    /// height grid (fetch a bigger region — note §1.5 asks for
    /// the rect + a 1-cell margin).
    OutsideGrid { footprint: (i64, i64, i64, i64) },
    /// The plan exceeds the caller-passed cell cap.
    OverCellCap { cells: usize, cap: usize },
    /// A height grid whose heights don't match its dimensions.
    BadGrid(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::EmptyRectangle => write!(f, "rectangle has no area"),
            PlanError::UnsupportedProto(p) => write!(f, "unsupported designation proto: {p}"),
            PlanError::ImplausibleTarget(t) => {
                write!(f, "target height {t} outside the ±{MAX_CORNER_ABS} sanity bound")
            }
            PlanError::OutsideGrid { footprint } => write!(
                f,
                "snapped footprint ({},{})-({},{}) is not fully inside the height grid",
                footprint.0, footprint.1, footprint.2, footprint.3
            ),
            PlanError::OverCellCap { cells, cap } => {
                write!(f, "plan has {cells} cells, over the cap of {cap}")
            }
            PlanError::BadGrid(why) => write!(f, "bad height grid: {why}"),
        }
    }
}

impl std::error::Error for PlanError {}

// ------------------------------------------------------------------
// Height grid (the /terrain/region shape)
// ------------------------------------------------------------------

/// An exact-height grid in tiles: origin tile (x0, y0), width ×
/// height tiles, heights row-major (y * width + x — the game's
/// own index order). Mirrors terrain.rs's `RegionGrid` shape
/// but stands alone (stage 4 adapts between the two).
#[derive(Debug, Clone, PartialEq)]
pub struct HeightGrid {
    pub x0: i64,
    pub y0: i64,
    pub width: usize,
    pub height: usize,
    pub heights: Vec<f64>,
}

impl HeightGrid {
    pub fn new(
        x0: i64,
        y0: i64,
        width: usize,
        height: usize,
        heights: Vec<f64>,
    ) -> Result<HeightGrid, PlanError> {
        if heights.len() != width * height {
            return Err(PlanError::BadGrid(format!(
                "{} heights for a {width}x{height} grid",
                heights.len()
            )));
        }
        Ok(HeightGrid { x0, y0, width, height, heights })
    }

    /// Build from raw Fix32 heights (the bridge's `heightsRaw`):
    /// raw / 65536 = tiles. The conversion rule duplicates
    /// terrain.rs's single point on purpose (standalone module).
    pub fn from_raw(
        x0: i64,
        y0: i64,
        width: usize,
        height: usize,
        raw: &[i64],
    ) -> Result<HeightGrid, PlanError> {
        let heights: Vec<f64> = raw.iter().map(|&r| r as f64 / RAW_PER_TILE).collect();
        HeightGrid::new(x0, y0, width, height, heights)
    }

    /// Parse a bridge `/coi/v1/terrain/region` payload (x1/y1
    /// exclusive) — the same fields terrain::region_from_json
    /// reads.
    pub fn from_region_json(v: &Value) -> Option<HeightGrid> {
        let raw = v["heightsRaw"].as_array()?;
        let raw: Vec<i64> = raw.iter().map(|h| h.as_i64()).collect::<Option<Vec<_>>>()?;
        let (x0, y0) = (v["x0"].as_i64()?, v["y0"].as_i64()?);
        let (x1, y1) = (v["x1"].as_i64()?, v["y1"].as_i64()?);
        if x1 < x0 || y1 < y0 {
            return None;
        }
        HeightGrid::from_raw(x0, y0, (x1 - x0) as usize, (y1 - y0) as usize, &raw).ok()
    }

    pub fn x1(&self) -> i64 {
        self.x0 + self.width as i64
    }

    pub fn y1(&self) -> i64 {
        self.y0 + self.height as i64
    }

    /// Height at a tile, or None outside the grid.
    pub fn height_at(&self, x: i64, y: i64) -> Option<f64> {
        if x < self.x0 || y < self.y0 || x >= self.x1() || y >= self.y1() {
            return None;
        }
        let idx = (y - self.y0) as usize * self.width + (x - self.x0) as usize;
        self.heights.get(idx).copied()
    }
}

// ------------------------------------------------------------------
// Pad planner (ships end-to-end — note §1.5)
// ------------------------------------------------------------------

/// One boundary vertex whose step to the existing terrain just
/// outside the pad breaks the 0/±1 snapping rule (snapping
/// procedure v2): the game accepts such a cell silently, so the
/// warning exists to be read at confirm time, not to refuse.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeWarning {
    /// The boundary vertex (tile coords of the shared corner).
    pub vertex: (i64, i64),
    /// The pad's target height at that vertex.
    pub target: i64,
    /// Existing terrain height of the tile just outside.
    pub outside_height: f64,
    /// target − outside_height; |step| > 1 fired the warning.
    pub step: f64,
}

/// A flatten / dump pad plan: level cells at one target height
/// over a snapped footprint, with earthworks volumes and the
/// edge-step warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct PadPlan {
    pub proto_id: String,
    /// The snapped cell footprint (tiles, x1/y1 exclusive).
    pub footprint: (i64, i64, i64, i64),
    pub cells: Vec<Cell>,
    pub cut_tiles3: f64,
    pub fill_tiles3: f64,
    pub warnings: Vec<EdgeWarning>,
}

impl PadPlan {
    pub fn cut_m3(&self) -> f64 {
        self.cut_tiles3 * TILE_VOLUME_M3
    }
    pub fn fill_m3(&self) -> f64 {
        self.fill_tiles3 * TILE_VOLUME_M3
    }
}

/// Floor to the 4-grid (the game's GetOrigin: (p & -4) — floor
/// division, correct for negative coords too).
fn floor4(v: i64) -> i64 {
    v.div_euclid(CELL_TILES) * CELL_TILES
}

/// Plan a level pad: `rect` is a tile rectangle (x0, y0, x1, y1;
/// x1/y1 exclusive) on `grid`, levelled to `target` (integer
/// tiles — corners are integers by construction), as one batch
/// of `proto_id` cells.
///
/// The rectangle snaps OUTWARD to whole 4×4 cells. Volumes are
/// summed over the snapped footprint with the same arithmetic
/// as `terrain::estimate` (cut = above target, fill = below;
/// reimplemented locally — standalone module). Refusals, in
/// order: empty rectangle → unsupported proto → implausible
/// target → snapped footprint not fully inside the grid →
/// over the caller-passed cell cap.
pub fn plan_pad(
    grid: &HeightGrid,
    rect: (i64, i64, i64, i64),
    target: i64,
    proto_id: &str,
    max_cells: usize,
) -> Result<PadPlan, PlanError> {
    let (rx0, ry0, rx1, ry1) = rect;
    if rx1 <= rx0 || ry1 <= ry0 {
        return Err(PlanError::EmptyRectangle);
    }
    if !ALLOWED_PROTOS.contains(&proto_id) {
        return Err(PlanError::UnsupportedProto(proto_id.to_string()));
    }
    if target.abs() > MAX_CORNER_ABS {
        return Err(PlanError::ImplausibleTarget(target));
    }
    // Snap outward: first cell origin at/below the rect corner,
    // last cell covering the rect's final tile.
    let (fx0, fy0) = (floor4(rx0), floor4(ry0));
    let (fx1, fy1) = (floor4(rx1 - 1) + CELL_TILES, floor4(ry1 - 1) + CELL_TILES);
    if fx0 < grid.x0 || fy0 < grid.y0 || fx1 > grid.x1() || fy1 > grid.y1() {
        return Err(PlanError::OutsideGrid { footprint: (fx0, fy0, fx1, fy1) });
    }
    let nx = ((fx1 - fx0) / CELL_TILES) as usize;
    let ny = ((fy1 - fy0) / CELL_TILES) as usize;
    if nx * ny > max_cells {
        return Err(PlanError::OverCellCap { cells: nx * ny, cap: max_cells });
    }

    let mut cells = Vec::with_capacity(nx * ny);
    for cy in 0..ny {
        for cx in 0..nx {
            cells.push(Cell::flat(
                fx0 + cx as i64 * CELL_TILES,
                fy0 + cy as i64 * CELL_TILES,
                target,
            ));
        }
    }

    // Volumes over the snapped footprint (terrain::estimate's
    // sums, locally): a tile exactly at target contributes 0.
    let (mut cut, mut fill) = (0.0f64, 0.0f64);
    for ty in fy0..fy1 {
        for tx in fx0..fx1 {
            let h = grid.height_at(tx, ty).expect("footprint checked inside grid");
            let d = h - target as f64;
            if d > 0.0 {
                cut += d;
            } else if d < 0.0 {
                fill += -d;
            }
        }
    }

    // Edge-step warnings: for each vertex on the footprint
    // boundary, look at the tiles touching it from OUTSIDE the
    // footprint; a step over ±1 against any of them warns. Tiles
    // outside the grid have no data and never warn (fetch the
    // rect + margin so the ring exists — note §1.5).
    let mut warnings = Vec::new();
    let mut vy = fy0;
    while vy <= fy1 {
        let mut vx = fx0;
        while vx <= fx1 {
            if vx == fx0 || vx == fx1 || vy == fy0 || vy == fy1 {
                for (tx, ty) in
                    [(vx - 1, vy - 1), (vx, vy - 1), (vx - 1, vy), (vx, vy)]
                {
                    let inside = tx >= fx0 && tx < fx1 && ty >= fy0 && ty < fy1;
                    if inside {
                        continue;
                    }
                    if let Some(h) = grid.height_at(tx, ty) {
                        let step = target as f64 - h;
                        if step.abs() > 1.0 {
                            warnings.push(EdgeWarning {
                                vertex: (vx, vy),
                                target,
                                outside_height: h,
                                step,
                            });
                        }
                    }
                }
            }
            vx += CELL_TILES;
        }
        vy += CELL_TILES;
    }

    Ok(PadPlan {
        proto_id: proto_id.to_string(),
        footprint: (fx0, fy0, fx1, fy1),
        cells,
        cut_tiles3: cut,
        fill_tiles3: fill,
        warnings,
    })
}

// ------------------------------------------------------------------
// Pit core — pure functions over per-column layers
// (BEHAVIOURS.md §1 rules 2/3/4/6/7; fixture-fed until a bridge
// per-column layers route exists — note §1.5)
// ------------------------------------------------------------------

/// One material layer in a column, top-down order. Thickness in
/// tiles. Layers are contiguous from the column's surface down:
/// layer 0 spans [surface − thickness, surface], each later
/// layer starts where the previous ended.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSpan {
    pub material: String,
    pub thickness: f64,
}

/// One terrain column (a single tile): surface height + its
/// layer stack, top-down, bedrock included as a layer whose
/// material is the bedrock material id.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub x: i64,
    pub y: i64,
    pub surface: f64,
    pub layers: Vec<LayerSpan>,
}

impl Column {
    /// (top, bottom) elevation of layer `i` (top-down).
    fn layer_span(&self, i: usize) -> (f64, f64) {
        let mut top = self.surface;
        for layer in &self.layers[..i] {
            top -= layer.thickness;
        }
        (top, top - self.layers[i].thickness)
    }

    /// Index of the first bedrock layer, walking top-down.
    fn first_bedrock(&self, bedrock: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.material == bedrock)
    }

    /// Top elevation of the first bedrock layer, if any.
    pub fn bedrock_top(&self, bedrock: &str) -> Option<f64> {
        self.first_bedrock(bedrock).map(|i| self.layer_span(i).0)
    }

    pub fn has_material(&self, material: &str) -> bool {
        self.layers.iter().any(|l| l.material == material)
    }

    /// Bottom elevation of the deepest target interval above
    /// the first bedrock layer (the spike filter's anchor;
    /// BEHAVIOURS §1 rule 2's `deepestTargetBottom`).
    pub fn deepest_target_bottom(&self, target: &str, bedrock: &str) -> Option<f64> {
        let stop = self.first_bedrock(bedrock).unwrap_or(self.layers.len());
        let mut deepest: Option<f64> = None;
        for i in 0..stop {
            if self.layers[i].material == target {
                deepest = Some(self.layer_span(i).1);
            }
        }
        deepest
    }

    /// Total thickness of everything above the first bedrock
    /// layer (bedrock excluded — the purity denominator).
    pub fn non_bedrock_thickness(&self, bedrock: &str) -> f64 {
        let stop = self.first_bedrock(bedrock).unwrap_or(self.layers.len());
        self.layers[..stop].iter().map(|l| l.thickness).sum()
    }
}

/// A tile-resolution grid of columns (one per tile), row-major.
/// Positions with no captured column are None and contribute
/// nothing to any rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnGrid {
    pub x0: i64,
    pub y0: i64,
    pub width: usize,
    pub height: usize,
    pub columns: Vec<Option<Column>>,
}

impl ColumnGrid {
    pub fn new(
        x0: i64,
        y0: i64,
        width: usize,
        height: usize,
        columns: Vec<Option<Column>>,
    ) -> Result<ColumnGrid, PlanError> {
        if columns.len() != width * height {
            return Err(PlanError::BadGrid(format!(
                "{} columns for a {width}x{height} grid",
                columns.len()
            )));
        }
        Ok(ColumnGrid { x0, y0, width, height, columns })
    }

    pub fn at(&self, x: i64, y: i64) -> Option<&Column> {
        if x < self.x0 || y < self.y0 || x >= self.x0 + self.width as i64 || y >= self.y0 + self.height as i64 {
            return None;
        }
        let idx = (y - self.y0) as usize * self.width + (x - self.x0) as usize;
        self.columns.get(idx).and_then(|c| c.as_ref())
    }

    fn index(&self, x: i64, y: i64) -> usize {
        (y - self.y0) as usize * self.width + (x - self.x0) as usize
    }
}

/// Median of a sample. For an even count this is the mean of
/// the two middle values (the spike filter's 8-neighbour case;
/// BEHAVIOURS §1 says "median of the 8" without pinning the
/// even-count convention — this is the standard one, flagged
/// in the design note's stage-3 section).
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// AllowedBedrockResidual (ATD, hardcoded — rule 2).
pub const SPIKE_ALLOWED_BEDROCK_RESIDUAL: f64 = 4.0;
/// MinimumBedrockNeighbors (ATD, hardcoded — rule 2): ALL 8
/// neighbours must have bedrock or the column is not corrected.
pub const SPIKE_MIN_BEDROCK_NEIGHBOURS: usize = 8;

/// The spike filter (rule 2), FIRST before any quality step.
/// Returns one entry per grid tile: Some(cutoff) where the
/// column is corrected, None where it is not.
///
/// A column is corrected iff it contains both a target layer
/// and bedrock, all 8 neighbours have bedrock, and
/// `median(neighbour bedrock tops) − centre bedrock top − 4`
/// exceeds 0.0001; the cutoff is then
/// `deepest_target_bottom + raise`. Target thickness below the
/// cutoff is invisible to every later rule (via
/// `target_intervals`). Terrain and layers are never modified.
/// The shipped detector has NO thickness criterion — a column
/// is corrected solely for its depressed bedrock top.
pub fn spike_cutoffs(grid: &ColumnGrid, target: &str, bedrock: &str) -> Vec<Option<f64>> {
    let mut out = vec![None; grid.width * grid.height];
    for y in grid.y0..grid.y0 + grid.height as i64 {
        for x in grid.x0..grid.x0 + grid.width as i64 {
            let idx = grid.index(x, y);
            let Some(col) = grid.at(x, y) else { continue };
            let (Some(centre_top), Some(target_bottom)) = (
                col.bedrock_top(bedrock),
                col.deepest_target_bottom(target, bedrock),
            ) else {
                continue;
            };
            if !col.has_material(target) {
                continue;
            }
            let mut tops: Vec<f64> = Vec::with_capacity(8);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    if let Some(n) = grid.at(x + dx, y + dy) {
                        if let Some(t) = n.bedrock_top(bedrock) {
                            tops.push(t);
                        }
                    }
                }
            }
            if tops.len() < SPIKE_MIN_BEDROCK_NEIGHBOURS {
                continue;
            }
            let raise = median(&mut tops) - centre_top - SPIKE_ALLOWED_BEDROCK_RESIDUAL;
            if raise > 0.0001 {
                out[idx] = Some(target_bottom + raise);
            }
        }
    }
    out
}

/// One target interval in a column, above the first bedrock:
/// its true span plus the spike-filtered visible thickness
/// (`clamp(top − cutoff, 0, thickness)`; no cutoff ⇒ full).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub top: f64,
    pub bottom: f64,
    pub thickness: f64,
    pub visible: f64,
}

/// The column's target intervals, top-down, stopping at the
/// first bedrock layer (the purity walk's stop rule).
pub fn target_intervals(
    col: &Column,
    target: &str,
    bedrock: &str,
    cutoff: Option<f64>,
) -> Vec<Interval> {
    let stop = col.first_bedrock(bedrock).unwrap_or(col.layers.len());
    let mut out = Vec::new();
    for i in 0..stop {
        let layer = &col.layers[i];
        if layer.material != target {
            continue;
        }
        let (top, bottom) = col.layer_span(i);
        let visible = match cutoff {
            Some(c) => (top - c).clamp(0.0, layer.thickness),
            None => layer.thickness,
        };
        out.push(Interval { top, bottom, thickness: layer.thickness, visible });
    }
    out
}

/// Spike-filtered target thickness of one column.
pub fn visible_target_thickness(
    col: &Column,
    target: &str,
    bedrock: &str,
    cutoff: Option<f64>,
) -> f64 {
    target_intervals(col, target, bedrock, cutoff).iter().map(|i| i.visible).sum()
}

/// Ore-quality gates by purity level 0–4 (ATD revision-5
/// arrays, BEHAVIOURS §1 rule 3). A gate value of 0 disables
/// that gate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PurityGate {
    pub min_ore_height: f64,
    pub min_bottom_density: f64,
    pub min_purity: f64,
    pub min_component_size: usize,
}

pub const PURITY_GATES: [PurityGate; 5] = [
    PurityGate { min_ore_height: 0.0, min_bottom_density: 0.0, min_purity: 0.0, min_component_size: 0 },
    PurityGate { min_ore_height: 0.25, min_bottom_density: 0.05, min_purity: 0.05, min_component_size: 2 },
    PurityGate { min_ore_height: 0.75, min_bottom_density: 0.20, min_purity: 0.20, min_component_size: 6 },
    PurityGate { min_ore_height: 2.0, min_bottom_density: 0.50, min_purity: 0.50, min_component_size: 20 },
    PurityGate { min_ore_height: 3.0, min_bottom_density: 0.75, min_purity: 0.75, min_component_size: 40 },
];

/// The gate row for a purity level (clamped to 0–4; level 0 =
/// Off is the ATD default).
pub fn purity_gate(level: usize) -> PurityGate {
    PURITY_GATES[level.min(4)]
}

/// The bottom-density walk (rule 3, `TryGetPurityAdjustedDepth`):
/// given a column's intervals shallow→deep (intervals with no
/// visible thickness are skipped — the walk runs on the
/// purity-adjusted interpretation), the dig depth: the
/// shallowest interval always qualifies; each later interval
/// extends the dig while
/// `visible ÷ (waste gap above it + its thickness)` ≥
/// `min_bottom_density`; the FIRST failure stops the walk —
/// nothing deeper is included even if it would pass alone.
/// Density 0 ⇒ the deepest interval's bottom. None when no
/// interval has visible thickness.
pub fn dig_depth(intervals: &[Interval], min_bottom_density: f64) -> Option<f64> {
    let live: Vec<&Interval> = intervals.iter().filter(|i| i.visible > 0.0).collect();
    let first = *live.first()?;
    if min_bottom_density <= 0.0 {
        return live.last().map(|i| i.bottom);
    }
    let mut depth = first.bottom;
    let mut prev = first;
    for iv in &live[1..] {
        let span = (prev.bottom - iv.top) + iv.thickness;
        let density = if span > 0.0 { iv.visible / span } else { 1.0 };
        if density < min_bottom_density {
            break;
        }
        depth = iv.bottom;
        prev = iv;
    }
    Some(depth)
}

/// Per-origin quality evaluation (rule 3 + the per-column dig
/// depths): ore height and purity summed over the origin's 16
/// columns; depth = the deepest justified dig among its columns
/// (the component flattening of rule 6 disciplines the result
/// afterwards — the distilled text does not pin a different
/// per-origin depth aggregation; flagged in the design note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OriginEval {
    /// Sum of spike-filtered target thickness (tiles).
    pub ore_height: f64,
    /// Sum of non-bedrock thickness (tiles) — purity denominator.
    pub total: f64,
    /// ore / total (0 when total is 0).
    pub purity: f64,
    /// Deepest dig bottom among the origin's columns.
    pub depth: Option<f64>,
    /// Ore-height and purity gates passed (bottom density shapes
    /// depth rather than gating; component size is rule 3's
    /// separate filter — `filter_components`).
    pub passes_gates: bool,
}

pub fn evaluate_origin(
    grid: &ColumnGrid,
    cutoffs: &[Option<f64>],
    origin: (i64, i64),
    target: &str,
    bedrock: &str,
    gate: &PurityGate,
) -> OriginEval {
    let (mut ore, mut total) = (0.0f64, 0.0f64);
    let mut depth: Option<f64> = None;
    for ty in origin.1..origin.1 + CELL_TILES {
        for tx in origin.0..origin.0 + CELL_TILES {
            let Some(col) = grid.at(tx, ty) else { continue };
            let cutoff = cutoffs.get(grid.index(tx, ty)).copied().flatten();
            ore += visible_target_thickness(col, target, bedrock, cutoff);
            total += col.non_bedrock_thickness(bedrock);
            let intervals = target_intervals(col, target, bedrock, cutoff);
            if let Some(d) = dig_depth(&intervals, gate.min_bottom_density) {
                depth = Some(depth.map_or(d, |cur: f64| cur.min(d)));
            }
        }
    }
    let purity = if total > 0.0 { ore / total } else { 0.0 };
    let passes_gates = (gate.min_ore_height <= 0.0 || ore >= gate.min_ore_height)
        && (gate.min_purity <= 0.0 || purity >= gate.min_purity);
    OriginEval { ore_height: ore, total, purity, depth, passes_gates }
}

/// Minimum surface height over an origin's captured columns
/// (rule 4's per-origin surface).
pub fn origin_surface_min(grid: &ColumnGrid, origin: (i64, i64)) -> Option<f64> {
    let mut min: Option<f64> = None;
    for ty in origin.1..origin.1 + CELL_TILES {
        for tx in origin.0..origin.0 + CELL_TILES {
            if let Some(col) = grid.at(tx, ty) {
                min = Some(min.map_or(col.surface, |cur: f64| cur.min(col.surface)));
            }
        }
    }
    min
}

/// Depth caps, in rule 4's order: relative cap first
/// (`MaxLayers`, ATD default 30, 0/None = no limit —
/// `depth = max(depth, surface − MaxLayers)`), then the
/// absolute floor (`MinElevation`, unset = no limit).
pub fn apply_depth_caps(
    depth: f64,
    origin_surface_min: f64,
    max_layers: Option<f64>,
    min_elevation: Option<f64>,
) -> f64 {
    let mut d = depth;
    if let Some(ml) = max_layers {
        if ml > 0.0 {
            d = d.max(origin_surface_min - ml);
        }
    }
    if let Some(me) = min_elevation {
        d = d.max(me);
    }
    d
}

// ------------------------------------------------------------------
// Components, floors (rule 6) and corners (rule 7)
// ------------------------------------------------------------------

/// 4-connected components of a set of cell origins (rule 3/6
/// connectivity). Deterministic: origins are visited in sorted
/// order, neighbours in a fixed order.
pub fn components(origins: &[(i64, i64)]) -> Vec<Vec<(i64, i64)>> {
    let set: HashSet<(i64, i64)> = origins.iter().copied().collect();
    let mut sorted: Vec<(i64, i64)> = set.iter().copied().collect();
    sorted.sort();
    let mut seen: HashSet<(i64, i64)> = HashSet::new();
    let mut out = Vec::new();
    for start in sorted {
        if seen.contains(&start) {
            continue;
        }
        let mut comp = Vec::new();
        let mut stack = vec![start];
        seen.insert(start);
        while let Some(o) = stack.pop() {
            comp.push(o);
            for d in [(CELL_TILES, 0), (-CELL_TILES, 0), (0, CELL_TILES), (0, -CELL_TILES)] {
                let n = (o.0 + d.0, o.1 + d.1);
                if set.contains(&n) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }
        comp.sort();
        out.push(comp);
    }
    out
}

/// Rule 3's component-size filter: the largest component is
/// always kept; any other component is removed if its size is
/// under the gate's minimum, else removed only if EVERY
/// origin's ore height is under the gate's minimum (one origin
/// at/above saves it). A zero minimum keeps everything.
pub fn filter_components(
    comps: Vec<Vec<(i64, i64)>>,
    ore_height: &impl Fn((i64, i64)) -> f64,
    gate: &PurityGate,
) -> Vec<Vec<(i64, i64)>> {
    if gate.min_component_size == 0 || comps.is_empty() {
        return comps;
    }
    let largest = comps.iter().map(|c| c.len()).max().unwrap_or(0);
    comps
        .into_iter()
        .filter(|comp| {
            if comp.len() == largest {
                return true;
            }
            if comp.len() < gate.min_component_size {
                return false;
            }
            comp.iter().any(|&o| ore_height(o) >= gate.min_ore_height)
        })
        .collect()
}

/// The component floor (rule 6): the percentile depth at
/// `strength` (clamped 1–10, default 5). Depths are floor
/// elevations; sorted ascending = deepest first;
/// `index = clamp(n × (10 − strength) / 10, 0, n−1)` — strength
/// 10 → the deepest tile, 5 → the median, 1 → the
/// 90th-percentile shallow. None for an empty component.
pub fn component_floor(depths: &[f64], strength: u32) -> Option<f64> {
    if depths.is_empty() {
        return None;
    }
    let strength = strength.clamp(1, 10);
    let mut sorted = depths.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (sorted.len() * (10 - strength) as usize / 10).min(sorted.len() - 1);
    Some(sorted[idx])
}

/// Flatten one component's depths to its floor (rule 6).
/// Components under 4 tiles are skipped (returned unchanged).
/// Lower-only (purity level 0): depths shallower than the floor
/// are pulled down to it, nothing is ever raised — in elevation
/// terms, elementwise min. Full levelling (purity > 0): every
/// depth set to the floor. No weighted blending exists in ATD.
pub fn flatten_depths(depths: &[f64], strength: u32, full_levelling: bool) -> Vec<f64> {
    if depths.len() < 4 {
        return depths.to_vec();
    }
    let floor = component_floor(depths, strength).expect("non-empty checked above");
    if full_levelling {
        vec![floor; depths.len()]
    } else {
        depths.iter().map(|&d| d.min(floor)).collect()
    }
}

/// How a corner's initial height is taken from its contributing
/// origins (rule 7): `Deepest` at purity ≤ 0 (the deepest
/// contributing tile depth); `Mean` at purity > 0 (ATD uses a
/// running integer average — order-dependent; this port uses
/// the true rounded mean, the deterministic reading, flagged
/// in the design note's stage-3 section).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerInit {
    Deepest,
    Mean,
}

/// The corner-height grid over a set of origins: vertices on
/// the 4-grid spanning the origins' bounds, `(nx + 1) × (ny + 1)`
/// vertices for nx × ny origins, row-major. Vertices touched by
/// no origin (holes in a non-rectangular plan) are not `live`:
/// they never smooth and are never emitted.
#[derive(Debug, Clone, PartialEq)]
pub struct CornerGrid {
    pub x0: i64,
    pub y0: i64,
    pub nx: usize,
    pub ny: usize,
    pub heights: Vec<i64>,
    pub live: Vec<bool>,
}

impl CornerGrid {
    fn vindex(&self, vx: i64, vy: i64) -> Option<usize> {
        if vx < self.x0 || vy < self.y0 {
            return None;
        }
        let (dx, dy) = (vx - self.x0, vy - self.y0);
        if dx % CELL_TILES != 0 || dy % CELL_TILES != 0 {
            return None;
        }
        let (ix, iy) = (dx / CELL_TILES, dy / CELL_TILES);
        if ix as usize > self.nx || iy as usize > self.ny {
            return None;
        }
        Some(iy as usize * (self.nx + 1) + ix as usize)
    }

    pub fn at(&self, vx: i64, vy: i64) -> Option<i64> {
        let i = self.vindex(vx, vy)?;
        if self.live[i] {
            Some(self.heights[i])
        } else {
            None
        }
    }
}

/// Build corner heights from per-origin integer floor heights
/// (`(origin_x, origin_y, floor)` triples), then smooth
/// (rule 7): iterate over all cardinally adjacent vertex pairs
/// until no pair differs by more than `max_height_diff`
/// (ATD default 1, clamp 1–3 — the caller passes the clamped
/// value); the shallower corner is LOWERED to the other +
/// max_height_diff, so smoothing only ever deepens. Iteration
/// cap 1,000 (ATD's); the returned bool is whether a full pass
/// changed nothing before the cap (converged). Heights are
/// elevations: "shallower" = the higher value, "lowered" =
/// decreased toward the deeper corner.
pub fn build_corners(
    floors: &[(i64, i64, i64)],
    init: CornerInit,
    max_height_diff: i64,
) -> (CornerGrid, bool) {
    let floor_map: HashMap<(i64, i64), i64> =
        floors.iter().map(|&(x, y, f)| ((x, y), f)).collect();
    let (mut min_x, mut min_y) = (i64::MAX, i64::MAX);
    let (mut max_x, mut max_y) = (i64::MIN, i64::MIN);
    for &(x, y, _) in floors {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if floors.is_empty() {
        return (
            CornerGrid { x0: 0, y0: 0, nx: 0, ny: 0, heights: vec![], live: vec![] },
            true,
        );
    }
    // Origin counts across/down (bounds are origins; the vertex
    // grid extends one cell past the last origin).
    let nx = ((max_x - min_x) / CELL_TILES) as usize + 1;
    let ny = ((max_y - min_y) / CELL_TILES) as usize + 1;
    let (vw, vh) = (nx + 1, ny + 1);
    let mut heights = vec![0i64; vw * vh];
    let mut live = vec![false; vw * vh];
    // Initial heights: each vertex from its contributing
    // origins (the up-to-4 cells that share the vertex).
    for iy in 0..vh {
        for ix in 0..vw {
            let (vx, vy) = (min_x + ix as i64 * CELL_TILES, min_y + iy as i64 * CELL_TILES);
            let mut contribs: Vec<i64> = Vec::with_capacity(4);
            for (ox, oy) in [
                (vx - CELL_TILES, vy - CELL_TILES),
                (vx, vy - CELL_TILES),
                (vx - CELL_TILES, vy),
                (vx, vy),
            ] {
                if let Some(&f) = floor_map.get(&(ox, oy)) {
                    contribs.push(f);
                }
            }
            if contribs.is_empty() {
                continue;
            }
            let idx = iy * vw + ix;
            live[idx] = true;
            heights[idx] = match init {
                CornerInit::Deepest => contribs.iter().copied().min().unwrap(),
                CornerInit::Mean => {
                    let sum: i64 = contribs.iter().sum();
                    (sum as f64 / contribs.len() as f64).round() as i64
                }
            };
        }
    }
    let mut grid =
        CornerGrid { x0: min_x, y0: min_y, nx, ny, heights, live };
    // Smoothing: only ever lowers the shallower of a pair.
    let mut converged = false;
    for _ in 0..1000 {
        let mut changed = false;
        for iy in 0..vh {
            for ix in 0..vw {
                let idx = iy * vw + ix;
                if !grid.live[idx] {
                    continue;
                }
                for (dx, dy) in [(1usize, 0usize), (0, 1)] {
                    let (jx, jy) = (ix + dx, iy + dy);
                    if jx >= vw || jy >= vh {
                        continue;
                    }
                    let j = jy * vw + jx;
                    if !grid.live[j] {
                        continue;
                    }
                    let (a, b) = (grid.heights[idx], grid.heights[j]);
                    if (a - b).abs() > max_height_diff {
                        if a > b {
                            grid.heights[idx] = b + max_height_diff;
                        } else {
                            grid.heights[j] = a + max_height_diff;
                        }
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            converged = true;
            break;
        }
    }
    (grid, converged)
}

/// Emit the job cells for a set of origins from a smoothed
/// corner grid, in sorted origin order. Origins whose corners
/// are not all live are skipped (cannot happen for origins the
/// grid was built from; the guard keeps the function total).
pub fn cells_from_corners(floors: &[(i64, i64, i64)], corners: &CornerGrid) -> Vec<Cell> {
    let mut origins: Vec<(i64, i64)> = floors.iter().map(|&(x, y, _)| (x, y)).collect();
    origins.sort();
    origins.dedup();
    let mut out = Vec::with_capacity(origins.len());
    for (ox, oy) in origins {
        let (Some(o), Some(px), Some(pxy), Some(py)) = (
            corners.at(ox, oy),
            corners.at(ox + CELL_TILES, oy),
            corners.at(ox + CELL_TILES, oy + CELL_TILES),
            corners.at(ox, oy + CELL_TILES),
        ) else {
            continue;
        };
        // Ctor order: origin, plusX, plusXY, plusY.
        out.push(Cell::new(ox, oy, [o, px, pxy, py]));
    }
    out
}

// ------------------------------------------------------------------
// Read-back: existing designations + the landed verdict
// ------------------------------------------------------------------

/// One designation as the bridge's `/coi/v1/designations` route
/// serves it (0.6.6 shape). `corners` is present only on bridge
/// 0.6.6+; `band` (min/max target height) is served by every
/// version and is the fallback evidence for older bridges.
#[derive(Debug, Clone, PartialEq)]
pub struct ExistingDesignation {
    pub x: i64,
    pub y: i64,
    /// Partition label from the bridge: "mining" / "dumping" /
    /// "other" (a Levelling cell labels "mining" — the mining
    /// set is checked first; `proto` disambiguates).
    pub kind: String,
    pub proto: String,
    /// Corner target heights in ctor order, bridge 0.6.6+.
    pub corners: Option<[i64; 4]>,
    /// (min, max) target-height band, all bridge versions.
    pub band: Option<(i64, i64)>,
    pub is_fulfilled: bool,
    pub jobs_assigned: i64,
}

/// Parse a `/coi/v1/designations` payload into rows. Malformed
/// rows are skipped, never fatal — read-back is evidence, and
/// partial evidence beats none.
pub fn existing_from_json(v: &Value) -> Vec<ExistingDesignation> {
    let Some(rows) = v["designations"].as_array() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let (Some(x), Some(y)) = (
            r["origin"][0].as_i64(),
            r["origin"][1].as_i64(),
        ) else {
            continue;
        };
        let corners = r["corners"].as_array().and_then(|a| {
            if a.len() == 4 {
                Some([
                    a[0].as_i64()?,
                    a[1].as_i64()?,
                    a[2].as_i64()?,
                    a[3].as_i64()?,
                ])
            } else {
                None
            }
        });
        let band = match (r["minTargetHeight"].as_i64(), r["maxTargetHeight"].as_i64()) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            _ => None,
        };
        out.push(ExistingDesignation {
            x,
            y,
            kind: r["kind"].as_str().unwrap_or("other").to_string(),
            proto: r["proto"].as_str().unwrap_or("").to_string(),
            corners,
            band,
            is_fulfilled: r["isFulfilled"].as_bool().unwrap_or(false),
            jobs_assigned: r["jobsAssigned"].as_i64().unwrap_or(0),
        });
    }
    out
}

/// The existing designation at a cell origin, if any (origins
/// are unique in the game — Add replaces).
pub fn existing_at<'a>(
    existing: &'a [ExistingDesignation],
    x: i64,
    y: i64,
) -> Option<&'a ExistingDesignation> {
    existing.iter().find(|e| e.x == x && e.y == y)
}

/// How one planned add-cell reads back against the live list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellMatch {
    /// Same origin, same proto, corners exactly equal (0.6.6).
    Exact,
    /// Same origin and proto, corners not served (pre-0.6.6
    /// bridge) but the min/max band equals the cell's corner
    /// band — necessary, not sufficient: a partial verdict.
    BandOnly,
    /// A designation sits at the origin but is not this cell
    /// (different proto and/or different corners).
    Different,
    /// Nothing is designated at the origin.
    Absent,
}

/// Match one planned cell against the read-back rows.
pub fn cell_match(
    cell: &Cell,
    proto_id: &str,
    existing: &[ExistingDesignation],
) -> CellMatch {
    let Some(e) = existing_at(existing, cell.x, cell.y) else {
        return CellMatch::Absent;
    };
    if e.proto != proto_id {
        return CellMatch::Different;
    }
    match e.corners {
        Some(c) if c == cell.corners => CellMatch::Exact,
        Some(_) => CellMatch::Different,
        None => {
            let lo = *cell.corners.iter().min().unwrap_or(&0);
            let hi = *cell.corners.iter().max().unwrap_or(&0);
            if e.band == Some((lo, hi)) {
                CellMatch::BandOnly
            } else {
                CellMatch::Different
            }
        }
    }
}

/// The add-job verdict over the whole plan: per-state counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AddDiff {
    pub exact: usize,
    pub band_only: usize,
    pub different: usize,
    pub absent: usize,
}

impl AddDiff {
    pub fn total(&self) -> usize {
        self.exact + self.band_only + self.different + self.absent
    }
    /// Cells reading back as this plan (exactly or at band
    /// level — the caller distinguishes the two in copy).
    pub fn landed(&self) -> usize {
        self.exact + self.band_only
    }
    pub fn all_landed(&self) -> bool {
        self.total() > 0 && self.landed() == self.total()
    }
}

/// Diff a confirmed add plan against the current read-back.
pub fn diff_add(cells: &[Cell], proto_id: &str, existing: &[ExistingDesignation]) -> AddDiff {
    let mut d = AddDiff::default();
    for c in cells {
        match cell_match(c, proto_id, existing) {
            CellMatch::Exact => d.exact += 1,
            CellMatch::BandOnly => d.band_only += 1,
            CellMatch::Different => d.different += 1,
            CellMatch::Absent => d.absent += 1,
        }
    }
    d
}

/// Remove-job verdict: how many of the job's origins are STILL
/// designated (0 = the removal fully landed).
pub fn diff_remove(origins: &[(i64, i64)], existing: &[ExistingDesignation]) -> usize {
    origins
        .iter()
        .filter(|&&(x, y)| existing_at(existing, x, y).is_some())
        .count()
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(material: &str, thickness: f64) -> LayerSpan {
        LayerSpan { material: material.to_string(), thickness }
    }

    fn flat_grid(size: i64, height: f64) -> HeightGrid {
        HeightGrid::new(
            0,
            0,
            size as usize,
            size as usize,
            vec![height; (size * size) as usize],
        )
        .unwrap()
    }

    // ---- canonical vectors (BUILD-NOTES round 9) ----

    #[test]
    fn canonical_add_vector_matches_published_hash() {
        let cells = [
            Cell::new(100, 200, [12, 12, 11, 12]),
            Cell::new(104, 200, [12, 11, 11, 12]),
        ];
        let canonical = canonical_designation(
            "22222222-3333-4444-5555-666666666666",
            2,
            "designations-add",
            "2026-11-06T20:00:00Z",
            "2026-11-06T20:30:00Z",
            "5583213724730880438",
            "captain",
            "Test pad \"A\" \\ Béton",
            Some("MiningDesignator"),
            Some(&cells),
            None,
            200,
        );
        assert_eq!(
            sha256_hex(&canonical),
            "e4166dd35533a12049ee495a5192358f3a262272cc86475c0a79979158ed57f7"
        );
        // The typed builder must produce the identical job.
        let meta = JobMeta {
            job_id: "22222222-3333-4444-5555-666666666666",
            created_utc: "2026-11-06T20:00:00Z",
            expires_utc: "2026-11-06T20:30:00Z",
            game_id: "5583213724730880438",
            companion_user: "captain",
            plan_name: "Test pad \"A\" \\ Béton",
        };
        let job = build_add_job(&meta, "MiningDesignator", &cells, 200);
        assert_eq!(job.canonical, canonical);
        assert_eq!(
            job.sha256,
            "e4166dd35533a12049ee495a5192358f3a262272cc86475c0a79979158ed57f7"
        );
        assert!(job
            .file_json
            .ends_with(",\"sha256\":\"e4166dd35533a12049ee495a5192358f3a262272cc86475c0a79979158ed57f7\"}"));
    }

    #[test]
    fn canonical_remove_vector_matches_published_hash() {
        let origins = [(100, 200), (108, 204)];
        let canonical = canonical_designation(
            "33333333-4444-5555-6666-777777777777",
            2,
            "designations-remove",
            "2026-11-06T20:05:00Z",
            "2026-11-06T20:35:00Z",
            "5583213724730880438",
            "captain",
            "Test removal",
            None,
            None,
            Some(&origins),
            200,
        );
        assert_eq!(
            sha256_hex(&canonical),
            "78fb273ccdb9d498cae5e05ecc077d1e18c58dd2c406d59c23d1a8617e70cfa4"
        );
        let meta = JobMeta {
            job_id: "33333333-4444-5555-6666-777777777777",
            created_utc: "2026-11-06T20:05:00Z",
            expires_utc: "2026-11-06T20:35:00Z",
            game_id: "5583213724730880438",
            companion_user: "captain",
            plan_name: "Test removal",
        };
        let job = build_remove_job(&meta, &origins, 200);
        assert_eq!(job.canonical, canonical);
        assert_eq!(job.sha256, sha256_hex(&canonical));
    }

    // ---- pad planner ----

    #[test]
    fn pad_flat_terrain() {
        let grid = flat_grid(16, 7.0);
        let plan = plan_pad(&grid, (0, 0, 16, 16), 7, PROTO_DUMPING, 200).unwrap();
        assert_eq!(plan.footprint, (0, 0, 16, 16));
        assert_eq!(plan.cells.len(), 16);
        assert!(plan.cells.iter().all(|c| c.is_flat() && c.corner_origin() == 7));
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.cut_tiles3, 0.0);
        assert_eq!(plan.fill_tiles3, 0.0);
    }

    #[test]
    fn pad_snaps_outward_to_whole_cells() {
        let grid = flat_grid(16, 7.0);
        let plan = plan_pad(&grid, (2, 2, 14, 14), 7, PROTO_LEVEL, 200).unwrap();
        assert_eq!(plan.footprint, (0, 0, 16, 16));
        assert_eq!(plan.cells.len(), 16);
        assert_eq!(plan.cells[0], Cell::flat(0, 0, 7));
    }

    #[test]
    fn pad_on_slope_warns_on_the_high_side_only() {
        // Height rises eastward: h(x, y) = x. Pad target 4.
        let size = 24usize;
        let mut heights = vec![0.0; size * size];
        for y in 0..size {
            for x in 0..size {
                heights[y * size + x] = x as f64;
            }
        }
        let grid = HeightGrid::new(0, 0, size, size, heights).unwrap();
        let plan = plan_pad(&grid, (4, 4, 20, 20), 4, PROTO_MINING, 200).unwrap();
        assert_eq!(plan.cells.len(), 16);
        // Cut: per row sum_{x=5..19} (x-4) = 120; 16 rows.
        assert_eq!(plan.cut_tiles3, 1920.0);
        assert_eq!(plan.fill_tiles3, 0.0);
        assert!(!plan.warnings.is_empty());
        // West edge (vx = 4): outside terrain is 3, step is +1 —
        // inside the rule, so no warning may carry vx == 4.
        assert!(plan.warnings.iter().all(|w| w.vertex.0 != 4));
        // Every east vertex (vx = 20) warns (outside height 19/20).
        for vy in [4, 8, 12, 16, 20] {
            assert!(
                plan.warnings.iter().any(|w| w.vertex == (20, vy) && w.step < -1.0),
                "expected an east-edge warning at (20, {vy})"
            );
        }
    }

    #[test]
    fn pad_outside_grid_refused() {
        let grid = flat_grid(16, 7.0);
        let err = plan_pad(&grid, (8, 8, 24, 24), 7, PROTO_MINING, 200).unwrap_err();
        assert!(matches!(err, PlanError::OutsideGrid { .. }));
        // A rect starting before the grid origin snaps further out.
        let err = plan_pad(&grid, (-2, 0, 8, 8), 7, PROTO_MINING, 200).unwrap_err();
        assert!(matches!(err, PlanError::OutsideGrid { .. }));
        // And the tame failures are typed too.
        assert_eq!(
            plan_pad(&grid, (4, 4, 4, 8), 7, PROTO_MINING, 200).unwrap_err(),
            PlanError::EmptyRectangle
        );
        assert!(matches!(
            plan_pad(&grid, (0, 0, 8, 8), 7, "ForestryDesignator", 200).unwrap_err(),
            PlanError::UnsupportedProto(_)
        ));
    }

    #[test]
    fn pad_over_cap_refused() {
        let grid = flat_grid(40, 5.0);
        let err = plan_pad(&grid, (0, 0, 40, 40), 5, PROTO_MINING, 10).unwrap_err();
        assert_eq!(err, PlanError::OverCellCap { cells: 100, cap: 10 });
    }

    // ---- pit core ----

    #[test]
    fn spike_column_is_corrected_and_partly_hidden() {
        // 3x3 columns; every neighbour's bedrock top is 10. The
        // centre column's bedrock top is depressed to 5, with a
        // target seam at [8, 12].
        let neighbour = |x: i64, y: i64| {
            Some(Column {
                x,
                y,
                surface: 20.0,
                layers: vec![layer("Dirt", 10.0), layer("Bedrock", 30.0)],
            })
        };
        let centre = Column {
            x: 1,
            y: 1,
            surface: 20.0,
            layers: vec![
                layer("Dirt", 8.0),
                layer("IronOre", 4.0),
                layer("Dirt", 3.0),
                layer("Bedrock", 25.0),
            ],
        };
        let mut columns = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                columns.push(if (x, y) == (1, 1) { Some(centre.clone()) } else { neighbour(x, y) });
            }
        }
        let grid = ColumnGrid::new(0, 0, 3, 3, columns).unwrap();
        let cutoffs = spike_cutoffs(&grid, "IronOre", "Bedrock");
        // raise = median(10 x8) - 5 - 4 = 1; cutoff = 8 + 1 = 9.
        assert_eq!(cutoffs[4], Some(9.0));
        assert!(cutoffs.iter().enumerate().all(|(i, c)| i == 4 || c.is_none()));
        // The seam [8,12] shows only 3 of its 4 tiles above 9.
        assert_eq!(visible_target_thickness(&centre, "IronOre", "Bedrock", Some(9.0)), 3.0);
        assert_eq!(visible_target_thickness(&centre, "IronOre", "Bedrock", None), 4.0);
    }

    #[test]
    fn dig_depth_bottom_density_stops_at_first_failure() {
        let intervals = [
            Interval { top: 10.0, bottom: 8.0, thickness: 2.0, visible: 2.0 },
            Interval { top: 6.0, bottom: 3.0, thickness: 3.0, visible: 3.0 },
            Interval { top: 3.0, bottom: 1.0, thickness: 2.0, visible: 2.0 },
        ];
        // Second interval density: 3 / ((8-6) + 3) = 0.6; the
        // third is contiguous (density 2 / ((3-3) + 2) = 1.0),
        // so at 0.5 the walk runs to the deepest bottom.
        assert_eq!(dig_depth(&intervals, 0.5), Some(1.0));
        // At 0.75 the second interval fails and the walk stops —
        // the third (density 1.0 alone) is never reached.
        assert_eq!(dig_depth(&intervals, 0.75), Some(8.0));
        assert_eq!(dig_depth(&intervals, 0.0), Some(1.0));
        assert_eq!(dig_depth(&[], 0.5), None);
    }

    #[test]
    fn origin_evaluation_applies_purity_gates() {
        // 4x4 identical columns: ore seam 2 over waste 3 over bedrock.
        let col = |x: i64, y: i64| {
            Some(Column {
                x,
                y,
                surface: 10.0,
                layers: vec![
                    layer("IronOre", 2.0),
                    layer("Dirt", 3.0),
                    layer("Bedrock", 20.0),
                ],
            })
        };
        let mut columns = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                columns.push(col(x, y));
            }
        }
        let grid = ColumnGrid::new(0, 0, 4, 4, columns).unwrap();
        let cutoffs = spike_cutoffs(&grid, "IronOre", "Bedrock");
        assert!(cutoffs.iter().all(|c| c.is_none()));
        let ev = evaluate_origin(&grid, &cutoffs, (0, 0), "IronOre", "Bedrock", &purity_gate(1));
        assert_eq!(ev.ore_height, 32.0);
        assert_eq!(ev.total, 80.0);
        assert_eq!(ev.purity, 0.4);
        assert_eq!(ev.depth, Some(8.0));
        assert!(ev.passes_gates);
        let ev4 = evaluate_origin(&grid, &cutoffs, (0, 0), "IronOre", "Bedrock", &purity_gate(4));
        assert!(!ev4.passes_gates); // purity 0.4 < 0.75
        assert_eq!(origin_surface_min(&grid, (0, 0)), Some(10.0));
        assert_eq!(apply_depth_caps(8.0, 10.0, Some(30.0), None), 8.0);
        assert_eq!(apply_depth_caps(-40.0, 10.0, Some(30.0), None), -20.0);
        assert_eq!(apply_depth_caps(-40.0, 10.0, Some(30.0), Some(-10.0)), -10.0);
        assert_eq!(apply_depth_caps(-40.0, 10.0, None, None), -40.0);
    }

    #[test]
    fn two_components_pick_their_own_median_floors() {
        let a = [(0, 0), (4, 0), (8, 0), (12, 0), (16, 0)];
        let b = [(40, 0), (44, 0), (48, 0), (52, 0)];
        let all: Vec<(i64, i64)> = a.iter().chain(b.iter()).copied().collect();
        let comps = components(&all);
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0], a.to_vec());
        assert_eq!(comps[1], b.to_vec());
        // Depths as elevations in origin order.
        let depths_a = [18.0, 16.0, 14.0, 12.0, 10.0];
        let depths_b = [26.0, 24.0, 22.0, 20.0];
        assert_eq!(component_floor(&depths_a, 5), Some(14.0));
        assert_eq!(component_floor(&depths_b, 5), Some(24.0));
        // Strength extremes: 10 = deepest, 1 = shallow end.
        assert_eq!(component_floor(&depths_a, 10), Some(10.0));
        assert_eq!(component_floor(&depths_a, 1), Some(18.0));
        // Lower-only flattening pulls the shallow side down.
        assert_eq!(flatten_depths(&depths_a, 5, false), vec![14.0, 14.0, 14.0, 12.0, 10.0]);
        assert_eq!(flatten_depths(&depths_a, 5, true), vec![14.0; 5]);
        // Components under 4 tiles are never flattened.
        assert_eq!(flatten_depths(&[3.0, 9.0], 5, false), vec![3.0, 9.0]);
    }

    #[test]
    fn component_size_filter_keeps_largest_and_saves_on_ore() {
        let a = [(0, 0), (4, 0), (8, 0), (12, 0), (16, 0)];
        let c = [(40, 0), (44, 0)];
        let all: Vec<(i64, i64)> = a.iter().chain(c.iter()).copied().collect();
        let comps = components(&all);
        assert_eq!(comps.len(), 2);
        // L2 (min size 6): the 2-cell component goes regardless.
        let kept = filter_components(comps.clone(), &|_| 100.0, &purity_gate(2));
        assert_eq!(kept.len(), 1);
        // L1 (min size 2): size passes, but no ore anywhere ->
        // removed; one origin at/above the ore gate saves it.
        let kept = filter_components(comps.clone(), &|_| 0.0, &purity_gate(1));
        assert_eq!(kept.len(), 1);
        let kept = filter_components(comps, &|o| if o.0 >= 40 { 0.5 } else { 0.0 }, &purity_gate(1));
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn corner_smoothing_honours_max_height_diff() {
        // Two side-by-side origins with floors 10 and 14: the
        // shared vertices start at the deepest (10), the far
        // vertices of the second cell at 14, and smoothing must
        // lower them to 11 (= 10 + 1) — never raise anything.
        let floors = [(0, 0, 10), (4, 0, 14)];
        let (grid, converged) = build_corners(&floors, CornerInit::Deepest, 1);
        assert!(converged);
        assert_eq!(grid.at(0, 0), Some(10));
        assert_eq!(grid.at(4, 0), Some(10));
        assert_eq!(grid.at(8, 0), Some(11));
        assert_eq!(grid.at(8, 4), Some(11));
        let cells = cells_from_corners(&floors, &grid);
        assert_eq!(
            cells,
            vec![Cell::flat(0, 0, 10), Cell::new(4, 0, [10, 11, 11, 10])]
        );
    }

    // ---- read-back + verdict diff ----

    fn existing_json() -> Value {
        serde_json::json!({"designations": [
            {"kind": "mining", "proto": "MiningDesignator",
             "origin": [20, 12], "area": {"minX": 20, "minY": 12, "maxXExcl": 24, "maxYExcl": 16},
             "sizeTiles": 4, "minTargetHeight": 6, "maxTargetHeight": 9,
             "corners": [6, 9, 9, 6], "centerRaw": 491520,
             "isFulfilled": false, "jobsAssigned": 3},
            {"kind": "dumping", "proto": "DumpingDesignator",
             "origin": [68, 28], "area": {"minX": 68, "minY": 28, "maxXExcl": 72, "maxYExcl": 32},
             "sizeTiles": 4, "minTargetHeight": 8, "maxTargetHeight": 8,
             "isFulfilled": true, "jobsAssigned": 0}
        ]})
    }

    #[test]
    fn readback_parses_066_and_065_shapes() {
        let rows = existing_from_json(&existing_json());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].corners, Some([6, 9, 9, 6]));
        assert_eq!(rows[0].band, Some((6, 9)));
        // The pre-0.6.6 row (no corners) still parses, band only.
        assert_eq!(rows[1].corners, None);
        assert_eq!(rows[1].band, Some((8, 8)));
        assert!(existing_from_json(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn verdict_exact_band_different_absent() {
        let rows = existing_from_json(&existing_json());
        // Exact: same origin + proto + corners (0.6.6 evidence).
        assert_eq!(
            cell_match(&Cell::new(20, 12, [6, 9, 9, 6]), PROTO_MINING, &rows),
            CellMatch::Exact
        );
        // Different corners at the same origin+proto.
        assert_eq!(
            cell_match(&Cell::new(20, 12, [6, 6, 6, 6]), PROTO_MINING, &rows),
            CellMatch::Different
        );
        // Different proto at the same origin.
        assert_eq!(
            cell_match(&Cell::new(20, 12, [6, 9, 9, 6]), PROTO_DUMPING, &rows),
            CellMatch::Different
        );
        // Band-only: the flat dumping cell matches its band on a
        // corners-less row — a partial verdict, not exact.
        assert_eq!(
            cell_match(&Cell::flat(68, 28, 8), PROTO_DUMPING, &rows),
            CellMatch::BandOnly
        );
        // Nothing there.
        assert_eq!(
            cell_match(&Cell::flat(200, 200, 8), PROTO_MINING, &rows),
            CellMatch::Absent
        );
        let d = diff_add(
            &[
                Cell::new(20, 12, [6, 9, 9, 6]),
                Cell::flat(68, 28, 8),
                Cell::flat(200, 200, 8),
            ],
            PROTO_MINING,
            &rows,
        );
        // Cell 2 is planned as MINING here but reads back as a
        // dumping designation -> Different, not BandOnly.
        assert_eq!(d, AddDiff { exact: 1, band_only: 0, different: 1, absent: 1 });
        assert!(!d.all_landed());
    }

    #[test]
    fn verdict_remove_counts_still_present() {
        let rows = existing_from_json(&existing_json());
        assert_eq!(diff_remove(&[(20, 12), (400, 400)], &rows), 1);
        assert_eq!(diff_remove(&[(400, 400)], &rows), 0);
        assert_eq!(diff_remove(&[], &rows), 0);
    }
}
