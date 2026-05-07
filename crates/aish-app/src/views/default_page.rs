//! 默认页：tab.content == Default 时显示。
//!
//! 主体是 host 卡片网格 + 顶部"+ 添加 host"。点击卡片：
//!   1. `state.open_connection(host_id)` 拿新 ConnectionId
//!   2. 当前 tab.content 替换为 Connection(conn_id)
//!   3. `bridge.spawn_session(conn_id, config)` 启动 actor
//!   4. session picker 弹窗的弹出由 app.rs 在 TmuxSessionsListed 事件触发

use std::sync::Arc;

use aish_types::HostId;
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, HostFormDraft, HostFormState, SshEvent, TabContent};

pub struct DefaultPageView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl DefaultPageView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    /// 点击 host 卡片：在当前 tab 启动新 connection。
    fn handle_card_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        let (conn_id, config, label) = self.state.update(cx, |state, cx| {
            let conn = state.open_connection(host_id);
            let cfg = state.hosts.iter().find(|h| h.id == host_id).cloned();
            let label = state
                .connections
                .get(&conn)
                .map(|c| c.label.clone())
                .unwrap_or_default();
            // 当前 tab → connection
            state.replace_current_tab(TabContent::Connection(conn), label.clone());
            cx.notify();
            (conn, cfg, label)
        });

        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!(?host_id, "host config not found");
                return;
            }
        };
        tracing::info!(?conn_id, %label, "default_page: spawn connection");

        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |state, _cx| {
            state.register_session(conn_id, sender);
        });
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

impl Render for DefaultPageView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);

        // 顶部"添加 host"按钮
        let add_btn = div()
            .px_4()
            .py_2()
            .text_color(rgb(0xeeeeee))
            .bg(rgb(0x2a2a2a))
            .rounded_md()
            .hover(|s| s.bg(rgb(0x3a3a3a)).cursor_pointer())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_add_click(cx)),
            )
            .child("+ 添加 host");

        // host 卡片
        let cards: Vec<_> = app
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let host_text = format!("{}@{}:{}", h.user, h.host, h.port);

                let edit_btn = div()
                    .px_2()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xffffff)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            let _ = ev;
                            this.handle_edit_click(id, cx);
                        }),
                    )
                    .child("✏");

                let delete_btn = div()
                    .px_2()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xff6666)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            let _ = ev;
                            this.handle_delete_click(id, cx);
                        }),
                    )
                    .child("🗑");

                let actions = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .opacity(0.0)
                    .group_hover("host_card", |s| s.opacity(1.0))
                    .child(edit_btn)
                    .child(delete_btn);

                // SSH chip
                let chip = div()
                    .px_2()
                    .py_0p5()
                    .text_size(px(11.0))
                    .text_color(rgb(0x4a9eff))
                    .bg(rgb(0x1f3a5c))
                    .rounded_md()
                    .child("SSH");

                div()
                    .group("host_card")
                    .px_4()
                    .py_3()
                    .bg(rgb(0x1e1e1e))
                    .border_1()
                    .border_color(rgb(0x2a2a2a))
                    .rounded_lg()
                    .hover(|s| {
                        s.bg(rgb(0x252525))
                            .border_color(rgb(0x3a3a3a))
                            .cursor_pointer()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_card_click(id, cx);
                        }),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(rgb(0xeeeeee))
                                            .text_size(px(14.0))
                                            .child(label),
                                    )
                                    .child(chip),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x888888))
                                    .text_size(px(11.0))
                                    .child(host_text),
                            ),
                    )
                    .child(actions)
            })
            .collect();

        let empty_hint = if app.hosts.is_empty() {
            Some(
                div()
                    .px_4()
                    .py_8()
                    .text_color(rgb(0x666666))
                    .text_size(px(13.0))
                    .child("还没有保存的连接 — 点上方 + 添加 host 开始"),
            )
        } else {
            None
        };

        div()
            .size_full()
            .bg(rgb(0x141414))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_6()
                    .py_4()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_color(rgb(0xeeeeee))
                            .text_size(px(18.0))
                            .child("已保存的连接"),
                    )
                    .child(add_btn),
            )
            .child(
                div()
                    .px_6()
                    .pb_6()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(cards)
                    .children(empty_hint),
            )
    }
}
