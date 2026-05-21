# M42 multiline TextInput 鼠标定位精度修复

> 2026-05-21 用户报告 input bar 鼠标定位不准：click 错位 / drag select
> 范围错 / 多行点击错行 / 滚动后偏移 / 空行换行视觉不显眼。
>
> 方案 A（用户确认）：补充 GPUI text_system 真实测量替换 approx_char_width
> 粗估，保留 per-glyph div + bounds_map 架构。

---

## 根因

`compute_visual_lines` 用 `approx_char_width` 估算字符宽度判断 wrap：
- ASCII / Latin / Greek / Cyrillic：`font_size × 0.6`
- CJK / 其他：`font_size × 1.2`

但 GPUI 实际 paint 用字体真实 glyph advance width。两者误差累积：
1. **wrap 位置偏移**：估算的 byte_end 跟 paint 后真实 row wrap 不一致
2. **点击行匹配错**：click.y 命中实际 row，但 vl.byte_end 对应另一行
3. **drag select 范围错**：anchor / cursor 都基于错位的 vls
4. **滚动后偏移**：scroll_offset_y 累积偏差更明显

`byte_offset_at_x` 用 bounds_map（每 glyph div 的 GPUI 真实 bounds），理论上
准 — 但 vls 跟 paint 行不对应时，filter row_entries 拿错 row 范围。

---

## GPUI 提供的 API

`gpui::WindowTextSystem`（`window.text_system()` 访问）：

- `layout_line(text, font_size, runs, force_width) -> Arc<LineLayout>` — 给单
  logical line（无 `\n`）shape 出真实 glyph metrics
- `LineLayout.compute_wrap_boundaries(text, wrap_width, max_lines) -> Vec<WrapBoundary>`
  — 真实算 wrap 边界
- `LineLayout.closest_index_for_x(x) -> usize` — hit test，给 x 拿最近 byte
- `LineLayout.x_for_index(byte) -> Pixels` — 给 byte 拿 x（视觉光标定位用）
- `WrapBoundary { run_ix, glyph_ix }` + `layout.runs[run_ix].glyphs[glyph_ix].index`
  → wrap 点对应的 byte

---

## 实施

### Phase 1（本 milestone）

**改动文件**：`crates/issh-ui/src/components/text_input.rs`

**新增字段** `TextInput`：
```rust
cached_visual_lines: Vec<VisualLine>,         // 上一帧 layout 后的 vls
cached_layouts: Vec<Arc<LineLayout>>,         // per logical line layouts
```

**新增函数**：
```rust
fn recompute_visual_lines_with_layout(
    text: &str,
    container_w: Pixels,
    text_system: &Arc<WindowTextSystem>,
    font: &Font,
    font_size: Pixels,
) -> (Vec<VisualLine>, Vec<Arc<LineLayout>>);
```

按 `\n` 拆 logical lines → 每段调 `layout_line` → 用 `compute_wrap_boundaries`
拿真实 wrap → 转 `VisualLine`。

**改造点**：
1. `render` multiline 分支入口调上述 fn，更新 `cached_*` 字段
2. iter `cached_visual_lines` 渲染 row（替换旧 `compute_visual_lines`）
3. `cursor_from_click_2d` 改用 `cached_visual_lines` 找 vl_idx + 用
   `cached_layouts[vl.logical_idx].closest_index_for_x` 做精确 hit test

**保留不动**：
- per-glyph div + bounds_map（selection 视觉渲染依赖）
- `compute_visual_lines` pure fn + 旧单测（作 fallback / 单测保留）
- `current_visual_lines` 方法 + keyboard nav 路径（cursor_up/down 用 byte-diff
  col，跟视觉宽度无关；自动受益于 wrap 位置准的 `cached_visual_lines`，但
  Phase 1 暂不改 method signature）

### Phase 2（未来）

- `current_visual_lines` 改用 cache，keyboard nav 跟 render 完全同源
- selection / cursor x 渲染走 `LineLayout.x_for_index`（更精确）
- cache 失效优化（避免每帧 re-layout — GPUI 有内置 cache 但本地也可加 dirty flag）

---

## Risk

| 风险 | 缓解 |
|---|---|
| 切换后 wrap 位置变化引起视觉跳动 | 真实测量更准，wrap 位置接近用户期望 |
| host_form / single-line 用例受影响 | 仅 multiline 分支改造，单行路径不动 |
| 字体不存在 / shape 失败 | layout_line 内部 fallback；返回空 layout 时回退到旧 compute_visual_lines |
| GPUI text_system API 性能 | GPUI 内置 line_layout_cache，每帧 layout 廉价 |

---

## 不在范围内

- `host_form` / `tab_bar` rename 的单行 TextInput — 单行无 wrap，approx 误差影响小
- bounds_map 写入逻辑（per-glyph paint 不变）
- selection 渲染路径（per-glyph bg 不变）
- IME 路径 / paste 路径
- keyboard nav 改用 cache（Phase 2）
