//! M43 流行主题包：10 个业界流行配色方案。
//!
//! 每个主题 factory `Theme::xxx()` 返回完整 Theme 含 ColorTokens + ANSI palette。
//! UI tokens 通过 `build_tokens` helper 从核心色派生（lighten / darken），
//! ANSI palette 提供给 terminal/colors.rs `palette_for(kind)` 路由使用。
//!
//! 主题数据来源：各主题官方仓库 / 文档（链接见各 factory doc）。

use gpui::{Hsla, Rgba};

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind};

/// 16 色 ANSI palette（hex）— 跟 terminal/colors.rs 的 DEFAULT_PALETTE_DARK
/// 同 layout：0-7 normal black/red/green/yellow/blue/magenta/cyan/white，
/// 8-15 bright 系列。
pub type AnsiPalette = [u32; 16];

/// 主题配色谱（核心色 + ANSI）— Theme factory 调用 build_tokens 时输入。
struct Spec {
    is_dark: bool,
    bg: u32,
    fg: u32,
    surface: u32, // card / popover / input bg
    border: u32,
    muted_fg: u32, // 次级文字 / 注释
    accent: u32,   // primary / accent / ring
    destructive: u32,
    success: u32,
    warning: u32,
}

/// HSL lightness shift — 派生 hover / active 色阶。
fn shift_l(c: Hsla, delta: f32) -> Hsla {
    Hsla {
        l: (c.l + delta).clamp(0.0, 1.0),
        ..c
    }
}

/// HSL saturation scale — 按钮 primary 派生时降饱和，让 brand 鲜艳色融入
/// 整体灰阶 UI（用户反馈鲜艳按钮跟 UI 不协调）。
fn desaturate(c: Hsla, factor: f32) -> Hsla {
    Hsla {
        s: (c.s * factor).clamp(0.0, 1.0),
        ..c
    }
}

fn build_tokens(s: Spec) -> ColorTokens {
    let bg = hex(s.bg);
    let fg = hex(s.fg);
    let surface = hex(s.surface);
    let accent = hex(s.accent);
    let destructive = hex(s.destructive);
    // dark 主题：hover = lighten；light 主题：hover = darken
    let h = if s.is_dark { 0.05 } else { -0.04 };
    let h2 = h * 2.0;
    let h3 = h * 3.5;

    // primary 按钮主色：accent 降 25% 饱和度，让 brand 色融入 UI 整体调性
    // （用户反馈鲜艳按钮跟整体灰阶不协调）。保留 accent 同 hue 维持主题
    // 识别度。ring / accent token 仍用满饱和 accent 让 cursor / focus 强可见。
    let primary_color = desaturate(accent, 0.75);
    ColorTokens {
        background: bg,
        foreground: fg,
        card: surface,
        card_foreground: fg,
        popover: surface,
        popover_foreground: fg,
        primary: primary_color,
        primary_foreground: hex(0xffffff),
        primary_hover: shift_l(primary_color, h),
        primary_active: shift_l(primary_color, h2),
        secondary: shift_l(bg, h),
        secondary_foreground: hex(s.muted_fg),
        secondary_hover: shift_l(bg, h2),
        secondary_active: shift_l(bg, h * 2.5),
        secondary_strongest: shift_l(bg, h3),
        muted: shift_l(bg, h * 0.8),
        muted_foreground: hex(s.muted_fg),
        // accent 容器 bg = accent 同 hue 但极淡（dark: darken / light: lighten）
        accent: shift_l(accent, if s.is_dark { -0.35 } else { 0.40 }),
        accent_foreground: accent,
        accent_active: shift_l(accent, if s.is_dark { -0.40 } else { 0.30 }),
        destructive,
        destructive_foreground: hex(0xffffff),
        destructive_hover: shift_l(destructive, h),
        destructive_active: shift_l(destructive, h2),
        border: hex(s.border),
        input: surface,
        ring: accent,
        success: hex(s.success),
        warning: hex(s.warning),
        // sidebar 顶部 bg 跟 background 拉开 1 档 lightness 形成 elevation
        sidebar_bg_top: shift_l(bg, h * 0.3),
        surface_workspace: bg,
    }
}

fn make_theme(kind: ThemeKind, spec: Spec) -> Theme {
    Theme {
        kind,
        colors: build_tokens(spec),
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

// ─────────────────── DARK themes ──────────────────────────────────────

impl Theme {
    /// Moshi —— Termius 自家 default dark（高对比、VS Code 风）。
    pub fn moshi() -> Self {
        make_theme(
            ThemeKind::Moshi,
            Spec {
                is_dark: true,
                bg: 0x1a1a1a,
                fg: 0xd4d4d4,
                surface: 0x252526,
                border: 0x3e3e42,
                muted_fg: 0x858585,
                accent: 0x0078d4,
                destructive: 0xe74c3c,
                success: 0x2ecc71,
                warning: 0xffb86c,
            },
        )
    }

    /// Dracula —— https://draculatheme.com
    pub fn dracula() -> Self {
        make_theme(
            ThemeKind::Dracula,
            Spec {
                is_dark: true,
                bg: 0x282a36,
                fg: 0xf8f8f2,
                surface: 0x44475a,
                border: 0x44475a,
                muted_fg: 0x6272a4,
                accent: 0xbd93f9,
                destructive: 0xff5555,
                success: 0x50fa7b,
                warning: 0xf1fa8c,
            },
        )
    }

    /// Nord —— https://www.nordtheme.com
    pub fn nord() -> Self {
        make_theme(
            ThemeKind::Nord,
            Spec {
                is_dark: true,
                bg: 0x2e3440,
                fg: 0xd8dee9,
                surface: 0x3b4252,
                border: 0x434c5e,
                // muted_fg 不用 nord3 #4c566a — 跟 secondary_hover (bg lighten 10%)
                // lightness 只差 3% 看不清。用 nord3 跟 nord4 之间的中间灰
                // 让 hover 行的次级文字可读。
                muted_fg: 0x7b88a1,
                accent: 0x88c0d0,
                destructive: 0xbf616a,
                success: 0xa3be8c,
                warning: 0xebcb8b,
            },
        )
    }

    /// Solarized Dark —— https://ethanschoonover.com/solarized
    pub fn solarized_dark() -> Self {
        make_theme(
            ThemeKind::SolarizedDark,
            Spec {
                is_dark: true,
                bg: 0x002b36,
                fg: 0x839496,
                surface: 0x073642,
                border: 0x586e75,
                muted_fg: 0x586e75,
                accent: 0x268bd2,
                destructive: 0xdc322f,
                success: 0x859900,
                warning: 0xb58900,
            },
        )
    }

    /// Gruvbox Dark —— https://github.com/morhetz/gruvbox
    pub fn gruvbox() -> Self {
        make_theme(
            ThemeKind::Gruvbox,
            Spec {
                is_dark: true,
                bg: 0x282828,
                fg: 0xebdbb2,
                surface: 0x3c3836,
                border: 0x504945,
                muted_fg: 0xa89984,
                accent: 0x83a598,
                destructive: 0xcc241d,
                success: 0x98971a,
                warning: 0xd79921,
            },
        )
    }

    /// Catppuccin Mocha —— https://github.com/catppuccin/catppuccin
    pub fn catppuccin_mocha() -> Self {
        make_theme(
            ThemeKind::CatppuccinMocha,
            Spec {
                is_dark: true,
                bg: 0x1e1e2e,
                fg: 0xcdd6f4,
                surface: 0x313244,
                border: 0x45475a,
                muted_fg: 0xa6adc8,
                accent: 0x89b4fa,
                destructive: 0xf38ba8,
                success: 0xa6e3a1,
                warning: 0xf9e2af,
            },
        )
    }

    // ─────────────────── LIGHT themes ──────────────────────────────────

    /// Solarized Light —— 同 Solarized Dark fg/bg 互换
    pub fn solarized_light() -> Self {
        make_theme(
            ThemeKind::SolarizedLight,
            Spec {
                is_dark: false,
                bg: 0xfdf6e3,
                fg: 0x657b83,
                surface: 0xeee8d5,
                border: 0x93a1a1,
                muted_fg: 0x93a1a1,
                accent: 0x268bd2,
                destructive: 0xdc322f,
                success: 0x859900,
                warning: 0xb58900,
            },
        )
    }

    /// Catppuccin Latte —— https://github.com/catppuccin/catppuccin
    pub fn catppuccin_latte() -> Self {
        make_theme(
            ThemeKind::CatppuccinLatte,
            Spec {
                is_dark: false,
                bg: 0xeff1f5,
                fg: 0x4c4f69,
                surface: 0xe6e9ef,
                border: 0xacb0be,
                muted_fg: 0x6c6f85,
                accent: 0x1e66f5,
                destructive: 0xd20f39,
                success: 0x40a02b,
                warning: 0xdf8e1d,
            },
        )
    }

    /// GitHub Light —— https://github.com/primer/primitives
    pub fn github_light() -> Self {
        make_theme(
            ThemeKind::GithubLight,
            Spec {
                is_dark: false,
                bg: 0xffffff,
                fg: 0x24292e,
                surface: 0xf6f8fa,
                border: 0xd0d7de,
                muted_fg: 0x6e7781,
                accent: 0x0366d6,
                destructive: 0xd73a49,
                success: 0x28a745,
                warning: 0xb08800,
            },
        )
    }

    /// Rosé Pine Dawn —— https://rosepinetheme.com
    pub fn rose_pine_dawn() -> Self {
        make_theme(
            ThemeKind::RosePineDawn,
            Spec {
                is_dark: false,
                bg: 0xfaf4ed,
                fg: 0x575279,
                surface: 0xfffaf3,
                border: 0xdfdad9,
                muted_fg: 0x797593,
                accent: 0x907aa9,
                destructive: 0xb4637a,
                success: 0x286983,
                warning: 0xea9d34,
            },
        )
    }
}

// ─────────────────── ANSI palettes per theme ──────────────────────────
// terminal/colors.rs 通过 ansi_palette_for(kind) 路由到对应 palette。

pub fn ansi_palette_for(kind: ThemeKind) -> AnsiPalette {
    match kind {
        ThemeKind::Moshi => [
            0x1a1a1a, 0xe74c3c, 0x2ecc71, 0xf39c12, 0x3498db, 0x9b59b6, 0x1abc9c, 0xd4d4d4,
            0x858585, 0xff6e6e, 0x69ff94, 0xffd86c, 0x70a8e8, 0xc586c0, 0x4fd1c5, 0xffffff,
        ],
        ThemeKind::Dracula => [
            0x21222c, 0xff5555, 0x50fa7b, 0xf1fa8c, 0xbd93f9, 0xff79c6, 0x8be9fd, 0xf8f8f2,
            0x6272a4, 0xff6e6e, 0x69ff94, 0xffffa5, 0xd6acff, 0xff92df, 0xa4ffff, 0xffffff,
        ],
        ThemeKind::Nord => [
            0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0,
            0x4c566a, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4,
        ],
        ThemeKind::SolarizedDark => [
            0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198, 0xeee8d5,
            0x002b36, 0xcb4b16, 0x586e75, 0x657b83, 0x839496, 0x6c71c4, 0x93a1a1, 0xfdf6e3,
        ],
        ThemeKind::Gruvbox => [
            0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984,
            0x928374, 0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2,
        ],
        ThemeKind::CatppuccinMocha => [
            0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
            0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
        ],
        ThemeKind::SolarizedLight => [
            0xeee8d5, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198, 0x073642,
            0xfdf6e3, 0xcb4b16, 0x93a1a1, 0x839496, 0x657b83, 0x6c71c4, 0x586e75, 0x002b36,
        ],
        ThemeKind::CatppuccinLatte => [
            0x5c5f77, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xacb0be,
            0x6c6f85, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179299, 0xbcc0cc,
        ],
        ThemeKind::GithubLight => [
            0x24292e, 0xd73a49, 0x28a745, 0xdbab09, 0x0366d6, 0x5a32a3, 0x0598bc, 0x6a737d,
            0x959da5, 0xcb2431, 0x22863a, 0xb08800, 0x005cc5, 0x5a32a3, 0x3192aa, 0xd1d5da,
        ],
        ThemeKind::RosePineDawn => [
            0xf2e9e1, 0xb4637a, 0x286983, 0xea9d34, 0x56949f, 0x907aa9, 0xd7827e, 0x575279,
            0x9893a5, 0xb4637a, 0x286983, 0xea9d34, 0x56949f, 0x907aa9, 0xd7827e, 0x575279,
        ],
        // 默认 Dark / Light fallback 到原 VS Code palette（保留兼容）
        _ => DEFAULT_PALETTE_PLACEHOLDER,
    }
}

/// 跟 terminal/colors.rs DEFAULT_PALETTE_DARK 同（默认主题 fallback 用）。
const DEFAULT_PALETTE_PLACEHOLDER: AnsiPalette = [
    0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666,
    0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
];

/// 终端 viewport bg / fg — 跟 ColorTokens.background / foreground 一致（一体化
/// 主题：UI bg == terminal bg）。
pub fn terminal_bg_for(kind: ThemeKind) -> u32 {
    match kind {
        ThemeKind::Moshi => 0x1a1a1a,
        ThemeKind::Dracula => 0x282a36,
        ThemeKind::Nord => 0x2e3440,
        ThemeKind::SolarizedDark => 0x002b36,
        ThemeKind::Gruvbox => 0x282828,
        ThemeKind::CatppuccinMocha => 0x1e1e2e,
        ThemeKind::SolarizedLight => 0xfdf6e3,
        ThemeKind::CatppuccinLatte => 0xeff1f5,
        ThemeKind::GithubLight => 0xffffff,
        ThemeKind::RosePineDawn => 0xfaf4ed,
        ThemeKind::Light => 0xfafbfc, // 跟 light.rs ColorTokens.background 对齐
        ThemeKind::Dark => 0x000000,
    }
}

pub fn terminal_fg_for(kind: ThemeKind) -> u32 {
    match kind {
        ThemeKind::Moshi => 0xd4d4d4,
        ThemeKind::Dracula => 0xf8f8f2,
        ThemeKind::Nord => 0xd8dee9,
        ThemeKind::SolarizedDark => 0x839496,
        ThemeKind::Gruvbox => 0xebdbb2,
        ThemeKind::CatppuccinMocha => 0xcdd6f4,
        ThemeKind::SolarizedLight => 0x657b83,
        ThemeKind::CatppuccinLatte => 0x4c4f69,
        ThemeKind::GithubLight => 0x24292e,
        ThemeKind::RosePineDawn => 0x575279,
        ThemeKind::Light => 0x333333,
        ThemeKind::Dark => 0xcccccc,
    }
}

/// Settings 主题列表 5 色块预览取色：[bg, fg, red, green, blue]，按用户截图
/// 风格固定 5 个槽位代表主题概览。
pub fn preview_swatches(kind: ThemeKind) -> [Hsla; 5] {
    let p = ansi_palette_for(kind);
    let bg = terminal_bg_for(kind);
    let fg = terminal_fg_for(kind);
    // 直接 hex → Rgba → Hsla 转换避免再 import hex helper
    let to_h = |c: u32| -> Hsla {
        let r = ((c >> 16) & 0xFF) as f32 / 255.0;
        let g = ((c >> 8) & 0xFF) as f32 / 255.0;
        let b = (c & 0xFF) as f32 / 255.0;
        Rgba { r, g, b, a: 1.0 }.into()
    };
    [to_h(bg), to_h(fg), to_h(p[1]), to_h(p[2]), to_h(p[4])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_distinct_backgrounds() {
        let mut seen = std::collections::HashSet::new();
        for k in crate::theme::ALL_THEMES {
            let bg = terminal_bg_for(*k);
            assert!(seen.insert(bg), "重复 bg in {:?}: {:x}", k, bg);
        }
    }

    #[test]
    fn dark_themes_have_dark_backgrounds() {
        for k in crate::theme::ALL_THEMES.iter().filter(|k| k.is_dark()) {
            let bg = terminal_bg_for(*k);
            let r = (bg >> 16) & 0xFF;
            // dark theme bg 三通道平均亮度应 < 100
            let avg = (r + ((bg >> 8) & 0xFF) + (bg & 0xFF)) / 3;
            assert!(avg < 100, "{:?} bg {:x} 亮度 {} 应 < 100", k, bg, avg);
        }
    }

    #[test]
    fn light_themes_have_light_backgrounds() {
        for k in crate::theme::ALL_THEMES.iter().filter(|k| !k.is_dark()) {
            let bg = terminal_bg_for(*k);
            let r = (bg >> 16) & 0xFF;
            let avg = (r + ((bg >> 8) & 0xFF) + (bg & 0xFF)) / 3;
            assert!(avg > 200, "{:?} bg {:x} 亮度 {} 应 > 200", k, bg, avg);
        }
    }

    #[test]
    fn ansi_palette_full_16_entries() {
        for k in crate::theme::ALL_THEMES {
            let p = ansi_palette_for(*k);
            assert_eq!(p.len(), 16);
        }
    }
}
