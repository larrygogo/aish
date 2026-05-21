//! Theme token 定义。颜色、圆角、间距、字号四类。
//! 命名参考 shadcn/ui，HSLA 内部存储。

use gpui::{hsla, point, px, BoxShadow, Hsla, Pixels, Rgba};

#[derive(Clone, Copy)]
pub struct ColorTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub card_foreground: Hsla,
    pub popover: Hsla,
    pub popover_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    // M15 新加：按 variant 的 hover / active 状态色
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub destructive_hover: Hsla,
    pub destructive_active: Hsla,
    // M17 新加：accent 系列容器按下反馈
    pub accent_active: Hsla,
    // M18 新加：secondary_strongest — Ghost button active 用，比 secondary_active
    // 再亮一档。原因：Ghost 按钮常嵌 Card / Row 内，容器 hover 时已经是
    // secondary_hover，Ghost active 若同等级色块会与容器融为一体；跳一档保证
    // active 状态在任何容器背景下都可识别。
    pub secondary_strongest: Hsla,
    // M35.1 D1 新加：sidebar bg vertical gradient 顶色，与 `background` (=底色)
    // 形成 OpenSFTP 风 elevation。ΔL ≈ 2 让眼睛说不出哪里变了但感觉"层次"。
    // 仅 sidebar 容器使用，main 区仍纯 background。
    // light theme 暂同 background（不引入渐变风险，与 T17 light 实验状态对齐）。
    pub sidebar_bg_top: Hsla,
    /// terminal / workspace 主区背景语义（M38 paseo borrowing 落地）。
    /// 当前等同 `background`，留独立语义位置以便未来微调（如 tmux attach
    /// 时偏色、fullscreen 模式不同 tint 等）—— 无需修改全 view 代码。
    /// 借鉴 paseo `surfaceWorkspace` 的命名。
    pub surface_workspace: Hsla,
    /// Aurora 背景 layer 1 主色（M37 引入背景光晕，M39 Phase 2 抽 token）。
    /// 含 alpha — app.rs render 时直接用 `linear_color_stop(aurora_a, 0.0)
    /// → linear_color_stop(aurora_a.opacity(0.0), 1.0)` 做色彩弥散。
    /// 各 dark variant 用不同配色（默认: indigo / midnight: 加亮 indigo /
    /// warp: Warp 紫高饱和）。
    pub aurora_a: Hsla,
    /// Aurora 背景 layer 2 补色（同 aurora_a，但 hue 偏一档形成双色光晕）。
    /// 默认: cyan / midnight: 冷紫 / warp: Warp 粉。alpha 一般比 aurora_a
    /// 低一档（layer 2 是 layer 1 的辅助）。
    pub aurora_b: Hsla,
}

#[derive(Clone, Copy)]
pub struct Radius {
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub full: Pixels,
}

impl Default for Radius {
    fn default() -> Self {
        Self {
            sm: px(4.0),
            md: px(6.0),
            lg: px(8.0),
            full: px(9999.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Spacing {
    pub px_1: Pixels,
    pub px_2: Pixels,
    pub px_3: Pixels,
    pub px_4: Pixels,
    pub px_6: Pixels,
    pub px_8: Pixels,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            px_1: px(4.0),
            px_2: px(8.0),
            px_3: px(12.0),
            px_4: px(16.0),
            px_6: px(24.0),
            px_8: px(32.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FontSize {
    pub xs: Pixels,
    pub sm: Pixels,
    pub base: Pixels,
    pub lg: Pixels,
    pub xl: Pixels,
}

impl Default for FontSize {
    fn default() -> Self {
        Self {
            xs: px(10.0),
            sm: px(12.0),
            base: px(14.0),
            lg: px(16.0),
            xl: px(18.0),
        }
    }
}

/// Icon 尺寸 token，5 档覆盖 issh-ui 实际用例（14 / 16 / 18 是当前最常见值）。
/// 新代码请用 `theme.icon_size.X` 而非 `px(N)` 硬编码，方便未来批量调整密度。
///
/// 选档建议：
/// - `xs` 12px：紧贴文字的 inline icon（kbd 旁的快捷键 hint）
/// - `sm` 14px：dropdown menu item leading / textinput 末尾 icon
/// - `md` 16px：默认 icon（IconButton 默认 / Card avatar inline）
/// - `lg` 18px：toast / EmptyState 主图标
/// - `xl` 20px：Tab bar / Dialog header
#[derive(Clone, Copy)]
pub struct IconSize {
    pub xs: Pixels,
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub xl: Pixels,
}

impl Default for IconSize {
    fn default() -> Self {
        Self {
            xs: px(12.0),
            sm: px(14.0),
            md: px(16.0),
            lg: px(18.0),
            xl: px(20.0),
        }
    }
}

/// Opacity token — **仅放 state semantic opacity**（disabled / press）。
/// 视觉效果用的 opacity（glow 0.05 / hover overlay 0.25 / glass 0.4）是
/// caller-specific，不归 token —— 那是 view-level 决策不是设计 token。
///
/// principles.md #3 + charter §10：disabled 状态绝不改色，统一走 opacity。
#[derive(Clone, Copy)]
pub struct Opacity {
    /// disabled 元素的 opacity（M11 起 0.6 — Button cursor_not_allowed 状态）
    pub disabled: f32,
    /// 按下反馈 opacity（M15 Button press 0.7）— 物理触觉，即使 reduced_motion
    /// 也保留
    pub press: f32,
}

impl Default for Opacity {
    fn default() -> Self {
        Self {
            disabled: 0.6,
            press: 0.7,
        }
    }
}

/// 主题种类。运行时切换 dark / light 时，view 可 `theme(cx).kind` 查询当前
/// 主题决定特定行为（如 settings switch 的 checked 状态）。
///
/// **DarkMidnight**（M38 G）：深紫蓝 surface + 加亮 indigo accent，冷调实验性
/// 变体。**DarkWarp**（M39 Phase 1）：Warp 风温暖紫 surface + Warp 紫 accent
/// (#7C5CFC)，暖调实验性变体。两者跟默认 dark 并列，用户在 Settings 切换。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeKind {
    Dark,
    Light,
    DarkMidnight,
    DarkWarp,
}

impl ThemeKind {
    /// 当前主题是否属于「dark family」（dark / dark_midnight / dark_warp 等）。
    /// 用于 caller 决定 dark/light bifurcation 时省略对每个 dark variant 单独处理。
    pub fn is_dark(self) -> bool {
        matches!(
            self,
            ThemeKind::Dark | ThemeKind::DarkMidnight | ThemeKind::DarkWarp
        )
    }
}

pub struct Theme {
    pub kind: ThemeKind,
    pub colors: ColorTokens,
    pub radius: Radius,
    pub spacing: Spacing,
    /// 旧 5 档 size token（xs/sm/base/lg/xl）—— M26 起新增代码用
    /// `typography` 字段，font_size 保留 fallback 渐进迁移。
    pub font_size: FontSize,
    /// Icon 尺寸 5 档（M38 paseo borrowing 落地）。新 icon 请用
    /// `theme.icon_size.X`，旧 `px(N)` 硬编码逐步迁移。
    pub icon_size: IconSize,
    /// State opacity 2 档（disabled / press）。disabled 元素绝不改色，
    /// 统一走 opacity；press 反馈即使 reduced_motion 也保留。
    pub opacity: Opacity,
    /// M26 新增：9 个语义 type role（size × weight × color_role 三维）。
    /// caller 通过 `.typography(TypeRole::Title3, t)` 一行 apply。
    pub typography: super::typography::Typography,
    /// M27 新增：Component anatomy — Card/Dialog/List/Form/Page/Overlay
    /// 内部 padding / gap / 行高规则。跨主题相同。
    pub anatomy: super::anatomy::Anatomy,
    /// M30 新增：动画 duration + easing token（4 档语义 ms）。
    pub motion: super::motion::Motion,
    /// M30 新增：accessibility — "减少动画"偏好。true 时所有
    /// `animate_or_skip` 调用跳过 Animation 包装直接输出 end-state。
    /// 由 Settings UI toggle 写盘到 app_state.toml，启动时回灌到 Theme。
    pub reduced_motion: bool,
}

impl gpui::Global for Theme {}

/// M24 elevation system — 3 档 subtle shadow（Warp/Linear 风）。
/// alpha 极低，让 elevation 通过分层暗示而非高对比阴影。dark theme
/// 黑底上 shadow 不易显，alpha 翻倍；light theme 反之。
///
/// 用法：
/// ```ignore
/// .shadow(elevation_2(theme(cx).kind))
/// ```
pub fn elevation_1(kind: ThemeKind) -> Vec<BoxShadow> {
    // M38 paseo borrowing G: DarkMidnight 走 dark family alpha
    let alpha = if kind.is_dark() { 0.16 } else { 0.08 };
    vec![BoxShadow {
        color: hsla(0.0, 0.0, 0.0, alpha),
        offset: point(px(0.0), px(1.0)),
        blur_radius: px(2.0),
        spread_radius: px(0.0),
    }]
}

/// elevation-2 — popover / dropdown 用。
pub fn elevation_2(kind: ThemeKind) -> Vec<BoxShadow> {
    let alpha = if kind.is_dark() { 0.50 } else { 0.15 };
    vec![BoxShadow {
        color: hsla(0.0, 0.0, 0.0, alpha),
        offset: point(px(0.0), px(4.0)),
        blur_radius: px(12.0),
        spread_radius: px(0.0),
    }]
}

/// elevation-3 — modal / toast 用，最高层悬浮。
pub fn elevation_3(kind: ThemeKind) -> Vec<BoxShadow> {
    let alpha = if kind.is_dark() { 0.65 } else { 0.22 };
    vec![BoxShadow {
        color: hsla(0.0, 0.0, 0.0, alpha),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(0.0),
    }]
}

/// 把 0xRRGGBB hex 转 Hsla。
pub(crate) fn hex(rgb: u32) -> Hsla {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    Rgba { r, g, b, a: 1.0 }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_pure_red_roundtrip() {
        let h = hex(0xff0000);
        assert!(h.h.abs() < 0.01 || (h.h - 1.0).abs() < 0.01);
        assert!(h.s > 0.99);
    }

    #[test]
    fn radius_defaults_ordered() {
        let r = Radius::default();
        assert!(r.sm < r.md);
        assert!(r.md < r.lg);
        assert!(r.lg < r.full);
    }

    #[test]
    fn spacing_defaults_ordered() {
        let s = Spacing::default();
        assert!(s.px_1 < s.px_2);
        assert!(s.px_2 < s.px_3);
        assert!(s.px_3 < s.px_4);
        assert!(s.px_4 < s.px_6);
        assert!(s.px_6 < s.px_8);
    }

    #[test]
    fn font_size_defaults_ordered() {
        let f = FontSize::default();
        assert!(f.xs < f.sm);
        assert!(f.sm < f.base);
        assert!(f.base < f.lg);
        assert!(f.lg < f.xl);
    }

    #[test]
    fn icon_size_defaults_ordered_and_match_doc_values() {
        let i = IconSize::default();
        assert!(i.xs < i.sm);
        assert!(i.sm < i.md);
        assert!(i.md < i.lg);
        assert!(i.lg < i.xl);
        // 与 doc comment 描述的具体值对齐（12/14/16/18/20）
        assert_eq!(i.xs, px(12.0));
        assert_eq!(i.sm, px(14.0));
        assert_eq!(i.md, px(16.0));
        assert_eq!(i.lg, px(18.0));
        assert_eq!(i.xl, px(20.0));
    }

    #[test]
    fn opacity_defaults_in_sensible_range() {
        let o = Opacity::default();
        // disabled 应该 dim 但仍可读，介于 0.4-0.7
        assert!(o.disabled > 0.4 && o.disabled < 0.7);
        // press 应该比 disabled 更轻（press 是瞬时反馈，不该太显眼）
        assert!(o.press > o.disabled);
        // 都不能 = 0 (透明无意义) 或 1 (没反馈)
        assert!(o.disabled > 0.0 && o.disabled < 1.0);
        assert!(o.press > 0.0 && o.press < 1.0);
    }
}
