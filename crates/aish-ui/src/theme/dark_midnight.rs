//! Dark Midnight 主题 — M38 paseo borrowing G 实验性变体。
//!
//! 设计语言：深紫蓝 surface + 加亮 indigo accent。比默认 dark 更冷、更紫，
//! 让 Linear indigo 在 surface 上更显眼。借鉴 paseo `midnightDarkColors` 配色
//! 方向（深紫蓝），但保留 aish 自己的 primary indigo (#5E6AD2 系) 不换 hue。
//!
//! 与默认 `Theme::dark()` 的差异：
//! - background / card / popover 整体 hue 朝紫蓝偏（230° 方向）+ saturation +5%
//! - primary `#5E6AD2` → `#6B7AE0`（同 hue +10% lightness，让 accent 在更冷的
//!   surface 上不被吃掉）
//! - foreground 略偏蓝白（#F0F2FF）
//! - destructive / success / warning 保持一致（跨主题视觉锚点）
//!
//! 启用方式（暂无 Settings UI 切换）：手动编辑
//! `{config_dir}/aish/app_state.toml`，设 `theme = "midnight"`。

use gpui::hsla;

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind};

impl Theme {
    pub fn dark_midnight() -> Self {
        Self {
            kind: ThemeKind::DarkMidnight,
            colors: ColorTokens {
                // ── Surface 阶梯（朝紫蓝偏，hue 230° 方向）──────────────
                background: hex(0x0c0d18),
                foreground: hex(0xf0f2ff),
                card: hex(0x14162a),
                card_foreground: hex(0xf0f2ff),
                popover: hex(0x1c1f3a),
                popover_foreground: hex(0xf0f2ff),
                // ── Primary 加亮 Indigo（在深紫 surface 上更显眼）────────
                primary: hex(0x6b7ae0),
                primary_foreground: hex(0xffffff),
                // ── Secondary / Muted（紫调灰阶）─────────────────────────
                secondary: hex(0x2a2d45),
                secondary_foreground: hex(0xcfd0e3),
                muted: hex(0x2a2d45),
                muted_foreground: hex(0x9398b3),
                // ── Accent 容器 hover bg ───────────────────────────────
                accent: hex(0x3a3e60),
                accent_foreground: hex(0xf0f2ff),
                // ── Destructive 跨主题一致 ─────────────────────────────
                destructive: hex(0xe5484d),
                destructive_foreground: hex(0xffffff),
                // ── Border / Input ─────────────────────────────────────
                border: hex(0x3d4258),
                input: hex(0x14162a),
                ring: hex(0x6b7ae0),
                // ── 状态色跨主题一致 ───────────────────────────────────
                success: hex(0x4fbb72),
                warning: hex(0xe8a658),
                // ── Variant 阶梯（M15 lightness 单调）──────────────────
                primary_hover: hex(0x7d8ce8),
                primary_active: hex(0x8f9def),
                secondary_hover: hex(0x32354f),
                secondary_active: hex(0x3e4258),
                destructive_hover: hex(0xed5e63),
                destructive_active: hex(0xf27479),
                // accent_active 比 accent 更深
                accent_active: hex(0x2a2e4a),
                // Ghost button strongest
                secondary_strongest: hex(0x52566f),
                // sidebar 顶色比 background 亮 ΔL≈2
                sidebar_bg_top: hex(0x0f111d),
                // surface_workspace 与 background 同
                surface_workspace: hex(0x0c0d18),
                // M39 Phase 2: midnight aurora — 加亮 indigo + 更深紫 layer 2
                // alpha 比默认 dark 略高（深紫底比中性黑底需要更显眼的 aurora）
                aurora_a: hex(0x6b7ae0).opacity(0.20),
                aurora_b: hsla(220.0 / 360.0, 0.5, 0.45, 0.15),
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
    fn midnight_kind_is_dark_family() {
        let t = Theme::dark_midnight();
        assert_eq!(t.kind, ThemeKind::DarkMidnight);
        // is_dark() 应该把 DarkMidnight 归到 dark family，让 dark/light
        // bifurcation 的 caller 省去对 midnight 单独处理。
        assert!(t.kind.is_dark());
    }

    #[test]
    fn midnight_primary_is_indigo_brighter_than_default_dark() {
        let mid = Theme::dark_midnight();
        let dark = Theme::dark();
        // primary 是 indigo 系（hue 0.6-0.72）
        assert!(mid.colors.primary.h > 0.60 && mid.colors.primary.h < 0.72);
        // midnight primary 比 default dark primary 亮（在更深的 surface 上需要更亮）
        assert!(
            mid.colors.primary.l > dark.colors.primary.l,
            "midnight primary lightness {} should be > default dark {}",
            mid.colors.primary.l,
            dark.colors.primary.l
        );
    }

    #[test]
    fn midnight_background_is_more_purple_than_default_dark() {
        let mid = Theme::dark_midnight();
        let dark = Theme::dark();
        // midnight background 有可感知 saturation（紫调），default dark 几乎中性
        assert!(
            mid.colors.background.s > dark.colors.background.s,
            "midnight bg should have higher saturation than default dark"
        );
    }

    #[test]
    fn midnight_destructive_matches_default_dark() {
        // destructive / success / warning 跨主题一致，保持视觉锚点
        let mid = Theme::dark_midnight();
        let dark = Theme::dark();
        assert_eq!(mid.colors.destructive, dark.colors.destructive);
        assert_eq!(mid.colors.success, dark.colors.success);
        assert_eq!(mid.colors.warning, dark.colors.warning);
    }
}
