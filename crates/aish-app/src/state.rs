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
    let config = TermConfig {
        scrolling_history: SCROLLBACK_LINES,
        ..TermConfig::default()
    };
    Term::new(config, &size, VoidListener)
}

/// modal 状态：当前是否在添加 / 编辑 / 删除确认 host。
#[derive(Debug)]
pub enum HostFormState {
    Adding(HostFormDraft),
    Editing { id: HostId, draft: HostFormDraft },
    DeleteConfirm { id: HostId, label: String },
}

/// 表单中间状态。port 用 String 让用户能临时输入非数字，提交时校验。
#[derive(Debug, Default, Clone)]
pub struct HostFormDraft {
    pub label: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub key_path: String,
    /// 校验失败时显示在 modal 底部的红字。
    pub error: Option<String>,
}

impl HostFormDraft {
    /// 从已有 HostConfig 填充（用于编辑）。
    pub fn from_config(cfg: &HostConfig) -> Self {
        let key_path = match &cfg.auth {
            aish_types::SshAuth::KeyFile { path } => path.display().to_string(),
            _ => String::new(),
        };
        Self {
            label: cfg.label.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            key_path,
            error: None,
        }
    }

    /// 校验并转回 HostConfig。`id` Some 表示编辑（保留原 id）/ None 表示新建。
    #[allow(clippy::wrong_self_convention)]
    pub fn into_config(&self, id: Option<HostId>) -> Result<HostConfig, String> {
        if self.label.trim().is_empty() {
            return Err("label 不能为空".into());
        }
        if self.host.trim().is_empty() {
            return Err("host 不能为空".into());
        }
        let port: u16 = self
            .port
            .trim()
            .parse()
            .map_err(|_| "port 必须是 1-65535 的数字".to_string())?;
        if self.user.trim().is_empty() {
            return Err("user 不能为空".into());
        }
        let key_path = self.key_path.trim();
        if key_path.is_empty() {
            return Err("key path 不能为空".into());
        }
        let key_pathbuf = std::path::PathBuf::from(key_path);
        if !key_pathbuf.exists() {
            return Err(format!("key 文件不存在: {}", key_path));
        }

        Ok(HostConfig {
            id: id.unwrap_or_else(|| HostId(uuid::Uuid::new_v4())),
            label: self.label.trim().into(),
            host: self.host.trim().into(),
            port,
            user: self.user.trim().into(),
            auth: aish_types::SshAuth::KeyFile { path: key_pathbuf },
            env_profile: None,
        })
    }
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
    /// 当前打开的 modal（添加/编辑/删除确认）；None = 无 modal
    pub modal: Option<HostFormState>,
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            selected: None,
            pane_terminals: HashMap::new(),
            pane_dimensions: HashMap::new(),
            sessions: HashMap::new(),
            modal: None,
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

    /// 添加一个新 host。
    pub fn add_host(&mut self, cfg: HostConfig) {
        self.hosts.push(cfg);
    }

    /// 替换已有 host（保持 id 不变；新 cfg.id 应等于 id）。
    /// 返回 true = 成功替换，false = id 未找到。
    pub fn update_host(&mut self, id: HostId, cfg: HostConfig) -> bool {
        if let Some(slot) = self.hosts.iter_mut().find(|h| h.id == id) {
            *slot = cfg;
            true
        } else {
            false
        }
    }

    /// 删除 host。同步清理 sessions / pane_terminals / pane_dimensions / 重置 selected。
    /// 返回 true = 成功删除，false = 未找到。
    pub fn remove_host(&mut self, id: HostId) -> bool {
        let idx = match self.hosts.iter().position(|h| h.id == id) {
            Some(i) => i,
            None => return false,
        };
        self.hosts.remove(idx);
        self.sessions.remove(&id);
        self.pane_terminals.remove(&id);
        self.pane_dimensions.remove(&id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        true
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

    fn write_temp_key_file() -> tempfile::NamedTempFile {
        tempfile::NamedTempFile::new().expect("temp file")
    }

    #[test]
    fn draft_into_config_validates_empty_label() {
        let draft = HostFormDraft {
            label: "".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: "/tmp/x".into(),
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("label"));
    }

    #[test]
    fn draft_into_config_validates_port_non_numeric() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "abc".into(),
            user: "root".into(),
            key_path: "/tmp/x".into(),
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.unwrap_err().contains("port"));
    }

    #[test]
    fn draft_into_config_validates_key_file_exists() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: "/nonexistent/path/aish_test_only".into(),
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.unwrap_err().contains("key 文件不存在"));
    }

    #[test]
    fn draft_into_config_succeeds_with_existing_key() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: key.path().display().to_string(),
            error: None,
        };
        let cfg = draft.into_config(None).unwrap();
        assert_eq!(cfg.label, "v");
        assert_eq!(cfg.port, 22);
    }

    #[test]
    fn draft_into_config_preserves_id_when_provided() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: key.path().display().to_string(),
            error: None,
        };
        let id = HostId(Uuid::new_v4());
        let cfg = draft.into_config(Some(id)).unwrap();
        assert_eq!(cfg.id, id);
    }

    #[test]
    fn draft_from_config_extracts_key_path() {
        let h = mk_host("v");
        let draft = HostFormDraft::from_config(&h);
        assert_eq!(draft.label, "v");
        assert_eq!(draft.port, "22");
        assert!(draft.key_path.contains("id_ed25519") || draft.key_path.contains("/tmp/k"));
    }

    #[test]
    fn add_host_appends() {
        let mut state = AppState::with_hosts(vec![]);
        let h = mk_host("v");
        let id = h.id;
        state.add_host(h);
        assert_eq!(state.hosts.len(), 1);
        assert_eq!(state.hosts[0].id, id);
    }

    #[test]
    fn update_host_replaces_in_place() {
        let h1 = mk_host("orig");
        let id = h1.id;
        let mut state = AppState::with_hosts(vec![h1]);

        let mut new_cfg = mk_host("renamed");
        new_cfg.id = id; // 保持 id
        let ok = state.update_host(id, new_cfg);
        assert!(ok);
        assert_eq!(state.hosts[0].label, "renamed");
    }

    #[test]
    fn update_host_returns_false_for_unknown_id() {
        let mut state = AppState::with_hosts(vec![]);
        let unknown = HostId(Uuid::new_v4());
        let cfg = mk_host("x");
        let ok = state.update_host(unknown, cfg);
        assert!(!ok);
    }

    #[tokio::test]
    async fn remove_host_clears_related_state() {
        let h = mk_host("v");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"hello"); // 创建 Term
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        state.select_host(id);

        let ok = state.remove_host(id);
        assert!(ok);
        assert!(state.hosts.is_empty());
        assert!(!state.pane_terminals.contains_key(&id));
        assert!(!state.pane_dimensions.contains_key(&id));
        assert!(!state.is_session_active(id));
        assert_eq!(state.selected, None);
    }

    #[test]
    fn remove_host_returns_false_for_unknown_id() {
        let mut state = AppState::with_hosts(vec![]);
        let unknown = HostId(Uuid::new_v4());
        assert!(!state.remove_host(unknown));
    }
}
