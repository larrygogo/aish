# aish M3b — tmux session 列表 + attach + 三栏 GUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 M3a 的 aish-tmux 协议状态机接到 ssh_actor，加三栏 GUI 让 user 看到远端 tmux sessions 并选择 attach（不强制 tmux）。

**Architecture:** ssh_actor 启动后并发跑 list-sessions 命令；中间栏按 6 状态展示（NotChecked/NoTmux/Detected/QueryFailed/Attaching/Attached）；user 点 session 触发 actor 切换 mode（RawShell -> TmuxAttached）— 关 raw PTY channel，开新 channel 跑 tmux -CC attach，TmuxController feed_bytes 派 events，per-pane Term 维护。

**Tech Stack:** russh exec channel, aish-tmux state machine, GPUI 三栏 layout, alacritty per-pane Term。

**主线分支：** main（直接 push，不开 PR）。

---

## File Structure

| 文件 | 责任 |
|---|---|
| crates/aish-types/src/lib.rs | 加 RemoteSession |
| crates/aish-ssh/src/client.rs | SshClient derive Clone + exec_command + ExecResult |
| crates/aish-app/src/state.rs | TmuxState enum + AppState 字段重命名/扩展 + SessionCommand/SshEvent 扩展 |
| crates/aish-app/src/ssh_actor.rs | parse_session_list + tmux_query_task + host_session_task 改造（ActorMode）|
| crates/aish-app/src/views/tmux_sidebar.rs (NEW) | TmuxSidebarView 6 子视图 |
| crates/aish-app/src/views/mod.rs | 注册 tmux_sidebar |
| crates/aish-app/src/views/terminal_view.rs | term_for_render 路由 |
| crates/aish-app/src/app.rs | RootView 三栏 + 8 个新 SshEvent 处理 |

---
## Task 1: aish-types — 加 RemoteSession

**Files:**
- Modify: `crates/aish-types/src/lib.rs`

- [ ] **Step 1: 在 SshAuth 之前加 RemoteSession struct + 1 个测试**

在 `crates/aish-types/src/lib.rs` 的 `/// SSH 认证方式` 这一行之前插入：

```rust
/// 远端 tmux list-sessions 输出的单条 session 信息（纯展示用，不含 windows/panes）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSession {
    pub id: SessionId,
    pub name: String,
}

```

在 `crates/aish-types/src/lib.rs` 的 tests mod 末尾加：

```rust
    #[test]
    fn remote_session_basic() {
        let s = RemoteSession {
            id: SessionId::new("$0"),
            name: "dev".into(),
        };
        assert_eq!(s.id.as_str(), "$0");
        assert_eq!(s.name, "dev");
    }
```

- [ ] **Step 2: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo test -p aish-types 2>&1 | tail -10
cargo clippy -p aish-types --all-targets -- -D warnings 2>&1 | tail -3
```

期望：aish-types 测试 13 passed (12 + 1)。clippy 全绿。

- [ ] **Step 3: commit**

```bash
cargo fmt --all
git add crates/aish-types/
git commit -m "feat(aish-types): 加 RemoteSession 远端 tmux 会话信息类型"
```

---

## Task 2: aish-ssh — SshClient derive Clone + exec_command

**Files:**
- Modify: `crates/aish-ssh/src/client.rs`
- Modify: `crates/aish-ssh/src/lib.rs`（如果未 reexport ExecResult）

- [ ] **Step 1: 给 SshClient derive Clone + 加 ExecResult struct + exec_command 方法**

在 `crates/aish-ssh/src/client.rs` 找到 `pub struct SshClient {` 块，整体替换为：

```rust
pub struct SshClient {
    handle: Handle<NoopHandler>,
}

impl Clone for SshClient {
    fn clone(&self) -> Self {
        // russh::client::Handle 内部是 Arc，clone 是引用计数克隆，共享底层连接
        Self { handle: self.handle.clone() }
    }
}

/// 远端命令执行结果。
#[derive(Debug)]
pub struct ExecResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: u32,
}
```

在 `impl SshClient` 块里（在 `pub async fn close(self)` 之前）加：

```rust
    /// 跑一条远端命令并等其完成。封装 channel_open + exec + 收 stdout/stderr/exit-code。
    /// 用于 tmux list-sessions 等短命令；不适合长跑（用 open_channel + shell）。
    pub async fn exec_command(&self, command: &str) -> Result<ExecResult, SshError> {
        use russh::ChannelMsg;
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(SshError::Protocol)?;
        channel
            .exec(true, command)
            .await
            .map_err(SshError::Protocol)?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code: Option<u32> = None;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, ext } if ext == 1 => {
                    stderr.extend_from_slice(&data)
                }
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                ChannelMsg::Eof => {}
                ChannelMsg::Close => break,
                _ => {}
            }
        }

        Ok(ExecResult {
            stdout,
            stderr,
            exit_code: exit_code.unwrap_or(255),
        })
    }
```

- [ ] **Step 2: 在 lib.rs 加 reexport**

打开 `crates/aish-ssh/src/lib.rs` 找到 `pub use client::SshClient;`（或类似行）改为：

```rust
pub use client::{ExecResult, SshClient};
```

- [ ] **Step 3: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-ssh 2>&1 | tail -5
cargo test -p aish-ssh 2>&1 | tail -10
cargo clippy -p aish-ssh --all-targets -- -D warnings 2>&1 | tail -3
```

期望：build PASS，原有 14 测试不破，clippy 全绿。

注：exec_command 不写单元测试（spec ADR 已说明 russh channel 不易 mock；靠端到端验证）。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-ssh/
git commit -m "feat(aish-ssh): SshClient derive Clone + exec_command + ExecResult"
```

---
## Task 3: state.rs — TmuxState/AppState 字段扩展 + SessionCommand/SshEvent 扩展

**Files:**
- Modify: `crates/aish-app/src/state.rs`
- Modify: `crates/aish-app/Cargo.toml`（如果缺 bytes / aish-tmux dep）

- [ ] **Step 1: 加 imports + TmuxState enum**

在 `crates/aish-app/src/state.rs` 顶部 use 段加（如果没有）：

```rust
use aish_types::{HostConfig, HostId, PaneId, RemoteSession, SessionId};
use aish_tmux::SessionTree;
```

在 `pub enum HostFormState {` 之前插入：

```rust
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
```

- [ ] **Step 2: 改 AppState 字段（重命名 + 加新字段）**

替换 `pub struct AppState { ... }` 为：

```rust
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
```

- [ ] **Step 3: 改 with_hosts 初始化所有新字段**

```rust
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
```

- [ ] **Step 4: 改现有方法用新字段名（pane_terminals -> host_pty_term，pane_dimensions -> host_pty_dimensions）**

完整改写：

```rust
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
```

- [ ] **Step 5: 改 remove_host 清理新字段**

```rust
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
```

- [ ] **Step 6: 加 apply_tmux_pane_output / apply_tmux_session_tree 新方法**

在 `impl AppState` 块里（resize_term 之后）加：

```rust
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
        self.tmux_state.insert(host, TmuxState::Attached { session_tree: tree });
    }
```

- [ ] **Step 7: 扩展 SessionCommand enum**

替换 `pub enum SessionCommand { ... }` 为：

```rust
/// 从 GPUI 发到 actor task 的命令。
#[derive(Debug)]
pub enum SessionCommand {
    SendBytes(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Disconnect,
    /// 触发 ssh_actor 跑 tmux list-sessions（独立 channel exec）
    QueryTmuxSessions,
    /// user 点了某个 session，actor 关 raw shell -> 开新 channel attach
    AttachTmux { session: SessionId },
}
```

- [ ] **Step 8: 扩展 SshEvent enum**

替换 `pub enum SshEvent { ... }` 为：

```rust
/// 从 SSH actor task 推回 GPUI 的事件。
#[derive(Debug)]
pub enum SshEvent {
    Connected { host: HostId },
    PaneOutput { host: HostId, bytes: Vec<u8> },
    Disconnected { host: HostId, reason: DisconnectReason },
    Error { host: HostId, kind: SshErrorKind, msg: String },
    /// list-sessions 开始
    TmuxQueryStarted { host: HostId },
    /// list-sessions 成功（包括 tmux 在但 0 session 的情况）
    TmuxSessionsListed { host: HostId, sessions: Vec<RemoteSession> },
    /// list-sessions 失败但远端有 tmux
    TmuxQueryFailed { host: HostId, msg: String },
    /// 远端没 tmux
    TmuxNoTmux { host: HostId },
    /// AttachTmux 命令收到，actor 正在切 mode
    TmuxAttaching { host: HostId, session: SessionId },
    /// -CC channel 已开 + TmuxController 已建
    TmuxAttached { host: HostId },
    /// SessionTree 有变化
    TmuxSessionTreeUpdated { host: HostId, tree: SessionTree },
    /// tmux 模式下某 pane 的输出
    TmuxPaneOutput { host: HostId, pane: PaneId, bytes: bytes::Bytes },
    /// -CC channel 关闭
    TmuxDetached { host: HostId, reason: String },
}
```

- [ ] **Step 9: 确保 Cargo.toml 有 bytes / aish-tmux dep**

打开 `crates/aish-app/Cargo.toml` 确认 `[dependencies]` 段已有 `aish-tmux = { workspace = true }`（M3a 已加）和 `bytes = { workspace = true }`。如果 bytes 没有，加：

```toml
bytes = { workspace = true }
```

- [ ] **Step 10: 加 / 改测试**

替换 `feed_bytes_creates_term_on_demand` / `feed_bytes_reflects_in_term_grid` 改用 host_pty_term：

```rust
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
```

替换 `register_session_inits_dimensions` / `drop_session_keeps_terminal` / `resize_updates_dimensions` / `remove_host_clears_related_state` 也用新字段名（`host_pty_term` / `host_pty_dimensions`）：

```rust
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
```

加新测试到 tests mod 末尾：

```rust
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
```

注意 `mk_host` helper 必须返回 KeyFile auth (M2c+M2d 既有兼容)，PaneId 在 aish_types — 确认 tests mod 里 use 加 `aish_types::PaneId`。

- [ ] **Step 11: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-app 2>&1 | tail -10
cargo test -p aish-app --lib state 2>&1 | tail -25
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

期望：build 可能因 ssh_actor.rs / app.rs / terminal_view.rs 引用旧字段名报错 — **这是 Task 4-9 修，预期失败**。state 测试 ≥ 24 passed（原 21 + 新 3 + 改 5）。

如果 build 失败，可单独跑 state 测试：`cargo test -p aish-app --lib state`

- [ ] **Step 12: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/state.rs crates/aish-app/Cargo.toml
git commit -m "feat(aish-app): TmuxState + AppState 字段扩展 + SessionCommand/SshEvent 扩展"
```

---
## Task 4: ssh_actor — parse_session_list helper + tmux_query_task

**Files:**
- Modify: `crates/aish-app/src/ssh_actor.rs`

- [ ] **Step 1: 加 parse_session_list 辅助函数 + tmux_query_task fn**

在 `crates/aish-app/src/ssh_actor.rs` 顶部 use 段加：

```rust
use aish_types::RemoteSession;
```

在文件末尾（tests mod 之前）加：

```rust
/// 解析 tmux list-sessions -F '#{session_id}|#{session_name}' 的 stdout。
fn parse_session_list(stdout: &[u8]) -> Vec<RemoteSession> {
    let s = String::from_utf8_lossy(stdout);
    s.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '|');
            let id = parts.next()?;
            let name = parts.next()?;
            let id_trimmed = id.trim();
            let name_trimmed = name.trim();
            if id_trimmed.is_empty() {
                return None;
            }
            Some(RemoteSession {
                id: aish_types::SessionId::new(id_trimmed),
                name: name_trimmed.to_string(),
            })
        })
        .collect()
}

/// 在独立 SSH exec channel 跑 tmux list-sessions，结果通过 SshEvent 推回。
async fn tmux_query_task(
    host: HostId,
    client: aish_ssh::SshClient,
    event_tx: mpsc::Sender<SshEvent>,
) {
    let _ = event_tx.send(SshEvent::TmuxQueryStarted { host }).await;
    let result = client
        .exec_command("tmux list-sessions -F '#{session_id}|#{session_name}'")
        .await;
    match result {
        Ok(r) if r.exit_code == 0 => {
            let sessions = parse_session_list(&r.stdout);
            let _ = event_tx
                .send(SshEvent::TmuxSessionsListed { host, sessions })
                .await;
        }
        Ok(r) => {
            let s = String::from_utf8_lossy(&r.stderr).to_string();
            if s.contains("command not found") || s.contains("not found") {
                let _ = event_tx.send(SshEvent::TmuxNoTmux { host }).await;
            } else if s.contains("no server running") || s.contains("no sessions") {
                let _ = event_tx
                    .send(SshEvent::TmuxSessionsListed {
                        host,
                        sessions: vec![],
                    })
                    .await;
            } else {
                let trimmed = s.trim();
                let msg = if trimmed.is_empty() {
                    format!("tmux list-sessions exit {}", r.exit_code)
                } else {
                    trimmed.to_string()
                };
                let _ = event_tx
                    .send(SshEvent::TmuxQueryFailed { host, msg })
                    .await;
            }
        }
        Err(e) => {
            let _ = event_tx
                .send(SshEvent::TmuxQueryFailed {
                    host,
                    msg: e.to_string(),
                })
                .await;
        }
    }
}
```

- [ ] **Step 2: 在 ssh_actor tests mod 顶部加 parse_session_list 测试**

在 tests mod 里（紧跟 `use super::*;` 之后）插入：

```rust
    #[test]
    fn parse_session_list_basic() {
        let s = b"$0|dev\n$1|work\n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id.as_str(), "$0");
        assert_eq!(result[0].name, "dev");
        assert_eq!(result[1].id.as_str(), "$1");
        assert_eq!(result[1].name, "work");
    }

    #[test]
    fn parse_session_list_empty_stdout() {
        let result = parse_session_list(b"");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_session_list_trims_whitespace() {
        let s = b"  $0  |  dev with spaces  \n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.as_str(), "$0");
        assert_eq!(result[0].name, "dev with spaces");
    }

    #[test]
    fn parse_session_list_skips_lines_without_pipe() {
        let s = b"$0|dev\nbroken-line\n$1|work\n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "dev");
        assert_eq!(result[1].name, "work");
    }
```

- [ ] **Step 3: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-app 2>&1 | tail -10
cargo test -p aish-app --lib parse_session_list 2>&1 | tail -10
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

期望：parse_session_list 4 个测试 passed。整个 aish-app build 仍可能因 host_session_task 用旧字段名报错 — 留 Task 5 修。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/ssh_actor.rs
git commit -m "feat(aish-app): tmux_query_task + parse_session_list 解析远端 sessions"
```

---
## Task 5: ssh_actor — host_session_task ActorMode 改造

**Files:**
- Modify: `crates/aish-app/src/ssh_actor.rs`

- [ ] **Step 1: 完整改写 host_session_task**

替换 `crates/aish-app/src/ssh_actor.rs` 的整个 `pub(crate) async fn host_session_task` 函数体（包括 keyring lazy read 段、SshClient::connect、open_channel、request_pty、shell、main loop）。直接覆盖现有实现：

```rust
pub(crate) async fn host_session_task(
    host: HostId,
    config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    use aish_secrets::{SecretError, SecretStore};
    use aish_ssh::{ChannelMsg, SshClient};
    use aish_tmux::{TmuxController, TmuxEvent};
    use aish_types::SshAuth;

    use crate::state::SshErrorKind;

    // 0. 如果是 Password auth 且 password 为空（来自 hosts.json），从 keyring 取
    let mut effective_config = config.clone();
    if let SshAuth::Password { password } = &mut effective_config.auth {
        if password.is_empty() {
            match SecretStore::get(host) {
                Ok(p) => {
                    *password = p;
                }
                Err(SecretError::NoEntry) => {
                    let _ = event_tx
                        .send(SshEvent::Error {
                            host,
                            kind: SshErrorKind::AuthFailed,
                            msg: "keyring 中没有该 host 的密码（请重新在 GUI 中输入并保存）".into(),
                        })
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(SshEvent::Error {
                            host,
                            kind: SshErrorKind::AuthFailed,
                            msg: format!("从 keyring 读取密码失败: {}", e),
                        })
                        .await;
                    return;
                }
            }
        }
    }

    // 1. 连接 + 认证
    let session = match SshClient::connect(&effective_config).await {
        Ok(s) => s,
        Err(err) => {
            let kind = match err.kind() {
                aish_ssh::SshErrorKind::ConnectFailed => SshErrorKind::ConnectFailed,
                aish_ssh::SshErrorKind::AuthFailed => SshErrorKind::AuthFailed,
                aish_ssh::SshErrorKind::Io => SshErrorKind::Io,
                aish_ssh::SshErrorKind::Protocol => SshErrorKind::Protocol,
            };
            let _ = event_tx
                .send(SshEvent::Error {
                    host,
                    kind,
                    msg: err.to_string(),
                })
                .await;
            return;
        }
    };

    // 2. 开 raw PTY channel（M2 行为）
    let mut chan = match session.open_channel().await {
        Ok(c) => c,
        Err(err) => {
            let _ = event_tx
                .send(SshEvent::Error {
                    host,
                    kind: SshErrorKind::Protocol,
                    msg: format!("open_channel: {}", err),
                })
                .await;
            return;
        }
    };
    if let Err(err) = chan.request_pty(120, 40, "xterm-256color").await {
        let _ = event_tx
            .send(SshEvent::Error {
                host,
                kind: SshErrorKind::Protocol,
                msg: format!("request_pty: {}", err),
            })
            .await;
        return;
    }
    if let Err(err) = chan.shell().await {
        let _ = event_tx
            .send(SshEvent::Error {
                host,
                kind: SshErrorKind::Protocol,
                msg: format!("shell: {}", err),
            })
            .await;
        return;
    }
    let _ = event_tx.send(SshEvent::Connected { host }).await;

    // 3. spawn 后台 list-sessions（独立 SSH exec channel）
    let session_for_query = session.clone();
    let tx_for_query = event_tx.clone();
    tokio::spawn(tmux_query_task(host, session_for_query, tx_for_query));

    // 4. mode state machine: RawShell <-> TmuxAttached
    enum ActorMode {
        RawShell,
        TmuxAttached(TmuxController),
    }
    let mut mode = ActorMode::RawShell;

    loop {
        tokio::select! {
            msg = chan.wait() => match msg {
                Some(ChannelMsg::Data { data }) => match &mut mode {
                    ActorMode::RawShell => {
                        let _ = event_tx
                            .send(SshEvent::PaneOutput {
                                host,
                                bytes: data.to_vec(),
                            })
                            .await;
                    }
                    ActorMode::TmuxAttached(controller) => {
                        let events = controller.feed_bytes(&data);
                        let mut tree_dirty = false;
                        for ev in events {
                            match ev {
                                TmuxEvent::PaneOutput { pane, data: bytes } => {
                                    let _ = event_tx
                                        .send(SshEvent::TmuxPaneOutput {
                                            host,
                                            pane,
                                            bytes,
                                        })
                                        .await;
                                }
                                _ => {
                                    tree_dirty = true;
                                }
                            }
                        }
                        if tree_dirty {
                            let _ = event_tx
                                .send(SshEvent::TmuxSessionTreeUpdated {
                                    host,
                                    tree: controller.session_tree().clone(),
                                })
                                .await;
                        }
                    }
                },
                Some(ChannelMsg::Eof) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::RemoteExited,
                        })
                        .await;
                    break;
                }
                Some(_) => {}
            },
            cmd = cmd_rx.recv() => match cmd {
                Some(SessionCommand::SendBytes(bytes)) => {
                    if let Err(e) = chan.data(&bytes[..]).await {
                        let _ = event_tx
                            .send(SshEvent::Disconnected {
                                host,
                                reason: DisconnectReason::NetworkError(e.to_string()),
                            })
                            .await;
                        break;
                    }
                }
                Some(SessionCommand::Resize { cols, rows }) => {
                    if let Err(e) = chan.window_change(cols as u32, rows as u32, 0, 0).await {
                        tracing::warn!("PTY resize failed: {}", e);
                    }
                }
                Some(SessionCommand::QueryTmuxSessions) => {
                    let session_for_query = session.clone();
                    let tx_for_query = event_tx.clone();
                    tokio::spawn(tmux_query_task(host, session_for_query, tx_for_query));
                }
                Some(SessionCommand::AttachTmux { session: sess_id }) => {
                    let _ = event_tx
                        .send(SshEvent::TmuxAttaching {
                            host,
                            session: sess_id.clone(),
                        })
                        .await;
                    let new_chan = match session.open_channel().await {
                        Ok(c) => c,
                        Err(err) => {
                            let _ = event_tx
                                .send(SshEvent::TmuxQueryFailed {
                                    host,
                                    msg: format!("open new channel: {}", err),
                                })
                                .await;
                            continue;
                        }
                    };
                    let cmd_str = format!(
                        "tmux -CC attach -t '{}'",
                        sess_id.as_str().replace('\'', "'\\''")
                    );
                    let mut new_chan = new_chan;
                    if let Err(err) = new_chan.exec(true, cmd_str).await {
                        let _ = event_tx
                            .send(SshEvent::TmuxQueryFailed {
                                host,
                                msg: format!("exec tmux -CC: {}", err),
                            })
                            .await;
                        continue;
                    }
                    chan = new_chan;
                    mode = ActorMode::TmuxAttached(TmuxController::new());
                    let _ = event_tx.send(SshEvent::TmuxAttached { host }).await;
                }
                Some(SessionCommand::Disconnect) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::UserRequested,
                        })
                        .await;
                    break;
                }
            },
        }
    }
}
```

注意：
- `chan.exec(true, cmd_str)` — true 是 want_reply（等 channel 确认）。如果 russh 实际签名不同，调整
- mode 切换通过 `chan = new_chan` 替换变量值（旧 chan drop）
- TmuxAttached mode 下，chan.wait() 拿 -CC 协议 bytes 喂给 controller
- aish-ssh::Channel 在 aish-ssh 内部封装，方法签名应当兼容（M2a 已经用 chan.request_pty/shell/data/window_change）

如果 `aish_ssh::Channel` 没有 `exec` 方法（M2a 没用过），需要先在 aish-ssh 加一个 wrapper。**预期 aish-ssh::Channel 已有 exec 方法**（如果没有 implementer 用 channel.exec 直接调 russh API）。

- [ ] **Step 2: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-app 2>&1 | tail -15
cargo test -p aish-app --lib ssh_actor 2>&1 | tail -15
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -5
```

期望：
- 如果 app.rs/terminal_view.rs 还引用旧字段名 build 失败 — 留 Task 7/8 修
- ssh_actor 测试 (parse_session_list 4 + 原 7 encode_key + 原 1 password_empty) = 12 passed

- [ ] **Step 3: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/ssh_actor.rs
git commit -m "feat(aish-app): host_session_task ActorMode 改造 + AttachTmux 处理"
```

---
## Task 6: TmuxSidebarView — 6 子视图渲染

**Files:**
- Create: `crates/aish-app/src/views/tmux_sidebar.rs`
- Modify: `crates/aish-app/src/views/mod.rs`

- [ ] **Step 1: 创建 `crates/aish-app/src/views/tmux_sidebar.rs`**

```rust
//! 中间栏：按 host 的 TmuxState 显示 6 种视图。

use std::sync::Arc;

use aish_tmux::SessionTree;
use aish_types::{HostId, RemoteSession, SessionId};
use gpui::{
    div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TmuxState};

pub struct TmuxSidebarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl TmuxSidebarView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn dispatch_command(&self, host: HostId, cmd: SessionCommand, cx: &mut Context<Self>) {
        let app = self.state.read(cx);
        if let Some(sender) = app.sessions.get(&host).cloned() {
            self.bridge.spawn(async move {
                let _ = sender.send(cmd).await;
            });
        }
    }

    fn handle_refresh(&mut self, host: HostId, cx: &mut Context<Self>) {
        self.dispatch_command(host, SessionCommand::QueryTmuxSessions, cx);
    }

    fn handle_attach(&mut self, host: HostId, session: SessionId, cx: &mut Context<Self>) {
        self.dispatch_command(host, SessionCommand::AttachTmux { session }, cx);
    }
}

impl Render for TmuxSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let host_opt = app.selected;

        let body = match host_opt {
            None => empty_view(),
            Some(host) => match app.tmux_state.get(&host) {
                None | Some(TmuxState::NotChecked) => spinner_view("查询 tmux 中…"),
                Some(TmuxState::NoTmux) => notmux_view(),
                Some(TmuxState::QueryFailed { msg }) => query_failed_view(msg),
                Some(TmuxState::Detected { sessions }) => session_list_view(host, sessions, cx),
                Some(TmuxState::Attaching { session }) => attaching_view(session),
                Some(TmuxState::Attached { session_tree }) => session_tree_view(session_tree),
            },
        };

        let host_for_buttons = host_opt;
        let mut container = div()
            .w(px(200.0))
            .h_full()
            .bg(rgb(0x202020))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col();

        if let Some(host) = host_for_buttons {
            let header = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(rgb(0x333333))
                .child(
                    div()
                        .text_color(rgb(0xeeeeee))
                        .text_size(px(13.0))
                        .child("tmux"),
                )
                .child(
                    div()
                        .px_2()
                        .text_color(rgb(0xaaaaaa))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                                this.handle_refresh(host, cx);
                            }),
                        )
                        .child("↻"),
                );
            container = container.child(header);
        }

        container.child(body)
    }
}

fn empty_view() -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .text_color(rgb(0x888888))
        .text_size(px(12.0))
        .child("未选择 host")
        .into_any_element()
}

fn spinner_view(msg: &str) -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(12.0))
                .child(msg.to_string()),
        )
        .child(
            div()
                .text_color(rgb(0xaaaaaa))
                .text_size(px(11.0))
                .child("⠋"),
        )
        .into_any_element()
}

fn notmux_view() -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xcccccc))
                .text_size(px(12.0))
                .child("未检测到 tmux"),
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("远端可能未安装 tmux 或不在 PATH"),
        )
        .into_any_element()
}

fn query_failed_view(msg: &str) -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xff6666))
                .text_size(px(12.0))
                .child("查询失败"),
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child(msg.to_string()),
        )
        .into_any_element()
}
```

```rust
fn session_list_view(
    host: HostId,
    sessions: &[RemoteSession],
    cx: &mut Context<TmuxSidebarView>,
) -> gpui::AnyElement {
    let mut col = div().flex().flex_col();

    if sessions.is_empty() {
        col = col.child(
            div()
                .px_3()
                .py_3()
                .text_color(rgb(0x888888))
                .text_size(px(12.0))
                .child("(无现有 session)"),
        );
    } else {
        for s in sessions {
            let session_id = s.id.clone();
            let label = format!("○ {}", s.name);
            let row = div()
                .px_3()
                .py_2()
                .text_color(rgb(0xcccccc))
                .text_size(px(13.0))
                .hover(|st| st.bg(rgb(0x2a2a2a)).cursor_pointer())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                        this.handle_attach(host, session_id.clone(), cx);
                    }),
                )
                .child(label);
            col = col.child(row);
        }
    }

    // + new session 按钮（M3b stub disabled）
    col = col.child(
        div()
            .px_3()
            .py_2()
            .text_color(rgb(0x666666))
            .text_size(px(12.0))
            .border_t_1()
            .border_color(rgb(0x333333))
            .child("+ new session (M3c)"),
    );

    col.into_any_element()
}

fn attaching_view(session: &SessionId) -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xcccccc))
                .text_size(px(12.0))
                .child(format!("连接 tmux session: {}", session.as_str())),
        )
        .child(
            div()
                .text_color(rgb(0xaaaaaa))
                .text_size(px(11.0))
                .child("⠋ -CC handshake…"),
        )
        .into_any_element()
}

fn session_tree_view(tree: &SessionTree) -> gpui::AnyElement {
    let mut col = div().flex().flex_col().px_2().py_2().gap_1();

    if tree.sessions.is_empty() {
        col = col.child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("(等待 tmux 协议数据…)"),
        );
    } else {
        for (sid, sess) in &tree.sessions {
            let is_active = tree.active_session.as_ref() == Some(sid);
            let s_marker = if is_active { "●" } else { "○" };
            col = col.child(
                div()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(13.0))
                    .child(format!("{} {} ({})", s_marker, sess.name, sid.as_str())),
            );
            for (wid, win) in &sess.windows {
                col = col.child(
                    div()
                        .pl_4()
                        .text_color(rgb(0xcccccc))
                        .text_size(px(12.0))
                        .child(format!("├─ {} ({})", win.name, wid)),
                );
                for pane in &win.panes {
                    col = col.child(
                        div()
                            .pl_8()
                            .text_color(rgb(0xaaaaaa))
                            .text_size(px(11.0))
                            .child(format!("├─ pane {}", pane)),
                    );
                }
            }
        }
    }

    col.into_any_element()
}
```

注：实际 `aish_tmux::SessionTree` 字段：
- `sessions: BTreeMap<SessionId, Session>`（Session 含 name, windows）
- `active_session: Option<SessionId>`
- `Session.windows: BTreeMap<WindowId, Window>`（Window 含 name, panes, layout）
- `Window.panes: Vec<PaneId>`

WindowId/PaneId 的 Display 实现 (M3a 加) 输出 `@N` / `%N`，session_tree_view 里直接 `{}` 格式化即可。

- [ ] **Step 2: 在 mod.rs 注册 + reexport**

替换 `crates/aish-app/src/views/mod.rs` 整个内容：

```rust
//! GPUI Views。

#![allow(dead_code)]

mod host_form;
mod host_list;
mod terminal_view;
mod tmux_sidebar;

pub use host_form::HostFormModal;
pub use host_list::HostListView;
pub use terminal_view::TerminalView;
pub use tmux_sidebar::TmuxSidebarView;
```

- [ ] **Step 3: 验证（仅编译）**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-app 2>&1 | tail -15
```

期望：build 仍可能因 app.rs / terminal_view.rs 引用旧字段名报错。tmux_sidebar.rs 自身应该 compile 干净。如有 GPUI API 不匹配，调整。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/views/tmux_sidebar.rs crates/aish-app/src/views/mod.rs
git commit -m "feat(aish-app): TmuxSidebarView 6 子视图（NotChecked/NoTmux/Failed/Detected/Attaching/Attached）"
```

---
## Task 7: terminal_view.rs — term_for_render 路由

**Files:**
- Modify: `crates/aish-app/src/views/terminal_view.rs`

- [ ] **Step 1: 加 term_for_render helper fn**

在 `crates/aish-app/src/views/terminal_view.rs` 末尾（在 `#[cfg(test)] mod tests {` 之前，如果有的话；否则在文件末尾）加：

```rust
/// 决定 terminal 显示哪个 Term：
///   - tmux Attached: 取 SessionTree first session -> first window -> first pane 的 Term
///   - 其他状态: 取 raw shell 模式的 host_pty_term
pub(crate) fn term_for_render<'a>(
    app: &'a crate::state::AppState,
    host: aish_types::HostId,
) -> Option<&'a alacritty_terminal::Term<alacritty_terminal::event::VoidListener>> {
    use crate::state::TmuxState;
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

- [ ] **Step 2: 改 take_snapshot 用 term_for_render**

替换 `crates/aish-app/src/views/terminal_view.rs` 的 `take_snapshot` 函数（约第 298-308 行）：

```rust
/// 在 prepaint 阶段读取 Term grid 快照（读借用安全）。
fn take_snapshot(
    host: Option<aish_types::HostId>,
    state: &Entity<AppState>,
    cx: &mut App,
) -> Option<GridSnapshot> {
    let host = host?;
    let app_state = state.read(cx);
    let term = term_for_render(&app_state, host)?;
    Some(GridSnapshot::from_term(term))
}
```

- [ ] **Step 3: 加 term_for_render 单元测试**

如果 terminal_view.rs 没 tests mod，在文件末尾加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, TmuxState};
    use aish_tmux::SessionTree;
    use aish_types::{HostId, PaneId};

    fn mk_state_with_host() -> (AppState, HostId) {
        let cfg = aish_types::HostConfig {
            id: HostId::new(),
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: aish_types::SshAuth::KeyFile {
                path: std::path::PathBuf::from("/tmp/k"),
            },
            env_profile: None,
        };
        let id = cfg.id;
        (AppState::with_hosts(vec![cfg]), id)
    }

    #[test]
    fn term_for_render_returns_host_pty_when_no_tmux() {
        let (mut state, id) = mk_state_with_host();
        state.feed_bytes(id, b"x");
        let term = term_for_render(&state, id);
        assert!(term.is_some());
    }

    #[test]
    fn term_for_render_returns_none_for_unknown_host() {
        let (state, _id) = mk_state_with_host();
        let unknown = HostId::new();
        let term = term_for_render(&state, unknown);
        assert!(term.is_none());
    }

    #[test]
    fn term_for_render_returns_pane_term_when_attached() {
        let (mut state, id) = mk_state_with_host();
        state.apply_tmux_pane_output(id, PaneId(7), b"hi");

        let mut tree = SessionTree::new();
        tree.add_session(aish_types::SessionId::new("$0"), "default".into());
        tree.add_window(
            aish_types::SessionId::new("$0"),
            aish_types::WindowId(0),
            "main".into(),
        )
        .ok();
        tree.add_pane(aish_types::WindowId(0), PaneId(7));
        state.apply_tmux_session_tree(id, tree);

        let term = term_for_render(&state, id);
        assert!(term.is_some());
    }

    #[test]
    fn term_for_render_attached_with_empty_tree_falls_back_to_none() {
        let (mut state, id) = mk_state_with_host();
        state.apply_tmux_session_tree(id, SessionTree::new());
        let term = term_for_render(&state, id);
        assert!(term.is_none());
    }
}
```

注意：`SessionTree::add_pane` 签名应为 `(&mut self, window: WindowId, pane: PaneId)`。如果实际签名不一致，调整测试代码（不改 types.rs）。

- [ ] **Step 4: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-app 2>&1 | tail -10
cargo test -p aish-app --lib terminal_view 2>&1 | tail -10
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

期望：build 仍可能因 app.rs 三栏布局未做报错（Task 9 修），terminal_view 单测应能 passed (4 个新)。

- [ ] **Step 5: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/views/terminal_view.rs
git commit -m "feat(aish-app): terminal_view term_for_render 按 tmux 状态路由 Term"
```

---

## Task 8: app.rs — 处理 8 个新 SshEvent

**Files:**
- Modify: `crates/aish-app/src/app.rs`

- [ ] **Step 1: 改 app.rs 的 SshEvent loop 处理新 variant**

替换 `crates/aish-app/src/app.rs` 的 SshEvent 接收 loop（在 `cx.spawn(async move |cx|` 块里的 match）。完整新逻辑：

```rust
        let state_for_loop = state.clone();
        let mut rx = channel.rx;
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                state_for_loop.update(cx, |state, cx| match event {
                    SshEvent::Connected { host: _ } => {
                        cx.notify();
                    }
                    SshEvent::PaneOutput { host, bytes } => {
                        state.feed_bytes(host, &bytes);
                        cx.notify();
                    }
                    SshEvent::Disconnected { host, reason: _ } => {
                        state.drop_session(host);
                        cx.notify();
                    }
                    SshEvent::Error { host, kind: _, msg } => {
                        tracing::error!(?host, msg, "SSH error");
                        state.drop_session(host);
                        cx.notify();
                    }
                    SshEvent::TmuxQueryStarted { host } => {
                        state.tmux_state.insert(host, crate::state::TmuxState::NotChecked);
                        cx.notify();
                    }
                    SshEvent::TmuxSessionsListed { host, sessions } => {
                        state.tmux_state.insert(
                            host,
                            crate::state::TmuxState::Detected { sessions },
                        );
                        cx.notify();
                    }
                    SshEvent::TmuxQueryFailed { host, msg } => {
                        state.tmux_state.insert(
                            host,
                            crate::state::TmuxState::QueryFailed { msg },
                        );
                        cx.notify();
                    }
                    SshEvent::TmuxNoTmux { host } => {
                        state.tmux_state.insert(host, crate::state::TmuxState::NoTmux);
                        cx.notify();
                    }
                    SshEvent::TmuxAttaching { host, session } => {
                        state.tmux_state.insert(
                            host,
                            crate::state::TmuxState::Attaching { session },
                        );
                        cx.notify();
                    }
                    SshEvent::TmuxAttached { host } => {
                        state.tmux_state.insert(
                            host,
                            crate::state::TmuxState::Attached {
                                session_tree: aish_tmux::SessionTree::new(),
                            },
                        );
                        cx.notify();
                    }
                    SshEvent::TmuxSessionTreeUpdated { host, tree } => {
                        state.apply_tmux_session_tree(host, tree);
                        cx.notify();
                    }
                    SshEvent::TmuxPaneOutput { host, pane, bytes } => {
                        state.apply_tmux_pane_output(host, pane, &bytes);
                        cx.notify();
                    }
                    SshEvent::TmuxDetached { host, reason: _ } => {
                        state.tmux_state.remove(&host);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
```

- [ ] **Step 2: 加 use（如果需要）**

确认 app.rs 顶部 `use` 段。`aish_tmux::SessionTree::new()` 用全路径所以不强求 use，但加 `use aish_tmux;` 让代码更整洁也可。

- [ ] **Step 3: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build -p aish-app 2>&1 | tail -10
```

期望：仅 RootView 三栏 layout 调整未做（Task 9）。其他编译错应该都消失了。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/app.rs
git commit -m "feat(aish-app): app.rs SshEvent loop 处理 8 个 tmux 新 variant"
```

---

## Task 9: app.rs — RootView 三栏布局

**Files:**
- Modify: `crates/aish-app/src/app.rs`

- [ ] **Step 1: RootView struct 加 tmux_sidebar Entity**

替换 `crates/aish-app/src/app.rs` 的 RootView struct + new 方法：

```rust
struct RootView {
    state: Entity<AppState>,
    host_list: Entity<crate::views::HostListView>,
    tmux_sidebar: Entity<crate::views::TmuxSidebarView>,
    terminal: Entity<crate::views::TerminalView>,
    host_form: Entity<crate::views::HostFormModal>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let host_list = cx.new(|cx| {
            crate::views::HostListView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let tmux_sidebar = cx.new(|cx| {
            crate::views::TmuxSidebarView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let terminal = cx.new(|cx| {
            crate::views::TerminalView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let host_form = cx.new(|cx| {
            crate::views::HostFormModal::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        Self {
            state,
            host_list,
            tmux_sidebar,
            terminal,
            host_form,
        }
    }
}
```

- [ ] **Step 2: 改 RootView Render 加中间栏**

替换 `impl Render for RootView` 块：

```rust
impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal_open = self.state.read(cx).modal.is_some();

        let main = div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x1d1f21))
            .child(self.host_list.clone())
            .child(self.tmux_sidebar.clone())
            .child(self.terminal.clone());

        let mut root = div().relative().size_full().child(main);

        if modal_open {
            root = root.child(self.host_form.clone());
        }

        root
    }
}
```

- [ ] **Step 3: 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build --workspace 2>&1 | tail -5
cargo test --workspace 2>&1 | grep -E "test result"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

期望：
- workspace build PASS
- 所有测试 passed（aish-types 13 + aish-ssh 14 + aish-app ≥ 65 + aish-tmux 73 + aish-secrets 5 = ≥ 170 passed）
- fmt PASS
- clippy 全绿

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/app.rs
git commit -m "feat(aish-app): RootView 三栏布局（host_list | tmux_sidebar | terminal）"
```

---

## Task 10: 端到端验证 + push

**Files:** 无文件改动；只验证 + push。

- [ ] **Step 1: 全 workspace 验证**

```bash
cd /c/Users/larry/Desktop/workspace/aish
cargo build --workspace 2>&1 | tail -5
cargo test --workspace 2>&1 | grep -E "test result"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: 全部退出码 0；测试 ≥ 170 passed。

- [ ] **Step 2: GUI 手动验证清单**

```bash
cargo run -p aish-app
```

按下面顺序：

1. 启动 GUI，选 test host 连接
2. 中间栏应当展示「查询 tmux 中…」spinner
3. 几秒后中间栏切换到下面三种之一：
   - 远端没 tmux → 「未检测到 tmux」
   - 远端有 tmux 没 session → session list 空 + 「+ new (M3c)」
   - 远端有 sessions → 列出 ○ <name>，user 可以点
4. 点某个 session → 中间栏 spinner「连接 tmux session: $X」
5. tmux -CC handshake 后中间栏改显 SessionTree 树
6. terminal 显示 first session -> first window -> first pane 的输出
7. 点中间栏 ↻ → 跑 list-sessions，结果 SshEvent 推回（attach 后状态会被覆盖回 Detected — 这是预期行为，user 知道；M3c 优化）
8. Disconnect host → 中间栏返回「未选择 host」（tmux_state 清空）

如果任一步失败，**STOP 不要 push**；记录失败现象后回到对应 task 修。

- [ ] **Step 3: push**

```bash
git config http.postBuffer 524288000
git push origin main
```

- [ ] **Step 4: 等 CI**

```bash
gh run list --limit 1
```

CI 应当跑通（apt install libdbus-1-dev 已在 ci.yml 里）。

- [ ] **Step 5: 完成报告**

```
M3b STATUS: DONE

Commits:
- aish-types RemoteSession
- aish-ssh exec_command + Clone
- aish-app TmuxState + AppState 字段扩展 + SessionCommand/SshEvent 扩展
- aish-app tmux_query_task + parse_session_list
- aish-app host_session_task ActorMode 改造
- aish-app TmuxSidebarView 6 子视图
- aish-app terminal_view term_for_render 路由
- aish-app app.rs SshEvent 8 新 variant 处理
- aish-app RootView 三栏布局

Tests: ≥ 170 passed
GUI 手测: 8/8 通过
CI: success
```

---

## 完成验证（M3b 整体）

```bash
cargo build --workspace
cargo test --workspace          # ≥ 170 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

git log 应有 9 个新 feat commit + spec + plan = 11 个新 commit。

---

## 下一步

M3b 完成后开始 M3c：

- click 切 pane / send-keys to pane / 新建 session / detach 回 raw shell / 重连恢复 SessionTree
- 扩 SessionTree 加 active_window/active_pane 字段，从 control mode 协议事件维护

---

## Self-Review

### Spec 覆盖
- ✅ RemoteSession (Task 1)
- ✅ aish-ssh exec_command (Task 2)
- ✅ TmuxState 6 variant + AppState 字段重命名/扩展 (Task 3)
- ✅ SessionCommand 加 2 variant + SshEvent 加 8 variant (Task 3)
- ✅ tmux_query_task + parse_session_list (Task 4)
- ✅ ssh_actor ActorMode 改造（RawShell <-> TmuxAttached）(Task 5)
- ✅ TmuxSidebarView 6 子视图 (Task 6)
- ✅ terminal_view term_for_render 路由 (Task 7)
- ✅ app.rs 处理 8 个新 SshEvent (Task 8)
- ✅ RootView 三栏布局 (Task 9)
- ✅ 端到端验证 + push (Task 10)

### Placeholder 扫描
- 测试代码完整给出
- 步骤都有具体文件路径 + 完整代码
- "M3b 不写 exec_command 单测" 是明确决策（spec 已说明）
- session_tree_view 里依赖 SessionId/WindowId/PaneId 的 Display 实现 — types.rs 已实现（M3a 实现 Window/Pane Display）

### Type 一致性
- TmuxState variants 在 state.rs / app.rs / tmux_sidebar.rs 都用同一名字
- SessionCommand QueryTmuxSessions / AttachTmux 在 actor / sidebar 都一致
- SshEvent 8 variant 在 actor 触发 + app.rs 接收都一致
- ExecResult 字段 stdout/stderr/exit_code 一致
- SessionTree::new()/add_session/add_window/add_pane 用法基于 M3a types.rs 实现

### 依赖顺序
- Task 1 (aish-types) 独立
- Task 2 (aish-ssh) 独立
- Task 3 (state.rs) 依赖 1
- Task 4 (parse + query_task) 依赖 1, 2, 3
- Task 5 (host_session_task) 依赖 3, 4
- Task 6 (TmuxSidebarView) 依赖 3
- Task 7 (terminal_view) 依赖 3
- Task 8 (app.rs SshEvent) 依赖 3
- Task 9 (RootView 三栏) 依赖 6
- Task 10 验证 + push 依赖前 9 都完成

执行顺序：1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10