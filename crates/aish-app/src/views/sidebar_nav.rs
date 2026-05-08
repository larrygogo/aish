//! SidebarNav：左侧 48px 纯文字 4-tab 导航（M4a 信息架构）。
//!
//! 4 个 tab：Home / Terminal / Inbox / Settings。
//! 选中态：左侧 2px ACCENT_BLUE 指示条 + 背景 SIDEBAR_NAV_BG_ACTIVE + 文字变白。
//! 标签暂用纯 ASCII 文字占位，未来换 SVG asset 时只需改本文件。

use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};
use crate::theme;

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

        let nav_item = |tab: SidebarTab, icon: &'static str, cx: &mut Context<SidebarNavView>| {
            let is_active = current == tab;
            let fg = if is_active {
                rgb(theme::SIDEBAR_NAV_FG_ACTIVE)
            } else {
                rgb(theme::SIDEBAR_NAV_FG_DEFAULT)
            };

            let mut item = div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .py(px(12.0))
                .cursor_pointer()
                .text_color(fg)
                .text_size(px(10.0));

            if is_active {
                item = item
                    .bg(rgb(theme::SIDEBAR_NAV_BG_ACTIVE))
                    .border_l_2()
                    .border_color(rgb(theme::SIDEBAR_ACTIVE_BAR));
            } else {
                item = item.border_l_2().border_color(rgb(theme::SIDEBAR_BG));
            }

            item = item
                .hover(|s| {
                    if current != tab {
                        s.text_color(rgb(theme::SIDEBAR_NAV_FG_HOVER))
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
            .w(px(theme::SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(theme::SIDEBAR_BORDER))
            .child(nav_item(SidebarTab::Home, "home", cx))
            .child(nav_item(SidebarTab::Terminal, "term", cx))
            .child(nav_item(SidebarTab::Inbox, "inbox", cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(nav_item(SidebarTab::Settings, "cfg", cx)),
            )
    }
}
