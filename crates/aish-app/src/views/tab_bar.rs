//! 顶部 Tab 栏：显示所有 tabs，点击切换，× 按钮关闭，末尾 + 按钮新建默认页。
//!
//! 关闭 tab 时若该 tab 引用了 connection，发 `SessionCommand::Disconnect` 并
//! `state.remove_connection`，让 actor 优雅退出。
//!
//! 双击 tab 标题进入 inline 重命名模式：
//! - 字母 / 数字 / 标点 → 追加到 buffer
//! - Backspace → 删一个字符
//! - Enter → commit
//! - Escape → 放弃

use std::sync::Arc;

use aish_types::TabId;
use gpui::{
    div, point, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent,
    MouseDownEvent, ScrollHandle, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TabContent};

pub struct TabBarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    /// 当前正在 inline 编辑标题的 tab。`None` = 无编辑中。
    editing_tab: Option<TabId>,
    /// 编辑中的 buffer。提交时写回 tab.title。
    edit_buffer: String,
    /// 编辑模式下接收键盘的 focus handle。
    focus_handle: FocusHandle,
    /// tabs 横向滚动 handle。绑定到 scroll 容器后可读 offset / max_offset 决定
    /// 是否显示左右 < > 箭头，并通过 set_offset 编程式滚动。
    scroll_handle: ScrollHandle,
}

impl TabBarView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self {
            state,
            bridge,
            tx,
            editing_tab: None,
            edit_buffer: String::new(),
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// 左箭头 click：向左滚 150px（约 1 个 tab 宽度）。GPUI scroll offset.x 是
    /// **负数**（offset 0 = 未滚动；越负向左滚得越多）。set 后 clamp 到 0 不
    /// 越过原点。
    fn handle_scroll_left(&mut self, cx: &mut Context<Self>) {
        let cur = self.scroll_handle.offset();
        let new_x = (cur.x + px(150.0)).min(px(0.0));
        self.scroll_handle.set_offset(point(new_x, cur.y));
        cx.notify();
    }

    /// 右箭头 click：向右滚 150px。clamp 到 max_offset.x（负数，越小越向左）。
    fn handle_scroll_right(&mut self, cx: &mut Context<Self>) {
        let cur = self.scroll_handle.offset();
        let max = self.scroll_handle.max_offset();
        let new_x = (cur.x - px(150.0)).max(max.x);
        self.scroll_handle.set_offset(point(new_x, cur.y));
        cx.notify();
    }

    /// 处理 tab 标题点击。click_count == 2 进入 rename 模式，否则只切换。
    fn handle_tab_click(
        &mut self,
        id: TabId,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if click_count >= 2 {
            // 进 inline 重命名
            let current_title = self
                .state
                .read(cx)
                .tabs
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.title.clone())
                .unwrap_or_default();
            self.editing_tab = Some(id);
            self.edit_buffer = current_title;
            self.focus_handle.focus(window, cx);
            cx.notify();
        } else {
            self.state.update(cx, |s, cx| {
                s.select_tab(id);
                cx.notify();
            });
        }
    }

    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.editing_tab.take() {
            let new_title = std::mem::take(&mut self.edit_buffer);
            // 空标题不接受，回退到一个占位字符串
            let final_title = if new_title.trim().is_empty() {
                "新连接".into()
            } else {
                new_title
            };
            // 用户手动改名 → rename_tab_locked 同时锁定 title_locked=true，
            // 之后 OSC 0/1/2 title event 不再覆盖，保留用户命名
            self.state.update(cx, |s, cx| {
                s.rename_tab_locked(id, final_title);
                cx.notify();
            });
        }
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.editing_tab = None;
        self.edit_buffer.clear();
        cx.notify();
    }

    fn handle_edit_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.editing_tab.is_none() {
            return;
        }
        let key = ev.keystroke.key.as_str();
        match key.to_lowercase().as_str() {
            "enter" => {
                self.commit_rename(cx);
                return;
            }
            "escape" | "esc" => {
                self.cancel_rename(cx);
                return;
            }
            "backspace" => {
                self.edit_buffer.pop();
                cx.notify();
                return;
            }
            _ => {}
        }
        // 普通可打印字符：用 keystroke.key_char 拿真实字符（处理 shift / unicode）
        if let Some(ref s) = ev.keystroke.key_char {
            // 过滤控制字符
            for c in s.chars() {
                if !c.is_control() {
                    self.edit_buffer.push(c);
                }
            }
            cx.notify();
        } else if key.len() == 1 {
            // 兜底：单字符 key
            self.edit_buffer.push_str(key);
            cx.notify();
        }
    }

    fn handle_close(&mut self, id: TabId, cx: &mut Context<Self>) {
        // 1. 拿到 tab content（若是 connection 需要发 Disconnect）
        let content = self
            .state
            .read(cx)
            .tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.content.clone());
        if let Some(TabContent::Connection(conn)) = content {
            // 给 actor 发 Disconnect（fire-and-forget；actor 自然退出后 sessions 也会清）
            if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                self.bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Disconnect).await;
                });
            }
            // 清 per-conn 状态 + 把所有引用它的 tab 转回 Default
            self.state.update(cx, |s, cx| {
                s.remove_connection(conn);
                cx.notify();
            });
        }
        // 2. 移除 tab 本身
        self.state.update(cx, |s, cx| {
            s.close_tab(id);
            cx.notify();
        });
        // 关掉的 tab 如果正在被编辑，退出编辑
        if self.editing_tab == Some(id) {
            self.editing_tab = None;
            self.edit_buffer.clear();
        }
    }

    fn handle_new_tab(&mut self, cx: &mut Context<Self>) {
        // M4a：+ 按钮切回 Home，让用户从 Home 选 host 开始新连接
        self.state.update(cx, |s, cx| {
            s.sidebar = crate::state::SidebarTab::Home;
            cx.notify();
        });
    }
}

impl Focusable for TabBarView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TabBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let selected = app.selected_tab;
        let editing_tab = self.editing_tab;
        let edit_buffer = self.edit_buffer.clone();

        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        let tab_items: Vec<_> = app
            .tabs
            .iter()
            .map(|t| {
                let id = t.id;
                let title = t.title.clone();
                let is_selected = selected == Some(id);
                let is_connection = matches!(t.content, TabContent::Connection(_));
                let is_editing = editing_tab == Some(id);
                // connection tab 是否还有活跃 actor（actor 退出后 sessions 里
                // 没了 → false）。用于绿/灰点 + 标题色的"在线/已断"指示。
                let is_alive = match t.content {
                    TabContent::Connection(c) => app.is_session_active(c),
                    _ => false,
                };

                // 关闭按钮（始终可见）
                let close_btn = aish_ui::IconButton::new(
                    gpui::SharedString::from(format!("tab-close-{}", id)),
                    aish_ui::IconName::X,
                )
                .small()
                .ghost()
                .on_click(cx.listener(
                    move |this, _ev: &MouseDownEvent, _w, cx| {
                        // 拦住事件不冒泡到外层 tab listener 触发 select_tab
                        cx.stop_propagation();
                        this.handle_close(id, cx);
                    },
                ));

                // 连接 tab：活跃 = 绿点，已断 = 灰点；默认页 tab 不带前缀
                let prefix: gpui::AnyElement = if is_connection {
                    let dot_color = if is_alive {
                        colors.success
                    } else {
                        colors.muted_foreground
                    };
                    div()
                        .text_color(dot_color)
                        .text_size(font_size.xs)
                        .child("●")
                        .into_any_element()
                } else {
                    div().child("").into_any_element()
                };

                // 标题部分：编辑中用蓝色边框替代光标 `|`
                let title_el: gpui::AnyElement = if is_editing {
                    div()
                        .text_color(colors.foreground)
                        .border_1()
                        .border_color(colors.ring)
                        .rounded_md()
                        .px_1p5()
                        .child(edit_buffer.clone())
                        .into_any_element()
                } else {
                    // 已断的 connection tab：标题用 muted 弱化
                    let title_color = if is_connection && !is_alive {
                        colors.muted_foreground
                    } else if is_selected {
                        colors.foreground
                    } else {
                        colors.secondary_foreground
                    };
                    div()
                        .text_color(title_color)
                        .child(title)
                        .into_any_element()
                };

                // suffix: SSH chip（connection tab 专属）+ 关闭按钮
                let suffix = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .when(is_connection, |d| {
                        d.child(aish_ui::Badge::new("SSH").primary())
                    })
                    .child(close_btn)
                    .into_any_element();

                aish_ui::TabItem::new(gpui::SharedString::from(format!("tab-{}", id)))
                    .prefix(prefix)
                    .title(title_el)
                    .suffix(suffix)
                    .active(is_selected)
                    .on_click(cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                        this.handle_tab_click(id, ev.click_count, w, cx);
                    }))
            })
            .collect();

        // 末尾 + 按钮新建默认页：Chrome 风格 mini-tab 外观
        // - 全高 40px 与 TabItem 一致，与 tab 同一 baseline 不悬浮
        // - 宽 40px 方形，比 tab 窄，但与 tab 视觉同源
        // - bg(card) 与 tab bar 同色（idle 时几乎隐形，只露 + icon）
        // - hover bg secondary_hover、active bg secondary_active，
        //   与 TabItem hover/active 完全一致的视觉反馈
        // - 不用 IconButton 包装：IconButton 自带 rounded + 固定 padding，
        //   与"和 tab 同 baseline 的方形区域"语义冲突；直接 div 更干净
        let hover_bg = colors.secondary_hover;
        let active_bg = colors.secondary_active;
        let plus_btn = div()
            .id("tab-new")
            .h(px(40.0))
            .w(px(40.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(colors.card)
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_new_tab(cx)),
            )
            .child(
                aish_ui::icon(aish_ui::IconName::Plus)
                    .size(px(16.0))
                    .text_color(colors.muted_foreground),
            );

        // 外层布局：[ < 箭头?] [ tabs 横向滚动容器 ] [ > 箭头?] [ + 按钮固定 ]
        //
        // 滚动箭头显示条件：
        // - show_left：offset.x < 0（已向右滚过，可以回左）
        // - show_right：tabs.len() >= 2 时**总是显示**右箭头入口
        //   设计取舍：基于 window viewport 的"是否溢出"精算屡屡偏差
        //   （TabItem 实际宽度 = max-w + suffix + 字号 + 渲染抖动，无法
        //   精确预测），用户截图 3 tab 实际溢出但估算公式没触发的情形
        //   反复出现。直接 fall back 到"有 2+ 个 tab 就给入口"，简单
        //   可靠：没溢出时点 > clamp 到 0 是 noop 无害。
        let tabs_len = app.tabs.len();
        let offset_x = self.scroll_handle.offset().x;
        let show_left = offset_x < px(-0.5);
        let show_right = tabs_len >= 2;

        let arrow_left = div()
            .id("tab-bar-arrow-left")
            .h_full()
            .w(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(colors.muted_foreground)
            .hover(move |s| s.bg(colors.secondary_hover).text_color(colors.foreground))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_scroll_left(cx);
                }),
            )
            .child(aish_ui::icon(aish_ui::IconName::ChevronLeft).size(px(14.0)));

        let arrow_right = div()
            .id("tab-bar-arrow-right")
            .h_full()
            .w(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(colors.muted_foreground)
            .hover(move |s| s.bg(colors.secondary_hover).text_color(colors.foreground))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_scroll_right(cx);
                }),
            )
            .child(aish_ui::icon(aish_ui::IconName::ChevronRight).size(px(14.0)));

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _w, cx| {
                this.handle_edit_key(ev, cx);
            }))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .bg(colors.card)
            .border_b_1()
            .border_color(colors.border)
            .h(px(40.0))
            .when(show_left, |d| d.child(arrow_left))
            .child(
                div()
                    .id("tab-bar-scroll")
                    .flex_1()
                    // flex item 默认 min_width=auto 拒绝 shrink → tab_items
                    // 撑大容器让 overflow_x_scroll 失效。min_w(0) 强制允许
                    // 压缩到 0，flex_1 才取父 div 剩余空间，溢出的 tab 滚动。
                    .min_w(px(0.0))
                    .h_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll_handle)
                    .children(tab_items),
            )
            .when(show_right, |d| d.child(arrow_right))
            .child(plus_btn)
    }
}
