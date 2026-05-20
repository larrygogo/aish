//! InlineError — form 字段 / Card 内部错误提示，单行。
//!
//! charter §10 「inline field error」标准实现，借鉴 paseo design.md §10
//! 「palette.red[300] xs sitting under the field」风格。
//!
//! 三个错误呈现层级（charter §10）：
//! - **page-level**：[`crate::toast_error`]（弹 toast）
//! - **placeholder**：[`crate::ErrorState`]（占位整个区域）
//! - **field-level**：本组件（贴在字段下方）
//!
//! anatomy（4-slot 简化）：
//! - 可选 leading icon（12px AlertCircle，默认关 — 字段上方 label 已有锚点）
//! - destructive color
//! - typography Caption (12/400)
//! - 紧贴上方字段下方（caller 控 gap，通常 anatomy.form.error_gap）
//!
//! 使用：
//! ```ignore
//! div()
//!     .child(my_input)
//!     .child(InlineError::new("端口必须是 1-65535 的数字"))
//! ```

use gpui::{div, prelude::*, App, IntoElement, SharedString, Window};

use crate::icons::{icon, IconName};
use crate::theme::theme;
use crate::TypographyExt;

#[derive(IntoElement)]
pub struct InlineError {
    message: SharedString,
    show_icon: bool,
}

impl InlineError {
    /// 创建一个 inline 错误提示。默认无 icon —— field 上方 label 已经是
    /// 视觉锚点，加 icon 反而碎。
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            show_icon: false,
        }
    }

    /// 显式开 icon（AlertCircle 12px）。用在没有上方 label 锚点的场景，
    /// 比如 Card 内独立错误行（不在 form 字段下方）。
    pub fn with_icon(mut self) -> Self {
        self.show_icon = true;
        self
    }
}

impl RenderOnce for InlineError {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t.spacing.px_1)
            .typography(crate::TypeRole::Caption, t)
            // typography(Caption) 默认 muted_foreground，这里 override 成 destructive
            .text_color(t.colors.destructive);

        if self.show_icon {
            row = row.child(
                icon(IconName::AlertCircle)
                    .size(t.icon_size.xs)
                    .text_color(t.colors.destructive),
            );
        }

        row.child(self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_error_new_stores_message() {
        let e = InlineError::new("test");
        assert_eq!(e.message.as_ref(), "test");
        assert!(!e.show_icon);
    }

    #[test]
    fn inline_error_with_icon_sets_flag() {
        let e = InlineError::new("oops").with_icon();
        assert!(e.show_icon);
    }
}
