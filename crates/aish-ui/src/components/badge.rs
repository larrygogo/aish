//! Badge — 小标签。胶囊形，5 种 variant。

use gpui::{div, prelude::*, App, IntoElement, SharedString, Window};

use crate::theme::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeVariant {
    Default,
    Primary,
    Success,
    Warning,
    Destructive,
}

#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    variant: BadgeVariant,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: BadgeVariant::Default,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = BadgeVariant::Primary;
        self
    }

    pub fn success(mut self) -> Self {
        self.variant = BadgeVariant::Success;
        self
    }

    pub fn warning(mut self) -> Self {
        self.variant = BadgeVariant::Warning;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.variant = BadgeVariant::Destructive;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let (bg, fg) = match self.variant {
            BadgeVariant::Default => (t.colors.muted, t.colors.muted_foreground),
            BadgeVariant::Primary => (t.colors.primary, t.colors.primary_foreground),
            BadgeVariant::Success => (t.colors.success, t.colors.background),
            BadgeVariant::Warning => (t.colors.warning, t.colors.background),
            BadgeVariant::Destructive => (t.colors.destructive, t.colors.destructive_foreground),
        };
        div()
            .h(t.spacing.px_4 + t.spacing.px_1 / 2.0) // ~18
            .px(t.spacing.px_2)
            .flex()
            .items_center()
            .rounded(t.radius.full)
            .bg(bg)
            .child(
                div()
                    .text_size(t.font_size.xs)
                    .text_color(fg)
                    .child(self.label),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_to_default_variant() {
        let b = Badge::new("hi");
        assert_eq!(b.variant, BadgeVariant::Default);
        assert_eq!(b.label.as_ref(), "hi");
    }

    #[test]
    fn primary_sets_variant() {
        let b = Badge::new("ok").primary();
        assert_eq!(b.variant, BadgeVariant::Primary);
    }

    #[test]
    fn destructive_sets_variant() {
        let b = Badge::new("err").destructive();
        assert_eq!(b.variant, BadgeVariant::Destructive);
    }

    #[test]
    fn success_warning_distinct() {
        assert_ne!(
            Badge::new("a").success().variant,
            Badge::new("a").warning().variant
        );
    }
}
