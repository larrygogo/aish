//! Card — 卡片容器。header / body / footer 三 slot + variant + on_click。
//!
//! `AnyElement` 不可 Clone，Card 走 `#[derive(IntoElement)] + RenderOnce`
//! 一次性消费（与 Dialog body 同模式）。每帧调用方通过 builder 重新构造。
//!
//! **关于"hover 才显示的浮层"（如编辑/删除按钮）**：Card 不内置 actions
//! 浮层（之前尝试用 absolute + group_hover 内置但定位歧义大）。需要这种效果
//! 时，调用方自己在 body 内安排：body 容器 `.relative()` + `.group(name)`，
//! 浮层 `.absolute().opacity(0).group_hover(name, |s| s.opacity(1.0))`。

use std::rc::Rc;

use gpui::{
    div, prelude::*, AnyElement, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Window,
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

        let mut el = div()
            .id(self.id)
            .flex()
            .flex_col()
            .bg(t.colors.card)
            .rounded(t.radius.lg);

        match self.variant {
            CardVariant::Default => {}
            CardVariant::Outlined => {
                el = el.border_1().border_color(t.colors.border);
            }
            CardVariant::Elevated => {
                el = el.border_1().border_color(t.colors.ring);
            }
        }

        if let Some(handler) = self.on_click {
            // 大容器 hover 用 secondary 灰阶提亮（modern UI 通用：hover 不换色调，
            // 只提亮一档）。accent 太染色，会让 Card 反客为主
            let hover_bg = t.colors.secondary_hover;
            let active_bg = t.colors.secondary_active;
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg))
                .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
        }

        el.when_some(self.header, |d, h| d.child(div().child(h)))
            .when_some(self.body, |d, b| d.child(div().flex_1().child(b)))
            .when_some(self.footer, |d, f| d.child(div().child(f)))
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
            .footer(gpui::div());
        assert!(c.header.is_some());
        assert!(c.body.is_some());
        assert!(c.footer.is_some());
    }

    #[test]
    fn on_click_stored() {
        let c = Card::new("a").on_click(|_, _, _| {});
        assert!(c.on_click.is_some());
    }
}
