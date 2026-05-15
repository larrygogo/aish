//! HomeView：4-tab 架构的 Home tab（M4a 信息架构）。
//!
//! 包含：Quick Actions（+ 添加 host）、Active Sessions（活跃连接列表）、
//! Hosts grid（host 卡片网格，复用 default_page.rs 原有逻辑）。

use std::sync::Arc;
use std::time::SystemTime;

use aish_types::{ConnectionId, HostId};
use aish_ui::TypographyExt;
use gpui::{
    div, prelude::*, px, rgb, Context, Entity, KeyDownEvent, MouseButton, MouseDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::state::{
    humanize_last_connected, AppState, HostFormDraft, HostFormState, SidebarTab, SshEvent, Tab,
    TabContent,
};

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
        Self {
            state,
            bridge,
            tx,
            context_menu,
            menu_host_id: None,
            menu_active_idx: 0,
            scrollbar: aish_ui::ScrollbarHandle::new(),
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
        let app = self.state.read(cx);
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        // ───── Quick Actions 顶部栏 ─────
        // 顶部主 CTA → primary（绿色突出），与 Settings 等次要按钮区分
        let add_btn = aish_ui::Button::new("home-add-host-btn")
            .label("+ 添加 host")
            .primary()
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
                // M26 T2: page title 用 Title1 (20/600/fg) 替代 xl size-only
                div()
                    .typography(aish_ui::TypeRole::Title1, theme)
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
                        // M26: active session row → label Body + time Caption
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Body, theme)
                                .child(label),
                        )
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Caption, theme)
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
                        // M26: ACTIVE SESSIONS section divider → Caption (与 HOSTS 同模式)
                        div()
                            .pb_2()
                            .typography(aish_ui::TypeRole::Caption, theme)
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

                // ───── 左侧 avatar：三种模式 ─────
                // 1. os_kind 已探测且 simpleicons SVG 已内置 → SVG + 品牌色
                //    (ubuntu/debian/arch/alpine/centos/fedora/redhat 7 个)
                // 2. os_kind 已探测但仅品牌色支持 → 单字母 + 品牌色
                //    (rocky/mint/manjaro/nixos/gentoo/opensuse/raspbian/elementary)
                // 3. os_kind 未探测 / 完全未识别 → fallback host label 首字母 + palette 色
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

                // ───── 右侧 chevron ─────
                let chevron = div()
                    .text_color(colors.muted_foreground)
                    .text_size(font_size.lg)
                    .child("›");

                // ───── 编辑 / 删除按钮（hover 才显形）─────
                // group/group_hover：body_row 标记 `.group(g)`，actions 子树
                // 默认 opacity(0)，body_row hover 时 actions opacity(1)。
                // 视觉上 default 仅 chevron，hover 多出 ✏ ⌫，chevron 位置不变
                // （actions 仍占 flex layout 空间，只是透明）。
                let group_name = gpui::SharedString::from(format!("host-card-row-{}", id));
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

                // delete 也走 ghost：destructive 红色 X 始终可见过于扎眼，
                // hover 才显形已经达到"操作可发现"目的，配色不需要再加 destructive
                let delete_btn = aish_ui::IconButton::new(
                    gpui::SharedString::from(format!("host-delete-{}", id)),
                    aish_ui::IconName::X,
                )
                .small()
                .ghost()
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
                    .group_hover(group_name.clone(), |s| s.opacity(1.0))
                    .child(edit_btn)
                    .child(delete_btn);

                // ───── 卡片主体 row ─────
                // M13 简化：放弃 absolute hover overlay（GPUI/Taffy absolute 子元素
                // 的 inset 在 flex container 内行为与 CSS 不一致，定位飘移），改为
                // actions inline 但透明，body_row 标记 group → actions
                // group_hover 显形。
                //
                // M27 anatomy：py 16 → 12，host card 更紧凑（Linear/Warp 风
                // dev tool 高密度），avatar 40 + 3 行 (14/13/12) = ~52 + py 24
                // = ~76px row 高，比之前 ~84px 略紧。
                let body_row = div()
                    .group(group_name)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
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
                                        // M26 T4: host label 用 Title3 (14/600/fg)
                                        // 之前 xl(18) + 手工 SEMIBOLD 偏大抢眼；
                                        // Title3 让 label 视觉略强于 body 但不超过
                                        // page title (Title1 20)，hierarchy 清晰
                                        div()
                                            .typography(aish_ui::TypeRole::Title3, theme)
                                            .child(label),
                                    )
                                    .child(chip)
                                    .children(active_chip),
                            )
                            .child(
                                // M26 T4: host_text user@addr:port → Body
                                div()
                                    .typography(aish_ui::TypeRole::Body, theme)
                                    .text_color(colors.secondary_foreground)
                                    .child(host_text),
                            )
                            // M26 T5: 上次连接 / 未连接 meta → Caption
                            // (12/400/muted) 替代 hardcoded px(11)
                            .child(
                                div()
                                    .typography(aish_ui::TypeRole::Caption, theme)
                                    .child(match last_conn_str {
                                        Some(s) => format!("上次连接 {}", s),
                                        None => "未连接".to_string(),
                                    }),
                            ),
                    )
                    .child(actions)
                    .child(chevron);

                // ───── 整张卡片 - 用 aish_ui::Card ─────
                // 外包 stateful wrapper div 加 right-click → context menu。
                // Card 自己已 stateful，wrapper id 与 Card id 不同避免冲突。
                //
                // M27: .no_padding() — body_row 已 px_4 py_3 padded（avatar +
                // label/host_text/last_conn 3 行），Card 默认内置 padding 会
                // 双重叠加。
                let card =
                    aish_ui::Card::new(gpui::SharedString::from(format!("host-card-{}", id)))
                        .no_padding()
                        .body(body_row)
                        .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_card_click(id, cx);
                        }));
                div()
                    .id(gpui::SharedString::from(format!("host-card-wrap-{}", id)))
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            this.menu_host_id = Some(id);
                            this.menu_active_idx = 0; // 重置键盘选中态
                            let pos = ev.position;
                            this.context_menu.update(cx, |m, cx| m.open_at(pos, cx));
                            cx.notify();
                        }),
                    )
                    .child(card)
                    .into_any_element()
            })
            .collect();

        // M28 T7: load 失败 → ErrorState + 重试 button；优先于 empty
        // hint，因为这是真错误不是空状态。
        let load_error = app.hosts_load_error.clone();
        let empty_hint = if let Some(err) = load_error {
            let retry_btn = aish_ui::Button::new("home-hosts-load-retry")
                .primary()
                .label("重试")
                .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_retry_load_hosts(cx);
                }));
            Some(
                aish_ui::ErrorState::new("home-hosts-load-failed")
                    .icon(aish_ui::IconName::FileQuestion)
                    .title("加载主机列表失败")
                    .description(err)
                    .action(retry_btn)
                    .into_any_element(),
            )
        }
        // M28 T4: hosts 空 → EmptyState 4-slot anatomy
        else if app.hosts.is_empty() {
            let add_btn = aish_ui::Button::new("home-empty-add-host")
                .primary()
                .label("+ 添加 host")
                .on_click(
                    cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_add_click(cx)),
                );
            Some(
                aish_ui::EmptyState::new("home-no-hosts")
                    .icon(aish_ui::IconName::Inbox)
                    .title("还没有保存的连接")
                    .description("点击右上角 + 添加 host 开始")
                    .action(add_btn)
                    .into_any_element(),
            )
        } else {
            None
        };

        // M26 T3: HOSTS 是 list-section divider label（Stripe/Linear 风），
        // 用 Caption (12/400/muted) — 之前 xs (10px) 偏小不清晰
        let hosts_section_label = div()
            .pb_2()
            .typography(aish_ui::TypeRole::Caption, theme)
            .child("HOSTS");

        // ScrollPage 内部封装了 wheel + scrollbar + flex_1/min_h(0) layout。
        // caller 只需：父 flex_col + 持 ScrollHandle 字段。ContextMenu 平级
        // 挂 outer 内（不在 ScrollPage 内 — absolute backdrop 会被 scroll
        // viewport 裁切）。
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .child(
                aish_ui::ScrollPage::new("home-scroll")
                    .scrollbar(&self.scrollbar)
                    .flex_1()
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
