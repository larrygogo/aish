//! Light theme — M24 Warp/Linear 风重塑。
//!
//! 与 dark 共享 primary indigo #5E6AD2（Linear 品牌惯例：同 accent 跨主题），
//! lightness 不变 hue 略调让 light bg 上对比足够。状态色 desaturate 一档与
//! dark 对称。
//!
//! hover/active 方向与 dark 反向：dark 提亮，light 加深 —— 都让 hover/active
//! 对比加强。M17 accent_active "沉下去"语义跨主题一致：dark/light 都是
//! lightness 进一步降低（dark accent 低，light accent 高，active 都更低）。

// hsla import 删除（aurora 字段 M43 移除后不再需要）

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind};

impl Theme {
    pub fn light() -> Self {
        Self {
            kind: ThemeKind::Light,
            colors: ColorTokens {
                // ── Neutral L0-L7（light）─────────────────────────────
                // L0 bg 最底 (近白略灰) / L1+L2 card+popover 纯白凸出
                // / L4 secondary / L5 hover / L6 active / L7 border = L5
                background: hex(0xfafbfc),
                foreground: hex(0x0d0e10),
                card: hex(0xffffff),
                card_foreground: hex(0x0d0e10),
                popover: hex(0xffffff),
                popover_foreground: hex(0x0d0e10),
                // ── Primary = Linear indigo（跨主题统一） ────────────
                primary: hex(0x5e6ad2),
                primary_foreground: hex(0xffffff),
                // ── Secondary / Muted 浅灰 ─────────────────────────────
                secondary: hex(0xf3f4f6),
                secondary_foreground: hex(0x404249),
                muted: hex(0xf3f4f6),
                muted_foreground: hex(0x6b6e78), // 冷调中灰，与 dark 对称
                // ── Accent 浅紫灰容器（与 dark 深紫灰对应）──────────────
                accent: hex(0xe9eaf8),
                accent_foreground: hex(0x2d3047),
                // ── Destructive 真红 ──────────────────────────────────
                destructive: hex(0xdc2626),
                destructive_foreground: hex(0xffffff),
                // ── Border / Input — border = L5 / input = card 纯白 ──
                border: hex(0xe6e8ec),
                input: hex(0xffffff),
                // focus ring = primary（caller 加 alpha 实现 glow）
                ring: hex(0x5e6ad2),
                // 状态色 desaturate（与 dark 对称）
                success: hex(0x16a34a),
                warning: hex(0xd97706),
                // ── Variant 阶梯（M15 light 反向 = lightness 递减）────
                // primary indigo：base → hover(加深) → active(再加深)
                primary_hover: hex(0x4f59c0),
                primary_active: hex(0x434daa),
                // secondary 灰阶递减
                secondary_hover: hex(0xe6e8ec),
                secondary_active: hex(0xd9dbdf),
                // destructive 红递减
                destructive_hover: hex(0xb91c1c),
                destructive_active: hex(0x991b1b),
                // M17：accent_active 比 accent 更深一档（"沉下去"语义）
                accent_active: hex(0xd1d4f0),
                // M18 Ghost button strongest 灰阶
                secondary_strongest: hex(0xb8bcc4),
                // M35.1 D1: light theme 暂同 background — 不引入渐变风险，
                // 与 T17 light 实验状态对齐（plan 范围仅 dark theme）。
                sidebar_bg_top: hex(0xfafbfc),
                // M38 paseo borrowing E: terminal/workspace 区背景。当前等同
                // background，留语义位置便于未来差异化。
                surface_workspace: hex(0xfafbfc),
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
    fn light_primary_is_indigo() {
        let t = Theme::light();
        // 与 dark 同 hue（indigo ≈ 0.66 ± 0.06），跨主题一致品牌色
        assert!(
            t.colors.primary.h > 0.60 && t.colors.primary.h < 0.72,
            "primary hue {} 不在 indigo 范围",
            t.colors.primary.h
        );
    }

    #[test]
    fn light_destructive_is_red() {
        let t = Theme::light();
        assert!(t.colors.destructive.h < 0.05 || t.colors.destructive.h > 0.95);
    }

    #[test]
    fn light_background_is_very_light() {
        let t = Theme::light();
        assert!(t.colors.background.l > 0.85);
    }

    #[test]
    fn light_foreground_is_very_dark() {
        let t = Theme::light();
        assert!(t.colors.foreground.l < 0.15);
    }

    // === hover/active 阶梯（与 dark **方向相反**：light 递减 = 越按越暗） ===

    #[test]
    fn light_primary_hover_is_darker_than_primary() {
        let t = Theme::light();
        assert!(t.colors.primary_hover.l < t.colors.primary.l);
    }

    #[test]
    fn light_primary_active_is_darker_than_hover() {
        let t = Theme::light();
        assert!(t.colors.primary_active.l < t.colors.primary_hover.l);
    }

    #[test]
    fn light_secondary_hover_is_darker_than_secondary() {
        let t = Theme::light();
        assert!(t.colors.secondary_hover.l < t.colors.secondary.l);
    }

    #[test]
    fn light_secondary_active_is_darker_than_hover() {
        let t = Theme::light();
        assert!(t.colors.secondary_active.l < t.colors.secondary_hover.l);
    }

    #[test]
    fn light_destructive_hover_is_darker_than_destructive() {
        let t = Theme::light();
        assert!(t.colors.destructive_hover.l < t.colors.destructive.l);
    }

    #[test]
    fn light_destructive_active_is_darker_than_hover() {
        let t = Theme::light();
        assert!(t.colors.destructive_active.l < t.colors.destructive_hover.l);
    }

    #[test]
    fn light_accent_active_is_darker_than_accent() {
        let t = Theme::light();
        // M17 一致："沉下去"跨主题：accent_active.l < accent.l
        assert!(t.colors.accent_active.l < t.colors.accent.l);
    }

    #[test]
    fn light_secondary_strongest_is_darker_than_active() {
        let t = Theme::light();
        assert!(t.colors.secondary_strongest.l < t.colors.secondary_active.l);
    }

    #[test]
    fn light_background_lighter_than_card_or_equal() {
        let t = Theme::light();
        assert!(t.colors.card.l >= t.colors.background.l);
    }

    #[test]
    fn light_and_dark_share_primary_hue() {
        // M24 设计：indigo accent 跨主题统一品牌色（Linear 风）
        let l = Theme::light();
        let d = Theme::dark();
        assert!(
            (l.colors.primary.h - d.colors.primary.h).abs() < 0.05,
            "light primary hue {} ≠ dark primary hue {}",
            l.colors.primary.h,
            d.colors.primary.h
        );
    }
}
