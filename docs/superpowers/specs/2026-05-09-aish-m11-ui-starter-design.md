---
title: M11 — aish-ui 起步套件（Foundations + 7 组件）
date: 2026-05-09
status: approved
parent: 2026-05-09-aish-ui-architecture-design.md
---

# M11 — aish-ui Foundations + 起步套件

## 0. 关系

本 spec 是 [`aish-ui 架构总览`](2026-05-09-aish-ui-architecture-design.md) 下的第一个 milestone。
所有 crate 结构 / theme token / API 风格 / icon 系统的总览决策见父 spec，本 spec
只描述 M11 范围内的具体组件设计和验收。

## 1. 范围

### 1.1 Scope（in）

- aish-ui crate 骨架（Cargo.toml / lib.rs / 目录结构）
- Theme 系统：`Theme` struct + `ColorTokens` + `Radius/Spacing/FontSize` scale + `Theme::dark()` + `theme(cx)` helper
- Icon 系统：`IconName` enum（约 15 个 M11 用得到的）+ `icon()` 函数 + assets/icons/ 编入
- 7 个组件：
  - **Button**（builder）— primary / secondary / destructive / ghost variants + disabled
  - **IconButton**（builder）— 仅 icon 的小按钮（Tooltip 触发器、Toast 关闭按钮）
  - **Badge**（builder）— 小标签（用于状态指示，例 SessionPicker 的 [SSH] 蓝胶囊）
  - **Separator**（builder）— 横/竖线分割
  - **Tooltip**（builder + hover detect）— 悬停提示
  - **TextInput**（Entity）— 单行输入框，含 cursor blink + selection + IME
  - **Toast**（Entity + ToastManager）— 顶部 / 底部弹出提示，自动消失
- aish-app 接入：注册 Theme global + 把 `InputBarView` 中的文本输入部分切到 `TextInput`

### 1.2 Out of scope（M12+）

- Card / Tabs / Dialog / Select / Checkbox / RadioGroup / Switch（M12）
- DropdownMenu / ContextMenu（M13）
- Light theme 完整实现（仅 stub）
- aish-app 中 HostFormModal / SessionPicker / SettingsView 的全面重构（M13）
- 把 InputBarView 完全删除（M11 仅切文本输入部分；图片选择 + send 逻辑保留）

## 2. 文件结构

```
crates/aish-ui/
  Cargo.toml
  src/
    lib.rs
    prelude.rs
    theme/
      mod.rs            # 导出
      tokens.rs         # ColorTokens / Radius / Spacing / FontSize / Theme
      dark.rs           # Theme::dark()
      light.rs          # Theme::light() — unimplemented! stub
    components/
      mod.rs            # pub use
      button.rs         # Button + ButtonVariant
      icon_button.rs    # IconButton
      badge.rs          # Badge + BadgeVariant
      separator.rs      # Separator + Orientation
      tooltip.rs        # Tooltip
      text_input.rs     # TextInput + TextInputImeHandler
      toast.rs          # Toast + ToastKind + ToastManager
    icons/
      mod.rs            # IconName + icon() + bytes()
  assets/
    icons/
      chevron-down.svg
      chevron-up.svg
      x.svg
      check.svg
      info.svg
      alert-circle.svg
      alert-triangle.svg
      send.svg
      plus.svg
      minus.svg
      search.svg
      settings.svg
      trash.svg
      copy.svg
      external-link.svg
```

`Cargo.toml`：

```toml
[package]
name = "aish-ui"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui = { workspace = true }
```

workspace 根 `Cargo.toml`：在 `members` 加 `"crates/aish-ui"`，`workspace.dependencies` 加 `aish-ui = { path = "crates/aish-ui" }`。

## 3. 组件详细设计

### 3.1 Button

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,      // 主操作：填充背景色 = primary
    Secondary,   // 次操作：填充 = secondary
    Destructive, // 危险：填充 = destructive
    Ghost,       // 透明背景，仅 hover 时高亮
}

pub struct Button {
    id: SharedString,
    label: SharedString,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
}

impl Button {
    pub fn new(id: impl Into<SharedString>) -> Self;
    pub fn label(self, l: impl Into<SharedString>) -> Self;
    pub fn primary(self) -> Self;
    pub fn secondary(self) -> Self;
    pub fn destructive(self) -> Self;
    pub fn ghost(self) -> Self;
    pub fn disabled(self, d: bool) -> Self;
    pub fn on_click(self, h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> Self;
}

impl IntoElement for Button { ... }
```

视觉：

| Variant | bg | fg | hover bg | disabled bg |
|---|---|---|---|---|
| Primary | `primary` | `primary_foreground` | `primary` 亮 +10% | `muted` |
| Secondary | `secondary` | `secondary_foreground` | `accent` | `muted` |
| Destructive | `destructive` | `destructive_foreground` | `destructive` 亮 +10% | `muted` |
| Ghost | transparent | `foreground` | `accent` | transparent |

尺寸：高度 `28px`，padding-x `12px`，圆角 `radius.md`，字号 `font_size.sm`。

### 3.2 IconButton

```rust
pub struct IconButton {
    id: SharedString,
    icon: IconName,
    variant: ButtonVariant,
    size: IconButtonSize,    // Sm = 24x24, Md = 32x32, Lg = 40x40
    disabled: bool,
    on_click: Option<Box<dyn Fn(...) + 'static>>,
}
```

API 类似 Button，但 `icon(IconName)` 而非 `label(text)`。圆角 `radius.sm`，icon 大小 = size - 8px（边距 4px）。

### 3.3 Badge

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BadgeVariant {
    Default,     // 灰底
    Primary,     // 蓝底
    Success,     // 绿底（color: 自定 #9ece6a）
    Warning,     // 黄底（自定 #e0af68）
    Destructive, // 红底
}

pub struct Badge {
    label: SharedString,
    variant: BadgeVariant,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self;
    pub fn primary(self) -> Self;
    pub fn success(self) -> Self;
    pub fn warning(self) -> Self;
    pub fn destructive(self) -> Self;
}
```

视觉：高度 `18px`，padding-x `6px`，字号 `font_size.xs`，圆角 `radius.full`（胶囊）。

注：success / warning 颜色是固定值，不进 Theme token（避免 token 失控膨胀）。
Tokyo Night 系列：`success = #9ece6a`，`warning = #e0af68`。

### 3.4 Separator

```rust
#[derive(Clone, Copy)]
pub enum Orientation { Horizontal, Vertical }

pub struct Separator { orientation: Orientation }

impl Separator {
    pub fn horizontal() -> Self;
    pub fn vertical() -> Self;
}
```

实现：1px 线，颜色 `border` token。横向 `w_full + h_px(1)`，纵向 `h_full + w_px(1)`。

### 3.5 Tooltip

shadcn 的 Tooltip 是 hover 触发的浮层，触发器是任意子元素。GPUI 上需要：

- 触发器 hover 检测（GPUI `on_hover` 或 mouse enter/leave）
- 浮层定位（绝对相对触发器）
- 延迟（200ms）后显示，离开立即隐藏

```rust
pub struct Tooltip {
    text: SharedString,
    placement: TooltipPlacement,  // Top / Bottom / Left / Right
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self;
    pub fn placement(self, p: TooltipPlacement) -> Self;
}

// 触发：调用方在 div 上 .tooltip(Tooltip::new("提示"))
// 通过 trait extension：
pub trait TooltipExt: Sized {
    fn tooltip(self, t: Tooltip) -> impl IntoElement;
}

impl<E: IntoElement> TooltipExt for E { ... }
```

实现路径：M11 task 内调研 GPUI 现有 Tooltip 接口（Zed 已有，可参考），如果可直接复用则薄封装；如果不能则用 `on_hover` 状态 + deferred element 自实现。

**关于 Hybrid 归类**：Tooltip 看似有"hover 状态"，但实际上 GPUI 的 hover 是
per-frame 事件（`on_hover` 闭包参数 `is_hovering: &bool`），不需要组件自己跨帧记忆。
延迟显示通过 `cx.spawn` + sleep 实现，但延迟句柄存在于触发器父 view 上、不在
Tooltip 组件自身。所以 Tooltip 仍归类为 builder。

延迟：hover 200ms 后显示。延迟用 `cx.spawn` + sleep 实现。

### 3.6 TextInput

最复杂的组件。M9 InputBarView 的文本部分迁过来 + 加 cursor blink + selection。

```rust
pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    cursor: usize,                              // byte offset
    selection_anchor: Option<usize>,             // 拖选起始
    blink_epoch: Instant,
    placeholder: SharedString,
    on_submit: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
    on_change: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
    last_click: Option<(Instant, usize)>,        // 双击检测
    bar_bounds: Option<Bounds<Pixels>>,           // IME 候选窗位置
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self;
    pub fn placeholder(&mut self, p: impl Into<SharedString>);
    pub fn on_submit(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static);
    pub fn on_change(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static);
    pub fn text(&self) -> &str;
    pub fn set_text(&mut self, t: impl Into<String>, cx: &mut Context<Self>);
    pub fn clear(&mut self, cx: &mut Context<Self>);
    pub fn focus(&self, window: &mut Window, cx: &mut App);
}

impl Render for TextInput { ... }
impl Focusable for TextInput { ... }
```

#### 3.6.1 Cursor blink

参考现有 `aish-app/src/terminal/cursor.rs` 的 `BLINK_PERIOD_MS = 600`：

```rust
const BLINK_PERIOD_MS: u64 = 600;

fn cursor_visible_now(epoch: Instant) -> bool {
    let phase = epoch.elapsed().as_millis() as u64 % BLINK_PERIOD_MS;
    phase < BLINK_PERIOD_MS / 2
}
```

每帧（GPUI 自动重绘到 60fps，但需要主动触发）触发重绘：在组件 mount 时启动一个 `cx.spawn` 定时器，每 100ms 调 `cx.notify()`。

```rust
fn start_blink(&self, cx: &mut Context<Self>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() { break; }
        }
    }).detach();
}
```

约束：blink 仅在 focused 时生效（失焦时不闪、不画 cursor）；任意操作（按键 / 鼠标点）后 `blink_epoch = Instant::now()`，让 cursor 立即可见。

#### 3.6.2 Selection

- 鼠标按下：清空 selection，记 `selection_anchor = cursor_at_pixel(x)`，`cursor = anchor`
- 鼠标拖动（mouse_move with left button down）：`cursor = cursor_at_pixel(x)`，selection range = (anchor.min, cursor.max)
- 鼠标抬起：保留 selection
- Ctrl+A：全选（anchor=0, cursor=text.len()）
- Esc / 任意非 selection 操作：清 selection
- 输入字符 / Backspace 时如果有 selection，先删除 selection 范围再操作

`cursor_at_pixel(x)`：M11 task 内做 — 简化版本：等宽字体下 `cursor = (x - text_left) / cell_width`，按 char_indices 对齐。中文 / IME composing 期间不支持。

#### 3.6.3 双击选词

- 单击：记录 `last_click = (now, cursor_pos)`
- 第二次点击：若 `now - last_click.0 < 500ms` 且 `last_click.1 == cursor_pos`，触发选词
- 选词逻辑：从 cursor 向前找空白 / 向后找空白，确定 word boundary

#### 3.6.4 IME

M9 InputBarView 已有 `InputBarImeHandler` + canvas paint 阶段注册。M11 把这套搬进 TextInput，改名 `TextInputImeHandler`：

- `replace_text_in_range`：删 selection（如有）+ insert text + 移动 cursor
- `bounds_for_range`：返回组件 bounds（候选窗位置）
- 其他方法默认实现

#### 3.6.5 复制粘贴

- Ctrl+C：将 selection 文本（如无 selection 则整行）写入剪贴板
- Ctrl+V：通过 IME 通道（系统 paste 走 `replace_text_in_range`），无需特殊处理
- Ctrl+X：cut = copy + delete selection

剪贴板用 `arboard`（aish 已有 dep）。

#### 3.6.6 不在 M11 范围内

- 多行输入（Shift+Enter 换行）— 留在调用方处理（InputBarView 仍单独管图片栏 + 提交逻辑）
- Undo / Redo
- 富文本 / preedit composing 高亮
- 横向滚动（文本超长时）— M11 简化为不滚动，超出隐藏

### 3.7 Toast

GPUI 里 `Render` 只能给 `Entity<T>` 实现，`Global` 是数据存储 trait，两者不能合并。
所以 Toast 拆成三部分：

```rust
// 数据
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind { Info, Success, Warning, Error }

#[derive(Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: SharedString,
    pub created_at: Instant,
    pub duration: Duration,    // 默认 3s
}

// 状态机：Entity
pub struct ToastManager {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl ToastManager {
    pub fn new(cx: &mut Context<Self>) -> Self;   // 构造时启动 cleanup 定时器
    pub fn push(&mut self, kind: ToastKind, msg: impl Into<SharedString>, cx: &mut Context<Self>);
    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>);
    fn cleanup_expired(&mut self, cx: &mut Context<Self>);   // 内部定时清理
}

impl Render for ToastManager { ... }   // 渲染当前 toasts 列表（绝对定位右上角）

// 全局 handle：Global trait
#[derive(Clone)]
pub struct ToastHandle(pub Entity<ToastManager>);
impl gpui::Global for ToastHandle {}

// 便利函数（M11 公共 API 入口）
pub fn toast(cx: &mut App, kind: ToastKind, msg: impl Into<SharedString>) {
    let handle = cx.global::<ToastHandle>().clone();
    handle.0.update(cx, |m, cx| m.push(kind, msg, cx));
}
pub fn toast_info(cx: &mut App, msg: impl Into<SharedString>) { toast(cx, ToastKind::Info, msg) }
pub fn toast_success(cx: &mut App, msg: impl Into<SharedString>) { toast(cx, ToastKind::Success, msg) }
pub fn toast_warning(cx: &mut App, msg: impl Into<SharedString>) { toast(cx, ToastKind::Warning, msg) }
pub fn toast_error(cx: &mut App, msg: impl Into<SharedString>) { toast(cx, ToastKind::Error, msg) }
```

注册（aish-app 端）：

```rust
let manager = cx.new(|cx| ToastManager::new(cx));
cx.set_global(ToastHandle(manager.clone()));
// RootView 内 .child(manager.clone()) 让它有渲染节点
```

布局：右上角向下叠加，每个 toast 高 40px，padding 12px，圆角 `radius.md`，按 kind 染色 + icon。

定时清理：`ToastManager::new` 内 `cx.spawn` 每 100ms 跑一次 `cleanup_expired`，
过期则从 `toasts` 中移除并 `cx.notify()`。

## 4. 单元测试矩阵

| 模块 | 测试 |
|---|---|
| `theme::dark` | `dark()` 返回的 `primary != transparent` / 各 token 不为 default |
| `icons` | 每个 IconName::bytes() 长度 > 50 且包含 `<svg` |
| `button` | `Button::new("a").primary().disabled(true)` 字段断言 |
| `badge` | 同 button |
| `separator` | horizontal / vertical 构造 |
| `text_input` | `cursor_left/right/backspace/delete/insert_str` 状态机 |
| `text_input::selection` | 拖选 / 双击选词 / 输入时清 selection |
| `text_input::blink` | `cursor_visible_now` 在 0..300ms / 300..600ms 的相位断言 |
| `toast::manager` | push / dismiss / cleanup_expired |

预期总数：30+ test。

## 5. aish-app 接入（M11 任务的最后一步）

### 5.1 改动列表

1. `Cargo.toml` 加 `aish-ui = { workspace = true }`
2. `app.rs::run()`：`cx.set_global(aish_ui::Theme::dark())`
3. **不删** `aish-app/src/theme.rs`（M13 才删）。M11 仅让两套并存
4. `views/input_bar.rs`：
   - 把 `text / cursor / placeholder / IME / cursor 渲染` 部分迁到 `aish_ui::TextInput`
   - 保留 `images / pick_images / send` 逻辑
   - 子组件结构：`InputBarView { input: Entity<TextInput>, images: ..., bridge: ..., state: ... }`
   - `send` 时 `input.read(cx).text()` 取文本，`input.update(cx, |i, _| i.clear())` 清空
5. ToastManager 注册为 global，`app.rs` 启动时 `cx.set_global(ToastManager::new())`，渲染层在 RootView 里加一个 `cx.global::<ToastManager>().clone()` 子节点

### 5.2 视觉回归

迁移前后用 `cargo run` 跑一次手测：

- InputBarView 看起来与 M9 一致或更好
- 没有 panic / crash
- IME 仍然工作（输中文）

## 6. Task 拆分（writing-plans 输入）

| # | Task | 依赖 |
|---|---|---|
| T1 | 新建 aish-ui crate 骨架（Cargo.toml / lib.rs / prelude.rs / mod 占位） | — |
| T2 | Theme 系统（tokens.rs / dark.rs / theme(cx) helper） | T1 |
| T3 | Icon 系统（IconName enum / bytes / icon() 函数 + assets/icons/） | T1 |
| T4 | Separator 组件（最简单，先跑通 builder + IntoElement 模式） | T2 |
| T5 | Badge 组件 | T2 |
| T6 | Button 组件 | T2, T4 |
| T7 | IconButton 组件 | T3, T6 |
| T8 | Tooltip 组件（hover detect + 延迟） | T2, T6 |
| T9 | TextInput 组件 — 基础（text / cursor / 键盘输入 / IME / focus） | T2 |
| T10 | TextInput — cursor blink | T9 |
| T11 | TextInput — selection（拖选 + 双击选词 + Ctrl+A）| T9 |
| T12 | TextInput — 复制粘贴（Ctrl+C/V/X with selection） | T11 |
| T13 | Toast + ToastManager | T2, T3 |
| T14 | aish-app 接入：注册 Theme global + ToastManager global | T2, T13 |
| T15 | aish-app 接入：InputBarView 切到 TextInput 组件 | T12, T14 |
| T16 | INDEX.md 更新 + 手测视觉回归 | T15 |

T1–T13 是 aish-ui 内部，可一气呵成；T14–T16 是 aish-app 接入与收尾。

## 7. 风险

| ID | 风险 | 应对 |
|---|---|---|
| R1 | GPUI svg() API 与设想不符 | T3 先做 SVG 渲染 spike，Tooltip 等先用空 div + label 占位 |
| R2 | TextInput selection / 双击选词复杂度 | T11 必要时拆 sub-task；M11 不做超长文本横向滚动 |
| R3 | cursor_at_pixel 精度（中英文混排） | M11 简化版只支持 ASCII 等宽；中文 / IME composing 期间用 char_indices 步进 |
| R4 | Theme global 注入时机晚于某些 view 构造 | aish-app::run 在创建 windows / views 之前 set_global |
| R5 | Tooltip 浮层定位（GPUI 绝对定位 + 边界检测） | M11 仅做 4 placement 默认上方，超出窗口边界时不做 fallback |
| R6 | Toast 全局 + 定时清理与多窗口的交互 | M11 仅单窗口，多窗口在未来 milestone 再考虑 |

## 8. 完成定义（DoD）

- [ ] aish-ui crate 编译通过，独立测试 30+ 通过
- [ ] 7 个组件 + Theme + Icon 全部实现
- [ ] aish-app 启动后 ToastManager global 可用，输入栏 cursor 闪烁、可选区、Ctrl+C 能复制
- [ ] IME（中文）仍工作，候选窗对齐输入框
- [ ] 全部质量门禁通过：fmt + clippy 0 warning + test
- [ ] INDEX.md 更新 M11 条目
- [ ] 父 spec（架构总览）的 Risk 表 R1–R6 实际遇到 / 未遇到的情况补记
