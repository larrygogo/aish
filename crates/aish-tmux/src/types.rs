//! SessionTree — tmux session/window/pane 三层结构。

use std::collections::BTreeMap;

use aish_types::{PaneId, SessionId, WindowId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionTree {
    pub sessions: BTreeMap<SessionId, Session>,
    pub active_session: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub windows: BTreeMap<WindowId, Window>,
    pub active_window: Option<WindowId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub session: SessionId,
    pub name: String,
    pub panes: BTreeMap<PaneId, Pane>,
    pub active_pane: Option<PaneId>,
    /// raw layout string (e.g. "bb62,278x67,0,0,1") — M3c 才解析
    pub layout: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub window: WindowId,
}

impl SessionTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_session(&mut self, id: SessionId, name: String) {
        self.sessions.insert(
            id,
            Session {
                name,
                windows: BTreeMap::new(),
                active_window: None,
            },
        );
    }

    pub fn remove_session(&mut self, id: &SessionId) -> bool {
        let removed = self.sessions.remove(id).is_some();
        if removed && self.active_session.as_ref() == Some(id) {
            self.active_session = None;
        }
        removed
    }

    pub fn rename_session(&mut self, id: &SessionId, new_name: String) -> bool {
        if let Some(s) = self.sessions.get_mut(id) {
            s.name = new_name;
            true
        } else {
            false
        }
    }

    pub fn add_window(
        &mut self,
        session: SessionId,
        window: WindowId,
        name: String,
    ) -> Result<(), String> {
        let s = self
            .sessions
            .get_mut(&session)
            .ok_or_else(|| format!("session {} not found", session))?;
        s.windows.insert(
            window,
            Window {
                session: session.clone(),
                name,
                panes: BTreeMap::new(),
                active_pane: None,
                layout: String::new(),
            },
        );
        Ok(())
    }

    pub fn remove_window(&mut self, window: &WindowId) -> bool {
        for s in self.sessions.values_mut() {
            if s.windows.remove(window).is_some() {
                if s.active_window.as_ref() == Some(window) {
                    s.active_window = None;
                }
                return true;
            }
        }
        false
    }

    pub fn rename_window(&mut self, window: &WindowId, new_name: String) -> bool {
        for s in self.sessions.values_mut() {
            if let Some(w) = s.windows.get_mut(window) {
                w.name = new_name;
                return true;
            }
        }
        false
    }

    pub fn set_window_layout(&mut self, window: &WindowId, layout: String) -> bool {
        for s in self.sessions.values_mut() {
            if let Some(w) = s.windows.get_mut(window) {
                w.layout = layout;
                return true;
            }
        }
        false
    }

    pub fn add_pane(&mut self, window: WindowId, pane: PaneId) -> Result<(), String> {
        for s in self.sessions.values_mut() {
            if let Some(w) = s.windows.get_mut(&window) {
                w.panes.insert(pane, Pane { window });
                return Ok(());
            }
        }
        Err(format!("window {} not found", window))
    }

    pub fn remove_pane(&mut self, pane: &PaneId) -> bool {
        for s in self.sessions.values_mut() {
            for w in s.windows.values_mut() {
                if w.panes.remove(pane).is_some() {
                    if w.active_pane.as_ref() == Some(pane) {
                        w.active_pane = None;
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn set_active_session(&mut self, id: SessionId) {
        self.active_session = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    #[test]
    fn add_session_inserts() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        assert!(tree.sessions.contains_key(&sid("$0")));
        assert_eq!(tree.sessions[&sid("$0")].name, "default");
    }

    #[test]
    fn remove_session_clears_active() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "a".into());
        tree.set_active_session(sid("$0"));
        assert!(tree.remove_session(&sid("$0")));
        assert!(tree.active_session.is_none());
    }

    #[test]
    fn rename_session_changes_name() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "old".into());
        assert!(tree.rename_session(&sid("$0"), "new".into()));
        assert_eq!(tree.sessions[&sid("$0")].name, "new");
    }

    #[test]
    fn add_window_attaches_to_session() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into())
            .unwrap();
        assert_eq!(tree.sessions[&sid("$0")].windows.len(), 1);
        assert_eq!(tree.sessions[&sid("$0")].windows[&WindowId(0)].name, "main");
    }

    #[test]
    fn add_window_to_missing_session_errors() {
        let mut tree = SessionTree::new();
        let e = tree.add_window(sid("$nope"), WindowId(0), "main".into());
        assert!(e.is_err());
    }

    #[test]
    fn add_pane_attaches_to_window() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into())
            .unwrap();
        tree.add_pane(WindowId(0), PaneId(0)).unwrap();
        assert_eq!(
            tree.sessions[&sid("$0")].windows[&WindowId(0)].panes.len(),
            1
        );
    }

    #[test]
    fn remove_pane_returns_true_when_found() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into())
            .unwrap();
        tree.add_pane(WindowId(0), PaneId(0)).unwrap();
        assert!(tree.remove_pane(&PaneId(0)));
        assert!(!tree.remove_pane(&PaneId(0)));
    }

    #[test]
    fn set_window_layout_stores_raw_string() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into())
            .unwrap();
        let layout = "bb62,278x67,0,0,1".to_string();
        assert!(tree.set_window_layout(&WindowId(0), layout.clone()));
        assert_eq!(
            tree.sessions[&sid("$0")].windows[&WindowId(0)].layout,
            layout
        );
    }

    #[test]
    fn sessions_are_btreemap_ordered() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$2"), "c".into());
        tree.add_session(sid("$0"), "a".into());
        tree.add_session(sid("$1"), "b".into());
        let names: Vec<_> = tree.sessions.values().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }
}
