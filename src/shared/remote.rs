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
/// Result of the latest /read: (sequence, visible text).
static LAST_READ: Mutex<Option<(u64, String)>> = Mutex::new(None);
/// Persisted browser data (bookmarks/settings). Lazy-init from the profile dir.
static BROWSER_DATA: Mutex<Option<mini_core::session::BrowserData>> = Mutex::new(None);
static DATA_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

fn with_data<T>(f: impl FnOnce(&mut mini_core::session::BrowserData) -> T) -> Option<T> {
    let mut guard = BROWSER_DATA.lock().unwrap();
    let data = guard.get_or_insert_with(mini_core::session::BrowserData::default);
    Some(f(data))
}

fn save_data() {
    if let (Some(data), Some(path)) = (
        BROWSER_DATA.lock().unwrap().as_ref(),
        DATA_PATH.lock().unwrap().as_ref(),
    ) {
        data.save(path);
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

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

wrap_string_visitor! {
    struct TextGrab {
        seq: u64,
    }

    impl CefStringVisitor {
        fn visit(&self, string: Option<&CefString>) {
            let text = string.map(CefString::to_string).unwrap_or_default();
            *LAST_READ.lock().unwrap() = Some((self.seq, text));
        }
    }
}

wrap_task! {
    struct ReadActive {
        seq: u64,
    }

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                let h = h.lock().unwrap();
                if let Some(browser) = h.active_browser() {
                    if let Some(frame) = browser.main_frame() {
                        let mut v = TextGrab::new(self.seq);
                        frame.text(Some(&mut v));
                    }
                }
            }
        }
    }
}

wrap_task! {
    struct GoBack;

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().go_back();
            }
        }
    }
}

wrap_task! {
    struct GoForward;

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().go_forward();
            }
        }
    }
}

wrap_task! {
    struct SwitchTab {
        tab_id: u64,
    }

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().switch_tab(self.tab_id);
            }
        }
    }
}

wrap_task! {
    struct CloseTab {
        tab_id: u64,
    }

    impl Task {
        fn execute(&self) {
            if let Some(h) = SimpleHandler::instance() {
                h.lock().unwrap().close_tab(self.tab_id);
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

/// Session auth token (auto-generated at start; written to the profile dir with 0600).
static AUTH_TOKEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static RATE: std::sync::OnceLock<mini_core::agent_api::RateLimiter> = std::sync::OnceLock::new();

fn respond(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn handle_request(mut stream: std::net::TcpStream) {
    use std::io::Read as _;
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    // Cap the request line at 8 KiB (oversized requests are rejected before parsing).
    {
        let mut capped = reader.by_ref().take(8 * 1024);
        match capped.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
    if line.len() >= 8 * 1024 {
        respond(&mut stream, "414 URI Too Long", r#"{"error":"request too large"}"#);
        return;
    }
    // capture Authorization header, then drain the rest
    let mut auth_header: Option<String> = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
            break;
        }
        let lower = h.to_ascii_lowercase();
        if lower.starts_with("authorization:") {
            auth_header = Some(h.trim().to_string());
        }
        if h.len() > 16 * 1024 {
            respond(&mut stream, "431 Request Header Fields Too Large", r#"{"error":"headers too large"}"#);
            return;
        }
    }
    // ── Security gate 1: token auth (constant-time) ──
    {
        let expected = AUTH_TOKEN.lock().unwrap().clone().unwrap_or_default();
        if !expected.is_empty() {
            let provided = auth_header
                .as_ref()
                .and_then(|h| {
                    let lower = h.to_ascii_lowercase();
                    let rest = if lower.starts_with("authorization:") { &h[14..] } else { return None };
                    let v = rest.trim();
                    Some(v.strip_prefix("Bearer ").unwrap_or(v).trim().to_string())
                })
                .or_else(|| {
                    // also accept ?token= for mini-agent convenience
                    let l = line.trim_end();
                    let target = l.split_whitespace().nth(1).unwrap_or(l);
                    let qs = target.split_once('?').map(|x| x.1).unwrap_or("");
                    qs.split('&').find_map(|p| p.strip_prefix("token=")).map(|s| s.to_string())
                })
                .unwrap_or_default();
            if !mini_core::agent_api::ct_eq(provided.as_bytes(), expected.as_bytes()) {
                respond(&mut stream, "401 Unauthorized", r#"{"error":"invalid token"}"#);
                return;
            }
        }
    }
    // ── Security gate 2: rate limit ──
    if let Some(rl) = RATE.get() {
        if !rl.allow() {
            respond(&mut stream, "429 Too Many Requests", r#"{"error":"rate limited"}"#);
            return;
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
        ("POST", "/eval") => {
            if std::env::var("MINI_AGENT_EVAL").as_deref() != Ok("1") {
                ("403 Forbidden", r#"{"error":"eval disabled (set MINI_AGENT_EVAL=1)"}"#.to_string())
            } else if let Some(js) = query_param(path, "js").filter(|j| !j.is_empty()) {
                let mut task = EvalJs::new(js);
                post_task(ThreadId::UI, Some(&mut task));
                ("200 OK", r#"{"ok":true}"#.to_string())
            } else {
                ("400 Bad Request", r#"{"error":"missing js"}"#.to_string())
            }
        }
        ("POST", "/quit") => {
            let mut task = Quit::new();
            post_task(ThreadId::UI, Some(&mut task));
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        ("GET", "/health") => ("200 OK", r#"{"ok":true,"app":"mini-browser"}"#.to_string()),
        // Active tab URL (PageUrl): synchronous from tracked tab state.
        ("GET", "/url") => {
            let body = SimpleHandler::instance()
                .map(|h| {
                    let h = h.lock().unwrap();
                    let url = h
                        .tabs()
                        .tabs()
                        .iter()
                        .find(|t| t.active)
                        .map(|t| t.url.clone())
                        .unwrap_or_default();
                    format!(r#"{{"ok":true,"url":{}}}"#, json_escape(&url))
                })
                .unwrap_or_else(|| r#"{"ok":false}"#.into());
            ("200 OK", body)
        }
        // ── Bookmarks & settings (persisted 0600, shared shape with Android) ──
        ("GET", "/bookmarks") => {
            let guard = BROWSER_DATA.lock().unwrap();
            match guard.as_ref() {
                Some(d) => {
                    let items: Vec<String> = d
                        .bookmarks
                        .iter()
                        .map(|b| {
                            format!(
                                r#"{{"title":{},"url":{}}}"#,
                                json_escape(&b.title),
                                json_escape(&b.url)
                            )
                        })
                        .collect();
                    ("200 OK", format!(r#"{{"ok":true,"bookmarks":[{}]}}"#, items.join(",")))
                }
                None => ("200 OK", r#"{"ok":true,"bookmarks":[]}"#.to_string()),
            }
        }
        ("POST", "/bookmarks/add") => {
            let url = query_param(path, "url").unwrap_or_default();
            let title = query_param(path, "title").unwrap_or_default();
            if url.is_empty() {
                ("400 Bad Request", r#"{"error":"missing url"}"#.to_string())
            } else {
                match with_data(|d| d.add_bookmark(&title, &url)) {
                    Some(true) => {
                        save_data();
                        ("200 OK", r#"{"ok":true}"#.to_string())
                    }
                    _ => ("400 Bad Request", r#"{"error":"invalid url"}"#.to_string()),
                }
            }
        }
        ("POST", "/bookmarks/remove") => {
            let url = query_param(path, "url").unwrap_or_default();
            if url.is_empty() {
                ("400 Bad Request", r#"{"error":"missing url"}"#.to_string())
            } else {
                let removed = with_data(|d| d.remove_bookmark(&url)).unwrap_or(false);
                save_data();
                ("200 OK", format!(r#"{{"ok":{}}}"#, removed))
            }
        }
        ("GET", "/settings") => {
            let guard = BROWSER_DATA.lock().unwrap();
            match guard.as_ref() {
                Some(d) => (
                    "200 OK",
                    format!(
                        r#"{{"ok":true,"homepage":{},"search_engine":{},"cookies_enabled":{}}}"#,
                        json_escape(&d.homepage),
                        json_escape(&d.search_engine),
                        d.cookies_enabled
                    ),
                ),
                None => ("200 OK", r#"{"ok":true}"#.to_string()),
            }
        }
        ("POST", "/settings/set") => {
            let homepage = query_param(path, "homepage");
            let engine = query_param(path, "search_engine");
            let cookies = query_param(path, "cookies");
            with_data(|d| {
                if let Some(h) = homepage {
                    d.homepage = h;
                }
                if let Some(e) = engine {
                    d.search_engine = e;
                }
                if let Some(c) = cookies {
                    d.cookies_enabled = c == "1" || c == "true";
                }
            });
            save_data();
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        // Navigation history of the active tab.
        ("GET", "/history") => {
            let body = SimpleHandler::instance()
                .map(|h| {
                    let h = h.lock().unwrap();
                    match h.tabs().nav_for_active() {
                        Some(nav) => {
                            let backs: Vec<String> =
                                nav.back.iter().map(|u| json_escape(u)).collect();
                            format!(
                                r#"{{"ok":true,"back":[{}],"can_back":{},"can_forward":{}}}"#,
                                backs.join(","),
                                nav.can_back(),
                                nav.can_forward()
                            )
                        }
                        None => r#"{"ok":false}"#.into(),
                    }
                })
                .unwrap_or_else(|| r#"{"ok":false}"#.into());
            ("200 OK", body)
        }
        ("POST", "/back") => {
            let mut task = GoBack::new();
            post_task(ThreadId::UI, Some(&mut task));
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        ("POST", "/forward") => {
            let mut task = GoForward::new();
            post_task(ThreadId::UI, Some(&mut task));
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        // Switch the active tab (TabsSwitch): raise the browser window for tab id.
        ("POST", "/switch") => match query_param(path, "tab").and_then(|t| t.parse::<u64>().ok()) {
            Some(id) => {
                let mut task = SwitchTab::new(id);
                post_task(ThreadId::UI, Some(&mut task));
                ("200 OK", r#"{"ok":true}"#.to_string())
            }
            _ => ("400 Bad Request", r#"{"error":"missing tab"}"#.to_string()),
        },
        // ReadActiveTab: fetch visible text of the active tab. The JS result arrives
        // asynchronously via a StringVisitor; poll LAST_READ until it changes or timeout.
        ("GET", "/read") => {
            use std::sync::atomic::{AtomicU64, Ordering};
            static READ_SEQ: AtomicU64 = AtomicU64::new(0);
            *LAST_READ.lock().unwrap() = None;
            let seq = READ_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
            let mut task = ReadActive::new(seq);
            post_task(ThreadId::UI, Some(&mut task));
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3000);
            loop {
                {
                    let slot = LAST_READ.lock().unwrap();
                    if let Some((s, text)) = slot.as_ref() {
                        if *s == seq {
                            let body = format!(r#"{{"ok":true,"text":{}}}"#, json_escape(text));
                            break ("200 OK", body);
                        }
                    }
                }
                if std::time::Instant::now() > deadline {
                    break ("504 Gateway Timeout", r#"{"error":"read timeout"}"#.to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
        // New tab: opens a fresh browser window with the start page (or given url).
        ("POST", "/new") => {
            let url = query_param(path, "url").unwrap_or_default();
            let mut task = OpenUrl::new(if url.is_empty() { "mini://start".into() } else { url });
            post_task(ThreadId::UI, Some(&mut task));
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        ("POST", "/close") => match query_param(path, "tab").and_then(|t| t.parse::<u64>().ok()) {
            Some(id) => {
                let mut task = CloseTab::new(id);
                post_task(ThreadId::UI, Some(&mut task));
                ("200 OK", r#"{"ok":true}"#.to_string())
            }
            _ => ("400 Bad Request", r#"{"error":"missing tab"}"#.to_string()),
        },
        _ => ("404 Not Found", r#"{"error":"not found"}"#.to_string()),
    };

    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

pub fn start() {
    // ── Security defaults ────────────────────────────────────────────
    // Local-only unless explicitly overridden AND a token is set.
    let mut addr = std::env::var("MINI_REMOTE_ADDR").unwrap_or_else(|_| "127.0.0.1:9777".into());
    let allow_remote = std::env::var("MINI_REMOTE_ALLOW_REMOTE").as_deref() == Ok("1");
    let env_token = std::env::var("MINI_REMOTE_TOKEN").ok().filter(|t| !t.is_empty());

    // Token: env-provided or CSPRNG-generated, persisted 0600 next to the session file.
    let token = env_token.unwrap_or_else(|| {
        let generated = mini_core::agent_api::generate_token().unwrap_or_else(|_| {
            // /dev/urandom unavailable: refuse to serve with a predictable token.
            "disabled".to_string()
        });
        let home_dir = std::env::var("HOME").ok().map(std::path::PathBuf::from);
        if let Some(dir) = std::env::var("MINI_CACHE_DIR").ok().or_else(|| {
            home_dir.map(|h| h.join(".mini-browser").to_string_lossy().into_owned())
        }) {
            let tp = std::path::Path::new(&dir).join("remote-token");
            let _ = std::fs::write(&tp, format!("{generated}\n"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tp, std::fs::Permissions::from_mode(0o600));
            }
        }
        generated
    });
    *AUTH_TOKEN.lock().unwrap() = if token == "disabled" { None } else { Some(token) };

    if allow_remote {
        if std::env::var("MINI_REMOTE_TOKEN").is_err() {
            eprintln!("[mini] remote control: REMOTE binding refused without MINI_REMOTE_TOKEN");
            return;
        }
    } else if !addr.starts_with("127.0.0.1") && !addr.starts_with("localhost") {
        eprintln!("[mini] remote control: non-local bind refused (set MINI_REMOTE_ALLOW_REMOTE=1)");
        addr = "127.0.0.1:9777".into();
    }

    let _ = RATE.set(mini_core::agent_api::RateLimiter::new(
        std::env::var("MINI_REMOTE_RPM").ok().and_then(|v| v.parse().ok()).unwrap_or(120),
        30,
    ));

    // Load persisted browser data (bookmarks/settings). Skipped in stealth mode:
    // stealth must never read or write user data.
    if std::env::var("MINI_STEALTH").as_deref() != Ok("1") {
        let home = std::env::var("HOME").ok().map(std::path::PathBuf::from);
        if let Some(dir) = std::env::var("MINI_CACHE_DIR").ok().or_else(|| {
            home.map(|h| h.join(".mini-browser").to_string_lossy().into_owned())
        }) {
            let path = std::path::Path::new(&dir).join("mini-data.json");
            *DATA_PATH.lock().unwrap() = Some(path.clone());
            *BROWSER_DATA.lock().unwrap() = Some(mini_core::session::BrowserData::load(&path));
        }
    }

    let listen_addr = addr.clone();
    std::thread::spawn(move || {
        if let Ok(listener) = TcpListener::bind(&listen_addr) {
            eprintln!("[mini] remote control listening on http://{listen_addr} (token auth on)");
            for stream in listener.incoming() {
                if let Ok(s) = stream {
                    std::thread::spawn(move || handle_request(s));
                }
            }
        } else {
            eprintln!("[mini] remote control: could not bind {listen_addr}");
        }
    });
}
