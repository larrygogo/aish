//! ScrollPage — 整页 scroll wrapper：size_full + overflow_y_scroll + 可选 bg/padding。
//!
//! 解决 Home / Settings / 未来 page-level view 共用的 boilerplate +
//! 易踩的 flex-shrink pitfall：
//!
//! - scroll 容器**必须** stateful（`.id()`）+ `.overflow_y_scroll()`，少一样
//!   scroll 都不生效
//! - **不能**在 scroll 容器上设 `.flex().flex_col()` —— flex 子级默认
//!   flex-shrink: 1，当 children 总高超出 container 时会被压扁而不是触发
//!   scroll，scrollbar 永远不出现（user-confirmed bug + fix 见
//!   `fix(home): scroll 容器去掉 flex_col`）
//!
//! 本组件直接走 block layout（无 flex），children 自然纵向流，溢出时
//! overflow_y_scroll 触发滚动。
//!
//! ## 用法
//!
//! ```ignore
//! use aish_ui::ScrollPage;
//! use gpui::px;
//!
//! ScrollPage::new("settings-scroll")
//!     .bg(theme.colors.background)
//!     .px(px(32.0))
//!     .py(px(24.0))
//!     .child(page_title)
//!     .child(card_a)
//!     .child(card_b)
//! ```
//!
//! 需要 ContextMenu / Toast 等 overlay 时，把 ScrollPage 放在 `relative()`
//! 父容器内，overlay 与 ScrollPage 平级 child（overlay **不能**放 ScrollPage
//! 内部 —— absolute 定位的 backdrop / 菜单会被 scroll viewport 裁切）。

use gpui::{div, prelude::*, AnyElement, App, ElementId, Hsla, IntoElement, Pixels, Window};

#[derive(IntoElement)]
pub struct ScrollPage {
    id: ElementId,
    bg: Option<Hsla>,
    px: Option<Pixels>,
    py: Option<Pixels>,
    children: Vec<AnyElement>,
}

impl ScrollPage {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            bg: None,
            px: None,
            py: None,
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
}
