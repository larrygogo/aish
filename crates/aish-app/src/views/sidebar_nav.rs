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
use aish_ui::{theme, ListRow, NavItem, Separator, TypographyExt};
use gpui::{div, prelude::*, px, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::app::retain_alive_entities;
use crate::bridge::Bridge;
use crate::state::{humanize_last_connected, AppState, SidebarTab, SshEvent, Tab, TabContent};
use crate::terminal::font::FONT_NAME;

const SIDEBAR_COLLAPSED_WIDTH: f32 = 64.0;
const SIDEBAR_EXPANDED_WIDTH: f32 = 220.0;
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

        // Icon helper（不持 cx 借用）— 同 fix 2bdb482 的 20×20 wrapper 居中
        let make_icon = |ch: &'static str| {
            div()
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .child(div().font_family(FONT_NAME).text_size(px(16.0)).child(ch))
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
                        .gap(px(2.0))
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
        let home_icon = make_icon("\u{f015}");
        let term_icon = make_icon("\u{f120}");
        let settings_icon = make_icon("\u{f013}");
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
            self.home_item.update(cx, |n, _| {
                n.icon(home_icon)
                    .label("Home")
                    .vertical()
                    .active(current == SidebarTab::Home);
            });
            self.terminal_item.update(cx, |n, _| {
                n.icon(term_icon)
                    .label("Terminal")
                    .vertical()
                    .active(current == SidebarTab::Terminal);
            });
            self.settings_item.update(cx, |n, _| {
                n.icon(settings_icon)
                    .label("Settings")
                    .vertical()
                    .active(current == SidebarTab::Settings);
            });
        }

        // ── Phase D：re-borrow theme for layout tokens（read borrow，本 phase 不调 mut cx）──
        let t = theme(cx);
        let colors = t.colors;
        let spacing = t.spacing;

        // toggle 按钮 glyph — chevron-left (展开 → 收起) / chevron-right (折叠 → 展开)
        let toggle_glyph = if expanded { "\u{f053}" } else { "\u{f054}" };

        if !expanded {
            // ── 折叠模式 64px ──
            let toggle_btn = div()
                .w_full()
                .py(px(8.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(colors.muted_foreground)
                .child(
                    div()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .font_family(FONT_NAME)
                                .text_size(px(12.0))
                                .child(toggle_glyph),
                        ),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                        this.toggle_expanded(cx);
                    }),
                );

            div()
                .w(px(SIDEBAR_COLLAPSED_WIDTH))
                .h_full()
                .flex()
                .flex_col()
                .bg(colors.background)
                .border_r_1()
                .border_color(colors.border)
                .child(toggle_btn)
                .child(self.home_item.clone())
                .child(self.terminal_item.clone())
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_end()
                        .child(self.settings_item.clone()),
                )
                .into_any_element()
        } else {
            // ── 展开模式 220px ──

            // 1. header：logo + name + toggle 按钮
            let header = div()
                .w_full()
                .px(spacing.px_3)
                .py(spacing.px_3)
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(gpui::img("aish-icon.png").w(px(22.0)).h(px(22.0)))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(colors.foreground)
                                .child("aish"),
                        ),
                )
                .child(
                    div()
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(colors.muted_foreground)
                        .rounded(px(6.0))
                        .hover(|s| s.bg(colors.secondary_hover))
                        .child(
                            div()
                                .font_family(FONT_NAME)
                                .text_size(px(12.0))
                                .child(toggle_glyph),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                                this.toggle_expanded(cx);
                            }),
                        ),
                );

            // 2. nav 3 items
            let nav_section = div()
                .px(spacing.px_2)
                .pt(spacing.px_1)
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(self.home_item.clone())
                .child(self.terminal_item.clone());

            // 3. 「最近连接」list（recent_row_entities 来自 Phase C）
            let recent_section: Option<gpui::AnyElement> = if recent_row_entities.is_empty() {
                None
            } else {
                Some(
                    div()
                        .px(spacing.px_3)
                        .pt(spacing.px_3)
                        .flex()
                        .flex_col()
                        .gap(spacing.px_1)
                        .child(
                            div()
                                .pb(spacing.px_1)
                                .typography(aish_ui::TypeRole::Caption, t)
                                .child("最近连接"),
                        )
                        .children(recent_row_entities)
                        .into_any_element(),
                )
            };

            // 4. settings 推到底部
            let settings_section = div()
                .px(spacing.px_2)
                .pb(spacing.px_2)
                .child(self.settings_item.clone());

            div()
                .w(px(SIDEBAR_EXPANDED_WIDTH))
                .h_full()
                .flex()
                .flex_col()
                .bg(colors.background)
                .border_r_1()
                .border_color(colors.border)
                .child(header)
                .child(Separator::horizontal())
                .child(nav_section)
                .children(recent_section)
                .child(div().flex_1())
                .child(settings_section)
                .into_any_element()
        }
    }
}
