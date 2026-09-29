//! mini-mcp — MCP server exposing the Browser Agent API as standardized tools.
//!
//! Transport: stdio JSON-RPC 2.0 (MCP stdio). Tools: browser_tabs_list, browser_tabs_create,
//! browser_tabs_close, browser_tabs_switch, browser_page_read, browser_page_url,
//! browser_page_eval (gated), browser_health.
//!
//! Security: reuses the same token file as the browser; no network listener of its own.
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;

fn api_addr() -> String {
    std::env::var("MINI_REMOTE_ADDR").unwrap_or_else(|_| "127.0.0.1:9777".into())
}

fn token() -> Option<String> {
    if let Ok(t) = std::env::var("MINI_REMOTE_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let home = std::env::var("HOME").ok()?;
    std::fs::read_to_string(std::path::Path::new(&home).join(".mini-browser/remote-token"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn request(method: &str, path: &str) -> Result<String, String> {
    let mut s = TcpStream::connect(api_addr())
        .map_err(|e| format!("cannot reach Mini Browser at {} ({e})", api_addr()))?;
    let auth = token()
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Connection: close\r\nContent-Length: 0\r\n\r\n"
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    s.read_to_string(&mut resp).map_err(|e| e.to_string())?;
    Ok(resp.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("").to_string())
}

fn pct(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':'
            | b'?' | b'=' | b'&' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The MCP tool catalog — mirrors the Browser Agent API `Op` surface.
fn tools() -> Value {
    json!([
      {"name":"browser_health","description":"Check Mini Browser is running.","inputSchema":{"type":"object","properties":{}}},
      {"name":"browser_tabs_list","description":"List open tabs.","inputSchema":{"type":"object","properties":{}}},
      {"name":"browser_tabs_create","description":"Open a URL in a new tab.","inputSchema":{"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}},
      {"name":"browser_tabs_close","description":"Close a tab by id.","inputSchema":{"type":"object","properties":{"tab":{"type":"integer"}},"required":["tab"]}},
      {"name":"browser_tabs_switch","description":"Make a tab active.","inputSchema":{"type":"object","properties":{"tab":{"type":"integer"}},"required":["tab"]}},
      {"name":"browser_page_read","description":"Read visible text of the active tab.","inputSchema":{"type":"object","properties":{}}},
      {"name":"browser_page_url","description":"Get the active tab URL.","inputSchema":{"type":"object","properties":{}}},
      {"name":"browser_page_eval","description":"Run JS in the active tab (requires MINI_AGENT_EVAL=1 on the browser).","inputSchema":{"type":"object","properties":{"js":{"type":"string"}},"required":["js"]}}
    ])
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    let a = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match name {
        "browser_health" => request("GET", "/health"),
        "browser_tabs_list" => request("GET", "/tabs"),
        "browser_tabs_create" => {
            let u = a("url");
            if u.is_empty() {
                return Err("url required".into());
            }
            request("POST", &format!("/new?url={}", pct(&u)))
        }
        "browser_tabs_close" => request("POST", &format!("/close?tab={}", a("tab"))),
        "browser_tabs_switch" => request("POST", &format!("/switch?tab={}", a("tab"))),
        "browser_page_read" => request("GET", "/read"),
        "browser_page_url" => request("GET", "/url"),
        "browser_page_eval" => {
            let js = a("js");
            if js.is_empty() {
                return Err("js required".into());
            }
            request("POST", &format!("/eval?js={}", pct(&js)))
        }
        n => Err(format!("unknown tool {n}")),
    }
}

fn main() {
    let stdin = io::stdin();
    let mut out = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let id = v.get("id").cloned().unwrap_or(Value::Null);
        let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let (result, error) = match method {
            "initialize" => (
                Some(json!({
                    "protocolVersion":"2024-11-05",
                    "serverInfo":{"name":"mini-browser","version":"1.1"},
                    "capabilities":{"tools":{}}
                })),
                None,
            ),
            "tools/list" => (Some(json!({ "tools": tools() })), None),
            "tools/call" => {
                let name = v
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = v
                    .get("params")
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({}));
                match call_tool(name, &args) {
                    Ok(text) => (
                        Some(json!({"content":[{"type":"text","text":text}]})),
                        None,
                    ),
                    Err(e) => (
                        None,
                        Some(json!({"code":-32000,"message":e})),
                    ),
                }
            }
            "notifications/initialized" | "shutdown" => {
                continue;
            }
            m => (
                None,
                Some(json!({"code":-32601,"message":format!("method not found: {m}")})),
            ),
        };
        let resp = if let Some(r) = result {
            json!({"jsonrpc":"2.0","id":id,"result":r})
        } else {
            json!({"jsonrpc":"2.0","id":id,"error":error})
        };
        let _ = writeln!(out, "{}", resp);
        let _ = out.flush();
    }
}
