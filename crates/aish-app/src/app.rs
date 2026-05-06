//! aish GPUI 主应用入口。

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{AppState, DisconnectReason, SshEvent};

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
                    SshEvent::Connected { host } => {
                        state.append_log(host, "[info] Connected".into());
                        cx.notify();
                    }
                    SshEvent::PaneOutput { host, bytes } => {
                        let s = String::from_utf8_lossy(&bytes);
                        for line in s.split('\n') {
                            let line = line.trim_end_matches('\r').to_string();
                            state.append_log(host, line);
                        }
                        cx.notify();
                    }
                    SshEvent::Disconnected { host, reason } => {
                        let msg = match reason {
                            DisconnectReason::RemoteExited => {
                                "[info] 远端 shell 已退出".to_string()
                            }
                            DisconnectReason::NetworkError(e) => format!("[error] 连接中断: {}", e),
                            DisconnectReason::UserRequested => "[info] 已断开".to_string(),
                        };
                        state.append_log(host, msg);
                        state.drop_session(host);
                        cx.notify();
                    }
                    SshEvent::Error { host, kind, msg } => {
                        state.append_log(host, format!("[error] {:?}: {}", kind, msg));
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
                title: Some(SharedString::from("aish — M2a")),
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
    host_pane: Entity<crate::views::HostPaneView>,
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
        let host_pane = cx.new(|cx| crate::views::HostPaneView::new(state, bridge, tx, cx));
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
