//! EmptyTerminalGuideView：sidebar=Terminal 且无任何会话时的引导页（M4a）。

use gpui::{div, prelude::*, px, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};

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
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        let go_home_btn = div()
            .px_6()
            .py_2()
            .text_size(px(14.0))
            .text_color(colors.primary_foreground)
            .bg(colors.primary)
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(colors.accent))
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
            .bg(colors.background)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_size(px(40.0))
                    .text_color(colors.muted_foreground)
                    .child(">_"),
            )
            .child(
                div()
                    .text_size(font_size.xl)
                    .text_color(colors.foreground)
                    .child("No active sessions yet"),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(colors.secondary_foreground)
                    .child("Pick a host from Home to get started."),
            )
            .child(go_home_btn)
    }
}
