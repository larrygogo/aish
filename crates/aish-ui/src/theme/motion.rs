//! M30 — Motion token + animate_or_skip helper。
//!
//! 4 档语义 duration（instant/fast/medium/slow）+ 2 个 easing：
//!
//! | Token | 值 | 用途 |
//! |---|---|---|
//! | `instant` | 0 ms | reduced_motion fallback / 测试 |
//! | `fast` | 80 ms | press-feedback / focus ring 出现 / icon swap |
//! | `medium` | 120 ms | dialog / popover open / hover transition / fade-scale |
//! | `slow` | 250 ms | toast slide-in / page transition |
//!
//! M35 T1: medium 150 → 120ms，与 Linear / Warp 微交互区间对齐；密集
//! 操作（连点 hover 切换）感知不再卡。6 个 stateful entity + Dialog +
//! Toast 全部受益。
//!
//! easing：`easing_standard = ease_out_quint`（启动快收尾慢，UI 主力），
//! `easing_standard_in = quadratic`（淡出 / 出场用）。
//!
//! `animate_or_skip` 是 M30 唯一对外 API：根据 `Theme.reduced_motion` 决定
//! 走 `with_animation` 还是直接 animator(el, 1.0)，保证 reduced motion
//! 偏好生效。所有动画 caller 不应直接调 `.with_animation(...)`。

use std::{rc::Rc, time::Duration};

use gpui::{ease_out_quint, quadratic, AnimationExt, AnyElement, ElementId, IntoElement};

use super::Theme;

/// 共享 easing 函数（Rc 让 Theme 可放进 Global，无需 Clone）。
pub type EasingFn = Rc<dyn Fn(f32) -> f32>;

/// 4 档 duration token + 2 个 easing。Theme 的第 6 层 token。
///
/// 不派生 Clone — Theme 也不 Clone（GPUI Global 持 owned）。Default
/// 用 GPUI 内置 `ease_out_quint()` + `quadratic`。
pub struct Motion {
    pub instant: Duration,
    pub fast: Duration,
    pub medium: Duration,
    pub slow: Duration,
    pub easing_standard: EasingFn,
    pub easing_standard_in: EasingFn,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            instant: Duration::from_millis(0),
            fast: Duration::from_millis(80),
            medium: Duration::from_millis(120),
            slow: Duration::from_millis(250),
            // ease_out_quint 是 fn() -> impl Fn，调用后返回 closure
            easing_standard: Rc::new(ease_out_quint()),
            // quadratic 直接是 fn(f32) -> f32
            easing_standard_in: Rc::new(quadratic),
        }
    }
}

/// 包装 `with_animation`，根据 `Theme.reduced_motion` 决定走动画还是直接
/// 渲染 end-state（animator 用 delta=1.0 调一次）。
///
/// **用法**：所有 M30 动画 caller 走此 helper，不要直接 `.with_animation`。
///
/// ```ignore
/// animate_or_skip(
///     div().opacity(0.0),
///     theme(cx),
///     "motion-dialog-enter",
///     Animation::new(theme(cx).motion.medium)
///         .with_easing({ let e = theme(cx).motion.easing_standard.clone(); move |d| e(d) }),
///     |el, delta| el.opacity(delta),
/// )
/// ```
///
/// 注意：`Animation::with_easing` 接 `impl Fn(f32) -> f32 + 'static`，
/// 不接 Rc<dyn Fn>。caller 需把 EasingFn 包成 owning closure
/// `move |d| easing(d)`，开销 1 个 Rc clone。
pub fn animate_or_skip<E, F>(
    el: E,
    t: &Theme,
    id: impl Into<ElementId>,
    anim: gpui::Animation,
    animator: F,
) -> AnyElement
where
    E: IntoElement + 'static,
    F: Fn(E, f32) -> E + 'static,
{
    if t.reduced_motion {
        // reduced_motion：直接出 end-state（delta=1.0），不走 with_animation
        animator(el, 1.0).into_any_element()
    } else {
        el.with_animation(id, anim, animator).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_defaults_correct_durations() {
        let m = Motion::default();
        assert_eq!(m.instant.as_millis(), 0);
        assert_eq!(m.fast.as_millis(), 80);
        assert_eq!(m.medium.as_millis(), 120);
        assert_eq!(m.slow.as_millis(), 250);
    }

    #[test]
    fn motion_durations_ordered() {
        let m = Motion::default();
        assert!(m.instant < m.fast);
        assert!(m.fast < m.medium);
        assert!(m.medium < m.slow);
    }

    #[test]
    fn motion_easing_standard_monotone() {
        // ease_out_quint(0) = 0, ease_out_quint(1) = 1，中间值单调递增
        let m = Motion::default();
        let e = &m.easing_standard;
        let d0 = e(0.0);
        let d25 = e(0.25);
        let d50 = e(0.50);
        let d75 = e(0.75);
        let d1 = e(1.0);
        assert!((d0 - 0.0).abs() < 1e-6, "easing(0) ≈ 0，得 {}", d0);
        assert!((d1 - 1.0).abs() < 1e-6, "easing(1) ≈ 1，得 {}", d1);
        assert!(d0 < d25 && d25 < d50 && d50 < d75 && d75 < d1, "单调递增");
        // ease-out 的特征：前半段比 linear 跑得快 → d50 > 0.5
        assert!(d50 > 0.5, "ease-out 前半段加速：d(0.5) = {} 应 > 0.5", d50);
    }

    #[test]
    fn motion_easing_standard_in_quadratic() {
        // quadratic(d) = d * d
        let m = Motion::default();
        let e = &m.easing_standard_in;
        assert!((e(0.0) - 0.0).abs() < 1e-6);
        assert!((e(0.5) - 0.25).abs() < 1e-6);
        assert!((e(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn motion_default_easing_share_rc_safe_to_clone() {
        // EasingFn 是 Rc<dyn Fn>，clone 不复制函数体，仅引用计数 +1
        let m = Motion::default();
        let e1 = m.easing_standard.clone();
        let e2 = m.easing_standard.clone();
        assert!((e1(0.5) - e2(0.5)).abs() < 1e-6);
    }
}
