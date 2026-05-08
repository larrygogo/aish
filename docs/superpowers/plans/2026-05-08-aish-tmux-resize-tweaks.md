# tmux 缩放跳动修复 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`../specs/2026-05-08-aish-tmux-resize-tweaks-design.md`](../specs/2026-05-08-aish-tmux-resize-tweaks-design.md)

**Goal:** 把 PTY resize debounce 从 100ms 加长到 250ms，并把本地 alacritty Term 的
resize 推迟到 SIGWINCH 发出后约 80ms，消除拖窗时的字符跳动 / ANSI 错位。

**Architecture:** 单文件改动 `terminal_view.rs::check_resize`。新增两个 module-level
const 表达"debounce 时间" + "本地 Term 跟随延迟"。把 spawn 闭包从单段 timer 重排成
两段 timer + 倒过来的执行顺序：先发 SIGWINCH，等 80ms 让远端 RTT 落地，再 resize 本地
Term。

**Tech Stack:** Rust + GPUI（cx.spawn / background_executor.timer）+ tokio mpsc
（bridge.spawn）+ alacritty_terminal Term::resize。

---

## File Structure（完成时）

```
crates/aish-app/src/views/
└── terminal_view.rs          ← 改：加 2 个 const + 重排 check_resize 闭包
```

不动其它任何文件。

---

## Task 1: 调整 PTY resize 时序

> ⚠️ **原子性说明**：本 Task 的 3 个修改步骤都在同一函数内。逐步落地不会破坏编译，
> 但语义上必须**全部完成才能跑手测**——单独留下"加 const 但没用"或"只重排顺序但
> debounce 仍 100ms"都没意义。建议一气呵成。

**Files:**
- Modify: `crates/aish-app/src/views/terminal_view.rs`（顶部加 const + check_resize 闭包重排）

---

### 改动步骤

- [ ] **Step 1.1: 在 `terminal_view.rs` 顶部 use 区下方加两个 module-level const**

定位：现有第 5 行是 `use std::time::Duration;`，第 22 行是 `pub struct TerminalView {`。
中间有 `use gpui::{...}` 和 `use crate::*` 几行。在 use 区结束后、`pub struct` 前
插入：

```rust
/// PTY resize debounce 窗口期。bounds 变化后等 N ms 才发 SIGWINCH，避免拖窗
/// 时每 100ms 触发一次远端重排（旧值 100ms 偏短，250ms 让拖动稳定后才发一次）。
const PTY_RESIZE_DEBOUNCE_MS: u64 = 250;

/// 本地 alacritty Term resize 相对于 SIGWINCH 发出的额外延迟。
/// 用于粗略覆盖 SSH RTT，让远端 tmux 先按新 size 重排再让本地 Term 跟随，
/// 避免本地按新 size 排版而远端旧字节按旧 size 算 → ANSI 错位 / 字符跳动。
/// 80ms 适配常见局域网 / 国内云 RTT < 50ms；越洋高延迟仍会留 100ms+ 错位窗口
/// （后续可加可配置环境变量，本次不做）。
const LOCAL_TERM_RESIZE_DELAY_MS: u64 = 80;
```

放在 `use crate::terminal::{...};` 之后、`pub struct TerminalView {` 之前。

---

- [ ] **Step 1.2: 重排 `check_resize` 闭包内的 timer + 执行顺序**

定位：`check_resize` 函数在 `terminal_view.rs:432-492`。需要替换的代码段在
**468-490 行**（spawn 闭包整体）。

**当前代码**（行 468-490）：

```rust
        // 启动 100ms debounce task，存储在 self.pending_resize — drop 即取消
        let task = cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;

            // resize alacritty Term 并通知 UI 重绘；
            // cx 是 &mut AsyncApp，通过 cx.update 拿 &mut App 来更新 state entity
            let sender_opt = cx.update(|app| {
                state.update(app, |app_state, cx| {
                    app_state.resize_term(conn, cols, rows);
                    cx.notify();
                    app_state.sessions.get(&conn).cloned()
                })
            });

            // 通知远端 PTY 执行 window_change（SIGWINCH）
            if let Some(sender) = sender_opt {
                bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Resize { cols, rows }).await;
                });
            }
        });
```

**替换为**：

```rust
        // 启动 debounce task，存储在 self.pending_resize — drop 即取消
        //
        // 4 段流水：
        //   1. PTY_RESIZE_DEBOUNCE_MS — 等拖动稳定
        //   2. SessionCommand::Resize → 远端 PTY SIGWINCH
        //   3. LOCAL_TERM_RESIZE_DELAY_MS — 等远端 RTT 落地
        //   4. state.resize_term — 本地 alacritty Term 按新 size 排
        //
        // 顺序为何"先远端再本地"：本地 Term 立即按新 size 排会让远端用旧
        // size 算的 ANSI 在新坐标系里错位（光标定位 / pane 边框等）。让远端
        // 先按新 size 重排吐字节，本地再切到新 size，可避开错位窗口。
        let task = cx.spawn(async move |_this, cx| {
            // (1) debounce
            cx.background_executor()
                .timer(Duration::from_millis(PTY_RESIZE_DEBOUNCE_MS))
                .await;

            // (2) 先发 SIGWINCH 给远端 PTY
            let sender_opt = cx.update(|app| {
                state.update(app, |app_state, _cx| app_state.sessions.get(&conn).cloned())
            });
            if let Some(sender) = sender_opt {
                bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Resize { cols, rows }).await;
                });
            }

            // (3) 等远端约一个 RTT 让 SIGWINCH 落地、tmux 重排
            cx.background_executor()
                .timer(Duration::from_millis(LOCAL_TERM_RESIZE_DELAY_MS))
                .await;

            // (4) 本地 Term 跟随新 size 并通知 UI 重绘
            let _ = cx.update(|app| {
                state.update(app, |app_state, cx| {
                    app_state.resize_term(conn, cols, rows);
                    cx.notify();
                })
            });
        });
```

**关键差异**：
- `Duration::from_millis(100)` → `Duration::from_millis(PTY_RESIZE_DEBOUNCE_MS)`
- 新增第二个 timer `Duration::from_millis(LOCAL_TERM_RESIZE_DELAY_MS)`
- 闭包前半部分原本"resize_term + 取 sender"两件事**拆开**：先取 sender 发
  Resize，再 resize_term
- 取 sender 时不再调 `cx.notify()`（因为没改 state，第 (4) 步 resize_term 才需要 notify）

---

- [ ] **Step 1.3: 更新 `terminal_view.rs` 顶部 module-level 注释**

定位：`terminal_view.rs:1-2` 当前是：

```rust
//! 主区终端视图。M2b1 Task 4 — 自绘 alacritty grid + 颜色 + 光标闪烁。
//! M2b1 Task 5 — PTY 跟随窗口 resize（100ms debounce）。
```

把 `100ms debounce` 改成 `250ms debounce + 80ms 远端落地延迟`：

```rust
//! 主区终端视图。M2b1 Task 4 — 自绘 alacritty grid + 颜色 + 光标闪烁。
//! M2b1 Task 5 — PTY 跟随窗口 resize（250ms debounce + 80ms 远端落地延迟，
//! 见 PTY_RESIZE_DEBOUNCE_MS / LOCAL_TERM_RESIZE_DELAY_MS）。
```

---

### 编译 / 检查 / 手测 / 提交

- [ ] **Step 1.4: 跑 nightly fmt**

```bash
cargo +nightly fmt --all
```
Expected: 无输出或仅自动空白调整，exit 0。

---

- [ ] **Step 1.5: 跑 clippy（必须 0 warning）**

```bash
cargo +nightly clippy --workspace --all-targets -- -D warnings
```
Expected: `Finished` 一行，无 `warning:` 行，exit 0。

可能命中的 clippy lint：
- `clippy::redundant_closure` — 不会，新闭包结构内是 await 语句
- `unused_variables` — 不会，所有变量都被新代码用到

若任意 lint 报错，按提示修；通常无需。

---

- [ ] **Step 1.6: 跑全量测试**

```bash
cargo test --workspace
```
Expected: 199 个 test 全过（基线 — 见 `docs/superpowers/INDEX.md`），0 failed。

本 Task **不删 / 不加任何 test**：涉及 timer + 远端时序，单测无法覆盖；唯一关键
参数（两个常量）由代码注释说明依据。

---

- [ ] **Step 1.7: 手测（视觉验收）**

```bash
cargo run -p aish-app
```

验收清单（要远端 host + 装了 tmux 的环境）：

1. **拖窗时不抖**：连一个 host → tmux attach → 拖动窗口边缘改变大小。拖动**期间**
   终端不重排（因 250ms debounce 在拖动中不断被重置，不发 SIGWINCH）；松手 ~330ms
   后远端 + 本地一起按新 size 排。
2. **拖完无错位**：拖窗后远端 prompt / pane border / vim 光标位置等不再"跳到错的
   行 / 列"；屏幕内容跟手动 `clear; <重画程序>` 后一致。
3. **vim / less 验证**：远端打开 vim 大文件 → 拖窗 → 内容换行不错位、光标在正确
   行号。
4. **常规 cd / ls 不受影响**：非 tmux 的 raw shell 拖窗也应一样平滑（流程相同）。

任意一项失败：检查 const 数值是否被正确读到（grep `PTY_RESIZE_DEBOUNCE_MS` 应被
spawn 闭包内引用）；检查闭包内 4 个步骤顺序是否如 Step 1.2 写的那样。

---

- [ ] **Step 1.8: Commit**

```bash
git add crates/aish-app/src/views/terminal_view.rs \
        docs/superpowers/specs/2026-05-08-aish-tmux-resize-tweaks-design.md \
        docs/superpowers/plans/2026-05-08-aish-tmux-resize-tweaks.md

git commit -m "$(cat <<'EOF'
fix(terminal): 拖窗 resize 时序调整，消除 ANSI 错位 / 字符跳动

- PTY resize debounce 100ms → 250ms（拖动期间不发 SIGWINCH）
- 本地 alacritty Term resize 推迟 80ms 在 SIGWINCH 之后（让远端 RTT 落地、
  tmux 先按新 size 重排吐字节，本地再切到新 size 排）
- check_resize 闭包改成 4 段流水：debounce → SIGWINCH → 80ms → resize_term
- 注释更新顶部 module doc 说明新流水

无新增 test：涉及 timer + 远端时序，单测无法覆盖。常量值依据写在注释里。
spec / plan 一并落库。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: `1 file changed`（terminal_view.rs）+ 2 个新文件（spec / plan），commit hash 输出。

---

## Self-Review

**1. Spec 覆盖**：

| Spec 条目 | Plan Step |
|---|---|
| §2.1 #5 — debounce 100ms→250ms (`PTY_RESIZE_DEBOUNCE_MS`) | Step 1.1 + 1.2 |
| §2.2 #4 — 本地 Term 延后 + LOCAL_TERM_RESIZE_DELAY_MS | Step 1.1 + 1.2 |
| §2.2 闭包 4 段流水（debounce / SIGWINCH / 延迟 / resize_term） | Step 1.2 |
| §2.3 不动列表（last_pty_size / chan.window_change / make_term） | 不出现在改动中（正确——保持原样） |
| §3 时序对比 | Step 1.2 注释 |
| §5.1 自动化（fmt / clippy / test） | Step 1.4 / 1.5 / 1.6 |
| §5.2 手测（拖窗 / vim / less） | Step 1.7 |

**2. Placeholder scan**：无 TBD / "implement later" / "appropriate ..."。所有
代码段完整给出。

**3. Type / 名称一致性**：
- `PTY_RESIZE_DEBOUNCE_MS` / `LOCAL_TERM_RESIZE_DELAY_MS` 两常量名在 Step 1.1 定义、
  Step 1.2 引用，拼写一致 ✅
- `Duration::from_millis(...)` 用法跟原代码相同 ✅
- `cx.update` / `state.update` / `bridge.spawn` / `app_state.sessions.get(&conn).cloned()`
  全部沿用现有 API ✅
- 闭包 `_cx` (Step 1.2 第 (2) 步取 sender 时不需要 notify) vs `cx` (Step 1.2 第 (4)
  步 resize_term 后要 notify) — 两处刻意不同名，避免编译器对未用变量警告 ✅
