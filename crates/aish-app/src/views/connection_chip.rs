//! 终端区上方的连接信息条（参考用户提供的移动端截图设计）。
//!
//! 当前 tab 是 `Connection(c)` 时显示，叠在 TerminalView 上方：
//!
//! ```text
//! ┌────────────────────────────────────────────┐
//! │ ● 腾讯云 #1   [SSH]            [⊖]  [×]   │
//! ├────────────────────────────────────────────┤
//! │           （TerminalView 内容）            │
//! └────────────────────────────────────────────┘
//! ```
//!
//! 按钮语义：
//! - `⊖` collapse：把当前 tab.content 切回 Default，但**保留** Connection
//!   仍然在跑（actor 不退出）。等价于"暂时回首页，但服务器还连着"。用户
//!   可以从默认页里某种"已活跃"标记重新进入 — 这条交互后续做（M3c+）；
//!   当前先实现"切回首页"，活跃 connection 通过 connections map 仍可见。
//! - `×` close：完整断连（发 Disconnect + remove_connection 走 tab_bar 同款
//!   流程）。
//!
//! 注意：本视图**只在** tab.content == Connection 时被 RootView 渲染（条件
//! 已在 app.rs 那边判断），所以 render 内部直接读 current_connection 即可。

use std::sync::Arc;

use aish_types::ConnectionId;
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TabContent};
use crate::theme;

pub struct ConnectionChipView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl ConnectionChipView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    /// collapse：当前 tab 切回 Default 页，actor 不退出（保留运行）。
    fn handle_collapse(&mut self, _conn: ConnectionId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.replace_current_tab(TabContent::Default, "新连接".into());
            cx.notify();
        });
    }

    /// close：发 Disconnect + remove_connection（remove_connection 内部会把
    /// 引用该 connection 的 tab 转回 Default + title 重置为"新连接"）。
    fn handle_close(&mut self, conn: ConnectionId, cx: &mut Context<Self>) {
        let sender = self.state.read(cx).sessions.get(&conn).cloned();
        if let Some(sender) = sender {
            self.bridge.spawn(async move {
                let _ = sender.send(SessionCommand::Disconnect).await;
            });
        }
        self.state.update(cx, |s, cx| {
            s.remove_connection(conn);
            cx.notify();
        });
    }
}

impl Render for ConnectionChipView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let conn = match app.current_connection() {
            Some(c) => c,
            None => return div().into_any_element(),
        };
        let label = app
            .connections
            .get(&conn)
            .map(|c| c.label.clone())
            .unwrap_or_else(|| "—".into());
        let is_alive = app.is_session_active(conn);
        let dot_color = if is_alive {
            rgb(theme::ACCENT_GREEN)
        } else {
            rgb(theme::TEXT_MUTED)
        };

        // SSH chip — 跟默认页同款胶囊
        let ssh_chip = div()
            .px_2p5()
            .py_0p5()
            .text_size(theme::text_xs())
            .text_color(rgb(theme::ACCENT_BLUE))
            .bg(rgb(theme::CHIP_BLUE_BG))
            .rounded_full()
            .child("SSH");

        // collapse 按钮（▾ 比 ⊖ 语义清楚 — "向下收回到默认页"）
        let collapse_btn = div()
            .px_2()
            .py_1()
            .rounded_md()
            .text_color(rgb(theme::TEXT_SECONDARY))
            .text_size(theme::text_lg())
            .hover(|s| {
                s.text_color(rgb(theme::TEXT_PRIMARY))
                    .bg(rgb(theme::BG_SELECTED))
                    .cursor_pointer()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_collapse(conn, cx);
                }),
            )
            .child("▾");

        // close 按钮
        let close_btn = div()
            .px_2()
            .py_1()
            .rounded_md()
            .text_color(rgb(theme::TEXT_SECONDARY))
            .text_size(theme::text_lg())
            .hover(|s| {
                s.text_color(rgb(theme::ACCENT_RED))
                    .bg(rgb(theme::BG_SELECTED))
                    .cursor_pointer()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_close(conn, cx);
                }),
            )
            .child("×");

        div()
            .w_full()
            .h(px(36.0))
            .bg(rgb(theme::BG_ELEVATED))
            .border_b_1()
            .border_color(rgb(theme::BORDER_SUBTLE))
            .flex()
            .flex_row()
            .items_center()
            .px_4()
            .gap_2()
            .child(
                div()
                    .text_color(dot_color)
                    .text_size(theme::text_xs())
                    .child("●"),
            )
            .child(
                div()
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .text_size(theme::text_lg())
                    .child(label),
            )
            .child(ssh_chip)
            .child(div().flex_1())
            .child(collapse_btn)
            .child(close_btn)
            .into_any_element()
    }
}
