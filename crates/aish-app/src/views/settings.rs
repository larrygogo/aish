//! SettingsView：静态三段设置页（M4b）。

use gpui::{div, prelude::*, px, rgb, Context, Window};

use crate::theme;

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

fn section_header(title: &'static str) -> impl IntoElement {
    div()
        .pt_4()
        .pb_1()
        .text_size(px(11.0))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(title)
}

fn shortcut_row(key: &'static str, action: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .py(px(2.0))
        .child(
            div()
                .w(px(180.0))
                .text_size(px(14.0))
                .text_color(rgb(theme::TEXT_SECONDARY))
                .child(key),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(rgb(theme::TEXT_PRIMARY))
                .child(action),
        )
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let version = env!("CARGO_PKG_VERSION");

        div()
            .id("settings-scroll")
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .overflow_y_scroll()
            .px_8()
            .py_6()
            // 页面标题
            .child(
                div()
                    .text_size(px(20.0))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child("SETTINGS"),
            )
            .child(
                div()
                    .h(px(1.0))
                    .my_3()
                    .bg(rgb(theme::BORDER_SUBTLE)),
            )
            // APP INFO
            .child(section_header("APP INFO"))
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child(format!("aish  v{}", version)),
            )
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child("Built 2026-05-08"),
            )
            // KEYBOARD SHORTCUTS
            .child(section_header("KEYBOARD SHORTCUTS"))
            .child(shortcut_row("Ctrl+Shift+V", "粘贴"))
            .child(shortcut_row("Ctrl+W", "关闭 tab"))
            .child(shortcut_row("Ctrl+T", "新 tab"))
            .child(shortcut_row("Ctrl+1", "Home"))
            .child(shortcut_row("Ctrl+2", "Terminal"))
            .child(shortcut_row("Ctrl+3", "Inbox"))
            .child(shortcut_row("Ctrl+4", "Settings"))
            // ABOUT
            .child(section_header("ABOUT"))
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child("github.com/larrygogo/aish"),
            )
            .child(
                div()
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .child("MIT License"),
            )
    }
}
