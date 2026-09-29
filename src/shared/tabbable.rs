//! Integration glue between the tabbed window chrome and the tab manager.

use cef::{
    BrowserView, ImplBrowser, ImplBrowserView, ImplPanel, ImplView, ImplWindow, ShowState, View,
};
use std::cell::RefCell;
use std::rc::Rc;

use super::tabbed_window;

/// The one tabbed window (views mode). None until first window creation.
/// All access is from the UI thread; RefCell borrows are short-lived and non-reentrant.
// SAFETY: all access is from the CEF UI thread; the Mutex only papers over the
// thread_local visibility gap between views callbacks and posted tasks.
/// Wrapper asserting single-thread use.
///
/// SAFETY: every read/write of `TabbableState` happens on the CEF UI thread (window
/// callbacks, posted UI tasks, the tab strip's click closures). The `Mutex` exists only
/// to make the state visible across those callbacks — it is never held concurrently.
pub struct SharedTabbed(Option<std::sync::Arc<TabbableState>>);
unsafe impl Send for SharedTabbed {}
unsafe impl Sync for SharedTabbed {}

static TABBED: std::sync::Mutex<SharedTabbed> = std::sync::Mutex::new(SharedTabbed(None));

pub struct TabbableState {
    pub chrome: std::sync::Arc<super::tabbed_window::TabbedWindow>,
    pub pages_view: RefCell<Option<cef::Panel>>,
    pub views: RefCell<Vec<(u64, BrowserView)>>,
    /// (tab id, browser identifier) pairs — stable mapping for load/title callbacks.
    pub ids: RefCell<Vec<(u64, i32)>>,
}

pub fn register(state: std::sync::Arc<TabbableState>) {
    TABBED.lock().unwrap().0 = Some(state);
}

pub fn get() -> Option<std::sync::Arc<TabbableState>> {
    TABBED.lock().unwrap().0.clone()
}

/// Attach a freshly created Browser as a tab page: hide all others, add + show its view,
/// then rebuild the strip. Called on the UI thread from on_after_created.
pub fn attach_browser(tab_id: u64, browser: cef::Browser) {
    let mut b = browser;
    if let Some(browser_view) = cef::browser_view_get_for_browser(Some(&mut b)) {
        attach_view(tab_id, browser_view);
    }
}

/// Build + register the tabbed shell on first use; later calls are a no-op.
/// Also hosts the startup browser view and starts the session restore.
pub fn ensure_shell(
    window: &mut cef::Window,
    show_state: ShowState,
    startup_view: Option<(u64, BrowserView)>,
) {
    if get().is_some() {
        return;
    }
    let chrome = std::sync::Arc::new(crate::shared::tabbed_window::TabbedWindow::new(show_state));
    crate::shared::tabbed_window::build_chrome(&chrome, window);
    let pages = chrome.pages.borrow().clone();
    let Some(pages) = pages else {
        if show_state != ShowState::HIDDEN {
            window.show();
        }
        return;
    };
    let mut views: Vec<(u64, BrowserView)> = Vec::new();
    let mut ids: Vec<(u64, i32)> = Vec::new();
    if let Some((tab_id, startup)) = startup_view {
        if let Some(b) = startup.browser() {
            ids.push((tab_id, b.identifier()));
        }
        let mut bv = View::from(&startup);
        pages.add_child_view(Some(&mut bv));
        bv.set_visible(1);
        views.push((tab_id, startup));
    }
    let _ = show_state;
    register(std::sync::Arc::new(TabbableState {
        chrome,
        pages_view: RefCell::new(Some(pages)),
        views: RefCell::new(views),
        ids: RefCell::new(ids),
    }));
    refresh();
}

/// Attach a BrowserView as a tab page.
pub fn attach_view(tab_id: u64, browser_view: BrowserView) {
    let Some(state) = get() else { return };
    let pages = state.pages_view.borrow_mut();
    if let Some(pages) = pages.as_ref() {
        // hide existing pages
        {
            let views = state.views.borrow();
            for (_, v) in views.iter() {
                let mut view = View::from(v);
                view.set_visible(0);
            }
        }
        let mut bv = View::from(&browser_view);
        pages.add_child_view(Some(&mut bv));
        bv.set_visible(1);
    }
    let browser_id = browser_view.browser().map(|b| b.identifier()).unwrap_or(-1);
    state.ids.borrow_mut().push((tab_id, browser_id));
    state.views.borrow_mut().push((tab_id, browser_view));
    refresh();
}

/// Browser identifier bound to a tab (for closing the right browser).
pub fn browser_id_for_tab(tab_id: u64) -> Option<i32> {
    let state = get()?;
    state.ids.borrow().iter().find_map(|(id, bid)| (*id == tab_id).then_some(*bid))
}

/// Map a Browser to its tab id by comparing its BrowserView against attached views.
pub fn tab_id_for_browser_inner(browser: &mut cef::Browser) -> Option<u64> {
    let id = browser.identifier();
    let state = get()?;
    state.ids.borrow().iter().find_map(|(tab, bid)| (*bid == id).then_some(*tab))
}

/// Show page for `tab_id`, hide others, refresh strip.
pub fn activate_tab(tab_id: u64) {
    let Some(state) = get() else { return };
    let views = state.views.borrow();
    for (id, v) in views.iter() {
        let mut view = View::from(v);
        view.set_visible((*id == tab_id) as _);
    }
    refresh();
}

/// Remove a closed tab's view from the window.
pub fn detach_tab(tab_id: u64) {
    let Some(state) = get() else { return };
    {
        let pages = state.pages_view.borrow();
        let views = state.views.borrow();
        if let Some(idx) = views.iter().position(|(id, _)| *id == tab_id) {
            if let (Some(pages), Some(v)) = (pages.as_ref(), views.get(idx)) {
                let mut view = View::from(&v.1);
                pages.remove_child_view(Some(&mut view));
            }
        }
    }
    state.views.borrow_mut().retain(|(id, _)| *id != tab_id);
    state.ids.borrow_mut().retain(|(id, _)| *id != tab_id);
    refresh();
}

/// Rebuild the strip from the handler's tab state.
pub fn refresh() {
    let Some(state) = get() else { return };
    let tabs: Vec<(u64, String, bool)> = {
        let Some(handler) = super::simple_handler::SimpleHandler::instance() else { return };
        handler
            .try_lock()
            .map(|h| {
                h.tabs()
                    .tabs()
                    .iter()
                    .map(|t| (t.id, t.title.clone(), t.active))
                    .collect()
            })
            .unwrap_or_default()
    };
    let on_click: Rc<dyn Fn(u64)> = Rc::new(|id| {
        let Some(handler) = super::simple_handler::SimpleHandler::instance() else { return };
        handler.lock().unwrap().activate_tab(id);
        activate_tab(id);
    });
    let on_new: Rc<dyn Fn()> = Rc::new(|| {
        super::remote::spawn_new_tab("mini://start".to_string());
    });
    tabbed_window::refresh_tabs(&state.chrome, &tabs, on_click, on_new);
}
