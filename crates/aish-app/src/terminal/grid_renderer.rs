//! 自绘 alacritty Term grid。
//!
//! 用 GPUI 低层 paint API 批量渲染 cell：
//!   1. 先用 paint_quad 画非默认背景色的矩形
//!   2. 再用 shape_line + paint 画前景字符（按行批量 shape）
//!
//! 参考 Zed `crates/terminal_view/src/terminal_element.rs` 的 layout_grid / LayoutRect / BatchedTextRun。

use alacritty_terminal::grid::Indexed;
use alacritty_terminal::index::Point;
#[cfg(test)]
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::selection::SelectionRange;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::NamedColor;
use alacritty_terminal::Term;
use gpui::{
    fill, font, point, size, App, Bounds, Font, Hsla, Pixels, SharedString, TextRun, Window,
};

use super::colors;
use super::font as terminal_font;

/// 终端 grid 布局参数（像素坐标系）。
pub struct GridLayout {
    pub cell_width: Pixels,
    pub cell_height: Pixels,
    pub origin_x: Pixels,
    pub origin_y: Pixels,
}

impl GridLayout {
    /// 将 alacritty 的 (line, column) 转换为像素坐标（左上角）。
    pub fn cell_to_pixel(&self, point: Point) -> (Pixels, Pixels) {
        (
            self.origin_x + self.cell_width * point.column.0 as f32,
            self.origin_y + self.cell_height * point.line.0 as f32,
        )
    }

    /// display_iter 里的 line 是相对 viewport 的偏移（负数表示历史，0 = 第一可见行）。
    /// 把 display line（i32）转换为像素 y 坐标。
    pub fn display_line_to_y(&self, line: i32) -> Pixels {
        self.origin_y + self.cell_height * line as f32
    }
}

/// 从 Term 中提取可见 grid 快照，避免后续 paint 时的借用冲突。
pub struct GridSnapshot {
    /// (point, cell) 按 display_iter 顺序。`point.line` 是 alacritty 内部的
    /// 绝对 line 坐标 — 滚到历史时为负数。viewport 顶行 = `Line(-display_offset)`。
    pub cells: Vec<Indexed<Cell>>,
    /// 光标位置（alacritty 绝对坐标）
    pub cursor_point: Point,
    /// 当前选中范围（grid 坐标）。None = 无选中
    pub selection_range: Option<SelectionRange>,
    /// 截快照时的 display_offset。paint 阶段用 `point.line.0 + display_offset`
    /// 把历史行的负数 line index 映射回 viewport 0..screen_lines 范围内显示。
    pub display_offset: usize,
}

impl GridSnapshot {
    pub fn from_term(term: &Term<crate::state::TitleListener>) -> Self {
        let cursor_point = term.grid().cursor.point;
        let display_offset = term.grid().display_offset();
        let cells = term
            .grid()
            .display_iter()
            .map(|indexed| Indexed {
                point: indexed.point,
                cell: indexed.cell.clone(),
            })
            .collect();
        let selection_range = term.selection.as_ref().and_then(|sel| sel.to_range(term));
        Self {
            cells,
            cursor_point,
            selection_range,
            display_offset,
        }
    }
}

/// 单行的文字渲染批次：同一前景色的相邻 cell 合并。
struct LineBatch {
    /// 起始列索引
    start_col: i32,
    /// 文字内容（包含多个字符）
    text: String,
    /// GPUI TextRun
    style: TextRun,
    /// 当前 batch 包含的 cell 数量（用于判断相邻性）
    cell_count: i32,
}

/// 获取用于 text_system 的终端字体（Normal weight + Normal style）。
fn terminal_gpui_font() -> Font {
    font(terminal_font::FONT_NAME)
}

/// 渲染 Term grid 快照到 GPUI Window。
///
/// 在 canvas 的 paint phase 调用。两遍渲染：
///   Pass 1 — 画背景色矩形（非默认背景）
///   Pass 2 — 逐行 shape + paint 文字
pub fn paint_grid(snapshot: &GridSnapshot, layout: &GridLayout, window: &mut Window, cx: &mut App) {
    let gpui_font = terminal_gpui_font();
    let font_size = gpui::px(terminal_font::FONT_SIZE);
    let line_height = layout.cell_height;
    let cell_width = layout.cell_width;
    // 当前 theme kind —— 决定 ANSI palette / fg / bg fallback。每帧从 global
    // 取，theme 切换 → cx.refresh_windows → 下帧重 paint 自动用新 palette。
    let theme_kind = aish_ui::theme(cx).kind;
    // alacritty 的 line index 是绝对坐标（看历史时为负数，viewport 顶 = -display_offset）。
    // 加上 display_offset 把 line 映射回 0..screen_lines 的 viewport 行号。
    let offset = snapshot.display_offset as i32;

    // --- Pass 1: 背景色矩形 + 选中高亮 ---
    // 选中色：dark 用 #3a3a8a α=0.8 / light 用 #bfdbfe α=0.7（colors::
    // selection_bg_for 内决策）。alpha 让底下字符仍可见。
    let selection_bg: Hsla = colors::selection_bg_for(theme_kind);

    for indexed in &snapshot.cells {
        let display_line = indexed.point.line.0 + offset;
        let col = indexed.point.column.0 as i32;
        let x = layout.origin_x + cell_width * col as f32;
        let y = layout.display_line_to_y(display_line);
        let bounds = Bounds::new(point(x, y), size(cell_width, line_height));

        let bg = indexed.cell.bg;
        // 跳过默认背景色（但选中时仍需画高亮）
        let has_custom_bg = !matches!(
            bg,
            alacritty_terminal::vte::ansi::Color::Named(NamedColor::Background)
        );
        if has_custom_bg {
            let bg_color: Hsla = colors::to_gpui(bg, false, theme_kind);
            window.paint_quad(fill(bounds, bg_color));
        }

        // 选中高亮（叠加在背景色上）
        let is_selected = snapshot
            .selection_range
            .as_ref()
            .map(|r| {
                let line = indexed.point.line;
                let col_idx = indexed.point.column;
                r.contains(alacritty_terminal::index::Point::new(line, col_idx))
            })
            .unwrap_or(false);
        if is_selected {
            window.paint_quad(fill(bounds, selection_bg));
        }
    }

    // --- Pass 2: 前景字符（按行批量 shape） ---
    let mut current_line: i32 = i32::MIN;
    let mut line_batches: Vec<LineBatch> = Vec::new();

    for indexed in &snapshot.cells {
        let c = indexed.cell.c;
        let display_line = indexed.point.line.0 + offset;

        // 换行时 flush
        if display_line != current_line {
            if !line_batches.is_empty() {
                flush_line(
                    &line_batches,
                    current_line,
                    layout,
                    font_size,
                    line_height,
                    window,
                    cx,
                );
                line_batches.clear();
            }
            current_line = display_line;
        }

        // 跳过空格和 null（默认空 cell）
        if c == ' ' || c == '\0' {
            continue;
        }

        // bold 文本：把 normal 色升级到对应 bright 色（'draw bold in bright'，
        // alacritty / iTerm2 / Windows Terminal 默认行为）。否则 Ubuntu PS1 这种
        // \033[01;32m (bold green) 显示成普通 green，在黑底上跟默认前景几乎
        // 没区别，看起来 prompt 颜色'丢了'。
        let raw_fg = if indexed.cell.flags.contains(Flags::BOLD) {
            colors::bold_promote(indexed.cell.fg)
        } else {
            indexed.cell.fg
        };
        let fg: Hsla = colors::to_gpui(raw_fg, true, theme_kind);
        let col = indexed.point.column.0 as i32;

        let cell_style = TextRun {
            len: c.len_utf8(),
            font: gpui_font.clone(),
            color: fg,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        // 尝试合并到最后一个 batch
        let merged = if let Some(last) = line_batches.last_mut() {
            if last.style.color == cell_style.color
                && last.style.font == cell_style.font
                && last.start_col + last.cell_count == col
            {
                last.text.push(c);
                last.style.len += c.len_utf8();
                last.cell_count += 1;
                true
            } else {
                false
            }
        } else {
            false
        };

        if !merged {
            line_batches.push(LineBatch {
                start_col: col,
                text: c.to_string(),
                style: cell_style,
                cell_count: 1,
            });
        }
    }

    // flush 最后一行
    if !line_batches.is_empty() {
        flush_line(
            &line_batches,
            current_line,
            layout,
            font_size,
            line_height,
            window,
            cx,
        );
    }
}

fn flush_line(
    batches: &[LineBatch],
    line: i32,
    layout: &GridLayout,
    font_size: Pixels,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    for batch in batches {
        let x = layout.origin_x + layout.cell_width * batch.start_col as f32;
        let y = layout.display_line_to_y(line);
        let origin = point(x, y);
        let text: SharedString = batch.text.clone().into();
        let shaped = window.text_system().shape_line(
            text,
            font_size,
            std::slice::from_ref(&batch.style),
            None,
        );
        let _ = shaped.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn cell_to_pixel_at_origin() {
        let layout = GridLayout {
            cell_width: px(10.0),
            cell_height: px(20.0),
            origin_x: px(0.0),
            origin_y: px(0.0),
        };
        let (x, y) = layout.cell_to_pixel(Point::new(Line(0), Column(0)));
        assert_eq!(x, px(0.0));
        assert_eq!(y, px(0.0));
    }

    #[test]
    fn cell_to_pixel_at_offset() {
        let layout = GridLayout {
            cell_width: px(10.0),
            cell_height: px(20.0),
            origin_x: px(5.0),
            origin_y: px(10.0),
        };
        let (x, y) = layout.cell_to_pixel(Point::new(Line(2), Column(3)));
        assert_eq!(x, px(5.0 + 30.0));
        assert_eq!(y, px(10.0 + 40.0));
    }
}
