//! alacritty Color → GPUI rgba。
//!
//! alacritty_terminal 0.26 的 Color enum 有三种 variant：
//!   - Named(NamedColor) — 16 个标准 ANSI 名（Black/Red/Green/...）
//!   - Spec(Rgb) — 任意 RGB
//!   - Indexed(u8) — 256 color palette index

use alacritty_terminal::vte::ansi::{Color as AlacColor, NamedColor, Rgb};
use gpui::{rgb, Hsla};

/// VS Code Dark+ ANSI palette。Bright 系列明显亮于 normal，配合 bold_promote
/// 让 bold 文本视觉差明显（Ubuntu PS1 \033[01;32m → 鲜亮绿）。
pub const DEFAULT_PALETTE_DARK: [u32; 16] = [
    0x000000, // 0  Black
    0xcd3131, // 1  Red
    0x0dbc79, // 2  Green
    0xe5e510, // 3  Yellow
    0x2472c8, // 4  Blue
    0xbc3fbc, // 5  Magenta
    0x11a8cd, // 6  Cyan
    0xe5e5e5, // 7  White
    0x666666, // 8  BrightBlack
    0xf14c4c, // 9  BrightRed
    0x23d18b, // 10 BrightGreen
    0xf5f543, // 11 BrightYellow
    0x3b8eea, // 12 BrightBlue
    0xd670d6, // 13 BrightMagenta
    0x29b8db, // 14 BrightCyan
    0xffffff, // 15 BrightWhite
];

/// VS Code Light+ ANSI palette。Light 主题终端用 —— 颜色饱和度高
/// 让浅底上对比清晰；BrightWhite 是中灰（在 light bg 上不能用白）。
pub const DEFAULT_PALETTE_LIGHT: [u32; 16] = [
    0x000000, // 0  Black
    0xcd3131, // 1  Red
    0x00bc00, // 2  Green (light 下用更深绿区别 bg)
    0x949800, // 3  Yellow (深黄，避免浅底刺眼)
    0x0451a5, // 4  Blue
    0xbc05bc, // 5  Magenta
    0x0598bc, // 6  Cyan
    0x555555, // 7  White (light 下 "white" 是中灰，与 bg #fafafa 区分)
    0x666666, // 8  BrightBlack
    0xcd3131, // 9  BrightRed
    0x14ce14, // 10 BrightGreen
    0xb5ba00, // 11 BrightYellow
    0x0451a5, // 12 BrightBlue
    0xbc05bc, // 13 BrightMagenta
    0x0598bc, // 14 BrightCyan
    0xa5a5a5, // 15 BrightWhite (light 下 BrightWhite 也是浅灰非纯白)
];

pub const DEFAULT_FOREGROUND_DARK: u32 = 0xcccccc;
pub const DEFAULT_BACKGROUND_DARK: u32 = 0x000000;
pub const DEFAULT_FOREGROUND_LIGHT: u32 = 0x333333;
pub const DEFAULT_BACKGROUND_LIGHT: u32 = 0xffffff;

/// 按 theme kind 取 palette / fg / bg。grid_renderer 在 paint phase 调
/// `aish_ui::theme(cx).kind` 拿到 kind 后传入。
pub fn palette_for(kind: aish_ui::ThemeKind) -> [u32; 16] {
    match kind {
        aish_ui::ThemeKind::Dark => DEFAULT_PALETTE_DARK,
        aish_ui::ThemeKind::Light => DEFAULT_PALETTE_LIGHT,
    }
}

pub fn default_foreground_for(kind: aish_ui::ThemeKind) -> u32 {
    match kind {
        aish_ui::ThemeKind::Dark => DEFAULT_FOREGROUND_DARK,
        aish_ui::ThemeKind::Light => DEFAULT_FOREGROUND_LIGHT,
    }
}

pub fn default_background_for(kind: aish_ui::ThemeKind) -> u32 {
    match kind {
        aish_ui::ThemeKind::Dark => DEFAULT_BACKGROUND_DARK,
        aish_ui::ThemeKind::Light => DEFAULT_BACKGROUND_LIGHT,
    }
}

/// 把 normal 色升级到对应 bright 色。用于 bold 文本：
/// alacritty / iTerm2 / Windows Terminal 默认行为 'draw bold text in bright
/// colors'。grid_renderer 在 cell.flags 包含 BOLD 时调用此函数升级 fg 色。
///
/// 已经是 bright 色 / Spec / Indexed≥8 / NamedColor::Foreground 等不变。
pub fn bold_promote(color: AlacColor) -> AlacColor {
    use NamedColor::*;
    match color {
        AlacColor::Named(Black) => AlacColor::Named(BrightBlack),
        AlacColor::Named(Red) => AlacColor::Named(BrightRed),
        AlacColor::Named(Green) => AlacColor::Named(BrightGreen),
        AlacColor::Named(Yellow) => AlacColor::Named(BrightYellow),
        AlacColor::Named(Blue) => AlacColor::Named(BrightBlue),
        AlacColor::Named(Magenta) => AlacColor::Named(BrightMagenta),
        AlacColor::Named(Cyan) => AlacColor::Named(BrightCyan),
        AlacColor::Named(White) => AlacColor::Named(BrightWhite),
        AlacColor::Indexed(i) if i < 8 => AlacColor::Indexed(i + 8),
        other => other,
    }
}

/// 主入口：把 alacritty Color 转成 GPUI Hsla。
///
/// `is_fg` 决定 Named::Foreground/Background 默认色选择。
/// `kind` 决定 palette / fg / bg fallback。
pub fn to_gpui(color: AlacColor, is_fg: bool, kind: aish_ui::ThemeKind) -> Hsla {
    match color {
        AlacColor::Named(named) => named_to_gpui(named, is_fg, kind),
        AlacColor::Spec(rgb_color) => rgb_to_gpui(rgb_color),
        AlacColor::Indexed(idx) => indexed_to_gpui(idx, is_fg, kind),
    }
}

fn named_to_gpui(named: NamedColor, is_fg: bool, kind: aish_ui::ThemeKind) -> Hsla {
    let palette = palette_for(kind);
    let default_fg = default_foreground_for(kind);
    let default_bg = default_background_for(kind);
    let hex = match named {
        NamedColor::Black => palette[0],
        NamedColor::Red => palette[1],
        NamedColor::Green => palette[2],
        NamedColor::Yellow => palette[3],
        NamedColor::Blue => palette[4],
        NamedColor::Magenta => palette[5],
        NamedColor::Cyan => palette[6],
        NamedColor::White => palette[7],
        NamedColor::BrightBlack => palette[8],
        NamedColor::BrightRed => palette[9],
        NamedColor::BrightGreen => palette[10],
        NamedColor::BrightYellow => palette[11],
        NamedColor::BrightBlue => palette[12],
        NamedColor::BrightMagenta => palette[13],
        NamedColor::BrightCyan => palette[14],
        NamedColor::BrightWhite => palette[15],
        NamedColor::Foreground => {
            if is_fg {
                default_fg
            } else {
                default_bg
            }
        }
        NamedColor::Background => default_bg,
        NamedColor::Cursor => default_fg,
        // ALAC-API: 0.26 还有更多 variants（DimBlack 等），用 catch-all
        _ => {
            if is_fg {
                default_fg
            } else {
                default_bg
            }
        }
    };
    rgb(hex).into()
}

fn rgb_to_gpui(c: Rgb) -> Hsla {
    let hex = ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    rgb(hex).into()
}

/// 256 color palette: 0-15 是 16 色名，16-231 是 6×6×6 cube，232-255 是 24 灰阶。
fn indexed_to_gpui(idx: u8, is_fg: bool, kind: aish_ui::ThemeKind) -> Hsla {
    if idx < 16 {
        named_to_gpui(NAMED_BY_IDX[idx as usize], is_fg, kind)
    } else if idx < 232 {
        // 6×6×6 cube
        let i = idx - 16;
        let r = (i / 36) % 6;
        let g = (i / 6) % 6;
        let b = i % 6;
        let scale = |c: u8| -> u32 {
            if c == 0 {
                0
            } else {
                (40 * c as u32) + 55
            }
        };
        let hex = (scale(r) << 16) | (scale(g) << 8) | scale(b);
        rgb(hex).into()
    } else {
        // 24 灰阶 (232-255)
        let v = (8 + 10 * (idx as u32 - 232)) & 0xff;
        let hex = (v << 16) | (v << 8) | v;
        rgb(hex).into()
    }
}

const NAMED_BY_IDX: [NamedColor; 16] = [
    NamedColor::Black,
    NamedColor::Red,
    NamedColor::Green,
    NamedColor::Yellow,
    NamedColor::Blue,
    NamedColor::Magenta,
    NamedColor::Cyan,
    NamedColor::White,
    NamedColor::BrightBlack,
    NamedColor::BrightRed,
    NamedColor::BrightGreen,
    NamedColor::BrightYellow,
    NamedColor::BrightBlue,
    NamedColor::BrightMagenta,
    NamedColor::BrightCyan,
    NamedColor::BrightWhite,
];

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Rgba;

    fn rgba_hex(c: Hsla) -> u32 {
        let rgba = Rgba::from(c);
        let r = (rgba.r * 255.0).round() as u32;
        let g = (rgba.g * 255.0).round() as u32;
        let b = (rgba.b * 255.0).round() as u32;
        (r << 16) | (g << 8) | b
    }

    const DARK: aish_ui::ThemeKind = aish_ui::ThemeKind::Dark;
    const LIGHT: aish_ui::ThemeKind = aish_ui::ThemeKind::Light;

    #[test]
    fn named_red_maps_to_palette_dark() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Red), true, DARK);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE_DARK[1]);
    }

    #[test]
    fn named_red_maps_to_palette_light() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Red), true, LIGHT);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE_LIGHT[1]);
    }

    #[test]
    fn named_foreground_returns_default_fg_dark() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Foreground), true, DARK);
        assert_eq!(rgba_hex(hsla), DEFAULT_FOREGROUND_DARK);
    }

    #[test]
    fn named_foreground_returns_default_fg_light() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Foreground), true, LIGHT);
        assert_eq!(rgba_hex(hsla), DEFAULT_FOREGROUND_LIGHT);
    }

    #[test]
    fn named_background_returns_default_bg_dark() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Background), false, DARK);
        assert_eq!(rgba_hex(hsla), DEFAULT_BACKGROUND_DARK);
    }

    #[test]
    fn named_background_returns_default_bg_light() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Background), false, LIGHT);
        assert_eq!(rgba_hex(hsla), DEFAULT_BACKGROUND_LIGHT);
    }

    #[test]
    fn spec_rgb_preserves_components() {
        let hsla = to_gpui(
            AlacColor::Spec(Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            }),
            true,
            DARK,
        );
        assert_eq!(rgba_hex(hsla), 0x123456);
    }

    #[test]
    fn indexed_15_maps_to_bright_white_dark() {
        let hsla = to_gpui(AlacColor::Indexed(15), true, DARK);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE_DARK[15]);
    }

    #[test]
    fn indexed_0_to_15_use_named_palette_dark() {
        for i in 0u8..16 {
            let hsla = to_gpui(AlacColor::Indexed(i), true, DARK);
            assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE_DARK[i as usize]);
        }
    }

    #[test]
    fn indexed_232_to_255_grayscale() {
        let dark = to_gpui(AlacColor::Indexed(232), true, DARK);
        let bright = to_gpui(AlacColor::Indexed(255), true, DARK);
        let dark_hex = rgba_hex(dark);
        let bright_hex = rgba_hex(bright);
        let dark_r = (dark_hex >> 16) & 0xff;
        let bright_r = (bright_hex >> 16) & 0xff;
        assert!(
            dark_r < 20,
            "dark grayscale should be near black, got {:x}",
            dark_hex
        );
        assert!(
            bright_r > 220,
            "bright grayscale should be near white, got {:x}",
            bright_hex
        );
    }

    #[test]
    fn indexed_cube_16_is_pure_black() {
        let hsla = to_gpui(AlacColor::Indexed(16), true, DARK);
        assert_eq!(rgba_hex(hsla), 0x000000);
    }
}
