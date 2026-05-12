//! aish GPUI 主应用入口。

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{
    AppState, DisconnectReason, SessionCommand, SidebarTab, SshErrorKind, SshEvent,
};

pub fn run() {
    let bridge_owner = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));
    // 保留一个引用用于 run 结束后 drop（确保 runtime 在所有窗口关闭后 shutdown）
    let bridge_keep = bridge_owner.clone();

    application()
        .with_assets(aish_ui::AishUiAssets)
        .run(move |cx: &mut App| {
            // 注册 aish-ui Theme global（必须在创建任何 view / 调用 theme(cx) 之前）
            cx.set_global(aish_ui::Theme::dark());

            // 创建 ToastManager Entity 并注册 ToastHandle global
            let toast_manager = cx.new(aish_ui::ToastManager::new);
            cx.set_global(aish_ui::ToastHandle(toast_manager.clone()));

            crate::terminal::font::register_bundled_font(cx);
            let hosts = match crate::persistence::load_hosts() {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("load hosts.json failed: {} — starting with empty list", e);
                    Vec::new()
                }
            };
            let loaded_state = crate::app_state_file::load_app_state();
            let last_connected = loaded_state.into_last_connected();
            let state = cx.new(|_cx| {
                let mut s = AppState::with_hosts(hosts);
                s.last_connected = last_connected;
                s
            });
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
                        SshEvent::Disconnected { conn, reason } => {
                            // UserRequested / RemoteExited 静默（用户自己断开 / shell exit
                            // 是常规流程，提示反而干扰）；NetworkError 才弹 toast。
                            if let DisconnectReason::NetworkError(ref e) = reason {
                                let label = state
                                    .connections
                                    .get(&conn)
                                    .map(|c| c.label.clone())
                                    .unwrap_or_else(|| "未知连接".to_string());
                                aish_ui::toast_error(cx, format!("{}: 连接中断 — {}", label, e));
                            }
                            state.drop_session(conn);
                            cx.notify();
                        }
                        SshEvent::Error { conn, kind, msg } => {
                            let label = state
                                .connections
                                .get(&conn)
                                .map(|c| c.label.clone())
                                .unwrap_or_else(|| "未知连接".to_string());
                            tracing::error!(?conn, label = %label, ?kind, msg = %msg, "SSH error");
                            let user_msg = match kind {
                                SshErrorKind::ConnectFailed => {
                                    format!("{}: 连接失败 — {}", label, msg)
                                }
                                SshErrorKind::AuthFailed => {
                                    format!("{}: 登录失败 — {}", label, msg)
                                }
                                SshErrorKind::Io => format!("{}: IO 错误 — {}", label, msg),
                                SshErrorKind::Protocol => {
                                    format!("{}: 协议错误 — {}", label, msg)
                                }
                            };
                            aish_ui::toast_error(cx, user_msg);
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
                        SshEvent::ImageUploaded { conn, path } => {
                            // 流式：每张图（无论单发还是 batch 中）成功后立即 append path
                            // 到 PTY。前置空格防止与用户已输入 token 粘连。
                            if let Some(sender) = state.sessions.get(&conn).cloned() {
                                let msg = format!(" {}", path);
                                let _ =
                                    sender.try_send(SessionCommand::SendBytes(msg.into_bytes()));
                            }
                        }
                        SshEvent::ImageUploadFailed { conn, msg } => {
                            // "失败的不插入" —— 不再发 PTY 错误字节，改 toast_error
                            // 给用户 UI 反馈，已成功上传的 path 已在 ImageUploaded 事
                            // 件路径插入，失败的这张不动 tmux。
                            let label = state
                                .connections
                                .get(&conn)
                                .map(|c| c.label.clone())
                                .unwrap_or_else(|| "未知连接".to_string());
                            tracing::warn!(?conn, label = %label, msg = %msg, "image upload failed");
                            aish_ui::toast_error(cx, format!("{}: 图片上传失败 — {}", label, msg));
                        }
                        SshEvent::BatchProgress { conn, done, total } => {
                            // 更新 pending_uploads → input_bar observe 后显示进度
                            state.pending_uploads.insert(conn, (done, total));
                            cx.notify();
                        }
                        SshEvent::BatchDone { conn, text } => {
                            // 全部图片上传完成（成功 + 失败都算完）：
                            // 1. 清 pending_uploads → input_bar 恢复可点 "发送"
                            // 2. 把 input bar 的 text 附加到 PTY（如非空）—— path 已
                            //    随每张 ImageUploaded 事件 append 过了，这里只补 text
                            state.pending_uploads.remove(&conn);
                            if !text.is_empty() {
                                if let Some(sender) = state.sessions.get(&conn).cloned() {
                                    let msg = format!(" {}", text);
                                    let _ = sender
                                        .try_send(SessionCommand::SendBytes(msg.into_bytes()));
                                }
                            }
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
                app_id: Some("aish".to_string()), // Linux WM_CLASS，与 .desktop 匹配
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
    settings: Entity<crate::views::SettingsView>,
    host_form: Entity<crate::views::HostFormModal>,
    session_picker: Entity<crate::views::SessionPickerView>,
    input_bar: Entity<crate::views::InputBarView>,
    toast_manager: Entity<aish_ui::ToastManager>,
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
        let settings = cx.new(|_cx| crate::views::SettingsView::new());
        let host_form = cx.new(|cx| {
            crate::views::HostFormModal::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let session_picker = cx.new(|cx| {
            crate::views::SessionPickerView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let input_bar =
            cx.new(|cx| crate::views::InputBarView::new(state.clone(), bridge.clone(), cx));

        let toast_manager = cx.global::<aish_ui::ToastHandle>().0.clone();

        Self {
            state,
            sidebar_nav,
            tab_bar,
            home,
            terminal,
            empty_terminal,
            settings,
            host_form,
            session_picker,
            input_bar,
            toast_manager,
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
                        .child(self.input_bar.clone())
                        .into_any_element()
                }
            }
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

        // Toast 总是叠在最顶层
        root = root.child(self.toast_manager.clone());

        root
    }
}
