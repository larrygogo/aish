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
use crate::theme;

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
            .text_size(theme::text_sm())
            .text_color(rgb(theme::TEXT_PRIMARY))
            .bg(rgb(theme::BG_ELEVATED))
            .border_1()
            .border_color(rgb(theme::BORDER_SUBTLE))
            .rounded_md()
            .hover(|s| {
                s.bg(rgb(theme::BG_HOVER))
                    .border_color(rgb(theme::BORDER_STRONG))
                    .cursor_pointer()
            })
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

                // 该 host 的活跃连接数
                let active_count = app.connections.values().filter(|c| c.host_id == id).count();

                // ───── 左侧 avatar：host 名首字母 + 调色板配色 ─────
                let initial = label
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let avatar_bg = theme::avatar_color_for(&label);
                let avatar = div()
                    .w(px(40.0))
                    .h(px(40.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(avatar_bg))
                    .rounded_xl()
                    .text_color(rgb(0xffffff))
                    .text_size(theme::text_lg())
                    .child(initial);

                // ───── SSH chip ─────
                let chip = div()
                    .px_2p5()
                    .py_0p5()
                    .text_size(theme::text_xs())
                    .text_color(rgb(theme::ACCENT_BLUE))
                    .bg(rgb(theme::CHIP_BLUE_BG))
                    .rounded_full()
                    .child("SSH");

                // ───── 活跃数 chip（仅当 active_count > 0） ─────
                let active_chip: Option<gpui::AnyElement> = if active_count > 0 {
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .px_2p5()
                            .py_0p5()
                            .text_size(theme::text_xs())
                            .text_color(rgb(theme::ACCENT_GREEN))
                            .bg(rgb(theme::CHIP_GREEN_BG))
                            .rounded_full()
                            .child(div().text_color(rgb(theme::ACCENT_GREEN)).child("●"))
                            .child(format!("{} 活跃", active_count))
                            .into_any_element(),
                    )
                } else {
                    None
                };

                // ───── 编辑 / 删除 hover 按钮 ─────
                let edit_btn = div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .hover(|s| {
                        s.text_color(rgb(theme::TEXT_PRIMARY))
                            .bg(rgb(theme::BG_SELECTED))
                            .cursor_pointer()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            // 拦住事件不冒泡到外层卡片 listener 触发连接
                            cx.stop_propagation();
                            this.handle_edit_click(id, cx);
                        }),
                    )
                    .child("✎");

                let delete_btn = div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .hover(|s| {
                        s.text_color(rgb(theme::ACCENT_RED))
                            .bg(rgb(theme::BG_SELECTED))
                            .cursor_pointer()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            // 拦住事件不冒泡到外层卡片 listener 触发连接
                            cx.stop_propagation();
                            this.handle_delete_click(id, cx);
                        }),
                    )
                    .child("×");

                let actions = div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .opacity(0.0)
                    .group_hover("host_card", |s| s.opacity(1.0))
                    .child(edit_btn)
                    .child(delete_btn);

                // ───── 右侧 chevron `›` 暗示可点击 ─────
                let chevron = div()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .text_size(theme::text_lg())
                    .child("›");

                // ───── 整个卡片：avatar | 中间内容 flex_1 | actions + chevron ─────
                div()
                    .group("host_card")
                    .px_4()
                    .py_3p5()
                    .bg(rgb(theme::BG_ELEVATED))
                    .rounded_2xl()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).cursor_pointer())
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
                    .child(avatar)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                // 第一行：label + chips
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(rgb(theme::TEXT_PRIMARY))
                                            .text_size(theme::text_lg())
                                            .child(label),
                                    )
                                    .child(chip)
                                    .children(active_chip),
                            )
                            .child(
                                // 第二行：user@host:port
                                div()
                                    .text_color(rgb(theme::TEXT_SECONDARY))
                                    .text_size(theme::text_sm())
                                    .child(host_text),
                            ),
                    )
                    .child(actions)
                    .child(chevron)
            })
            .collect();

        let empty_hint = if app.hosts.is_empty() {
            Some(
                div()
                    .px_4()
                    .py_8()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .text_size(theme::text_sm())
                    .child("还没有保存的连接 — 点上方 + 添加 host 开始"),
            )
        } else {
            None
        };

        div()
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_8()
                    .pt_6()
                    .pb_3()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_color(rgb(theme::TEXT_PRIMARY))
                            .text_size(theme::text_xl())
                            .child("已保存的连接"),
                    )
                    .child(add_btn),
            )
            .child(
                div()
                    .px_8()
                    .pb_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(cards)
                    .children(empty_hint),
            )
    }
}
