//! aish 视觉规范（M3d-ui-polish）：色值 / 字号 / 半径常量。
//!
//! 所有 view 用本文件常量，不要散写魔法值。改色调一处全局生效。
//! 设计文档：`docs/superpowers/specs/2026-05-08-aish-ui-polish-design.md`。

#![allow(dead_code)]

use gpui::{px, Pixels};

// ────────── 背景层（深 → 浅） ──────────
/// RootView 全局底色。纯黑（参考图 9 类 iOS 风）。
pub const BG_BASE: u32 = 0x000000;
/// 卡片 / chip 填充。亮一档让卡片"浮"出来。
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
/// SSH chip / 输入 focus / 选中 tab 底线。
pub const ACCENT_BLUE: u32 = 0x4a9eff;
/// 活跃指示 ●。
pub const ACCENT_GREEN: u32 = 0x4ec9b0;
/// 删除 / 错误 / × hover。
pub const ACCENT_RED: u32 = 0xff6b6b;
/// 连接中 / 警告（暂未启用）。
pub const ACCENT_YELLOW: u32 = 0xf5c242;

// ────────── Chip 底色（基于 accent 调成深色调，配 accent 文字色对比清晰） ──────────
pub const CHIP_BLUE_BG: u32 = 0x1f3a5c;
pub const CHIP_GREEN_BG: u32 = 0x16382f;

// ────────── 字号 ──────────
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
