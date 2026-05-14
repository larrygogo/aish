# M21 — TextInput 多行 vertical scrollbar + drag-to-edge auto-scroll（Plan）

**Spec**: [`../specs/2026-05-14-aish-m21-textinput-vscroll-design.md`](../specs/2026-05-14-aish-m21-textinput-vscroll-design.md)
**实施目标**: 实现 D-1 ~ D-5 决策，验证 R1-R6 风险均可接受

---

## File Structure

```
crates/aish-ui/src/components/text_input.rs    （主体改造，单文件）
```

---

## Tasks（顺序，每条独立 commit）

### T1: drag_target_y 字段 + step_drag_auto_scroll vertical 路径

- 新字段：`drag_target_y: Option<Pixels>`（默认 None）
- on_mouse_move handler 同时更新 `drag_target_x` 和 `drag_target_y`（multiline 才 set y，单行 set None 不开新分支）
- `step_drag_auto_scroll` 加 vertical 路径：
  - `if self.multiline`：
    - `vp_top = vb.origin.y`、`vp_bottom = vb.origin.y + vb.size.height`
    - `y > vp_bottom - margin && cursor < text.len()`：cursor_down_visual（扩一 visual line）
    - `y < vp_top + margin && cursor > 0`：cursor_up_visual
  - 单行不变（仅 x 路径）
- mouse_up / window mouse_up 兜底 listener 同步清 `drag_target_y`
- 单测：
  - `step_drag_auto_scroll_vertical_down_when_multiline`
  - `step_drag_auto_scroll_vertical_up_when_multiline`
  - `step_drag_auto_scroll_no_vertical_in_singleline`

**质量门禁**: fmt + clippy + test 通过。

---

### T2: cursor_dirty_for_scroll flag + update_scroll_to_cursor 守门

- 新字段：`cursor_dirty_for_scroll: bool`（默认 true，首次 prepaint 跑一次）
- 所有 cursor 变化路径 set dirty=true：
  - handle_key 的 left / right / up / down / home / end / Backspace / Delete / Enter / insert_str / paste
  - handle_mouse_down_at / cursor drag move（含 step_drag_auto_scroll）
  - set_text / clear
- `update_scroll_to_cursor` 入口检查：
  - `if !self.cursor_dirty_for_scroll { return; }`
  - 跑完后 `self.cursor_dirty_for_scroll = false`
- 单测：
  - `cursor_dirty_set_on_keyboard_nav`
  - `cursor_dirty_cleared_after_scroll_update`

**质量门禁**: 测试覆盖 dirty flag 转换。

---

### T3: wheel handler（multiline 路由 scroll_offset_y）

- multiline 容器加 `.on_scroll_wheel(ev)` listener
  - `ev.delta.y > 0`：scroll up → `scroll_offset_y += line_h`（但 clamp ≤ 0）
  - `ev.delta.y < 0`：scroll down → `scroll_offset_y -= line_h`（clamp ≥ -max_scroll_offset）
  - max_scroll_offset = `(content_lines - max_lines) * line_h`（仅 > 0 时）
  - **不动** cursor / selection / dirty flag
- 单行不接 wheel（保留 GPUI 默认 / 父容器消化）
- 单测：
  - `wheel_in_multiline_scrolls_offset_y_not_cursor`
  - `wheel_clamps_to_max_scroll`
  - `wheel_in_singleline_unaffected`

**质量门禁**: fmt + clippy + 单测。

---

### T4: scrollbar thumb 渲染

- multiline 容器加 absolute 定位的 scrollbar 子元素：
  - 仅 `content_lines > max_lines` 时画
  - 位置：absolute right_0 top_0 w(6) h_full
  - track div: bg muted_foreground/opacity_20 / 圆角 3
  - thumb div: bg muted_foreground / 圆角 3
    - thumb_h = max(20, viewport_h * viewport_h / content_h)
    - thumb_top = -scroll_offset_y * (viewport_h - thumb_h) / max_scroll
  - thumb hover 切 foreground 色
  - thumb on_mouse_down stop_propagation（防冒泡到 input cursor 定位）
- 单测跳过（render 逻辑手测）

**质量门禁**: 手测 — 长内容显示 thumb，短内容不显示，wheel 后 thumb 同步移动。

---

### T5: 文档 + INDEX 更新

- 更新 `docs/superpowers/INDEX.md` 加 M21 entry
- 更新 spec 末尾"已实现"标记 + Risk 实际遇到的偏差
- 写 commits 汇总
- 把 INDEX 当前状态行的 M21 候选移到完成区，"下一里程碑"改 Settings 实质内容 / collapse-orphan-conn

---

## Self-Review Checklist

- [ ] D-1 ~ D-5 决策每条都对应 task
- [ ] Risk R1-R6 在 task 内有 mitigation 落地
- [ ] 单行行为 0 改变（drag_target_y 仅 multiline 路径使用 / wheel 单行不接 / scrollbar 单行不画）
- [ ] aish-ui 测试新增 ≥ 6 个（drag vertical / cursor dirty / wheel）
- [ ] fmt + clippy + test 在每 task commit 前全过
- [ ] commits 严格按 task 顺序，每 task 1 commit

---

## 实施顺序与依赖

```
T1 (drag_target_y + vertical step) ─────┐
                                         ↓
T2 (cursor_dirty flag + scroll 守门) ─┐
                                       ↓
T3 (wheel handler) ←────── T2 dirty flag
                                       ↓
T4 (scrollbar 渲染) ────────────────────┘
                                       ↓
T5 (文档 + INDEX)
```
