//! Component Anatomy token — M27 设计骨架。
//!
//! 定义 Card / Dialog / List / Form / Page / Overlay 6 类组件的内部
//! padding / gap / 行高规则，让所有 view 严格对齐节奏。
//!
//! 与 Color / Radius / Spacing / FontSize / Typography 并列 Theme 第 5
//! 层 token。跨主题相同（不区分 dark/light，dev tool 紧凑风一致）。
//!
//! 关键设计：
//! - **Card**：caller 不再 各自决定 padding，组件内置默认值 + `.no_padding()` opt-out
//! - **Dialog**：body 比 Card 松一档（modal 主角感）
//! - **ListRow** 三档密度：dense (h26) / comfortable (h32) / spacious (h40)
//!   分别对应 dropdown 风 / settings 风 / host card 风
//! - **Form** 4 gap 语义化：field_gap / row_gap / inline_gap / footer_gap
//! - **Page**：outer padding + section_gap 节奏统一

use gpui::{px, Pixels};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CardAnatomy {
    pub header_px: Pixels,
    pub header_py: Pixels,
    pub body_px: Pixels,
    pub body_py: Pixels,
    pub footer_px: Pixels,
    pub footer_py: Pixels,
}

impl Default for CardAnatomy {
    fn default() -> Self {
        // spec D-2: header 16/10, body 16/12, footer 16/10
        Self {
            header_px: px(16.0),
            header_py: px(10.0),
            body_px: px(16.0),
            body_py: px(12.0),
            footer_px: px(16.0),
            footer_py: px(10.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DialogAnatomy {
    pub header_px: Pixels,
    pub header_py: Pixels,
    pub body_px: Pixels,
    pub body_py: Pixels,
    pub footer_px: Pixels,
    pub footer_py: Pixels,
}

impl Default for DialogAnatomy {
    fn default() -> Self {
        // spec D-3: header 16/12, body 16/16, footer 16/12（body 比 Card 松一档）
        Self {
            header_px: px(16.0),
            header_py: px(12.0),
            body_px: px(16.0),
            body_py: px(16.0),
            footer_px: px(16.0),
            footer_py: px(12.0),
        }
    }
}

/// List row 三档密度。caller 按场景选：
/// - dense（h26）：dropdown menu / context menu / 高密度信息列表
/// - comfortable（h32）：settings rows / 中等密度
/// - spacious（h40）：host card / 主信息列表
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ListRowAnatomy {
    pub h_dense: Pixels,
    pub h_comfortable: Pixels,
    pub h_spacious: Pixels,
    pub px_dense: Pixels,
    pub px_comfortable: Pixels,
    pub px_spacious: Pixels,
    pub gap_dense: Pixels,
    pub gap_comfortable: Pixels,
    pub gap_spacious: Pixels,
    pub py_dense: Pixels,
    pub py_comfortable: Pixels,
    pub py_spacious: Pixels,
}

impl Default for ListRowAnatomy {
    fn default() -> Self {
        // spec D-4: 三档 h 26/32/40, px 12/12/16, gap 8/8/12, py 8/10/12
        Self {
            h_dense: px(26.0),
            h_comfortable: px(32.0),
            h_spacious: px(40.0),
            px_dense: px(12.0),
            px_comfortable: px(12.0),
            px_spacious: px(16.0),
            gap_dense: px(8.0),
            gap_comfortable: px(8.0),
            gap_spacious: px(12.0),
            py_dense: px(8.0),
            py_comfortable: px(10.0),
            py_spacious: px(12.0),
        }
    }
}

/// Form 4 个 gap 语义区分（值有重复但语义不同）：
/// - field_gap: 字段块之间纵向间距（label+input 一组 → 下一组）
/// - row_gap: 同 form 内不同 row 间距（如 keyfile_row 与 password_row）
/// - inline_gap: 同一字段内 label-input / input-icon 间距
/// - footer_gap: 主体到 footer button 区间距
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FormAnatomy {
    pub field_gap: Pixels,
    pub row_gap: Pixels,
    pub inline_gap: Pixels,
    pub footer_gap: Pixels,
}

impl Default for FormAnatomy {
    fn default() -> Self {
        // spec D-5: field 12 / row 12 / inline 4 / footer 8
        Self {
            field_gap: px(12.0),
            row_gap: px(12.0),
            inline_gap: px(4.0),
            footer_gap: px(8.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PageAnatomy {
    pub outer_px: Pixels,
    pub outer_py_top: Pixels,
    pub outer_py_bottom: Pixels,
    /// M35 T5: hero / page header 用更宽松的 top padding（Linear / Vercel 风
    /// 80-120px 区间偏 lower 端）。caller 在 page top 包 hero section 时用，
    /// 替代默认 outer_py_top 给视觉呼吸感。
    pub outer_py_spacious: Pixels,
    pub section_gap: Pixels,
    pub header_to_content_gap: Pixels,
}

impl Default for PageAnatomy {
    fn default() -> Self {
        // spec D-6: outer_px 32 / outer_py 24/24 / section_gap 16 / header_to_content 16
        // M35 T5: 新加 outer_py_spacious 40px（页面 hero section 留白用）
        Self {
            outer_px: px(32.0),
            outer_py_top: px(24.0),
            outer_py_bottom: px(24.0),
            outer_py_spacious: px(40.0),
            section_gap: px(16.0),
            header_to_content_gap: px(16.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct OverlayAnatomy {
    pub toast_p: Pixels,
    pub toast_gap: Pixels,
    pub tooltip_px: Pixels,
    pub tooltip_py: Pixels,
}

impl Default for OverlayAnatomy {
    fn default() -> Self {
        // spec D-7: toast p 12 gap 12 / tooltip px 8 py 4
        Self {
            toast_p: px(12.0),
            toast_gap: px(12.0),
            tooltip_px: px(8.0),
            tooltip_py: px(4.0),
        }
    }
}

/// Theme 第 5 层 token（与 Color/Radius/Spacing/FontSize/Typography 并列）。
/// 跨主题相同，dark/light 都用 `Anatomy::default()`。
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Anatomy {
    pub card: CardAnatomy,
    pub dialog: DialogAnatomy,
    pub list_row: ListRowAnatomy,
    pub form: FormAnatomy,
    pub page: PageAnatomy,
    pub overlay: OverlayAnatomy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_card_anatomy_values() {
        let c = CardAnatomy::default();
        assert_eq!(c.header_px, px(16.0));
        assert_eq!(c.header_py, px(10.0));
        assert_eq!(c.body_px, px(16.0));
        assert_eq!(c.body_py, px(12.0));
        assert_eq!(c.footer_px, px(16.0));
        assert_eq!(c.footer_py, px(10.0));
    }

    #[test]
    fn default_dialog_anatomy_values() {
        let d = DialogAnatomy::default();
        assert_eq!(d.header_px, px(16.0));
        assert_eq!(d.header_py, px(12.0));
        assert_eq!(d.body_px, px(16.0));
        assert_eq!(d.body_py, px(16.0));
        assert_eq!(d.footer_px, px(16.0));
        assert_eq!(d.footer_py, px(12.0));
    }

    #[test]
    fn dialog_body_looser_than_card_body() {
        // spec D-3 设计契约：Dialog body padding 比 Card body 松（modal 主角感）
        let card = CardAnatomy::default();
        let dialog = DialogAnatomy::default();
        assert!(dialog.body_py > card.body_py);
    }

    #[test]
    fn default_list_row_anatomy_monotone() {
        let l = ListRowAnatomy::default();
        // h 严格单调递增
        assert!(l.h_dense < l.h_comfortable);
        assert!(l.h_comfortable < l.h_spacious);
        // py 同样单调
        assert!(l.py_dense < l.py_comfortable);
        assert!(l.py_comfortable < l.py_spacious);
        // px / gap 至少不递减（comfortable 与 dense 同值，spacious 跳一档）
        assert!(l.px_dense <= l.px_comfortable);
        assert!(l.px_comfortable < l.px_spacious);
        assert!(l.gap_dense <= l.gap_comfortable);
        assert!(l.gap_comfortable < l.gap_spacious);
    }

    #[test]
    fn default_form_anatomy_values() {
        let f = FormAnatomy::default();
        assert_eq!(f.field_gap, px(12.0));
        assert_eq!(f.row_gap, px(12.0));
        assert_eq!(f.inline_gap, px(4.0));
        assert_eq!(f.footer_gap, px(8.0));
    }

    #[test]
    fn default_page_anatomy_values() {
        let p = PageAnatomy::default();
        assert_eq!(p.outer_px, px(32.0));
        assert_eq!(p.outer_py_top, px(24.0));
        assert_eq!(p.outer_py_bottom, px(24.0));
        assert_eq!(p.outer_py_spacious, px(40.0));
        assert_eq!(p.section_gap, px(16.0));
        assert_eq!(p.header_to_content_gap, px(16.0));
    }

    /// M35 T5：spacious 必须大于普通 outer_py_top（让 hero 视觉呼吸感更宽）。
    #[test]
    fn page_spacious_is_more_than_normal_top() {
        let p = PageAnatomy::default();
        assert!(p.outer_py_spacious > p.outer_py_top);
    }

    #[test]
    fn default_overlay_anatomy_values() {
        let o = OverlayAnatomy::default();
        assert_eq!(o.toast_p, px(12.0));
        assert_eq!(o.toast_gap, px(12.0));
        assert_eq!(o.tooltip_px, px(8.0));
        assert_eq!(o.tooltip_py, px(4.0));
    }

    #[test]
    fn anatomy_default_composes_all_sub_structs() {
        let a = Anatomy::default();
        assert_eq!(a.card, CardAnatomy::default());
        assert_eq!(a.dialog, DialogAnatomy::default());
        assert_eq!(a.list_row, ListRowAnatomy::default());
        assert_eq!(a.form, FormAnatomy::default());
        assert_eq!(a.page, PageAnatomy::default());
        assert_eq!(a.overlay, OverlayAnatomy::default());
    }
}
