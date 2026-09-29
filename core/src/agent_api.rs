//! Browser Agent API — the typed boundary between agents and the browser.
//!
//! Security model (number one priority):
//! - **Local-only by default**: the transport binds 127.0.0.1; remote binding requires
//!   MINI_REMOTE_ALLOW_REMOTE=1 AND a token.
//! - **Token auth**: every request must carry `Authorization: Bearer <token>` or
//!   `?token=<token>`. The token is auto-generated (CSPRNG) per session, stored 0600 in
//!   the profile dir; agents read it from the file, never from the process env of others.
//! - **Constant-time comparison** for token checks.
//! - **Rate limited**: token bucket per client (default 60 req/min, burst 20).
//! - **Request allowlist**: only the ops in `Op` are reachable; no free-form routing.
//! - **Eval allowlist mode**: `MINI_AGENT_EVAL=0` (default for stealth) blocks JS execution.
//! - **Size caps**: request line ≤ 8 KiB, response text ≤ 2 MiB.
//! - **Stealth**: no requests, responses, or tokens are logged in stealth mode.

use serde::{Deserialize, Serialize};

/// The complete agent-facing surface. Each op maps 1:1 to an endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "domain", content = "op", rename_all = "snake_case")]
pub enum Op {
    // ── Tabs ─────────────────────────────────────────────
    TabsList,
    TabsCreate { url: Option<String>, background: bool },
    TabsClose { tab_id: u64 },
    TabsSwitch { tab_id: u64 },

    // ── Page ─────────────────────────────────────────────
    PageRead,                        // visible text of active tab
    PageUrl,                         // url of active tab
    PageEval { js: String },         // gated by MINI_AGENT_EVAL
    PageScreenshot { tab_id: Option<u64> }, // reserved (v2)

    // ── Browser ──────────────────────────────────────────
    BrowserHealth,
    BrowserQuit,
}

impl Op {
    /// Query-string helper (percent-decoding happens at the transport layer).
    fn q<'a>(qs: &'a str, key: &str) -> Option<String> {
        qs.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key && !v.is_empty()).then(|| v.to_string())
        })
    }

    /// Parse an HTTP method+path (with query) into an Op. Unknown routes -> None.
    pub fn from_route(method: &str, path: &str) -> Option<Op> {
        let (p, qs) = path.split_once('?').unwrap_or((path, ""));
        match (method, p) {
            ("GET", "/tabs") => Some(Op::TabsList),
            ("POST", "/tabs/new") => Some(Op::TabsCreate {
                url: Self::q(qs, "url"),
                background: Self::q(qs, "background").as_deref() == Some("1"),
            }),
            ("POST", "/tabs/close") => Some(Op::TabsClose {
                tab_id: Self::q(qs, "tab")?.parse().ok()?,
            }),
            ("POST", "/tabs/switch") => Some(Op::TabsSwitch {
                tab_id: Self::q(qs, "tab")?.parse().ok()?,
            }),
            ("GET", "/page/read") => Some(Op::PageRead),
            ("GET", "/page/url") => Some(Op::PageUrl),
            ("POST", "/page/eval") => Some(Op::PageEval {
                js: Self::q(qs, "js")?,
            }),
            ("GET", "/browser/health") => Some(Op::BrowserHealth),
            ("POST", "/browser/quit") => Some(Op::BrowserQuit),
            _ => None,
        }
    }
}

/// Rate limiter: simple token bucket, per-process (single-user local API).
pub struct RateLimiter {
    tokens: std::sync::Mutex<f64>,
    last: std::sync::Mutex<std::time::Instant>,
    rate: f64,          // tokens per second
    burst: f64,
}

impl RateLimiter {
    pub fn new(per_minute: u32, burst: u32) -> Self {
        Self {
            tokens: std::sync::Mutex::new(burst as f64),
            last: std::sync::Mutex::new(std::time::Instant::now()),
            rate: per_minute as f64 / 60.0,
            burst: burst as f64,
        }
    }

    pub fn allow(&self) -> bool {
        let mut t = self.tokens.lock().unwrap();
        let mut last = self.last.lock().unwrap();
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(*last).as_secs_f64();
        *last = now;
        *t = (*t + elapsed * self.rate).min(self.burst);
        if *t >= 1.0 {
            *t -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Constant-time equality for secret comparison.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Generate a CSPRNG token (hex). Uses the OS CSPRNG via /dev/urandom.
pub fn generate_token() -> std::io::Result<String> {
    use std::io::Read;
    let mut buf = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_basic() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }

    #[test]
    fn token_length_and_uniqueness() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn rate_limiter_burst_and_refill() {
        let rl = RateLimiter::new(60, 3);
        assert!(rl.allow());
        assert!(rl.allow());
        assert!(rl.allow());
        assert!(!rl.allow()); // burst exhausted
    }
}
