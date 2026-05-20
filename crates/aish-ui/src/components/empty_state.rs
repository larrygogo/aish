//! EmptyState / ErrorState — M28 state design.
//!
//! 4-slot anatomy（spec D-1）：
//! - icon container：32×32 rounded-full bg(secondary) + 18px icon
//! - gap 12px → title (Title3)
//! - gap 4px → description (Body + muted + max-w 320 + center)
//! - gap 16px → action (Button 或自定义)
//!
//! `EmptyState` 默认 icon = caller 决定（无 fallback；建议用 Inbox/Server/
//! Loader 等）+ icon_color = muted_foreground。
//! `ErrorState` 默认 icon = AlertCircle + icon_color = destructive。
//!
//! 两者共享内部 `StatusView` struct，仅 variant 字段差异。

use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};

use crate::icons::{icon, IconName};
use crate::theme::theme;
use crate::{TypeRole, TypographyExt};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StatusVariant {
    Empty,
    Error,
}

/// 4-slot 状态视图：icon / title / description / action。
/// 不直接 pub 构造 — 通过 `EmptyState::new(id)` / `ErrorState::new(id)`
/// 工厂返回，区别仅 variant + 默认 icon / color。
#[derive(IntoElement)]
pub struct StatusView {
    id: ElementId,
    variant: StatusVariant,
    icon: Option<IconName>,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
}

impl StatusView {
    pub fn icon(mut self, i: IconName) -> Self {
        self.icon = Some(i);
        self
    }

    pub fn title(mut self, t: impl Into<SharedString>) -> Self {
        self.title = t.into();
        self
    }

    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }

    pub fn action(mut self, a: impl IntoElement) -> Self {
        self.action = Some(a.into_any_element());
        self
    }
}

impl RenderOnce for StatusView {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let (icon_color, default_icon) = match self.variant {
            StatusVariant::Empty => (t.colors.muted_foreground, IconName::Inbox),
            StatusVariant::Error => (t.colors.destructive, IconName::AlertCircle),
        };
        let icon_name = self.icon.unwrap_or(default_icon);
        let icon_bg = t.colors.secondary;

        let mut el = div()
            .id(self.id)
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .py(px(32.0));

        // icon container — 32×32 rounded-full + 18px icon
        el = el.child(
            div()
                .w(px(32.0))
                .h(px(32.0))
                .rounded_full()
                .bg(icon_bg)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(icon_name).size(t.icon_size.lg).text_color(icon_color)),
        );

        // title — Title3 (14/600/fg)
        el = el.child(div().typography(TypeRole::Title3, t).child(self.title));

        // description — Body (13/400) + muted override + max-w 320 + center
        if let Some(desc) = self.description {
            el = el.child(
                div()
                    .max_w(px(320.0))
                    .text_center()
                    .typography(TypeRole::Body, t)
                    .text_color(t.colors.muted_foreground)
                    .child(desc),
            );
        }

        // action — pt 4 让 button 与 description 之间留呼吸
        if let Some(action) = self.action {
            el = el.child(div().pt(px(4.0)).child(action));
        }

        el
    }
}

/// Empty 状态工厂 — 默认 icon = Inbox / icon_color = muted_foreground。
/// caller 通常通过 `.icon(IconName::Server)` 等 override 默认 icon。
pub struct EmptyState;

impl EmptyState {
    /// new 故意返回 StatusView 而非 Self（unit struct）—— 工厂模式让
    /// `EmptyState::new(id)` 读起来像构造但实际是 StatusView builder。
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<ElementId>) -> StatusView {
        StatusView {
            id: id.into(),
            variant: StatusVariant::Empty,
            icon: None,
            title: SharedString::default(),
            description: None,
            action: None,
        }
    }
}

/// Error 状态工厂 — 默认 icon = AlertCircle / icon_color = destructive。
pub struct ErrorState;

impl ErrorState {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<ElementId>) -> StatusView {
        StatusView {
            id: id.into(),
            variant: StatusVariant::Error,
            icon: None,
            title: SharedString::default(),
            description: None,
            action: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_defaults() {
        let s = EmptyState::new("test");
        assert_eq!(s.variant, StatusVariant::Empty);
        assert!(s.icon.is_none());
        assert!(s.title.is_empty());
        assert!(s.description.is_none());
        assert!(s.action.is_none());
    }

    #[test]
    fn error_state_default_variant_is_error() {
        let s = ErrorState::new("test");
        assert_eq!(s.variant, StatusVariant::Error);
    }

    #[test]
    fn builders_set_fields() {
        let s = EmptyState::new("test")
            .icon(IconName::Server)
            .title("No connections")
            .description("Connect from Home to get started");
        assert_eq!(s.icon, Some(IconName::Server));
        assert_eq!(s.title.as_ref(), "No connections");
        assert!(s.description.is_some());
    }

    #[test]
    fn action_stored() {
        let s = EmptyState::new("test").action(div());
        assert!(s.action.is_some());
    }

    #[test]
    fn error_state_falls_back_to_alert_circle_icon() {
        // 通过 variant 推断默认 icon — render 时 unwrap_or(AlertCircle)
        let s = ErrorState::new("test");
        assert!(s.icon.is_none()); // caller 没 set → 走 default
        assert_eq!(s.variant, StatusVariant::Error);
    }

    #[test]
    fn empty_state_falls_back_to_inbox_icon() {
        let s = EmptyState::new("test");
        assert!(s.icon.is_none());
        assert_eq!(s.variant, StatusVariant::Empty);
    }
}
