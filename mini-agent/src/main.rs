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

fn request(method: &str, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(addr()).map_err(|e| {
        format!(
            "cannot reach Mini Browser at {} ({e}). Is mini-browser running?",
            addr()
        )
    })?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
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
    let usage = "usage: mini-agent <open|navigate|eval|tabs|quit> [arg]";
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
