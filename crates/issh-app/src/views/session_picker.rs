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
//! M12 重写：外壳换 `issh_ui::Dialog`；session 行保持手画（导航式列表
//! ↑/↓+Enter 不需要 Select 的下拉语义）。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{div, prelude::*, px, Context, Entity, IntoElement, Window};
use issh_types::{ConnectionId, SessionId};
use issh_ui::{theme, Dialog, ListRow, TypographyExt};

use crate::app::retain_alive_entities;
use crate::bridge::Bridge;
use crate::state::{humanize_last_connected, AppState, SessionCommand, SidebarTab, TmuxState};

pub struct SessionPickerView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    dialog: Entity<Dialog>,
    /// 是否已为当前 pending_session_picker 打开过 dialog。状态 mirror
    /// 防止每帧重复 open（dialog 自身有幂等性，但避免无意义 notify）。
    is_open_for: Option<ConnectionId>,
    /// 键盘 ↑/↓ 选中的行索引（0-based，对应 sessions vec）。打开 dialog
    /// 时重置为 0。Enter 触发 handle_pick(sessions[selected_idx])。
    /// session 数量为 0 时无效（render 直接显示"无 session"提示）。
    selected_idx: usize,
    /// 每个 session 一个 ListRow Entity（hover transition + press feedback）。
    /// render 前 retain 同步当前 sessions 集合，避免 entity 泄漏。
    session_rows: HashMap<SessionId, Entity<ListRow>>,
}

impl SessionPickerView {
    pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, _state, cx| {
            this.sync_from_state(cx);
            cx.notify();
        })
        .detach();

        let dialog = cx.new(Dialog::new);
        let weak = cx.weak_entity();
        let weak_key = cx.weak_entity();
        dialog.update(cx, move |d, _cx| {
            d.title("Tmux 会话");
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
            dialog,
            is_open_for: None,
            selected_idx: 0,
            session_rows: HashMap::new(),
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
        session: issh_types::SessionId,
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
        // ── Phase 0：early return + ensure row entities（独立 cx.new 调用，不嵌套
        //   read 借用）+ retain 同步 ──
        let (conn, sessions) = {
            let app = self.state.read(cx);
            let Some(conn) = app.pending_session_picker else {
                return self.dialog.clone().into_any_element();
            };
            let sessions = match app.tmux_state.get(&conn) {
                Some(TmuxState::Detected { sessions, .. }) => sessions.clone(),
                _ => Vec::new(),
            };
            (conn, sessions)
        };

        let alive_sids: std::collections::HashSet<SessionId> =
            sessions.iter().map(|s| s.id.clone()).collect();
        retain_alive_entities(&mut self.session_rows, |k| alive_sids.contains(k));

        for s in &sessions {
            if !self.session_rows.contains_key(&s.id) {
                let row_id: gpui::ElementId =
                    gpui::SharedString::from(format!("session-row-{}", s.id)).into();
                let entity = cx.new(|c| {
                    let mut r = ListRow::new(row_id, c);
                    r.padding(px(12.0), px(8.0));
                    r
                });
                self.session_rows.insert(s.id.clone(), entity);
            }
        }

        let selected_idx = self.selected_idx.min(sessions.len().saturating_sub(1));

        // ── Phase A：block scope — 读 app + theme，build 每行 body AnyElement ──
        let (body_phase1, spacing_px_1): (Vec<(SessionId, gpui::AnyElement, bool)>, gpui::Pixels) = {
            let _app = self.state.read(cx);
            let t = theme(cx);
            let colors = t.colors;
            let spacing = t.spacing;

            let rows: Vec<(SessionId, gpui::AnyElement, bool)> = sessions
                .iter()
                .enumerate()
                .map(|(idx, s)| {
                    let is_kb_selected = idx == selected_idx;
                    let inner = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(spacing.px_3)
                        .child(
                            div()
                                .typography(issh_ui::TypeRole::Micro, t)
                                .text_color(colors.success)
                                .child("●"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .typography(issh_ui::TypeRole::Body, t)
                                .child(s.name.clone()),
                        )
                        .when(s.windows > 0, |d| {
                            d.child(
                                div()
                                    .typography(issh_ui::TypeRole::Caption, t)
                                    .child(format!("{} 窗口", s.windows)),
                            )
                        })
                        .when(s.activity > 0, |d| {
                            let last = std::time::UNIX_EPOCH
                                + std::time::Duration::from_secs(s.activity as u64);
                            d.child(
                                div()
                                    .typography(issh_ui::TypeRole::Caption, t)
                                    .child(humanize_last_connected(last)),
                            )
                        })
                        .child(
                            div()
                                .typography(issh_ui::TypeRole::Caption, t)
                                .child("回车"),
                        );
                    (s.id.clone(), inner.into_any_element(), is_kb_selected)
                })
                .collect();
            (rows, spacing.px_1)
        };
        // Phase A end — app / theme borrow 释放

        // ── Phase B：entity.update 灌 body / selected / on_click ──
        let row_entities: Vec<Entity<ListRow>> = body_phase1
            .into_iter()
            .map(|(sid, inner, is_kb_selected)| {
                let entity = self
                    .session_rows
                    .get(&sid)
                    .cloned()
                    .expect("session_rows 已 ensure for current sessions");
                let weak = cx.weak_entity();
                let sid_for_click = sid.clone();
                entity.update(cx, |r, _| {
                    r.body(inner)
                        .selected(is_kb_selected)
                        .on_click(move |_ev, _w, cx| {
                            if let Some(this) = weak.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.handle_pick(conn, sid_for_click.clone(), cx);
                                });
                            }
                        });
                });
                entity
            })
            .collect();

        // ── Phase C：拼装 dialog body ──
        let body: gpui::AnyElement = if sessions.is_empty() {
            // M28 T6: 用 EmptyState 替代 muted 一行文字，dialog 内紧凑场景
            // 不带 action（关闭即可回 raw shell，不需要主 CTA）。
            issh_ui::EmptyState::new("session-picker-empty")
                .icon(issh_ui::IconName::Inbox)
                .title("没有可用的 tmux 会话")
                .description("关闭此弹窗回到原始 shell")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(spacing_px_1)
                .children(row_entities)
                .into_any_element()
        };

        self.dialog.update(cx, |d, _cx| {
            d.body(body);
        });

        self.dialog.clone().into_any_element()
    }
}
