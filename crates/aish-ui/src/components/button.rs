//! Button — 主要操作组件。4 个 variant + disabled。

use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, App, BoxShadow, ElementId, FocusHandle, IntoElement, MouseButton,
    MouseDownEvent, SharedString, Window,
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
    focus_handle: Option<FocusHandle>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            variant: ButtonVariant::Primary,
            disabled: false,
            on_click: None,
            focus_handle: None,
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

    pub fn focus_handle(mut self, h: FocusHandle) -> Self {
        self.focus_handle = Some(h);
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;

        // (idle_bg, hover_bg, active_bg, fg)
        let (idle_bg, hover_bg, active_bg, fg) = if disabled {
            // disabled 状态下面 `if !disabled { hover/active }` 不挂，
            // hover_bg / active_bg 这两个值实际不会被消费，三写仅占位。
            (
                t.colors.muted,
                t.colors.muted,
                t.colors.muted,
                t.colors.muted_foreground,
            )
        } else {
            match self.variant {
                ButtonVariant::Primary => (
                    t.colors.primary,
                    t.colors.primary_hover,
                    t.colors.primary_active,
                    t.colors.primary_foreground,
                ),
                ButtonVariant::Secondary => (
                    t.colors.secondary,
                    t.colors.secondary_hover,
                    t.colors.secondary_active,
                    t.colors.secondary_foreground,
                ),
                ButtonVariant::Destructive => (
                    t.colors.destructive,
                    t.colors.destructive_hover,
                    t.colors.destructive_active,
                    t.colors.destructive_foreground,
                ),
                ButtonVariant::Ghost => (
                    gpui::transparent_black(),
                    // Ghost hover/active 比"大容器 hover"亮一档：
                    //   idle: transparent
                    //   hover: secondary_active (#404040)  ← 容器 hover (#2a2a2a) + 一档
                    //   active: secondary_strongest (#525252) ← 再亮一档
                    // 原因：Ghost button 常嵌在 Card / Row 内部，当容器自己处于
                    // hover 状态（bg=secondary_hover #2a2a2a），按钮 hover 若同色
                    // 会与容器融为一体看不出。跳一档保证任何容器状态下按钮 hover
                    // 都有可识别的色块凸显。M18 token 化 #525252 → secondary_strongest。
                    t.colors.secondary_active,
                    t.colors.secondary_strongest,
                    t.colors.foreground,
                ),
            }
        };

        let ring = t.colors.ring;
        let is_focused = self
            .focus_handle
            .as_ref()
            .map(|h| h.is_focused(window))
            .unwrap_or(false);

        let mut el = div()
            .id(self.id)
            .h(t.spacing.px_3 + t.spacing.px_4) // ~28
            .px(t.spacing.px_3)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.md)
            .bg(idle_bg)
            .child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(fg)
                    .child(self.label),
            );

        if let Some(ref handle) = self.focus_handle {
            el = el.track_focus(handle);
        }

        if is_focused {
            // M24 D-9：focus ring 改 alpha glow 而非实色 2px spread —— ring
            // 色 (primary indigo) 加 alpha 0.4 + 4px blur + 2px spread，
            // Linear 风软光晕，不抢主信息。
            let mut glow = ring;
            glow.a = 0.4;
            el = el.shadow(vec![BoxShadow {
                color: glow,
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(4.0),
                spread_radius: px(2.0),
            }]);
        }

        if !disabled {
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
            if let Some(handler) = self.on_click {
                el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
            }
        } else {
            // Disabled 精细化：cursor 禁止图标 + opacity 60% 整体褪色。
            // 单靠 muted bg/fg 不够明显（在某些主题下与 idle 状态对比弱）；
            // opacity 让按钮明显"退场"在视觉重量上。on_click 永远不挂，
            // 即便误触也不发生。
            el = el.cursor_not_allowed().opacity(0.6);
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

    #[test]
    fn focus_handle_default_none() {
        let b = Button::new("a");
        assert!(b.focus_handle.is_none());
    }

    #[test]
    fn focus_handle_field_exists() {
        let b = Button::new("a");
        let _: &Option<FocusHandle> = &b.focus_handle;
    }
}
