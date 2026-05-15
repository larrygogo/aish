//! Button — 主要操作组件。4 个 variant + disabled。
//!
//! M31：旁挂 `Button` stateful 版本 — `cx.new(|cx| Button::new(id, cx)...)`
//! caller 持 Entity 字段，render 时 `.clone()`。引入：
//! - press feedback：mouse_down → 80ms opacity 0.85→1.0 ease_out_quint
//! - focus ring fade-in：focus 得到时 80ms opacity 0→1，失去时直接消失
//! - reduced_motion 偏好自动 fallback（M30 animate_or_skip）
//!
//! 旧 stateless `Button` 继续可用，T6 阶段统一删除 + rename
//! `Button → Button`。

use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, Animation, App, BoxShadow, Context, ElementId, FocusHandle,
    IntoElement, MouseButton, MouseDownEvent, SharedString, Window,
};

use crate::theme::{animate_or_skip, theme};
use crate::TypographyExt;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Destructive,
    Ghost,
}

/// Stateful Button — caller 持 `Entity<Button>` 字段。
///
/// 状态：
/// - `pressing`：mouse_down 触发后 80ms 内为 true，timer 清回 false
/// - `focus_animated`：focus 得到的一刻为 true，触发 80ms 渐显，timer 清
/// - `was_focused_prev`：跨帧 focus 状态比较，let render 决定是否触发 fade-in
/// - `press_count`：每次 mouse_down +1，让 ElementId 唯一让 GPUI 创建新
///   animation state（同 ID 复用 state 会让连点不重新播放）
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<ClickHandler>,
    focus_handle: FocusHandle,
    pressing: bool,
    focus_animated: bool,
    was_focused_prev: bool,
    press_count: u64,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            variant: ButtonVariant::Primary,
            disabled: false,
            on_click: None,
            focus_handle: cx.focus_handle(),
            pressing: false,
            focus_animated: false,
            was_focused_prev: false,
            press_count: 0,
        }
    }

    pub fn label(&mut self, l: impl Into<SharedString>) -> &mut Self {
        self.label = l.into();
        self
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

    /// 暴露 focus_handle 给 caller，对 M29 D-9 `Dialog::initial_focus()` 兼容。
    /// 与 stateless 版的 `.focus_handle(handle)` 注入语义不同 — Entity 内置
    /// focus_handle，caller 仅"读取"作为 initial_focus 目标使用。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 触发 press feedback — mouse_down listener 调用。状态机 + spawn 80ms
    /// timer。timer 用 weak.update + 幂等 check（与 dialog.rs schedule_state_transition
    /// 同模式 M30）。
    fn fire_press(&mut self, cx: &mut Context<Self>) {
        self.pressing = true;
        self.press_count = self.press_count.wrapping_add(1);
        let expected = self.press_count;
        let dur = theme(cx).motion.fast;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(dur).await;
            let _ = this.update(cx, |this, cx| {
                // 幂等 check：若期间再次按下，press_count 已变，本 timer 不动
                if this.press_count == expected && this.pressing {
                    this.pressing = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// focus_animated 80ms 后清 — 让 render 走非 animate 路径（focus ring
    /// 显示但不重播动画）。
    fn schedule_clear_focus_anim(&mut self, cx: &mut Context<Self>) {
        let dur = theme(cx).motion.fast;
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

impl gpui::Focusable for Button {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Button {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // focus 状态跨帧检测 — 从 false 切到 true 时触发 fade-in
        let now_focused = self.focus_handle.is_focused(window);
        if !self.was_focused_prev && now_focused {
            self.focus_animated = true;
            self.schedule_clear_focus_anim(cx);
        } else if self.was_focused_prev && !now_focused {
            // 失焦直接消失 ring（D-3 简化）
            self.focus_animated = false;
        }
        self.was_focused_prev = now_focused;

        // 提前把所有需要的 token 从 theme 取出来 — 避免 cx borrow 跨闭包冲突
        let t = theme(cx);
        let disabled = self.disabled;
        let (idle_bg, hover_bg, active_bg, fg) = pick_button_colors(self.variant, disabled, t);
        let height = t.spacing.px_3 + t.spacing.px_4;
        let padding_x = t.spacing.px_3;
        let radius = t.radius.md;
        let ring_color = t.colors.ring;
        let fast_duration = t.motion.fast;
        let easing = t.motion.easing_standard.clone();
        let static_ring = ring_shadow(t, 1.0);

        let pressing = self.pressing;
        let focus_animating = now_focused && self.focus_animated;
        let need_anim = pressing || focus_animating;
        let press_count = self.press_count;

        // base Div — 不依赖闭包，直接用值构造。两个分支后续各自决定 wrap。
        let handler = self.on_click.clone();
        let on_press_listener = cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
            this.fire_press(cx);
            if let Some(h) = handler.clone() {
                h(ev, window, cx);
            }
        });

        let mut el = div()
            .id(self.id.clone())
            .h(height)
            .px(padding_x)
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .bg(idle_bg)
            .track_focus(&self.focus_handle)
            .child(
                div()
                    .typography(crate::TypeRole::BodyStrong, t)
                    .text_color(fg)
                    .child(self.label.clone()),
            );

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
            // 无动画路径：focused 时挂静态 ring，否则裸 div
            if now_focused {
                el = el.shadow(static_ring);
            }
            return el.into_any_element();
        }

        // 动画路径：单 animate_or_skip 同时驱动 press opacity + ring alpha fade
        // ring 在两种态：focus_animating=true 时 alpha 0→0.4 渐变；
        //                focused 但 !focus_animating 时静态 ring（按下时已 focused 场景）
        let ring_show_static = now_focused && !focus_animating;
        let anim_id: ElementId = ("motion-btn", press_count as usize).into();

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

/// 构造 focus ring shadow vec，alpha 系数 [0, 1]。
fn ring_shadow(t: &crate::theme::Theme, alpha_factor: f32) -> Vec<BoxShadow> {
    let mut glow = t.colors.ring;
    glow.a = 0.4 * alpha_factor.clamp(0.0, 1.0);
    vec![BoxShadow {
        color: glow,
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(4.0),
        spread_radius: px(2.0),
    }]
}

/// 纯函数：按 variant + disabled 返回 (idle_bg, hover_bg, active_bg, fg)。
/// 抽出供 stateless Button + Button + IconButton 共用（pub(crate)）。
pub(crate) fn pick_button_colors(
    variant: ButtonVariant,
    disabled: bool,
    t: &crate::theme::Theme,
) -> (gpui::Hsla, gpui::Hsla, gpui::Hsla, gpui::Hsla) {
    if disabled {
        return (
            t.colors.muted,
            t.colors.muted,
            t.colors.muted,
            t.colors.muted_foreground,
        );
    }
    match variant {
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
            t.colors.secondary_active,
            t.colors.secondary_strongest,
            t.colors.foreground,
        ),
    }
}

/// press feedback opacity：linear 0.85 → 1.0 在 delta ∈ [0, 1]。
/// 与 with_animation 的 easing 配合 — easing 已 ease-out，本 fn 仅线性映射。
pub(crate) fn press_opacity_at(delta: f32) -> f32 {
    let d = delta.clamp(0.0, 1.0);
    0.85 + 0.15 * d
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // M31 — Button 状态机 pure fn 单测（D-9 模拟）。
    // ============================================================================

    /// 模拟 fire_press 的状态机：mouse_down 让 pressing=true + press_count++。
    /// timer fire 时若 press_count 仍 == expected 且 pressing 仍 true → 清。
    fn next_pressing_state(prev_pressing: bool, prev_count: u64, mouse_down: bool) -> (bool, u64) {
        if mouse_down {
            (true, prev_count.wrapping_add(1))
        } else {
            (prev_pressing, prev_count)
        }
    }

    #[test]
    fn pressing_state_set_on_mouse_down() {
        let (p, c) = next_pressing_state(false, 0, true);
        assert!(p);
        assert_eq!(c, 1);
    }

    #[test]
    fn pressing_state_idempotent_no_mouse_down() {
        let (p, c) = next_pressing_state(false, 0, false);
        assert!(!p);
        assert_eq!(c, 0);
    }

    #[test]
    fn pressing_count_increments_per_press() {
        let (_, c1) = next_pressing_state(true, 0, true);
        let (_, c2) = next_pressing_state(true, c1, true);
        let (_, c3) = next_pressing_state(true, c2, true);
        assert_eq!(c1, 1);
        assert_eq!(c2, 2);
        assert_eq!(c3, 3);
    }

    /// 模拟 timer fire 时的幂等 check — press_count 必须等于 timer 启动时的
    /// expected 才推进，否则说明期间又按了一次（press_count 已不匹配）→ 不动。
    fn clear_pressing_if_match(prev_pressing: bool, prev_count: u64, expected_count: u64) -> bool {
        if prev_pressing && prev_count == expected_count {
            false
        } else {
            prev_pressing
        }
    }

    #[test]
    fn timer_clears_pressing_when_count_matches() {
        let new_pressing = clear_pressing_if_match(true, 1, 1);
        assert!(!new_pressing);
    }

    #[test]
    fn timer_skips_when_press_count_changed() {
        // T0 按下 → count=1，spawn timer expected=1
        // T50ms 又按下 → count=2，spawn timer expected=2
        // T80ms 旧 timer fire（expected=1）→ count=2 != 1，不动
        let new_pressing = clear_pressing_if_match(true, 2, 1);
        assert!(new_pressing, "stale timer 不应清掉新 pressing 状态");
    }

    /// focus 跨帧 transition：(prev_focused, now_focused) → 是否触发 fade-in
    fn focus_animator_should_start(prev: bool, cur: bool) -> bool {
        !prev && cur
    }

    #[test]
    fn focus_anim_starts_only_on_gain() {
        assert!(focus_animator_should_start(false, true));
        assert!(!focus_animator_should_start(false, false));
        assert!(!focus_animator_should_start(true, true));
        assert!(!focus_animator_should_start(true, false));
    }

    #[test]
    fn press_opacity_at_endpoints() {
        assert!((press_opacity_at(0.0) - 0.85).abs() < 1e-6);
        assert!((press_opacity_at(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn press_opacity_at_midpoint() {
        // delta=0.5 → 0.85 + 0.15*0.5 = 0.925
        assert!((press_opacity_at(0.5) - 0.925).abs() < 1e-6);
    }

    #[test]
    fn press_opacity_clamped() {
        assert!((press_opacity_at(-0.5) - 0.85).abs() < 1e-6);
        assert!((press_opacity_at(1.5) - 1.0).abs() < 1e-6);
    }
}
