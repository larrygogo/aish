//! aish-app App State。
//!
//! M3b 起：HostConfig（持久化配置，键 `HostId`）与 Connection（运行时连接，
//! 键 `ConnectionId`）分离。一个 HostConfig 可派生 N 个并发 Connection，
//! 每个 Connection 有独立的 actor / PTY / tmux 状态。所有 per-runtime 状态
//! map（sessions / host_pty_term / host_pty_dimensions / tmux_state）以
//! `ConnectionId` 为键。

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::SystemTime;

use aish_types::{ConnectionId, HostConfig, HostId, RemoteSession, SessionId, TabId};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::vte::ansi::{Processor as AnsiProcessor, StdSyncHandler};
use alacritty_terminal::Term;
use tokio::sync::mpsc;

/// 从 SSH actor task 推回 GPUI 的事件。事件以 `ConnectionId` 寻址 —— 同一
/// HostConfig 的多个连接需要被独立路由到各自的 Term / tmux_state。
#[derive(Debug)]
pub enum SshEvent {
    Connected {
        conn: ConnectionId,
    },
    PaneOutput {
        conn: ConnectionId,
        bytes: Vec<u8>,
    },
    Disconnected {
        conn: ConnectionId,
        reason: DisconnectReason,
    },
    Error {
        conn: ConnectionId,
        kind: SshErrorKind,
        msg: String,
    },
    /// list-sessions 开始
    TmuxQueryStarted {
        conn: ConnectionId,
    },
    /// list-sessions 成功（包括 tmux 在但 0 session 的情况）
    TmuxSessionsListed {
        conn: ConnectionId,
        sessions: Vec<RemoteSession>,
    },
    /// list-sessions 失败但远端有 tmux
    TmuxQueryFailed {
        conn: ConnectionId,
        msg: String,
    },
    /// 远端没 tmux
    TmuxNoTmux {
        conn: ConnectionId,
    },
    /// AttachTmux 命令已派发到 raw shell channel。
    TmuxAttached {
        conn: ConnectionId,
        session: SessionId,
    },
    /// SFTP 上传成功，path 是远端绝对路径（如 /tmp/aish-clip-123456.png）。
    ImageUploaded {
        conn: ConnectionId,
        path: String,
    },
    /// SFTP 上传失败，msg 是错误描述。
    ImageUploadFailed {
        conn: ConnectionId,
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
    /// 上传本地剪贴板图片（PNG bytes）到远端 /tmp。
    UploadImage {
        data: Vec<u8>,
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
///
/// M3-archived（2026-05-07）：之前的 `Attaching` / `Attached { session_tree }`
/// 是 tmux -CC 控制模式专用，aish 已回退到 raw attach 路径，attach 后由 tmux
/// 自身绘制 UI（含状态栏），状态栏字段 / SessionTree / pane 树都不再维护。
/// 当前侧栏只展示 list-sessions 的 sessions 列表 + "最近 attached 的 SessionId"
/// 用于高亮，不区分 Attaching 中间态。
#[derive(Debug, Clone)]
pub enum TmuxState {
    /// 刚连上，list-sessions 还没跑（瞬态）
    NotChecked,
    /// 远端没装 tmux（exec 失败 + stderr 含 "command not found" / "not found"）
    NoTmux,
    /// list-sessions 成功（可能空 vec — 远端有 tmux 但无 session）。
    /// `attached` 记录最近一次点击 attach 的 session id，仅用于侧栏高亮；
    /// 用户在 tmux 内 detach 后，aish 不感知（保持 Some），需要重新 list 才会更新。
    Detected {
        sessions: Vec<RemoteSession>,
        attached: Option<SessionId>,
    },
    /// list-sessions 失败但远端有 tmux
    QueryFailed { msg: String },
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

/// 一个活跃连接的元数据（运行时数据，不持久化）。
#[derive(Debug, Clone)]
pub struct Connection {
    pub id: ConnectionId,
    pub host_id: HostId,
    /// 显示用，自动生成 `"<host.label> #N"`。N 从 1 开始按 host 内自增。
    pub label: String,
    pub opened_at: SystemTime,
}

/// 顶层 4-tab 导航当前选中项（M4a 信息架构）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SidebarTab {
    #[default]
    Home,
    Terminal,
    Inbox,
    Settings,
}

/// Tab 内容类型。默认页显示 host 卡片，连接页显示该 connection 的终端。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabContent {
    Default,
    Connection(ConnectionId),
}

/// 顶部 tab 栏的一个 tab。
#[derive(Debug, Clone)]
pub struct Tab {
    pub id: TabId,
    pub content: TabContent,
    pub title: String,
}

/// 单一 root Model：所有 UI 共享状态。
#[derive(Default)]
pub struct AppState {
    /// 持久化配置列表（hosts.json 内容）。
    pub hosts: Vec<HostConfig>,
    /// 活跃连接元数据。键 ConnectionId 对应运行时 actor。
    pub connections: HashMap<ConnectionId, Connection>,

    /// 顶部 tab 栏（顺序敏感）。
    pub tabs: Vec<Tab>,
    /// 当前选中的 tab。`None` 仅在 tabs 为空时出现（启动一瞬间）。
    pub selected_tab: Option<TabId>,
    /// 等待用户在 session picker 弹窗里选择的连接。
    /// `Some(conn)` = 该 conn 已收到 TmuxSessionsListed 但用户还没选；显示弹窗。
    /// `None` = 无弹窗。
    pub pending_session_picker: Option<ConnectionId>,

    pub modal: Option<HostFormState>,
    pub last_connected: HashMap<HostId, SystemTime>,
    pub sidebar: SidebarTab,

    /// 每连接一个 actor 命令通道。
    pub sessions: HashMap<ConnectionId, mpsc::Sender<SessionCommand>>,
    /// 每连接一个 alacritty Term（保留 scrollback）。
    pub host_pty_term: HashMap<ConnectionId, Term<VoidListener>>,
    /// 每连接一个 ANSI parser。Processor 是 stateful（VTE parser 跨字节包
    /// 维护 escape sequence 解析进度），必须 per-conn 持久化。之前每次
    /// feed_bytes 都 Processor::new()，escape 跨 SSH frame 时会被错解析。
    pub host_pty_processor: HashMap<ConnectionId, AnsiProcessor<StdSyncHandler>>,
    /// 每连接一个 PTY 尺寸。
    pub host_pty_dimensions: HashMap<ConnectionId, (u16, u16)>,
    /// 每连接一个 tmux 状态（同一 host 的多个连接独立 list-sessions）。
    pub tmux_state: HashMap<ConnectionId, TmuxState>,
}

impl Connection {
    /// 返回自 opened_at 到现在的 humanize 字符串，用于 Active Sessions 显示。
    pub fn humanize_opened_at(&self) -> String {
        let secs = self.opened_at.elapsed().unwrap_or_default().as_secs();
        if secs < 60 {
            "just now".into()
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else if secs < 172800 {
            "yesterday".into()
        } else {
            format!("{}d ago", secs / 86400)
        }
    }
}

/// 将历史时间戳转为可读字符串（同 humanize_opened_at 阈值，接受 SystemTime 参数）。
pub fn humanize_last_connected(last: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(last)
        .unwrap_or_default()
        .as_secs();
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 172800 {
        "yesterday".into()
    } else {
        format!("{}d ago", secs / 86400)
    }
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            connections: HashMap::new(),
            tabs: vec![],
            selected_tab: None,
            sidebar: SidebarTab::Home,
            pending_session_picker: None,
            sessions: HashMap::new(),
            modal: None,
            last_connected: HashMap::new(),
            host_pty_term: HashMap::new(),
            host_pty_processor: HashMap::new(),
            host_pty_dimensions: HashMap::new(),
            tmux_state: HashMap::new(),
        }
    }

    // ───────── Tab 管理 ─────────

    /// 当前选中的 tab。
    pub fn current_tab(&self) -> Option<&Tab> {
        let id = self.selected_tab?;
        self.tabs.iter().find(|t| t.id == id)
    }

    /// 当前选中 tab 对应的 connection（若该 tab 是 Connection 类型）。
    pub fn current_connection(&self) -> Option<ConnectionId> {
        match self.current_tab()?.content {
            TabContent::Connection(c) => Some(c),
            TabContent::Default => None,
        }
    }

    pub fn select_tab(&mut self, id: TabId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.selected_tab = Some(id);
        }
    }

    /// 把当前 tab 替换为指定 content/title（用于"在默认页里点了 host 卡片
    /// → 同一个 tab 变成 connection tab"的流程）。
    pub fn replace_current_tab(&mut self, content: TabContent, title: String) {
        let id = match self.selected_tab {
            Some(i) => i,
            None => return,
        };
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.content = content;
            t.title = title;
        }
    }

    /// 关闭一个 tab。如果是 Connection tab，**调用方**负责发 SessionCommand::Disconnect
    /// 并 remove_connection（因为发命令需要 bridge）。本函数只做 state 端的 tab 维护：
    ///   - 从 tabs 列表移除
    ///   - 如果是当前选中，自动切到相邻
    ///   - 如果删完了所有 tab，自动开一个新的默认页（避免空白）
    pub fn close_tab(&mut self, id: TabId) -> Option<TabContent> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let removed = self.tabs.remove(idx);
        if self.selected_tab == Some(id) {
            // 选相邻 tab；若全空则新建一个默认页
            self.selected_tab = self
                .tabs
                .get(idx)
                .or_else(|| self.tabs.last())
                .map(|t| t.id);
            // tabs 可以为空，sidebar=Terminal 时主区会显示 EmptyTerminalGuideView
        }
        Some(removed.content)
    }

    pub fn host_label(&self, id: HostId) -> Option<String> {
        self.hosts
            .iter()
            .find(|h| h.id == id)
            .map(|h| h.label.clone())
    }

    /// 给某 host 生成下一个连接 label：`"<host.label> #N"`，N = 当前该 host
    /// 的连接数 + 1（没考虑回收 id —— 用户关一个再开一个会得到更大的 N）。
    fn next_label_for(&self, host_id: HostId) -> String {
        let base = self.host_label(host_id).unwrap_or_else(|| "host".into());
        let n = self
            .connections
            .values()
            .filter(|c| c.host_id == host_id)
            .count()
            + 1;
        format!("{} #{}", base, n)
    }

    /// 创建一个新 Connection 元数据；caller 之后再 `register_session` 绑定 actor sender。
    /// 返回新生成的 ConnectionId。注意本函数**不**改 tab 状态 —— 调用方决定是
    /// 替换当前 tab 还是新开一个 tab 显示该 connection。
    pub fn open_connection(&mut self, host_id: HostId) -> ConnectionId {
        let id = ConnectionId::new();
        let label = self.next_label_for(host_id);
        self.connections.insert(
            id,
            Connection {
                id,
                host_id,
                label,
                opened_at: SystemTime::now(),
            },
        );
        id
    }

    /// 该连接是否仍持有 actor sender（即"还活着"）。
    pub fn is_session_active(&self, id: ConnectionId) -> bool {
        self.sessions.contains_key(&id)
    }

    pub fn register_session(&mut self, id: ConnectionId, sender: mpsc::Sender<SessionCommand>) {
        self.sessions.insert(id, sender);
        self.host_pty_dimensions
            .insert(id, (DEFAULT_COLS, DEFAULT_ROWS));
    }

    /// 关闭一个连接：清 actor sender + tmux_state，保留 host_pty_term（用户可能想看 scrollback）。
    /// 完全清理（含 Term + Connection meta）走 [`remove_connection`]。
    pub fn drop_session(&mut self, id: ConnectionId) {
        self.sessions.remove(&id);
        self.tmux_state.remove(&id);
    }

    /// 完全移除一个连接：从 connections 和所有 per-conn map 里删掉。
    /// 如果有 tab 引用了它，把那些 tab 的 content 改为 Default（保留 tab 不删）。
    pub fn remove_connection(&mut self, id: ConnectionId) {
        self.connections.remove(&id);
        self.sessions.remove(&id);
        self.host_pty_term.remove(&id);
        self.host_pty_processor.remove(&id);
        self.host_pty_dimensions.remove(&id);
        self.tmux_state.remove(&id);
        let ids_to_close: Vec<TabId> = self
            .tabs
            .iter()
            .filter(|t| t.content == TabContent::Connection(id))
            .map(|t| t.id)
            .collect();
        for tab_id in ids_to_close {
            self.close_tab(tab_id);
        }
        // 关掉的连接如果正在弹 picker，也得清
        if self.pending_session_picker == Some(id) {
            self.pending_session_picker = None;
        }
    }

    /// 标记某连接已 attach 到指定 tmux session（仅用于侧栏高亮）。
    pub fn mark_tmux_attached(&mut self, conn: ConnectionId, session: SessionId) {
        if let Some(TmuxState::Detected { attached, .. }) = self.tmux_state.get_mut(&conn) {
            *attached = Some(session);
        }
    }

    /// raw shell 模式下，feed bytes 到指定连接的 Term。
    pub fn feed_bytes(&mut self, conn: ConnectionId, bytes: &[u8]) {
        let (cols, rows) = self
            .host_pty_dimensions
            .get(&conn)
            .copied()
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        let term = self
            .host_pty_term
            .entry(conn)
            .or_insert_with(|| make_term(cols, rows));
        // Processor 跨 feed_bytes 持久化 — 让 ANSI escape 序列跨 SSH frame
        // 仍能正确解析（之前每次 new 会让 \x1b[3 / 1m 这种切包被当字面字符）。
        let processor = self.host_pty_processor.entry(conn).or_default();
        processor.advance(term, bytes);
    }

    /// 取指定连接的 Term（只读）。
    pub fn term_of(&self, conn: ConnectionId) -> Option<&Term<VoidListener>> {
        self.host_pty_term.get(&conn)
    }

    /// 调整指定连接的 PTY 大小（GPUI 端 alacritty grid + 远端 SIGWINCH 由 actor 完成）。
    pub fn resize_term(&mut self, conn: ConnectionId, cols: u16, rows: u16) {
        if let Some(term) = self.host_pty_term.get_mut(&conn) {
            let size = TermSize::new(cols as usize, rows as usize);
            term.resize(size);
        }
        self.host_pty_dimensions.insert(conn, (cols, rows));
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

    /// 删除 host 配置。**保留**该 host 名下所有活跃 Connection（继续运行直到
    /// 用户主动断开或 actor 自然退出）—— 删配置不强制断连。
    ///
    /// **注意**：此函数**不**清理 keyring 条目 — 调用方（host_form save）
    /// 在调本函数前/后调 `persistence::delete_secret_for(id)`。
    pub fn remove_host(&mut self, id: HostId) -> bool {
        let idx = match self.hosts.iter().position(|h| h.id == id) {
            Some(i) => i,
            None => return false,
        };
        self.hosts.remove(idx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_types::SessionId;
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
        assert!(state.host_pty_term.is_empty());
        assert!(state.host_pty_dimensions.is_empty());
    }

    #[test]
    fn feed_bytes_creates_term_on_demand() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        state.feed_bytes(conn, b"hello\r\n");
        assert!(state.host_pty_term.contains_key(&conn));
    }

    #[test]
    fn feed_bytes_reflects_in_term_grid() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        state.feed_bytes(conn, b"abc");
        let term = state.term_of(conn).unwrap();
        let grid = term.grid();
        let first_row = &grid[alacritty_terminal::index::Line(0)];
        assert_eq!(first_row[alacritty_terminal::index::Column(0)].c, 'a');
        assert_eq!(first_row[alacritty_terminal::index::Column(1)].c, 'b');
        assert_eq!(first_row[alacritty_terminal::index::Column(2)].c, 'c');
    }

    #[tokio::test]
    async fn register_session_inits_dimensions() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(conn, tx);
        assert_eq!(
            state.host_pty_dimensions.get(&conn),
            Some(&(DEFAULT_COLS, DEFAULT_ROWS))
        );
    }

    #[tokio::test]
    async fn drop_session_keeps_terminal() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(conn, tx);
        state.feed_bytes(conn, b"x");
        state.drop_session(conn);
        assert!(state.host_pty_term.contains_key(&conn));
        assert!(!state.is_session_active(conn));
    }

    #[test]
    fn resize_updates_dimensions() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        state.feed_bytes(conn, b"");
        state.resize_term(conn, 100, 30);
        assert_eq!(state.host_pty_dimensions.get(&conn), Some(&(100, 30)));
    }

    #[test]
    fn open_connection_assigns_incrementing_label() {
        let h = mk_host("box");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let c1 = state.open_connection(host_id);
        let c2 = state.open_connection(host_id);
        assert_ne!(c1, c2);
        assert_eq!(state.connections[&c1].label, "box #1");
        assert_eq!(state.connections[&c2].label, "box #2");
    }

    #[test]
    fn remove_connection_clears_all_per_conn_state() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        state.feed_bytes(conn, b"x");
        state.host_pty_dimensions.insert(conn, (80, 24));
        state.tmux_state.insert(conn, TmuxState::NotChecked);
        state.remove_connection(conn);
        assert!(!state.connections.contains_key(&conn));
        assert!(!state.host_pty_term.contains_key(&conn));
        assert!(!state.host_pty_dimensions.contains_key(&conn));
        assert!(!state.tmux_state.contains_key(&conn));
    }

    #[test]
    fn remove_connection_closes_referencing_tab() {
        use aish_types::TabId;
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        // 手动 push 一个 Connection tab
        let tab_id = TabId::new();
        state.tabs.push(Tab {
            id: tab_id,
            content: TabContent::Connection(conn),
            title: "x".into(),
        });
        state.selected_tab = Some(tab_id);
        state.remove_connection(conn);
        // 该 tab 应被关闭
        assert!(!state.tabs.iter().any(|t| t.id == tab_id));
    }

    #[test]
    fn replace_current_tab_swaps_in_place() {
        use aish_types::TabId;
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        // 手动 push 一个初始 tab（M4a 起 with_hosts 不自动创建）
        let initial_tab_id = TabId::new();
        state.tabs.push(Tab {
            id: initial_tab_id,
            content: TabContent::Default,
            title: "新连接".into(),
        });
        state.selected_tab = Some(initial_tab_id);
        state.replace_current_tab(TabContent::Connection(conn), "腾讯云 #1".into());
        assert_eq!(state.selected_tab, Some(initial_tab_id));
        assert_eq!(state.current_tab().unwrap().title, "腾讯云 #1");
        assert_eq!(state.current_connection(), Some(conn));
    }

    #[test]
    fn close_tab_picks_neighbor_when_current() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        // 手动 push 两个 tab
        let id1 = TabId::new();
        let id2 = TabId::new();
        state.tabs.push(Tab {
            id: id1,
            content: TabContent::Default,
            title: "1".into(),
        });
        state.tabs.push(Tab {
            id: id2,
            content: TabContent::Default,
            title: "2".into(),
        });
        state.selected_tab = Some(id2);
        state.close_tab(id2);
        assert_eq!(state.selected_tab, Some(id1));
        assert_eq!(state.tabs.len(), 1);
    }

    #[test]
    fn remove_host_keeps_active_connections() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        assert!(state.remove_host(host_id));
        // host 配置删了，但 connection 还在 — 用户能继续用直到主动断
        assert!(state.hosts.is_empty());
        assert!(state.connections.contains_key(&conn));
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

    #[test]
    fn remove_host_only_removes_config() {
        let h = mk_host("v");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let ok = state.remove_host(host_id);
        assert!(ok);
        assert!(state.hosts.is_empty());
    }

    #[test]
    fn remove_host_returns_false_for_unknown_id() {
        let mut state = AppState::with_hosts(vec![]);
        let unknown = HostId(Uuid::new_v4());
        assert!(!state.remove_host(unknown));
    }

    #[test]
    fn drop_session_clears_tmux_state() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(conn, tx);
        state.tmux_state.insert(conn, TmuxState::NotChecked);
        state.drop_session(conn);
        assert!(!state.tmux_state.contains_key(&conn));
    }

    #[test]
    fn mark_tmux_attached_updates_detected_state() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        let sess = SessionId::new("$0");
        state.tmux_state.insert(
            conn,
            TmuxState::Detected {
                sessions: vec![],
                attached: None,
            },
        );
        state.mark_tmux_attached(conn, sess.clone());
        match state.tmux_state.get(&conn) {
            Some(TmuxState::Detected { attached, .. }) => {
                assert_eq!(attached.as_ref(), Some(&sess))
            }
            other => panic!("expected Detected, got {:?}", other),
        }
    }

    #[test]
    fn mark_tmux_attached_noop_when_not_detected() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        state.tmux_state.insert(conn, TmuxState::NotChecked);
        state.mark_tmux_attached(conn, SessionId::new("$0"));
        assert!(matches!(
            state.tmux_state.get(&conn),
            Some(TmuxState::NotChecked)
        ));
    }

    #[test]
    fn sidebar_default_is_home() {
        let state = AppState::with_hosts(vec![]);
        assert_eq!(state.sidebar, SidebarTab::Home);
    }

    #[test]
    fn with_hosts_starts_with_empty_tabs() {
        let state = AppState::with_hosts(vec![]);
        assert!(state.tabs.is_empty());
        assert_eq!(state.selected_tab, None);
    }

    #[test]
    fn close_tab_allows_empty_tabs() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        // 手动 push 一个 tab 再关掉
        let tab_id = TabId::new();
        state.tabs.push(Tab {
            id: tab_id,
            content: TabContent::Default,
            title: "test".into(),
        });
        state.selected_tab = Some(tab_id);
        state.close_tab(tab_id);
        assert!(
            state.tabs.is_empty(),
            "tabs should be empty after closing last tab"
        );
    }

    #[test]
    fn humanize_opened_at_just_now() {
        use std::time::SystemTime;
        let conn = Connection {
            id: ConnectionId::new(),
            host_id: aish_types::HostId::new(),
            label: "test".into(),
            opened_at: SystemTime::now(),
        };
        assert_eq!(conn.humanize_opened_at(), "just now");
    }

    #[test]
    fn humanize_opened_at_minutes() {
        use std::time::{Duration, SystemTime};
        let conn = Connection {
            id: ConnectionId::new(),
            host_id: aish_types::HostId::new(),
            label: "test".into(),
            opened_at: SystemTime::now() - Duration::from_secs(125),
        };
        assert_eq!(conn.humanize_opened_at(), "2m ago");
    }

    #[test]
    fn humanize_last_connected_just_now() {
        let t = SystemTime::now();
        assert_eq!(humanize_last_connected(t), "just now");
    }

    #[test]
    fn humanize_last_connected_hours() {
        let t = SystemTime::now() - std::time::Duration::from_secs(7200);
        assert_eq!(humanize_last_connected(t), "2h ago");
    }

    #[test]
    fn last_connected_field_accessible() {
        let state = AppState::with_hosts(vec![]);
        assert!(state.last_connected.is_empty());
    }

    #[test]
    fn upload_image_command_constructible() {
        let cmd = SessionCommand::UploadImage {
            data: vec![0u8, 1, 2, 3],
        };
        match cmd {
            SessionCommand::UploadImage { data } => assert_eq!(data.len(), 4),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn image_uploaded_event_carries_path() {
        use aish_types::ConnectionId;
        let conn = ConnectionId::new();
        let event = SshEvent::ImageUploaded {
            conn,
            path: "/tmp/aish-clip-123.png".into(),
        };
        match event {
            SshEvent::ImageUploaded { conn: c, path } => {
                assert_eq!(c, conn);
                assert_eq!(path, "/tmp/aish-clip-123.png");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn image_upload_failed_event_carries_msg() {
        use aish_types::ConnectionId;
        let conn = ConnectionId::new();
        let event = SshEvent::ImageUploadFailed {
            conn,
            msg: "permission denied".into(),
        };
        match event {
            SshEvent::ImageUploadFailed { conn: c, msg } => {
                assert_eq!(c, conn);
                assert!(msg.contains("permission"));
            }
            _ => panic!("wrong variant"),
        }
    }
}
