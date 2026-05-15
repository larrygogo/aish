//! M30 — lerp helper（Hsla / Pixels 线性插值）。
//!
//! GPUI 内置 Animation API 只给 delta ∈ [0, 1]，caller 自己用它驱动属性
//! 变化。色与像素值常用 lerp(a, b, t) 计算中间值。`Hsla` / `Pixels` 都不
//! 提供，自己写 9 行覆盖。
//!
//! **clamp 行为**：t < 0 → a，t > 1 → b。caller 应保证 t ∈ [0, 1]，clamp
//! 仅作 defense-in-depth，不应作为常态依赖。

use gpui::{px, Hsla, Pixels};

/// 4 个分量分别线性插值。Hsla.h 不做 hue 圆环最短路径（M30 use case 都是
/// 同 hue 或纯 opacity 变化）。
pub fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    Hsla {
        h: a.h + (b.h - a.h) * t,
        s: a.s + (b.s - a.s) * t,
        l: a.l + (b.l - a.l) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// Pixels 线性插值。Pixels.0 是 pub(crate) 外部不可访问，用 `f32::from(p)`
/// 转 f32。
pub fn lerp_px(a: Pixels, b: Pixels, t: f32) -> Pixels {
    let t = t.clamp(0.0, 1.0);
    let af = f32::from(a);
    let bf = f32::from(b);
    px(af + (bf - af) * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::hsla;

    #[test]
    fn lerp_hsla_endpoints() {
        let a = hsla(0.0, 0.5, 0.5, 1.0);
        let b = hsla(0.6, 0.8, 0.2, 0.5);
        let r0 = lerp_hsla(a, b, 0.0);
        let r1 = lerp_hsla(a, b, 1.0);
        assert!((r0.h - a.h).abs() < 1e-6);
        assert!((r0.s - a.s).abs() < 1e-6);
        assert!((r0.l - a.l).abs() < 1e-6);
        assert!((r0.a - a.a).abs() < 1e-6);
        assert!((r1.h - b.h).abs() < 1e-6);
        assert!((r1.s - b.s).abs() < 1e-6);
        assert!((r1.l - b.l).abs() < 1e-6);
        assert!((r1.a - b.a).abs() < 1e-6);
    }

    #[test]
    fn lerp_hsla_midpoint() {
        let a = hsla(0.0, 0.0, 0.0, 0.0);
        let b = hsla(1.0, 1.0, 1.0, 1.0);
        let m = lerp_hsla(a, b, 0.5);
        assert!((m.h - 0.5).abs() < 1e-6);
        assert!((m.s - 0.5).abs() < 1e-6);
        assert!((m.l - 0.5).abs() < 1e-6);
        assert!((m.a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn lerp_hsla_clamps_t() {
        let a = hsla(0.0, 0.0, 0.0, 1.0);
        let b = hsla(0.0, 0.0, 1.0, 1.0);
        // t < 0 clamp 到 0 → a
        let r = lerp_hsla(a, b, -0.5);
        assert!((r.l - a.l).abs() < 1e-6);
        // t > 1 clamp 到 1 → b
        let r = lerp_hsla(a, b, 1.5);
        assert!((r.l - b.l).abs() < 1e-6);
    }

    #[test]
    fn lerp_px_endpoints_and_midpoint() {
        let a = px(0.0);
        let b = px(20.0);
        assert!((f32::from(lerp_px(a, b, 0.0)) - 0.0).abs() < 1e-6);
        assert!((f32::from(lerp_px(a, b, 1.0)) - 20.0).abs() < 1e-6);
        assert!((f32::from(lerp_px(a, b, 0.5)) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_px_clamps_t() {
        let a = px(0.0);
        let b = px(10.0);
        assert!((f32::from(lerp_px(a, b, -1.0)) - 0.0).abs() < 1e-6);
        assert!((f32::from(lerp_px(a, b, 2.0)) - 10.0).abs() < 1e-6);
    }
}
