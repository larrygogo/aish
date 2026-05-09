---
title: aish-ui 组件库架构总览
date: 2026-05-09
status: approved
supersedes: aish-app::theme（迁移）
related-milestones: M11 / M12 / M13
---

# aish-ui 组件库架构总览

## 0. 目标

在 GPUI 之上封装一套受 [shadcn/ui](https://ui.shadcn.com/) 启发的桌面端组件库，
作为独立 crate `aish-ui`，让 aish-app（以及未来其他基于 GPUI 的桌面应用）
能像写 React 一样**声明式地拼界面**，而不是每次都在 `div` 上手糊样式与状态机。

**非目标**：

- 不复制 shadcn 全部 50+ 组件，按 aish 实际需求做"中等体量"（~15 个）
- 不在本里程碑做 light theme（仅留扩展位）
- 不做无障碍审查 / 屏幕阅读器适配（GPUI 自身限制）
- 不做服务端渲染 / 动画引擎

## 1. 上下文

### 1.1 当前痛点

- aish-app 内部 UI 全靠 `div + .bg() + .px() + .border_*()` 链式手写
- `theme.rs` 是 const 集合，没有语义层（`BG_LAYER_2` 等同于"中色背景"，但每处都自己拼）
- TextInput / Modal / Tooltip 反复造，InputBarView 的 cursor 不会闪、HostFormModal 的 focus 管理散乱
- 每个新 view 都重新发明 padding / radius / 颜色搭配，视觉一致性靠 reviewer 心智维护

### 1.2 为什么要做

- M9–M10 之后 UI 复杂度持续上升（Inbox、Settings 实质内容、AI agent 集成都是 view 密集）
- 不抽象出来，后续每个 milestone 都会重复同样的样板
- 一次性沉淀基础设施 + 三个 milestone 集中产出，胜过 6+ 个 milestone 散迭

### 1.3 参考

- [shadcn/ui](https://ui.shadcn.com/docs/components) — 命名 / API 心智模型
- Zed 的 `crates/ui/` — GPUI 上的事实标杆，部分实现可借鉴
- [Lucide](https://lucide.dev/) — icon 来源（MIT）

## 2. Workspace 布局

```
aish/
  crates/
    aish-types/
    aish-ssh/
    aish-tmux/
    aish-sftp/
    aish-secrets/
    aish-ui/              # 🆕 新 crate
      Cargo.toml
      src/
        lib.rs            # pub use 公共 re-exports
        theme/
          mod.rs           # Theme struct + cx.global helper
          tokens.rs        # 语义 token 定义（type alias / scale enum）
          dark.rs          # Theme::dark() 实现
          light.rs         # Theme::light() — M11 仅留 stub，不实现
        components/
          mod.rs
          button.rs
          icon_button.rs
          badge.rs
          separator.rs
          tooltip.rs
          text_input.rs
          toast.rs
          # M12 后续填：card / tabs / dialog / select / checkbox / radio_group / switch
          # M13 后续填：dropdown_menu / context_menu
        icons/
          mod.rs           # IconName enum + icon() 函数
        prelude.rs         # 常用 re-exports，调用方 use aish_ui::prelude::*;
      assets/
        icons/             # *.svg，include_bytes! 编入 binary
    aish-app/             # 已有，作为消费者
```

### 2.1 crate 依赖

```toml
[dependencies]
gpui = { workspace = true }
# 不依赖 aish-types / aish-ssh / aish-app 等任何业务 crate
```

`aish-app` 在 M11 task 1 加入 `aish-ui = { workspace = true }` 依赖。

### 2.2 公开 API 入口

```rust
// aish-ui/src/lib.rs
pub mod theme;
pub mod components;
pub mod icons;
pub mod prelude;

pub use components::*;       // Button / TextInput / ...
pub use icons::{icon, IconName};
pub use theme::{theme, Theme};
```

```rust
// aish-ui/src/prelude.rs
pub use crate::components::*;
pub use crate::icons::{icon, IconName};
pub use crate::theme::{theme, Theme};
```

调用方：

```rust
use aish_ui::prelude::*;

Button::new("send")
    .label("发送")
    .primary()
    .on_click(cx.listener(...))
```

## 3. Theme / Token 系统

### 3.1 设计原则

- **语义命名而非具体值**：调用方写 `theme(cx).primary` 而不是 `rgb(0x6c91c2)`
- **HSLA 内部存储**：与 GPUI `Hsla` 类型一致，避免每处转换
- **不可变 `Theme` struct**：`cx.set_global(Theme::dark())` 一次性注入，组件只读
- **Scale 用强类型 enum**：`Radius::Sm` 而不是 magic number `4.0`

### 3.2 完整 Token 列表

```rust
// theme/tokens.rs

use gpui::{Hsla, Pixels};

/// 语义色板，命名与 shadcn 对齐。
pub struct ColorTokens {
    pub background: Hsla,            // 主背景：窗口 / 主区
    pub foreground: Hsla,            // 主文字色

    pub card: Hsla,                  // 卡片背景（比 background 略亮）
    pub card_foreground: Hsla,

    pub popover: Hsla,               // 弹层背景（Tooltip / DropdownMenu）
    pub popover_foreground: Hsla,

    pub primary: Hsla,               // 主操作（发送按钮、确认）
    pub primary_foreground: Hsla,

    pub secondary: Hsla,             // 次操作（取消、辅助按钮）
    pub secondary_foreground: Hsla,

    pub muted: Hsla,                 // 弱化前景（disabled、placeholder）
    pub muted_foreground: Hsla,

    pub accent: Hsla,                // 强调（hover 高亮、选中态）
    pub accent_foreground: Hsla,

    pub destructive: Hsla,           // 危险操作（删除、错误提示）
    pub destructive_foreground: Hsla,

    pub border: Hsla,                // 边框默认色
    pub input: Hsla,                 // input 控件背景
    pub ring: Hsla,                  // focus ring 颜色
}

#[derive(Clone, Copy)]
pub struct Radius {
    pub sm: Pixels,                  // 4
    pub md: Pixels,                  // 6
    pub lg: Pixels,                  // 8
    pub full: Pixels,                // 9999（圆形）
}

#[derive(Clone, Copy)]
pub struct Spacing {
    pub px_1: Pixels,  // 4
    pub px_2: Pixels,  // 8
    pub px_3: Pixels,  // 12
    pub px_4: Pixels,  // 16
    pub px_6: Pixels,  // 24
    pub px_8: Pixels,  // 32
}

#[derive(Clone, Copy)]
pub struct FontSize {
    pub xs: Pixels,   // 10
    pub sm: Pixels,   // 12
    pub base: Pixels, // 14
    pub lg: Pixels,   // 16
    pub xl: Pixels,   // 18
}

/// 顶层 Theme，cx.set_global 注入。
pub struct Theme {
    pub colors: ColorTokens,
    pub radius: Radius,
    pub spacing: Spacing,
    pub font_size: FontSize,
}

impl gpui::Global for Theme {}

/// 便利函数。
pub fn theme(cx: &gpui::App) -> &Theme { cx.global::<Theme>() }
```

### 3.3 默认 Dark 主题数值

参考 aish 现有 `theme.rs` 的视觉风格（Tokyo Night 系），扩展为完整 token：

| Token | HSLA / hex | 备注 |
|---|---|---|
| `background` | `#1a1b26` | 现 `BG_LAYER_0` |
| `foreground` | `#c0caf5` | 现 `FG_PRIMARY` |
| `card` | `#1f2030` | 比 background 亮 +5% |
| `popover` | `#24253a` | Tooltip / 菜单背景 |
| `primary` | `#3d59a1` | 蓝色主操作 |
| `primary_foreground` | `#c0caf5` | |
| `secondary` | `#2d2d3f` | 中灰 |
| `secondary_foreground` | `#a9b1d6` | |
| `muted` | `#2d2d3f` | |
| `muted_foreground` | `#565f89` | placeholder 灰 |
| `accent` | `#6c91c2` | hover 蓝 |
| `accent_foreground` | `#c0caf5` | |
| `destructive` | `#f7768e` | 红 |
| `destructive_foreground` | `#1a1b26` | |
| `border` | `#2d2d3f` | |
| `input` | `#16161e` | 输入框深底 |
| `ring` | `#6c91c2` | focus ring |

Light theme 留空 stub（`Theme::light() -> unimplemented!()`），M11 不做。

### 3.4 注入

```rust
// aish-app/src/app.rs，application().run 内部，cx.activate 之前
cx.set_global(aish_ui::Theme::dark());
```

## 4. API 风格：Hybrid

### 4.1 决策树

```
组件需要跨帧记住状态吗？
├── 是 → Entity<T> + Render
│   - 例：TextInput（cursor 位置 / selection）
│         Toast（队列）/ Dialog（open 状态）/ Select（展开）
│         Tabs（active 索引）
└── 否 → Builder + IntoElement
    - 例：Button / IconButton / Badge / Separator / Tooltip / Card
```

### 4.2 Builder 模式范例

```rust
pub struct Button {
    id: SharedString,
    label: SharedString,
    variant: ButtonVariant,
    on_click: Option<Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>>,
    disabled: bool,
}

impl Button {
    pub fn new(id: impl Into<SharedString>) -> Self { ... }
    pub fn label(mut self, l: impl Into<SharedString>) -> Self { ... }
    pub fn primary(mut self) -> Self { self.variant = ButtonVariant::Primary; self }
    pub fn secondary(mut self) -> Self { ... }
    pub fn destructive(mut self) -> Self { ... }
    pub fn disabled(mut self, d: bool) -> Self { ... }
    pub fn on_click(mut self, h: impl Fn(...) + 'static) -> Self { ... }
}

impl IntoElement for Button {
    type Element = AnyElement;
    fn into_element(self) -> Self::Element {
        let t = /* read theme */;
        // 根据 variant 选 token，构造 div
        ...
    }
}
```

### 4.3 Entity 模式范例

```rust
pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    cursor: usize,
    selection: Option<Range<usize>>,
    blink_epoch: Instant,
    placeholder: SharedString,
    on_submit: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self { ... }
    pub fn placeholder(&mut self, p: impl Into<SharedString>) { ... }
    pub fn on_submit(&mut self, h: impl Fn(...) + 'static) { ... }
    pub fn text(&self) -> &str { &self.text }
}

impl Render for TextInput { ... }
impl Focusable for TextInput { ... }
```

调用方：

```rust
let input = cx.new(|cx| TextInput::new(cx));
input.update(cx, |i, _| i.placeholder("输入文字"));
// 渲染时直接 .child(input.clone())
```

### 4.4 共同约定

- 所有组件都 `pub use` 在 `aish_ui::components::*`
- 公开字段保持最小，内部状态全 `pub(crate)` 或私有
- Builder 链 `Self` 返回，方便链式
- Entity 类暴露 `update` 接口：`pub fn set_text(&mut self, ...)` 不直接暴露字段
- 所有"事件"（on_click / on_submit / on_change）签名统一：`impl Fn(&E, &mut Window, &mut App) + 'static`

## 5. Icon 系统

### 5.1 资源结构

```
aish-ui/assets/icons/
  chevron-down.svg
  chevron-up.svg
  chevron-left.svg
  chevron-right.svg
  x.svg
  check.svg
  info.svg
  alert-circle.svg
  alert-triangle.svg
  ...
```

来源：[Lucide](https://lucide.dev/) MIT 许可，~25 个常用 icon。每个 svg 大小 ~200B，编入 binary 总计 ~5KB。

### 5.2 API

```rust
// icons/mod.rs

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    X,
    Check,
    Info,
    AlertCircle,
    AlertTriangle,
    Search,
    Settings,
    Trash,
    Plus,
    Minus,
    Send,
    // M12/M13 后续填
}

impl IconName {
    pub fn bytes(&self) -> &'static [u8] {
        match self {
            IconName::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
            IconName::ChevronUp => include_bytes!("../../assets/icons/chevron-up.svg"),
            // ...
        }
    }
}

pub fn icon(name: IconName) -> impl IntoElement {
    let bytes = name.bytes();
    // GPUI svg 元素加载（需调研 GPUI svg API 的精确签名）
    svg().path(...).size_4()  // 默认 16px
}
```

### 5.3 着色

GPUI `svg()` 元素支持 `text_color()` —— 通过 CSS `currentColor` 染色，调用方：

```rust
icon(IconName::X).text_color(theme(cx).colors.muted_foreground)
```

assets 里的 svg 必须用 `stroke="currentColor"` 或 `fill="currentColor"`，不写死颜色。

## 6. 测试策略

### 6.1 范畴

- **状态机测试**（主力）：TextInput 光标移动 / Tabs 切换 / Toast 队列 / Select 展开收起
- **构造测试**：Button::new(...).primary().disabled(true) 能编过 + 字段断言正确
- **token 数值测试**：Theme::dark() 关键 token 不为 transparent / 数值在合理区间
- **icon bytes 测试**：每个 IconName::bytes() 长度 > 50 且包含 `<svg`

### 6.2 不测什么

- 不做视觉快照测试（GPUI 没现成方案，不在 milestone 内做）
- 不做集成端到端测试（aish-app 启 GUI 才能验，由用户手测）

### 6.3 质量门禁

每个 task 完成后：

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

任一失败必须修后才能 commit。

## 7. Milestone 分解

| Milestone | 内容 | 工期估算 | spec 文件 |
|---|---|---|---|
| **M11** UI Foundations + 起步套件 | aish-ui crate 骨架 + Theme/tokens + Icon 系统 + Button + IconButton + Badge + Separator + Tooltip + TextInput + Toast | 3–4 天 | `2026-05-09-aish-m11-ui-starter-design.md` |
| **M12** 表单与导航 | Card + Tabs + Dialog + Select + Checkbox + RadioGroup + Switch | 2–3 天 | M12 spec（M11 完成后写） |
| **M13** 菜单 + 文档 + aish-app 迁移 | DropdownMenu + ContextMenu + crate README + examples/ + 把 InputBarView/HostFormModal/SessionPicker/SettingsView 切到 aish-ui | 2–3 天 | M13 spec |

每个 milestone 完成后，立即把 aish-app 中相应的临时实现切到 aish-ui 上（M11 做 InputBarView，M12 做 HostFormModal，M13 做 SessionPicker / SettingsView 重构 + cleanup）。

## 8. 迁移路径

### 8.1 aish-app 改动（M11 阶段）

1. `Cargo.toml` 加 `aish-ui = { workspace = true }`
2. `app.rs` 启动时 `cx.set_global(Theme::dark())`
3. 删除 `crates/aish-app/src/theme.rs`，所有 `use crate::theme::*` 改成 `use aish_ui::prelude::*`
4. `views/input_bar.rs` 重构：cursor / IME / send 全切到 `TextInput` 组件
5. `views/coming_soon.rs`、`empty_terminal.rs` 等用新 Button / Badge

### 8.2 风险与回滚

- aish-ui 提交但 aish-app 接入失败 → 临时 revert aish-app 改动，不影响 aish-ui crate 本身
- Theme 数值与现有视觉差异过大 → 在 M11 task 内做对比 screenshot，按需微调 token

## 9. Risk 表

| ID | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | GPUI `svg()` API 与设想不符 | 中 | 中 | M11 task 1 先做 SVG render spike，验证后再写正式组件 |
| R2 | Theme 全局后 aish-app 视觉退化 | 中 | 高 | 迁移前后对比截图，按需保留旧 const fallback |
| R3 | TextInput selection / 双击选词复杂度被低估 | 高 | 中 | M11 spec 内单列 TextInput task，必要时拆 sub-task |
| R4 | Hybrid API 决策树不清，组件归类摇摆 | 低 | 低 | 本 spec § 4.1 已锁规则，新组件归类需先 PR 评审 |
| R5 | Icon 资源管理（include_bytes 多了编译变慢） | 低 | 低 | 25 个 SVG ≈ 5KB，增量编译影响可忽略 |
| R6 | Light theme stub 未来填充时 token 不够 | 中 | 低 | M11 spec 设计 token 时按 light 友好命名（`background` 而非 `dark_bg`）|

## 10. 决策记录（ADR-style）

### ADR-001：用独立 crate 而非 aish-app/src/components/

**选项**：(A) 独立 crate (B) aish-app 内目录 (C) 独立但 theme 留外
**选**：A
**理由**：边界最干净；不依赖业务 state；未来能单独开源；workspace 已有多 crate 模式，新增一个零成本

### ADR-002：API 走 Hybrid 而非纯 builder / 纯 Entity

**选项**：(A) Hybrid (B) 纯 builder (C) 纯 Entity
**选**：A
**理由**：纯 builder 让 TextInput 这类必须有内部状态的组件状态外抛、调用方负担重；纯 Entity 让 Badge 这类无状态组件冗余；Hybrid 让组件归类自然，符合 GPUI 哲学

### ADR-003：M11 只做 dark，留 light stub

**选**：dark only
**理由**：light theme 设计本身需独立色彩工程，与组件骨架耦合度低；先把 token 抽象做对，后续填 light 不影响组件代码

### ADR-004：Icon 用 SVG 资源而非 Nerd Font

**选项**：(A) SVG 资源 (B) Nerd Font 字符 (C) Unicode/ASCII
**选**：A
**理由**：可控（设计统一）；可染色；跨平台一致；体积可接受；未来扩展 icon 不依赖字体更新

## 11. 完成定义（DoD）

整个 aish-ui 项目（M11+M12+M13）完成需满足：

- [ ] aish-ui crate 在 workspace 内独立编译通过
- [ ] 全部 ~15 个组件实现 + 单元测试覆盖
- [ ] aish-app 内 InputBarView / HostFormModal / SessionPicker / SettingsView 全部迁到 aish-ui
- [ ] aish-app/src/theme.rs 删除
- [ ] 视觉回归：截图对比迁移前后无明显退化
- [ ] crate-level README + examples/ 演示
- [ ] Light theme stub 函数签名定下（实现可空）
- [ ] 全部质量门禁通过
- [ ] INDEX.md 更新

## 12. 后续候选（aish-ui 范围外）

- Light theme 实现（独立小 milestone，1 天）
- 无障碍：键盘导航、Tab 顺序、Esc 行为统一
- Animation / transitions（GPUI 自身 animation API 调研后定）
- Form 抽象（schema 驱动多组件组合）
- Table / DataGrid（aish 还没需求，后续）
