//! 中间栏：按 host 的 TmuxState 显示 4 种视图。
//!
//! M3-archived（2026-05-07）：之前 -CC 模式有 Attaching / Attached 两个额外视图
//! （含 SessionTree 树形展开），已随 raw attach 改造一并删除。点击 session 行
//! 后由 actor 在 raw shell 里发送 `tmux attach -t '<sess>'\r`，sidebar 仅高亮
//! 当前 attached 的 session，不再展开 windows/panes。

use std::sync::Arc;

use aish_types::{ConnectionId, RemoteSession, SessionId};
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TmuxState};

pub struct TmuxSidebarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl TmuxSidebarView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn dispatch_command(&self, conn: ConnectionId, cmd: SessionCommand, cx: &mut Context<Self>) {
        let app = self.state.read(cx);
        match app.sessions.get(&conn).cloned() {
            Some(sender) => {
                tracing::info!(?conn, ?cmd, "tmux_sidebar: dispatch SessionCommand");
                self.bridge.spawn(async move {
                    if let Err(e) = sender.send(cmd).await {
                        tracing::error!("tmux_sidebar: send to actor failed: {}", e);
                    }
                });
            }
            None => {
                tracing::warn!(?conn, "tmux_sidebar: no session sender for connection");
            }
        }
    }

    fn handle_refresh(&mut self, conn: ConnectionId, cx: &mut Context<Self>) {
        tracing::info!(?conn, "tmux_sidebar: refresh clicked");
        self.dispatch_command(conn, SessionCommand::QueryTmuxSessions, cx);
    }

    fn handle_attach(&mut self, conn: ConnectionId, session: SessionId, cx: &mut Context<Self>) {
        tracing::info!(?conn, ?session, "tmux_sidebar: session row clicked");
        self.dispatch_command(conn, SessionCommand::AttachTmux { session }, cx);
    }
}

impl Render for TmuxSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot: Option<(ConnectionId, TmuxState)> = {
            let app = self.state.read(cx);
            app.current_connection().map(|c| {
                (
                    c,
                    app.tmux_state
                        .get(&c)
                        .cloned()
                        .unwrap_or(TmuxState::NotChecked),
                )
            })
        };

        let body = match &snapshot {
            None => empty_view(),
            Some((conn, state)) => match state {
                TmuxState::NotChecked => spinner_view("查询 tmux 中…"),
                TmuxState::NoTmux => notmux_view(),
                TmuxState::QueryFailed { msg } => query_failed_view(msg),
                TmuxState::Detected { sessions, attached } => {
                    session_list_view(*conn, sessions, attached.as_ref(), cx)
                }
            },
        };

        let conn_for_buttons = snapshot.as_ref().map(|(c, _)| *c);
        let mut container = div()
            .w(px(200.0))
            .h_full()
            .bg(rgb(0x202020))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col();

        if let Some(conn) = conn_for_buttons {
            let header = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(rgb(0x333333))
                .child(
                    div()
                        .text_color(rgb(0xeeeeee))
                        .text_size(px(13.0))
                        .child("tmux"),
                )
                .child(
                    div()
                        .px_2()
                        .text_color(rgb(0xaaaaaa))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                                this.handle_refresh(conn, cx);
                            }),
                        )
                        .child("↻"),
                );
            container = container.child(header);
        }

        container.child(body)
    }
}

fn empty_view() -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .text_color(rgb(0x888888))
        .text_size(px(12.0))
        .child("未选择连接")
        .into_any_element()
}

fn spinner_view(msg: &str) -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(12.0))
                .child(msg.to_string()),
        )
        .child(
            div()
                .text_color(rgb(0xaaaaaa))
                .text_size(px(11.0))
                .child("⠋"),
        )
        .into_any_element()
}

fn notmux_view() -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xcccccc))
                .text_size(px(12.0))
                .child("未检测到 tmux"),
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("远端可能未安装 tmux 或不在 PATH"),
        )
        .into_any_element()
}

fn query_failed_view(msg: &str) -> gpui::AnyElement {
    div()
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xff6666))
                .text_size(px(12.0))
                .child("查询失败"),
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child(msg.to_string()),
        )
        .into_any_element()
}

fn session_list_view(
    conn: ConnectionId,
    sessions: &[RemoteSession],
    attached: Option<&SessionId>,
    cx: &mut Context<TmuxSidebarView>,
) -> gpui::AnyElement {
    let mut col = div().flex().flex_col();

    if sessions.is_empty() {
        col = col.child(
            div()
                .px_3()
                .py_3()
                .text_color(rgb(0x888888))
                .text_size(px(12.0))
                .child("(无现有 session)"),
        );
    } else {
        for s in sessions {
            let session_id = s.id.clone();
            let is_attached = attached == Some(&s.id);
            // ● = 当前 attached（绿色），○ = 未 attach（灰色）
            let (marker, marker_color) = if is_attached {
                ("●", rgb(0x4ec9b0))
            } else {
                ("○", rgb(0x888888))
            };
            let row = div()
                .px_3()
                .py_2()
                .flex()
                .flex_row()
                .gap_2()
                .text_color(rgb(0xcccccc))
                .text_size(px(13.0))
                .hover(|st| st.bg(rgb(0x2a2a2a)).cursor_pointer())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                        this.handle_attach(conn, session_id.clone(), cx);
                    }),
                )
                .child(div().text_color(marker_color).child(marker))
                .child(div().child(s.name.clone()));
            col = col.child(row);
        }
    }

    col = col.child(
        div()
            .px_3()
            .py_2()
            .text_color(rgb(0x666666))
            .text_size(px(12.0))
            .border_t_1()
            .border_color(rgb(0x333333))
            .child("+ new session (M3c)"),
    );

    col.into_any_element()
}
