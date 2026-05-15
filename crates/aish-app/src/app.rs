//! aish GPUI 主应用入口。

use std::collections::HashMap;
use std::sync::Arc;

use aish_types::ConnectionId;
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, Entity, SharedString, TitlebarOptions, Window,
    WindowBounds, WindowControlArea, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{
    AppState, DisconnectReason, SessionCommand, SidebarTab, SshErrorKind, SshEvent,
};

/// 把 per-conn entity HashMap 与"哪些 key 还 alive"做同步：drop 已 stale
/// 的 entry（HashMap 调用 retain；V 被销毁，Entity 之类的就会顺着 Drop
/// chain 把内部 timer / observe 等也清掉）。
///
/// 提出来便于 unit test：本身就是 std `HashMap::retain` 的薄包装 + 注释，
/// 没什么逻辑，但 caller 现场写 retain 时容易把谓词写反（"哪些保留" vs
/// "哪些丢弃"），名字明确为 `retain_alive_entities` 避免反向歧义。
///
/// 当前唯一 caller：RootView observe(state) 回调里同步 `input_bars`
/// 与 `state.connections`（M22 per-conn InputBar 改造）。
pub(crate) fn retain_alive_entities<K, V>(
    map: &mut HashMap<K, V>,
    mut is_alive: impl FnMut(&K) -> bool,
) where
    K: Eq + std::hash::Hash,
{
    map.retain(|k, _| is_alive(k));
}

/// AssetSource 链：先尝试 aish-ui 的 IconName 体系，再尝试 aish-app 自己的
/// 本地资源（titlebar logo / 应用 icon 等）。GPUI Application::with_assets
/// 只接一个 source，用此 wrapper 合并两套 namespace。
struct AppAssets;

impl gpui::AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        // 先 forward 到 aish-ui（icons/x.svg / icons/distros/ubuntu.svg 等）
        if let Some(bytes) = gpui::AssetSource::load(&aish_ui::AishUiAssets, path)? {
            return Ok(Some(bytes));
        }
        // aish-app 本地资源：
        // - logo.svg 单色 path（如果未来需要 svg() monochrome 渲染）
        // - aish-icon.png 真品牌多色 logo（titlebar img() 用，保留蓝/黑双色）
        match path {
            "logo.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!(
                "../assets/logo.svg"
            )))),
            "aish-icon.png" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!(
                "../../../assets/icons/aish-128.png"
            )))),
            _ => Ok(None),
        }
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(vec![])
    }
}

pub fn run() {
    let bridge_owner = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));
    // 保留一个引用用于 run 结束后 drop（确保 runtime 在所有窗口关闭后 shutdown）
    let bridge_keep = bridge_owner.clone();

    application()
        .with_assets(AppAssets)
        .run(move |cx: &mut App| {
            // 注册 aish-ui Theme global（必须在创建任何 view / 调用 theme(cx) 之前）
            // 启动主题：先 load app_state.theme，未持久化时默认 dark。
            // M30：同时回灌 reduced_motion 偏好（None / Some(false) 都视为 false）。
            // 注意：load_app_state 还会在下面被复用读 recent，重复 IO 但极便宜。
            let snapshot = crate::app_state_file::load_app_state();
            let mut init_theme = match snapshot.theme.as_deref() {
                Some("light") => aish_ui::Theme::light(),
                _ => aish_ui::Theme::dark(),
            };
            init_theme.reduced_motion = snapshot.reduced_motion.unwrap_or(false);
            cx.set_global(init_theme);

            // 创建 ToastManager Entity 并注册 ToastHandle global
            let toast_manager = cx.new(aish_ui::ToastManager::new);
            cx.set_global(aish_ui::ToastHandle(toast_manager.clone()));

            crate::terminal::font::register_bundled_font(cx);
            // M28 T7: load 失败时记 error，Home 渲染 ErrorState + 重试入口
            // 替代之前 silent fail（用户感知不到）。
            let (hosts, hosts_load_error) = match crate::persistence::load_hosts() {
                Ok(h) => (h, None),
                Err(e) => {
                    tracing::error!("load hosts.json failed: {}", e);
                    (Vec::new(), Some(format!("{}", e)))
                }
            };
            let loaded_state = crate::app_state_file::load_app_state();
            let last_connected = loaded_state.into_last_connected();
            let channel = EventChannel::new();
            let tx_for_state = channel.tx.clone();
            let state = cx.new(|_cx| {
                let mut s = AppState::with_hosts(hosts);
                s.last_connected = last_connected;
                s.hosts_load_error = hosts_load_error;
                // 注入 event_tx：让 alacritty Term 的 TitleListener 能把 OSC
                // 0/1/2 title event 推回主循环
                s.event_tx = Some(tx_for_state);
                s
            });

            // 接收 SshEvent loop
            let state_for_loop = state.clone();
            let mut rx = channel.rx;
            cx.spawn(async move |cx| {
                while let Some(event) = rx.recv().await {
                    state_for_loop.update(cx, |state, cx| match event {
                        SshEvent::Connected { conn } => {
                            state.mark_connected(conn);
                            cx.notify();
                        }
                        SshEvent::PaneOutput { conn, bytes } => {
                            state.feed_bytes(conn, &bytes);
                            cx.notify();
                        }
                        SshEvent::Disconnected { conn, reason } => {
                            // UserRequested / RemoteExited 静默（用户自己断开 / shell exit
                            // 是常规流程，提示反而干扰）；NetworkError 才弹 toast。
                            let reason_str = match &reason {
                                DisconnectReason::UserRequested => "用户主动断开".to_string(),
                                DisconnectReason::RemoteExited => "远端 shell 退出".to_string(),
                                DisconnectReason::NetworkError(e) => format!("网络错误: {}", e),
                            };
                            if let DisconnectReason::NetworkError(ref e) = reason {
                                let label = state
                                    .connections
                                    .get(&conn)
                                    .map(|c| c.label.clone())
                                    .unwrap_or_else(|| "未知连接".to_string());
                                aish_ui::toast_error(cx, format!("{}: 连接中断 — {}", label, e));
                            }
                            state.drop_session(conn, reason_str);
                            cx.notify();
                        }
                        SshEvent::Error { conn, kind, msg } => {
                            let label = state
                                .connections
                                .get(&conn)
                                .map(|c| c.label.clone())
                                .unwrap_or_else(|| "未知连接".to_string());
                            tracing::error!(?conn, label = %label, ?kind, msg = %msg, "SSH error");
                            let kind_zh = match kind {
                                SshErrorKind::ConnectFailed => "连接失败",
                                SshErrorKind::AuthFailed => "登录失败",
                                SshErrorKind::Io => "网络中断",
                                SshErrorKind::Protocol => "会话异常",
                            };
                            // Disconnect 这种'远端主动断 / 长时间 idle 后 client
                            // 自己关'的场景 msg 含 'Disconnected'。比'协议错误'
                            // 更直观，提示用户双击 tab 可重连。
                            let user_msg = if msg.contains("Disconnect") {
                                format!("{}: 连接已断开 — 双击 tab 可重连", label)
                            } else {
                                format!("{}: {} — {}", label, kind_zh, msg)
                            };
                            aish_ui::toast_error(cx, user_msg);
                            state.drop_session(conn, format!("{}: {}", kind_zh, msg));
                            cx.notify();
                        }
                        SshEvent::OsDetected {
                            conn: _,
                            host_id,
                            os_kind,
                        } => {
                            // 写入对应 HostConfig.os_kind + 持久化 hosts.json
                            // 失败时仅 log warning，不打扰用户（探测本身就是 best-effort）。
                            let mut changed = false;
                            if let Some(host) =
                                state.hosts.iter_mut().find(|h| h.id == host_id)
                            {
                                if host.os_kind != os_kind {
                                    host.os_kind = os_kind;
                                    changed = true;
                                }
                            }
                            if changed {
                                if let Err(e) = crate::persistence::save_hosts(&state.hosts) {
                                    tracing::warn!("save hosts.json after os detect: {}", e);
                                }
                                cx.notify();
                            }
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
                        SshEvent::TmuxMouseDisabled { conn } => {
                            // 仅在 tmux 装了的情况下发。提示用户开 mouse 否则
                            // click/drag/wheel 在 tmux 内全失效（aish 的 SGR
                            // mouse 转发只对开了 mouse on 的 tmux 生效）。
                            let label = state
                                .connections
                                .get(&conn)
                                .map(|c| c.label.clone())
                                .unwrap_or_else(|| "未知连接".to_string());
                            aish_ui::toast_warning(
                                cx,
                                format!(
                                    "{}: 远端 tmux 鼠标未开启 — 在 ~/.tmux.conf 加 `set -g mouse on` 后 tmux source-file 一次",
                                    label
                                ),
                            );
                        }
                        SshEvent::TmuxAttached { conn, session } => {
                            // raw attach 已派发到 PTY；标记 sidebar 高亮当前 session。
                            state.mark_tmux_attached(conn, session);
                            cx.notify();
                        }
                        SshEvent::TmuxSessionDetached { conn, session } => {
                            // detach-detect：actor 在 channel data 检测到
                            // "[detached" 标记 → 清 sidebar attached 标记
                            state.mark_tmux_detached(conn, session);
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
                        SshEvent::BatchAborted { conn, succeeded, total, reason } => {
                            // 批量上传中途失败（含 30s 超时）：清进度并写
                            // last_aborted_batch 给 InputBar 在 last_uploading 边沿
                            // 决定 drain 前 succeeded 张已成功的、保留剩余 + text
                            // 供用户 retry。text 不附加到 PTY（用户没发完整批次）。
                            state.pending_uploads.remove(&conn);
                            state.last_aborted_batch.insert(conn, (succeeded, total));
                            let label = state
                                .connections
                                .get(&conn)
                                .map(|c| c.label.clone())
                                .unwrap_or_else(|| "未知连接".to_string());
                            tracing::warn!(?conn, label = %label, succeeded, total, reason = %reason, "batch aborted");
                            aish_ui::toast_error(
                                cx,
                                format!(
                                    "{}: 上传失败（{}/{}）— {}，请重试",
                                    label, succeeded, total, reason
                                ),
                            );
                            cx.notify();
                        }
                        SshEvent::TitleChanged { conn, title } => {
                            // 远端 OSC 0/1/2 title 通过 TitleListener 推上来。
                            // helper 内部判断 title_locked，locked 时静默忽略
                            // （保留用户手动重命名）。
                            if state.set_tab_title_for_conn(conn, title) {
                                cx.notify();
                            }
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
                    // 隐掉系统原生 titlebar 内容（macOS/Windows），由 RootView
                    // 顶部自绘 36px strip：aish 标识 + 拖拽区 + 三按钮。
                    // 注意：启用后必须保证自绘按钮逻辑正确（remove_window 等），
                    // 否则用户无法关窗口。
                    appears_transparent: true,
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
    /// bridge 保留用于 lazy 创建 per-conn InputBarView（M22 改造）。
    bridge: Arc<Bridge>,
    sidebar_nav: Entity<crate::views::SidebarNavView>,
    tab_bar: Entity<crate::views::TabBarView>,
    home: Entity<crate::views::HomeView>,
    terminal: Entity<crate::views::TerminalView>,
    empty_terminal: Entity<crate::views::EmptyTerminalGuideView>,
    settings: Entity<crate::views::SettingsView>,
    host_form: Entity<crate::views::HostFormModal>,
    session_picker: Entity<crate::views::SessionPickerView>,
    /// M22：每个 ConnectionId 一个独立 InputBarView entity，草稿（文字 + 图片
    /// 缩略图 + TextInput cursor / IME）天然按 conn 隔离。lazy 创建于 render
    /// 内首次遇到该 conn 时；conn 销毁（state.connections 不再包含）→ observe
    /// 回调内 retain 清掉 entry → Entity 自动 drop 释放 timer + 内存。
    input_bars: HashMap<ConnectionId, Entity<crate::views::InputBarView>>,
    toast_manager: Entity<aish_ui::ToastManager>,
    /// 全局 key handler 用 focus_handle 让 root 在 focus dispatch path 上，
    /// 接 Ctrl+1/2/3 切 sidebar tab。不主动 .focus()，子 view（terminal /
    /// input_bar / settings）仍能 focus 处理自己的键。
    focus_handle: gpui::FocusHandle,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        // M22：state.notify 每次都做 InputBar HashMap reconcile —— 把已被
        // remove_connection 清掉的 conn 对应 entity 也从 input_bars 删除（drop
        // 释放 timer + 草稿内存）。retain 是 O(N) 且 N=同时 alive 的 conn 数，
        // 通常 < 10；feed_bytes 高频 notify 也无所谓，无 alloc。
        cx.observe(&state, |this, state, cx| {
            let alive_state = state.read(cx);
            retain_alive_entities(&mut this.input_bars, |conn| {
                alive_state.connections.contains_key(conn)
            });
            cx.notify();
        })
        .detach();

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
        let settings = cx.new(crate::views::SettingsView::new);
        let host_form = cx.new(|cx| {
            crate::views::HostFormModal::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let session_picker = cx.new(|cx| {
            crate::views::SessionPickerView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });

        let toast_manager = cx.global::<aish_ui::ToastHandle>().0.clone();

        Self {
            state,
            bridge,
            sidebar_nav,
            tab_bar,
            home,
            terminal,
            empty_terminal,
            settings,
            host_form,
            session_picker,
            input_bars: HashMap::new(),
            toast_manager,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Ctrl+1/2/3 切 sidebar tab。Ctrl+W / Ctrl+T / Ctrl+Tab 仍只在 terminal_view
    /// focused 时生效（涉及当前 tab 操作，Home/Settings 模式无意义）。
    fn handle_global_key(&mut self, ev: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let m = &ev.keystroke.modifiers;
        if !m.control || m.shift || m.alt {
            return;
        }
        let target = match key {
            "1" => SidebarTab::Home,
            "2" => SidebarTab::Terminal,
            "3" => SidebarTab::Settings,
            _ => return,
        };
        self.state.update(cx, |s, cx| {
            if s.sidebar != target {
                s.sidebar = target;
                cx.notify();
            }
        });
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let sidebar = app.sidebar;
        let modal_open = app.modal.is_some();
        // SessionPicker 只在 Terminal sidebar 且当前 tab 是该 connection 时显示。
        // 用户在 Home / Settings 时不该被 picker 打扰；连接成功事件触发的
        // pending_session_picker 仍保留在 state，等用户切回该 conn 的 tab 时
        // SessionPickerView.sync_from_state 会自动 open。
        let picker_open = app.pending_session_picker.is_some()
            && app.sidebar == SidebarTab::Terminal
            && app.current_connection() == app.pending_session_picker;
        let colors = aish_ui::theme(cx).colors;
        let tabs_empty = app.tabs.is_empty();
        let _ = app;

        // 主区内容：按 sidebar 分支
        let main_body: gpui::AnyElement = match sidebar {
            SidebarTab::Home => self.home.clone().into_any_element(),
            SidebarTab::Terminal => {
                if tabs_empty {
                    self.empty_terminal.clone().into_any_element()
                } else {
                    // session_picker 挂在 terminal viewport 内（relative 容器
                    // 子级 absolute backdrop+dialog），仅遮挡 terminal + input_bar
                    // 区域；sidebar / titlebar / tab_bar 不被挡，用户仍可切
                    // tab / 切 home / 关窗口。
                    let mut terminal_area = div()
                        .relative()
                        .flex_1()
                        .min_h(px(0.0))
                        .child(self.terminal.clone());
                    if picker_open {
                        terminal_area = terminal_area.child(self.session_picker.clone());
                    }
                    // M22：lazy 拿 / 建当前 conn 的 InputBarView entity。
                    // - Default tab（current_connection() == None）：不挂 InputBar
                    //   元素，main_body 只 tab_bar + terminal_area。语义对齐
                    //   "没 shell 没 input"。
                    // - Connection tab：HashMap 没该 conn → 先 cx.new 建一个
                    //   per-conn entity 插进 input_bars（分两步借局部变量绕
                    //   borrow checker：cx.new 借 &mut cx，insert 借 &mut self，
                    //   合在 entry().or_insert_with 闭包里会双 mut borrow 报错）。
                    let current_conn = self.state.read(cx).current_connection();
                    let input_bar_el = current_conn.map(|conn| {
                        if !self.input_bars.contains_key(&conn) {
                            let state = self.state.clone();
                            let bridge = self.bridge.clone();
                            let entity = cx
                                .new(|cx| crate::views::InputBarView::new(conn, state, bridge, cx));
                            self.input_bars.insert(conn, entity);
                        }
                        self.input_bars
                            .get(&conn)
                            .expect("input_bars 刚 insert 必存在")
                            .clone()
                    });
                    let mut col = div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .child(self.tab_bar.clone())
                        .child(terminal_area);
                    if let Some(ib) = input_bar_el {
                        col = col.child(ib);
                    }
                    col.into_any_element()
                }
            }
            SidebarTab::Settings => self.settings.clone().into_any_element(),
        };

        // 外层：sidebar + 主区横排
        let main = div()
            .flex()
            .flex_row()
            .size_full()
            // root bg 跟随 theme：dark=#050505 / light=#fafafa（colors.background
            // global，theme 切换时 refresh_windows 让所有 view re-render 拿新值）
            .bg(colors.background)
            .child(self.sidebar_nav.clone())
            // 主区 flex_1 必须 min_w(0) + min_h(0)，否则 main_body 内任何
            // 超长子（如 tab_bar 内的 tab items 总宽 > viewport - sidebar，
            // 或 home cards 总高 > viewport）会撑大 flex item（min_w/min_h
            // 默认 auto = 内容宽/高，拒绝 shrink），导致 sidebar 被挤出窗口、
            // tab_bar overflow_x_scroll 失效（容器宽 = 内容宽 = "没溢出"），
            // home 滚动也失效（容器高 = 内容高 → scroll_max = 0）。
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .child(main_body),
            );

        // ───── 自绘 titlebar（40px strip，与 tab bar 同高）─────
        // appears_transparent=true 后系统不画 titlebar，必须自己提供拖拽区 + 窗口
        // 控件。整个 titlebar bg=card（与 tab bar 同色无缝衔接），36 改 40 与
        // tab bar 齐高避免"上窄下宽"。
        //
        // 平台差异：
        // - macOS：系统仍把 traffic-light（close/min/max）画在左上 ~80px，
        //   自绘内容必须让位；右侧不再画一套自绘按钮（与系统重复）。
        //   布局：[80px 系统占位][拖拽区 + 居中 logo+aish]。
        // - Windows/Linux：系统不提供任何窗口控件，全部自绘。
        //   布局：[左 logo+aish][中拖拽][右 min/max/close]。
        //
        // logo + 标题：aish 真品牌 logo + "aish" 文字。
        // 注意：GPUI 的 svg() 元素是 **monochrome 模式** —— 整个 SVG 用 text_color
        // 染色覆盖原 fill，无法显示多色 logo。aish 品牌 logo 是蓝+黑双色 + 装饰
        // SSH 钥匙，必须用 img() 加载 PNG 才保留原色。
        // assets/icons/aish-128.png 是项目品牌图标的 128×128 PNG（多色 raster），
        // 通过 AppAssets 的 path="aish-icon.png" 暴露给 GPUI img() Embedded
        // resource 路径。22px 显示时 GPUI 会自动缩放。
        let is_macos = cfg!(target_os = "macos");

        // 中部拖拽区：
        // - Windows：window_control_area(Drag) 让 WM_NCHITTEST 返回 HTCAPTION，
        //   OS 接管拖拽 + Aero snap（拖到屏幕边缘自动 1/2 屏；之前自绘 mouse_down
        //   走 start_window_move 触发的拖拽不走 NC 路径，没有 Aero snap）
        // - macOS / Linux：fall-through 到 mouse_down，调 start_window_move
        // - macOS 上把 logo+aish 文字塞进拖拽区并居中，避开左上红绿灯
        let titlebar_drag_area = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .when(is_macos, |d| {
                d.justify_center().gap(px(8.0)).child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(gpui::img("aish-icon.png").w(px(22.0)).h(px(22.0)))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(colors.foreground)
                                .child("aish"),
                        ),
                )
            })
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(gpui::MouseButton::Left, |_ev, window, _cx| {
                window.start_window_move()
            });

        // 左侧区域：
        // - macOS：80px 空占位，让出系统 traffic-light
        // - 其它：放 logo + "aish" 文字
        let titlebar_left = if is_macos {
            div().w(px(80.0)).h_full()
        } else {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px(px(12.0))
                .child(gpui::img("aish-icon.png").w(px(22.0)).h(px(22.0)))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(colors.foreground)
                        .child("aish"),
                )
        };

        // 窗口控件按钮（minimize / maximize / close）：
        // - area 参数 → window_control_area(area)：Windows 上让 WM_NCHITTEST
        //   返回对应 HTMINBUTTON / HTMAXBUTTON / HTCLOSE。Max 区 Win11 自动
        //   hover 显示 snap layouts overlay（1/2 屏、四宫格等），系统接管点击。
        // - 非 Windows 平台 window_control_area 是 no-op，fall-through 到
        //   on_mouse_down 走 GPUI 通用 minimize/zoom/remove。
        // - 注意 Windows 上 NC 区域 OS 已接管点击，on_mouse_down 不会触发；
        //   两套 handler 并存兼容跨平台。
        // 闭包 + impl trait 泛型避免 Box<dyn> 类型复杂度（clippy::type_complexity）
        fn titlebar_btn<F>(
            id: &'static str,
            glyph: &'static str,
            base_fg: gpui::Hsla,
            hover_bg: gpui::Hsla,
            hover_fg: gpui::Hsla,
            area: WindowControlArea,
            on_click: F,
        ) -> impl IntoElement
        where
            F: Fn(&mut Window, &mut App) + 'static,
        {
            div()
                .id(id)
                .h_full()
                .w(px(46.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_size(px(11.0))
                .text_color(base_fg)
                .window_control_area(area)
                .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                .on_mouse_down(gpui::MouseButton::Left, move |_ev, window, cx| {
                    on_click(window, cx);
                })
                .child(glyph)
        }

        // macOS 右侧不再画自绘 min/max/close —— 系统已在左上提供 traffic-light，
        // 重复一套会视觉冲突 + 与 native 应用习惯不符。
        let titlebar = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(40.0))
            .w_full()
            .bg(colors.card)
            .border_b_1()
            .border_color(colors.border)
            .child(titlebar_left)
            .child(titlebar_drag_area)
            .when(!is_macos, |d| {
                d.child(titlebar_btn(
                    "tb-min",
                    "\u{2014}", // em dash
                    colors.muted_foreground,
                    colors.secondary_hover,
                    colors.foreground,
                    WindowControlArea::Min,
                    |window, _cx| window.minimize_window(),
                ))
                .child(titlebar_btn(
                    "tb-max",
                    "\u{25A2}", // white square with rounded corners
                    colors.muted_foreground,
                    colors.secondary_hover,
                    colors.foreground,
                    WindowControlArea::Max,
                    |window, _cx| window.zoom_window(),
                ))
                .child(titlebar_btn(
                    "tb-close",
                    "\u{2715}", // multiplication x
                    colors.muted_foreground,
                    // close 按钮 hover 用 Windows 11 风真红（#e81123），与系统标题
                    // 栏 close 按钮颜色一致 + 与 minimize/maximize 走 muted hover
                    // 形成强警示对比。原 destructive token (#f7768e Tokyo Night)
                    // 偏粉淡，在 muted 深底上不够"危险"，用户感知偏白。
                    gpui::rgb(0xe81123).into(),
                    // hover_fg 强制白色 — destructive_foreground 是 0x050505 黑色
                    // (dark) / 0xfafafa 白色 (light)，dark 下黑 X on 红底对比够；
                    // 但 hardcoded 白色保证 dark/light 一致 + 与 Windows close 风
                    // 一致
                    gpui::rgb(0xffffff).into(),
                    WindowControlArea::Close,
                    |window, _cx| window.remove_window(),
                ))
            });

        // 把 titlebar 放在 root 顶部，main 在下面
        //
        // 关键：main wrapper 必须 min_h(0) —— flex_col 内 flex_1 item 默认
        // min-height: auto，children 撑大时 item 跟着撑大不 shrink，让下游
        // home/settings 的 scroll 容器拿到的 bounds.height 也跟着膨胀，
        // 失去 scroll_max 计算依据。CSS 经典三栏布局必加 min-height: 0。
        let root_with_titlebar = div()
            .flex()
            .flex_col()
            .size_full()
            .child(titlebar)
            .child(div().flex_1().min_h(px(0.0)).child(main));

        let mut root = div()
            .relative()
            .size_full()
            // track_focus 让 RootView 在 focus dispatch path 末端有占位，
            // 子 view 处理完不消费 key 后 bubble 到这里接 Ctrl+1/2/3 全局切
            // sidebar。不主动 .focus() — 不窃取子 view 焦点。
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _w, cx| {
                this.handle_global_key(ev, cx);
            }))
            .child(root_with_titlebar);

        // session_picker 不在 root 而在 terminal_area 内（见 main_body Terminal
        // 分支）—— 让 backdrop 仅遮 terminal viewport，不挡 sidebar / titlebar /
        // tab_bar，用户可继续切 tab / 切 home 不被锁死。
        if modal_open {
            root = root.child(self.host_form.clone());
        }

        // Context menus 在 root 顶层 mount —— tab_bar / home 自己 mount 时
        // paint 顺序在下游 view（terminal_view / session_picker）之前，会被
        // absolute backdrop / popup 盖掉。提到 root 顶层（与 toast / host_form
        // 同层）让 z-order 最高。open=false 时 menu render 返回空 div 不占
        // 空间，永驻 mount 无成本。
        root = root
            .child(self.tab_bar.read(cx).context_menu_entity())
            .child(self.home.read(cx).context_menu_entity());

        // Toast 总是叠在最顶层
        root = root.child(self.toast_manager.clone());

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retain_alive_entities_keeps_all_when_all_alive() {
        let mut map: HashMap<u32, &'static str> = HashMap::from([(1, "a"), (2, "b"), (3, "c")]);
        retain_alive_entities(&mut map, |_| true);
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn retain_alive_entities_drops_all_when_none_alive() {
        let mut map: HashMap<u32, &'static str> = HashMap::from([(1, "a"), (2, "b")]);
        retain_alive_entities(&mut map, |_| false);
        assert!(map.is_empty());
    }

    #[test]
    fn retain_alive_entities_drops_only_stale() {
        let mut map: HashMap<u32, &'static str> =
            HashMap::from([(1, "a"), (2, "b"), (3, "c"), (4, "d")]);
        // alive = 偶数
        retain_alive_entities(&mut map, |k| k % 2 == 0);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&2), Some(&"b"));
        assert_eq!(map.get(&4), Some(&"d"));
        assert!(!map.contains_key(&1));
        assert!(!map.contains_key(&3));
    }

    #[test]
    fn retain_alive_entities_handles_empty_map() {
        let mut map: HashMap<u32, &'static str> = HashMap::new();
        retain_alive_entities(&mut map, |_| true);
        assert!(map.is_empty());
        retain_alive_entities(&mut map, |_| false);
        assert!(map.is_empty());
    }
}
