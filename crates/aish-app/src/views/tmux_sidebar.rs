//! 中间栏：按 host 的 TmuxState 显示 6 种视图。

use std::sync::Arc;

use aish_tmux::SessionTree;
use aish_types::{HostId, RemoteSession, SessionId};
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

    fn dispatch_command(&self, host: HostId, cmd: SessionCommand, cx: &mut Context<Self>) {
        let app = self.state.read(cx);
        match app.sessions.get(&host).cloned() {
            Some(sender) => {
                tracing::info!(?host, ?cmd, "tmux_sidebar: dispatch SessionCommand");
                self.bridge.spawn(async move {
                    if let Err(e) = sender.send(cmd).await {
                        tracing::error!("tmux_sidebar: send to actor failed: {}", e);
                    }
                });
            }
            None => {
                tracing::warn!(?host, "tmux_sidebar: no session sender for host");
            }
        }
    }

    fn handle_refresh(&mut self, host: HostId, cx: &mut Context<Self>) {
        tracing::info!(?host, "tmux_sidebar: refresh clicked");
        self.dispatch_command(host, SessionCommand::QueryTmuxSessions, cx);
    }

    fn handle_attach(&mut self, host: HostId, session: SessionId, cx: &mut Context<Self>) {
        tracing::info!(?host, ?session, "tmux_sidebar: session row clicked");
        self.dispatch_command(host, SessionCommand::AttachTmux { session }, cx);
    }
}

impl Render for TmuxSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot: Option<(HostId, TmuxState)> = {
            let app = self.state.read(cx);
            app.selected.map(|h| {
                (
                    h,
                    app.tmux_state
                        .get(&h)
                        .cloned()
                        .unwrap_or(TmuxState::NotChecked),
                )
            })
        };

        let body = match &snapshot {
            None => empty_view(),
            Some((host, state)) => match state {
                TmuxState::NotChecked => spinner_view("查询 tmux 中…"),
                TmuxState::NoTmux => notmux_view(),
                TmuxState::QueryFailed { msg } => query_failed_view(msg),
                TmuxState::Detected { sessions } => session_list_view(*host, sessions, cx),
                TmuxState::Attaching { session } => attaching_view(session),
                TmuxState::Attached { session_tree } => session_tree_view(session_tree),
            },
        };

        let host_for_buttons = snapshot.as_ref().map(|(h, _)| *h);
        let mut container = div()
            .w(px(200.0))
            .h_full()
            .bg(rgb(0x202020))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col();

        if let Some(host) = host_for_buttons {
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
                                this.handle_refresh(host, cx);
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
        .child("未选择 host")
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
    host: HostId,
    sessions: &[RemoteSession],
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
            let label = format!("○ {}", s.name);
            let row = div()
                .px_3()
                .py_2()
                .text_color(rgb(0xcccccc))
                .text_size(px(13.0))
                .hover(|st| st.bg(rgb(0x2a2a2a)).cursor_pointer())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                        this.handle_attach(host, session_id.clone(), cx);
                    }),
                )
                .child(label);
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

fn attaching_view(session: &SessionId) -> gpui::AnyElement {
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
                .child(format!("连接 tmux session: {}", session.as_str())),
        )
        .child(
            div()
                .text_color(rgb(0xaaaaaa))
                .text_size(px(11.0))
                .child("⠋ -CC handshake…"),
        )
        .into_any_element()
}

fn session_tree_view(tree: &SessionTree) -> gpui::AnyElement {
    let mut col = div().flex().flex_col().px_2().py_2().gap_1();

    if tree.sessions.is_empty() {
        col = col.child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("(等待 tmux 协议数据…)"),
        );
    } else {
        for (sid, sess) in &tree.sessions {
            let is_active = tree.active_session.as_ref() == Some(sid);
            let s_marker = if is_active { "●" } else { "○" };
            col = col.child(
                div()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(13.0))
                    .child(format!("{} {} ({})", s_marker, sess.name, sid.as_str())),
            );
            for (wid, win) in &sess.windows {
                col = col.child(
                    div()
                        .pl_4()
                        .text_color(rgb(0xcccccc))
                        .text_size(px(12.0))
                        .child(format!("├─ {} ({})", win.name, wid)),
                );
                for pane_id in win.panes.keys() {
                    col = col.child(
                        div()
                            .pl_8()
                            .text_color(rgb(0xaaaaaa))
                            .text_size(px(11.0))
                            .child(format!("├─ pane {}", pane_id)),
                    );
                }
            }
        }
    }

    col.into_any_element()
}
