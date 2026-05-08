//! aish GPUI 主应用入口。

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{AppState, SidebarTab, SshEvent};

pub fn run() {
    let bridge_owner = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));
    // 保留一个引用用于 run 结束后 drop（确保 runtime 在所有窗口关闭后 shutdown）
    let bridge_keep = bridge_owner.clone();

    application().run(move |cx: &mut App| {
        crate::terminal::font::register_bundled_font(cx);
        let hosts = match crate::persistence::load_hosts() {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("load hosts.json failed: {} — starting with empty list", e);
                Vec::new()
            }
        };
        let state = cx.new(|_cx| AppState::with_hosts(hosts));
        let channel = EventChannel::new();

        // 接收 SshEvent loop
        let state_for_loop = state.clone();
        let mut rx = channel.rx;
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                state_for_loop.update(cx, |state, cx| match event {
                    SshEvent::Connected { conn: _ } => {
                        cx.notify();
                    }
                    SshEvent::PaneOutput { conn, bytes } => {
                        state.feed_bytes(conn, &bytes);
                        cx.notify();
                    }
                    SshEvent::Disconnected { conn, reason: _ } => {
                        state.drop_session(conn);
                        cx.notify();
                    }
                    SshEvent::Error { conn, kind: _, msg } => {
                        tracing::error!(?conn, msg, "SSH error");
                        state.drop_session(conn);
                        cx.notify();
                    }
                    SshEvent::TmuxQueryStarted { conn } => {
                        state
                            .tmux_state
                            .insert(conn, crate::state::TmuxState::NotChecked);
                        cx.notify();
                    }
                    SshEvent::TmuxSessionsListed { conn, sessions } => {
                        // 进入 Detected 状态时清空 attached 标记 —— 重新查询时
                        // 上次 attach 的 session 可能已经不存在或被改名。
                        let has_sessions = !sessions.is_empty();
                        state.tmux_state.insert(
                            conn,
                            crate::state::TmuxState::Detected {
                                sessions,
                                attached: None,
                            },
                        );
                        // 远端有 tmux session 且当前 tab 正是该 connection
                        // → 弹 picker 让用户选 attach 哪个（或跳过进 raw shell）。
                        if has_sessions && state.current_connection() == Some(conn) {
                            state.pending_session_picker = Some(conn);
                        }
                        cx.notify();
                    }
                    SshEvent::TmuxQueryFailed { conn, msg } => {
                        state
                            .tmux_state
                            .insert(conn, crate::state::TmuxState::QueryFailed { msg });
                        cx.notify();
                    }
                    SshEvent::TmuxNoTmux { conn } => {
                        state
                            .tmux_state
                            .insert(conn, crate::state::TmuxState::NoTmux);
                        cx.notify();
                    }
                    SshEvent::TmuxAttached { conn, session } => {
                        // raw attach 已派发到 PTY；标记 sidebar 高亮当前 session。
                        state.mark_tmux_attached(conn, session);
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
                title: Some(SharedString::from("aish")),
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

/// 根视图。布局：左侧 SidebarNav（48px）+ 右侧主区（按 sidebar 分支）。
/// HostFormModal / SessionPickerView 作为顶层叠加 modal。
struct RootView {
    state: Entity<AppState>,
    sidebar_nav: Entity<crate::views::SidebarNavView>,
    tab_bar: Entity<crate::views::TabBarView>,
    home: Entity<crate::views::HomeView>,
    terminal: Entity<crate::views::TerminalView>,
    empty_terminal: Entity<crate::views::EmptyTerminalGuideView>,
    inbox: Entity<crate::views::ComingSoonView>,
    settings: Entity<crate::views::ComingSoonView>,
    host_form: Entity<crate::views::HostFormModal>,
    session_picker: Entity<crate::views::SessionPickerView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();

        let sidebar_nav = cx.new(|cx| crate::views::SidebarNavView::new(state.clone(), cx));
        let tab_bar = cx
            .new(|cx| crate::views::TabBarView::new(state.clone(), bridge.clone(), tx.clone(), cx));
        let home =
            cx.new(|cx| crate::views::HomeView::new(state.clone(), bridge.clone(), tx.clone(), cx));
        let terminal = cx.new(|cx| {
            crate::views::TerminalView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let empty_terminal =
            cx.new(|cx| crate::views::EmptyTerminalGuideView::new(state.clone(), cx));
        let inbox =
            cx.new(|_cx| crate::views::ComingSoonView::new(crate::views::ComingSoonKind::Inbox));
        let settings =
            cx.new(|_cx| crate::views::ComingSoonView::new(crate::views::ComingSoonKind::Settings));
        let host_form = cx.new(|cx| {
            crate::views::HostFormModal::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let session_picker = cx.new(|cx| {
            crate::views::SessionPickerView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });

        Self {
            state,
            sidebar_nav,
            tab_bar,
            home,
            terminal,
            empty_terminal,
            inbox,
            settings,
            host_form,
            session_picker,
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let sidebar = app.sidebar;
        let modal_open = app.modal.is_some();
        let picker_open = app.pending_session_picker.is_some();
        let tabs_empty = app.tabs.is_empty();
        let _ = app;

        // 主区内容：按 sidebar 分支
        let main_body: gpui::AnyElement = match sidebar {
            SidebarTab::Home => self.home.clone().into_any_element(),
            SidebarTab::Terminal => {
                if tabs_empty {
                    self.empty_terminal.clone().into_any_element()
                } else {
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .child(self.tab_bar.clone())
                        .child(div().flex_1().child(self.terminal.clone()))
                        .into_any_element()
                }
            }
            SidebarTab::Inbox => self.inbox.clone().into_any_element(),
            SidebarTab::Settings => self.settings.clone().into_any_element(),
        };

        // 外层：sidebar + 主区横排
        let main = div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x000000))
            .child(self.sidebar_nav.clone())
            .child(div().flex_1().child(main_body));

        let mut root = div().relative().size_full().child(main);

        if picker_open {
            root = root.child(self.session_picker.clone());
        }
        if modal_open {
            root = root.child(self.host_form.clone());
        }

        root
    }
}
