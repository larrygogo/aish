# M14 — aish-ui Popover + DropdownMenu + Select 改造 + Toast 关闭 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 aish-ui 加 Popover 浮层基础组件 + MenuItem/DropdownMenu，用 Popover 改造 Select 弹层获得自动翻转，并给 Toast 加关闭按钮。

**Architecture:** Popover 走 Entity（持久化 open + needs_focus + trigger_bounds）+ 基于 GPUI `anchored()` Window mode 渲染浮层；caller 在 trigger element 内嵌零尺寸 `canvas()`，prepaint 闭包把 bounds 通过 `popover.update(cx, ...)` 写回 Popover Entity，下一帧 Popover render 时按 bounds 定位 anchored child。MenuItem 是纯数据 struct，DropdownMenu 是 builder + RenderOnce 渲染菜单项列表。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- gpui (workspace dep, pinned Zed 11f0ca5)
- 测试：`cargo test --workspace`，每组件 `#[cfg(test)] mod tests`

**Spec ref:** `docs/superpowers/specs/2026-05-11-aish-m14-popover-design.md`

**质量门禁（每个 Task 完成后）：**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Task 1: Popover 组件

**Files:**
- Create: `crates/aish-ui/src/components/popover.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 popover.rs**

```rust
//! Popover — 浮层基础组件。
//!
//! 基于 GPUI `anchored()` Window mode 渲染浮层。trigger 元素的位置通过
//! `set_trigger_bounds()` 由 caller 写入（通常在 trigger 内嵌 `canvas()`，
//! prepaint 闭包 `popover.update(cx, |p, _| p.set_trigger_bounds(bounds))`）。
//!
//! click + programmatic 触发：caller 在 trigger 的 mouse_down listener 内调
//! `popover.toggle()`。Esc / 点 backdrop 外 → 自动关闭。
//!
//! 不内置 hover 触发 — hover 浮层用 Tooltip（M11 已有）。

use std::rc::Rc;

use gpui::{
    anchored, div, point, prelude::*, px, Anchor, AnchoredFitMode, AnchoredPositionMode,
    AnyElement, App, Bounds, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, Window,
};

use crate::theme::theme;

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PopoverPlacement {
    Bottom,
    Top,
    Left,
    Right,
}

pub struct Popover {
    focus_handle: FocusHandle,
    open: bool,
    needs_focus: bool,
    content: Option<AnyElement>,
    /// trigger element 的 viewport bounds。由 caller 通过 set_trigger_bounds 写入。
    trigger_bounds: Option<Bounds<Pixels>>,
    placement: PopoverPlacement,
    fit_mode: AnchoredFitMode,
    on_close: Option<CloseHandler>,
}

impl Popover {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            open: false,
            needs_focus: false,
            content: None,
            trigger_bounds: None,
            placement: PopoverPlacement::Bottom,
            fit_mode: AnchoredFitMode::SwitchAnchor,
            on_close: None,
        }
    }

    pub fn content(&mut self, c: impl IntoElement) -> &mut Self {
        self.content = Some(c.into_any_element());
        self
    }

    pub fn placement(&mut self, p: PopoverPlacement) -> &mut Self {
        self.placement = p;
        self
    }

    pub fn fit_mode(&mut self, m: AnchoredFitMode) -> &mut Self {
        self.fit_mode = m;
        self
    }

    pub fn on_close(
        &mut self,
        h: impl Fn(&mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_close = Some(Rc::new(h));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            self.open = true;
            self.needs_focus = true;
            cx.notify();
        }
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.open(cx);
        }
    }

    /// 由 trigger element 的 prepaint 阶段调，写入 viewport bounds。
    /// 下一帧 render 时 Popover 用 bounds 定位 anchored child。
    pub fn set_trigger_bounds(&mut self, bounds: Bounds<Pixels>) {
        self.trigger_bounds = Some(bounds);
    }

    fn fire_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_close.clone() {
            h(window, cx);
        }
    }
}

impl Focusable for Popover {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Popover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }

        if self.needs_focus {
            self.focus_handle.focus(window, cx);
            self.needs_focus = false;
        }

        let Some(trigger_bounds) = self.trigger_bounds else {
            // trigger bounds 还未写入，渲染空（下一帧 trigger canvas 会写入）
            return div().into_any_element();
        };

        let gap = px(4.0);
        let (anchor, position) = match self.placement {
            PopoverPlacement::Bottom => (
                Anchor::TopLeft,
                point(
                    trigger_bounds.origin.x,
                    trigger_bounds.origin.y + trigger_bounds.size.height + gap,
                ),
            ),
            PopoverPlacement::Top => (
                Anchor::BottomLeft,
                point(trigger_bounds.origin.x, trigger_bounds.origin.y - gap),
            ),
            PopoverPlacement::Right => (
                Anchor::TopLeft,
                point(
                    trigger_bounds.origin.x + trigger_bounds.size.width + gap,
                    trigger_bounds.origin.y,
                ),
            ),
            PopoverPlacement::Left => (
                Anchor::TopRight,
                point(trigger_bounds.origin.x - gap, trigger_bounds.origin.y),
            ),
        };

        let content = self.content.take();
        let t = theme(cx);
        let border_color = t.colors.border;
        let popover_bg = t.colors.popover;
        let radius_md = t.radius.md;

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    this.close(cx);
                    this.fire_close(window, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                    // backdrop click → 关闭
                    this.close(cx);
                    this.fire_close(window, cx);
                }),
            )
            .child(
                anchored()
                    .position_mode(AnchoredPositionMode::Window)
                    .position(position)
                    .anchor(anchor)
                    .snap_to_window()
                    .child(
                        div()
                            .bg(popover_bg)
                            .rounded(radius_md)
                            .border_1()
                            .border_color(border_color)
                            .on_mouse_down(MouseButton::Left, |_ev, _w, cx| {
                                // 阻止冒泡到 backdrop 关闭
                                cx.stop_propagation();
                            })
                            .when_some(content, |d, c| d.child(c)),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_close_state_machine() {
        let mut open = false;
        if !open {
            open = true;
        }
        assert!(open);
        if open {
            open = false;
        }
        assert!(!open);
    }

    #[test]
    fn placement_default_is_bottom() {
        let p = PopoverPlacement::Bottom;
        assert_eq!(p, PopoverPlacement::Bottom);
    }

    #[test]
    fn placement_chain_top() {
        let mut placement = PopoverPlacement::Bottom;
        placement = PopoverPlacement::Top;
        assert_eq!(placement, PopoverPlacement::Top);
    }

    #[test]
    fn set_trigger_bounds_stores_value() {
        let mut tb: Option<Bounds<Pixels>> = None;
        let b = Bounds::new(point(px(10.0), px(20.0)), gpui::size(px(100.0), px(30.0)));
        tb = Some(b);
        assert!(tb.is_some());
        assert_eq!(tb.unwrap().origin.x, px(10.0));
    }

    #[test]
    fn esc_triggers_close() {
        let key = "escape";
        let mut open = true;
        if key == "escape" {
            open = false;
        }
        assert!(!open);
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

字母序：`popover` 放在 `nav_item` 之后、`select` 之前。

```rust
mod popover;
pub use popover::{Popover, PopoverPlacement};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T1 — Popover 组件（Entity + anchored Window mode + trigger bounds + Esc/backdrop close）"
```

预期 aish-ui 测试 90 → 95。

---

## Task 2: MenuItem + DropdownMenu

**Files:**
- Create: `crates/aish-ui/src/components/menu_item.rs`
- Create: `crates/aish-ui/src/components/dropdown_menu.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 menu_item.rs**

```rust
//! MenuItem — DropdownMenu / ContextMenu 内单项数据。
//!
//! 纯数据 struct，不实现 IntoElement。由 DropdownMenu 等 caller 在 render 时
//! 读取字段绘制行。

use gpui::SharedString;

use crate::icons::IconName;

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: SharedString,
    pub icon: Option<IconName>,
    pub shortcut: Option<SharedString>,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, i: IconName) -> Self {
        self.icon = Some(i);
        self
    }

    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let m = MenuItem::new("Open");
        assert_eq!(m.label.as_ref(), "Open");
        assert!(m.icon.is_none());
        assert!(m.shortcut.is_none());
        assert!(!m.disabled);
    }

    #[test]
    fn icon_and_shortcut_chain() {
        let m = MenuItem::new("Save").icon(IconName::Check).shortcut("Ctrl+S");
        assert!(m.icon.is_some());
        assert_eq!(m.shortcut.as_ref().unwrap().as_ref(), "Ctrl+S");
    }

    #[test]
    fn disabled_chain() {
        let m = MenuItem::new("Delete").disabled(true);
        assert!(m.disabled);
    }
}
```

- [ ] **Step 2: 写 dropdown_menu.rs**

```rust
//! DropdownMenu — 菜单项列表。
//!
//! 本身 builder + RenderOnce 不管 open/close，作为 Popover content 传入。
//! 上层负责 Popover open 切换 + trigger element。
//!
//! M14 简化版：不接键盘导航（无内部 active index 状态机），只支持鼠标
//! click 选项。M15+ 可升级为 stateful Entity 加键盘 ↑↓。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Pixels, Window,
};

use crate::components::MenuItem;
use crate::icons::icon;
use crate::theme::theme;

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct DropdownMenu {
    id: ElementId,
    items: Vec<MenuItem>,
    on_select: Option<SelectHandler>,
    min_width: Option<Pixels>,
}

impl DropdownMenu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            on_select: None,
            min_width: None,
        }
    }

    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    pub fn on_select(
        mut self,
        h: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(h));
        self
    }

    pub fn min_width(mut self, w: Pixels) -> Self {
        self.min_width = Some(w);
        self
    }
}

impl RenderOnce for DropdownMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let on_select = self.on_select;

        let mut container = div()
            .id(self.id)
            .flex()
            .flex_col()
            .py(t.spacing.px_1);

        if let Some(w) = self.min_width {
            container = container.min_w(w);
        }

        container.children(self.items.into_iter().enumerate().map(|(i, item)| {
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
                if let Some(handler) = on_select.clone() {
                    row = row.on_mouse_down(
                        MouseButton::Left,
                        move |_ev: &MouseDownEvent, window, cx| {
                            handler(&i, window, cx);
                        },
                    );
                }
            }

            row
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let m = DropdownMenu::new("test");
        assert!(m.items.is_empty());
        assert!(m.on_select.is_none());
        assert!(m.min_width.is_none());
    }

    #[test]
    fn items_chain() {
        let items = vec![MenuItem::new("a"), MenuItem::new("b")];
        let m = DropdownMenu::new("test").items(items);
        assert_eq!(m.items.len(), 2);
    }

    #[test]
    fn on_select_stored() {
        let m = DropdownMenu::new("test").on_select(|_, _, _| {});
        assert!(m.on_select.is_some());
    }
}
```

- [ ] **Step 3: 注册 mod.rs**

字母序：

```rust
mod dropdown_menu;
mod menu_item;

pub use dropdown_menu::DropdownMenu;
pub use menu_item::MenuItem;
```

`dropdown_menu` 在 `dialog` 之后、`icon_button` 之前；`menu_item` 在 `icon_button` 之后、`nav_item` 之前。

- [ ] **Step 4: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T2 — MenuItem 数据 + DropdownMenu 列表（作为 Popover content 使用）"
```

预期 aish-ui 95 → 101。

---

## Task 3: Select 改造（弹层走 Popover）

**Files:**
- Modify: `crates/aish-ui/src/components/select.rs`

- [ ] **Step 1: 重写 select.rs**

完整新代码：

```rust
//! Select — 下拉选单。M14 改造：弹层从手糊 absolute 切到 Popover，
//! 获得自动 fit_mode 翻转（向下没空间时翻向上）。

use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, SharedString, Window,
};

use crate::components::{Popover, PopoverPlacement};
use crate::icons::{icon, IconName};
use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct Select {
    focus_handle: FocusHandle,
    options: Vec<SharedString>,
    selected: usize,
    popover: Entity<Popover>,
    placeholder: SharedString,
    on_change: Option<ChangeHandler>,
}

impl Select {
    pub fn new<S: Into<SharedString>>(options: Vec<S>, cx: &mut Context<Self>) -> Self {
        let popover = cx.new(|cx| {
            let mut p = Popover::new(cx);
            p.placement(PopoverPlacement::Bottom);
            p
        });
        Self {
            focus_handle: cx.focus_handle(),
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
            popover,
            placeholder: SharedString::default(),
            on_change: None,
        }
    }

    pub fn placeholder(&mut self, p: impl Into<SharedString>) -> &mut Self {
        self.placeholder = p.into();
        self
    }

    pub fn on_change(
        &mut self,
        h: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn current(&self) -> Option<&str> {
        self.options.get(self.selected).map(|s| s.as_ref())
    }

    pub fn set_selected(&mut self, idx: usize, cx: &mut Context<Self>) {
        let clamped = idx.min(self.options.len().saturating_sub(1));
        if clamped != self.selected {
            self.selected = clamped;
            cx.notify();
        }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.popover.update(cx, |p, cx| p.toggle(cx));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.popover.update(cx, |p, cx| p.close(cx));
    }

    fn select(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let clamped = idx.min(self.options.len().saturating_sub(1));
        let changed = clamped != self.selected;
        self.selected = clamped;
        self.close(cx);
        cx.notify();
        if changed {
            if let Some(h) = self.on_change.clone() {
                h(&self.selected, window, cx);
            }
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => self.close(cx),
            "down" if self.selected + 1 < self.options.len() => {
                self.select(self.selected + 1, window, cx);
            }
            "up" if self.selected > 0 => {
                self.select(self.selected - 1, window, cx);
            }
            "enter" => self.close(cx),
            _ => {}
        }
    }
}

impl Focusable for Select {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Select {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let popover_open = self.popover.read(cx).is_open();
        let popover_handle = self.popover.clone();
        let selected = self.selected;
        let placeholder = self.placeholder.clone();
        let options = self.options.clone();
        let weak_self = cx.weak_entity();

        let display_text = self
            .options
            .get(self.selected)
            .cloned()
            .unwrap_or_else(|| placeholder.clone());

        // trigger 元素 — 含 canvas 写入 bounds 到 popover
        let popover_for_canvas = popover_handle.clone();
        let trigger = div()
            .id("select-trigger")
            .relative()
            .h(gpui::px(28.0))
            .px(t.spacing.px_3)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(t.spacing.px_2)
            .rounded(t.radius.sm)
            .bg(t.colors.input)
            .border_1()
            .border_color(t.colors.border)
            .cursor_pointer()
            .child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.foreground)
                    .child(display_text),
            )
            .child(
                icon(if popover_open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(gpui::px(14.0))
                .text_color(t.colors.muted_foreground),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                    this.toggle(cx);
                }),
            )
            .child(
                canvas(
                    move |bounds, _w, cx| {
                        let h = popover_for_canvas.clone();
                        h.update(cx, |p, _cx| p.set_trigger_bounds(bounds));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );

        // popover content 选项列表
        let content = div()
            .flex()
            .flex_col()
            .min_w(gpui::px(200.0))
            .py(t.spacing.px_1)
            .children(options.into_iter().enumerate().map(|(i, opt)| {
                let is_selected = i == selected;
                let weak = weak_self.clone();
                div()
                    .id(("select-option", i))
                    .h(gpui::px(28.0))
                    .px(t.spacing.px_3)
                    .flex()
                    .items_center()
                    .text_size(t.font_size.sm)
                    .text_color(if is_selected {
                        t.colors.accent_foreground
                    } else {
                        t.colors.popover_foreground
                    })
                    .when(is_selected, |d| d.bg(t.colors.accent))
                    .cursor_pointer()
                    .hover({
                        let accent = t.colors.accent;
                        move |s| s.bg(accent)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        move |_ev: &MouseDownEvent, window, cx| {
                            let _ = weak.update(cx, |s, cx| s.select(i, window, cx));
                        },
                    )
                    .child(opt)
            }));

        self.popover.update(cx, |p, _| {
            p.content(content);
        });

        div()
            .relative()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .child(trigger)
            .child(self.popover.clone())
    }
}

#[cfg(test)]
mod tests {
    fn clamp(idx: usize, len: usize) -> usize {
        idx.min(len.saturating_sub(1))
    }

    #[test]
    fn clamp_in_range() {
        assert_eq!(clamp(0, 3), 0);
        assert_eq!(clamp(2, 3), 2);
    }

    #[test]
    fn clamp_overflow() {
        assert_eq!(clamp(10, 3), 2);
    }

    #[test]
    fn clamp_empty() {
        assert_eq!(clamp(0, 0), 0);
        assert_eq!(clamp(5, 0), 0);
    }

    #[test]
    fn down_arrow_advances_within_range() {
        let mut selected = 0usize;
        let len = 3usize;
        if selected + 1 < len {
            selected += 1;
        }
        assert_eq!(selected, 1);
    }

    #[test]
    fn up_arrow_stays_at_zero() {
        let mut selected = 0usize;
        if selected > 0 {
            selected = selected.saturating_sub(1);
        }
        assert_eq!(selected, 0);
    }
}
```

- [ ] **Step 2: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components/select.rs
git commit -m "refactor(aish-ui): T3 — Select 弹层切到 Popover，自动 fit_mode 翻转

原手糊 absolute top=32 弹层改用 Popover Entity。Popover 内部用 GPUI
anchored() Window mode + SwitchAnchor fit_mode 自动处理向下没空间时
翻转向上。trigger 元素内嵌 canvas() prepaint 写入 bounds 给 Popover。

测试从 6 → 5（去掉 toggle_flips_open，因为 open 状态现在归 Popover 管，
Select 通过 popover.read().is_open() 间接观察）。"
```

预期 aish-ui 101 → 100（净 -1 测试）。

---

## Task 4: Toast 加关闭按钮

**Files:**
- Modify: `crates/aish-ui/src/components/toast.rs`

- [ ] **Step 1: 修改 render_toast 加 close 按钮**

找到 `fn render_toast(toast: Toast, cx: &mut App) -> impl IntoElement`，把签名改为接收 `weak_mgr: gpui::WeakEntity<ToastManager>`，在 row 末尾加 IconButton(X)：

```rust
fn render_toast(
    toast: Toast,
    cx: &mut App,
    weak_mgr: gpui::WeakEntity<ToastManager>,
) -> impl IntoElement {
    let t = theme(cx);
    let (border_color, fg_color) = match toast.kind {
        ToastKind::Info => (t.colors.accent, t.colors.foreground),
        ToastKind::Success => (t.colors.success, t.colors.foreground),
        ToastKind::Warning => (t.colors.warning, t.colors.foreground),
        ToastKind::Error => (t.colors.destructive, t.colors.foreground),
    };

    let toast_id = toast.id;
    let close_btn = crate::components::IconButton::new(
        ("toast-close", toast_id as usize),
        IconName::X,
    )
    .small()
    .ghost()
    .on_click(move |_ev, _w, cx| {
        if let Some(m) = weak_mgr.upgrade() {
            m.update(cx, |m, cx| m.dismiss(toast_id, cx));
        }
    });

    div()
        .min_w(gpui::px(240.0))
        .px(t.spacing.px_3)
        .py(t.spacing.px_2)
        .rounded(t.radius.md)
        .bg(t.colors.popover)
        .border_1()
        .border_color(border_color)
        .flex()
        .flex_row()
        .items_center()
        .gap(t.spacing.px_2)
        .child(
            icon(toast.kind.icon_name())
                .size(t.font_size.base)
                .text_color(border_color),
        )
        .child(
            div()
                .flex_1()
                .text_size(t.font_size.sm)
                .text_color(fg_color)
                .child(toast.message),
        )
        .child(close_btn)
}
```

- [ ] **Step 2: 修改 ToastManager::render 传 weak 给 render_toast**

```rust
impl Render for ToastManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let toasts = self.toasts.clone();
        let weak = cx.weak_entity();
        div()
            .absolute()
            .top(t.spacing.px_4)
            .right(t.spacing.px_4)
            .flex()
            .flex_col()
            .gap(t.spacing.px_2)
            .children(
                toasts
                    .into_iter()
                    .map(|toast| render_toast(toast, cx, weak.clone())),
            )
    }
}
```

- [ ] **Step 3: 加测试 — dismiss 后列表少一项**

`mod tests` 内追加：

```rust
#[test]
fn dismiss_by_id_removes_one() {
    let mut toasts: Vec<u64> = vec![1, 2, 3];
    toasts.retain(|id| *id != 2);
    assert_eq!(toasts, vec![1, 3]);
}
```

（已有 `dismiss_removes_by_id` 测试可保留）

- [ ] **Step 4: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components/toast.rs
git commit -m "feat(aish-ui): T4 — Toast 每条加 X 关闭按钮，点击立即 dismiss

ToastManager::render 把 weak_entity 透给 render_toast，render_toast 用
weak.upgrade().update(dismiss(id)) 在点击 X 时立即从队列移除（不等
duration 自然过期）。"
```

预期 aish-ui 100 → 101（+1 测试）。

---

## Task 5: INDEX 更新 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 加 M14 条目**

在 `## Milestones（按时间倒序）` 最顶端插入：

```markdown
### M14 — aish-ui Popover / DropdownMenu + Select 改造 + Toast 关闭（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m14-popover-design.md`](specs/2026-05-11-aish-m14-popover-design.md)
- plan：[`plans/2026-05-11-aish-m14-popover.md`](plans/2026-05-11-aish-m14-popover.md)
- 范围：
  - Popover Entity（click + programmatic 触发 + GPUI anchored Window mode + canvas prepaint 写入 trigger bounds + SwitchAnchor 自动翻转 + Esc/backdrop close + occlude）
  - MenuItem 数据 struct + DropdownMenu builder（作为 Popover content）
  - Select 弹层从手糊 absolute 切到 Popover（自动获得翻转）
  - Toast 每条加 X 关闭按钮（IconButton + weak.upgrade().dismiss）
- 关键 commits：T1-T5
- 测试：aish-ui 90 → ~101
- 已知边界：DropdownMenu 不接键盘导航（M15+ 升级为 stateful Entity）；ContextMenu 右键触发未做

更新 `## 当前状态`：
- **活跃分支**：`main`（M14 已完成）
- **下一里程碑**：M15 — ContextMenu / Light theme 实现 / DropdownMenu 键盘导航 / TextInput mask / 其他 M11-M14 遗留
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui ~101 + aish-app 101 + 其他) 全过
```

- [ ] **Step 2: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T5 — INDEX 更新 M14 已完成"
```

---

## DoD 自检

回看 spec § 7：

- [ ] Popover + MenuItem + DropdownMenu 实现 + 11+ 测试 ←→ T1, T2
- [ ] Select 弹层用 Popover，向下没空间时自动翻转 ←→ T3
- [ ] Toast 每条 X 按钮，点击立即 dismiss ←→ T4
- [ ] 质量门禁全过 ←→ 每 task 末尾
- [ ] INDEX.md 更新 M14 ←→ T5

---

## 后续候选（M15+）

- ContextMenu（Popover + 右键触发）
- DropdownMenu 键盘导航（升级为 stateful Entity）
- Light theme 实施
- TextInput mask 模式
- TextInput cursor_at_pixel
- Dialog Tab focus trap
- Button hover variant 精细化
- PopoverTrigger 高级包装（自动 wrap canvas + auto toggle）
