//! SettingsView：静态三段设置页（M4b）。

use gpui::{div, prelude::*, px, Context, Hsla, Window};

use aish_ui::theme::ColorTokens;

pub struct SettingsView;

impl SettingsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

fn section_header(title: &'static str, muted: Hsla) -> impl IntoElement {
    div()
        .pt_4()
        .pb_1()
        .text_size(px(11.0))
        .text_color(muted)
        .child(title)
}

fn shortcut_row(key: &'static str, action: &'static str, colors: ColorTokens) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .py(px(2.0))
        .child(
            div()
                .w(px(180.0))
                .text_size(px(14.0))
                .text_color(colors.secondary_foreground)
                .child(key),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(colors.foreground)
                .child(action),
        )
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = env!("CARGO_PKG_VERSION");
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;

        div()
            .id("settings-scroll")
            .size_full()
            .bg(colors.background)
            .overflow_y_scroll()
            .px_8()
            .py_6()
            // 页面标题
            .child(
                div()
                    .text_size(px(20.0))
                    .text_color(colors.foreground)
                    .child("SETTINGS"),
            )
            .child(
                div()
                    .h(px(1.0))
                    .my_3()
                    .bg(colors.border),
            )
            // APP INFO
            .child(section_header("APP INFO", colors.muted_foreground))
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(colors.foreground)
                    .child(format!("aish  v{}", version)),
            )
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(colors.secondary_foreground)
                    .child("Built 2026-05-08"),
            )
            // KEYBOARD SHORTCUTS
            .child(section_header("KEYBOARD SHORTCUTS", colors.muted_foreground))
            .child(shortcut_row("Ctrl+Shift+V", "粘贴", colors))
            .child(shortcut_row("Ctrl+W", "关闭 tab", colors))
            .child(shortcut_row("Ctrl+T", "新 tab", colors))
            .child(shortcut_row("Ctrl+1", "Home", colors))
            .child(shortcut_row("Ctrl+2", "Terminal", colors))
            .child(shortcut_row("Ctrl+3", "Inbox", colors))
            .child(shortcut_row("Ctrl+4", "Settings", colors))
            // ABOUT
            .child(section_header("ABOUT", colors.muted_foreground))
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(colors.foreground)
                    .child("github.com/larrygogo/aish"),
            )
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(colors.secondary_foreground)
                    .child("MIT License"),
            )
    }
}
