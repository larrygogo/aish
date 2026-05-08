//! ComingSoonView：Inbox / Settings tab 占位页（M4a）。

use gpui::{div, prelude::*, px, rgb, Context, Window};

use crate::theme;

#[derive(Clone, Copy)]
pub enum ComingSoonKind {
    Inbox,
    Settings,
}

pub struct ComingSoonView {
    kind: ComingSoonKind,
}

impl ComingSoonView {
    pub fn new(kind: ComingSoonKind) -> Self {
        Self { kind }
    }
}

impl Render for ComingSoonView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (icon, title, description) = match self.kind {
            ComingSoonKind::Inbox => (
                "✉",
                "Inbox · Coming soon",
                "Agent events, tool completions, and approval requests will appear here.",
            ),
            ComingSoonKind::Settings => (
                "⚙",
                "Settings · Coming soon",
                "Appearance, input, notifications, and host defaults — coming in a future update.",
            ),
        };

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
                    .child(icon),
            )
            .child(
                div()
                    .text_size(px(20.0))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(360.0))
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child(description),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("See roadmap-moshi-desktop.md for the full plan."),
            )
    }
}
