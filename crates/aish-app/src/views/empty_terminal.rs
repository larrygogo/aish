//! EmptyTerminalGuideView：sidebar=Terminal 且无任何会话时的引导页（M4a）。

use aish_ui::TypographyExt;
use gpui::{div, prelude::*, px, Context, Entity, MouseDownEvent, Window};

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

        let go_home_btn = aish_ui::Button::new("empty-terminal-go-home")
            .label("Go to Home")
            .primary()
            .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                this.state.update(cx, |s, cx| {
                    s.sidebar = SidebarTab::Home;
                    cx.notify();
                });
            }));

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
                // M26 T2: 与 Home/Settings page title 同 Title1
                div()
                    .typography(aish_ui::TypeRole::Title1, theme)
                    .child("No active sessions yet"),
            )
            .child(
                // M26 T4: subtitle 用 Body (13/400/fg)，secondary_fg override
                div()
                    .typography(aish_ui::TypeRole::Body, theme)
                    .text_color(colors.secondary_foreground)
                    .child("Pick a host from Home to get started."),
            )
            .child(go_home_btn)
    }
}
