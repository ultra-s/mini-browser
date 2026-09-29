//! mini-agent: drives Mini Browser over its local remote-control HTTP API.
//! Usage:
//!   mini-agent open <url>      open url in a new browser window
//!   mini-agent navigate <url>  navigate the active tab
//!   mini-agent eval "<js>"     run JavaScript in the active tab
//!   mini-agent tabs            list tabs (JSON)
//!   mini-agent quit            close the browser

use std::io::{Read, Write};
use std::net::TcpStream;

fn addr() -> String {
    std::env::var("MINI_REMOTE_ADDR").unwrap_or_else(|_| "127.0.0.1:9777".into())
}

/// Auth token: MINI_REMOTE_TOKEN env, or the token file the browser wrote (0600).
fn token() -> Option<String> {
    if let Ok(t) = std::env::var("MINI_REMOTE_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let p = std::path::Path::new(&home)
        .join(".mini-browser")
        .join("remote-token");
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn request(method: &str, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(addr()).map_err(|e| {
        format!(
            "cannot reach Mini Browser at {} ({e}). Is mini-browser running?",
            addr()
        )
    })?;
    let auth = token()
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Connection: close\r\nContent-Length: 0\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).map_err(|e| e.to_string())?;
    let body = resp.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    Ok(body.to_string())
}

fn percent_encode(s: &str) -> String {
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: mini-agent <open|navigate|eval|read|url|history|back|forward|new|close|switch|tabs|bookmarks|bm-add|bm-remove|settings|quit> [arg]";
    let Some(cmd) = args.first() else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let result = match (cmd.as_str(), args.get(1)) {
        ("open", Some(url)) => request("POST", &format!("/open?url={}", percent_encode(url))),
        ("navigate", Some(url)) => {
            request("POST", &format!("/navigate?url={}", percent_encode(url)))
        }
        ("eval", Some(js)) => request("POST", &format!("/eval?js={}", percent_encode(js))),
        ("read", _) => request("GET", "/read"),
        ("url", _) => request("GET", "/url"),
        ("history", _) => request("GET", "/history"),
        ("bookmarks", _) => request("GET", "/bookmarks"),
        ("bm-add", Some(spec)) => {
            // spec: "url" or "title|url"
            let (title, url) = spec.split_once('|').unwrap_or(("", spec));
            request("POST", &format!("/bookmarks/add?title={}&url={}", percent_encode(title), percent_encode(url)))
        }
        ("bm-remove", Some(url)) => {
            request("POST", &format!("/bookmarks/remove?url={}", percent_encode(url)))
        }
        ("settings", _) => request("GET", "/settings"),
        ("back", _) => request("POST", "/back"),
        ("forward", _) => request("POST", "/forward"),
        ("switch", Some(id)) => request("POST", &format!("/switch?tab={}", percent_encode(id))),
        ("switch", None) => Err("switch requires a tab id".to_string()),
        ("new", arg) => match arg {
            Some(url) => request("POST", &format!("/new?url={}", percent_encode(url))),
            None => request("POST", "/new"),
        },
        ("close", Some(id)) => request("POST", &format!("/close?tab={}", percent_encode(id))),
        ("tabs", _) => request("GET", "/tabs"),
        ("quit", _) => request("POST", "/quit"),
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    match result {
        Ok(body) => println!("{body}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
