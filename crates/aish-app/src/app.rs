//! aish GPUI 主应用入口。

use std::sync::Arc;

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
    let bridge = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));
    // 保留一个引用用于 run 结束后 drop（确保 runtime 在所有窗口关闭后 shutdown）
    let bridge_owner = bridge.clone();

    application().run(move |cx: &mut App| {
        let state = cx.new(|_cx| AppState::with_mock_hosts());
        let channel = EventChannel::new();

        // 2. 启动 GPUI cx.spawn 接收 channel 事件，update Model
        let state_for_loop = state.clone();
        let mut rx = channel.rx;
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                state_for_loop.update(cx, |state, cx| {
                    let crate::state::MockEvent::PaneOutput { host, line } = event;
                    state.append_log(host, line);
                    cx.notify();
                });
            }
        })
        .detach();

        // 3. 开窗口，传入 bridge + tx 让 HostListView 能 spawn mock task
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("aish — M1 skeleton")),
                ..Default::default()
            }),
            ..Default::default()
        };

        let bridge_for_window = bridge.clone();
        let tx_for_window = channel.tx.clone();
        let state_for_window = state.clone();

        cx.open_window(window_options, move |_window, cx| {
            cx.new(|cx| {
                RootView::new(
                    state_for_window.clone(),
                    bridge_for_window.clone(),
                    tx_for_window.clone(),
                    cx,
                )
            })
        })
        .expect("主窗口应能打开");

        cx.activate(true);
    });

    drop(bridge_owner);
}

struct RootView {
    host_list: Entity<crate::views::HostListView>,
    host_pane: Entity<crate::views::HostPaneView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<crate::state::MockEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let host_list = cx.new(|cx| crate::views::HostListView::new(state.clone(), bridge, tx, cx));
        let host_pane = cx.new(|cx| crate::views::HostPaneView::new(state, cx));
        Self {
            host_list,
            host_pane,
        }
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
            .child(self.host_pane.clone())
    }
}
