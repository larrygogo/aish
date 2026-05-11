//! HomeView：4-tab 架构的 Home tab（M4a 信息架构）。
//!
//! 包含：Quick Actions（+ 添加 host）、Active Sessions（活跃连接列表）、
//! Hosts grid（host 卡片网格，复用 default_page.rs 原有逻辑）。

use std::sync::Arc;
use std::time::SystemTime;

use aish_types::{ConnectionId, HostId};
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{
    humanize_last_connected, AppState, HostFormDraft, HostFormState, SidebarTab, SshEvent, Tab,
    TabContent,
};

pub struct HomeView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HomeView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_add_click(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.modal = Some(HostFormState::Adding(HostFormDraft::default()));
            cx.notify();
        });
    }

    fn handle_card_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        let (conn_id, config, label) = self.state.update(cx, |s, cx| {
            let conn = s.open_connection(host_id);
            let cfg = s.hosts.iter().find(|h| h.id == host_id).cloned();
            let label = s
                .connections
                .get(&conn)
                .map(|c| c.label.clone())
                .unwrap_or_default();
            let tab_id = aish_types::TabId::new();
            let tab = Tab {
                id: tab_id,
                content: TabContent::Connection(conn),
                title: label.clone(),
            };
            s.tabs.push(tab);
            s.selected_tab = Some(s.tabs.last().unwrap().id);
            s.sidebar = SidebarTab::Terminal;
            s.last_connected.insert(host_id, SystemTime::now());
            let snapshot =
                crate::app_state_file::AppStateFile::from_last_connected(&s.last_connected);
            crate::app_state_file::save_app_state(&snapshot);
            cx.notify();
            (conn, cfg, label)
        });

        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!(?host_id, "home: host config not found");
                return;
            }
        };
        tracing::info!(?conn_id, %label, "home: spawn connection");

        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |s, _cx| {
            s.register_session(conn_id, sender);
        });
    }

    fn handle_edit_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            if let Some(cfg) = s.hosts.iter().find(|h| h.id == host_id).cloned() {
                s.modal = Some(HostFormState::Editing {
                    id: host_id,
                    draft: HostFormDraft::from_config(&cfg),
                });
                cx.notify();
            }
        });
    }

    fn handle_delete_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            if let Some(cfg) = s.hosts.iter().find(|h| h.id == host_id).cloned() {
                s.modal = Some(HostFormState::DeleteConfirm {
                    id: host_id,
                    label: cfg.label,
                });
                cx.notify();
            }
        });
    }

    fn handle_open_session(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            let tab_id = s
                .tabs
                .iter()
                .find(|t| t.content == TabContent::Connection(conn_id))
                .map(|t| t.id);

            if let Some(id) = tab_id {
                s.selected_tab = Some(id);
            } else {
                let label = s
                    .connections
                    .get(&conn_id)
                    .map(|c| c.label.clone())
                    .unwrap_or_else(|| "connection".into());
                let tab = Tab {
                    id: aish_types::TabId::new(),
                    content: TabContent::Connection(conn_id),
                    title: label,
                };
                s.tabs.push(tab);
                s.selected_tab = Some(s.tabs.last().unwrap().id);
            }
            s.sidebar = SidebarTab::Terminal;
            cx.notify();
        });
    }
}

impl Render for HomeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        // ───── Quick Actions 顶部栏 ─────
        let add_btn = aish_ui::Button::new("home-add-host-btn")
            .label("+ 添加 host")
            .secondary()
            .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_add_click(cx)));

        let header = div()
            .px_8()
            .pt_6()
            .pb_3()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_color(colors.foreground)
                    .text_size(font_size.xl)
                    .child("Home"),
            )
            .child(add_btn);

        // ───── Active Sessions 区 ─────
        // 收集所有 connection 的快照（避免在闭包里借用 app）
        let active_connections: Vec<(ConnectionId, String, String, bool)> = app
            .connections
            .values()
            .map(|c| {
                let time_str = c.humanize_opened_at();
                let is_active = app.is_session_active(c.id);
                (c.id, c.label.clone(), time_str, is_active)
            })
            .collect();

        let active_section: Option<gpui::AnyElement> = if active_connections.is_empty() {
            None
        } else {
            let rows: Vec<_> = active_connections
                .into_iter()
                .map(|(conn_id, label, time_str, is_active)| {
                    // 左侧状态圆点
                    let dot_color = if is_active {
                        colors.success
                    } else {
                        colors.muted_foreground
                    };
                    let dot = div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(dot_color)
                        .flex_shrink_0();

                    // label + time
                    let label_part = div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(colors.foreground)
                                .text_size(font_size.sm)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_color(colors.muted_foreground)
                                .text_size(font_size.xs)
                                .child(format!("· {}", time_str)),
                        );

                    // Open 按钮
                    let open_btn = aish_ui::Button::new(gpui::SharedString::from(format!(
                        "active-session-open-{}",
                        conn_id
                    )))
                    .label("Open ▶")
                    .secondary()
                    .on_click(cx.listener(
                        move |this, _ev: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            this.handle_open_session(conn_id, cx);
                        },
                    ));

                    // 整行可点击
                    div()
                        .px_4()
                        .py_2p5()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(|s| s.bg(colors.card))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                                this.handle_open_session(conn_id, cx);
                            }),
                        )
                        .child(dot)
                        .child(label_part)
                        .child(open_btn)
                })
                .collect();

            Some(
                div()
                    .px_8()
                    .pb_4()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .pb_2()
                            .text_color(colors.muted_foreground)
                            .text_size(font_size.xs)
                            .child("ACTIVE SESSIONS"),
                    )
                    .children(rows)
                    .into_any_element(),
            )
        };

        // ───── Hosts grid ─────
        let cards: Vec<_> = app
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let host_text = format!("{}@{}:{}", h.user, h.host, h.port);
                let last_conn_str: Option<String> = app
                    .last_connected
                    .get(&id)
                    .map(|t| humanize_last_connected(*t));

                // 该 host 的活跃连接数
                let active_count = app.connections.values().filter(|c| c.host_id == id).count();

                // ───── 左侧 avatar：host 名首字母 + 调色板配色 ─────
                let initial = label
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let avatar_bg = crate::avatar::avatar_color_for(&label);
                let avatar = div()
                    .w(px(40.0))
                    .h(px(40.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(avatar_bg))
                    .rounded_xl()
                    .text_color(colors.primary_foreground)
                    .text_size(font_size.lg)
                    .child(initial);

                // ───── SSH chip ─────
                let chip = aish_ui::Badge::new("SSH").primary();

                // ───── 活跃数 chip（仅当 active_count > 0） ─────
                let active_chip: Option<gpui::AnyElement> = if active_count > 0 {
                    Some(
                        aish_ui::Badge::new(format!("● {} 活跃", active_count))
                            .success()
                            .into_any_element(),
                    )
                } else {
                    None
                };

                // ───── 编辑 / 删除 hover 按钮 ─────
                let edit_btn = aish_ui::IconButton::new(
                    gpui::SharedString::from(format!("host-edit-{}", id)),
                    aish_ui::IconName::Pencil,
                )
                .small()
                .ghost()
                .on_click(cx.listener(
                    move |this, _ev: &MouseDownEvent, _w, cx| {
                        cx.stop_propagation();
                        this.handle_edit_click(id, cx);
                    },
                ));

                let delete_btn = aish_ui::IconButton::new(
                    gpui::SharedString::from(format!("host-delete-{}", id)),
                    aish_ui::IconName::X,
                )
                .small()
                .destructive()
                .on_click(cx.listener(
                    move |this, _ev: &MouseDownEvent, _w, cx| {
                        cx.stop_propagation();
                        this.handle_delete_click(id, cx);
                    },
                ));

                let actions = div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .opacity(0.0)
                    .group_hover("host_card", |s| s.opacity(1.0))
                    .child(edit_btn)
                    .child(delete_btn);

                // ───── 右侧 chevron ─────
                let chevron = div()
                    .text_color(colors.muted_foreground)
                    .text_size(font_size.lg)
                    .child("›");

                // ───── 整个卡片 ─────
                div()
                    .group("host_card")
                    .px_4()
                    .py_3p5()
                    .bg(colors.card)
                    .rounded_2xl()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.accent))
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
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(colors.foreground)
                                            .text_size(font_size.lg)
                                            .child(label),
                                    )
                                    .child(chip)
                                    .children(active_chip),
                            )
                            .child(
                                div()
                                    .text_color(colors.secondary_foreground)
                                    .text_size(font_size.sm)
                                    .child(host_text),
                            )
                            .children(last_conn_str.map(|s| {
                                div()
                                    .text_color(colors.muted_foreground)
                                    .text_size(px(11.0))
                                    .child(format!("上次连接 {}", s))
                            })),
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
                    .text_color(colors.muted_foreground)
                    .text_size(font_size.sm)
                    .child("还没有保存的连接 — 点上方 + 添加 host 开始"),
            )
        } else {
            None
        };

        let hosts_section_label = div()
            .pb_2()
            .text_color(colors.muted_foreground)
            .text_size(font_size.xs)
            .child("HOSTS");

        div()
            .size_full()
            .bg(colors.background)
            .flex()
            .flex_col()
            .child(header)
            .children(active_section)
            .child(
                div()
                    .px_8()
                    .pb_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(hosts_section_label)
                    .children(cards)
                    .children(empty_hint),
            )
    }
}
