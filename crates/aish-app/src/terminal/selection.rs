//! Mouse drag → alacritty Term selection 状态机。
//!
//! - `pixel_to_grid`: 像素坐标 → (Line, Column, Side)
//! - `start_selection`: mouse_down → Selection::new
//! - `update_selection`: mouse_drag → sel.update
//! - `clear_selection`: 清除 selection
//! - `selected_text`: 取选中文本（用于 Ctrl+Shift+C）

use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::Term;
use gpui::Pixels;

use super::grid_renderer::GridLayout;

/// 把 pixel 坐标转成 (Line, Column, Side)。
///
/// Side 决定 cell 的左/右半边，影响选中范围精度。
pub fn pixel_to_grid(
    px: Pixels,
    py: Pixels,
    layout: &GridLayout,
    grid_lines: usize,
    grid_cols: usize,
) -> (Line, Column, Side) {
    let local_x = f32::from(px) - f32::from(layout.origin_x);
    let local_y = f32::from(py) - f32::from(layout.origin_y);

    let cw = f32::from(layout.cell_width);
    let ch = f32::from(layout.cell_height);

    let row = (local_y / ch)
        .floor()
        .clamp(0.0, (grid_lines as f32 - 1.0).max(0.0)) as i32;

    let col_f = (local_x / cw).clamp(0.0, grid_cols as f32);
    let col = col_f.floor().clamp(0.0, (grid_cols as f32 - 1.0).max(0.0)) as usize;

    // Side: x 落在 cell 左半边 → Left，右半边 → Right
    let frac = col_f - col_f.floor();
    let side = if frac < 0.5 { Side::Left } else { Side::Right };

    (Line(row), Column(col), side)
}

/// 开始 mouse selection（mouse_down 时调用）。
pub fn start_selection(
    term: &mut Term<crate::state::TitleListener>,
    line: Line,
    col: Column,
    side: Side,
) {
    term.selection = Some(Selection::new(
        SelectionType::Simple,
        Point::new(line, col),
        side,
    ));
}

/// 拖拽中更新选中末端（mouse_move + dragging 时调用）。
pub fn update_selection(
    term: &mut Term<crate::state::TitleListener>,
    line: Line,
    col: Column,
    side: Side,
) {
    if let Some(ref mut sel) = term.selection {
        sel.update(Point::new(line, col), side);
    }
}

/// 清除 selection。
pub fn clear_selection(term: &mut Term<crate::state::TitleListener>) {
    term.selection = None;
}

/// 取选中的文本（用于复制到剪贴板）。空选中或无 selection 返回 None。
///
/// 使用 alacritty Term::selection_to_string（0.26 已有）。
pub fn selected_text(term: &Term<crate::state::TitleListener>) -> Option<String> {
    let text = term.selection_to_string()?;
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    fn mk_layout() -> GridLayout {
        GridLayout {
            cell_width: px(10.0),
            cell_height: px(20.0),
            origin_x: px(0.0),
            origin_y: px(0.0),
        }
    }

    #[test]
    fn pixel_at_origin_is_grid_zero_zero() {
        let (line, col, side) = pixel_to_grid(px(0.0), px(0.0), &mk_layout(), 24, 80);
        assert_eq!(line, Line(0));
        assert_eq!(col, Column(0));
        assert_eq!(side, Side::Left);
    }

    #[test]
    fn pixel_at_cell_2_3_right_half() {
        // 列 3：x 范围 [30, 40)，中点 35 → Right
        let (line, col, side) = pixel_to_grid(px(35.0), px(45.0), &mk_layout(), 24, 80);
        assert_eq!(line, Line(2));
        assert_eq!(col, Column(3));
        assert_eq!(side, Side::Right);
    }

    #[test]
    fn pixel_clamp_to_grid_max() {
        let (line, col, _) = pixel_to_grid(px(10000.0), px(10000.0), &mk_layout(), 24, 80);
        assert_eq!(line, Line(23));
        assert_eq!(col, Column(79));
    }
}
