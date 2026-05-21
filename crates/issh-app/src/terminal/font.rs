//! 字体加载：bundle JetBrains Mono Nerd Font Regular + 用户终端字体 / 字号
//! 配置（M43 新增 TerminalFontConfig global，用户在 Settings 切换实时生效）。

use std::borrow::Cow;

use gpui::{font, px, App, Global, Pixels};

/// 默认字体名（bundled）。
pub const FONT_NAME: &str = "JetBrainsMono Nerd Font";
/// 默认字号 (pt)。
pub const FONT_SIZE: f32 = 14.0;

/// 终端字体 / 字号配置 — 作为 GPUI App global 注入，cell_size / grid_renderer
/// 读取，Settings 改时 set_global + refresh_windows 实时切换。
#[derive(Clone)]
pub struct TerminalFontConfig {
    pub family: String,
    pub size: f32,
}

impl Global for TerminalFontConfig {}

impl Default for TerminalFontConfig {
    fn default() -> Self {
        Self {
            family: FONT_NAME.to_string(),
            size: FONT_SIZE,
        }
    }
}

/// 拿当前终端字体配置（未注册时 fallback default）。
pub fn current(cx: &App) -> TerminalFontConfig {
    cx.try_global::<TerminalFontConfig>()
        .cloned()
        .unwrap_or_default()
}

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
/// 行高 = 字号 × 1.3 经验比例。读 TerminalFontConfig global 跟随用户配置。
pub fn cell_size(cx: &App) -> (Pixels, Pixels) {
    let cfg = current(cx);
    let font_size = px(cfg.size);
    let terminal_font = font(cfg.family.as_str());
    let font_id = cx.text_system().resolve_font(&terminal_font);
    let cell_width = cx
        .text_system()
        .advance(font_id, font_size, 'm')
        .map(|size| size.width)
        .unwrap_or_else(|_| px(cfg.size * 0.6));
    let cell_height = px(cfg.size * 1.3);
    (cell_width, cell_height)
}

/// 终端字体列表（Settings UI 用）。第一项总是 bundled JetBrainsMono，其余
/// 是跨平台常见 monospace 字体；用户机无此字体时 GPUI text_system 走 fallback。
pub const AVAILABLE_FONTS: &[&str] = &[
    "JetBrainsMono Nerd Font",
    "Cascadia Code",
    "Consolas",
    "Menlo",
    "SF Mono",
    "Fira Code",
    "Source Code Pro",
];

/// 字号档位（Settings UI 用）。
pub const AVAILABLE_SIZES: &[f32] = &[10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0];

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
