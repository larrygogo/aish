//! Tmux Session Picker 弹窗。
//!
//! 触发：actor 收到远端 list-sessions 结果（事件 TmuxSessionsListed）后，
//! 若 sessions 非空且对应的 ConnectionId 是当前 tab，app 设置
//! `pending_session_picker = Some(conn)`，本视图渲染。
//!
//! 行为：
//! - 列出所有 sessions，点击 → 发 AttachTmux + 关弹窗
//! - "跳过"按钮 → 仅关弹窗（留在 raw shell）

use std::sync::Arc;

use aish_types::ConnectionId;
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TmuxState};

pub struct SessionPickerView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl SessionPickerView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_skip(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.pending_session_picker = None;
            cx.notify();
        });
    }

    fn handle_pick(
        &mut self,
        conn: ConnectionId,
        session: aish_types::SessionId,
        cx: &mut Context<Self>,
    ) {
        // 给 actor 发 AttachTmux
        let sender = self.state.read(cx).sessions.get(&conn).cloned();
        if let Some(sender) = sender {
            self.bridge.spawn(async move {
                let _ = sender.send(SessionCommand::AttachTmux { session }).await;
            });
        }
        self.state.update(cx, |s, cx| {
            s.pending_session_picker = None;
            cx.notify();
        });
    }
}

impl Render for SessionPickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let conn = match app.pending_session_picker {
            Some(c) => c,
            None => return div().into_any_element(),
        };
        let sessions = match app.tmux_state.get(&conn) {
            Some(TmuxState::Detected { sessions, .. }) => sessions.clone(),
            _ => Vec::new(),
        };

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_5()
            .py_4()
            .border_b_1()
            .border_color(rgb(0x2a2a2a))
            .child(
                div()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(16.0))
                    .child("Tmux Sessions"),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_color(rgb(0xaaaaaa))
                    .text_size(px(12.0))
                    .bg(rgb(0x2a2a2a))
                    .rounded_md()
                    .hover(|s| {
                        s.bg(rgb(0x3a3a3a))
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_skip(cx)),
                    )
                    .child("跳过 ▷|"),
            );

        let session_rows: Vec<_> = sessions
            .iter()
            .map(|s| {
                let sid = s.id.clone();
                let name = s.name.clone();
                div()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0x2a2a2a))
                    .hover(|st| st.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_pick(conn, sid.clone(), cx);
                        }),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(div().text_color(rgb(0x4ec9b0)).child("●"))
                    .child(div().flex_1().text_color(rgb(0xeeeeee)).child(name))
                    .child(
                        div()
                            .text_color(rgb(0x666666))
                            .text_size(px(11.0))
                            .child("Enter"),
                    )
            })
            .collect();

        let empty = if sessions.is_empty() {
            Some(
                div()
                    .px_5()
                    .py_8()
                    .text_color(rgb(0x666666))
                    .text_size(px(13.0))
                    .child("(无 session — 点跳过进 raw shell)"),
            )
        } else {
            None
        };

        // 半透明遮罩 + 居中卡片
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000aa))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                // 点遮罩 = 跳过
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_skip(cx)),
            )
            .child(
                div()
                    .w(px(480.0))
                    .max_h(px(560.0))
                    .bg(rgb(0x1a1a1a))
                    .border_1()
                    .border_color(rgb(0x333333))
                    .rounded_lg()
                    .flex()
                    .flex_col()
                    // 阻止点击内部区域穿透到遮罩 handle_skip。GPUI listener
                    // 默认不阻止冒泡，必须显式调 stop_propagation。
                    .on_mouse_down(MouseButton::Left, |_ev: &MouseDownEvent, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(header)
                    .child(div().flex_col().children(session_rows).children(empty)),
            )
            .into_any_element()
    }
}

fn rgba(value: u32) -> gpui::Rgba {
    let r = ((value >> 24) & 0xff) as f32 / 255.0;
    let g = ((value >> 16) & 0xff) as f32 / 255.0;
    let b = ((value >> 8) & 0xff) as f32 / 255.0;
    let a = (value & 0xff) as f32 / 255.0;
    gpui::Rgba { r, g, b, a }
}
