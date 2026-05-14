//! SettingsView — Card 分组布局（2026-05-12 UI-polish 重写）。
//!
//! 原版（M4b 静态三段 + M12 加 Switch 演示位）问题：
//!   - 全部 section 平铺无分组，section header 11px 灰字识别度低
//!   - 行高 py(2~4px) 过密
//!   - SETTINGS 主标题 20px 后跟 1px border 显得突兀
//!
//! 重写后：每个 section 用 aish_ui::Card outlined 包裹（header = section
//! title），section 间 gap_4 间距；行内统一 px_4 / py_3 内边距；shortcut
//! 表格化 grid 风格（左列固定 200px）。
//!
//! 注：keyboard shortcuts 当前仍是**文档级**展示（无实际键盘绑定代码），
//! Ctrl+1/2/3 路由到 sidebar tab 等留 backlog。

use aish_ui::theme::{ColorTokens, FontSize, ThemeKind};
use aish_ui::{theme, Card, Switch, Theme};
use gpui::{div, prelude::*, px, AnyElement, Context, IntoElement, SharedString, Window};

pub struct SettingsView {}

impl SettingsView {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

/// section card 的 header：粗体 14px + px/py 一致内边距 + 底部 border。
fn section_header(title: &'static str, colors: ColorTokens, fs: FontSize) -> AnyElement {
    div()
        .px_4()
        .py_3()
        .text_size(fs.sm)
        .text_color(colors.foreground)
        .border_b_1()
        .border_color(colors.border)
        .child(title)
        .into_any_element()
}

/// 两列行：左 200px 固定，右自然宽。用于 shortcut / info pair。
fn two_column_row(left: &str, right: &str, colors: ColorTokens, fs: FontSize) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .px_4()
        .py(px(8.0))
        .child(
            div()
                .w(px(200.0))
                .text_size(fs.sm)
                .text_color(colors.secondary_foreground)
                .child(SharedString::from(left.to_string())),
        )
        .child(
            div()
                .flex_1()
                .text_size(fs.sm)
                .text_color(colors.foreground)
                .child(SharedString::from(right.to_string())),
        )
        .into_any_element()
}

/// 标签 + 可选副标题 + 控件横向行（用于 Appearance 的 Dark mode 这种
/// label + Switch）。helper 非空时在 label 下方显示 muted_foreground xs 灰字，
/// 用于提示控件状态 / 限制（如"Light theme not implemented"）。
fn control_row(
    label: &'static str,
    helper: Option<&'static str>,
    control: AnyElement,
    colors: ColorTokens,
    fs: FontSize,
) -> AnyElement {
    let left = div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(fs.sm)
                .text_color(colors.foreground)
                .child(label),
        )
        .when_some(helper, |d, h| {
            d.child(
                div()
                    .text_size(fs.xs)
                    .text_color(colors.muted_foreground)
                    .child(h),
            )
        });
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_4()
        .py(px(10.0))
        .child(left)
        .child(control)
        .into_any_element()
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = env!("CARGO_PKG_VERSION");
        let t = theme(cx);
        let colors = t.colors;
        let fs = t.font_size;
        // 当前主题种类从 global Theme 读 —— 切换后 set_global + refresh_windows
        // 让所有 view 重新 render 拿新 theme。Switch 的 checked 状态直接反映
        // global 真值，不再用 self.dark_mode 镜像（之前 镜像 + disabled 是因为
        // Light 未实现的占位）。
        let dark = matches!(t.kind, ThemeKind::Dark);

        // ───── 页面标题 ─────
        let page_title = div()
            .pb_6()
            .text_size(px(24.0))
            .text_color(colors.foreground)
            .child("Settings");

        // ───── Appearance ─────
        // M18-light 启用：toggle 真切 Theme global + 持久化到 app_state.toml
        // 让重启保留选择。
        let dark_switch = Switch::new("settings-dark-mode")
            .checked(dark)
            .on_change(cx.listener(|_this, new_value: &bool, _w, cx| {
                let dark_now = *new_value;
                let new_theme = if dark_now {
                    Theme::dark()
                } else {
                    Theme::light()
                };
                cx.set_global(new_theme);
                // refresh_windows 让所有 view 重 render 拿新 theme global。
                // 不用 cx.notify(this) —— 该 view 自身只是众多受影响 view 之一。
                cx.refresh_windows();
                // 持久化：load 当前 state、改 theme 字段、save。
                let mut snapshot = crate::app_state_file::load_app_state();
                snapshot.theme = Some(if dark_now { "dark" } else { "light" }.to_string());
                crate::app_state_file::save_app_state(&snapshot);
            }))
            .into_any_element();

        let appearance_card = Card::new("settings-appearance")
            .outlined()
            .header(section_header("Appearance", colors, fs))
            .body(div().flex().flex_col().child(control_row(
                "Dark mode",
                None,
                dark_switch,
                colors,
                fs,
            )));

        // ───── Keyboard Shortcuts ─────
        // 当前仅文档级展示（实际键盘路由 backlog）。Inbox 删除后顺序：
        // Ctrl+1=Home / Ctrl+2=Terminal / Ctrl+3=Settings。
        let shortcuts_card = Card::new("settings-shortcuts")
            .outlined()
            .header(section_header("Keyboard Shortcuts", colors, fs))
            .body(
                div()
                    .flex()
                    .flex_col()
                    .child(two_column_row("Ctrl+Shift+V", "粘贴", colors, fs))
                    .child(two_column_row("Ctrl+W", "关闭 tab", colors, fs))
                    .child(two_column_row("Ctrl+T", "新 tab", colors, fs))
                    .child(two_column_row("Ctrl+1", "Home", colors, fs))
                    .child(two_column_row("Ctrl+2", "Terminal", colors, fs))
                    .child(two_column_row("Ctrl+3", "Settings", colors, fs)),
            );

        // ───── About（合并原 APP INFO + ABOUT）─────
        let about_card = Card::new("settings-about")
            .outlined()
            .header(section_header("About", colors, fs))
            .body(
                div()
                    .flex()
                    .flex_col()
                    .child(two_column_row(
                        "Version",
                        &format!("aish v{}", version),
                        colors,
                        fs,
                    ))
                    .child(two_column_row("Build date", "2026-05-08", colors, fs))
                    .child(two_column_row(
                        "Repository",
                        "github.com/larrygogo/aish",
                        colors,
                        fs,
                    ))
                    .child(two_column_row("License", "MIT", colors, fs)),
            );

        // ───── 整页布局 ─────
        div()
            .id("settings-scroll")
            .size_full()
            .bg(colors.background)
            .overflow_y_scroll()
            .px_8()
            .py_6()
            .child(page_title)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(appearance_card)
                    .child(shortcuts_card)
                    .child(about_card),
            )
    }
}
