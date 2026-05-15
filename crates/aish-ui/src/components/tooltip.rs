//! Tooltip — 悬停提示。封装 GPUI 内置 `.tooltip()` API。
//!
//! GPUI 实际 API（`StatefulInteractiveElement::tooltip`）是 fluent 写法，
//! 接受 `impl Fn(&mut Window, &mut App) -> AnyView + 'static`，返回 `Self`。
//!
//! 调用方示例：
//! ```ignore
//! use aish_ui::prelude::*;
//! div().id("btn").child("hi").with_tooltip(Tooltip::new("你好"))
//! ```

use gpui::{div, prelude::*, AnyView, App, Context, IntoElement, Render, SharedString, Window};

use crate::theme::theme;

// ── Tooltip 描述 ─────────────────────────────────────────────────────────────

/// 静态文本 tooltip 描述。
///
/// 通过 [`TooltipExt::with_tooltip`] 附加到任何 stateful 元素上。
pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// 构造文本 tooltip 描述。
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }

    /// 转为 GPUI tooltip builder closure，供 `.tooltip()` 直接使用。
    pub fn into_builder(self) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = self.text;
        move |_, cx| cx.new(|_| TooltipView { text: text.clone() }).into()
    }
}

// ── TooltipView（内部渲染 view）────────────────────────────────────────────────

/// 内部 view，承载 tooltip 的实际渲染。
pub struct TooltipView {
    text: SharedString,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        // M27 anatomy.overlay.tooltip_px (8) / tooltip_py (4)
        div()
            .px(t.anatomy.overlay.tooltip_px)
            .py(t.anatomy.overlay.tooltip_py)
            .rounded(t.radius.sm)
            .bg(t.colors.popover)
            .border_1()
            .border_color(t.colors.border)
            .child(
                div()
                    .text_size(t.font_size.xs)
                    .text_color(t.colors.popover_foreground)
                    .child(self.text.clone()),
            )
    }
}

// ── TooltipExt trait ──────────────────────────────────────────────────────────

/// 给所有 `StatefulInteractiveElement` 加 `.with_tooltip(Tooltip::new(…))` 快捷方法。
///
/// GPUI 的 `tooltip` 方法定义在 `StatefulInteractiveElement` 上（需要元素有 `id`），
/// 是 fluent 写法：接受 closure，返回 `Self`。
pub trait TooltipExt: StatefulInteractiveElement {
    /// 附加静态文本 tooltip。
    fn with_tooltip(self, tooltip: Tooltip) -> Self;
}

impl<T: StatefulInteractiveElement> TooltipExt for T {
    fn with_tooltip(self, tooltip: Tooltip) -> Self {
        self.tooltip(tooltip.into_builder())
    }
}

// ── 单元测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_stores_text() {
        let t = Tooltip::new("hello");
        assert_eq!(t.text.as_ref(), "hello");
    }

    #[test]
    fn tooltip_new_with_string_slice() {
        let t = Tooltip::new("world");
        assert_eq!(t.text.as_ref(), "world");
    }

    #[test]
    fn tooltip_view_holds_text() {
        let v = TooltipView {
            text: "world".into(),
        };
        assert_eq!(v.text.as_ref(), "world");
    }
}
