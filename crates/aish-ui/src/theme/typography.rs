//! Typography token — M26 真正系统化的 type hierarchy。
//!
//! 9 个语义 type role，每个 = (size, weight, default_color_role)。
//! caller 通过 `.typography(TypeRole::Title3, t)` 一行 apply。
//!
//! 设计原则参考 Linear / Warp / Stripe Dashboard：
//! - **size + weight + color 三维**建立层级，而非单 size 跳跃
//! - body 与 title_3 仅差 1px (13→14)，主对比靠 weight (400→600)
//! - body 与 caption 同 size (13/12)，主对比靠 color (fg→muted_fg)
//!
//! 与旧 `FontSize` (xs/sm/base/lg/xl) **并存**，渐进迁移期不强制全改。

use gpui::{px, FontWeight, Hsla, Pixels, Styled};

use super::tokens::{ColorTokens, Theme};

/// 等宽字体名 — 与 aish-app 的 terminal::font::FONT_NAME 一致（同一份
/// bundled 字体）。aish-ui 不直接 include_bytes 字体（grid 渲染由 aish-app
/// 控），但需要一个 well-known name 让 typography(TypeRole::Code) 接管
/// font_family。caller 需保证 aish-app 已 init font。
pub const CODE_FONT_NAME: &str = "JetBrainsMono Nerd Font";

/// 默认 color role —— caller 链 `.text_color(custom)` 仍可 override。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorRole {
    Foreground,
    MutedForeground,
    SecondaryForeground,
}

impl ColorRole {
    pub fn resolve(self, colors: &ColorTokens) -> Hsla {
        match self {
            ColorRole::Foreground => colors.foreground,
            ColorRole::MutedForeground => colors.muted_foreground,
            ColorRole::SecondaryForeground => colors.secondary_foreground,
        }
    }
}

/// 一个 type style 三元组（size × weight × default color）。
#[derive(Clone, Copy)]
pub struct TypeStyle {
    pub size: Pixels,
    pub weight: FontWeight,
    pub default_color_role: ColorRole,
}

/// 9 个语义 role —— 对应 `.typography(TypeRole::Title3, t)` 的枚举值。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeRole {
    /// 11/400/muted — 快捷键 / badge / 角标 micro 文字
    Micro,
    /// 12/400/muted — meta 信息 / helper / 上次连接时间
    Caption,
    /// 13/400/fg — 列表项 / Card 内容正文
    Body,
    /// 13/500/fg — inline 强调 / 当前选中项 label
    BodyStrong,
    /// 13/500/fg — form field label / 表头
    Label,
    /// 14/600/fg — Card / section header / list group title
    Title3,
    /// 16/600/fg — Dialog title / sidebar group title
    Title2,
    /// 20/600/fg — page title (Home / Settings 等)
    Title1,
    /// 13/400/fg — inline code / 路径文字。M35 T2 起 typography() trait
    /// 自动 apply `CODE_FONT_NAME` font_family（JetBrains Mono Nerd Font）。
    Code,
}

/// 全套 typography token。Theme 持一个 default 实例，未来若想做 dense /
/// cozy 切换则提供 alt constructor。
#[derive(Clone, Copy)]
pub struct Typography {
    pub micro: TypeStyle,
    pub caption: TypeStyle,
    pub body: TypeStyle,
    pub body_strong: TypeStyle,
    pub label: TypeStyle,
    pub title_3: TypeStyle,
    pub title_2: TypeStyle,
    pub title_1: TypeStyle,
    pub code: TypeStyle,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            micro: TypeStyle {
                size: px(11.0),
                weight: FontWeight::NORMAL,
                default_color_role: ColorRole::MutedForeground,
            },
            caption: TypeStyle {
                size: px(12.0),
                weight: FontWeight::NORMAL,
                default_color_role: ColorRole::MutedForeground,
            },
            body: TypeStyle {
                size: px(13.0),
                weight: FontWeight::NORMAL,
                default_color_role: ColorRole::Foreground,
            },
            body_strong: TypeStyle {
                size: px(13.0),
                weight: FontWeight::MEDIUM,
                default_color_role: ColorRole::Foreground,
            },
            label: TypeStyle {
                size: px(13.0),
                weight: FontWeight::MEDIUM,
                default_color_role: ColorRole::Foreground,
            },
            title_3: TypeStyle {
                size: px(14.0),
                weight: FontWeight::SEMIBOLD,
                default_color_role: ColorRole::Foreground,
            },
            title_2: TypeStyle {
                size: px(16.0),
                weight: FontWeight::SEMIBOLD,
                default_color_role: ColorRole::Foreground,
            },
            title_1: TypeStyle {
                size: px(20.0),
                weight: FontWeight::SEMIBOLD,
                default_color_role: ColorRole::Foreground,
            },
            code: TypeStyle {
                size: px(13.0),
                weight: FontWeight::NORMAL,
                default_color_role: ColorRole::Foreground,
            },
        }
    }
}

impl Typography {
    /// 取指定 role 的 TypeStyle。`.typography(role, t)` 内部用。
    pub fn role(&self, role: TypeRole) -> TypeStyle {
        match role {
            TypeRole::Micro => self.micro,
            TypeRole::Caption => self.caption,
            TypeRole::Body => self.body,
            TypeRole::BodyStrong => self.body_strong,
            TypeRole::Label => self.label,
            TypeRole::Title3 => self.title_3,
            TypeRole::Title2 => self.title_2,
            TypeRole::Title1 => self.title_1,
            TypeRole::Code => self.code,
        }
    }
}

/// Extension trait — 让 `div().typography(TypeRole::Title3, t)` 一行
/// 应用 size + weight + 默认 color。caller 后续仍可 `.text_color(...)`
/// override default color_role。
///
/// 实现接 `Styled` blanket，任何 GPUI styled element（div / svg / 自定义
/// view） 都自动获得 `.typography(...)` 方法。
pub trait TypographyExt: Styled + Sized {
    fn typography(self, role: TypeRole, t: &Theme) -> Self {
        let style = t.typography.role(role);
        let s = self
            .text_size(style.size)
            .text_color(style.default_color_role.resolve(&t.colors));
        // M35 T2: Code role 自动应用 monospace font_family，让 host /
        // user@host:port / 路径等 SSH 主题信息有 developer tool 等宽视觉。
        // M35.2 T2: 走 .font(Font) 路径同时挂 fallback chain，weight 一并塞
        // Font（.font() 会覆盖 text_style.font_weight）。非 Code role 走原
        // .font_weight() 路径不动 family/fallback。
        if role == TypeRole::Code {
            let mut code = crate::font::code_font();
            code.weight = style.weight;
            s.font(code)
        } else {
            s.font_weight(style.weight)
        }
    }
}

impl<E: Styled + Sized> TypographyExt for E {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size_monotone() {
        let t = Typography::default();
        // 严格单调：micro < caption < body == body_strong < title_3 < title_2 < title_1
        assert!(t.micro.size < t.caption.size);
        assert!(t.caption.size < t.body.size);
        assert_eq!(t.body.size, t.body_strong.size);
        assert_eq!(t.body.size, t.label.size);
        assert_eq!(t.body.size, t.code.size);
        assert!(t.body.size < t.title_3.size);
        assert!(t.title_3.size < t.title_2.size);
        assert!(t.title_2.size < t.title_1.size);
    }

    #[test]
    fn default_weight_progression() {
        let t = Typography::default();
        // micro/caption/body/code = 400 normal
        assert_eq!(t.micro.weight, FontWeight::NORMAL);
        assert_eq!(t.caption.weight, FontWeight::NORMAL);
        assert_eq!(t.body.weight, FontWeight::NORMAL);
        assert_eq!(t.code.weight, FontWeight::NORMAL);
        // body_strong/label = 500 medium
        assert_eq!(t.body_strong.weight, FontWeight::MEDIUM);
        assert_eq!(t.label.weight, FontWeight::MEDIUM);
        // title_* = 600 semibold
        assert_eq!(t.title_3.weight, FontWeight::SEMIBOLD);
        assert_eq!(t.title_2.weight, FontWeight::SEMIBOLD);
        assert_eq!(t.title_1.weight, FontWeight::SEMIBOLD);
    }

    #[test]
    fn micro_and_caption_default_muted() {
        let t = Typography::default();
        assert_eq!(t.micro.default_color_role, ColorRole::MutedForeground);
        assert_eq!(t.caption.default_color_role, ColorRole::MutedForeground);
    }

    #[test]
    fn body_strong_same_size_as_body() {
        // 关键设计：body_strong 与 body 仅差 weight，不差 size
        let t = Typography::default();
        assert_eq!(t.body.size, t.body_strong.size);
        assert!(t.body_strong.weight > t.body.weight);
    }

    #[test]
    fn label_same_as_body_strong() {
        // label 与 body_strong 等价（13/500/fg），仅语义不同
        let t = Typography::default();
        assert_eq!(t.label.size, t.body_strong.size);
        assert_eq!(t.label.weight, t.body_strong.weight);
    }

    #[test]
    fn typography_role_lookup() {
        let t = Typography::default();
        assert_eq!(t.role(TypeRole::Title1).size, t.title_1.size);
        assert_eq!(t.role(TypeRole::Body).weight, t.body.weight);
        assert_eq!(
            t.role(TypeRole::Caption).default_color_role,
            ColorRole::MutedForeground
        );
    }

    /// M35 T2：Code role 是 normal-weight body-size fg-color，与 Body 仅
    /// 在 typography() trait apply 时差 font_family（自动加 monospace）。
    #[test]
    fn code_role_same_metrics_as_body() {
        let t = Typography::default();
        assert_eq!(t.code.size, t.body.size);
        assert_eq!(t.code.weight, t.body.weight);
        assert_eq!(t.code.default_color_role, ColorRole::Foreground);
    }

    #[test]
    fn code_font_name_is_jetbrains_mono() {
        assert_eq!(CODE_FONT_NAME, "JetBrainsMono Nerd Font");
    }
}
