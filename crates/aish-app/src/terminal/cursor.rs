//! 光标渲染：方块 + 闪烁 600ms + 失焦时空心。

use std::time::Instant;

use gpui::{fill, outline, point, size, App, BorderStyle, Bounds, Hsla, Window};

use super::colors::default_foreground_for;
use super::grid_renderer::{GridLayout, GridSnapshot};

/// 闪烁周期（前 300ms 显示，后 300ms 不显示）。
pub const BLINK_PERIOD_MS: u64 = 600;

/// 光标渲染状态。
#[derive(Clone, Copy)]
pub struct CursorState {
    pub epoch: Instant,
    pub focused: bool,
}

impl CursorState {
    pub fn new(focused: bool) -> Self {
        Self {
            epoch: Instant::now(),
            focused,
        }
    }

    /// 当前是否应该显示光标（根据闪烁相位）。
    pub fn is_visible_now(&self) -> bool {
        let elapsed_ms = self.epoch.elapsed().as_millis() as u64;
        let phase = elapsed_ms % BLINK_PERIOD_MS;
        phase < (BLINK_PERIOD_MS / 2)
    }
}

/// 在 GPUI Window 上画光标（使用 GridSnapshot 中的光标位置）。
///
/// 实心方块（聚焦）：bg = 终端默认前景色
/// 空心方块（失焦）：仅描边
pub fn paint_cursor(
    snapshot: &GridSnapshot,
    cursor_state: &CursorState,
    layout: &GridLayout,
    window: &mut Window,
    cx: &mut App,
) {
    // 失焦下静态空心，不闪烁 — 闪烁是 active 状态信号，shell 失焦了仍闪会
    // 让用户误以为终端在等待输入。仅 focused 时按 blink 相位决定是否画。
    if cursor_state.focused && !cursor_state.is_visible_now() {
        return;
    }
    let theme_kind = aish_ui::theme(cx).kind;
    let default_fg = default_foreground_for(theme_kind);

    let cursor_point = snapshot.cursor_point;
    // alacritty cursor.point 是活动屏幕坐标（0..screen_lines-1）；用户滚到历史时
    // viewport 的"虚拟原点"在 Line(-display_offset)，加 offset 后 cursor 在
    // viewport 中的行号 = cursor_line + offset。超出 0..screen_lines 时不画。
    let offset = snapshot.display_offset as i32;
    let display_line = cursor_point.line.0 + offset;
    let col = cursor_point.column.0 as i32;
    let x = layout.origin_x + layout.cell_width * col as f32;
    let y = layout.display_line_to_y(display_line);
    let bounds = Bounds::new(point(x, y), size(layout.cell_width, layout.cell_height));

    if cursor_state.focused {
        // 实心方块：用终端默认前景色
        let cursor_color: Hsla = gpui::rgb(default_fg).into();
        window.paint_quad(fill(bounds, cursor_color));
    } else {
        // 空心描边：用前景色，确保在深色背景下可见
        let cursor_color: Hsla = gpui::rgb(default_fg).into();
        window.paint_quad(outline(bounds, cursor_color, BorderStyle::default()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_visible_first_300ms() {
        let state = CursorState::new(true);
        // 刚创建时 elapsed ≈ 0，应该 visible
        assert!(state.is_visible_now());
    }

    #[test]
    fn blink_period_constant() {
        assert_eq!(BLINK_PERIOD_MS, 600);
    }

    /// 失焦时 paint_cursor 不再检查 is_visible_now → 静态画空心。
    /// 模拟 paint 决策：返回 true = 应画（focused 时按 blink，unfocused 总画）。
    fn should_paint(focused: bool, visible_now: bool) -> bool {
        // 还原 paint_cursor 入口的 guard：
        // if cursor_state.focused && !cursor_state.is_visible_now() { return; }
        // 取反后的 De Morgan 等价式（clippy::nonminimal_bool 要求最简形式）。
        !focused || visible_now
    }

    #[test]
    fn unfocused_paints_static_no_blink() {
        // 失焦下不论 blink 相位都应画（静态空心）
        assert!(should_paint(false, true));
        assert!(should_paint(false, false));
    }

    #[test]
    fn focused_respects_blink_phase() {
        // 聚焦下按 blink：visible=true 画，visible=false 不画
        assert!(should_paint(true, true));
        assert!(!should_paint(true, false));
    }
}
