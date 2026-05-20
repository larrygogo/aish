//! aish-app App State。
//!
//! M3b 起：HostConfig（持久化配置，键 `HostId`）与 Connection（运行时连接，
//! 键 `ConnectionId`）分离。一个 HostConfig 可派生 N 个并发 Connection，
//! 每个 Connection 有独立的 actor / PTY / tmux 状态。所有 per-runtime 状态
//! map（sessions / host_pty_term / host_pty_dimensions / tmux_state）以
//! `ConnectionId` 为键。

use std::collections::HashMap;
use std::time::SystemTime;

use aish_types::{
    ConnectionId, HostCapabilities, HostConfig, HostId, RemoteSession, SessionId, TabId,
};
use alacritty_terminal::event::{Event as TermEvent, EventListener};
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
    /// 远端 tmux 装了但 mouse 没开。鼠标 click/drag/wheel 在 tmux 内不生效，
    /// 弹 toast 引导用户加 `set -g mouse on` 到 ~/.tmux.conf。
    /// 仅在 list-sessions 成功（tmux 确实在）后才会发，无 tmux 不会触发。
    TmuxMouseDisabled {
        conn: ConnectionId,
    },
    /// AttachTmux 命令已派发到 raw shell channel。
    TmuxAttached {
        conn: ConnectionId,
        session: SessionId,
    },
    /// 远端 tmux client 退出（用户在 tmux 内按 prefix+d / detach 命令 /
    /// kill-session 等触发）。actor 在 channel 输出里检测到 "[detached"
    /// 字符串后发，让 app.rs 清 tmux_state[conn].attached 标记，sidebar
    /// 同步显示"已 detach"状态。
    TmuxSessionDetached {
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
    /// 探测到远程系统类型（解析 /etc/os-release 的 ID 字段）。app.rs 收到
    /// 后写入对应 HostConfig.os_kind 并 persist。`None` = 探测失败 / macOS
    /// 没该文件 / 命令 exec 出错，仍走事件让 UI 不再无限等待。
    OsDetected {
        host_id: aish_types::HostId,
        os_kind: Option<String>,
    },
    /// 批量上传进度：每张图（无论成败）完成后发一次。done/total 用于 UI 进度展示。
    BatchProgress {
        conn: ConnectionId,
        done: usize,
        total: usize,
    },
    /// 批量上传全部结束（无论失败几张）。text 是用户在 input bar 填的附加文字，
    /// 由 app.rs 在收到本事件后 append 到 PTY（path 是逐张到达 ImageUploaded
    /// 时立即 append 的，text 只能等 batch 结束才发，否则可能在 path 之前出现）。
    BatchDone {
        conn: ConnectionId,
        text: String,
    },
    /// 批量上传中途失败（含 SFTP 错误 / 单张超时 30s）。actor 一旦遇错立即
    /// break 不再发后续 BatchProgress / BatchDone，改发本事件。
    /// - `succeeded`：失败前已成功的张数（path 已逐张 append 到 PTY）
    /// - `total`：本批原始张数
    /// - `reason`：失败原因（toast 展示）
    ///
    /// InputBar 收到后 drain 前 succeeded 张缩略图（已传成功的视觉移除），
    /// 保留剩余 images + text 供用户点 "发送" retry。
    BatchAborted {
        conn: ConnectionId,
        succeeded: usize,
        total: usize,
        reason: String,
    },
    /// 远端 shell 通过 OSC 0/1/2 escape sequence 设置 tab 标题
    /// （bash/zsh 默认 PS1 hook 会 emit `ESC ]0;user@host: cwd\BEL` 等）。
    /// alacritty Term 的 Event::Title 通过自定义 EventListener 转发至此。
    /// 仅在对应 tab.title_locked == false 时覆盖 tab.title。
    TitleChanged {
        conn: ConnectionId,
        title: String,
    },
    /// 远端通过 OSC 52 escape sequence 把文本写入本机剪贴板（tmux copy-mode
    /// "y" / vim "+y" / 任何 set-clipboard on 的工具）。alacritty 已 base64
    /// decode，text 是明文。app.rs 收到后 cx.write_to_clipboard 真正落盘。
    ClipboardWrite {
        text: String,
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
    /// 批量上传图片并追加文字到 PTY。images 是 (原始文件字节, 扩展名不含点) 列表。
    UploadBatch {
        images: Vec<(Vec<u8>, String)>,
        text: String,
    },
}

/// 默认 PTY 大小（首次 connect 时用，后续按窗口 resize 调整）。
pub const DEFAULT_COLS: u16 = 120;
pub const DEFAULT_ROWS: u16 = 40;

/// scrollback buffer 大小。
const SCROLLBACK_LINES: usize = 10_000;

/// alacritty Term 的 EventListener 实现：拦截 OSC 0/1/2 → Event::Title 转发到
/// SshEvent::TitleChanged 让 app.rs 更新 tab.title。其它 alacritty event 丢弃。
///
/// send_event 是 &self（不是 &mut），必须 try_send 同步推 channel。容量满时
/// 静默丢弃（title 变化非关键事件，丢一两条无所谓 —— 下次 PS1 emit 会重发）。
pub struct TitleListener {
    conn: ConnectionId,
    /// None = void mode（测试 fixture / 未注入 event_tx 时）。
    tx: Option<mpsc::Sender<SshEvent>>,
}

impl EventListener for TitleListener {
    fn send_event(&self, ev: TermEvent) {
        let Some(tx) = self.tx.as_ref() else {
            return;
        };
        match ev {
            TermEvent::Title(t) => {
                let _ = tx.try_send(SshEvent::TitleChanged {
                    conn: self.conn,
                    title: t,
                });
            }
            TermEvent::ResetTitle => {
                let _ = tx.try_send(SshEvent::TitleChanged {
                    conn: self.conn,
                    title: String::new(),
                });
            }
            // OSC 52 - 远端要求写本机剪贴板（tmux copy-mode / vim "+y" 等）。
            // alacritty 已 base64 decode，text 是明文。
            TermEvent::ClipboardStore(_ty, text) => {
                let _ = tx.try_send(SshEvent::ClipboardWrite { text });
            }
            _ => {}
        }
    }
}

/// 创建一个空 Term。tx None 时 listener 走 void（测试用），Some 时 OSC 0/1/2
/// title 通过 channel 推回 GPUI 主循环。
pub fn make_term(
    conn: ConnectionId,
    tx: Option<mpsc::Sender<SshEvent>>,
    cols: u16,
    rows: u16,
) -> Term<TitleListener> {
    let size = TermSize::new(cols as usize, rows as usize);
    let config = TermConfig {
        scrolling_history: SCROLLBACK_LINES,
        ..TermConfig::default()
    };
    Term::new(config, &size, TitleListener { conn, tx })
}

/// 表单中选中的认证类型（radio 控件）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    KeyFile,
    Password,
}

/// 单个连接的生命周期阶段。
///
/// 流程：register_session 时设 Connecting → SshEvent::Connected 设 Connected
/// → Disconnected / Error 设 Disconnected{reason}。reopen 时回到 Connecting。
/// connection 元数据 + scrollback Term 在 Disconnected 时**仍保留**，让用户能
/// 看到断开前的输出 + 点击"重连"复用同一 ConnectionId 重启 actor。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionPhase {
    Connecting,
    Connected,
    Disconnected { reason: String },
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
    /// 校验失败时显示在 modal 底部的红字。
    pub error: Option<String>,
}

impl HostFormDraft {
    /// 从已有 HostConfig 填充（用于编辑）。
    /// 注意：Password 模式下 password 字段保持 ""，placeholder 提示「(unchanged)」；
    /// 不从 keyring 预读密码（最小化内存暴露 + 编辑保存空 = 不动 keyring）。
    pub fn from_config(cfg: &HostConfig) -> Self {
        let (auth_kind, key_path) = match &cfg.auth {
            aish_types::SshAuth::KeyFile { path, .. } => {
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
                // passphrase 复用 self.password 字段（KeyFile 模式下当 passphrase；
                // 未加密私钥则保持 ""）。语义同 Password 模式：空表示不改 keyring
                // 已存 passphrase（编辑场景）；新建时空表示明文未加密私钥。
                aish_types::SshAuth::KeyFile {
                    path: key_pathbuf,
                    passphrase: self.password.clone(),
                }
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
            capabilities: HostCapabilities::default(),
        })
    }
}

/// 一个活跃连接的元数据（运行时数据，不持久化）。
#[derive(Debug, Clone)]
pub struct Connection {
    // M36 T3: id 字段是 HashMap key 的副本，原 home.rs active_connections
    // 迭代用 c.id；现在 home.rs 改走 `(id, conn)` iter pattern 后无 read 路径，
    // 但保留字段方便 debug / 将来 actor 内 self-reference。
    #[allow(dead_code)]
    pub id: ConnectionId,
    pub host_id: HostId,
    /// 显示用，自动生成 `"<host.label> #N"`。N 从 1 开始按 host 内自增。
    pub label: String,
    pub opened_at: SystemTime,
}

/// 顶层 3-tab 导航当前选中项（M4a 信息架构；Inbox 在 2026-05-12 删除）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SidebarTab {
    #[default]
    Home,
    Terminal,
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
    /// 用户是否手动重命名过本 tab。false（默认）时 SshEvent::TitleChanged
    /// （远端 OSC 0/1/2 + alacritty Event::Title）会自动覆盖 title；
    /// true 时 OSC 被忽略，让用户的命名保留住（iTerm2 / WezTerm /
    /// Windows Terminal 一致行为）。
    pub title_locked: bool,
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

    /// M35 T8: 全局 Command Palette 开关。true = Ctrl+P / Cmd+P / Cmd+K
    /// 触发；palette 显示后用户 fuzzy search hosts，Enter 直接 open_connection。
    /// 通过 RootView handle_global_key 切换。
    pub pending_palette: bool,

    pub modal: Option<HostFormState>,
    pub last_connected: HashMap<HostId, SystemTime>,
    pub sidebar: SidebarTab,
    /// M35 T9: sidebar 是否展开（220px 含「最近连接」list）。
    /// false = 64px icon-only 折叠模式（v0.next 默认 / muscle memory），
    /// true = 220px 含完整 nav + 最近连接 list。
    /// 通过 sidebar 顶部 logo 区域点击 toggle，持久化到 app_state.toml。
    pub sidebar_expanded: bool,

    /// 每连接一个 actor 命令通道。
    pub sessions: HashMap<ConnectionId, mpsc::Sender<SessionCommand>>,
    /// 每连接一个 alacritty Term（保留 scrollback）。
    pub host_pty_term: HashMap<ConnectionId, Term<TitleListener>>,
    /// 每连接一个 ANSI parser。Processor 是 stateful（VTE parser 跨字节包
    /// 维护 escape sequence 解析进度），必须 per-conn 持久化。之前每次
    /// feed_bytes 都 Processor::new()，escape 跨 SSH frame 时会被错解析。
    pub host_pty_processor: HashMap<ConnectionId, AnsiProcessor<StdSyncHandler>>,
    /// 每连接一个 PTY 尺寸。
    pub host_pty_dimensions: HashMap<ConnectionId, (u16, u16)>,
    /// 每连接一个 tmux 状态（同一 host 的多个连接独立 list-sessions）。
    pub tmux_state: HashMap<ConnectionId, TmuxState>,
    /// 流式批量上传进度：(done, total)。`Some` 表示该 conn 当前有上传中，
    /// input_bar 据此显示"上传中 (done/total)"提示 + disable "发送"按钮防止
    /// 重复发送；`BatchDone` 事件清掉。
    pub pending_uploads: HashMap<ConnectionId, (usize, usize)>,
    /// 最近一次 batch 异常 abort 后留给 InputBar 的 retry 提示：
    /// (succeeded, total)。BatchAborted handler 写入，InputBar 在 last_uploading
    /// 边沿 true→false 时 read 决定是 drain 前 succeeded 张 + 保留剩余 + text，
    /// 还是走常规 cleanup；read 后 InputBar 调 `consume_last_aborted_batch` 清掉。
    pub last_aborted_batch: HashMap<ConnectionId, (usize, usize)>,
    /// 每连接生命周期阶段。register_session 设 Connecting，SshEvent::Connected
    /// 转 Connected，Disconnected/Error 转 Disconnected{reason}。terminal_view
    /// 根据本字段渲染 loading / reconnect overlay。
    pub connection_phases: HashMap<ConnectionId, ConnectionPhase>,
    /// SshEvent channel sender，由 app.rs 启动后注入。
    /// 用于 alacritty Term 创建时构造 TitleListener 把 OSC title event
    /// 推回 GPUI 主循环。`None` 时 listener 走 fallback 不发事件（测试 fixture
    /// 创建 AppState 时常见，无 listener 也能正常构造 Term）。
    pub event_tx: Option<mpsc::Sender<SshEvent>>,
    /// M28 T7：hosts.json load 失败时 error message。`None` = 加载成功。
    /// 启动时 `app.rs` 在 `load_hosts()` Err 路径 set；Home 据此渲染
    /// ErrorState + 重试 button 替代默认 hosts 列表。重试成功后清回 None。
    pub hosts_load_error: Option<String>,
}

impl Connection {
    /// 返回自 opened_at 到现在的 humanize 字符串，用于 Active Sessions 显示。
    ///
    /// M36 T3 起 home.rs 改用 `home_preview::format_active_duration`
    /// （中文 "5m active" / "12h active" / "2d active"），本 method 保留
    /// 供后续可能的过去时（"5m ago"）场景复用 + 自身 unit test 验证算法。
    #[allow(dead_code)]
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
            sidebar_expanded: false, // M35 T9: 默认折叠，保留 muscle memory
            pending_session_picker: None,
            pending_palette: false,
            sessions: HashMap::new(),
            modal: None,
            last_connected: HashMap::new(),
            host_pty_term: HashMap::new(),
            host_pty_processor: HashMap::new(),
            host_pty_dimensions: HashMap::new(),
            tmux_state: HashMap::new(),
            pending_uploads: HashMap::new(),
            last_aborted_batch: HashMap::new(),
            connection_phases: HashMap::new(),
            event_tx: None,
            hosts_load_error: None,
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

    /// 拖拽 reorder：把 source tab 移到 target tab 的位置。
    /// - source == target → noop 返回 false
    /// - 任一 id 找不到 → noop 返回 false
    ///
    /// 语义：source 占据 target 的当前位置，target 让出方向：
    /// - src < tgt（向右拖）：tabs.remove(src) 后 target 落到 (tgt_pos-1)，
    ///   insert(tgt_pos) 把 source 放 target 之后 → 与"向右拖"方向一致
    /// - src > tgt（向左拖）：tabs.remove(src) 后 target 还在 tgt_pos，
    ///   insert(tgt_pos) 把 source 放 target 之前 → 与"向左拖"方向一致
    ///
    /// 两个 case insert_pos 都 = tgt_pos（原 target 位置）—— 直观且与
    /// Chrome / VSCode tab drag 行为一致。
    pub fn move_tab(&mut self, source: TabId, target: TabId) -> bool {
        if source == target {
            return false;
        }
        let Some(src_pos) = self.tabs.iter().position(|t| t.id == source) else {
            return false;
        };
        let Some(tgt_pos) = self.tabs.iter().position(|t| t.id == target) else {
            return false;
        };
        let tab = self.tabs.remove(src_pos);
        let insert_pos = tgt_pos.min(self.tabs.len());
        self.tabs.insert(insert_pos, tab);
        true
    }

    /// 循环切换 tab。delta = +1 下一个、-1 上一个，到边界 wrap 到首/末。
    /// tabs 空 / 单个 / 无选中时 noop 返 false。
    pub fn cycle_selected_tab(&mut self, delta: i32) -> bool {
        if delta == 0 || self.tabs.len() < 2 {
            return false;
        }
        let Some(id) = self.selected_tab else {
            return false;
        };
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        let len = self.tabs.len() as i32;
        let new_pos = (pos as i32 + delta).rem_euclid(len) as usize;
        self.selected_tab = Some(self.tabs[new_pos].id);
        true
    }

    /// 在末尾追加一个 Default tab 并选中。caller 一般同时切 sidebar 到 Home
    /// 以触发 host picker。Ctrl+T / + 按钮等"新 tab"入口走此函数。
    pub fn append_default_tab(&mut self) -> TabId {
        let id = TabId::new();
        self.tabs.push(Tab {
            id,
            content: TabContent::Default,
            title: "新连接".into(),
            title_locked: false,
        });
        self.selected_tab = Some(id);
        id
    }

    /// 把当前选中的 tab 向左 (delta=-1) 或向右 (delta=+1) 移动一格。
    /// 已在边界 / 无选中时 noop。delta != ±1 也 noop（防御性）。
    /// 返回 true 表示真发生了交换，调用方可据此决定是否 cx.notify。
    pub fn move_selected_tab(&mut self, delta: i32) -> bool {
        if delta != -1 && delta != 1 {
            return false;
        }
        let Some(id) = self.selected_tab else {
            return false;
        };
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        let new_pos = pos as i32 + delta;
        if new_pos < 0 || (new_pos as usize) >= self.tabs.len() {
            return false;
        }
        self.tabs.swap(pos, new_pos as usize);
        true
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
        // 注册即"正在尝试连接"，等 SshEvent::Connected 才转 Connected
        self.connection_phases
            .insert(id, ConnectionPhase::Connecting);
    }

    /// 关闭一个连接：清 actor sender + tmux_state，保留 host_pty_term（用户可能想看 scrollback）。
    /// **phase 转 Disconnected{reason}**，让 UI 显示重连按钮（reason 由 caller 传入，
    /// app.rs 收 Disconnected/Error 事件时调本方法时传入用户可读的原因文字）。
    /// 完全清理（含 Term + Connection meta）走 [`remove_connection`]。
    pub fn drop_session(&mut self, id: ConnectionId, reason: impl Into<String>) {
        self.sessions.remove(&id);
        self.tmux_state.remove(&id);
        self.connection_phases.insert(
            id,
            ConnectionPhase::Disconnected {
                reason: reason.into(),
            },
        );
    }

    /// 标记连接已成功（收到 SshEvent::Connected 时调）。
    pub fn mark_connected(&mut self, id: ConnectionId) {
        self.connection_phases
            .insert(id, ConnectionPhase::Connected);
    }

    /// 重连：复用同一 ConnectionId，注册新 sender，phase 回到 Connecting。
    /// 调用方先 spawn 新 actor 拿到 sender 再调本方法。返回值是该 conn 的
    /// host_id（供 caller spawn actor 时拿配置用，可选 helper）。
    pub fn reopen_connection(
        &mut self,
        id: ConnectionId,
        sender: mpsc::Sender<SessionCommand>,
    ) -> Option<HostId> {
        let host_id = self.connections.get(&id).map(|c| c.host_id)?;
        self.sessions.insert(id, sender);
        self.tmux_state.remove(&id);
        self.connection_phases
            .insert(id, ConnectionPhase::Connecting);
        Some(host_id)
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
        self.connection_phases.remove(&id);
        self.pending_uploads.remove(&id);
        self.last_aborted_batch.remove(&id);
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

    /// detach-detect：清掉 tmux_state attached 标记。actor 在 channel data
    /// 内检测到 "[detached" 标记后 emit TmuxSessionDetached 触发本方法。
    /// 仅当当前 attached 与 emit 的 session 匹配时才清（防 race）。
    pub fn mark_tmux_detached(&mut self, conn: ConnectionId, session: SessionId) {
        if let Some(TmuxState::Detected { attached, .. }) = self.tmux_state.get_mut(&conn) {
            if attached.as_ref() == Some(&session) {
                *attached = None;
            }
        }
    }

    /// raw shell 模式下，feed bytes 到指定连接的 Term。
    pub fn feed_bytes(&mut self, conn: ConnectionId, bytes: &[u8]) {
        let (cols, rows) = self
            .host_pty_dimensions
            .get(&conn)
            .copied()
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        let tx_clone = self.event_tx.clone();
        let term = self
            .host_pty_term
            .entry(conn)
            .or_insert_with(|| make_term(conn, tx_clone, cols, rows));
        // Processor 跨 feed_bytes 持久化 — 让 ANSI escape 序列跨 SSH frame
        // 仍能正确解析（之前每次 new 会让 \x1b[3 / 1m 这种切包被当字面字符）。
        let processor = self.host_pty_processor.entry(conn).or_default();
        processor.advance(term, bytes);
    }

    /// 取指定连接的 Term（只读）。
    pub fn term_of(&self, conn: ConnectionId) -> Option<&Term<TitleListener>> {
        self.host_pty_term.get(&conn)
    }

    /// SshEvent::TitleChanged handler：找到 content=Connection(conn) 的 tab，
    /// 如果该 tab.title_locked == false，把 tab.title 覆盖为 title；locked 时
    /// 静默忽略（保留用户手动重命名）。返回是否实际更新（caller 据此 cx.notify）。
    pub fn set_tab_title_for_conn(&mut self, conn: ConnectionId, title: String) -> bool {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return false;
        }
        for tab in self.tabs.iter_mut() {
            if tab.content == TabContent::Connection(conn) && !tab.title_locked {
                if tab.title != trimmed {
                    tab.title = trimmed.to_string();
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// 用户双击 tab 改名后调：直接覆盖 title 并锁定（之后 OSC 不再覆盖）。
    pub fn rename_tab_locked(&mut self, tab_id: TabId, new_title: String) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.title = new_title;
            tab.title_locked = true;
        }
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
                passphrase: String::new(),
            },
            env_profile: None,
            capabilities: HostCapabilities::default(),
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
    fn hosts_load_error_default_is_none() {
        // M28 T7: new AppState 应该 hosts_load_error = None（加载成功）
        // app.rs Err 路径才 set Some(err)
        let state = AppState::with_hosts(vec![]);
        assert!(state.hosts_load_error.is_none());
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
        state.drop_session(conn, "test");
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
            title_locked: false,
        });
        state.selected_tab = Some(tab_id);
        state.remove_connection(conn);
        // 该 tab 应被关闭
        assert!(!state.tabs.iter().any(|t| t.id == tab_id));
    }

    #[test]
    fn cycle_selected_tab_wraps_around() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let ids: Vec<_> = (0..3).map(|_| TabId::new()).collect();
        for id in &ids {
            state.tabs.push(Tab {
                id: *id,
                content: TabContent::Default,
                title: "t".into(),
                title_locked: false,
            });
        }
        state.selected_tab = Some(ids[2]);
        // 最末 +1 → 首
        assert!(state.cycle_selected_tab(1));
        assert_eq!(state.selected_tab, Some(ids[0]));
        // 首 -1 → 末
        assert!(state.cycle_selected_tab(-1));
        assert_eq!(state.selected_tab, Some(ids[2]));
    }

    #[test]
    fn cycle_selected_tab_single_tab_noop() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let id = TabId::new();
        state.tabs.push(Tab {
            id,
            content: TabContent::Default,
            title: "t".into(),
            title_locked: false,
        });
        state.selected_tab = Some(id);
        assert!(!state.cycle_selected_tab(1));
        assert!(!state.cycle_selected_tab(-1));
    }

    #[test]
    fn append_default_tab_adds_and_selects() {
        let mut state = AppState::with_hosts(vec![]);
        let len_before = state.tabs.len();
        let id = state.append_default_tab();
        assert_eq!(state.tabs.len(), len_before + 1);
        assert_eq!(state.selected_tab, Some(id));
        assert!(matches!(
            state.tabs.last().unwrap().content,
            TabContent::Default
        ));
    }

    #[test]
    fn move_tab_drag_right() {
        // [A B C D]，把 A 拖到 C 上 → [B C A D]（A 占 C 原位，C 让左）
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let ids: Vec<_> = (0..4).map(|_| TabId::new()).collect();
        for (i, id) in ids.iter().enumerate() {
            state.tabs.push(Tab {
                id: *id,
                content: TabContent::Default,
                title: format!("{}", i),
                title_locked: false,
            });
        }
        assert!(state.move_tab(ids[0], ids[2]));
        assert_eq!(state.tabs[0].id, ids[1]);
        assert_eq!(state.tabs[1].id, ids[2]);
        assert_eq!(state.tabs[2].id, ids[0]);
        assert_eq!(state.tabs[3].id, ids[3]);
    }

    #[test]
    fn move_tab_drag_left() {
        // [A B C D]，把 C 拖到 B 上 → [A C B D]（C 占 B 原位，B 让右）
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let ids: Vec<_> = (0..4).map(|_| TabId::new()).collect();
        for (i, id) in ids.iter().enumerate() {
            state.tabs.push(Tab {
                id: *id,
                content: TabContent::Default,
                title: format!("{}", i),
                title_locked: false,
            });
        }
        assert!(state.move_tab(ids[2], ids[1]));
        assert_eq!(state.tabs[0].id, ids[0]);
        assert_eq!(state.tabs[1].id, ids[2]);
        assert_eq!(state.tabs[2].id, ids[1]);
        assert_eq!(state.tabs[3].id, ids[3]);
    }

    #[test]
    fn move_tab_same_id_noop() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let id = TabId::new();
        state.tabs.push(Tab {
            id,
            content: TabContent::Default,
            title: "x".into(),
            title_locked: false,
        });
        assert!(!state.move_tab(id, id));
        assert_eq!(state.tabs.len(), 1);
    }

    #[test]
    fn move_tab_nonexistent_noop() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let real = TabId::new();
        let ghost = TabId::new();
        state.tabs.push(Tab {
            id: real,
            content: TabContent::Default,
            title: "x".into(),
            title_locked: false,
        });
        assert!(!state.move_tab(ghost, real));
        assert!(!state.move_tab(real, ghost));
    }

    #[test]
    fn move_selected_tab_right_swaps_with_neighbor() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let id1 = TabId::new();
        let id2 = TabId::new();
        state.tabs.push(Tab {
            id: id1,
            content: TabContent::Default,
            title: "1".into(),
            title_locked: false,
        });
        state.tabs.push(Tab {
            id: id2,
            content: TabContent::Default,
            title: "2".into(),
            title_locked: false,
        });
        state.selected_tab = Some(id1);
        assert!(state.move_selected_tab(1));
        assert_eq!(state.tabs[0].id, id2);
        assert_eq!(state.tabs[1].id, id1);
        // selected_tab 应该跟着 id1 走（id 不变，pos 变了，但 selected_tab 是 id）
        assert_eq!(state.selected_tab, Some(id1));
    }

    #[test]
    fn move_selected_tab_at_boundary_noop() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let id1 = TabId::new();
        state.tabs.push(Tab {
            id: id1,
            content: TabContent::Default,
            title: "1".into(),
            title_locked: false,
        });
        state.selected_tab = Some(id1);
        // 只有一个 tab，左右都到边界
        assert!(!state.move_selected_tab(-1));
        assert!(!state.move_selected_tab(1));
    }

    #[test]
    fn move_selected_tab_no_selection_noop() {
        let mut state = AppState::with_hosts(vec![]);
        // selected_tab = None
        assert!(!state.move_selected_tab(1));
        assert!(!state.move_selected_tab(-1));
    }

    #[test]
    fn move_selected_tab_invalid_delta_noop() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        let id1 = TabId::new();
        let id2 = TabId::new();
        state.tabs.push(Tab {
            id: id1,
            content: TabContent::Default,
            title: "1".into(),
            title_locked: false,
        });
        state.tabs.push(Tab {
            id: id2,
            content: TabContent::Default,
            title: "2".into(),
            title_locked: false,
        });
        state.selected_tab = Some(id1);
        // delta != ±1 拒绝（防御性 —— 调用方应该只传 -1 或 +1）
        assert!(!state.move_selected_tab(0));
        assert!(!state.move_selected_tab(2));
        assert!(!state.move_selected_tab(-2));
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
            title_locked: false,
        });
        state.tabs.push(Tab {
            id: id2,
            content: TabContent::Default,
            title: "2".into(),
            title_locked: false,
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
            capabilities: HostCapabilities::default(),
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
        state.drop_session(conn, "test");
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
            title_locked: false,
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

    #[test]
    fn upload_batch_command_carries_images_and_text() {
        let cmd = SessionCommand::UploadBatch {
            images: vec![
                (vec![0u8, 1, 2], "png".into()),
                (vec![3u8, 4], "jpg".into()),
            ],
            text: "describe this".into(),
        };
        match cmd {
            SessionCommand::UploadBatch { images, text } => {
                assert_eq!(images.len(), 2);
                assert_eq!(images[0].1, "png");
                assert_eq!(text, "describe this");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn batch_progress_event_carries_done_total() {
        use aish_types::ConnectionId;
        let conn = ConnectionId::new();
        let event = SshEvent::BatchProgress {
            conn,
            done: 2,
            total: 5,
        };
        match event {
            SshEvent::BatchProgress { done, total, .. } => {
                assert_eq!(done, 2);
                assert_eq!(total, 5);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn batch_done_event_carries_text() {
        use aish_types::ConnectionId;
        let conn = ConnectionId::new();
        let event = SshEvent::BatchDone {
            conn,
            text: "hello world".into(),
        };
        match event {
            SshEvent::BatchDone { text, .. } => {
                assert_eq!(text, "hello world");
            }
            _ => panic!("wrong variant"),
        }
    }
}
