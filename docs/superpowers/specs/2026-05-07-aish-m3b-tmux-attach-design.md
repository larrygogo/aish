# aish M3b — tmux session 列表 + attach + 三栏 GUI 设计

> Status: Spec | Drafted 2026-05-07

## 0. 背景

M2 完成 SSH + alacritty 终端 + host CRUD + 密码登录；M3a 完成 aish-tmux 协议状态机
（pure state machine，无 IO）。M3b 把状态机接到 ssh_actor，同时引入三栏 GUI
让 user 看到远端 tmux 现有 sessions，选择 attach 或跳过。

不强制 tmux —— user 能在 GUI 看到远端有什么 sessions，自主决定是否进入 tmux 模式；
没 tmux 时静默降级 raw shell（M2 行为），中间栏明示提示。

## 1. 目标 / 非目标

### 目标
- 三栏 GUI：host_list (220px) | tmux 中间栏 (200px) | terminal (flex)
- 连上 host 后 background 跑 `tmux list-sessions` 拿现有 sessions
- 中间栏按 6 状态展示：查询中 / 未检测到 tmux / 检测失败 / session 列表 / 连接中 / 已 attach（SessionTree 树）
- user 点 session → 关 raw shell channel → 开新 channel 跑 `tmux -CC attach -t <n>`
- TmuxController 启动 + feed bytes，SessionTree 同步到 AppState
- per-pane alacritty Term，terminal 显示 active pane（active = BTreeMap first 链）
- 手动 ↻ 刷新 list-sessions

### 非目标（M3b 不做 — 留 M3c）
- detach 按钮（attach 后无法回 raw shell）
- 多 window / pane 点击切换
- 新建 session（中间栏「+ new」按钮 stub disabled，点了 toast「M3c 实现」）
- 重连恢复 SessionTree（断开后丢，重连重新走 list-sessions）
- active pane 的 control mode 跟踪（仅用 BTreeMap first 简化）
- tmux 版本检测 / 老版本兼容（< 2.6 用户行为：tmux -CC 报错 → QueryFailed 状态）

## 2. 架构概述

```
GUI (RootView)
  HostList 220px | TmuxSidebar 200px | TerminalView flex
                                          shows active pane Term

AppState
  - hosts / selected / sessions (CommandSender)
  - tmux_state: HashMap<HostId, TmuxState>
  - pane_terminals: HashMap<(HostId, PaneId), Term>
  - host_pty_term: HashMap<HostId, Term>  (raw shell fallback)

ssh_actor::host_session_task
  raw PTY chan shell loop  ||  tmux query task (list-sessions)
                                       (concurrent)
  on user pick session:
    close raw shell chan
    open new chan running: tmux -CC attach -t <name>
    start TmuxController
    feed_bytes loop -> TmuxEvent
       PaneOutput -> per-pane Term
       others    -> sync SessionTree
```

## 3. 详细设计

### 3.1 数据模型

#### RemoteSession（新加 aish-types）
```rust
/// list-sessions 输出的单条 session 信息（远端纯展示用，不含 windows/panes）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSession {
    pub id: SessionId,    // "$0"
    pub name: String,     // "dev"
}
```

#### TmuxState（新加 aish-app/src/state.rs）
```rust
/// 单个 host 的 tmux 状态。每次连接重置，断开清空。
#[derive(Debug, Clone)]
pub enum TmuxState {
    /// 刚连上，list-sessions 还没跑（瞬态）
    NotChecked,
    /// 远端没装 tmux（exec 返回 "command not found" 或非 0 exit code 且 stderr 含相关字样）
    NoTmux,
    /// list-sessions 成功
    Detected { sessions: Vec<RemoteSession> },
    /// list-sessions 失败但远端有 tmux（其他错误）
    QueryFailed { msg: String },
    /// user 点了某 session，正在 attach（瞬态）
    Attaching { session: SessionId },
    /// 已 attach，TmuxController 在 actor 里运行，SessionTree 同步过来
    Attached { session_tree: aish_tmux::SessionTree },
}
```

#### AppState 字段调整
```rust
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
    pub modal: Option<HostFormState>,

    // M2b1 字段，重命名（区分 raw shell vs per-pane）
    pub host_pty_term: HashMap<HostId, Term<VoidListener>>,
    pub host_pty_dimensions: HashMap<HostId, (u16, u16)>,

    // M3b 新增
    pub tmux_state: HashMap<HostId, TmuxState>,
    pub pane_terminals: HashMap<(HostId, PaneId), Term<VoidListener>>,
    pub pane_dimensions: HashMap<(HostId, PaneId), (u16, u16)>,
}
```

#### SessionCommand 扩展
```rust
pub enum SessionCommand {
    SendBytes(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Disconnect,
    // M3b 新增
    QueryTmuxSessions,
    AttachTmux { session: aish_types::SessionId },
}
```

#### SshEvent 扩展
```rust
pub enum SshEvent {
    Connected { host: HostId },
    PaneOutput { host: HostId, bytes: Vec<u8> },         // raw shell 模式
    Disconnected { host: HostId, reason: DisconnectReason },
    Error { host: HostId, kind: SshErrorKind, msg: String },
    // M3b 新增
    TmuxQueryStarted { host: HostId },
    TmuxSessionsListed { host: HostId, sessions: Vec<RemoteSession> },
    TmuxQueryFailed { host: HostId, msg: String },
    TmuxNoTmux { host: HostId },
    TmuxAttaching { host: HostId, session: aish_types::SessionId },
    TmuxAttached { host: HostId },
    TmuxSessionTreeUpdated { host: HostId, tree: aish_tmux::SessionTree },
    TmuxPaneOutput { host: HostId, pane: aish_types::PaneId, bytes: bytes::Bytes },
    TmuxDetached { host: HostId, reason: String },
}
```

### 3.2 ssh_actor 状态机改造

`host_session_task` 主循环逻辑：

```
state: enum ActorMode { RawShell { chan }, TmuxAttached { chan, controller } }

connect -> open raw PTY chan -> request_pty -> shell
mode = RawShell { chan }
emit Connected
spawn tmux_query_task(host, session_handle)

main loop:
    select! {
        msg = current_chan.wait() => match (mode, msg) {
            (RawShell, Data) => emit PaneOutput
            (TmuxAttached(_, ctrl), Data) => {
                let events = ctrl.feed_bytes(&data);
                for ev in events {
                    PaneOutput -> emit TmuxPaneOutput
                    others -> mark tree dirty
                }
                if dirty: emit TmuxSessionTreeUpdated(ctrl.session_tree().clone())
            }
            (_, Eof|None) => emit Disconnected, break
        }
        cmd = cmd_rx.recv() => match cmd {
            SendBytes(b) => current_chan.data(b)
            Resize(c,r) => current_chan.window_change(c,r)
            QueryTmuxSessions => spawn tmux_query_task(...)
            AttachTmux { session } => {
                emit TmuxAttaching
                close current_chan
                let new_chan = open new chan
                let cmd = format!("tmux -CC attach -t {}", shell_escape(session))
                new_chan.exec(cmd)
                let controller = TmuxController::new()
                mode = TmuxAttached { chan: new_chan, controller }
                emit TmuxAttached
            }
            Disconnect | None => break
        }
    }
```

### 3.3 tmux_query_task

独立 async fn，用现有 SshSession 开 exec channel 跑 list-sessions：

```rust
async fn tmux_query_task(
    host: HostId,
    session_handle: Arc<SshClient>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    let _ = event_tx.send(SshEvent::TmuxQueryStarted { host }).await;
    let result = session_handle
        .exec_command("tmux list-sessions -F '#{session_id}|#{session_name}'")
        .await;
    match result {
        Ok(r) if r.exit_code == 0 => {
            let sessions = parse_session_list(&r.stdout);
            let _ = event_tx.send(SshEvent::TmuxSessionsListed { host, sessions }).await;
        }
        Ok(r) => {
            let s = String::from_utf8_lossy(&r.stderr).to_string();
            if s.contains("command not found") || s.contains("not found") {
                let _ = event_tx.send(SshEvent::TmuxNoTmux { host }).await;
            } else if s.contains("no server running") || s.contains("no sessions") {
                let _ = event_tx.send(SshEvent::TmuxSessionsListed {
                    host, sessions: vec![],
                }).await;
            } else {
                let _ = event_tx.send(SshEvent::TmuxQueryFailed { host, msg: s }).await;
            }
        }
        Err(e) => {
            let _ = event_tx.send(SshEvent::TmuxQueryFailed {
                host, msg: e.to_string(),
            }).await;
        }
    }
}

fn parse_session_list(stdout: &[u8]) -> Vec<RemoteSession> {
    let s = String::from_utf8_lossy(stdout);
    s.lines().filter_map(|line| {
        let mut parts = line.splitn(2, '|');
        let id = parts.next()?;
        let name = parts.next()?;
        Some(RemoteSession {
            id: SessionId::new(id.trim()),
            name: name.trim().to_string(),
        })
    }).collect()
}
```

注：`SshClient::exec_command` 是新需要在 aish-ssh 添加的简化 API（封装 channel_open + exec + 收 stdout/stderr/exit-code）。

### 3.4 aish-ssh 新增 exec_command

```rust
impl SshClient {
    /// 跑一条远端命令并等其完成。封装 channel_open_session + exec + 收输出。
    /// 用于 list-sessions 等短命令；不适合长跑（用 open_channel + shell）。
    pub async fn exec_command(&self, command: &str) -> Result<ExecResult, SshError> {
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code: Option<u32> = None;

        while let Some(msg) = channel.wait().await {
            use russh::ChannelMsg::*;
            match msg {
                Data { data } => stdout.extend_from_slice(&data),
                ExtendedData { data, ext: 1 } => stderr.extend_from_slice(&data),
                ExitStatus { exit_status } => exit_code = Some(exit_status),
                Eof => {}
                Close => break,
                _ => {}
            }
        }

        Ok(ExecResult {
            stdout, stderr,
            exit_code: exit_code.unwrap_or(255),
        })
    }
}

pub struct ExecResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: u32,
}
```

### 3.5 TmuxSidebarView (GUI 中间栏)

```rust
// crates/aish-app/src/views/tmux_sidebar.rs (新)
pub struct TmuxSidebarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: mpsc::Sender<SshEvent>,
}

impl Render for TmuxSidebarView {
    fn render(&mut self, _w, cx) -> impl IntoElement {
        let app = self.state.read(cx);
        let host = match app.selected {
            Some(h) => h,
            None => return self.empty_view(),
        };
        match app.tmux_state.get(&host) {
            None | Some(TmuxState::NotChecked) => self.spinner_view("查询 tmux 中…"),
            Some(TmuxState::NoTmux) => self.notmux_view(host),
            Some(TmuxState::QueryFailed { msg }) => self.query_failed_view(host, msg),
            Some(TmuxState::Detected { sessions }) => self.session_list_view(host, sessions),
            Some(TmuxState::Attaching { session }) => self.attaching_view(session),
            Some(TmuxState::Attached { session_tree }) => self.session_tree_view(session_tree),
        }
    }
}
```

session 列表项 on_mouse_down 通过 `app.sessions[host].send(SessionCommand::AttachTmux { session })`
触发 actor 切换 mode。session tree view 显示嵌套：`session.name -> window.name -> pane.id`，
全部只读（无 on_click handler — M3c 加）。

### 3.6 TerminalView 调整

```rust
fn term_for_render(app: &AppState, host: HostId) -> Option<&Term> {
    match app.tmux_state.get(&host) {
        Some(TmuxState::Attached { session_tree }) => {
            let session = session_tree.sessions.values().next()?;
            let window = session.windows.values().next()?;
            let pane = window.panes.first()?;
            app.pane_terminals.get(&(host, *pane))
        }
        _ => app.host_pty_term.get(&host),
    }
}
```

注：`Window.panes: Vec<PaneId>`（types.rs 已有）— `.first()` 取第一个。

### 3.7 TmuxState 状态转换

```
[host 未选/未连]          [host 选了，连上后]
   (no entry)              spawn tmux_query_task
                                   v
                              NotChecked
                                   v
                       Detected | NoTmux | QueryFailed
                                   v
                        user 点 session -> AttachTmux cmd
                                   v
                              Attaching
                                   v
                          Attached { tree: Default }
                                   v (each event)
                          Attached { tree: 更新 }

connection drop / Disconnect -> tmux_state.remove(host)
```

## 4. ADR

### ADR-1: 不强制启 tmux，让 user 选
- **背景**：M3a 设计时假设 attach 默认 session（`tmux -CC new-session -A -s aish-default`）
- **决策**：连上后**不**自动启 tmux；中间栏展示远端现有 sessions 给 user 选
- **理由**：不强 tmux 用户更友好；user 看到 "我的 vps 有什么 sessions" 比 "强行被塞 aish-default" 更尊重 user 意图；与 iTerm2 / Termius 等成熟客户端的 tmux integration UX 一致
- **替代**：强制启 → 简单但 invasive
- **影响**：增加 list-sessions 步骤；中间栏需要 5 状态机；UX 复杂度上升

### ADR-2: list-sessions 用独立 SSH exec channel
- **背景**：怎么不影响 raw PTY shell 拿到 sessions 列表？
- **决策**：独立 channel exec `tmux list-sessions -F`，并发 raw shell loop
- **理由**：单 SSH 连接多 channel 是 SSH 协议标准支持；不污染 raw shell；完成后 channel 自然关闭
- **替代**：在 raw shell 里 echo 命令拿输出 — 污染 user 终端
- **影响**：需要 aish-ssh 加 exec_command API（thin wrapper）

### ADR-3: per-pane alacritty Term
- **背景**：M2b1 是 per-host 单 Term；tmux 多 pane 输出怎么不混乱
- **决策**：`pane_terminals: HashMap<(HostId, PaneId), Term>`，每 pane 独立 Term
- **理由**：多 pane 输出 feed 同一 Term 会破坏 VT100 状态机；per-pane forward-compatible，M3c 加点击切 pane 只需改 active 选择
- **替代**：单 Term 只 feed active pane → M3c 必须重写
- **影响**：AppState 结构调整；TerminalView 取 Term 逻辑改为按 active pane 路由

### ADR-4: active pane = BTreeMap first 链
- **背景**：M3b 不实现 click 切 pane；显示哪个 pane？
- **决策**：取 SessionTree 第一个 session 的第一个 window 的 panes[0]
- **理由**：简单，0 协议改动；大多数 attach 场景 user 的 session 只有 1 window 1 pane
- **替代**：扩 SessionTree 加 active_pane 字段 + 协议事件维护 → 30 行改动 + 测试，超出 M3b 范围
- **影响**：M3c 必须做 active 跟踪

### ADR-5: 不做 detach（attach 后无返回 raw shell）
- **背景**：user 进入 tmux 模式后能否回 raw shell？
- **决策**：M3b **不**做 detach；attach 后只能 Disconnect 整个 host 重连
- **理由**：YAGNI；detach 涉及 channel 切换 + Term 状态保留 / 切换的复杂度，M3c 一并做
- **替代**：M3b 加 detach 按钮 — 估增 50 行 + 状态保留逻辑
- **影响**：M3b user 想退出 tmux 必须 Disconnect

### ADR-6: + new session 按钮 stub
- **背景**：UI 显示「+ new」按钮但 M3b 不实现新建 session
- **决策**：按钮显示但点击仅 toast「M3c 实现」
- **理由**：UI 对未来可见性；不增加实现成本
- **替代**：完全不显示 — 用户不知道未来能新建
- **影响**：toast 通知组件需要存在（GPUI 没内置 — 用 modal-style div）

### ADR-7: SessionTree 通过 SshEvent clone 推到 GUI
- **背景**：TmuxController 在 actor task 内，SessionTree 在 controller 里。GUI 怎么读？
- **决策**：每次 feed_bytes 后，actor 把 `controller.session_tree().clone()` 通过 `TmuxSessionTreeUpdated` event 推给 AppState
- **理由**：单向数据流（actor -> AppState -> GUI）符合 GPUI 模型；SessionTree 是 BTreeMap 嵌套，clone 成本可接受（M3b 单 host 单 session）
- **替代**：SessionTree 用 Arc<Mutex<>> 共享 → 跨线程锁 + GPUI 上下文复杂
- **影响**：每次 tmux 事件都 clone SessionTree，性能 M3b 完全够用

### ADR-8: TmuxState 六态枚举
- **背景**：tmux 检测状态和 attach 状态分两个枚举还是合一？
- **决策**：合一 enum TmuxState 6 个 variant
- **理由**：GUI 中间栏渲染本质是 match 一种状态；互斥（不可能既 NoTmux 又 Attaching）
- **替代**：detection: enum + attach: enum 两字段 — 需要在渲染时合并 match
- **影响**：TmuxState 状态转换图清晰

### ADR-9: TmuxNoTmux 通过 stderr 字符串模糊匹配
- **背景**：tmux 不存在的判定逻辑
- **决策**：list-sessions 失败 + stderr 含 "command not found" 或 "not found" → NoTmux；含 "no server running" / "no sessions" → Detected (空)；其他 → QueryFailed
- **理由**：跨 shell 的 "command not found" 字样比较稳定（bash/zsh/fish 都有）
- **风险**：非英文 locale 可能不匹配 — fallback 到 QueryFailed，user 仍能看到原始 stderr
- **影响**：parse_session_list 测试要覆盖 4 种 stderr 模式

## 5. 测试策略

### 单元测试

**aish-types**:
- `RemoteSession` derive 测试

**aish-ssh** (难真测，简化):
- `exec_command` 走 mock channel？russh 内部不易 mock。**M3b 不写 exec_command 单测**，靠端到端验证

**aish-app**:
- `state.rs`: TmuxState 状态转换 + tmux_state hash map 操作
  - tmux_state 从 NotChecked -> Detected -> Attaching -> Attached 链
  - tmux_state.remove(host) on Disconnect
- `ssh_actor.rs`: parse_session_list 单测（4 种 stderr 模式）
  - stdout 含 "$0|dev\n$1|work\n" -> 2 个 RemoteSession
  - stdout 空 + exit 0 -> 0 个 RemoteSession
  - stdout 含 BOM / trailing whitespace -> 正确 trim
- `terminal_view.rs::term_for_render`: BTreeMap first 链选择
  - tmux_state Attached 取 pane_terminals[(host, first_pane)]
  - 其他状态取 host_pty_term[host]
  - SessionTree 空 panes vec -> 返回 None

### 集成测试
- 不在 CI 跑（涉及真 SSH + tmux）
- 手测清单：

1. 连 host (没 tmux) -> 中间栏「未检测到 tmux · ↻」
2. 连 host (有 tmux 没 session) -> 中间栏「+ new」+ ↻ + 「无 session」
3. 连 host (有 sessions) -> 中间栏列出 ● ○ session names
4. 点 session attach -> 中间栏 spinner -> 接着展开 SessionTree
5. terminal 显示 active pane 输出（应当能 ls 拿到远端 file list）
6. 点 ↻ 刷新 -> 中间栏重新跑 list-sessions
7. Disconnect -> 中间栏返回「未选择 host」（tmux_state 清空）

## 6. 兼容性 / 迁移

### 数据
- hosts.json 无 schema 变化
- AppState 字段重命名：`pane_terminals: HashMap<HostId, Term>` -> `host_pty_term: HashMap<HostId, Term>`
  - 仅运行时数据，无持久化影响
- 新加 `pane_terminals: HashMap<(HostId, PaneId), Term>`

### API
- `aish-ssh` 加 `SshClient::exec_command` — 新公开 API，向后兼容
- `aish-types` 加 `RemoteSession` — 新类型
- `aish-app::state::SessionCommand` enum 加 2 variant，调用方需要扩展 match
- `aish-app::state::SshEvent` enum 加 8 variant，调用方需要扩展 match

### 用户体验
- M2 user 不感知变化（连 host 后中间栏 NoTmux 默认 raw shell）
- 装了 tmux 的 host 立即能在中间栏看到列表

## 7. 待 plan 阶段细化

- TmuxSidebarView 子视图（spinner / list / tree / status）每个 div 的精确样式
- toast 通知 GPUI 组件实现（new session stub 用）
- ssh_actor 主循环 select! 在切换 mode 时（RawShell -> TmuxAttached）的 channel 拥有权交接
- 各 task 文件 / 函数 / 测试 case 列表
- 验证步骤手测 checklist