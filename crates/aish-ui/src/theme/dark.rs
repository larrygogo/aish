//! 默认 dark 主题 — M24 Warp/Linear 风重塑。
//!
//! 设计语言：准黑白 + 单 indigo accent。中性 neutral 灰阶 8 档 ramp 表达
//! elevation，accent #5E6AD2 (Linear indigo) 仅在 CTA / focus / 关键交互。
//! 状态色（success/warning/destructive）desaturate 一档，避免 Tokyo Night
//! 风高饱和与 minimal 灰阶冲突。
//!
//! 与 M11/M15/M17 旧版（终端绿 + Tokyo Night 红粉）相比：
//! - primary: #00CC33 → #5E6AD2（Matrix 绿换 Linear 紫）
//! - accent:  #2F6E3E → #2D3047（暗绿换深紫灰）
//! - destructive: #F7768E → #E5484D（粉红换真红）
//! - bg/card/popover: #050505/#0D0D0D/#161616 → #08090A/#101113/#191B1F
//!   （加 hue 偏冷，与 indigo accent 协调）

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind};

impl Theme {
    pub fn dark() -> Self {
        Self {
            kind: ThemeKind::Dark,
            colors: ColorTokens {
                // ── Neutral L0-L7（dark）────────────────────────────────
                // L0 bg 最底 / L1 card / L2 popover / L4 secondary / L5 hover
                // / L6 active / L7 border = L4
                background: hex(0x08090a),
                foreground: hex(0xf4f5f8), // 高对比白（dev tool 惯例）
                card: hex(0x101113),
                card_foreground: hex(0xf4f5f8),
                popover: hex(0x191b1f),
                popover_foreground: hex(0xf4f5f8),
                // ── Primary = Linear Indigo ─────────────────────────────
                primary: hex(0x5e6ad2),
                primary_foreground: hex(0xffffff),
                // ── Secondary / Muted 中性灰 ────────────────────────────
                secondary: hex(0x26282d),
                secondary_foreground: hex(0xc8cacf),
                muted: hex(0x26282d),
                muted_foreground: hex(0x8b8d97), // 冷调中灰
                // ── Accent 容器 hover bg — 深紫灰，与 primary 同 hue 但
                // 极低饱和，避免大面积 fill 抢主信息 ────────────────────
                accent: hex(0x2d3047),
                accent_foreground: hex(0xf4f5f8),
                // ── Destructive 真红（替代 Tokyo Night 粉红）────────────
                destructive: hex(0xe5484d),
                destructive_foreground: hex(0xffffff),
                // ── Border / Input ──────────────────────────────────────
                // M35 T3: border 从 L4 (0x26282d) → L5+ (0x33363c) 提亮，与
                // card L1 (0x101113) 的 ΔL 从 8 → 14，让 Outlined Card 边框
                // 在 dark theme 下真的可见（之前几乎看不见）。
                // input bg 与 card 同 (L1)，比 background 略亮"凸出"感
                border: hex(0x33363c),
                input: hex(0x101113),
                // focus ring = primary（accent 系），caller 加 alpha 实现 glow
                ring: hex(0x5e6ad2),
                // 状态色 desaturate 一档
                success: hex(0x4fbb72),
                warning: hex(0xe8a658),
                // ── Variant 阶梯（M15 lightness 单调）──────────────────
                // primary indigo 阶梯：base → hover (提亮) → active (再提亮)
                primary_hover: hex(0x7079db),
                primary_active: hex(0x8189e0),
                // secondary 灰阶（L4 base → L5 hover → L6 active）
                secondary_hover: hex(0x2e3036),
                secondary_active: hex(0x3a3d44),
                // destructive 阶梯：真红 → 偏亮红 → 浅红
                destructive_hover: hex(0xed5e63),
                destructive_active: hex(0xf27479),
                // accent_active 比 accent 更深（容器按下"沉下去"，M17 方向）
                accent_active: hex(0x1f2236),
                // M18 Ghost button 按下反馈，比 secondary_active 再亮一档
                secondary_strongest: hex(0x4a4d55),
                // M35.1 D1: sidebar 顶色，比 background #08090a 亮 ΔL≈2，
                // 配合 linear_gradient(180°) 从 top 渐到 background 形成 elevation
                sidebar_bg_top: hex(0x0a0b0e),
            },
            radius: Radius::default(),
            spacing: Spacing::default(),
            font_size: FontSize::default(),
            icon_size: super::tokens::IconSize::default(),
            opacity: super::tokens::Opacity::default(),
            typography: super::typography::Typography::default(),
            anatomy: super::anatomy::Anatomy::default(),
            motion: super::motion::Motion::default(),
            reduced_motion: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_primary_is_indigo() {
        let t = Theme::dark();
        // Linear indigo #5E6AD2 归一化 hue ≈ 0.66（蓝紫方向，介于纯蓝 0.67 和紫 0.75）
        // 容差宽一点防 hex→hsla 转换浮点误差
        assert!(
            t.colors.primary.h > 0.60 && t.colors.primary.h < 0.72,
            "primary hue {} 不在 indigo 范围",
            t.colors.primary.h
        );
    }

    #[test]
    fn dark_destructive_is_red() {
        let t = Theme::dark();
        // #E5484D 是真红，hue ≈ 0 或 ≈ 1（红色环绕 0/1 边界）
        // 旧 Tokyo Night #F7768E 偏粉，hue ≈ 0.96 也满足；新 #E5484D hue ≈ 0.00
        assert!(t.colors.destructive.h < 0.05 || t.colors.destructive.h > 0.95);
    }

    #[test]
    fn dark_background_is_very_dark() {
        let t = Theme::dark();
        // #08090A 极低亮度
        assert!(t.colors.background.l < 0.05);
    }

    #[test]
    fn dark_background_is_low_saturation() {
        let t = Theme::dark();
        // M24 允许偏冷一档（#08090A 有微弱 cool hue），但 saturation 仍极低
        assert!(t.colors.background.s < 0.15);
    }

    #[test]
    fn dark_primary_hover_is_lighter_than_primary() {
        let t = Theme::dark();
        assert!(t.colors.primary_hover.l > t.colors.primary.l);
    }

    #[test]
    fn dark_primary_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.primary_active.l > t.colors.primary_hover.l);
    }

    #[test]
    fn dark_secondary_hover_is_lighter_than_secondary() {
        let t = Theme::dark();
        assert!(t.colors.secondary_hover.l > t.colors.secondary.l);
    }

    #[test]
    fn dark_secondary_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.secondary_active.l > t.colors.secondary_hover.l);
    }

    #[test]
    fn dark_destructive_hover_is_lighter_than_destructive() {
        let t = Theme::dark();
        assert!(t.colors.destructive_hover.l > t.colors.destructive.l);
    }

    #[test]
    fn dark_destructive_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.destructive_active.l > t.colors.destructive_hover.l);
    }

    #[test]
    fn dark_accent_active_is_darker_than_accent() {
        let t = Theme::dark();
        // M17：accent_active 比 accent 更深（容器按下"沉下去"，与 M15 系列变亮方向相反）
        assert!(t.colors.accent_active.l < t.colors.accent.l);
    }

    #[test]
    fn dark_secondary_strongest_is_lighter_than_active() {
        let t = Theme::dark();
        // M18：secondary_strongest 比 active 再亮一档（Ghost button 按下反馈）
        assert!(t.colors.secondary_strongest.l > t.colors.secondary_active.l);
    }

    /// M35 T3：border 与 card 的 ΔL ≥ 0.05 — Outlined Card 在 dark theme
    /// 下边框必须可见，否则失去 outlined 视觉意图。
    #[test]
    fn dark_border_visible_against_card() {
        let t = Theme::dark();
        let delta_l = (t.colors.border.l - t.colors.card.l).abs();
        assert!(
            delta_l >= 0.05,
            "border / card ΔL = {} < 0.05，border 不可见",
            delta_l
        );
    }
}
