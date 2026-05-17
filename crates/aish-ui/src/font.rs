//! 字体 fallback chain — 跨平台 symbol/CJK 兜底，防 tofu 方块。
//!
//! GPUI `Styled::font_family(name)` 只设 family、不挂 fallback；任何主字体不带的
//! glyph（U+2327 Miscellaneous Technical / 罕见 CJK / emoji）直接 tofu。本模块
//! 提供带 fallback chain 的 Font helper，统一治理。
//!
//! Caller 用 `Styled::font(font)` 整套挂上（会覆盖 family + features + weight +
//! style + fallbacks 5 字段，所以 weight 要 caller 自己在 Font 上设）。

use std::sync::OnceLock;

use gpui::{font, Font, FontFallbacks};

use crate::theme::typography::CODE_FONT_NAME;

/// fallback chain — 按"图标 > 系统符号 > CJK"顺序，平台找不到的字体自动跳过。
///
/// - **Symbols Nerd Font**：Nerd Font 图标符号专用，Linux/macOS Homebrew 常装
/// - **Segoe UI Symbol**：Windows 内置，覆盖 Miscellaneous Technical（含 U+2327）
/// - **Apple Symbols**：macOS 内置，同上覆盖
/// - **Noto Sans Symbols 2**：Linux 多数发行版预装 / Google Noto
/// - **Noto Sans CJK SC**：CJK 罕用字兜底
const FONT_FALLBACK_CHAIN: &[&str] = &[
    "Symbols Nerd Font",
    "Segoe UI Symbol",
    "Apple Symbols",
    "Noto Sans Symbols 2",
    "Noto Sans CJK SC",
];

static FALLBACKS: OnceLock<FontFallbacks> = OnceLock::new();

/// 拿全局 fallback chain（Arc 内部，克隆零拷贝）。
pub fn fallbacks() -> FontFallbacks {
    FALLBACKS
        .get_or_init(|| {
            FontFallbacks::from_fonts(FONT_FALLBACK_CHAIN.iter().map(|s| s.to_string()).collect())
        })
        .clone()
}

/// Code font (mono Nerd Font) + symbol fallback chain。
///
/// weight/style 默认 (Normal/Normal)；caller 用 `Styled::font(...)` apply 前自己
/// 设 weight / style 即可（避免 helper 接受过多参数）。
pub fn code_font() -> Font {
    let mut f = font(CODE_FONT_NAME);
    f.fallbacks = Some(fallbacks());
    f
}

/// Sans font (系统 UI 字体 `.SystemUIFont`) + symbol fallback chain。
///
/// GPUI 内置 special name `.SystemUIFont` 自动展开为 Windows Segoe UI /
/// macOS SF Pro / Linux 系统 sans，比 hardcode 单一字体名跨平台稳。
pub fn sans_font() -> Font {
    let mut f = font(".SystemUIFont");
    f.fallbacks = Some(fallbacks());
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_chain_contains_segoe_ui_symbol() {
        let fb = fallbacks();
        let list = fb.fallback_list();
        assert!(
            list.iter().any(|s| s == "Segoe UI Symbol"),
            "fallback chain 缺 Segoe UI Symbol（Windows U+2327 兜底）: {list:?}"
        );
    }

    #[test]
    fn fallback_chain_has_five_entries() {
        let fb = fallbacks();
        assert_eq!(
            fb.fallback_list().len(),
            5,
            "fallback chain 应有 5 项（Symbols Nerd Font + 3 系统 symbol + 1 CJK）"
        );
    }

    #[test]
    fn code_font_has_fallbacks() {
        let f = code_font();
        assert_eq!(f.family.as_ref(), CODE_FONT_NAME);
        assert!(f.fallbacks.is_some(), "code_font 必须挂 fallback chain");
    }

    #[test]
    fn sans_font_uses_system_ui() {
        let f = sans_font();
        assert_eq!(
            f.family.as_ref(),
            ".SystemUIFont",
            "sans_font 应走 GPUI special name .SystemUIFont"
        );
        assert!(f.fallbacks.is_some(), "sans_font 必须挂 fallback chain");
    }

    #[test]
    fn fallbacks_is_singleton_via_oncelock() {
        // OnceLock 保证全程构造一次；两次调用应得到等价 chain（同 Arc 内部）
        let a = fallbacks();
        let b = fallbacks();
        assert_eq!(a.fallback_list(), b.fallback_list());
    }
}
