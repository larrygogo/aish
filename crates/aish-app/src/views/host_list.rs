//! 左栏：拆成两段 —— **活跃会话**（运行中的 Connection，每条独立）+
//! **已保存的连接**（HostConfig 列表，点击 → 启动新连接）。
//!
//! M3b 起：一个 HostConfig 可同时派生 N 个 Connection；点击同一个 host 行
//! N 次就开 N 个独立连接（label 自动 `"<host.label> #N"`）。

use std::sync::Arc;

use aish_types::{ConnectionId, HostId};
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, HostFormDraft, HostFormState, SessionCommand, SshEvent};

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

    /// 点 "已保存的连接" 里的 host 行 → 启动新 Connection。
    fn handle_open_new(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        let (conn_id, config) = self.state.update(cx, |state, cx| {
            let conn = state.open_connection(host_id);
            let cfg = state.hosts.iter().find(|h| h.id == host_id).cloned();
            cx.notify();
            (conn, cfg)
        });

        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!(?host_id, "host config not found");
                return;
            }
        };

        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |state, _cx| {
            state.register_session(conn_id, sender);
        });
    }

    /// 点 "活跃会话" 里某条 → 切换 terminal 显示该 connection。
    fn handle_select_connection(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.select_connection(conn_id);
            cx.notify();
        });
    }

    /// 活跃会话项的 × 按钮：先发 Disconnect 让 actor 优雅退出，UI 立即移除。
    fn handle_close_connection(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        let sender = self.state.read(cx).sessions.get(&conn_id).cloned();
        if let Some(sender) = sender {
            self.bridge.spawn(async move {
                let _ = sender.send(SessionCommand::Disconnect).await;
            });
        }
        self.state.update(cx, |state, cx| {
            state.remove_connection(conn_id);
            cx.notify();
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

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected_conn = state.selected_connection;

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

        // ───────── 活跃会话区 ─────────
        // 按 opened_at 升序排，让 #1 总在 #2 上方。
        let mut active_conns: Vec<_> = state.connections.values().collect();
        active_conns.sort_by_key(|c| c.opened_at);

        let active_header = div()
            .px_3()
            .py_2()
            .text_color(rgb(0x888888))
            .text_size(px(11.0))
            .border_t_1()
            .border_color(rgb(0x333333))
            .child(format!("活跃会话 ({})", active_conns.len()));

        let active_rows: Vec<_> = active_conns
            .iter()
            .map(|c| {
                let conn_id = c.id;
                let label = c.label.clone();
                let is_alive = state.is_session_active(conn_id);
                let is_selected = selected_conn == Some(conn_id);
                let prefix = if is_alive { "● " } else { "○ " };

                let close_btn = div()
                    .px_1()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xff6666)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_close_connection(conn_id, cx);
                        }),
                    )
                    .child("×");

                let icons = div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .opacity(0.0)
                    .group_hover("conn_row", |s| s.opacity(1.0))
                    .child(close_btn);

                div()
                    .group("conn_row")
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_select_connection(conn_id, cx);
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

        let active_empty = if active_conns.is_empty() {
            Some(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x666666))
                    .text_size(px(11.0))
                    .child("(暂无活跃连接)"),
            )
        } else {
            None
        };

        // ───────── 已保存的连接 ─────────
        let saved_header = div()
            .px_3()
            .py_2()
            .text_color(rgb(0x888888))
            .text_size(px(11.0))
            .border_t_1()
            .border_color(rgb(0x333333))
            .child("已保存的连接");

        let saved_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();

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

                let icons = div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .opacity(0.0)
                    .group_hover("host_row", |s| s.opacity(1.0))
                    .child(edit_btn)
                    .child(delete_btn);

                div()
                    .group("host_row")
                    .px_3()
                    .py_2()
                    .text_color(rgb(0xcccccc))
                    .bg(rgb(0x1e1e1e))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_open_new(id, cx);
                        }),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().flex_1().child(label))
                    .child(icons)
            })
            .collect();

        let saved_empty = if state.hosts.is_empty() {
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

        let mut col = div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .child(plus_button)
            .child(active_header)
            .children(active_rows);
        if let Some(e) = active_empty {
            col = col.child(e);
        }
        col = col.child(saved_header).children(saved_rows);
        if let Some(e) = saved_empty {
            col = col.child(e);
        }
        col
    }
}
