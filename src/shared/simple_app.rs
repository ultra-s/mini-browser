use cef::*;
use std::cell::RefCell;

use super::simple_handler::*;

wrap_window_delegate! {
    pub struct MiniWindowDelegate {
        browser_view: RefCell<Option<BrowserView>>,
        runtime_style: RuntimeStyle,
        initial_show_state: ShowState,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size {
                width: 800,
                height: 600,
            }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            // The tabbed shell is built eagerly by `ensure_shell` (called right after
            // window creation). Fall back to a plain show for any window created later
            // (e.g. the native-window path).
            let browser_view = self.browser_view.borrow();
            let (Some(window), Some(browser_view)) = (window, browser_view.as_ref()) else {
                return;
            };
            if crate::shared::tabbable::get().is_some() {
                return;
            }
            crate::shared::tabbable::ensure_shell(
                window,
                self.initial_show_state,
                Some((0, browser_view.clone())),
            );
            if self.initial_show_state != ShowState::HIDDEN {
                window.show();
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            let mut browser_view = self.browser_view.borrow_mut();
            *browser_view = None;
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            // Allow the window to close if the browser says it's OK.
            let browser_view = self.browser_view.borrow();
            let browser_view = browser_view.as_ref().expect("BrowserView is None");
            if let Some(browser) = browser_view.browser() {
                let browser_host = browser.host().expect("BrowserHost is None");
                browser_host.try_close_browser()
            } else {
                1
            }
        }

        fn initial_show_state(&self, _window: Option<&mut Window>) -> ShowState {
            self.initial_show_state
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }
    }
}

wrap_browser_view_delegate! {
    pub struct SimpleBrowserViewDelegate {
        runtime_style: RuntimeStyle,
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn on_popup_browser_view_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            popup_browser_view: Option<&mut BrowserView>,
            _is_devtools: i32,
        ) -> i32 {
            // Create a new top-level Window for the popup. It will show itself after
            // creation.
            let mut window_delegate = MiniWindowDelegate::new(
                RefCell::new(popup_browser_view.cloned()),
                self.runtime_style,
                ShowState::NORMAL,
            );
            window_create_top_level(Some(&mut window_delegate));

            // We created the Window.
            1
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }
    }
}

wrap_app! {
    pub struct SimpleApp;

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(SimpleBrowserProcessHandler::new(RefCell::new(None)))
        }
    }
}

wrap_browser_process_handler! {
    struct SimpleBrowserProcessHandler {
        client: RefCell<Option<Client>>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            debug_assert_ne!(currently_on(ThreadId::UI), 0);

            // Tabbed windows host multiple BrowserViews, so ALLOY style is required
            // (CHROME style allows only one BrowserView per window).
            let command_line = command_line_get_global().expect("Failed to get command line");
            // Tabbed windows host multiple BrowserViews per window: force ALLOY style
            // for every view (CHROME style allows only one BrowserView per window).
            if command_line.has_switch(Some(&CefString::from("use-alloy-style"))) == 0 {
                command_line.append_switch(Some(&CefString::from("use-alloy-style")));
            }
            let use_alloy_style = true;
            let runtime_style = RuntimeStyle::ALLOY;

            {
                // SimpleHandler implements browser-level callbacks.
                let handler = SimpleHandler::new(use_alloy_style);
                crate::shared::remote::register(handler.clone());
                crate::shared::remote::start();
                let mut client = self.client.borrow_mut();
                *client = Some(SimpleHandlerClient::new(handler));
            }

            // Specify CEF browser settings here.
            let settings = BrowserSettings::default();

            // Check if a "--url=" value was provided via the command-line. If so, use
            // that instead of the default URL.
            let stealth = std::env::var("MINI_STEALTH").map(|v| v == "1").unwrap_or(false);
            let url = CefString::from(&command_line.switch_value(Some(&CefString::from("url"))))
                .to_string();
            let url = if url.is_empty() {
                // Serve the embedded start page from disk (Chromium needs a real file:// or served URL).
                let cache = std::env::var("MINI_CACHE_DIR").unwrap_or_else(|_| {
                    format!("{}/.mini-browser", std::env::var("HOME").unwrap_or_default())
                });
                let page = format!("{}/start.html", cache);
                let _ = std::fs::write(
                    &page,
                    crate::shared::resources::start_page_html(stealth),
                );
                let flag = if stealth { "?stealth=1" } else { "" };
                let _ = stealth;
                &format!("file://{page}{flag}")
            } else {
                url.as_str()
            };
            let mut url_override: Option<String> = None;
            // Track the startup tab: fresh start opens a start-page tab; with a restored
            // session, reopen the saved tabs as real browser views and point the startup
            // browser at the active restored tab's URL.
            {
                let handler = SimpleHandler::instance().expect("handler");
                let mut h = handler.lock().unwrap();
                if h.tabs().tabs().is_empty() {
                    h.track_new_tab("mini://start");
                } else {
                    if let Some(active) = h.tabs().tabs().iter().find(|t| t.active) {
                        let u = active.url.clone();
                        if u != "mini://start" {
                            url_override = Some(u);
                        }
                    }
                    h.restore_tabs_as_views();
                }
            }
            let url = CefString::from(url_override.as_deref().unwrap_or(url));

            // Views framework is required for the tabbed shell. (`--use-native` kept for
            // debugging but views remains the default.)
            let _ = &command_line;
            let use_views = true;

            // If using Views create the browser using the Views framework, otherwise
            // create the browser using the native platform framework.
            if use_views {
                // Create the BrowserView.
                let mut client = self.default_client();
                let mut delegate = SimpleBrowserViewDelegate::new(runtime_style);
                let browser_view = browser_view_create(
                    client.as_mut(),
                    Some(&url),
                    Some(&settings),
                    None,
                    None,
                    Some(&mut delegate),
                );

                // Optionally configure the initial show state.
                let initial_show_state = CefString::from(
                    &command_line.switch_value(Some(&CefString::from("initial-show-state"))),
                )
                .to_string();
                let initial_show_state = match initial_show_state.as_str() {
                    "minimized" => ShowState::MINIMIZED,
                    "maximized" => ShowState::MAXIMIZED,
                    // Hidden show state is only supported on MacOS.
                    #[cfg(target_os = "macos")]
                    "hidden" => ShowState::HIDDEN,
                    _ => ShowState::NORMAL,
                };

                // Create the Window. It will show itself after creation.
                // Which tab does the startup browser show? The active restored tab if a
                // session was restored, otherwise the freshly tracked start-page tab (id 0).
                let first_tab_id = SimpleHandler::instance()
                    .map(|h| {
                        h.lock()
                            .unwrap()
                            .tabs()
                            .tabs()
                            .iter()
                            .find(|t| t.active)
                            .map(|t| t.id)
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);
                let mut delegate = MiniWindowDelegate::new(
                    RefCell::new(browser_view.clone()),
                    runtime_style,
                    initial_show_state,
                );
                let window = window_create_top_level(Some(&mut delegate));
                // Build the tabbed shell now: waiting for on_window_created is unreliable
                // (under software rendering it can fire seconds later, after the first paint).
                if let Some(mut window) = window {
                    crate::shared::tabbable::ensure_shell(
                        &mut window,
                        initial_show_state,
                        browser_view.map(|v| (first_tab_id, v)),
                    );
                }
            } else {
                // Information used when creating the native window.
                let window_info = WindowInfo {
                    runtime_style,
                    ..Default::default()
                };

                #[cfg(target_os = "windows")]
                let window_info = window_info.set_as_popup(Default::default(), "cefsimple");

                let mut client = self.default_client();
                browser_host_create_browser(
                    Some(&window_info),
                    client.as_mut(),
                    Some(&url),
                    Some(&settings),
                    None,
                    None,
                );
            }
        }

        fn default_client(&self) -> Option<Client> {
            self.client.borrow().clone()
        }
    }
}