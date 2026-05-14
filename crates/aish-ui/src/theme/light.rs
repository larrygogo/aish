//! Light theme — M18-light 落地。
//!
//! 色板设计原则：
//! - 与 dark 共享同一终端绿 primary 系（保持 aish 品牌一致），仅调
//!   lightness 让在 light bg 上有足够对比（30~40% l，避免荧光感）。
//! - bg / card / popover / secondary / muted：shadcn/ui Light 标准灰阶
//!   （bg=#fafafa， card/popover=#ffffff，secondary/muted=#f4f4f4）。
//! - **hover/active 方向与 dark 反向**：dark 是 hover→active 越亮（lightness
//!   递增），light 是 hover→active 越**暗**（lightness 递减）。两者一致：
//!   都让 hover/active 对比加强。
//! - accent 系列：浅绿容器 hover bg（与 dark 暗绿对应），accent_active 更深
//!   一档（与 dark 同方向："沉下去"语义跨主题一致）。

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind};

impl Theme {
    pub fn light() -> Self {
        Self {
            kind: ThemeKind::Light,
            colors: ColorTokens {
                // shadcn/ui Light 灰阶 + 微浅 bg 与纯白 card 拉开层次
                background: hex(0xfafafa),
                foreground: hex(0x0a0a0a),
                card: hex(0xffffff),
                card_foreground: hex(0x0a0a0a),
                popover: hex(0xffffff),
                popover_foreground: hex(0x0a0a0a),
                // primary 终端绿系（与 dark 同 hue，lightness 降低保证 light bg 上
                // 对比足够）。logo 原色 #00ff41 在 light 下太刺眼，用 #16a34a
                // （tailwind green-600，hue 同方向）。
                primary: hex(0x16a34a),
                primary_foreground: hex(0xfafafa), // 绿底白字
                // secondary / muted 浅灰
                secondary: hex(0xf4f4f4),
                secondary_foreground: hex(0x404040),
                muted: hex(0xf4f4f4),
                muted_foreground: hex(0x737373),
                // accent 浅绿容器（与 dark 暗绿 #2f6e3e 对应，light 用浅绿）
                accent: hex(0xdcfce7),
                accent_foreground: hex(0x14532d), // 浅绿底深绿字
                // destructive 红
                destructive: hex(0xef4444),
                destructive_foreground: hex(0xfafafa),
                // border / input：input 略亮于 bg（同 dark 设计：input 凸出而非
                // 凹陷），light 下 input 用纯白与 bg #fafafa 拉开层次
                border: hex(0xe5e5e5),
                input: hex(0xffffff),
                // focus ring 与 primary 同方向但更亮一档（light 下 ring 加深增强对比）
                ring: hex(0x15803d),
                success: hex(0x16a34a),
                warning: hex(0xd97706),
                // M15 阶梯（lightness **递减** = light theme 反向）
                // primary 绿色系：#16a34a → #15803d → #166534
                primary_hover: hex(0x15803d),
                primary_active: hex(0x166534),
                // secondary 灰阶递减：#f4f4f4 → #e5e5e5 → #d4d4d4
                secondary_hover: hex(0xe5e5e5),
                secondary_active: hex(0xd4d4d4),
                // destructive 红递减：#ef4444 → #dc2626 → #b91c1c
                destructive_hover: hex(0xdc2626),
                destructive_active: hex(0xb91c1c),
                // M17：accent 按下反馈，比 accent 更深（与 dark 同方向"沉下去"）
                // accent 浅绿系：#dcfce7 → #bbf7d0
                accent_active: hex(0xbbf7d0),
                // M18：Ghost button active 灰阶序列再深一档
                // secondary #f4f4f4 → hover #e5e5e5 → active #d4d4d4 → strongest #a3a3a3
                secondary_strongest: hex(0xa3a3a3),
            },
            radius: Radius::default(),
            spacing: Spacing::default(),
            font_size: FontSize::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_primary_is_green_ish() {
        let t = Theme::light();
        // hue 绿色 ≈ 0.25..0.45（与 dark primary 同方向，保持品牌一致）
        assert!(t.colors.primary.h > 0.25 && t.colors.primary.h < 0.45);
    }

    #[test]
    fn light_destructive_is_red_ish() {
        let t = Theme::light();
        // #ef4444 hue ≈ 0 (正红)
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
        // M17 一致："沉下去"语义跨主题 —— accent_active 比 accent 更深，
        // 但在 light 下"更深"= lightness 更低（dark 下 accent 已经低，
        // active 比它更低；light 下 accent 也算高，active 更低）。
        assert!(t.colors.accent_active.l < t.colors.accent.l);
    }

    #[test]
    fn light_secondary_strongest_is_darker_than_active() {
        let t = Theme::light();
        // M18：strongest 灰阶比 active 更深一档（dark 反向但同语义"再加一档"）
        assert!(t.colors.secondary_strongest.l < t.colors.secondary_active.l);
    }

    #[test]
    fn light_background_lighter_than_card_or_equal() {
        let t = Theme::light();
        // light: bg=#fafafa, card=#ffffff，card 比 bg 略亮（与 dark 反向，
        // dark 是 card 比 bg 略亮也是同方向 —— 都是 card 凸显于 bg）。
        // 注意：dark 下 bg=#050505 / card=#0d0d0d，card 也是更亮；
        // light 下 bg=#fafafa / card=#ffffff，card 同样略亮 —— 跨主题一致。
        assert!(t.colors.card.l >= t.colors.background.l);
    }
}
