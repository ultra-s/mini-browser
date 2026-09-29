//! Tab management shared by desktop and Android.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub type TabId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub active: bool,
    pub pinned: bool,
}

/// Engine-agnostic tab state. The platform shell (CEF window / Android Activity)
/// renders this state and forwards user actions back in.
#[derive(Debug, Default)]
pub struct TabManager {
    tabs: Vec<Tab>,
    next_id: TabId,
}

impl TabManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, url: impl Into<String>, background: bool) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            url: url.into(),
            title: String::new(),
            active: self.tabs.is_empty() || !background,
            pinned: false,
        });
        if !background {
            self.activate(id);
        }
        id
    }

    pub fn activate(&mut self, id: TabId) {
        for tab in self.tabs.iter_mut() {
            tab.active = tab.id == id;
        }
    }

    pub fn close(&mut self, id: TabId) {
        self.tabs.retain(|t| t.id != id);
        if !self.tabs.iter().any(|t| t.active) {
            if let Some(last) = self.tabs.last_mut() {
                last.active = true;
            }
        }
    }

    pub fn set_url_for_active(&mut self, url: impl Into<String>) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.active) {
            tab.url = url.into();
        }
    }

    pub fn set_title_for_url(&mut self, url: &str, title: impl Into<String>) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.url == url) {
            tab.title = title.into();
        }
    }

    pub fn set_title(&mut self, id: TabId, title: impl Into<String>) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.title = title.into();
        }
    }

    pub fn active(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.active)
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Persist tab state to disk (session restore).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string(&self.tabs)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Load tab state from disk (session restore). Returns false if absent/corrupt.
    pub fn load(&mut self, path: &Path) -> bool {
        let Ok(json) = std::fs::read_to_string(path) else {
            return false;
        };
        let Ok(tabs) = serde_json::from_str::<Vec<Tab>>(&json) else {
            return false;
        };
        self.next_id = tabs.iter().map(|t| t.id + 1).max().unwrap_or(0);
        self.tabs = tabs;
        true
    }
}
