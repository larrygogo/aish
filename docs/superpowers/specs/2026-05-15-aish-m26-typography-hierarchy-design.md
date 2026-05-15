# M26 — Typography × Information Hierarchy 体系

**日期**: 2026-05-15
**父 spec**: [`2026-05-15-aish-m24-visual-redesign-design.md`](2026-05-15-aish-m24-visual-redesign-design.md)
**目标**: 建立 size + weight + color-role 三维 type system，所有 view
按语义 type role 渲染文字，建立明确信息层级
**预计工程量**: 1-2 天，T1 token + T2-T5 view 改造分阶段

---

## 1. 动机

M24 仅做色彩 token 置换，**视觉系统骨架仍未建立**。当前 codebase audit：

- 73 处 `text_size` 调用
- 用了 12 种不同 size（5 个 token + 7 处 hardcoded px(9/11/12/13/14/16/24/40)）
- `font_weight` 全 codebase **仅 1 处**调用（home.rs label SEMIBOLD）
- 没有"title / body / caption / label" 等语义角色 token

Linear / Warp / Stripe Dashboard 等成熟商业级 UI 共同点：**字体层级 ≠ size
跳变**，而是 **(size × weight × color)** 三维。例：
- title 14px / **600** / foreground
- body 13px / **400** / foreground
- caption 12px / **400** / **muted_foreground**

title 与 body 仅差 1px 但 weight 提供主对比，caption 与 body 同 size 但
color 弱化 — 这是 dev tool / dashboard 通用 pattern，远比"title 20 / body
14"size 跳跃更精致。

---

## 2. 决策记录（ADR-style）

### D-1: 9 个语义 type role

**采**：定义 9 个 type token（每个 = size + weight + 默认 color_role）：

| Token | Size | Weight | Default Color | 用途 |
|---|---|---|---|---|
| `micro` | 11px | 400 | muted_foreground | 键盘快捷键 / badge / 角标 |
| `caption` | 12px | 400 | muted_foreground | meta 信息 / helper 文字 / 上次连接时间 |
| `body` | 13px | 400 | foreground | 列表项 / 普通文字 / Card 内容 |
| `body_strong` | 13px | 500 | foreground | 强调 inline 词 / 当前选中项 label |
| `label` | 13px | 500 | foreground | form field label / 表头 |
| `title_3` | 14px | 600 | foreground | Card / section header / list group title |
| `title_2` | 16px | 600 | foreground | Dialog title / sidebar group title |
| `title_1` | 20px | 600 | foreground | page title (Home / Settings 等) |
| `code` | 13px | 400 | foreground | inline code / mono path (monospace family) |

**拒**：保留旧 xs/sm/base/lg/xl 5 档 size-only token（无 weight / color
语义，caller 仍要 chain 3 行 API）；命名 h1/h2/h3 / p / small（HTML
惯例但与 GPUI 抽象不一致）。

### D-2: API — Styled trait extension

**采**：trait `Typography` 给所有 `Styled` 元素加 `.typography(role, t)`
方法，一行 apply size + weight + 默认 color：

```rust
div()
    .typography(TypeRole::Title3, t)
    .child("Settings")
```

caller 仍可后续 chain `.text_color(custom)` override 默认 color_role。

**拒**：每个 role 一个 free fn（`title_3(div, t)` 之类）—— 不符合 GPUI
fluent builder 风格，可读性差。

### D-3: 保留旧 FontSize 5 档作 fallback

新加 `Typography` struct 在 `Theme`，**不动**旧 `FontSize` token —— 73 处
现有 callsite 不强制全改，逐步迁移。两套并存期 6-12 个月，新增代码用
`.typography(...)`，旧代码渐进改。

**拒**：一次性 deprecate FontSize —— 73 处全改单 commit 太大、回归风险高。

### D-4: 默认 color role 但 caller 可 override

`TypeRole` 内置 `default_color_role`，但 `.typography(role, t)` 不强制
text_color —— caller 后续 `.text_color(custom)` 仍可覆盖。

理由：destructive Button 内的 body 文字色应是 destructive_foreground
（白），不是 foreground。该场景 caller 先 .typography(Body, t) 再
.text_color(t.colors.destructive_foreground)。

### D-5: GPUI font_weight 兼容性

GPUI `Styled::font_weight` 接 `FontWeight`（gpui::FontWeight 枚举：
THIN/LIGHT/NORMAL/MEDIUM/SEMIBOLD/BOLD 等）。.typography 内部 match
TypeRole 应用对应 FontWeight。

实测：GPUI 在 system font 上多数 weight 都渲染，但 NORMAL=400 / MEDIUM=500
/ SEMIBOLD=600 是稳定支持的最常见三档。

### D-6: 不引入新字体

继续用 system sans（Inter / Segoe UI 等）+ FONT_NAME 自带 monospace。
type role 仅控 size + weight，字体 family 不动。

字体替换若需要走独立 milestone。

### D-7: 改造范围

T1 加 token + ext trait。T2-T4 分批改最受益的 view：
- **Phase 1 (T2)**: page title — Home / Settings / EmptyTerminalGuide 三处 page header
- **Phase 2 (T3)**: Card / Section headers — settings 3 个 card / home active section
- **Phase 3 (T4)**: list items — host card / dropdown / session_picker / settings rows
- **Phase 4 (T5)**: dialog / form labels — host_form 各字段 label / footer

总共 ~25 处主要 callsite。其余 50 处保留旧 API（多是 px(12/11) 微调，
风险低、变更频繁，强制迁移收益小）。

---

## 3. 架构变化总览

```
+-------------------------------------------------------+
| theme/typography.rs (新增)                              |
|   pub enum TypeRole { Micro Caption Body BodyStrong   |
|       Label Title3 Title2 Title1 Code }               |
|   pub struct TypeStyle { size weight default_color }  |
|   pub struct Typography { 9 个 TypeStyle 字段 }        |
|   pub trait TypographyExt { fn typography(...) }       |
+-------------------------------------------------------+
| theme/tokens.rs                                         |
|   Theme 加 typography: Typography 字段（与 FontSize 并存） |
+-------------------------------------------------------+
| views 分阶段迁                                           |
|   page title / card header / list item / form label    |
+-------------------------------------------------------+
```

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | 73 处 text_size 渐进迁移期视觉不一致 | 中 | T2-T5 按 view 分批，每批 commit 内完全切换该 view；新增代码强制用新 API |
| R2 | medium/semibold weight 在某些字体上渲染相同 | 低 | system font 标准 weight 都支持；GPUI Inter / Segoe UI 实测 OK |
| R3 | trait ext 与 GPUI 内部 trait method 冲突 | 低 | 名字 `typography` 与 gpui 无冲突；测试 caller chain 不失败 |
| R4 | caller 误用 — 同 view 内多种 role 滥用 | 中 | spec 表格明示每个 role 用途，code review 把关 |
| R5 | hardcoded px(11/13) 仍存在 50 处不迁 | 低 | 决策接受（D-3 fallback），将来若需要再补 |

---

## 5. Out of scope（M26 不做）

- 字体替换 / monospace 字体调优
- letter-spacing / line-height token（GPUI 暂未暴露 letter-spacing API）
- 响应式 type scale（小窗口字号自适应）
- 一次性 deprecate 旧 FontSize 5 档
- 73 处全 audit 替换（仅迁 25 处核心）

---

## 6. 测试策略

### 单测（aish-ui）

- `Typography::default()` 9 个 TypeStyle 字段值正确
- size 单调性：micro < caption < body ≤ body_strong < title_3 < title_2 < title_1
- weight 单调性：caption/body 400, label/body_strong 500, title_* 600

### 集成（手测）

- 切到 dark / light 看 page title / Card header / list item 三层视觉
  是否清晰
- 改造前后截图对比，确认 hierarchy 加强

---

## 7. Plan 引用

见 [`../plans/2026-05-15-aish-m26-typography-hierarchy.md`](../plans/2026-05-15-aish-m26-typography-hierarchy.md)
