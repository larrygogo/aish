---
title: M14 — aish-ui Popover + DropdownMenu + Select 改造 + Toast 关闭按钮
date: 2026-05-11
status: approved
parent: 2026-05-09-aish-ui-architecture-design.md
---

# M14 — aish-ui Popover + DropdownMenu + Select 改造 + Toast 关闭按钮

## 0. 关系

本 spec 是 [aish-ui 架构总览](2026-05-09-aish-ui-architecture-design.md) 下的第四个里程碑。

M11/M12/M13 已交付：
- M11：Foundations（Theme + Icon + 7 起步组件）
- M12：Forms & Nav（5 组件 + HostForm/SessionPicker/Settings 迁移）
- M13：Card/NavItem/TabItem + home/sidebar/tab_bar 迁移

M14 目标：把"基于绝对定位的浮层"封装到 Popover 基础组件，解决 M11-M13 多次踩到的 GPUI/Taffy absolute 在 flex container 内定位歧义问题；在 Popover 之上造 DropdownMenu；改造 Select 弹层用 Popover 获得自动翻转；顺带加 Toast 关闭按钮。

## 1. 范围

### 1.1 Scope（in）

新组件：

- **Popover**（Entity）—— click + programmatic 触发；基于 GPUI `anchored` + prepaint 写入 trigger bounds；fit_mode 自动翻转（向下没空间翻向上）；Esc / 点外面关闭；on_close 回调
- **MenuItem**（数据 struct）—— label + icon + shortcut 文字 + disabled
- **DropdownMenu**（builder + RenderOnce）—— 接收 `Vec<MenuItem>` + on_select；本身不管 open/close，作为 Popover content 传入

应用 / 迁移：

- **Select 弹层** —— M12 的手糊 absolute 弹层改用 Popover，获得 SwitchAnchor 自动翻转
- **Toast 关闭按钮** —— ToastManager::render_toast 内每个 toast 末尾加 IconButton(X) 调 `dismiss(id)`

### 1.2 Out of scope（M15+）

- **ContextMenu** —— 右键触发，aish 当前无场景
- **Light theme 实施** —— Theme::light() 仍 unimplemented! stub
- **TextInput mask** —— HostForm password 字段需要，但与 Popover 无关
- **TextInput cursor_at_pixel** —— 精确点击定位
- **Dialog Tab focus trap** —— Esc + backdrop close 已足够
- **Button hover variant 精细化** —— Primary 亮 +10% 等

## 2. 文件结构

```
crates/aish-ui/src/components/
  popover.rs              # 新
  menu_item.rs            # 新
  dropdown_menu.rs        # 新
  select.rs               # 改：弹层走 Popover
  toast.rs                # 改：加 close button
  mod.rs                  # 追加 mod / pub use

crates/aish-ui/src/prelude.rs   # 自动 re-export
```

## 3. 组件详细设计

### 3.1 Popover

**核心挑战**：trigger element 的位置只有渲染时才能知道。`anchored` 元素需要在 paint 阶段给定 viewport 坐标。trigger 在 prepaint 阶段可以拿到 bounds，需要把 bounds 写入 Popover Entity 让它在下一帧用。

**API**：

```rust
use std::rc::Rc;
use gpui::{
    Anchor, AnchoredFitMode, AnchoredPositionMode, AnyElement, Bounds, FocusHandle, Pixels, ...
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PopoverPlacement {
    Bottom,   // trigger 下方
    Top,      // trigger 上方
    Left,     // trigger 左侧
    Right,    // trigger 右侧
}

pub struct Popover {
    focus_handle: FocusHandle,
    open: bool,
    needs_focus: bool,
    content: Option<AnyElement>,
    /// trigger element 的 viewport bounds，由 trigger 的 prepaint 写入。
    /// 用于 anchored 定位的 position。
    trigger_bounds: Option<Bounds<Pixels>>,
    placement: PopoverPlacement,
    /// SwitchAnchor: 边界外时翻转方向（默认）
    /// SnapToWindow: 边界外时贴窗口边
    /// None-equivalent: 不约束
    fit_mode: AnchoredFitMode,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Popover {
    pub fn new(cx: &mut Context<Self>) -> Self;

    pub fn content(&mut self, c: impl IntoElement) -> &mut Self;
    pub fn placement(&mut self, p: PopoverPlacement) -> &mut Self;
    pub fn fit_mode(&mut self, m: AnchoredFitMode) -> &mut Self;
    pub fn on_close(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self;

    pub fn open(&mut self, cx: &mut Context<Self>);
    pub fn close(&mut self, cx: &mut Context<Self>);
    pub fn toggle(&mut self, cx: &mut Context<Self>);
    pub fn is_open(&self) -> bool;

    /// trigger 的 prepaint 阶段调，写入 viewport bounds。
    /// Popover 下一帧 render 时用 bounds 定位 anchored content。
    pub fn set_trigger_bounds(&mut self, bounds: Bounds<Pixels>);
}

impl Render for Popover { ... }
impl Focusable for Popover { ... }
```

**Render 逻辑**：

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    if !self.open {
        return div().into_any_element();
    }

    if self.needs_focus {
        self.focus_handle.focus(window, cx);
        self.needs_focus = false;
    }

    let Some(trigger_bounds) = self.trigger_bounds else {
        // trigger 还没写入 bounds，渲染空
        return div().into_any_element();
    };

    // 根据 placement 算 anchor + position
    let (anchor, position) = match self.placement {
        PopoverPlacement::Bottom => (
            Anchor::TopLeft,
            point(trigger_bounds.left(), trigger_bounds.bottom() + px(4.0)),
        ),
        PopoverPlacement::Top => (
            Anchor::BottomLeft,
            point(trigger_bounds.left(), trigger_bounds.top() - px(4.0)),
        ),
        PopoverPlacement::Right => (
            Anchor::TopLeft,
            point(trigger_bounds.right() + px(4.0), trigger_bounds.top()),
        ),
        PopoverPlacement::Left => (
            Anchor::TopRight,
            point(trigger_bounds.left() - px(4.0), trigger_bounds.top()),
        ),
    };

    let content = self.content.take();
    let t = theme(cx);

    // backdrop 全屏占位 — 点击外部关闭，键盘事件捕获
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .occlude()              // 阻挡底层鼠标（同 Dialog）
        .track_focus(&self.focus_handle)
        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
            if ev.keystroke.key.as_str() == "escape" {
                this.close(cx);
                this.fire_close(window, cx);
            }
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _ev, window, cx| {
                this.close(cx);
                this.fire_close(window, cx);
            }),
        )
        .child(
            anchored()
                .position_mode(AnchoredPositionMode::Window)
                .position(position)
                .anchor(anchor)
                .child(
                    // popover content box
                    div()
                        .bg(t.colors.popover)
                        .rounded(t.radius.md)
                        .border_1()
                        .border_color(t.colors.border)
                        .on_mouse_down(MouseButton::Left, |_ev, _w, cx| {
                            // 阻止冒泡到 backdrop 关闭
                            cx.stop_propagation();
                        })
                        .when_some(content, |d, c| d.child(c)),
                ),
        )
        .into_any_element()
}
```

**Trigger 集成模式**：

caller 在 trigger 元素的 prepaint 阶段调 `popover.set_trigger_bounds(bounds)`。GPUI 提供 `canvas()` 元素的 prepaint 回调最适合：

```rust
// trigger element 的 wrapper：
div()
    .child(trigger_content)
    .child(
        canvas(
            move |bounds, _window, cx| {
                // prepaint: 把 bounds 写入 Popover
                let popover = popover_handle.clone();
                popover.update(cx, |p, _cx| p.set_trigger_bounds(bounds));
            },
            |_bounds, _, _, _| {},  // paint: 不画东西
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),  // canvas 占满 trigger，bounds 即 trigger bounds
    )
```

更简化的封装可以放在 Popover 的 helper 方法里。**M14 简化**：先把 set_trigger_bounds 暴露，由 caller 自己 canvas 写入（同 M9 InputBar IME handler 模式）。M14 不造 PopoverTrigger 高级包装。

### 3.2 MenuItem

```rust
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: SharedString,
    pub icon: Option<IconName>,
    pub shortcut: Option<SharedString>,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self;
    pub fn icon(mut self, i: IconName) -> Self;
    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self;
    pub fn disabled(mut self, d: bool) -> Self;
}
```

**视觉**：高 28px / px(12) / flex_row / gap(8) / text_size(sm) / 包含 [optional icon] + label + [optional shortcut（右对齐 muted_foreground）]。

Disabled item：text_color = muted_foreground，无 hover，不响应 click。

### 3.3 DropdownMenu

```rust
type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct DropdownMenu {
    id: ElementId,
    items: Vec<MenuItem>,
    on_select: Option<SelectHandler>,
    /// 已选中索引，用于键盘导航高亮（默认 None / 0）。
    /// 简化版 M14：不接 selected_index，每次打开默认无高亮。
    /// 调用方用 Popover 控制 open/close。
    min_width: Option<Pixels>,
}

impl DropdownMenu {
    pub fn new(id: impl Into<ElementId>) -> Self;
    pub fn items(self, items: Vec<MenuItem>) -> Self;
    pub fn on_select(self, h: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self;
    pub fn min_width(self, w: Pixels) -> Self;
}
```

**Render**：

```rust
impl RenderOnce for DropdownMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let items = self.items;
        let on_select = self.on_select;

        div()
            .id(self.id)
            .flex()
            .flex_col()
            .py(t.spacing.px_1)
            .when_some(self.min_width, |d, w| d.min_w(w))
            .children(items.into_iter().enumerate().map(|(i, item)| {
                let is_disabled = item.disabled;
                let fg = if is_disabled {
                    t.colors.muted_foreground
                } else {
                    t.colors.popover_foreground
                };

                let mut row = div()
                    .h(gpui::px(28.0))
                    .px(t.spacing.px_3)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t.spacing.px_2)
                    .text_size(t.font_size.sm)
                    .text_color(fg);

                if let Some(icon_name) = item.icon {
                    row = row.child(icon(icon_name).size(gpui::px(14.0)).text_color(fg));
                }
                row = row.child(div().flex_1().child(item.label.clone()));
                if let Some(sc) = item.shortcut {
                    row = row.child(
                        div()
                            .text_color(t.colors.muted_foreground)
                            .text_size(t.font_size.xs)
                            .child(sc),
                    );
                }

                if !is_disabled {
                    let hover_bg = t.colors.accent;
                    row = row.cursor_pointer().hover(move |s| s.bg(hover_bg));
                    if let Some(h) = on_select.clone() {
                        row = row.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                            h(&i, window, cx);
                            let _ = ev;
                        });
                    }
                }

                row
            }))
    }
}
```

### 3.4 Select 改造

**M12 → M14 关键改动**：

```rust
pub struct Select {
    focus_handle: FocusHandle,
    options: Vec<SharedString>,
    selected: usize,
    popover: Entity<Popover>,             // ← 新增（替换原 open: bool）
    placeholder: SharedString,
    on_change: Option<ChangeHandler>,
}

impl Select {
    pub fn new<S: Into<SharedString>>(options: Vec<S>, cx: &mut Context<Self>) -> Self {
        let popover = cx.new(|cx| {
            let mut p = Popover::new(cx);
            p.placement(PopoverPlacement::Bottom);
            p.fit_mode(AnchoredFitMode::SwitchAnchor);
            p
        });
        Self { /* ... */, popover, /* ... */ }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.popover.update(cx, |p, cx| p.toggle(cx));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.popover.update(cx, |p, cx| p.close(cx));
    }
    // ...
}

impl Render for Select {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let popover_open = self.popover.read(cx).is_open();
        let popover_handle = self.popover.clone();

        // trigger element
        let display = /* 显示选中文字或 placeholder */;
        let trigger = div()
            .id("select-trigger")
            .h(gpui::px(28.0))
            // ... 视觉同 M12
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.toggle(cx)))
            .child(display)
            .child(icon(if popover_open { ChevronUp } else { ChevronDown }))
            .child(
                // canvas 写入 trigger bounds 到 popover
                canvas(
                    move |bounds, _w, cx| {
                        let h = popover_handle.clone();
                        h.update(cx, |p, _| p.set_trigger_bounds(bounds));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );

        // popover content：选项列表
        let options = self.options.clone();
        let selected = self.selected;
        let weak_self = cx.weak_entity();
        let content = div()
            .flex()
            .flex_col()
            .min_w(gpui::px(200.0))
            .py(t.spacing.px_1)
            .children(options.into_iter().enumerate().map(|(i, opt)| {
                let is_selected = i == selected;
                div()
                    .id(("select-option", i))
                    .h(gpui::px(28.0))
                    .px(t.spacing.px_3)
                    .flex()
                    .items_center()
                    .text_size(t.font_size.sm)
                    .text_color(if is_selected { t.colors.accent_foreground } else { t.colors.popover_foreground })
                    .when(is_selected, |d| d.bg(t.colors.accent))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.accent))
                    .on_mouse_down(MouseButton::Left, {
                        let weak = weak_self.clone();
                        move |_, window, cx| {
                            let _ = weak.update(cx, |s, cx| s.select(i, window, cx));
                        }
                    })
                    .child(opt)
            }));

        self.popover.update(cx, |p, _| { p.content(content); });

        div().child(trigger).child(self.popover.clone())
    }
}
```

### 3.5 Toast 关闭按钮

ToastManager 内 `render_toast` 末尾追加 close 按钮：

```rust
fn render_toast(toast: Toast, cx: &mut App, weak_mgr: WeakEntity<ToastManager>) -> impl IntoElement {
    // ... 原 toast row
    .child(
        IconButton::new(("toast-close", toast.id), IconName::X)
            .small()
            .ghost()
            .on_click(move |_ev, _w, cx| {
                if let Some(m) = weak_mgr.upgrade() {
                    m.update(cx, |m, cx| m.dismiss(toast.id, cx));
                }
            }),
    )
}
```

ToastManager render 内传入 weak 引用给 render_toast。

## 4. 单元测试矩阵

| 组件 | 测试 |
|---|---|
| Popover | 5 个：open/close 状态机 / placement chain / fit_mode chain / set_trigger_bounds 存储 / on_close 触发 |
| MenuItem | 3 个：new 默认 / icon+shortcut chain / disabled chain |
| DropdownMenu | 3 个：new 默认 / items chain / on_select 存储 |
| Select 改造 | 已有 6 测试不需调整（toggle/clamp 等纯逻辑测试） |
| Toast 关闭 | 1 个新测试：dismiss 后 toasts 列表少一项 |

预计 aish-ui 90 → ~102（+12）。

## 5. Risk

| ID | 风险 | 应对 |
|---|---|---|
| R1 | trigger canvas 写入 bounds 跨 Entity 协作 | T1 起手 spike — 验证 canvas prepaint 闭包内能 `popover_handle.update(cx, ...)` |
| R2 | Popover open 时 needs_focus 抢 focus，导致 trigger 失焦 + 反复触发 toggle | Popover focus_handle 只在 open 状态下 needs_focus；trigger 的 on_mouse_down 直接调 toggle 不依赖焦点 |
| R3 | Popover backdrop occlude 与底层 Dialog backdrop occlude 冲突 | M14 简化：不嵌套 Dialog + Popover；如果出现，按 z-order 后者 occlude 即可 |
| R4 | Select 改造引入 Popover 后键盘 ↑↓/Enter/Esc 怎么走 | Popover focus_handle 收 Esc，↑↓/Enter 仍由 Select 顶层 track_focus 处理。需要确认 trigger track_focus 仍生效 |
| R5 | DropdownMenu 当前不接键盘导航 | M14 暂不做（builder 模式无内部状态），M15+ 加 stateful 版本支持键盘 |

## 6. Milestone 阶段拆分

| Task | 内容 | 工期 |
|---|---|---|
| T1 | Popover 基础组件（含 trigger bounds spike） | 1 天 |
| T2 | MenuItem + DropdownMenu | 0.5 天 |
| T3 | Select 改造（弹层走 Popover + 自动翻转） | 0.5 天 |
| T4 | Toast 加关闭按钮 | 0.25 天 |
| T5 | INDEX 更新 + 视觉手测 + DoD | 0.25 天 |

总计 ~2.5 天。

## 7. 完成定义（DoD）

- [ ] Popover + MenuItem + DropdownMenu 实现 + 11+ 单元测试
- [ ] Select 弹层用 Popover，窗口下方没空间时自动向上翻转
- [ ] Toast 每条右侧显示 X 关闭按钮，点击立即 dismiss
- [ ] 质量门禁：fmt + clippy 0 warning + test 全过
- [ ] INDEX.md 更新 M14 条目
- [ ] Spec Risk R1 (trigger bounds 跨 Entity) 实际遇到 / 未遇到补记

## 8. 后续候选（M15+）

- ContextMenu（Popover + 右键触发 + 位置作 trigger bounds）
- DropdownMenu 加键盘导航（升级为 stateful Entity）
- Light theme 实施 + Settings Switch 真切
- TextInput mask 模式
- TextInput cursor_at_pixel 精确点击定位
- Dialog Tab focus trap
- Button hover variant 精细化
- PopoverTrigger 高级包装（trigger element 自动 wrap canvas + 自动 toggle popover）
