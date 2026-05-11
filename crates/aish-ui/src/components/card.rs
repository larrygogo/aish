//! Card — 卡片容器。header / body / footer / actions 四 slot。
//!
//! `AnyElement` 不可 Clone，Card 走 `#[derive(IntoElement)] + RenderOnce`
//! 一次性消费（与 Dialog body 同模式）。每帧调用方通过 builder 重新构造。

use std::rc::Rc;

use gpui::{
    div, prelude::*, AnyElement, App, ElementId, IntoElement, MouseButton, MouseDownEvent,
    SharedString, Window,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardVariant {
    Default,
    Outlined,
    Elevated,
}

#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    header: Option<AnyElement>,
    body: Option<AnyElement>,
    footer: Option<AnyElement>,
    actions: Option<AnyElement>,
    variant: CardVariant,
    on_click: Option<ClickHandler>,
}

impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            header: None,
            body: None,
            footer: None,
            actions: None,
            variant: CardVariant::Default,
            on_click: None,
        }
    }

    pub fn header(mut self, h: impl IntoElement) -> Self {
        self.header = Some(h.into_any_element());
        self
    }

    pub fn body(mut self, b: impl IntoElement) -> Self {
        self.body = Some(b.into_any_element());
        self
    }

    pub fn footer(mut self, f: impl IntoElement) -> Self {
        self.footer = Some(f.into_any_element());
        self
    }

    pub fn actions(mut self, a: impl IntoElement) -> Self {
        self.actions = Some(a.into_any_element());
        self
    }

    pub fn variant(mut self, v: CardVariant) -> Self {
        self.variant = v;
        self
    }

    pub fn outlined(self) -> Self {
        self.variant(CardVariant::Outlined)
    }

    pub fn elevated(self) -> Self {
        self.variant(CardVariant::Elevated)
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let id = self.id;
        let variant = self.variant;
        let on_click = self.on_click;
        let header = self.header;
        let body = self.body;
        let footer = self.footer;
        let actions = self.actions;

        let group_name = SharedString::from(format!("card-group-{:?}", id));

        let mut el = div()
            .id(id)
            .group(group_name.clone())
            .flex()
            .flex_col()
            .bg(t.colors.card)
            .rounded(t.radius.lg);

        match variant {
            CardVariant::Default => {}
            CardVariant::Outlined => {
                el = el.border_1().border_color(t.colors.border);
            }
            CardVariant::Elevated => {
                el = el.border_1().border_color(t.colors.ring);
            }
        }

        if let Some(handler) = on_click {
            let accent = t.colors.accent;
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(accent))
                .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
        }

        el = el
            .when_some(header, |d, h| d.child(div().child(h)))
            .when_some(body, |d, b| d.child(div().flex_1().child(b)))
            .when_some(footer, |d, f| d.child(div().child(f)));

        if let Some(a) = actions {
            el = el.child(
                div()
                    .opacity(0.0)
                    .group_hover(group_name, |s| s.opacity(1.0))
                    .child(a),
            );
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let c = Card::new("test");
        assert_eq!(c.variant, CardVariant::Default);
        assert!(c.header.is_none());
        assert!(c.body.is_none());
        assert!(c.footer.is_none());
        assert!(c.actions.is_none());
        assert!(c.on_click.is_none());
    }

    #[test]
    fn variant_chain() {
        assert_eq!(Card::new("a").outlined().variant, CardVariant::Outlined);
        assert_eq!(Card::new("a").elevated().variant, CardVariant::Elevated);
    }

    #[test]
    fn slots_can_be_set() {
        let c = Card::new("a")
            .header(gpui::div())
            .body(gpui::div())
            .footer(gpui::div())
            .actions(gpui::div());
        assert!(c.header.is_some());
        assert!(c.body.is_some());
        assert!(c.footer.is_some());
        assert!(c.actions.is_some());
    }

    #[test]
    fn on_click_stored() {
        let c = Card::new("a").on_click(|_, _, _| {});
        assert!(c.on_click.is_some());
    }
}
