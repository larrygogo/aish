//! SidebarNav：左侧 48px iconfont 4-tab 导航（M4a 信息架构）。
//!
//! 4 个 tab：Home / Terminal / Inbox / Settings。
//! 选中态：左侧 2px primary 指示条 + 背景 card + icon 变 foreground。
//! icon 使用 Nerd Font（JetBrains Mono Nerd Font 已 bundle）Font Awesome 字形：
//!   Home \u{f015}  Terminal \u{f120}  Inbox \u{f01c}  Settings \u{f013}

use gpui::{div, prelude::*, px, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};
use crate::terminal::font::FONT_NAME;

const SIDEBAR_WIDTH: f32 = 48.0;

pub struct SidebarNavView {
    state: Entity<AppState>,
}

impl SidebarNavView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }

    fn handle_click(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.sidebar = tab;
            cx.notify();
        });
    }
}

impl Render for SidebarNavView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.state.read(cx).sidebar;
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;

        let nav_item = |tab: SidebarTab, icon: &'static str, cx: &mut Context<SidebarNavView>| {
            let is_active = current == tab;
            let fg = if is_active {
                colors.foreground
            } else {
                colors.muted_foreground
            };

            let mut item = div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .py(px(14.0))
                .cursor_pointer()
                .font_family(FONT_NAME)
                .text_color(fg)
                .text_size(px(16.0));

            if is_active {
                item = item
                    .bg(colors.card)
                    .border_l_2()
                    .border_color(colors.primary);
            } else {
                item = item.border_l_2().border_color(colors.background);
            }

            item = item
                .hover(|s| {
                    if current != tab {
                        s.text_color(colors.secondary_foreground)
                    } else {
                        s
                    }
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                        this.handle_click(tab, cx);
                    }),
                )
                .child(icon);

            item
        };

        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .border_r_1()
            .border_color(colors.border)
            .child(nav_item(SidebarTab::Home, "\u{f015}", cx))
            .child(nav_item(SidebarTab::Terminal, "\u{f120}", cx))
            .child(nav_item(SidebarTab::Inbox, "\u{f01c}", cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(nav_item(SidebarTab::Settings, "\u{f013}", cx)),
            )
    }
}
