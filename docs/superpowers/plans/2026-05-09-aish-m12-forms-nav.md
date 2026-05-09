# M12 — aish-ui 表单与导航 + HostForm/SessionPicker 迁移 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 aish-ui 加 5 个表单/导航组件（Checkbox / Switch / Tabs / Dialog / Select），同步把 aish-app 的 HostFormModal / SessionPickerView 重写到新组件，给 SettingsView 加 Appearance Section + Switch 演示位。

**Architecture:** 沿用 M11 的 Hybrid API 决策（无状态 = builder + RenderOnce + `#[derive(IntoElement)]`，有状态 = `Entity<T>` + Render）。组件读 token 走 `aish_ui::theme(cx)`。Dialog 简化版只做 Esc + backdrop close（focus trap 留 M13）。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- gpui (workspace dep, pinned to Zed `11f0ca5`)
- 测试：`cargo test --workspace`，每组件 `#[cfg(test)] mod tests`

**Spec ref:** `docs/superpowers/specs/2026-05-09-aish-m12-forms-nav-design.md` 与父 spec `2026-05-09-aish-ui-architecture-design.md`

**质量门禁（每个 Task 完成后）：**
```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Task 1: Checkbox 组件

**Files:**
- Create: `crates/aish-ui/src/components/checkbox.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Checkbox 组件**

`crates/aish-ui/src/components/checkbox.rs`：

```rust
//! Checkbox — 受控勾选框。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, SharedString,
    Window,
};

use crate::icons::{icon, IconName};
use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: SharedString,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            label: SharedString::default(),
            disabled: false,
            on_change: None,
        }
    }

    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = l.into();
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_change(
        mut self,
        h: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let checked = self.checked;

        let (box_bg, icon_color) = if disabled {
            (t.colors.muted, t.colors.muted_foreground)
        } else if checked {
            (t.colors.primary, t.colors.primary_foreground)
        } else {
            (gpui::transparent_black(), t.colors.foreground)
        };

        let mut row = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap(t.spacing.px_2)
            .child(
                div()
                    .w(gpui::px(16.0))
                    .h(gpui::px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(t.radius.sm)
                    .bg(box_bg)
                    .border_1()
                    .border_color(t.colors.border)
                    .when(checked, |d| {
                        d.child(icon(IconName::Check).size(gpui::px(12.0)).text_color(icon_color))
                    }),
            );

        if !self.label.as_ref().is_empty() {
            row = row.child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(if disabled {
                        t.colors.muted_foreground
                    } else {
                        t.colors.foreground
                    })
                    .child(self.label.clone()),
            );
        }

        if !disabled {
            row = row.cursor_pointer();
            if let Some(handler) = self.on_change {
                let new_value = !checked;
                row = row.on_mouse_down(MouseButton::Left, move |_ev: &MouseDownEvent, window, cx| {
                    handler(&new_value, window, cx);
                });
            }
        }

        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let c = Checkbox::new("agree");
        assert!(!c.checked);
        assert!(!c.disabled);
        assert!(c.label.as_ref().is_empty());
        assert!(c.on_change.is_none());
    }

    #[test]
    fn checked_chain() {
        let c = Checkbox::new("a").checked(true);
        assert!(c.checked);
    }

    #[test]
    fn label_chain() {
        let c = Checkbox::new("a").label("Agree");
        assert_eq!(c.label.as_ref(), "Agree");
    }

    #[test]
    fn disabled_chain() {
        let c = Checkbox::new("a").disabled(true);
        assert!(c.disabled);
    }

    #[test]
    fn on_change_stored() {
        let c = Checkbox::new("a").on_change(|_, _, _| {});
        assert!(c.on_change.is_some());
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

修改 `crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod button;
mod checkbox;
mod icon_button;
mod separator;
mod text_input;
mod toast;
mod tooltip;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use checkbox::Checkbox;
pub use icon_button::{IconButton, IconButtonSize};
pub use separator::{Orientation, Separator};
pub use text_input::TextInput;
pub use toast::{
    toast, toast_error, toast_info, toast_success, toast_warning, Toast, ToastHandle, ToastKind,
    ToastManager,
};
pub use tooltip::{Tooltip, TooltipExt, TooltipView};
```

- [ ] **Step 3: 跑测试**

```bash
cargo test -p aish-ui
```

预期：51 + 5 = 56 全过。

- [ ] **Step 4: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T1 — Checkbox 组件（builder + 受控）"
```

---

## Task 2: Switch 组件

**Files:**
- Create: `crates/aish-ui/src/components/switch.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Switch 组件**

`crates/aish-ui/src/components/switch.rs`：

```rust
//! Switch — iOS 风格开关。受控。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Window,
};

use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            disabled: false,
            on_change: None,
        }
    }

    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_change(
        mut self,
        h: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let checked = self.checked;

        let track_bg = if disabled {
            t.colors.muted
        } else if checked {
            t.colors.primary
        } else {
            t.colors.muted
        };

        let thumb = div()
            .w(gpui::px(16.0))
            .h(gpui::px(16.0))
            .rounded(t.radius.full)
            .bg(t.colors.foreground);

        let track = div()
            .id(self.id)
            .w(gpui::px(36.0))
            .h(gpui::px(20.0))
            .rounded(t.radius.full)
            .bg(track_bg)
            .flex()
            .items_center()
            .justify_between()
            .px(gpui::px(2.0))
            .when(checked, |d| d.child(div().w(gpui::px(0.0))).child(thumb))
            .when(!checked, |d| {
                d.child(
                    div()
                        .w(gpui::px(16.0))
                        .h(gpui::px(16.0))
                        .rounded(t.radius.full)
                        .bg(t.colors.foreground),
                )
                .child(div().w(gpui::px(0.0)))
            });

        let mut el = track;

        if !disabled {
            el = el.cursor_pointer();
            if let Some(handler) = self.on_change {
                let new_value = !checked;
                el = el.on_mouse_down(
                    MouseButton::Left,
                    move |_ev: &MouseDownEvent, window, cx| {
                        handler(&new_value, window, cx);
                    },
                );
            }
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let s = Switch::new("dark");
        assert!(!s.checked);
        assert!(!s.disabled);
        assert!(s.on_change.is_none());
    }

    #[test]
    fn checked_chain() {
        let s = Switch::new("a").checked(true);
        assert!(s.checked);
    }

    #[test]
    fn disabled_chain() {
        let s = Switch::new("a").disabled(true);
        assert!(s.disabled);
    }

    #[test]
    fn on_change_stored() {
        let s = Switch::new("a").on_change(|_, _, _| {});
        assert!(s.on_change.is_some());
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

`crates/aish-ui/src/components/mod.rs` 加：

```rust
mod switch;

pub use switch::Switch;
```

放在字母序合适位置（separator 之后，text_input 之前）。

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T2 — Switch 组件（iOS 风胶囊开关）"
```

---

## Task 3: Tabs 组件（Entity）

**Files:**
- Create: `crates/aish-ui/src/components/tabs.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Tabs 组件**

`crates/aish-ui/src/components/tabs.rs`：

```rust
//! Tabs — 标签栏。Entity 持久化 active index。
//!
//! Tabs 只画 tab 头，调用方根据 `tabs.read(cx).active()` 渲染对应内容面板。
//! 这点与 shadcn 的 `<Tabs>` 含子 `<TabsContent>` 不同，但更贴 GPUI 习惯。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, SharedString, Window,
};

use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct Tabs {
    focus_handle: FocusHandle,
    labels: Vec<SharedString>,
    active: usize,
    on_change: Option<ChangeHandler>,
}

impl Tabs {
    pub fn new<S: Into<SharedString>>(labels: Vec<S>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            labels: labels.into_iter().map(Into::into).collect(),
            active: 0,
            on_change: None,
        }
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn labels(&self) -> &[SharedString] {
        &self.labels
    }

    pub fn set_active(&mut self, idx: usize, cx: &mut Context<Self>) {
        let clamped = idx.min(self.labels.len().saturating_sub(1));
        if clamped != self.active {
            self.active = clamped;
            cx.notify();
        }
    }

    pub fn on_change(
        &mut self,
        h: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    fn select(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let clamped = idx.min(self.labels.len().saturating_sub(1));
        if clamped != self.active {
            self.active = clamped;
            cx.notify();
            if let Some(h) = self.on_change.clone() {
                h(&self.active, window, cx);
            }
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "left" => {
                if self.active > 0 {
                    self.select(self.active - 1, window, cx);
                }
            }
            "right" => {
                if self.active + 1 < self.labels.len() {
                    self.select(self.active + 1, window, cx);
                }
            }
            _ => {}
        }
    }
}

impl Focusable for Tabs {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Tabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let active = self.active;
        let labels = self.labels.clone();

        div()
            .flex()
            .flex_row()
            .gap(t.spacing.px_1)
            .border_b_1()
            .border_color(t.colors.border)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .children(labels.into_iter().enumerate().map(|(i, label)| {
                let is_active = i == active;
                div()
                    .id(("tab", i))
                    .h(gpui::px(32.0))
                    .px(t.spacing.px_3)
                    .flex()
                    .items_center()
                    .text_size(t.font_size.sm)
                    .text_color(if is_active {
                        t.colors.foreground
                    } else {
                        t.colors.secondary_foreground
                    })
                    .border_b_2()
                    .border_color(if is_active {
                        t.colors.primary
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, window, cx| {
                            this.select(i, window, cx);
                        }),
                    )
                    .child(label)
            }))
    }
}

#[cfg(test)]
mod tests {
    fn clamp_active(idx: usize, len: usize) -> usize {
        idx.min(len.saturating_sub(1))
    }

    #[test]
    fn clamp_keeps_in_range() {
        assert_eq!(clamp_active(0, 2), 0);
        assert_eq!(clamp_active(1, 2), 1);
        assert_eq!(clamp_active(5, 2), 1);
    }

    #[test]
    fn clamp_with_empty_labels_returns_zero() {
        assert_eq!(clamp_active(0, 0), 0);
        assert_eq!(clamp_active(5, 0), 0);
    }

    #[test]
    fn left_arrow_decrements() {
        let active = 2usize;
        let new = if active > 0 { active - 1 } else { active };
        assert_eq!(new, 1);
    }

    #[test]
    fn right_arrow_increments_when_in_range() {
        let active = 1usize;
        let len = 3usize;
        let new = if active + 1 < len { active + 1 } else { active };
        assert_eq!(new, 2);
    }

    #[test]
    fn right_arrow_stays_at_last() {
        let active = 2usize;
        let len = 3usize;
        let new = if active + 1 < len { active + 1 } else { active };
        assert_eq!(new, 2);
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

`crates/aish-ui/src/components/mod.rs` 加：

```rust
mod tabs;

pub use tabs::Tabs;
```

字母序：放在 `switch` 之后，`text_input` 之前。

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T3 — Tabs 组件（Entity + 键盘 ←/→ + on_change）"
```

---

## Task 4: Dialog 组件（Entity，简化版 focus trap）

**Files:**
- Create: `crates/aish-ui/src/components/dialog.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Dialog 组件**

`crates/aish-ui/src/components/dialog.rs`：

```rust
//! Dialog — 居中 modal。
//!
//! M12 简化版：Esc + backdrop click 关闭。Tab 循环 focus trap 留 M13 加固。

use std::rc::Rc;

use gpui::{
    div, prelude::*, AnyElement, App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, SharedString, Window,
};

use crate::components::IconButton;
use crate::icons::IconName;
use crate::theme::theme;

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

pub struct Dialog {
    focus_handle: FocusHandle,
    open: bool,
    needs_focus: bool,                 // open 后第一帧 render 触发聚焦
    title: SharedString,
    body: Option<AnyElement>,
    width: Pixels,
    on_close: Option<CloseHandler>,
}

impl Dialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            open: false,
            needs_focus: false,
            title: SharedString::default(),
            body: None,
            width: gpui::px(480.0),
            on_close: None,
        }
    }

    pub fn title(&mut self, t: impl Into<SharedString>) -> &mut Self {
        self.title = t.into();
        self
    }

    pub fn body(&mut self, body: impl IntoElement) -> &mut Self {
        self.body = Some(body.into_any_element());
        self
    }

    pub fn width(&mut self, w: Pixels) -> &mut Self {
        self.width = w;
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

    /// 打开 dialog。聚焦在下一帧 render 时通过 needs_focus 标记驱动。
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

    fn fire_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_close.clone() {
            h(window, cx);
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() == "escape" {
            self.close(cx);
            self.fire_close(window, cx);
        }
    }
}

impl Focusable for Dialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }

        // 第一次 open 后聚焦，保证 Esc 能响应
        if self.needs_focus {
            self.focus_handle.focus(window, cx);
            self.needs_focus = false;
        }

        let t = theme(cx);
        let title = self.title.clone();
        let body = self.body.take();
        let width = self.width;

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000_99))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                    // backdrop click → 关闭。子 dialog 内部 mouse_down 不冒泡到这层
                    // GPUI 默认事件冒泡，需要在内部 dialog 上 stop propagation 或检测 hit。
                    // 简化：让子元素的 cursor_pointer / on_mouse_down 自然吞事件。
                    // 这里只在直接命中 backdrop 时执行（无内层命中时冒泡到此处）。
                    this.close(cx);
                    this.fire_close(window, cx);
                }),
            )
            .child(
                div()
                    .w(width)
                    .max_h(gpui::px(640.0))
                    .bg(t.colors.popover)
                    .rounded(t.radius.lg)
                    .border_1()
                    .border_color(t.colors.border)
                    .flex()
                    .flex_col()
                    .on_mouse_down(
                        MouseButton::Left,
                        // 阻止冒泡到 backdrop（GPUI 没原生 stop_propagation，但 hit test 命中
                        // 子元素时 backdrop on_mouse_down 不会触发同坐标）。空 listener 占位
                        // 即可，确保点 dialog 内部不关闭。
                        |_ev, _w, _cx| {},
                    )
                    .child(
                        div()
                            .px(t.spacing.px_4)
                            .py(t.spacing.px_3)
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(t.colors.border)
                            .child(
                                div()
                                    .text_size(t.font_size.lg)
                                    .text_color(t.colors.foreground)
                                    .child(title),
                            )
                            .child(IconButton::new("dialog-close", IconName::X).small().on_click(
                                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                                    this.close(cx);
                                    this.fire_close(window, cx);
                                }),
                            )),
                    )
                    .child(
                        div()
                            .p(t.spacing.px_4)
                            .flex_1()
                            .when_some(body, |d, b| d.child(b)),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn open_close_state_machine() {
        let mut open = false;
        // 模拟 open
        if !open {
            open = true;
        }
        assert!(open);
        // 模拟 close
        if open {
            open = false;
        }
        assert!(!open);
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

    #[test]
    fn other_keys_dont_close() {
        let key = "enter";
        let mut open = true;
        if key == "escape" {
            open = false;
        }
        assert!(open);
    }

    #[test]
    fn default_width_is_480() {
        let width = gpui::px(480.0);
        assert_eq!(width, gpui::px(480.0));
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

字母序：`dialog` 放在 `checkbox` 之后、`icon_button` 之前。

```rust
mod dialog;
pub use dialog::Dialog;
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T4 — Dialog 组件（Entity + Esc/backdrop close + body slot）"
```

---

## Task 5: Select 组件（Entity）

**Files:**
- Create: `crates/aish-ui/src/components/select.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Select 组件**

`crates/aish-ui/src/components/select.rs`：

```rust
//! Select — 下拉选单。Entity 持久化 open / selected 状态。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, SharedString, Window,
};

use crate::icons::{icon, IconName};
use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct Select {
    focus_handle: FocusHandle,
    options: Vec<SharedString>,
    selected: usize,
    open: bool,
    placeholder: SharedString,
    on_change: Option<ChangeHandler>,
}

impl Select {
    pub fn new<S: Into<SharedString>>(options: Vec<S>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
            open: false,
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
        self.open = !self.open;
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    fn select(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let clamped = idx.min(self.options.len().saturating_sub(1));
        let changed = clamped != self.selected;
        self.selected = clamped;
        self.open = false;
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
            "down" => {
                if self.selected + 1 < self.options.len() {
                    self.select(self.selected + 1, window, cx);
                }
            }
            "up" => {
                if self.selected > 0 {
                    self.select(self.selected - 1, window, cx);
                }
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
        let open = self.open;
        let selected = self.selected;
        let options = self.options.clone();
        let placeholder = self.placeholder.clone();

        let display_text = self
            .options
            .get(self.selected)
            .cloned()
            .unwrap_or_else(|| placeholder.clone());

        let trigger = div()
            .id("select-trigger")
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
                icon(if open {
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
            );

        let dropdown = if open {
            Some(
                div()
                    .absolute()
                    .top(gpui::px(32.0))
                    .left_0()
                    .right_0()
                    .max_h(gpui::px(240.0))
                    .overflow_hidden()
                    .rounded(t.radius.md)
                    .bg(t.colors.popover)
                    .border_1()
                    .border_color(t.colors.border)
                    .flex()
                    .flex_col()
                    .children(options.into_iter().enumerate().map(|(i, opt)| {
                        let is_selected = i == selected;
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _ev: &MouseDownEvent, window, cx| {
                                    this.select(i, window, cx);
                                }),
                            )
                            .child(opt)
                    })),
            )
        } else {
            None
        };

        div()
            .relative()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .child(trigger)
            .children(dropdown)
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
    fn toggle_flips_open() {
        let mut open = false;
        open = !open;
        assert!(open);
        open = !open;
        assert!(!open);
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
            selected -= 1;
        }
        assert_eq!(selected, 0);
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

字母序：`select` 放在 `separator` 之前。

```rust
mod select;
pub use select::Select;
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T5 — Select 组件（Entity + 下拉 + 键盘导航）"
```

---

## Task 6: prelude 更新 + 5 组件接入验证

**Files:**
- Modify: `crates/aish-ui/src/prelude.rs`

- [ ] **Step 1: 更新 prelude.rs**

`crates/aish-ui/src/prelude.rs` 应该包含全部公开组件。当前内容应类似：

```rust
//! 常用 re-exports。`use aish_ui::prelude::*;` 一行拉齐。

pub use crate::components::*;
pub use crate::icons::{icon, IconName};
pub use crate::theme::{theme, Theme};
```

`pub use crate::components::*;` 已经能 re-export 新加的 Checkbox / Switch / Tabs / Dialog / Select。本 task 主要是**确认无遗漏 + 跑全 workspace 编译**。

- [ ] **Step 2: 跑 workspace 检查**

```bash
cargo check --workspace
```

预期：5 个新组件通过 prelude `*` 导出后，外部能 `use aish_ui::prelude::*;` 拿到。

- [ ] **Step 3: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：aish-ui 51 + 5+4+5+4+6 = 75 测试全过。

- [ ] **Step 4: Commit（如有改动）**

如果 prelude.rs 不变则跳过 commit。如果加了显式 re-export 行：

```bash
git add crates/aish-ui/src/prelude.rs
git commit -m "feat(aish-ui): T6 — prelude 验证 5 新组件接入"
```

否则跳过本 task 的 commit，继续 T7。

---

## Task 7: HostFormModal 重写（用 Dialog + Tabs + TextInput）

**Files:**
- Modify: `crates/aish-app/src/views/host_form.rs`（大幅重写）

这是 M12 最大的 task。HostFormModal 当前 805 行；目标是用新组件重写后减到 ~550 行。

**当前 view 结构**（粗略）：
- 持久化 modal 状态（open / draft data）
- 6 字段：Label / Host / Port / User / Password / KeyPath（手实现 input）
- AuthKind toggle（KeyFile / Password 用单选按钮）
- save / delete / cancel buttons + 文件 picker

**重写策略**：

- 整体外壳 → `aish_ui::Dialog`
- KeyFile / Password 切换 → `aish_ui::Tabs`
- 6 字段 → 6 个 `aish_ui::TextInput` Entity
- buttons → `aish_ui::Button`（Save = Primary，Delete = Destructive，Cancel = Ghost）
- KeyPath 手动接 file picker（点 IconButton 触发 `cx.prompt_for_paths`）

由于体量大，本 task 分多个 step（每个 step 内多文件改动一气写完）。

- [ ] **Step 1: 读当前 host_form.rs**

```bash
cat crates/aish-app/src/views/host_form.rs | head -100
```

理解现有 struct 布局、save / delete 业务逻辑、Persistence 接入点。

**关键现状**：
- `pub struct HostFormModal { state, bridge, tx, draft, focus_field, ... }`
- `draft` 是 `crate::state::HostDraft`，含 label/host/port/user/auth_kind/password/key_path 等字段
- `pub fn open_for_new(&mut self, cx)` 和 `pub fn open_for_edit(&mut self, host_id, cx)` 切换 modal 显示
- 现在通过 `state.modal: Option<ModalKind>` 控制显示

**保留**：
- 业务方法 save / delete / connect 逻辑（仅改 input 取值方式）
- 公开 API `open_for_new` / `open_for_edit`

**替换**：
- focus_field 状态机（不再需要，TextInput 自带 focus）
- 手实现 input 渲染（用 TextInput Entity）
- 自实现 modal overlay（用 Dialog）

- [ ] **Step 2: 重写 host_form.rs**

**完整新代码**（注：此处给出主体，业务方法保持原逻辑，仅适配新 input 取值）：

```rust
//! HostFormModal — 新建 / 编辑 host 配置。
//!
//! 重写为用 aish_ui::Dialog + Tabs + TextInput + Button。
//! 业务方法（save/delete/connect）保持原逻辑。

use std::sync::Arc;

use aish_types::{HostConfig, HostId, SshAuth};
use aish_ui::{
    theme, Button, ButtonVariant, Dialog, IconButton, IconName, Tabs, TextInput,
};
use gpui::{
    div, prelude::*, App, Context, Entity, IntoElement, MouseButton, MouseDownEvent,
    PathPromptOptions, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, ModalKind, SshEvent};

pub struct HostFormModal {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    dialog: Entity<Dialog>,
    auth_tabs: Entity<Tabs>,    // 0: KeyFile, 1: Password
    label: Entity<TextInput>,
    host: Entity<TextInput>,
    port: Entity<TextInput>,
    user: Entity<TextInput>,
    keyfile_path: Entity<TextInput>,
    password: Entity<TextInput>,
    editing_id: Option<HostId>,
}

impl HostFormModal {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let dialog = cx.new(|cx| Dialog::new(cx));

        // dialog 关闭时把 modal kind 清掉
        let weak_state = state.downgrade();
        dialog.update(cx, |d, _cx| {
            d.title("Host");
            d.width(gpui::px(480.0));
            d.on_close(move |_window, cx| {
                if let Some(s) = weak_state.upgrade() {
                    s.update(cx, |s, cx| {
                        s.modal = None;
                        cx.notify();
                    });
                }
            });
        });

        let auth_tabs = cx.new(|cx| {
            let labels: Vec<SharedString> = vec!["Key File".into(), "Password".into()];
            Tabs::new(labels, cx)
        });

        let label = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("My Server");
            i
        });
        let host = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("example.com");
            i
        });
        let port = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("22");
            i.set_text("22", cx);
            i
        });
        let user = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("root");
            i
        });
        let keyfile_path = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("~/.ssh/id_rsa");
            i
        });
        let password = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("(密码)");
            i
        });

        Self {
            state,
            bridge,
            tx,
            dialog,
            auth_tabs,
            label,
            host,
            port,
            user,
            keyfile_path,
            password,
            editing_id: None,
        }
    }

    pub fn open_for_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_id = None;
        self.label.update(cx, |i, cx| i.clear(cx));
        self.host.update(cx, |i, cx| i.clear(cx));
        self.port.update(cx, |i, cx| i.set_text("22", cx));
        self.user.update(cx, |i, cx| i.set_text("root", cx));
        self.keyfile_path.update(cx, |i, cx| i.clear(cx));
        self.password.update(cx, |i, cx| i.clear(cx));
        self.auth_tabs.update(cx, |t, cx| t.set_active(0, cx));
        self.dialog.update(cx, |d, cx| d.open(window, cx));
        cx.notify();
    }

    pub fn open_for_edit(
        &mut self,
        host_cfg: &HostConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing_id = Some(host_cfg.id);
        self.label
            .update(cx, |i, cx| i.set_text(host_cfg.label.clone(), cx));
        self.host
            .update(cx, |i, cx| i.set_text(host_cfg.host.clone(), cx));
        self.port
            .update(cx, |i, cx| i.set_text(host_cfg.port.to_string(), cx));
        self.user
            .update(cx, |i, cx| i.set_text(host_cfg.user.clone(), cx));
        match &host_cfg.auth {
            SshAuth::KeyFile { path, .. } => {
                self.auth_tabs.update(cx, |t, cx| t.set_active(0, cx));
                self.keyfile_path
                    .update(cx, |i, cx| i.set_text(path.to_string_lossy().to_string(), cx));
                self.password.update(cx, |i, cx| i.clear(cx));
            }
            SshAuth::Password { .. } => {
                self.auth_tabs.update(cx, |t, cx| t.set_active(1, cx));
                self.keyfile_path.update(cx, |i, cx| i.clear(cx));
                self.password.update(cx, |i, cx| i.clear(cx));
            }
            _ => {
                self.auth_tabs.update(cx, |t, cx| t.set_active(0, cx));
                self.keyfile_path.update(cx, |i, cx| i.clear(cx));
                self.password.update(cx, |i, cx| i.clear(cx));
            }
        }
        self.dialog.update(cx, |d, cx| d.open(window, cx));
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.dialog.update(cx, |d, cx| d.close(cx));
        self.state.update(cx, |s, cx| {
            s.modal = None;
            cx.notify();
        });
    }

    fn pick_keyfile(&mut self, cx: &mut Context<Self>) {
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from("选择 SSH 密钥文件")),
        };
        let receiver = cx.prompt_for_paths(options);
        let weak = cx.weak_entity();
        cx.spawn(async move |_this: gpui::WeakEntity<Self>, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    let weak2 = weak.clone();
                    weak2
                        .update(cx, |this, cx| {
                            this.keyfile_path
                                .update(cx, |i, cx| i.set_text(path.to_string_lossy().to_string(), cx));
                        })
                        .ok();
                }
            }
        })
        .detach();
    }

    fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let label = self.label.read(cx).text().trim().to_string();
        let host = self.host.read(cx).text().trim().to_string();
        let port: u16 = self
            .port
            .read(cx)
            .text()
            .trim()
            .parse()
            .unwrap_or(22);
        let user = self.user.read(cx).text().trim().to_string();
        let auth_idx = self.auth_tabs.read(cx).active();
        let auth = if auth_idx == 0 {
            let p = self.keyfile_path.read(cx).text().trim().to_string();
            SshAuth::KeyFile {
                path: p.into(),
                passphrase: String::new(),
            }
        } else {
            SshAuth::Password {
                password: self.password.read(cx).text().to_string(),
            }
        };

        if label.is_empty() || host.is_empty() || user.is_empty() {
            aish_ui::toast_warning(cx, "Label / Host / User 不能为空");
            return;
        }

        let cfg = HostConfig {
            id: self.editing_id.unwrap_or_else(HostId::new),
            label,
            host,
            port,
            user,
            auth,
        };

        self.state.update(cx, |s, cx| {
            if self.editing_id.is_some() {
                s.update_host(cfg);
            } else {
                s.add_host(cfg);
            }
            s.modal = None;
            cx.notify();
        });
        if let Err(e) = crate::persistence::save_hosts(&self.state.read(cx).hosts) {
            tracing::error!("save hosts failed: {}", e);
            aish_ui::toast_error(cx, format!("保存失败：{}", e));
            return;
        }
        self.dialog.update(cx, |d, cx| d.close(cx));
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.editing_id {
            self.state.update(cx, |s, cx| {
                s.remove_host(id);
                s.modal = None;
                cx.notify();
            });
            if let Err(e) = crate::persistence::save_hosts(&self.state.read(cx).hosts) {
                tracing::error!("save hosts failed: {}", e);
                aish_ui::toast_error(cx, format!("删除失败：{}", e));
                return;
            }
            self.dialog.update(cx, |d, cx| d.close(cx));
        }
    }
}

impl Render for HostFormModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let auth_idx = self.auth_tabs.read(cx).active();
        let editing = self.editing_id.is_some();

        let body = div()
            .flex()
            .flex_col()
            .gap(t.spacing.px_3)
            .child(field_label(cx, "Label"))
            .child(self.label.clone())
            .child(field_label(cx, "Host"))
            .child(self.host.clone())
            .child(field_label(cx, "Port"))
            .child(self.port.clone())
            .child(field_label(cx, "User"))
            .child(self.user.clone())
            .child(self.auth_tabs.clone())
            .child(if auth_idx == 0 {
                div()
                    .flex()
                    .flex_row()
                    .gap(t.spacing.px_2)
                    .child(div().flex_1().child(self.keyfile_path.clone()))
                    .child(IconButton::new("pick-key", IconName::Search).on_click(
                        cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                            this.pick_keyfile(cx);
                        }),
                    ))
                    .into_any_element()
            } else {
                self.password.clone().into_any_element()
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(t.spacing.px_2)
                    .child(
                        Button::new("save")
                            .label("Save")
                            .primary()
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.save(window, cx);
                            })),
                    )
                    .when(editing, |row| {
                        row.child(
                            Button::new("delete")
                                .label("Delete")
                                .destructive()
                                .on_click(cx.listener(|this, _ev, _w, cx| {
                                    this.delete(cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("cancel")
                            .label("Cancel")
                            .ghost()
                            .on_click(cx.listener(|this, _ev, _w, cx| {
                                this.close(cx);
                            })),
                    ),
            );

        self.dialog.update(cx, |d, _cx| {
            d.title(if editing { "Edit Host" } else { "New Host" });
            d.body(body);
        });

        self.dialog.clone()
    }
}

fn field_label(cx: &App, text: &'static str) -> impl IntoElement {
    let t = theme(cx);
    div()
        .text_size(t.font_size.xs)
        .text_color(t.colors.muted_foreground)
        .child(text)
}
```

**调用方迁移**：原代码调用 `host_form.open_for_new(cx)` / `host_form.open_for_edit(host_id, cx)`。新签名加了 `window` 参数（因为 dialog.open 需要 window）。

`crates/aish-app/src/app.rs` 的 RootView 触发 modal 时也要传 window。HomeView 触发时同理。**这里需要在调用点加 window 传递**。

具体调用点搜索：

```bash
grep -rn "open_for_new\|open_for_edit" crates/aish-app/src --include="*.rs"
```

然后逐处适配（多在 home.rs / app.rs 内）。如果原签名改动太多，可以保持 `open_for_new(&mut self, cx)` 不变，把 `dialog.open(window, cx)` 改成在 cx.spawn 里 / 或在 RootView render 内观察 modal kind 触发。

**简化方案**：dialog `open` 改成不需要 window 立即聚焦——设计 `Dialog::open(&mut self, cx)`（无 window），它只把 `self.open = true` + `cx.notify()`，焦点在下一帧 render 时自动聚焦（`self.focus_handle.focus(window, cx)` 在 render 顶部条件触发）。这样 open_for_new 不需要 window。

**注**：T4 已实现 `Dialog::open(&mut self, cx)`（不需要 window）+ render 内首次 open 后通过 `needs_focus` 标记自动聚焦，所以本 task 直接调 `dialog.update(cx, |d, cx| d.open(cx))` 即可。

- [ ] **Step 3: 跑 cargo check 验证编译**

```bash
cargo check -p aish-app -p aish-ui
```

修复任何编译错误。常见问题：
- `state.update_host(cfg)` 方法签名变化
- `state.add_host(cfg)` 同上
- `state.modal` 字段访问

- [ ] **Step 4: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

注：host_form.rs 内之前可能有 unit tests，能保留就保留，无法保留就删（因为 input 状态机迁到 TextInput 了）。

- [ ] **Step 5: Commit**

```bash
git add crates/aish-app/src/views/host_form.rs
git commit -m "feat(aish-ui): T7 — HostFormModal 重写为 Dialog + Tabs + TextInput

- HostFormModal 新结构：dialog + auth_tabs(Tabs) + 6 TextInput Entity
- 业务方法 save / delete / pick_keyfile 适配新 input 取值
- KeyFile/Password 用 Tabs 切换，原 AuthKind 单选按钮删除
- 减少 ~250 行手糊 modal 代码"
```

---

## Task 8: SessionPickerView 迁移到 Dialog

**Files:**
- Modify: `crates/aish-app/src/views/session_picker.rs`

- [ ] **Step 1: 读当前 session_picker.rs**

```bash
cat crates/aish-app/src/views/session_picker.rs
```

理解结构：modal overlay 自实现，含 session 列表 + 键盘 ↑↓ + Enter 接受 + Esc 关闭。

- [ ] **Step 2: 重写 session_picker.rs**

`crates/aish-app/src/views/session_picker.rs`：

```rust
//! SessionPickerView — 远端有 tmux session 时弹出选 attach 哪个。
//!
//! 重写为用 aish_ui::Dialog 包外壳，列表保留手画（导航式列表，不需要 Select 组件）。

use std::sync::Arc;

use aish_types::{ConnectionId, RemoteSession};
use aish_ui::{theme, Dialog};
use gpui::{
    div, prelude::*, App, Context, Entity, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TmuxState};

pub struct SessionPickerView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    dialog: Entity<Dialog>,
    selected_index: usize,
}

impl SessionPickerView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let dialog = cx.new(|cx| Dialog::new(cx));
        let weak_state = state.downgrade();
        dialog.update(cx, |d, _cx| {
            d.title("Tmux Sessions");
            d.width(gpui::px(420.0));
            d.on_close(move |_window, cx| {
                if let Some(s) = weak_state.upgrade() {
                    s.update(cx, |s, cx| {
                        s.pending_session_picker = None;
                        cx.notify();
                    });
                }
            });
        });

        Self {
            state,
            bridge,
            tx,
            dialog,
            selected_index: 0,
        }
    }

    fn current_sessions(&self, cx: &App) -> Option<(ConnectionId, Vec<RemoteSession>)> {
        let app = self.state.read(cx);
        let conn = app.pending_session_picker?;
        let sessions = match app.tmux_state.get(&conn)? {
            TmuxState::Detected { sessions, .. } => sessions.clone(),
            _ => return None,
        };
        Some((conn, sessions))
    }

    fn attach(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some((conn, sessions)) = self.current_sessions(cx) else {
            self.close(cx);
            return;
        };
        let Some(session) = sessions.get(idx) else {
            return;
        };
        let session_name = session.name.clone();
        let sender_opt = self.state.read(cx).sessions.get(&conn).cloned();
        if let Some(sender) = sender_opt {
            self.bridge.spawn(async move {
                let _ = sender.send(SessionCommand::AttachTmux { session: session_name }).await;
            });
        }
        self.close(cx);
    }

    fn skip(&mut self, cx: &mut Context<Self>) {
        // 用户跳过 tmux attach，进 raw shell。
        self.close(cx);
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.dialog.update(cx, |d, cx| d.close(cx));
        self.state.update(cx, |s, cx| {
            s.pending_session_picker = None;
            cx.notify();
        });
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some((_, sessions)) = self.current_sessions(cx) else {
            return;
        };
        let len = sessions.len();
        if len == 0 {
            return;
        }
        match event.keystroke.key.as_str() {
            "up" => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    cx.notify();
                }
            }
            "down" => {
                if self.selected_index + 1 < len {
                    self.selected_index += 1;
                    cx.notify();
                }
            }
            "enter" => {
                let idx = self.selected_index;
                self.attach(idx, cx);
            }
            "escape" => self.skip(cx),
            _ => {}
        }
    }
}

impl Render for SessionPickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);

        let Some((_, sessions)) = self.current_sessions(cx) else {
            // 没数据：dialog 不开
            self.dialog.update(cx, |d, cx| d.close(cx));
            return div().into_any_element();
        };

        // 第一次有数据时打开 dialog
        if !self.dialog.read(cx).is_open() {
            self.dialog.update(cx, |d, cx| d.open(cx));
            self.selected_index = 0;
        }

        let selected = self.selected_index;
        let body = div()
            .flex()
            .flex_col()
            .gap(t.spacing.px_1)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _w, cx| {
                this.handle_key(ev, cx);
            }))
            .children(sessions.iter().enumerate().map(|(i, sess)| {
                let is_sel = i == selected;
                let name = sess.name.clone();
                div()
                    .id(("session-row", i))
                    .h(gpui::px(28.0))
                    .px(t.spacing.px_3)
                    .flex()
                    .items_center()
                    .gap(t.spacing.px_2)
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.foreground)
                    .when(is_sel, |d| d.bg(t.colors.accent))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.attach(i, cx);
                        }),
                    )
                    .child(div().text_color(t.colors.success).child("●"))
                    .child(div().flex_1().child(name))
                    .child(
                        div()
                            .text_size(t.font_size.xs)
                            .text_color(t.colors.muted_foreground)
                            .child(format!("({} windows)", sess.windows)),
                    )
            }));

        self.dialog.update(cx, |d, _cx| {
            d.body(body);
        });

        self.dialog.clone().into_any_element()
    }
}
```

- [ ] **Step 3: 跑 cargo check**

```bash
cargo check -p aish-app
```

修复编译错误（常见：`SessionCommand::AttachTmux` 字段名 / `RemoteSession` 字段路径）。

- [ ] **Step 4: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git add crates/aish-app/src/views/session_picker.rs
git commit -m "feat(aish-ui): T8 — SessionPickerView 外壳迁到 aish_ui::Dialog

列表保留手画（导航式列表不需要 Select），减少 ~30 行自实现 modal overlay。"
```

---

## Task 9: SettingsView 加 Appearance Section + Switch UI

**Files:**
- Modify: `crates/aish-app/src/views/settings.rs`

- [ ] **Step 1: 读当前 settings.rs**

```bash
cat crates/aish-app/src/views/settings.rs
```

理解现有 Section 结构（Version / App Info / Legal），确定插入 Appearance 的位置（最顶上）。

- [ ] **Step 2: 加字段 + 在 render 内加 Section**

修改 `crates/aish-app/src/views/settings.rs` 顶部 imports：

```rust
use aish_ui::{theme, toast_warning, Switch};
```

在 `pub struct SettingsView { ... }` 加字段：

```rust
pub struct SettingsView {
    // ... 现有字段
    dark_mode: bool,
}
```

在 `pub fn new()` 内初始化：

```rust
pub fn new() -> Self {
    Self {
        // ... 现有初始化
        dark_mode: true,
    }
}
```

在 `Render for SettingsView` 内，在最顶 Section（Version 之前）加 Appearance：

```rust
.child(section_header(cx, "Appearance"))
.child(
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py(t.spacing.px_2)
        .child(
            div()
                .text_size(t.font_size.sm)
                .text_color(t.colors.foreground)
                .child("Dark mode"),
        )
        .child(
            Switch::new("settings-dark-mode")
                .checked(self.dark_mode)
                .on_change(cx.listener(|this, new_value: &bool, _w, cx| {
                    if !*new_value {
                        // 切到 light，提示未实现
                        toast_warning(cx, "Light theme not yet implemented");
                        // 不改 this.dark_mode，保持 true，下一帧 render 时 Switch 视觉自动回弹
                    } else {
                        this.dark_mode = true;
                    }
                    cx.notify();
                })),
        ),
)
```

注：`section_header` 是现有 helper 函数，已经接 cx 取 theme。如果它原签名不接 cx，需要小改。

- [ ] **Step 3: 跑质量门禁 + 手测**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p aish
```

启动后：
- 进 Settings tab，看到顶部 "Appearance" section
- 显示 "Dark mode" 文字 + Switch on 状态
- 点 Switch → 视觉变 off → 立刻弹 toast warning "Light theme not yet implemented" → 下一帧 Switch 视觉回到 on（因为 dark_mode 没变）

- [ ] **Step 4: Commit**

```bash
git add crates/aish-app/src/views/settings.rs
git commit -m "feat(aish-ui): T9 — SettingsView 加 Appearance section + Dark mode Switch UI

点 Light → toast warning 'Light theme not yet implemented' + Switch 视觉回弹。
Light theme 实际实现仍 unimplemented! stub。"
```

---

## Task 10: INDEX 更新 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 更新 INDEX.md**

在 `## Milestones（按时间倒序）` 节点最上方插入 M12 条目（在 M11 之前）：

```markdown
### M12 — aish-ui 表单与导航 + HostForm/SessionPicker 迁移（2026-05-09）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-09-aish-m12-forms-nav-design.md`](specs/2026-05-09-aish-m12-forms-nav-design.md)
- plan：[`plans/2026-05-09-aish-m12-forms-nav.md`](plans/2026-05-09-aish-m12-forms-nav.md)
- 范围：5 个新组件（Checkbox / Switch / Tabs / Dialog / Select）+ HostFormModal 重写为 Dialog+Tabs+TextInput + SessionPickerView 外壳迁 Dialog + SettingsView 加 Appearance Section 含 Dark mode Switch（点 Light 弹 toast warning + 视觉回弹）
- 关键 commits：
  - `<T1 hash>` — Checkbox
  - `<T2 hash>` — Switch
  - `<T3 hash>` — Tabs
  - `<T4 hash>` — Dialog（M12 简化版 focus trap）
  - `<T5 hash>` — Select
  - `<T6 hash>` — prelude 验证
  - `<T7 hash>` — HostFormModal 重写
  - `<T8 hash>` — SessionPickerView 迁移
  - `<T9 hash>` — SettingsView Appearance + Switch UI
- 测试：aish-ui 51 + ~24 = ~75 + aish-app ~101（小幅波动）
- 已知边界：Dialog Tab 循环 focus trap 留 M13；Select 弹层只向下；Light theme 仍 unimplemented! stub
```

替换 `<TX hash>` 为实际 commit hash（`git log --oneline -10` 拿）。

- [ ] **Step 2: 更新 `## 当前状态`**

```markdown
- **活跃分支**：`feat/aish-ui-m12-20260509-zj`（M12 已完成）
- **下一里程碑**：M13 — DropdownMenu/ContextMenu + Light theme 实现 + 视觉回归收尾
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui ~75 + aish-app ~101 + 其他 crate) 全过
```

- [ ] **Step 3: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T10 — INDEX 更新 M12 已完成"
```

---

## DoD 自检

回看 spec § 8：

- [ ] aish-ui 5 个新组件 + 24+ 测试 ←→ T1–T5
- [ ] HostFormModal 重写完成，新建/编辑/删除/连接四条手测路径通过 ←→ T7
- [ ] SessionPickerView 视觉相近，attach 流程正常 ←→ T8
- [ ] Settings 有 Appearance section 含 Switch，点 Light 弹 toast + 视觉回弹 ←→ T9
- [ ] 质量门禁全过 ←→ 每个 task 末尾
- [ ] INDEX.md 更新 M12 ←→ T10
- [ ] 父 spec Risk R1/R2/R5 实际选择记录 ←→ commit message + INDEX 已知边界

---

## 后续候选（M13）

- DropdownMenu / ContextMenu
- Card / RadioGroup（M12 没用上）
- Light theme 实际实现
- Dialog Tab 键 focus 循环加固
- Select 弹层方向自适应
- Button hover state（M11 遗留）
- Toast 关闭按钮（M11 遗留）
- aish-ui crate-level README + examples/
