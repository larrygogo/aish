//! ScrollPage — 整页 scroll wrapper：viewport 内部 overflow scroll +
//! 自带 wheel handler + 内置 thumb scrollbar。
//!
//! ## 用法
//!
//! 简单页面（内容很可能不溢出）— 不传 scroll_handle，走 GPUI 内置
//! overflow_y_scroll：
//!
//! ```ignore
//! ScrollPage::new("settings").bg(...).px(...).py(...).child(...)
//! ```
//!
//! 真正可能溢出的页面（推荐）— caller 持 ScrollHandle 字段传入，ScrollPage
//! 自动 wheel + scrollbar：
//!
//! ```ignore
//! pub struct MyView { scroll_handle: ScrollHandle }
//!
//! ScrollPage::new("my-scroll")
//!     .scroll_handle(&self.scroll_handle)
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

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, Hsla, IntoElement, Pixels, Point, ScrollDelta,
    ScrollHandle, ScrollWheelEvent, Window,
};

use crate::theme::theme;

type WheelHandler = Rc<dyn Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct ScrollPage {
    id: ElementId,
    bg: Option<Hsla>,
    px: Option<Pixels>,
    py: Option<Pixels>,
    scroll_handle: Option<ScrollHandle>,
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
            scroll_handle: None,
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

    /// 关联 caller 持有的 ScrollHandle —— 启用后 ScrollPage 自动：
    /// - .track_scroll(handle) 让 children 按 offset transform paint
    /// - .overflow_hidden 让 GPUI 内置 wheel 不与 caller wheel 冲突
    /// - on_scroll_wheel 自带 wheel handler（60px/tick set_offset）
    /// - render scrollbar thumb（仅 max_offset.y > 0 时）
    ///
    /// caller 唯一职责：在 view struct 保有 `ScrollHandle` 字段（不能放
    /// 局部变量 — 每帧重建 handle 状态会丢失），其余不用管。
    pub fn scroll_handle(mut self, handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(handle.clone());
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

impl RenderOnce for ScrollPage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // scrollbar thumb：仅 scroll_handle 模式 + max_offset.y > 0（内容
        // 溢出）时画。viewport_h 从 ScrollHandle.bounds() 拿（首 paint 后才
        // 有），首帧 bounds 是空 size → max_offset 也是 0 → 自动不画 thumb。
        let muted_fg = theme(cx).colors.muted_foreground;
        let scrollbar_overlay = self.scroll_handle.as_ref().and_then(|h| {
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
                            .hover(|s| s.opacity(0.9)),
                    ),
            )
        });

        // scroll 容器构建。
        // scroll_div 永远在 outer wrapper (.flex_col) 内作 flex_1 + min_h(0)：
        // outer wrapper 高度由 caller 控（flex_1 模式则在 caller 父 strict
        // fit；否则 size_full），scroll_div 在 wrapper 内严格 fit wrapper。
        // - 持 ScrollHandle：.overflow_hidden + .track_scroll + 自带 wheel
        // - 无 ScrollHandle：.overflow_y_scroll fallback GPUI 内置 wheel
        let mut scroll_div = div().id(self.id).flex_1().min_h(px(0.0));
        scroll_div = if self.scroll_handle.is_some() {
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
        if let Some(h) = &self.scroll_handle {
            scroll_div = scroll_div.track_scroll(h);
            // wheel handler 优先级：caller override > 默认 60px/tick
            if let Some(custom) = self.on_wheel.clone() {
                scroll_div = scroll_div.on_scroll_wheel(move |ev, w, cx| custom(ev, w, cx));
            } else {
                let handle = h.clone();
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
        wrapper.child(scroll_div).children(scrollbar_overlay)
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
        assert!(p.scroll_handle.is_none());
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
    fn scroll_handle_sets_handle() {
        let handle = ScrollHandle::new();
        let p = ScrollPage::new("test").scroll_handle(&handle);
        assert!(p.scroll_handle.is_some());
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
