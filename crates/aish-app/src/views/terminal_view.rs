//! 主区终端视图。M2b1 Task 3 阶段渲染 placeholder；Task 4 实装真 grid 绘制。

use std::sync::Arc;

use gpui::{
    div, prelude::*, rgb, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};

pub struct TerminalView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
}

impl TerminalView {
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

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
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

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.state.read(cx).selected;
        let placeholder = match selected {
            None => "请从左侧选择主机",
            Some(_) => "(终端渲染将在 Task 4 实装)",
        };

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .flex_1()
            .h_full()
            .bg(rgb(0x1d1f21))
            .text_color(rgb(0xc5c8c6))
            .p_4()
            .child(placeholder)
    }
}
