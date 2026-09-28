#[cfg(test)]
mod tests {
    use crate::session::TabManager;

    #[test]
    fn open_activates_and_tracks() {
        let mut tm = TabManager::new();
        let a = tm.open("https://a.com", false);
        let b = tm.open("https://b.com", false);
        assert_eq!(tm.active().unwrap().url, "https://b.com");
        assert_eq!(tm.tabs().len(), 2);
        tm.activate(a);
        assert_eq!(tm.active().unwrap().id, a);
        let _ = b;
    }

    #[test]
    fn background_tab_does_not_steal_focus() {
        let mut tm = TabManager::new();
        let a = tm.open("https://a.com", false);
        let b = tm.open("https://b.com", true);
        assert_eq!(tm.active().unwrap().id, a);
        assert!(!tm.tabs().iter().find(|t| t.id == b).unwrap().active);
    }

    #[test]
    fn close_promotes_last_tab() {
        let mut tm = TabManager::new();
        let a = tm.open("https://a.com", false);
        let b = tm.open("https://b.com", false);
        tm.activate(a);
        tm.close(a);
        assert_eq!(tm.active().unwrap().id, b);
        tm.close(b);
        assert!(tm.active().is_none());
        assert!(tm.tabs().is_empty());
    }

    #[test]
    fn title_update() {
        let mut tm = TabManager::new();
        let id = tm.open("https://a.com", false);
        tm.set_title(id, "Example");
        assert_eq!(tm.active().unwrap().title, "Example");
    }

    #[test]
    fn session_save_load_roundtrip() {
        let path = std::path::PathBuf::from(std::env::temp_dir()).join("mini-test-session.json");
        let _ = std::fs::remove_file(&path);

        let mut tm = TabManager::new();
        tm.open("https://a.com", false);
        let b = tm.open("https://b.com", false);
        tm.set_title(b, "B Page");
        tm.save(&path).expect("save");

        let mut restored = TabManager::new();
        assert!(restored.load(&path));
        assert_eq!(restored.tabs().len(), 2);
        assert_eq!(restored.active().unwrap().url, "https://b.com");
        assert_eq!(restored.active().unwrap().title, "B Page");

        // new tabs after restore don't collide with restored ids
        let c = restored.open("https://c.com", true);
        assert!(c > b);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_is_false() {
        let mut tm = TabManager::new();
        assert!(!tm.load(std::path::Path::new("/nonexistent/session.json")));
    }

    #[test]
    fn stealth_strips_trackers() {
        use crate::stealth::StealthConfig;
        let cfg = StealthConfig {
            enabled: true,
            ..Default::default()
        };
        let url = cfg.sanitize_url("https://news.site/article?utm_source=x&id=7&fbclid=abc");
        assert_eq!(url, "https://news.site/article?id=7");
        // off: untouched
        let cfg_off = StealthConfig::default();
        assert_eq!(
            cfg_off.sanitize_url("https://a.com/?utm_source=x"),
            "https://a.com/?utm_source=x"
        );
        // all params trackers: query removed
        assert_eq!(
            cfg.sanitize_url("https://a.com/?utm_campaign=y"),
            "https://a.com/"
        );
    }

    #[test]
    fn stealth_user_agent_is_hardened() {
        use crate::stealth::StealthConfig;
        let cfg = StealthConfig {
            enabled: true,
            ..Default::default()
        };
        let ua = cfg.user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36",
        );
        assert!(ua.starts_with("Mozilla/5.0 (Chrome/154"));
        assert!(ua.contains("Mini Stealth"));
        assert!(!ua.contains("X11"));
    }
}
