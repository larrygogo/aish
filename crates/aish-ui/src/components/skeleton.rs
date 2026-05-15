//! Skeleton — M28 异步加载占位原语。
//!
//! 不提供成品 layout（让 caller mirror 真实 layout 用 block/circle 组合
//! 出 host card / list row 等 placeholder），仅提供两种基础形状：
//! - `Skeleton::block(id)` — 矩形 + rounded_sm（文字行 / Card 内容）
//! - `Skeleton::circle(id)` — 圆形（avatar）
//!
//! M30 起接入 shimmer 动画：`.with_shimmer(true)` 时 opacity 0.5↔1.0
//! sine 呼吸 1.2s 循环（`pulsating_between` easing + `Animation::repeat`）。
//! `reduced_motion=true` 时通过 `animate_or_skip` 自动 fallback 到 opacity=1.0
//! 静态（无 shimmer）。
//!
//! caller 需提供 unique `ElementId`（与 GPUI Animation state 绑定，同 parent
//! 下多个 shimmer skeleton 用 `("skeleton", index)` 等区分）。

use std::rc::Rc;
use std::time::Duration;

use gpui::{div, prelude::*, ElementId, IntoElement, Pixels, Window};

use crate::theme::{animate_or_skip, theme};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkeletonShape {
    Block,
    Circle,
}

#[derive(IntoElement)]
pub struct Skeleton {
    id: ElementId,
    shape: SkeletonShape,
    width: Option<Pixels>,
    height: Option<Pixels>,
    shimmer: bool,
}

impl Skeleton {
    /// 矩形 placeholder（rounded_sm，用于文字行 / 内容块）。
    /// `id` 是 GPUI Animation state 的绑定 key — caller 需在 element tree
    /// 内保证 unique（多个 shimmer skeleton 用 `("name", index)` 区分）。
    pub fn block(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            shape: SkeletonShape::Block,
            width: None,
            height: None,
            shimmer: false,
        }
    }

    /// 圆形 placeholder（用于 avatar / dot indicator）。
    pub fn circle(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
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

    /// M30：shimmer = true 时 opacity sine 呼吸（0.5↔1.0，1.2s 循环）。
    /// reduced_motion 偏好启用时通过 `animate_or_skip` 自动 fallback 到 opacity=1.0
    /// 静态（视觉退化为纯 bg(secondary) 占位）。
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

        if self.shimmer {
            // pulsating_between sine 呼吸：delta 永远 ∈ [0.5, 1.0]，
            // Animation::repeat 让 delta 在 [0, 1] 循环（pulsating 内部用
            // sin(delta * 2π) 算 breath 曲线，1.2s 一个完整周期）。
            let easing = Rc::new(gpui::pulsating_between(0.5, 1.0));
            animate_or_skip(
                d,
                t,
                self.id,
                gpui::Animation::new(Duration::from_millis(1200))
                    .repeat()
                    .with_easing(move |delta| easing(delta)),
                |el, alpha| el.opacity(alpha),
            )
        } else {
            d.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn block_defaults() {
        let s = Skeleton::block("test");
        assert_eq!(s.shape, SkeletonShape::Block);
        assert!(s.width.is_none());
        assert!(s.height.is_none());
        assert!(!s.shimmer);
    }

    #[test]
    fn circle_defaults() {
        let s = Skeleton::circle("test");
        assert_eq!(s.shape, SkeletonShape::Circle);
    }

    #[test]
    fn builders_set_dimensions() {
        let s = Skeleton::block("test").w(px(200.0)).h(px(16.0));
        assert_eq!(s.width, Some(px(200.0)));
        assert_eq!(s.height, Some(px(16.0)));
    }

    #[test]
    fn size_sets_both_w_h() {
        let s = Skeleton::circle("test").size(px(40.0));
        assert_eq!(s.width, Some(px(40.0)));
        assert_eq!(s.height, Some(px(40.0)));
    }

    #[test]
    fn shimmer_flag_stored() {
        let s = Skeleton::block("test").with_shimmer(true);
        assert!(s.shimmer);
    }

    /// M30: pulsating_between 在端点和半周期处取值断言（与 GPUI 内部
    /// `(t * t * t + t) / 2.0` 公式对照）— 验证我们引入的 easing 输出
    /// 严格落在 [0.5, 1.0]，不会让 skeleton 完全透明（用户看不到 placeholder）。
    #[test]
    fn shimmer_easing_within_bounds() {
        let e = gpui::pulsating_between(0.5, 1.0);
        // sin(0)=0 → breath=0 → normalized=0.5 → 0.5 + 0.5*0.5 = 0.75
        // 数学上 t=0 时 alpha 应是 (min+max)/2 = 0.75
        let a0 = e(0.0);
        let a_mid = e(0.5);
        let a_end = e(1.0);
        // 所有值都应在 [0.5, 1.0]，决不让 skeleton 完全透明
        for a in [a0, a_mid, a_end] {
            assert!(
                (0.5..=1.0).contains(&a),
                "shimmer alpha {} 超出 [0.5, 1.0]",
                a
            );
        }
    }
}
