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
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseDownEvent, Window,
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
        }
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
            self.state.update(cx, |s, cx| {
                if let Some(t) = s.tabs.iter_mut().find(|t| t.id == id) {
                    t.title = final_title;
                }
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
                let close_btn = div()
                    .px_1p5()
                    .text_color(colors.secondary_foreground)
                    .cursor_pointer()
                    .hover(|s| s.text_color(colors.destructive))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            // 拦住事件不冒泡到外层 tab listener 触发 select_tab
                            // （目前 select_tab 对已关闭 id 是 noop，但仍规范化）
                            cx.stop_propagation();
                            this.handle_close(id, cx);
                        }),
                    )
                    .child("×");

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

                // connection tab 在标题与关闭键之间显示蓝色 [SSH] 胶囊
                // （从已删除的 ConnectionChip 横条迁移过来，保留连接类型可视化标识）
                let ssh_chip: gpui::AnyElement = if is_connection {
                    div()
                        .px_2p5()
                        .py_0p5()
                        .text_size(font_size.xs)
                        .text_color(colors.primary)
                        .bg(colors.secondary)
                        .rounded_full()
                        .child("SSH")
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                // 选中态底部 2px primary 线（用 border_b 实现）
                let bottom_line: gpui::AnyElement = if is_selected {
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(2.0))
                        .bg(colors.primary)
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .h(px(40.0))
                    .text_size(font_size.sm)
                    .bg(if is_selected {
                        colors.background
                    } else {
                        colors.card
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.accent))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                            this.handle_tab_click(id, ev.click_count, w, cx);
                        }),
                    )
                    .child(prefix)
                    .child(title_el)
                    .child(ssh_chip)
                    .child(close_btn)
                    .child(bottom_line)
            })
            .collect();

        // 末尾 + 按钮新建默认页
        let plus_btn = div()
            .px_4()
            .h(px(40.0))
            .flex()
            .items_center()
            .text_size(font_size.lg)
            .text_color(colors.secondary_foreground)
            .bg(colors.card)
            .cursor_pointer()
            .hover(|s| s.bg(colors.accent).text_color(colors.foreground))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_new_tab(cx)),
            )
            .child("+");

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
            .children(tab_items)
            .child(plus_btn)
    }
}
