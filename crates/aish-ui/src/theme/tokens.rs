//! Theme token 定义。颜色、圆角、间距、字号四类。
//! 命名参考 shadcn/ui，HSLA 内部存储。

use gpui::{px, Hsla, Pixels, Rgba};

#[derive(Clone, Copy)]
pub struct ColorTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub card_foreground: Hsla,
    pub popover: Hsla,
    pub popover_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    // M15 新加：按 variant 的 hover / active 状态色
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub destructive_hover: Hsla,
    pub destructive_active: Hsla,
    // M17 新加：accent 系列容器按下反馈
    pub accent_active: Hsla,
    // M18 新加：secondary_strongest — Ghost button active 用，比 secondary_active
    // 再亮一档。原因：Ghost 按钮常嵌 Card / Row 内，容器 hover 时已经是
    // secondary_hover，Ghost active 若同等级色块会与容器融为一体；跳一档保证
    // active 状态在任何容器背景下都可识别。
    pub secondary_strongest: Hsla,
}

#[derive(Clone, Copy)]
pub struct Radius {
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub full: Pixels,
}

impl Default for Radius {
    fn default() -> Self {
        Self {
            sm: px(4.0),
            md: px(6.0),
            lg: px(8.0),
            full: px(9999.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Spacing {
    pub px_1: Pixels,
    pub px_2: Pixels,
    pub px_3: Pixels,
    pub px_4: Pixels,
    pub px_6: Pixels,
    pub px_8: Pixels,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            px_1: px(4.0),
            px_2: px(8.0),
            px_3: px(12.0),
            px_4: px(16.0),
            px_6: px(24.0),
            px_8: px(32.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FontSize {
    pub xs: Pixels,
    pub sm: Pixels,
    pub base: Pixels,
    pub lg: Pixels,
    pub xl: Pixels,
}

impl Default for FontSize {
    fn default() -> Self {
        Self {
            xs: px(10.0),
            sm: px(12.0),
            base: px(14.0),
            lg: px(16.0),
            xl: px(18.0),
        }
    }
}

/// 主题种类。运行时切换 dark / light 时，view 可 `theme(cx).kind` 查询当前
/// 主题决定特定行为（如 settings switch 的 checked 状态）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeKind {
    Dark,
    Light,
}

pub struct Theme {
    pub kind: ThemeKind,
    pub colors: ColorTokens,
    pub radius: Radius,
    pub spacing: Spacing,
    pub font_size: FontSize,
}

impl gpui::Global for Theme {}

/// 把 0xRRGGBB hex 转 Hsla。
pub(crate) fn hex(rgb: u32) -> Hsla {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    Rgba { r, g, b, a: 1.0 }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_pure_red_roundtrip() {
        let h = hex(0xff0000);
        assert!(h.h.abs() < 0.01 || (h.h - 1.0).abs() < 0.01);
        assert!(h.s > 0.99);
    }

    #[test]
    fn radius_defaults_ordered() {
        let r = Radius::default();
        assert!(r.sm < r.md);
        assert!(r.md < r.lg);
        assert!(r.lg < r.full);
    }

    #[test]
    fn spacing_defaults_ordered() {
        let s = Spacing::default();
        assert!(s.px_1 < s.px_2);
        assert!(s.px_2 < s.px_3);
        assert!(s.px_3 < s.px_4);
        assert!(s.px_4 < s.px_6);
        assert!(s.px_6 < s.px_8);
    }

    #[test]
    fn font_size_defaults_ordered() {
        let f = FontSize::default();
        assert!(f.xs < f.sm);
        assert!(f.sm < f.base);
        assert!(f.base < f.lg);
        assert!(f.lg < f.xl);
    }
}
