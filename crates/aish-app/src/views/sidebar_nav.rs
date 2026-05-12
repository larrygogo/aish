//! SidebarNav：左侧 48px iconfont 4-tab 导航（M4a 信息架构）。
//!
//! M13 重写为用 aish_ui::NavItem.vertical()，icon 通过 div+font_family 包装传入。

use gpui::{div, prelude::*, px, Context, Entity, MouseDownEvent, Window};

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
        let colors = aish_ui::theme(cx).colors;

        let make_item = |tab: SidebarTab,
                         icon_char: &'static str,
                         tag: &'static str,
                         cx: &mut Context<SidebarNavView>| {
            let icon = div()
                .font_family(FONT_NAME)
                .text_size(px(16.0))
                .child(icon_char);
            aish_ui::NavItem::new(gpui::SharedString::from(format!("sidebar-nav-{tag}")))
                .vertical()
                .icon(icon)
                .active(current == tab)
                .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_click(tab, cx);
                }))
        };

        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .border_r_1()
            .border_color(colors.border)
            .child(make_item(SidebarTab::Home, "\u{f015}", "home", cx))
            .child(make_item(SidebarTab::Terminal, "\u{f120}", "terminal", cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(make_item(SidebarTab::Settings, "\u{f013}", "settings", cx)),
            )
    }
}
