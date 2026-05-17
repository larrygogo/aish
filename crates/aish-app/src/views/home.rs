//! HomeView：4-tab 架构的 Home tab（M4a 信息架构）。
//!
//! 包含：Quick Actions（+ 添加 host）、Active Sessions（活跃连接列表）、
//! Hosts grid（host 卡片网格，复用 default_page.rs 原有逻辑）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use aish_types::{ConnectionId, HostId};
use aish_ui::{Button, CardEntity, IconButton, TypographyExt};
use gpui::{div, prelude::*, px, rgb, Context, Entity, KeyDownEvent, MouseDownEvent, Window};

use crate::app::retain_alive_entities;
use crate::bridge::Bridge;
use crate::state::{
    humanize_last_connected, AppState, ConnectionPhase, HostFormDraft, HostFormState, SidebarTab,
    SshEvent, Tab, TabContent,
};
use crate::views::home_preview::{
    extract_term_chars_or_empty, last_n_rows_from_chars, preview_branch_for_phase, PreviewBranch,
    PreviewSnapshot,
};

/// M31：每张 host card 的 edit/delete IconButton entity 对，
/// 按 HostId 在 HomeView.host_card_buttons HashMap 维护。
struct HostCardButtons {
    edit: Entity<IconButton>,
    delete: Entity<IconButton>,
}

pub struct HomeView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    /// host 卡片右键菜单。content 每帧由 render 根据 menu_host_id 重设。
    /// host 卡片右键菜单 entity。RootView root 顶层 mount，避免被
    /// 下游 view 盖（与 tab_bar 同模式）。
    context_menu: Entity<aish_ui::ContextMenu>,
    /// 当前右键菜单针对的 host id。`None` = 菜单关闭。
    menu_host_id: Option<HostId>,
    /// 键盘导航当前选中菜单项索引。打开时重置 0，↑/↓ 调，Enter 触发。
    menu_active_idx: usize,
    /// hosts 列表纵向滚动 + 可拖拽 thumb 状态。aish_ui::ScrollbarHandle 内
    /// 含 ScrollHandle + drag flag，ScrollPage builder 一句接管所有 wheel /
    /// scrollbar / 拖拽行为。
    scrollbar: aish_ui::ScrollbarHandle,
    /// M31：顶部 page header "+ 添加 host" button（永远显示）。
    header_add_btn: Entity<aish_ui::Button>,
    /// M31：空 hosts 状态 EmptyState 内的 add button（条件显示）。
    empty_add_btn: Entity<aish_ui::Button>,
    /// M31：hosts.json 加载失败时 ErrorState 内的 retry button（条件显示）。
    retry_btn: Entity<aish_ui::Button>,
    /// M31：每张 host card 的 edit + delete IconButton 对，按 HostId 索引。
    /// render 前 retain_alive_entities 同步 host 集合，避免 entity 泄漏。
    host_card_buttons: HashMap<HostId, HostCardButtons>,
    /// M31：active sessions 列表每行的 "Open ▶" Button，按 ConnectionId 索引。
    session_open_buttons: HashMap<ConnectionId, Entity<Button>>,
    /// M33: 每张 host card 的 CardEntity（hover transition + press feedback），
    /// 按 HostId 索引。render 顶部 retain + ensure 同 host_card_buttons。
    host_cards: HashMap<HostId, Entity<CardEntity>>,
    /// M36 T3 + M36.1: Active session 大卡（CardEntity）替代原 ListRow，
    /// 按 ConnectionId 索引。render 顶部 retain + ensure 同 host_cards。
    /// M36.1 改 poster 风：body 内 z-stack（preview 满铺底层 + bottom
    /// gradient scrim + overlay 文字顶层），由 phase B 在每帧灌 body。
    active_cards: HashMap<ConnectionId, Entity<CardEntity>>,
}

/// host 右键菜单 item 数量（编辑 / 复制 / 删除）。与 render 内 items() 匹配。
const HOST_MENU_ITEM_COUNT: usize = 3;

impl HomeView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let context_menu = cx.new(aish_ui::ContextMenu::new);
        let weak_close = cx.weak_entity();
        let weak_key = cx.weak_entity();
        context_menu.update(cx, move |m, _cx| {
            m.on_close(move |_w, cx| {
                if let Some(this) = weak_close.upgrade() {
                    this.update(cx, |this, _cx| {
                        this.menu_host_id = None;
                    });
                }
            });
            // 键盘 ↑/↓/Enter 导航
            m.on_key(move |ev, _w, cx| {
                if let Some(this) = weak_key.upgrade() {
                    this.update(cx, |this, cx| this.handle_menu_key(ev, cx));
                }
            });
        });
        // M31: 3 个单例 button entity，weak.upgrade callback 透传到 self method
        let weak_add_header = cx.weak_entity();
        let header_add_btn = cx.new(|cx| {
            let mut b = aish_ui::Button::new("home-add-host-btn", cx);
            b.label("+ 添加 host")
                .primary()
                .on_click(move |_ev, _w, cx| {
                    if let Some(this) = weak_add_header.upgrade() {
                        this.update(cx, |this, cx| this.handle_add_click(cx));
                    }
                });
            b
        });
        let weak_add_empty = cx.weak_entity();
        let empty_add_btn = cx.new(|cx| {
            let mut b = aish_ui::Button::new("home-empty-add-host", cx);
            b.label("+ 添加 host")
                .primary()
                .on_click(move |_ev, _w, cx| {
                    if let Some(this) = weak_add_empty.upgrade() {
                        this.update(cx, |this, cx| this.handle_add_click(cx));
                    }
                });
            b
        });
        let weak_retry = cx.weak_entity();
        let retry_btn = cx.new(|cx| {
            let mut b = aish_ui::Button::new("home-hosts-load-retry", cx);
            b.label("重试加载").primary().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_retry.upgrade() {
                    this.update(cx, |this, cx| this.handle_retry_load_hosts(cx));
                }
            });
            b
        });

        Self {
            state,
            bridge,
            tx,
            context_menu,
            menu_host_id: None,
            menu_active_idx: 0,
            scrollbar: aish_ui::ScrollbarHandle::new(),
            header_add_btn,
            empty_add_btn,
            retry_btn,
            host_card_buttons: HashMap::new(),
            session_open_buttons: HashMap::new(),
            host_cards: HashMap::new(),
            active_cards: HashMap::new(),
        }
    }

    /// context menu 键盘导航：↑/↓ 改 active_idx，Enter 触发选中项。
    fn handle_menu_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(host_id) = self.menu_host_id else {
            return;
        };
        match ev.keystroke.key.as_str() {
            "up" => {
                self.menu_active_idx = if self.menu_active_idx == 0 {
                    HOST_MENU_ITEM_COUNT - 1
                } else {
                    self.menu_active_idx - 1
                };
                cx.notify();
            }
            "down" => {
                self.menu_active_idx = (self.menu_active_idx + 1) % HOST_MENU_ITEM_COUNT;
                cx.notify();
            }
            "enter" => {
                let idx = self.menu_active_idx;
                self.handle_menu_select(host_id, idx, cx);
            }
            _ => {}
        }
    }

    /// 复制 SSH 命令到剪贴板（如 `ssh -p 22 user@host`）。
    fn copy_ssh_command(&self, host_id: HostId, cx: &mut Context<Self>) {
        let cfg = self
            .state
            .read(cx)
            .hosts
            .iter()
            .find(|h| h.id == host_id)
            .cloned();
        if let Some(cfg) = cfg {
            // port 22 省略 -p（标准默认）；非 22 才显式 -p
            let cmd = if cfg.port == 22 {
                format!("ssh {}@{}", cfg.user, cfg.host)
            } else {
                format!("ssh -p {} {}@{}", cfg.port, cfg.user, cfg.host)
            };
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd.clone()));
            aish_ui::toast_success(cx, format!("已复制：{}", cmd));
        }
    }

    /// 右键菜单 select 路由。idx 对应 build_context_menu 内 MenuItem 顺序。
    fn handle_menu_select(&mut self, host_id: HostId, idx: usize, cx: &mut Context<Self>) {
        match idx {
            0 => self.handle_edit_click(host_id, cx),
            1 => self.copy_ssh_command(host_id, cx),
            2 => self.handle_delete_click(host_id, cx),
            _ => {}
        }
        self.context_menu.update(cx, |m, cx| m.close(cx));
        self.menu_host_id = None;
    }

    fn handle_add_click(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.modal = Some(HostFormState::Adding(HostFormDraft::default()));
            cx.notify();
        });
    }

    /// M28 T7：hosts.json load 失败 ErrorState 上点"重试"路径。
    /// 重新调 persistence::load_hosts()，成功清 error + 替换 state.hosts；
    /// 失败更新 error message。
    fn handle_retry_load_hosts(&mut self, cx: &mut Context<Self>) {
        match crate::persistence::load_hosts() {
            Ok(hosts) => {
                self.state.update(cx, |s, cx| {
                    s.hosts = hosts;
                    s.hosts_load_error = None;
                    cx.notify();
                });
                aish_ui::toast_success(cx, "主机列表已恢复");
            }
            Err(e) => {
                let msg = format!("{}", e);
                self.state.update(cx, |s, cx| {
                    s.hosts_load_error = Some(msg.clone());
                    cx.notify();
                });
                aish_ui::toast_error(cx, format!("加载失败：{}", msg));
            }
        }
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
            // tab 默认 title 用 host.label（不带 #N 后缀），等 shell PS1 通过
            // OSC 0/1/2 发来真实 title 再覆盖。Connection.label 内部仍是
            // "<host.label> #N" 用于 toast / phase overlay 区分多连接。
            let default_title = cfg
                .as_ref()
                .map(|c| c.label.clone())
                .unwrap_or_else(|| label.clone());
            let tab_id = aish_types::TabId::new();
            let tab = Tab {
                id: tab_id,
                content: TabContent::Connection(conn),
                title: default_title,
                title_locked: false,
            };
            s.tabs.push(tab);
            s.selected_tab = Some(s.tabs.last().unwrap().id);
            s.sidebar = SidebarTab::Terminal;
            s.last_connected.insert(host_id, SystemTime::now());
            // merge 而非 from_*：保留 theme 等其他持久化字段，避免覆盖用户偏好
            let snapshot =
                crate::app_state_file::load_app_state().merge_last_connected(&s.last_connected);
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

    /// M36 T5: active 大卡 click 按 phase 分流。
    /// - Connected/Connecting: 走 attach 路径（handle_open_session 复用）
    /// - Disconnected: 走 reconnect 路径（重 spawn actor + reopen_connection）
    fn handle_active_card_click(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        let is_disconnected = matches!(
            self.state.read(cx).connection_phases.get(&conn_id),
            Some(ConnectionPhase::Disconnected { .. })
        );
        if is_disconnected {
            self.handle_reconnect(conn_id, cx);
        } else {
            self.handle_open_session(conn_id, cx);
        }
    }

    /// M36 T5: 重连 — 复用同一 ConnectionId 重 spawn actor，phase 回 Connecting。
    /// 顺序按 state.rs:766 doc：先 spawn 拿 sender，再调 reopen_connection。
    fn handle_reconnect(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        let config = {
            let app = self.state.read(cx);
            app.connections
                .get(&conn_id)
                .and_then(|c| app.hosts.iter().find(|h| h.id == c.host_id).cloned())
        };
        let Some(config) = config else {
            tracing::warn!(?conn_id, "home: reconnect skipped — host config missing");
            return;
        };
        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |s, cx| {
            s.reopen_connection(conn_id, sender);
            cx.notify();
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
                // 默认 title 取 host.label（与 handle_card_click 一致）
                let host_id = s.connections.get(&conn_id).map(|c| c.host_id);
                let default_title = host_id
                    .and_then(|hid| {
                        s.hosts
                            .iter()
                            .find(|h| h.id == hid)
                            .map(|h| h.label.clone())
                    })
                    .unwrap_or_else(|| "connection".into());
                let tab = Tab {
                    id: aish_types::TabId::new(),
                    content: TabContent::Connection(conn_id),
                    title: default_title,
                    title_locked: false,
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
        // 每帧重设 context menu content（AnyElement take 消耗，必须重设）
        if let Some(host_id) = self.menu_host_id {
            let weak = cx.weak_entity();
            let menu = aish_ui::DropdownMenu::new("host-context-menu")
                .items(vec![
                    aish_ui::MenuItem::new("编辑").icon(aish_ui::IconName::Pencil),
                    aish_ui::MenuItem::new("复制 SSH 命令"),
                    aish_ui::MenuItem::new("删除").icon(aish_ui::IconName::Trash),
                ])
                .min_width(gpui::px(200.0))
                .selected_idx(Some(self.menu_active_idx))
                .on_select(move |idx, _w, cx| {
                    let idx = *idx;
                    if let Some(this) = weak.upgrade() {
                        this.update(cx, |this, cx| this.handle_menu_select(host_id, idx, cx));
                    }
                });
            self.context_menu.update(cx, |m, _cx| {
                m.content(menu);
            });
        }
        // M31：先同步 host_card_buttons / session_open_buttons HashMap
        // —— retain 清掉已删除的 host / closed connection 对应 entity，
        //    然后 ensure 当前活跃 key 都有 entry（lazy create）。
        {
            let app = self.state.read(cx);
            let host_ids: std::collections::HashSet<HostId> =
                app.hosts.iter().map(|h| h.id).collect();
            let conn_ids: std::collections::HashSet<ConnectionId> =
                app.connections.keys().copied().collect();
            retain_alive_entities(&mut self.host_card_buttons, |k| host_ids.contains(k));
            retain_alive_entities(&mut self.session_open_buttons, |k| conn_ids.contains(k));
            // M33: host_cards 同 host 集合同步
            retain_alive_entities(&mut self.host_cards, |k| host_ids.contains(k));
            retain_alive_entities(&mut self.active_cards, |k| conn_ids.contains(k));
        }
        // ensure entries (lazy create)
        let hosts_snapshot: Vec<HostId> = self.state.read(cx).hosts.iter().map(|h| h.id).collect();
        for id in &hosts_snapshot {
            if !self.host_card_buttons.contains_key(id) {
                let host_id = *id;
                let weak_e = cx.weak_entity();
                let weak_d = cx.weak_entity();
                let edit = cx.new(move |cx| {
                    let mut b = IconButton::new(
                        gpui::SharedString::from(format!("host-edit-{}", host_id)),
                        aish_ui::IconName::Pencil,
                        cx,
                    );
                    b.small().ghost().on_click(move |_ev, _w, cx| {
                        if let Some(this) = weak_e.upgrade() {
                            this.update(cx, |this, cx| {
                                cx.stop_propagation();
                                this.handle_edit_click(host_id, cx);
                            });
                        }
                    });
                    b
                });
                let delete = cx.new(move |cx| {
                    let mut b = IconButton::new(
                        gpui::SharedString::from(format!("host-delete-{}", host_id)),
                        aish_ui::IconName::X,
                        cx,
                    );
                    b.small().ghost().on_click(move |_ev, _w, cx| {
                        if let Some(this) = weak_d.upgrade() {
                            this.update(cx, |this, cx| {
                                cx.stop_propagation();
                                this.handle_delete_click(host_id, cx);
                            });
                        }
                    });
                    b
                });
                self.host_card_buttons
                    .insert(host_id, HostCardButtons { edit, delete });
            }
        }
        let conns_snapshot: Vec<ConnectionId> =
            self.state.read(cx).connections.keys().copied().collect();
        for conn_id in &conns_snapshot {
            if !self.session_open_buttons.contains_key(conn_id) {
                let cid = *conn_id;
                let weak_o = cx.weak_entity();
                let btn = cx.new(move |cx| {
                    let mut b = Button::new(
                        gpui::SharedString::from(format!("active-session-open-{}", cid)),
                        cx,
                    );
                    b.label("打开 ▶").secondary().on_click(move |_ev, _w, cx| {
                        if let Some(this) = weak_o.upgrade() {
                            this.update(cx, |this, cx| {
                                cx.stop_propagation();
                                this.handle_open_session(cid, cx);
                            });
                        }
                    });
                    b
                });
                self.session_open_buttons.insert(*conn_id, btn);
            }
            // M36 T3/T5: active session 大卡（CardEntity），inner = header + meta
            // + preview + actions 由 phase B 灌 body。整卡 click → 路由到
            // handle_active_card_click 内按 phase 分流（Connected/Connecting
            // → handle_open_session attach；Disconnected → handle_reconnect）。
            if !self.active_cards.contains_key(conn_id) {
                let cid = *conn_id;
                let card_id: gpui::ElementId =
                    gpui::SharedString::from(format!("home-active-card-{}", cid)).into();
                let weak = cx.weak_entity();
                let primary = aish_ui::theme(cx).colors.primary;
                let card = cx.new(move |c| {
                    let mut card = CardEntity::new(card_id, c);
                    card.hover_glow(primary).on_click(move |_ev, _w, cx| {
                        if let Some(this) = weak.upgrade() {
                            this.update(cx, |this, cx| this.handle_active_card_click(cid, cx));
                        }
                    });
                    card
                });
                self.active_cards.insert(*conn_id, card);
            }
            // M36.1 T1: 删 attach_buttons —— 整卡 click 已支持 attach/reconnect 分流
            // （handle_active_card_click），attach button 视觉冗余。
        }
        // M33: ensure host_cards CardEntity for each host
        for id in &hosts_snapshot {
            if !self.host_cards.contains_key(id) {
                let host_id = *id;
                let weak = cx.weak_entity();
                let primary = aish_ui::theme(cx).colors.primary;
                let card = cx.new(move |cx| {
                    let mut c = CardEntity::new(
                        gpui::SharedString::from(format!("host-card-{}", host_id)),
                        cx,
                    );
                    c.no_padding()
                        .hover_glow(primary)
                        .on_click(move |_ev, _w, cx| {
                            if let Some(this) = weak.upgrade() {
                                this.update(cx, |this, cx| this.handle_card_click(host_id, cx));
                            }
                        });
                    c
                });
                self.host_cards.insert(host_id, card);
            }
        }

        // M33 续做 + render split：phase A 用 block scope 包 app + theme borrow，
        // 收集所有 owned outputs（header / active_section / cards_phase1 /
        // hosts_section_label + captured anatomy / bg / load_error 等）；
        // block 结束 borrow 释放。phase B 调 host_cards entity.update(cx, ...)
        // 灌 body + build cards wrap Vec。phase C 用 captured values 组装
        // final layout（不再借 theme/app）。
        let (
            header_el,
            active_section_label,
            active_cards_phase1,
            separator_el,
            cards_phase1,
            hosts_section_label_el,
            load_error,
            hosts_is_empty,
            bg_color,
            anatomy_outer_px,
            anatomy_outer_py_bottom,
            anatomy_list_gap,
        ) = {
            let app = self.state.read(cx);
            let theme = aish_ui::theme(cx);
            let colors = theme.colors;
            let font_size = theme.font_size;

            // ───── Quick Actions 顶部栏 ─────
            // 顶部主 CTA → primary。M31：header_add_btn entity 持 press feedback。
            let add_btn = self.header_add_btn.clone();

            // M27: page header padding 走 anatomy.page。
            // M35 T5: outer_py_top → outer_py_spacious（24 → 40）让 hero
            // 顶部更宽松，符合 Linear / Vercel 留白风格。
            let header = div()
                .px(theme.anatomy.page.outer_px)
                .pt(theme.anatomy.page.outer_py_spacious)
                .pb(theme.anatomy.page.header_to_content_gap)
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    // M26 T2: page title 用 Title1 (20/600/fg) 替代 xl size-only
                    div()
                        .typography(aish_ui::TypeRole::Title1, theme)
                        .child("主页"),
                )
                .child(add_btn);

            // ───── Active Sessions 区 ─────
            // M36 T3: active_connections 老 4-tuple shape (id, label, time, is_active)
            // 删除 — 被下方 active_previews + active_cards_phase1 取代。原
            // is_active 用于 dot 着色，新设计按 ConnectionPhase 给 dot 着色
            // (success/muted/destructive)，语义更精确。

            // M36 T2: 收集 active session preview snapshots (Phase A — owned)
            // 走 owned 路径（chars 二维数组 + last_n_rows trim），drop app borrow 后
            // T3/T4 phase B 用 snapshot 构造大卡 inner。3 phase enum → 3 个 bool
            // 解耦本模块（home_preview 不依赖 state::ConnectionPhase 类型）。
            let active_previews: HashMap<ConnectionId, PreviewSnapshot> = app
                .connections
                .iter()
                .filter_map(|(id, conn)| {
                    let phase = app.connection_phases.get(id).cloned()?;
                    let term_opt = app.host_pty_term.get(id);

                    let (phase_is_connected, phase_is_connecting, phase_is_disconnected, reason) =
                        match &phase {
                            ConnectionPhase::Connected => (true, false, false, None),
                            ConnectionPhase::Connecting => (false, true, false, None),
                            ConnectionPhase::Disconnected { reason } => {
                                (false, false, true, Some(reason.clone()))
                            }
                        };

                    let (preview, cursor_in_window) = if let Some(term) = term_opt {
                        let chars = extract_term_chars_or_empty(term);
                        let total_rows = chars.len();
                        let rows = last_n_rows_from_chars(chars, 6);
                        // cursor 在 last 6 行窗口内才记录 (row 是 0-based 从 top)
                        let cursor_pt = term.grid().cursor.point;
                        let cursor_line_from_top = cursor_pt.line.0 as usize;
                        let window_start = total_rows.saturating_sub(6);
                        let cursor_in_window = if cursor_line_from_top >= window_start
                            && cursor_line_from_top < total_rows
                        {
                            Some((cursor_line_from_top - window_start, cursor_pt.column.0))
                        } else {
                            None
                        };
                        (rows, cursor_in_window)
                    } else {
                        (Vec::new(), None)
                    };

                    Some((
                        *id,
                        PreviewSnapshot {
                            phase_is_connected,
                            phase_is_connecting,
                            phase_is_disconnected,
                            disconnect_reason: reason,
                            preview,
                            cursor_in_window,
                            opened_at: conn.opened_at,
                        },
                    ))
                })
                .collect();

            // M36 T3: active 大卡 phase A build。每张大卡 inner = header
            // (phase dot + host label + tmux chip) + meta (user@host:port + 存活
            // 时长) + preview 占位框 (T4 接入 4 phase 兜底视觉)。
            //
            // Phase A 内 borrow app 查 host_label / user_at_host / tmux_label
            // 不需要塞 PreviewSnapshot；inner 在 Phase A scope 内组装为
            // AnyElement，Phase B 仅做 CardEntity.body() 灌入。
            let active_cards_phase1: Vec<(ConnectionId, gpui::AnyElement)> = {
                use crate::state::TmuxState;
                use crate::views::home_preview::format_active_duration;

                // M36 fix（闪烁 root cause）：HashMap iter 顺序不稳定 →
                // 多个 active session 时 grid children 位置每帧 swap → mouse
                // 静止但 hover 目标在两卡间跳变 → fire_hover(true)/(false)
                // 反复触发 → animate path 反复重启 → border/bg 视觉闪。
                // 按 ConnectionId.0 (Uuid) 排序保证顺序稳定。
                let mut sorted_previews: Vec<(&ConnectionId, &PreviewSnapshot)> =
                    active_previews.iter().collect();
                sorted_previews.sort_by_key(|(id, _)| id.0);

                let mut out: Vec<(ConnectionId, gpui::AnyElement)> = Vec::new();
                for (conn_id, snap) in sorted_previews.iter().copied() {
                    let host_cfg_opt = app
                        .connections
                        .get(conn_id)
                        .and_then(|c| app.hosts.iter().find(|h| h.id == c.host_id));
                    let host_label = host_cfg_opt
                        .map(|h| h.label.clone())
                        .unwrap_or_else(|| "unknown".into());
                    let user_at_host = host_cfg_opt
                        .map(|h| format!("{}@{}:{}", h.user, h.host, h.port))
                        .unwrap_or_default();
                    let tmux_label: Option<String> = match app.tmux_state.get(conn_id) {
                        Some(TmuxState::Detected {
                            sessions,
                            attached: Some(sid),
                        }) => sessions
                            .iter()
                            .find(|s| &s.id == sid)
                            .map(|s| s.name.clone()),
                        _ => None,
                    };

                    // phase dot 色按 ConnectionPhase
                    let phase_dot_color = if snap.phase_is_connected {
                        colors.success
                    } else if snap.phase_is_connecting {
                        colors.muted_foreground
                    } else {
                        colors.destructive
                    };

                    // M36.1 T2: header overlay 文字（host_label + tmux chip）—
                    // foreground 高对比 + Title3，作为 poster 顶部信息。
                    let mut header_row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(phase_dot_color)
                                .flex_shrink_0(),
                        )
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Title3, theme)
                                .text_color(colors.foreground)
                                .child(host_label.clone()),
                        );
                    if let Some(t) = tmux_label {
                        header_row = header_row
                            .child(div().text_color(colors.muted_foreground).child("·"))
                            .child(
                                // M36.1 follow-up: 之前用 Nerd Font PUA 字符
                                // \u{e712} (nf-dev-tmux)，依赖 bundled 字体；
                                // 改 SVG IconName::Tmux 跨字体稳，与 sidebar
                                // SVG icon 系统一致。chip = icon + label 横排。
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .typography(aish_ui::TypeRole::Code, theme)
                                    .text_color(colors.muted_foreground)
                                    .child(
                                        aish_ui::icon(aish_ui::IconName::Tmux)
                                            .size(px(12.0))
                                            .text_color(colors.muted_foreground),
                                    )
                                    .child(format!("tmux:{}", t)),
                            );
                    }

                    let duration_str = format_active_duration(snap.opened_at, SystemTime::now());
                    let meta_row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Code, theme)
                                .text_color(colors.secondary_foreground)
                                .child(user_at_host),
                        )
                        .child(div().text_color(colors.muted_foreground).child("·"))
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Caption, theme)
                                .text_color(colors.muted_foreground)
                                .child(duration_str),
                        );

                    // M36 T4 + M36.1 T2/T3: preview 内容按 PreviewBranch 4 分支
                    // 渲染。M36.1 改 poster 风：每个分支返回**满铺底层**内容，
                    // 不再独立 padding（py_2 / py_2 / 中央居中由各 phase 自己挂）
                    let preview_empty = snap.preview.iter().all(|line| line.is_empty());
                    let branch = preview_branch_for_phase(
                        snap.phase_is_connected,
                        snap.phase_is_connecting,
                        snap.phase_is_disconnected,
                        preview_empty,
                    );

                    let preview_layer: gpui::AnyElement = match branch {
                        PreviewBranch::ShowCells => {
                            let lines = snap.preview.clone();
                            let cursor = snap.cursor_in_window;
                            // 满铺 cells 到卡片 edge，pt_3 让顶部 cells 不被
                            // overlay 文字 absolute 区域贴住（虽然 overlay 在
                            // 底，header 在底而非顶 → 顶部不需要避让）；保留
                            // px_3 让 cells 不顶卡片左右 edge 太紧
                            div()
                                .size_full()
                                .flex()
                                .flex_col()
                                .px_3()
                                .py_2()
                                .children(lines.into_iter().enumerate().map(|(row_idx, line)| {
                                    let line_with_cursor =
                                        if cursor.map(|(r, _)| r) == Some(row_idx) {
                                            format!("{}█", line)
                                        } else {
                                            line
                                        };
                                    div()
                                        .text_size(px(10.0))
                                        .font(aish_ui::code_font())
                                        .text_color(colors.muted_foreground)
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .child(line_with_cursor)
                                        .into_any_element()
                                }))
                                .into_any_element()
                        }
                        PreviewBranch::WaitingForOutput => div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .typography(aish_ui::TypeRole::Caption, theme)
                                    .text_color(colors.muted_foreground)
                                    .child("等待输出..."),
                            )
                            .into_any_element(),
                        PreviewBranch::Loading => div()
                            .size_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                aish_ui::icon(aish_ui::IconName::Loader)
                                    .size(px(16.0))
                                    .text_color(colors.muted_foreground),
                            )
                            .child(
                                div()
                                    .typography(aish_ui::TypeRole::Caption, theme)
                                    .text_color(colors.muted_foreground)
                                    .child("连接中..."),
                            )
                            .into_any_element(),
                        PreviewBranch::DisconnectedHint => div()
                            .size_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .bg(colors.destructive.opacity(0.05))
                            .child(
                                aish_ui::icon(aish_ui::IconName::AlertTriangle)
                                    .size(px(16.0))
                                    .text_color(colors.destructive),
                            )
                            .child(
                                div()
                                    .typography(aish_ui::TypeRole::Caption, theme)
                                    .text_color(colors.destructive)
                                    .child("已断开 · 点击重连"),
                            )
                            .into_any_element(),
                    };

                    // M36.1 T2: Poster z-stack
                    // - 父 relative + 固定高度（180px ≈ 原 vertical stack 总高）
                    // - 子1 preview_layer：absolute inset 0 满铺底层
                    // - 子2 scrim+overlay：absolute 浮底部，linear_gradient
                    //   transparent → card_bg 覆盖下 ~45% (px 80)，内含 header+meta
                    // - overflow_hidden 防 ShowCells 多余行溢出卡片
                    use gpui::{linear_color_stop, linear_gradient};
                    let card_bg = colors.card;
                    let inner = div()
                        .relative()
                        // M36.1 follow-up: aspect_ratio + min/max 4 边夹住
                        // - aspect_ratio(1.6): 16:10 目标比例（Steam / Apple
                        //   Music album view 同款），GPUI taffy 在 grid item
                        //   下不一定严格生效，min/max 兜底
                        // - 宽度区间 [280, 420]: 用户反馈"还是太大"，max
                        //   从 520 收到 420，下限 320→280 防 grid 间距挤压
                        // - 高度区间 [175, 263]: width / 1.6 配套
                        .aspect_ratio(1.6)
                        .w_full()
                        .min_w(px(280.0))
                        .max_w(px(420.0))
                        .min_h(px(175.0))
                        .max_h(px(263.0))
                        .overflow_hidden()
                        .child(
                            // 底层：preview 满铺
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .child(preview_layer),
                        )
                        .child(
                            // 顶层：bottom gradient scrim + overlay 文字
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .h(px(80.0))
                                .bg(linear_gradient(
                                    180.0,
                                    linear_color_stop(card_bg.opacity(0.0), 0.0),
                                    linear_color_stop(card_bg, 1.0),
                                ))
                                .flex()
                                .flex_col()
                                .justify_end()
                                .px_3()
                                .pb_3()
                                .gap_1()
                                .child(header_row)
                                .child(meta_row),
                        );

                    out.push((*conn_id, inner.into_any_element()));
                }
                out
            };

            // M35 T4: ACTIVE SESSIONS 改名「继续工作」+ 升 Title3 视觉权重。
            // 「继续工作」是 verb-driven 引导（用户打开 aish 大概率为恢复
            // 上次状态而非配置新主机），视觉应在 Home 首位重权重 — 替代
            // 原 muted Caption divider 模式。
            let active_section_label: Option<gpui::AnyElement> = if active_cards_phase1.is_empty() {
                None
            } else {
                Some(
                    div()
                        .pb_2()
                        .typography(aish_ui::TypeRole::Title3, theme)
                        .child("继续工作")
                        .into_any_element(),
                )
            };

            // ───── Hosts grid ─────
            // M36 T6: saved 卡 vertical layout 重设计。
            // - avatar top-left → 3 行 text stack (label / connection / time + 活跃 chip)
            // - edit/delete IconButton 右下角 absolute (group_hover 显形)
            // - 删 SSH chip / chevron / 活跃数 Badge — 简化视觉信息密度
            // - 活跃 chip 改弱视觉 inline 文本（11/SEMIBOLD/success），在 time 行尾
            let cards_phase1: Vec<(HostId, gpui::AnyElement)> = app
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

                    // 该 host 是否有 active connection（不显示数量，只显 active chip）
                    let active_for_this_host = app.connections.values().any(|c| c.host_id == id);

                    // ───── avatar：三种模式（M22 沿用）─────
                    // 1. os_kind + SVG → 品牌色背景 + SVG icon
                    // 2. os_kind + 仅 Letter → 单字母 + 品牌色
                    // 3. fallback → label 首字母 + palette 色
                    let os_avatar = h.os_kind.as_deref().and_then(crate::avatar::os_avatar_for);
                    let avatar: gpui::AnyElement = match os_avatar {
                        Some(crate::avatar::OsAvatar::Svg { icon, bg }) => div()
                            .w(px(40.0))
                            .h(px(40.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(bg))
                            .rounded_xl()
                            .child(aish_ui::icon(icon).size(px(22.0)).text_color(gpui::white()))
                            .into_any_element(),
                        Some(crate::avatar::OsAvatar::Letter { letter, bg }) => div()
                            .w(px(40.0))
                            .h(px(40.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(bg))
                            .rounded_xl()
                            .text_color(gpui::white())
                            .text_size(font_size.lg)
                            .child(letter.to_string())
                            .into_any_element(),
                        None => {
                            let initial = label
                                .chars()
                                .next()
                                .unwrap_or('?')
                                .to_uppercase()
                                .to_string();
                            let avatar_bg = crate::avatar::avatar_color_for(&label);
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(avatar_bg))
                                .rounded_xl()
                                .text_color(colors.primary_foreground)
                                .text_size(font_size.lg)
                                .child(initial)
                                .into_any_element()
                        }
                    };

                    let group_name = gpui::SharedString::from(format!("host-card-row-{}", id));
                    let buttons = self
                        .host_card_buttons
                        .get(&id)
                        .expect("host_card_buttons 在 render 顶部已 ensure");
                    let edit_btn = buttons.edit.clone();
                    let delete_btn = buttons.delete.clone();

                    // time + 活跃 chip — 弱视觉文本（active 时尾追 "· ● 活跃"）
                    let time_meta = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(div().typography(aish_ui::TypeRole::Caption, theme).child(
                            match last_conn_str {
                                Some(s) => s,
                                None => "未连接".to_string(),
                            },
                        ))
                        .when(active_for_this_host, |d| {
                            d.child(div().text_color(colors.muted_foreground).child("·"))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(colors.success)
                                        .child("● 活跃"),
                                )
                        });

                    // ───── 卡片主体 vertical col ─────
                    // avatar 顶部独占 row → 3 行 text stack → actions absolute 右下
                    let body_col = div()
                        .relative()
                        .group(group_name.clone())
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_3()
                        .py_3()
                        .child(avatar)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .typography(aish_ui::TypeRole::Title3, theme)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .typography(aish_ui::TypeRole::Code, theme)
                                        .text_color(colors.secondary_foreground)
                                        .child(host_text),
                                )
                                .child(time_meta),
                        )
                        .child(
                            // edit/delete IconButton 右下角 absolute
                            // group_hover 显形（与 M22 horizontal layout 同 pattern）
                            div()
                                .absolute()
                                .bottom_2()
                                .right_2()
                                .flex()
                                .flex_row()
                                .gap_1()
                                .opacity(0.0)
                                .group_hover(group_name, |s| s.opacity(1.0))
                                .child(edit_btn)
                                .child(delete_btn),
                        );

                    (id, body_col.into_any_element())
                })
                .collect();

            // M35 T4: HOSTS 改名「保存的主机 (N)」+ 升 Title3 视觉权重，与
            // 「继续工作」section label 等级一致。右上角 「⌘K 搜索」hint
            // 作为未来 T8 CommandPalette 的 visual anchor（不挂事件）。
            let hosts_count = app.hosts.len();
            let hosts_section_label = div()
                .pb_2()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .typography(aish_ui::TypeRole::Title3, theme)
                        .child(format!("保存的主机 ({})", hosts_count)),
                )
                .child(
                    div()
                        .typography(aish_ui::TypeRole::Caption, theme)
                        .child("⌘K 搜索"),
                );

            // M35 T4: 两 section 间分隔条 — 仅当两 section 都将显示时画。
            // 视觉作用：明确「继续工作」与「保存的主机」是两类不同 task —
            // 前者是 verb-driven 恢复，后者是 noun-list 选择。
            let show_separator = !active_cards_phase1.is_empty() && !app.hosts.is_empty();
            let separator_el: Option<gpui::AnyElement> = if show_separator {
                Some(
                    div()
                        .px(theme.anatomy.page.outer_px)
                        .pb_4()
                        .child(aish_ui::Separator::horizontal())
                        .into_any_element(),
                )
            } else {
                None
            };

            // capture phase A 输出（drop borrow 前必须 own / Copy）
            (
                header.into_any_element(),
                active_section_label,
                active_cards_phase1,
                separator_el,
                cards_phase1,
                hosts_section_label.into_any_element(),
                app.hosts_load_error.clone(),
                app.hosts.is_empty(),
                colors.background,
                theme.anatomy.page.outer_px,
                theme.anatomy.page.outer_py_bottom,
                theme.anatomy.list_row.gap_spacious,
            )
        };
        // phase A end — app + theme borrow 释放

        // ───── Phase B: active cards + host cards Vec build ─────
        // M36 T3: active_cards 替代原 active_session_rows (ListRow)，inner
        // 在 Phase A 已 build 完，本段仅做 CardEntity.body() 灌入。
        let active_cards_el: Vec<gpui::AnyElement> = active_cards_phase1
            .into_iter()
            .map(|(conn_id, body_inner)| {
                let card_entity = self
                    .active_cards
                    .get(&conn_id)
                    .cloned()
                    .expect("active_cards 在 render 顶部已 ensure");
                card_entity.update(cx, |c, _| {
                    c.body(body_inner);
                });
                card_entity.into_any_element()
            })
            .collect();

        let active_section_el: Option<gpui::AnyElement> = if active_cards_el.is_empty() {
            None
        } else {
            Some(
                div()
                    .px_8()
                    .pb_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when_some(active_section_label, |d, l| d.child(l))
                    .child(
                        // M36 T3: grid 2 列布局（spec §3.2，≥ 1000px 视窗）。
                        // T9 性能实测后看是否需要响应式列数（< 700 / 700-1000）。
                        div().grid().grid_cols(2).gap_3().children(active_cards_el),
                    )
                    .into_any_element(),
            )
        };

        let cards: Vec<gpui::AnyElement> = cards_phase1
            .into_iter()
            .map(|(id, body_row)| {
                let card_entity = self
                    .host_cards
                    .get(&id)
                    .cloned()
                    .expect("host_cards 在 render 顶部已 ensure");
                card_entity.update(cx, |c, _| {
                    c.body(body_row);
                });
                div()
                    .id(gpui::SharedString::from(format!("host-card-wrap-{}", id)))
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            this.menu_host_id = Some(id);
                            this.menu_active_idx = 0;
                            let pos = ev.position;
                            this.context_menu.update(cx, |m, cx| m.open_at(pos, cx));
                            cx.notify();
                        }),
                    )
                    .child(card_entity)
                    .into_any_element()
            })
            .collect();

        // ───── Phase C: empty_hint + final layout（用 captured values）─────
        // M28 T7: load 失败 → ErrorState 优先于 empty hint
        // M28 T4: hosts 空 → EmptyState 4-slot anatomy
        let empty_hint: Option<gpui::AnyElement> = if let Some(err) = load_error {
            Some(
                aish_ui::ErrorState::new("home-hosts-load-failed")
                    .icon(aish_ui::IconName::FileQuestion)
                    .title("加载主机列表失败")
                    .description(err)
                    .action(self.retry_btn.clone())
                    .into_any_element(),
            )
        } else if hosts_is_empty {
            Some(
                aish_ui::EmptyState::new("home-no-hosts")
                    .icon(aish_ui::IconName::Inbox)
                    .title("还没有保存的连接")
                    .description("点击右上角 + 添加 host 开始")
                    .action(self.empty_add_btn.clone())
                    .into_any_element(),
            )
        } else {
            None
        };

        // ScrollPage 内部封装了 wheel + scrollbar + flex_1/min_h(0) layout。
        // caller 只需：父 flex_col + 持 ScrollHandle 字段。ContextMenu 平级
        // 挂 outer 内（不在 ScrollPage 内 — absolute backdrop 会被 scroll
        // viewport 裁切）。
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg_color)
            .child(
                aish_ui::ScrollPage::new("home-scroll")
                    .scrollbar(&self.scrollbar)
                    .flex_1()
                    .child(header_el)
                    .children(active_section_el)
                    .children(separator_el)
                    .child(
                        div()
                            .px(anatomy_outer_px)
                            .pb(anatomy_outer_py_bottom)
                            .flex()
                            .flex_col()
                            .gap(anatomy_list_gap)
                            .child(hosts_section_label_el)
                            .children(cards)
                            .children(empty_hint),
                    ),
            )
        // context_menu 不在此 mount — RootView 在 root 顶层 mount 让浮层
        // 不被下游 view（如 modal / session_picker）盖（与 tab_bar 同模式）。
    }
}

impl HomeView {
    /// 暴露 context_menu entity 让 RootView 在 root 顶层 mount。
    pub fn context_menu_entity(&self) -> Entity<aish_ui::ContextMenu> {
        self.context_menu.clone()
    }
}
