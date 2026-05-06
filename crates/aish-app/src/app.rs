//! aish GPUI 主应用入口。

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::AppState;

/// 启动 GPUI App 与主窗口。
///
/// 此函数 block 直到所有窗口关闭，调用方应在 main 末尾调它。
pub fn run() {
    // 1. 启动 tokio runtime（先于 GPUI App，让 spawn 在窗口未开时也可用）
    let bridge = Bridge::start().expect("tokio runtime 启动失败");
    let _channel = EventChannel::new();

    // 2. 启动 GPUI App
    application().run(move |cx: &mut App| {
        let state = cx.new(|_cx| AppState::with_mock_hosts());
        let state_for_window = state.clone();

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("aish — M1 skeleton")),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(window_options, move |_window, cx| {
            cx.new(|cx| RootView::new(state_for_window.clone(), cx))
        })
        .expect("主窗口应能打开");

        cx.activate(true);
    });

    drop(bridge);
}

struct RootView {
    host_list: Entity<crate::views::HostListView>,
}

impl RootView {
    fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let host_list = cx.new(|cx| crate::views::HostListView::new(state.clone(), cx));
        Self { host_list }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x121212))
            .child(self.host_list.clone())
            .child(
                div()
                    .flex_1()
                    .text_color(rgb(0x888888))
                    .p_4()
                    .child("请从左侧选择主机"),
            )
    }
}
