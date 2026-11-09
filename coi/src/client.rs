//! Minimal HTTP/1.1 GET client for the CoI web API.
//!
//! Why hand-rolled: the game server demands `Host: 127.0.0.1:3073` (or
//! localhost) even when reached through a raw TCP tunnel forward under
//! a different name, and some access paths go through an HTTP proxy
//! with CONNECT tunnelling. A tiny client gives exact control of both.
//! Plain HTTP only — every supported path is loopback or a private
//! tunnel TCP forward.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct Client {
    /// Authority used to dial (direct) or CONNECT (via proxy): host:port.
    pub authority: String,
    /// Host header sent to the game. Defaults to `authority`.
    pub host_header: String,
    /// Optional HTTP proxy for CONNECT tunnelling.
    pub proxy: Option<ProxyCfg>,
    pub timeout_secs: u64,
}

/// Proxy endpoint plus optional Basic credentials (never printed).
#[derive(Clone)]
pub struct ProxyCfg {
    pub dial: String,
    pub auth_b64: Option<String>,
}

/// Parse a proxy given as full URL (`http://user:pass@host:port`) or
/// plain `host:port`. Credentials, if any, are kept only as a base64
/// Proxy-Authorization value and are never included in error messages.
pub fn parse_proxy(raw: &str) -> ProxyCfg {
    let rest = raw.split("://").nth(1).unwrap_or(raw);
    match rest.rfind('@') {
        Some(at) => ProxyCfg {
            dial: rest[at + 1..].trim_end_matches('/').to_string(),
            auth_b64: Some(base64_encode(&rest.as_bytes()[..at])),
        },
        None => ProxyCfg {
            dial: rest.trim_end_matches('/').to_string(),
            auth_b64: None,
        },
    }
}

/// Derive a CONNECT tunnel proxy from the HTTPS_PROXY environment
/// variable by replacing its port with 3130 — the recipe some hosted
/// runtimes use to expose a private-tunnel proxy to processes.
/// Returns None when HTTPS_PROXY is unset.
pub fn env_proxy() -> Option<ProxyCfg> {
    let hp = std::env::var("HTTPS_PROXY").ok()?;
    let (scheme, rest) = match hp.split_once("://") {
        Some((s, r)) => (format!("{s}://"), r.to_string()),
        None => (String::new(), hp.clone()),
    };
    let rest = match rest.rfind(':') {
        Some(idx)
            if rest[idx + 1..].chars().all(|c| c.is_ascii_digit())
                && !rest[idx + 1..].is_empty() =>
        {
            format!("{}:3130", &rest[..idx])
        }
        _ => format!("{rest}:3130"),
    };
    Some(parse_proxy(&format!("{scheme}{rest}")))
}

/// Backward-compatible alias for `env_proxy` (the pre-cleanup name,
/// kept so existing `--proxy tailnet` invocations keep working).
pub fn tailnet_proxy() -> Option<ProxyCfg> {
    env_proxy()
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Client {
    pub fn get(&self, path: &str) -> Result<Response, String> {
        self.request("GET", path, None)
    }

    /// POST a JSON body (used for /command). Returns the raw response;
    /// the game answers once the command has been processed.
    pub fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<Response, String> {
        let bytes = serde_json::to_vec(body).map_err(|e| format!("json encode: {e}"))?;
        self.request("POST", path, Some(("application/json", bytes)))
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<(&str, Vec<u8>)>,
    ) -> Result<Response, String> {
        let mut stream = self.dial()?;
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: {hh}\r\nAccept: application/json, image/jpeg, */*\r\n",
            hh = self.host_header
        );
        if let Some((ctype, bytes)) = &body {
            req.push_str(&format!(
                "Content-Type: {ctype}\r\nContent-Length: {}\r\n",
                bytes.len()
            ));
        }
        req.push_str("Connection: close\r\n\r\n");
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("write failed: {e}"))?;
        if let Some((_, bytes)) = &body {
            stream
                .write_all(bytes)
                .map_err(|e| format!("write body failed: {e}"))?;
        }
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|e| format!("read failed: {e}"))?;
        parse_response(&raw)
    }

    fn dial(&self) -> Result<TcpStream, String> {
        let timeout = Duration::from_secs(self.timeout_secs);
        match &self.proxy {
            None => {
                let s = TcpStream::connect(&self.authority)
                    .map_err(|e| format!("connect {} failed: {e}", self.authority))?;
                s.set_read_timeout(Some(timeout)).ok();
                s.set_write_timeout(Some(timeout)).ok();
                Ok(s)
            }
            Some(proxy) => {
                let mut s = TcpStream::connect(&proxy.dial)
                    .map_err(|e| format!("connect to proxy failed: {e}"))?;
                s.set_read_timeout(Some(timeout)).ok();
                s.set_write_timeout(Some(timeout)).ok();
                let auth = match &proxy.auth_b64 {
                    Some(a) => format!("Proxy-Authorization: Basic {a}\r\n"),
                    None => String::new(),
                };
                let connect = format!(
                    "CONNECT {a} HTTP/1.1\r\nHost: {a}\r\n{auth}Proxy-Connection: close\r\n\r\n",
                    a = self.authority
                );
                s.write_all(connect.as_bytes())
                    .map_err(|e| format!("proxy write failed: {e}"))?;
                // Read the proxy response header (terminated by CRLFCRLF).
                let mut buf: Vec<u8> = Vec::new();
                let mut byte = [0u8; 1];
                while !buf.ends_with(b"\r\n\r\n") {
                    let n = s
                        .read(&mut byte)
                        .map_err(|e| format!("proxy read failed: {e}"))?;
                    if n == 0 {
                        return Err("proxy closed connection during CONNECT".into());
                    }
                    buf.push(byte[0]);
                    if buf.len() > 64 * 1024 {
                        return Err("proxy CONNECT response too large".into());
                    }
                }
                let head = String::from_utf8_lossy(&buf).to_string();
                if !head.contains(" 200") {
                    return Err(format!(
                        "proxy CONNECT refused: {}",
                        head.lines().next().unwrap_or("")
                    ));
                }
                Ok(s)
            }
        }
    }
}

fn parse_response(raw: &[u8]) -> Result<Response, String> {
    let sep = find(raw, b"\r\n\r\n").ok_or("malformed HTTP response (no header end)")?;
    let head = String::from_utf8_lossy(&raw[..sep]).to_string();
    let mut lines = head.lines();
    let status_line = lines.next().ok_or("empty HTTP response")?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad status line: {status_line}"))?;
    let chunked = head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked");
    let body = &raw[sep + 4..];
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    Ok(Response { status, body })
}

fn dechunk(mut data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = find(data, b"\r\n").ok_or("bad chunk (no size line)")?;
        let size_str = String::from_utf8_lossy(&data[..eol]).to_string();
        let size = usize::from_str_radix(size_str.trim().split(';').next().unwrap_or(""), 16)
            .map_err(|_| format!("bad chunk size: {size_str}"))?;
        data = &data[eol + 2..];
        if size == 0 {
            break;
        }
        if data.len() < size + 2 {
            return Err("truncated chunk".into());
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
    Ok(out)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
