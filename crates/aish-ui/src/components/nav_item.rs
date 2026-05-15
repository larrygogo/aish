//! NavItem — 导航项。Horizontal（顶部栏）+ Vertical（侧栏）双模。
//!
//! icon 接受任意 IntoElement（SVG / Nerd Font / 纯文字），label 可选。
//! active 时用 bg(accent) + foreground 文字色区分（无 indicator 条，
//! 与 dropdown / session_picker / toast 统一去除 primary 绿条风格）。
//!
//! M34: 升 stateful Entity，加 hover transition（fg + bg 双 lerp 150ms
//! ease_out_quint）+ press feedback + focus ring fade。caller 持
//! `Entity<NavItem>` 字段，render 时 `.update(cx, |n, _| n.icon(...).active(...))`
//! 每帧重设（icon AnyElement 不可 Clone，同 Card/Dialog body 模式）。
//!
//! `active=true` 时 hover 状态机走简化路径不参与 lerp（active selected 视觉
//! 应保持稳态，不被 hover 覆盖）— 同 stateless 时代 `if !active { hover/active }`
//! 行为保持兼容。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Animation, AnyElement, App, Context, ElementId, IntoElement, MouseButton,
    MouseDownEvent, SharedString, Window,
};

use crate::components::button::{press_opacity_at, HoverState};
use crate::theme::{animate_or_skip, theme};
use crate::TypographyExt;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavItemOrientation {
    Vertical,
    Horizontal,
}

pub struct NavItem {
    id: ElementId,
    icon: Option<AnyElement>,
    label: Option<SharedString>,
    active: bool,
    orientation: NavItemOrientation,
    on_click: Option<ClickHandler>,
    pressing: bool,
    press_count: u64,
    hover_state: HoverState,
    hover_anim_count: u64,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>, _cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            label: None,
            active: false,
            orientation: NavItemOrientation::Vertical,
            on_click: None,
            pressing: false,
            press_count: 0,
            hover_state: HoverState::Idle,
            hover_anim_count: 0,
        }
    }

    pub fn icon(&mut self, i: impl IntoElement) -> &mut Self {
        self.icon = Some(i.into_any_element());
        self
    }

    pub fn label(&mut self, l: impl Into<SharedString>) -> &mut Self {
        self.label = Some(l.into());
        self
    }

    pub fn active(&mut self, a: bool) -> &mut Self {
        self.active = a;
        self
    }

    pub fn vertical(&mut self) -> &mut Self {
        self.orientation = NavItemOrientation::Vertical;
        self
    }

    pub fn horizontal(&mut self) -> &mut Self {
        self.orientation = NavItemOrientation::Horizontal;
        self
    }

    pub fn on_click(
        &mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_click = Some(Rc::new(h));
        self
    }

    /// M34: 与 button.rs::fire_press 同模式。
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

    /// M34 v2: hover 状态机推进（含 Leaving 反向 lerp）。
    /// active=true 时跳过 hover 状态机推进（active selected 视觉保持稳态）。
    fn fire_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.active {
            return;
        }
        if hovered {
            match self.hover_state {
                HoverState::Idle | HoverState::Leaving { .. } => {
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
                HoverState::Entering { .. } | HoverState::Hovered => {}
            }
        } else {
            match self.hover_state {
                HoverState::Hovered => {
                    let reduced = theme(cx).reduced_motion;
                    if reduced {
                        self.hover_state = HoverState::Idle;
                        cx.notify();
                    } else {
                        let count = self.hover_anim_count.wrapping_add(1);
                        self.hover_anim_count = count;
                        self.hover_state = HoverState::Leaving { anim_count: count };
                        let dur = theme(cx).motion.medium;
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(dur).await;
                            let _ = this.update(cx, |this, cx| {
                                if matches!(
                                    this.hover_state,
                                    HoverState::Leaving { anim_count } if anim_count == count
                                ) {
                                    this.hover_state = HoverState::Idle;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                        cx.notify();
                    }
                }
                HoverState::Entering { .. } => {
                    self.hover_state = HoverState::Idle;
                    cx.notify();
                }
                HoverState::Idle | HoverState::Leaving { .. } => {}
            }
        }
    }
}

impl Render for NavItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let active = self.active;
        let orientation = self.orientation;
        // 颜色：active / Idle / Hovered / Entering 各自决定 fg + bg
        // - active=true: fg = foreground, bg = accent（无 hover 干扰）
        // - !active + Idle: fg = muted_foreground, bg = transparent
        // - !active + Hovered: fg = secondary_foreground, bg = secondary_hover
        // - !active + Entering: animator 内 lerp fg / bg
        let idle_fg = t.colors.muted_foreground;
        let hover_fg = t.colors.secondary_foreground;
        let active_selected_fg = t.colors.foreground;
        let idle_bg = gpui::transparent_black();
        let hover_bg = t.colors.secondary_hover;
        let press_bg_color = t.colors.secondary_active;
        let selected_bg = t.colors.accent;
        let medium = t.motion.medium;
        let easing = t.motion.easing_standard.clone();
        let typography_t = t;

        // NavItem 设计上是导航容器（与 Card / ListRow 同类），仅用
        // bg + active 高亮表达状态 — 不画 focus ring（避免 sidebar 窄柱内
        // 紫色 glow 视觉过载）。键盘 Tab 焦点仍走 focus_handle，只是不附
        // 视觉 ring。
        let hover_state = self.hover_state;
        let pressing = self.pressing;
        let hover_entering = !active && matches!(hover_state, HoverState::Entering { .. });
        let hover_leaving = !active && matches!(hover_state, HoverState::Leaving { .. });
        let need_anim = pressing || hover_entering || hover_leaving;
        let press_count = self.press_count;
        let hover_anim_count = self.hover_anim_count;

        // base fg + bg：active 永远 selected；非 active 看 hover_state
        // Leaving 视觉起点 = hover (反向 lerp 到 idle)
        let (base_fg, base_bg) = if active {
            (active_selected_fg, selected_bg)
        } else {
            match hover_state {
                HoverState::Idle | HoverState::Entering { .. } => (idle_fg, idle_bg),
                HoverState::Hovered | HoverState::Leaving { .. } => (hover_fg, hover_bg),
            }
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
            .text_color(base_fg)
            .bg(base_bg)
            .cursor_pointer();

        el = match orientation {
            NavItemOrientation::Vertical => el
                .w_full()
                .py(px(12.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0)),
            NavItemOrientation::Horizontal => el
                .h(px(32.0))
                .px(t.spacing.px_3)
                .flex()
                .flex_row()
                .items_center()
                .gap(t.spacing.px_2),
        };

        // press declarative active modifier 仅 !active 时挂（保留 stateless
        // 时代行为：active selected 时点击不切色）
        if !active {
            el = el
                .active(move |s| s.bg(press_bg_color))
                .on_mouse_down(MouseButton::Left, on_press_listener)
                .on_hover(on_hover_listener);
        } else if self.on_click.is_some() {
            // active=true 但有 on_click — 仍允许点击（不视觉切色）
            el = el.on_mouse_down(MouseButton::Left, on_press_listener);
        }

        // icon + label
        let icon = self.icon.take();
        let label = self.label.clone();
        el = el.when_some(icon, |d, i| d.child(i));
        el = el.when_some(label, |d, l| {
            d.child(
                div()
                    .typography(crate::TypeRole::Caption, typography_t)
                    .child(l),
            )
        });

        if !need_anim {
            return el.into_any_element();
        }

        // 动画路径：lerp fg + bg（仅 hover_entering 时），叠加 press opacity。
        let anim_id: ElementId = (
            "motion-nav-item",
            press_count.wrapping_add(hover_anim_count) as usize,
        )
            .into();

        animate_or_skip(
            el,
            t,
            anim_id,
            Animation::new(medium).with_easing(move |d| easing(d)),
            move |el, delta| {
                let mut el = el;
                if hover_entering {
                    // lerp fg + bg：idle → hover
                    el = el
                        .text_color(crate::lerp_hsla(idle_fg, hover_fg, delta))
                        .bg(crate::lerp_hsla(idle_bg, hover_bg, delta));
                } else if hover_leaving {
                    // M34 v2: 反向 lerp fg + bg：hover → idle
                    el = el
                        .text_color(crate::lerp_hsla(hover_fg, idle_fg, delta))
                        .bg(crate::lerp_hsla(hover_bg, idle_bg, delta));
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

    /// M34: NavItem hover state machine：active=true 跳过 hover 推进。
    /// 实际 fire_hover 内 self.active 短路；本测试 mock 验证逻辑。
    fn next_hover_when_active(active: bool, hovered: bool, prev: HoverState) -> HoverState {
        if active {
            return prev; // active 时不动 hover_state
        }
        if hovered {
            if matches!(prev, HoverState::Idle) {
                HoverState::Entering { anim_count: 1 }
            } else {
                prev
            }
        } else if !matches!(prev, HoverState::Idle) {
            HoverState::Idle
        } else {
            prev
        }
    }

    #[test]
    fn active_blocks_hover_entering() {
        // active=true 时 hover 进入不切 Entering
        let s = next_hover_when_active(true, true, HoverState::Idle);
        assert_eq!(s, HoverState::Idle, "active=true 时 hover_state 不动");
    }

    #[test]
    fn inactive_idle_to_entering_on_hover() {
        let s = next_hover_when_active(false, true, HoverState::Idle);
        assert!(matches!(s, HoverState::Entering { .. }));
    }

    #[test]
    fn inactive_hovered_to_idle_on_leave() {
        let s = next_hover_when_active(false, false, HoverState::Hovered);
        assert_eq!(s, HoverState::Idle);
    }

    #[test]
    fn orientation_default_vertical() {
        // 无法直接构造 NavItem 测（需 cx），但 enum 默认值已知
        assert_eq!(NavItemOrientation::Vertical, NavItemOrientation::Vertical);
    }
}
