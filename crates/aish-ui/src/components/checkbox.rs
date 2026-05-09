//! Checkbox — 受控勾选框。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, SharedString, Window,
};

use crate::icons::{icon, IconName};
use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: SharedString,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            label: SharedString::default(),
            disabled: false,
            on_change: None,
        }
    }

    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = l.into();
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_change(mut self, h: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let checked = self.checked;

        let (box_bg, icon_color) = if disabled {
            (t.colors.muted, t.colors.muted_foreground)
        } else if checked {
            (t.colors.primary, t.colors.primary_foreground)
        } else {
            (gpui::transparent_black(), t.colors.foreground)
        };

        let mut row = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap(t.spacing.px_2)
            .child(
                div()
                    .w(gpui::px(16.0))
                    .h(gpui::px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(t.radius.sm)
                    .bg(box_bg)
                    .border_1()
                    .border_color(t.colors.border)
                    .when(checked, |d| {
                        d.child(
                            icon(IconName::Check)
                                .size(gpui::px(12.0))
                                .text_color(icon_color),
                        )
                    }),
            );

        if !self.label.as_ref().is_empty() {
            row = row.child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(if disabled {
                        t.colors.muted_foreground
                    } else {
                        t.colors.foreground
                    })
                    .child(self.label.clone()),
            );
        }

        if !disabled {
            row = row.cursor_pointer();
            if let Some(handler) = self.on_change {
                let new_value = !checked;
                row = row.on_mouse_down(
                    MouseButton::Left,
                    move |_ev: &MouseDownEvent, window, cx| {
                        handler(&new_value, window, cx);
                    },
                );
            }
        }

        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let c = Checkbox::new("agree");
        assert!(!c.checked);
        assert!(!c.disabled);
        assert!(c.label.as_ref().is_empty());
        assert!(c.on_change.is_none());
    }

    #[test]
    fn checked_chain() {
        let c = Checkbox::new("a").checked(true);
        assert!(c.checked);
    }

    #[test]
    fn label_chain() {
        let c = Checkbox::new("a").label("Agree");
        assert_eq!(c.label.as_ref(), "Agree");
    }

    #[test]
    fn disabled_chain() {
        let c = Checkbox::new("a").disabled(true);
        assert!(c.disabled);
    }

    #[test]
    fn on_change_stored() {
        let c = Checkbox::new("a").on_change(|_, _, _| {});
        assert!(c.on_change.is_some());
    }
}
