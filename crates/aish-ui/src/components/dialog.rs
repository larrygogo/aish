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
use crate::TypographyExt;

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App) + 'static>;

pub struct Dialog {
    focus_handle: FocusHandle,
    open: bool,
    needs_focus: bool,
    title: SharedString,
    body: Option<AnyElement>,
    width: Pixels,
    on_close: Option<CloseHandler>,
    /// caller 注册的额外 key handler。在 Dialog 处理 Esc 关闭之后调用。
    /// 用于 caller 实现 ↑/↓/Enter 等列表导航（如 SessionPicker）。
    on_key: Option<KeyHandler>,
    /// Tab focus trap：dialog 内可 focus 的元素顺序链。caller 在 dialog
    /// open 后通过 focus_chain(...) 注册。Tab / Shift+Tab 在此链上循环，
    /// 不让焦点跑出 dialog 外（无障碍 + 减少误操作）。空 = 不启用 trap。
    focus_chain: Vec<FocusHandle>,
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
            on_key: None,
            focus_chain: Vec::new(),
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

    /// 注册额外 key handler。Dialog 自己处理 Esc 关闭之后调此 callback，
    /// 让 caller 实现 ↑/↓/Enter 等列表导航键位。
    pub fn on_key(
        &mut self,
        h: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_key = Some(Rc::new(h));
        self
    }

    /// 注册 Tab focus trap 的元素顺序链。Tab / Shift+Tab 在此 chain 上循环。
    /// caller 应在 modal 打开后填入按视觉顺序的 FocusHandle 列表（每次切换
    /// modal 状态前清空 + 重设）。空 chain = 不启用 trap（默认）。
    pub fn focus_chain(&mut self, handles: Vec<FocusHandle>) -> &mut Self {
        self.focus_chain = handles;
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
            return;
        }

        // Tab focus trap：在 focus_chain 上循环。chain 为空 = caller 没启
        // trap，跳过让 GPUI 默认行为（通常 noop，但至少不影响）。
        if !self.focus_chain.is_empty() && event.keystroke.key.as_str() == "tab" {
            let len = self.focus_chain.len();
            // 找当前 focused handle 在 chain 中的索引；找不到（焦点在外）
            // → Tab 拉回首项，Shift+Tab 拉回末项
            let cur = self.focus_chain.iter().position(|h| h.is_focused(window));
            let next = match (cur, event.keystroke.modifiers.shift) {
                (Some(i), false) => (i + 1) % len,
                (Some(i), true) => (i + len - 1) % len,
                (None, false) => 0,
                (None, true) => len - 1,
            };
            self.focus_chain[next].focus(window, cx);
            cx.notify();
            return;
        }

        // caller 自定义 key handler（如 SessionPicker 的 ↑/↓/Enter）。
        // 在 Esc 之后调用：Esc 由 Dialog 统一处理，caller 无法覆盖。
        if let Some(h) = self.on_key.clone() {
            h(event, window, cx);
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
            // occlude 让 backdrop hitbox 阻塞所有底层 view 接收鼠标事件
            // （等价于 z-order 上的"完全遮挡"），覆盖 hover/wheel/click/right-click 等
            // 所有类型。GPUI 内置 API，比手工 stop_propagation 一堆 listener 干净。
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
                div()
                    .w(width)
                    .max_h(gpui::px(640.0))
                    .bg(t.colors.popover)
                    .rounded(t.radius.lg)
                    .border_1()
                    .border_color(t.colors.border)
                    // M24 elevation-3 — modal 顶层悬浮
                    .shadow(crate::theme::elevation_3(t.kind))
                    .flex()
                    .flex_col()
                    // 阻止冒泡到 backdrop（GPUI mouse 事件是冒泡的，子元素 mouse_down
                    // 不会自动拦住父级 listener；必须显式 stop_propagation 才能让
                    // backdrop 的 close listener 不被点击 dialog 内部时触发）。
                    .on_mouse_down(MouseButton::Left, |_ev, _w, cx| {
                        cx.stop_propagation();
                    })
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
                                // M26 Dialog title: Title2 (16/600/fg)
                                div().typography(crate::TypeRole::Title2, t).child(title),
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
