//! Mini Browser: CEF initialization and message loop.

use cef::*;

pub mod remote;
pub mod tabbed_window;
pub mod tabbable;
pub mod resources;
pub mod simple_app;
pub mod simple_handler;

#[cfg(target_os = "macos")]
pub type Library = library_loader::LibraryLoader;

#[cfg(not(target_os = "macos"))]
pub struct Library;

#[allow(dead_code)]
pub fn load_cef() -> Library {
    #[cfg(target_os = "macos")]
    let library = {
        let loader = library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), false);
        assert!(loader.load());
        loader
    };
    #[cfg(not(target_os = "macos"))]
    let library = Library;

    // Initialize the CEF API version.
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    #[cfg(target_os = "macos")]
    crate::mac::setup_simple_application();

    library
}

#[allow(dead_code)]
pub fn run_main(main_args: &MainArgs, cmd_line: &CommandLine, sandbox_info: *mut u8) {
    let switch = CefString::from("type");
    let is_browser_process = cmd_line.has_switch(Some(&switch)) != 1;

    let ret = execute_process(Some(main_args), None, sandbox_info);

    if is_browser_process {
        println!("launch browser process");
        assert_eq!(ret, -1, "cannot execute browser process");
    } else {
        let process_type = CefString::from(&cmd_line.switch_value(Some(&switch)));
        println!("launch process {process_type}");
        assert!(ret >= 0, "cannot execute non-browser process");
        return;
    }

    let mut app = simple_app::SimpleApp::new();

    // Persistent profile: cache, cookies and storage survive restarts.
    // Stealth mode (MINI_STEALTH=1): everything stays in a throwaway temp dir,
    // nothing persists, session restore is skipped.
    let stealth = std::env::var("MINI_STEALTH").map(|v| v == "1").unwrap_or(false);
    let cache: std::path::PathBuf = if stealth {
        std::env::temp_dir().join(format!("mini-stealth-{}", std::process::id()))
    } else {
        std::env::var("MINI_CACHE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                let mut p = std::env::home_dir().unwrap_or_default();
                p.push(".mini-browser");
                p
            })
    };
    let _ = std::fs::create_dir_all(&cache);

    // Session restore: tabs are reloaded from disk when the handler is created
    // and saved when the last browser closes. Disabled in stealth mode.
    if !stealth {
        simple_handler::set_session_path(std::path::Path::new(&cache).join("session.json"));
    }

    let mut settings = Settings {
        no_sandbox: !cfg!(feature = "sandbox") as _,
        root_cache_path: CefString::from(cache.to_string_lossy().as_ref()),
        ..Default::default()
    };
    // Stealth hardening via settings:
    if stealth {
        // Engine-level UA, uniform across installs.
        settings.user_agent = CefString::from(
            "Mozilla/5.0 (Chrome/154; Mini Stealth) Safari/537.36",
        );
        // Do not persist session cookies.
        settings.persist_session_cookies = 0;
    }
    // --- Engine switches ---------------------------------------------------
    if let Some(cmd) = command_line_get_global() {
        // GPU acceleration on by default (smooth scrolling, raster, WebGL).
        // Force ALLOY runtime style: tabbed windows host multiple BrowserViews
        // per window (CHROME style allows only one BrowserView per window).
        cmd.append_switch(Some(&CefString::from("use-alloy-style")));
        cmd.append_switch(Some(&CefString::from("enable-gpu-rasterization")));
        cmd.append_switch(Some(&CefString::from("enable-zero-copy")));
        cmd.append_switch(Some(&CefString::from("enable-smooth-scrolling")));
        cmd.append_switch(Some(&CefString::from("ignore-gpu-blocklist")));

        // Custom UA profiles: MINI_UA=desktop (default) | mobile | stealth | <literal>
        let ua = std::env::var("MINI_UA").unwrap_or_else(|_| "desktop".into());
        let ua_value = match ua.as_str() {
            "mobile" => Some("Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Mobile Safari/537.36".to_string()),
            "stealth" => Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36".to_string()),
            "desktop" => None, // engine default
            other => Some(other.to_string()),
        };
        if let Some(v) = ua_value {
            cmd.append_switch_with_value(Some(&CefString::from("user-agent")), Some(&CefString::from(v.as_str())));
        }

        // Stealth hardening: no referrer leak, disable background tracking traffic.
        if stealth {
            cmd.append_switch_with_value(Some(&CefString::from("referer")), Some(&CefString::from("no-referrer")));
            cmd.append_switch(Some(&CefString::from("disable-background-networking")));
            cmd.append_switch(Some(&CefString::from("disable-component-update")));
            cmd.append_switch(Some(&CefString::from("disable-sync")));
            cmd.append_switch(Some(&CefString::from("no-default-browser-check")));
            cmd.append_switch(Some(&CefString::from("disable-features=OptimizationHints,MediaRouter")));
        }
    }

    // Stealth/Tor proxy must go through the command line: MINI_PROXY=socks5://127.0.0.1:9050
    if let Ok(proxy) = std::env::var("MINI_PROXY") {
        if !proxy.is_empty() {
            if let Some(cmd) = command_line_get_global() {
                cmd.append_switch_with_value(
                    Some(&CefString::from("proxy-server")),
                    Some(&CefString::from(proxy.as_str())),
                );
            }
        }
    }
    assert_eq!(
        initialize(
            Some(main_args),
            Some(&settings),
            Some(&mut app),
            sandbox_info,
        ),
        1
    );

    run_message_loop();

    shutdown();

    // Stealth: wipe the throwaway cache dir completely on exit.
    if stealth {
        let _ = std::fs::remove_dir_all(&cache);
    }
}
