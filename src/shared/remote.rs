//! Minimal remote-control layer: a local HTTP server that lets a CLI or agent
//! open URLs in the browser (the ACP-style "agent embed" primitive).
//! Endpoints:
//!   POST /open?url=https://example.com   -> opens a new window/tab
//!   POST /quit                            -> closes all browsers
use crate::shared::simple_handler::SimpleHandler;
use cef::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

static HANDLER: Mutex<Option<Arc<Mutex<SimpleHandler>>>> = Mutex::new(None);

pub fn register(handler: Arc<Mutex<SimpleHandler>>) {
    *HANDLER.lock().unwrap() = Some(handler);
}

wrap_task! {
    struct OpenUrl {
        url: String,
    }

    impl Task {
        fn execute(&self) {
            debug_assert_ne!(currently_on(ThreadId::UI), 0);
            let command_line = command_line_get_global().expect("cmd line");
            let runtime_style = if command_line.has_switch(Some(&CefString::from("use-alloy-style"))) != 0 {
                RuntimeStyle::ALLOY
            } else {
                RuntimeStyle::DEFAULT
            };

            let url = CefString::from(self.url.as_str());
            let mut client = crate::shared::simple_handler::SimpleHandlerClient::new(
                SimpleHandler::instance().expect("handler"),
            );
            let browser_view = browser_view_create(
                Some(&mut client),
                Some(&url),
                Some(&BrowserSettings::default()),
                None,
                None,
                None,
            );
            let mut window_delegate = crate::shared::simple_app::MiniWindowDelegate::new(
                std::cell::RefCell::new(browser_view),
                runtime_style,
                ShowState::NORMAL,
            );
            window_create_top_level(Some(&mut window_delegate));
        }
    }
}

wrap_task! {
    struct Quit;

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().close_all_browsers(true);
            }
        }
    }
}

wrap_task! {
    struct Navigate {
        url: String,
    }

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().navigate_active(&self.url);
            }
        }
    }
}

wrap_task! {
    struct EvalJs {
        js: String,
    }

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().eval_active(&self.js);
            }
        }
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param<'a>(path: &'a str, key: &str) -> Option<String> {
    let qs = path.split_once('?')?.1;
    for pair in qs.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(percent_decode(v));
            }
        }
    }
    None
}

fn handle_request(mut stream: std::net::TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    // drain headers
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
            break;
        }
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let (status, body) = match (method, path.split('?').next().unwrap_or(path)) {
        ("POST", "/open") => match query_param(path, "url") {
            Some(url) if !url.is_empty() => {
                let mut task = OpenUrl::new(url);
                post_task(ThreadId::UI, Some(&mut task));
                ("200 OK", r#"{"ok":true}"#.to_string())
            }
            _ => ("400 Bad Request", r#"{"error":"missing url"}"#.to_string()),
        },
        ("GET", "/tabs") => {
            let body = SimpleHandler::instance()
                .map(|h| h.lock().unwrap().tabs_json())
                .unwrap_or_else(|| r#"{"tabs":[]}"#.into());
            ("200 OK", body)
        }
        ("POST", "/navigate") => match query_param(path, "url") {
            Some(url) if !url.is_empty() => {
                let mut task = Navigate::new(url);
                post_task(ThreadId::UI, Some(&mut task));
                ("200 OK", r#"{"ok":true}"#.to_string())
            }
            _ => ("400 Bad Request", r#"{"error":"missing url"}"#.to_string()),
        },
        ("POST", "/eval") => match query_param(path, "js") {
            Some(js) if !js.is_empty() => {
                let mut task = EvalJs::new(js);
                post_task(ThreadId::UI, Some(&mut task));
                ("200 OK", r#"{"ok":true}"#.to_string())
            }
            _ => ("400 Bad Request", r#"{"error":"missing js"}"#.to_string()),
        },
        ("POST", "/quit") => {
            let mut task = Quit::new();
            post_task(ThreadId::UI, Some(&mut task));
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        ("GET", "/health") => ("200 OK", r#"{"ok":true,"app":"mini-browser"}"#.to_string()),
        _ => ("404 Not Found", r#"{"error":"not found"}"#.to_string()),
    };

    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

pub fn start() {
    let addr = std::env::var("MINI_REMOTE_ADDR").unwrap_or_else(|_| "127.0.0.1:9777".into());
    std::thread::spawn(move || {
        if let Ok(listener) = TcpListener::bind(&addr) {
            eprintln!("[mini] remote control listening on http://{addr}");
            for stream in listener.incoming() {
                if let Ok(s) = stream {
                    std::thread::spawn(move || handle_request(s));
                }
            }
        } else {
            eprintln!("[mini] remote control: could not bind {addr}");
        }
    });
}
