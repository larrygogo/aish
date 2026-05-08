# 删掉 ConnectionChip 横条 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`../specs/2026-05-08-aish-remove-connection-chip-design.md`](../specs/2026-05-08-aish-remove-connection-chip-design.md)

**Goal:** 移除 ConnectionChipView 横条，把 `[SSH]` 蓝色胶囊并入 tab 标题。

**Architecture:** 单 commit 的纯 UI 删除 + 元素重排。删 1 个 view 文件、改 mod
re-export、改 RootView 渲染分支、在 TabBar 渲染中插入 SSH 胶囊。无 actor / state /
SSH 层改动。

**Tech Stack:** Rust + GPUI（git dep，pin Zed main）+ aish workspace。

---

## File Structure（完成时）

```
crates/aish-app/src/
├── app.rs                                ← 改：RootView 删 connection_chip 字段 + 渲染分支简化
├── views/
│   ├── mod.rs                            ← 改：删 mod / re-export
│   ├── connection_chip.rs                ← 删除
│   └── tab_bar.rs                        ← 改：connection tab 元素链插入 SSH 胶囊
```

不动：`state.rs`、`theme.rs`、`terminal_view.rs`、`default_page.rs`、`host_form.rs`、
`session_picker.rs`。

---

## Task 1: 删除 ConnectionChip + tab 栏加 SSH 胶囊

> ⚠️ **原子性说明**：本 Task 的 5 个修改步骤**必须连续完成再编译/提交**。中间任意
> 单步落地都会破坏 import / 字段引用，工作树短暂不可编译。所有修改完成后再跑
> fmt + clippy + test + 手测，最后一次性 commit。

**Files:**
- Delete: `crates/aish-app/src/views/connection_chip.rs`
- Modify: `crates/aish-app/src/views/mod.rs`
- Modify: `crates/aish-app/src/app.rs`
- Modify: `crates/aish-app/src/views/tab_bar.rs`

---

### 改动步骤（先全做完，再编译）

- [ ] **Step 1.1: 改 `crates/aish-app/src/views/mod.rs` — 删 mod 与 re-export**

定位现有内容：
```rust
mod connection_chip;
mod default_page;
mod host_form;
...
pub use connection_chip::ConnectionChipView;
pub use default_page::DefaultPageView;
...
```

删 `mod connection_chip;`（第 5 行）和 `pub use connection_chip::ConnectionChipView;`
（第 15 行）。改后头部应为：

```rust
//! GPUI Views。

#![allow(dead_code)]

mod default_page;
mod host_form;
mod session_picker;
mod tab_bar;
mod terminal_view;
// tmux_sidebar：M3c 起废弃（功能被 SessionPickerView 弹窗取代）。模块保留备用，不再 pub use。
#[allow(dead_code)]
mod tmux_sidebar;

pub use default_page::DefaultPageView;
pub use host_form::HostFormModal;
pub use session_picker::SessionPickerView;
pub use tab_bar::TabBarView;
pub use terminal_view::TerminalView;
```

---

- [ ] **Step 1.2: 改 `crates/aish-app/src/app.rs` — 删 RootView 字段、构造、渲染分支**

定位三处修改：

**(a) 结构体字段（第 139 行）** — 删整行：
```rust
connection_chip: Entity<crate::views::ConnectionChipView>,
```

**(b) `RootView::new` 内构造（第 160-162 行）+ 字段填充（第 174 行）**

删除：
```rust
let connection_chip = cx.new(|cx| {
    crate::views::ConnectionChipView::new(state.clone(), bridge.clone(), tx.clone(), cx)
});
```
和 `Self { ... }` 字面量里的 `connection_chip,` 一行。

**(c) `RootView::render` 内 body 分支（第 191-203 行）**

替换：
```rust
// connection tab：chip 在上方占固定高度，terminal 占剩余 flex
// default tab：直接显示默认页（无 chip）
let body: gpui::AnyElement = if is_connection_tab {
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(self.connection_chip.clone())
        .child(div().flex_1().child(self.terminal.clone()))
        .into_any_element()
} else {
    self.default_page.clone().into_any_element()
};
```

为：
```rust
// connection tab：terminal 直接占满整个 body（已删 ConnectionChip 横条）
// default tab：显示默认页
let body: gpui::AnyElement = if is_connection_tab {
    self.terminal.clone().into_any_element()
} else {
    self.default_page.clone().into_any_element()
};
```

**注意**：`is_connection_tab` 变量本身保留（render 顶部 `let is_connection_tab = ...`
一行不动）。

---

- [ ] **Step 1.3: 改 `crates/aish-app/src/views/tab_bar.rs` — connection tab 元素链插入 SSH 胶囊**

定位现有 tab 元素组装（render 函数体内 `tab_items` map 闭包末尾）：

```rust
div()
    .relative()
    .flex()
    ...
    .child(prefix)
    .child(title_el)
    .child(close_btn)
    .child(bottom_line)
```

在 `prefix` / `title_el` / `close_btn` 之间增加 SSH 胶囊。在闭包内、生成 `prefix`
之后、构建外层 `div()` 之前，新增局部变量：

```rust
// connection tab 在标题与关闭键之间显示蓝色 [SSH] 胶囊（来自原 ConnectionChip）
let ssh_chip: gpui::AnyElement = if is_connection {
    div()
        .px_2p5()
        .py_0p5()
        .text_size(theme::text_xs())
        .text_color(rgb(theme::ACCENT_BLUE))
        .bg(rgb(theme::CHIP_BLUE_BG))
        .rounded_full()
        .child("SSH")
        .into_any_element()
} else {
    div().into_any_element()
};
```

然后改组装链插入位置——把 `.child(close_btn)` 前一行 `.child(title_el)` 之后插一行
`.child(ssh_chip)`：

```rust
div()
    .relative()
    .flex()
    .flex_row()
    .items_center()
    .gap_2()
    .px_4()
    .h(px(40.0))
    .text_size(theme::text_sm())
    .bg(rgb(if is_selected {
        theme::BG_BASE
    } else {
        theme::BG_ELEVATED
    }))
    .hover(|s| s.bg(rgb(theme::BG_HOVER)).cursor_pointer())
    .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
            this.handle_tab_click(id, ev.click_count, w, cx);
        }),
    )
    .child(prefix)
    .child(title_el)
    .child(ssh_chip)        // ← 新增
    .child(close_btn)
    .child(bottom_line)
```

`is_connection` 变量已经存在（render 函数 map 闭包顶部有 `let is_connection = matches!(t.content, TabContent::Connection(_));`），不需要重新定义。

---

- [ ] **Step 1.4: 删除文件 `crates/aish-app/src/views/connection_chip.rs`**

```bash
git rm crates/aish-app/src/views/connection_chip.rs
```

---

### 编译 / 检查 / 手测 / 提交

- [ ] **Step 1.5: 跑 fmt**

```bash
cargo +nightly fmt --all
```
Expected: 无输出（或仅自动格式化几个空白），exit 0。

---

- [ ] **Step 1.6: 跑 clippy（必须 0 warning）**

```bash
cargo +nightly clippy --workspace --all-targets -- -D warnings
```
Expected: `Finished` 一行，无 `warning:` 行，exit 0。

若 clippy 报"unused import"等遗漏：通常是 `tab_bar.rs` 引入 `theme::*` 已存在则不
需新增；若新增 `ssh_chip` 时引入了多余的 `use`，按提示删除即可。

---

- [ ] **Step 1.7: 跑全量测试**

```bash
cargo test --workspace
```
Expected: 所有 test 通过（基线 197 个 — 见 `docs/superpowers/INDEX.md`），不应新增失败。
本 Task 不删 / 不改任何 test 用例（`replace_current_tab_swaps_in_place` 等保持原状）。

---

- [ ] **Step 1.8: 手测（视觉验收）**

```bash
cargo run -p aish-app
```

验收清单：
1. 启动 → 默认页正常显示（无横条，与改动前一致）
2. 点击一个 host 卡片连接 → 终端区**直接占满 body 区域**，上方**没有**多余的 36px
   ConnectionChip 横条
3. tab 栏对应 connection tab 显示形如 `● teste #3 SSH ×` —— 绿点 + 标题 + 蓝色
   小胶囊 + 关闭键
4. tab 切回另一个默认页 tab → 默认页 tab 上**不显示** SSH 胶囊
5. 双击 connection tab 标题 → inline 重命名仍正常工作（胶囊不影响 hit-test）
6. 点 connection tab 上的 `×` → actor 正常断连，tab 移除（与改动前一致）

任何一项失败：回到 Step 1.1-1.3 检查；不应需要修改 actor / state 层。

---

- [ ] **Step 1.9: Commit**

```bash
git add crates/aish-app/src/views/mod.rs \
        crates/aish-app/src/app.rs \
        crates/aish-app/src/views/tab_bar.rs \
        crates/aish-app/src/views/connection_chip.rs \
        docs/superpowers/specs/2026-05-08-aish-remove-connection-chip-design.md \
        docs/superpowers/plans/2026-05-08-aish-remove-connection-chip.md

git commit -m "$(cat <<'EOF'
refactor(ui): 删除 ConnectionChip 横条，[SSH] 胶囊并入 tab

- views/connection_chip.rs 删除（信息与 tab 栏 99% 重复，▾ 折叠按钮无闭环入口）
- RootView 简化：connection tab 的 body 由 TerminalView 直接占满
- TabBarView 在 connection tab 的标题与 × 之间插入蓝色 SSH 胶囊
- spec / plan 一并落库

actor / state 层无改动。replace_current_tab 仍由 default_page.rs:46 调用
（点 host 卡片 → 当前 tab 转 connection），保持原样。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: `1 file changed` 删 + 多个 modified，提交 hash 输出。

---

- [ ] **Step 1.10: 更新 INDEX.md（任选 — 可与下次 milestone 合批）**

非必须；如果当前没有其他待索引项，可在本次一并加一行：

```markdown
### M3d-ui-iter2 — 删 ConnectionChip 横条（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-remove-connection-chip-design.md`](specs/2026-05-08-aish-remove-connection-chip-design.md)
- plan：[`plans/2026-05-08-aish-remove-connection-chip.md`](plans/2026-05-08-aish-remove-connection-chip.md)
- 关键 commits：本次 commit hash
```

如更新则一起 amend 上条 commit；不更新则跳过本步。

---

## Self-Review

1. **Spec 覆盖**：
   - Spec §2.1 删除 connection_chip → Step 1.4 ✅
   - Spec §2.1 mod 引用 → Step 1.1 ✅
   - Spec §2.2 RootView 改 → Step 1.2 ✅
   - Spec §2.3 Tab 元素新顺序 → Step 1.3 ✅
   - Spec §2.4 保留不动 → 不出现在 plan 中（正确——无需改动）✅
   - Spec §5 验证清单 → Step 1.5-1.8 ✅

2. **Placeholder scan**：无 TBD / "implement later" / "add appropriate ..."。

3. **类型一致性**：单 Task 内符号引用 — `ConnectionChipView` 删除后 mod.rs 与
   app.rs 同步移除；`is_connection` / `is_selected` / `prefix` / `title_el` /
   `close_btn` / `bottom_line` 均使用 tab_bar.rs 现有变量名。

4. **风险吸收**：
   - 工作树中间不可编译 → Step 1.5 之前不跑编译，Task 注释已标 ⚠️ 原子性
   - SSH 胶囊高度 → spec §4 实测可放下；若 `cargo run` 视觉上挤压，调 `py_0p5`→`py_0` 即可（不在本 Task 范围）
