//! EmptyTerminalGuideView：sidebar=Terminal 且无任何会话时的引导页（M4a）。

use gpui::{div, prelude::*, Context, Entity, Window};
use issh_ui::Button;

use crate::state::{AppState, SidebarTab};

pub struct EmptyTerminalGuideView {
    state: Entity<AppState>,
    /// M31：go-home button 升 stateful entity，press feedback 80ms。
    go_home_btn: Entity<Button>,
}

impl EmptyTerminalGuideView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // 构造 go_home_btn entity — weak.upgrade callback 修改 state.sidebar
        let weak = cx.weak_entity();
        let go_home_btn = cx.new(|cx| {
            let mut b = Button::new("empty-terminal-go-home", cx);
            b.label("回到主页").primary().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| {
                        this.state.update(cx, |s, cx| {
                            s.sidebar = SidebarTab::Home;
                            cx.notify();
                        });
                    });
                }
            });
            b
        });
        Self { state, go_home_btn }
    }
}

impl Render for EmptyTerminalGuideView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = issh_ui::theme(cx).colors;

        // M28 T5: EmptyState 4-slot anatomy 替代 ">_" 自绘字符 + 文字。
        // icon Server / title / description / action 复用 EmptyState 统一风。
        div()
            .size_full()
            .bg(colors.background)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                issh_ui::EmptyState::new("empty-terminal")
                    .icon(issh_ui::IconName::Server)
                    .title("还没有活跃连接")
                    .description("从主页选择一个主机开始 SSH 会话")
                    .action(self.go_home_btn.clone()),
            )
    }
}
