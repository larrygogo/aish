//! SettingsView：M4b 静态三段 + M12 加 Appearance section（Switch 演示位）。

use aish_ui::theme::ColorTokens;
use aish_ui::{theme, toast_warning, Switch};
use gpui::{div, prelude::*, px, Context, Hsla, Window};

pub struct SettingsView {
    /// Dark mode toggle 状态。M12：实际不真切 theme（Light 未实现），
    /// 用户点 Light 时 toast warning + 视觉回弹（不更新 dark_mode）。
    dark_mode: bool,
}

impl SettingsView {
    pub fn new() -> Self {
        Self { dark_mode: true }
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
        let colors = theme(cx).colors;
        let dark = self.dark_mode;

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
            .child(div().h(px(1.0)).my_3().bg(colors.border))
            // APPEARANCE — M12 新增
            .child(section_header("APPEARANCE", colors.muted_foreground))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .py(px(4.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(colors.foreground)
                            .child("Dark mode"),
                    )
                    .child(
                        Switch::new("settings-dark-mode")
                            .checked(dark)
                            .on_change(cx.listener(
                                |this, new_value: &bool, _w, cx| {
                                    if !*new_value {
                                        // 切到 Light：未实现，弹 toast + Switch 视觉回弹
                                        toast_warning(
                                            cx,
                                            "Light theme not yet implemented",
                                        );
                                        // 不更新 this.dark_mode → 下一帧 render 时
                                        // Switch 用 dark=true 自动回弹到 on。
                                        cx.notify();
                                    } else {
                                        this.dark_mode = true;
                                        cx.notify();
                                    }
                                },
                            )),
                    ),
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
            .child(section_header(
                "KEYBOARD SHORTCUTS",
                colors.muted_foreground,
            ))
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
