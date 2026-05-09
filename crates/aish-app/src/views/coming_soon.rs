//! ComingSoonView：Inbox / Settings tab 占位页（M4a）。

use gpui::{div, prelude::*, px, Context, Window};

use crate::terminal::font::FONT_NAME;

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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        let (icon, title, description) = match self.kind {
            ComingSoonKind::Inbox => (
                "\u{f01c}",
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
            .bg(colors.background)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .font_family(FONT_NAME)
                    .text_size(px(40.0))
                    .text_color(colors.muted_foreground)
                    .child(icon),
            )
            .child(
                div()
                    .text_size(font_size.xl)
                    .text_color(colors.foreground)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(360.0))
                    .text_size(px(14.0))
                    .text_color(colors.secondary_foreground)
                    .child(description),
            )
            .child(
                div()
                    .text_size(font_size.xs)
                    .text_color(colors.muted_foreground)
                    .child("See roadmap-moshi-desktop.md for the full plan."),
            )
    }
}
