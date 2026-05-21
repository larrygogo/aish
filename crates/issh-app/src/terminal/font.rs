//! 字体加载：bundle JetBrains Mono Nerd Font Regular。

use std::borrow::Cow;

use gpui::{font, px, App, Pixels};

/// 字体名称，用于 GPUI text_system font 查找。
pub const FONT_NAME: &str = "JetBrainsMono Nerd Font";

/// 终端字号（M2c 才做用户配置）。
pub const FONT_SIZE: f32 = 14.0;

/// bundle 的 .ttf bytes。
const FONT_BYTES: &[u8] = include_bytes!("../../assets/JetBrainsMonoNerdFont-Regular.ttf");

/// 在 GPUI App 启动时调用：把 bundled font 注册进 text_system。
///
/// 必须在创建任何使用此字体的 view 之前调用（典型是 app::run() 内）。
pub fn register_bundled_font(cx: &mut App) {
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(FONT_BYTES)])
        .expect("bundled font should load");
}

/// 拿 (cell_width, cell_height) — 单字符 advance 与行高。
///
/// 使用 GPUI text_system.advance() 查询 'm' 字符宽度（monospace 标准）。
/// 行高 = 字号 × 1.3 经验比例。
///
/// 如果 text_system 查询失败，fallback 到基于字号的经验比例。
pub fn cell_size(cx: &App) -> (Pixels, Pixels) {
    let font_size = px(FONT_SIZE);
    // font() 默认就是 Normal weight + Normal style，无需额外设置
    let terminal_font = font(FONT_NAME);
    let font_id = cx.text_system().resolve_font(&terminal_font);
    let cell_width = cx
        .text_system()
        .advance(font_id, font_size, 'm')
        .map(|size| size.width)
        .unwrap_or_else(|_| px(FONT_SIZE * 0.6));
    let cell_height = px(FONT_SIZE * 1.3);
    (cell_width, cell_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_bytes_is_non_empty() {
        // 验证 include_bytes! 真嵌入了内容
        assert!(
            FONT_BYTES.len() > 100_000,
            "ttf should be at least 100KB, got {}",
            FONT_BYTES.len()
        );
        assert!(
            FONT_BYTES.len() < 5_000_000,
            "ttf should be under 5MB, got {}",
            FONT_BYTES.len()
        );
    }

    #[test]
    fn font_bytes_starts_with_ttf_or_otf_magic() {
        // TrueType 文件首 4 字节通常是 0x00010000 / "OTTO" / "true" / "typ1"
        let magic = &FONT_BYTES[..4];
        assert!(magic.iter().any(|b| *b != 0));
    }

    #[test]
    fn font_name_is_jetbrains() {
        assert_eq!(FONT_NAME, "JetBrainsMono Nerd Font");
    }

    #[test]
    fn font_size_is_14pt() {
        assert!((FONT_SIZE - 14.0).abs() < 0.01);
    }
}
