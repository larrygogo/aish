//! 默认 dark 主题。
//!
//! 色板：黑底 CRT 终端风。background / card / popover / secondary / muted /
//! border / input 全用中性黑灰阶（去 Tokyo Night 蓝调），primary / accent /
//! ring 走 logo 终端绿系（primary_active 直接用 logo 原色 #00ff41 作为按下
//! 高光）。destructive / success / warning 保留鲜艳警示色作对比。

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme};

impl Theme {
    pub fn dark() -> Self {
        Self {
            colors: ColorTokens {
                // 黑底 + 中性灰阶（去蓝调）
                background: hex(0x050505),
                foreground: hex(0xe0e0e0),
                card: hex(0x0d0d0d),
                card_foreground: hex(0xe0e0e0),
                popover: hex(0x161616),
                popover_foreground: hex(0xe0e0e0),
                // primary 走 logo 终端绿
                primary: hex(0x00cc33),
                primary_foreground: hex(0x050505), // 绿底深字，对比清晰
                // secondary / muted 中性灰
                secondary: hex(0x1f1f1f),
                secondary_foreground: hex(0xbfbfbf),
                muted: hex(0x1f1f1f),
                muted_foreground: hex(0x808080),
                // accent 暗中绿，作为容器 hover bg（s~40% l~31%，避免大面积 fill 反客为主）
                accent: hex(0x2f6e3e),
                accent_foreground: hex(0xe0e0e0), // 暗绿底配浅白文字
                // destructive 保留 Tokyo Night 红粉（警示色与绿成对比）
                destructive: hex(0xf7768e),
                destructive_foreground: hex(0x050505),
                // border / input 中性深灰。input 略亮于 background（#050505），
                // 让输入框"凸出"而非凹陷（之前 #0a0a0a 比 background 还深，
                // 视觉像"洞"，与用户对 input 的直觉相反）
                border: hex(0x1f1f1f),
                input: hex(0x0d0d0d),
                // focus ring 比 primary 亮一档（与 primary_hover 同 #00e63a），让
                // focused button 与 idle button 视觉明显区分（之前 ring=primary 完全
                // 同色 → focus 时按钮看着和未 focus 一样，外圈 ring 也分不清）
                ring: hex(0x00e63a),
                success: hex(0x9ece6a),
                warning: hex(0xe0af68),
                // M15 阶梯（lightness 单调递增 idle→hover→active）
                // primary 绿色系：#00cc33 → #00e63a → #00ff41（logo 原色）
                primary_hover: hex(0x00e63a),
                primary_active: hex(0x00ff41),
                // secondary 中性灰阶梯（active 与 hover 拉开档差，按下反馈更清晰；
                // 旧 #333333 与 hover #2a2a2a 仅 9 亮度差用户感知不到）
                secondary_hover: hex(0x2a2a2a),
                secondary_active: hex(0x404040),
                // destructive 保留 Tokyo Night 阶梯
                destructive_hover: hex(0xff8aa1),
                destructive_active: hex(0xff9cb5),
                // M17：accent 按下反馈，比 accent 更深（与 M15 方向相反）
                // accent 暗绿系：#2f6e3e → #1f4a2c
                accent_active: hex(0x1f4a2c),
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
    fn dark_primary_is_green_ish() {
        let t = Theme::dark();
        // hue 绿色 ≈ 0.25..0.45（logo CRT 终端绿系）
        assert!(t.colors.primary.h > 0.25 && t.colors.primary.h < 0.45);
    }

    #[test]
    fn dark_destructive_is_red_ish() {
        let t = Theme::dark();
        // Tokyo Night destructive = 0xf7768e，归一化 hue ≈ 0.963（红-粉方向）
        // hue < 0.05 覆盖正红，hue > 0.95 覆盖红-粉
        assert!(t.colors.destructive.h < 0.05 || t.colors.destructive.h > 0.95);
    }

    #[test]
    fn dark_background_is_very_dark() {
        let t = Theme::dark();
        assert!(t.colors.background.l < 0.15);
    }

    #[test]
    fn dark_background_is_neutral_no_hue() {
        let t = Theme::dark();
        // #050505 是中性灰阶，saturation 应该极低（远离原 Tokyo Night #1a1b26 的蓝调）
        assert!(t.colors.background.s < 0.05);
    }

    #[test]
    fn dark_primary_hover_is_lighter_than_primary() {
        let t = Theme::dark();
        assert!(t.colors.primary_hover.l > t.colors.primary.l);
    }

    #[test]
    fn dark_primary_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.primary_active.l > t.colors.primary_hover.l);
    }

    #[test]
    fn dark_secondary_hover_is_lighter_than_secondary() {
        let t = Theme::dark();
        assert!(t.colors.secondary_hover.l > t.colors.secondary.l);
    }

    #[test]
    fn dark_secondary_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.secondary_active.l > t.colors.secondary_hover.l);
    }

    #[test]
    fn dark_destructive_hover_is_lighter_than_destructive() {
        let t = Theme::dark();
        assert!(t.colors.destructive_hover.l > t.colors.destructive.l);
    }

    #[test]
    fn dark_destructive_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.destructive_active.l > t.colors.destructive_hover.l);
    }

    #[test]
    fn dark_accent_active_is_darker_than_accent() {
        let t = Theme::dark();
        // M17：accent_active 比 accent 更深（容器按下"沉下去"，与 M15 系列变亮方向相反）
        assert!(t.colors.accent_active.l < t.colors.accent.l);
    }
}
