//! NavItem — 导航项。Horizontal（顶部栏）+ Vertical（侧栏）双模。
//!
//! icon 接受任意 IntoElement（SVG / Nerd Font / 纯文字），label 可选。
//! active 时画 indicator：vertical 在左侧 2px primary 条，
//! horizontal 在底部 2px primary 条。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, IntoElement, MouseButton, MouseDownEvent,
    SharedString, Window,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavItemOrientation {
    Vertical,
    Horizontal,
}

#[derive(IntoElement)]
pub struct NavItem {
    id: ElementId,
    icon: Option<AnyElement>,
    label: Option<SharedString>,
    active: bool,
    orientation: NavItemOrientation,
    on_click: Option<ClickHandler>,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            label: None,
            active: false,
            orientation: NavItemOrientation::Vertical,
            on_click: None,
        }
    }

    pub fn icon(mut self, i: impl IntoElement) -> Self {
        self.icon = Some(i.into_any_element());
        self
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    pub fn active(mut self, a: bool) -> Self {
        self.active = a;
        self
    }

    pub fn vertical(mut self) -> Self {
        self.orientation = NavItemOrientation::Vertical;
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.orientation = NavItemOrientation::Horizontal;
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

impl RenderOnce for NavItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let active = self.active;
        let orientation = self.orientation;
        let icon = self.icon;
        let label = self.label;
        let on_click = self.on_click;

        let fg = if active {
            t.colors.foreground
        } else {
            t.colors.muted_foreground
        };

        let indicator_color = if active {
            t.colors.primary
        } else {
            gpui::transparent_black()
        };

        let mut el = div().id(self.id).text_color(fg).cursor_pointer();

        el = match orientation {
            NavItemOrientation::Vertical => el
                .w_full()
                .py(px(14.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .border_l_2()
                .border_color(indicator_color),
            NavItemOrientation::Horizontal => el
                .h(px(36.0))
                .px(t.spacing.px_3)
                .flex()
                .flex_row()
                .items_center()
                .gap(t.spacing.px_2)
                .border_b_2()
                .border_color(indicator_color),
        };

        if active && orientation == NavItemOrientation::Vertical {
            el = el.bg(t.colors.card);
        }

        if !active {
            let hover_fg = t.colors.secondary_foreground;
            el = el.hover(move |s| s.text_color(hover_fg));
        }

        if let Some(handler) = on_click {
            el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                handler(ev, window, cx);
            });
        }

        el = el.when_some(icon, |d, i| d.child(i));
        el = el.when_some(label, |d, l| {
            d.child(div().text_size(t.font_size.sm).child(l))
        });

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let n = NavItem::new("test");
        assert!(!n.active);
        assert_eq!(n.orientation, NavItemOrientation::Vertical);
        assert!(n.icon.is_none());
        assert!(n.label.is_none());
        assert!(n.on_click.is_none());
    }

    #[test]
    fn orientation_chain() {
        assert_eq!(
            NavItem::new("a").horizontal().orientation,
            NavItemOrientation::Horizontal
        );
        assert_eq!(
            NavItem::new("a").vertical().orientation,
            NavItemOrientation::Vertical
        );
    }

    #[test]
    fn active_chain() {
        let n = NavItem::new("a").active(true);
        assert!(n.active);
    }

    #[test]
    fn icon_and_label_stored() {
        let n = NavItem::new("a").icon(gpui::div()).label("Home");
        assert!(n.icon.is_some());
        assert_eq!(n.label.as_ref().unwrap().as_ref(), "Home");
    }

    #[test]
    fn on_click_stored() {
        let n = NavItem::new("a").on_click(|_, _, _| {});
        assert!(n.on_click.is_some());
    }
}
