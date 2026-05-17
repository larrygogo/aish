//! Home active session 大卡的 shell preview 数据提取 + format 辅助。
//!
//! pure-fn 抽出（与 GPUI / alacritty 类型解耦），便于 cargo test 单元测试。
//! 真实 Term grid 转 Vec<Vec<char>> 在 home.rs phase A 内 inline 做（thin
//! wrapper 不测），本模块从 chars 二维数组开始 pure 操作。

use std::time::{Duration, SystemTime};

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line},
    Term,
};

use crate::state::TitleListener;

/// Phase A read app borrow 时 owned 出的 active session snapshot。
/// 含 phase 标识 (3 bool) + preview 6 行 + cursor 位置（窗口内 Some / 外 None）。
#[derive(Debug, Clone)]
pub struct PreviewSnapshot {
    pub phase_is_connected: bool,
    pub phase_is_connecting: bool,
    pub phase_is_disconnected: bool,
    /// 断开原因（T4 当前仅显示 "Disconnected · 点击重连" 固定文本，
    /// 保留字段供未来扩展为 "Disconnected: connection reset · 点击重连"）。
    #[allow(dead_code)]
    pub disconnect_reason: Option<String>,
    pub preview: Vec<String>,
    pub cursor_in_window: Option<(usize, usize)>,
    pub opened_at: SystemTime,
}

/// 从 alacritty Term 提取 grid 全部可见行的 chars 二维数组。
///
/// thin wrapper，不写 unit test（依赖真实 Term）— 测试通过
/// [`last_n_rows_from_chars`] 覆盖 pure 逻辑。
pub fn extract_term_chars_or_empty(term: &Term<TitleListener>) -> Vec<Vec<char>> {
    let grid = term.grid();
    let cols = grid.columns();
    let screen_lines = grid.screen_lines();
    let bottom = grid.bottommost_line();
    (0..screen_lines)
        .map(|offset_from_top| {
            // line_idx = bottom - (screen_lines - 1 - offset_from_top)
            // 等价于 from-top 顺序：bottom-(n-1), bottom-(n-2), ..., bottom
            let line_idx_i32 = bottom.0 - (screen_lines as i32 - 1 - offset_from_top as i32);
            (0..cols)
                .map(|col| grid[Line(line_idx_i32)][Column(col)].c)
                .collect()
        })
        .collect()
}

/// 4 phase 兜底的视觉分支（spec §4.3）。
///
/// 3 个 `ConnectionPhase` enum variant 映射 4 个视觉分支：
/// Connected 按 preview_empty 拆 ShowCells / WaitingForOutput。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewBranch {
    /// Online + cell buffer 非空 → 渲染实际 cells
    ShowCells,
    /// Online + cell buffer 空 → "等待输出..."
    WaitingForOutput,
    /// Connecting → spinner + "Connecting..."
    Loading,
    /// Disconnected{reason} → ⚠ + "Disconnected · 点击重连"
    DisconnectedHint,
}

/// 从 grid chars 二维数组取最后 n 行，每行 trim trailing whitespace 转 String。
///
/// 输入 rows 个数 < n 时返回所有行；> n 时取最后 n 行。
pub fn last_n_rows_from_chars(grid_chars: Vec<Vec<char>>, n: usize) -> Vec<String> {
    let total = grid_chars.len();
    let skip = total.saturating_sub(n);
    grid_chars
        .into_iter()
        .skip(skip)
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect()
}

/// 根据 phase 和 preview 是否空决定视觉分支。
///
/// 输入 `phase_is_connected` / `phase_is_connecting` / `phase_is_disconnected`
/// 三个 bool（caller 从 `ConnectionPhase` 匹配出），避免本模块依赖
/// `state::ConnectionPhase` 类型。`preview_empty` 仅 Connected 时被检查。
pub fn preview_branch_for_phase(
    phase_is_connected: bool,
    phase_is_connecting: bool,
    phase_is_disconnected: bool,
    preview_empty: bool,
) -> PreviewBranch {
    if phase_is_disconnected {
        PreviewBranch::DisconnectedHint
    } else if phase_is_connecting {
        PreviewBranch::Loading
    } else if phase_is_connected {
        if preview_empty {
            PreviewBranch::WaitingForOutput
        } else {
            PreviewBranch::ShowCells
        }
    } else {
        // 不可达分支（3 phase 互斥），fallback 走 Loading 视觉
        PreviewBranch::Loading
    }
}

/// 格式化连接存活时长：
/// - < 1 分钟 → "刚刚 active"
/// - < 60 分钟 → "{N}m active"
/// - < 24 小时 → "{N}h active"
/// - ≥ 24 小时 → "{N}d active"
///
/// 与 M22 humanize_last_connected 语义不同：那个是"上次连接时间"（过去
/// 完成态），本函数是"当前 session 已活了多久"（进行时）。
pub fn format_active_duration(connected_at: SystemTime, now: SystemTime) -> String {
    let elapsed = now.duration_since(connected_at).unwrap_or(Duration::ZERO);
    let secs = elapsed.as_secs();
    if secs < 60 {
        "刚刚连接".to_string()
    } else if secs < 3600 {
        format!("已连接 {} 分钟", secs / 60)
    } else if secs < 86400 {
        format!("已连接 {} 小时", secs / 3600)
    } else {
        format!("已连接 {} 天", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- last_n_rows_from_chars ----------

    #[test]
    fn last_n_rows_empty_input() {
        let rows: Vec<Vec<char>> = vec![];
        assert_eq!(last_n_rows_from_chars(rows, 6), Vec::<String>::new());
    }

    #[test]
    fn last_n_rows_fewer_than_n() {
        let rows = vec![vec!['$', ' ', 'l', 's'], vec!['f', 'o', 'o']];
        assert_eq!(
            last_n_rows_from_chars(rows, 6),
            vec!["$ ls".to_string(), "foo".to_string()]
        );
    }

    #[test]
    fn last_n_rows_exactly_n() {
        let rows: Vec<Vec<char>> = (0..6).map(|i| vec![char::from(b'0' + i)]).collect();
        let result = last_n_rows_from_chars(rows, 6);
        assert_eq!(result.len(), 6);
        assert_eq!(result[0], "0");
        assert_eq!(result[5], "5");
    }

    #[test]
    fn last_n_rows_more_than_n_takes_last() {
        let rows: Vec<Vec<char>> = (0..10).map(|i| vec![char::from(b'a' + i)]).collect();
        let result = last_n_rows_from_chars(rows, 6);
        assert_eq!(result.len(), 6);
        assert_eq!(result[0], "e"); // 取 last 6: e f g h i j
        assert_eq!(result[5], "j");
    }

    #[test]
    fn last_n_rows_trim_trailing_whitespace() {
        let rows = vec![vec!['$', ' ', 'l', 's', ' ', ' ', ' ', ' ']];
        assert_eq!(last_n_rows_from_chars(rows, 6), vec!["$ ls".to_string()]);
    }

    // ---------- preview_branch_for_phase ----------

    #[test]
    fn preview_branch_connected_with_content() {
        assert_eq!(
            preview_branch_for_phase(true, false, false, false),
            PreviewBranch::ShowCells
        );
    }

    #[test]
    fn preview_branch_connected_empty() {
        assert_eq!(
            preview_branch_for_phase(true, false, false, true),
            PreviewBranch::WaitingForOutput
        );
    }

    #[test]
    fn preview_branch_connecting() {
        // preview_empty 不影响 Connecting 分支
        assert_eq!(
            preview_branch_for_phase(false, true, false, false),
            PreviewBranch::Loading
        );
        assert_eq!(
            preview_branch_for_phase(false, true, false, true),
            PreviewBranch::Loading
        );
    }

    #[test]
    fn preview_branch_disconnected() {
        assert_eq!(
            preview_branch_for_phase(false, false, true, false),
            PreviewBranch::DisconnectedHint
        );
    }

    // ---------- format_active_duration ----------

    #[test]
    fn format_active_duration_less_than_minute() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let connected_at = SystemTime::UNIX_EPOCH + Duration::from_secs(70);
        assert_eq!(format_active_duration(connected_at, now), "刚刚连接");
    }

    #[test]
    fn format_active_duration_minutes_hours_days() {
        let base = SystemTime::UNIX_EPOCH;
        // 5 分钟
        assert_eq!(
            format_active_duration(base, base + Duration::from_secs(5 * 60)),
            "已连接 5 分钟"
        );
        // 12 小时
        assert_eq!(
            format_active_duration(base, base + Duration::from_secs(12 * 3600)),
            "已连接 12 小时"
        );
        // 2 天
        assert_eq!(
            format_active_duration(base, base + Duration::from_secs(2 * 86400)),
            "已连接 2 天"
        );
    }

    #[test]
    fn format_active_duration_clock_skew_returns_zero_dur() {
        // connected_at 在未来 → duration_since 失败 → 走 Duration::ZERO 路径 → "刚刚 active"
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let connected_at = SystemTime::UNIX_EPOCH + Duration::from_secs(200);
        assert_eq!(format_active_duration(connected_at, now), "刚刚连接");
    }
}
