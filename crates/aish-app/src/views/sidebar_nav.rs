//! SidebarNav：左侧导航 — 双模式（折叠 64px / 展开 220px）。
//!
//! 历史：
//! - M13：用 aish_ui::NavItem.vertical()
//! - M34：NavItem 升 stateful Entity（hover transition + press feedback）
//! - M35 T6：SIDEBAR_WIDTH 48 → 64，加 Caption label 让导航自明
//! - M35 T9 v2：加展开 220px 双模式 — 含顶部 logo / toggle / 「最近连接」list
//!
//! ## Borrow path 设计（先 trace 再写，防 T9 v1 borrow 冲突 revert 再现）
//!
//! render 内严格 5 phase 拆分，每个 phase 内**唯一**借用类型：
//!
//! ```text
//! Phase A   read app borrow（block scope）  → 取 current / expanded / recent_snapshot owned
//! Phase A.5 mut cx 借（retain + ensure entities + 构 make_icon owned）
//! Phase B   read theme borrow（block scope）→ build rows_phase1 含 AnyElement owned
//! Phase C   mut cx 借（entity.update 灌 body / NavItem icon / label / active）
//! Phase D   read theme borrow（layout tokens）→ assemble final el
//! ```
//!
//! 关键：theme borrow 与 mut cx 借**永不重叠**。
//! 同 home.rs / session_picker.rs 已落地的 phase 模式。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use aish_types::{HostId, TabId};
use aish_ui::{icon, theme, IconName, ListRow, NavItem, TypographyExt};
use gpui::{div, prelude::*, px, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::app::retain_alive_entities;
use crate::bridge::Bridge;
use crate::state::{humanize_last_connected, AppState, SidebarTab, SshEvent, Tab, TabContent};

// shadcn 数值参考：折叠 48 (3rem) / 展开 240 (15rem, 略短于 shadcn 16rem 给
// aish 主区留更多空间)
const SIDEBAR_COLLAPSED_WIDTH: f32 = 48.0;
const SIDEBAR_EXPANDED_WIDTH: f32 = 240.0;
const RECENT_LIST_MAX: usize = 5;

pub struct SidebarNavView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    home_item: Entity<NavItem>,
    terminal_item: Entity<NavItem>,
    settings_item: Entity<NavItem>,
    /// 「最近连接」list 行 entity（仅展开模式渲染，按 HostId 索引 + retain）。
    recent_rows: HashMap<HostId, Entity<ListRow>>,
}

impl SidebarNavView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();

        let weak_home = cx.weak_entity();
        let home_item = cx.new(|cx| {
            let mut n = NavItem::new("sidebar-nav-home", cx);
            n.on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_home.upgrade() {
                    this.update(cx, |this, cx| this.handle_click(SidebarTab::Home, cx));
                }
            });
            n
        });
        let weak_term = cx.weak_entity();
        let terminal_item = cx.new(|cx| {
            let mut n = NavItem::new("sidebar-nav-terminal", cx);
            n.on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_term.upgrade() {
                    this.update(cx, |this, cx| this.handle_click(SidebarTab::Terminal, cx));
                }
            });
            n
        });
        let weak_settings = cx.weak_entity();
        let settings_item = cx.new(|cx| {
            let mut n = NavItem::new("sidebar-nav-settings", cx);
            n.on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_settings.upgrade() {
                    this.update(cx, |this, cx| this.handle_click(SidebarTab::Settings, cx));
                }
            });
            n
        });

        Self {
            state,
            bridge,
            tx,
            home_item,
            terminal_item,
            settings_item,
            recent_rows: HashMap::new(),
        }
    }

    fn handle_click(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.sidebar = tab;
            cx.notify();
        });
    }

    /// 切换 sidebar_expanded 状态 + 持久化到 app_state.toml。
    fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        let new_val = !self.state.read(cx).sidebar_expanded;
        self.state.update(cx, |s, cx| {
            s.sidebar_expanded = new_val;
            cx.notify();
        });
        let mut snapshot = crate::app_state_file::load_app_state();
        snapshot.sidebar_expanded = Some(new_val);
        crate::app_state_file::save_app_state(&snapshot);
    }

    /// 「最近连接」行 click → 同 home::handle_card_click 模式：
    /// open_connection + 切 Terminal tab + spawn actor。
    fn handle_recent_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        let (conn_id, config) = self.state.update(cx, |s, cx| {
            let conn = s.open_connection(host_id);
            let cfg = s.hosts.iter().find(|h| h.id == host_id).cloned();
            let default_title = cfg
                .as_ref()
                .map(|c| c.label.clone())
                .unwrap_or_else(|| "新连接".into());
            let tab = Tab {
                id: TabId::new(),
                content: TabContent::Connection(conn),
                title: default_title,
                title_locked: false,
            };
            s.tabs.push(tab);
            s.selected_tab = Some(s.tabs.last().unwrap().id);
            s.sidebar = SidebarTab::Terminal;
            s.last_connected.insert(host_id, SystemTime::now());
            let snapshot =
                crate::app_state_file::load_app_state().merge_last_connected(&s.last_connected);
            crate::app_state_file::save_app_state(&snapshot);
            cx.notify();
            (conn, cfg)
        });
        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!(?host_id, "sidebar: host config not found");
                return;
            }
        };
        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |s, _cx| {
            s.register_session(conn_id, sender);
        });
    }
}

impl Render for SidebarNavView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ── Phase A：read app borrow，block scope → 取 owned snapshot ──
        let (current, expanded, recent_snapshot) = {
            let app = self.state.read(cx);
            let current = app.sidebar;
            let expanded = app.sidebar_expanded;
            let mut pairs: Vec<(HostId, SystemTime, String)> = app
                .last_connected
                .iter()
                .filter_map(|(id, t)| {
                    app.hosts
                        .iter()
                        .find(|h| h.id == *id)
                        .map(|h| (*id, *t, h.label.clone()))
                })
                .collect();
            pairs.sort_by_key(|p| std::cmp::Reverse(p.1));
            pairs.truncate(RECENT_LIST_MAX);
            (current, expanded, pairs)
        };
        // app borrow dropped ✓

        // ── Phase A.5：mut cx 借 — retain + ensure entities ──
        let alive: std::collections::HashSet<HostId> =
            recent_snapshot.iter().map(|(id, _, _)| *id).collect();
        retain_alive_entities(&mut self.recent_rows, |k| alive.contains(k));

        for (id, _, _) in &recent_snapshot {
            if !self.recent_rows.contains_key(id) {
                let row_id: gpui::ElementId =
                    gpui::SharedString::from(format!("sidebar-recent-{}", id.0)).into();
                let row = cx.new(|c| {
                    let mut r = ListRow::new(row_id, c);
                    r.padding(px(10.0), px(6.0)).radius(px(6.0));
                    r
                });
                self.recent_rows.insert(*id, row);
            }
        }

        // Icon helper：SVG IconName。GPUI svg() 颜色不从父 text_color 自动
        // 继承 — 必须显式 .text_color() 否则 stroke 不可见。
        // M35.1 D5: active 时 icon 切 primary 色对照 inset glow border / bg
        // 的紫色 tint — 让 active 整体视觉一致（fill / border / icon 三处
        // 同色调），inactive 保持 muted。
        let muted_icon = theme(cx).colors.muted_foreground;
        let primary_icon = theme(cx).colors.primary;
        // shadcn 参考 size-4 = 16px
        let make_icon = move |name: IconName, active: bool| {
            let color = if active { primary_icon } else { muted_icon };
            icon(name).size(px(16.0)).text_color(color)
        };

        // ── Phase B：read theme borrow，block scope → build inner body AnyElement ──
        let rows_phase1: Vec<(HostId, gpui::AnyElement)> = {
            let t = theme(cx);
            recent_snapshot
                .iter()
                .map(|(id, time, label)| {
                    let humanized = humanize_last_connected(*time);
                    let inner = div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Body, t)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(label.clone()),
                        )
                        .child(
                            div()
                                .typography(aish_ui::TypeRole::Caption, t)
                                .child(humanized),
                        );
                    (*id, inner.into_any_element())
                })
                .collect()
        };
        // theme borrow dropped ✓

        // ── Phase C：mut cx 借 — entity.update 灌 body / NavItem orientation/label/active ──
        let recent_row_entities: Vec<Entity<ListRow>> = rows_phase1
            .into_iter()
            .map(|(id, inner)| {
                let entity = self
                    .recent_rows
                    .get(&id)
                    .cloned()
                    .expect("recent_rows ensure 已做");
                let weak = cx.weak_entity();
                entity.update(cx, |r, _| {
                    r.body(inner).on_click(move |_ev, _w, cx| {
                        if let Some(this) = weak.upgrade() {
                            this.update(cx, |this, cx| this.handle_recent_click(id, cx));
                        }
                    });
                });
                entity
            })
            .collect();

        // NavItem entity.update — orientation 由 expanded 决定
        // M35.1 D5: icon color 随 active 切（primary / muted），三处统一
        let home_active = current == SidebarTab::Home;
        let term_active = current == SidebarTab::Terminal;
        let settings_active = current == SidebarTab::Settings;
        let home_icon = make_icon(IconName::Home, home_active);
        let term_icon = make_icon(IconName::Terminal, term_active);
        let settings_icon = make_icon(IconName::Settings, settings_active);
        if expanded {
            self.home_item.update(cx, |n, _| {
                n.icon(home_icon)
                    .label("Home")
                    .horizontal()
                    .active(current == SidebarTab::Home);
            });
            self.terminal_item.update(cx, |n, _| {
                n.icon(term_icon)
                    .label("Terminal")
                    .horizontal()
                    .active(current == SidebarTab::Terminal);
            });
            self.settings_item.update(cx, |n, _| {
                n.icon(settings_icon)
                    .label("Settings")
                    .horizontal()
                    .active(current == SidebarTab::Settings);
            });
        } else {
            // 折叠模式：no_label() icon-only + horizontal rounded card 视觉
            self.home_item.update(cx, |n, _| {
                n.icon(home_icon)
                    .no_label()
                    .horizontal()
                    .active(current == SidebarTab::Home);
            });
            self.terminal_item.update(cx, |n, _| {
                n.icon(term_icon)
                    .no_label()
                    .horizontal()
                    .active(current == SidebarTab::Terminal);
            });
            self.settings_item.update(cx, |n, _| {
                n.icon(settings_icon)
                    .no_label()
                    .horizontal()
                    .active(current == SidebarTab::Settings);
            });
        }

        // ── Phase D：re-borrow theme for layout tokens（read borrow，本 phase 不调 mut cx）──
        let t = theme(cx);
        let colors = t.colors;
        let spacing = t.spacing;

        // toggle 按钮 icon — chevron-left (展开 → 收起) / chevron-right (折叠 → 展开)
        let toggle_icon_name = if expanded {
            IconName::ChevronLeft
        } else {
            IconName::ChevronRight
        };

        // toggle_btn helper — 两种模式共用，sidebar 底部 settings 下方。
        // 位置一致让用户切折叠 / 展开后 toggle 不\"跳位\"。
        let toggle_btn = div()
            .w_full()
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .rounded(px(6.0))
            .hover(|s| s.bg(colors.secondary_hover))
            .child(
                icon(toggle_icon_name)
                    .size(px(12.0))
                    .text_color(colors.muted_foreground),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.toggle_expanded(cx);
                }),
            );

        // settings + toggle 同区，两种模式共用 layout
        // M35.1 D2: item 间距 gap_1(4) → gap_2(8) 对照 shadcn sidebar
        let bottom_section = div()
            .px(spacing.px_2)
            .pb(spacing.px_2)
            .flex()
            .flex_col()
            .gap(spacing.px_2)
            .child(self.settings_item.clone())
            .child(toggle_btn);

        // nav section — 两种模式都用 horizontal NavItem（rounded card 视觉），
        // 折叠模式 NavItem.no_label() icon-only + justify_center
        // M35.1 D2: item 间距 gap_1(4) → gap_2(8)
        let nav_section = div()
            .px(spacing.px_2)
            .pt(spacing.px_2)
            .flex()
            .flex_col()
            .gap(spacing.px_2)
            .child(self.home_item.clone())
            .child(self.terminal_item.clone());

        let width = if expanded {
            SIDEBAR_EXPANDED_WIDTH
        } else {
            SIDEBAR_COLLAPSED_WIDTH
        };

        // 「最近连接」list（仅展开模式显示 — 折叠 64px 无空间放 host label）
        let recent_section: Option<gpui::AnyElement> =
            if expanded && !recent_row_entities.is_empty() {
                Some(
                    div()
                        .px(spacing.px_3)
                        .pt(spacing.px_4)
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            // 11px MEDIUM secondary_fg — 显式 text_color 防 typography
                            // inheritance 异常。secondary_fg 替代 muted_fg 让 11px 小字
                            // 在 dark bg 上对比度 9:1 完全过 WCAG AA（之前 muted 在
                            // secondary bg 上仅 3.7:1 不过 AA 正文标准）。
                            div()
                                .pl(spacing.px_2)
                                .pb(spacing.px_1)
                                .text_size(px(11.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(colors.secondary_foreground)
                                .child("最近连接"),
                        )
                        .children(recent_row_entities)
                        .into_any_element(),
                )
            } else {
                None
            };

        div()
            .w(px(width))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .border_r_1()
            .border_color(colors.border)
            .child(nav_section)
            .children(recent_section)
            .child(div().flex_1())
            .child(bottom_section)
            .into_any_element()
    }
}
