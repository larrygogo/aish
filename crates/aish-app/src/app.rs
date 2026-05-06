//! aish GPUI 主应用入口。

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, SharedString, TitlebarOptions, Window,
    WindowBounds, WindowOptions,
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
        let _state = cx.new(|_cx| AppState::with_mock_hosts());

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("aish — M1 skeleton")),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(window_options, |_window, cx| cx.new(|_cx| RootView))
            .expect("主窗口应能打开");

        cx.activate(true);
    });

    drop(bridge);
}

/// 临时 root view —— Task 6/7 会扩展为左栏 + 主区。
struct RootView;

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xeeeeee))
            .child("aish M1 — empty window")
    }
}
