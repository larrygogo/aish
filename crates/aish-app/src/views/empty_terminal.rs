//! EmptyTerminalGuideView：sidebar=Terminal 且无任何会话时的引导页（M4a）。

use gpui::{div, prelude::*, Context, Entity, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};

pub struct EmptyTerminalGuideView {
    state: Entity<AppState>,
}

impl EmptyTerminalGuideView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }
}

impl Render for EmptyTerminalGuideView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = aish_ui::theme(cx).colors;

        // M28 T5: EmptyState 4-slot anatomy 替代 ">_" 自绘字符 + 文字。
        // icon Server / title / description / action 复用 EmptyState 统一风。
        let go_home_btn = aish_ui::Button::new("empty-terminal-go-home")
            .label("回到 Home")
            .primary()
            .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                this.state.update(cx, |s, cx| {
                    s.sidebar = SidebarTab::Home;
                    cx.notify();
                });
            }));

        div()
            .size_full()
            .bg(colors.background)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                aish_ui::EmptyState::new("empty-terminal")
                    .icon(aish_ui::IconName::Server)
                    .title("还没有活跃连接")
                    .description("从 Home 选择一个 host 开始 SSH 会话")
                    .action(go_home_btn),
            )
    }
}
