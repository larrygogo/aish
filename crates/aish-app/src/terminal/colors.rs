//! alacritty Color → GPUI rgba。
//!
//! alacritty_terminal 0.26 的 Color enum 有三种 variant：
//!   - Named(NamedColor) — 16 个标准 ANSI 名（Black/Red/Green/...）
//!   - Spec(Rgb) — 任意 RGB
//!   - Indexed(u8) — 256 color palette index

use alacritty_terminal::vte::ansi::{Color as AlacColor, NamedColor, Rgb};
use gpui::{rgb, Hsla};

/// 默认 16 色 ANSI palette（VS Code Dark+ 主题，与现代终端如 Windows Terminal /
/// iTerm2 默认风格接近）。
///
/// 比之前 Tomorrow Night palette 有两个关键改进：
/// 1. 颜色饱和度更高（红更红、绿更绿），在纯黑底上肉眼可辨；之前的橄榄绿
///    `#b5bd68` 在黑底上偏灰白，看起来像普通文字色。
/// 2. Bright 系列与 normal 真正区分（之前 BrightGreen/Yellow/Blue 与 normal
///    完全相同），bold→bright 映射后才有视觉效果。
pub const DEFAULT_PALETTE: [u32; 16] = [
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

pub const DEFAULT_FOREGROUND: u32 = 0xcccccc;
pub const DEFAULT_BACKGROUND: u32 = 0x000000;

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
pub fn to_gpui(color: AlacColor, is_fg: bool) -> Hsla {
    match color {
        AlacColor::Named(named) => named_to_gpui(named, is_fg),
        AlacColor::Spec(rgb_color) => rgb_to_gpui(rgb_color),
        AlacColor::Indexed(idx) => indexed_to_gpui(idx, is_fg),
    }
}

fn named_to_gpui(named: NamedColor, is_fg: bool) -> Hsla {
    let hex = match named {
        NamedColor::Black => DEFAULT_PALETTE[0],
        NamedColor::Red => DEFAULT_PALETTE[1],
        NamedColor::Green => DEFAULT_PALETTE[2],
        NamedColor::Yellow => DEFAULT_PALETTE[3],
        NamedColor::Blue => DEFAULT_PALETTE[4],
        NamedColor::Magenta => DEFAULT_PALETTE[5],
        NamedColor::Cyan => DEFAULT_PALETTE[6],
        NamedColor::White => DEFAULT_PALETTE[7],
        NamedColor::BrightBlack => DEFAULT_PALETTE[8],
        NamedColor::BrightRed => DEFAULT_PALETTE[9],
        NamedColor::BrightGreen => DEFAULT_PALETTE[10],
        NamedColor::BrightYellow => DEFAULT_PALETTE[11],
        NamedColor::BrightBlue => DEFAULT_PALETTE[12],
        NamedColor::BrightMagenta => DEFAULT_PALETTE[13],
        NamedColor::BrightCyan => DEFAULT_PALETTE[14],
        NamedColor::BrightWhite => DEFAULT_PALETTE[15],
        NamedColor::Foreground => {
            if is_fg {
                DEFAULT_FOREGROUND
            } else {
                DEFAULT_BACKGROUND
            }
        }
        NamedColor::Background => DEFAULT_BACKGROUND,
        NamedColor::Cursor => DEFAULT_FOREGROUND,
        // ALAC-API: 0.26 还有更多 variants（DimBlack 等），用 catch-all
        _ => {
            if is_fg {
                DEFAULT_FOREGROUND
            } else {
                DEFAULT_BACKGROUND
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
fn indexed_to_gpui(idx: u8, is_fg: bool) -> Hsla {
    if idx < 16 {
        named_to_gpui(NAMED_BY_IDX[idx as usize], is_fg)
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

    #[test]
    fn named_red_maps_to_palette() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Red), true);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE[1]);
    }

    #[test]
    fn named_foreground_returns_default_fg_when_is_fg() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Foreground), true);
        assert_eq!(rgba_hex(hsla), DEFAULT_FOREGROUND);
    }

    #[test]
    fn named_background_returns_default_bg() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Background), false);
        assert_eq!(rgba_hex(hsla), DEFAULT_BACKGROUND);
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
        );
        assert_eq!(rgba_hex(hsla), 0x123456);
    }

    #[test]
    fn indexed_15_maps_to_bright_white() {
        let hsla = to_gpui(AlacColor::Indexed(15), true);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE[15]);
    }

    #[test]
    fn indexed_0_to_15_use_named_palette() {
        for i in 0u8..16 {
            let hsla = to_gpui(AlacColor::Indexed(i), true);
            assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE[i as usize]);
        }
    }

    #[test]
    fn indexed_232_to_255_grayscale() {
        let dark = to_gpui(AlacColor::Indexed(232), true);
        let bright = to_gpui(AlacColor::Indexed(255), true);
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
        let hsla = to_gpui(AlacColor::Indexed(16), true);
        assert_eq!(rgba_hex(hsla), 0x000000);
    }
}
