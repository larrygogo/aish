//! Button — 主要操作组件。4 个 variant + disabled。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, SharedString, Window,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Destructive,
    Ghost,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            variant: ButtonVariant::Primary,
            disabled: false,
            on_click: None,
        }
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = l.into();
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.variant = ButtonVariant::Destructive;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
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

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let (bg, fg) = if disabled {
            (t.colors.muted, t.colors.muted_foreground)
        } else {
            match self.variant {
                ButtonVariant::Primary => (t.colors.primary, t.colors.primary_foreground),
                ButtonVariant::Secondary => (t.colors.secondary, t.colors.secondary_foreground),
                ButtonVariant::Destructive => {
                    (t.colors.destructive, t.colors.destructive_foreground)
                }
                ButtonVariant::Ghost => (gpui::transparent_black(), t.colors.foreground),
            }
        };

        let mut el = div()
            .id(self.id)
            .h(t.spacing.px_3 + t.spacing.px_4) // ~28
            .px(t.spacing.px_3)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.md)
            .bg(bg)
            .child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(fg)
                    .child(self.label),
            );

        if !disabled {
            let hover_bg = t.colors.accent;
            el = el.cursor_pointer().hover(move |s| s.bg(hover_bg));
            if let Some(handler) = self.on_click {
                el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
            }
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let b = Button::new("send");
        assert_eq!(b.variant, ButtonVariant::Primary);
        assert!(!b.disabled);
        assert!(b.on_click.is_none());
    }

    #[test]
    fn variant_chains() {
        assert_eq!(
            Button::new("a").secondary().variant,
            ButtonVariant::Secondary
        );
        assert_eq!(
            Button::new("a").destructive().variant,
            ButtonVariant::Destructive
        );
        assert_eq!(Button::new("a").ghost().variant, ButtonVariant::Ghost);
    }

    #[test]
    fn disabled_chain() {
        let b = Button::new("a").disabled(true);
        assert!(b.disabled);
    }

    #[test]
    fn on_click_stored() {
        let b = Button::new("a").on_click(|_, _, _| {});
        assert!(b.on_click.is_some());
    }

    #[test]
    fn label_stored() {
        let b = Button::new("send").label("发送");
        assert_eq!(b.label.as_ref(), "发送");
    }
}
