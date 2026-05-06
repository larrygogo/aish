//! aish-app App State — M2a 起用真实类型 + Actor model session 管理。

#![allow(dead_code)]

use std::collections::HashMap;

use aish_types::{HostConfig, HostId};
use tokio::sync::mpsc;

/// 从 SSH actor task 推回 GPUI 的事件。
#[derive(Debug)]
pub enum SshEvent {
    Connected {
        host: HostId,
    },
    PaneOutput {
        host: HostId,
        bytes: Vec<u8>,
    },
    Disconnected {
        host: HostId,
        reason: DisconnectReason,
    },
    Error {
        host: HostId,
        kind: SshErrorKind,
        msg: String,
    },
}

#[derive(Debug, Clone)]
pub enum DisconnectReason {
    UserRequested,
    RemoteExited,
    NetworkError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    ConnectFailed,
    AuthFailed,
    Io,
    Protocol,
}

/// 从 GPUI 发到 actor task 的命令。
#[derive(Debug)]
pub enum SessionCommand {
    SendBytes(Vec<u8>),
    Disconnect,
}

/// 单一 root Model：所有 UI 共享状态。
#[derive(Default)]
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    pub pane_logs: HashMap<HostId, Vec<String>>,
    /// 已连接 host 的 SessionCommand sender。
    /// 缺失 = 未连接，存在 = 有活跃 session。
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            selected: None,
            pane_logs: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    pub fn select_host(&mut self, id: HostId) {
        self.selected = Some(id);
    }

    pub fn append_log(&mut self, host: HostId, line: String) {
        self.pane_logs.entry(host).or_default().push(line);
    }

    pub fn logs_of(&self, host: HostId) -> &[String] {
        self.pane_logs
            .get(&host)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn host_label(&self, id: HostId) -> Option<String> {
        self.hosts
            .iter()
            .find(|h| h.id == id)
            .map(|h| h.label.clone())
    }

    pub fn is_session_active(&self, id: HostId) -> bool {
        self.sessions.contains_key(&id)
    }

    pub fn register_session(&mut self, id: HostId, sender: mpsc::Sender<SessionCommand>) {
        self.sessions.insert(id, sender);
    }

    pub fn drop_session(&mut self, id: HostId) {
        self.sessions.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_types::SshAuth;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn mk_host(label: &str) -> HostConfig {
        HostConfig {
            id: HostId(Uuid::new_v4()),
            label: label.into(),
            host: "example.com".into(),
            port: 22,
            user: "larry".into(),
            auth: SshAuth::KeyFile {
                path: PathBuf::from("/tmp/k"),
            },
            env_profile: None,
        }
    }

    #[test]
    fn with_hosts_initializes_correctly() {
        let h1 = mk_host("a");
        let h2 = mk_host("b");
        let state = AppState::with_hosts(vec![h1.clone(), h2.clone()]);
        assert_eq!(state.hosts.len(), 2);
        assert_eq!(state.hosts[0].label, "a");
        assert!(state.selected.is_none());
        assert!(state.pane_logs.is_empty());
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn select_host_sets_selected() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.select_host(id);
        assert_eq!(state.selected, Some(id));
    }

    #[test]
    fn append_log_per_host_isolation() {
        let h1 = mk_host("a");
        let h2 = mk_host("b");
        let id1 = h1.id;
        let id2 = h2.id;
        let mut state = AppState::with_hosts(vec![h1, h2]);
        state.append_log(id1, "line A1".into());
        state.append_log(id2, "line B1".into());
        state.append_log(id1, "line A2".into());
        assert_eq!(
            state.logs_of(id1),
            &["line A1".to_string(), "line A2".into()]
        );
        assert_eq!(state.logs_of(id2), &["line B1".to_string()]);
    }

    #[test]
    fn host_label_returns_correct_label() {
        let h = mk_host("my-vps");
        let id = h.id;
        let state = AppState::with_hosts(vec![h]);
        assert_eq!(state.host_label(id), Some("my-vps".into()));
        assert_eq!(state.host_label(HostId(Uuid::new_v4())), None);
    }

    #[tokio::test]
    async fn session_register_and_drop() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        assert!(!state.is_session_active(id));
        state.register_session(id, tx);
        assert!(state.is_session_active(id));
        state.drop_session(id);
        assert!(!state.is_session_active(id));
    }
}
