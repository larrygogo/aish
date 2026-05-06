//! 左栏：host 列表，"+" 按钮 + hover ✏️/🗑 icons + 点击触发 SSH 连接。

use std::sync::Arc;

use aish_types::HostId;
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, HostFormDraft, HostFormState, SshEvent};

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

    fn handle_add_click(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.modal = Some(HostFormState::Adding(HostFormDraft::default()));
            cx.notify();
        });
    }

    fn handle_edit_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(cfg) = state.hosts.iter().find(|h| h.id == host).cloned() {
                state.modal = Some(HostFormState::Editing {
                    id: host,
                    draft: HostFormDraft::from_config(&cfg),
                });
                cx.notify();
            }
        });
    }

    fn handle_delete_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(cfg) = state.hosts.iter().find(|h| h.id == host).cloned() {
                state.modal = Some(HostFormState::DeleteConfirm {
                    id: host,
                    label: cfg.label,
                });
                cx.notify();
            }
        });
    }
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;

        // 顶部 "+ 添加 host" 按钮
        let plus_button = div()
            .px_3()
            .py_2()
            .text_color(rgb(0xaaaaaa))
            .bg(rgb(0x1e1e1e))
            .border_b_1()
            .border_color(rgb(0x333333))
            .hover(|s| s.bg(rgb(0x2a2a2a)).cursor_pointer())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_add_click(cx)),
            )
            .child("+ 添加 host");

        // 每行：label + hover 时显示 ✏/🗑 按钮
        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                let is_active = state.is_session_active(id);
                let prefix = if is_active { "● " } else { "○ " };

                let edit_btn = div()
                    .px_1()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xffffff)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_edit_click(id, cx);
                        }),
                    )
                    .child("✏");

                let delete_btn = div()
                    .px_1()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xff6666)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_delete_click(id, cx);
                        }),
                    )
                    .child("🗑");

                // icons 容器：默认隐藏，group hover 时以 flex 显示
                let icons = div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .hidden()
                    .group_hover("host_row", |s| s.flex())
                    .child(edit_btn)
                    .child(delete_btn);

                div()
                    .group("host_row")
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_click(id, cx);
                        }),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().flex_1().child(format!("{}{}", prefix, label)))
                    .child(icons)
            })
            .collect();

        let empty_hint = if state.hosts.is_empty() {
            Some(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x888888))
                    .text_size(px(12.0))
                    .child("(无 host：点上方 + 添加)"),
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
            .child(plus_button)
            .children(host_rows)
            .children(empty_hint)
    }
}
