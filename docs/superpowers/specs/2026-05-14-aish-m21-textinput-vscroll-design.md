# M21 — TextInput 多行 vertical scrollbar + drag-to-edge auto-scroll

**日期**: 2026-05-14
**父 spec**: [`2026-05-09-aish-ui-architecture-design.md`](2026-05-09-aish-ui-architecture-design.md)
**前置**: [M19 — TextInput 多行](2026-05-14-aish-m19-textinput-multiline-design.md) 完成 multiline / word-wrap / auto-grow
**目标**: 补齐 M19 留的两件 vertical 交互 — 滚动条 UI + drag 拖到上下边沿 auto-scroll
**预计工程量**: ~半天

---

## 1. 动机

M19 落地后 multiline TextInput 在内容超出 `max_lines` 时：
- 用 `overflow_hidden` 裁掉超出部分（视觉断尾）
- cursor 不可见时 `update_scroll_to_cursor` 自动滚到 cursor 行
- **但** 用户无法主动 scroll viewport — 不能 wheel 翻看上文 / 不能 drag 选中跨屏文本

两条具体缺：

1. **drag 拖到上/下边沿没 auto-scroll** — M17 单行水平版本（drag_target_x + 30ms timer）已 OK，多行垂直版本 M19 T5b 留到 M21。
   用户场景：drag select 跨多行长文本，鼠标停在底部边沿，期望持续选中 + 持续滚下。

2. **没 vertical scrollbar UI** — 视觉上不知道有多少内容 / 现在滚到哪里 / 还能滚多少。
   现代 textarea（VS Code chat / Slack / GitHub PR comment）都画一条细 thumb 条作为 affordance。

---

## 2. 决策记录（ADR-style）

### D-1: drag-to-edge 双向并存

**采**：扩 `drag_target_y: Option<Pixels>`，`step_drag_auto_scroll` 同帧检查 x **和** y 两个方向；命中 vertical 边沿时 `cursor_up_visual` / `cursor_down_visual` 扩一行，`update_scroll_to_cursor` 自然把 `scroll_offset_y` 调到让新 cursor 可见。

**拒**：拆两个独立 timer（一个 horizontal 一个 vertical）—— 复用现有 30ms timer + 同一 task 控制更省，drop 时一并 abort。

**单行场景**：horizontal 不变；vertical 走 multiline 路径才生效（`if self.multiline && y 命中 vertical 边沿`）。

### D-2: scrollbar 渲染条件

**采**：multiline=true **且** 内容超出 `max_lines` 才画。
- `content_visual_lines > max_lines`：画
- 否则：不画

**拒**：永远画 placeholder 条 —— 内容短时浪费 6px 横向空间，干扰非滚动场景。

### D-3: scrollbar 视觉

- 位置：input 容器右内边沿，宽 6px，垂直填满 viewport
- track：`colors.border` 半透明（光看半透明意味着可滚）
- thumb：`colors.muted_foreground` + 圆角 3px
- thumb 高 = `viewport_h * (viewport_h / content_h)`，最小 20px 防过短
- thumb top = `-scroll_offset_y * (viewport_h - thumb_h) / max_scroll_offset`
- hover thumb：稍变深（colors.foreground 半透明）
- **不可拖**：M21 仅做可视，drag thumb scrollbar 留 backlog

**拒**：thumb 可拖 —— 实现复杂（需要 capture mouse + 算 ratio + 防止抖动），M21 优先 wheel + drag-to-edge 体感，scrollbar drag 留 backlog。

### D-4: wheel handler

**采**：multiline=true **且** scrollbar 显示时，input 内 wheel → 滚 `scroll_offset_y`，**不动 cursor / selection**。
- wheel down → `scroll_offset_y -= scroll_step`（content 上移 = viewport 看到更下方）
- wheel up → `scroll_offset_y += scroll_step`
- clamp 到 [-max_scroll_offset, 0]
- scroll_step = line_h（每 tick 滚一行）

**拒**：滚 cursor —— 与 textarea 主流不符（VS Code / 浏览器 textarea 都是滚 viewport 不动 cursor）

**关键 trade-off**：wheel 滚走 cursor 后 `update_scroll_to_cursor` 每帧跑会立即滚回。  
**修**：`update_scroll_to_cursor` 加 dirty flag `cursor_changed_since_last_scroll`，cursor 变化时 set true，跑完 scroll 后 set false；wheel 路径不动 cursor → 不 set dirty → 不滚回。

### D-5: scrollbar 不进 bounds_map

scrollbar thumb / track 是装饰元素，mouse_down 在 scrollbar 上不参与 cursor 定位。  
通过把 scrollbar 包在独立 div + `cursor_pointer` + `on_mouse_down(stop_propagation)` 防止冒泡到 input cursor 定位路径（M21 即使 thumb 不可拖也防止误点）。

---

## 3. 架构变化总览

```
+--------------------------------------------------------------+
| 字段新增                                                       |
|   drag_target_y: Option<Pixels>                                |
|   cursor_dirty_for_scroll: bool                                |
+--------------------------------------------------------------+
| 方法新增 / 改造                                                 |
|   step_drag_auto_scroll: + y 路径（multiline 时）              |
|   update_scroll_to_cursor: 检 cursor_dirty 才跑               |
|   handle_wheel(ev): scroll_offset_y 调整（multiline）         |
+--------------------------------------------------------------+
| Render 改造                                                    |
|   multiline 容器右侧：on_scroll_wheel + scrollbar div          |
|   scrollbar div: track + thumb（仅 content > max_lines 时）    |
+--------------------------------------------------------------+
| 不变                                                           |
|   单行 horizontal drag-to-edge / scroll_offset 不动           |
|   bounds_map / cursor 定位 / 键盘 nav 全不动                  |
+--------------------------------------------------------------+
```

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | wheel 与 update_scroll_to_cursor 冲突（滚 viewport 后立即被 cursor 拉回） | 高 | dirty flag 控 — cursor 变化才 set true，wheel 不变化 cursor 不 set，scroll 路径不跑 |
| R2 | scrollbar 占 6px 宽影响 input 宽度计算 / wrap 边界 | 中 | 仅在 content > max_lines 才画；视觉用 `absolute` 定位贴 input 右内沿，不挤压 text_row |
| R3 | drag 同时命中水平 + 垂直边沿（角落） | 低 | step_drag_auto_scroll 串行检查 — 先 x 后 y，两步走，cursor 跳到角落自然 |
| R4 | content 短不显示 scrollbar 时切换到长内容才画，UI 跳变 | 低 | scrollbar 是 absolute 不挤压；显示 / 隐藏切换不引起 reflow |
| R5 | wheel handler 截走 InputBar 容器的 scroll → InputBar / 滚动区域受影响 | 中 | wheel handler 只 stop_propagation 当 scrollbar 可见（仍有空间滚），否则 bubble 让父消化 |
| R6 | drag thumb 不能拖 — 用户可能尝试拖 | 低 | M21 仅可视，cursor 用 `default` 不引诱 drag；backlog 列出 |

---

## 5. Out of scope（M21 不做）

- thumb 可 drag 改 scroll_offset_y
- thumb auto-hide（无 hover 渐隐）
- horizontal scrollbar（multiline word-wrap 不需要，单行已有 ml(scroll_offset) 隐式滚）
- 平滑滚动动画
- scroll-into-view animation（仍是 jump）

---

## 6. 测试策略

### 单测（aish-ui）

- `step_drag_auto_scroll_vertical_up` / `step_drag_auto_scroll_vertical_down`：multiline + drag_target_y 在边沿 → cursor 上下扩一行
- `step_drag_auto_scroll_horizontal_unaffected_in_multiline`：multiline 模式 drag_target_x 只触发水平（不应跳出 multiline 行）
- `wheel_in_multiline_scrolls_offset_y_not_cursor`：wheel 后 cursor 不变，scroll_offset_y 变
- `update_scroll_to_cursor_skips_when_clean`：cursor 不变时不 update（dirty flag = false）
- `cursor_dirty_set_on_keyboard_nav`：键盘 ↑/↓ 后 dirty = true
- `scrollbar_thumb_dim` 不做（render 逻辑手测）

### 集成（手测）

- 多行 input 内容超 max_lines → 右侧出现 scrollbar thumb
- wheel down → thumb 下移 + 内容上移，cursor 不动
- wheel up → 反向
- drag select 跨多行 + 鼠标停在底部边沿 → 持续选中 + 持续滚下
- 键盘 ↓ 跨行让 cursor 进屏外 → viewport 自动滚到 cursor 行

---

## 7. Plan 引用

见 [`../plans/2026-05-14-aish-m21-textinput-vscroll.md`](../plans/2026-05-14-aish-m21-textinput-vscroll.md)
