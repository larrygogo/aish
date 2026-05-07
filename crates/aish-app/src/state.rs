//! aish-app App State — M2b1 起持有 alacritty_terminal::Term per host。

#![allow(dead_code)]

use std::collections::HashMap;

use aish_tmux::SessionTree;
use aish_types::{HostConfig, HostId, PaneId, RemoteSession, SessionId};
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
    /// list-sessions 开始
    TmuxQueryStarted {
        host: HostId,
    },
    /// list-sessions 成功（包括 tmux 在但 0 session 的情况）
    TmuxSessionsListed {
        host: HostId,
        sessions: Vec<RemoteSession>,
    },
    /// list-sessions 失败但远端有 tmux
    TmuxQueryFailed {
        host: HostId,
        msg: String,
    },
    /// 远端没 tmux
    TmuxNoTmux {
        host: HostId,
    },
    /// AttachTmux 命令收到，actor 正在切 mode
    TmuxAttaching {
        host: HostId,
        session: SessionId,
    },
    /// -CC channel 已开 + TmuxController 已建
    TmuxAttached {
        host: HostId,
    },
    /// SessionTree 有变化
    TmuxSessionTreeUpdated {
        host: HostId,
        tree: SessionTree,
    },
    /// tmux 模式下某 pane 的输出
    TmuxPaneOutput {
        host: HostId,
        pane: PaneId,
        bytes: bytes::Bytes,
    },
    /// -CC channel 关闭
    TmuxDetached {
        host: HostId,
        reason: String,
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
    Resize {
        cols: u16,
        rows: u16,
    },
    Disconnect,
    /// 触发 ssh_actor 跑 tmux list-sessions（独立 channel exec）
    QueryTmuxSessions,
    /// user 点了某个 session，actor 关 raw shell -> 开新 channel attach
    AttachTmux {
        session: SessionId,
    },
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

/// 表单中选中的认证类型（radio 控件）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    KeyFile,
    Password,
}

/// 单个 host 的 tmux 状态。每次连接重置，断开清空。
#[derive(Debug, Clone)]
pub enum TmuxState {
    /// 刚连上，list-sessions 还没跑（瞬态）
    NotChecked,
    /// 远端没装 tmux（exec 失败 + stderr 含 "command not found" / "not found"）
    NoTmux,
    /// list-sessions 成功（可能空 vec — 远端有 tmux 但无 session）
    Detected { sessions: Vec<RemoteSession> },
    /// list-sessions 失败但远端有 tmux
    QueryFailed { msg: String },
    /// user 点了某 session，正在 attach（瞬态）
    Attaching { session: SessionId },
    /// 已 attach，TmuxController 在 actor 里运行，SessionTree clone 同步过来
    Attached { session_tree: SessionTree },
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
    /// 当前选中的 auth 类型（radio）
    pub auth_kind: AuthKind,
    /// auth_kind == KeyFile 时使用
    pub key_path: String,
    /// auth_kind == Password 时使用。
    /// 编辑模式下默认 ""，留空表示「不改密码」（保留 keyring 现有值）。
    pub password: String,
    /// 控制密码字段 mask / 明文 显示（👁 toggle）
    pub password_visible: bool,
    /// 校验失败时显示在 modal 底部的红字。
    pub error: Option<String>,
}

impl HostFormDraft {
    /// 从已有 HostConfig 填充（用于编辑）。
    /// 注意：Password 模式下 password 字段保持 ""，placeholder 提示「(unchanged)」；
    /// 不从 keyring 预读密码（最小化内存暴露 + 编辑保存空 = 不动 keyring）。
    pub fn from_config(cfg: &HostConfig) -> Self {
        let (auth_kind, key_path) = match &cfg.auth {
            aish_types::SshAuth::KeyFile { path } => {
                (AuthKind::KeyFile, path.display().to_string())
            }
            aish_types::SshAuth::Password { .. } => (AuthKind::Password, String::new()),
            aish_types::SshAuth::Agent => (AuthKind::KeyFile, String::new()),
        };
        Self {
            label: cfg.label.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            auth_kind,
            key_path,
            password: String::new(),
            password_visible: false,
            error: None,
        }
    }

    /// 校验并转回 HostConfig。`id` Some 表示编辑（保留原 id）/ None 表示新建。
    ///
    /// auth_kind 决定走 KeyFile 还是 Password 校验路径：
    ///   - KeyFile: 校验 key path 非空 + 文件存在
    ///   - Password: 校验 password 非空（**编辑模式例外**：编辑时空 password 表示「不改」，
    ///     由 caller 在 save 流程中区分；into_config 这里要求新建模式必须填密码）
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

        let auth = match self.auth_kind {
            AuthKind::KeyFile => {
                let key_path = self.key_path.trim();
                if key_path.is_empty() {
                    return Err("key path 不能为空".into());
                }
                let key_pathbuf = std::path::PathBuf::from(key_path);
                if !key_pathbuf.exists() {
                    return Err(format!("key 文件不存在: {}", key_path));
                }
                aish_types::SshAuth::KeyFile { path: key_pathbuf }
            }
            AuthKind::Password => {
                // 新建模式：必须填密码
                // 编辑模式：留空表示「不改」（caller 解释；into_config 仅做语义透传）
                if id.is_none() && self.password.is_empty() {
                    return Err("password 不能为空".into());
                }
                aish_types::SshAuth::Password {
                    password: self.password.clone(),
                }
            }
        };

        Ok(HostConfig {
            id: id.unwrap_or_else(|| HostId(uuid::Uuid::new_v4())),
            label: self.label.trim().into(),
            host: self.host.trim().into(),
            port,
            user: self.user.trim().into(),
            auth,
            env_profile: None,
        })
    }
}

/// 单一 root Model：所有 UI 共享状态。
#[derive(Default)]
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
    pub modal: Option<HostFormState>,

    /// raw shell 模式的 per-host alacritty Term（M2b1 行为）
    pub host_pty_term: HashMap<HostId, Term<VoidListener>>,
    pub host_pty_dimensions: HashMap<HostId, (u16, u16)>,

    /// 单个 host 的 tmux 状态（M3b 新加）
    pub tmux_state: HashMap<HostId, TmuxState>,
    /// tmux attach 后 per-pane alacritty Term
    pub pane_terminals: HashMap<(HostId, PaneId), Term<VoidListener>>,
    pub pane_dimensions: HashMap<(HostId, PaneId), (u16, u16)>,
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            selected: None,
            sessions: HashMap::new(),
            modal: None,
            host_pty_term: HashMap::new(),
            host_pty_dimensions: HashMap::new(),
            tmux_state: HashMap::new(),
            pane_terminals: HashMap::new(),
            pane_dimensions: HashMap::new(),
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
        self.host_pty_dimensions
            .insert(id, (DEFAULT_COLS, DEFAULT_ROWS));
    }

    pub fn drop_session(&mut self, id: HostId) {
        self.sessions.remove(&id);
        // 不删 host_pty_term — 保留 scrollback
        // 但清空 tmux_state（每次连接重新查询）
        self.tmux_state.remove(&id);
        // pane_terminals 也清空（tmux 模式 attach 状态丢失）
        self.pane_terminals.retain(|(h, _), _| h != &id);
        self.pane_dimensions.retain(|(h, _), _| h != &id);
    }

    /// raw shell 模式下，feed bytes 到指定 host 的 Term。
    pub fn feed_bytes(&mut self, host: HostId, bytes: &[u8]) {
        let (cols, rows) = self
            .host_pty_dimensions
            .get(&host)
            .copied()
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        let term = self
            .host_pty_term
            .entry(host)
            .or_insert_with(|| make_term(cols, rows));
        let mut processor = alacritty_terminal::vte::ansi::Processor::<
            alacritty_terminal::vte::ansi::StdSyncHandler,
        >::new();
        processor.advance(term, bytes);
    }

    /// raw shell 模式：取指定 host 的 Term（只读）。
    pub fn term_of(&self, host: HostId) -> Option<&Term<VoidListener>> {
        self.host_pty_term.get(&host)
    }

    /// raw shell 模式：resize host PTY 大小。
    pub fn resize_term(&mut self, host: HostId, cols: u16, rows: u16) {
        if let Some(term) = self.host_pty_term.get_mut(&host) {
            let size = TermSize::new(cols as usize, rows as usize);
            term.resize(size);
        }
        self.host_pty_dimensions.insert(host, (cols, rows));
    }

    /// tmux 模式：feed bytes 到指定 (host, pane) 的 Term。
    pub fn apply_tmux_pane_output(&mut self, host: HostId, pane: PaneId, bytes: &[u8]) {
        let key = (host, pane);
        let (cols, rows) = self
            .pane_dimensions
            .get(&key)
            .copied()
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        let term = self
            .pane_terminals
            .entry(key)
            .or_insert_with(|| make_term(cols, rows));
        let mut processor = alacritty_terminal::vte::ansi::Processor::<
            alacritty_terminal::vte::ansi::StdSyncHandler,
        >::new();
        processor.advance(term, bytes);
    }

    /// tmux 模式：更新 host 的 SessionTree（actor 推过来）。
    pub fn apply_tmux_session_tree(&mut self, host: HostId, tree: SessionTree) {
        self.tmux_state
            .insert(host, TmuxState::Attached { session_tree: tree });
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

    /// 删除 host。同步清理 sessions / host_pty_term / host_pty_dimensions / tmux_state /
    /// pane_terminals / pane_dimensions / 重置 selected。
    /// 返回 true = 成功删除，false = 未找到。
    ///
    /// **注意**：此函数**不**清理 keyring 条目 — 调用方（host_form save）
    /// 在调本函数前/后调 `persistence::delete_secret_for(id)`。
    pub fn remove_host(&mut self, id: HostId) -> bool {
        let idx = match self.hosts.iter().position(|h| h.id == id) {
            Some(i) => i,
            None => return false,
        };
        self.hosts.remove(idx);
        self.sessions.remove(&id);
        self.host_pty_term.remove(&id);
        self.host_pty_dimensions.remove(&id);
        self.tmux_state.remove(&id);
        self.pane_terminals.retain(|(h, _), _| h != &id);
        self.pane_dimensions.retain(|(h, _), _| h != &id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_tmux::SessionTree;
    use aish_types::SshAuth;
    use aish_types::{PaneId, SessionId};
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
        assert!(state.host_pty_term.is_empty());
        assert!(state.host_pty_dimensions.is_empty());
    }

    #[test]
    fn feed_bytes_creates_term_on_demand() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"hello\r\n");
        assert!(state.host_pty_term.contains_key(&id));
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
            state.host_pty_dimensions.get(&id),
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
        assert!(state.host_pty_term.contains_key(&id));
        assert!(!state.is_session_active(id));
    }

    #[test]
    fn resize_updates_dimensions() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"");
        state.resize_term(id, 100, 30);
        assert_eq!(state.host_pty_dimensions.get(&id), Some(&(100, 30)));
    }

    fn write_temp_key_file() -> tempfile::NamedTempFile {
        tempfile::NamedTempFile::new().expect("temp file")
    }

    #[test]
    fn draft_keyfile_into_config_validates_empty_label() {
        let draft = HostFormDraft {
            label: "".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: "/tmp/x".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("label"));
    }

    #[test]
    fn draft_keyfile_into_config_validates_port_non_numeric() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "abc".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: "/tmp/x".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        assert!(draft.into_config(None).unwrap_err().contains("port"));
    }

    #[test]
    fn draft_keyfile_into_config_validates_key_file_exists() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: "/nonexistent/path/aish_test_only".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        assert!(draft
            .into_config(None)
            .unwrap_err()
            .contains("key 文件不存在"));
    }

    #[test]
    fn draft_keyfile_into_config_succeeds_with_existing_key() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: key.path().display().to_string(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let cfg = draft.into_config(None).unwrap();
        assert_eq!(cfg.label, "v");
        assert!(matches!(cfg.auth, aish_types::SshAuth::KeyFile { .. }));
    }

    #[test]
    fn draft_keyfile_into_config_preserves_id_when_provided() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: key.path().display().to_string(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let id = HostId(Uuid::new_v4());
        let cfg = draft.into_config(Some(id)).unwrap();
        assert_eq!(cfg.id, id);
    }

    #[test]
    fn draft_password_new_requires_nonempty_password() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::Password,
            key_path: "".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("password"));
    }

    #[test]
    fn draft_password_new_succeeds_with_password() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::Password,
            key_path: "".into(),
            password: "secret".into(),
            password_visible: false,
            error: None,
        };
        let cfg = draft.into_config(None).unwrap();
        match cfg.auth {
            aish_types::SshAuth::Password { password } => assert_eq!(password, "secret"),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn draft_password_edit_allows_empty_password() {
        let id = HostId(Uuid::new_v4());
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::Password,
            key_path: "".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let cfg = draft.into_config(Some(id)).unwrap();
        match cfg.auth {
            aish_types::SshAuth::Password { password } => assert_eq!(password, ""),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn draft_from_config_password_keeps_password_empty() {
        let host = HostConfig {
            id: HostId::new(),
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: aish_types::SshAuth::Password {
                password: "this-should-be-ignored".into(),
            },
            env_profile: None,
        };
        let draft = HostFormDraft::from_config(&host);
        assert_eq!(draft.auth_kind, AuthKind::Password);
        assert_eq!(draft.password, ""); // 不预填
    }

    #[test]
    fn draft_from_config_keyfile_extracts_path() {
        let h = mk_host("v");
        let draft = HostFormDraft::from_config(&h);
        assert_eq!(draft.auth_kind, AuthKind::KeyFile);
        assert!(draft.key_path.contains("/tmp/k") || draft.key_path.contains("\\tmp\\k"));
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
        state.feed_bytes(id, b"hello");
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        state.select_host(id);

        let ok = state.remove_host(id);
        assert!(ok);
        assert!(state.hosts.is_empty());
        assert!(!state.host_pty_term.contains_key(&id));
        assert!(!state.host_pty_dimensions.contains_key(&id));
        assert!(!state.is_session_active(id));
        assert_eq!(state.selected, None);
        assert!(!state.tmux_state.contains_key(&id));
    }

    #[test]
    fn remove_host_returns_false_for_unknown_id() {
        let mut state = AppState::with_hosts(vec![]);
        let unknown = HostId(Uuid::new_v4());
        assert!(!state.remove_host(unknown));
    }

    #[test]
    fn apply_tmux_pane_output_creates_per_pane_term() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.apply_tmux_pane_output(id, PaneId(7), b"hi");
        let term = state.pane_terminals.get(&(id, PaneId(7))).unwrap();
        let grid = term.grid();
        let first_row = &grid[alacritty_terminal::index::Line(0)];
        assert_eq!(first_row[alacritty_terminal::index::Column(0)].c, 'h');
        assert_eq!(first_row[alacritty_terminal::index::Column(1)].c, 'i');
    }

    #[test]
    fn apply_tmux_session_tree_sets_attached_state() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let tree = SessionTree::new();
        state.apply_tmux_session_tree(id, tree);
        match state.tmux_state.get(&id) {
            Some(TmuxState::Attached { .. }) => {}
            other => panic!("expected Attached, got {:?}", other),
        }
    }

    #[test]
    fn drop_session_clears_tmux_state_and_pane_terminals() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        state.apply_tmux_pane_output(id, PaneId(0), b"x");
        state.tmux_state.insert(id, TmuxState::NotChecked);
        state.drop_session(id);
        assert!(!state.tmux_state.contains_key(&id));
        assert!(!state.pane_terminals.contains_key(&(id, PaneId(0))));
    }

    // SessionId 在 tests 模块中未直接构造，但 import 已在，编译器会检测到未使用
    // 保留 import 以确保类型可用性验证
    #[allow(dead_code)]
    fn _assert_session_id_usable() -> SessionId {
        SessionId::new("test")
    }
}
