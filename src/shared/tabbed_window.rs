//! Tabbed window: one top-level Window containing a custom tab strip (a horizontal Panel
//! of LabelButtons) plus one BrowserView per tab. Only the active BrowserView is visible.
//!
//! Tab state comes from mini-core TabManager; the strip is rebuilt on every change.

use cef::*;
use std::cell::RefCell;
use std::rc::Rc;

type CefButton = Button;
type CefPanel = Panel;

pub struct TabbedWindow {
    pub strip: RefCell<Option<CefPanel>>,
    pub pages: RefCell<Option<CefPanel>>,
    pub show_state: ShowState,
}

impl TabbedWindow {
    pub fn new(show_state: ShowState) -> Self {
        Self {
            strip: RefCell::new(None),
            pages: RefCell::new(None),
            show_state,
        }
    }
}

fn box_settings(horizontal: bool) -> BoxLayoutSettings {
    BoxLayoutSettings {
        horizontal: horizontal as _,
        between_child_spacing: 4,
        main_axis_alignment: AxisAlignment::START,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }
}

/// Build the window chrome (root panel + tab strip + pages container) and attach to the window.
pub fn build_chrome(win: &TabbedWindow, window: &mut Window) {
    if let Some(mut root) = panel_create(None) {
        root.set_to_box_layout(Some(&box_settings(false)));
        if let Some(strip) = panel_create(None) {
            strip.set_to_box_layout(Some(&box_settings(true)));
            let mut sv = View::from(&strip);
            root.add_child_view(Some(&mut sv));
            *win.strip.borrow_mut() = Some(strip);
        }
        if let Some(pages) = panel_create(None) {
            pages.set_to_fill_layout();
            let mut pv = View::from(&pages);
            root.add_child_view(Some(&mut pv));
            *win.pages.borrow_mut() = Some(pages);
        }
        let mut rv = View::from(&root);
        window.add_child_view(Some(&mut rv));
    }
    if win.show_state != ShowState::HIDDEN {
        window.show();
    }
}

/// One clickable tab button in the strip.
struct TabButton {
    on_click: Rc<dyn Fn()>,
}

wrap_button_delegate! {
    struct TabButtonWrap { on_click: Rc<dyn Fn()> }

    impl ViewDelegate {}

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut CefButton>) {
            (self.on_click.clone())();
        }
    }
}

/// Rebuild the strip from tab state: (tab_id, title, is_active). `on_click(tab_id)` fires on click.
pub fn refresh_tabs(win: &TabbedWindow, tabs: &[(u64, String, bool)], on_click: Rc<dyn Fn(u64)>, on_new: Rc<dyn Fn()>) {
    {
        let mut strip_guard = win.strip.borrow_mut();
        let Some(strip) = strip_guard.as_mut() else {
            return;
        };
        for (id, title, active) in tabs {
            let label = if title.is_empty() { "New Tab".to_string() } else { title.clone() };
            let shown = if *active { format!("▶ {label}") } else { label };
            let id = *id;
            let on_click = on_click.clone();
            let click: Rc<dyn Fn()> = Rc::new(move || on_click(id));
            let mut del = TabButtonWrap::new(click);
            if let Some(btn) = label_button_create(Some(&mut del), Some(&CefString::from(shown.as_str()))) {
                let mut v = View::from(&btn);
                strip.add_child_view(Some(&mut v));
            }
        }
        let mut plus_del = TabButtonWrap::new(on_new);
        if let Some(btn) = label_button_create(Some(&mut plus_del), Some(&CefString::from("+"))) {
            let mut v = View::from(&btn);
            strip.add_child_view(Some(&mut v));
        }
        strip.invalidate_layout();
    }
}
