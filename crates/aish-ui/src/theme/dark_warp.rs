//! Dark Warp 主题 — M39 Phase 1 paseo/Warp 视觉重做实验性变体。
//!
//! 设计语言：**温暖紫 surface + Warp 紫 accent + 暖调 brand identity**。
//! 借鉴 Warp.dev 终端的视觉 DNA — 紫到粉的 brand gradient + 半透明 glass +
//! 高饱和度（比默认 dark / midnight 都更显眼）。
//!
//! 与其他 dark variant 对比：
//!
//! | token | dark (默认) | dark_midnight | dark_warp |
//! |---|---|---|---|
//! | primary | #5E6AD2 indigo (中性紫蓝) | #6B7AE0 indigo (亮一档) | #7C5CFC Warp 紫 (偏 magenta) |
//! | background | #08090A (中性黑) | #0C0D18 (深紫蓝黑) | #0E0A18 (深紫红黑) |
//! | foreground | #F4F5F8 (中性白) | #F0F2FF (蓝白) | #F4F0FF (暖白偏紫) |
//! | 整体调性 | 克制 / dev tool 惯例 | 深紫蓝冷调 | 温暖 / brand 强 / Warp 风 |
//!
//! 跨主题一致（视觉锚点）：destructive #E5484D / success #4FBB72 /
//! warning #E8A658 三色不变。
//!
//! 启用方式：Settings → 外观 → 深色变体 → 选 "Warp Aurora"。

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind};

impl Theme {
    pub fn dark_warp() -> Self {
        Self {
            kind: ThemeKind::DarkWarp,
            colors: ColorTokens {
                // ── Surface 阶梯（朝紫红偏，hue 280° 方向，比 midnight 暖）─
                background: hex(0x0e0a18),
                foreground: hex(0xf4f0ff),
                card: hex(0x1a1428),
                card_foreground: hex(0xf4f0ff),
                popover: hex(0x251c38),
                popover_foreground: hex(0xf4f0ff),
                // ── Primary = Warp 紫（偏 magenta 一档，亮且暖）────────────
                primary: hex(0x7c5cfc),
                primary_foreground: hex(0xffffff),
                // ── Secondary / Muted（紫红调灰阶）───────────────────────
                secondary: hex(0x2e2438),
                secondary_foreground: hex(0xd0c8e0),
                muted: hex(0x2e2438),
                muted_foreground: hex(0xa099b8), // 紫调中灰
                // ── Accent 容器 hover bg（深紫，与 primary 同 hue 极低饱和）─
                accent: hex(0x3d2e5a),
                accent_foreground: hex(0xf4f0ff),
                // ── Destructive 跨主题一致 ─────────────────────────────
                destructive: hex(0xe5484d),
                destructive_foreground: hex(0xffffff),
                // ── Border / Input ─────────────────────────────────────
                border: hex(0x3d3554),
                input: hex(0x1a1428),
                ring: hex(0x7c5cfc),
                // ── 状态色跨主题一致 ───────────────────────────────────
                success: hex(0x4fbb72),
                warning: hex(0xe8a658),
                // ── Variant 阶梯（M15 lightness 单调）──────────────────
                primary_hover: hex(0x8c6ffe),
                primary_active: hex(0x9a82ff),
                secondary_hover: hex(0x362b44),
                secondary_active: hex(0x423650),
                destructive_hover: hex(0xed5e63),
                destructive_active: hex(0xf27479),
                // accent_active 比 accent 更深
                accent_active: hex(0x2d2042),
                // Ghost button strongest
                secondary_strongest: hex(0x5a4d6e),
                // sidebar 顶色比 background 亮 ΔL≈2
                sidebar_bg_top: hex(0x120e1f),
                // surface_workspace 与 background 同
                surface_workspace: hex(0x0e0a18),
                // M39 Phase 2: Warp aurora — 紫+粉双色高饱和暖调（ADR-002 C）。
                // Warp 紫 #7C5CFC (hue 252°) + Warp 粉 #FF7B9F (hue 343°)，
                // 双色暖 brand 形成 Warp 风视觉签名。alpha 比默认 dark 高一档
                // （0.25 vs 0.18）让暖调更显眼但仍可读
                aurora_a: hex(0x7c5cfc).opacity(0.25),
                aurora_b: hex(0xff7b9f).opacity(0.20),
            },
            radius: Radius::default(),
            spacing: Spacing::default(),
            font_size: FontSize::default(),
            icon_size: super::tokens::IconSize::default(),
            opacity: super::tokens::Opacity::default(),
            typography: super::typography::Typography::default(),
            anatomy: super::anatomy::Anatomy::default(),
            motion: super::motion::Motion::default(),
            reduced_motion: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_kind_is_dark_family() {
        let t = Theme::dark_warp();
        assert_eq!(t.kind, ThemeKind::DarkWarp);
        assert!(t.kind.is_dark(), "DarkWarp should be in dark family");
    }

    #[test]
    fn warp_primary_is_magenta_indigo_brighter_and_more_saturated_than_default() {
        let warp = Theme::dark_warp();
        let dark = Theme::dark();
        // primary #7C5CFC 应该在紫偏 magenta 范围（hue ≈ 0.70-0.78）
        assert!(
            warp.colors.primary.h > 0.68 && warp.colors.primary.h < 0.78,
            "warp primary hue {} 应该比默认 dark indigo 更偏 magenta",
            warp.colors.primary.h
        );
        // 比默认 dark primary 更饱和（warp 风高饱和特征）
        assert!(
            warp.colors.primary.s > dark.colors.primary.s,
            "warp primary saturation {} 应该 > 默认 dark {}",
            warp.colors.primary.s,
            dark.colors.primary.s
        );
        // 比默认 dark primary 更亮
        assert!(
            warp.colors.primary.l > dark.colors.primary.l,
            "warp primary lightness {} 应该 > 默认 dark {}",
            warp.colors.primary.l,
            dark.colors.primary.l
        );
    }

    #[test]
    fn warp_background_is_warmer_purple_than_midnight() {
        let warp = Theme::dark_warp();
        let mid = Theme::dark_midnight();
        // warp bg #0E0A18 (hue ~270°) 比 midnight bg #0C0D18 (hue ~234°) 更紫
        // 即更偏 magenta 方向（hue 数值更大）
        assert!(
            warp.colors.background.h > mid.colors.background.h,
            "warp bg hue {} 应该 > midnight bg hue {} (更偏 magenta/暖)",
            warp.colors.background.h,
            mid.colors.background.h
        );
    }

    #[test]
    fn warp_status_colors_match_default_dark() {
        // 状态色跨主题一致（视觉锚点）
        let warp = Theme::dark_warp();
        let dark = Theme::dark();
        assert_eq!(warp.colors.destructive, dark.colors.destructive);
        assert_eq!(warp.colors.success, dark.colors.success);
        assert_eq!(warp.colors.warning, dark.colors.warning);
    }

    #[test]
    fn warp_primary_hover_active_lightness_monotone() {
        // primary base → hover → active 必须 lightness 单调递增（M15 规则）
        let t = Theme::dark_warp();
        assert!(t.colors.primary_hover.l > t.colors.primary.l);
        assert!(t.colors.primary_active.l > t.colors.primary_hover.l);
    }

    #[test]
    fn warp_accent_active_darker_than_accent() {
        // accent_active 比 accent 更深（M17 容器按下"沉下去"语义）
        let t = Theme::dark_warp();
        assert!(t.colors.accent_active.l < t.colors.accent.l);
    }

    #[test]
    fn warp_aurora_is_purple_pink_warm_double() {
        // M39 Phase 2: Warp aurora 是紫+粉暖双色（ADR-002 C）
        let t = Theme::dark_warp();
        // aurora_a 应该匹配 primary hue (Warp 紫 ~252°)
        assert!(
            (t.colors.aurora_a.h - t.colors.primary.h).abs() < 0.05,
            "warp aurora_a hue {} 应该跟 primary {} 接近",
            t.colors.aurora_a.h,
            t.colors.primary.h
        );
        // aurora_b 应该在粉红范围 (hue ~340-350°，即 0.93-0.97)
        assert!(
            t.colors.aurora_b.h > 0.92 && t.colors.aurora_b.h < 0.98,
            "warp aurora_b hue {} 应该在 pink 范围 0.92-0.98",
            t.colors.aurora_b.h
        );
        // aurora alpha 比默认 dark 高一档（暖调更显眼）
        let dark = Theme::dark();
        assert!(
            t.colors.aurora_a.a > dark.colors.aurora_a.a,
            "warp aurora_a alpha {} 应该 > 默认 dark {}（暖调高饱和）",
            t.colors.aurora_a.a,
            dark.colors.aurora_a.a
        );
    }
}
