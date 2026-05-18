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

/// fallback chain — 按"图标 > 系统符号 > 简中 CJK > 繁中/日韩"顺序，平台
/// 找不到的字体自动跳过。
///
/// 图标 / 符号：
/// - **Symbols Nerd Font**：Nerd Font 图标符号专用，Linux/macOS Homebrew 常装
/// - **Segoe UI Symbol**：Windows 内置，覆盖 Miscellaneous Technical（含 U+2327）
/// - **Apple Symbols**：macOS 内置，同上覆盖
/// - **Noto Sans Symbols 2**：Linux 多数发行版预装 / Google Noto
///
/// 简中 CJK（按系统默认优先）：
/// - **PingFang SC**：macOS 系统中文默认（macOS 10.11+）
/// - **Microsoft YaHei**：Windows 系统中文默认（Win 7+）
/// - **Source Han Sans SC** / **思源黑体**：Adobe + Google 跨平台开源，部分 Linux 装
/// - **Noto Sans CJK SC**：Google Noto，Linux 多数发行版 noto-cjk 包提供
/// - **WenQuanYi Micro Hei** / **文泉驿微米黑**：老 Linux 发行版默认
///
/// 日韩（罕用兜底）：
/// - **Hiragino Sans**：macOS 日文默认
/// - **Yu Gothic**：Windows 日文默认
const FONT_FALLBACK_CHAIN: &[&str] = &[
    // 图标 / 符号
    "Symbols Nerd Font",
    "Segoe UI Symbol",
    "Apple Symbols",
    "Noto Sans Symbols 2",
    // 简中 CJK（按系统默认顺序）
    "PingFang SC",
    "Microsoft YaHei",
    "Source Han Sans SC",
    "Noto Sans CJK SC",
    "WenQuanYi Micro Hei",
    // 日韩
    "Hiragino Sans",
    "Yu Gothic",
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
    fn fallback_chain_has_entries() {
        let fb = fallbacks();
        let list = fb.fallback_list();
        assert!(
            list.len() >= 10,
            "fallback chain 应至少 10 项（图标 + 简中 CJK + 日韩）实际 {}",
            list.len()
        );
    }

    #[test]
    fn fallback_chain_covers_cjk_per_os() {
        let fb = fallbacks();
        let list = fb.fallback_list();
        // 每个平台都至少有一个 CJK 字体兜底
        assert!(
            list.iter().any(|s| s == "PingFang SC"),
            "macOS CJK 默认 PingFang SC 缺失"
        );
        assert!(
            list.iter().any(|s| s == "Microsoft YaHei"),
            "Windows CJK 默认 Microsoft YaHei 缺失"
        );
        assert!(
            list.iter().any(|s| s == "Noto Sans CJK SC"),
            "Linux CJK Noto 缺失"
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
