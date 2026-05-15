//! Card — 卡片容器。header / body / footer 三 slot + variant + on_click。
//!
//! `AnyElement` 不可 Clone，Card 走 `#[derive(IntoElement)] + RenderOnce`
//! 一次性消费（与 Dialog body 同模式）。每帧调用方通过 builder 重新构造。
//!
//! ## M27 内置 padding（默认开启）
//!
//! 每个 slot 默认应用 `t.anatomy.card.{header,body,footer}_{px,py}`：
//! - header: 16/10
//! - body: 16/12
//! - footer: 16/10
//!
//! caller 的 child（如 settings section_header / two_column_row）不应再
//! 自己设 px_4/py_3 — 会与 Card 内置 padding 双重叠加。需要 opt-out 时调
//! `.no_padding()`（host card 自身 row 已 px_4/py_3 padded 的场景）。
//!
//! **关于"hover 才显示的浮层"（如编辑/删除按钮）**：Card 不内置 actions
//! 浮层（之前尝试用 absolute + group_hover 内置但定位歧义大）。需要这种效果
//! 时，调用方自己在 body 内安排：body 容器 `.relative()` + `.group(name)`，
//! 浮层 `.absolute().opacity(0).group_hover(name, |s| s.opacity(1.0))`。

use std::rc::Rc;

use gpui::{
    div, prelude::*, Animation, AnyElement, App, Context, ElementId, FocusHandle, IntoElement,
    MouseButton, MouseDownEvent, Window,
};

use crate::components::button::{hover_bg_at, press_opacity_at, HoverState};
use crate::theme::{animate_or_skip, theme};

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
    /// M27：内置 padding 开关。默认 true（每 slot 走 anatomy.card.*_p{x,y}）。
    /// caller 自身 child 已 padded 时调 `.no_padding()` opt-out 防双重叠加
    /// （典型场景：host card body 是 `.px_4.py_3` 的 row 容器）。
    padding: bool,
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
            padding: true,
        }
    }

    /// 关闭内置 padding —— caller 自己控制每 slot 内部 padding。
    /// 用于 host card 这种 body 已是完整 padded row 的场景。
    pub fn no_padding(mut self) -> Self {
        self.padding = false;
        self
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
                // M24/M25：Elevated 用 hairline border + elevation_1 shadow
                // 浮起（Linear/Warp 风），替代之前用 ring 色 border 的 hack
                // （ring 现在是 indigo，紫 border 太抢眼与 Card 主信息冲突）
                el = el
                    .border_1()
                    .border_color(t.colors.border)
                    .shadow(crate::theme::elevation_1(t.kind));
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

        // M27 内置 padding（默认）：每 slot 包裹 div 应用
        // anatomy.card.{header/body/footer}_{px/py}。.no_padding() opt-out 时
        // 走 M11 原行为（裸 child 不加 wrapper padding）。
        let pad = self.padding;
        let a = t.anatomy.card;
        el.when_some(self.header, |d, h| {
            let wrapper = if pad {
                div().px(a.header_px).py(a.header_py)
            } else {
                div()
            };
            d.child(wrapper.child(h))
        })
        .when_some(self.body, |d, b| {
            let wrapper = if pad {
                div().flex_1().px(a.body_px).py(a.body_py)
            } else {
                div().flex_1()
            };
            d.child(wrapper.child(b))
        })
        .when_some(self.footer, |d, f| {
            let wrapper = if pad {
                div().px(a.footer_px).py(a.footer_py)
            } else {
                div()
            };
            d.child(wrapper.child(f))
        })
    }
}

// ============================================================================
// M33：CardEntity — stateful Card 支持 hover transition + press feedback。
// ============================================================================

/// M33: stateful Card。caller 持 `Entity<CardEntity>` 字段，render 时
/// `.update(cx, |c, _| c.header(...).body(...))` 重设 slots（AnyElement
/// 不可 Clone）+ `.clone()` 嵌入元素树。
///
/// 仅当 `on_click` 设置时启用 hover transition + press feedback；
/// 无 on_click 时（如 Settings 装饰 Card）走简化 render，无 listener / timer。
pub struct CardEntity {
    id: ElementId,
    header: Option<AnyElement>,
    body: Option<AnyElement>,
    footer: Option<AnyElement>,
    variant: CardVariant,
    on_click: Option<ClickHandler>,
    padding: bool,
    focus_handle: FocusHandle,
    pressing: bool,
    focus_animated: bool,
    was_focused_prev: bool,
    press_count: u64,
    hover_state: HoverState,
    hover_anim_count: u64,
}

impl CardEntity {
    pub fn new(id: impl Into<ElementId>, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            header: None,
            body: None,
            footer: None,
            variant: CardVariant::Default,
            on_click: None,
            padding: true,
            focus_handle: cx.focus_handle(),
            pressing: false,
            focus_animated: false,
            was_focused_prev: false,
            press_count: 0,
            hover_state: HoverState::Idle,
            hover_anim_count: 0,
        }
    }

    pub fn no_padding(&mut self) -> &mut Self {
        self.padding = false;
        self
    }

    pub fn header(&mut self, h: impl IntoElement) -> &mut Self {
        self.header = Some(h.into_any_element());
        self
    }

    pub fn body(&mut self, b: impl IntoElement) -> &mut Self {
        self.body = Some(b.into_any_element());
        self
    }

    pub fn footer(&mut self, f: impl IntoElement) -> &mut Self {
        self.footer = Some(f.into_any_element());
        self
    }

    pub fn variant(&mut self, v: CardVariant) -> &mut Self {
        self.variant = v;
        self
    }

    pub fn outlined(&mut self) -> &mut Self {
        self.variant(CardVariant::Outlined)
    }

    pub fn elevated(&mut self) -> &mut Self {
        self.variant(CardVariant::Elevated)
    }

    pub fn on_click(
        &mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_click = Some(Rc::new(h));
        self
    }

    /// M33: 与 button.rs::fire_press 同模式（press_count 幂等 check）。
    fn fire_press(&mut self, cx: &mut Context<Self>) {
        self.pressing = true;
        self.press_count = self.press_count.wrapping_add(1);
        let expected = self.press_count;
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

    /// M33: 与 button.rs::fire_hover 同模式。
    fn fire_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            if matches!(self.hover_state, HoverState::Idle) {
                let reduced = theme(cx).reduced_motion;
                if reduced {
                    self.hover_state = HoverState::Hovered;
                    cx.notify();
                } else {
                    let count = self.hover_anim_count.wrapping_add(1);
                    self.hover_anim_count = count;
                    self.hover_state = HoverState::Entering { anim_count: count };
                    let dur = theme(cx).motion.medium;
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(dur).await;
                        let _ = this.update(cx, |this, cx| {
                            if matches!(
                                this.hover_state,
                                HoverState::Entering { anim_count } if anim_count == count
                            ) {
                                this.hover_state = HoverState::Hovered;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                    cx.notify();
                }
            }
        } else if !matches!(self.hover_state, HoverState::Idle) {
            self.hover_state = HoverState::Idle;
            cx.notify();
        }
    }
}

impl gpui::Focusable for CardEntity {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CardEntity {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // focus 跨帧检测（仅 on_click 卡片才参与 focus，但状态机仍统一推进）
        let now_focused = self.focus_handle.is_focused(window);
        if !self.was_focused_prev && now_focused {
            self.focus_animated = true;
            self.schedule_clear_focus_anim(cx);
        } else if self.was_focused_prev && !now_focused {
            self.focus_animated = false;
        }
        self.was_focused_prev = now_focused;

        let t = theme(cx);
        // Card 所有 variant 的 idle bg 都是 t.colors.card；hover/active 用 secondary
        // 灰阶（与 stateless 时代逻辑一致，line 137-149）
        let idle_bg = t.colors.card;
        let hover_bg = t.colors.secondary_hover;
        let active_bg = t.colors.secondary_active;
        let radius_lg = t.radius.lg;
        let border_color = t.colors.border;
        let theme_kind = t.kind;
        let fast_duration = t.motion.medium;
        let easing = t.motion.easing_standard.clone();

        let has_click = self.on_click.is_some();
        let pressing = self.pressing;
        let focus_animating = now_focused && self.focus_animated && has_click;
        let hover_state = self.hover_state;
        let hover_entering = has_click && matches!(hover_state, HoverState::Entering { .. });
        let need_anim = pressing || focus_animating || hover_entering;
        let press_count = self.press_count;
        let hover_anim_count = self.hover_anim_count;
        let variant = self.variant;

        // hover-aware base bg（仅 on_click 启用 hover；无 on_click 时永远 idle_bg）
        let base_bg = if has_click {
            match hover_state {
                HoverState::Idle | HoverState::Entering { .. } => idle_bg,
                HoverState::Hovered => hover_bg,
            }
        } else {
            idle_bg
        };

        let handler = self.on_click.clone();
        let on_press_listener = cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
            this.fire_press(cx);
            if let Some(h) = handler.clone() {
                h(ev, window, cx);
            }
        });
        let on_hover_listener = cx.listener(move |this, &hovered: &bool, _w, cx| {
            this.fire_hover(hovered, cx);
        });

        let mut el = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .bg(base_bg)
            .rounded(radius_lg);

        match variant {
            CardVariant::Default => {}
            CardVariant::Outlined => {
                el = el.border_1().border_color(border_color);
            }
            CardVariant::Elevated => {
                el = el
                    .border_1()
                    .border_color(border_color)
                    .shadow(crate::theme::elevation_1(theme_kind));
            }
        }

        if has_click {
            el = el
                .cursor_pointer()
                .active(move |s| s.bg(active_bg))
                .on_mouse_down(MouseButton::Left, on_press_listener)
                .on_hover(on_hover_listener);
        }

        // slots：M27 内置 padding 路径与 stateless 一致
        let pad = self.padding;
        let a = t.anatomy.card;
        let header = self.header.take();
        let body = self.body.take();
        let footer = self.footer.take();
        el = el
            .when_some(header, |d, h| {
                let wrapper = if pad {
                    div().px(a.header_px).py(a.header_py)
                } else {
                    div()
                };
                d.child(wrapper.child(h))
            })
            .when_some(body, |d, b| {
                let wrapper = if pad {
                    div().flex_1().px(a.body_px).py(a.body_py)
                } else {
                    div().flex_1()
                };
                d.child(wrapper.child(b))
            })
            .when_some(footer, |d, f| {
                let wrapper = if pad {
                    div().px(a.footer_px).py(a.footer_py)
                } else {
                    div()
                };
                d.child(wrapper.child(f))
            });

        if !need_anim {
            return el.into_any_element();
        }

        // 动画路径：单 animate_or_skip 同时驱动 hover bg lerp + press opacity
        let anim_id: ElementId = (
            "motion-card",
            press_count.wrapping_add(hover_anim_count) as usize,
        )
            .into();

        animate_or_skip(
            el,
            t,
            anim_id,
            Animation::new(fast_duration).with_easing(move |d| easing(d)),
            move |el, delta| {
                let mut el = el;
                if hover_entering {
                    el = el.bg(hover_bg_at(
                        crate::components::ButtonVariant::Primary,
                        idle_bg,
                        hover_bg,
                        delta,
                    ));
                }
                if pressing {
                    el = el.opacity(press_opacity_at(delta));
                }
                // focus_animating: Card 通常无 ring（focus_animating=true 仅
                // 在 has_click 时为 true，但 Card 没装 focus ring 视觉。这里
                // 不画 ring — Card 不是 actionable label，与 Button 视觉模式
                // 不一样。若以后需要可加 shadow ring）
                let _ = focus_animating;
                el
            },
        )
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

    #[test]
    fn padding_default_true() {
        // M27: Card 默认带 anatomy.card.* padding
        let c = Card::new("a");
        assert!(c.padding);
    }

    #[test]
    fn no_padding_chains() {
        let c = Card::new("a").no_padding();
        assert!(!c.padding);
    }
}
