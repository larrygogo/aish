//! 主区：渲染 selected host 的 pane log。

use std::sync::Arc;

use gpui::{div, prelude::*, rgb, AnyElement, Context, Entity, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, SshEvent};

pub struct HostPaneView {
    state: Entity<AppState>,
    #[allow(dead_code)] // Task 7 加键盘输入时用
    bridge: Arc<Bridge>,
    #[allow(dead_code)] // Task 7 加键盘输入时用
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HostPaneView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
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
