# M4a 信息架构 4-tab 化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 aish 顶层导航重组为 4-tab 信息架构（左侧 48px 纯 icon sidebar），为后续 AI agent 子项目准备 UI 容器。

**Architecture:** 在 AppState 加 `SidebarTab` enum + `sidebar` 字段；RootView 拆成 `SidebarNav`（左侧 48px icon bar）+ 主区（按 sidebar 分支渲染 HomeView / TerminalView / ComingSoonView）；顶部 SessionTabBar 仅 sidebar=Terminal 时渲染；HomeView 接管现有 default_page.rs 的 host 卡片网格逻辑并新增 Active Sessions 区块。

**Tech Stack:** Rust stable + GPUI（`gpui`/`gpui_platform` git dep）；`std::time::SystemTime`（humanize elapsed，不引入 chrono）；全测试跑 `cargo test --workspace`；fmt/clippy 跑 `cargo +nightly fmt --all` / `cargo +nightly clippy --workspace --all-targets -- -D warnings`。

**Note on TDD:** GPUI view 层无现成测试基础设施，view task 跳过写测试。只有 state.rs 的纯逻辑函数做 TDD。每个 task 完成后必须跑 `cargo check --workspace` 保持编译通过。

**Spec:** `docs/superpowers/specs/2026-05-08-aish-m4a-info-arch-design.md`

---

## 文件结构

**新增：**
- `crates/aish-app/src/views/sidebar_nav.rs` — 左侧 48px 4-icon 导航条
- `crates/aish-app/src/views/home.rs` — Home tab：Quick Actions + Active Sessions + Hosts grid
- `crates/aish-app/src/views/empty_terminal.rs` — sidebar=Terminal 且 0 会话时的引导卡
- `crates/aish-app/src/views/coming_soon.rs` — Inbox/Settings 通用 placeholder

**修改：**
- `crates/aish-app/src/state.rs` — 加 `SidebarTab` enum、`AppState.sidebar`、Connection 方法、调整 `with_hosts` / `close_tab` / `remove_connection`
- `crates/aish-app/src/theme.rs` — 加 sidebar 相关常量
- `crates/aish-app/src/views/mod.rs` — 暴露新模块，删 default_page
- `crates/aish-app/src/app.rs` — RootView 重构（加 SidebarNav，按 sidebar 分支）
- `crates/aish-app/src/views/tab_bar.rs` — `+` 按钮改为切 sidebar=Home

**删除：**
- `crates/aish-app/src/views/default_page.rs`

---

## Task 1: state.rs — SidebarTab + sidebar 字段 + 方法调整

**Files:**
- Modify: `crates/aish-app/src/state.rs`

**背景知识：**
- `Connection.opened_at: SystemTime` 已经存在，用它来显示 last-active 时间，不需要新加字段。
- `close_tab` fallback 目前在 tabs 清空后新建一个 Default tab——M4a 起不需要，tabs 允许为空。
- `remove_connection` 目前把相关 tab 的 content 改成 `TabContent::Default`——M4a 起应该改成关闭这些 tab，避免出现 "没有 DefaultPage 但 tab content 是 Default" 的状态。
- `new_default_tab` 目前被 tab_bar 的 + 按钮调用——Task 9 会改掉 tab_bar，本 task 暂时保留 `new_default_tab` 加 `#[allow(dead_code)]`，Task 9 完成后再删。

- [ ] **Step 1: 在 state.rs 顶部加 SidebarTab enum**

在 `state.rs` 文件里，找到 `pub enum TabContent` 的上方（约 L271），插入：

```rust
/// 顶层 4-tab 导航当前选中项（M4a 信息架构）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SidebarTab {
    #[default]
    Home,
    Terminal,
    Inbox,
    Settings,
}
```

- [ ] **Step 2: 在 AppState struct 加 sidebar 字段**

找到 `pub struct AppState` 定义（约 L287），在 `pub modal: Option<HostFormState>,` 行后插入：

```rust
    /// 顶层 4-tab 导航当前选中项（M4a 信息架构）。
    pub sidebar: SidebarTab,
```

- [ ] **Step 3: 在 with_hosts 初始化 sidebar + 移除初始 Default tab**

找到 `pub fn with_hosts` 方法（约 L319）。当前代码创建了一个初始 Default tab 并设置 `selected_tab`。M4a 起默认 sidebar=Home，tabs 可以为空，所以：

将：
```rust
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        // 启动时自动开一个默认页 tab，避免界面空白。
        let initial_tab = Tab {
            id: TabId::new(),
            content: TabContent::Default,
            title: "新连接".into(),
        };
        let initial_tab_id = initial_tab.id;
        Self {
            hosts,
            connections: HashMap::new(),
            tabs: vec![initial_tab],
            selected_tab: Some(initial_tab_id),
            pending_session_picker: None,
            sessions: HashMap::new(),
            modal: None,
            host_pty_term: HashMap::new(),
            host_pty_processor: HashMap::new(),
            host_pty_dimensions: HashMap::new(),
            tmux_state: HashMap::new(),
        }
    }
```

改为：
```rust
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
            host_pty_term: HashMap::new(),
            host_pty_processor: HashMap::new(),
            host_pty_dimensions: HashMap::new(),
            tmux_state: HashMap::new(),
        }
    }
```

- [ ] **Step 4: 修改 close_tab — 删完 tabs 时不再新建 Default tab**

找到 `pub fn close_tab` 方法（约 L395）。将 fallback 逻辑删掉：

将：
```rust
        if self.selected_tab.is_none() {
            self.new_default_tab();
        }
```

改为：
```rust
        // tabs 可以为空，sidebar=Terminal 时主区会显示 EmptyTerminalGuideView
```

注意：还要保留 `#[allow(dead_code)]` 注解给 `new_default_tab`（Task 9 改完 tab_bar 后再删）。

- [ ] **Step 5: 修改 remove_connection — 关闭 tab 而不是变成 Default**

找到 `pub fn remove_connection` 方法（约 L470）。将原来把 tab content 变为 Default 的逻辑改为关闭这些 tab：

将：
```rust
        for t in &mut self.tabs {
            if t.content == TabContent::Connection(id) {
                t.content = TabContent::Default;
                t.title = "新连接".into();
            }
        }
```

改为：
```rust
        let ids_to_close: Vec<TabId> = self
            .tabs
            .iter()
            .filter(|t| t.content == TabContent::Connection(id))
            .map(|t| t.id)
            .collect();
        for tab_id in ids_to_close {
            self.close_tab(tab_id);
        }
```

- [ ] **Step 6: 给 new_default_tab 加 #[allow(dead_code)] 注解**

找到 `pub fn new_default_tab` 方法，在 `pub fn` 前一行加注解（Task 9 完成后再删整个方法）：

```rust
    #[allow(dead_code)] // Task 9 改完 tab_bar 的 + 按钮后删
    pub fn new_default_tab(&mut self) -> TabId {
```

- [ ] **Step 7: 给 Connection 加 humanize_opened_at 方法**

找到 `pub struct Connection` 定义（约 L262），在其 `impl` 块（如果没有则新建）里加：

在 state.rs 中 `pub struct Connection { ... }` 定义后，找合适的位置（比如在 `impl AppState` 上方）插入：

```rust
impl Connection {
    /// 返回自 opened_at 到现在的 humanize 字符串，用于 Active Sessions 显示。
    pub fn humanize_opened_at(&self) -> String {
        let secs = self
            .opened_at
            .elapsed()
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
}
```

- [ ] **Step 8: 修改受影响的现有测试**

`with_hosts` 改为 tabs 起始为空 + `remove_connection` 改为关闭 tab 后，以下现有测试需要同步修改：

**删除**（测试的是 M4a 之后不再成立的行为）：
- `with_hosts_creates_initial_default_tab` — 删除整个函数（已被新加的 `with_hosts_starts_with_empty_tabs` 取代）
- `close_last_tab_auto_creates_default` — 删除整个函数（M4a 起 close last tab → tabs 为空，不再自动新建）

**修改** `remove_connection_resets_referencing_tab_to_default`（L682）— M4a 后 remove_connection 关闭 tab 而非变成 Default：

将：
```rust
    #[test]
    fn remove_connection_resets_referencing_tab_to_default() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        state.replace_current_tab(TabContent::Connection(conn), "x".into());
        state.remove_connection(conn);
        let cur = state.current_tab().unwrap();
        assert_eq!(cur.content, TabContent::Default);
        assert_eq!(cur.title, "新连接");
    }
```

改为：
```rust
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
```

**修改** `replace_current_tab_swaps_in_place`（L712）— `with_hosts` 之后 tabs 为空，需要先手动 push 一个 tab：

将：
```rust
    #[test]
    fn replace_current_tab_swaps_in_place() {
        let h = mk_host("a");
        let host_id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let conn = state.open_connection(host_id);
        let initial_id = state.selected_tab.unwrap();
        state.replace_current_tab(TabContent::Connection(conn), "腾讯云 #1".into());
        assert_eq!(state.selected_tab, Some(initial_id));
        assert_eq!(state.current_tab().unwrap().title, "腾讯云 #1");
        assert_eq!(state.current_connection(), Some(conn));
    }
```

改为：
```rust
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
```

**修改** `close_tab_picks_neighbor_when_current`（L725）— 同样需要手动 push tabs：

将：
```rust
    #[test]
    fn close_tab_picks_neighbor_when_current() {
        let mut state = AppState::with_hosts(vec![]);
        let t1 = state.selected_tab.unwrap();
        let t2 = state.new_default_tab();
        state.close_tab(t2);
        assert_eq!(state.selected_tab, Some(t1));
        assert_eq!(state.tabs.len(), 1);
    }
```

改为：
```rust
    #[test]
    fn close_tab_picks_neighbor_when_current() {
        use aish_types::TabId;
        let mut state = AppState::with_hosts(vec![]);
        // 手动 push 两个 tab
        let id1 = TabId::new();
        let id2 = TabId::new();
        state.tabs.push(Tab { id: id1, content: TabContent::Default, title: "1".into() });
        state.tabs.push(Tab { id: id2, content: TabContent::Default, title: "2".into() });
        state.selected_tab = Some(id2);
        state.close_tab(id2);
        assert_eq!(state.selected_tab, Some(id1));
        assert_eq!(state.tabs.len(), 1);
    }
```

- [ ] **Step 9: 在 state.rs 的 #[cfg(test)] 区域加单测**

在 state.rs 末尾的 `#[cfg(test)]` 块内追加以下测试（找到末尾的 `}` 前面插入）：

```rust
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
        let mut state = AppState::with_hosts(vec![]);
        let tab_id = state.new_default_tab();
        state.close_tab(tab_id);
        assert!(state.tabs.is_empty(), "tabs should be empty after closing last tab");
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
```

- [ ] **Step 10: 运行测试，确认通过**

```bash
cargo test --workspace
```

预期：原有测试删/改后总数会小于 199，但编译通过且全绿。新加的 5 个单测（Step 9）也全过。

- [ ] **Step 11: 运行 fmt + clippy**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
```

预期：0 warning。如有 unused `TabContent::Default` 等 warning，加 `#[allow(dead_code)]` 处理（已在 views/mod.rs 顶层有全局 allow(dead_code)）。

- [ ] **Step 12: Commit**

```bash
git add crates/aish-app/src/state.rs
git commit -m "feat(state): 加 SidebarTab enum + sidebar 字段，M4a 信息架构地基

- 新增 SidebarTab { Home, Terminal, Inbox, Settings }（Default 实现 = Home）
- AppState 加 sidebar 字段，with_hosts 初始值 Home，tabs 起始为空
- close_tab 不再 fallback 新建 Default tab（tabs 可以为空）
- remove_connection 改为关闭相关 tab 而非变成 Default
- Connection 加 humanize_opened_at()（just now / Xm / Xh / yesterday / Xd）
- 修改/删除 5 个受影响的现有测试，新增 5 个单测

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: theme.rs — 加 sidebar 常量

**Files:**
- Modify: `crates/aish-app/src/theme.rs`

- [ ] **Step 1: 在 theme.rs 追加 sidebar 常量**

在 `CHIP_GREEN_BG` 常量后（约 L42），追加：

```rust
// ────────── Sidebar 4-tab（M4a 信息架构）──────────
/// 左侧 48px sidebar 背景（比 BG_BASE 再深一档）。
pub const SIDEBAR_BG: u32 = 0x0a0a0c;
/// sidebar 右边框。
pub const SIDEBAR_BORDER: u32 = 0x1f1f23;
/// active tab 左侧 2px 指示条颜色（= ACCENT_BLUE）。
pub const SIDEBAR_ACTIVE_BAR: u32 = ACCENT_BLUE;
/// sidebar nav icon 默认色（暗灰）。
pub const SIDEBAR_NAV_FG_DEFAULT: u32 = 0x6b6b73;
/// sidebar nav icon hover 色。
pub const SIDEBAR_NAV_FG_HOVER: u32 = 0x9a9aa3;
/// sidebar nav icon active 色（白）。
pub const SIDEBAR_NAV_FG_ACTIVE: u32 = 0xffffff;
/// sidebar nav active 背景（微亮底）。
pub const SIDEBAR_NAV_BG_ACTIVE: u32 = 0x15151a;
/// sidebar 宽度像素值。
pub const SIDEBAR_WIDTH: f32 = 48.0;
```

- [ ] **Step 2: 运行 fmt + cargo check**

```bash
cargo +nightly fmt --all
cargo check --workspace
```

预期：编译通过，0 error。

- [ ] **Step 3: Commit**

```bash
git add crates/aish-app/src/theme.rs
git commit -m "feat(theme): 加 sidebar 4-tab 相关常量（M4a）

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: views/sidebar_nav.rs — 新建 SidebarNav

**Files:**
- Create: `crates/aish-app/src/views/sidebar_nav.rs`

**背景知识：**
GPUI 里 SVG icon 用 `svg()` + `.path(cx.asset_source()...)` 读取 asset，但 aish 目前没有 icon asset。用 SVG inline path 字符串的方式不直接支持。最简单的替代是用 Unicode 字符做占位 icon（`⌂ ▶ ✉ ⚙`），等未来 icon asset 系统建立后替换。这不影响功能验收。

- [ ] **Step 1: 创建 sidebar_nav.rs**

新建文件 `crates/aish-app/src/views/sidebar_nav.rs`，内容：

```rust
//! SidebarNav：左侧 48px 纯 icon 4-tab 导航（M4a 信息架构）。
//!
//! 4 个 tab：Home / Terminal / Inbox / Settings。
//! 选中态：左侧 2px ACCENT_BLUE 指示条 + 背景 SIDEBAR_NAV_BG_ACTIVE + icon 变白。
//! icon 暂用 Unicode 占位（⌂ ▶ ✉ ⚙），未来换 SVG asset 时只需改本文件。

use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};
use crate::theme;

pub struct SidebarNavView {
    state: Entity<AppState>,
}

impl SidebarNavView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }

    fn handle_click(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.sidebar = tab;
            cx.notify();
        });
    }
}

impl Render for SidebarNavView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.state.read(cx).sidebar;

        let nav_item = |tab: SidebarTab, icon: &'static str, cx: &mut Context<SidebarNavView>| {
            let is_active = current == tab;
            let fg = if is_active {
                rgb(theme::SIDEBAR_NAV_FG_ACTIVE)
            } else {
                rgb(theme::SIDEBAR_NAV_FG_DEFAULT)
            };

            let mut item = div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .py(px(10.0))
                .cursor_pointer()
                .text_color(fg)
                .text_size(px(18.0));

            // active 态：左侧指示条 + 背景
            if is_active {
                item = item
                    .bg(rgb(theme::SIDEBAR_NAV_BG_ACTIVE))
                    .border_l_2()
                    .border_color(rgb(theme::SIDEBAR_ACTIVE_BAR));
            } else {
                // 非 active：透明左边框占位，保持宽度一致
                item = item
                    .border_l_2()
                    .border_color(gpui::transparent_black());
            }

            item = item
                .hover(|s| {
                    if current != tab {
                        s.text_color(rgb(theme::SIDEBAR_NAV_FG_HOVER))
                    } else {
                        s
                    }
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                        this.handle_click(tab, cx);
                    }),
                )
                .child(icon);

            item
        };

        div()
            .w(px(theme::SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(theme::SIDEBAR_BORDER))
            .child(nav_item(SidebarTab::Home, "⌂", cx))
            .child(nav_item(SidebarTab::Terminal, ">_", cx))
            .child(nav_item(SidebarTab::Inbox, "✉", cx))
            .child(
                // Settings 推到底部
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(nav_item(SidebarTab::Settings, "⚙", cx)),
            )
    }
}
```

- [ ] **Step 2: 在 views/mod.rs 暂时加模块声明（不 pub use，先让编译能看到）**

打开 `crates/aish-app/src/views/mod.rs`，在 `mod default_page;` 上方加：

```rust
mod sidebar_nav;
```

（先不 pub use，Task 7 统一处理 exports。）

- [ ] **Step 3: 运行 cargo check**

```bash
cargo check --workspace
```

预期：编译通过。如有 `transparent_black` 不存在，用 `gpui::black().opacity(0.0)` 替代，或者简单用一个非常深的颜色让边框"看不见"：`rgb(theme::SIDEBAR_BG)`。

- [ ] **Step 4: Commit**

```bash
git add crates/aish-app/src/views/sidebar_nav.rs crates/aish-app/src/views/mod.rs
git commit -m "feat(ui): 新增 SidebarNavView 48px icon sidebar（M4a）

Unicode 占位 icon（⌂ >_ ✉ ⚙），active 态左侧蓝色指示条 + 亮背景。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: views/home.rs — HomeView

**Files:**
- Create: `crates/aish-app/src/views/home.rs`

**背景知识：**
HomeView 接管 default_page.rs 里所有的 host 卡片渲染逻辑（avatar / chip / edit / delete / 点击连接），同时新增 Active Sessions 区和 Quick Actions 区。

host 卡片点击后需要：
1. `state.open_connection(host_id)` 拿 conn_id
2. push 新 tab（不再用 `replace_current_tab`，因为 Home 里没有当前 tab 可替换）
3. `state.sidebar = Terminal`
4. `bridge.spawn_session` + `register_session`

Active Sessions 区点击 "Open" 按钮需要：
1. 在 tabs 里找到 content=Connection(conn_id) 的 tab，select_tab
2. 若找不到（tab 被关了但 conn 还在），push 一个新 tab
3. `state.sidebar = Terminal`

- [ ] **Step 1: 创建 home.rs**

新建 `crates/aish-app/src/views/home.rs`：

```rust
//! HomeView：4-tab 架构的 Home tab（M4a 信息架构）。
//!
//! 包含：Quick Actions（+ 添加 host）、Active Sessions（活跃连接列表）、
//! Hosts grid（host 卡片网格，复用 default_page.rs 原有逻辑）。

use std::sync::Arc;

use aish_types::{HostId, TabId};
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, ConnectionId, HostFormDraft, HostFormState, SidebarTab, SshEvent, Tab, TabContent};
use crate::theme;

pub struct HomeView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HomeView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_add_click(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.modal = Some(HostFormState::Adding(HostFormDraft::default()));
            cx.notify();
        });
    }

    fn handle_card_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        let (conn_id, config, label) = self.state.update(cx, |s, cx| {
            let conn = s.open_connection(host_id);
            let cfg = s.hosts.iter().find(|h| h.id == host_id).cloned();
            let label = s
                .connections
                .get(&conn)
                .map(|c| c.label.clone())
                .unwrap_or_default();
            // 新建 tab 并选中
            let tab = Tab {
                id: TabId::new(),
                content: TabContent::Connection(conn),
                title: label.clone(),
            };
            s.tabs.push(tab);
            s.selected_tab = Some(s.tabs.last().unwrap().id);
            // 自动切到 Terminal
            s.sidebar = SidebarTab::Terminal;
            cx.notify();
            (conn, cfg, label)
        });

        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!(?host_id, "home: host config not found");
                return;
            }
        };
        tracing::info!(?conn_id, %label, "home: spawn connection");

        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |s, _cx| {
            s.register_session(conn_id, sender);
        });
    }

    fn handle_edit_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            if let Some(cfg) = s.hosts.iter().find(|h| h.id == host_id).cloned() {
                s.modal = Some(HostFormState::Editing {
                    id: host_id,
                    draft: HostFormDraft::from_config(&cfg),
                });
                cx.notify();
            }
        });
    }

    fn handle_delete_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            if let Some(cfg) = s.hosts.iter().find(|h| h.id == host_id).cloned() {
                s.modal = Some(HostFormState::DeleteConfirm {
                    id: host_id,
                    label: cfg.label,
                });
                cx.notify();
            }
        });
    }

    fn handle_open_session(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            // 找到对应 tab，若找不到则新建
            let tab_id = s
                .tabs
                .iter()
                .find(|t| t.content == TabContent::Connection(conn_id))
                .map(|t| t.id);

            if let Some(id) = tab_id {
                s.selected_tab = Some(id);
            } else {
                // tab 已被关闭但 conn 还在：新建一个 tab
                let label = s
                    .connections
                    .get(&conn_id)
                    .map(|c| c.label.clone())
                    .unwrap_or_else(|| "connection".into());
                let tab = Tab {
                    id: TabId::new(),
                    content: TabContent::Connection(conn_id),
                    title: label,
                };
                s.tabs.push(tab);
                s.selected_tab = Some(s.tabs.last().unwrap().id);
            }
            s.sidebar = SidebarTab::Terminal;
            cx.notify();
        });
    }
}

impl Render for HomeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);

        // ─── Quick Actions 区 ───
        let add_btn = div()
            .px_4()
            .py_2()
            .text_size(theme::text_sm())
            .text_color(rgb(0xffffff))
            .bg(rgb(theme::ACCENT_BLUE))
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::ACCENT_BLUE_HOVER)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_add_click(cx)),
            )
            .child("+ 添加 host");

        // ─── Active Sessions 区 ───
        let active_sessions: Vec<_> = app
            .connections
            .values()
            .map(|conn| {
                let conn_id = conn.id;
                let label = conn.label.clone();
                let time_str = conn.humanize_opened_at();
                let is_alive = app.is_session_active(conn_id);
                let dot_color = if is_alive {
                    rgb(theme::ACCENT_GREEN)
                } else {
                    rgb(theme::TEXT_MUTED)
                };

                let open_btn = div()
                    .px_3()
                    .py_1()
                    .text_size(theme::text_xs())
                    .text_color(rgb(theme::ACCENT_BLUE))
                    .bg(rgb(theme::CHIP_BLUE_BG))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_SELECTED)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            this.handle_open_session(conn_id, cx);
                        }),
                    )
                    .child("Open ▶");

                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_open_session(conn_id, cx);
                        }),
                    )
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(dot_color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(rgb(theme::TEXT_PRIMARY))
                                    .text_size(theme::text_sm())
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .text_size(theme::text_xs())
                                    .child(format!("· {}", time_str)),
                            ),
                    )
                    .child(open_btn)
            })
            .collect();

        let active_section: Option<gpui::AnyElement> = if !app.connections.is_empty() {
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .px_4()
                            .pt_4()
                            .pb_1()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .text_size(theme::text_xs())
                            .child("ACTIVE SESSIONS"),
                    )
                    .children(active_sessions)
                    .into_any_element(),
            )
        } else {
            None
        };

        // ─── Hosts grid 区 ───
        let cards: Vec<_> = app
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let host_text = format!("{}@{}:{}", h.user, h.host, h.port);
                let active_count = app.connections.values().filter(|c| c.host_id == id).count();

                let initial = label
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let avatar_bg = theme::avatar_color_for(&label);
                let avatar = div()
                    .w(px(40.0))
                    .h(px(40.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(avatar_bg))
                    .rounded_xl()
                    .text_color(rgb(0xffffff))
                    .text_size(theme::text_lg())
                    .child(initial);

                let chip = div()
                    .px_2p5()
                    .py_0p5()
                    .text_size(theme::text_xs())
                    .text_color(rgb(theme::ACCENT_BLUE))
                    .bg(rgb(theme::CHIP_BLUE_BG))
                    .rounded_full()
                    .child("SSH");

                let active_chip: Option<gpui::AnyElement> = if active_count > 0 {
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .px_2p5()
                            .py_0p5()
                            .text_size(theme::text_xs())
                            .text_color(rgb(theme::ACCENT_GREEN))
                            .bg(rgb(theme::CHIP_GREEN_BG))
                            .rounded_full()
                            .child(div().text_color(rgb(theme::ACCENT_GREEN)).child("●"))
                            .child(format!("{} 活跃", active_count))
                            .into_any_element(),
                    )
                } else {
                    None
                };

                let edit_btn = div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .cursor_pointer()
                    .hover(|s| {
                        s.text_color(rgb(theme::TEXT_PRIMARY))
                            .bg(rgb(theme::BG_SELECTED))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            this.handle_edit_click(id, cx);
                        }),
                    )
                    .child("✎");

                let delete_btn = div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .cursor_pointer()
                    .hover(|s| {
                        s.text_color(rgb(theme::ACCENT_RED))
                            .bg(rgb(theme::BG_SELECTED))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            this.handle_delete_click(id, cx);
                        }),
                    )
                    .child("×");

                let actions = div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .opacity(0.0)
                    .group_hover("host_card", |s| s.opacity(1.0))
                    .child(edit_btn)
                    .child(delete_btn);

                let chevron = div()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .text_size(theme::text_lg())
                    .child("›");

                div()
                    .group("host_card")
                    .px_4()
                    .py_3p5()
                    .bg(rgb(theme::BG_ELEVATED))
                    .rounded_2xl()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_card_click(id, cx);
                        }),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(avatar)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(rgb(theme::TEXT_PRIMARY))
                                            .text_size(theme::text_lg())
                                            .child(label),
                                    )
                                    .child(chip)
                                    .children(active_chip),
                            )
                            .child(
                                div()
                                    .text_color(rgb(theme::TEXT_SECONDARY))
                                    .text_size(theme::text_sm())
                                    .child(host_text),
                            ),
                    )
                    .child(actions)
                    .child(chevron)
            })
            .collect();

        let empty_hint: Option<gpui::AnyElement> = if app.hosts.is_empty() {
            Some(
                div()
                    .px_4()
                    .py_8()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .text_size(theme::text_sm())
                    .child("还没有保存的连接 — 点上方 + 添加 host 开始")
                    .into_any_element(),
            )
        } else {
            None
        };

        // ─── 整体布局 ───
        div()
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .flex()
            .flex_col()
            .overflow_y_scroll()
            // Quick Actions
            .child(
                div()
                    .px_8()
                    .pt_6()
                    .pb_3()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_color(rgb(theme::TEXT_PRIMARY))
                            .text_size(theme::text_xl())
                            .child("Home"),
                    )
                    .child(add_btn),
            )
            // Active Sessions（可选）
            .children(active_section)
            // Hosts 标题
            .child(
                div()
                    .px_4()
                    .pt_4()
                    .pb_1()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .text_size(theme::text_xs())
                    .child("HOSTS"),
            )
            // Hosts grid
            .child(
                div()
                    .px_8()
                    .pb_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(cards)
                    .children(empty_hint),
            )
    }
}
```

- [ ] **Step 2: 在 views/mod.rs 加模块声明**

在 `mod sidebar_nav;` 下面加：

```rust
mod home;
```

- [ ] **Step 3: 运行 cargo check**

```bash
cargo check --workspace
```

预期：编译通过。如果有 `ConnectionId` import 问题，确认 `crate::state::ConnectionId` 路径，或者从 `aish_types` 导入。

- [ ] **Step 4: Commit**

```bash
git add crates/aish-app/src/views/home.rs crates/aish-app/src/views/mod.rs
git commit -m "feat(ui): 新增 HomeView（hosts grid + active sessions + quick actions）

接管 default_page.rs 的 host 卡片渲染逻辑，新增 Active Sessions 区。
点 host 卡片 / Open 按钮均自动切换 sidebar = Terminal。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: views/empty_terminal.rs + views/coming_soon.rs

**Files:**
- Create: `crates/aish-app/src/views/empty_terminal.rs`
- Create: `crates/aish-app/src/views/coming_soon.rs`

- [ ] **Step 1: 创建 empty_terminal.rs**

新建 `crates/aish-app/src/views/empty_terminal.rs`：

```rust
//! EmptyTerminalGuideView：sidebar=Terminal 且无任何会话时的引导页（M4a）。

use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};
use crate::theme;

pub struct EmptyTerminalGuideView {
    state: Entity<AppState>,
}

impl EmptyTerminalGuideView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }
}

impl Render for EmptyTerminalGuideView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let go_home_btn = div()
            .px_6()
            .py_2()
            .text_size(theme::text_sm())
            .text_color(rgb(0xffffff))
            .bg(rgb(theme::ACCENT_BLUE))
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::ACCENT_BLUE_HOVER)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.state.update(cx, |s, cx| {
                        s.sidebar = SidebarTab::Home;
                        cx.notify();
                    });
                }),
            )
            .child("Go to Home");

        div()
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_size(px(40.0))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(">_"),
            )
            .child(
                div()
                    .text_size(theme::text_xl())
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child("No active sessions yet"),
            )
            .child(
                div()
                    .text_size(theme::text_sm())
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child("Pick a host from Home to get started."),
            )
            .child(go_home_btn)
    }
}
```

- [ ] **Step 2: 创建 coming_soon.rs**

新建 `crates/aish-app/src/views/coming_soon.rs`：

```rust
//! ComingSoonView：Inbox / Settings tab 占位页（M4a）。

use gpui::{div, prelude::*, px, rgb, Context, Window};

use crate::theme;

#[derive(Clone, Copy)]
pub enum ComingSoonKind {
    Inbox,
    Settings,
}

pub struct ComingSoonView {
    kind: ComingSoonKind,
}

impl ComingSoonView {
    pub fn new(kind: ComingSoonKind) -> Self {
        Self { kind }
    }
}

impl Render for ComingSoonView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (icon, title, description) = match self.kind {
            ComingSoonKind::Inbox => (
                "✉",
                "Inbox · Coming soon",
                "Agent events, tool completions, and approval requests will appear here.",
            ),
            ComingSoonKind::Settings => (
                "⚙",
                "Settings · Coming soon",
                "Appearance, input, notifications, and host defaults — coming in a future update.",
            ),
        };

        div()
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_size(px(40.0))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(icon),
            )
            .child(
                div()
                    .text_size(theme::text_xl())
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(360.0))
                    .text_size(theme::text_sm())
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .text_align(gpui::TextAlign::Center)
                    .child(description),
            )
            .child(
                div()
                    .text_size(theme::text_xs())
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("See roadmap-moshi-desktop.md for the full plan."),
            )
    }
}
```

- [ ] **Step 3: 在 views/mod.rs 加模块声明**

在 `mod home;` 下面加：

```rust
mod empty_terminal;
mod coming_soon;
```

- [ ] **Step 4: 运行 cargo check**

```bash
cargo check --workspace
```

预期：编译通过。

- [ ] **Step 5: Commit**

```bash
git add crates/aish-app/src/views/empty_terminal.rs crates/aish-app/src/views/coming_soon.rs crates/aish-app/src/views/mod.rs
git commit -m "feat(ui): 新增 EmptyTerminalGuideView + ComingSoonView（M4a 占位页）

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: views/mod.rs — 统一 exports + app.rs — RootView 重构

**Files:**
- Modify: `crates/aish-app/src/views/mod.rs`
- Modify: `crates/aish-app/src/app.rs`

- [ ] **Step 1: 更新 views/mod.rs**

将 `crates/aish-app/src/views/mod.rs` 全部替换为：

```rust
//! GPUI Views。

#![allow(dead_code)]

mod coming_soon;
mod empty_terminal;
mod home;
mod host_form;
mod session_picker;
mod sidebar_nav;
mod tab_bar;
mod terminal_view;
// tmux_sidebar：M3c 起废弃（功能被 SessionPickerView 弹窗取代）。
#[allow(dead_code)]
mod tmux_sidebar;

pub use coming_soon::{ComingSoonKind, ComingSoonView};
pub use empty_terminal::EmptyTerminalGuideView;
pub use home::HomeView;
pub use host_form::HostFormModal;
pub use session_picker::SessionPickerView;
pub use sidebar_nav::SidebarNavView;
pub use tab_bar::TabBarView;
pub use terminal_view::TerminalView;
```

（不导出 default_page——该文件还存在但 Task 10 会删掉，这里先不 pub use 以便编译器在 Task 10 之前也不依赖它。mod default_page 暂保留一行，Task 10 时再删。）

实际上为避免 Task 10 时有 dangling mod，这里直接**不加** `mod default_page;`——因为 app.rs 重构后也不再引用 DefaultPageView。

- [ ] **Step 2: 重构 app.rs — RootView**

将 `crates/aish-app/src/app.rs` 全部替换为：

```rust
//! aish GPUI 主应用入口。

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{AppState, SidebarTab, SshEvent};

pub fn run() {
    let bridge_owner = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));
    let bridge_keep = bridge_owner.clone();

    application().run(move |cx: &mut App| {
        crate::terminal::font::register_bundled_font(cx);
        let hosts = match crate::persistence::load_hosts() {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("load hosts.json failed: {} — starting with empty list", e);
                Vec::new()
            }
        };
        let state = cx.new(|_cx| AppState::with_hosts(hosts));
        let channel = EventChannel::new();

        // 接收 SshEvent loop
        let state_for_loop = state.clone();
        let mut rx = channel.rx;
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                state_for_loop.update(cx, |state, cx| match event {
                    SshEvent::Connected { conn: _ } => {
                        cx.notify();
                    }
                    SshEvent::PaneOutput { conn, bytes } => {
                        state.feed_bytes(conn, &bytes);
                        cx.notify();
                    }
                    SshEvent::Disconnected { conn, reason: _ } => {
                        state.drop_session(conn);
                        cx.notify();
                    }
                    SshEvent::Error { conn, kind: _, msg } => {
                        tracing::error!(?conn, msg, "SSH error");
                        state.drop_session(conn);
                        cx.notify();
                    }
                    SshEvent::TmuxQueryStarted { conn } => {
                        state
                            .tmux_state
                            .insert(conn, crate::state::TmuxState::NotChecked);
                        cx.notify();
                    }
                    SshEvent::TmuxSessionsListed { conn, sessions } => {
                        let has_sessions = !sessions.is_empty();
                        state.tmux_state.insert(
                            conn,
                            crate::state::TmuxState::Detected {
                                sessions,
                                attached: None,
                            },
                        );
                        if has_sessions && state.current_connection() == Some(conn) {
                            state.pending_session_picker = Some(conn);
                        }
                        cx.notify();
                    }
                    SshEvent::TmuxQueryFailed { conn, msg } => {
                        state
                            .tmux_state
                            .insert(conn, crate::state::TmuxState::QueryFailed { msg });
                        cx.notify();
                    }
                    SshEvent::TmuxNoTmux { conn } => {
                        state
                            .tmux_state
                            .insert(conn, crate::state::TmuxState::NoTmux);
                        cx.notify();
                    }
                    SshEvent::TmuxAttached { conn, session } => {
                        state.mark_tmux_attached(conn, session);
                        cx.notify();
                    }
                });
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("aish")),
                ..Default::default()
            }),
            ..Default::default()
        };

        let bridge_for_window = bridge_owner.clone();
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
        .expect("主窗口应能打开");

        cx.activate(true);
    });

    drop(bridge_keep);
}

/// 根视图。布局：左侧 SidebarNav（48px）+ 右侧主区（按 sidebar 分支）。
/// HostFormModal / SessionPickerView 作为顶层叠加 modal。
struct RootView {
    state: Entity<AppState>,
    sidebar_nav: Entity<crate::views::SidebarNavView>,
    tab_bar: Entity<crate::views::TabBarView>,
    home: Entity<crate::views::HomeView>,
    terminal: Entity<crate::views::TerminalView>,
    empty_terminal: Entity<crate::views::EmptyTerminalGuideView>,
    inbox: crate::views::ComingSoonView,
    settings: crate::views::ComingSoonView,
    host_form: Entity<crate::views::HostFormModal>,
    session_picker: Entity<crate::views::SessionPickerView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();

        let sidebar_nav =
            cx.new(|cx| crate::views::SidebarNavView::new(state.clone(), cx));
        let tab_bar = cx
            .new(|cx| crate::views::TabBarView::new(state.clone(), bridge.clone(), tx.clone(), cx));
        let home = cx.new(|cx| {
            crate::views::HomeView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let terminal = cx.new(|cx| {
            crate::views::TerminalView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let empty_terminal =
            cx.new(|cx| crate::views::EmptyTerminalGuideView::new(state.clone(), cx));
        let host_form = cx.new(|cx| {
            crate::views::HostFormModal::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let session_picker = cx.new(|cx| {
            crate::views::SessionPickerView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });

        Self {
            state,
            sidebar_nav,
            tab_bar,
            home,
            terminal,
            empty_terminal,
            inbox: crate::views::ComingSoonView::new(crate::views::ComingSoonKind::Inbox),
            settings: crate::views::ComingSoonView::new(crate::views::ComingSoonKind::Settings),
            host_form,
            session_picker,
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let sidebar = app.sidebar;
        let modal_open = app.modal.is_some();
        let picker_open = app.pending_session_picker.is_some();
        let tabs_empty = app.tabs.is_empty();
        drop(app);

        // 主区内容：按 sidebar 分支
        let main_body: gpui::AnyElement = match sidebar {
            SidebarTab::Home => self.home.clone().into_any_element(),
            SidebarTab::Terminal => {
                if tabs_empty {
                    self.empty_terminal.clone().into_any_element()
                } else {
                    // tab_bar + terminal 竖排
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .child(self.tab_bar.clone())
                        .child(div().flex_1().child(self.terminal.clone()))
                        .into_any_element()
                }
            }
            SidebarTab::Inbox => cx
                .new(|_cx| crate::views::ComingSoonView::new(crate::views::ComingSoonKind::Inbox))
                .into_any_element(),
            SidebarTab::Settings => cx
                .new(|_cx| {
                    crate::views::ComingSoonView::new(crate::views::ComingSoonKind::Settings)
                })
                .into_any_element(),
        };

        // 外层：sidebar + 主区横排
        let main = div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x000000))
            .child(self.sidebar_nav.clone())
            .child(div().flex_1().child(main_body));

        let mut root = div().relative().size_full().child(main);

        if picker_open {
            root = root.child(self.session_picker.clone());
        }
        if modal_open {
            root = root.child(self.host_form.clone());
        }

        root
    }
}
```

**注意**：`ComingSoonView` 不实现 `Clone`，所以 Inbox/Settings 每次 render 时用 `cx.new` 新建——这样足够简单且 GPUI 会正确管理生命周期。将 struct 里的 `inbox` / `settings` 字段删掉（改为 render 里动态 cx.new）。上面代码已经改好了（RootView struct 里没有 inbox/settings 字段）。

- [ ] **Step 3: 运行 cargo check**

```bash
cargo check --workspace
```

预期：编译通过。常见错误：
- `SidebarNavView::new` 只需要 `state`（不需要 bridge/tx）——check 一遍签名
- `EmptyTerminalGuideView` 类似
- `ComingSoonView::new` 参数是 `ComingSoonKind`，不需要 state

如果 `drop(app)` 出现 borrow conflict，把 `let tabs_empty = ...` 这行从 `app` 里读完，然后提前 drop。

- [ ] **Step 4: 运行 fmt + clippy**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/aish-app/src/views/mod.rs crates/aish-app/src/app.rs
git commit -m "feat(ui): RootView 重构为 SidebarNav + 4-tab 主区（M4a）

- 左侧 SidebarNavView + 主区按 sidebar 分支（Home/Terminal/Inbox/Settings）
- sidebar=Terminal + tabs 为空 → EmptyTerminalGuideView
- sidebar=Terminal + 有 tabs → TabBar + TerminalView 竖排
- sidebar=Inbox/Settings → ComingSoonView

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: views/tab_bar.rs — + 按钮改为 sidebar→Home + 删除 default_page.rs

**Files:**
- Modify: `crates/aish-app/src/views/tab_bar.rs`
- Delete: `crates/aish-app/src/views/default_page.rs`
- Modify: `crates/aish-app/src/state.rs` (删 new_default_tab)

- [ ] **Step 1: 修改 tab_bar.rs 的 handle_new_tab**

找到 `fn handle_new_tab` 方法（约 L180）：

将：
```rust
    fn handle_new_tab(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.new_default_tab();
            cx.notify();
        });
    }
```

改为：
```rust
    fn handle_new_tab(&mut self, cx: &mut Context<Self>) {
        // M4a：+ 按钮切回 Home，让用户从 Home 选 host 开始新连接
        self.state.update(cx, |s, cx| {
            s.sidebar = crate::state::SidebarTab::Home;
            cx.notify();
        });
    }
```

- [ ] **Step 2: 删除 default_page.rs**

```bash
rm crates/aish-app/src/views/default_page.rs
```

（Windows PowerShell：`Remove-Item crates/aish-app/src/views/default_page.rs`）

- [ ] **Step 3: 删除 state.rs 里 new_default_tab（及其 #[allow(dead_code)] 注解）**

找到 `new_default_tab` 方法整体（约 L359-L369），删除这整段：

```rust
    #[allow(dead_code)] // Task 9 改完 tab_bar 的 + 按钮后删
    pub fn new_default_tab(&mut self) -> TabId {
        let tab = Tab {
            id: TabId::new(),
            content: TabContent::Default,
            title: "新连接".into(),
        };
        let id = tab.id;
        self.tabs.push(tab);
        self.selected_tab = Some(id);
        id
    }
```

同时把 state.rs 里调用 `self.new_default_tab()` 的 close_tab fallback（Task 1 已改成注释）也确认已删净。

- [ ] **Step 4: 检查 state.rs tests 里是否引用 new_default_tab**

找到 test 里的 `new_default_tab_pushes_and_selects` 等测试函数，全部删除（这些测试测的是已删方法）：

删除 state.rs #[cfg(test)] 块里这些测试函数：
- `new_default_tab_pushes_and_selects`（如果存在）
- 任何其他调用 `new_default_tab` 的测试

Task 1 Step 3 写的 `close_tab_allows_empty_tabs` 内部调用了 `new_default_tab()`——需要改：

```rust
    #[test]
    fn close_tab_allows_empty_tabs() {
        use aish_types::TabId;
        use crate::state::TabContent;
        let mut state = AppState::with_hosts(vec![]);
        // 手动 push 一个 tab 再关掉
        let tab_id = TabId::new();
        state.tabs.push(crate::state::Tab {
            id: tab_id,
            content: TabContent::Default,
            title: "test".into(),
        });
        state.selected_tab = Some(tab_id);
        state.close_tab(tab_id);
        assert!(state.tabs.is_empty(), "tabs should be empty after closing last tab");
    }
```

- [ ] **Step 5: 运行 cargo check + tests**

```bash
cargo check --workspace
cargo test --workspace
```

预期：编译通过，测试全过。

- [ ] **Step 6: 运行 fmt + clippy**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
```

预期：0 warning。

- [ ] **Step 7: Commit**

```bash
git add crates/aish-app/src/views/tab_bar.rs crates/aish-app/src/state.rs
git rm crates/aish-app/src/views/default_page.rs
git commit -m "refactor(ui): 删除 DefaultPageView + 清理 new_default_tab（M4a）

- tab_bar 的 + 按钮改为切 sidebar=Home
- 删除 default_page.rs（逻辑已迁移到 home.rs）
- state.rs 删除 new_default_tab 方法
- 更新相关单测

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: 质量门禁 + 手动验收

**Files:** 无新增，只跑命令

- [ ] **Step 1: 完整质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：fmt 无改动、clippy 0 warning、测试全过（199 + N 个）。

- [ ] **Step 2: 启动应用手动验收**

```bash
cargo run -p aish-app
```

逐条检查（对照 spec §10）：
- [ ] 左侧 48px sidebar 可见，Home icon 高亮（蓝色左侧指示条）
- [ ] 点 Home：主区显示 Hosts grid + "Home" 标题 + "+ 添加 host" 按钮
- [ ] 点 "+ 添加 host"：HostForm modal 弹出
- [ ] 点某个 host 卡片：sidebar 自动切到 Terminal，顶部会话 tab 出现，终端开始连接
- [ ] 手动切回 Home sidebar：Active Sessions 区显示该连接（绿点 + label + "just now" + Open ▶）
- [ ] 点 Active Sessions 的 "Open ▶"：切回 Terminal sidebar，激活对应 tab
- [ ] 点 sidebar Inbox：ComingSoon 页（✉ 图标 + 说明文字）
- [ ] 点 sidebar Settings：ComingSoon 页（⚙ 图标 + 说明文字）
- [ ] 关闭所有会话 tab，保持 sidebar=Terminal：显示 EmptyTerminalGuideView（>_ 图标 + "No active sessions yet"）
- [ ] 点 EmptyTerminalGuideView 的 [Go to Home]：sidebar 切回 Home
- [ ] Terminal tab 里的 + 按钮：切回 Home（而不是新建空 tab）
- [ ] 现有功能回归：host 持久化（重启后 hosts 仍在）、keyring 密钥读取、tmux attach 流程、Ctrl+Shift+V 粘贴

- [ ] **Step 3: 更新 INDEX.md**

将 INDEX.md 中 M4a 的状态从 "🟡 进行中" 改为 "✅ 已完成"，并补充实际产出：

```markdown
### M4a — 信息架构 4-tab 化（2026-05-08）— ✅ 已完成
- spec：...
- plan：[`plans/2026-05-08-aish-m4a-info-arch.md`](plans/2026-05-08-aish-m4a-info-arch.md)
- 实际产出：SidebarNav 48px + HomeView(hosts+active+quick) + EmptyTerminalGuideView
  + ComingSoonView(Inbox/Settings) + RootView 重构
- 关键 commits：...（填入实际 hash）
```

- [ ] **Step 4: 更新 roadmap-moshi-desktop.md**

将子项目 A M4a 行状态改为 ✅，填入关联 milestone 和完成日期。

- [ ] **Step 5: 最终 commit**

```bash
git add docs/superpowers/INDEX.md docs/superpowers/roadmap-moshi-desktop.md
git commit -m "docs: M4a 信息架构完成，更新 INDEX + roadmap 状态

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```
