# aish UI 美化 — Implementation Plan

**Spec:** [`../specs/2026-05-08-aish-ui-polish-design.md`](../specs/2026-05-08-aish-ui-polish-design.md)

**Goal:** 按 spec 把 4 个区域调到接近移动端参考图视觉质量。预计 5 个 commit。

**前置:** 已合并到本分支 `fix/tmux-client-size-follow-window-20260507-zj` 的 7 个
feature；切换到 superpowers 工作流（commit `28e826e`）。

---

## File Structure（完成时）

```
crates/aish-app/src/
├── theme.rs                              ← NEW：色值 / 字号 / 半径常量
├── views/
│   ├── default_page.rs                   ← 改：用 theme 常量重写卡片
│   ├── tab_bar.rs                        ← 改：圆角 + 选中线 + bg 分层
│   ├── connection_chip.rs                ← 改：高度 / 颜色 / 折叠图标
│   └── host_form.rs                      ← 改：圆角输入框 + segmented + chip
└── ...
```

---

## Task 1: theme.rs — 色值 / 字号 / 半径常量

**文件**：`crates/aish-app/src/theme.rs`（新）+ `lib.rs` 注册 mod

**内容骨架**（全是 const，无运行时逻辑）：

```rust
//! aish 视觉规范（M3d-ui-polish）：色值 / 字号 / 半径 / 间距常量。
//!
//! 所有 view 用本文件常量，不要散写魔法值。改色调一处全局生效。

use gpui::{Pixels, px, rgb, Rgba};

// ===== Background =====
pub const BG_BASE: u32 = 0x0a0a0c;
pub const BG_ELEVATED: u32 = 0x15161a;
pub const BG_HOVER: u32 = 0x1f2128;
pub const BG_SELECTED: u32 = 0x26282f;

// ===== Border =====
pub const BORDER_SUBTLE: u32 = 0x1f2128;
pub const BORDER_STRONG: u32 = 0x2f323a;

// ===== Text =====
pub const TEXT_PRIMARY: u32 = 0xeaeaee;
pub const TEXT_SECONDARY: u32 = 0x888a93;
pub const TEXT_MUTED: u32 = 0x5b5d66;

// ===== Accent =====
pub const ACCENT_BLUE: u32 = 0x4a9eff;
pub const ACCENT_GREEN: u32 = 0x4ec9b0;
pub const ACCENT_RED: u32 = 0xff6b6b;

// Chip 底（比 accent 深，配上 accent 文字色对比清晰）
pub const CHIP_BLUE_BG: u32 = 0x1f3a5c;
pub const CHIP_GREEN_BG: u32 = 0x16382f;

// ===== 字号 =====
pub fn text_xl() -> Pixels { px(16.0) }
pub fn text_lg() -> Pixels { px(14.0) }
pub fn text_sm() -> Pixels { px(12.0) }
pub fn text_xs() -> Pixels { px(11.0) }
```

**验证**：
- `cargo check -p aish-app` 通过
- 文件被 main.rs / lib.rs 注册

**Commit**：`refactor(ui): 抽 theme 常量集中色值 / 字号`

---

## Task 2: 默认页改造

**文件**：`crates/aish-app/src/views/default_page.rs`

**改动**：
- import `crate::theme`
- 全屏底 `bg(rgb(BG_BASE))`
- "已保存的连接" header padding `px_8 pt_6 pb_3`，标题 `text_xl primary`
- 卡片：
  - `bg(rgb(BG_ELEVATED))` + `border_color(rgb(BORDER_SUBTLE))` + `rounded_xl`
  - hover: `bg(BG_HOVER)` + `border_color(BORDER_STRONG)`
  - 内部 padding `px_4 py_3`，gap 用 `gap_3`
- label：`text_lg primary`
- 副信息行 `user@host:port`：`text_sm secondary`，加在 label 下面（flex_col gap_1）
- SSH chip：`bg(CHIP_BLUE_BG)` + `text_color(ACCENT_BLUE)` + `rounded_md` + `px_2 py_0p5`
- 编辑/删除按钮 hover 出现，符号改 `✎` / `×`，hover 颜色 `ACCENT_RED`（删）/ `TEXT_PRIMARY`（编）
- "+ 添加 host" 按钮：`bg(BG_ELEVATED)` + `border 1` + `rounded_md` + hover `BG_HOVER`

**验证**：cargo run 看默认页，对比 spec 里的 ASCII 图

**Commit**：`feat(ui): 默认页改大圆角卡片 + 副信息行 + theme 配色`

---

## Task 3: Tab 栏改造

**文件**：`crates/aish-app/src/views/tab_bar.rs`

**改动**：
- 高度 36 → 40px（`h(px(40.0))`）
- 每个 tab：
  - 顶部 8px 圆角 `rounded_t_lg`，下平
  - 间距用 `gap_0p5` 而不是 border_r 切割
  - 选中：`bg(BG_SELECTED)` + 底部 `border_b_2` 颜色 `ACCENT_BLUE`
  - 非选中：`bg(BG_BASE)` + 文字 `TEXT_SECONDARY`
  - hover：`bg(BG_HOVER)` + 文字 `TEXT_PRIMARY`
- `+` 按钮独立成块，`bg(BG_BASE)` hover `BG_HOVER` + 文字 primary
- 双击 inline edit 模式：去掉 `|` 光标字符，改 1px `border` 颜色 `ACCENT_BLUE`
  （inline buffer 直接显示文本，光标视觉用边框暗示焦点）

**验证**：cargo run 看 tab 栏选中态有蓝条

**Commit**：`feat(ui): Tab 栏圆角 + 选中蓝线 + bg 分层`

---

## Task 4: Connection chip 改造

**文件**：`crates/aish-app/src/views/connection_chip.rs`

**改动**：
- 高度 32 → 36px
- `bg(BG_ELEVATED)`（之前 #141414 太接近 base）
- 下边 `border_b_1` 颜色 `BORDER_SUBTLE`
- left padding `px_4`
- ⊖ 替换为 `▾`，hover 颜色 `TEXT_PRIMARY`
- × hover 颜色 `ACCENT_RED`
- label：`text_lg primary`，SSH chip 同 default_page 样式（复用常量）
- 活跃 dot：`text_color(rgb(ACCENT_GREEN))`

**验证**：cargo run attach connection 看 chip 高度 + 颜色

**Commit**：`feat(ui): Connection chip 高度 + 折叠图标 + theme 配色`

---

## Task 5: Host form modal 改造

**文件**：`crates/aish-app/src/views/host_form.rs`

**改动**（先 read 现状再决定细节，初步思路）：
- 整体 modal 卡片：`bg(BG_ELEVATED)` + `rounded_xl` + 居中 + 半透明遮罩底
- modal padding `px_6 py_5`
- 标题 `text_xl primary`
- 每个字段一行：label `text_sm secondary`（上方 4px gap） + input 块
- input：
  - `bg(BG_BASE)`（深于 modal 底，下沉感）
  - `rounded_md` + `border_color(BORDER_SUBTLE)`
  - `px_3 py_2`
  - focused：`border_color(ACCENT_BLUE)`
- 认证方式 segmented control：
  - 横向两个 div，`bg(BG_BASE)` 包外层 `rounded_md`
  - 选中那个 `bg(BG_SELECTED)` + `text_primary`
  - 未选 `text_secondary`
- 私钥 chip：保留 emoji `🔑` + SHA256 缩写 + 右侧 `×` 清除按钮
- 按钮：
  - 取消：`bg(BG_BASE)` + 文字 secondary + hover primary
  - 保存：`bg(rgb(ACCENT_BLUE))` + 文字 `TEXT_PRIMARY` + `rounded_md`
- 删除确认对话：警告色用 `ACCENT_RED`

**验证**：cargo run 点 + 添加 host / 编辑现有 host 看 modal

**Commit**：`feat(ui): Host form modal 圆角输入框 + segmented + 配色统一`

---

## 完成验证（整体）

| 项 | 检查 |
|---|---|
| 4 个区域全部应用 theme 常量 | grep `0x[0-9a-f]{6}` 无散写魔法值（除 theme.rs 外） |
| fmt + clippy + test | 全过 |
| 用户截图对比参考图 | iterate 调整 |

完成后更新 `docs/superpowers/INDEX.md`：把本里程碑加进 ✅ 完成区，
backlog 清单改进对应条目。

---

## Self-Review

### Spec 覆盖
- ✅ 默认页（Task 2）
- ✅ Tab 栏（Task 3）
- ✅ Connection chip（Task 4）
- ✅ Host form（Task 5）
- ✅ theme 抽象（Task 1，前置依赖）

### Placeholder 扫描
- Task 5 host_form 细节"先 read 再决定"是真实情况，不是偷懒
- font_weight / shadow / animation 在 spec 里明确标了"不依赖 / fallback"
- segmented control 自实现方案在 spec 风险表里说明

### 依赖顺序
1. Task 1（theme.rs）必须先做，2-5 引用其常量
2. Task 2-5 互不依赖，可任意顺序
3. 每条 task 独立 commit，方便 user 看截图反馈调

### 不做
- 主题切换（暗/亮）
- 动效（GPUI 0.x 不稳定）
- 字体替换（Nerd Font 已 bundled）
- 设计系统抽象层（CSS variables 等价物）

---

## 下一步

Task 1 → 5 顺序执行。每条独立 commit，user 跑 `cargo run` 截图反馈，
基于截图 iterate（可能产生小调整 commit）。完成后更新 INDEX.md。
