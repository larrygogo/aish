# M19 — TextInput 多行 + word-wrap + auto-grow（Plan）

**Spec**: [`../specs/2026-05-14-aish-m19-textinput-multiline-design.md`](../specs/2026-05-14-aish-m19-textinput-multiline-design.md)
**实施目标**: 实现 D-1 ~ D-9 决策，验证 R1-R7 风险均可接受

---

## File Structure

```
crates/aish-ui/src/components/text_input.rs    (主体改造)
crates/aish-ui/src/components/text_input/      (可选拆 module 如果文件太长)
crates/aish-app/src/views/input_bar.rs         (接 multiline + Ctrl+Enter)
```

---

## Tasks（顺序，每条独立 commit）

### T1: 加字段 + builder API（无功能）

- TextInput 加字段:
  ```rust
  multiline: bool,
  max_lines: usize,
  preferred_col: Option<usize>,
  ```
- new() 默认: `multiline=false, max_lines=6, preferred_col=None`
- builder:
  ```rust
  pub fn multiline(&mut self, b: bool) -> &mut Self
  pub fn max_lines(&mut self, n: usize) -> &mut Self
  ```
- 行为完全不变（multiline=false 默认所有现有 callsite 不受影响）
- 单测：`new_defaults_single_line`, `multiline_builder_sets_field`

**质量门禁**: fmt + clippy + test 通过。

---

### T2: compute_visual_lines + 双向转换 helper

- 新结构 `VisualLine`:
  ```rust
  struct VisualLine {
      logical_line: usize,    // 第几个 logical line (按 \n 分)
      byte_start: usize,      // 源 text 内此 visual line 起始 byte
      byte_end: usize,        // 结束 byte（不含 \n / wrap point）
      // chars + 宽度信息可以在 render 时算，不存
  }
  ```
- pure fn `compute_visual_lines(text: &str, container_width: Pixels, font_size: Pixels) -> Vec<VisualLine>`
  - 按 \n 分 logical lines
  - 每 logical line 按估算宽度 wrap：ASCII char ≈ font_size * 0.6, CJK ≈ font_size * 1.2
  - 优先 word boundary 断（空格 / CJK 字符间），单 word 超长强制 char-level
- pure fn `byte_to_visual_pos(byte: usize, vls: &[VisualLine]) -> (usize, usize)` (vl_idx, col_in_visual)
- pure fn `visual_pos_to_byte(vl_idx: usize, col: usize, vls: &[VisualLine]) -> usize`
- 单测覆盖：
  - 空文本 → 1 个空 visual line
  - 单行 ASCII `"hello"` → 1 visual line
  - 含 \n `"a\nb"` → 2 visual lines
  - 超宽 ASCII auto-wrap
  - CJK `"中文长行"` 估算
  - 单 word 超宽强制 break
  - byte ↔ visual pos roundtrip

**质量门禁**: 测试覆盖关键 case。

---

### T3: render 改造 — multiline 路径

- render 内根据 `self.multiline` 分支：
  - false: 走现有单行 inline row（不变）
  - true: 算 visual_lines + 按行渲染
- multiline render 结构:
  ```
  container (overflow_y_scroll, min_h, max_h)
    div.flex_col
      for vl in visual_lines:
        div.flex_row.items_center
          for char in vl:
            glyph_div (byte, ch, cursor 在此 col 时 cursor_div 插入此处)
  ```
- container 高度: `min_h(line_h) max_h(max_lines * line_h)`
- cursor 渲染：cursor 落在哪 visual line + col → 在该 row 内 col 位置插 cursor div
- bounds_map 仍 push 每 char absolute bounds（多行情况下 y 不同）

**质量门禁**: 手测多行渲染正常 + cursor 在不同 line 闪烁正确。

---

### T4: 键盘 nav 跨行

- handle_key 新分支:
  - `"enter"` & multiline & no ctrl: insert `\n`，preferred_col 清掉
  - `"enter"` & multiline & ctrl: trigger on_submit（与单行 Enter 等价）
  - `"up"` & multiline: cursor_up_visual（基于 preferred_col）
  - `"down"` & multiline: cursor_down_visual
  - `"home"` & multiline: 到当前 visual line 行首
  - `"end"` & multiline: 到当前 visual line 行末
- preferred_col 更新：left/right/click 等"横向"操作清掉 preferred_col；up/down 不清

**质量门禁**: 手测跨行导航；自动测：`cursor_up_visual_preserves_col`。

---

### T5: mouse 跨行（click + drag）

- cursor_from_click 改 `(y, x) -> byte`:
  - 用 bounds_map 找最近的 char bounds（先按 y 范围筛 visual line，再按 x 找 col）
- handle_mouse_down: 不变（已经接 ev.position 整 Point）
- drag-to-edge auto-scroll: 改成同时管 horizontal（单行用）和 vertical（多行用）
  - multiline 模式下 drag 到上/下边沿 → cursor 上/下扩一行

**质量门禁**: 手测跨行 drag select；中文 + ASCII 混排 click 定位偏差 ≤ 1 char。

---

### T6: InputBar 接 multiline + 验证

- InputBar 把 TextInput 改 .multiline(true) .max_lines(6)
- on_submit 行为不变（TextInput 内部已经把 Ctrl+Enter 路由到 on_submit）
- 验证：
  - 在 InputBar 输入 prompt → Enter → 换行
  - 输完按 Ctrl+Enter → 发送
  - 粘贴长文本 → word-wrap
  - Send 按钮点击仍然走原 callback（不影响）

**质量门禁**: 手测 InputBar 完整流程；与远端连接验证 prompt 内容含 \n 正确转 PTY。

---

### T7: 文档 + INDEX 更新

- 更新 `docs/superpowers/INDEX.md` 加 M19 entry
- 更新 spec 末尾"已实现"标记 + 修订 risk 表（实际遇到的偏差）
- 写 commits 汇总

---

## Self-Review Checklist

- [ ] D-1 ~ D-9 决策每条都对应 task
- [ ] Risk R1-R7 在 task 内有 mitigation 落地
- [ ] 单行行为 0 改变（默认 multiline=false）
- [ ] aish-ui 测试新增 ≥ 10 个（compute_visual_lines / 双向转换 / preferred_col）
- [ ] fmt + clippy + test 在每 task commit 前全过
- [ ] commits 严格按 task 顺序，每 task 1 commit
- [ ] 末尾合并 push 一波

---

## 实施顺序与依赖

```
T1 (字段 + API) ─────┐
                     ↓
T2 (compute_visual_lines + 转换) ─┐
                     ↓             ↓
T3 (render multiline) ←─────────────┘
                     ↓
T4 (键盘 nav) ←──── T2 helpers
                     ↓
T5 (mouse 跨行) ←─── T2 helpers + T3 render bounds_map
                     ↓
T6 (InputBar 接入 + 集成测试) ─── 所有 T1-T5
                     ↓
T7 (文档)
```
