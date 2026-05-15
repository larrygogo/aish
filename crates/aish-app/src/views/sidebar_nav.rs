//! SidebarNav：左侧 64px iconfont 3-tab 导航（M4a 信息架构）。
//!
//! M13 重写为用 aish_ui::NavItem.vertical()，icon 通过 div+font_family 包装传入。
//! M34: NavItem 升 stateful Entity（hover transition + press feedback），
//! 持 3 个 `Entity<NavItem>` 字段，render 每帧 `.update(cx, |n, _|
//! n.icon(...).active(...))` 重设（icon AnyElement 不可 Clone）。
//! M35 T6: SIDEBAR_WIDTH 48 → 64，加 Caption label 让导航自明（icon-only
//! 不可识别问题）。Label 用英文与代码 / commit message 风格一致。

use aish_ui::NavItem;
use gpui::{div, prelude::*, px, Context, Entity, Window};

use crate::state::{AppState, SidebarTab};
use crate::terminal::font::FONT_NAME;

const SIDEBAR_WIDTH: f32 = 64.0;

pub struct SidebarNavView {
    state: Entity<AppState>,
    home_item: Entity<NavItem>,
    terminal_item: Entity<NavItem>,
    settings_item: Entity<NavItem>,
}

impl SidebarNavView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();

        // 3 个 NavItem entity 持 click handler，weak.upgrade 透传 handle_click
        let weak_home = cx.weak_entity();
        let home_item = cx.new(|cx| {
            let mut n = NavItem::new("sidebar-nav-home", cx);
            n.vertical().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_home.upgrade() {
                    this.update(cx, |this, cx| this.handle_click(SidebarTab::Home, cx));
                }
            });
            n
        });
        let weak_term = cx.weak_entity();
        let terminal_item = cx.new(|cx| {
            let mut n = NavItem::new("sidebar-nav-terminal", cx);
            n.vertical().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_term.upgrade() {
                    this.update(cx, |this, cx| this.handle_click(SidebarTab::Terminal, cx));
                }
            });
            n
        });
        let weak_settings = cx.weak_entity();
        let settings_item = cx.new(|cx| {
            let mut n = NavItem::new("sidebar-nav-settings", cx);
            n.vertical().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_settings.upgrade() {
                    this.update(cx, |this, cx| this.handle_click(SidebarTab::Settings, cx));
                }
            });
            n
        });

        Self {
            state,
            home_item,
            terminal_item,
            settings_item,
        }
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

        // 每帧重设 icon + active（icon 是 AnyElement 不可 Clone，必须每帧重 build）
        let home_icon = div()
            .font_family(FONT_NAME)
            .text_size(px(16.0))
            .child("\u{f015}");
        let term_icon = div()
            .font_family(FONT_NAME)
            .text_size(px(16.0))
            .child("\u{f120}");
        let settings_icon = div()
            .font_family(FONT_NAME)
            .text_size(px(16.0))
            .child("\u{f013}");

        // M35 T6: 加 .label() — icon + Caption 标签让导航自明
        self.home_item.update(cx, |n, _| {
            n.icon(home_icon)
                .label("Home")
                .active(current == SidebarTab::Home);
        });
        self.terminal_item.update(cx, |n, _| {
            n.icon(term_icon)
                .label("Terminal")
                .active(current == SidebarTab::Terminal);
        });
        self.settings_item.update(cx, |n, _| {
            n.icon(settings_icon)
                .label("Settings")
                .active(current == SidebarTab::Settings);
        });

        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .border_r_1()
            .border_color(colors.border)
            .child(self.home_item.clone())
            .child(self.terminal_item.clone())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(self.settings_item.clone()),
            )
    }
}
