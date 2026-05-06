//! 主区终端视图。M2b1 Task 4 — 自绘 alacritty grid + 颜色 + 光标闪烁。

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    canvas, div, prelude::*, px, rgb, App, Bounds, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, Pixels, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};
use crate::terminal::{
    cursor::{paint_cursor, CursorState},
    font,
    grid_renderer::{paint_grid, GridLayout, GridSnapshot},
};

pub struct TerminalView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
    cursor_state: CursorState,
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
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let host = match self.state.read(cx).selected {
            Some(h) => h,
            None => return,
        };
        let sender = match self.state.read(cx).sessions.get(&host).cloned() {
            Some(s) => s,
            None => return,
        };

        let ctrl = event.keystroke.modifiers.control;
        let alt = event.keystroke.modifiers.alt;
        let key = event.keystroke.key.as_str();

        let bytes = encode_key(key, ctrl, alt);
        if bytes.is_empty() {
            return;
        }

        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.state.read(cx).selected;
        let cursor_state = self.cursor_state;
        let state_entity = self.state.clone();

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .flex_1()
            .h_full()
            .bg(rgb(0x1d1f21))
            .child(
                canvas(
                    move |_bounds, _window, cx| {
                        // prepaint：从 Term 提取快照（读借用在这里完成，不影响 paint 阶段）
                        take_snapshot(host, &state_entity, cx)
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

/// 在 prepaint 阶段读取 Term grid 快照（读借用安全）。
fn take_snapshot(
    host: Option<aish_types::HostId>,
    state: &Entity<AppState>,
    cx: &mut App,
) -> Option<GridSnapshot> {
    let host = host?;
    let app_state = state.read(cx);
    let term = app_state.term_of(host)?;
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
