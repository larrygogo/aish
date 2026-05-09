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
            .text_size(px(14.0))
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
                    .text_size(px(20.0))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child("No active sessions yet"),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child("Pick a host from Home to get started."),
            )
            .child(go_home_btn)
    }
}
