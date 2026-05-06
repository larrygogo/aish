//! aish-app App State — M2b1 起持有 alacritty_terminal::Term per host。

#![allow(dead_code)]

use std::collections::HashMap;

use aish_types::{HostConfig, HostId};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::Term;
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
    Resize { cols: u16, rows: u16 },
    Disconnect,
}

/// 默认 PTY 大小（首次 connect 时用，后续按窗口 resize 调整）。
pub const DEFAULT_COLS: u16 = 120;
pub const DEFAULT_ROWS: u16 = 40;

/// scrollback buffer 大小。
const SCROLLBACK_LINES: usize = 10_000;

/// 创建一个空 Term（M2b1 用 VoidListener — 不接收 alacritty 事件）。
pub fn make_term(cols: u16, rows: u16) -> Term<VoidListener> {
    let size = TermSize::new(cols as usize, rows as usize);
    let config = TermConfig { scrolling_history: SCROLLBACK_LINES, ..TermConfig::default() };
    Term::new(config, &size, VoidListener)
}

/// 单一 root Model：所有 UI 共享状态。
#[derive(Default)]
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    /// per-host alacritty Term（vt100 状态机 + grid + scrollback）
    pub pane_terminals: HashMap<HostId, Term<VoidListener>>,
    /// per-host 当前 PTY 大小（cols, rows）
    pub pane_dimensions: HashMap<HostId, (u16, u16)>,
    /// 已连接 host 的 SessionCommand sender
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            selected: None,
            pane_terminals: HashMap::new(),
            pane_dimensions: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    pub fn select_host(&mut self, id: HostId) {
        self.selected = Some(id);
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
        self.pane_dimensions
            .insert(id, (DEFAULT_COLS, DEFAULT_ROWS));
    }

    pub fn drop_session(&mut self, id: HostId) {
        self.sessions.remove(&id);
        // 不删 pane_terminals — 保留 scrollback，重连时用户能看到旧输出
    }

    /// feed bytes 到指定 host 的 Term（VT100 状态机）。
    /// 如果 Term 不存在则创建。
    pub fn feed_bytes(&mut self, host: HostId, bytes: &[u8]) {
        let (cols, rows) = self
            .pane_dimensions
            .get(&host)
            .copied()
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        let term = self
            .pane_terminals
            .entry(host)
            .or_insert_with(|| make_term(cols, rows));
        // 使用 vte::ansi::Processor（alacritty 自己在 event_loop 中用的入口）
        // Term<T> 实现了 vte::ansi::Handler，Processor::advance 接受 &mut Handler
        let mut processor = alacritty_terminal::vte::ansi::Processor::<
            alacritty_terminal::vte::ansi::StdSyncHandler,
        >::new();
        processor.advance(term, bytes);
    }

    /// 取指定 host 的 Term（只读）。
    pub fn term_of(&self, host: HostId) -> Option<&Term<VoidListener>> {
        self.pane_terminals.get(&host)
    }

    /// resize 指定 host 的 Term（同步 alacritty grid）。
    pub fn resize_term(&mut self, host: HostId, cols: u16, rows: u16) {
        if let Some(term) = self.pane_terminals.get_mut(&host) {
            let size = TermSize::new(cols as usize, rows as usize);
            term.resize(size);
        }
        self.pane_dimensions.insert(host, (cols, rows));
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
    fn with_hosts_initializes() {
        let h = mk_host("a");
        let state = AppState::with_hosts(vec![h]);
        assert_eq!(state.hosts.len(), 1);
        assert!(state.pane_terminals.is_empty());
        assert!(state.pane_dimensions.is_empty());
    }

    #[test]
    fn feed_bytes_creates_term_on_demand() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"hello\r\n");
        assert!(state.pane_terminals.contains_key(&id));
    }

    #[test]
    fn feed_bytes_reflects_in_term_grid() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"abc");
        let term = state.term_of(id).unwrap();
        let grid = term.grid();
        let first_row = &grid[alacritty_terminal::index::Line(0)];
        assert_eq!(first_row[alacritty_terminal::index::Column(0)].c, 'a');
        assert_eq!(first_row[alacritty_terminal::index::Column(1)].c, 'b');
        assert_eq!(first_row[alacritty_terminal::index::Column(2)].c, 'c');
    }

    #[tokio::test]
    async fn register_session_inits_dimensions() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        assert_eq!(
            state.pane_dimensions.get(&id),
            Some(&(DEFAULT_COLS, DEFAULT_ROWS))
        );
    }

    #[tokio::test]
    async fn drop_session_keeps_terminal() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        state.feed_bytes(id, b"x");
        state.drop_session(id);
        assert!(state.pane_terminals.contains_key(&id));
        assert!(!state.is_session_active(id));
    }

    #[test]
    fn resize_updates_dimensions() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"");
        state.resize_term(id, 100, 30);
        assert_eq!(state.pane_dimensions.get(&id), Some(&(100, 30)));
    }
}
