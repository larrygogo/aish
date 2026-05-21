//! M30 — lerp helper（Hsla / Pixels 线性插值）。
//!
//! GPUI 内置 Animation API 只给 delta ∈ [0, 1]，caller 自己用它驱动属性
//! 变化。色与像素值常用 lerp(a, b, t) 计算中间值。`Hsla` / `Pixels` 都不
//! 提供，自己写覆盖。
//!
//! **clamp 行为**：t < 0 → a，t > 1 → b。caller 应保证 t ∈ [0, 1]，clamp
//! 仅作 defense-in-depth，不应作为常态依赖。

use gpui::{px, Hsla, Pixels, Rgba};

/// HSL → Hsla 线性插值用 **RGB 空间**做中间值。
///
/// M43 bug fix：原版直接 lerp hue 字段，当两端 hue 跨越圆环上的不同位置
/// （如 hue 0° 灰 → hue 204° 蓝）中间会出现黄绿色，hover transition 时
/// 闪出意外色相（用户报告 Solarized Dark 按钮 hover 闪绿）。
///
/// RGB 空间 lerp 在 perceptual 色空间走「直线」，无 hue 圆环跨越问题。
/// 转换成本：每帧 2 次 Hsla→Rgba + 1 次 Rgba→Hsla（< 100ns / 帧）。
pub fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    let ra: Rgba = a.into();
    let rb: Rgba = b.into();
    Rgba {
        r: ra.r + (rb.r - ra.r) * t,
        g: ra.g + (rb.g - ra.g) * t,
        b: ra.b + (rb.b - ra.b) * t,
        a: ra.a + (rb.a - ra.a) * t,
    }
    .into()
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
    fn lerp_hsla_midpoint_via_rgb() {
        // M43 起 lerp 走 RGB 空间。black (Hsla(0,0,0,1)) → white (Hsla(0,0,1,1))
        // 中点应为灰 (RGB 0.5/0.5/0.5)。
        let a = hsla(0.0, 0.0, 0.0, 1.0);
        let b = hsla(0.0, 0.0, 1.0, 1.0);
        let m = lerp_hsla(a, b, 0.5);
        let rgba: Rgba = m.into();
        assert!((rgba.r - 0.5).abs() < 0.05);
        assert!((rgba.g - 0.5).abs() < 0.05);
        assert!((rgba.b - 0.5).abs() < 0.05);
    }

    #[test]
    fn lerp_hsla_no_hue_ring_artifact() {
        // 关键回归测试：灰 (h=0, s=0) → 蓝 (h=0.567, s=0.7) 中点不该出现
        // 黄绿色 (g 远大于 r/b)。M43 之前 lerp_hsla 字段直接插值导致此 bug。
        let gray = hsla(0.0, 0.0, 0.5, 1.0);
        let blue = hsla(0.567, 0.7, 0.4, 1.0);
        let m = lerp_hsla(gray, blue, 0.5);
        let rgba: Rgba = m.into();
        // 中点 RGB 应是「灰偏蓝」— g 不该明显大于 r 和 b
        assert!(
            rgba.g <= rgba.r.max(rgba.b) + 0.05,
            "lerp 中点不该出黄绿色 (r={}, g={}, b={})",
            rgba.r,
            rgba.g,
            rgba.b
        );
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
