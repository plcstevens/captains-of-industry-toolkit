//! Productivity evidence — QOL-MOD-PREP §5, κ-free forms only.
//!
//! Two estimators, both shipped WITHOUT κ calibration (κ maps game
//! monthly flows to dataset per-minute nameplates; the two disagree
//! by ~5 orders of magnitude under naive conversion, so any absolute
//! "percent of nameplate" would be fabricated — recon §5.4):
//!
//! (a) Machine-state estimator over persisted per-snapshot state
//!     aggregates (store schema v2). Working share of eligible
//!     (enabled, not Paused) machine-samples over a trailing 30-day
//!     window, with Wilson-CI firing and cause separation from the
//!     state mix. This is the estimator that can say *starved* vs
//!     *blocked* vs *parked*.
//! (b) Self-relative flow estimator: a product's finished-month
//!     production against its own best finished month in the trailing
//!     window. No nameplate is involved, so no κ is needed. It can
//!     say "output fell"; it can NEVER name a cause.
//!
//! Evidence discipline (recon §5.7, mandatory): every verdict carries
//! a label — estimate class (MEASURED / ESTIMATED / INSUFFICIENT for
//! (a); FLOW-EXACT for (b)'s exact game-aggregate numerators), window,
//! N_eff, the estimate and its CI or U value, the cause, and the
//! denominator. Below the ESTIMATED line the verdict is INSUFFICIENT
//! and the proto is *listed as unjudged* — silence, never "healthy".
//! High productivity is a digest note ("running hot"), never an
//! alert. Firing uses hysteresis: fire below the line, clear above a
//! strictly higher line, and the band between persists the prior
//! state; a same-cause re-fire inside one month is suppressed.
//!
//! Year-window classes from the prep (same N_eff gates against a
//! 360-day span) are deliberately NOT implemented yet: the store has
//! no year of cadence to judge, and the month window is the prep's
//! prerequisite form. Named here so the gap is explicit.

use serde_json::Value;

use crate::store::Store;
use crate::{importance, is_seasonal, Issue, Live};

pub const TICKS_PER_DAY: i64 = 2_880;
pub const TICKS_PER_MONTH: i64 = 86_400;
/// Trailing window for estimator (a): one in-game month (30 days).
pub const WINDOW_TICKS: i64 = TICKS_PER_MONTH;

// ---------------------------------------------------------------------------
// Thresholds (config: [productivity] in coi.toml; defaults = prep §5)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Thresholds {
    /// Fire when the Wilson 95% CI upper bound is below this.
    pub fire_ci_upper: f64,
    /// …and the point estimate is below this.
    pub fire_point: f64,
    /// Severe fire: point estimate below this…
    pub severe_point: f64,
    /// …with CI upper below this.
    pub severe_ci_upper: f64,
    /// Clear when a later window's CI lower bound exceeds this.
    pub clear_ci_lower: f64,
    /// Confirmation leg (ii): the window's second half independently
    /// has CI upper below this.
    pub confirm_second_half_ci_upper: f64,
    /// Paused share at/above which a proto is PARKED, not an alert.
    pub parked_share: f64,
    pub starved_share: f64,
    pub blocked_share: f64,
    pub mixed_share: f64,
    pub other_share: f64,
    /// Population gate: minimum instances…
    pub min_instances: i64,
    /// …at this share of counted snapshots.
    pub population_share: f64,
    pub measured_min_samples: i64,
    pub estimated_min_samples: i64,
    pub measured_span_days: f64,
    pub estimated_span_days: f64,
    pub measured_max_gap_days: f64,
    pub estimated_max_gap_days: f64,
    /// Anti-aliasing: a snapshot counts only if at least this many
    /// ticks after the last counted one (6 in-game hours).
    pub thin_ticks: i64,
    /// No re-fire for the same (target, cause) within this span.
    pub cooldown_ticks: i64,
    /// Ramp exemption: instance growth factor over the window.
    pub ramp_growth: f64,
    /// (b) fire: U below this for 2 consecutive finished months…
    pub b_fire_u: f64,
    /// …or below this in a single finished month.
    pub b_fire_u_single: f64,
    /// (b) clear: U above this for a finished month.
    pub b_clear_u: f64,
    /// Working share at/above which a proto is "running hot" (note).
    pub hot_share: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            fire_ci_upper: 0.50,
            fire_point: 0.45,
            severe_point: 0.25,
            severe_ci_upper: 0.35,
            clear_ci_lower: 0.60,
            confirm_second_half_ci_upper: 0.55,
            parked_share: 0.50,
            starved_share: 0.30,
            blocked_share: 0.30,
            mixed_share: 0.20,
            other_share: 0.20,
            min_instances: 3,
            population_share: 0.80,
            measured_min_samples: 96,
            estimated_min_samples: 30,
            measured_span_days: 27.0,
            estimated_span_days: 24.0,
            measured_max_gap_days: 3.0,
            estimated_max_gap_days: 7.0,
            thin_ticks: 720,
            cooldown_ticks: TICKS_PER_MONTH,
            ramp_growth: 1.5,
            b_fire_u: 0.50,
            b_fire_u_single: 0.35,
            b_clear_u: 0.65,
            hot_share: 0.90,
        }
    }
}

impl Thresholds {
    /// Load overrides from the `[productivity]` section of coi.toml.
    /// Search order: $COI_CONFIG, ./coi.toml, coi.toml next to the db.
    /// The parser is intentionally minimal (flat `key = value` lines,
    /// `#` comments) so the companion gains no new dependency for a
    /// dozen numbers; unknown keys are ignored, unparseable values
    /// keep the prep defaults.
    pub fn load(db_path: &str) -> Thresholds {
        let mut th = Thresholds::default();
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(p) = std::env::var("COI_CONFIG") {
            candidates.push(p.into());
        }
        candidates.push("coi.toml".into());
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() {
                candidates.push(parent.join("coi.toml"));
            }
        }
        for path in candidates {
            if let Ok(text) = std::fs::read_to_string(&path) {
                th.apply_toml(&text);
                break;
            }
        }
        th
    }

    fn apply_toml(&mut self, text: &str) {
        let mut in_section = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.starts_with('[') {
                in_section = line == "[productivity]";
                continue;
            }
            if !in_section {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let (key, val) = (key.trim(), val.trim());
            let num: Option<f64> = val.parse().ok();
            let int: Option<i64> = val.parse().ok();
            match key {
                "fire_ci_upper" => self.fire_ci_upper = num.unwrap_or(self.fire_ci_upper),
                "fire_point" => self.fire_point = num.unwrap_or(self.fire_point),
                "severe_point" => self.severe_point = num.unwrap_or(self.severe_point),
                "severe_ci_upper" => self.severe_ci_upper = num.unwrap_or(self.severe_ci_upper),
                "clear_ci_lower" => self.clear_ci_lower = num.unwrap_or(self.clear_ci_lower),
                "confirm_second_half_ci_upper" => {
                    self.confirm_second_half_ci_upper =
                        num.unwrap_or(self.confirm_second_half_ci_upper)
                }
                "parked_share" => self.parked_share = num.unwrap_or(self.parked_share),
                "starved_share" => self.starved_share = num.unwrap_or(self.starved_share),
                "blocked_share" => self.blocked_share = num.unwrap_or(self.blocked_share),
                "mixed_share" => self.mixed_share = num.unwrap_or(self.mixed_share),
                "other_share" => self.other_share = num.unwrap_or(self.other_share),
                "min_instances" => self.min_instances = int.unwrap_or(self.min_instances),
                "population_share" => self.population_share = num.unwrap_or(self.population_share),
                "measured_min_samples" => {
                    self.measured_min_samples = int.unwrap_or(self.measured_min_samples)
                }
                "estimated_min_samples" => {
                    self.estimated_min_samples = int.unwrap_or(self.estimated_min_samples)
                }
                "measured_span_days" => {
                    self.measured_span_days = num.unwrap_or(self.measured_span_days)
                }
                "estimated_span_days" => {
                    self.estimated_span_days = num.unwrap_or(self.estimated_span_days)
                }
                "measured_max_gap_days" => {
                    self.measured_max_gap_days = num.unwrap_or(self.measured_max_gap_days)
                }
                "estimated_max_gap_days" => {
                    self.estimated_max_gap_days = num.unwrap_or(self.estimated_max_gap_days)
                }
                "thin_ticks" => self.thin_ticks = int.unwrap_or(self.thin_ticks),
                "cooldown_ticks" => self.cooldown_ticks = int.unwrap_or(self.cooldown_ticks),
                "ramp_growth" => self.ramp_growth = num.unwrap_or(self.ramp_growth),
                "b_fire_u" => self.b_fire_u = num.unwrap_or(self.b_fire_u),
                "b_fire_u_single" => self.b_fire_u_single = num.unwrap_or(self.b_fire_u_single),
                "b_clear_u" => self.b_clear_u = num.unwrap_or(self.b_clear_u),
                "hot_share" => self.hot_share = num.unwrap_or(self.hot_share),
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Statistics helpers
// ---------------------------------------------------------------------------

/// Wilson score interval (95%, z = 1.96) for `successes` out of `n`.
/// Returns (lower, upper); (0, 1) for an empty sample.
pub fn wilson(successes: i64, n: i64) -> (f64, f64) {
    if n <= 0 {
        return (0.0, 1.0);
    }
    let z = 1.96f64;
    let n = n as f64;
    let p = successes as f64 / n;
    let d = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / d;
    let half = (z / d) * ((p * (1.0 - p) / n) + (z * z / (4.0 * n * n))).sqrt();
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// Anti-aliasing thinning (prep §5(d)3): from ascending ticks, keep a
/// tick only if it is at least `thin_ticks` after the last kept one.
pub fn thin(ticks: &[i64], thin_ticks: i64) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    for &t in ticks {
        match out.last() {
            Some(&last) if t - last < thin_ticks => {}
            _ => out.push(t),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Estimator (a): machine working share per proto
// ---------------------------------------------------------------------------

/// One stored machine-state row: (tick, working, not_enough_input,
/// output_full, paused, other, instances).
type AggRow = (i64, i64, i64, i64, i64, i64, i64);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Verdict {
    Fire { severe: bool },
    Clear,
    DeadZone,
    Parked,
    Ramp,
    Hot,
    Insufficient,
}

pub struct ProtoEval {
    pub proto: String,
    pub class: &'static str, // MEASURED | ESTIMATED | INSUFFICIENT
    pub p_hat: Option<f64>,
    pub cause: Option<&'static str>,
    pub parked_share: f64,
    pub verdict: Verdict,
    /// The mandatory evidence label (recon §5.7 field order); it
    /// carries N_eff, span, max gap and the CI, so they are not
    /// duplicated as struct fields.
    pub label: String,
    /// For INSUFFICIENT: what is missing. Empty otherwise.
    pub missing: String,
}

fn class_of(n_eff: i64, span_days: f64, max_gap_days: f64, th: &Thresholds) -> &'static str {
    if n_eff >= th.measured_min_samples
        && span_days >= th.measured_span_days
        && max_gap_days <= th.measured_max_gap_days
    {
        "MEASURED"
    } else if n_eff >= th.estimated_min_samples
        && span_days >= th.estimated_span_days
        && max_gap_days <= th.estimated_max_gap_days
    {
        "ESTIMATED"
    } else {
        "INSUFFICIENT"
    }
}

/// Evaluate estimator (a) for every proto in `series` (rows keyed by
/// proto), over the trailing window ending at `window_end`.
/// `all_ticks` is the game's full snapshot tick series (thinning is
/// computed over the whole series, then restricted to the window, so
/// a snapshot just before the window still thins the first one in).
pub fn eval_protos(
    series: &std::collections::BTreeMap<String, Vec<AggRow>>,
    all_ticks: &[i64],
    window_end: i64,
    th: &Thresholds,
) -> Vec<ProtoEval> {
    let window_start = window_end - WINDOW_TICKS;
    let counted_all = thin(all_ticks, th.thin_ticks);
    let counted: Vec<i64> = counted_all
        .into_iter()
        .filter(|t| *t > window_start && *t <= window_end)
        .collect();
    let mid = window_start + WINDOW_TICKS / 2;
    let mut out = Vec::new();

    for (proto, rows) in series {
        let by_tick: std::collections::HashMap<i64, &AggRow> =
            rows.iter().map(|r| (r.0, r)).collect();
        let first_seen = rows.iter().map(|r| r.0).min().unwrap_or(window_end);

        // Population gate + pooled sums over counted ticks.
        let mut gate_hits = 0usize;
        let (mut w, mut nei, mut of, mut oth, mut elig, mut paused, mut inst) =
            (0i64, 0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
        let (mut w2, mut e2) = (0i64, 0i64); // second-half confirmation pool
        let mut present_ticks: Vec<i64> = Vec::new();
        let mut first_inst: Option<i64> = None;
        let mut last_inst: i64 = 0;
        for &t in &counted {
            match by_tick.get(&t) {
                Some(r) => {
                    present_ticks.push(t);
                    if r.6 >= th.min_instances {
                        gate_hits += 1;
                    }
                    if first_inst.is_none() {
                        first_inst = Some(r.6);
                    }
                    last_inst = r.6;
                    w += r.1;
                    nei += r.2;
                    of += r.3;
                    paused += r.4;
                    oth += r.5;
                    inst += r.6;
                    elig += r.6 - r.4;
                    if t > mid {
                        w2 += r.1;
                        e2 += r.6 - r.4;
                    }
                }
                None => {
                    // Absent from a counted snapshot = 0 instances:
                    // counts against the population gate only.
                }
            }
        }
        let span_days = match (present_ticks.first(), present_ticks.last()) {
            (Some(&a), Some(&b)) if present_ticks.len() > 1 => (b - a) as f64 / TICKS_PER_DAY as f64,
            _ => 0.0,
        };
        let max_gap_days = present_ticks
            .windows(2)
            .map(|p| (p[1] - p[0]) as f64 / TICKS_PER_DAY as f64)
            .fold(0.0f64, f64::max);
        let class = class_of(elig, span_days, max_gap_days, th);
        let parked_share = if inst > 0 {
            paused as f64 / inst as f64
        } else {
            0.0
        };
        let gate_ok = !counted.is_empty()
            && (gate_hits as f64 / counted.len() as f64) >= th.population_share;

        let (p_hat, ci) = if elig > 0 {
            let p = w as f64 / elig as f64;
            (Some(p), Some(wilson(w, elig)))
        } else {
            (None, None)
        };

        // Cause separation from the state mix (eligible denominator).
        let cause: Option<&'static str> = if elig > 0 {
            let s_nei = nei as f64 / elig as f64;
            let s_of = of as f64 / elig as f64;
            let s_oth = oth as f64 / elig as f64;
            if s_nei >= th.starved_share && s_nei >= s_of {
                Some("starved")
            } else if s_of >= th.blocked_share {
                Some("blocked")
            } else if s_nei >= th.mixed_share && s_of >= th.mixed_share {
                Some("mixed")
            } else if s_oth >= th.other_share {
                Some("unclassified")
            } else {
                Some("idle")
            }
        } else {
            None
        };

        let ramp = first_seen > window_end - TICKS_PER_MONTH
            || match first_inst {
                Some(f) if f > 0 => (last_inst as f64) > th.ramp_growth * f as f64,
                _ => false,
            };

        let mut missing = String::new();
        let verdict = if !gate_ok {
            missing = format!(
                "population gate: needs ≥{} instances at ≥{:.0}% of counted snapshots (gate met at {}/{})",
                th.min_instances,
                th.population_share * 100.0,
                gate_hits,
                counted.len()
            );
            Verdict::Insufficient
        } else if parked_share >= th.parked_share {
            // Parked-ness is a state observation, not a productivity
            // judgement: it does not need eligible samples, so it is
            // checked before the evidence-class gate (a fully paused
            // proto has N_eff = 0 by construction).
            Verdict::Parked
        } else if class == "INSUFFICIENT" {
            missing = format!(
                "needs N_eff≥{} (have {}), span≥{:.0}d (have {:.1}d), max_gap≤{:.0}d (have {:.1}d) for ESTIMATED",
                th.estimated_min_samples, elig, th.estimated_span_days, span_days,
                th.estimated_max_gap_days, max_gap_days
            );
            Verdict::Insufficient
        } else if ramp {
            Verdict::Ramp
        } else {
            let (p, (lo, hi)) = (p_hat.unwrap_or(1.0), ci.unwrap_or((0.0, 1.0)));
            if p >= th.hot_share {
                Verdict::Hot
            } else if hi < th.fire_ci_upper && p < th.fire_point {
                // Confirmation leg (ii): second half independently low.
                let (_, hi2) = wilson(w2, e2);
                if e2 > 0 && hi2 < th.confirm_second_half_ci_upper {
                    Verdict::Fire {
                        severe: p < th.severe_point && hi < th.severe_ci_upper,
                    }
                } else {
                    Verdict::DeadZone // unconfirmed: persist prior state
                }
            } else if lo > th.clear_ci_lower {
                Verdict::Clear
            } else {
                Verdict::DeadZone
            }
        };

        let label = match (p_hat, ci) {
            (Some(p), Some((lo, hi))) => format!(
                "class={} est=a target={} window=30d N_eff={} span={:.1}d max_gap={:.1}d p̂={:.3} CI95=[{:.3}, {:.3}] cause={} denominator=eligible machine-samples (enabled, not Paused), pooled over {} counted snapshots",
                class,
                proto,
                elig,
                span_days,
                max_gap_days,
                p,
                lo,
                hi,
                cause.unwrap_or("n/a"),
                present_ticks.len()
            ),
            _ => format!(
                "class={} est=a target={} window=30d N_eff={} span={:.1}d max_gap={:.1}d cause=n/a denominator=eligible machine-samples (enabled, not Paused)",
                class, proto, elig, span_days, max_gap_days
            ),
        };

        out.push(ProtoEval {
            proto: proto.clone(),
            class,
            p_hat,
            cause,
            parked_share,
            verdict,
            label,
            missing,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Estimator (b): self-relative finished-month output per product
// ---------------------------------------------------------------------------

pub struct ProductEval {
    pub id: String,
    pub name: String,
    pub u_last: f64,
    pub verdict: Verdict,
    /// Evidence label; carries best-month, U and the window counts.
    pub label: String,
}

/// Pure core of (b): verdict from a finished-month production series
/// (oldest first, in-progress month already removed).
/// Returns (verdict, U_last, U_prev, best).
pub fn b_verdict(finished: &[i64], th: &Thresholds) -> (Verdict, f64, Option<f64>, i64) {
    let best = finished.iter().copied().max().unwrap_or(0);
    if finished.len() < 2 || best <= 0 {
        return (Verdict::Insufficient, 0.0, None, best);
    }
    let last = *finished.last().unwrap() as f64 / best as f64;
    let prev = finished
        .get(finished.len() - 2)
        .map(|v| *v as f64 / best as f64);
    let verdict = if last > th.b_clear_u {
        Verdict::Clear
    } else if last < th.b_fire_u_single || (last < th.b_fire_u && prev.map(|p| p < th.b_fire_u).unwrap_or(false)) {
        Verdict::Fire { severe: last < th.b_fire_u_single }
    } else {
        Verdict::DeadZone
    };
    (verdict, last, prev, best)
}

/// Evaluate estimator (b) over the live product set. Seasonal and
/// virtual products are exempt (prep §5, reusing the board's own
/// `is_seasonal` verbatim); products with no self-baseline (best
/// finished month = 0) are the stall detector's territory, not (b)'s.
pub fn eval_products(live: &Live, th: &Thresholds) -> Vec<ProductEval> {
    let mut out = Vec::new();
    let Some(map) = live.stats["products"].as_object() else {
        return out;
    };
    for (id, s) in map {
        if live.kinds.get(id).map(|k| k.as_str()) == Some("virtual") {
            continue;
        }
        if is_seasonal(live, id, s) {
            continue;
        }
        let name = live.names.get(id).cloned().unwrap_or_else(|| id.clone());
        let series: Vec<i64> = live.history["products"][id]["produced"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_i64().unwrap_or(0)).collect())
            .unwrap_or_default();
        // Last history point is the month in progress: finished only.
        let finished: Vec<i64> = series[..series.len().saturating_sub(1)].to_vec();
        let (verdict, u_last, _u_prev, best) = b_verdict(&finished, th);
        if best <= 0 {
            continue; // no self-baseline — never produced; not (b)'s call
        }
        let label = format!(
            "class=FLOW-EXACT est=b target={} window=finished-months N_eff={} span={}d U={:.2} best={}/mo cause=below-baseline denominator=self-relative (own best finished month in window); no nameplate used",
            id,
            finished.len(),
            finished.len() * 30,
            u_last,
            best
        );
        out.push(ProductEval {
            id: id.clone(),
            name,
            u_last,
            verdict,
            label,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Hysteresis state machine (shared by both estimators)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Transition {
    /// New alert (no prior fire, cause changed, or cooldown expired).
    EmitNew { since: i64 },
    /// Alert persists (fired before, same cause; or dead-zone hold).
    EmitOngoing { since: i64 },
    /// Same-cause re-fire inside the cooldown after a clear: swallow.
    Suppress,
    /// A fired alert's clear condition is now met.
    Cleared,
    /// Nothing to record or emit.
    None,
}

/// Prior state as stored: (status, cause, since_tick, value).
type Prior = (String, Option<String>, i64, Option<f64>);

pub fn transition(prior: Option<&Prior>, raw: Verdict, cause: Option<&str>, tick: i64, th: &Thresholds) -> Transition {
    match raw {
        Verdict::Fire { .. } => match prior {
            Some((status, pcause, since, _)) if status == "fired" => {
                if pcause.as_deref() == cause {
                    Transition::EmitOngoing { since: *since }
                } else {
                    // A cause change is a new alert (prep §5).
                    Transition::EmitNew { since: tick }
                }
            }
            Some((status, pcause, since, _)) if status == "cleared" => {
                if pcause.as_deref() == cause && tick - since < th.cooldown_ticks {
                    Transition::Suppress
                } else {
                    Transition::EmitNew { since: tick }
                }
            }
            _ => Transition::EmitNew { since: tick },
        },
        Verdict::Clear => match prior {
            Some((status, _, _, _)) if status == "fired" => Transition::Cleared,
            _ => Transition::None,
        },
        Verdict::DeadZone => match prior {
            Some((status, _, since, _)) if status == "fired" => {
                Transition::EmitOngoing { since: *since }
            }
            _ => Transition::None,
        },
        _ => Transition::None,
    }
}

// ---------------------------------------------------------------------------
// Snapshot-time evaluation (records state, returns issues + log lines)
// ---------------------------------------------------------------------------

fn cause_words(cause: &str) -> &'static str {
    match cause {
        "starved" => "input-starved",
        "blocked" => "output-blocked",
        "mixed" => "starved and blocked",
        "unclassified" => "idle on infrastructure/other states",
        "below-baseline" => "output below its own baseline",
        _ => "idle, cause undetermined",
    }
}

fn fix_for(cause: &str) -> &'static str {
    match cause {
        "starved" => "Input is the constraint: check the supply chain feeding these machines (upstream output, transport, buffers) before adding machines — more machines would starve too.",
        "blocked" => "Output is the constraint: downstream consumption or storage is full/slow. Clear the downstream blockage before adding machines.",
        "mixed" => "Both starvation and blocking are present. Relieve the output side first (usually the binding constraint), then re-check input supply.",
        "unclassified" => "Idle time is dominated by 'other' states (power, maintenance, upgrades), not input/output. Check power and upkeep — do not treat this as a production-rate problem.",
        "below-baseline" => "Output fell against the product's own best finished month. Flows alone cannot name the cause: check what changed (inputs, workers, power, demand) around the drop.",
        _ => "No dominant input/output cause in the state mix: the line may be oversized for current demand, or throttled by something the states don't separate. Check demand before expanding.",
    }
}

/// Evaluate both estimators for the snapshot being taken. `snap` is
/// the pending snapshot (not yet inserted); `issues` already holds
/// the flow-based board for this snapshot and is extended in place
/// ((b) evidence composes into an existing stall/deficit issue for
/// the same product instead of duplicating it). Returns log lines.
pub fn evaluate_snapshot(
    store: &mut Store,
    snap: &Value,
    live: &Live,
    issues: &mut Vec<Issue>,
    th: &Thresholds,
) -> Vec<String> {
    let mut log: Vec<String> = Vec::new();
    let game_id = snap["game"]["gameId"].as_str().unwrap_or("").to_string();
    let tick = snap["game"]["tick"].as_i64().unwrap_or(0);
    if game_id.is_empty() {
        return log;
    }

    // ---- estimator (a) over stored series + the pending snapshot ----
    let mut series: std::collections::BTreeMap<String, Vec<AggRow>> = Default::default();
    let mut all_ticks: Vec<i64> = Vec::new();
    match store.snapshot_ticks(&game_id) {
        Ok(t) => all_ticks = t,
        Err(e) => log.push(format!("productivity: tick series unreadable: {e}")),
    }
    if let Ok(rows) = store.machine_state_series(&game_id) {
        for (t, proto, w, n, o, p, ot, inst, _src) in rows {
            series.entry(proto).or_default().push((t, w, n, o, p, ot, inst));
        }
    }
    // Pending snapshot's aggregates (parsed exactly as insert will).
    for (proto, w, n, o, p, ot, inst, _src) in crate::store::snapshot_machine_rows(snap) {
        series.entry(proto).or_default().push((tick, w, n, o, p, ot, inst));
    }
    if !all_ticks.contains(&tick) {
        all_ticks.push(tick);
        all_ticks.sort_unstable();
    }
    for rows in series.values_mut() {
        rows.sort_by_key(|r| r.0);
    }

    for ev in eval_protos(&series, &all_ticks, tick, th) {
        let cause = ev.cause.unwrap_or("idle");
        match ev.verdict {
            Verdict::Fire { severe } => {
                let prior = store.prod_state_get(&game_id, &ev.proto).ok().flatten();
                match transition(prior.as_ref(), ev.verdict, Some(cause), tick, th) {
                    Transition::EmitNew { since } | Transition::EmitOngoing { since } => {
                        let _ = store.prod_state_put(
                            &game_id, &ev.proto, "proto", "fired", Some(cause), since,
                            ev.p_hat, &ev.label, tick,
                        );
                        let imp = importance(&ev.proto);
                        issues.push(Issue {
                            quadrant: if severe && imp == 3 {
                                "Fix now"
                            } else if severe || imp == 3 {
                                "Fix later"
                            } else {
                                "Watch"
                            },
                            product: ev.proto.clone(),
                            title: format!(
                                "Low machine productivity — {} ({})",
                                ev.proto,
                                cause_words(cause)
                            ),
                            evidence: ev.label.clone(),
                            fix: fix_for(cause).to_string(),
                            class: match cause {
                                "starved" => "prod-starved",
                                "blocked" => "prod-blocked",
                                "mixed" => "prod-mixed",
                                _ => "prod-unclassified",
                            },
                        });
                        log.push(format!("productivity (a): FIRE {} — {}", ev.proto, ev.label));
                    }
                    Transition::Suppress => {
                        log.push(format!(
                            "productivity (a): {} re-fire suppressed (cooldown, same cause)",
                            ev.proto
                        ));
                    }
                    _ => {}
                }
            }
            Verdict::Clear | Verdict::Hot => {
                // Both are "above the clear line" for hysteresis: a
                // proto running hot has unambiguously recovered.
                let prior = store.prod_state_get(&game_id, &ev.proto).ok().flatten();
                let cleared = matches!(
                    transition(prior.as_ref(), Verdict::Clear, Some(cause), tick, th),
                    Transition::Cleared
                );
                if cleared {
                    // Keep since_tick = the fire tick (cooldown anchor).
                    let since = prior.as_ref().map(|p| p.2).unwrap_or(tick);
                    let _ = store.prod_state_put(
                        &game_id, &ev.proto, "proto", "cleared", Some(cause), since,
                        ev.p_hat, &ev.label, tick,
                    );
                    log.push(format!("productivity (a): CLEAR {} — {}", ev.proto, ev.label));
                } else if matches!(ev.verdict, Verdict::Hot) {
                    log.push(format!(
                        "productivity (a): {} running hot (p̂={:.2}) — digest note, not an alert",
                        ev.proto,
                        ev.p_hat.unwrap_or(0.0)
                    ));
                }
            }
            Verdict::DeadZone => {
                let prior = store.prod_state_get(&game_id, &ev.proto).ok().flatten();
                if let Transition::EmitOngoing { since } =
                    transition(prior.as_ref(), ev.verdict, Some(cause), tick, th)
                {
                    let _ = store.prod_state_put(
                        &game_id, &ev.proto, "proto", "fired", Some(cause), since,
                        ev.p_hat, &ev.label, tick,
                    );
                    issues.push(Issue {
                        quadrant: "Watch",
                        product: ev.proto.clone(),
                        title: format!(
                            "Low machine productivity — {} ({}) — persisting (hysteresis dead zone)",
                            ev.proto,
                            cause_words(cause)
                        ),
                        evidence: ev.label.clone(),
                        fix: fix_for(cause).to_string(),
                            class: match cause {
                                "starved" => "prod-starved",
                                "blocked" => "prod-blocked",
                                "mixed" => "prod-mixed",
                                _ => "prod-unclassified",
                            },
                    });
                }
            }
            Verdict::Parked => log.push(format!(
                "productivity (a): {} PARKED (paused share {:.0}% ≥ {:.0}%) — not an alert",
                ev.proto,
                ev.parked_share * 100.0,
                th.parked_share * 100.0
            )),
            _ => {}
        }
    }

    // ---- estimator (b) over live finished-month flows ----------------
    for ev in eval_products(live, th) {
        match ev.verdict {
            Verdict::Fire { severe } => {
                let prior = store.prod_state_get(&game_id, &ev.id).ok().flatten();
                match transition(prior.as_ref(), ev.verdict, Some("below-baseline"), tick, th) {
                    Transition::EmitNew { since } | Transition::EmitOngoing { since } => {
                        let _ = store.prod_state_put(
                            &game_id, &ev.id, "product", "fired", Some("below-baseline"), since,
                            Some(ev.u_last), &ev.label, tick,
                        );
                        // Compose into an existing stall/deficit issue
                        // for the same product rather than duplicating.
                        let host = issues.iter_mut().find(|i| {
                            i.product == ev.name
                                && (i.title.contains("stalled")
                                    || i.title.to_lowercase().contains("deficit"))
                        });
                        match host {
                            Some(h) => {
                                h.evidence.push_str(&format!(
                                    " Productivity evidence: {}.",
                                    ev.label
                                ));
                                log.push(format!(
                                    "productivity (b): {} evidence composed into existing issue '{}'",
                                    ev.id, h.title
                                ));
                            }
                            None => {
                                let imp = importance(&ev.id);
                                issues.push(Issue {
                                    quadrant: if severe && imp == 3 {
                                        "Fix now"
                                    } else if severe || imp >= 2 {
                                        "Fix later"
                                    } else {
                                        "Watch"
                                    },
                                    product: ev.name.clone(),
                                    title: format!(
                                        "Output below its own baseline — {} at {:.0}% of its best finished month",
                                        ev.name,
                                        ev.u_last * 100.0
                                    ),
                                    evidence: ev.label.clone(),
                                    fix: fix_for("below-baseline").to_string(),
                                    class: "below-baseline",
                                });
                                log.push(format!(
                                    "productivity (b): FIRE {} — {}",
                                    ev.id, ev.label
                                ));
                            }
                        }
                    }
                    Transition::Suppress => log.push(format!(
                        "productivity (b): {} re-fire suppressed (cooldown)",
                        ev.id
                    )),
                    _ => {}
                }
            }
            Verdict::Clear => {
                let prior = store.prod_state_get(&game_id, &ev.id).ok().flatten();
                if let Transition::Cleared =
                    transition(prior.as_ref(), ev.verdict, Some("below-baseline"), tick, th)
                {
                    let since = prior.as_ref().map(|p| p.2).unwrap_or(tick);
                    let _ = store.prod_state_put(
                        &game_id, &ev.id, "product", "cleared", Some("below-baseline"), since,
                        Some(ev.u_last), &ev.label, tick,
                    );
                    log.push(format!("productivity (b): CLEAR {} — {}", ev.id, ev.label));
                }
            }
            _ => {}
        }
    }
    log
}

// ---------------------------------------------------------------------------
// Read-only report (coi report section + serve Issues page section)
// ---------------------------------------------------------------------------

/// Human-readable productivity lines for the latest stored game,
/// evaluated read-only from the persisted aggregates (estimator (a)
/// re-evaluated exactly; estimator (b) shown from its last
/// snapshot-time state, since it needs the live flow payload).
/// INSUFFICIENT targets are listed as unjudged — never as healthy.
pub fn report_lines(store: &Store, th: &Thresholds) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let Some(game_id) = store.latest_game_id()? else {
        return Ok(lines);
    };
    let Some((tick, date)) = store.latest_snapshot_info(&game_id)? else {
        return Ok(lines);
    };
    let ticks = store.snapshot_ticks(&game_id)?;
    let mut series: std::collections::BTreeMap<String, Vec<AggRow>> = Default::default();
    for (t, proto, w, n, o, p, ot, inst, _src) in store.machine_state_series(&game_id)? {
        series.entry(proto).or_default().push((t, w, n, o, p, ot, inst));
    }
    if series.is_empty() {
        lines.push(format!(
            "No machine-state evidence stored yet (snapshots up to {date}, tick {tick} carry no bridge census/machines data). Estimator (a) is INSUFFICIENT by construction until snapshots with bridge data accumulate — take snapshots via `coi watch` while the bridge is up."
        ));
    } else {
        let evals = eval_protos(&series, &ticks, tick, th);
        let (mut nm, mut ne, mut ni) = (0usize, 0usize, 0usize);
        for ev in &evals {
            match ev.class {
                "MEASURED" => nm += 1,
                "ESTIMATED" => ne += 1,
                _ => ni += 1,
            }
        }
        lines.push(format!(
            "Estimator (a) over the trailing 30d window ({} snapshots stored): {} protos — {} MEASURED, {} ESTIMATED, {} INSUFFICIENT.",
            ticks.len(),
            evals.len(),
            nm,
            ne,
            ni
        ));
        for ev in evals {
            match ev.verdict {
                Verdict::Fire { severe } => lines.push(format!(
                    "{}FIRING: {}",
                    if severe { "SEVERE " } else { "" },
                    ev.label
                )),
                Verdict::Parked => lines.push(format!(
                    "PARKED (paused share {:.0}%), not an alert: {}",
                    ev.parked_share * 100.0,
                    ev.label
                )),
                Verdict::Ramp => lines.push(format!(
                    "RAMPING (new or expanding line; exempt from low alerts): {}",
                    ev.label
                )),
                Verdict::Hot => lines.push(format!(
                    "Running hot (digest note, not an alert): {}",
                    ev.label
                )),
                Verdict::Insufficient => lines.push(format!(
                    "INSUFFICIENT evidence — unjudged, NOT healthy: {} — {}",
                    ev.label, ev.missing
                )),
                Verdict::DeadZone => lines.push(format!(
                    "In the hysteresis dead zone (no fire, no clear): {}",
                    ev.label
                )),
                Verdict::Clear => lines.push(format!("Clear of the fire line: {}", ev.label)),
            }
        }
    }
    for (target, kind, status, cause, _since, _value, evidence) in store.prod_states(&game_id)? {
        if kind == "product" {
            lines.push(format!(
                "Estimator (b) [last snapshot evaluation]: {} {} (cause {}) — {}",
                target,
                status.to_uppercase(),
                cause.unwrap_or_default(),
                evidence.unwrap_or_default()
            ));
        }
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn th() -> Thresholds {
        Thresholds::default()
    }

    #[test]
    fn wilson_known_values() {
        // Hand-computed: p=0.3, n=100, z=1.96 → [0.2190, 0.3958].
        let (lo, hi) = wilson(30, 100);
        assert!((lo - 0.219).abs() < 0.005, "lo={lo}");
        assert!((hi - 0.396).abs() < 0.005, "hi={hi}");
        assert_eq!(wilson(0, 0), (0.0, 1.0));
        let (lo, hi) = wilson(93, 310);
        assert!(hi < 0.36 && lo > 0.24, "CI [{lo}, {hi}]");
    }

    #[test]
    fn thinning_drops_dense_snapshots() {
        let ticks = vec![0, 100, 719, 720, 1000, 1440, 5000];
        assert_eq!(thin(&ticks, 720), vec![0, 720, 1440, 5000]);
        assert_eq!(thin(&[], 720), Vec::<i64>::new());
    }

    /// Daily snapshots for `days` days, one proto, constant state mix.
    fn series(days: i64, per_day: (i64, i64, i64, i64, i64, i64)) -> (std::collections::BTreeMap<String, Vec<AggRow>>, Vec<i64>) {
        let mut m: std::collections::BTreeMap<String, Vec<AggRow>> = Default::default();
        let mut ticks = Vec::new();
        for d in 0..days {
            let t = 1_000_000 + d * TICKS_PER_DAY;
            ticks.push(t);
            m.entry("ProtoX".to_string()).or_default().push((
                t, per_day.0, per_day.1, per_day.2, per_day.3, per_day.4, per_day.5,
            ));
        }
        (m, ticks)
    }

    #[test]
    fn dense_starved_series_is_measured_and_fires() {
        // 10 instances: 3 working, 7 starved → p̂ = 0.30, starved.
        let (m, ticks) = series(31, (3, 7, 0, 0, 0, 10));
        let end = *ticks.last().unwrap();
        let ev = eval_protos(&m, &ticks, end, &th());
        assert_eq!(ev.len(), 1);
        let e = &ev[0];
        assert_eq!(e.class, "MEASURED", "label: {}", e.label);
        assert_eq!(e.cause, Some("starved"));
        assert!(matches!(e.verdict, Verdict::Fire { .. }), "verdict: {}", e.label);
    }

    #[test]
    fn sparse_series_is_insufficient_and_silent() {
        let (m, ticks) = series(3, (3, 7, 0, 0, 0, 10));
        let end = *ticks.last().unwrap();
        let ev = eval_protos(&m, &ticks, end, &th());
        assert_eq!(ev[0].class, "INSUFFICIENT");
        assert!(matches!(ev[0].verdict, Verdict::Insufficient));
    }

    #[test]
    fn parked_proto_is_not_an_alert() {
        // 8 instances, all in the paused bucket, none eligible.
        // Parked-ness is checked before the class gate (it needs no
        // eligible samples), so this is PARKED — and never a Fire.
        let (m, ticks) = series(31, (0, 0, 0, 8, 0, 8));
        let end = *ticks.last().unwrap();
        let ev = eval_protos(&m, &ticks, end, &th());
        assert!(matches!(ev[0].verdict, Verdict::Parked), "label: {}", ev[0].label);
    }

    #[test]
    fn half_parked_with_healthy_rest_is_parked() {
        // 10 instances: 5 paused, 5 working → parked share = 0.5.
        let (m, ticks) = series(31, (5, 0, 0, 5, 0, 10));
        let end = *ticks.last().unwrap();
        let ev = eval_protos(&m, &ticks, end, &th());
        assert!(matches!(ev[0].verdict, Verdict::Parked), "label: {}", ev[0].label);
    }

    #[test]
    fn healthy_series_clears() {
        // 10 instances: 9 working → p̂ = 0.9, CI lower > 0.60 → Clear.
        let (m, ticks) = series(31, (9, 1, 0, 0, 0, 10));
        let end = *ticks.last().unwrap();
        let ev = eval_protos(&m, &ticks, end, &th());
        assert!(matches!(ev[0].verdict, Verdict::Hot), "label: {}", ev[0].label);
    }

    #[test]
    fn b_verdict_rules() {
        let th = th();
        // Sustained halving fires.
        let (v, u, _, best) = b_verdict(&[800, 820, 400, 380], &th);
        assert_eq!(best, 820);
        assert!((u - 380.0 / 820.0).abs() < 1e-9);
        assert!(matches!(v, Verdict::Fire { .. }));
        // Single deep month fires severely.
        let (v, _, _, _) = b_verdict(&[800, 800, 200], &th);
        assert!(matches!(v, Verdict::Fire { severe: true }));
        // Recovery clears.
        let (v, _, _, _) = b_verdict(&[800, 400, 700], &th);
        assert!(matches!(v, Verdict::Clear));
        // No baseline → insufficient.
        let (v, _, _, _) = b_verdict(&[0, 0, 0], &th);
        assert!(matches!(v, Verdict::Insufficient));
    }

    #[test]
    fn hysteresis_transitions() {
        let th = th();
        let fire = Verdict::Fire { severe: false };
        // First fire → new.
        assert!(matches!(
            transition(None, fire, Some("starved"), 1000, &th),
            Transition::EmitNew { .. }
        ));
        let fired = ("fired".to_string(), Some("starved".to_string()), 1000, Some(0.3));
        // Same cause while fired → ongoing, original since kept.
        match transition(Some(&fired), fire, Some("starved"), 2000, &th) {
            Transition::EmitOngoing { since } => assert_eq!(since, 1000),
            other => panic!("{other:?}"),
        }
        // Cause change → new alert.
        assert!(matches!(
            transition(Some(&fired), fire, Some("blocked"), 2000, &th),
            Transition::EmitNew { .. }
        ));
        // Clear while fired → cleared.
        assert!(matches!(
            transition(Some(&fired), Verdict::Clear, Some("starved"), 3000, &th),
            Transition::Cleared
        ));
        // Re-fire same cause within cooldown of the fire → suppressed.
        let cleared = ("cleared".to_string(), Some("starved".to_string()), 1000, Some(0.3));
        assert!(matches!(
            transition(Some(&cleared), fire, Some("starved"), 1000 + 1000, &th),
            Transition::Suppress
        ));
        // …but after the cooldown it is a fresh alert.
        assert!(matches!(
            transition(Some(&cleared), fire, Some("starved"), 1000 + TICKS_PER_MONTH + 1, &th),
            Transition::EmitNew { .. }
        ));
        // Dead zone holds a fired alert, does nothing otherwise.
        match transition(Some(&fired), Verdict::DeadZone, Some("starved"), 2000, &th) {
            Transition::EmitOngoing { since } => assert_eq!(since, 1000),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            transition(None, Verdict::DeadZone, Some("starved"), 2000, &th),
            Transition::None
        ));
    }

    #[test]
    fn store_round_trip_persists_machine_rows() {
        // Regression guard for the insert path itself: a snapshot
        // carrying bridge payloads must land in the v2 tables, with
        // the machines route taking precedence over the census.
        let path = std::env::temp_dir().join(format!(
            "coi-prod-test-{}.db",
            std::process::id()
        ));
        let path = path.to_str().unwrap();
        let _ = std::fs::remove_file(path);
        let mut st = Store::open(path).unwrap();
        let snap = serde_json::json!({
            "schema": "coi-snapshot/1",
            "takenAtUnix": 1700000000,
            "game": {"gameId": "t1", "tick": 5000, "date": "1-1-1",
                     "isPaused": false, "speed": 1, "sessionId": "s"},
            "products": [],
            "history": {"products": {}},
            "bridgeCensus": {"machines": {
                "FurnaceT1": {"states": {"Working": 1, "NotEnoughInput": 1, "NotEnoughPower": 2}}
            }},
            "bridgeMachines": {"machines": [
                {"id": 42, "proto": "FurnaceT1", "state": "Working", "isPaused": false, "isEnabled": true},
                {"id": 43, "proto": "FurnaceT1", "state": "NotEnoughInput", "isPaused": false, "isEnabled": true},
                {"id": 44, "proto": "PressT1", "state": "Paused", "isPaused": true, "isEnabled": true}
            ]}
        });
        let id = st.insert_snapshot(&snap, &[]).unwrap();
        assert!(id.is_some());
        let rows = st.machine_state_series("t1").unwrap();
        // FurnaceT1 from instances (2 rows: 1 working, 1 starved);
        // PressT1 from instances (1 paused). Census-only FurnaceT1
        // aggregates (incl. the unknown NotEnoughPower state) are
        // superseded by the instances source for that proto.
        assert_eq!(rows.len(), 2, "rows: {rows:?}");
        let furn = rows.iter().find(|r| r.1 == "FurnaceT1").unwrap();
        assert_eq!((furn.2, furn.3, furn.7, furn.8.as_str()), (1, 1, 2, "instances"));
        let press = rows.iter().find(|r| r.1 == "PressT1").unwrap();
        assert_eq!((press.5, press.7), (1, 1)); // paused=1, instances=1
        // Duplicate tick rejected.
        assert!(st.insert_snapshot(&snap, &[]).unwrap().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn census_only_rows_bucket_unknown_states_as_other() {
        let snap = serde_json::json!({
            "bridgeCensus": {"machines": {
                "FurnaceT1": {"states": {"Working": 1, "NotEnoughPower": 2, "Paused": 1}}
            }}
        });
        let rows = crate::store::snapshot_machine_rows(&snap);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        // working=1, paused=1, other=2 (NotEnoughPower), instances=4.
        assert_eq!((r.1, r.4, r.5, r.6), (1, 1, 2, 4));
        assert_eq!(r.7, "census");
    }

    #[test]
    fn thresholds_toml_overrides() {
        let mut th = Thresholds::default();
        th.apply_toml("[productivity]\nfire_point = 0.40\nthin_ticks = 1440\n# comment\nunknown_key = 3\n[other]\nfire_point = 0.99\n");
        assert_eq!(th.fire_point, 0.40);
        assert_eq!(th.thin_ticks, 1440);
    }
}
