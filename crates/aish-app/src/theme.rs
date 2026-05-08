//! aish 视觉规范（M3d-ui-polish）：色值 / 字号 / 半径常量。
//!
//! 所有 view 用本文件常量，不要散写魔法值。改色调一处全局生效。
//! 设计文档：`docs/superpowers/specs/2026-05-08-aish-ui-polish-design.md`。

#![allow(dead_code)]

use gpui::{px, Pixels};

// ────────── 背景层（深 → 浅）—— Moshi app 类 iOS 暗色风 ──────────
/// RootView 全局底色。纯黑（参考 Moshi 应用本体）。
pub const BG_BASE: u32 = 0x000000;
/// 卡片 / chip 填充，比 base 亮一档让卡片"浮"起。
pub const BG_ELEVATED: u32 = 0x1c1d22;
/// hover 高亮态。
pub const BG_HOVER: u32 = 0x26282f;
/// 选中态 / 按下态。
pub const BG_SELECTED: u32 = 0x2f3138;

// ────────── 边框 ──────────
/// 卡片默认边框，几乎贴 elevated 不刺眼。
pub const BORDER_SUBTLE: u32 = 0x1f2128;
/// hover / focus 边框。
pub const BORDER_STRONG: u32 = 0x2f323a;

// ────────── 文本 ──────────
pub const TEXT_PRIMARY: u32 = 0xeaeaee;
pub const TEXT_SECONDARY: u32 = 0x888a93;
pub const TEXT_MUTED: u32 = 0x5b5d66;

// ────────── 强调色 ──────────
pub const ACCENT_BLUE: u32 = 0x4a9eff;
/// ACCENT_BLUE 按钮（如 Save）hover 时的 bg。比 ACCENT_BLUE 亮一档。
pub const ACCENT_BLUE_HOVER: u32 = 0x66b3ff;
pub const ACCENT_GREEN: u32 = 0x4ec9b0;
pub const ACCENT_RED: u32 = 0xff6b6b;
pub const ACCENT_YELLOW: u32 = 0xf5c242;

// ────────── Chip 底色（accent 同色调减明度，配 accent 文字色对比清晰） ──────────
pub const CHIP_BLUE_BG: u32 = 0x1f3a5c;
pub const CHIP_GREEN_BG: u32 = 0x16382f;

// ────────── Avatar 调色板（host 首字母图标背景轮换用） ──────────
pub const AVATAR_PALETTE: &[u32] = &[
    0x6366f1, // indigo
    0x8b5cf6, // violet
    0xec4899, // pink
    0xef4444, // red
    0xf59e0b, // amber
    0x10b981, // emerald
    0x06b6d4, // cyan
    0x3b82f6, // blue
];

/// 按 string hash 选 avatar 颜色，保证同一 host label 总是同色。
pub fn avatar_color_for(label: &str) -> u32 {
    let mut sum: u32 = 0;
    for b in label.bytes() {
        sum = sum.wrapping_add(b as u32);
    }
    AVATAR_PALETTE[(sum as usize) % AVATAR_PALETTE.len()]
}

// ────────── 字号（iOS 风：标题大、副信息小、对比明显） ──────────
/// section 大标题（"已保存的连接"等）。
pub fn text_xl() -> Pixels {
    px(20.0)
}
/// host label / tab 标题 / 主要内容。
pub fn text_lg() -> Pixels {
    px(15.0)
}
/// 副信息、按钮文字、chip。
pub fn text_sm() -> Pixels {
    px(12.0)
}
/// 时间戳 / 占位说明。
pub fn text_xs() -> Pixels {
    px(11.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_layers_are_distinct() {
        // 编译期 sanity：背景四层不能重复
        let layers = [BG_BASE, BG_ELEVATED, BG_HOVER, BG_SELECTED];
        for i in 0..layers.len() {
            for j in (i + 1)..layers.len() {
                assert_ne!(layers[i], layers[j], "bg layer {} == {}", i, j);
            }
        }
    }

    #[test]
    fn font_sizes_ordered() {
        assert!(text_xl() > text_lg());
        assert!(text_lg() > text_sm());
        assert!(text_sm() > text_xs());
    }
}
