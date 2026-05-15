//! Radio — M29 D-3 单选组件（替代 Tabs 的 KeyFile / Password 切换）。
//!
//! 视觉：14px 圆 + 1px border（idle: border / hover: accent / checked: primary）
//! + 6px center dot（仅 checked 时）+ 8px gap + label (Body)。
//!
//! Radio 是 stateless 控件 — caller 持选中状态（如 host_form.auth_kind:
//! AuthKind 字段），on_click 时回调 caller 更新状态再 re-render。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, ElementId, IntoElement, MouseButton, MouseDownEvent, SharedString,
    Window,
};

use crate::theme::theme;
use crate::{TypeRole, TypographyExt};

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    label: SharedString,
    checked: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            checked: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = l.into();
        self
    }

    pub fn checked(mut self, b: bool) -> Self {
        self.checked = b;
        self
    }

    pub fn disabled(mut self, b: bool) -> Self {
        self.disabled = b;
        self
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let checked = self.checked;
        let disabled = self.disabled;
        let handler = self.on_click;

        let border_color = if checked {
            t.colors.primary
        } else {
            t.colors.border
        };

        // 14px 外圆
        let mut outer_circle = div()
            .w(px(14.0))
            .h(px(14.0))
            .rounded_full()
            .border_1()
            .border_color(border_color)
            .bg(t.colors.input)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0();

        if checked {
            // 6px 内 dot — primary 色
            outer_circle = outer_circle.child(
                div()
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(t.colors.primary),
            );
        }

        let mut row = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .typography(TypeRole::Body, t)
            .child(outer_circle)
            .child(self.label.clone());

        if disabled {
            row = row.opacity(0.5);
        } else {
            // hover：border 切 accent（柔和提示可点）
            let hover_border = t.colors.accent;
            row = row
                .cursor_pointer()
                .hover(move |s| s.border_color(hover_border));
            if let Some(h) = handler {
                row = row.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    h(ev, window, cx);
                });
            }
        }

        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_default_unchecked() {
        let r = Radio::new("test");
        assert!(!r.checked);
        assert!(!r.disabled);
        assert!(r.label.is_empty());
        assert!(r.on_click.is_none());
    }

    #[test]
    fn radio_with_label_text() {
        let r = Radio::new("test").label("KeyFile");
        assert_eq!(r.label.as_ref(), "KeyFile");
    }

    #[test]
    fn radio_checked_builder() {
        let r = Radio::new("test").checked(true);
        assert!(r.checked);
    }

    #[test]
    fn radio_disabled_builder() {
        let r = Radio::new("test").disabled(true);
        assert!(r.disabled);
    }

    #[test]
    fn radio_on_click_handler_stored() {
        let r = Radio::new("test").on_click(|_, _, _| {});
        assert!(r.on_click.is_some());
    }
}
