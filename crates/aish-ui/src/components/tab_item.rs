//! TabItem — 顶部 tab 栏单项。薄布局 + 3 slot。
//!
//! Tab item 业务多变（连接状态 dot / inline rename / SSH chip / close 按钮），
//! TabItem 不试图通用化所有细节，只提供 prefix / title / suffix 三 slot + active
//! + on_click（透传 click_count）让调用方在 slot 内拼自己业务。
//!
//! M34: 升 stateful Entity，加 hover transition + press feedback。caller 持
//! Entity<TabItem>，render 每帧 update 重设 slots（AnyElement 不可 Clone）。
//! active=true 时跳过 hover 状态机推进（同 NavItem 模式 M34 polish）。

use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, Animation, AnyElement, App, BoxShadow, Context, ElementId,
    FocusHandle, IntoElement, MouseButton, MouseDownEvent, Window,
};

use crate::components::button::{press_opacity_at, HoverState};
use crate::theme::{animate_or_skip, theme};
use crate::TypographyExt;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

pub struct TabItem {
    id: ElementId,
    prefix: Option<AnyElement>,
    title: Option<AnyElement>,
    suffix: Option<AnyElement>,
    active: bool,
    on_click: Option<ClickHandler>,
    focus_handle: FocusHandle,
    pressing: bool,
    focus_animated: bool,
    was_focused_prev: bool,
    press_count: u64,
    hover_state: HoverState,
    hover_anim_count: u64,
    /// active indicator fade-in 动画。active false → true 时启动 150ms
    /// opacity 0→1 渐显（强调"切换到本 tab"的视觉反馈）。active true → false
    /// 不 fade out（instant 消失，让用户清晰看到失活）。
    active_fading_in: bool,
    /// active_fading_in 的 anim_count 幂等 check — 与 press_count 同模式。
    active_anim_count: u64,
    /// 上帧 active 值，用于 render head detect active 0→1 跳变启动 fade。
    was_active_prev: bool,
}

impl TabItem {
    pub fn new(id: impl Into<ElementId>, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            prefix: None,
            title: None,
            suffix: None,
            active: false,
            on_click: None,
            focus_handle: cx.focus_handle(),
            pressing: false,
            focus_animated: false,
            was_focused_prev: false,
            press_count: 0,
            hover_state: HoverState::Idle,
            hover_anim_count: 0,
            active_fading_in: false,
            active_anim_count: 0,
            was_active_prev: false,
        }
    }

    pub fn prefix(&mut self, p: impl IntoElement) -> &mut Self {
        self.prefix = Some(p.into_any_element());
        self
    }

    pub fn title(&mut self, t: impl IntoElement) -> &mut Self {
        self.title = Some(t.into_any_element());
        self
    }

    pub fn suffix(&mut self, s: impl IntoElement) -> &mut Self {
        self.suffix = Some(s.into_any_element());
        self
    }

    pub fn active(&mut self, a: bool) -> &mut Self {
        self.active = a;
        self
    }

    pub fn on_click(
        &mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_click = Some(Rc::new(h));
        self
    }

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

    /// M34 v2: hover 状态机推进（含 Leaving 反向 lerp）。
    /// active=true 时跳过 hover 状态机推进（active selected 视觉保持稳态）—
    /// 同 NavItem 模式。
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

impl gpui::Focusable for TabItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TabItem {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now_focused = self.focus_handle.is_focused(window);
        if !self.was_focused_prev && now_focused {
            self.focus_animated = true;
            self.schedule_clear_focus_anim(cx);
        } else if self.was_focused_prev && !now_focused {
            self.focus_animated = false;
        }
        self.was_focused_prev = now_focused;

        // active 0→1 跳变 → 启动 indicator fade-in。reduced_motion 跳过动画。
        // active 1→0 不 fade out（instant 消失，让用户清晰看到失活）。
        if !self.was_active_prev && self.active {
            let reduced = theme(cx).reduced_motion;
            if !reduced {
                self.active_fading_in = true;
                self.active_anim_count = self.active_anim_count.wrapping_add(1);
                let expected = self.active_anim_count;
                let dur = theme(cx).motion.medium;
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(dur).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.active_anim_count == expected && this.active_fading_in {
                            this.active_fading_in = false;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        } else if self.was_active_prev && !self.active {
            // 失活 instant — 取消任何残留 fade
            self.active_fading_in = false;
        }
        self.was_active_prev = self.active;

        let t = theme(cx);
        let active = self.active;
        let selected_bg = t.colors.background;
        let idle_bg = t.colors.card;
        let hover_bg = t.colors.secondary_hover;
        let active_press_bg = t.colors.secondary_active;
        let primary = t.colors.primary;
        let ring_color = t.colors.ring;
        let spacing_px_4 = t.spacing.px_4;
        let spacing_px_2 = t.spacing.px_2;
        let medium = t.motion.medium;
        let easing = t.motion.easing_standard.clone();
        let typography_t = t;

        let hover_state = self.hover_state;
        let pressing = self.pressing;
        let focus_animating = now_focused && self.focus_animated;
        let hover_entering = !active && matches!(hover_state, HoverState::Entering { .. });
        let hover_leaving = !active && matches!(hover_state, HoverState::Leaving { .. });
        let active_fading_in = self.active_fading_in;
        let need_anim =
            pressing || focus_animating || hover_entering || hover_leaving || active_fading_in;
        let press_count = self.press_count;
        let hover_anim_count = self.hover_anim_count;
        let active_anim_count = self.active_anim_count;

        // base_bg robustness：active 用 selected；非 active 永远 idle_bg。
        // 实际 hover 视觉由 non-animate 路径的 GPUI .hover() / animate 路径
        // 的 closure lerp 接管，防 hover_state 卡 Hovered 时 bg 不消（与
        // NavItem / Card / ListRow 同模式）。
        let base_bg = if active { selected_bg } else { idle_bg };

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
            .relative()
            // h_full 让 TabItem 适配 tab_bar content area（自动让 1px
            // border-b 给容器），防 hover bg / active selected_bg 覆盖
            // tab_bar.border_b 视觉断线（同 plus_btn fix 模式）。
            .h_full()
            .max_w(px(200.0))
            .overflow_hidden()
            .flex_shrink_0()
            .px(spacing_px_4)
            .flex()
            .flex_row()
            .items_center()
            .gap(spacing_px_2)
            .typography(crate::TypeRole::Body, typography_t)
            .bg(base_bg)
            .cursor_pointer()
            .track_focus(&self.focus_handle);

        if !active {
            el = el
                .active(move |s| s.bg(active_press_bg))
                .on_mouse_down(MouseButton::Left, on_press_listener)
                .on_hover(on_hover_listener);
        } else if self.on_click.is_some() {
            el = el.on_mouse_down(MouseButton::Left, on_press_listener);
        }

        let prefix = self.prefix.take();
        let title = self.title.take();
        let suffix = self.suffix.take();
        el = el.when_some(prefix, |d, p| d.child(p));
        el = el.when_some(title, |d, ti| {
            d.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(ti),
            )
        });
        el = el.when_some(suffix, |d, s| d.child(s));

        // 非动画 path 内 active 直接画 indicator(opacity=1)；动画 path 内
        // 走 closure 用 delta 控 opacity（active_fading_in 时 0→1，否则 1）。
        if !need_anim {
            if active {
                el = el.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(2.0))
                        .bg(primary),
                );
            } else {
                // 非 active 时附 GPUI .hover() 真实视觉源（机制同 NavItem）
                el = el.hover(move |s| s.bg(hover_bg));
            }
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
        let anim_id: ElementId = (
            "motion-tab-item",
            press_count
                .wrapping_add(hover_anim_count)
                .wrapping_add(active_anim_count) as usize,
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
                    el = el.bg(crate::lerp_hsla(idle_bg, hover_bg, delta));
                } else if hover_leaving {
                    // M34 v2: 反向 lerp hover → idle
                    el = el.bg(crate::lerp_hsla(hover_bg, idle_bg, delta));
                }
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
                // active indicator: active_fading_in 时 opacity=delta（0→1）；
                // 否则 active=true 都画 opacity=1。non-active 不画。
                if active {
                    let indicator_opacity = if active_fading_in { delta } else { 1.0 };
                    el = el.child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(px(2.0))
                            .bg(primary)
                            .opacity(indicator_opacity),
                    );
                }
                el
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_hover_when_active(active: bool, hovered: bool, prev: HoverState) -> HoverState {
        if active {
            return prev;
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

    /// active indicator fade-in detect 纯逻辑：was_active=false 且 now_active=true
    /// → 启动 fade-in；其它组合不启动。
    fn should_start_indicator_fade_in(was_active: bool, now_active: bool) -> bool {
        !was_active && now_active
    }

    #[test]
    fn fade_in_starts_on_inactive_to_active() {
        assert!(should_start_indicator_fade_in(false, true));
    }

    #[test]
    fn fade_in_no_start_on_active_to_inactive() {
        assert!(!should_start_indicator_fade_in(true, false));
    }

    #[test]
    fn fade_in_no_start_on_steady_state() {
        assert!(!should_start_indicator_fade_in(true, true));
        assert!(!should_start_indicator_fade_in(false, false));
    }

    /// indicator opacity 选择：active_fading_in=true 用 delta，否则 1.0
    /// （non-active 不进入这个 branch）。
    fn indicator_opacity_at(active_fading_in: bool, delta: f32) -> f32 {
        if active_fading_in {
            delta
        } else {
            1.0
        }
    }

    #[test]
    fn indicator_opacity_lerps_when_fading() {
        assert_eq!(indicator_opacity_at(true, 0.0), 0.0);
        assert_eq!(indicator_opacity_at(true, 0.5), 0.5);
        assert_eq!(indicator_opacity_at(true, 1.0), 1.0);
    }

    #[test]
    fn indicator_opacity_full_when_not_fading() {
        assert_eq!(indicator_opacity_at(false, 0.0), 1.0);
        assert_eq!(indicator_opacity_at(false, 0.5), 1.0);
    }
}
