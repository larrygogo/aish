//! Skeleton — M28 异步加载占位原语。
//!
//! 不提供成品 layout（让 caller mirror 真实 layout 用 block/circle 组合
//! 出 host card / list row 等 placeholder），仅提供两种基础形状：
//! - `Skeleton::block()` — 矩形 + rounded_sm（文字行 / Card 内容）
//! - `Skeleton::circle()` — 圆形（avatar）
//!
//! v1 实现：纯 bg(secondary) 静态占位，shimmer flag 预留 hook 但无副作用
//! （后续 M30 加 animation 后可加 shimmer 反复扫光）。

use gpui::{div, prelude::*, IntoElement, Pixels, Window};

use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkeletonShape {
    Block,
    Circle,
}

#[derive(IntoElement)]
pub struct Skeleton {
    shape: SkeletonShape,
    width: Option<Pixels>,
    height: Option<Pixels>,
    shimmer: bool,
}

impl Skeleton {
    /// 矩形 placeholder（rounded_sm，用于文字行 / 内容块）。
    pub fn block() -> Self {
        Self {
            shape: SkeletonShape::Block,
            width: None,
            height: None,
            shimmer: false,
        }
    }

    /// 圆形 placeholder（用于 avatar / dot indicator）。
    pub fn circle() -> Self {
        Self {
            shape: SkeletonShape::Circle,
            width: None,
            height: None,
            shimmer: false,
        }
    }

    pub fn w(mut self, w: Pixels) -> Self {
        self.width = Some(w);
        self
    }

    pub fn h(mut self, h: Pixels) -> Self {
        self.height = Some(h);
        self
    }

    /// 同时设 w + h（多用于 circle，宽高一致）。
    pub fn size(mut self, s: Pixels) -> Self {
        self.width = Some(s);
        self.height = Some(s);
        self
    }

    /// shimmer 扫光动画 — v1 stub（M30 animation 落地后接入）。
    pub fn with_shimmer(mut self, b: bool) -> Self {
        self.shimmer = b;
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _w: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let t = theme(cx);
        let mut d = div().bg(t.colors.secondary);
        d = match self.shape {
            SkeletonShape::Block => d.rounded(t.radius.sm),
            SkeletonShape::Circle => d.rounded_full(),
        };
        if let Some(w) = self.width {
            d = d.w(w);
        }
        if let Some(h) = self.height {
            d = d.h(h);
        }
        // shimmer 字段保留但 v1 无 effect — 借 _ 抑制 dead_code
        let _ = self.shimmer;
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn block_defaults() {
        let s = Skeleton::block();
        assert_eq!(s.shape, SkeletonShape::Block);
        assert!(s.width.is_none());
        assert!(s.height.is_none());
        assert!(!s.shimmer);
    }

    #[test]
    fn circle_defaults() {
        let s = Skeleton::circle();
        assert_eq!(s.shape, SkeletonShape::Circle);
    }

    #[test]
    fn builders_set_dimensions() {
        let s = Skeleton::block().w(px(200.0)).h(px(16.0));
        assert_eq!(s.width, Some(px(200.0)));
        assert_eq!(s.height, Some(px(16.0)));
    }

    #[test]
    fn size_sets_both_w_h() {
        let s = Skeleton::circle().size(px(40.0));
        assert_eq!(s.width, Some(px(40.0)));
        assert_eq!(s.height, Some(px(40.0)));
    }

    #[test]
    fn shimmer_flag_stored() {
        let s = Skeleton::block().with_shimmer(true);
        assert!(s.shimmer);
    }
}
