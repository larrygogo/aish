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
use crate::state::{
    humanize_last_connected, AppState, SessionCommand, SidebarTab, SshEvent, TmuxState,
};

pub struct SessionPickerView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    dialog: Entity<Dialog>,
    /// 是否已为当前 pending_session_picker 打开过 dialog。状态 mirror
    /// 防止每帧重复 open（dialog 自身有幂等性，但避免无意义 notify）。
    is_open_for: Option<ConnectionId>,
    /// 键盘 ↑/↓ 选中的行索引（0-based，对应 sessions vec）。打开 dialog
    /// 时重置为 0。Enter 触发 handle_pick(sessions[selected_idx])。
    /// session 数量为 0 时无效（render 直接显示"无 session"提示）。
    selected_idx: usize,
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
        let weak_key = cx.weak_entity();
        dialog.update(cx, move |d, _cx| {
            d.title("Tmux Sessions");
            d.width(gpui::px(480.0));
            d.on_close(move |_window, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.handle_skip(cx));
                }
            });
            // ↑/↓ 移动 selected_idx，Enter 触发 pick。
            d.on_key(move |ev, _w, cx| {
                if let Some(this) = weak_key.upgrade() {
                    this.update(cx, |this, cx| this.handle_dialog_key(ev, cx));
                }
            });
        });

        Self {
            state,
            bridge,
            tx,
            dialog,
            is_open_for: None,
            selected_idx: 0,
        }
    }

    /// 处理 Dialog 透传过来的 key event（Esc 已被 Dialog 自身吃掉，到这里
    /// 只有 ↑ / ↓ / Enter）。
    fn handle_dialog_key(&mut self, ev: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        let Some(conn) = self.is_open_for else {
            return;
        };
        let sessions = match self.state.read(cx).tmux_state.get(&conn) {
            Some(TmuxState::Detected { sessions, .. }) => sessions.clone(),
            _ => return,
        };
        if sessions.is_empty() {
            return;
        }
        match ev.keystroke.key.as_str() {
            "up" => {
                self.selected_idx = if self.selected_idx == 0 {
                    sessions.len() - 1
                } else {
                    self.selected_idx - 1
                };
                cx.notify();
            }
            "down" => {
                self.selected_idx = (self.selected_idx + 1) % sessions.len();
                cx.notify();
            }
            "enter" => {
                if let Some(s) = sessions.get(self.selected_idx) {
                    let sid = s.id.clone();
                    self.handle_pick(conn, sid, cx);
                }
            }
            _ => {}
        }
    }

    fn sync_from_state(&mut self, cx: &mut Context<Self>) {
        // 只在用户当前看 terminal sidebar + 选中的 tab 正是该 connection 时开。
        // 用户在 Home / Settings sidebar 不该被 picker 打扰；切换 tab 离开
        // 该 connection 时 picker 也该关。
        // pending_session_picker 保留在 state 不清，等 user 切回时再 sync 开。
        let (pending, should_show) = {
            let s = self.state.read(cx);
            let pending = s.pending_session_picker;
            let show = pending.is_some_and(|c| {
                s.sidebar == SidebarTab::Terminal && s.current_connection() == Some(c)
            });
            (pending, show)
        };

        if !should_show {
            if self.is_open_for.is_some() {
                self.is_open_for = None;
                self.dialog.update(cx, |d, cx| d.close(cx));
            }
            return;
        }
        // should_show=true 隐含 pending.is_some()
        let next = pending.unwrap();
        if self.is_open_for != Some(next) {
            self.is_open_for = Some(next);
            // 每次新打开重置 selected_idx 到 0（首项）。否则切换不同 conn 时
            // 残留旧 idx，可能 out of range 或选错 session。
            self.selected_idx = 0;
            self.dialog.update(cx, |d, cx| d.open(cx));
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

        let selected_idx = self.selected_idx.min(sessions.len().saturating_sub(1));
        let rows: Vec<_> = sessions
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                let sid = s.id.clone();
                let name = s.name.clone();
                let windows = s.windows;
                let activity = s.activity;
                let is_kb_selected = idx == selected_idx;
                // M17 一致性：大容器 hover 用 secondary 灰阶（与 Card / TabItem
                // 同源），不再用 accent 染色（accent 暗绿 #2f6e3e fill 整行
                // 视觉过冲）。row 之间用 gap 替代 border-b，每行 rounded 让
                // hover 高亮成块状而不是横条。
                let hover_bg = colors.secondary_hover;
                let active_bg = colors.secondary_active;
                // 键盘选中行用 secondary_hover bg + accent border-l 区分鼠标
                // hover（防止键盘 + 鼠标同时操作时混淆）。
                let kb_bg = if is_kb_selected {
                    colors.secondary_hover
                } else {
                    colors.popover
                };
                // .active() 要求 stateful div → 必须 .id()，否则 compile 失败
                div()
                    .id(SharedString::from(format!("session-row-{}", sid)))
                    .relative()
                    .px(spacing.px_3)
                    .py(spacing.px_2)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(spacing.px_3)
                    .rounded(radius.md)
                    .bg(kb_bg)
                    .cursor_pointer()
                    .hover(move |st| st.bg(hover_bg))
                    .active(move |st| st.bg(active_bg))
                    // 键盘选中行左侧 2px primary 竖条 indicator
                    .when(is_kb_selected, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(gpui::px(2.0))
                                .bg(colors.primary),
                        )
                    })
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
                    // windows 数（tmux 1.6+ 字段）。0 = 未知，不显示。
                    .when(windows > 0, |d| {
                        d.child(
                            div()
                                .text_size(font_size.xs)
                                .text_color(colors.muted_foreground)
                                .child(format!("{} win", windows)),
                        )
                    })
                    // 上次活跃时间（humanize 成 "5m ago" / "2h ago"）。activity == 0
                    // = 旧 tmux 不支持该字段；不显示。
                    .when(activity > 0, |d| {
                        let last = std::time::UNIX_EPOCH
                            + std::time::Duration::from_secs(activity as u64);
                        d.child(
                            div()
                                .text_size(font_size.xs)
                                .text_color(colors.muted_foreground)
                                .child(humanize_last_connected(last)),
                        )
                    })
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
