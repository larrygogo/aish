//! 左栏：host 列表，点击切换 selected + 触发 SSH 连接。

use std::sync::Arc;

use aish_types::HostId;
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, SshEvent};

pub struct HostListView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HostListView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        let needs_connect = self.state.update(cx, |state, cx| {
            state.select_host(host);
            let needs = !state.is_session_active(host);
            cx.notify();
            needs
        });

        if needs_connect {
            let config = match self
                .state
                .read(cx)
                .hosts
                .iter()
                .find(|h| h.id == host)
                .cloned()
            {
                Some(c) => c,
                None => {
                    tracing::error!(?host, "host config not found");
                    return;
                }
            };

            let sender = self.bridge.spawn_session(host, config, self.tx.clone());
            self.state.update(cx, |state, _cx| {
                state.register_session(host, sender);
            });
        }
    }
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;
        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                let is_active = state.is_session_active(id);
                let prefix = if is_active { "● " } else { "○ " };
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _window, cx| {
                            this.handle_click(id, cx);
                        }),
                    )
                    .child(format!("{}{}", prefix, label))
            })
            .collect();

        let empty_hint = if state.hosts.is_empty() {
            Some(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x888888))
                    .child("（无 host：设置 AISH_DEV_HOST/USER/KEY_PATH 环境变量）"),
            )
        } else {
            None
        };

        div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .children(host_rows)
            .children(empty_hint)
    }
}
