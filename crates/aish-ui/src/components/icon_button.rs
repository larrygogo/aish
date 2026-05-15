//! IconButton — 仅 icon 的方形按钮。复用 Button 的 variant 系统。
//!
//! M31：旁挂 `IconButton` stateful 版本（与 Button 对称）。
//! 旧 stateless `IconButton` T6 阶段统一删除 + rename。

use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, Animation, App, BoxShadow, Context, ElementId, FocusHandle,
    IntoElement, MouseButton, MouseDownEvent, Pixels, Window,
};

use crate::components::button::{pick_button_colors, press_opacity_at};
use crate::components::ButtonVariant;
use crate::icons::{icon, IconName};
use crate::theme::{animate_or_skip, theme};

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

/// Stateful IconButton — 与 Button 对称（press + focus_animated）。
pub struct IconButton {
    id: ElementId,
    icon_name: IconName,
    variant: ButtonVariant,
    size: IconButtonSize,
    disabled: bool,
    on_click: Option<ClickHandler>,
    focus_handle: FocusHandle,
    pressing: bool,
    focus_animated: bool,
    was_focused_prev: bool,
    press_count: u64,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon_name: IconName, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            icon_name,
            variant: ButtonVariant::Ghost,
            size: IconButtonSize::Md,
            disabled: false,
            on_click: None,
            focus_handle: cx.focus_handle(),
            pressing: false,
            focus_animated: false,
            was_focused_prev: false,
            press_count: 0,
        }
    }

    pub fn primary(&mut self) -> &mut Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn secondary(&mut self) -> &mut Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    pub fn destructive(&mut self) -> &mut Self {
        self.variant = ButtonVariant::Destructive;
        self
    }

    pub fn ghost(&mut self) -> &mut Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn small(&mut self) -> &mut Self {
        self.size = IconButtonSize::Sm;
        self
    }

    pub fn medium(&mut self) -> &mut Self {
        self.size = IconButtonSize::Md;
        self
    }

    pub fn large(&mut self) -> &mut Self {
        self.size = IconButtonSize::Lg;
        self
    }

    pub fn disabled(&mut self, d: bool) -> &mut Self {
        self.disabled = d;
        self
    }

    pub fn on_click(
        &mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_click = Some(Rc::new(h));
        self
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn fire_press(&mut self, cx: &mut Context<Self>) {
        self.pressing = true;
        self.press_count = self.press_count.wrapping_add(1);
        let expected = self.press_count;
        // press 用 medium 150ms（M31 v2 UX 调整，详 button.rs 注释）
        let dur = theme(cx).motion.medium;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(dur).await;
            let _ = this.update(cx, |this, cx| {
                if this.press_count == expected && this.pressing {
                    this.pressing = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn schedule_clear_focus_anim(&mut self, cx: &mut Context<Self>) {
        let dur = theme(cx).motion.medium;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(dur).await;
            let _ = this.update(cx, |this, cx| {
                if this.focus_animated {
                    this.focus_animated = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl gpui::Focusable for IconButton {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for IconButton {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now_focused = self.focus_handle.is_focused(window);
        if !self.was_focused_prev && now_focused {
            self.focus_animated = true;
            self.schedule_clear_focus_anim(cx);
        } else if self.was_focused_prev && !now_focused {
            self.focus_animated = false;
        }
        self.was_focused_prev = now_focused;

        let t = theme(cx);
        let disabled = self.disabled;
        let (idle_bg, hover_bg, active_bg, fg) = pick_button_colors(self.variant, disabled, t);
        let bs = self.size.box_size();
        let isz = self.size.icon_size();
        let radius_sm = t.radius.sm;
        let ring_color = t.colors.ring;
        // press + ring fade 共用 medium 150ms（M31 v2 UX 调整）
        let fast_duration = t.motion.medium;
        let easing = t.motion.easing_standard.clone();

        let pressing = self.pressing;
        let focus_animating = now_focused && self.focus_animated;
        let need_anim = pressing || focus_animating;
        let press_count = self.press_count;

        let handler = self.on_click.clone();
        let on_press_listener = cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
            this.fire_press(cx);
            if let Some(h) = handler.clone() {
                h(ev, window, cx);
            }
        });

        let mut el = div()
            .id(self.id.clone())
            .w(bs)
            .h(bs)
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius_sm)
            .bg(idle_bg)
            .track_focus(&self.focus_handle)
            .child(icon(self.icon_name).size(isz).text_color(fg));

        if !disabled {
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg))
                .on_mouse_down(MouseButton::Left, on_press_listener);
        } else {
            el = el.cursor_not_allowed().opacity(0.6);
        }

        if !need_anim {
            if now_focused {
                let mut glow = ring_color;
                glow.a = 0.4;
                el = el.shadow(vec![BoxShadow {
                    color: glow,
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(4.0),
                    spread_radius: px(2.0),
                }]);
            }
            return el.into_any_element();
        }

        let ring_show_static = now_focused && !focus_animating;
        let anim_id: ElementId = ("motion-icon-btn", press_count as usize).into();

        animate_or_skip(
            el,
            t,
            anim_id,
            Animation::new(fast_duration).with_easing(move |d| easing(d)),
            move |el, delta| {
                let mut el = el;
                if focus_animating {
                    let mut glow = ring_color;
                    glow.a = 0.4 * delta;
                    el = el.shadow(vec![BoxShadow {
                        color: glow,
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(4.0),
                        spread_radius: px(2.0),
                    }]);
                } else if ring_show_static {
                    let mut glow = ring_color;
                    glow.a = 0.4;
                    el = el.shadow(vec![BoxShadow {
                        color: glow,
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(4.0),
                        spread_radius: px(2.0),
                    }]);
                }
                if pressing {
                    el = el.opacity(press_opacity_at(delta));
                }
                el
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IconButton 大小关系断言（pure fn 测试，不构造 entity）。
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
