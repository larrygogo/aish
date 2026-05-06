# aish M1 — GPUI 起步实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 aish-app 内搭建 GPUI 主窗口 + 左栏/主区两栏布局；启动独立 tokio runtime 通过 mpsc channel 与 GPUI 互通；用 mock 数据验证整套 GPUI/tokio 桥接，做到完整 demo 11 项（spec Section 9）全绿。

**Architecture:** 单 root `Model<AppState>` 持有所有 UI 共享状态；GPUI Views 订阅该 Model 并重渲染；独立 worker thread 跑 tokio multi-thread runtime；mock_ssh_task 通过 `tokio::sync::mpsc::Sender<MockEvent>` 发事件回 GPUI；GPUI 用 `cx.spawn` 跑 async 块接收并 `update_model`。

**Tech Stack:** Rust stable, GPUI (git dep, pin 到 Zed main 上某个稳定 commit), tokio multi_thread runtime, tokio::sync::mpsc

**前置:** M0 已完成（commit `9f19f68` 之后），M1 spec 已落盘 (`docs/superpowers/specs/2026-05-06-aish-m1-gpui-bootstrap-design.md` commit `82d9de5`)。

---

## ⚠️ 实施者须知（GPUI API 现实）

GPUI 在 Zed 主仓库里持续演进，没有 crates.io release，API 名称（method 名、trait 边界）会变。**本 plan 给的 GPUI 代码是参考骨架，标注关键 API 概念**，实际实施时：

1. Task 1 中 pin 的 commit 一旦确定，后续所有 GPUI 代码都要以**该 commit 的 GPUI API 为准**
2. 遇到本 plan 代码与 Zed 当前 GPUI 不一致时，**按 Zed examples 调整**：
   - `<zed-reference>/crates/gpui/examples/` — 官方 example（最重要的学习材料）
   - `<zed-reference>/crates/zed/src/main.rs` — Zed 自己的 App 入口
   - `<zed-reference>/crates/workspace/src/workspace.rs` — 多面板布局参考
3. 不要为了"贴 plan"硬改 GPUI API；plan 错了就修 plan，不要扭曲实现

下面所有 GPUI 代码的注释 `// GPUI-API` 标注处，都属于"按 Zed examples 调整"范围。

---

## File Structure (M1 完成时的目标状态)

```
aish/
├── Cargo.toml                       # workspace deps：加 gpui git dep
├── Cargo.lock                       # cargo update 后产物
├── crates/aish-app/
│   ├── Cargo.toml                   # 加 gpui dep
│   └── src/
│       ├── main.rs                  # 修改：启 tokio runtime + 调 app::run()
│       ├── app.rs                   # 新：GPUI App 入口 + 主窗口
│       ├── state.rs                 # 新：AppState/HostId/MockHost/MockEvent
│       ├── bridge.rs                # 新：tokio runtime worker + channel helper
│       ├── mock.rs                  # 新：mock_ssh_task
│       └── views/
│           ├── mod.rs               # 新：reexport
│           ├── host_list.rs         # 新：左栏 List
│           └── host_pane.rs         # 新：主区 log
└── （Zed 仓库 clone 在项目外：C:\Users\larry\Desktop\workspace\zed-reference）
```

---

## Task 1: 准备工作 — clone Zed + pin GPUI commit + 加依赖

**Files:**
- Modify: `Cargo.toml`（workspace 根，workspace.dependencies 加 gpui）
- Modify: `crates/aish-app/Cargo.toml`（加 gpui dep）
- 项目外 clone: `C:\Users\larry\Desktop\workspace\zed-reference`

- [ ] **Step 1: clone Zed 仓库到学习参考目录**

```powershell
$zedPath = "C:\Users\larry\Desktop\workspace\zed-reference"
if (-not (Test-Path $zedPath)) {
    git clone --depth 50 https://github.com/zed-industries/zed $zedPath
} else {
    Set-Location $zedPath
    git fetch
}
```

`--depth 50` 拿最近 50 个 commit（足够选 pin commit，省下载时间）。完整 monorepo 约 1-2GB，用 shallow clone 控制在 ~500MB。

- [ ] **Step 2: 选定要 pin 的 GPUI commit**

```powershell
Set-Location C:\Users\larry\Desktop\workspace\zed-reference
# 看最近 30 天 GPUI 改动
git log --since="30 days ago" --oneline -- crates/gpui/
# 找一个 GPUI 没大改 + Zed CI 当时绿的 commit
# 倾向选 release tag 附近（v0.X.0 这种）
git tag --sort=-creatordate | Select-Object -First 5
```

**选择标准（implementer 自己决定）：**
- 该 commit 在 GPUI 改动密集期之外（看 `git log --stat` GPUI 改动量小）
- 该 commit 在 Zed release tag 附近（稳定性背书）
- 该 commit 距今不超过 30 天（避免太老错过 bug 修复）

记下选定的 commit SHA（完整 40 位），下一步用。

> 如果 30 天内 GPUI 改动剧烈（找不到稳定 commit），扩大到 60 天；仍找不到考虑 fork GPUI 自管。后者属于阻塞情况，BLOCKED 报告。

- [ ] **Step 3: 修改 workspace `Cargo.toml`，在 `[workspace.dependencies]` 添加 gpui**

回到 aish 项目：

```powershell
Set-Location C:\Users\larry\Desktop\workspace\aish
```

在 `Cargo.toml` 的 `[workspace.dependencies]` 段末尾追加（替换 `<COMMIT_SHA>` 为 Step 2 选定的 SHA）：

```toml
gpui = { git = "https://github.com/zed-industries/zed", rev = "<COMMIT_SHA>" }
```

- [ ] **Step 4: 修改 `crates/aish-app/Cargo.toml` 引入 gpui**

在 `[dependencies]` 段追加：

```toml
gpui = { workspace = true }
```

- [ ] **Step 5: cargo build 验证 GPUI 能拉取 + 编译**

```bash
cargo build -p aish-app
```

Expected: GPUI 及其大量 transitive deps（wgpu, async-task 等）首次下载/编译。**预计 5-15 分钟**。结束后退出码 0。

如失败，常见原因：
- Windows 上 GPUI 需要某些系统依赖（DirectX SDK 等）—— 看错误消息 + 查 Zed README 的 Windows build 指南
- 选的 commit 不能 build —— 退到 Step 2 换 commit

- [ ] **Step 6: commit**

```bash
git add Cargo.toml crates/aish-app/Cargo.toml Cargo.lock
git commit -m "feat(aish-app): 引入 GPUI 依赖（pin 到 Zed <短SHA>）"
```

commit message 里 `<短SHA>` 替换为 7 位 SHA 前缀。

---

## Task 2: AppState Model + 基础类型（TDD）

**Files:**
- Create: `crates/aish-app/src/state.rs`
- Modify: `crates/aish-app/src/main.rs`（加 `mod state;` 让编译器看到）

- [ ] **Step 1: 创建 `crates/aish-app/src/state.rs` 含基础类型**

```rust
//! aish-app 内部 App State：M1 阶段的 Model。
//!
//! 注意：此处的 `HostId` 是 M1 的 mock 类型（u32 newtype），
//! 与 `aish_types::HostId`（UUID）不冲突——M2 接入真实 SSH 时再切换。

use std::collections::HashMap;

/// M1 阶段的 mock host 标识（u32 newtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostId(pub u32);

/// M1 阶段的 mock host（M2 时换成 aish_types::HostConfig）。
#[derive(Debug, Clone)]
pub struct MockHost {
    pub id: HostId,
    pub label: String,
}

/// 从 bridge 推回 GPUI 的事件（M2 时会扩展更多 variant）。
#[derive(Debug, Clone)]
pub enum MockEvent {
    PaneOutput { host: HostId, line: String },
}

/// 单一 root Model：所有 UI 共享状态的 source of truth。
#[derive(Debug, Default)]
pub struct AppState {
    pub hosts: Vec<MockHost>,
    pub selected: Option<HostId>,
    pub pane_logs: HashMap<HostId, Vec<String>>,
}

impl AppState {
    /// 用三个固定 mock host 初始化。
    pub fn with_mock_hosts() -> Self {
        Self {
            hosts: vec![
                MockHost { id: HostId(1), label: "server-A (mock)".into() },
                MockHost { id: HostId(2), label: "server-B (mock)".into() },
                MockHost { id: HostId(3), label: "server-C (mock)".into() },
            ],
            selected: None,
            pane_logs: HashMap::new(),
        }
    }

    /// 切换选中 host。
    pub fn select_host(&mut self, id: HostId) {
        self.selected = Some(id);
    }

    /// 追加一行到指定 host 的 pane log。
    pub fn append_log(&mut self, host: HostId, line: String) {
        self.pane_logs.entry(host).or_default().push(line);
    }

    /// 读指定 host 的 pane log（若无则返回空切片）。
    pub fn logs_of(&self, host: HostId) -> &[String] {
        self.pane_logs
            .get(&host)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_mock_hosts_returns_three() {
        let state = AppState::with_mock_hosts();
        assert_eq!(state.hosts.len(), 3);
        assert_eq!(state.hosts[0].id, HostId(1));
        assert_eq!(state.hosts[2].label, "server-C (mock)");
        assert!(state.selected.is_none());
        assert!(state.pane_logs.is_empty());
    }

    #[test]
    fn select_host_sets_selected() {
        let mut state = AppState::with_mock_hosts();
        state.select_host(HostId(2));
        assert_eq!(state.selected, Some(HostId(2)));
    }

    #[test]
    fn append_log_creates_entry_for_new_host() {
        let mut state = AppState::default();
        state.append_log(HostId(7), "hello".into());
        assert_eq!(state.logs_of(HostId(7)), &["hello".to_string()]);
    }

    #[test]
    fn append_log_accumulates_per_host() {
        let mut state = AppState::default();
        state.append_log(HostId(1), "line A1".into());
        state.append_log(HostId(2), "line B1".into());
        state.append_log(HostId(1), "line A2".into());
        assert_eq!(state.logs_of(HostId(1)), &["line A1".to_string(), "line A2".into()]);
        assert_eq!(state.logs_of(HostId(2)), &["line B1".to_string()]);
    }

    #[test]
    fn logs_of_missing_host_returns_empty_slice() {
        let state = AppState::default();
        assert!(state.logs_of(HostId(99)).is_empty());
    }
}
```

- [ ] **Step 2: 在 `crates/aish-app/src/main.rs` 顶部加 `mod state;`**

`main.rs` 当前内容：

```rust
//! aish 主入口。M0 只做：tracing 初始化 + hello。M1 起接入 GPUI。

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M0 skeleton)");
    println!("aish skeleton — see docs/superpowers/specs/ for design");
}

/// 初始化全局 tracing 订阅器。RUST_LOG 环境变量可覆盖默认 INFO 级别。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
```

替换为：

```rust
//! aish 主入口。M1 起接入 GPUI。

mod state;

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M1 skeleton)");
    println!("aish skeleton — see docs/superpowers/specs/ for design");
}

/// 初始化全局 tracing 订阅器。RUST_LOG 环境变量可覆盖默认 INFO 级别。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
```

后续 task 会继续在 main.rs 顶部加 mod 声明。

- [ ] **Step 3: 验证测试通过**

```bash
cargo test -p aish-app
```

Expected: `5 passed`（state 模块 5 个测试）。

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): 添加 AppState Model 与 mock 类型"
```

---

## Task 3: Bridge — tokio runtime + mpsc channel

**Files:**
- Create: `crates/aish-app/src/bridge.rs`
- Modify: `crates/aish-app/src/main.rs`（加 `mod bridge;`）

- [ ] **Step 1: 创建 `crates/aish-app/src/bridge.rs`**

```rust
//! Bridge：把 tokio runtime 与 GPUI executor 解耦。
//!
//! 启动一个 multi-thread runtime 在专属 worker thread 上，所有 SSH/SFTP/timer 类
//! async 工作都通过 `Bridge::spawn` 提交。runtime 通过 `tokio::sync::mpsc::Sender`
//! 把事件推回 GPUI 端，由 GPUI 用 `cx.spawn` 跑 async 块接收。

use std::future::Future;
use std::sync::Arc;

use crate::state::MockEvent;

/// 与 GPUI 端共享的事件 channel 对端。
///
/// `tx` 给 tokio task 用来发事件；`rx` 在 GPUI 端用 `cx.spawn` 接收。
pub struct EventChannel {
    pub tx: tokio::sync::mpsc::Sender<MockEvent>,
    pub rx: tokio::sync::mpsc::Receiver<MockEvent>,
}

impl EventChannel {
    /// 创建一个容量 64 的有限 channel（防止 OOM；M1 mock 流量不会满）。
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        Self { tx, rx }
    }
}

/// tokio runtime 包装。Drop 时 runtime 会优雅 shutdown。
pub struct Bridge {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Bridge {
    /// 启动 multi-thread runtime（默认 worker 数 = 物理核数）。
    pub fn start() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("aish-tokio")
            .build()?;
        Ok(Self { runtime: Arc::new(rt) })
    }

    /// 在 runtime 上提交一个 future。
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }

    /// 拿一个 runtime handle 用于跨线程 spawn（如果调用方不持有 Bridge 引用）。
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bridge_can_spawn_and_send_events() {
        let bridge = Bridge::start().expect("runtime should start");
        let mut chan = EventChannel::new();
        let tx = chan.tx.clone();

        bridge.spawn(async move {
            for i in 1..=3 {
                tx.send(MockEvent::PaneOutput {
                    host: crate::state::HostId(i),
                    line: format!("line {}", i),
                })
                .await
                .ok();
            }
        });

        // 同步等待 3 个事件到达（最多等 1 秒）
        let received = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut got = Vec::new();
                while got.len() < 3 {
                    match tokio::time::timeout(Duration::from_secs(1), chan.rx.recv()).await {
                        Ok(Some(ev)) => got.push(ev),
                        _ => break,
                    }
                }
                got
            })
        })
        .join()
        .unwrap();

        assert_eq!(received.len(), 3);
        if let MockEvent::PaneOutput { host, line } = &received[0] {
            assert_eq!(host.0, 1);
            assert_eq!(line, "line 1");
        }
    }

    #[test]
    fn event_channel_capacity_is_64() {
        // 不直接测容量，而是测 64 个 send 可以连续完成（buffer 装得下）
        let chan = EventChannel::new();
        let tx = chan.tx;
        // 用 try_send 避免 await：未满会成功
        for i in 0..64 {
            tx.try_send(MockEvent::PaneOutput {
                host: crate::state::HostId(i),
                line: "x".into(),
            })
            .expect("buffer of 64 should accept 64 sends without blocking");
        }
        // 第 65 个应该满
        assert!(tx
            .try_send(MockEvent::PaneOutput {
                host: crate::state::HostId(65),
                line: "x".into(),
            })
            .is_err());
    }
}
```

- [ ] **Step 2: 在 `main.rs` 加 `mod bridge;`**

在 `main.rs` 顶部已有的 `mod state;` 之后追加：

```rust
mod bridge;
```

- [ ] **Step 3: 验证测试通过**

```bash
cargo test -p aish-app bridge
```

Expected: `bridge_can_spawn_and_send_events` + `event_channel_capacity_is_64`，2 passed。整个 aish-app 现在共 7 passed。

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): 添加 Bridge（tokio runtime + mpsc channel）"
```

---

## Task 4: Mock SSH task

**Files:**
- Create: `crates/aish-app/src/mock.rs`
- Modify: `crates/aish-app/src/main.rs`（加 `mod mock;`）

- [ ] **Step 1: 创建 `crates/aish-app/src/mock.rs`**

```rust
//! Mock SSH 行为：模拟真实 SSH 连接的延迟与输出。
//!
//! M2 接入真实 `aish_ssh::SshClient` 时整体替换此模块。

use std::time::Duration;

use tokio::sync::mpsc::Sender;

use crate::state::{HostId, MockEvent};

/// 模拟"连上 server，3 秒后收到 welcome 消息"。
///
/// 实际产生效果：
///   t=0     立即返回（caller 可继续做事）
///   t+3s    通过 channel send 一行 "Welcome to <label>!"
pub async fn mock_ssh_task(host: HostId, label: String, tx: Sender<MockEvent>) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    let _ = tx
        .send(MockEvent::PaneOutput {
            host,
            line: format!("Welcome to {}! (mocked SSH output)", label),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mock_ssh_task_emits_welcome_after_three_seconds() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let started = Instant::now();
        tokio::spawn(mock_ssh_task(HostId(42), "test-server".into(), tx));

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("should not timeout")
            .expect("channel should yield event");
        let elapsed = started.elapsed();

        // 时间窗口宽松：2.8s ~ 4s 都算正常（CI 环境抖动）
        assert!(
            elapsed >= Duration::from_millis(2800) && elapsed < Duration::from_secs(4),
            "expected ~3s, got {:?}",
            elapsed
        );

        match event {
            MockEvent::PaneOutput { host, line } => {
                assert_eq!(host, HostId(42));
                assert!(line.contains("test-server"));
                assert!(line.contains("mocked SSH output"));
            }
        }
    }
}
```

- [ ] **Step 2: 在 `main.rs` 加 `mod mock;`**

在已有 `mod bridge;` 之后追加：

```rust
mod mock;
```

- [ ] **Step 3: 验证测试通过**

```bash
cargo test -p aish-app mock
```

Expected: 1 passed。**测试本身耗时约 3 秒**（mock_ssh_task 真在等）。

整个 aish-app 现在共 8 passed。

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): 添加 mock_ssh_task（3 秒后发 PaneOutput）"
```

---

## Task 5: GPUI App + 主窗口（hello world 阶段）

**Files:**
- Create: `crates/aish-app/src/app.rs`
- Modify: `crates/aish-app/src/main.rs`

⚠️ **本 Task 起进入 GPUI 部分**。所有标 `// GPUI-API` 的代码需要按 Task 1 选定的 commit 调整 API 名称。**先看 `<zed-reference>/crates/gpui/examples/hello_world.rs`** 找当前正确的 API 形态。

- [ ] **Step 1: 看 Zed example 确认当前 GPUI 启动 App + 开窗口的 API 形态**

```bash
ls C:\Users\larry\Desktop\workspace\zed-reference\crates\gpui\examples\
cat C:\Users\larry\Desktop\workspace\zed-reference\crates\gpui\examples\hello_world.rs
```

记下：
- `App::new()` 还是 `Application::new()`？
- 开窗口的方法名是 `open_window` / `new_window` / 别的？
- WindowOptions 的字段（如 bounds、title、focus 等）

- [ ] **Step 2: 创建 `crates/aish-app/src/app.rs` — 最小窗口骨架**

参考代码（按上面 Step 1 看到的 API 调整）：

```rust
//! aish GPUI 主应用入口。

use gpui::*; // GPUI-API: 实际 prelude 名按当前版本

use crate::bridge::{Bridge, EventChannel};
use crate::state::AppState;

/// 启动 GPUI App 与主窗口。
///
/// 此函数 block 直到所有窗口关闭，调用方应在 main 末尾调它。
pub fn run() {
    // 1. 启动 tokio runtime（先于 GPUI App，让 spawn 在窗口未开时也可用）
    let bridge = Bridge::start().expect("tokio runtime should start");
    let _channel = EventChannel::new();

    // 2. 启动 GPUI App
    // GPUI-API: 下面是 Zed example 风格，实际看当前 hello_world.rs
    Application::new().run(move |cx: &mut App| {
        let _state = cx.new(|_cx| AppState::with_mock_hosts());

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.0), px(800.0)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("aish — M1 skeleton".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(window_options, |_window, cx| {
            cx.new(|_cx| RootView)
        })
        .expect("window should open");
    });

    // bridge 在这里 drop，runtime 优雅 shutdown
    drop(bridge);
}

/// 临时 root view —— Task 6/7 会扩展为左栏 + 主区。
struct RootView;

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // GPUI-API: div() / flex / px() 等按当前版本
        div()
            .flex()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child("aish M1 — empty window")
    }
}
```

> **关键点**：如果上面 API 与 Zed 当前不一致，**改这里的代码而不是去改 Zed**。具体 prelude 的 trait/types 名字可能不同（比如 `App` vs `AppContext`、`cx.new` vs `cx.new_view` vs `cx.new_model`）。

- [ ] **Step 3: 修改 `main.rs` 调用 `app::run()`**

```rust
//! aish 主入口。M1 起接入 GPUI。

mod app;
mod bridge;
mod mock;
mod state;
mod views;

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M1)");
    app::run();
}

/// 初始化全局 tracing 订阅器。RUST_LOG 环境变量可覆盖默认 INFO 级别。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
```

注意：`mod views;` 此刻引用的目录还不存在；编译会报错。下一步先创建占位再继续。

- [ ] **Step 4: 创建 `crates/aish-app/src/views/mod.rs` 占位**

```rust
//! GPUI Views。Task 6/7 实际填充。
```

- [ ] **Step 5: 验证 cargo run 能开窗口**

```bash
cargo run -p aish-app
```

Expected: 1200×800 窗口弹出，标题"aish — M1 skeleton"，内容是深灰背景 + 一行白色文本"aish M1 — empty window"。**关掉窗口进程退出**。

如果编译失败：按 GPUI 错误消息查 Zed 源码当前 API 名称，修 `app.rs`。

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): GPUI 主窗口 hello world"
```

---

## Task 6: HostListView（左栏）

**Files:**
- Create: `crates/aish-app/src/views/host_list.rs`
- Modify: `crates/aish-app/src/views/mod.rs`
- Modify: `crates/aish-app/src/app.rs`（在 root view 中嵌入 HostListView）

- [ ] **Step 1: 看 Zed 的 List 渲染参考**

```bash
# 找 Zed 里类似 sidebar 列表的实现
ls C:\Users\larry\Desktop\workspace\zed-reference\crates\workspace\src\
cat C:\Users\larry\Desktop\workspace\zed-reference\crates\workspace\src\sidebar.rs 2>$null | Select-Object -First 80
# 或者看 file_finder / project_panel
```

记下"从 Vec 生成 N 个可点击 child element"的常见模式。

- [ ] **Step 2: 创建 `crates/aish-app/src/views/host_list.rs`**

参考代码：

```rust
//! 左栏：mock host 列表，点击切换 selected。

use gpui::*; // GPUI-API

use crate::state::{AppState, HostId};

pub struct HostListView {
    state: Entity<AppState>, // GPUI-API: Entity / Model 名按当前版本
}

impl HostListView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // 订阅 state 变更，自动重渲染
        cx.observe(&state, |_this, _state, cx| cx.notify())
            .detach();
        Self { state }
    }

    fn handle_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.select_host(host);
            // 立即追加 "Connecting..." 行，让 UI 立刻反馈
            let label = state
                .hosts
                .iter()
                .find(|h| h.id == host)
                .map(|h| h.label.clone())
                .unwrap_or_else(|| format!("host {:?}", host));
            let line = format!(
                "[{}] Connecting to {}...",
                chrono_like_now(),
                label
            );
            state.append_log(host, line);
            cx.notify();
        });

        // Task 8 会在这里加 bridge.spawn(mock_ssh_task(...))
    }
}

/// 简易"当前时间"字符串，避免引入 chrono 依赖。
fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 86400)
        .unwrap_or(0);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;
        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                        this.handle_click(id, cx);
                    }))
                    .child(label)
            })
            .collect();

        div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .children(host_rows)
    }
}
```

- [ ] **Step 3: 在 `crates/aish-app/src/views/mod.rs` 中 reexport**

```rust
//! GPUI Views。

mod host_list;

pub use host_list::HostListView;
```

- [ ] **Step 4: 修改 `app.rs` 把 `HostListView` 嵌入 root view**

把 `RootView` 替换为：

```rust
struct RootView {
    host_list: Entity<crate::views::HostListView>,
}

impl RootView {
    fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let host_list = cx.new(|cx| crate::views::HostListView::new(state.clone(), cx));
        Self { host_list }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x121212))
            .child(self.host_list.clone())
            .child(
                div()
                    .flex_1()
                    .text_color(rgb(0x888888))
                    .p_4()
                    .child("请从左侧选择主机"),
            )
    }
}
```

并修改 `app::run` 中创建 root view 的部分，改为：

```rust
        let state = cx.new(|_cx| AppState::with_mock_hosts());
        let state_for_window = state.clone();

        cx.open_window(window_options, move |_window, cx| {
            cx.new(|cx| RootView::new(state_for_window.clone(), cx))
        })
        .expect("window should open");
```

- [ ] **Step 5: 验证 cargo run**

```bash
cargo run -p aish-app
```

Expected:
- 窗口弹出，左栏 220px 宽，深色背景
- 左栏显示三行：`server-A (mock)` / `server-B (mock)` / `server-C (mock)`
- 主区显示 "请从左侧选择主机" 灰色提示
- 点击 server-A → 该行高亮（白色文字 + 较亮背景），但**主区仍是提示文字**（Task 7 才接通）
- hover 任一行有视觉反馈

如果点击没高亮，或没有 hover 反馈：检查 GPUI 当前的 `cx.listener` / `on_mouse_down` API 名。

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): 左栏 HostListView（点击切换 selected）"
```

---

## Task 7: HostPaneView（主区）

**Files:**
- Create: `crates/aish-app/src/views/host_pane.rs`
- Modify: `crates/aish-app/src/views/mod.rs`
- Modify: `crates/aish-app/src/app.rs`

- [ ] **Step 1: 创建 `crates/aish-app/src/views/host_pane.rs`**

```rust
//! 主区：渲染当前 selected host 的 pane log。

use gpui::*; // GPUI-API

use crate::state::AppState;

pub struct HostPaneView {
    state: Entity<AppState>,
}

impl HostPaneView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify())
            .detach();
        Self { state }
    }
}

impl Render for HostPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        match state.selected {
            None => div()
                .flex_1()
                .h_full()
                .text_color(rgb(0x888888))
                .p_4()
                .child("请从左侧选择主机")
                .into_any_element(),
            Some(host) => {
                let lines = state.logs_of(host);
                let text_lines: Vec<AnyElement> = lines
                    .iter()
                    .map(|line| {
                        div()
                            .text_color(rgb(0xeeeeee))
                            .child(line.clone())
                            .into_any_element()
                    })
                    .collect();

                div()
                    .flex_1()
                    .h_full()
                    .bg(rgb(0x121212))
                    .text_color(rgb(0xeeeeee))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(text_lines)
                    .into_any_element()
            }
        }
    }
}
```

- [ ] **Step 2: 在 `views/mod.rs` 加 reexport**

```rust
//! GPUI Views。

mod host_list;
mod host_pane;

pub use host_list::HostListView;
pub use host_pane::HostPaneView;
```

- [ ] **Step 3: 修改 `app.rs` 把 HostPaneView 加进 RootView**

`RootView` 改为：

```rust
struct RootView {
    host_list: Entity<crate::views::HostListView>,
    host_pane: Entity<crate::views::HostPaneView>,
}

impl RootView {
    fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let host_list = cx.new(|cx| crate::views::HostListView::new(state.clone(), cx));
        let host_pane = cx.new(|cx| crate::views::HostPaneView::new(state, cx));
        Self { host_list, host_pane }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x121212))
            .child(self.host_list.clone())
            .child(self.host_pane.clone())
    }
}
```

- [ ] **Step 4: 验证 cargo run**

```bash
cargo run -p aish-app
```

Expected:
- 与 Task 6 一样，但点击 server-A 后**主区显示** `[<时间>] Connecting to server-A...`（HostListView click handler 已 append log，HostPaneView 订阅 Model 重渲染）
- 切换到 server-B → 主区切换成 server-B 内容（如果之前没点过，主区只显示新点击 append 的那一行）
- 切回 server-A → 之前的 Connecting 行**仍在**

⚠️ **此时还没接 mock_ssh_task**，所以**不会有 3 秒后的 Welcome 行**。Task 8 接通。

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): 主区 HostPaneView（订阅 selected + pane_logs）"
```

---

## Task 8: 接通 click → bridge → channel → AppState 更新（完整 demo）

**Files:**
- Modify: `crates/aish-app/src/app.rs`
- Modify: `crates/aish-app/src/views/host_list.rs`

- [ ] **Step 1: 在 `app.rs` 中把 Bridge 与 EventChannel 共享给 HostListView**

`app.rs` 的 `run()` 函数改为：

```rust
pub fn run() {
    let bridge = std::sync::Arc::new(
        Bridge::start().expect("tokio runtime should start")
    );

    Application::new().run(move |cx: &mut App| {
        let state = cx.new(|_cx| AppState::with_mock_hosts());
        let channel = EventChannel::new();

        // 1. 启动 GPUI cx.spawn 接收 channel 事件，update Model
        let state_for_loop = state.clone();
        cx.spawn(async move |mut cx| {
            let mut rx = channel.rx;
            while let Some(event) = rx.recv().await {
                let _ = state_for_loop.update(&mut cx, |state, cx| {
                    match event {
                        crate::state::MockEvent::PaneOutput { host, line } => {
                            state.append_log(host, line);
                            cx.notify();
                        }
                    }
                });
            }
        })
        .detach();

        // 2. 开窗口，传入 bridge + tx 让 HostListView 能 spawn mock task
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.0), px(800.0)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("aish — M1 skeleton".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let bridge_for_window = bridge.clone();
        let tx_for_window = channel.tx.clone();
        let state_for_window = state.clone();

        cx.open_window(window_options, move |_window, cx| {
            cx.new(|cx| {
                RootView::new(
                    state_for_window.clone(),
                    bridge_for_window.clone(),
                    tx_for_window.clone(),
                    cx,
                )
            })
        })
        .expect("window should open");
    });
}
```

- [ ] **Step 2: 修改 `RootView::new` 签名 + 把 bridge/tx 传给 HostListView**

```rust
struct RootView {
    host_list: Entity<crate::views::HostListView>,
    host_pane: Entity<crate::views::HostPaneView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: std::sync::Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<crate::state::MockEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let host_list = cx.new(|cx| {
            crate::views::HostListView::new(state.clone(), bridge, tx, cx)
        });
        let host_pane = cx.new(|cx| crate::views::HostPaneView::new(state, cx));
        Self { host_list, host_pane }
    }
}
```

- [ ] **Step 3: 修改 `views/host_list.rs` 接收 bridge/tx 并在 click 时 spawn**

整个文件替换为：

```rust
//! 左栏：mock host 列表，点击切换 selected 并触发 mock SSH。

use std::sync::Arc;

use gpui::*; // GPUI-API

use crate::bridge::Bridge;
use crate::mock::mock_ssh_task;
use crate::state::{AppState, HostId, MockEvent};

pub struct HostListView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<MockEvent>,
}

impl HostListView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<MockEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify())
            .detach();
        Self { state, bridge, tx }
    }

    fn handle_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        // 1. 立即更新 Model（让 UI 立刻反馈 "Connecting..."）
        let label = self.state.update(cx, |state, cx| {
            state.select_host(host);
            let label = state
                .hosts
                .iter()
                .find(|h| h.id == host)
                .map(|h| h.label.clone())
                .unwrap_or_else(|| format!("host {:?}", host));
            let line = format!("[{}] Connecting to {}...", chrono_like_now(), label);
            state.append_log(host, line);
            cx.notify();
            label
        });

        // 2. 在 tokio runtime 上 spawn mock_ssh_task；3 秒后 channel 收事件 → app.rs 的 spawn loop 处理
        let tx = self.tx.clone();
        self.bridge.spawn(mock_ssh_task(host, label, tx));
    }
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 86400)
        .unwrap_or(0);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;
        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                        this.handle_click(id, cx);
                    }))
                    .child(label)
            })
            .collect();

        div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .children(host_rows)
    }
}
```

- [ ] **Step 4: 验证完整 demo（spec Section 9 11 项）**

```bash
cargo run -p aish-app
```

逐项验证（手动操作）：

1. ☐ 窗口弹出（1200×800，可 resize）
2. ☐ 左栏显示三行 `server-A (mock)` / `server-B (mock)` / `server-C (mock)`
3. ☐ 主区初始为 "请从左侧选择主机"
4. ☐ 点击 `server-A` → 主区**立刻**显示 `[<时间>] Connecting to server-A...`
5. ☐ 等 3 秒 → 主区追加 `Welcome to server-A (mock)! (mocked SSH output)`
6. ☐ 切到 `server-B` → 主区切换；快速点回 A → A 之前的两行**仍在**
7. ☐ 立刻点 A 然后立刻点 B（间隔 < 1s）→ 3 秒后 A 和 B 各自的 Welcome 行**都到位**
8. ☐ 关窗口 → 进程在 1 秒内退出（`echo $LASTEXITCODE` 是 0）

如果哪一项不通过：debug + 记录原因；常见问题 + 解法：
- "Connecting" 不显示：`cx.notify()` 没调；或者 cx.update 写法错
- 3 秒后 Welcome 不出现：检查 channel rx 在 spawn loop 里有没有正确接收（加 tracing::debug! 看）
- 切换 host 内容丢失：HashMap key 类型不匹配（HostId Hash impl 缺）

- [ ] **Step 5: 跑剩余验证**

```bash
cargo test --workspace                                       # 应 8 + 4 + 2 + 1 = 至少 15 passed (M0 5 + M1 8)
cargo fmt --all -- --check                                   # PASS
cargo clippy --workspace --all-targets -- -D warnings        # PASS
```

- [ ] **Step 6: commit + push**

```bash
git add crates/aish-app/
git commit -m "feat(aish-app): 接通 click → bridge → mock SSH → channel → Model（M1 demo 完整）"
git push origin main
```

push 后等 GitHub Actions CI 跑完（10-15 分钟首跑），观察是否三平台都过。

如果 CI 挂在某平台（特别是 Windows GPUI 编译），按 spec 风险表的指引：尝试 fix；如果是 GPUI 上游 bug，记录到 follow-up，M2 之前再处理。

---

## 完成验证（M1 整体）

执行下面，全部应成功：

```bash
cargo build --workspace
cargo test --workspace                                       # 至少 15 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p aish-app                                        # demo 11 项全过
```

git log 应该新增 8 个 commit（Task 1-8 各一个），加上 M0 的 11 个 commit + spec 2 个 commit，total ~21 commit。

---

## 下一步

M1 完成后开始 M2（SSH 连接 + 单 PTY 终端）：
- 接入 `aish_ssh::SshClient::connect`（用 russh）
- 集成 `alacritty_terminal::Term` 替换 mock pane log
- 引入 `aish_types::HostConfig` 替换 `MockHost`
- 引入 `~/.aish/hosts.json` 持久化主机列表

M2 不在本 plan 范围。M1 完成后单独 brainstorm → spec → plan → implement。
