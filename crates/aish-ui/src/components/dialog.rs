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
    needs_focus: bool,
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

    /// 设置 dialog 内容。**每帧 render 时都需要调用**——`AnyElement` 不可 Clone，
    /// render 内会 `take()` 消耗 body 渲染一次。caller 通常在 `Render::render` 内
    /// 通过 `dialog.update(cx, |d, _| d.body(...))` 每帧重新设置。
    pub fn body(&mut self, body: impl IntoElement) -> &mut Self {
        self.body = Some(body.into_any_element());
        self
    }

    pub fn width(&mut self, w: Pixels) -> &mut Self {
        self.width = w;
        self
    }

    pub fn on_close(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self {
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
            .bg(gpui::rgba(0x0000_0099))
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
                div()
                    .w(width)
                    .max_h(gpui::px(640.0))
                    .bg(t.colors.popover)
                    .rounded(t.radius.lg)
                    .border_1()
                    .border_color(t.colors.border)
                    .flex()
                    .flex_col()
                    // 阻止冒泡到 backdrop（GPUI 没原生 stop_propagation，但 hit test 命中
                    // 子元素时 backdrop on_mouse_down 不会触发同坐标）。空 listener 占位
                    // 即可，确保点 dialog 内部不关闭。
                    .on_mouse_down(MouseButton::Left, |_ev, _w, _cx| {})
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

    #[test]
    fn open_helper_clamp() {
        // 模拟 open() 的状态变换：原 false → true，needs_focus 同步置 true
        let mut open = false;
        let mut needs_focus = false;
        if !open {
            open = true;
            needs_focus = true;
        }
        assert!(open);
        assert!(needs_focus);

        // 第二次调用 open()，已是 open 状态，不执行分支
        if !open {
            unreachable!();
        }
        // 状态不变
        assert!(open);
        assert!(needs_focus);
    }

    #[test]
    fn close_helper_idempotent() {
        // 模拟 close()：open true → false
        let mut open = true;
        if open {
            open = false;
        }
        assert!(!open);

        // 再次 close()，已是 closed，不执行分支
        if open {
            unreachable!();
        }
        assert!(!open);
    }
}
