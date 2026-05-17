# M36 Home Launchpad Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Home 页改 Warp 风 launchpad — active session 大卡含 shell 缩略图 + 4 phase 兜底；saved hosts grid 卡 vertical 重设计；与 sidebar M35.1 视觉同语言（inset glow hover）。

**Architecture:** 单 view 重构（home.rs 内 active/saved 两 section 重写），抽 1 个新 pure-fn 文件做 preview 提取 + format。复用现有 phase A/B/C borrow pattern，复用 M28/M35 EmptyState/ErrorState/Kbd/Spinner 组件，不动 aish-ui 通用层。

**Tech Stack:** Rust 2021 + GPUI（git dep zed） + alacritty_terminal 0.26 + tokio + cargo workspace。

**Spec：** `docs/superpowers/specs/2026-05-17-aish-m36-home-launchpad-design.md`

**Nomenclature 对齐（spec → 实际代码）：**
- spec "Online" = 实际 `ConnectionPhase::Connected`
- spec "Connecting/Reconnecting" = 实际 `ConnectionPhase::Connecting`（无 Reconnecting variant）
- spec "Disconnected" = 实际 `ConnectionPhase::Disconnected { reason: String }`
- 3 个 enum variant 映射 4 个视觉分支（Connected 拆 "有 buffer" / "空 buffer" 子分支）

---

## Tasks

### Task 0: alacritty API isolation 验证

**Files:**
- Create (temporary, not commit): `crates/aish-app/examples/preview_isolation.rs`

**Goal:** 在 isolation example 里验证 `Term<TitleListener>.grid().display_iter() / screen_lines() / columns()` 在当前 dep version (alacritty_terminal 0.26.0) 行为，避免 plan 落地时 API 不符。

- [ ] **Step 1: 在 cargo registry 翻 alacritty Term API 确认**

已 trace 完毕（writing-plans skill 阶段做过）：
- `term.grid() -> &Grid<Cell>` （`term/mod.rs:645`）
- `grid.screen_lines() / columns() -> usize` (`grid/mod.rs:528/533`)
- `grid.bottommost_line() -> Line`，`Line(i32)` newtype
- `grid[line] -> &Row<Cell>`（`grid/mod.rs:457`）
- `row[Column(col)] -> &Cell`（`row.rs:194`）
- `Cell.c: char` (`term/cell.rs:135`)
- `grid.cursor.point: Point` (内部 `cursor` 字段)

**结论**：API 与实施假设一致，无需 isolation example。**跳过本 task**，直接进 T1。

- [ ] **Step 2: Mark T0 done**

---

### Task 1: home_preview.rs — pure-fn + 单元测试（TDD）

**Files:**
- Create: `crates/aish-app/src/views/home_preview.rs`
- Modify: `crates/aish-app/src/views/mod.rs`（注册模块）

**Goal:** 抽 3 个 pure-fn 到独立文件，TDD 写 12 个单元测试，先 ship 测试不依赖 GPUI 层。

- [ ] **Step 1: 注册新模块**

Modify `crates/aish-app/src/views/mod.rs`，在其他 view mod 声明后追加：

```rust
pub mod home_preview;
```

- [ ] **Step 2: 写 home_preview.rs 框架 + 第一个测试（last_n_rows_from_chars）**

Create `crates/aish-app/src/views/home_preview.rs`:

```rust
//! Home active session 大卡的 shell preview 数据提取 + format 辅助。
//!
//! pure-fn 抽出（与 GPUI / alacritty 类型解耦），便于 cargo test 单元测试。
//! 真实 Term grid 转 Vec<Vec<char>> 在 home.rs phase A 内 inline 做（thin
//! wrapper 不测），本模块从 chars 二维数组开始 pure 操作。

use std::time::{Duration, SystemTime};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_n_rows_empty_input() {
        let rows: Vec<Vec<char>> = vec![];
        assert_eq!(last_n_rows_from_chars(rows, 6), Vec::<String>::new());
    }
}
```

- [ ] **Step 3: 跑测试确认 pass**

```bash
cargo test --package aish-app --lib views::home_preview
```
Expected: `1 passed; 0 failed`

- [ ] **Step 4: 加 last_n_rows_from_chars 其余 4 个测试**

在 `mod tests` 内追加：

```rust
#[test]
fn last_n_rows_fewer_than_n() {
    let rows = vec![
        vec!['$', ' ', 'l', 's'],
        vec!['f', 'o', 'o'],
    ];
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
```

- [ ] **Step 5: 跑测试确认 5 个 pass**

```bash
cargo test --package aish-app --lib views::home_preview
```
Expected: `5 passed; 0 failed`

- [ ] **Step 6: 加 PreviewBranch enum + preview_branch_for_phase + 4 测试**

在 home_preview.rs 顶部（在 pure-fn 前）加：

```rust
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
```

在 `mod tests` 内加 4 个测试：

```rust
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
```

- [ ] **Step 7: 跑测试确认 9 个 pass**

```bash
cargo test --package aish-app --lib views::home_preview
```
Expected: `9 passed; 0 failed`

- [ ] **Step 8: 加 format_active_duration + 3 测试**

在 home_preview.rs 加：

```rust
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
        "刚刚 active".to_string()
    } else if secs < 3600 {
        format!("{}m active", secs / 60)
    } else if secs < 86400 {
        format!("{}h active", secs / 3600)
    } else {
        format!("{}d active", secs / 86400)
    }
}
```

在 `mod tests` 内加：

```rust
#[test]
fn format_active_duration_less_than_minute() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let connected_at = SystemTime::UNIX_EPOCH + Duration::from_secs(70);
    assert_eq!(format_active_duration(connected_at, now), "刚刚 active");
}

#[test]
fn format_active_duration_minutes_hours_days() {
    let base = SystemTime::UNIX_EPOCH;
    // 5 分钟
    assert_eq!(
        format_active_duration(base, base + Duration::from_secs(5 * 60)),
        "5m active"
    );
    // 12 小时
    assert_eq!(
        format_active_duration(base, base + Duration::from_secs(12 * 3600)),
        "12h active"
    );
    // 2 天
    assert_eq!(
        format_active_duration(base, base + Duration::from_secs(2 * 86400)),
        "2d active"
    );
}

#[test]
fn format_active_duration_clock_skew_returns_zero_dur() {
    // connected_at 在未来 → duration_since 失败 → 走 Duration::ZERO 路径 → "刚刚 active"
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let connected_at = SystemTime::UNIX_EPOCH + Duration::from_secs(200);
    assert_eq!(format_active_duration(connected_at, now), "刚刚 active");
}
```

- [ ] **Step 9: 跑测试确认 12 个 pass**

```bash
cargo test --package aish-app --lib views::home_preview
```
Expected: `12 passed; 0 failed`

- [ ] **Step 10: 质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy clean / 全测试 583 pass（571 + 12）

```bash
git add crates/aish-app/src/views/home_preview.rs crates/aish-app/src/views/mod.rs
git commit -m "feat(home): M36 T1 — home_preview pure-fn + 12 单元测试

抽出 3 个 pure-fn 到 home_preview 模块：
- last_n_rows_from_chars(grid_chars, n) → 取最后 n 行 trim 转 String
- preview_branch_for_phase(...) → 4 phase 视觉分支决策
- format_active_duration(at, now) → \"5m active\" / \"12h active\" / \"2d active\"

12 个单元测试覆盖 empty / fewer / exactly / more / trim / 4 phase / 时长边界。
测试基线 571 → 583。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: home.rs Phase A — collect active_previews snapshot

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`（Phase A read app borrow scope）

**Goal:** 在 home.rs render Phase A 里 owned 出 `HashMap<ConnectionId, PreviewSnapshot>`，供 Phase B 构造大卡时用。不引入新的 borrow 冲突。

- [ ] **Step 1: 加 PreviewSnapshot struct 定义**

在 `home_preview.rs` 模块顶部（PreviewBranch 上方）加：

```rust
use std::time::SystemTime;

/// Phase A read app borrow 时 owned 出的 active session snapshot。
/// 含 phase 标识 (3 bool) + preview 6 行 + cursor 位置（窗口内 Some / 外 None）。
#[derive(Debug, Clone)]
pub struct PreviewSnapshot {
    pub phase_is_connected: bool,
    pub phase_is_connecting: bool,
    pub phase_is_disconnected: bool,
    pub disconnect_reason: Option<String>,
    pub preview: Vec<String>,
    pub cursor_in_window: Option<(usize, usize)>,
    pub opened_at: SystemTime,
}
```

- [ ] **Step 2: 在 home.rs 导入 + Phase A 内构造 snapshot**

修改 `home.rs` 文件顶部 use 块加：

```rust
use crate::views::home_preview::{
    extract_term_chars_or_empty, last_n_rows_from_chars, PreviewSnapshot,
};
use crate::state::ConnectionPhase;
use alacritty_terminal::index::{Column, Line};
```

在 home_preview.rs 加 thin wrapper（不单元测试，依赖 alacritty 类型）：

```rust
use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line},
    Term,
};
use crate::state::TitleListener;

/// 从 alacritty Term 提取 grid 全部可见行的 chars 二维数组。
/// thin wrapper，不写 unit test（依赖真实 Term）— 测试通过
/// `last_n_rows_from_chars` 覆盖 pure 逻辑。
pub fn extract_term_chars_or_empty(term: &Term<TitleListener>) -> Vec<Vec<char>> {
    let grid = term.grid();
    let cols = grid.columns();
    let screen_lines = grid.screen_lines();
    let bottom = grid.bottommost_line();
    (0..screen_lines)
        .map(|offset_from_top| {
            let line_idx = bottom - (screen_lines - 1 - offset_from_top) as i32;
            (0..cols).map(|col| grid[Line(line_idx.0)][Column(col)].c).collect()
        })
        .collect()
}
```

- [ ] **Step 3: 修改 home.rs render Phase A 收集 active_previews**

在 home.rs `fn render` 内 Phase A read app borrow scope 内（约 line 350-380 之间，紧邻收集 active connections 处），追加：

```rust
// M36 T2: collect active session preview snapshots (Phase A — owned)
let active_previews: std::collections::HashMap<ConnectionId, PreviewSnapshot> =
    app.connections
        .iter()
        .filter_map(|(id, conn)| {
            let phase = app.connection_phases.get(id).cloned()?;
            let term_opt = app.host_pty_term.get(id);

            let (phase_is_connected, phase_is_connecting, phase_is_disconnected, reason) =
                match &phase {
                    ConnectionPhase::Connected => (true, false, false, None),
                    ConnectionPhase::Connecting => (false, true, false, None),
                    ConnectionPhase::Disconnected { reason } => {
                        (false, false, true, Some(reason.clone()))
                    }
                };

            let (preview, cursor_in_window) = if let Some(term) = term_opt {
                let chars = extract_term_chars_or_empty(term);
                let rows = last_n_rows_from_chars(chars, 6);
                let cursor_pt = term.grid().cursor.point;
                // cursor 在 last 6 行窗口内才记录
                let screen_lines = term.grid().screen_lines();
                let cursor_line_from_top = cursor_pt.line.0 as usize;
                let window_start = screen_lines.saturating_sub(6);
                let cursor_in_window = if cursor_line_from_top >= window_start
                    && cursor_line_from_top < screen_lines
                {
                    Some((cursor_line_from_top - window_start, cursor_pt.column.0))
                } else {
                    None
                };
                (rows, cursor_in_window)
            } else {
                (Vec::new(), None)
            };

            Some((
                *id,
                PreviewSnapshot {
                    phase_is_connected,
                    phase_is_connecting,
                    phase_is_disconnected,
                    disconnect_reason: reason,
                    preview,
                    cursor_in_window,
                    opened_at: conn.opened_at,
                },
            ))
        })
        .collect();
```

确认 `Connection` struct 有 `connected_at: SystemTime` 字段。如缺，T2 末加。

- [ ] **Step 4: 确认 — Connection.opened_at 已存在**

state.rs:397 已有 `pub opened_at: SystemTime`（writing-plans 阶段 grep 确认）。
本步骤**无改动**，仅作 sanity check：

```bash
grep -n "opened_at" crates/aish-app/src/state.rs | head -5
```
Expected: line 397 `pub opened_at: SystemTime,` + 创建 Connection 的初始化代码

- [ ] **Step 5: 把 active_previews 加入 Phase A 末尾 tuple capture**

修改 Phase A 末尾 `(...)` capture，把 `active_previews` 加入。

- [ ] **Step 6: 跑 cargo build 验证 borrow 路径无冲突**

```bash
cargo build --package aish-app 2>&1 | tail -10
```
Expected: 编译通过（即使没用到 active_previews）

- [ ] **Step 7: 跑门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

```bash
git add crates/aish-app/src/views/home.rs crates/aish-app/src/views/home_preview.rs
git add -u  # 如改动 state.rs
git commit -m "feat(home): M36 T2 — Phase A 收集 active_previews snapshot

home.rs render Phase A 内 owned 出 HashMap<ConnectionId, PreviewSnapshot>，
含 phase 3-bool / preview 6 行 / cursor 窗口位置 / connected_at。
extract_term_chars_or_empty thin wrapper 在 home_preview.rs 内（不测试，
依赖 alacritty 类型）。pure 提取逻辑由 T1 的 12 个测试覆盖。

借用规则：snapshot owned 后 app borrow drop，phase B 用 snapshot 构造
inner，与 home.rs 现有 3-phase pattern 一致。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: home.rs active 大卡 layout — header + meta + preview 容器

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`（Phase B/C 构造 active 大卡）

**Goal:** 把当前 active session row（ListRow Entity）换成 active 大卡 layout：header + meta + preview 容器 + actions 三段。preview 容器 T3 先画"占位"，T4 接 4 phase 兜底。

- [ ] **Step 1: 删除现有 active_session_rows 路径**

home.rs 内删除 `active_session_rows: HashMap<ConnectionId, Entity<ListRow>>` 字段及其 init / retain / render 相关代码。改用新结构 `active_cards: HashMap<ConnectionId, Entity<CardEntity>>`（复用现有 CardEntity 类型，M33 stateful）。

```rust
// 替换字段：
- active_session_rows: HashMap<ConnectionId, Entity<ListRow>>,
+ active_cards: HashMap<ConnectionId, Entity<CardEntity>>,
```

- [ ] **Step 2: Phase A.5 retain + ensure active_cards entity**

参考 sidebar_nav.rs:189 retain_alive_entities pattern：

```rust
let alive_active: std::collections::HashSet<ConnectionId> =
    active_previews.keys().copied().collect();
retain_alive_entities(&mut self.active_cards, |k| alive_active.contains(k));

for id in &alive_active {
    if !self.active_cards.contains_key(id) {
        let card_id: gpui::ElementId =
            gpui::SharedString::from(format!("home-active-card-{}", id.0)).into();
        let card = cx.new(|c| {
            let mut e = CardEntity::new(card_id, c);
            e.padding(spacing.px_4, spacing.px_4)
                .radius(radius.lg);
            e
        });
        self.active_cards.insert(*id, card);
    }
}
```

- [ ] **Step 3: Phase B 构造 active 大卡 inner（header + meta + preview 占位）**

替换原 `active_rows_phase1` 构造代码：

```rust
let active_cards_phase1: Vec<(ConnectionId, gpui::AnyElement)> = active_previews
    .iter()
    .map(|(conn_id, snap)| {
        let host_label = app
            .hosts
            .iter()
            .find(|h| Some(h.id) == app.connections.get(conn_id).map(|c| c.host_id))
            .map(|h| h.label.clone())
            .unwrap_or_else(|| "unknown".into());

        // user@host:port 来自 host 配置（Connection 只存 host_id + label + opened_at），
        // 从 app.hosts 查对应 HostConfig 取 user/host/port。
        let user_at_host = app
            .connections
            .get(conn_id)
            .and_then(|c| app.hosts.iter().find(|h| h.id == c.host_id))
            .map(|h| format!("{}@{}:{}", h.user, h.host, h.port))
            .unwrap_or_default();

        let tmux_label = app
            .tmux_state
            .get(conn_id)
            .and_then(|s| s.session_name.clone());

        // phase dot 色按 phase
        let phase_dot_color = if snap.phase_is_connected {
            colors.success
        } else if snap.phase_is_connecting {
            colors.muted_foreground
        } else {
            colors.destructive
        };

        let header_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(spacing.px_2)
            .child(
                div()
                    .size(px(6.0))
                    .rounded_full()
                    .bg(phase_dot_color)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .typography(aish_ui::TypeRole::Title3, theme)
                    .child(host_label.clone()),
            )
            .when_some(tmux_label, |d, t| {
                d.child(div().text_color(colors.muted_foreground).child("·"))
                    .child(
                        div()
                            .typography(aish_ui::TypeRole::Code, theme)
                            .text_color(colors.muted_foreground)
                            .child(format!("⌧ tmux:{}", t)),
                    )
            });

        let duration_str = crate::views::home_preview::format_active_duration(
            snap.opened_at,
            SystemTime::now(),
        );
        let meta_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(spacing.px_2)
            .child(
                div()
                    .typography(aish_ui::TypeRole::Code, theme)
                    .text_color(colors.secondary_foreground)
                    .child(user_at_host),
            )
            .child(div().text_color(colors.muted_foreground).child("·"))
            .child(
                div()
                    .typography(aish_ui::TypeRole::Caption, theme)
                    .child(duration_str),
            );

        // T3 占位 preview 容器（T4 填 4 phase 兜底视觉）
        let preview_placeholder = div()
            .h(px(120.0))
            .w_full()
            .rounded(radius.md)
            .border_1()
            .border_color(colors.secondary_strongest)
            .bg(colors.background);

        let inner = div()
            .flex()
            .flex_col()
            .gap(spacing.px_3)
            .child(header_row)
            .child(meta_row)
            .child(preview_placeholder);

        (*conn_id, inner.into_any_element())
    })
    .collect();
```

- [ ] **Step 4: Phase C 应用到 CardEntity**

```rust
let active_cards: Vec<gpui::AnyElement> = active_cards_phase1
    .into_iter()
    .map(|(conn_id, body)| {
        let card_entity = self
            .active_cards
            .get(&conn_id)
            .cloned()
            .expect("active_cards ensured in Phase A.5");
        card_entity.update(cx, |c, _| {
            c.body(body);
        });
        card_entity.into_any_element()
    })
    .collect();
```

- [ ] **Step 5: Active section grid layout（2 列 ≥ 1000px）**

替换原 active section render，改 grid 2 列：

```rust
let active_section_el: Option<gpui::AnyElement> = if active_cards.is_empty() {
    None
} else {
    Some(
        div()
            .px(theme.anatomy.page.outer_px)
            .pb(spacing.px_4)
            .flex()
            .flex_col()
            .gap(spacing.px_3)
            .when_some(active_section_label, |d, l| d.child(l))
            .child(
                div()
                    .grid()
                    .grid_cols(2)  // T9 加响应式
                    .gap(spacing.px_3)
                    .children(active_cards),
            )
            .into_any_element(),
    )
};
```

注：如 GPUI 无 `.grid().grid_cols()` API，fallback 用 `.flex().flex_wrap()` + 每 card 设 `flex_basis` 50%。spec §6 调研未覆盖此点，T3 实施时验证。

- [ ] **Step 6: 跑门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 7: manual 验收**

启动 app，连一个 SSH host，确认 active 区显示大卡（header + meta + placeholder 灰框）。

- [ ] **Step 8: Commit**

```bash
git add crates/aish-app/src/views/home.rs
git commit -m "feat(home): M36 T3 — active 大卡 layout（header + meta + preview 占位）

替换 active_session_rows (ListRow) 为 active_cards (CardEntity)。
- header: ● phase dot + host label (Title3) + tmux chip (when_some)
- meta: user@host:port (Code dim) + 存活时长 (Caption)
- preview 容器: T3 占位灰框（T4 填 4 phase 兜底）

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: home.rs active 大卡 — 4 phase 兜底视觉

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`（替换 T3 preview_placeholder 占位）

**Goal:** preview 容器按 PreviewBranch 4 分支渲染：ShowCells / WaitingForOutput / Loading / DisconnectedHint。

- [ ] **Step 1: 在 home.rs Phase B 内 build preview content（替换 preview_placeholder）**

替换 T3 步骤 3 内的 `preview_placeholder` 为：

```rust
use crate::views::home_preview::{preview_branch_for_phase, PreviewBranch};

let preview_empty = snap.preview.iter().all(|line| line.is_empty());
let branch = preview_branch_for_phase(
    snap.phase_is_connected,
    snap.phase_is_connecting,
    snap.phase_is_disconnected,
    preview_empty,
);

let preview_inner: gpui::AnyElement = match branch {
    PreviewBranch::ShowCells => {
        // 渲染 6 行 cells，dim 统一色 (v1)
        let lines = snap.preview.clone();
        let cursor = snap.cursor_in_window;
        div()
            .flex()
            .flex_col()
            .px(spacing.px_2)
            .py(spacing.px_2)
            .children(lines.into_iter().enumerate().map(|(row_idx, line)| {
                let line_with_cursor = if cursor.map(|(r, _)| r) == Some(row_idx) {
                    format!("{}█", line)
                } else {
                    line
                };
                div()
                    .text_size(px(10.0))
                    .font_family("JetBrains Mono")
                    .text_color(colors.muted_foreground)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(line_with_cursor)
                    .into_any_element()
            }))
            .into_any_element()
    }
    PreviewBranch::WaitingForOutput => div()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .typography(aish_ui::TypeRole::Caption, theme)
                .child("等待输出..."),
        )
        .into_any_element(),
    PreviewBranch::Loading => div()
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(spacing.px_2)
        .child(aish_ui::icon(aish_ui::IconName::Loader).size(px(16.0)).text_color(colors.muted_foreground))
        .child(
            div()
                .typography(aish_ui::TypeRole::Caption, theme)
                .child("Connecting..."),
        )
        .into_any_element(),
    PreviewBranch::DisconnectedHint => div()
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(spacing.px_2)
        .child(aish_ui::icon(aish_ui::IconName::AlertTriangle).size(px(16.0)).text_color(colors.destructive))
        .child(
            div()
                .typography(aish_ui::TypeRole::Caption, theme)
                .text_color(colors.destructive)
                .child("Disconnected · 点击重连"),
        )
        .into_any_element(),
};

let preview_container = div()
    .h(px(120.0))
    .w_full()
    .rounded(radius.md)
    .border_1()
    .border_color(colors.secondary_strongest)
    .bg(if snap.phase_is_disconnected {
        colors.destructive.opacity(0.05)
    } else {
        colors.background
    })
    .child(preview_inner);
```

替换 T3 内 `.child(preview_placeholder)` 为 `.child(preview_container)`。

- [ ] **Step 2: 跑门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: manual 验收 — 切换 4 phase**

1. 启动 app，进 host_form 加一个故意连不上的 host → 看 Connecting 分支
2. 连一个真 host，连上但不输任何 → 看 WaitingForOutput 分支
3. 输 `ls` 看 ShowCells 分支
4. kill 远端 sshd → 看 DisconnectedHint 分支

- [ ] **Step 4: Commit**

```bash
git add crates/aish-app/src/views/home.rs
git commit -m "feat(home): M36 T4 — active 大卡 4 phase 兜底视觉

preview 容器按 PreviewBranch 4 分支渲染：
- ShowCells: 6 行 10px Code dim 统一色 + cursor █（v1 不保 ANSI）
- WaitingForOutput: 中央 Caption \"等待输出...\"
- Loading: spinner icon + \"Connecting...\"
- DisconnectedHint: ⚠ + \"Disconnected · 点击重连\"，bg 5% 红 tint

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: home.rs active 大卡 — Attach button + 整卡 click

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`

**Goal:** 在 active 大卡底部右对齐 `[Attach ↵]` Button primary。整卡 click 等价 Attach。Disconnected 卡 click 触发重连而非 attach。

- [ ] **Step 1: 在 HomeView 加 attach_buttons HashMap**

```rust
attach_buttons: HashMap<ConnectionId, Entity<Button>>,
```

init 时 `HashMap::new()`。

- [ ] **Step 2: Phase A.5 retain + ensure attach_buttons**

```rust
retain_alive_entities(&mut self.attach_buttons, |k| alive_active.contains(k));

for id in &alive_active {
    if !self.attach_buttons.contains_key(id) {
        let id_copy = *id;
        let weak = cx.weak_entity();
        let btn = cx.new(|c| {
            let mut b = Button::new(format!("home-attach-{}", id_copy.0), c);
            b.label("Attach")  // Kbd ↵ 在 T5 step 4 加
                .primary()
                .on_click(move |_ev, _w, cx| {
                    if let Some(this) = weak.upgrade() {
                        this.update(cx, |this, cx| this.handle_attach_click(id_copy, cx));
                    }
                });
            b
        });
        self.attach_buttons.insert(*id, btn);
    }
}
```

- [ ] **Step 3: 加 handle_attach_click + handle_reconnect_click 方法**

```rust
fn handle_attach_click(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
    // 切到 Terminal tab 并 select 这个 conn
    self.state.update(cx, |s, cx| {
        let tab_id = s.tabs.iter().find(|t| match t.content {
            TabContent::Connection(c) => c == conn_id,
            _ => false,
        }).map(|t| t.id);
        if let Some(tid) = tab_id {
            s.selected_tab = Some(tid);
            s.sidebar = SidebarTab::Terminal;
            cx.notify();
        }
    });
}

fn handle_reconnect_click(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
    // 顺序（按 state.rs:755-768 doc 要求）：先 spawn 新 actor 拿 sender，再
    // 调 reopen_connection(conn_id, sender) 注册并把 phase 复位 Connecting。
    let config = self.state.read(cx)
        .connections.get(&conn_id)
        .and_then(|c| self.state.read(cx).hosts.iter().find(|h| h.id == c.host_id).cloned());
    let Some(config) = config else { return; };
    let sender = self.bridge.spawn_session(conn_id, config, self.tx.clone());
    self.state.update(cx, |s, cx| {
        s.reopen_connection(conn_id, sender);
        cx.notify();
    });
}
```

`reopen_connection` 签名（state.rs:758）：
```rust
pub fn reopen_connection(&mut self, id: ConnectionId, sender: mpsc::Sender<SessionCommand>) -> Option<HostId>
```
内部自动注册 sender + tmux_state clear + phase = Connecting。

- [ ] **Step 4: Phase B 构造 actions row + 整卡 click handler**

替换 T3 inner div 末尾，加 actions row 和卡片 click：

```rust
let actions_row = div()
    .flex()
    .flex_row()
    .justify_end()
    .child({
        let btn = self.attach_buttons.get(conn_id).cloned().expect("ensured");
        btn.into_any_element()
    });

let inner = div()
    .flex()
    .flex_col()
    .gap(spacing.px_3)
    .child(header_row)
    .child(meta_row)
    .child(preview_container)
    .child(actions_row);

// 整卡 click handler — Disconnected 走 reconnect，其他走 attach
let conn_id_copy = *conn_id;
let weak_card = cx.weak_entity();
let is_disconnected = snap.phase_is_disconnected;
let card_click_listener = cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
    if is_disconnected {
        this.handle_reconnect_click(conn_id_copy, cx);
    } else {
        this.handle_attach_click(conn_id_copy, cx);
    }
});

// 灌到 CardEntity 时同时挂 click handler
card_entity.update(cx, |c, _| {
    c.body(inner)
        .on_click(...);  // CardEntity 的 click handler 接入
});
```

确认 `CardEntity` 是否支持 `.on_click()`。如不支持，在外层 div wrap 一层 `.on_mouse_down()`。

- [ ] **Step 5: Kbd ↵ inline 在 Attach button**

Button label 改成含 Kbd chip。如果 Button 不支持复合 label，改用 IconButton + 自定义 label div。或：

```rust
b.label_with_kbd("Attach", aish_ui::Kbd::new("↵"))
```

如 Button 无此 API，T5 step 5 跳过 Kbd chip（label "Attach" 简化版），加入 backlog 后续 polish。

- [ ] **Step 6: 跑门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 7: manual 验收**

- 连一个 host，点 active 卡片 → 切到 Terminal tab
- 点 Attach 按钮 → 同上
- kill 远端 sshd 触发 Disconnected → 点卡片 → 重连

- [ ] **Step 8: Commit**

```bash
git add crates/aish-app/src/views/home.rs
git commit -m "feat(home): M36 T5 — active 大卡 Attach button + 整卡 click

- attach_buttons HashMap<ConnectionId, Entity<Button>> 替换原 session_open_buttons
- Phase A.5 retain + ensure，weak.upgrade callback 接 handle_attach_click
- 整卡 click handler 按 phase 分流：
  - Connected/Connecting → attach (切 Terminal tab + select)
  - Disconnected → reopen_connection + Bridge.spawn_session

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: home.rs saved 卡 vertical layout 重设计

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`（Phase B 内 hosts cards build）

**Goal:** saved 卡从 horizontal layout (现状 avatar 40 + 3 行 text + actions + chevron) 改 vertical (avatar top + name + connection + time + 活跃 chip + edit/delete 右下角)。

- [ ] **Step 1: Phase A 内 owned active_host_ids HashSet**

在 home.rs Phase A read app borrow scope 内（紧邻 active_previews 构造处）追加：

```rust
let active_host_ids: std::collections::HashSet<HostId> =
    app.connections.values().map(|c| c.host_id).collect();
```

加入 Phase A 末尾 tuple capture（与 active_previews 一起）。

- [ ] **Step 2: 替换 host card body_row（home.rs:746-799）**

替换原 `body_row` 构造代码，用 step 1 的 `active_host_ids`：

```rust
let active_for_this_host = active_host_ids.contains(&id);

let body_col = div()
    .flex()
    .flex_col()
    .gap(spacing.px_2)
    .px(spacing.px_3)
    .py(spacing.px_3)
    .child(
        // avatar top-left
        div()
            .child(avatar),
    )
    .child(
        // 3 行 text stack
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .typography(aish_ui::TypeRole::Title3, theme)
                    .child(label),
            )
            .child(
                div()
                    .typography(aish_ui::TypeRole::Code, theme)
                    .text_color(colors.secondary_foreground)
                    .child(host_text),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(spacing.px_1)
                    .child(
                        div()
                            .typography(aish_ui::TypeRole::Caption, theme)
                            .child(match last_conn_str {
                                Some(s) => format!("{}", s),
                                None => "未连接".to_string(),
                            }),
                    )
                    .when(active_for_this_host, |d| {
                        d.child(div().text_color(colors.muted_foreground).child("·"))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(colors.success)
                                    .child("● 活跃"),
                            )
                    }),
            ),
    );
```

- [ ] **Step 3: edit/delete IconButton 右下角 absolute**

```rust
let actions_overlay = div()
    .absolute()
    .bottom_2()
    .right_2()
    .flex()
    .flex_row()
    .gap_1()
    .opacity(0.0)
    .group_hover(group_name.clone(), |s| s.opacity(1.0))
    .child(edit_btn)
    .child(delete_btn);

let card_outer = div()
    .relative()
    .group(group_name)
    .child(body_col)
    .child(actions_overlay);
```

- [ ] **Step 4: 跑门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 5: manual 验收**

- saved 卡 vertical layout 显示
- hover 卡片 edit/delete 出现在右下角
- 有 active connection 的 host 卡片 time 行尾追 "· ● 活跃" chip

step 编号修正：本 task 实际是 step 1 (active_host_ids) → 2 (body_col) → 3 (overlay) → 4 (门禁) → 5 (验收) → 6 (commit)。

- [ ] **Step 6: Commit**

```bash
git add crates/aish-app/src/views/home.rs
git commit -m "feat(home): M36 T6 — saved 卡 vertical layout 重设计

- horizontal (avatar + 3 行 + actions + chevron) → vertical (avatar top
  + name + connection + time + active chip)
- edit/delete IconButton 移右下角 absolute (group_hover 显形)
- 删除 chevron（vertical 看不到右侧，由整卡 click 代替）
- 有 active connection 的 host time 行尾追 \"· ● 活跃\" chip (弱视觉)

active_host_ids HashSet Phase A 内 owned，Phase B contains 判 active chip。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: home.rs 卡片 hover state 统一 inset glow

**Files:**
- Modify: `crates/aish-app/src/views/home.rs` 或 `crates/aish-ui/src/components/card.rs`

**Goal:** active 大卡 + saved 卡 hover state 视觉与 M35.1 D5 sidebar 同语言 — primary 5% bg blend + primary 25% border。

- [ ] **Step 1: 检查 CardEntity 现有 hover state**

```bash
grep -n "fn hover\|secondary_hover\|hover_bg" crates/aish-ui/src/components/card.rs | head -10
```

如 CardEntity 已支持 `.hover_bg(color)` / `.hover_border(color)` API，T7 在 home.rs 调用层覆盖即可。如无，需在 card.rs 内加配置。

- [ ] **Step 2 (情况 A — Card 已支持配置)：home.rs 内覆盖 hover**

```rust
let mut e = CardEntity::new(card_id, c);
e.padding(spacing.px_4, spacing.px_4)
    .radius(radius.lg)
    .hover_bg_blend(colors.primary, 0.05)        // 假设有此 API
    .hover_border_color(colors.primary.opacity(0.25));
```

- [ ] **Step 2 (情况 B — Card 不支持)：在 card.rs 加 hover_glow 配置**

在 `card.rs` 加 builder:

```rust
pub fn hover_glow(&mut self, primary: Hsla) -> &mut Self {
    self.hover_bg = Some(primary.opacity(0.05));
    self.hover_border = Some(primary.opacity(0.25));
    self
}
```

并在 Render 内的 hover transition 路径用 `self.hover_bg` / `self.hover_border` 而非 hardcoded `secondary_hover`。

- [ ] **Step 3: home.rs active + saved 卡都接 hover_glow**

```rust
// active 大卡
e.padding(spacing.px_4, spacing.px_4)
    .radius(radius.lg)
    .hover_glow(colors.primary);

// saved 卡
e.padding(spacing.px_3, spacing.px_3)
    .radius(radius.md)
    .hover_glow(colors.primary);
```

- [ ] **Step 4: 跑门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 5: manual 验收**

hover active / saved 卡 → primary tint glow + 紫色细边，与 sidebar NavItem active 视觉一致。

- [ ] **Step 6: Commit**

```bash
git add crates/aish-app/src/views/home.rs
git add -u  # 如改 card.rs
git commit -m "feat(home): M36 T7 — 卡片 hover state 统一 inset glow

active 大卡 + saved 卡 hover 视觉对齐 M35.1 D5 sidebar inset glow：
- bg: card + primary.opacity(0.05) blend
- border: primary.opacity(0.25) 1px

[如需改 card.rs] CardEntity 加 hover_glow(primary) builder API。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: empty / error states 验证

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`（确认 EmptyState / ErrorState 路径走通）

**Goal:** 5 个 empty/error 场景视觉验收（spec §7.1）。本 task 不改 EmptyState / ErrorState 组件本身。

- [ ] **Step 1: manual 验收 — 5 个状态**

1. 删空 `~/.config/aish/hosts.json` → 重启 → 看 hero EmptyState 占满 page
2. 留 1 个 saved 但不连 → active 区整段隐藏（无 separator），saved grid 正常
3. 不可能场景（active 无 saved）— 跳过
4. corrupt hosts.json（破坏 JSON 格式）→ 看 ErrorState retry_btn
5. 连一个 host 然后 kill 远端 sshd → active 大卡内 DisconnectedHint 分支（T4 已实施）

- [ ] **Step 2: 修复任何不达预期的视觉（如有）**

如某状态显示异常，回到对应 task 修，否则跳过。

- [ ] **Step 3: Commit（如本 task 有 fix；否则跳过 commit）**

```bash
git commit -m "fix(home): M36 T8 — empty/error states 视觉验收 fix [若有]"
```

---

### Task 9: 性能 baseline 实测（spike — 可能不出代码）

**性质**：此 task 为 spike，**仅产出决策**。如实测达标则跳过代码改动；如不达标，
GPUI throttle 路径需先调研（writing-plans 阶段未确定具体 GPUI API）—
不达标场景下另开 M36.1 follow-up plan 实施 throttle，**本 plan 不强行实施**。

**Files:**
- 无（无改动情况下；如开 M36.1 follow-up 则另议）

**Goal:** 实测 5+ active session 时 home rerender 性能，给 throttle 决策。

- [ ] **Step 1: 开 5 个 active SSH session**

启 app，连 5 个 host（或同一 host 开 5 个 connection），每个执行
`top -b -n 0 > /dev/null` 持续输出。

- [ ] **Step 2: 实测帧率 / CPU**

观察：
- 主观视觉是否卡顿
- 终端 `top` 看 aish 进程 CPU %
- GPUI 若有 frame counter 工具开起来（spec 阶段未调研，本步可跳）

- [ ] **Step 3: 记录决策到 INDEX.md M36 段 lessons**

| 实测结果 | 决策 | INDEX 记录 |
|---|---|---|
| CPU < 30% / 主观流畅 | 不加 throttle | "5 active session 实测 CPU N%，不加 throttle 直接 ship" |
| CPU ≥ 30% / 主观卡 | 开 M36.1 follow-up plan | "M36 v1 性能不达标（CPU N%），M36.1 加 throttle (spike 调研 GPUI 节流 API 路径)" |

T9 本身**不写代码**。决策结果在 T10 INDEX 收尾时一起记录。

---

### Task 10: INDEX.md 收尾

**Files:**
- Modify: `docs/superpowers/INDEX.md`

**Goal:** 在 `### M35.1 follow-up` 之后追加 `### M36 Home Launchpad` 段，记录 commits / lessons / 测试基线变化。

- [ ] **Step 1: 在 INDEX.md M35.1 follow-up 子条目下方追加新 ### M36 段**

定位 INDEX.md 的 M35 节末尾（M35.1 follow-up 子条目之后、下一个 `### hover leave fade-out` 段之前），插入：

```markdown
### M36 — Home Launchpad（信息架构重设计）（2026-05-17）— ✅ 已完成

- 范围：Warp 风 launchpad — active session 大卡含 shell 缩略图 + 4 phase 兜底；
  saved hosts grid 卡 vertical 重设计；与 sidebar M35.1 视觉同语言
- ~430 行净变化、3 文件主要改动、~12 新 pure-fn 测试
- 关键 commits：[填实际 sha]
- Lessons:
  - [实施时收录的具体 lesson]
- Spec：[`specs/2026-05-17-aish-m36-home-launchpad-design.md`](specs/2026-05-17-aish-m36-home-launchpad-design.md)
- Plan：[`plans/2026-05-17-aish-m36-home-launchpad.md`](plans/2026-05-17-aish-m36-home-launchpad.md)
- 测试基线：571 → 583（+12 home_preview pure-fn 测试）
```

实际 sha 在 T10 实施时 `git log --oneline -15` 取。

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(INDEX): M36 Home Launchpad 收尾

记录 M36 commits + lessons + 测试基线 571 → 583。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Quality Gates（每 task 完成后）

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

任一失败必须修后才能 commit。

## Acceptance

- [ ] T1-T10 全部 commit（T0 跳过，T8/T9 条件 commit）
- [ ] 用户视觉验收通过（重点：active 大卡 4 phase 切换流畅 / saved 卡 vertical 美观）
- [ ] INDEX.md 记录 commits + lessons
- [ ] 测试 571 → ~583 全过
- [ ] clippy clean

## Risk & Rollback

每 task 独立 commit，可针对性 revert：
- 用户嫌大卡太挤 → revert T3/T4/T5
- 用户嫌 saved vertical 不好看 → revert T6
- 用户嫌 hover glow 抢眼 → revert T7

## Co-Authored-By

每 commit 末尾：
```
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```
