//! Stealth mode: privacy posture shared by desktop and Android.
//! When enabled the browser:
//!  - never writes profile data to disk (in-memory profile)
//!  - keeps no history (TabManager operates without persistence)
//!  - strips tracking URL parameters
//!  - sends a hardened User-Agent and drops referrer to host-only
//!  - forces a SOCKS5/HTTP proxy (Tor-compatible) when configured

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthConfig {
    pub enabled: bool,
    /// Proxy in host:port form (e.g. Tor: 127.0.0.1:9050).
    pub proxy: Option<String>,
    /// Strip tracking query parameters (utm_*, fbclid, gclid, ...).
    pub strip_trackers: bool,
    /// Send Do-Not-Track and Global-Privacy-Control headers.
    pub dnt_gpc: bool,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy: None,
            strip_trackers: true,
            dnt_gpc: true,
        }
    }
}

/// Query parameters that exist to track clicks across sites.
const TRACKER_PARAMS: &[&str] = &[
    "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
    "utm_id", "fbclid", "gclid", "dclid", "msclkid", "twclid", "igshid",
    "mc_cid", "mc_eid", "_hsenc", "_hsmi", "vero_id", "wickedid", "yclid",
    "ttclid", "s_kwcid", "li_fat_id",
];

impl StealthConfig {
    /// Rewrite a URL: strip tracker params when enabled.
    pub fn sanitize_url(&self, url: &str) -> String {
        if !self.enabled || !self.strip_trackers {
            return url.to_string();
        }
        let (base, query) = match url.split_once('?') {
            Some((b, q)) => (b, q),
            None => return url.to_string(),
        };
        let kept: Vec<&str> = query
            .split('&')
            .filter(|pair| {
                let key = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
                !TRACKER_PARAMS.contains(&key.as_str())
            })
            .collect();
        if kept.is_empty() {
            base.to_string()
        } else {
            format!("{base}?{}", kept.join("&"))
        }
    }

    /// Hardened User-Agent: engine version only, no OS fingerprint details.
    pub fn user_agent(&self, default_ua: &str) -> String {
        if !self.enabled {
            return default_ua.to_string();
        }
        // Take only the engine token: "Mozilla/5.0 ... Chrome/154 ..." style
        // Keep it short and uniform across installs.
        let engine = default_ua
            .split_whitespace()
            .find(|t| t.starts_with("Chrome/") || t.starts_with("Gecko/"))
            .unwrap_or("Chrome/154");
        format!("Mozilla/5.0 ({engine}; Mini Stealth) Safari/537.36")
    }
}
