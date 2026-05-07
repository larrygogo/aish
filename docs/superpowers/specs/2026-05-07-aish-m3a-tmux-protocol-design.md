# aish M3a — tmux control mode 协议层设计

- **日期**: 2026-05-07
- **状态**: Design (approved by user, ready for implementation planning)
- **里程碑**: M3a（M3 第一阶段；M3 整体拆 M3a / M3b / M3c）
- **前置**: M2c 已完成（commit `efd6e81` 之后；GUI host 增删改 + hosts.json 持久化）
- **作者**: larry
- **预计周期**: 1-2 周

---

## 1. M3 拆分背景

主 spec section 7 把 M3 描述为"tmux control mode + 可视化"。完整 M3 含：协议解析 / ssh_actor 集成 / 检测降级 / GUI sidebar 树 / 多 pane 渲染 / send-keys / 重连恢复。**单 spec 装不下**，按 brainstorming scope check 拆为：

- **M3a（本 spec）**：aish-tmux crate 协议层（pure state machine，不接 ssh_actor 不接 GUI）
- **M3b**：ssh_actor 集成 + tmux 版本检测降级 + GUI sidebar 树（**仅展示**，无切换无 send-keys）
- **M3c**：点击切换 + 多 pane 渲染 + send-keys + 重连状态恢复

每个 sub-milestone 独立可 demo + 测试。

---

## 2. M3a 范围与目标

### Phase 1（M3a，本 spec 范围）

| 模块 | 范围 |
|---|---|
| `aish-tmux::TmuxController` | Pure state machine — `feed_bytes(&[u8]) -> Vec<TmuxEvent>` + `build_command(TmuxCommand) -> Vec<u8>`，不持有 IO |
| 协议解析 | tmux control mode v2 (`tmux -CC`) 输出流：`%begin/%end` / `%output` / `%session-changed` 等 12+ 种 `%xxx` events |
| `SessionTree` 数据结构 | sessions/windows/panes 三层 BTreeMap，layout string 仅存 raw |
| `TmuxEvent` enum | 12 个 variants 覆盖 daily 操作（冷门事件留远期） |
| `TmuxCommand` enum + builder | 结构化命令：send-keys (literal/hex) / new-window / kill-pane / resize-window / switch-client / select-pane / attach-session |
| send-keys 编码安全 | 默认 `-l` literal 模式（所有字符字面）；特殊键（Ctrl+C/Esc/方向键）用 hex 编码 |
| 单元/集成测试 fixtures | `tests/fixtures/*.txt` 录制真实 tmux control mode 输出，按场景命名 |

### 不在范围内（明确边界）

- **ssh_actor 集成**（启动 `tmux -CC new-session` / 接 control channel） → M3b
- **tmux 版本检测 + 降级到 raw PTY**（< 2.6 老版本） → M3b
- **GUI sidebar 树展示**（host > session > window > pane） → M3b
- **点击切换 / send-keys 接通 GPUI** → M3c
- **多 pane 渲染**（同一 host 多个 pane 各自 alacritty Term） → M3c
- **layout string 几何解析**（计算 pane 位置/尺寸） → M3c（渲染时才需要）
- **断网重连后 tmux 状态恢复** → M3c
- **冷门 tmux events**（title-changed / status-changed / copy-mode-changed / message 等） → 远期

---

## 3. 关键技术决策（M3a-ADR）

| ID | 决策 | 备选 | 理由 |
|---|---|---|---|
| M3a-ADR-1 | **三拆 M3a/b/c 而非单 spec** | 单 spec / 二拆 | 4-8 周单 spec 中间无 checkpoint 风险大；三拆每段 1-3 周独立可测可 demo；M3a 纯协议工作天然独立 |
| M3a-ADR-2 | **TmuxController 用 pure state machine（无 IO）** | Actor pattern (channel) | 与 M2b1 的 alacritty_terminal::Term 设计一致；架构对称；单元测试极简（喂 fixture bytes 验证 events + state） |
| M3a-ADR-3 | **TmuxEvent enum 含 12 个 variants** | 更全 (含 title/status/copy-mode) / 更少 | YAGNI — daily SSH+tmux 操作不需要 title/status；冷门 event 留远期按需扩 |
| M3a-ADR-4 | **SessionTree 用 BTreeMap** | HashMap | UI 列表顺序需要稳定（按 session/window/pane id 排序），BTreeMap 内置排序，HashMap 要手动排 |
| M3a-ADR-5 | **layout string 仅存 raw，不解析** | M3a 解析 / 完全不存 | 解析 layout 是渲染层关注，M3c 才用得到；M3a 协议层只负责"把 layout 字段透传"；存 raw 让 M3c 拿到完整信息 |
| M3a-ADR-6 | **send-keys 默认 `-l` literal + 特殊键 hex** | 字节级 hex / shell escape | literal 模式 tmux 不解析转义，安全（防注入）；特殊键（Ctrl+C 等）走 hex 因为 literal 没办法发不可见字符 |
| M3a-ADR-7 | **测试 fixtures 用真 tmux 输出录制** | 全手写 mock / 仅手写 | 真录制能覆盖 tmux 实现的边角细节（行尾 \r\n、字段顺序、转义）；手写 fixtures 容易遗漏；user 在 plan 阶段提供录制（或 implementer 用 docker 起 tmux 录） |
| M3a-ADR-8 | **TmuxController 不区分 stdout/stderr stream id** | 区分两路 | tmux control mode 的 `%output` 不带 stream id（PTY 输出本来就混在一起）；区分仅在 ssh 命令模式有意义；M3 不做 |
| M3a-ADR-9 | **错误 enum TmuxError 与 SshError 平行**（不复用） | 复用 SshError | tmux 协议错误（解析失败 / 不识别 event 行）与 SSH 错误正交，分开 enum 更清晰；M3b 集成时各自处理 |

---

## 4. 系统架构

### 数据流（不接 IO 的 state machine）

```
                     ┌────────────────────────────────────┐
                     │     上层（M3b：ssh_actor 集成）       │
                     │                                    │
   PTY bytes (raw) ─►│                                    │
   from russh        │  controller.feed_bytes(&[u8])      │
                     │     ↓                              │
                     │     events: Vec<TmuxEvent>         │─► 推回 GPUI
                     │                                    │
   GPUI keypress    ─►│  controller.build_command(cmd)    │
                     │     → bytes: Vec<u8>               │─► 写回 russh
                     │                                    │
                     │  controller.session_tree()         │─► 读 SessionTree 给 sidebar
                     └────────────────────────────────────┘

                     ┌────────────────────────────────────┐
                     │   aish-tmux::TmuxController        │
                     │                                    │
                     │   state: SessionTree (private)     │
                     │   parser_buf: Vec<u8> (按行解析)    │
                     │                                    │
                     │   feed_bytes:                      │
                     │     1. append to parser_buf        │
                     │     2. drain complete %xxx 行      │
                     │     3. 每行 dispatch 到 handle_*   │
                     │     4. 更新 state + 收集 events    │
                     │     5. 返回 Vec<TmuxEvent>         │
                     │                                    │
                     │   build_command:                   │
                     │     enum TmuxCommand → byte cmd    │
                     └────────────────────────────────────┘
```

### tmux control mode 协议简述

`tmux -CC new-session` 启动后，stdout 是行式协议，每行一个事件。常见 event 类型：

```
%begin <ts> <num> <flags>      # 命令响应开始
<output...>
%end <ts> <num> <flags>        # 命令响应结束

%error <ts> <num> <flags>      # 错误响应

%output %<pane> <hex-encoded-bytes>   # PTY 输出（hex 编码避免行内换行污染协议）

%sessions-changed              # session 列表整体变化（无字段）
%session-changed $<id> <name>  # 当前活跃 session 变化
%session-renamed $<id> <name>
%window-add @<id>
%window-close @<id>
%window-renamed @<id> <name>
%pane-mode-changed %<id>
%layout-change @<id> <layout-string> [<visible-layout>] [<window-flags>]
%client-detached <client>
%exit [<reason>]
```

**关键点**：

- 所有 event 行以 `\r\n` 或 `\n` 结尾
- `%output` 的 bytes 是 hex 编码（每字节两个 hex char）— 避免 PTY 输出含 `\n` 干扰协议解析
- `%begin/%end` 包裹的内容是命令的输出，不直接对应 event；M3a 仅用于 list-sessions 等命令拿初始 SessionTree 快照
- 启动时 tmux 会推送一段 `%begin ... %end` 含全部 sessions/windows/panes 的初始状态

### TmuxController 公开接口

```rust
pub struct TmuxController {
    state: SessionTree,
    parser_buf: Vec<u8>,
    pending_command_response: Option<PendingCommand>,
}

impl TmuxController {
    pub fn new() -> Self { ... }

    /// 喂入 tmux control channel 的 raw bytes，返回派生的 events 与 state mutations。
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<TmuxEvent>;

    /// 构造一个 tmux 命令的字节流（含 `\n` 行尾），调用方写入 control channel。
    pub fn build_command(&self, cmd: TmuxCommand) -> Vec<u8>;

    /// 只读访问当前 SessionTree 快照。
    pub fn session_tree(&self) -> &SessionTree;
}

pub enum TmuxEvent {
    SessionAdded(SessionId),
    SessionRemoved(SessionId),
    SessionRenamed { id: SessionId, name: String },
    WindowAdded { session: SessionId, window: WindowId, name: String },
    WindowRemoved(WindowId),
    WindowRenamed { window: WindowId, name: String },
    PaneAdded { window: WindowId, pane: PaneId },
    PaneOutput { pane: PaneId, data: bytes::Bytes },
    PaneDied(PaneId),
    LayoutChanged { window: WindowId, layout: String },
    ClientSessionChanged { session: SessionId },
    Exit { reason: String },
}

pub enum TmuxCommand {
    SendText { pane: PaneId, text: String },         // 用 -l literal
    SendKey { pane: PaneId, key: Key },               // 用 hex 编码特殊键
    NewWindow { session: SessionId, name: Option<String> },
    KillPane { pane: PaneId },
    ResizePane { pane: PaneId, cols: u16, rows: u16 },
    SwitchClient { session: SessionId },
    SelectPane { pane: PaneId },
    AttachSession { name: String },
    ListSessions,                                     // 主动同步 SessionTree
}

pub enum Key {
    CtrlC, CtrlD, CtrlZ, CtrlL,
    Enter, Tab, Esc, Backspace,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Home, End, PageUp, PageDown,
}

pub enum TmuxError {
    /// 协议行不识别（含原始字节用于调试）
    UnknownEvent(String),
    /// %output 的 hex 字段解码失败
    HexDecodeFailed(String),
    /// `%error` event（tmux 主动报错）
    TmuxProtocolError { ts: u64, num: u64, message: String },
    /// 内部状态不一致（例如 PaneAdded 但 Window 不存在）
    InconsistentState(String),
}
```

### SessionTree 数据结构

```rust
use std::collections::BTreeMap;

pub struct SessionTree {
    pub sessions: BTreeMap<SessionId, Session>,
    pub active_session: Option<SessionId>,
}

pub struct Session {
    pub name: String,
    pub windows: BTreeMap<WindowId, Window>,
    pub active_window: Option<WindowId>,
}

pub struct Window {
    pub session: SessionId,
    pub name: String,
    pub panes: BTreeMap<PaneId, Pane>,
    pub active_pane: Option<PaneId>,
    pub layout: String,  // raw layout string，M3c 才解析
}

pub struct Pane {
    pub window: WindowId,
    // M3a 只存元数据；alacritty Term 在 aish-app::AppState 里 keyed by (HostId, PaneId)
}
```

### Type aliases（与 aish-types 关系）

`SessionId / WindowId / PaneId` 在 aish-types 已经定义（M0 Task 3）：

```rust
pub struct SessionId(String);    // tmux session 名（如 "default"）
pub struct WindowId(pub u32);    // tmux @<n>
pub struct PaneId(pub u32);      // tmux %<n>
```

aish-tmux 直接复用，不再新定义。

---

## 5. File Structure（M3a 完成时）

```
aish/
├── Cargo.toml                          # 加 bytes (M2 已有) — 不新增 dep
├── crates/
│   └── aish-tmux/
│       ├── Cargo.toml                  # 修改：加 bytes / aish-types / thiserror dep
│       └── src/
│           ├── lib.rs                  # 改写：mod 声明 + reexport
│           ├── error.rs                # 新：TmuxError
│           ├── events.rs               # 新：TmuxEvent enum
│           ├── commands.rs             # 新：TmuxCommand + Key + build_command 实现
│           ├── types.rs                # 新：SessionTree / Session / Window / Pane
│           ├── protocol.rs             # 新：行式 parser + dispatch 到 handle_*
│           └── controller.rs           # 新：TmuxController state machine
└── crates/aish-tmux/tests/
    ├── fixtures/                       # 新建目录
    │   ├── startup_one_session.txt
    │   ├── attach_existing_session.txt
    │   ├── multi_window.txt
    │   ├── pane_output_stream.txt
    │   ├── pane_died.txt
    │   ├── window_renamed.txt
    │   └── session_close.txt
    └── protocol_test.rs                # 集成测试：跑 fixtures
```

新增 7 个 .rs + 7 个 .txt 文件。aish-tmux crate 当前是 M0 骨架（仅 lib.rs 有 smoke test），整体重写。

不影响 aish-app / aish-ssh / 其他 crate。

---

## 6. 核心数据流（fed bytes 例子）

ssh_actor (M3b 才接) 收到一段 tmux control bytes（来自 russh PTY channel）：

    "%session-changed $0 default\r\n%window-add @0\r\n%pane-add %0\r\n"

→ controller.feed_bytes(bytes)：

1. parser_buf 累积 bytes
2. 按 \n 切行：
   - "%session-changed $0 default"
   - "%window-add @0"
   - "%pane-add %0"
3. 逐行 dispatch:
   - protocol::parse_line("%session-changed $0 default") → ParsedEvent::SessionChanged { id: $0, name: "default" } → controller.handle_session_changed(...) → state.active_session = Some($0); 如果 sessions 不含 $0 加进去 → 返回 TmuxEvent::SessionAdded($0) + ClientSessionChanged($0)
   - protocol::parse_line("%window-add @0") → state.sessions[$0].windows.insert(@0, Window::new(...)) → 返回 TmuxEvent::WindowAdded { session: $0, window: @0, name: "" }
   - protocol::parse_line("%pane-add %0") → state.sessions[$0].windows[@0].panes.insert(%0, Pane::new(...)) → 返回 TmuxEvent::PaneAdded { window: @0, pane: %0 }
4. 返回 vec![SessionAdded, ClientSessionChanged, WindowAdded, PaneAdded]

M3b 拿到 events 后:

```
for ev in events {
    sshevent_tx.send(SshEvent::TmuxEvent { host, event: ev }).await;
}
```

GPUI 端接收 → 更新 sidebar tree

### 命令构造例子

```rust
// GPUI sidebar 用户点击 pane %3 切换
// ssh_actor 调:
let cmd = controller.build_command(TmuxCommand::SelectPane { pane: PaneId(3) });
// cmd = b"select-pane -t %3\n".to_vec()
chan.data(&cmd).await?;

// 用户键盘输入 "ls\n"
let cmd = controller.build_command(TmuxCommand::SendText {
    pane: PaneId(3),
    text: "ls\n".into(),
});
// cmd = b"send-keys -t %3 -l 'ls\\n'\n".to_vec()
// (注意 shell-escape: 'ls\n' 包成单引号；内含单引号要转义)

// 用户按 Ctrl+C
let cmd = controller.build_command(TmuxCommand::SendKey {
    pane: PaneId(3),
    key: Key::CtrlC,
});
// cmd = b"send-keys -t %3 0x03\n".to_vec()  (0x03 是 Ctrl+C 的 byte)
```

---

## 7. 错误处理矩阵

| 场景 | 处理 |
|---|---|
| **解析行失败**（未知 `%xxx`） | feed_bytes 返回的 events 列表中**不包含**该行；TmuxController 内部 `tracing::warn!` 记录 raw 行；后续行继续解析（容错） |
| **`%output` hex 解码失败** | 同上：log + 跳过该行 |
| **`%error` event** | 解析为 `TmuxEvent::Exit { reason: ... }` 推回上层；不内部 panic |
| **`%pane-died` 但 pane 不在 state** | log + 忽略（state 与远端不同步是网络异常的常态，最终 `ListSessions` 重建会修复） |
| **`%window-close` 但 window 不在 state** | 同上 |
| **`%begin/%end` 配对错误**（缺 `%end`） | 容错：超时机制由 M3b 在 ssh_actor 层做；M3a 假设 tmux 协议正确 |
| **build_command 含非法 PaneId** | TmuxController 不校验 PaneId 是否在 state（YAGNI：调用方按 state 拿 ID），只负责拼字节 |

---

## 8. 测试策略

### 单元测试（在各模块内 `#[cfg(test)] mod tests`）

| 模块 | 测试覆盖 |
|---|---|
| `protocol::parse_line` | 每种 `%xxx` event 一个测试 — 输入完整行 → 验证 ParsedEvent |
| `commands::build_command` | 每个 TmuxCommand variant 一个测试 — 验证字节流精确 |
| `commands::Key` → hex | 16 个 Key variants 各一个测试 |
| `types::SessionTree` mutations | add_session / remove_session / rename / etc 单测 |
| `controller` 边界 | feed_bytes 含部分行（半 \r\n 后续 feed 接上）/ 未知 event 容错 / 多 event 一次 feed |

### 集成测试（`tests/protocol_test.rs`）

```rust
fn run_fixture(name: &str) -> (Vec<TmuxEvent>, SessionTree) {
    let bytes = include_bytes!(concat!("fixtures/", name));
    let mut ctrl = TmuxController::new();
    let events = ctrl.feed_bytes(bytes);
    (events, ctrl.session_tree().clone())
}

#[test]
fn startup_one_session_emits_expected_events() {
    let (events, tree) = run_fixture("startup_one_session.txt");
    // 断言 events 序列 + tree 结构
}
```

### Fixture 收集（M3a Task 1）

User 提供 fixtures 收集（在你 VPS 上跑）：

```bash
# 在你的 VPS 上
tmux kill-server  # 确保 fresh
tmux -CC new-session -A -s test_default 2>&1 | head -50 > /tmp/startup_one_session.txt
# Ctrl+B + d 退出
# 然后 cat /tmp/startup_one_session.txt 复制内容
```

或者 implementer 用 testcontainers 起 docker tmux 录制（M5 计划做的，M3a 提前用）。

如果都不方便，**implementer 按 tmux control mode 文档手写 fixtures**（按已知协议格式，单元测试覆盖 parser 而非真 tmux 行为）— 这是 M3a 接受的降级方案。

CI 仍只跑 `cargo build / test / fmt / clippy`。

---

## 9. M3a 完成验证

```
cargo build --workspace
cargo test --workspace            # ~95-100 passed (M2c 76 + M3a 单测 + 集成 ~20-25)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

**M3a 不产生 GUI 变化**（不接 ssh_actor 不接 GUI），demo 验证靠：

1. ✅ aish-tmux 的所有单元测试覆盖 12 种 TmuxEvent + 8 种 TmuxCommand
2. ✅ `tests/fixtures/` 至少 3 个真 tmux 输出文件（其余 4 个手写也行）
3. ✅ 集成测试 7+ 个 (每个 fixture 一个 test)
4. ✅ TmuxController state machine round-trip：feed → events → SessionTree 一致
5. ✅ 主 spec ADR-4 / 0005 ADR 在文档中体现（control mode 路径选择 / 单 channel 复用）

---

## 10. M3a → M3b 演进路径

| M3a | → M3b |
|---|---|
| TmuxController state machine（不接 IO） | ssh_actor 内持有 TmuxController；read PTY bytes → controller.feed_bytes → events 推回 GPUI |
| 仅协议层 | 加 tmux 版本检测：通过 ssh 跑 `tmux -V` 命令 → 解析 `tmux X.Y` → 选 control mode 或降级 raw PTY |
| TmuxCommand 字节构造 | ssh_actor 接收 SessionCommand::TmuxCommand 转 controller.build_command(cmd) → chan.data(bytes) |
| 无 GUI | 新增 TmuxSidebarView：从 AppState.tmux_trees: HashMap<HostId, SessionTree> 渲染树形结构（仅展示） |

M3b 不动 aish-tmux crate（除非发现 protocol bug）。

---

## 11. 已知风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| **tmux control mode 协议文档不全** | 某些 `%xxx` event 实际格式与 manpage 不一致 | 用真录制 fixtures 兜底；遇到不识别的 event 容错（log + 跳过），不 panic |
| **不同 tmux 版本协议字段顺序差异** | 老版 tmux 解析失败 | M3a 假设 tmux >= 2.6；M3b 加版本检测降级 |
| **layout string 格式复杂**（如 `bb62,278x67,0,0{139x67,0,0,0,138x67,140,0,1}`） | M3c 解析时遇困难 | M3a 仅存 raw 字符串；M3c 任务才写解析器 |
| **`%output` hex 编码量大** | feed_bytes 性能 | hex decode 是简单 char-by-char，1MB/s 足够；alacritty Term feed 才是 hot path |
| **fixtures 收集依赖 user VPS** | implementer 跑不了 | 降级方案：手写 fixtures 按 tmux 文档 |
| **TmuxController 与 alacritty Term 关系**：tmux PaneOutput bytes 是否要 feed 给 Term? | M3b 集成时纠结 | 答案：是。M3b ssh_actor 拿到 TmuxEvent::PaneOutput 后调 `state.feed_bytes_to_pane(host, pane, &data)` 喂给该 pane 的 alacritty Term。aish-tmux 不直接接触 Term |

---

## 12. 不在本 spec 范围内（边界提醒）

- ssh_actor 集成 / 启动 `tmux -CC new-session` → M3b
- tmux 版本检测 + 降级到 raw PTY → M3b
- GUI sidebar 树展示 → M3b
- 点击切换 / send-keys 接通 GPUI → M3c
- 多 pane 渲染 → M3c
- layout string 几何解析 → M3c
- 断网重连后 tmux 状态恢复 → M3c
- 冷门 events（title/status/copy-mode/message） → 远期
- copy mode 滚动（远端 tmux 的 copy mode） → 远期