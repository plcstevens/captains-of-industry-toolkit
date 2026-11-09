//! Offline `.save` file inspector — a Rust port of the browser save
//! reader (`~/workspace/coi-save-reader/site/coi-save-parser.js`), whose
//! format knowledge came from probe/PROBE.md + probe/CRACKING.md.
//!
//! Parses the 40-byte header + CRC32 checks, gunzips the payload, reads
//! the ModTypes table and the SaveInfo block (game version, map name,
//! save counter, embedded screenshot), and scans for the difficulty
//! preset, map seed, data-table vocabulary, and the currently-researched
//! node. Read-only: never writes to the save file.

use serde_json::{json, Value};
use std::io::Read;

pub fn cmd(args: &[String]) -> i32 {
    let mut path: Option<String> = None;
    let mut json_out = false;
    let mut shot_out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_out = true,
            "--extract-screenshot" => {
                i += 1;
                shot_out = args.get(i).cloned();
            }
            other => {
                if path.is_none() {
                    path = Some(other.to_string());
                } else {
                    eprintln!("unexpected argument: {other}");
                    return 2;
                }
            }
        }
        i += 1;
    }
    let path = match path {
        Some(p) => p,
        None => {
            eprintln!("usage: coi saveinfo <file.save> [--json] [--extract-screenshot out.jpg]");
            return 2;
        }
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("saveinfo: cannot read {path}: {e}");
            return 1;
        }
    };
    let info = parse_save(&bytes);
    if let Some(out) = &shot_out {
        match info.screenshot_range {
            Some((off, len)) => {
                // Screenshot bytes live in the decompressed payload; re-derive.
                match extract_screenshot(&bytes) {
                    Some(jpeg) if jpeg.len() == len && off == off => {
                        if let Err(e) = std::fs::write(out, &jpeg) {
                            eprintln!("saveinfo: cannot write screenshot: {e}");
                            return 1;
                        }
                        println!("screenshot written: {out} ({} bytes)", jpeg.len());
                    }
                    _ => {
                        eprintln!("saveinfo: screenshot extraction failed");
                        return 1;
                    }
                }
            }
            None => {
                eprintln!("saveinfo: no screenshot found in this save");
                return 1;
            }
        }
    }
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&info.to_json(&path)).unwrap()
        );
    } else {
        print!("{}", info.summary(&path));
    }
    if info.ok {
        0
    } else {
        1
    }
}

// ---------------------------------------------------------------------------
// CRC32 (ISO-HDLC, same as zlib crc32)
// ---------------------------------------------------------------------------

fn crc32(bytes: &[u8]) -> u32 {
    static mut TABLE: [u32; 256] = [0; 256];
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        for n in 0..256u32 {
            let mut c = n;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            TABLE[n as usize] = c;
        }
    });
    let table = unsafe { TABLE };
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xFF) as usize];
    }
    crc ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------
// Byte reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    b: &'a [u8],
    o: usize,
}

type R<T> = Result<T, String>;

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, o: 0 }
    }
    fn u8(&mut self) -> R<u8> {
        let v = *self.b.get(self.o).ok_or("unexpected end of data")?;
        self.o += 1;
        Ok(v)
    }
    fn leb(&mut self) -> R<u64> {
        let mut v: u64 = 0;
        let mut s: u32 = 0;
        loop {
            let x = self.u8()?;
            v += ((x & 0x7F) as u64) << s;
            s += 7;
            if x & 0x80 == 0 {
                break;
            }
            if s > 63 {
                return Err("LEB128 value too large".into());
            }
        }
        Ok(v)
    }
    fn bytes(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self.o.checked_add(n).ok_or("offset overflow")?;
        let r = self.b.get(self.o..end).ok_or("unexpected end of data")?;
        self.o = end;
        Ok(r)
    }
    fn str(&mut self) -> R<String> {
        let n = self.leb()? as usize;
        let raw = self.bytes(n)?;
        // JS reader decoded byte-per-char (latin1); names are ASCII in practice.
        Ok(raw.iter().map(|&b| b as char).collect())
    }
    fn u32(&mut self) -> R<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> R<u64> {
        let b = self.bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
}

fn ascii(b: &[u8], from: usize, to: usize) -> String {
    b[from..to].iter().map(|&c| c as char).collect()
}

fn leb_at(b: &[u8], mut o: usize) -> R<(u64, usize)> {
    let mut v: u64 = 0;
    let mut s: u32 = 0;
    loop {
        let x = *b.get(o).ok_or("unexpected end of data")?;
        o += 1;
        v += ((x & 0x7F) as u64) << s;
        s += 7;
        if x & 0x80 == 0 {
            break;
        }
        if s > 63 {
            return Err("LEB128 value too large".into());
        }
    }
    Ok((v, o))
}

// ---------------------------------------------------------------------------
// Save parse
// ---------------------------------------------------------------------------

fn format_to_game(v: u32) -> Option<&'static str> {
    match v {
        329 => Some("0.8.7b"),
        331 => Some("0.8.7c"),
        333 => Some("0.8.7d"),
        _ => None,
    }
}

pub struct SaveInfo {
    pub ok: bool,
    pub errors: Vec<String>,
    pub magic_ok: bool,
    pub format_version: Option<u32>,
    pub header_field12: Option<u32>,
    pub compressed_size: Option<u64>,
    pub compressed_size_ok: bool,
    pub stream_crc_ok: bool,
    pub payload_size: Option<usize>,
    pub payload_size_ok: bool,
    pub payload_crc_ok: bool,
    pub save_stop_ok: bool,
    pub file_size: usize,
    pub mods: Vec<(u64, String)>,
    pub save_info_type: Option<String>,
    pub counter: Option<u64>,
    pub game_version: Option<String>,
    pub map_name: Option<String>,
    pub screenshot_range: Option<(usize, usize)>, // (offset, len) in payload
    pub screenshot_dims: Option<(u32, u32)>,
    pub screenshot_soi_ok: bool,
    pub screenshot_eoi_ok: bool,
    pub difficulty_preset: Option<String>,
    pub map_seed: Option<String>,
    pub vocab_counts: (usize, usize, usize, usize),
    pub research: Option<Value>,
}

impl SaveInfo {
    fn blank(file_size: usize) -> Self {
        SaveInfo {
            ok: false,
            errors: Vec::new(),
            magic_ok: false,
            format_version: None,
            header_field12: None,
            compressed_size: None,
            compressed_size_ok: false,
            stream_crc_ok: false,
            payload_size: None,
            payload_size_ok: false,
            payload_crc_ok: false,
            save_stop_ok: false,
            file_size,
            mods: Vec::new(),
            save_info_type: None,
            counter: None,
            game_version: None,
            map_name: None,
            screenshot_range: None,
            screenshot_dims: None,
            screenshot_soi_ok: false,
            screenshot_eoi_ok: false,
            difficulty_preset: None,
            map_seed: None,
            vocab_counts: (0, 0, 0, 0),
            research: None,
        }
    }

    pub fn to_json(&self, path: &str) -> Value {
        json!({
            "file": path,
            "ok": self.ok,
            "errors": self.errors,
            "fileSize": self.file_size,
            "magicOk": self.magic_ok,
            "formatVersion": self.format_version,
            "gameVersionForFormat": self.format_version.and_then(format_to_game),
            "headerField12": self.header_field12,
            "compressedSize": self.compressed_size,
            "compressedSizeOk": self.compressed_size_ok,
            "streamCrcOk": self.stream_crc_ok,
            "payloadSize": self.payload_size,
            "payloadSizeOk": self.payload_size_ok,
            "payloadCrcOk": self.payload_crc_ok,
            "saveStopOk": self.save_stop_ok,
            "mods": self.mods.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
            "saveInfoType": self.save_info_type,
            "counter": self.counter,
            "gameVersion": self.game_version,
            "mapName": self.map_name,
            "screenshot": self.screenshot_range.map(|(off, len)| json!({
                "offset": off, "length": len,
                "soiOk": self.screenshot_soi_ok, "eoiOk": self.screenshot_eoi_ok,
                "dims": self.screenshot_dims.map(|(w, h)| json!({"width": w, "height": h})),
            })),
            "difficultyPreset": self.difficulty_preset,
            "mapSeed": self.map_seed,
            "vocabCounts": {
                "products": self.vocab_counts.0,
                "research": self.vocab_counts.1,
                "goals": self.vocab_counts.2,
                "contracts": self.vocab_counts.3,
            },
            "research": self.research,
        })
    }

    pub fn summary(&self, path: &str) -> String {
        let mut s = String::new();
        s.push_str(&format!("Save: {path} ({} bytes)\n", self.file_size));
        match self.format_version {
            Some(v) => s.push_str(&format!(
                "  format {} ({})\n",
                v,
                format_to_game(v).unwrap_or("unknown — newer/older than the 0.8.7d table")
            )),
            None => s.push_str("  format: unreadable\n"),
        }
        if let Some(gv) = &self.game_version {
            s.push_str(&format!("  game version (SaveInfo): {gv}\n"));
        }
        if let Some(m) = &self.map_name {
            s.push_str(&format!("  map: {m}\n"));
        }
        if let Some(c) = self.counter {
            s.push_str(&format!("  save counter: {c}\n"));
        }
        if let Some(d) = &self.difficulty_preset {
            s.push_str(&format!("  difficulty preset: {d}\n"));
        }
        if let Some(seed) = &self.map_seed {
            s.push_str(&format!("  map seed: {seed}\n"));
        }
        s.push_str(&format!(
            "  integrity: magic {} · stream CRC {} · payload size {} · payload CRC {} · end marker {}\n",
            yn(self.magic_ok),
            yn(self.stream_crc_ok),
            yn(self.payload_size_ok),
            yn(self.payload_crc_ok),
            yn(self.save_stop_ok),
        ));
        if self.mods.is_empty() {
            s.push_str("  mods: none (vanilla + DLC only)\n");
        } else {
            let names: Vec<String> = self.mods.iter().map(|(_, n)| n.clone()).collect();
            s.push_str(&format!("  mods: {}\n", names.join(", ")));
        }
        if let Some((off, len)) = self.screenshot_range {
            let dims = self
                .screenshot_dims
                .map(|(w, h)| format!("{w}x{h}"))
                .unwrap_or_else(|| "?".into());
            s.push_str(&format!(
                "  screenshot: {len} bytes at payload offset {off}, {dims}, SOI {} / EOI {}\n",
                yn(self.screenshot_soi_ok),
                yn(self.screenshot_eoi_ok)
            ));
        }
        if let Some(r) = &self.research {
            s.push_str(&format!(
                "  research: {} nodes; currently researching: {}\n",
                r["nodeCount"],
                r["currentId"].as_str().unwrap_or("(none / unnamed)")
            ));
        }
        s.push_str(&format!(
            "  vocabulary: {} products, {} research ids, {} goals, {} contracts\n",
            self.vocab_counts.0, self.vocab_counts.1, self.vocab_counts.2, self.vocab_counts.3
        ));
        for e in &self.errors {
            s.push_str(&format!("  ERROR: {e}\n"));
        }
        s
    }
}

fn yn(b: bool) -> &'static str {
    if b {
        "OK"
    } else {
        "FAIL"
    }
}

fn gunzip(stream: &[u8]) -> R<Vec<u8>> {
    let mut d = flate2::read::GzDecoder::new(stream);
    let mut out = Vec::new();
    d.read_to_end(&mut out).map_err(|e| format!("gzip failed: {e}"))?;
    Ok(out)
}

fn extract_screenshot(bytes: &[u8]) -> Option<Vec<u8>> {
    let stream = bytes.get(40..)?;
    let payload = gunzip(stream).ok()?;
    let parsed = parse_payload_head(&payload).ok()?;
    let (off, len, _, _, _) = parsed.5;
    payload.get(off..off + len).map(|s| s.to_vec())
}

/// Parses ModTypes + SaveInfo; returns (mods, type, counter, gameVersion,
/// mapName, screenshot(offset,len,dims,soi,eoi)).
#[allow(clippy::type_complexity)]
fn parse_payload_head(
    payload: &[u8],
) -> R<(
    Vec<(u64, String)>,
    String,
    u64,
    String,
    String,
    (usize, usize, Option<(u32, u32)>, bool, bool),
)> {
    let mut p = Reader::new(payload);
    if ascii(payload, 0, 8) != "sepyTdoM" {
        return Err("ModTypes marker missing".into());
    }
    p.o = 8;
    let count = p.leb()?;
    let mut mods = Vec::new();
    for _ in 0..count {
        let id = p.leb()?;
        let name = p.str()?;
        p.bytes(8)?;
        mods.push((id, name));
    }
    if ascii(payload, p.o, p.o + 8) != "ofnIevaS" {
        return Err(format!("SaveInfo marker missing at {}", p.o));
    }
    p.o += 8;
    p.u8()?;
    p.u8()?;
    let ty = p.str()?;
    let counter = p.leb()?;
    if p.u8()? != 0x07 {
        return Err("SaveInfo tag 07 (game version) missing".into());
    }
    let game_version = p.str()?;
    p.bytes(3)?;
    if p.u8()? != 0x08 {
        return Err("SaveInfo tag 08 (map name) missing".into());
    }
    let map_name = p.str()?;
    if p.u8()? != 0x09 {
        return Err("SaveInfo tag 09 missing".into());
    }
    p.u8()?;
    p.leb()?;
    p.leb()?;
    p.leb()?;
    p.u8()?;
    if p.u8()? != 0x0A {
        return Err("SaveInfo tag 0a (screenshot) missing".into());
    }
    p.u8()?; // screenshot sub-tag
    p.str()?; // screenshot type
    let jlen = p.leb()? as usize;
    p.bytes(4)?; // screenshot float
    let jstart = p.o;
    let jpeg = payload
        .get(jstart..jstart + jlen)
        .ok_or("screenshot out of bounds")?;
    let soi = jpeg.len() >= 2 && jpeg[0] == 0xFF && jpeg[1] == 0xD8;
    let eoi = jpeg.len() >= 2 && jpeg[jpeg.len() - 2] == 0xFF && jpeg[jpeg.len() - 1] == 0xD9;
    let dims = jpeg_dims(jpeg);
    Ok((mods, ty, counter, game_version, map_name, (jstart, jlen, dims, soi, eoi)))
}

fn jpeg_dims(j: &[u8]) -> Option<(u32, u32)> {
    let mut o = 2usize;
    while o + 9 < j.len() {
        if j[o] != 0xFF {
            o += 1;
            continue;
        }
        let m = j[o + 1];
        if m == 0xC0 || m == 0xC1 || m == 0xC2 {
            let h = ((j[o + 5] as u32) << 8) | j[o + 6] as u32;
            let w = ((j[o + 7] as u32) << 8) | j[o + 8] as u32;
            return Some((w, h));
        }
        if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) {
            o += 2;
            continue;
        }
        let len = ((j[o + 2] as usize) << 8) | j[o + 3] as usize;
        if len < 2 {
            break;
        }
        o += 2 + len;
    }
    None
}

fn parse_save(bytes: &[u8]) -> SaveInfo {
    let mut res = SaveInfo::blank(bytes.len());
    if bytes.len() < 40 {
        res.errors
            .push("File is smaller than the 40-byte save header.".into());
        return res;
    }
    res.magic_ok = ascii(bytes, 0, 8) == "evaSiFaM";
    if !res.magic_ok {
        res.errors.push("Magic mismatch: not a Captain of Industry save (expected \"evaSiFaM\").".into());
    }
    let mut r = Reader::new(bytes);
    r.o = 8;
    let header = (|| -> R<(u32, u32, u64, u32, u64, u32)> {
        Ok((
            r.u32()?,
            r.u32()?,
            r.u64()?,
            r.u32()?,
            r.u64()?,
            r.u32()?,
        ))
    })();
    let (format_version, field12, comp_size, stream_crc, payload_size_decl, payload_crc) =
        match header {
            Ok(h) => h,
            Err(e) => {
                res.errors.push(format!("header parse failed: {e}"));
                return res;
            }
        };
    res.format_version = Some(format_version);
    res.header_field12 = Some(field12);
    res.compressed_size = Some(comp_size);
    let stream = &bytes[40..];
    res.compressed_size_ok = comp_size == stream.len() as u64;
    res.stream_crc_ok = crc32(stream) == stream_crc;

    let payload = match gunzip(stream) {
        Ok(p) => p,
        Err(e) => {
            res.errors.push(format!("gzip decompression failed: {e}"));
            return res;
        }
    };
    res.payload_size = Some(payload.len());
    res.payload_size_ok = payload_size_decl == payload.len() as u64;
    res.payload_crc_ok = crc32(&payload) == payload_crc;
    res.save_stop_ok = payload.len() >= 8 && ascii(&payload, payload.len() - 8, payload.len()) == "potSevaS";

    match parse_payload_head(&payload) {
        Ok((mods, ty, counter, gv, map, (off, len, dims, soi, eoi))) => {
            res.mods = mods;
            res.save_info_type = Some(ty);
            res.counter = Some(counter);
            res.game_version = Some(gv);
            res.map_name = Some(map);
            res.screenshot_range = Some((off, len));
            res.screenshot_dims = dims;
            res.screenshot_soi_ok = soi;
            res.screenshot_eoi_ok = eoi;
        }
        Err(e) => res.errors.push(format!("SaveInfo parse failed: {e}")),
    }

    res.difficulty_preset = scan_difficulty(&payload);
    res.map_seed = scan_map_seed(&payload);
    res.vocab_counts = extract_vocab_counts(&payload);
    res.research = extract_research(&payload).ok().flatten();
    res.ok = res.errors.is_empty();
    res
}

fn scan_difficulty(payload: &[u8]) -> Option<String> {
    let needle = b"GameDifficulty__";
    let mut i = 0;
    while i + needle.len() < payload.len() {
        if &payload[i..i + needle.len()] == needle {
            let start = i + needle.len();
            let mut end = start;
            while end < payload.len() && payload[end].is_ascii_alphabetic() {
                end += 1;
            }
            if end > start {
                // The JS regex ([A-Za-z]+)(Title|Description|Explanation)
                // relies on backtracking: the alpha run may swallow the
                // suffix (e.g. "NormalDescription"), so try split points
                // from longest capture to shortest.
                for split in (start + 1..end).rev() {
                    let rest = &payload[split..];
                    if rest.starts_with(b"Title")
                        || rest.starts_with(b"Description")
                        || rest.starts_with(b"Explanation")
                    {
                        return Some(
                            payload[start..split].iter().map(|&b| b as char).collect(),
                        );
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn scan_map_seed(payload: &[u8]) -> Option<String> {
    let limit = payload.len().min(400_000);
    let mut i = 0;
    while i + 14 <= limit {
        if payload[i] == 0x04 && payload[i + 1] == 0x0C {
            let cand = &payload[i + 2..i + 14];
            if cand.iter().all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
                return Some(cand.iter().map(|&b| b as char).collect());
            }
        }
        i += 1;
    }
    None
}

/// Token scanner equivalent to the JS regex vocab extraction: find
/// tokens starting with a prefix and extending over [A-Za-z0-9_].
fn scan_tokens(payload: &[u8], prefix: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + prefix.len() <= payload.len() {
        if &payload[i..i + prefix.len()] == prefix {
            let mut end = i + prefix.len();
            while end < payload.len()
                && (payload[end].is_ascii_alphanumeric() || payload[end] == b'_')
            {
                end += 1;
            }
            if end > i + prefix.len() {
                out.push(payload[i..end].iter().map(|&b| b as char).collect());
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

fn uniq_sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

fn extract_vocab_counts(payload: &[u8]) -> (usize, usize, usize, usize) {
    let products: Vec<String> = scan_tokens(payload, b"Product_")
        .into_iter()
        .filter(|s| !s.contains("_For_"))
        .collect();
    let research: Vec<String> = scan_tokens(payload, b"Research")
        .into_iter()
        .filter(|s| {
            s.len() > 8 && s.as_bytes()[8].is_ascii_uppercase() && s != "ResearchManager"
        })
        .collect();
    let goals = scan_tokens(payload, b"Goal_");
    let mut contracts = scan_tokens(payload, b"Contract_");
    contracts.extend(scan_tokens(payload, b"Trade_"));
    (
        uniq_sorted(products).len(),
        uniq_sorted(research).len(),
        uniq_sorted(goals).len(),
        uniq_sorted(contracts).len(),
    )
}

// ---------------------------------------------------------------------------
// Research state (port of the JS extractResearch; see CRACKING.md §4)
// ---------------------------------------------------------------------------

fn extract_research(payload: &[u8]) -> R<Option<Value>> {
    // --- catalogue from proto registry blocks ---
    // Tokens "Research[A-Za-z0-9]+" preceded by a length byte == token len,
    // followed by (slimId LEB, tail LEB); keep slimIds in 1500..=8000.
    let mut cand: Vec<(u64, String)> = Vec::new();
    let mut i = 0;
    while i < payload.len() {
        if payload[i..].starts_with(b"Research") {
            let mut end = i + 8;
            while end < payload.len() && payload[end].is_ascii_alphanumeric() {
                end += 1;
            }
            let tok_len = end - i;
            if tok_len > 8 && i > 0 && payload[i - 1] as usize == tok_len {
                if let Ok((slim, after1)) = leb_at(payload, end) {
                    if let Ok((_tail, _)) = leb_at(payload, after1) {
                        if (1500..=8000).contains(&slim) {
                            cand.push((
                                slim,
                                payload[i..end].iter().map(|&b| b as char).collect(),
                            ));
                        }
                    }
                }
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    let mut catalogue: Vec<String> = Vec::new();
    let mut cur: Vec<(u64, String)> = Vec::new();
    for c in cand {
        if !cur.is_empty() && c.0 == cur[cur.len() - 1].0 + 1 {
            cur.push(c);
        } else {
            if cur.len() >= 3 {
                catalogue.extend(cur.drain(..).map(|x| x.1));
            }
            cur = vec![c];
        }
    }
    if cur.len() >= 3 {
        catalogue.extend(cur.into_iter().map(|x| x.1));
    }

    // --- node directory + current-research field ---
    let prefix = b"Mafi.Core.Research.ResearchNode, Mafi.Core, Version=";
    let suffix = b", Culture=neutral, PublicKeyToken=null";
    let mut pos = 0;
    while pos + prefix.len() <= payload.len() {
        if payload[pos..].starts_with(prefix) {
            let mut o = pos + prefix.len();
            while o < payload.len() && (payload[o].is_ascii_digit() || payload[o] == b'.') {
                o += 1;
            }
            if payload[o..].starts_with(suffix) {
                o += suffix.len();
                if let Some(v) = parse_research_directory(payload, o, &catalogue) {
                    return Ok(Some(v));
                }
            }
        }
        pos += 1;
    }
    Ok(None)
}

fn parse_research_directory(payload: &[u8], mut o: usize, catalogue: &[String]) -> Option<Value> {
    let (count, no) = leb_at(payload, o).ok()?;
    o = no;
    if !(100..=400).contains(&count) {
        return None;
    }
    let mut base: Option<u64> = None;
    let mut type_id: Option<u64> = None;
    for k in 0..count {
        let (pa, no) = leb_at(payload, o).ok()?;
        o = no;
        let (pb, no) = leb_at(payload, o).ok()?;
        o = no;
        if base.is_none() {
            base = Some(pa);
            type_id = Some(pb);
        }
        if pa != base? + k || pb != type_id? {
            return None;
        }
    }
    let base = base?;
    let (flag, no) = leb_at(payload, o).ok()?;
    o = no;
    let mut current_index: Option<u64> = None;
    if flag == 1 {
        let (obj, no) = leb_at(payload, o).ok()?;
        o = no;
        let idx = obj.checked_sub(base)?;
        if idx < count {
            current_index = Some(idx);
        }
    } else if flag != 0 {
        return None;
    }
    // following field (unidentified): marker 0x01 + three (x, x+18) pairs
    let mut triples: Option<(u64, u64, u64)> = None;
    if let Ok((mk, no)) = leb_at(payload, o) {
        if mk == 1 {
            let mut vs = Vec::new();
            let mut oo = no;
            let mut okk = true;
            for _ in 0..6 {
                match leb_at(payload, oo) {
                    Ok((v, nno)) => {
                        vs.push(v);
                        oo = nno;
                    }
                    Err(_) => {
                        okk = false;
                        break;
                    }
                }
            }
            if okk && vs.len() == 6 && vs[1] == vs[0] + 18 && vs[3] == vs[2] + 18 && vs[5] == vs[4] + 18 {
                triples = Some((vs[0], vs[2], vs[4]));
            }
        }
    }
    let _ = o;
    Some(json!({
        "nodeCount": count,
        "currentIndex": current_index,
        "currentId": current_index.and_then(|ix| catalogue.get(ix as usize)).cloned(),
        "catalogueCount": catalogue.len(),
        "tripleIndices": triples.map(|(a, b, c)| json!([a, b, c])),
    }))
}
