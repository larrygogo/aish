//! 左栏：mock host 列表，点击切换 selected。

use gpui::{
    div, prelude::*, px, rgb, Context, Entity, MouseButton, MouseDownEvent, Render, Window,
};

use crate::state::{AppState, HostId};

pub struct HostListView {
    state: Entity<AppState>,
}

impl HostListView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state }
    }

    fn handle_click(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.select_host(host_id);
            let label = state
                .hosts
                .iter()
                .find(|h| h.id == host_id)
                .map(|h| h.label.clone())
                .unwrap_or_else(|| format!("host {:?}", host_id));
            let line = format!("[{}] Connecting to {}...", simple_time(), label);
            state.append_log(host_id, line);
            cx.notify();
        });
    }
}

/// 简易时间字符串，避免引入 chrono 依赖。
fn simple_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 86400)
        .unwrap_or(0);
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;
        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|style| style.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _window, cx| {
                            this.handle_click(id, cx);
                        }),
                    )
                    .child(label)
            })
            .collect();

        div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .children(host_rows)
    }
}
