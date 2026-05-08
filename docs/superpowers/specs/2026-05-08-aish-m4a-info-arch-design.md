# aish M4a 信息架构 4-tab 化 — Design Spec

> 属于 [桌面版 Moshi Roadmap](../roadmap-moshi-desktop.md) · 子项目 **A** · M4a 第一刀。

**Goal**：把 aish 顶层导航重组为 Moshi 风的 4-tab 信息架构（左侧 48px 纯 icon
侧边栏 + 顶部会话 tab bar），为后续 AI agent 子项目（Inbox 事件中心 / Activity
活动条 / 图片粘贴等）准备 UI 容器。**仅做地基**，不集成任何 agent 协议。

**Non-Goal**：
- 不做 AI agent 检测 / Activity / Inbox 实质内容（→ 子项目 B/C/D，后续 milestone）
- 不做图片粘贴 / 语音 / Token 用量（→ 子项目 E/F/G）
- 不做 Recent 持久化（→ M4b）
- 不做 theme / 字体可配置（→ 后续）
- 不动现有 SSH/tmux 行为，仅 UI 层重组
- 不实现键盘快捷键 Ctrl+1..4 切 4-tab（避开 keymap 冲突调研，记入 roadmap）

**用户决策**：
- 方向：桌面版 Moshi
- 第一个 milestone 选定：**A. 4-tab 信息架构**（其他子项目 backlog 化）
- 4-tab 安置：**外层侧边栏 4-tab + 顶部保留会话 tab**（顶部 tab bar 仅 sidebar=Terminal 时可见）
- 侧边栏样式：**A · 48px 纯 icon · VS Code 风**（hover tooltip 显示名称，蓝色左侧 active 指示条）
- Home 区块（本 milestone 全选）：**Hosts grid + Active Sessions + Quick Action**
- 范围：**瘦 M4a**（Recent / Settings 实质内容推后）
- Empty Terminal 行为：**显示引导卡片**（"还没有会话，去 Home 选一个 host 开始"）
- Inbox / Settings：**ComingSoon placeholder**（图标 + 简介 + 链回 roadmap）
- last-active 时间：**humanize 字符串**（"30m ago / 2h ago / yesterday"）
- 长期 roadmap：单独写到 `docs/superpowers/roadmap-moshi-desktop.md`

---

## 1. 现状盘点

| 元素 | 现状 | 本 milestone 命运 |
|---|---|---|
| `app.rs::RootView` | TabBar + body（按 current_tab.content 切 DefaultPage / Terminal） + overlay | 重构：左侧 SidebarNav + 主区按 sidebar 分支 |
| `views/tab_bar.rs` | 顶部 tab bar，全局可见 | 仅在 sidebar=Terminal 时渲染（外部 if 控制） |
| `views/default_page.rs` | host 卡片网格 + "+ 添加 host" + 点击启动连接 | **删除**（host 卡片渲染抽到 home.rs；TabContent::Default 不再可达） |
| `views/host_form.rs` | host 表单 modal | 不动（modal 跨 4-tab 都浮顶层） |
| `views/session_picker.rs` | tmux session picker modal | 不动 |
| `views/terminal_view.rs` | TerminalView | 不动 |
| `views/tmux_sidebar.rs` | tmux 会话侧栏 | 不动（sidebar=Terminal 内部行为不变） |
| `state.rs::AppState` | tabs / current_tab / connections / hosts / modal | 加 `sidebar: SidebarTab` |
| `state.rs::ConnectionRuntime` | 已有运行时字段 | 加 `created_at: Instant` 用于 Active Sessions 排序 + last-active 显示 |
| `state.rs::TabContent::Default` | 默认 tab 标记 | **保留**枚举值 + `#[allow(dead_code)]`（Recent 等可能复用） |

---

## 2. 信息架构

```
┌──────┬─────────────────────────────────────────────┐
│  ⌂   │   顶部会话 tab bar                          │
│      │   （仅 sidebar=Terminal 时可见）            │
│ >_   ├─────────────────────────────────────────────┤
│      │                                             │
│ ✉    │              主内容区                       │
│      │   按 sidebar 当前选中分支渲染：             │
│      │   Home / Terminal / Inbox / Settings        │
│      │                                             │
│ ⚙    │                                             │
└──────┴─────────────────────────────────────────────┘
 48px               flex
```

### 2.1 SidebarNav

**视觉规范**：
- 宽度：48px（精确像素，不缩放）
- 背景：`#0a0a0c`（比 RootView base 更深一档）
- 右边框：`1px solid #1f1f23`
- 4 个 nav button：
  - 高度：自适应 padding（垂直 9-10px）
  - icon：18×18 stroke 风 SVG
  - 颜色：默认 `#6b6b73`，active `#ffffff`，hover `#9a9aa3`
  - active 态：左侧 2px 宽 `#4a9eff` 指示条 + 背景 `#15151a`
- icon 选择：
  - Home → 房子（轮廓）
  - Terminal → `>_`（箭头 + 下划线）
  - Inbox → 收件箱 / 信封
  - Settings → 齿轮
- hover tooltip：显示名称（GPUI 已有 tooltip 能力？如无，本 milestone 不做 tooltip，留 backlog）

**键位**：本 milestone 不绑定（roadmap 记 Ctrl+1..4 后续做）。

### 2.2 顶部会话 tab bar

仅在 `sidebar = SidebarTab::Terminal` 时渲染。其他 sidebar 状态时主区直接占满
顶部空间。**实现细节**：在 RootView render 内加 `if sidebar == Terminal { tab_bar }`。

会话 tab 内部行为（点击切换、关闭、重命名、+ 新建）保持不变。

### 2.3 主内容区分支

| sidebar | 主区渲染 |
|---|---|
| `Home` | `HomeView` |
| `Terminal` + `tabs.is_empty()` | `EmptyTerminalGuideView` |
| `Terminal` + 有会话 | 现有 `current_tab.content` 渲染（DefaultPage 路径已死，实际只走 Connection） |
| `Inbox` | `ComingSoonView { kind: Inbox }` |
| `Settings` | `ComingSoonView { kind: Settings }` |

---

## 3. Home tab 详细设计

```
┌─────────────────────────────────────────────────────┐
│  Welcome / Quick Actions                            │
│  ┌─[+ Add host]─┐                                   │
│                                                     │
│  Active Sessions      ← 仅 ≥1 个 connection 时显示  │
│  ◉ vps-tokyo · 30m ago                  [Open ▶]    │
│  ◉ build-box · 5m ago                   [Open ▶]    │
│                                                     │
│  Hosts                                              │
│  ┌─card─┐ ┌─card─┐ ┌─card─┐                         │
│  │ vps  │ │build │ │home  │                         │
│  └──────┘ └──────┘ └──────┘                         │
└─────────────────────────────────────────────────────┘
```

### 3.1 Quick Actions 区
- "+ Add host" 主按钮（accent-blue 底，白字，圆角 8px），点击打开 HostForm modal（沿用现有逻辑）
- 本 milestone 仅此一个按钮；Roadmap 提到的 SSH config 导入 / 统计等推后

### 3.2 Active Sessions 区
- 标题 `Active Sessions`（label 风 14px 半透明）
- 列表项（每个 connection 一行）：
  - 左侧 8px 绿点（lit）/ 灰点（disconnected）
  - host label（primary 文本）
  - " · 30m ago"（secondary 文本，humanize 时间字符串）
  - 右侧 [Open ▶] 按钮
- 排序：按 `created_at` 倒序（最近建的在最上）
- 点行任意位置（除按钮）= 点 [Open ▶] = 切 sidebar=Terminal + 激活对应 conn 的会话 tab
- 区块仅当 `connections` 非空时渲染（连一个都没有时整块隐藏）

**humanize 时间**：写一个轻量函数 `humanize(elapsed: Duration) -> String`：
- `< 60s` → "just now"
- `< 60m` → "{n}m ago"
- `< 24h` → "{n}h ago"
- `< 48h` → "yesterday"
- 否则 → "{n}d ago"

不引入 `chrono` 依赖（standard `Instant::elapsed()` 够用）。

### 3.3 Hosts grid 区
- 完全复用现有 default_page.rs 的卡片渲染逻辑（迁移到 `home.rs::hosts_grid`）
- 点卡片行为：原 `handle_card_click`（启动 connection）+ 新加一行 `state.sidebar = Terminal`

---

## 4. 状态机 / 数据流

### 4.1 数据模型变化（state.rs）

```rust
/// 顶层 4-tab 当前选中项
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SidebarTab {
    #[default]
    Home,
    Terminal,
    Inbox,
    Settings,
}

pub struct AppState {
    // 已有字段保持不动 ……
    pub sidebar: SidebarTab,           // 新增
}

pub struct ConnectionRuntime {
    // 已有字段 ……
    pub created_at: std::time::Instant,   // 新增
}
```

### 4.2 关键 flow

**flow-1：从 Home 启动新连接**
```
HomeView.host_card_click(host_id)
  └─ state.update:
       1. open_connection(host_id) → conn_id
       2. push tab(Connection(conn_id))
       3. sidebar = SidebarTab::Terminal     ← M4a 新加
       4. cx.notify
  └─ bridge.spawn_session(conn_id, config, tx)
  └─ register_session(conn_id, sender)
```

**flow-2：从 Active Sessions 跳回会话**
```
HomeView.active_session_open(conn_id)
  └─ state.update:
       1. 在 tabs 找到 content=Connection(conn_id) 的 tab
       2. 找不到则 push 一个新 tab content=Connection(conn_id)（极端情况：tab 被关了但 conn 还在）
       3. current_tab = 那个 tab
       4. sidebar = SidebarTab::Terminal
       5. cx.notify
```

**flow-3：点 sidebar 切换**
```
SidebarNav.click(SidebarTab::X)
  └─ state.update:
       1. sidebar = X
       2. cx.notify
  // 不动 tabs / current_tab — Terminal sidebar 切回时自动恢复
```

### 4.3 路由保留

切换 sidebar 不破坏：
- tabs（会话 tab 列表）
- current_tab（上次活跃 tab id）
- connections（运行中的 SSH actor）
- hosts / modal / 其他

切回 sidebar=Terminal 时主区显示：
- 若 tabs 非空 → current_tab 对应的 content 渲染
- 若 tabs 为空 → EmptyTerminalGuideView

---

## 5. Empty / placeholder 视图

### 5.1 EmptyTerminalGuideView
独立简化页（不复用 Home），居中布局：
- 大图标（terminal SVG，48px，muted 色）
- 主文本：`No active sessions yet`（24px primary）
- 副文本：`Pick a host from Home to get started.`（14px secondary）
- 按钮：[Go to Home]（点击 → `state.sidebar = Home`）

### 5.2 ComingSoonView
通用 placeholder，参数化按 `kind: enum { Inbox, Settings }` 切显示：
- 大图标（Inbox 信封 / Settings 齿轮，48px muted）
- 主文本：`Inbox · Coming soon` / `Settings · Coming soon`
- 副文本：每个 kind 有一段简介，如：
  - Inbox：`Agent events and notifications will appear here. Sub-project D in the roadmap.`
  - Settings：`Customize appearance, input, and notifications. Sub-project A · M4b and beyond.`
- 底部小字：`See the [Moshi Desktop Roadmap](../roadmap-moshi-desktop.md) for details.`（点击打开 OS 默认 markdown 浏览器或暂时纯文本，不实现 hyperlink）

---

## 6. 文件结构

### 6.1 新增

| 路径 | 职责 | LOC |
|---|---|---|
| `crates/aish-app/src/views/sidebar_nav.rs` | 左侧 48px 4-icon 导航条 | ~120 |
| `crates/aish-app/src/views/home.rs` | Home tab：Quick Actions + Active Sessions + Hosts grid | ~180 |
| `crates/aish-app/src/views/empty_terminal.rs` | sidebar=Terminal 且 0 会话时的引导卡 | ~60 |
| `crates/aish-app/src/views/coming_soon.rs` | Inbox/Settings 通用 placeholder | ~80 |

### 6.2 修改

| 路径 | 改动概要 |
|---|---|
| `crates/aish-app/src/app.rs` | RootView render 重构：horizontal flex(SidebarNav + main_column)；main_column 按 sidebar 分支 |
| `crates/aish-app/src/views/tab_bar.rs` | 不动；外部 if `sidebar == Terminal` 控制 |
| `crates/aish-app/src/views/default_page.rs` | **整文件删除** |
| `crates/aish-app/src/views/mod.rs` | 暴露新 view 模块；删 `default_page` |
| `crates/aish-app/src/state.rs` | 加 `SidebarTab` enum、`AppState.sidebar`、`ConnectionRuntime.created_at`；`open_connection` 内置 sidebar 切换 |
| `crates/aish-app/src/theme.rs` | 加 `SIDEBAR_BG / SIDEBAR_WIDTH / SIDEBAR_BORDER / SIDEBAR_ACTIVE_BAR / SIDEBAR_NAV_*` |

总计：新增 ~440 LOC + 修改 ~200 LOC，删除 ~344 LOC（default_page.rs）。

---

## 7. theme 新增常量

```rust
// theme.rs
pub const SIDEBAR_WIDTH: f32 = 48.0;
pub const SIDEBAR_BG: Rgb = rgb(0x0a0a0c);
pub const SIDEBAR_BORDER: Rgb = rgb(0x1f1f23);
pub const SIDEBAR_ACTIVE_BAR: Rgb = rgb(0x4a9eff);

pub const SIDEBAR_NAV_FG_DEFAULT: Rgb = rgb(0x6b6b73);
pub const SIDEBAR_NAV_FG_HOVER: Rgb = rgb(0x9a9aa3);
pub const SIDEBAR_NAV_FG_ACTIVE: Rgb = rgb(0xffffff);
pub const SIDEBAR_NAV_BG_ACTIVE: Rgb = rgb(0x15151a);
```

复用现有 BG_BASE / BG_ELEVATED / TEXT_PRIMARY 等。

---

## 8. 测试计划

- 既有 `cargo test --workspace` 必须 199 全过
- 新加单测（state.rs）：
  - `sidebar_default_is_home`
  - `open_connection_sets_sidebar_terminal`
  - `connection_runtime_records_created_at`
  - `humanize_elapsed_*`（小时 / 分钟 / 天的边界）
- 不为 GPUI view 加视图测试（aish 暂无 GPUI 测试基础设施，不在本 milestone scope 引入）
- 手动验收：见 §10

---

## 9. 风险 / Open questions

| 风险 | 缓解 |
|---|---|
| GPUI sidebar 选中态切换不触发 tab_bar / main 区重渲染 | RootView 已 observe state；新 sidebar 字段在同 state 内，无新依赖 |
| 删除 default_page.rs 后被引用的地方需全部清理 | 编译器保证（rust 强类型）；删完跑 `cargo check --workspace` |
| `TabContent::Default` 保留但不可达，clippy 可能 warn | 加 `#[allow(dead_code)]` 注释 + 注释说明保留原因 |
| Home 顶部空间小屏可能拥挤 | 暂不处理（roadmap：响应式布局后续做） |
| GPUI tooltip 能力未确认 | 本 milestone 不做 tooltip；记 backlog `sidebar-tooltip` |
| 主区切换 sidebar 时 GPUI Entity drop / re-create 性能 | 各 sidebar view 用 lazy create + cache（首次切到才 create，后续复用）；如基线 OK 就不做 cache |

---

## 10. 验收标准（手动）

**功能性**：
- [ ] 启动应用，左侧 48px 侧边栏可见，4 个 icon，默认高亮 Home
- [ ] 点 Home：主区显示 Hosts grid + Quick Actions（Active Sessions 区当无会话时不渲染，"+ Add host" 按钮可见）
- [ ] 点 "+ Add host"：打开 HostForm modal（沿用现有体验）
- [ ] 点某个 host 卡片：自动切到 Terminal sidebar，会话 tab 出现，终端开始连接
- [ ] 切回 Home：Active Sessions 区显示 1 行（绿点 + label + "just now / Xm ago" + Open ▶）
- [ ] 点 Active Sessions 行：跳回 Terminal sidebar 并激活那个会话 tab
- [ ] 启动多个会话，关闭其中一个：Active Sessions 区相应行消失
- [ ] 点 Inbox：显示 ComingSoonView（信封图标 + 介绍 + roadmap 链接文本）
- [ ] 点 Settings：同 Inbox（齿轮图标）
- [ ] 关闭所有会话 tab，sidebar 仍在 Terminal：主区显示 EmptyTerminalGuideView
- [ ] 点 EmptyTerminalGuideView 的 [Go to Home]：sidebar 切回 Home

**质量门禁**：
- [ ] `cargo +nightly fmt --all` 通过
- [ ] `cargo +nightly clippy --workspace --all-targets -- -D warnings` 0 warning
- [ ] `cargo test --workspace` 199+N 全过
- [ ] 手动跑一遍：现有 host 持久化、keyring、tmux attach、剪贴板粘贴等行为 0 回归

---

## 11. 后续 milestone 链路

完成 M4a 后：
1. 更新 `docs/superpowers/roadmap-moshi-desktop.md`：把 A 子项目 M4a 行打勾
2. 更新 `docs/superpowers/INDEX.md`：M4a 列入完成区
3. **M4b**（如继续做）：Recent 持久化 + Settings 实质内容
4. **M5**（agent 系列起步）：sub-project B AI agent 会话识别预研

完整后续链：见 [`roadmap-moshi-desktop.md`](../roadmap-moshi-desktop.md)。
