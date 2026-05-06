//! aish GPUI 主应用入口。

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{AppState, SshEvent};

pub fn run() {
    let bridge_owner = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));
    // 保留一个引用用于 run 结束后 drop（确保 runtime 在所有窗口关闭后 shutdown）
    let bridge_keep = bridge_owner.clone();

    application().run(move |cx: &mut App| {
        let hosts = crate::fixtures::dev_hosts();
        let state = cx.new(|_cx| AppState::with_hosts(hosts));
        let channel = EventChannel::new();

        // 接收 SshEvent loop
        let state_for_loop = state.clone();
        let mut rx = channel.rx;
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                state_for_loop.update(cx, |state, cx| match event {
                    SshEvent::Connected { host: _ } => {
                        // M2b1: 状态变更通过 host_list 的 ●/○ 显示，不写 pane
                        cx.notify();
                    }
                    SshEvent::PaneOutput { host, bytes } => {
                        state.feed_bytes(host, &bytes);
                        cx.notify();
                    }
                    SshEvent::Disconnected { host, reason: _ } => {
                        state.drop_session(host);
                        cx.notify();
                    }
                    SshEvent::Error { host, kind: _, msg } => {
                        tracing::error!(?host, msg, "SSH error");
                        state.drop_session(host);
                        cx.notify();
                    }
                });
            }
        })
        .detach();

        // 开窗口
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("aish — M2b1")),
                ..Default::default()
            }),
            ..Default::default()
        };

        let bridge_for_window = bridge_owner.clone();
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

    drop(bridge_keep);
}

struct RootView {
    host_list: Entity<crate::views::HostListView>,
    terminal: Entity<crate::views::TerminalView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let host_list = cx.new(|cx| {
            crate::views::HostListView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let terminal = cx.new(|cx| crate::views::TerminalView::new(state, bridge, tx, cx));
        Self {
            host_list,
            terminal,
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x1d1f21))
            .child(self.host_list.clone())
            .child(self.terminal.clone())
    }
}
