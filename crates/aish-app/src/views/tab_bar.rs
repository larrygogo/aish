//! 顶部 Tab 栏：显示所有 tabs，点击切换，× 按钮关闭，末尾 + 按钮新建默认页。
//!
//! 关闭 tab 时若该 tab 引用了 connection，发 `SessionCommand::Disconnect` 并
//! `state.remove_connection`，让 actor 优雅退出。

use std::sync::Arc;

use aish_types::TabId;
use gpui::{div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Window};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TabContent};

pub struct TabBarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl TabBarView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_select(&mut self, id: TabId, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.select_tab(id);
            cx.notify();
        });
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
    }

    fn handle_new_tab(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.new_default_tab();
            cx.notify();
        });
    }
}

impl Render for TabBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let selected = app.selected_tab;

        let tab_items: Vec<_> = app
            .tabs
            .iter()
            .map(|t| {
                let id = t.id;
                let title = t.title.clone();
                let is_selected = selected == Some(id);
                let is_connection = matches!(t.content, TabContent::Connection(_));

                // 关闭按钮（始终可见 — tab 本来就该好关）
                let close_btn = div()
                    .px_1()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xff6666)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            // 阻止冒泡到 tab 选中
                            let _ = ev;
                            this.handle_close(id, cx);
                        }),
                    )
                    .child("×");

                // 连接 tab 用绿点，默认页 tab 不带前缀
                let prefix: gpui::AnyElement = if is_connection {
                    div()
                        .text_color(rgb(0x4ec9b0))
                        .child("●")
                        .into_any_element()
                } else {
                    div().child("").into_any_element()
                };

                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_r_1()
                    .border_color(rgb(0x333333))
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xaaaaaa }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1a1a1a }))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_select(id, cx);
                        }),
                    )
                    .child(prefix)
                    .child(div().child(title))
                    .child(close_btn)
            })
            .collect();

        // 末尾 + 按钮新建默认页
        let plus_btn = div()
            .px_3()
            .py_2()
            .text_color(rgb(0xaaaaaa))
            .hover(|s| {
                s.bg(rgb(0x252525))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_new_tab(cx)),
            )
            .child("+");

        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .bg(rgb(0x1a1a1a))
            .border_b_1()
            .border_color(rgb(0x333333))
            .h(px(36.0))
            .children(tab_items)
            .child(plus_btn)
    }
}
