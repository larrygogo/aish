//! 主区终端视图。M2b1 Task 4 — 自绘 alacritty grid + 颜色 + 光标闪烁。
//! M2b1 Task 5 — PTY 跟随窗口 resize（250ms debounce + 80ms 远端落地延迟，
//! 见 PTY_RESIZE_DEBOUNCE_MS / LOCAL_TERM_RESIZE_DELAY_MS）。

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    canvas, div, prelude::*, px, rgb, App, Bounds, ClipboardItem, Context, Entity, FocusHandle,
    Focusable, InputHandler, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent, UTF16Selection, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, ConnectionPhase, SessionCommand, SshEvent};
use crate::terminal::{
    cursor::{paint_cursor, CursorState},
    font,
    grid_renderer::{self, paint_grid, GridLayout, GridSnapshot},
};

/// PTY resize debounce 窗口期。bounds 变化后等 N ms 才发 SIGWINCH，避免拖窗
/// 时每 100ms 触发一次远端重排（旧值 100ms 偏短，250ms 让拖动稳定后才发一次）。
const PTY_RESIZE_DEBOUNCE_MS: u64 = 250;

/// IME 输入处理器：把 WM_CHAR / WM_IME_COMPOSITION 提交的文本字节转发给 PTY。
/// 通过 window.handle_input() 在每帧 paint 阶段注册，让 GPUI 的 Windows 平台层
/// 能把 IME 结果（中文/日文/韩文等）和普通可打印字符路由到此处。
struct TerminalImeHandler {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    /// 当前光标的屏幕坐标（窗口逻辑像素），供 WM_IME_STARTCOMPOSITION 定位候选窗口。
    cursor_bounds: Option<gpui::Bounds<gpui::Pixels>>,
}

impl InputHandler for TerminalImeHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        // 返回有效范围表示接受输入
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &mut self,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _replacement_range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut App,
    ) {
        if text.is_empty() {
            return;
        }
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
        let sender = match self.state.read(cx).sessions.get(&conn).cloned() {
            Some(s) => s,
            None => return,
        };
        let bytes = text.as_bytes().to_vec();
        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        // IME preedit（组合阶段）：terminal 没有内联 preedit 区域，暂不处理
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        self.cursor_bounds
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

/// 本地 alacritty Term resize 相对于 SIGWINCH 发出的额外延迟。
/// 用于粗略覆盖 SSH RTT，让远端 tmux 先按新 size 重排再让本地 Term 跟随，
/// 避免本地按新 size 排版而远端旧字节按旧 size 算 → ANSI 错位 / 字符跳动。
/// 80ms 适配常见局域网 / 国内云 RTT < 50ms；越洋高延迟仍会留 100ms+ 错位窗口
/// （后续可加可配置环境变量，本次不做）。
const LOCAL_TERM_RESIZE_DELAY_MS: u64 = 80;

pub struct TerminalView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
    cursor_state: CursorState,
    /// 上次已生效的 (connection, cols, rows)，用于检测变化。把 connection 加进 cache
    /// key 避免切换连接时把 size 当成"没变"跳过 SIGWINCH。
    last_pty_size: Option<(aish_types::ConnectionId, u16, u16)>,
    /// 进行中的 resize debounce task — drop 即取消。
    pending_resize: Option<gpui::Task<()>>,
    /// canvas 的 paint bounds（窗口绝对坐标）。每次 paint 由 prepaint 通过
    /// on_next_frame 异步写入。mouse_down/move 事件的 position 是窗口绝对坐标，
    /// 必须减去 bounds.origin + padding 才能映射到 grid 坐标。
    canvas_bounds: Option<Bounds<Pixels>>,
    /// M31：Disconnected 状态下的 reconnect button entity（press feedback 80ms）。
    /// 仅在 Disconnected 状态 render，其他状态 entity 静置不渲染。
    reconnect_btn: Entity<aish_ui::Button>,
}

impl TerminalView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        let cursor_state = CursorState::new(true);

        // 启动闪烁定时器：每 300ms 触发 cx.notify 重绘
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            if this.update(cx, |_this, cx| cx.notify()).is_err() {
                break;
            }
        })
        .detach();

        // M31: reconnect button entity，weak.upgrade callback 触发 self.handle_reconnect
        let weak = cx.weak_entity();
        let reconnect_btn = cx.new(|cx| {
            let mut b = aish_ui::Button::new("terminal-reconnect", cx);
            b.label("重新连接").primary().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.handle_reconnect(cx));
                }
            });
            b
        });

        Self {
            state,
            bridge,
            tx,
            focus_handle,
            cursor_state,
            last_pty_size: None,
            pending_resize: None,
            canvas_bounds: None,
            reconnect_btn,
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };

        let ctrl = event.keystroke.modifiers.control;
        let shift = event.keystroke.modifiers.shift;
        let alt = event.keystroke.modifiers.alt;
        let key = event.keystroke.key.as_str();

        // Ctrl+Shift+C：复制选中文本到剪贴板，不发到远端
        if ctrl && shift && key.eq_ignore_ascii_case("c") {
            let text = self
                .state
                .read(cx)
                .term_of(conn)
                .and_then(crate::terminal::selection::selected_text);
            if let Some(text) = text {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            return;
        }

        // Ctrl+Shift+V：从剪贴板粘贴。bracketed paste mode 自动检测。
        if ctrl && shift && key.eq_ignore_ascii_case("v") {
            self.paste(conn, cx);
            return;
        }

        // Ctrl+Shift+PageUp / PageDown：左右移动当前 tab。
        // 在 encode_key（line 230 把 pageup 转 \x1b[5~ 发到远端）之前拦截：
        // 用户按这个组合时显然是想操作 UI，不是给远端 less/vim 翻页。
        if ctrl && shift && (key == "pageup" || key == "pagedown") {
            let delta = if key == "pageup" { -1 } else { 1 };
            self.state.update(cx, |s, cx| {
                if s.move_selected_tab(delta) {
                    cx.notify();
                }
            });
            return;
        }

        // Ctrl+W：关闭当前 tab（Chrome / VSCode 标准）。
        // 远端 shell 也有 Ctrl+W = delete-prev-word，但用户在 GUI 终端里期望
        // Ctrl+W 关 tab 更强（用户能用 prefix-bs 或选词替代远端 ctrl-w）。
        if ctrl && !shift && !alt && key.eq_ignore_ascii_case("w") {
            self.close_current_tab(cx);
            return;
        }

        // Ctrl+T：新建 Default tab 并切到 Home sidebar 让用户选 host 连接。
        // 与 tab_bar + 按钮路径一致。覆盖远端 shell Ctrl+T = transpose-chars，
        // 与 Ctrl+W 一样优先 GUI 操作。
        if ctrl && !shift && !alt && key.eq_ignore_ascii_case("t") {
            self.state.update(cx, |s, cx| {
                s.append_default_tab();
                s.sidebar = crate::state::SidebarTab::Home;
                cx.notify();
            });
            return;
        }

        // Ctrl+Tab / Ctrl+Shift+Tab：下一个 / 上一个 tab 循环切换。
        // 远端 shell 不接受 Ctrl+Tab（key_char = None）所以无冲突；
        // Tab 单独传给远端走 encode_key（'\t'）路径，本分支只匹配 ctrl 修饰。
        if ctrl && !alt && key == "tab" {
            let delta = if shift { -1 } else { 1 };
            self.state.update(cx, |s, cx| {
                if s.cycle_selected_tab(delta) {
                    cx.notify();
                }
            });
            return;
        }

        // 有 key_char 且满足以下任一条件时，交由 WM_CHAR → InputHandler 路径发送：
        //   a) 无 Ctrl/Alt 修饰（普通可打印字符、IME 拼音字母）
        //   b) prefer_character_input（AltGr 欧洲键盘，Ctrl+Alt 实为一个字符键）
        // IME 组合中的字母：key_char = Some，但 IME 抑制 WM_CHAR → 不发到 PTY，正确。
        // Enter/Tab/方向键等：key_char = None → 不受影响，走 encode_key。
        let via_input_handler =
            event.keystroke.key_char.is_some() && (!ctrl && !alt || event.prefer_character_input);
        if via_input_handler {
            return;
        }

        let sender = match self.state.read(cx).sessions.get(&conn).cloned() {
            Some(s) => s,
            None => return,
        };

        let bytes = encode_key(key, ctrl, alt);
        if bytes.is_empty() {
            return;
        }

        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }

    /// Ctrl+Shift+V：从剪贴板读文本 → 行结尾归一 → 按远端 bracketed paste mode
    /// 决定是否包 \x1b[200~..\x1b[201~ → 透传到远端 PTY。
    ///
    /// - 行结尾：\r\n / \n → \r（PTY enter = 0x0d，多行命令逐行执行需要 \r）
    /// - bracketed mode（vim/zsh/bash 现代交互模式自动启用）：远端识别为字面字符
    /// - 不在 bracketed mode：直接发，多行命令会逐行执行（用户预期）
    // 关闭当前选中的 tab。Ctrl+W 触发，与 tab_bar handle_close 等价但用
    // selected_tab 而非接 id 参数。
    //
    // 与 tab_bar handle_close 重复的逻辑（取 conn 后发 Disconnect、
    // remove_connection、close_tab）暂不抽 helper，state 没法直接发
    // SessionCommand（需要 bridge），抽出会跨多层 API；20 行内联可接受。
    fn close_current_tab(&mut self, cx: &mut Context<Self>) {
        let (id, content) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected_tab else {
                return;
            };
            let content = s
                .tabs
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.content.clone());
            (id, content)
        };
        if let Some(crate::state::TabContent::Connection(conn)) = content {
            if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                self.bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Disconnect).await;
                });
            }
            self.state.update(cx, |s, cx| {
                s.remove_connection(conn);
                cx.notify();
            });
        }
        self.state.update(cx, |s, cx| {
            s.close_tab(id);
            cx.notify();
        });
    }

    fn paste(&mut self, conn: aish_types::ConnectionId, cx: &mut Context<Self>) {
        match arboard::Clipboard::new() {
            Err(e) => tracing::warn!("paste: arboard init failed: {e}"),
            Ok(mut cb) => {
                if let Ok(img) = cb.get_image() {
                    tracing::info!(
                        w = img.width,
                        h = img.height,
                        "paste: got image from clipboard"
                    );
                    match crate::terminal::image::encode_rgba_to_png(
                        img.width as u32,
                        img.height as u32,
                        &img.bytes,
                    ) {
                        Ok(png) => {
                            if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                                self.bridge.spawn(async move {
                                    let _ = sender
                                        .send(SessionCommand::UploadImage { data: png })
                                        .await;
                                });
                            }
                            return;
                        }
                        Err(e) => {
                            tracing::warn!("图片粘贴：PNG 编码失败：{}", e);
                            return;
                        }
                    }
                }
            }
        }

        use alacritty_terminal::term::TermMode;

        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        if text.is_empty() {
            return;
        }

        // 行结尾归一：\r\n → \r，独立 \n → \r，\r 保留
        let mut normalized = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => {
                    normalized.push('\r');
                    // \r\n → 吃掉后面的 \n
                    if matches!(chars.peek(), Some('\n')) {
                        chars.next();
                    }
                }
                '\n' => normalized.push('\r'),
                other => normalized.push(other),
            }
        }

        // 检测远端 bracketed paste mode
        let bracketed = self
            .state
            .read(cx)
            .term_of(conn)
            .map(|t| t.mode().contains(TermMode::BRACKETED_PASTE))
            .unwrap_or(false);

        let bytes = if bracketed {
            let mut out = Vec::with_capacity(normalized.len() + 12);
            out.extend_from_slice(b"\x1b[200~");
            out.extend_from_slice(normalized.as_bytes());
            out.extend_from_slice(b"\x1b[201~");
            out
        } else {
            normalized.into_bytes()
        };

        let sender = match self.state.read(cx).sessions.get(&conn).cloned() {
            Some(s) => s,
            None => return,
        };
        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }

    /// 根据当前字体 + 最近一次 canvas bounds 计算 GridLayout（含 8px padding）。
    /// 鼠标事件的 ev.position 是窗口绝对坐标 — 必须减 canvas.origin 才能落到
    /// grid 上。如果 canvas_bounds 还没就绪（首帧之前），回退到原点 + 8px padding。
    fn current_layout(&self, cx: &App) -> grid_renderer::GridLayout {
        let (cw, ch) = font::cell_size(cx);
        let (origin_x, origin_y) = match self.canvas_bounds {
            Some(b) => (b.origin.x + px(8.0), b.origin.y + px(8.0)),
            None => (px(8.0), px(8.0)),
        };
        grid_renderer::GridLayout {
            cell_width: cw,
            cell_height: ch,
            origin_x,
            origin_y,
        }
    }

    /// 处理鼠标左键按下。
    ///
    /// 两路分流：
    /// 1. alacritty Term 含 MOUSE_MODE（远端 tmux/vim 等开了 mouse） → 发 SGR
    ///    mouse press 字节给远端，不在本地起 selection（避免本地选区干扰
    ///    远端高亮）。
    /// 2. 否则 → 本地 alacritty Selection（原行为）。
    fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        // drag 期间（如 tab 拖拽 reorder）鼠标 hover 到终端区时，不该把鼠标
        // 事件转 SGR 发到远端 —— 用户是在操作 UI 而不是终端。同样 mouse_move
        // / mouse_up / scroll_wheel 入口都加此 guard。
        if cx.has_active_drag() {
            return;
        }
        use alacritty_terminal::term::TermMode;
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };

        let mode = self
            .state
            .read(cx)
            .host_pty_term
            .get(&conn)
            .map(|t| t.mode());

        // 远端 mouse mode：转发并 return
        if let Some(mode) = mode {
            if mode.intersects(TermMode::MOUSE_MODE) && mode.contains(TermMode::SGR_MOUSE) {
                if let Some(button) = sgr_mouse_button(ev.button) {
                    if let Some(bytes) =
                        self.build_sgr_mouse_bytes(ev.position, button, &ev.modifiers, true, cx)
                    {
                        let sender = self.state.read(cx).sessions.get(&conn).cloned();
                        if let Some(sender) = sender {
                            self.bridge.spawn(async move {
                                let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                            });
                        }
                        return;
                    }
                }
            }
        }

        // 本地 selection 路径（原行为）— 仅左键
        if !matches!(ev.button, MouseButton::Left) {
            return;
        }
        let (cols, rows) = self
            .state
            .read(cx)
            .host_pty_dimensions
            .get(&conn)
            .copied()
            .unwrap_or((crate::state::DEFAULT_COLS, crate::state::DEFAULT_ROWS));
        let layout = self.current_layout(cx);
        let (line, col, side) = crate::terminal::selection::pixel_to_grid(
            ev.position.x,
            ev.position.y,
            &layout,
            rows as usize,
            cols as usize,
        );
        self.state.update(cx, |state, cx| {
            if let Some(term) = state.host_pty_term.get_mut(&conn) {
                crate::terminal::selection::start_selection(term, line, col, side);
            }
            cx.notify();
        });
    }

    /// 处理鼠标抬起。仅在远端 mouse mode 时发 SGR release。
    fn handle_mouse_up(&mut self, ev: &MouseUpEvent, cx: &mut Context<Self>) {
        if cx.has_active_drag() {
            return;
        }
        use alacritty_terminal::term::TermMode;
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
        let mode = match self.state.read(cx).host_pty_term.get(&conn) {
            Some(t) => t.mode(),
            None => return,
        };
        if !mode.intersects(TermMode::MOUSE_MODE) || !mode.contains(TermMode::SGR_MOUSE) {
            return;
        }
        let button = match sgr_mouse_button(ev.button) {
            Some(b) => b,
            None => return,
        };
        if let Some(bytes) =
            self.build_sgr_mouse_bytes(ev.position, button, &ev.modifiers, false, cx)
        {
            let sender = self.state.read(cx).sessions.get(&conn).cloned();
            if let Some(sender) = sender {
                self.bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                });
            }
        }
    }

    /// 处理鼠标拖拽 / 移动。两路分流同 mouse_down。
    fn handle_mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if cx.has_active_drag() {
            return;
        }
        use alacritty_terminal::term::TermMode;
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };

        let mode = self
            .state
            .read(cx)
            .host_pty_term
            .get(&conn)
            .map(|t| t.mode());

        if let Some(mode) = mode {
            if mode.intersects(TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG)
                && mode.contains(TermMode::SGR_MOUSE)
            {
                // MOUSE_DRAG 仅当按住按钮时发；MOUSE_MOTION 任何 motion 都发。
                let pressed_button = ev.pressed_button;
                let any_pressed = pressed_button.is_some();
                if mode.contains(TermMode::MOUSE_DRAG) && !any_pressed {
                    // 仅 drag 模式下，未按键的 motion 不发
                } else {
                    let base: u8 = pressed_button
                        .and_then(sgr_mouse_button)
                        .unwrap_or(35) // NoneMove
                        + 32;
                    if let Some(bytes) =
                        self.build_sgr_mouse_bytes(ev.position, base, &ev.modifiers, true, cx)
                    {
                        let sender = self.state.read(cx).sessions.get(&conn).cloned();
                        if let Some(sender) = sender {
                            self.bridge.spawn(async move {
                                let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                            });
                        }
                        return;
                    }
                }
            }
        }

        // 本地 selection 更新（原行为）— 仅在拖拽时
        if !ev.dragging() {
            return;
        }
        let (cols, rows) = self
            .state
            .read(cx)
            .host_pty_dimensions
            .get(&conn)
            .copied()
            .unwrap_or((crate::state::DEFAULT_COLS, crate::state::DEFAULT_ROWS));
        let layout = self.current_layout(cx);
        let (line, col, side) = crate::terminal::selection::pixel_to_grid(
            ev.position.x,
            ev.position.y,
            &layout,
            rows as usize,
            cols as usize,
        );
        self.state.update(cx, |state, cx| {
            if let Some(term) = state.host_pty_term.get_mut(&conn) {
                crate::terminal::selection::update_selection(term, line, col, side);
            }
            cx.notify();
        });
    }

    /// 处理鼠标滚轮：滚动 alacritty Term 的 display offset 看 scrollback。
    ///
    /// alacritty `Scroll::Delta(n)`：n>0 = 把视口向上滚（看更老的内容），
    /// n<0 = 向下（回到当前 prompt）。
    ///
    /// GPUI 的 ScrollDelta:
    /// - `Lines(Point<f32>)`: 鼠标滚轮（每 tick 通常 1 行），y 正 = 向上
    /// - `Pixels(Point<Pixels>)`: 触摸板，按 cell_height 换算成行数
    ///
    /// 本地 alacritty grid 滚动**不改变** PTY size，远端不感知。
    fn handle_scroll(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        if cx.has_active_drag() {
            return;
        }
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
        let lines: i32 = match ev.delta {
            ScrollDelta::Lines(p) => p.y.round() as i32,
            ScrollDelta::Pixels(p) => {
                let (_, ch) = font::cell_size(cx);
                let ch = f32::from(ch).max(1.0);
                (f32::from(p.y) / ch).round() as i32
            }
        };
        if lines == 0 {
            return;
        }
        // 两条路径用不同步长：
        // - SGR mouse 路径（远端 tmux 等开 mouse on）：build_sgr_wheel_bytes
        //   按 scroll_amount **重复发 n 个 wheel event** 给 tmux，而
        //   tmux 每个 wheel event 内部又滚 5 行（默认 `bind WheelUpPane -N 5`），
        //   两层乘起来体感暴。改 SGR 路径每 tick 固定 ±1 event，让 tmux
        //   按自己配置决定步长（5 行/tick，接近主流终端体感）。
        // - 本地 alacritty 路径（raw shell scrollback）：tmux 不接管，用 ±3
        //   行/tick 合理步长。
        let sgr_amount = lines.signum();
        let local_amount = lines.signum() * 3;

        // 两条处理路径，按 alacritty Term mode 决定：
        // 1. MOUSE_MODE 启用（tmux `set -g mouse on` / vim `mouse=a` 等开了
        //    远端鼠标支持）→ 发 SGR mouse wheel escape 给 PTY，远端处理：
        //    tmux 进 copy mode 翻 scrollback / vim 翻 buffer / less 上滚。
        // 2. 否则 → 本地 alacritty scroll_display（raw shell 模式有效；
        //    tmux alt screen 没 history 也无效，需要用户开 mouse on）。
        // 不走 ALTERNATE_SCROLL（发裸方向键）路径 — 在 mouse off 时会把方向
        // 键传给 TUI 应用搞乱屏幕。
        use alacritty_terminal::term::TermMode;

        let term_mode = self
            .state
            .read(cx)
            .host_pty_term
            .get(&conn)
            .map(|t| t.mode());

        if let Some(mode) = term_mode {
            if mode.intersects(TermMode::MOUSE_MODE) {
                // SGR mouse wheel: 发字节给远端
                if let Some(bytes) = self.build_sgr_wheel_bytes(ev, sgr_amount, *mode, cx) {
                    let sender = self.state.read(cx).sessions.get(&conn).cloned();
                    if let Some(sender) = sender {
                        self.bridge.spawn(async move {
                            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                        });
                    }
                    return;
                }
            }
        }

        // 本地 alacritty scroll
        self.state.update(cx, |state, cx| {
            if let Some(term) = state.host_pty_term.get_mut(&conn) {
                term.scroll_display(alacritty_terminal::grid::Scroll::Delta(local_amount));
            }
            cx.notify();
        });
    }

    /// 把 wheel 事件 + 行数翻成 SGR mouse escape 字节。
    ///
    /// 仅当 alacritty Term 已宣告 MOUSE_MODE 且 SGR_MOUSE 时调用。每行发一个
    /// button 64/65（wheel up/down）press 事件。
    fn build_sgr_wheel_bytes(
        &self,
        ev: &ScrollWheelEvent,
        scroll_amount: i32,
        mode: alacritty_terminal::term::TermMode,
        cx: &App,
    ) -> Option<Vec<u8>> {
        use alacritty_terminal::term::TermMode;
        if !mode.contains(TermMode::SGR_MOUSE) {
            return None;
        }
        let button = if scroll_amount > 0 { 64u8 } else { 65 };
        let n = scroll_amount.unsigned_abs() as usize;
        let single = self.build_sgr_mouse_bytes(ev.position, button, &ev.modifiers, true, cx)?;
        let mut out = Vec::with_capacity(n * single.len());
        for _ in 0..n {
            out.extend_from_slice(&single);
        }
        Some(out)
    }

    /// 把 (position, button, modifiers, pressed) 翻成 SGR mouse escape 字节。
    ///
    /// 格式：`\x1b[<{button + mods};{col};{row}{M|m}` —— `M` = press / motion，
    /// `m` = release。col/row 1-based grid 坐标。modifiers 影响 button 高位：
    /// shift+4, alt+8, ctrl+16。
    fn build_sgr_mouse_bytes(
        &self,
        position: Point<Pixels>,
        button: u8,
        modifiers: &Modifiers,
        pressed: bool,
        cx: &App,
    ) -> Option<Vec<u8>> {
        let layout = self.current_layout(cx);
        let local_x = f32::from(position.x) - f32::from(layout.origin_x);
        let local_y = f32::from(position.y) - f32::from(layout.origin_y);
        if local_x < 0.0 || local_y < 0.0 {
            return None;
        }
        let col = ((local_x / f32::from(layout.cell_width)).floor() as i32 + 1).max(1);
        let row = ((local_y / f32::from(layout.cell_height)).floor() as i32 + 1).max(1);
        let mut mods: u8 = 0;
        if modifiers.shift {
            mods += 4;
        }
        if modifiers.alt {
            mods += 8;
        }
        if modifiers.control {
            mods += 16;
        }
        let suffix = if pressed { 'M' } else { 'm' };
        Some(format!("\x1b[<{};{};{}{}", button + mods, col, row, suffix).into_bytes())
    }

    /// 检测 bounds 变化，算新 cols/rows，若有变化则 debounce 100ms 后触发 resize。
    /// 同时把 bounds 缓存到 `self.canvas_bounds` 供 mouse handler 用于 grid 坐标换算。
    ///
    /// 在 canvas prepaint 的下一帧回调中调用（通过 window.on_next_frame）。
    /// 用户在 Disconnected 状态点击"重新连接"按钮时调用：
    /// 在同一 ConnectionId 上 spawn 新 actor，phase 回到 Connecting。
    /// 终端 scrollback / tab 等不重置（用户能继续看到断开前的输出）。
    fn handle_reconnect(&mut self, cx: &mut Context<Self>) {
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
        let cfg = {
            let app = self.state.read(cx);
            app.connections
                .get(&conn)
                .map(|c| c.host_id)
                .and_then(|hid| app.hosts.iter().find(|h| h.id == hid).cloned())
        };
        let cfg = match cfg {
            Some(c) => c,
            None => {
                tracing::warn!(?conn, "reconnect: host config 已不存在");
                return;
            }
        };
        let sender = self.bridge.spawn_session(conn, cfg, self.tx.clone());
        self.state.update(cx, |s, cx| {
            s.reopen_connection(conn, sender);
            cx.notify();
        });
    }

    fn check_resize(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        self.canvas_bounds = Some(bounds);

        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
        if !self.state.read(cx).is_session_active(conn) {
            return;
        }

        // 算新 cols/rows（与 paint_terminal 中 origin 偏移一致：8px padding）
        let (cw, ch) = font::cell_size(cx);
        let cw = f32::from(cw);
        let ch = f32::from(ch);
        if cw <= 0.0 || ch <= 0.0 {
            return;
        }
        let w = f32::from(bounds.size.width);
        let h = f32::from(bounds.size.height);
        let cols = (((w - 16.0) / cw).floor()).max(1.0) as u16;
        let rows = (((h - 16.0) / ch).floor()).max(1.0) as u16;

        // last_pty_size 是 per-view 字段，当前 view 切换 connection 时也得重新触发；
        // 把 conn 也带进 cache key，避免切到另一连接还以为没变化跳过 resize。
        if Some((conn, cols, rows)) == self.last_pty_size {
            return;
        }
        self.last_pty_size = Some((conn, cols, rows));

        // drop 旧 pending task（取消上次 debounce）
        self.pending_resize = None;

        let state = self.state.clone();
        let bridge = self.bridge.clone();

        // 启动 debounce task，存储在 self.pending_resize — drop 即取消
        //
        // 4 段流水：
        //   1. PTY_RESIZE_DEBOUNCE_MS — 等拖动稳定
        //   2. SessionCommand::Resize → 远端 PTY SIGWINCH
        //   3. LOCAL_TERM_RESIZE_DELAY_MS — 等远端 RTT 落地
        //   4. state.resize_term — 本地 alacritty Term 按新 size 排
        //
        // 顺序为何"先远端再本地"：本地 Term 立即按新 size 排会让远端用旧
        // size 算的 ANSI 在新坐标系里错位（光标定位 / pane 边框等）。让远端
        // 先按新 size 重排吐字节，本地再切到新 size，可避开错位窗口。
        let task = cx.spawn(async move |_this, cx| {
            // (1) debounce
            cx.background_executor()
                .timer(Duration::from_millis(PTY_RESIZE_DEBOUNCE_MS))
                .await;

            // (2) 先发 SIGWINCH 给远端 PTY
            let sender_opt = cx.update(|app| {
                state.update(app, |app_state, _cx| app_state.sessions.get(&conn).cloned())
            });
            if let Some(sender) = sender_opt {
                bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Resize { cols, rows }).await;
                });
            }

            // (3) 等远端约一个 RTT 让 SIGWINCH 落地、tmux 重排
            cx.background_executor()
                .timer(Duration::from_millis(LOCAL_TERM_RESIZE_DELAY_MS))
                .await;

            // (4) 本地 Term 跟随新 size 并通知 UI 重绘
            cx.update(|app| {
                state.update(app, |app_state, cx| {
                    app_state.resize_term(conn, cols, rows);
                    cx.notify();
                })
            });
        });
        self.pending_resize = Some(task);
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let conn = self.state.read(cx).current_connection();
        // cursor_state.focused 每帧按当前 focus 状态算 — self.cursor_state 内的
        // focused 是构造时初始值（true），从不更新。Render 内拿 focus_handle
        // 的 is_focused 写入派生 cursor_state，paint_cursor 据此分实心 / 空心
        // + 是否闪烁。epoch（blink 相位）保留 self 的不每帧 reset。
        //
        // 双重检查：
        // - is_focused(window)：element 在窗口内是否获焦（切到 InputBar / 别 tab 会 false）
        // - window.is_window_active()：OS 窗口是否激活（切到别 App 会 false）
        // 任一 false 都算失焦，paint 走静态空心不闪。与 TextInput 同语义。
        let focused_now = self.focus_handle.is_focused(window) && window.is_window_active();
        let cursor_state = CursorState {
            focused: focused_now,
            ..self.cursor_state
        };
        let state_entity = self.state.clone();

        // 当前 conn 的 phase + label，用于渲染 connecting loading / disconnected
        // reconnect overlay（Connected 状态不加 overlay，让终端正常显示）
        let phase_overlay: Option<gpui::AnyElement> = conn.and_then(|c| {
            let app = self.state.read(cx);
            let phase = app.connection_phases.get(&c).cloned();
            let label = app.connections.get(&c).map(|x| x.label.clone());
            let colors = aish_ui::theme(cx).colors;
            let fs = aish_ui::theme(cx).font_size;
            match phase {
                Some(ConnectionPhase::Connecting) => Some(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .bg(gpui::rgba(0x000000cc)) // 半透明黑遮罩
                        .child(
                            div()
                                .text_size(fs.lg)
                                .text_color(colors.foreground)
                                .child(format!(
                                    "正在连接 {} …",
                                    label.clone().unwrap_or_default()
                                )),
                        )
                        .child(
                            div()
                                .text_size(fs.sm)
                                .text_color(colors.muted_foreground)
                                .child("SSH 握手 + PTY 初始化 + tmux/OS 探测"),
                        )
                        .into_any_element(),
                ),
                Some(ConnectionPhase::Disconnected { reason }) => Some(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .px_4()
                        .py_3()
                        .bg(colors.card)
                        .border_t_1()
                        .border_color(colors.border)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_size(fs.sm)
                                        .text_color(colors.foreground)
                                        .child("连接已断开"),
                                )
                                .child(
                                    div()
                                        .text_size(fs.xs)
                                        .text_color(colors.muted_foreground)
                                        .child(reason),
                                ),
                        )
                        .child(self.reconnect_btn.clone())
                        .into_any_element(),
                ),
                _ => None,
            }
        });

        // 拿当前 view 的弱引用，用于在 on_next_frame 回调中调用 check_resize
        let weak_view = cx.weak_entity();

        // IME 输入处理器所需资源：在 canvas paint 阶段调用 window.handle_input
        let focus_for_ime = self.focus_handle.clone();
        let state_for_ime = self.state.clone();
        let bridge_for_ime = self.bridge.clone();

        // 计算当前光标的屏幕坐标，用于定位 IME 候选窗口
        let cursor_bounds_for_ime: Option<gpui::Bounds<gpui::Pixels>> = {
            let app = self.state.read(cx);
            let conn = app.current_connection();
            let term_opt = conn.and_then(|c| app.host_pty_term.get(&c));
            let canvas_opt = self.canvas_bounds;
            if let (Some(term), Some(canvas)) = (term_opt, canvas_opt) {
                let cursor = term.grid().cursor.point;
                let display_offset = term.grid().display_offset() as i32;
                let screen_row = cursor.line.0 + display_offset;
                if screen_row >= 0 {
                    let (cw, ch) = font::cell_size(cx);
                    let origin_x = canvas.origin.x + px(8.0);
                    let origin_y = canvas.origin.y + px(8.0);
                    Some(gpui::Bounds::new(
                        gpui::Point::new(
                            origin_x + cw * cursor.column.0 as f32,
                            origin_y + ch * screen_row as f32,
                        ),
                        gpui::Size::new(cw, ch),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        };

        div()
            // relative 让 phase overlay 的 absolute 子元素相对终端区定位
            .relative()
            // 让 div 变 stateful — GPUI 对 stateful 元素的 scroll wheel 事件路由
            // 比 stateless 稳定（mouse_down 用 is_hovered 路径不依赖 stateful，
            // 但 scroll_wheel 走 should_handle_scroll → mouse_hit_test.ids，需要
            // hitbox 已注册到 next_frame 的 hit test 索引中）。
            .id("terminal-pane")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            // 监听三个鼠标键：左 / 中 / 右。handle_mouse_down 内部按 alacritty
            // mode 决定走 SGR 转发还是本地 selection。
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                    this.handle_mouse_down(ev, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                    this.handle_mouse_down(ev, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                    this.handle_mouse_down(ev, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseUpEvent, _window, cx| {
                    this.handle_mouse_up(ev, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, ev: &MouseUpEvent, _window, cx| {
                    this.handle_mouse_up(ev, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, ev: &MouseUpEvent, _window, cx| {
                    this.handle_mouse_up(ev, cx);
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                this.handle_scroll(ev, cx);
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                this.handle_mouse_move(ev, cx);
            }))
            .flex_1()
            .h_full()
            // 终端区 bg：dark 用纯黑 / light 用纯白（colors::default_background_for）。
            // 跟着 theme kind 走，与 ANSI palette default bg fallback 完全一致 ——
            // 当远端文字用 NamedColor::Background 时 paint_grid 取到相同 hex。
            .bg(rgb(crate::terminal::colors::default_background_for(
                aish_ui::theme(cx).kind,
            )))
            .child(
                canvas(
                    move |bounds: Bounds<Pixels>, window, cx| {
                        // 在下一帧通过 WeakEntity 安全地更新 TerminalView 触发 resize 检测。
                        // 不能在 prepaint 里直接调用 view.update，因为 render 调用链持有 &mut TerminalView；
                        // on_next_frame 在 render/prepaint 阶段结束后执行，此时借用已释放。
                        let weak = weak_view.clone();
                        window.on_next_frame(move |_window, cx| {
                            let _ = weak.update(cx, |view, cx| {
                                view.check_resize(bounds, cx);
                            });
                        });

                        // prepaint：从 Term 提取快照（读借用在这里完成，不影响 paint 阶段）
                        take_snapshot(conn, &state_entity, cx)
                    },
                    move |bounds: Bounds<Pixels>, snapshot, window, cx| {
                        if let Some(snapshot) = snapshot {
                            paint_terminal(&snapshot, &cursor_state, bounds, window, cx);
                        }
                        // 注册 IME 输入处理器：每帧 paint 阶段调用，让 GPUI Windows 平台层
                        // 把 WM_CHAR / WM_IME_COMPOSITION 路由到 replace_text_in_range，
                        // 从而支持中文等 IME 输入法输入。
                        window.handle_input(
                            &focus_for_ime,
                            TerminalImeHandler {
                                state: state_for_ime,
                                bridge: bridge_for_ime,
                                cursor_bounds: cursor_bounds_for_ime,
                            },
                            cx,
                        );
                    },
                )
                .size_full(),
            )
            .children(phase_overlay)
    }
}

/// 把 GPUI 的 MouseButton 翻成 SGR mouse 协议的 button code。
/// 参考 alacritty / iTerm2 的标准约定。Navigate 按键（前进 / 后退）映射成
/// "Other"（99），调用方应过滤掉不发。
fn sgr_mouse_button(b: MouseButton) -> Option<u8> {
    match b {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Navigate(_) => None,
    }
}

/// 决定 terminal 显示哪个 Term。
///
/// raw attach 路径下不再区分 tmux/non-tmux —— 整条 PTY 字节流都喂给
/// `host_pty_term`，tmux 自身画状态栏/窗口列表/pane 边框，alacritty Term 当
/// 一块大画布即可。M3-archived：之前 -CC 模式的 per-pane Term 已废。
pub(crate) fn term_for_render(
    app: &AppState,
    conn: aish_types::ConnectionId,
) -> Option<&alacritty_terminal::Term<crate::state::TitleListener>> {
    app.host_pty_term.get(&conn)
}

/// 在 prepaint 阶段读取 Term grid 快照（读借用安全）。
fn take_snapshot(
    conn: Option<aish_types::ConnectionId>,
    state: &Entity<AppState>,
    cx: &mut App,
) -> Option<GridSnapshot> {
    let conn = conn?;
    let app_state = state.read(cx);
    let term = term_for_render(app_state, conn)?;
    Some(GridSnapshot::from_term(term))
}

/// 在 paint 阶段使用快照渲染终端（此时没有 cx 读借用冲突）。
fn paint_terminal(
    snapshot: &GridSnapshot,
    cursor_state: &CursorState,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let (cell_width, cell_height) = font::cell_size(cx);
    let layout = GridLayout {
        cell_width,
        cell_height,
        origin_x: bounds.origin.x + px(8.0),
        origin_y: bounds.origin.y + px(8.0),
    };

    paint_grid(snapshot, &layout, window, cx);
    paint_cursor(snapshot, cursor_state, &layout, window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use aish_types::{ConnectionId, HostId};

    fn mk_state_with_host() -> (AppState, HostId) {
        let cfg = aish_types::HostConfig {
            id: HostId::new(),
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: aish_types::SshAuth::KeyFile {
                path: std::path::PathBuf::from("/tmp/k"),
            },
            env_profile: None,
            os_kind: None,
        };
        let id = cfg.id;
        (AppState::with_hosts(vec![cfg]), id)
    }

    #[test]
    fn term_for_render_returns_host_pty() {
        let (mut state, host_id) = mk_state_with_host();
        let conn = state.open_connection(host_id);
        state.feed_bytes(conn, b"x");
        let term = term_for_render(&state, conn);
        assert!(term.is_some());
    }

    #[test]
    fn term_for_render_returns_none_for_unknown_conn() {
        let (state, _host_id) = mk_state_with_host();
        let unknown = ConnectionId::new();
        let term = term_for_render(&state, unknown);
        assert!(term.is_none());
    }
}
