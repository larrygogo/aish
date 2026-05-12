//! Tmux Session Picker 弹窗。
//!
//! 触发：actor 收到远端 list-sessions 结果（事件 TmuxSessionsListed）后，
//! 若 sessions 非空且对应的 ConnectionId 是当前 tab，app 设置
//! `pending_session_picker = Some(conn)`，本视图渲染。
//!
//! 行为：
//! - 列出所有 sessions，点击 → 发 AttachTmux + 关弹窗
//! - dialog 标题栏 X / Esc / 点击遮罩 → 仅关弹窗（留在 raw shell）
//!
//! M12 重写：外壳换 `aish_ui::Dialog`；session 行保持手画（导航式列表
//! ↑/↓+Enter 不需要 Select 的下拉语义）。

use std::sync::Arc;

use aish_types::ConnectionId;
use aish_ui::{theme, Dialog};
use gpui::{
    div, prelude::*, Context, Entity, IntoElement, MouseButton, MouseDownEvent, SharedString,
    Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TmuxState};

pub struct SessionPickerView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    dialog: Entity<Dialog>,
    /// 是否已为当前 pending_session_picker 打开过 dialog。状态 mirror
    /// 防止每帧重复 open（dialog 自身有幂等性，但避免无意义 notify）。
    is_open_for: Option<ConnectionId>,
}

impl SessionPickerView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |this, _state, cx| {
            this.sync_from_state(cx);
            cx.notify();
        })
        .detach();

        let dialog = cx.new(Dialog::new);
        let weak = cx.weak_entity();
        dialog.update(cx, move |d, _cx| {
            d.title("Tmux Sessions");
            d.width(gpui::px(480.0));
            d.on_close(move |_window, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.handle_skip(cx));
                }
            });
        });

        Self {
            state,
            bridge,
            tx,
            dialog,
            is_open_for: None,
        }
    }

    fn sync_from_state(&mut self, cx: &mut Context<Self>) {
        let current = self.state.read(cx).pending_session_picker;
        match (self.is_open_for, current) {
            (_, None) => {
                if self.is_open_for.is_some() {
                    self.is_open_for = None;
                    self.dialog.update(cx, |d, cx| d.close(cx));
                }
            }
            (prev, Some(next)) if prev != Some(next) => {
                self.is_open_for = Some(next);
                self.dialog.update(cx, |d, cx| d.open(cx));
            }
            _ => {}
        }
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
        let Some(conn) = app.pending_session_picker else {
            return self.dialog.clone().into_any_element();
        };
        let sessions = match app.tmux_state.get(&conn) {
            Some(TmuxState::Detected { sessions, .. }) => sessions.clone(),
            _ => Vec::new(),
        };

        let (colors, font_size, spacing, radius) = {
            let t = theme(cx);
            (t.colors, t.font_size, t.spacing, t.radius)
        };

        let rows: Vec<_> = sessions
            .iter()
            .map(|s| {
                let sid = s.id.clone();
                let name = s.name.clone();
                // M17 一致性：大容器 hover 用 secondary 灰阶（与 Card / TabItem
                // 同源），不再用 accent 染色（accent 暗绿 #2f6e3e fill 整行
                // 视觉过冲）。row 之间用 gap 替代 border-b，每行 rounded 让
                // hover 高亮成块状而不是横条。
                let hover_bg = colors.secondary_hover;
                let active_bg = colors.secondary_active;
                // .active() 要求 stateful div → 必须 .id()，否则 compile 失败
                div()
                    .id(SharedString::from(format!("session-row-{}", sid)))
                    .px(spacing.px_3)
                    .py(spacing.px_2)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(spacing.px_3)
                    .rounded(radius.md)
                    .cursor_pointer()
                    .hover(move |st| st.bg(hover_bg))
                    .active(move |st| st.bg(active_bg))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_pick(conn, sid.clone(), cx);
                        }),
                    )
                    // success 绿点缩小到 xs，避免与 row hover 颜色互相干扰
                    .child(
                        div()
                            .text_color(colors.success)
                            .text_size(font_size.xs)
                            .child("●"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(font_size.sm)
                            .text_color(colors.foreground)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(font_size.xs)
                            .text_color(colors.muted_foreground)
                            .child("Enter"),
                    )
            })
            .collect();

        let body: gpui::AnyElement = if sessions.is_empty() {
            div()
                .py(spacing.px_4)
                .text_size(font_size.sm)
                .text_color(colors.muted_foreground)
                .child("(无 session — 关闭弹窗回到 raw shell)")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(spacing.px_1)
                .children(rows)
                .into_any_element()
        };

        self.dialog.update(cx, |d, _cx| {
            d.body(body);
        });

        self.dialog.clone().into_any_element()
    }
}
