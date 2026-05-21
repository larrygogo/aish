//! ContextMenu — 右键 cursor-position 浮层。
//!
//! 与 `Popover` 的区别：
//! - Popover 锚定到 trigger 元素 bounds（上/下/左/右某侧）
//! - ContextMenu 锚定到 cursor 的 viewport 坐标（任意点）
//!
//! 典型用法：
//! ```ignore
//! // View 内持有 menu：
//! let menu = cx.new(ContextMenu::new);
//!
//! // 每帧 render 内更新 content（content: AnyElement 不可 Clone，必须每帧重设）：
//! menu.update(cx, |m, _| m.content(build_menu_content()));
//!
//! // trigger 元素右键打开 menu：
//! .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev, _w, cx| {
//!     this.menu.update(cx, |m, cx| m.open_at(ev.position, cx));
//! }))
//!
//! // view tree 恒挂载 menu entity，open 时才 render 浮层：
//! .child(this.menu.clone())
//! ```
//!
//! 关闭：Esc / 左键点 backdrop / caller 主动 close。

use std::rc::Rc;

use gpui::{
    anchored, div, prelude::*, Anchor, AnchoredPositionMode, AnyElement, App, Context, FocusHandle,
    Focusable, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, Window,
};

use crate::theme::theme;

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App) + 'static>;

pub struct ContextMenu {
    focus_handle: FocusHandle,
    open: bool,
    needs_focus: bool,
    content: Option<AnyElement>,
    position: Option<Point<Pixels>>,
    on_close: Option<CloseHandler>,
    /// caller 注册的额外 key handler。Esc 处理之后调用，让 caller 实现
    /// ↑/↓/Enter 等列表导航键位（典型用 DropdownMenu.selected_idx 高亮 +
    /// Enter 触发 on_select）。
    on_key: Option<KeyHandler>,
}

impl ContextMenu {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            open: false,
            needs_focus: false,
            content: None,
            position: None,
            on_close: None,
            on_key: None,
        }
    }

    /// 设置浮层内容。**每帧必须重设** —— `AnyElement` 不 Clone，render 内 take 消耗。
    pub fn content(&mut self, c: impl IntoElement) -> &mut Self {
        self.content = Some(c.into_any_element());
        self
    }

    pub fn on_close(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self {
        self.on_close = Some(Rc::new(h));
        self
    }

    /// 注册额外 key handler。Esc 处理之后调用，让 caller 实现 ↑/↓/Enter 等
    /// 列表导航键位。同 Dialog::on_key 设计。
    pub fn on_key(
        &mut self,
        h: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_key = Some(Rc::new(h));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 在指定 viewport 坐标打开 menu。caller 在 right-click handler 内调用，
    /// 传入 `MouseDownEvent::position` 即可（已是 viewport 坐标）。
    pub fn open_at(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) {
        self.position = Some(point);
        self.open = true;
        self.needs_focus = true;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    fn fire_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_close.clone() {
            h(window, cx);
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() == "escape" {
            self.close(cx);
            self.fire_close(window, cx);
            return;
        }
        // caller 自定义 key 路径（↑/↓/Enter 等）。Esc 之后调用：Esc 由
        // ContextMenu 统一处理，caller 不能覆盖。
        if let Some(h) = self.on_key.clone() {
            h(event, window, cx);
        }
    }
}

impl Focusable for ContextMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let Some(position) = self.position else {
            return div().into_any_element();
        };
        if self.needs_focus {
            self.focus_handle.focus(window, cx);
            self.needs_focus = false;
        }

        let content = self.content.take();
        let t = theme(cx);
        let border_color = t.colors.border;
        let popover_bg = t.colors.popover;
        let radius_md = t.radius.md;

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            // occlude 整窗吸收所有底层 mouse 事件，确保左键点 backdrop 关 menu
            // 且不会同时触发底层 view 的 click（如 Card.on_click）
            .occlude()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                    this.close(cx);
                    this.fire_close(window, cx);
                }),
            )
            // 右键点空白也关 menu（用户期望右键 = 再次操作 / 取消）
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                    this.close(cx);
                    this.fire_close(window, cx);
                }),
            )
            .child(
                anchored()
                    .position_mode(AnchoredPositionMode::Window)
                    .position(position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window()
                    .child(
                        div()
                            .bg(popover_bg)
                            .rounded(radius_md)
                            .border_1()
                            .border_color(border_color)
                            // M24 elevation-2 — context menu 中层浮起
                            .shadow(crate::theme::elevation_2(t.kind))
                            // 阻止冒泡到 backdrop 关闭：用户点 menu item 应该是
                            // 触发 item callback，不该被 backdrop 抢先关 menu
                            .on_mouse_down(MouseButton::Left, |_ev, _w, cx| {
                                cx.stop_propagation();
                            })
                            .when_some(content, |d, c| d.child(c)),
                    ),
            )
            .into_any_element()
    }
}

// 行为级测试需要 GPUI Context 注入，超出单测范围；省略 stub 测试。
// 集成验证走 tab_bar / host_list 等 caller 实际接入测试。
