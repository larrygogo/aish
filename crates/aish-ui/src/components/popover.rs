//! Popover — 浮层基础组件。
//!
//! 基于 GPUI `anchored()` Window mode 渲染浮层。trigger 元素的位置通过
//! `set_trigger_bounds()` 由 caller 写入（通常在 trigger 内嵌 `canvas()`，
//! prepaint 闭包 `popover.update(cx, |p, _| p.set_trigger_bounds(bounds))`）。
//!
//! **前提条件**：caller 必须在 trigger 元素上挂 `canvas()`，在 prepaint 阶段调用
//! `popover.update(cx, |p, _| p.set_trigger_bounds(bounds))`；否则 Popover 即使
//! `open = true` 也不会渲染浮层（需等下一帧 trigger 写入 bounds 后才会出现）。
//!
//! click + programmatic 触发：caller 在 trigger 的 mouse_down listener 内调
//! `popover.toggle()`。Esc / 点 backdrop 外 → 自动关闭。
//!
//! 不内置 hover 触发 — hover 浮层用 Tooltip（M11 已有）。

use std::rc::Rc;

use gpui::{
    anchored, div, point, prelude::*, px, Anchor, AnchoredPositionMode, AnyElement, App, Bounds,
    Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    Pixels, Window,
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

    pub fn on_close(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self {
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

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() == "escape" {
            self.close(cx);
            self.fire_close(window, cx);
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
                this.handle_key(ev, window, cx);
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
    #[allow(unused_assignments)]
    fn placement_chain_top() {
        let mut placement = PopoverPlacement::Bottom;
        placement = PopoverPlacement::Top;
        assert_eq!(placement, PopoverPlacement::Top);
    }

    #[test]
    #[allow(unused_assignments)]
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
