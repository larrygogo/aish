//! 主区：渲染 selected host 的 pane log + 接键盘输入发到 PTY。

use std::sync::Arc;

use gpui::{
    div, prelude::*, rgb, AnyElement, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent,
    Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};

pub struct HostPaneView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)] // 保留 tx 备未来扩展用
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
}

impl HostPaneView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        Self {
            state,
            bridge,
            tx,
            focus_handle,
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let host = match self.state.read(cx).selected {
            Some(h) => h,
            None => return,
        };
        let sender = match self.state.read(cx).sessions.get(&host).cloned() {
            Some(s) => s,
            None => return,
        };

        let ctrl = event.keystroke.modifiers.control;
        let alt = event.keystroke.modifiers.alt;
        let key = event.keystroke.key.as_str();

        let bytes = encode_key(key, ctrl, alt);
        if bytes.is_empty() {
            return;
        }

        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }
}

impl Focusable for HostPaneView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HostPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.state.read(cx).selected;
        let body: AnyElement = match selected {
            None => div()
                .text_color(rgb(0x888888))
                .p_4()
                .child("请从左侧选择主机")
                .into_any_element(),
            Some(host) => {
                let lines = self.state.read(cx).logs_of(host).to_vec();
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
                    .text_color(rgb(0xeeeeee))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(text_lines)
                    .into_any_element()
            }
        };

        div()
            .track_focus(&self.focus_handle(cx))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .flex_1()
            .h_full()
            .bg(rgb(0x121212))
            .child(body)
    }
}
