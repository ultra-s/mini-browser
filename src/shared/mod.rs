//! Mini Browser: CEF initialization and message loop.

use cef::*;

pub mod remote;
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
    let cache = std::env::var("MINI_CACHE_DIR")
        .unwrap_or_else(|_| format!("{}/.mini-browser", std::env::var("HOME").unwrap_or_default()));
    let _ = std::fs::create_dir_all(&cache);

    // Session restore: tabs are reloaded from disk when the handler is created
    // and saved when the last browser closes.
    simple_handler::set_session_path(std::path::Path::new(&cache).join("session.json"));

    let settings = Settings {
        no_sandbox: !cfg!(feature = "sandbox") as _,
        root_cache_path: CefString::from(cache.as_str()),
        ..Default::default()
    };
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
}
