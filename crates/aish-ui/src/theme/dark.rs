//! 默认 dark 主题，色板基于 Tokyo Night 系。

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme};

impl Theme {
    pub fn dark() -> Self {
        Self {
            colors: ColorTokens {
                background: hex(0x1a1b26),
                foreground: hex(0xc0caf5),
                card: hex(0x1f2030),
                card_foreground: hex(0xc0caf5),
                popover: hex(0x24253a),
                popover_foreground: hex(0xc0caf5),
                primary: hex(0x3d59a1),
                primary_foreground: hex(0xc0caf5),
                secondary: hex(0x2d2d3f),
                secondary_foreground: hex(0xa9b1d6),
                muted: hex(0x2d2d3f),
                muted_foreground: hex(0x565f89),
                accent: hex(0x6c91c2),
                accent_foreground: hex(0xc0caf5),
                destructive: hex(0xf7768e),
                destructive_foreground: hex(0x1a1b26),
                border: hex(0x2d2d3f),
                input: hex(0x16161e),
                ring: hex(0x6c91c2),
                success: hex(0x9ece6a),
                warning: hex(0xe0af68),
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
    fn dark_primary_is_blue_ish() {
        let t = Theme::dark();
        // hue 蓝色 ≈ 0.5..0.75
        assert!(t.colors.primary.h > 0.5 && t.colors.primary.h < 0.75);
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
}
