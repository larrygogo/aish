//! ScrollPage — 整页 scroll wrapper：size_full + overflow_y_scroll + 可选 bg/padding。
//!
//! 解决 Home / Settings / 未来 page-level view 共用的 boilerplate +
//! 易踩的 flex-shrink pitfall：
//!
//! - scroll 容器**必须** stateful（`.id()`）+ `.overflow_y_scroll()`，少一样
//!   scroll 都不生效
//! - **不能**在 scroll 容器上设 `.flex().flex_col()` —— flex 子级默认
//!   flex-shrink: 1，当 children 总高超出 container 时会被压扁而不是触发
//!   scroll，scrollbar 永远不出现
//!
//! 本组件直接走 block layout（无 flex），children 自然纵向流，溢出时
//! overflow_y_scroll 触发滚动。
//!
//! ## ScrollHandle（推荐持有）
//!
//! GPUI 的 `overflow_y_scroll` 在某些 layout 嵌套下内置 wheel handler 不
//! 触发；推荐 caller 持有 `ScrollHandle` 字段（在 view struct 内），通过
//! `.track_scroll(&handle)` 传入 — 这样 caller 还能手写 `on_scroll_wheel`
//! handler 强制 wheel → handle.set_offset，可靠跨 layout。
//!
//! ```ignore
//! use aish_ui::ScrollPage;
//! use gpui::{px, ScrollHandle};
//!
//! pub struct MyView { scroll_handle: ScrollHandle }
//!
//! impl Render for MyView {
//!     fn render(&mut self, _w, cx) -> impl IntoElement {
//!         ScrollPage::new("my-scroll")
//!             .track_scroll(&self.scroll_handle)
//!             .on_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| {
//!                 this.handle_wheel(ev, cx);
//!             }))
//!             .child(...)
//!     }
//! }
//! ```
//!
//! 需要 ContextMenu / Toast 等 overlay 时，把 ScrollPage 放在 `relative()`
//! 父容器内，overlay 与 ScrollPage 平级 child（overlay **不能**放 ScrollPage
//! 内部 —— absolute 定位的 backdrop / 菜单会被 scroll viewport 裁切）。

use std::rc::Rc;

use gpui::{
    div, prelude::*, AnyElement, App, ElementId, Hsla, IntoElement, Pixels, ScrollHandle,
    ScrollWheelEvent, Window,
};

type WheelHandler = Rc<dyn Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct ScrollPage {
    id: ElementId,
    bg: Option<Hsla>,
    px: Option<Pixels>,
    py: Option<Pixels>,
    scroll_handle: Option<ScrollHandle>,
    on_wheel: Option<WheelHandler>,
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

    /// 关联 caller 持有的 ScrollHandle — 让 caller 能在 view struct 内
    /// 拿到 offset / max_offset / set_offset，常配合手写 on_wheel 使用。
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(handle.clone());
        self
    }

    /// 接管 wheel 事件 — caller 自己算 step + set_offset，可靠跨 layout
    /// 触发。设此 handler 后建议同时调 `.track_scroll(...)`，否则 wheel
    /// handler 拿不到 ScrollHandle 改 offset 没用。
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

impl RenderOnce for ScrollPage {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut d = div().id(self.id).size_full().overflow_y_scroll();
        if let Some(c) = self.bg {
            d = d.bg(c);
        }
        if let Some(p) = self.px {
            d = d.px(p);
        }
        if let Some(p) = self.py {
            d = d.py(p);
        }
        if let Some(h) = self.scroll_handle {
            d = d.track_scroll(&h);
        }
        if let Some(handler) = self.on_wheel {
            d = d.on_scroll_wheel(move |ev, w, cx| handler(ev, w, cx));
        }
        d.children(self.children)
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
    }

    #[test]
    fn builders_set_optional_fields() {
        let p = ScrollPage::new("test")
            .bg(gpui::rgb(0x000000).into())
            .px(gpui::px(16.0))
            .py(gpui::px(8.0));
        assert!(p.bg.is_some());
        assert_eq!(p.px, Some(gpui::px(16.0)));
        assert_eq!(p.py, Some(gpui::px(8.0)));
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
    fn track_scroll_sets_handle() {
        let handle = ScrollHandle::new();
        let p = ScrollPage::new("test").track_scroll(&handle);
        assert!(p.scroll_handle.is_some());
    }

    #[test]
    fn on_wheel_sets_handler() {
        let p = ScrollPage::new("test").on_wheel(|_ev, _w, _cx| {});
        assert!(p.on_wheel.is_some());
    }
}
