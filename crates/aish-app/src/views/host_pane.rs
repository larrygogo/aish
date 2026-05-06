//! 主区：渲染当前 selected host 的 pane log。

use gpui::{div, prelude::*, rgb, AnyElement, Context, Entity, Render, Window};

use crate::state::AppState;

pub struct HostPaneView {
    state: Entity<AppState>,
}

impl HostPaneView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state }
    }
}

impl Render for HostPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        match state.selected {
            None => div()
                .flex_1()
                .h_full()
                .text_color(rgb(0x888888))
                .p_4()
                .child("请从左侧选择主机")
                .into_any_element(),
            Some(host) => {
                let lines = state.logs_of(host);
                let text_lines: Vec<AnyElement> = lines
                    .iter()
                    .map(|line| {
                        div()
                            .text_color(rgb(0xeeeeee))
                            .child(line.clone())
                            .into_any_element()
                    })
                    .collect();

                div()
                    .flex_1()
                    .h_full()
                    .bg(rgb(0x121212))
                    .text_color(rgb(0xeeeeee))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(text_lines)
                    .into_any_element()
            }
        }
    }
}
