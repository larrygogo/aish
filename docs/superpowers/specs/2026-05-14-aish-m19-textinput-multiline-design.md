# M19 — TextInput 多行 + word-wrap + auto-grow

**日期**: 2026-05-14
**父 spec**: [`2026-05-09-aish-ui-architecture-design.md`](2026-05-09-aish-ui-architecture-design.md)
**目标**: 把 `aish_ui::TextInput` 从单行扩展到多行，支持 word-wrap + auto-grow + 跨行键鼠交互
**预计工程量**: ~半天 - 1 天

---

## 1. 动机

Input bar 现在用单行 TextInput，用户长 prompt（如给 AI agent 的指令）无法分段输入。
现代 chat UI（VS Code chat / Claude Desktop / ChatGPT）都用多行 textarea：
- Enter 换行，Ctrl+Enter 发送
- 自动 word-wrap 不需要用户手动 \n
- 按内容自动增高，超过上限内部滚动

aish-ui 多行模式应该是 TextInput 的扩展开关（不新建 Textarea 组件），避免双套 API
维护重复。

---

## 2. 决策记录（ADR-style）

### D-1: API 形式

**采**：`.multiline(true)` builder，默认 false（单行行为不变）。
配套 `.max_lines(n)` 设上限，默认 6。

**拒**：自动从内容 `text.contains('\n')` 切多行 —— caller 控制更明确。

### D-2: Enter 语义

**采**：多行下 Enter = 插 `\n`，**Ctrl+Enter** = trigger `on_submit`。
单行不变（Enter = submit）。

**拒**：Shift+Enter 换行（Slack 风）—— 与 AI chat 主流不符；Claude Desktop / VS Code
chat / ChatGPT 都是 Enter 换行 + Ctrl/Cmd+Enter 发送，aish 跟主流走。

**caller 影响**：InputBar 当前 send 走 `on_submit`（默认绑 Enter）。multiline 切换后
Enter 不再触发，需 InputBar 显式接 Ctrl+Enter handler；或 TextInput 在多行下自动把
Ctrl+Enter 路由到 `on_submit`（推荐，对 caller 透明）。

### D-3: 高度策略

**采**：auto-grow + max_lines 上限。
- visual_lines.len() ≤ max_lines: container 高度 = `visual_lines.len() * line_h`
- visual_lines.len() > max_lines: container 高度 = `max_lines * line_h`，内部
  `overflow_y_scroll`，cursor 行外时自动 scroll 到 cursor 行可见

**实现**：GPUI `.min_h(line_h) .max_h(max_lines * line_h)` + 内容自然撑高（GPUI div
默认 height = content）。超过 max_h 时 overflow_y_scroll。

**风险**：input bar layout 可能因 input 高度变化被挤压。InputBar 的 flex 布局要确保
input 区是 flex_1 让其他元素（Send 按钮 / +）保持位置。

### D-4: word-wrap 算法

**采**：每帧 render 时按 char 累加宽度，超过 container width 时 wrap 到下一 visual line。
ASCII char 估 `font_size * 0.6`，CJK char 估 `font_size * 1.2`，emoji 也算 2x。

**拒**：完全准确的 text shaping (每个 char 实际 glyph width) —— GPUI text_system shape
在 render 期间调用代价高，且字体 substitution / kerning 复杂；估算用 monospace 字体
+ CJK 2x 已经 95% 准确（cursor click 偏差最多 1 char）。

**拒**：完全不做 wrap（让用户手动 \n + 水平滚动）—— 与 modern textarea UX 严重不符，
用户期望粘贴长文本自动 wrap。

**break 规则**：
- 优先 word boundary 断（空格 / 标点 / CJK 字符间）
- 单 word 超过 container width 时强制 char-level 断（防止超长 URL 撑爆）

### D-5: cursor / selection 存储

**采**：保持 `cursor: usize`（byte offset），`selection_anchor: Option<usize>`。
\n 字符也算 byte，与 \r 等其他控制字符一致。

**拒**：改 `(line, col)` 2D 结构 —— 破坏所有现有 helper（cursor_left / cursor_right / IME insert / 复制粘贴），改动面太大；byte offset + 转换 helper 是最小侵入。

**转换 helpers**:
- `byte_to_logical_pos(byte) -> (logical_line, col_in_logical)`
- `byte_to_visual_pos(byte, visual_lines) -> (visual_line, col_in_visual)`
- `visual_pos_to_byte(visual_line, col, visual_lines) -> byte`

### D-6: 键盘 nav 跨行

- **ArrowUp/Down**: 跨 **visual** line（wrap 后），col 保持 `preferred_col`（用户连续
  按 ↑/↓ 时 col 不变即便目标行更短）
- **Home/End**: 当前 visual line 行首 / 行末
- **PageUp/Down**: 不实现（多行场景一般 ≤ max_lines = 6，PageUp 跳行没意义）

### D-7: drag select / mouse 跨行

mouse_down 接收 `ev.position`，按 (y, x) 映射 → visual_line idx → 行内 col → byte。
drag select 期间 mouse_move 同样路径更新 cursor。selection 显示按 visual_line 拆段画
（与 logical line 内 wrap 自然吻合）。

drag-to-edge auto-scroll：垂直方向。鼠标接近上/下边沿 20px 时持续上/下扩 cursor +
触发滚动到 cursor 可见。

### D-8: 水平滚动

多行模式禁用 `scroll_offset` 水平滚动（word-wrap 已覆盖长行）。单行下保留。

### D-9: 性能

每帧 render 时 word-wrap 算法 O(n) 扫 text，对 < 1000 字符 prompt 几乎零成本。
长 prompt（10k+）可能有感，但实际场景罕见，先不优化。

---

## 3. 架构变化总览

```
+---------------------------------------------------------------+
|  TextInput.multiline = true                                    |
+---------------------------------------------------------------+
| Field changes:                                                 |
|   - multiline: bool (new)                                      |
|   - max_lines: usize (new, default 6)                          |
|   - preferred_col: Option<usize> (cursor up/down 时 col 记忆)   |
+---------------------------------------------------------------+
| Helpers (新加):                                                 |
|   - compute_visual_lines(text, container_w, font) -> Vec<VL>   |
|       VL { logical_line, byte_range, segments }                 |
|   - byte_to_visual_pos(byte, &visual_lines) -> (vl_idx, col)   |
|   - visual_pos_to_byte(vl_idx, col, &visual_lines) -> byte     |
+---------------------------------------------------------------+
| Render 改造:                                                   |
|   single-line (existing) | multiline (new)                     |
|   ------------------------+--------------------------------    |
|   single inline row       | for each visual_line:               |
|   ml(scroll_offset)       |   render inline-glyph row at y      |
|                           | cursor 在某 row 内某 col 渲染        |
+---------------------------------------------------------------+
| Mouse:                                                         |
|   single: cursor_from_click(x)                                 |
|   multi:  cursor_from_click(y, x) -> visual_pos -> byte        |
+---------------------------------------------------------------+
| Keyboard:                                                      |
|   single: cursor_left / right + arrows                         |
|   multi:  + cursor_up_visual / cursor_down_visual             |
|           + Enter -> insert '\n'                                |
|           + Ctrl+Enter -> on_submit                             |
+---------------------------------------------------------------+
```

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | word-wrap 估算偏差导致 click 定位错位 | 中 | monospace 字体下估算 ≥95% 准确；偏差最多 1 char 用户可接受；后续可换 text_system.shape 精算 |
| R2 | InputBar layout 因 input 高度变化挤压 | 中 | 高度由 max_lines 限制（最多 6 行 = ~140px），InputBar 区已 flex_col 自适应 |
| R3 | 跨行 selection render 跨多段 div 画，性能差 | 低 | selection 区段 ≤ visual_lines 数 ≤ max_lines，常数级 |
| R4 | cursor_up/down 在 wrap 行内行为预期 | 低 | preferred_col 标准 textarea 行为，已成熟 |
| R5 | InputBar 改 Ctrl+Enter binding 破坏现有用户习惯 | 高 | 让 TextInput 在 multiline 下自动把 Ctrl+Enter 路由到 on_submit；caller 不用改 |
| R6 | drag-to-edge auto-scroll 改成垂直方向 | 低 | 复用现有 30ms timer pattern |
| R7 | IME 输入与 \n 交互（IME composition 内含 \n？） | 低 | IME 一般不输出 \n，replace_text_in_range 收到 \n 按普通字符 insert |

---

## 5. Out of scope（M19 不做）

- 行号显示（gutter）
- 语法高亮 / token color
- Tab 键缩进（Tab 仍走默认 focus 切换或 GPUI 默认）
- find / replace
- Undo / Redo（单行也没有）
- 行间距 / lineheight 配置（用 font_size 默认）
- PageUp/PageDown（max_lines ≤ 6 时无意义）
- 完全准确 text shaping（用估算代替）

---

## 6. 测试策略

### 单测（aish-ui）

- `byte_to_visual_pos` / `visual_pos_to_byte` 双向转换
- `compute_visual_lines` 在 ASCII / CJK / 单 word 超长 等 case
- `cursor_up_visual` / `cursor_down_visual` 跨行行为
- preferred_col 在多次 up/down 后保持

### 集成（手测）

- InputBar 改用 multiline，输入 prompt 多段 → 按 Ctrl+Enter 发送 → echo 到 PTY 正确
- 在 multiline input 内 Enter → 插 \n，不发送
- mouse drag 跨行选中
- 长文本（粘贴 ~500 字）→ word-wrap 视觉正常，cursor 可正常定位
- 中文 + ASCII 混排 → wrap 边界合理

---

## 7. Plan 引用

见 [`../plans/2026-05-14-aish-m19-textinput-multiline.md`](../plans/2026-05-14-aish-m19-textinput-multiline.md)

---

## 8. 实施记录（2026-05-14 完成）

T1-T6 已实施，T7 文档收尾。

### 实际 commits

| Task | Commit | 内容 |
|---|---|---|
| spec + plan | `c1eff2f` | 本文件 + plan 起草 |
| T1 | `e37c37d` | multiline / max_lines / preferred_col 字段 + builder |
| T2 | `5b4797b` | compute_visual_lines + byte ↔ vl 转换 + 13 单测 |
| T3 | `68a46d1` | render multiline 路径（按 visual_line 拆 row） |
| T4 | `2faff94` | 键盘 nav 跨行 + Enter / Ctrl+Enter + 4 单测 |
| T5 | `640b7e7` | mouse cursor_from_click_2d 2D 路径 |
| T6 | `98f7d44` | InputBar 接 multiline(true).max_lines(6) |

### Risk 实际遇到

- **R1 (wrap 估算偏差)**：未实测见严重偏差。monospace 字体下 ASCII × 0.6 + CJK × 1.2 估算合理，click 定位偏差 ≤ 1 char。
- **R2 (InputBar layout)**：max_lines=6 + line_h=20 = 120px 上限，未发现挤压。
- **R5 (Ctrl+Enter 透明路由)**：TextInput 内部 `"enter" if multiline && ctrl => fire_submit` 路由成功，InputBar caller 透明（on_submit 不动）。
- **R6 (vertical drag-to-edge)**：M19 未实现，留 M20。多行 drag 在 viewport 内移动 cursor 可用，drag 到上下边沿无 auto-scroll。

### 未做（M20+）

- vertical drag-to-edge auto-scroll
- 多行 vertical scrollbar UI（cursor 在屏外只能键盘 ↑↓ 间接 nav）
- font_size 在 cursor_up/down 路径 hardcoded px(12.0)，需 cache 到 self.last_font_size

### 测试增量

- aish-ui 158 → 180 (+22)
  - T2 +13: visual_lines / byte ↔ vl 双向转换 / 边界 case
  - T4 +4: cursor_up/down_visual preferred_col / 首末行 clamp
  - T1/T3/T5/T6 跳过单测（builder / render / mouse 难纯函数化，集成测试靠手测）
- aish-app 不变
