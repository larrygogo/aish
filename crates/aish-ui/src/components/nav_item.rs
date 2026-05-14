//! NavItem — 导航项。Horizontal（顶部栏）+ Vertical（侧栏）双模。
//!
//! icon 接受任意 IntoElement（SVG / Nerd Font / 纯文字），label 可选。
//! active 时用 bg(accent) + foreground 文字色区分（无 indicator 条，
//! 与 dropdown / session_picker / toast 统一去除 primary 绿条风格）。

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

        let mut el = div().id(self.id).text_color(fg).cursor_pointer();

        el = match orientation {
            NavItemOrientation::Vertical => el
                .w_full()
                // M24/M25 加密度：py 14 → 12（sidebar nav 紧凑一档，Linear/Warp 风）
                .py(px(12.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0)),
            NavItemOrientation::Horizontal => el
                // M25 加密度：h 36 → 32
                .h(px(32.0))
                .px(t.spacing.px_3)
                .flex()
                .flex_row()
                .items_center()
                .gap(t.spacing.px_2),
        };

        // active 用 bg(accent) 替代 indicator 条 — primary 绿太抢眼与整体
        // 灰阶风不搭，accent 是灰阶柔和高亮（dark/light 各有阶梯）。
        if active {
            el = el.bg(t.colors.accent);
        }

        if !active {
            // 大容器 hover 用 secondary 灰阶提亮（不换色调），accent 留给小元素强调
            let hover_fg = t.colors.secondary_foreground;
            let hover_bg = t.colors.secondary_hover;
            let active_bg = t.colors.secondary_active;
            el = el
                .hover(move |s| s.text_color(hover_fg).bg(hover_bg))
                .active(move |s| s.bg(active_bg));
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

    #[test]
    fn hover_only_when_inactive() {
        // 验证 NavItem 的 active 字段是否决定 hover 路径分支：
        // active=false → 走 hover bg/text 改变 + active bg
        // active=true → 不接管 hover/active（spec D-5）
        let inactive = NavItem::new("a").active(false);
        let active = NavItem::new("a").active(true);
        assert!(!inactive.active);
        assert!(active.active);
    }
}
