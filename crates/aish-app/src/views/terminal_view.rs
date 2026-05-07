//! 主区终端视图。M2b1 Task 4 — 自绘 alacritty grid + 颜色 + 光标闪烁。
//! M2b1 Task 5 — PTY 跟随窗口 resize（100ms debounce）。

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    canvas, div, prelude::*, px, rgb, App, Bounds, ClipboardItem, Context, Entity, FocusHandle,
    Focusable, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, ScrollDelta,
    ScrollWheelEvent, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};
use crate::terminal::{
    cursor::{paint_cursor, CursorState},
    font,
    grid_renderer::{self, paint_grid, GridLayout, GridSnapshot},
};

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

        Self {
            state,
            bridge,
            tx,
            focus_handle,
            cursor_state,
            last_pty_size: None,
            pending_resize: None,
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

    /// 根据当前字体计算 GridLayout（固定 8px padding）。
    fn current_layout(&self, cx: &App) -> grid_renderer::GridLayout {
        let (cw, ch) = font::cell_size(cx);
        grid_renderer::GridLayout {
            cell_width: cw,
            cell_height: ch,
            origin_x: px(8.0),
            origin_y: px(8.0),
        }
    }

    /// 处理鼠标左键按下：开始 selection。
    fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
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

    /// 处理鼠标拖拽：更新 selection 末端。
    fn handle_mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => return,
        };
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
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => {
                tracing::info!("scroll: no current_connection, ignoring");
                return;
            }
        };
        let lines: i32 = match ev.delta {
            ScrollDelta::Lines(p) => p.y.round() as i32,
            ScrollDelta::Pixels(p) => {
                let (_, ch) = font::cell_size(cx);
                let ch = f32::from(ch).max(1.0);
                (f32::from(p.y) / ch).round() as i32
            }
        };
        tracing::info!(?ev.delta, lines, "scroll: wheel event");
        if lines == 0 {
            return;
        }
        let scroll_amount = lines * 3;
        self.state.update(cx, |state, cx| {
            if let Some(term) = state.host_pty_term.get_mut(&conn) {
                term.scroll_display(alacritty_terminal::grid::Scroll::Delta(scroll_amount));
                tracing::info!(delta = scroll_amount, "scroll: alacritty scrolled");
            }
            cx.notify();
        });
    }

    /// 检测 bounds 变化，算新 cols/rows，若有变化则 debounce 100ms 后触发 resize。
    ///
    /// 在 canvas prepaint 的下一帧回调中调用（通过 window.on_next_frame）。
    fn check_resize(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
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

        // 启动 100ms debounce task，存储在 self.pending_resize — drop 即取消
        let task = cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;

            // resize alacritty Term 并通知 UI 重绘；
            // cx 是 &mut AsyncApp，通过 cx.update 拿 &mut App 来更新 state entity
            let sender_opt = cx.update(|app| {
                state.update(app, |app_state, cx| {
                    app_state.resize_term(conn, cols, rows);
                    cx.notify();
                    app_state.sessions.get(&conn).cloned()
                })
            });

            // 通知远端 PTY 执行 window_change（SIGWINCH）
            if let Some(sender) = sender_opt {
                bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Resize { cols, rows }).await;
                });
            }
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let conn = self.state.read(cx).current_connection();
        let cursor_state = self.cursor_state;
        let state_entity = self.state.clone();

        // 拿当前 view 的弱引用，用于在 on_next_frame 回调中调用 check_resize
        let weak_view = cx.weak_entity();

        div()
            // 让 div 变 stateful — GPUI 对 stateful 元素的 scroll wheel 事件路由
            // 比 stateless 稳定（mouse_down 用 is_hovered 路径不依赖 stateful，
            // 但 scroll_wheel 走 should_handle_scroll → mouse_hit_test.ids，需要
            // hitbox 已注册到 next_frame 的 hit test 索引中）。
            .id("terminal-pane")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                    this.handle_mouse_down(ev, cx);
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                this.handle_scroll(ev, cx);
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                if ev.dragging() {
                    this.handle_mouse_move(ev, cx);
                }
            }))
            .flex_1()
            .h_full()
            .bg(rgb(0x1d1f21))
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
                    },
                )
                .size_full(),
            )
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
) -> Option<&alacritty_terminal::Term<alacritty_terminal::event::VoidListener>> {
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
