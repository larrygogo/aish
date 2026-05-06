# aish M2a — SSH 接入 + 单 PTY shell 设计

- **日期**: 2026-05-06
- **状态**: Design (approved by user, ready for implementation planning)
- **里程碑**: M2a（M2 第一阶段，整体 M2 拆为 M2a / M2b / M2c）
- **前置**: M1 已完成（commit `e5ad92f` 之后；GPUI/tokio 桥接 demo 验证通过）
- **作者**: larry
- **预计周期**: 5-10 天

---

## 1. M2 拆分背景

主 spec (Section 7) 把 M2 描述为"SSH 连接 + 单 PTY 终端"，覆盖：

1. russh 接入（aish-ssh crate 实际实现）
2. PTY channel
3. alacritty_terminal::Term 集成（vt100 解析 + grid 渲染）
4. GPUI TerminalView（自绘字符 grid + 键盘事件）
5. 类型迁移（`state::HostId(u32)` → `aish_types::HostId(Uuid)`，`MockHost` → `HostConfig`）
6. `~/.aish/hosts.json` 持久化
7. 添加 / 编辑 / 删除 host 的 UI

单一 spec 装不下，按 brainstorming skill scope check 拆为三个 sub-milestone：

- **M2a（本 spec）**：1 + 2 + 5 + 一部分 6（hardcoded fixtures，无持久化）
- **M2b**：3 + 4（终端渲染）
- **M2c**：6 + 7（持久化与 host 管理 UI）

每个 sub-milestone 独立可 demo。

---

## 2. M2a 范围与目标

### Phase 1（M2a，本 spec 范围）

| 模块 | 范围 |
|---|---|
| `aish-ssh` 真实实现 | russh 包装：connect / open_channel / request_pty / writer / reader（最小可用集） |
| Actor 模式 SSH session | 每 host 一个 tokio task，own SshSession + PTY channel；通过 SessionCommand / SshEvent channel 与 GPUI 通信 |
| 类型迁移 | 删 `state::HostId(u32)` / `MockHost` / `MockEvent`；改用 `aish_types::HostId(Uuid)` / `HostConfig` |
| Mock 退役 | 删 `crates/aish-app/src/mock.rs` 整个文件 |
| Hardcoded fixtures | `crates/aish-app/src/fixtures.rs`（M2a 临时方案，含 1-2 个真 host config）；M2c 替换为 hosts.json |
| 长连接 + 命令行交互 | 首次 click → 连接 + open shell channel + 持续 read loop；键盘输入 → 写入 PTY；输出 → 主区 |
| 错误处理 | 连不上 / auth 失败 / 远端 shell 退出 / 网络断 — 4 种场景在主区 inline 显示 |

### 不在范围内（明确边界）

- **alacritty_terminal 集成 / 终端 grid 渲染**（M2b）
- **`~/.aish/hosts.json` 读写持久化**（M2c）
- **添加 / 编辑 / 删除 host 的 UI**（M2c）
- **PTY 跟随窗口 resize**（M2c，依赖 alacritty_terminal）
- **网络断后自动重连**（M3，与 tmux attach 一并做）
- **SSH Agent / Password 认证**（M2c+，仅 KeyFile）
- **`SecretStore`**（M5，aish-secrets crate 仍是 M0 骨架）
- **SFTP**（M4）
- **send_env / 环境变量注入**（M5）
- **Ctrl+C / Tab 等控制字符精细处理**（M2b，alacritty_terminal 接管）

### M2a 完成时主区会显示什么

> 控制字符**显示成乱码**（如 `[0m`、`[1;31m`、`[H`）。**这是 M2a 的预期糙状态**，M2b 上 alacritty_terminal 后自然消失。

例：跑 `ls --color` 主区会看到：

```
[0m[01;34mDocuments[0m  [01;34mDownloads[0m  [01;32mscript.sh[0m
```

而不是带颜色的目录列表。可读性够，能确认 SSH 接通。

---

## 3. 关键技术决策（M2a-ADR）

| ID | 决策 | 备选 | 理由 |
|---|---|---|---|
| M2a-ADR-1 | **完全 retire mock，不保留双轨** | 保留 mock 作为 dev 工具 / SshAuth::Mock variant | mock 是 M1 学 GPUI 的训练用品，使命已完成。双轨制污染类型 + 违反 ADR-8（不预先抽象 dispatcher）。GUI bug 排查可 git checkout 回 M1 commit |
| M2a-ADR-2 | **只支持 KeyFile 认证，不引入 SecretStore** | KeyFile + Agent / stub SecretStore | YAGNI：你 VPS 已经有 key auth。Password 需要 UI 弹窗（M2c），Agent 不是必须。SecretStore 留 M5 实施时一次到位（connect 签名届时增第二参数，破坏性变更但可控） |
| M2a-ADR-3 | **长连接 + Actor model（每 host 一个 tokio task）** | 每 click 新 connect / Arc<Mutex<SshSession>> 共享 | 长连接是 daily SSH baseline；Actor model 把 SshSession 限制在 tokio 内部，避免 GPUI 端持有非 Send 类型 + async lock contention，与 M1 Bridge/channel 模式风格一致 |
| M2a-ADR-4 | **不实现网络断后重连** | 重连 + 指数退避 | M3 实施 tmux attach 时一并做更经济（重连后 tmux 自动 attach 回原 session 才有意义；M2a 重连后 shell 是新的，状态丢失，体验不完整） |
| M2a-ADR-5 | **PTY 大小 hardcoded 120×40，TERM=xterm-256color，不跟随窗口 resize** | 80×24 默认 / 动态算 | 120×40 适配 1200×800 窗口的现代默认；动态 resize 涉及字体度量 + alacritty_terminal，留 M2c |
| M2a-ADR-6 | **bytes → utf8_lossy → split by `\n` → Vec<String> 追加** | Vec<u8> raw / 立即上 alacritty_terminal | M1 数据结构不变（HashMap<HostId, Vec<String>>），最小改动；M2b 一上 alacritty_terminal 这个 decode 路径就被替换 |
| M2a-ADR-7 | **Hardcoded fixtures 在 `aish-app/src/fixtures.rs`，不引入 hosts.json** | 直接读 hosts.json | 持久化是 M2c 范围；fixtures.rs 是临时方案，给 M2a demo 用，M2c 时删除 |
| M2a-ADR-8 | **错误统一显示在主区 inline，不弹模态对话框** | 模态弹窗 / 状态栏 toast | M2a 还没设计 toast / 弹窗组件。inline 错误（红字 / 灰字）复用现有渲染；错误信息留在 pane_logs 与正常输出共存，便于回看 |

---

## 4. 系统架构（Actor model）

### 总览

```
┌──────────────────────────────────────────────────────────────┐
│                  GPUI Application Process                     │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Views (M1 复用 + 改键盘输入)             │    │
│  │  HostListView (左栏)   HostPaneView (主区)            │    │
│  └────────┬─────────────────────┬──────────────────────┘    │
│           │ subscribe             │ subscribe + key event      │
│  ┌────────▼─────────────────────▼──────────────────────┐    │
│  │            AppState (Model<T>)                       │    │
│  │  hosts: Vec<HostConfig>                              │    │
│  │  selected: Option<HostId>                            │    │
│  │  pane_logs: HashMap<HostId, Vec<String>>             │    │
│  │  sessions: HashMap<HostId, mpsc::Sender<SessionCmd>> │    │
│  └────────────┬─────────────────────────────────────────┘    │
│               │ cx.spawn 接收 SshEvent (与 M1 同模式)         │
│               │                                               │
│  ┌────────────▼─────────────────────────────────────────┐    │
│  │           Bridge (tokio runtime)                      │    │
│  │   ┌────────────────────────────────────────────────┐ │    │
│  │   │  per-host actor task (spawn on first connect)  │ │    │
│  │   │  ┌──────────────────────────────────────────┐  │ │    │
│  │   │  │  - own SshSession                         │  │ │    │
│  │   │  │  - own PTY channel (read + write halves)  │  │ │    │
│  │   │  │  - listen on mpsc::Receiver<SessionCmd>   │  │ │    │
│  │   │  │  - read loop → 转 SshEvent → tx            │  │ │    │
│  │   │  └──────────────────────────────────────────┘  │ │    │
│  │   └────────────────────────────────────────────────┘ │    │
│  └─────────────────┬────────────────────────────────────┘    │
└──────────────────────┼────────────────────────────────────────┘
                       │ TCP (russh)
                       ▼
                 ┌─────────────┐
                 │ Remote Host │
                 └─────────────┘
```

### 关键消息类型

```rust
// 从 GPUI 发到 actor task
pub enum SessionCommand {
    SendBytes(Vec<u8>),     // 键盘输入（已编码）
    Disconnect,             // 主动关闭
}

// 从 actor task 发回 GPUI
pub enum SshEvent {
    Connected { host: HostId },
    PaneOutput { host: HostId, bytes: Vec<u8> },     // 原始字节流
    Disconnected { host: HostId, reason: DisconnectReason },
    Error { host: HostId, kind: SshErrorKind, msg: String },
}

pub enum DisconnectReason {
    UserRequested,          // 主动 Disconnect
    RemoteExited,           // shell 进程退出（read 拿到 EOF）
    NetworkError(String),   // 网络断（IO 错误）
}

pub enum SshErrorKind {
    ConnectFailed,
    AuthFailed,
}
```

### 典型 host 生命周期

```
[首次 click server-A]
  AppState.handle_click(host_a)
    1. state.update: select_host + pane_logs.entry(host_a).push("[info] Connecting...")
    2. 检查 sessions[host_a] 是否存在
       - 存在 → 只切 selected（步骤结束）
       - 不存在 → 进 3
    3. bridge.spawn_session(host_a, config_a, sshevent_tx)
       Bridge 内部:
         - mpsc::channel::<SessionCommand>(64) 创建
         - sessions HashMap 把 sender 存起来（不，实际上 sender 直接返回给 GPUI 端）
         - tokio::spawn(host_session_task(...))
       返回 SessionCommand sender
    4. AppState.sessions.insert(host_a, sender)
  
[host_session_task 内部]
  1. SshSession::connect(config) → 失败发 SshEvent::Error → 退出
  2. session.open_channel().request_pty(120, 40, "xterm-256color")
  3. tx.send(SshEvent::Connected)
  4. fork:
     - read_loop: while let Some(bytes) = reader.next().await { tx.send(PaneOutput) }
     - cmd_loop: while let Some(cmd) = rx.recv().await { match cmd { ... } }
  5. 任一 loop 退出 → 整个 task 结束 → SshSession drop → russh close
  6. 发 SshEvent::Disconnected

[再次 click server-A]
  AppState.handle_click(host_a)
    sessions[host_a] 存在 → 只切 selected → 不重连

[click server-B (未连过)]
  同首次 click 逻辑

[键盘输入到 server-A]
  HostPaneView.on_key(event)
    bytes = encode(event)  // 见 4.4
    sender = AppState.sessions[selected_host].clone()
    bridge.spawn(async move { sender.send(SessionCommand::SendBytes(bytes)).await })

[关闭主窗口]
  AppState drop → sessions HashMap drop → 所有 mpsc::Sender drop
    actor task 的 cmd_loop 收到 None → 退出 → SshSession drop → russh close
```

### 4.1 SessionCommand 通道容量

`mpsc::channel::<SessionCommand>(64)`。键盘输入按字节量很小，64 足够。

### 4.2 SshEvent 通道（沿用 M1）

EventChannel 复用 M1 的 mpsc，但内容类型从 `MockEvent` 改名 `SshEvent`。所有 host 共享一个 SshEvent channel（actor task 持有 tx clone）。

### 4.3 PTY read 路径

russh::Channel::wait() 拿 ChannelMsg，匹配 `Data` / `ExtendedData` / `Eof` / `ExitStatus` / `Close`：
- `Data(bytes)` → 转 `SshEvent::PaneOutput`
- `Eof` / `Close` → break read loop，发 `SshEvent::Disconnected { reason: RemoteExited }`
- `ExitStatus(_)` → 记下，等 Eof / Close 触发实际退出

### 4.4 键盘输入编码

GPUI `KeyDownEvent` → 字节流：

| GPUI key | 字节 |
|---|---|
| 普通字符 (a-z / 数字 / 符号) | UTF-8 字节 |
| Enter | `\r` (0x0D) |
| Backspace | `\x7f` (DEL，传统 unix 终端) |
| Tab | `\t` (0x09) |
| Esc | `\x1b` |
| Ctrl+<X> | `(X - 0x40) & 0x1f`（Ctrl+C → 0x03，Ctrl+D → 0x04） |
| 方向键 / Home / End / F1-12 | xterm 应用键序列（`\x1b[A` 等）— **M2a 不实现**，按 dead key 处理 |
| Alt+ | `\x1b` 前缀 + 字符 — **M2a 不实现** |

**M2a 范围**：普通字符 / Enter / Backspace / Tab / Esc / Ctrl+A-Z。其他键留 M2b 与 alacritty_terminal 一并接管（alacritty_terminal 自带完整键序列编码）。

---

## 5. File Structure（M2a 完成时）

```
aish/
├── crates/
│   ├── aish-types/             # 不变（M0 已完成）
│   ├── aish-ssh/
│   │   ├── Cargo.toml          # 修改：加 russh + russh-keys 依赖
│   │   └── src/
│   │       ├── lib.rs          # 改写：reexport 公共 API
│   │       ├── client.rs       # 新：SshClient (russh 包装)
│   │       ├── channel.rs      # 新：Channel / PtyChannel (含 PTY)
│   │       └── error.rs        # 新：SshError + From<russh::Error>
│   ├── aish-tmux/              # 不变
│   ├── aish-sftp/              # 不变
│   ├── aish-secrets/           # 不变（M5 才实现）
│   └── aish-app/
│       └── src/
│           ├── main.rs         # 修改：加 mod fixtures / mod ssh_actor
│           ├── app.rs          # 修改：channel 类型从 MockEvent → SshEvent；run() 路由 SshEvent 到 AppState
│           ├── state.rs        # 改写：HostId 用 aish_types，新增 sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>
│           ├── bridge.rs       # 修改：MockEvent → SshEvent；新增 spawn_session helper（启 actor task）
│           ├── ssh_actor.rs    # 新：host_session_task + SessionCommand + SshEvent + DisconnectReason + SshErrorKind
│           ├── fixtures.rs     # 新：hardcoded HostConfig 列表（M2a 临时，M2c 删除）
│           ├── mock.rs         # ❌ 删除
│           └── views/
│               ├── mod.rs
│               ├── host_list.rs    # 修改：HostId 类型迁移；click 调 spawn_session 替代 mock_ssh_task
│               └── host_pane.rs    # 修改：渲染逻辑不变；新增 GPUI 键盘事件处理 → 发 SessionCommand::SendBytes
```

新增 4 个文件：`fixtures.rs` / `ssh_actor.rs` + `aish-ssh/src/{client,channel,error}.rs`
删除 1 个文件：`aish-app/src/mock.rs`
修改 6 个文件：`main.rs` / `app.rs` / `state.rs` / `bridge.rs` / `views/host_list.rs` / `views/host_pane.rs`
修改 2 个 Cargo.toml：`aish-ssh/Cargo.toml`（加 russh）/ `aish-app/Cargo.toml`（依赖 aish-types）

---

## 6. 核心数据流（完整 click → 输入 → 输出）

### 6.1 用户首次 click server-A

```
HostListView.on_mouse_down
  → AppState.update(|state, cx| {
       state.select_host(host_a);
       state.pane_logs.entry(host_a)
         .or_default()
         .push("[info] Connecting to my-vps...".into());
       cx.notify();
     })
  → Bridge.spawn_session(host_a, config_a.clone(), event_tx.clone())
       内部:
         let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(64);
         tokio::spawn(host_session_task(host_a, config_a, cmd_rx, event_tx));
         返回 cmd_tx
  → AppState.sessions.insert(host_a, cmd_tx)
```

### 6.2 host_session_task 内部

```rust
async fn host_session_task(
    host: HostId,
    config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    // 1. 连接 + 认证
    let session = match SshClient::connect(&config).await {
        Ok(s) => s,
        Err(SshError::ConnectFailed(e)) => {
            let _ = event_tx.send(SshEvent::Error {
                host, kind: SshErrorKind::ConnectFailed, msg: e.to_string(),
            }).await;
            return;
        }
        Err(SshError::AuthFailed(e)) => {
            let _ = event_tx.send(SshEvent::Error {
                host, kind: SshErrorKind::AuthFailed, msg: e.to_string(),
            }).await;
            return;
        }
        Err(other) => { /* 同上 */ return; }
    };

    // 2. 开 PTY channel
    let mut chan = session.open_channel().await?;
    chan.request_pty(120, 40, "xterm-256color").await?;
    chan.shell().await?;

    let _ = event_tx.send(SshEvent::Connected { host }).await;

    // 3. fork: read_loop + cmd_loop（用 tokio::select!）
    loop {
        tokio::select! {
            // 远端输出 → SshEvent
            msg = chan.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    let _ = event_tx.send(SshEvent::PaneOutput {
                        host, bytes: data.to_vec(),
                    }).await;
                }
                Some(ChannelMsg::Eof) | None => {
                    let _ = event_tx.send(SshEvent::Disconnected {
                        host, reason: DisconnectReason::RemoteExited,
                    }).await;
                    break;
                }
                _ => {}  // ExtendedData / ExitStatus 等先忽略
            },
            // 本地输入 → 写 PTY
            cmd = cmd_rx.recv() => match cmd {
                Some(SessionCommand::SendBytes(bytes)) => {
                    if let Err(e) = chan.data(&bytes[..]).await {
                        let _ = event_tx.send(SshEvent::Disconnected {
                            host, reason: DisconnectReason::NetworkError(e.to_string()),
                        }).await;
                        break;
                    }
                }
                Some(SessionCommand::Disconnect) | None => {
                    // None = AppState drop sender，自然退出
                    let _ = event_tx.send(SshEvent::Disconnected {
                        host, reason: DisconnectReason::UserRequested,
                    }).await;
                    break;
                }
            },
        }
    }
    // session drop → russh close
}
```

### 6.3 GPUI 接收 SshEvent 并 update Model

`app.rs` 中的 cx.spawn loop（M1 已建立的模式，类型从 MockEvent 改 SshEvent）：

```rust
cx.spawn(async move |mut cx| {
    while let Some(event) = rx.recv().await {
        let _ = state.update(&mut cx, |state, cx| match event {
            SshEvent::Connected { host } => {
                state.append_log(host, "[info] Connected".into());
                cx.notify();
            }
            SshEvent::PaneOutput { host, bytes } => {
                let s = String::from_utf8_lossy(&bytes);
                for line in s.split('\n') {
                    state.append_log(host, line.to_string());
                }
                cx.notify();
            }
            SshEvent::Disconnected { host, reason } => {
                let prefix = match reason {
                    DisconnectReason::RemoteExited => "[info] 远端 shell 已退出",
                    DisconnectReason::NetworkError(e) => &format!("[error] 连接中断: {}", e),
                    DisconnectReason::UserRequested => "[info] 已断开",
                };
                state.append_log(host, prefix.to_string());
                state.sessions.remove(&host);  // 清掉 sender，下次 click 会重连
                cx.notify();
            }
            SshEvent::Error { host, kind, msg } => {
                state.append_log(host, format!("[error] {:?}: {}", kind, msg));
                state.sessions.remove(&host);
                cx.notify();
            }
        });
    }
}).detach();
```

### 6.4 键盘输入

HostPaneView 增加 GPUI 键盘订阅：

```rust
impl Render for HostPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        // ... 现有渲染逻辑 ...
        div()
            .focusable()  // GPUI-API: 让此 div 可接收键盘焦点
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            // ... children ...
    }
}

impl HostPaneView {
    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(host) = self.state.read(cx).selected else { return };
        let Some(sender) = self.state.read(cx).sessions.get(&host).cloned() else { return };

        let bytes = encode_key(event);  // 见 4.4
        if !bytes.is_empty() {
            // 在 tokio runtime spawn 一个 oneshot send
            self.bridge.spawn(async move {
                let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
            });
        }
    }
}
```

需要把 `bridge: Arc<Bridge>` 也注入 HostPaneView（之前只有 HostListView 有）。

具体 `encode_key` 函数实现按 4.4 表格。

---

## 7. 错误处理矩阵

| 场景 | 触发 | actor task 行为 | UI 表现 |
|---|---|---|---|
| **连不上** | TCP 超时 / DNS 失败 / refused | 不进 read loop，发 `SshEvent::Error { ConnectFailed }` 后退出 | 主区红字 `[error] ConnectFailed: <io::Error msg>` |
| **auth 失败** | 私钥不被接受 / key 文件不存在 | 同上发 `AuthFailed` 后退出 | 主区红字 `[error] AuthFailed: <russh::Error msg>` |
| **远端 shell 退出** | 用户跑 `exit` → channel EOF | read loop 收 Eof，发 `Disconnected { RemoteExited }` 后退出 | 主区灰字 `[info] 远端 shell 已退出` |
| **网络断** | 已连后 read loop 拿到 IO err | 发 `Disconnected { NetworkError }` 后退出 | 主区红字 `[error] 连接中断: <reason>` |
| **写 PTY 失败** | 已连后 chan.data() 报错 | 同 NetworkError 路径 | 主区红字 `[error] 连接中断: <reason>` |
| **AppState drop** | 关窗口 | cmd_rx.recv() 返 None，发 `Disconnected { UserRequested }` 后退出 | 用户已经在关窗口，事件不会被处理（cx.spawn loop 也死了） |
| **GPUI render panic** | UI 代码 bug | 主 spec 已定：panic hook + crash log + 进程退出；actor task 随 Bridge drop 而 abort | 应用退出，下次启动崩溃报告窗 |

`SshEvent::Disconnected` / `Error` 处理时**清掉 `sessions` 里的 sender entry**，让用户可以再次 click 同 host 触发重连（手动重连，非自动）。

**M2a 不区分** UI 显示颜色（红字 vs 灰字 vs 普通字符）—— 当前 HostPaneView 全是 0xeeeeee 浅色。错误识别靠 `[error]` / `[info]` 前缀文字。M2b 上 alacritty_terminal 后用控制字符上色更优雅。

---

## 8. 测试策略

| 模块 | 测试方式 |
|---|---|
| `aish-ssh::error` | 单元测试 `From<russh::Error>` 转换正确 |
| `aish-ssh::client` | 集成测试**推迟到 M5**（与 aish-secrets 一起用 testcontainers） |
| `aish-ssh::channel` | 同上 |
| `aish-app::ssh_actor::encode_key` | 单元测试键盘按键 → 字节映射（独立 pure function） |
| `aish-app::state` | 单元测试 sessions HashMap 增删（不依赖真 SSH） |
| 端到端 demo | **user 手动验证 9 项**（见 Section 9） |

CI 仍只跑：`cargo build / test / fmt / clippy`，无 SSH 集成测试。

---

## 9. M2a 完成验证（demo 标准）

执行 `cargo run -p aish-app`，按以下步骤手动验证（VPS 信息已在 `fixtures.rs`）：

1. ✅ 窗口弹出（沿用 M1，1200×800）
2. ✅ 左栏显示真 host（如 `my-vps`）
3. ✅ 主区初始为 "请从左侧选择主机"
4. ✅ 点击 `my-vps` → 主区立刻显示 `[info] Connecting to my-vps...`
5. ✅ 1-3 秒后 → 主区追加 `[info] Connected` + 看到 shell prompt（如 `larry@vps:~$ `）
6. ✅ 键盘输入 `ls` + Enter → 看到目录 listing（控制字符乱码可接受）
7. ✅ 键盘输入 `echo hello` + Enter → 看到 `hello`
8. ✅ 键盘输入 `exit` + Enter → 主区追加 `[info] 远端 shell 已退出`，可再次 click 重连
9. ✅ 故意改 fixtures.rs 用错的 key 路径 → 启动后 click → 看到 `[error] AuthFailed: ...`
10. ✅ 关窗口 → 进程在 1 秒内退出（`echo $LASTEXITCODE` = 0）
11. ✅ `cargo test --workspace` 全绿
12. ✅ `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` 全绿
13. ✅ CI 三平台 test job 全绿（push 后自动验证）

---

## 10. M2a → M2b 演进路径

M2b（终端渲染）的最小改动：

| M2a | → M2b |
|---|---|
| `pane_logs: HashMap<HostId, Vec<String>>` | `pane_terminals: HashMap<HostId, alacritty_terminal::Term>` |
| `SshEvent::PaneOutput { bytes }` 处理：utf8_lossy + split + append_log | `SshEvent::PaneOutput { bytes }` 处理：feed bytes 到 Term |
| `HostPaneView` 渲染：每行一个 div | `HostPaneView` 渲染：自绘 Term grid（字符 + 颜色 + 光标） |
| 键盘 `encode_key`：M2a 自实现（普通字符 + Ctrl + Enter 等） | 键盘 `encode_key`：alacritty_terminal 提供完整 xterm key 序列编码 |
| PTY size 120×40 hardcoded | PTY size 按主区 px + 字体度量动态算 + window resize 触发 chan.window_change |
| 错误前缀 `[error]` / `[info]` 文字 | 错误前缀加 ANSI 颜色（红 / 灰），通过 alacritty_terminal 渲染 |

`aish-ssh` / `ssh_actor` / `fixtures` / `state.sessions` 这部分 M2b 不动。

M2c（持久化 + UI）的演进：

| M2a | → M2c |
|---|---|
| `fixtures.rs` 删除 | 启动时读 `~/.aish/hosts.json` 反序列化为 `Vec<HostConfig>` |
| 没有 host 增删 UI | 新增 "Add Host" 按钮 + 表单对话框 + 写回 hosts.json |
| 没有 host 编辑 / 删除 | 左栏 right-click 上下文菜单 |

---

## 11. 已知风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| **russh 在 Windows 上的 platform-specific bug** | 连接 / handshake 异常 | russh 跨平台支持成熟，但遇 bug 退到 ssh2 (libssh2 binding) — 工作量 +1-2 天 |
| **GPUI 键盘事件 / focusable 的 API 不稳定** | M2a Section 6.4 实现可能与 plan 不一致 | implementer 按 Zed example 调整（参考 Zed `crates/editor` 的键盘处理） |
| **fixtures.rs 含真 host 信息，git 不应 commit 真凭证** | 如果 implementer 把 VPS host / user 写死并 commit，凭证泄露 | plan 阶段明确：fixtures.rs 用环境变量读 host 信息，或 fixtures.rs 加进 `.gitignore`；commit 一个 `fixtures.example.rs` 模板 |
| **键盘编码不全，部分操作没法执行** | 用户用方向键、F1-12 时无响应 | M2a 文档明确告知"M2b 才完整"；常用操作（普通字符 / Enter / Backspace / Ctrl+C / Ctrl+D / Tab）覆盖 90% 日常 |
| **utf8_lossy 截断多字节字符** | 命令输出含中文 / emoji 时偶尔显示 `\u{fffd}`（替换字符） | M2a 接受；M2b alacritty_terminal 内部有完整 utf8 流式 decoder |
| **demo 验证依赖你机器 + VPS 凭证** | implementer subagent 无法验证 demo（subagent 无 VPS 凭证） | 由 user 本人 demo 验证；subagent 只验证 build + 启动无 panic + cargo test 全绿 |

---

## 12. 不在本 spec 范围内（边界提醒）

- alacritty_terminal 集成 / 终端 grid 渲染 → M2b
- `~/.aish/hosts.json` 持久化 → M2c
- 添加 / 编辑 / 删除 host UI → M2c
- PTY 跟随窗口 resize → M2c
- 网络断后自动重连 → M3
- SSH Agent / Password 认证 → M2c+
- SecretStore（aish-secrets crate） → M5
- SFTP → M4
- env 注入（send_env） → M5
- Ctrl+C / Tab 等控制字符精细处理 → M2b（alacritty_terminal 接管）
- 方向键 / Home / End / F1-12 / Alt+ 键编码 → M2b（alacritty_terminal 提供）
- tmux control mode → M3
