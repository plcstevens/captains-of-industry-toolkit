//! Terrain sensing (CoiBridge v0.6.5, recon round 13): the
//! earthworks estimator plus the region / deposit JSON shapes the
//! playing layer consumes.
//!
//! The bridge serves terrain heights as raw Fix32 ints
//! (65536 = 1.0) in TILE units. The game's own constants
//! (TerrainTile): a tile is 2 m x 2 m, one height unit is 2 m,
//! one tile-cubed is 8 m3. Terrain (x, y) is the same grid as
//! entity (x, y) — an entity's Tile3i.Z is the floored terrain
//! height — so no coordinate transform exists anywhere in here.
//!
//! The estimator is pure: heights in tiles, target in tiles,
//! volumes out in tiles-cubed AND cubic metres (x8, the game's
//! TILE_VOLUME_M). There is no per-material yield in an
//! earthworks number — cut volume is material-agnostic dirt/rock
//! movement; what a cut YIELDS is the deposits view's business
//! (the bridge computes it from the layer model with the game's
//! own conversion).

use serde_json::Value;

use crate::{get_json, make_client, Config};

/// Fix32 raw units per 1.0 tile of height (bridge payload basis).
pub const RAW_PER_TILE: f64 = 65536.0;

/// Cubic metres in one tile-cubed (TerrainTile.TILE_VOLUME_M).
pub const TILE_VOLUME_M3: f64 = 8.0;

/// Earthworks for flattening a footprint to `target` (all heights
/// in tiles): cut = material above target (to mine away), fill =
/// void below target (to dump). Volumes in tiles-cubed; the m3
/// views multiply by the game's TILE_VOLUME_M.
#[derive(Debug, Clone, PartialEq)]
pub struct Earthworks {
    pub tiles: usize,
    pub tiles_above: usize,
    pub tiles_below: usize,
    pub tiles_at: usize,
    pub cut_tiles3: f64,
    pub fill_tiles3: f64,
}

impl Earthworks {
    pub fn cut_m3(&self) -> f64 {
        self.cut_tiles3 * TILE_VOLUME_M3
    }
    pub fn fill_m3(&self) -> f64 {
        self.fill_tiles3 * TILE_VOLUME_M3
    }
    /// The estimator result as the documented JSON shape the
    /// playing layer consumes (see SPEC Addendum XIX).
    pub fn to_json(&self, target: f64) -> Value {
        serde_json::json!({
            "targetTiles": target,
            "tiles": self.tiles,
            "tilesAbove": self.tiles_above,
            "tilesBelow": self.tiles_below,
            "tilesAt": self.tiles_at,
            "cutTilesCubed": round3(self.cut_tiles3),
            "fillTilesCubed": round3(self.fill_tiles3),
            "cutCubicMeters": round1(self.cut_m3()),
            "fillCubicMeters": round1(self.fill_m3()),
        })
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Sum of height deltas over the footprint. A tile exactly at
/// target counts as `tiles_at` and contributes nothing.
pub fn estimate(heights: &[f64], target: f64) -> Earthworks {
    let mut ew = Earthworks {
        tiles: heights.len(),
        tiles_above: 0,
        tiles_below: 0,
        tiles_at: 0,
        cut_tiles3: 0.0,
        fill_tiles3: 0.0,
    };
    for &hgt in heights {
        let d = hgt - target;
        if d > 0.0 {
            ew.tiles_above += 1;
            ew.cut_tiles3 += d;
        } else if d < 0.0 {
            ew.tiles_below += 1;
            ew.fill_tiles3 += -d;
        } else {
            ew.tiles_at += 1;
        }
    }
    ew
}

/// An exact-height region as served by
/// `GET /coi/v1/terrain/region`: bounds (x1/y1 exclusive),
/// whether the bridge shrank the request to its area cap, and
/// the heights in tiles, row-major (y * width + x — the game's
/// own index order).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionGrid {
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
    pub truncated: bool,
    pub heights: Vec<f64>,
}

impl RegionGrid {
    pub fn width(&self) -> usize {
        (self.x1 - self.x0).max(0) as usize
    }
    pub fn height(&self) -> usize {
        (self.y1 - self.y0).max(0) as usize
    }
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "x0": self.x0,
            "y0": self.y0,
            "x1": self.x1,
            "y1": self.y1,
            "width": self.width(),
            "height": self.height(),
            "truncated": self.truncated,
        })
    }
}

/// Parse a bridge region payload. Heights arrive as raw Fix32
/// ints and are converted to tiles here — the ONLY conversion
/// point, so every consumer sees tiles.
pub fn region_from_json(v: &Value) -> Option<RegionGrid> {
    let raw = v["heightsRaw"].as_array()?;
    let mut heights = Vec::with_capacity(raw.len());
    for hgt in raw {
        heights.push(hgt.as_i64()? as f64 / RAW_PER_TILE);
    }
    Some(RegionGrid {
        x0: v["x0"].as_i64()?,
        y0: v["y0"].as_i64()?,
        x1: v["x1"].as_i64()?,
        y1: v["y1"].as_i64()?,
        truncated: v["truncated"].as_bool().unwrap_or(false),
        heights,
    })
}

/// `coi terrain` — the debug-shell face of the terrain routes
/// (the website's Terrain page is the product surface):
///   coi terrain deposits                       deposits JSON
///   coi terrain region X0 Y0 X1 Y1             region JSON
///   coi terrain estimate X0 Y0 X1 Y1 TARGET    estimator JSON
/// TARGET is a height in tiles (fractions allowed, e.g. 12.5).
pub fn cmd(cfg: &Config, args: &[String]) -> i32 {
    let c = make_client(cfg);
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    match sub {
        "deposits" => match get_json(&c, "/coi/v1/deposits") {
            Ok(v) => {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
                0
            }
            Err(e) => {
                eprintln!("terrain deposits: {e}");
                1
            }
        },
        "region" | "estimate" => {
            let nums: Option<Vec<f64>> = args
                .get(1..)
                .map(|rest| {
                    rest.iter()
                        .map(|s| s.parse::<f64>())
                        .collect::<Result<_, _>>()
                        .ok()
                })
                .unwrap_or(None);
            let want = if sub == "region" { 4 } else { 5 };
            let Some(nums) = nums.filter(|n| n.len() == want) else {
                eprintln!(
                    "usage: coi terrain {sub} X0 Y0 X1 Y1{}",
                    if sub == "estimate" { " TARGET" } else { "" }
                );
                return 2;
            };
            let path = format!(
                "/coi/v1/terrain/region?x0={}&y0={}&x1={}&y1={}",
                nums[0] as i64, nums[1] as i64, nums[2] as i64, nums[3] as i64
            );
            let v = match get_json(&c, &path) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("terrain {sub}: {e}");
                    return 1;
                }
            };
            let Some(region) = region_from_json(&v) else {
                eprintln!(
                    "terrain {sub}: bridge returned no usable region (CoiBridge 0.6.5+ required)"
                );
                return 1;
            };
            if sub == "region" {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
                return 0;
            }
            let ew = estimate(&region.heights, nums[4]);
            let mut out = ew.to_json(nums[4]);
            out["region"] = region.to_json();
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            0
        }
        _ => {
            eprintln!(
                "usage: coi terrain deposits | region X0 Y0 X1 Y1 | estimate X0 Y0 X1 Y1 TARGET"
            );
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_at_target_is_zero_work() {
        let heights = vec![5.0; 100];
        let ew = estimate(&heights, 5.0);
        assert_eq!(ew.tiles, 100);
        assert_eq!(ew.tiles_at, 100);
        assert_eq!(ew.cut_tiles3, 0.0);
        assert_eq!(ew.fill_tiles3, 0.0);
    }

    #[test]
    fn flat_above_target_is_all_fill() {
        // Pad at 5, target 3: every tile needs 2 tiles of fill.
        let heights = vec![5.0; 100];
        let ew = estimate(&heights, 7.0);
        assert_eq!(ew.tiles_below, 100);
        assert_eq!(ew.fill_tiles3, 200.0);
        assert_eq!(ew.fill_m3(), 1600.0);
        assert_eq!(ew.cut_tiles3, 0.0);
    }

    #[test]
    fn ramp_splits_cut_and_fill() {
        // One row 0..=9, target 4.5: cut = .5+1.5+2.5+3.5+4.5,
        // fill = 4.5+3.5+2.5+1.5+.5 — symmetric, 12.5 each.
        let heights: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let ew = estimate(&heights, 4.5);
        assert_eq!(ew.tiles_above, 5);
        assert_eq!(ew.tiles_below, 5);
        assert_eq!(ew.cut_tiles3, 12.5);
        assert_eq!(ew.fill_tiles3, 12.5);
        assert_eq!(ew.cut_m3(), 100.0);
    }

    #[test]
    fn peak_is_pure_cut() {
        let mut heights = vec![2.0; 9];
        heights[4] = 12.0;
        let ew = estimate(&heights, 2.0);
        assert_eq!(ew.tiles_above, 1);
        assert_eq!(ew.tiles_at, 8);
        assert_eq!(ew.cut_tiles3, 10.0);
        assert_eq!(ew.fill_tiles3, 0.0);
    }

    #[test]
    fn fractional_heights_convert_from_raw() {
        // 5.5 tiles = raw 360448; the region parser is the only
        // raw->tiles conversion point.
        let v = serde_json::json!({
            "x0": 10, "y0": 20, "x1": 11, "y1": 21,
            "truncated": false,
            "heightsRaw": [360448],
        });
        let r = region_from_json(&v).unwrap();
        assert_eq!(r.width(), 1);
        assert_eq!(r.height(), 1);
        assert_eq!(r.heights, vec![5.5]);
    }
}
