//! `coi plan-check` — DRY-RUN plan evaluator for CoI game plans
//! (GAME-PLAN-FORMAT.md v1), the Rust port of the reference
//! implementation `tools/plan_check.py` (Round 3). The Python tool
//! remains the verified reference: this port reproduces its
//! three-valued gate semantics (§4.4 TRUE / FALSE / UNEVALUABLE),
//! its active-phase derivation (§9), its counting rules and its
//! would-prepare order rendering, and parity against the Python
//! dry-run reports is a release gate for changes here (Round 4).
//!
//! Input is a plan in the canonical JSON form emitted by
//! `tools/plan_to_json.py` (the crate has no YAML dependency and
//! serde_yaml is absent from the offline cargo cache), plus a state
//! directory in the captured `live/` shape (game-info.json,
//! stats.json, products.json, pinned.json, bridge-status.json,
//! bridge-research.json, bridge-census.json, bridge-storages.json,
//! bridge-trains.json).
//!
//! HARD SAFETY: this module is read-only over local files. It opens
//! no sockets, spawns nothing, and has no code path that can prepare,
//! confirm or apply a game command. Every report carries the
//! "DRY RUN — nothing was executed" banner.
//!
//! The supervised session runner `coi play` (below) shares that
//! posture exactly: it is PREPARE-ONLY and has NO apply path — no
//! sockets, no HTTP client use, no §12 prepare/confirm/apply calls.
//! It reads a plan plus a §4.3.1 state sequence from local files,
//! walks the sequence prefix by prefix, records the exact command
//! payloads a supervised executor WOULD prepare (a human reviews
//! them and confirms each one in the companion website), and writes
//! only the session JSON / transcript files it was asked for.

use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tri {
    True,
    False,
    Unevaluable,
}

impl Tri {
    fn name(self) -> &'static str {
        match self {
            Tri::True => "TRUE",
            Tri::False => "FALSE",
            Tri::Unevaluable => "UNEVALUABLE",
        }
    }
}

#[derive(Clone)]
struct Res {
    v: Tri,
    ev: String,
}

impl Res {
    fn new(v: Tri, ev: String) -> Res {
        Res { v, ev }
    }
    fn uneval(ev: String) -> Res {
        Res::new(Tri::Unevaluable, ev)
    }
}

// ---------------------------------------------------------------- state

fn load_json(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("capture {} unreadable: {e}", path.display()))?;
    match serde_json::from_str(&text) {
        Ok(v) => Ok(Some(v)),
        Err(e) => Err(format!("capture {} unreadable: {e}", path.display())),
    }
}

fn obj_or_empty(v: Option<Value>) -> Value {
    match v {
        Some(Value::Object(_)) => v.unwrap(),
        _ => Value::Object(Map::new()),
    }
}

struct State {
    dir: PathBuf,
    game_info: Value,
    stats: Map<String, Value>,
    catalogue: Map<String, Value>,
    pinned: HashSet<String>,
    pinned_count: usize,
    status: Value,
    research: Value,
    census: Value,
    storages: Vec<Value>,
    train_lines: Option<Vec<Value>>,
}

impl State {
    fn load(dir: &Path) -> Result<State, String> {
        let game_info = obj_or_empty(load_json(&dir.join("game-info.json"))?);
        let stats_doc = load_json(&dir.join("stats.json"))?;
        let stats: Map<String, Value> = match &stats_doc {
            Some(Value::Object(m)) => match m.get("products") {
                Some(Value::Object(p)) => p.clone(),
                _ => Map::new(),
            },
            _ => Map::new(),
        };
        let cat_doc = load_json(&dir.join("products.json"))?;
        let mut catalogue: Map<String, Value> = Map::new();
        if let Some(Value::Object(m)) = &cat_doc {
            if let Some(Value::Array(plist)) = m.get("products") {
                for p in plist {
                    if let Some(id) = p.get("id").and_then(|i| i.as_str()) {
                        catalogue.insert(id.to_string(), p.clone());
                    }
                }
            }
        }
        let pinned_doc = load_json(&dir.join("pinned.json"))?;
        let mut pinned: HashSet<String> = HashSet::new();
        if let Some(Value::Object(m)) = &pinned_doc {
            if let Some(Value::Object(products)) = m.get("products") {
                for k in products.keys() {
                    pinned.insert(k.clone());
                }
            }
        }
        let pinned_count = pinned.len();
        let status = obj_or_empty(load_json(&dir.join("bridge-status.json"))?);
        let research = obj_or_empty(load_json(&dir.join("bridge-research.json"))?);
        let census = obj_or_empty(load_json(&dir.join("bridge-census.json"))?);
        let stor_doc = load_json(&dir.join("bridge-storages.json"))?;
        let storages: Vec<Value> = match &stor_doc {
            Some(Value::Object(m)) => match m.get("storages") {
                Some(Value::Array(rows)) => rows.clone(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        let trains_doc = load_json(&dir.join("bridge-trains.json"))?;
        let train_lines: Option<Vec<Value>> = match &trains_doc {
            Some(Value::Object(m)) => match m.get("lines") {
                Some(Value::Array(lines)) => Some(lines.clone()),
                _ => None,
            },
            _ => None,
        };
        Ok(State {
            dir: dir.to_path_buf(),
            game_info,
            stats,
            catalogue,
            pinned,
            pinned_count,
            status,
            research,
            census,
            storages,
            train_lines,
        })
    }
}

// ---------------------------------------------------------------- values

/// Python-style plain rendering (str()).
fn fmt_val(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Number(n) => fmt_num(n),
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Python-style repr() for the values predicates carry.
fn fmt_repr(v: &Value) -> String {
    match v {
        Value::String(s) => {
            if s.contains('\'') && !s.contains('"') {
                format!("\"{s}\"")
            } else {
                format!("'{s}'")
            }
        }
        Value::Number(n) => fmt_num(n),
        other => fmt_val(other),
    }
}

fn fmt_num(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    if let Some(f) = n.as_f64() {
        return fmt_f64(f);
    }
    n.to_string()
}

fn fmt_f64(f: f64) -> String {
    if f.is_finite() && f == f.trunc() && f.abs() < 9.0e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// Python `==` across JSON values, with Python's numeric tower
/// (100 == 100.0 is True there; serde_json's Number equality would
/// say false, so numbers are compared numerically here).
fn py_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => num_cmp(x, y) == Some(std::cmp::Ordering::Equal),
        _ => a == b,
    }
}

fn num_cmp(x: &serde_json::Number, y: &serde_json::Number) -> Option<std::cmp::Ordering> {
    if let (Some(a), Some(b)) = (x.as_i64(), y.as_i64()) {
        return Some(a.cmp(&b));
    }
    if let (Some(a), Some(b)) = (x.as_u64(), y.as_u64()) {
        return Some(a.cmp(&b));
    }
    let (a, b) = (x.as_f64()?, y.as_f64()?);
    a.partial_cmp(&b)
}

/// Port of plan_check.py's compare(): Ok(bool) or Err(reason) for
/// ill-typed comparisons (which the caller turns into UNEVALUABLE).
fn compare(actual: &Value, op: &str, expected: &Value) -> Result<bool, String> {
    if expected.is_null() && (op == "==" || op == "!=") {
        let eq = actual.is_null();
        return Ok(if op == "==" { eq } else { !eq });
    }
    // `none` sentinel for research.current (plans write the string).
    if let Value::String(s) = expected {
        if s == "none" {
            let eq = actual.is_null();
            return match op {
                "==" => Ok(eq),
                "!=" => Ok(!eq),
                _ => Err("ordering comparison against 'none'".to_string()),
            };
        }
    }
    let a_bool = actual.is_boolean();
    let e_bool = expected.is_boolean();
    if a_bool || e_bool {
        if !(a_bool && e_bool) {
            return match op {
                "==" => Ok(py_eq(actual, expected)),
                "!=" => Ok(!py_eq(actual, expected)),
                _ => Err(format!(
                    "cannot order-compare {} {op} {}",
                    fmt_repr(actual),
                    fmt_repr(expected)
                )),
            };
        }
        return match op {
            "==" => Ok(actual == expected),
            "!=" => Ok(actual != expected),
            _ => Err(format!("cannot order-compare booleans {op}")),
        };
    }
    match op {
        "==" => return Ok(py_eq(actual, expected)),
        "!=" => return Ok(!py_eq(actual, expected)),
        "<" | "<=" | ">" | ">=" => {}
        _ => return Err(format!("unknown operator {op:?}")),
    }
    // Ordering comparisons.
    if let (Value::Number(x), Value::Number(y)) = (actual, expected) {
        let ord = num_cmp(x, y)
            .ok_or_else(|| format!("cannot compare {} {op} {}", fmt_repr(actual), fmt_repr(expected)))?;
        return Ok(match op {
            "<" => ord == std::cmp::Ordering::Less,
            "<=" => ord != std::cmp::Ordering::Greater,
            ">" => ord == std::cmp::Ordering::Greater,
            _ => ord != std::cmp::Ordering::Less,
        });
    }
    if let (Value::String(x), Value::String(y)) = (actual, expected) {
        return Ok(match op {
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        });
    }
    Err(format!(
        "cannot compare {} {op} {}",
        fmt_repr(actual),
        fmt_repr(expected)
    ))
}

fn parse_date(s: &str) -> Result<(i64, i64, i64), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("unparseable in-game date {s:?}"));
    }
    let mut out = [0i64; 3];
    for (i, p) in parts.iter().enumerate() {
        out[i] = p
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("unparseable in-game date {s:?}"))?;
    }
    Ok((out[0], out[1], out[2]))
}

/// (name, placeholder-ish) — mirrors plan_check.py's _node_name: ANY
/// mapping node yields a placeholder-ish second slot (the placeholder
/// string itself, or a repr of the mapping).
fn node_name(node: Option<&Value>) -> (Option<String>, Option<String>) {
    match node {
        Some(Value::Object(m)) => {
            if let Some(p) = m.get("placeholder") {
                (None, Some(fmt_val(p)))
            } else {
                (None, Some(serde_json::to_string(&Value::Object(m.clone())).unwrap_or_default()))
            }
        }
        Some(Value::String(s)) => (Some(s.clone()), None),
        Some(other) => (Some(fmt_val(other)), None),
        None => (None, None),
    }
}

fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_object().and_then(|m| m.get(key))
}

fn has(v: &Value, key: &str) -> bool {
    v.as_object().map(|m| m.contains_key(key)).unwrap_or(false)
}

// ---------------------------------------------------------------- leaves

const WINDOWS: [(&str, usize); 5] = [
    ("thisMonth", 0),
    ("lastMonth", 1),
    ("thisYear", 2),
    ("lastYear", 3),
    ("lifetime", 4),
];

fn bool_res(actual: bool, op: &str, expected: &Value, ev: String) -> Res {
    match compare(&Value::Bool(actual), op, expected) {
        Ok(true) => Res::new(Tri::True, ev),
        Ok(false) => Res::new(Tri::False, ev),
        Err(e) => Res::uneval(e),
    }
}

fn cmp_res(actual: &Value, op: &str, expected: &Value, ev: String) -> Res {
    match compare(actual, op, expected) {
        Ok(true) => Res::new(Tri::True, ev),
        Ok(false) => Res::new(Tri::False, ev),
        Err(e) => Res::uneval(e),
    }
}

fn eval_leaf(pred: &Map<String, Value>, st: &State) -> Res {
    let metric = match pred.get("metric") {
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Res::uneval("predicate 'metric' is not a string".into()),
        None => return Res::uneval("predicate has no 'metric' key".into()),
    };
    let has_op_value = pred.contains_key("op") || pred.contains_key("value");
    let op: String = if has_op_value {
        match pred.get("op") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => return Res::uneval(format!("unknown operator {}", fmt_repr(other))),
            None => "==".to_string(),
        }
    } else {
        "==".to_string()
    };
    let expected: Value = if has_op_value {
        pred.get("value").cloned().unwrap_or(Value::Bool(true))
    } else {
        Value::Bool(true)
    };
    let op = op.as_str();

    // ---- products -------------------------------------------------
    if metric.starts_with("product.") {
        let pid = match pred.get("product") {
            Some(Value::String(s)) => s.clone(),
            _ => {
                return Res::uneval(format!(
                    "{metric}: product selector missing/not a string"
                ))
            }
        };
        if metric == "product.unlocked" {
            let row = match st.catalogue.get(&pid) {
                Some(r) => r,
                None => {
                    return Res::uneval(format!("{pid} not in products.json catalogue"))
                }
            };
            let actual = match row.get("isUnlocked") {
                Some(v) => v.clone(),
                None => {
                    return Res::uneval(format!("{pid} has no isUnlocked in catalogue"))
                }
            };
            let ev = format!("{pid}.isUnlocked = {} (products.json)", fmt_val(&actual));
            return cmp_res(&actual, op, &expected, ev);
        }
        if metric == "product.pinned" {
            if st.pinned.is_empty() {
                return Res::uneval("pinned.json capture empty/missing".into());
            }
            let actual = st.pinned.contains(&pid);
            let ev = format!(
                "{pid} in pinned set = {} (pinned.json lists {} pinned products)",
                fmt_val(&Value::Bool(actual)),
                st.pinned_count
            );
            return bool_res(actual, op, &expected, ev);
        }
        let row = match st.stats.get(&pid) {
            Some(r) => r,
            None => {
                return Res::uneval(format!(
                    "{pid} not in stats.json products (selector matched zero rows)"
                ))
            }
        };
        let field = |name: &str| -> Result<Value, Res> {
            match row.get(name) {
                Some(v) => Ok(v.clone()),
                None => Err(Res::uneval(format!(
                    "evaluation error: stats row for {pid} has no field '{name}'"
                ))),
            }
        };
        match metric.as_str() {
            "product.quantity" => {
                let actual = match field("quantity") {
                    Ok(v) => v,
                    Err(r) => return r,
                };
                let ev = format!("{pid}.quantity = {}", fmt_val(&actual));
                return cmp_res(&actual, op, &expected, ev);
            }
            "product.stored" => {
                let actual = match field("stored") {
                    Ok(v) => v,
                    Err(r) => return r,
                };
                let cap = row.get("storageCapacity").cloned().unwrap_or(Value::Null);
                let ev = format!(
                    "{pid}.stored = {} (storageCapacity {})",
                    fmt_val(&actual),
                    fmt_val(&cap)
                );
                return cmp_res(&actual, op, &expected, ev);
            }
            "product.storage_capacity" => {
                let actual = match field("storageCapacity") {
                    Ok(v) => v,
                    Err(r) => return r,
                };
                let ev = format!("{pid}.storageCapacity = {}", fmt_val(&actual));
                return cmp_res(&actual, op, &expected, ev);
            }
            "product.produced" | "product.consumed" | "product.net_flow" => {
                let window = pred.get("window").and_then(|w| w.as_str()).unwrap_or("");
                let idx = match WINDOWS.iter().find(|(name, _)| *name == window) {
                    Some((_, i)) => *i,
                    None => {
                        return Res::uneval(format!(
                            "{metric}: bad/missing window {}",
                            fmt_repr(pred.get("window").unwrap_or(&Value::Null))
                        ))
                    }
                };
                let flow = |name: &str| -> Result<f64, Res> {
                    let arr = match row.get(name) {
                        Some(Value::Array(a)) => a.clone(),
                        _ => {
                            return Err(Res::uneval(format!(
                                "evaluation error: stats row for {pid} has no flow array '{name}'"
                            )))
                        }
                    };
                    match arr.get(idx).and_then(as_f64) {
                        Some(x) => Ok(x),
                        None => Err(Res::uneval(format!(
                            "evaluation error: {pid} {name}[{window}] missing"
                        ))),
                    }
                };
                let prod = match flow("produced") {
                    Ok(v) => v,
                    Err(r) => return r,
                };
                let cons = match flow("consumed") {
                    Ok(v) => v,
                    Err(r) => return r,
                };
                let (actual, ev) = if metric == "product.produced" {
                    (
                        prod,
                        format!("{pid} produced[{window}] = {}", fmt_f64(prod)),
                    )
                } else if metric == "product.consumed" {
                    (
                        cons,
                        format!("{pid} consumed[{window}] = {}", fmt_f64(cons)),
                    )
                } else {
                    (
                        prod - cons,
                        format!(
                            "{pid} window={window}: produced {} - consumed {} = net {}",
                            fmt_f64(prod),
                            fmt_f64(cons),
                            fmt_f64(prod - cons)
                        ),
                    )
                };
                let actual_v = serde_json::json!(actual);
                return cmp_res(&actual_v, op, &expected, ev);
            }
            _ => {
                return Res::uneval(format!(
                    "metric {metric:?} not in §4.1 vocabulary / no capture source (closed-vocabulary violation)"
                ))
            }
        }
    }

    // ---- game -----------------------------------------------------
    if metric == "game.paused" {
        if !has(&st.game_info, "isPaused") {
            return Res::uneval("game-info.json has no isPaused".into());
        }
        let actual = get(&st.game_info, "isPaused").cloned().unwrap();
        let ev = format!("isPaused = {}", fmt_val(&actual));
        return cmp_res(&actual, op, &expected, ev);
    }
    if metric == "game.speed" {
        if !has(&st.game_info, "speed") {
            return Res::uneval("game-info.json has no speed".into());
        }
        let actual = get(&st.game_info, "speed").cloned().unwrap();
        let ev = format!("speed = {}", fmt_val(&actual));
        return cmp_res(&actual, op, &expected, ev);
    }
    if metric == "game.date" {
        if !has(&st.game_info, "date") {
            return Res::uneval("game-info.json has no date".into());
        }
        let date_s = fmt_val(get(&st.game_info, "date").unwrap());
        let actual = match parse_date(&date_s) {
            Ok(d) => d,
            Err(e) => return Res::uneval(e),
        };
        let exp_s = match &expected {
            Value::String(s) => s.clone(),
            other => fmt_val(other),
        };
        let exp = match parse_date(&exp_s) {
            Ok(d) => d,
            Err(e) => return Res::uneval(e),
        };
        let ord = actual.cmp(&exp);
        let r = match op {
            "==" => ord == std::cmp::Ordering::Equal,
            "!=" => ord != std::cmp::Ordering::Equal,
            "<" => ord == std::cmp::Ordering::Less,
            "<=" => ord != std::cmp::Ordering::Greater,
            ">" => ord == std::cmp::Ordering::Greater,
            ">=" => ord != std::cmp::Ordering::Less,
            _ => return Res::uneval(format!("unknown operator {op:?}")),
        };
        return Res::new(
            if r { Tri::True } else { Tri::False },
            format!("date = {date_s}"),
        );
    }

    // ---- settlement -----------------------------------------------
    let settle_field = match metric.as_str() {
        "settlement.population" => Some("population"),
        "settlement.housing_capacity" => Some("totalHousingCapacity"),
        "settlement.free_housing" => Some("freeHousingCapacity"),
        "settlement.homeless" => Some("numberOfHomeless"),
        "settlement.months_of_food" => Some("monthsOfFood"),
        "settlement.starving" => Some("arePeopleStarving"),
        _ => None,
    };
    if let Some(field_name) = settle_field {
        if !has(&st.status, field_name) {
            return Res::uneval(format!("bridge-status.json has no {field_name}"));
        }
        let actual = get(&st.status, field_name).cloned().unwrap();
        let ev = format!("{field_name} = {} (bridge-status.json)", fmt_val(&actual));
        return cmp_res(&actual, op, &expected, ev);
    }

    // ---- research -------------------------------------------------
    if metric == "research.completed" {
        let (node, placeholder) = node_name(pred.get("node"));
        if let Some(ph) = placeholder {
            return Res::uneval(format!(
                "node is a §4.5 placeholder ({ph}) — manual-only, never auto-evaluated"
            ));
        }
        let node = match node {
            Some(n) => n,
            None => return Res::uneval("research.completed: node selector missing".into()),
        };
        if !has(&st.research, "researched") {
            return Res::uneval(
                "bridge-research.json (pre-v0.6.2 capture) has no `researched` completed-node list — format G1; executor-observed completions are not in the captures"
                    .into(),
            );
        }
        let list = match get(&st.research, "researched") {
            Some(Value::Array(a)) => a,
            _ => {
                return Res::uneval(
                    "evaluation error: bridge-research.json `researched` is not a list".into(),
                )
            }
        };
        let actual = list.iter().any(|n| n.as_str() == Some(node.as_str()));
        let ev = format!("{node} in researched list = {}", fmt_val(&Value::Bool(actual)));
        return bool_res(actual, op, &expected, ev);
    }
    if metric == "research.current" {
        let cur = get(&st.research, "current");
        let actual: Value = match cur {
            Some(Value::Object(m)) => m.get("id").cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        };
        let ev = format!(
            "research.current = {}",
            if actual.is_null() {
                "none".to_string()
            } else {
                fmt_val(&actual)
            }
        );
        return cmp_res(&actual, op, &expected, ev);
    }
    if metric == "research.current_progress_pct" {
        let cur = get(&st.research, "current");
        let raw = match cur {
            Some(Value::Object(m)) => m.get("progressPerc").and_then(as_f64),
            _ => None,
        };
        let raw = match raw {
            Some(r) => r,
            None => return Res::uneval("no active research node in capture".into()),
        };
        let actual = raw / 1000.0;
        let ev = format!("progressPerc {} / 1000 = {actual:.3}%", fmt_f64(raw));
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }
    if metric == "research.queue_contains" {
        let (node, placeholder) = node_name(pred.get("node"));
        if let Some(ph) = placeholder {
            return Res::uneval(format!("node is a §4.5 placeholder ({ph})"));
        }
        let queue = match get(&st.research, "queue") {
            Some(Value::Array(q)) => q.clone(),
            _ => return Res::uneval("bridge-research.json has no queue".into()),
        };
        let node = node.unwrap_or_default();
        let actual = queue.iter().any(|n| n.as_str() == Some(node.as_str()));
        let ev = format!(
            "queue = {}; contains {node} = {}",
            serde_json::to_string(&Value::Array(queue)).unwrap_or_default(),
            fmt_val(&Value::Bool(actual))
        );
        return bool_res(actual, op, &expected, ev);
    }
    if metric == "research.queue_length" {
        let queue = match get(&st.research, "queue") {
            Some(Value::Array(q)) => q.clone(),
            _ => return Res::uneval("bridge-research.json has no queue".into()),
        };
        let actual = queue.len() as i64;
        let ev = format!(
            "len(queue) = {actual} {}",
            serde_json::to_string(&Value::Array(queue)).unwrap_or_default()
        );
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }
    if metric == "research.researched_count" {
        if !has(&st.research, "researchedCount") {
            return Res::uneval("bridge-research.json has no researchedCount".into());
        }
        let actual = get(&st.research, "researchedCount").cloned().unwrap();
        let ev = format!("researchedCount = {}", fmt_val(&actual));
        return cmp_res(&actual, op, &expected, ev);
    }
    if metric == "research.has_active_lab" {
        if !has(&st.research, "hasActiveLab") {
            return Res::uneval("bridge-research.json has no hasActiveLab".into());
        }
        let actual = get(&st.research, "hasActiveLab").cloned().unwrap();
        let ev = format!("hasActiveLab = {}", fmt_val(&actual));
        return cmp_res(&actual, op, &expected, ev);
    }

    // ---- census ---------------------------------------------------
    if metric == "census.proto_count" {
        let proto = pred.get("proto").and_then(|p| p.as_str()).unwrap_or("");
        let by = match get(&st.census, "byProto") {
            Some(Value::Object(m)) => m.clone(),
            _ => return Res::uneval("bridge-census.json has no byProto".into()),
        };
        let actual = by.get(proto).and_then(as_f64).unwrap_or(0.0);
        let ev = format!("byProto[{proto}] = {} (absent proto counts 0)", fmt_f64(actual));
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }
    if metric == "census.machine_state_count" || metric == "census.machine_state_share" {
        let proto = pred.get("proto").and_then(|p| p.as_str()).unwrap_or("");
        let state = pred.get("state").and_then(|p| p.as_str()).unwrap_or("");
        let machines = match get(&st.census, "machines") {
            Some(Value::Object(m)) => m.clone(),
            _ => return Res::uneval("bridge-census.json has no machines".into()),
        };
        let state_count = |entry: &Value| -> (f64, f64) {
            let states = entry.get("states");
            let c = states
                .and_then(|s| s.get(state))
                .and_then(as_f64)
                .unwrap_or(0.0);
            let t = match states {
                Some(Value::Object(sm)) => sm.values().filter_map(as_f64).sum(),
                _ => 0.0,
            };
            (c, t)
        };
        let (count, total, scope) = if proto == "*" {
            let mut c = 0.0;
            let mut t = 0.0;
            for entry in machines.values() {
                let (ec, et) = state_count(entry);
                c += ec;
                t += et;
            }
            (c, t, "all machine protos".to_string())
        } else {
            match machines.get(proto) {
                Some(entry) => {
                    let (c, t) = state_count(entry);
                    (c, t, proto.to_string())
                }
                None => (0.0, 0.0, proto.to_string()),
            }
        };
        if metric == "census.machine_state_count" {
            let ev = format!("{scope}: state {state} count = {}", fmt_f64(count));
            return cmp_res(&serde_json::json!(count), op, &expected, ev);
        }
        if total == 0.0 {
            return Res::uneval(format!("{scope}: total machine-state count is 0"));
        }
        let actual = count / total;
        let ev = format!(
            "{scope}: {state} {}/{} = {actual:.3}",
            fmt_f64(count),
            fmt_f64(total)
        );
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }
    if metric == "census.transports_fully_connected_ratio" {
        let tr = get(&st.census, "transports");
        let (total, connected) = match tr {
            Some(Value::Object(m)) => (
                m.get("total").and_then(as_f64),
                m.get("fullyConnected").and_then(as_f64),
            ),
            _ => (None, None),
        };
        let (total, connected) = match (total, connected) {
            (Some(t), Some(c)) => (t, c),
            _ => return Res::uneval("bridge-census.json has no transports block".into()),
        };
        if total == 0.0 {
            return Res::uneval("transports.total = 0 (ratio undefined, §4.1)".into());
        }
        let actual = connected / total;
        let ev = format!(
            "fullyConnected {}/{} = {actual:.4}",
            fmt_f64(connected),
            fmt_f64(total)
        );
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }

    // ---- storages -------------------------------------------------
    if metric == "storage.percent_full" {
        let rows = match storage_rows(pred, st) {
            Ok(r) => r,
            Err(e) => return Res::uneval(e),
        };
        if rows.is_empty() {
            return Res::uneval(
                "storage selector matched zero rows in bridge-storages.json".into(),
            );
        }
        let cap: f64 = rows
            .iter()
            .filter_map(|r| r.get("capacity").and_then(as_f64))
            .sum();
        // Python sums r.get("capacity", 0) — rows without capacity add 0;
        // filter_map+sum is equivalent for numeric/missing mixes.
        if cap == 0.0 {
            return Res::uneval("matching storage rows have zero total capacity".into());
        }
        let qty: f64 = rows
            .iter()
            .filter_map(|r| r.get("quantity").and_then(as_f64))
            .sum();
        let actual: f64 = rows
            .iter()
            .map(|r| {
                (r.get("percentFullRaw").and_then(as_f64).unwrap_or(0.0) / 1000.0)
                    * r.get("capacity").and_then(as_f64).unwrap_or(0.0)
            })
            .sum::<f64>()
            / cap;
        let mut ev = format!(
            "{} matching row(s): quantity {}/{} capacity, capacity-weighted percentFull = {actual:.1}%",
            rows.len(),
            fmt_f64(qty),
            fmt_f64(cap)
        );
        if let Some(pid) = pred.get("product").and_then(|p| p.as_str()) {
            if let Some(srow) = st.stats.get(pid) {
                ev += &format!(
                    "; cross-ref stats.json: stored {}/{} (stats totals include stock outside the bridge storage rows)",
                    fmt_val(srow.get("stored").unwrap_or(&Value::Null)),
                    fmt_val(srow.get("storageCapacity").unwrap_or(&Value::Null))
                );
            }
        }
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }
    if metric == "storage.logistics_in_disabled" || metric == "storage.logistics_out_disabled" {
        let sid = pred.get("storage_id").cloned().unwrap_or(Value::Null);
        let row = st
            .storages
            .iter()
            .find(|r| r.get("id").map(|i| py_eq(i, &sid)).unwrap_or(false));
        let row = match row {
            Some(r) => r,
            None => {
                return Res::uneval(format!(
                    "no storage row with id {}",
                    fmt_repr(&sid)
                ))
            }
        };
        let field_name = if metric.ends_with("in_disabled") {
            "logisticsInDisabled"
        } else {
            "logisticsOutDisabled"
        };
        let actual = row.get(field_name).cloned().unwrap_or(Value::Null);
        let ev = format!(
            "storage {}.{field_name} = {}",
            fmt_val(&sid),
            fmt_val(&actual)
        );
        return cmp_res(&actual, op, &expected, ev);
    }

    // ---- trains ---------------------------------------------------
    if metric == "trains.line_count" {
        let lines = match &st.train_lines {
            Some(l) => l,
            None => return Res::uneval("bridge-trains.json has no lines list".into()),
        };
        let actual = lines.len() as i64;
        let ev = format!("len(bridge-trains lines) = {actual}");
        return cmp_res(&serde_json::json!(actual), op, &expected, ev);
    }

    // ---- sources with no capture: UNEVALUABLE by design -----------
    if metric.starts_with("power.") {
        return Res::uneval(format!(
            "{metric}: no power capture exists — the captures predate CoiBridge v0.6.2's /coi/v1/power route (format G2); field {metric} is absent from every capture file"
        ));
    }
    if metric.starts_with("contracts.") || metric.starts_with("loans.") || metric.starts_with("edicts.")
    {
        return Res::uneval(format!("{metric}: no v0.6 trade capture in live/"));
    }
    if metric.starts_with("time.") {
        return Res::uneval(format!(
            "{metric}: no executor run anchor (plan-start date/tick) exists in the captures — dry-run has no run history"
        ));
    }
    Res::uneval(format!(
        "metric {metric:?} not in §4.1 vocabulary / no capture source (closed-vocabulary violation)"
    ))
}

fn storage_rows<'a>(
    pred: &Map<String, Value>,
    st: &'a State,
) -> Result<Vec<&'a Value>, String> {
    if let Some(sid) = pred.get("storage_id") {
        return Ok(st
            .storages
            .iter()
            .filter(|r| r.get("id").map(|i| py_eq(i, sid)).unwrap_or(false))
            .collect());
    }
    if let Some(p) = pred.get("product") {
        return Ok(st
            .storages
            .iter()
            .filter(|r| r.get("product").map(|x| py_eq(x, p)).unwrap_or(false))
            .collect());
    }
    if let Some(p) = pred.get("proto") {
        return Ok(st
            .storages
            .iter()
            .filter(|r| r.get("proto").map(|x| py_eq(x, p)).unwrap_or(false))
            .collect());
    }
    Err("storage predicate needs storage_id, product or proto".to_string())
}

// ---------------------------------------------------------------- composite

fn combine_all(subs: &[Res]) -> Res {
    let v = if subs.iter().any(|s| s.v == Tri::False) {
        Tri::False
    } else if subs.iter().any(|s| s.v == Tri::Unevaluable) {
        Tri::Unevaluable
    } else {
        Tri::True
    };
    Res::new(
        v,
        subs.iter()
            .map(|s| format!("[{}] {}", s.v.name(), s.ev))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

fn combine_any(subs: &[Res]) -> Res {
    let v = if subs.iter().any(|s| s.v == Tri::True) {
        Tri::True
    } else if subs.iter().any(|s| s.v == Tri::Unevaluable) {
        Tri::Unevaluable
    } else {
        Tri::False
    };
    Res::new(
        v,
        subs.iter()
            .map(|s| format!("[{}] {}", s.v.name(), s.ev))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// Absolute in-game day index (§4.3.1: 12 months x 30 days).
fn abs_day(st: &State) -> Result<i64, String> {
    let s = st
        .game_info
        .get("date")
        .map(fmt_val)
        .unwrap_or_else(|| "None".to_string());
    let (y, m, d) = parse_date(&s)?;
    Ok(((y - 1) * 12 + (m - 1)) * 30 + (d - 1))
}

fn date_of(st: &State) -> String {
    st.game_info
        .get("date")
        .map(fmt_val)
        .unwrap_or_else(|| "None".to_string())
}

fn duration_kind(dur: &Value) -> Option<&'static str> {
    if let Some(o) = dur.as_object() {
        if o.contains_key("months") {
            return Some("months");
        }
        if o.contains_key("evaluations") {
            return Some("evaluations");
        }
    }
    None
}

/// Evaluate a predicate whose `for:` has already been stripped.
fn eval_core(core: &Map<String, Value>, st: &State, seq: Option<&[State]>) -> Res {
    if let Some(all) = core.get("all") {
        let subs: Vec<Res> = match all {
            Value::Array(items) => items
                .iter()
                .map(|p| eval_predicate(p, st, seq))
                .collect(),
            _ => Vec::new(),
        };
        combine_all(&subs)
    } else if let Some(any) = core.get("any") {
        let subs: Vec<Res> = match any {
            Value::Array(items) => items
                .iter()
                .map(|p| eval_predicate(p, st, seq))
                .collect(),
            _ => Vec::new(),
        };
        combine_any(&subs)
    } else if let Some(not) = core.get("not") {
        let sub = eval_predicate(&not.clone(), st, seq);
        let v = match sub.v {
            Tri::True => Tri::False,
            Tri::False => Tri::True,
            Tri::Unevaluable => Tri::Unevaluable,
        };
        Res::new(v, format!("not({})", sub.ev))
    } else {
        eval_leaf(core, st)
    }
}

/// §4.3.1 sequence semantics for a `for:` wrapper. `seq` has >= 2
/// states, sorted by in-game date; seq.last() is the evaluation point.
fn eval_duration_seq(core: &Map<String, Value>, dur: &Value, seq: &[State]) -> Res {
    let dur_s = serde_json::to_string(dur).unwrap_or_default();
    let kind = duration_kind(dur).unwrap_or("");
    let latest = &seq[seq.len() - 1];
    let mut days: Vec<i64> = Vec::new();
    for s in seq {
        match abs_day(s) {
            Ok(d) => days.push(d),
            Err(e) => {
                return Res::uneval(format!(
                    "sequence snapshot date unusable: {e} [for: {dur_s}]"
                ))
            }
        }
    }
    if kind == "months" {
        let months = dur
            .get("months")
            .and_then(as_f64)
            .unwrap_or(0.0);
        let boundary = days[days.len() - 1] as f64 - 30.0 * months;
        let mut anchor: Option<usize> = None;
        for (i, d) in days.iter().enumerate() {
            if (*d as f64) <= boundary {
                anchor = Some(i);
            }
        }
        let mut window: Vec<usize> = Vec::new();
        if let Some(a) = anchor {
            window.push(a);
        }
        for (i, d) in days.iter().enumerate() {
            if (*d as f64) > boundary {
                window.push(i);
            }
        }
        let results: Vec<(usize, Res)> = window
            .iter()
            .map(|&i| (i, eval_core(core, &seq[i], Some(&seq[..=i]))))
            .collect();
        let span = (days[days.len() - 1] - days[window[0]]) as f64 / 30.0;
        for (i, r) in &results {
            if r.v == Tri::False {
                return Res::new(
                    Tri::False,
                    format!(
                        "{} [for: {dur_s} — inner predicate FALSE at {} inside the trailing {}-month window, so the duration did not hold]",
                        r.ev,
                        date_of(&seq[*i]),
                        fmt_f64(months)
                    ),
                );
            }
        }
        for (i, r) in &results {
            if r.v == Tri::Unevaluable {
                return Res::new(
                    Tri::Unevaluable,
                    format!(
                        "{} [for: {dur_s} — inner predicate UNEVALUABLE at {} inside the window; the hold cannot be proven]",
                        r.ev,
                        date_of(&seq[*i])
                    ),
                );
            }
        }
        if anchor.is_none() {
            return Res::new(
                Tri::Unevaluable,
                format!(
                    "inner predicate TRUE at all {} snapshot(s) in the trailing window ({} .. {}) [for: {dur_s} — UNEVALUABLE: history begins inside the window, no anchor snapshot at or before the window start; observed span {span:.2} months < {}]",
                    results.len(),
                    date_of(&seq[window[0]]),
                    date_of(latest),
                    fmt_f64(months)
                ),
            );
        }
        return Res::new(
            Tri::True,
            format!(
                "inner predicate TRUE at every snapshot in the trailing {}-month window (anchor {}, {} snapshot(s), span {span:.2} months) [for: {dur_s}]",
                fmt_f64(months),
                date_of(&seq[anchor.unwrap()]),
                results.len()
            ),
        );
    }
    if kind == "evaluations" {
        let n = dur.get("evaluations").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if seq.len() < n {
            return Res::new(
                Tri::Unevaluable,
                format!(
                    "sequence holds {} snapshot(s) < the {n} evaluations required [for: {dur_s}]",
                    seq.len()
                ),
            );
        }
        let start = seq.len() - n;
        let results: Vec<(usize, Res)> = (start..seq.len())
            .map(|i| (i, eval_core(core, &seq[i], Some(&seq[..=i]))))
            .collect();
        for (i, r) in &results {
            if r.v == Tri::False {
                return Res::new(
                    Tri::False,
                    format!(
                        "{} [for: {dur_s} — inner predicate FALSE at {} within the last {n} evaluations]",
                        r.ev,
                        date_of(&seq[*i])
                    ),
                );
            }
        }
        for (i, r) in &results {
            if r.v == Tri::Unevaluable {
                return Res::new(
                    Tri::Unevaluable,
                    format!(
                        "{} [for: {dur_s} — inner predicate UNEVALUABLE at {} within the last {n} evaluations]",
                        r.ev,
                        date_of(&seq[*i])
                    ),
                );
            }
        }
        return Res::new(
            Tri::True,
            format!(
                "inner predicate TRUE at each of the last {n} evaluations ({} .. {}) [for: {dur_s}]",
                date_of(&seq[start]),
                date_of(latest)
            ),
        );
    }
    Res::uneval(format!("unknown duration shape {dur_s}"))
}

fn eval_predicate(pred: &Value, st: &State, seq: Option<&[State]>) -> Res {
    let obj = match pred.as_object() {
        Some(o) => o,
        None => {
            return Res::uneval(format!(
                "predicate is not a mapping: {}",
                fmt_repr(pred)
            ))
        }
    };
    let duration = obj.get("for").cloned();
    let mut core = obj.clone();
    core.remove("for");
    if let Some(dur) = &duration {
        if let Some(s) = seq {
            if s.len() > 1 && duration_kind(dur).is_some() {
                return eval_duration_seq(&core, dur, s);
            }
        }
    }
    let res = eval_core(&core, st, seq);
    if let Some(dur) = duration {
        let dur_s = serde_json::to_string(&dur).unwrap_or_default();
        return match res.v {
            Tri::False => Res::new(
                Tri::False,
                format!(
                    "{} [for: {dur_s} — currently false, so the duration cannot hold]",
                    res.ev
                ),
            ),
            Tri::True => Res::new(
                Tri::Unevaluable,
                format!(
                    "{} [for: {dur_s} — instantaneous value is TRUE but duration is UNEVALUABLE from a single snapshot: no executor evaluation history in the captures]",
                    res.ev
                ),
            ),
            Tri::Unevaluable => Res::new(
                Tri::Unevaluable,
                format!("{} [for: {dur_s}]", res.ev),
            ),
        };
    }
    res
}

/// A gate field is a list read as an implicit all: (§4.2). Returns the
/// combined result plus the per-predicate results.
fn eval_gate(
    gate: Option<&Value>,
    st: &State,
    seq: Option<&[State]>,
) -> (Res, Vec<(Value, Res)>) {
    let items: &[Value] = match gate {
        Some(Value::Array(a)) => a,
        _ => &[],
    };
    if items.is_empty() {
        return (
            Res::new(Tri::True, "empty gate — enters immediately (§7)".to_string()),
            Vec::new(),
        );
    }
    let results: Vec<(Value, Res)> = items
        .iter()
        .map(|p| (p.clone(), eval_predicate(p, st, seq)))
        .collect();
    let combined = combine_all(&results.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>());
    (combined, results)
}

/// §7/§9 status derivation from evaluated entry/exit Results.
/// Returns (statuses, index of the loop-position phase).
fn derive_statuses(entry_exit: &[(Res, Res)]) -> (Vec<String>, Option<usize>) {
    let mut statuses: Vec<String> = Vec::new();
    let mut current_idx: Option<usize> = None;
    for (i, (entry, exit)) in entry_exit.iter().enumerate() {
        if current_idx.is_some() {
            statuses.push("NOT REACHED".to_string());
        } else if exit.v == Tri::True {
            statuses.push("COMPLETE".to_string());
        } else {
            current_idx = Some(i);
            statuses.push(match entry.v {
                Tri::True => "ACTIVE".to_string(),
                Tri::Unevaluable => "HELD AT ENTRY GATE (UNEVALUABLE)".to_string(),
                Tri::False => "NOT ENTERED — entry gate FALSE".to_string(),
            });
        }
    }
    (statuses, current_idx)
}

// ---------------------------------------------------------------- rendering

fn render_predicate(pred: &Value) -> String {
    let obj = match pred.as_object() {
        Some(o) => o,
        None => return fmt_repr(pred),
    };
    let mut s = if let Some(Value::Array(all)) = obj.get("all") {
        let inner = all
            .iter()
            .map(render_predicate)
            .collect::<Vec<_>>()
            .join("; ");
        format!("all({inner})")
    } else if let Some(Value::Array(any)) = obj.get("any") {
        let inner = any
            .iter()
            .map(render_predicate)
            .collect::<Vec<_>>()
            .join("; ");
        format!("any({inner})")
    } else if let Some(not) = obj.get("not") {
        format!("not({})", render_predicate(not))
    } else {
        let metric = obj
            .get("metric")
            .map(fmt_val)
            .unwrap_or_else(|| "?".to_string());
        let mut sel: Vec<String> = Vec::new();
        for key in ["product", "proto", "state", "window", "node", "storage_id", "edict"] {
            if let Some(v) = obj.get(key) {
                if let Some(m) = v.as_object() {
                    if let Some(ph) = m.get("placeholder") {
                        sel.push(format!("{key}=<placeholder {}>", fmt_val(ph)));
                        continue;
                    }
                }
                sel.push(format!("{key}={}", fmt_val(v)));
            }
        }
        if obj.contains_key("value") || obj.contains_key("op") {
            let op = obj.get("op").map(fmt_val).unwrap_or_else(|| "==".to_string());
            let val = obj.get("value").cloned().unwrap_or(Value::Null);
            format!("{metric}({}) {op} {}", sel.join(", "), fmt_repr(&val))
        } else {
            format!("{metric}({}) == True (implicit)", sel.join(", "))
        }
    };
    if let Some(dur) = obj.get("for") {
        s += &format!("  for: {}", serde_json::to_string(dur).unwrap_or_default());
    }
    s
}

fn contains_placeholder(node: &Value) -> bool {
    match node {
        Value::Object(m) => {
            m.contains_key("placeholder")
                || m.values().any(contains_placeholder)
        }
        Value::Array(a) => a.iter().any(contains_placeholder),
        _ => false,
    }
}

const NON_COMMAND_KINDS: [&str; 4] = ["place-blueprint", "manual-step", "observe-only", "wait"];

fn render_order(order: &Value, st: &State, gate_status: &str, seq: Option<&[State]>) -> Vec<String> {
    let oid = order
        .get("id")
        .map(fmt_val)
        .unwrap_or_else(|| "<no-id>".to_string());
    let do_ = order
        .get("do")
        .map(fmt_val)
        .unwrap_or_else(|| "<no-do>".to_string());
    let mut lines: Vec<String> = Vec::new();
    let pre = order.get("preconditions");
    let (pre_res, has_pre) = match pre {
        Some(Value::Array(a)) if !a.is_empty() => {
            let (r, _) = eval_gate(pre, st, seq);
            (Some(r), true)
        }
        _ => (None, false),
    };
    let mut held: Vec<String> = Vec::new();
    if gate_status != "ACTIVE" {
        held.push(format!("phase not entered ({gate_status})"));
    }
    if let Some(r) = &pre_res {
        if r.v != Tri::True {
            held.push(format!("preconditions {}: {}", r.v.name(), r.ev));
        }
    }
    if let Some(args) = order.get("args") {
        if contains_placeholder(args) {
            held.push(
                "§4.5 placeholder id in args — manual-only until a human sources the id"
                    .to_string(),
            );
        }
    }
    if order
        .get("requires_sandbox_validation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        held.push(
            "requires_sandbox_validation — manual-only on a real save (§6.1/G3)".to_string(),
        );
    }
    if order.get("enabled").and_then(|v| v.as_str()) == Some("on-fallback") {
        held.push("enabled: on-fallback — not due in normal flow".to_string());
    }
    let is_kind = NON_COMMAND_KINDS.contains(&do_.as_str());
    let rendered = if is_kind {
        match do_.as_str() {
            "place-blueprint" => {
                let mut s = format!(
                    "place-blueprint `{}` district={}",
                    order.get("blueprint_ref").map(fmt_val).unwrap_or_else(|| "?".into()),
                    order.get("district").map(fmt_val).unwrap_or_else(|| "?".into())
                );
                if let Some(c) = order.get("count") {
                    s += &format!(" count={}", fmt_val(c));
                }
                if let Some(a) = order.get("asset_status") {
                    s += &format!(" asset_status={}", fmt_val(a));
                }
                s
            }
            "manual-step" => format!(
                "manual-step: {}",
                order.get("step").map(fmt_val).unwrap_or_default().trim()
            ),
            "observe-only" => format!(
                "observe-only: {}",
                order.get("observe").map(fmt_val).unwrap_or_default().trim()
            ),
            _ => format!("wait until: {}", order.get("until").map(fmt_val).unwrap_or_default()),
        }
    } else {
        let args = order.get("args").cloned().unwrap_or(Value::Object(Map::new()));
        format!(
            "command `{do_}` args={}",
            serde_json::to_string(&args).unwrap_or_default()
        )
    };
    let status = if !held.is_empty() {
        format!("HELD — {}", held.join("; "))
    } else if !is_kind {
        "WOULD PREPARE (command, §12 prepare step — not sent)".to_string()
    } else {
        "WOULD PRESENT (human/agent task — not executed)".to_string()
    };
    lines.push(format!("- **{oid}** — {status}"));
    lines.push(format!("  - {rendered}"));
    if let Some(note) = order.get("note") {
        lines.push(format!("  - note: {}", fmt_val(note).trim().to_string()));
    }
    if has_pre {
        let r = pre_res.as_ref().unwrap();
        lines.push(format!(
            "  - preconditions: **{}** — {}",
            r.v.name(),
            r.ev
        ));
    }
    if let Some(verify) = order.get("verify") {
        if let Some(Value::Array(a)) = Some(verify) {
            if !a.is_empty() {
                let (vres, _) = eval_gate(Some(verify), st, seq);
                lines.push(format!(
                    "  - verify (current state): **{}** — {}",
                    vres.v.name(),
                    vres.ev
                ));
            }
        }
    }
    lines
}

// ---------------------------------------------------------------- report

struct Counts {
    t: usize,
    f: usize,
    u: usize,
}

impl Counts {
    fn record(&mut self, r: &Res) {
        match r.v {
            Tri::True => self.t += 1,
            Tri::False => self.f += 1,
            Tri::Unevaluable => self.u += 1,
        }
    }
}

fn build_report(plan_path: &Path, st: &State, seq: Option<&[State]>) -> Result<String, String> {
    let text = std::fs::read_to_string(plan_path)
        .map_err(|e| format!("clean error: cannot read plan file {}: {e}", plan_path.display()))?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "clean error: plan JSON does not parse: {e} (if this is a YAML plan, convert it first: python3 tools/plan_to_json.py)"
        )
    })?;
    let doc_obj = match doc.as_object() {
        Some(o) => o,
        None => return Err("clean error: plan document lacks top-level plan/phases".into()),
    };
    if !doc_obj.contains_key("plan") || !doc_obj.contains_key("phases") {
        return Err("clean error: plan document lacks top-level plan/phases".into());
    }
    let meta = doc_obj.get("plan").cloned().unwrap_or(Value::Null);
    let globals = doc_obj.get("globals").cloned().unwrap_or(Value::Null);
    let phases: Vec<Value> = match doc_obj.get("phases") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    let gi = &st.game_info;
    let mut out: Vec<String> = Vec::new();
    out.push("# DRY RUN — nothing was executed".to_string());
    out.push(String::new());
    out.push("> **DRY RUN — nothing was executed.** This report is a read-only evaluation of a plan file against local state captures. No command was prepared, confirmed, applied or sent anywhere; no game was contacted. Orders below are what a supervised executor *would* prepare next.".to_string());
    out.push(String::new());
    out.push(format!(
        "# Plan dry-run: {} v{}",
        meta.get("id").map(fmt_val).unwrap_or_else(|| "?".into()),
        meta.get("version").map(fmt_val).unwrap_or_else(|| "?".into())
    ));
    out.push(String::new());
    out.push(format!("- Title: {}", meta.get("title").map(fmt_val).unwrap_or_else(|| "?".into())));
    out.push(format!(
        "- Map/difficulty (plan): {} — executor map/difficulty/version match is a live check (§3.1) and cannot be settled from captures; not asserted here.",
        meta.get("map").map(fmt_val).unwrap_or_else(|| "None".into())
    ));
    out.push(format!(
        "- Plan game_version: {}",
        meta.get("game_version").map(fmt_val).unwrap_or_else(|| "None".into())
    ));
    out.push(format!(
        "- Requires bridge: {}",
        meta.get("requires")
            .and_then(|r| r.get("bridge"))
            .map(fmt_val)
            .unwrap_or_else(|| "None".into())
    ));
    out.push(format!(
        "- State source: `{}` capture files — game-info date **{}**, tick **{}**, gameId `{}`, paused={}, speed={}",
        st.dir.display(),
        gi.get("date").map(fmt_val).unwrap_or_else(|| "None".into()),
        gi.get("tick").map(fmt_val).unwrap_or_else(|| "None".into()),
        gi.get("gameId").map(fmt_val).unwrap_or_else(|| "None".into()),
        gi.get("isPaused").map(fmt_val).unwrap_or_else(|| "None".into()),
        gi.get("speed").map(fmt_val).unwrap_or_else(|| "None".into())
    ));
    if let Some(s) = seq {
        if s.len() > 1 {
            out.push(format!(
                "- **Sequence mode (§4.3.1):** {} snapshots, {} .. {}, one gameId `{}`. `for:` durations evaluate over this history; all other predicates read the latest snapshot. Per-prefix phase history is tabulated below.",
                s.len(),
                date_of(&s[0]),
                date_of(&s[s.len() - 1]),
                gi.get("gameId").map(fmt_val).unwrap_or_else(|| "None".into())
            ));
        }
    }
    out.push(format!(
        "- Population {}, housing {} (free {}, homeless {}), monthsOfFood {}, starving {}; researchedCount {}, current research {}",
        st.status.get("population").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.status.get("totalHousingCapacity").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.status.get("freeHousingCapacity").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.status.get("numberOfHomeless").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.status.get("monthsOfFood").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.status.get("arePeopleStarving").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.research.get("researchedCount").map(fmt_val).unwrap_or_else(|| "None".into()),
        st.research
            .get("current")
            .and_then(|c| c.get("id"))
            .map(fmt_val)
            .unwrap_or_else(|| "None".into())
    ));
    out.push(String::new());
    out.push("## State-source mapping used by this evaluator".to_string());
    out.push(String::new());
    out.push("- `product.*` quantity/stored/capacity/flows ← `stats.json` (`Product_`-prefixed ids, exactly as plans name them); windows thisMonth=0, lastMonth=1, thisYear=2, lastYear=3, lifetime=4; `net_flow` = produced − consumed.".to_string());
    out.push("- `product.unlocked` ← `products.json` catalogue; `product.pinned` ← membership of `pinned.json`'s product keys (the pinned=true view, captured in stats shape).".to_string());
    out.push("- `game.*` ← `game-info.json`; `settlement.*` ← `bridge-status.json`; `research.*` ← `bridge-research.json` (progressPerc ÷ 1000); `census.*` ← `bridge-census.json`; `storage.*` ← `bridge-storages.json` (percentFullRaw ÷ 1000, capacity-weighted over matching rows); `trains.line_count` ← `bridge-trains.json`.".to_string());
    let research_note = match get(&st.research, "researched") {
        Some(Value::Array(a)) => format!(
            "`research.completed` reads the capture's v0.6.2 `researched` list ({} nodes), format G1 resolved",
            a.len()
        ),
        _ => "`research.completed` (captures predate CoiBridge v0.6.2 — no `researched` list in `bridge-research.json`, format G1)".to_string(),
    };
    let duration_note = match seq {
        Some(s) if s.len() > 1 => "any `for:` duration the sequence history cannot prove — insufficient history or a blind snapshot inside the window (§4.3.1)".to_string(),
        _ => "any `for:` duration whose instantaneous value is TRUE (a single snapshot cannot prove a duration, §4.3 — see interpretation note)".to_string(),
    };
    out.push(format!(
        "- **UNEVALUABLE by design:** {research_note}, all `power.*` (no power capture, G2), `contracts/loans/edicts` (no v0.6 captures), `time.*` (no executor run anchor), and {duration_note}."
    ));
    out.push(String::new());

    let mut counts = Counts { t: 0, f: 0, u: 0 };

    // ---- globals -------------------------------------------------------
    out.push("## Globals".to_string());
    out.push(String::new());
    if let Some(Value::Array(inv)) = globals.get("invariants") {
        if !inv.is_empty() {
            out.push("### Invariants (§5.2 — evaluated every iteration)".to_string());
            out.push(String::new());
            for pred in inv {
                let res = eval_predicate(pred, st, seq);
                counts.record(&res);
                out.push(format!(
                    "- **{}** — `{}` — {}",
                    res.v.name(),
                    render_predicate(pred),
                    res.ev
                ));
            }
            out.push(String::new());
        }
    }
    if let Some(Value::Array(gchecks)) = globals.get("checks") {
        if !gchecks.is_empty() {
            out.push("### Global checks".to_string());
            out.push(String::new());
            for chk in gchecks {
                let when = chk.get("when").cloned().unwrap_or(Value::Null);
                let res = eval_predicate(&when, st, seq);
                counts.record(&res);
                let verdict = match res.v {
                    Tri::True => "WOULD FIRE",
                    Tri::Unevaluable => "check blind (UNEVALUABLE)",
                    Tri::False => "not firing",
                };
                out.push(format!(
                    "- `{}` ({}): **{}** — {} — {}",
                    chk.get("id").map(fmt_val).unwrap_or_default(),
                    chk.get("severity").map(fmt_val).unwrap_or_default(),
                    res.v.name(),
                    verdict,
                    res.ev
                ));
            }
            out.push(String::new());
        }
    }

    // ---- phases ---------------------------------------------------------
    struct PhaseEval {
        phase: Value,
        entry: Res,
        entry_detail: Vec<(Value, Res)>,
        exit: Res,
        exit_detail: Vec<(Value, Res)>,
        checks: Vec<(Value, Res)>,
    }
    let mut phase_eval: Vec<PhaseEval> = Vec::new();
    for ph in &phases {
        let (entry, entry_detail) = eval_gate(ph.get("entry_gate"), st, seq);
        let (exit, exit_detail) = eval_gate(ph.get("exit_gate"), st, seq);
        for (_, r) in entry_detail.iter().chain(exit_detail.iter()) {
            counts.record(r);
        }
        let mut checks_eval: Vec<(Value, Res)> = Vec::new();
        if let Some(Value::Array(checks)) = ph.get("checks") {
            for chk in checks {
                let when = chk.get("when").cloned().unwrap_or(Value::Null);
                let res = eval_predicate(&when, st, seq);
                counts.record(&res);
                checks_eval.push((chk.clone(), res));
            }
        }
        phase_eval.push(PhaseEval {
            phase: ph.clone(),
            entry,
            entry_detail,
            exit,
            exit_detail,
            checks: checks_eval,
        });
    }

    // Active-phase derivation (§7/§9) — same rule as the reference.
    let (statuses, current_idx) = derive_statuses(
        &phase_eval
            .iter()
            .map(|pe| (pe.entry.clone(), pe.exit.clone()))
            .collect::<Vec<_>>(),
    );
    let (plan_status, active_idx) = match current_idx {
        None => (
            "PLAN COMPLETE (every phase's exit gate is TRUE)".to_string(),
            None,
        ),
        Some(i) => (
            format!(
                "loop position: phase {} of {} — {} [{}]",
                i + 1,
                phases.len(),
                phase_eval[i].phase.get("id").map(fmt_val).unwrap_or_default(),
                statuses[i]
            ),
            Some(i),
        ),
    };

    out.push("## Active-phase determination (§9 play-loop rules)".to_string());
    out.push(String::new());
    for (i, pe) in phase_eval.iter().enumerate() {
        let ph = &pe.phase;
        let why = if statuses[i] == "COMPLETE" {
            format!("exit gate TRUE ({})", pe.exit.ev)
        } else if Some(i) == current_idx && statuses[i] == "ACTIVE" {
            format!(
                "first non-complete phase; entry gate TRUE, exit gate {}",
                pe.exit.v.name()
            )
        } else if Some(i) == current_idx {
            format!(
                "first non-complete phase; entry gate {} — loop holds here, orders do not run (§4.4/§9 step 4)",
                pe.entry.v.name()
            )
        } else {
            "an earlier phase is not complete; phases never overlap (§7)".to_string()
        };
        out.push(format!(
            "- `{}` — **{}** — {}",
            ph.get("id").map(fmt_val).unwrap_or_default(),
            statuses[i],
            why
        ));
    }
    out.push(String::new());
    match active_idx {
        None => out.push(format!("**Active phase: none — {plan_status}**")),
        Some(i) => out.push(format!(
            "**Active phase: {}** — {}",
            phase_eval[i].phase.get("id").map(fmt_val).unwrap_or_default(),
            statuses[i]
        )),
    }
    out.push(String::new());

    // ---- sequence history (per prefix) -----------------------------------
    if let Some(s) = seq {
        if s.len() > 1 {
            out.push("## Sequence phase history (per prefix, §4.3.1)".to_string());
            out.push(String::new());
            out.push("Each row re-derives phase statuses using only the history up to that snapshot — the way a live executor's history grows. The last row is this report's verdict.".to_string());
            out.push(String::new());
            let short = |status: &str| -> &str {
                match status {
                    "COMPLETE" => "COMPLETE",
                    "ACTIVE" => "ACTIVE",
                    "HELD AT ENTRY GATE (UNEVALUABLE)" => "HELD (U)",
                    "NOT ENTERED — entry gate FALSE" => "NOT ENTERED (F)",
                    _ => "NOT REACHED",
                }
            };
            let header = format!(
                "| snapshot | {} |",
                (0..phases.len())
                    .map(|i| format!("phase {i}"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
            out.push(header);
            out.push(format!("|{}|", "---|".repeat(phases.len() + 1)));
            for k in 1..=s.len() {
                let prefix = &s[..k];
                let pst = &s[k - 1];
                let mut pairs: Vec<(Res, Res)> = Vec::new();
                for ph in &phases {
                    let (e, _) = eval_gate(ph.get("entry_gate"), pst, Some(prefix));
                    let (x, _) = eval_gate(ph.get("exit_gate"), pst, Some(prefix));
                    pairs.push((e, x));
                }
                let (p_statuses, _) = derive_statuses(&pairs);
                let row = p_statuses
                    .iter()
                    .map(|x| short(x))
                    .collect::<Vec<_>>()
                    .join(" | ");
                out.push(format!("| {} | {} |", date_of(pst), row));
            }
            out.push(String::new());
            out.push(format!(
                "Phase key: {}",
                phases
                    .iter()
                    .enumerate()
                    .map(|(i, ph)| format!(
                        "phase {i} = `{}`",
                        ph.get("id").map(fmt_val).unwrap_or_default()
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
            out.push(String::new());
        }
    }

    // ---- per-phase detail ------------------------------------------------
    out.push("## Phase-by-phase evaluation".to_string());
    for (i, pe) in phase_eval.iter().enumerate() {
        let ph = &pe.phase;
        out.push(String::new());
        out.push(format!(
            "### {} — {} [{}]",
            ph.get("id").map(fmt_val).unwrap_or_default(),
            ph.get("name").map(fmt_val).unwrap_or_default(),
            statuses[i]
        ));
        out.push(String::new());
        out.push(format!(
            "Intent: {}",
            ph.get("intent").map(fmt_val).unwrap_or_default().trim()
        ));
        out.push(String::new());
        let mut entry_line = format!("**Entry gate: {}**", pe.entry.v.name());
        if pe.entry_detail.is_empty() {
            entry_line += &format!(" — {}", pe.entry.ev);
        }
        out.push(entry_line);
        for (pred, res) in &pe.entry_detail {
            out.push(format!(
                "- **{}** — `{}` — {}",
                res.v.name(),
                render_predicate(pred),
                res.ev
            ));
        }
        out.push(String::new());
        let mut exit_line = format!("**Exit gate: {}**", pe.exit.v.name());
        if pe.exit_detail.is_empty() {
            exit_line += &format!(" — {}", pe.exit.ev);
        }
        out.push(exit_line);
        for (pred, res) in &pe.exit_detail {
            out.push(format!(
                "- **{}** — `{}` — {}",
                res.v.name(),
                render_predicate(pred),
                res.ev
            ));
        }
        if !pe.checks.is_empty() {
            out.push(String::new());
            out.push("Checks (evaluated against current state):".to_string());
            for (chk, res) in &pe.checks {
                let verdict = match res.v {
                    Tri::True => "WOULD FIRE",
                    Tri::Unevaluable => "check blind (UNEVALUABLE)",
                    Tri::False => "not firing",
                };
                out.push(format!(
                    "- `{}` ({}, action={}): **{}** — {} — {}",
                    chk.get("id").map(fmt_val).unwrap_or_default(),
                    chk.get("severity").map(fmt_val).unwrap_or_default(),
                    chk.get("action").map(fmt_val).unwrap_or_default(),
                    res.v.name(),
                    verdict,
                    res.ev
                ));
            }
        }
        if let Some(fb) = ph.get("fallbacks") {
            if !fb.is_null() && fb.as_object().map(|m| !m.is_empty()).unwrap_or(false) {
                out.push(String::new());
                out.push(format!(
                    "Fallbacks: {}",
                    serde_json::to_string(fb).unwrap_or_default()
                ));
            }
        }
    }

    // ---- next orders ------------------------------------------------------
    out.push(String::new());
    out.push("## Next orders (DRY RUN — would be prepared, nothing sent)".to_string());
    out.push(String::new());
    match active_idx {
        None => {
            out.push("No active phase — the plan is complete by its gates; no orders would be prepared.".to_string());
        }
        Some(i) => {
            let ph = &phase_eval[i].phase;
            out.push(format!(
                "For active phase `{}` [{}], in plan order:",
                ph.get("id").map(fmt_val).unwrap_or_default(),
                statuses[i]
            ));
            out.push(String::new());
            if let Some(Value::Array(orders)) = ph.get("orders") {
                for order in orders {
                    out.extend(render_order(order, st, &statuses[i], seq));
                }
            }
            if statuses[i] != "ACTIVE" {
                out.push(String::new());
                out.push("All orders above are held: the phase has not been entered, so no order is due (§9 step 5).".to_string());
            }
        }
    }
    out.push(String::new());
    out.push("## Summary".to_string());
    out.push(String::new());
    out.push(format!(
        "- Predicates evaluated (globals invariants/checks plus phase entry/exit gates and phase checks; composites count once): TRUE {}, FALSE {}, UNEVALUABLE {} (leaf evidence inside composites is in the per-phase tables via the combined evidence strings).",
        counts.t, counts.f, counts.u
    ));
    out.push(format!("- Plan status: {plan_status}."));
    let note1 = match seq {
        Some(s) if s.len() > 1 => "- Interpretation notes: (1) `for:` durations evaluate over the state sequence per §4.3.1 — TRUE only with an anchor snapshot at/before the window start and the inner predicate TRUE at every snapshot in the window; FALSE if it broke anywhere in the window; UNEVALUABLE on insufficient history, never by default. (2) A phase counts COMPLETE purely when its exit gate is TRUE (§7/§9 re-derivation from state); human-confirm manual orders inside a gate-complete phase are not re-litigated by the evaluator. (3) Unknown / out-of-vocabulary metrics evaluate UNEVALUABLE with a closed-vocabulary note, never a crash and never a guess.".to_string(),
        _ => "- Interpretation notes: (1) `for:` durations — instantaneous FALSE stays FALSE; instantaneous TRUE becomes UNEVALUABLE because one snapshot cannot prove the predicate held for the whole duration (§4.3). (2) A phase counts COMPLETE purely when its exit gate is TRUE (§7/§9 re-derivation from state); human-confirm manual orders inside a gate-complete phase are not re-litigated by the evaluator. (3) Unknown / out-of-vocabulary metrics evaluate UNEVALUABLE with a closed-vocabulary note, never a crash and never a guess.".to_string(),
    };
    out.push(note1);
    out.push(String::new());
    out.push("---".to_string());
    out.push("DRY RUN — nothing was executed. Generated by `coi plan-check` (Rust port of `tools/plan_check.py`; read-only over local files).".to_string());
    Ok(out.join("\n") + "\n")
}

// ---------------------------------------------------------------- cmd

/// Load a §4.3.1 state sequence: either a directory that itself holds
/// game-info.json (1-snapshot sequence) or a directory of per-snapshot
/// state dirs. Sorted by in-game date; one gameId required.
fn load_sequence(seq_dir: &Path) -> Result<Vec<State>, String> {
    if !seq_dir.exists() {
        return Err(format!(
            "clean error: state sequence dir not found: {}",
            seq_dir.display()
        ));
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if seq_dir.join("game-info.json").exists() {
        dirs.push(seq_dir.to_path_buf());
    } else {
        let rd = std::fs::read_dir(seq_dir).map_err(|e| {
            format!("clean error: cannot list {}: {e}", seq_dir.display())
        })?;
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("game-info.json").exists() {
                dirs.push(p);
            }
        }
        dirs.sort();
    }
    if dirs.is_empty() {
        return Err(format!(
            "clean error: no snapshot state dirs (each holding game-info.json) found under {}",
            seq_dir.display()
        ));
    }
    let mut states: Vec<State> = Vec::new();
    for d in &dirs {
        states.push(State::load(d)?);
    }
    let mut keyed: Vec<(i64, State)> = Vec::new();
    for s in states {
        let day = abs_day(&s)
            .map_err(|e| format!("clean error: sequence snapshot date unusable: {e}"))?;
        keyed.push((day, s));
    }
    keyed.sort_by_key(|(d, _)| *d);
    let states: Vec<State> = keyed.into_iter().map(|(_, s)| s).collect();
    let first_id = states[0]
        .game_info
        .get("gameId")
        .map(fmt_val)
        .unwrap_or_default();
    if states
        .iter()
        .any(|s| s.game_info.get("gameId").map(fmt_val).unwrap_or_default() != first_id)
    {
        return Err(
            "clean error: state sequence mixes gameIds — a §4.3.1 sequence is one run (one gameId)"
                .to_string(),
        );
    }
    Ok(states)
}

pub fn cmd(args: &[String]) -> i32 {
    let mut plan_path: Option<String> = None;
    let mut state_dir = "live".to_string();
    let mut seq_dir: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--state-dir" => {
                i += 1;
                match args.get(i) {
                    Some(v) => state_dir = v.clone(),
                    None => {
                        eprintln!("clean error: --state-dir needs a value");
                        return 2;
                    }
                }
            }
            "--state-seq" => {
                i += 1;
                match args.get(i) {
                    Some(v) => seq_dir = Some(v.clone()),
                    None => {
                        eprintln!("clean error: --state-seq needs a value");
                        return 2;
                    }
                }
            }
            "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("clean error: --out needs a value");
                        return 2;
                    }
                }
            }
            "-h" | "--help" => {
                eprintln!(
                    "coi plan-check <plan.json> [--state-dir DIR] [--state-seq DIR] [--out report.md]\n\
                     DRY-RUN plan evaluation against a captured state directory\n\
                     (default: ./live), or against a §4.3.1 state sequence\n\
                     with --state-seq (a directory of per-snapshot state\n\
                     dirs). Plans are canonical JSON from\n\
                     tools/plan_to_json.py. Read-only: executes nothing."
                );
                return 0;
            }
            other if plan_path.is_none() && !other.starts_with("--") => {
                plan_path = Some(other.to_string());
            }
            other => {
                eprintln!("clean error: unexpected argument: {other}");
                return 2;
            }
        }
        i += 1;
    }
    let plan_path = match plan_path {
        Some(p) => p,
        None => {
            eprintln!("clean error: no plan file given (usage: coi plan-check <plan.json> [--state-dir DIR] [--out report.md])");
            return 2;
        }
    };
    let plan_pb = PathBuf::from(&plan_path);
    if !plan_pb.exists() {
        eprintln!("clean error: plan file not found: {plan_path}");
        return 2;
    }
    if plan_path.ends_with(".yaml") || plan_path.ends_with(".yml") {
        eprintln!(
            "clean error: plan-check consumes canonical JSON plans; convert first: python3 tools/plan_to_json.py {plan_path}"
        );
        return 2;
    }
    let seq_states: Option<Vec<State>> = match &seq_dir {
        Some(d) => match load_sequence(&PathBuf::from(d)) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("{e}");
                return 2;
            }
        },
        None => None,
    };
    let single_state: Option<State> = match &seq_states {
        Some(_) => None,
        None => {
            let state_pb = PathBuf::from(&state_dir);
            if !state_pb.exists() {
                eprintln!("clean error: state captures dir not found: {state_dir}");
                return 2;
            }
            match State::load(&state_pb) {
                Ok(loaded) => Some(loaded),
                Err(e) => {
                    eprintln!("clean error: {e}");
                    return 2;
                }
            }
        }
    };
    let st: &State = match (&seq_states, &single_state) {
        (Some(s), _) => &s[s.len() - 1],
        (None, Some(one)) => one,
        (None, None) => unreachable!(),
    };
    let seq_ref: Option<&[State]> = seq_states.as_deref();
    match build_report(&plan_pb, st, seq_ref) {
        Ok(report) => {
            if let Some(out) = out_path {
                if let Err(e) = std::fs::write(&out, &report) {
                    eprintln!("clean error: cannot write report {out}: {e}");
                    return 2;
                }
                println!("wrote {out}");
            } else {
                print!("{report}");
            }
            0
        }
        Err(e) => {
            eprintln!("{e}");
            2
        }
    }
}

// ---------------------------------------------------------------- play
//
// `coi play` — the supervised session runner (Round 8). It walks a
// §4.3.1 state sequence prefix by prefix (prefix k = states[0..=k]),
// re-deriving phase statuses from each prefix with the same gate
// machinery `plan-check` uses, and records — in plan order, at the
// first step each phase is ACTIVE — the orders a supervised executor
// would prepare for that phase. Command orders with concrete args
// are recorded as PREPARED with their exact §12 command envelope
// ({"type": <cmd>, "args": {...}}); everything the format reserves
// for humans (non-command kinds, the manual class, §4.5 placeholder
// ids, §6.2 $entity selectors) is recorded as SKIPPED with its
// reason. Phase completions are recorded the first step a phase's
// exit gate evaluates TRUE under that prefix.
//
// HARD SAFETY: this runner is PREPARE-ONLY and has NO apply path.
// It opens no sockets, uses no HTTP client, and never calls the
// §12 prepare/confirm/apply flow — prepared payloads are data in
// the session file for a human to review and confirm in the
// companion website. Its only writes are the session JSON and
// transcript files named on the command line (or stdout).

/// Per-prefix phase evaluation for the play walk: entry/exit gate
/// results (with per-predicate exit detail) plus the §7/§9 derived
/// statuses and loop position for that prefix.
struct PrefixEval {
    statuses: Vec<String>,
    loop_idx: Option<usize>,
    exits: Vec<Res>,
    exit_detail: Vec<Vec<(Value, Res)>>,
}

fn eval_prefix(phases: &[Value], prefix: &[State]) -> PrefixEval {
    let pst = &prefix[prefix.len() - 1];
    let mut pairs: Vec<(Res, Res)> = Vec::new();
    let mut exits: Vec<Res> = Vec::new();
    let mut exit_detail: Vec<Vec<(Value, Res)>> = Vec::new();
    for ph in phases {
        let (entry, _) = eval_gate(ph.get("entry_gate"), pst, Some(prefix));
        let (exit, detail) = eval_gate(ph.get("exit_gate"), pst, Some(prefix));
        pairs.push((entry, exit.clone()));
        exits.push(exit);
        exit_detail.push(detail);
    }
    let (statuses, loop_idx) = derive_statuses(&pairs);
    PrefixEval {
        statuses,
        loop_idx,
        exits,
        exit_detail,
    }
}

/// True when any value inside `v` is a §6.2 `$entity` selector: an
/// object key named `$entity`, or a string mentioning one.
fn contains_entity_selector(v: &Value) -> bool {
    match v {
        Value::Object(m) => {
            m.keys().any(|k| k == "$entity") || m.values().any(contains_entity_selector)
        }
        Value::Array(a) => a.iter().any(contains_entity_selector),
        Value::String(s) => s.contains("$entity"),
        _ => false,
    }
}

struct OrderDecision {
    prepared: bool,
    payload: Option<Value>,
    reason: Option<String>,
}

/// Decide one order under the runner's recording rules. PREPARED
/// only when `do` names a catalogue command (and is not the
/// policy-excluded one) and its args are fully concrete: no §4.5
/// placeholder objects and no §6.2 `$entity` selectors. Everything
/// else is SKIPPED with the reason that applies, checked in this
/// order: non-command kind / manual class, excluded or unknown
/// command, placeholder research id, $entity selector, any other
/// placeholder.
fn decide_order(order: &Value) -> OrderDecision {
    let do_ = order
        .get("do")
        .map(fmt_val)
        .unwrap_or_else(|| "<no-do>".to_string());
    let class = order.get("class").map(fmt_val).unwrap_or_default();
    let args = order
        .get("args")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    let skipped = |reason: String| OrderDecision {
        prepared: false,
        payload: None,
        reason: Some(reason),
    };
    if NON_COMMAND_KINDS.contains(&do_.as_str()) || class == "manual" {
        let reason = if NON_COMMAND_KINDS.contains(&do_.as_str()) {
            format!(
                "{do_} order — non-command kind (§6.3): a human/agent task presented in-game, never prepared as a command"
            )
        } else {
            format!("{do_} order — class manual: manual-only, never prepared as a command")
        };
        return skipped(reason);
    }
    match crate::catalogue::CATALOGUE.iter().find(|c| c.name == do_) {
        None => {
            return skipped(format!(
                "{do_} is not a command in the captured catalogue — cannot be prepared"
            ))
        }
        Some(info) if info.status == "excluded" => {
            return skipped(format!(
                "{do_} is excluded by policy (§5.2) — never prepared"
            ))
        }
        _ => {}
    }
    if do_ == "ResearchStartCmd" {
        if let Some(node) = args.get("nodeId") {
            if contains_placeholder(node) {
                return skipped("placeholder research id (unsourced)".to_string());
            }
        }
    }
    if contains_entity_selector(&args) {
        return skipped("entity selector requires human resolution (§6.2)".to_string());
    }
    if contains_placeholder(&args) {
        return skipped(
            "placeholder id in args (unsourced) — manual-only until a human sources the id"
                .to_string(),
        );
    }
    OrderDecision {
        prepared: true,
        payload: Some(serde_json::json!({"type": do_, "args": args})),
        reason: None,
    }
}

pub fn play_cmd(args: &[String]) -> i32 {
    let mut plan_path: Option<String> = None;
    let mut seq_dir: Option<String> = None;
    let mut session_out: Option<String> = None;
    let mut transcript_out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--state-seq" => {
                i += 1;
                match args.get(i) {
                    Some(v) => seq_dir = Some(v.clone()),
                    None => {
                        eprintln!("clean error: --state-seq needs a value");
                        return 2;
                    }
                }
            }
            "--session-out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => session_out = Some(v.clone()),
                    None => {
                        eprintln!("clean error: --session-out needs a value");
                        return 2;
                    }
                }
            }
            "--transcript-out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => transcript_out = Some(v.clone()),
                    None => {
                        eprintln!("clean error: --transcript-out needs a value");
                        return 2;
                    }
                }
            }
            "-h" | "--help" => {
                eprintln!(
                    "coi play <plan.json> --state-seq DIR [--session-out session.json] [--transcript-out transcript.md]\n\
                     PREPARE-ONLY supervised session walk over a §4.3.1 state\n\
                     sequence (a directory of per-snapshot state dirs). At each\n\
                     prefix it derives phase statuses, records the active phase's\n\
                     orders as prepared payloads or skips with reasons, and halts\n\
                     blocked on the loop phase's exit-gate evidence or complete.\n\
                     It has NO apply path: nothing is sent anywhere; a human\n\
                     confirms prepared payloads in the companion website."
                );
                return 0;
            }
            other if plan_path.is_none() && !other.starts_with("--") => {
                plan_path = Some(other.to_string());
            }
            other => {
                eprintln!("clean error: unexpected argument: {other}");
                return 2;
            }
        }
        i += 1;
    }
    let plan_path = match plan_path {
        Some(p) => p,
        None => {
            eprintln!("clean error: no plan file given (usage: coi play <plan.json> --state-seq DIR [--session-out session.json] [--transcript-out transcript.md])");
            return 2;
        }
    };
    let seq_dir = match seq_dir {
        Some(d) => d,
        None => {
            eprintln!("clean error: --state-seq DIR is required (coi play walks a state sequence)");
            return 2;
        }
    };
    let plan_pb = PathBuf::from(&plan_path);
    if !plan_pb.exists() {
        eprintln!("clean error: plan file not found: {plan_path}");
        return 2;
    }
    if plan_path.ends_with(".yaml") || plan_path.ends_with(".yml") {
        eprintln!(
            "clean error: coi play consumes canonical JSON plans; convert first: python3 tools/plan_to_json.py {plan_path}"
        );
        return 2;
    }
    let doc: Value = {
        let text = match std::fs::read_to_string(&plan_pb) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("clean error: cannot read plan file {}: {e}", plan_pb.display());
                return 2;
            }
        };
        match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("clean error: plan JSON does not parse: {e} (if this is a YAML plan, convert it first: python3 tools/plan_to_json.py)");
                return 2;
            }
        }
    };
    let doc_obj = match doc.as_object() {
        Some(o) => o,
        None => {
            eprintln!("clean error: plan document lacks top-level plan/phases");
            return 2;
        }
    };
    if !doc_obj.contains_key("plan") || !doc_obj.contains_key("phases") {
        eprintln!("clean error: plan document lacks top-level plan/phases");
        return 2;
    }
    let meta = doc_obj.get("plan").cloned().unwrap_or(Value::Null);
    let phases: Vec<Value> = match doc_obj.get("phases") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    let plan_id = meta.get("id").map(fmt_val).unwrap_or_else(|| "?".into());
    let plan_version = meta
        .get("version")
        .map(fmt_val)
        .unwrap_or_else(|| "?".into());
    let states = match load_sequence(&PathBuf::from(&seq_dir)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    let pcount = phases.len();
    let phase_ids: Vec<String> = phases
        .iter()
        .map(|ph| ph.get("id").map(fmt_val).unwrap_or_default())
        .collect();
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut entered_step: Vec<Option<usize>> = vec![None; pcount];
    let mut entered_date: Vec<Option<String>> = vec![None; pcount];
    let mut completed_step: Vec<Option<usize>> = vec![None; pcount];
    let mut completed_date: Vec<Option<String>> = vec![None; pcount];
    let mut was_active: Vec<bool> = vec![false; pcount];
    let mut recorded: HashSet<usize> = HashSet::new();
    let mut orders_out: Vec<Value> = Vec::new();
    let mut steps_out: Vec<Value> = Vec::new();
    let mut transcript_steps: Vec<String> = Vec::new();
    let mut final_eval: Option<PrefixEval> = None;

    for k in 0..states.len() {
        let prefix = &states[..=k];
        let ev = eval_prefix(&phases, prefix);
        let date = date_of(&states[k]);
        for (idx, s) in ev.statuses.iter().enumerate() {
            if s == "ACTIVE" {
                was_active[idx] = true;
            }
        }
        // The one phase whose orders may be recorded at this step:
        // the loop phase, when it is ACTIVE and not yet recorded.
        let record_phase: Option<usize> = match ev.loop_idx {
            Some(idx)
                if ev.statuses[idx] == "ACTIVE"
                    && !recorded.contains(&idx)
                    && matches!(phases[idx].get("orders"), Some(Value::Array(a)) if !a.is_empty()) =>
            {
                Some(idx)
            }
            _ => None,
        };
        let mut events: Vec<Value> = Vec::new();
        let mut t_lines: Vec<String> = Vec::new();
        for idx in 0..pcount {
            if ev.statuses[idx] != "NOT REACHED" && entered_step[idx].is_none() {
                entered_step[idx] = Some(k);
                entered_date[idx] = Some(date.clone());
                events.push(serde_json::json!({
                    "type": "phase_entered",
                    "phase": phase_ids[idx],
                    "status": ev.statuses[idx],
                }));
                t_lines.push(format!(
                    "- Phase entered: `{}` [{}]",
                    phase_ids[idx], ev.statuses[idx]
                ));
            }
            if record_phase == Some(idx) {
                let mut ids: Vec<String> = Vec::new();
                let mut o_lines: Vec<String> = Vec::new();
                if let Some(Value::Array(orders)) = phases[idx].get("orders") {
                    for order in orders {
                        let oid = order
                            .get("id")
                            .map(fmt_val)
                            .unwrap_or_else(|| "<no-id>".to_string());
                        let dec = decide_order(order);
                        ids.push(oid.clone());
                        if dec.prepared {
                            let payload = dec.payload.clone().unwrap_or(Value::Null);
                            o_lines.push(format!(
                                "  - `{oid}` — PREPARED — payload `{}`",
                                serde_json::to_string(&payload).unwrap_or_default()
                            ));
                            orders_out.push(serde_json::json!({
                                "orderId": oid,
                                "phase": phase_ids[idx],
                                "step": k,
                                "gameDate": date,
                                "status": "prepared",
                                "payload": payload,
                                "reason": Value::Null,
                                "confirmation": Value::Null,
                            }));
                        } else {
                            let reason = dec.reason.clone().unwrap_or_default();
                            o_lines.push(format!("  - `{oid}` — SKIPPED — {reason}"));
                            orders_out.push(serde_json::json!({
                                "orderId": oid,
                                "phase": phase_ids[idx],
                                "step": k,
                                "gameDate": date,
                                "status": "skipped",
                                "payload": Value::Null,
                                "reason": reason,
                                "confirmation": Value::Null,
                            }));
                        }
                    }
                }
                recorded.insert(idx);
                events.push(serde_json::json!({
                    "type": "orders_recorded",
                    "phase": phase_ids[idx],
                    "orders": ids,
                }));
                t_lines.push(format!(
                    "- Orders recorded for `{}` (first ACTIVE step, plan order):",
                    phase_ids[idx]
                ));
                t_lines.extend(o_lines);
            }
            if ev.statuses[idx] == "COMPLETE" && completed_step[idx].is_none() {
                completed_step[idx] = Some(k);
                completed_date[idx] = Some(date.clone());
                events.push(serde_json::json!({
                    "type": "phase_completed",
                    "phase": phase_ids[idx],
                    "evidence": ev.exits[idx].ev,
                }));
                t_lines.push(format!(
                    "- Phase completed: `{}` — exit gate TRUE — evidence: {}",
                    phase_ids[idx], ev.exits[idx].ev
                ));
            }
        }
        let mut smap = Map::new();
        for idx in 0..pcount {
            smap.insert(
                phase_ids[idx].clone(),
                Value::String(ev.statuses[idx].clone()),
            );
        }
        let loop_phase: Value = match ev.loop_idx {
            Some(idx) => Value::String(phase_ids[idx].clone()),
            None => Value::Null,
        };
        steps_out.push(serde_json::json!({
            "step": k,
            "stateDir": states[k].dir.display().to_string(),
            "gameDate": date,
            "phaseStatuses": Value::Object(smap),
            "loopPhase": loop_phase,
            "events": Value::Array(events),
        }));
        // Transcript block for this step.
        let dir_name = states[k]
            .dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| states[k].dir.display().to_string());
        let status_str = (0..pcount)
            .map(|idx| format!("`{}` {}", phase_ids[idx], ev.statuses[idx]))
            .collect::<Vec<_>>()
            .join(" · ");
        let loop_str = match ev.loop_idx {
            Some(idx) => format!("`{}` [{}]", phase_ids[idx], ev.statuses[idx]),
            None => "none — every phase COMPLETE".to_string(),
        };
        let mut block = format!(
            "### Step {k} — `{dir_name}` — game date {date}\n\n- Phase statuses: {status_str}\n- Loop phase: {loop_str}\n"
        );
        if t_lines.is_empty() {
            block.push_str("- No phase entries, order recordings or completions at this step.\n");
        } else {
            for l in &t_lines {
                block.push_str(l);
                block.push('\n');
            }
        }
        transcript_steps.push(block);
        final_eval = Some(ev);
    }

    let fe = final_eval.expect("load_sequence yields at least one state");
    let (final_status, final_phase, blockers): (&str, Option<String>, Vec<Value>) =
        match fe.loop_idx {
            None => ("complete", None, Vec::new()),
            Some(idx) => {
                let blockers = fe.exit_detail[idx]
                    .iter()
                    .map(|(pred, res)| {
                        serde_json::json!({
                            "predicate": render_predicate(pred),
                            "verdict": res.v.name(),
                            "evidence": res.ev,
                        })
                    })
                    .collect();
                ("blocked", Some(phase_ids[idx].clone()), blockers)
            }
        };

    let phase_history: Vec<Value> = (0..pcount)
        .map(|idx| {
            serde_json::json!({
                "phase": phase_ids[idx],
                "enteredStep": entered_step[idx].map(|s| serde_json::json!(s)).unwrap_or(Value::Null),
                "enteredDate": entered_date[idx].clone().map(Value::String).unwrap_or(Value::Null),
                "completedStep": completed_step[idx].map(|s| serde_json::json!(s)).unwrap_or(Value::Null),
                "completedDate": completed_date[idx].clone().map(Value::String).unwrap_or(Value::Null),
            })
        })
        .collect();

    let session = serde_json::json!({
        "schema": "coi-play-session/1",
        "plan": {"id": plan_id, "version": plan_version, "file": plan_path},
        "source": {"kind": "state-seq", "dir": seq_dir},
        "startedAtUnix": started,
        "runner": "coi play — prepare-only, no apply path",
        "steps": steps_out,
        "orders": orders_out,
        "phaseHistory": phase_history,
        "final": {
            "status": final_status,
            "phase": final_phase.clone().map(Value::String).unwrap_or(Value::Null),
            "blockers": blockers.clone(),
        },
    });

    // ---- transcript -------------------------------------------------
    let mut t: Vec<String> = Vec::new();
    t.push(format!("# CoI play session — {plan_id} v{plan_version}"));
    t.push(String::new());
    t.push("> **PREPARE-ONLY — nothing was executed.** This transcript is a walk over captured state files by `coi play`, the supervised session runner. The runner has **no apply path**: it opens no sockets, prepares nothing through the §12 flow, and sends nothing to a game. Payloads below are the exact command envelopes a supervised executor *would* prepare; a human reviews them and confirms each one in the companion website.".to_string());
    t.push(String::new());
    t.push(format!("- Plan: `{plan_path}` — `{plan_id}` v{plan_version}"));
    t.push(format!(
        "- Source: state sequence `{seq_dir}` — {} snapshot(s), {} .. {}",
        states.len(),
        date_of(&states[0]),
        date_of(&states[states.len() - 1])
    ));
    t.push("- Runner: coi play — prepare-only, no apply path".to_string());
    t.push(format!("- Started (unix): {started}"));
    t.push(String::new());
    t.push("## Walk (prefix by prefix, §4.3.1)".to_string());
    t.push(String::new());
    t.push("Step k uses the prefix states[0..=k]; phase statuses are re-derived from that prefix alone, exactly as a live executor's history grows. An order is recorded once, at the first step its phase is ACTIVE.".to_string());
    t.push(String::new());
    for block in &transcript_steps {
        t.push(block.clone());
    }
    t.push("## Phase history".to_string());
    t.push(String::new());
    t.push("| phase | entered (step · date) | completed (step · date) |".to_string());
    t.push("|---|---|---|".to_string());
    for idx in 0..pcount {
        let entered = match (entered_step[idx], &entered_date[idx]) {
            (Some(s), Some(d)) => format!("step {s} · {d}"),
            _ => "— (never reached)".to_string(),
        };
        let completed = match (completed_step[idx], &completed_date[idx]) {
            (Some(s), Some(d)) => format!("step {s} · {d}"),
            _ => "—".to_string(),
        };
        t.push(format!("| `{}` | {entered} | {completed} |", phase_ids[idx]));
    }
    t.push(String::new());
    for idx in 0..pcount {
        if completed_step[idx].is_some() && !was_active[idx] {
            t.push(format!(
                "- Note: `{}` reached COMPLETE without an ACTIVE step in this sequence (it was already gate-complete when it first became reachable, or moved HELD → COMPLETE), so under the runner's rule — orders are recorded at a phase's first ACTIVE step — none of its orders were recorded.",
                phase_ids[idx]
            ));
        }
    }
    t.push(String::new());
    if final_status == "complete" {
        t.push("## Halt — COMPLETE".to_string());
        t.push(String::new());
        t.push("Every phase's exit gate is TRUE at the final prefix; the plan is complete by its gates. Nothing was executed — any prepared payloads above remain for a human to confirm in the companion website.".to_string());
    } else {
        let fp = final_phase.clone().unwrap_or_default();
        t.push(format!("## Halt — BLOCKED on `{fp}`"));
        t.push(String::new());
        t.push(format!(
            "After the final snapshot the loop position is `{fp}`, whose exit gate is not TRUE. Per-predicate exit-gate results at the final prefix (engine evidence, quoted verbatim):"
        ));
        t.push(String::new());
        for b in &blockers {
            t.push(format!(
                "- `{}` — **{}** — {}",
                b.get("predicate").map(fmt_val).unwrap_or_default(),
                b.get("verdict").map(fmt_val).unwrap_or_default(),
                b.get("evidence").map(fmt_val).unwrap_or_default()
            ));
        }
        t.push(String::new());
        t.push("The session halts here. The next action is human: review the prepared payloads in the session JSON and confirm them in the companion website (§12); the runner itself applies nothing.".to_string());
    }
    t.push(String::new());
    t.push("---".to_string());
    t.push("PREPARE-ONLY — nothing was executed. Generated by `coi play` (prepare-only session runner; read-only over local files; no apply path).".to_string());
    let transcript = t.join("\n") + "\n";

    let session_text = serde_json::to_string_pretty(&session).unwrap_or_default() + "\n";
    match &session_out {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &session_text) {
                eprintln!("clean error: cannot write session {path}: {e}");
                return 2;
            }
            println!("wrote {path}");
        }
        None => print!("{session_text}"),
    }
    match &transcript_out {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &transcript) {
                eprintln!("clean error: cannot write transcript {path}: {e}");
                return 2;
            }
            println!("wrote {path}");
        }
        None => print!("{transcript}"),
    }
    0
}
