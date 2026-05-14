//! ScrollPage — 整页 scroll wrapper：viewport 内部 overflow scroll +
//! 自带 wheel handler + 内置 thumb scrollbar（可拖拽）。
//!
//! ## 用法
//!
//! 简单页面（内容很可能不溢出）— 不传 scrollbar handle，走 GPUI 内置
//! overflow_y_scroll：
//!
//! ```ignore
//! ScrollPage::new("settings").bg(...).px(...).py(...).child(...)
//! ```
//!
//! 真正可能溢出的页面（推荐）— caller 持 ScrollbarHandle 字段传入，
//! ScrollPage 自动 wheel + scrollbar + thumb 拖拽：
//!
//! ```ignore
//! pub struct MyView { scrollbar: ScrollbarHandle }
//!
//! ScrollPage::new("my-scroll")
//!     .scrollbar(&self.scrollbar)
//!     .flex_1()  // caller 父必须 .flex().flex_col()
//!     .child(...)
//! ```
//!
//! ## CSS 三栏布局必知
//!
//! flex item 默认 `min-width: auto / min-height: auto` 拒绝 shrink —
//! children 撑大时 item 跟着撑大，下游 scroll 容器拿到的 bounds 跟着膨胀
//! 让 scroll_max = 0 滚动失效。RootView 那层每个 flex_1 wrapper **都要**
//! 加 `min_w(0)` / `min_h(0)`，scroll 容器才能严格 fit viewport。

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, App, DispatchPhase, ElementId, Hsla, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollHandle,
    ScrollWheelEvent, Window,
};

use crate::theme::theme;

type WheelHandler = Rc<dyn Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static>;

/// 包 ScrollHandle + thumb drag 状态。caller 在 view struct 持一个字段，
/// ScrollPage 通过 builder 拿引用接管 wheel / scrollbar / 拖拽。
///
/// drag 状态只需一个 `dragging` flag —— mouse_move 内每次按当前鼠标 y 直接
/// 算 thumb 中心位置 → ratio → set_offset，不需要记 start offset 累积差量。
///
/// `Default` 实现新建空 handle，`Clone` 是 Rc 浅克隆共享底层 state。
#[derive(Clone, Default)]
pub struct ScrollbarHandle {
    scroll: ScrollHandle,
    dragging: Rc<Cell<bool>>,
}

impl ScrollbarHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// 暴露内部 ScrollHandle 让 caller 在需要时用 GPUI 原生 API
    /// （`.offset()` / `.max_offset()` / `.set_offset(...)`）。
    pub fn scroll_handle(&self) -> &ScrollHandle {
        &self.scroll
    }
}

#[derive(IntoElement)]
pub struct ScrollPage {
    id: ElementId,
    bg: Option<Hsla>,
    px: Option<Pixels>,
    py: Option<Pixels>,
    scrollbar: Option<ScrollbarHandle>,
    on_wheel: Option<WheelHandler>,
    flex_1: bool,
    children: Vec<AnyElement>,
}

impl ScrollPage {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            bg: None,
            px: None,
            py: None,
            scrollbar: None,
            on_wheel: None,
            flex_1: false,
            children: Vec::new(),
        }
    }

    pub fn bg(mut self, color: Hsla) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn px(mut self, p: Pixels) -> Self {
        self.px = Some(p);
        self
    }

    pub fn py(mut self, p: Pixels) -> Self {
        self.py = Some(p);
        self
    }

    /// 启用 flex_1 + min_h(0) 模式（caller 父必须 flex_col）。
    /// flex_1 严格 fit remaining height，min_h(0) 强制允许 shrink 不被
    /// children 撑大 — scroll_max > 0 滚动真生效。
    pub fn flex_1(mut self) -> Self {
        self.flex_1 = true;
        self
    }

    /// 关联 caller 持有的 ScrollbarHandle —— 启用后 ScrollPage 自动：
    /// - .track_scroll(handle) 让 children 按 offset transform paint
    /// - .overflow_hidden 让 GPUI 内置 wheel 不与 caller wheel 冲突
    /// - on_scroll_wheel 自带 wheel handler（60px/tick set_offset）
    /// - render scrollbar thumb（仅 max_offset.y > 0 时）+ thumb 可拖拽
    pub fn scrollbar(mut self, handle: &ScrollbarHandle) -> Self {
        self.scrollbar = Some(handle.clone());
        self
    }

    /// override 内部默认 wheel handler — caller 想自己定 step / 方向 / 边界
    /// 行为时用。一般不用，默认 60px/tick 与 tab_bar / textarea 一致。
    pub fn on_wheel(
        mut self,
        h: impl Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_wheel = Some(Rc::new(h));
        self
    }
}

impl ParentElement for ScrollPage {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// wheel sign helper — Pixels / Lines 都归到 -1 / 0 / 1，防高 DPI 鼠标
/// 单 tick 跨半屏。与 tab_bar.handle_wheel 同模式。
fn wheel_sign_y(delta: ScrollDelta) -> f32 {
    match delta {
        ScrollDelta::Pixels(p) => {
            if p.y > px(0.0) {
                1.0
            } else if p.y < px(0.0) {
                -1.0
            } else {
                0.0
            }
        }
        ScrollDelta::Lines(l) => {
            if l.y > 0.0 {
                1.0
            } else if l.y < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
    }
}

/// 默认 wheel handler — 60px/tick 滚 ScrollHandle.offset.y，clamp 到
/// [-max_offset.y, 0]。返回 true 表示有变化（调用方 refresh）。
fn default_wheel_step_y(handle: &ScrollHandle, ev: &ScrollWheelEvent) -> bool {
    let sign = wheel_sign_y(ev.delta);
    if sign == 0.0 {
        return false;
    }
    let step = px(60.0 * sign);
    let cur = handle.offset();
    let max = handle.max_offset();
    let new_y = (cur.y + step).clamp(-max.y, px(0.0));
    if new_y == cur.y {
        return false;
    }
    handle.set_offset(Point::new(cur.x, new_y));
    true
}

/// drag 中根据当前鼠标 y 反推 scroll offset：让 thumb 中心跟随鼠标 y。
/// 算法：mouse_y_in_viewport = clamp(mouse.y - viewport.top, 0, viewport_h)
/// thumb_top = mouse_y_in_viewport - thumb_h / 2，反推 ratio。
fn drag_to_offset_y(handle: &ScrollHandle, mouse_y: Pixels) -> bool {
    let vp = handle.bounds();
    let viewport_h_f = f32::from(vp.size.height);
    let max_y_f = f32::from(handle.max_offset().y);
    if viewport_h_f <= 0.0 || max_y_f <= 0.0 {
        return false;
    }
    let content_h_f = viewport_h_f + max_y_f;
    let thumb_h_f = (viewport_h_f * viewport_h_f / content_h_f).max(20.0);
    let track_h_f = viewport_h_f - thumb_h_f;
    if track_h_f <= 0.0 {
        return false;
    }
    let mouse_y_in_vp = f32::from(mouse_y) - f32::from(vp.origin.y);
    let thumb_top_f = (mouse_y_in_vp - thumb_h_f / 2.0).clamp(0.0, track_h_f);
    let ratio = thumb_top_f / track_h_f;
    let new_y = px(-ratio * max_y_f);
    let cur = handle.offset();
    if new_y == cur.y {
        return false;
    }
    handle.set_offset(Point::new(cur.x, new_y));
    true
}

impl RenderOnce for ScrollPage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted_fg = theme(cx).colors.muted_foreground;

        // scrollbar thumb + 拖拽 listener：仅 scrollbar 模式 + max_offset.y > 0
        // 时画。首帧 ScrollHandle.bounds() 还是空，自动不画 thumb（max_offset
        // 也是 0），下一帧 paint 完 layout 后 bounds 就位。
        let scrollbar_overlay = self.scrollbar.as_ref().and_then(|sb| {
            let h = &sb.scroll;
            let vp = h.bounds();
            let viewport_h = vp.size.height;
            let max_y = h.max_offset().y;
            let cur_y = h.offset().y;
            if max_y <= px(0.0) || viewport_h <= px(0.0) {
                return None;
            }
            let viewport_h_f = f32::from(viewport_h);
            let max_y_f = f32::from(max_y);
            let content_h_f = viewport_h_f + max_y_f;
            let thumb_h_f = (viewport_h_f * viewport_h_f / content_h_f).max(20.0);
            let thumb_h = px(thumb_h_f);
            let ratio = (-f32::from(cur_y) / max_y_f).clamp(0.0, 1.0);
            let thumb_top = px((viewport_h_f - thumb_h_f) * ratio);

            // mouse_down：thumb 启拖拽 + 立即 jump 到点击位置
            let scroll_down = h.clone();
            let dragging_down = sb.dragging.clone();
            // mouse_move：拖拽中按当前鼠标 y 同步 offset
            let scroll_move = h.clone();
            let dragging_move = sb.dragging.clone();
            // mouse_up：清拖拽 flag（鼠标在 thumb 外松开也清）
            let dragging_up = sb.dragging.clone();

            Some(
                div()
                    .absolute()
                    .right(px(2.0))
                    .top(px(0.0))
                    .h(viewport_h)
                    .w(px(6.0))
                    .child(
                        div()
                            .id("scroll-page-thumb")
                            .absolute()
                            .top(thumb_top)
                            .left(px(0.0))
                            .w(px(6.0))
                            .h(thumb_h)
                            .rounded(px(3.0))
                            .bg(muted_fg)
                            .opacity(0.5)
                            .hover(|s| s.opacity(0.9))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, _w, cx| {
                                dragging_down.set(true);
                                // 立即 jump 让 thumb 中心对齐鼠标 y
                                if drag_to_offset_y(&scroll_down, ev.position.y) {
                                    cx.refresh_windows();
                                }
                                cx.stop_propagation();
                            })
                            .on_mouse_move(move |ev: &MouseMoveEvent, _w, cx| {
                                if !dragging_move.get() {
                                    return;
                                }
                                if drag_to_offset_y(&scroll_move, ev.position.y) {
                                    cx.refresh_windows();
                                }
                            })
                            .on_mouse_up(MouseButton::Left, move |_ev, _w, _cx| {
                                dragging_up.set(false);
                            }),
                    ),
            )
        });

        // 拖拽 mouse_up 兜底：用户 drag 拖出 thumb 外才松开 — element 内的
        // on_mouse_up 不触发，得在 window-level 注册全局兜底。
        let drag_release_fallback = self.scrollbar.as_ref().and_then(|sb| {
            if sb.dragging.get() {
                Some((sb.dragging.clone(), sb.scroll.clone()))
            } else {
                None
            }
        });

        // scroll 容器构建。
        // scroll_div 永远在 outer wrapper (.flex_col) 内作 flex_1 + min_h(0)：
        // outer wrapper 高度由 caller 控（flex_1 模式则在 caller 父 strict
        // fit；否则 size_full），scroll_div 在 wrapper 内严格 fit wrapper。
        // - 持 scrollbar：.overflow_hidden + .track_scroll + 自带 wheel
        // - 无 scrollbar：.overflow_y_scroll fallback GPUI 内置 wheel
        let mut scroll_div = div().id(self.id).flex_1().min_h(px(0.0));
        scroll_div = if self.scrollbar.is_some() {
            scroll_div.overflow_hidden()
        } else {
            scroll_div.overflow_y_scroll()
        };
        if let Some(c) = self.bg {
            scroll_div = scroll_div.bg(c);
        }
        if let Some(p) = self.px {
            scroll_div = scroll_div.px(p);
        }
        if let Some(p) = self.py {
            scroll_div = scroll_div.py(p);
        }
        if let Some(sb) = &self.scrollbar {
            scroll_div = scroll_div.track_scroll(&sb.scroll);
            if let Some(custom) = self.on_wheel.clone() {
                scroll_div = scroll_div.on_scroll_wheel(move |ev, w, cx| custom(ev, w, cx));
            } else {
                let handle = sb.scroll.clone();
                scroll_div = scroll_div.on_scroll_wheel(move |ev, _w, cx| {
                    if default_wheel_step_y(&handle, ev) {
                        // App 上没 cx.notify(entity)，用 refresh_windows 触发
                        // 全 window 重绘 — wheel 频率低（用户实操），不耗。
                        cx.refresh_windows();
                        cx.stop_propagation();
                    }
                });
            }
        } else if let Some(custom) = self.on_wheel.clone() {
            scroll_div = scroll_div.on_scroll_wheel(move |ev, w, cx| custom(ev, w, cx));
        }
        scroll_div = scroll_div.children(self.children);

        // outer wrapper：.relative 让 scrollbar absolute 定位绑到此层，与
        // scroll 容器同 bounds 但不在 scroll viewport 内（不被 transform）。
        // flex_col 让 inner scroll_div 走 flex_1 + min_h(0) 严格 fit。
        // flex_1 模式：在 caller flex_col 内拿 remaining；否则 size_full。
        let mut wrapper = div().relative().flex().flex_col();
        wrapper = if self.flex_1 {
            wrapper.flex_1().min_h(px(0.0))
        } else {
            wrapper.size_full()
        };

        // drag mouse_move + mouse_up 全局 listener：让用户拖到 thumb 外或者
        // 拖出窗口边再松开时也能清 drag。在 prepaint callback 内通过 window
        // 注册（与 TextInput drag select 同模式）。
        wrapper = wrapper.child(scroll_div).children(scrollbar_overlay);
        if let Some((dragging, scroll)) = drag_release_fallback {
            let dragging_move = dragging.clone();
            let scroll_move = scroll.clone();
            let dragging_up = dragging;
            wrapper = wrapper.child(gpui::canvas(
                move |_bounds, window, _cx| {
                    // mouse_move 全局：drag 中按当前鼠标 y 更新 offset
                    let dragging = dragging_move.clone();
                    let scroll = scroll_move.clone();
                    window.on_mouse_event(
                        move |ev: &MouseMoveEvent,
                              phase: DispatchPhase,
                              _window: &mut Window,
                              cx: &mut App| {
                            if phase != DispatchPhase::Bubble || !dragging.get() {
                                return;
                            }
                            if drag_to_offset_y(&scroll, ev.position.y) {
                                cx.refresh_windows();
                            }
                        },
                    );
                    // mouse_up 全局：任何位置松开都清 drag
                    let dragging = dragging_up.clone();
                    window.on_mouse_event(
                        move |_ev: &MouseUpEvent,
                              phase: DispatchPhase,
                              _window: &mut Window,
                              _cx: &mut App| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            dragging.set(false);
                        },
                    );
                },
                |_bounds, _request, _window, _cx| {},
            ));
        }
        wrapper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_no_children_and_no_optional_style() {
        let p = ScrollPage::new("test");
        assert!(p.children.is_empty());
        assert!(p.bg.is_none());
        assert!(p.px.is_none());
        assert!(p.py.is_none());
        assert!(p.scrollbar.is_none());
        assert!(p.on_wheel.is_none());
        assert!(!p.flex_1);
    }

    #[test]
    fn builders_set_optional_fields() {
        let p = ScrollPage::new("test")
            .bg(gpui::rgb(0x000000).into())
            .px(gpui::px(16.0))
            .py(gpui::px(8.0))
            .flex_1();
        assert!(p.bg.is_some());
        assert_eq!(p.px, Some(gpui::px(16.0)));
        assert_eq!(p.py, Some(gpui::px(8.0)));
        assert!(p.flex_1);
    }

    #[test]
    fn parent_element_extend_pushes_children() {
        let mut p = ScrollPage::new("test");
        p.extend(vec![
            div().into_any_element(),
            div().into_any_element(),
            div().into_any_element(),
        ]);
        assert_eq!(p.children.len(), 3);
    }

    #[test]
    fn scrollbar_handle_starts_not_dragging() {
        let h = ScrollbarHandle::new();
        assert!(!h.dragging.get());
    }

    #[test]
    fn scrollbar_handle_clone_shares_state() {
        let h = ScrollbarHandle::new();
        let h2 = h.clone();
        h.dragging.set(true);
        assert!(h2.dragging.get());
    }

    #[test]
    fn scrollbar_sets_handle() {
        let h = ScrollbarHandle::new();
        let p = ScrollPage::new("test").scrollbar(&h);
        assert!(p.scrollbar.is_some());
    }

    #[test]
    fn on_wheel_override_sets_handler() {
        let p = ScrollPage::new("test").on_wheel(|_ev, _w, _cx| {});
        assert!(p.on_wheel.is_some());
    }

    #[test]
    fn wheel_sign_y_pixels() {
        assert_eq!(
            wheel_sign_y(ScrollDelta::Pixels(gpui::point(px(0.0), px(10.0)))),
            1.0
        );
        assert_eq!(
            wheel_sign_y(ScrollDelta::Pixels(gpui::point(px(0.0), px(-5.0)))),
            -1.0
        );
        assert_eq!(
            wheel_sign_y(ScrollDelta::Pixels(gpui::point(px(0.0), px(0.0)))),
            0.0
        );
    }

    #[test]
    fn wheel_sign_y_lines() {
        assert_eq!(wheel_sign_y(ScrollDelta::Lines(gpui::point(0.0, 3.0))), 1.0);
        assert_eq!(
            wheel_sign_y(ScrollDelta::Lines(gpui::point(0.0, -2.0))),
            -1.0
        );
        assert_eq!(wheel_sign_y(ScrollDelta::Lines(gpui::point(0.0, 0.0))), 0.0);
    }
}
