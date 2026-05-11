//! IconButton — 仅 icon 的方形按钮。复用 Button 的 variant 系统。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Pixels, Window,
};

use crate::components::ButtonVariant;
use crate::icons::{icon, IconName};
use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconButtonSize {
    Sm,
    Md,
    Lg,
}

impl IconButtonSize {
    fn box_size(&self) -> Pixels {
        match self {
            IconButtonSize::Sm => px(24.0),
            IconButtonSize::Md => px(32.0),
            IconButtonSize::Lg => px(40.0),
        }
    }

    fn icon_size(&self) -> Pixels {
        self.box_size() - px(8.0)
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon_name: IconName,
    variant: ButtonVariant,
    size: IconButtonSize,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon_name: IconName) -> Self {
        Self {
            id: id.into(),
            icon_name,
            variant: ButtonVariant::Ghost,
            size: IconButtonSize::Md,
            disabled: false,
            on_click: None,
        }
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

    pub fn small(mut self) -> Self {
        self.size = IconButtonSize::Sm;
        self
    }

    pub fn medium(mut self) -> Self {
        self.size = IconButtonSize::Md;
        self
    }

    pub fn large(mut self) -> Self {
        self.size = IconButtonSize::Lg;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
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

        let bs = self.size.box_size();
        let isz = self.size.icon_size();

        let mut el = div()
            .id(self.id)
            .w(bs)
            .h(bs)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.sm)
            .bg(bg)
            .child(icon(self.icon_name).size(isz).text_color(fg));

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
    fn new_defaults_to_ghost_md() {
        let b = IconButton::new("close", IconName::X);
        assert_eq!(b.variant, ButtonVariant::Ghost);
        assert_eq!(b.size, IconButtonSize::Md);
        assert!(!b.disabled);
    }

    #[test]
    fn size_chains() {
        assert_eq!(
            IconButton::new("a", IconName::X).small().size,
            IconButtonSize::Sm
        );
        assert_eq!(
            IconButton::new("a", IconName::X).large().size,
            IconButtonSize::Lg
        );
    }

    #[test]
    fn box_size_relationships() {
        assert!(IconButtonSize::Sm.box_size() < IconButtonSize::Md.box_size());
        assert!(IconButtonSize::Md.box_size() < IconButtonSize::Lg.box_size());
        assert_eq!(
            IconButtonSize::Md.icon_size(),
            IconButtonSize::Md.box_size() - px(8.0)
        );
    }
}
