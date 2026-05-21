//! Command Palette（⌘K / Ctrl+P）— fuzzy host search 全局快速访问。
//!
//! M35 T8 MVP 范围：
//! - 全局 Ctrl+P / Cmd+P / Cmd+K 触发（RootView handle_global_key）
//! - Dialog overlay 含 TextInput + 结果列表
//! - fuzzy match host.label / host / user 三字段，subseq scorer
//! - ↑/↓ 选中，Enter 触发 open_connection + 关 palette
//! - Esc 关 palette
//!
//! 不做（v1 范围）：commands / settings 索引 / 最近用过的命令 / 跨 caller 复用
//! （仅 host search 一个 caller，YAGNI 不放 issh-ui）。
//!
//! state-driven open：state.pending_palette = true 时 sync_from_state 打开
//! Dialog；用户 Esc / 选中后 = false 关闭。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{div, prelude::*, px, Context, Entity, IntoElement, Window};
use issh_types::{HostConfig, HostId, TabId};
use issh_ui::{theme, Dialog, ListRow, TextInput, TypographyExt};

use crate::app::retain_alive_entities;
use crate::bridge::Bridge;
use crate::state::{AppState, SidebarTab, SshEvent, Tab, TabContent};

/// fuzzy match scorer（subseq match）。
///
/// 算法：query 每个 char 必须按顺序在 target 中出现（case-insensitive）。
/// 评分要素：
/// - 起始字符 match：+20 bonus（强匹配位置 like prefix）
/// - 普通 char match：+10
/// - 连续 char match：每多一个连续 +5（推动短 query 找到 substring 而非散点）
/// - 不匹配的 char：-1（轻度惩罚长 query 中的远距离 match）
///
/// 返回 `Some(score)` 表示 query 是 target 的 subseq；`None` 表示无匹配。
/// 空 query → `Some(0)`（rank 不变，全列表保留）。
pub fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let query = query.to_lowercase();
    let target = target.to_lowercase();
    let qb = query.as_bytes();
    let tb = target.as_bytes();
    let mut qi = 0usize;
    let mut score: i32 = 0;
    let mut consecutive: i32 = 0;
    for (ti, &c) in tb.iter().enumerate() {
        if qi >= qb.len() {
            break;
        }
        if c == qb[qi] {
            qi += 1;
            consecutive += 1;
            score += 10 + consecutive * 5;
            if ti == 0 {
                score += 20;
            }
        } else {
            consecutive = 0;
            score -= 1;
        }
    }
    if qi == qb.len() {
        Some(score)
    } else {
        None
    }
}

/// host 的整体 score = max(label/host/user 三字段 score)。
fn host_score(query: &str, h: &HostConfig) -> Option<i32> {
    let s_label = fuzzy_score(query, &h.label);
    let s_host = fuzzy_score(query, &h.host);
    let s_user = fuzzy_score(query, &h.user);
    [s_label, s_host, s_user].into_iter().flatten().max()
}

pub struct CommandPaletteView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    dialog: Entity<Dialog>,
    input: Entity<TextInput>,
    /// 键盘选中的行索引（0-based，相对当前过滤后的结果列表）
    selected_idx: usize,
    /// 每个 host 一个 ListRow entity（hover transition + press feedback）
    host_rows: HashMap<HostId, Entity<ListRow>>,
    /// 已 open 过的 mirror，防止每帧重 open（与 session_picker 同模式）
    is_open: bool,
}

impl CommandPaletteView {
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
            d.title("⌘K — 快速访问");
            d.width(gpui::px(560.0));
            d.on_close(move |_w, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.handle_close(cx));
                }
            });
            // ↑/↓ 移动 selected_idx，Enter pick
            d.on_key(move |ev, _w, cx| {
                if let Some(this) = weak_key.upgrade() {
                    this.update(cx, |this, cx| this.handle_dialog_key(ev, cx));
                }
            });
        });

        let input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("搜索主机（标签 / 地址 / 用户名）...");
            i
        });
        // input 文本变化 → 重置 selected_idx + cx.notify 触发重 render 排序
        let weak_input = cx.weak_entity();
        input.update(cx, |i, _| {
            i.on_change(move |_text, _w, cx| {
                if let Some(this) = weak_input.upgrade() {
                    this.update(cx, |this, cx| {
                        this.selected_idx = 0;
                        cx.notify();
                    });
                }
            });
        });

        Self {
            state,
            bridge,
            tx,
            dialog,
            input,
            selected_idx: 0,
            host_rows: HashMap::new(),
            is_open: false,
        }
    }

    fn sync_from_state(&mut self, cx: &mut Context<Self>) {
        let pending = self.state.read(cx).pending_palette;
        if pending && !self.is_open {
            self.is_open = true;
            self.selected_idx = 0;
            // 打开时清空 input（每次 ⌘K 从头开始）
            self.input.update(cx, |i, cx| {
                i.set_text("", cx);
            });
            self.dialog.update(cx, |d, cx| d.open(cx));
        } else if !pending && self.is_open {
            self.is_open = false;
            self.dialog.update(cx, |d, cx| d.close(cx));
        }
    }

    fn handle_close(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.pending_palette = false;
            cx.notify();
        });
    }

    fn handle_dialog_key(&mut self, ev: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        let results = self.filtered_hosts(cx);
        if results.is_empty() {
            return;
        }
        match ev.keystroke.key.as_str() {
            "up" => {
                self.selected_idx = if self.selected_idx == 0 {
                    results.len() - 1
                } else {
                    self.selected_idx - 1
                };
                cx.notify();
            }
            "down" => {
                self.selected_idx = (self.selected_idx + 1) % results.len();
                cx.notify();
            }
            "enter" => {
                if let Some((host_id, _)) = results.get(self.selected_idx) {
                    self.handle_pick(*host_id, cx);
                }
            }
            _ => {}
        }
    }

    /// 当前 query 过滤 + 排序的 host 列表。返回 `Vec<(HostId, score)>`。
    fn filtered_hosts(&self, cx: &mut Context<Self>) -> Vec<(HostId, i32)> {
        let query = self.input.read(cx).text();
        let app = self.state.read(cx);
        let mut scored: Vec<(HostId, i32)> = app
            .hosts
            .iter()
            .filter_map(|h| host_score(query, h).map(|s| (h.id, s)))
            .collect();
        // score desc 排序；同分按 label 字典序（稳定 + 用户可预期）
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| {
                let la = app.hosts.iter().find(|h| h.id == a.0).map(|h| &h.label);
                let lb = app.hosts.iter().find(|h| h.id == b.0).map(|h| &h.label);
                la.cmp(&lb)
            })
        });
        scored
    }

    fn handle_pick(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        // 与 home.rs::handle_card_click 同模式 — 开 connection + tab + spawn actor
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
            // 同步关 palette
            s.pending_palette = false;
            cx.notify();
            (conn, cfg)
        });

        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!(?host_id, "command_palette: host config not found");
                return;
            }
        };
        let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
        self.state.update(cx, |s, _cx| {
            s.register_session(conn_id, sender);
        });
    }
}

impl Render for CommandPaletteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_open {
            return self.dialog.clone().into_any_element();
        }

        // ── Phase 0: 取当前过滤结果 + retain row entities ──
        let results = self.filtered_hosts(cx);
        let alive: std::collections::HashSet<HostId> = results.iter().map(|(id, _)| *id).collect();
        retain_alive_entities(&mut self.host_rows, |k| alive.contains(k));

        for (id, _) in &results {
            if !self.host_rows.contains_key(id) {
                let row_id: gpui::ElementId =
                    gpui::SharedString::from(format!("palette-row-{}", id.0)).into();
                let row = cx.new(|c| {
                    let mut r = ListRow::new(row_id, c);
                    r.padding(px(12.0), px(8.0));
                    r
                });
                self.host_rows.insert(*id, row);
            }
        }

        let selected_idx = self.selected_idx.min(results.len().saturating_sub(1));

        // ── Phase A: block scope read 构造 body 内容 ──
        let body_phase1: Vec<(HostId, gpui::AnyElement, bool)> = {
            let app = self.state.read(cx);
            let t = theme(cx);
            let colors = t.colors;
            let spacing = t.spacing;

            results
                .iter()
                .enumerate()
                .map(|(idx, (id, _))| {
                    let is_kb_selected = idx == selected_idx;
                    let h = app.hosts.iter().find(|x| x.id == *id);
                    let (label, target, user) = match h {
                        Some(h) => (
                            h.label.clone(),
                            format!("{}:{}", h.host, h.port),
                            h.user.clone(),
                        ),
                        None => (String::new(), String::new(), String::new()),
                    };
                    let inner = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(spacing.px_3)
                        .child(
                            div()
                                .flex_1()
                                .typography(issh_ui::TypeRole::Body, t)
                                .child(label),
                        )
                        .child(
                            div()
                                .typography(issh_ui::TypeRole::Code, t)
                                .text_color(colors.muted_foreground)
                                .child(format!("{}@{}", user, target)),
                        )
                        .child(
                            div()
                                .typography(issh_ui::TypeRole::Caption, t)
                                .child("回车"),
                        );
                    (*id, inner.into_any_element(), is_kb_selected)
                })
                .collect()
        };

        // ── Phase B: entity.update 灌 body / selected / on_click ──
        let row_entities: Vec<Entity<ListRow>> = body_phase1
            .into_iter()
            .map(|(id, inner, is_kb_selected)| {
                let entity = self
                    .host_rows
                    .get(&id)
                    .cloned()
                    .expect("host_rows ensure 已做");
                let weak = cx.weak_entity();
                entity.update(cx, |r, _| {
                    r.body(inner)
                        .selected(is_kb_selected)
                        .on_click(move |_ev, _w, cx| {
                            if let Some(this) = weak.upgrade() {
                                this.update(cx, |this, cx| this.handle_pick(id, cx));
                            }
                        });
                });
                entity
            })
            .collect();

        // ── Phase C: 拼装 dialog body ──
        let t = theme(cx);
        let spacing = t.spacing;
        let colors = t.colors;
        let body: gpui::AnyElement = if results.is_empty() {
            // 空状态：input 已有 query 但无 match → 提示；query 空 → 提示开始输入
            let query_empty = self.input.read(cx).text().is_empty();
            let hint = if query_empty {
                "开始输入以搜索保存的主机"
            } else {
                "无匹配的主机"
            };
            div()
                .py(spacing.px_4)
                .flex()
                .items_center()
                .justify_center()
                .typography(issh_ui::TypeRole::Caption, t)
                .child(hint)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(spacing.px_1)
                .children(row_entities)
                .into_any_element()
        };

        let full_body = div()
            .flex()
            .flex_col()
            .gap(spacing.px_3)
            .child(self.input.clone())
            .child(issh_ui::Separator::horizontal())
            .child(body)
            .child(
                div()
                    .pt(spacing.px_2)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .text_color(colors.muted_foreground)
                    .typography(issh_ui::TypeRole::Caption, t)
                    .child("↑↓ 选择 · Enter 打开 · Esc 关闭")
                    .child(format!("{} 个结果", results.len())),
            );

        self.dialog.update(cx, |d, _| {
            d.body(full_body);
        });

        self.dialog.clone().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_score_empty_query_returns_zero() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn fuzzy_score_subseq_match() {
        // "abc" subseq of "aXbYc"
        let s = fuzzy_score("abc", "aXbYc");
        assert!(s.is_some());
    }

    #[test]
    fn fuzzy_score_no_match() {
        assert_eq!(fuzzy_score("xyz", "abc"), None);
    }

    #[test]
    fn fuzzy_score_case_insensitive() {
        assert!(fuzzy_score("ABC", "abc").is_some());
        assert!(fuzzy_score("abc", "ABC").is_some());
    }

    #[test]
    fn fuzzy_score_prefix_match_higher_than_middle() {
        let s_prefix = fuzzy_score("dev", "dev-server").unwrap();
        let s_middle = fuzzy_score("dev", "my-dev-box").unwrap();
        // 前缀 match 因 ti=0 bonus + 连续 + 无前置 miss penalty，应得分更高
        assert!(
            s_prefix > s_middle,
            "prefix {} should > middle {}",
            s_prefix,
            s_middle
        );
    }

    #[test]
    fn fuzzy_score_consecutive_beats_scattered() {
        let s_consec = fuzzy_score("abc", "abc").unwrap();
        let s_scattered = fuzzy_score("abc", "aXbYcZ").unwrap();
        assert!(s_consec > s_scattered);
    }

    #[test]
    fn fuzzy_score_partial_query_unmatched_returns_none() {
        // query "ace" — target "abcd" 有 a/c 但无 e
        assert_eq!(fuzzy_score("ace", "abcd"), None);
    }
}
