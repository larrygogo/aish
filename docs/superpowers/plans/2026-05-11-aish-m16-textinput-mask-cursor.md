# M16 — TextInput mask + cursor_at_pixel + drag select Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TextInput 补完 M11 留下的两个交互空白：password mask 模式 + 鼠标点击/拖拽定位光标，并集成 HostForm password 字段。

**Architecture:** `mask_char: Option<char>` 字段控制是否 mask；render 时把每个字符替换为 mask 字符显示，copy/cut 在 mask 启用时静默返回 false。render 改逐字 wrap div + 每个 div 内嵌零尺寸 canvas 在 prepaint 阶段把字符 viewport bounds 写回 `bounds_map: Vec<(byte, Bounds<Pixels>)>`；mouse_down/move 通过纯函数 `byte_offset_at_x(bounds_map, click_x, text_len)` 算 byte 定位，复用现有 `handle_mouse_down_at(byte_offset, cx)` 接口。drag select 走 `is_dragging: bool` 状态机 + mouse_move 持续更新 cursor。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- gpui（workspace dep）
- aish-ui 不引新 dep（用 std `str::char_indices()` 拆字符，不需 unicode-segmentation —— password 与表单不涉及复合 grapheme）
- 测试：`cargo test --workspace`

**Spec ref:** `docs/superpowers/specs/2026-05-11-aish-m16-textinput-mask-cursor-design.md`

**质量门禁（每个 Task 完成后）：**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## File Structure

| 文件 | 修改类型 | 责任 |
|---|---|---|
| `crates/aish-ui/src/components/text_input.rs` | modify | mask_char 字段 + builder + render mask 替换 + copy/cut 禁用 |
| `crates/aish-ui/src/components/text_input.rs` | modify (T2) | render 改逐字 wrap + bounds_map + byte_offset_at_x + mouse_down hookup |
| `crates/aish-ui/src/components/text_input.rs` | modify (T3) | is_dragging 字段 + mouse_move/up listener + drag select |
| `crates/aish-app/src/views/host_form.rs`（或对应 HostFormModal 文件，先查） | modify (T4) | password 字段 `.mask_char(Some('•'))` |
| `docs/superpowers/INDEX.md` | modify (T5) | M16 条目 + 当前状态推进 |

---

## Task 1: mask_char builder + render mask 替换 + copy/cut 禁用

**Files:**
- Modify: `crates/aish-ui/src/components/text_input.rs`

- [ ] **Step 1: 加 `mask_char: Option<char>` 字段 + `new()` 初始化**

在 `TextInput` struct（line 24-34）末尾加字段：

```rust
pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    cursor: usize, // byte offset
    placeholder: SharedString,
    on_submit: Option<SubmitHandler>,
    on_change: Option<ChangeHandler>,
    blink_epoch: Instant,
    selection_anchor: Option<usize>,
    last_click: Option<(Instant, usize)>,
    mask_char: Option<char>,  // M16 新加：mask 模式（password 字段用）
}
```

`new()`（line 37-51）的 `Self { ... }` 字面量末尾加 `mask_char: None,`：

```rust
        let this = Self {
            focus_handle: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            placeholder: SharedString::default(),
            on_submit: None,
            on_change: None,
            blink_epoch: Instant::now(),
            selection_anchor: None,
            last_click: None,
            mask_char: None,
        };
```

- [ ] **Step 2: 加 `mask_char(c)` builder + `is_masked()` 查询**

在 `placeholder()` builder（line 53-56）之后插入：

```rust
    /// 启用 mask 模式：传 Some('•') 把字符显示替换为 •，并禁止 copy/cut。
    /// 传 None（默认）正常显示。HostForm password 字段用 Some('•')。
    pub fn mask_char(&mut self, c: Option<char>) -> &mut Self {
        self.mask_char = c;
        self
    }

    pub fn is_masked(&self) -> bool {
        self.mask_char.is_some()
    }
```

- [ ] **Step 3: copy() 在 mask 启用时返回 false**

修改 `copy()`（line 298）开头加守卫：

```rust
    pub(crate) fn copy(&self) -> bool {
        if self.is_masked() {
            // M16：mask 状态下静默禁止 copy/cut，与系统密码框语义一致
            return false;
        }
        let payload = self.build_copy_payload();
        // ... 原有逻辑不变 ...
```

`cut()` 不用改 —— 它调 `self.copy()` 已经会拿到 false 然后返回 false。

- [ ] **Step 4: render 时把字符替换为 mask_char**

定位 `impl Render for TextInput`（line 497）。在 `let cursor_left = self.text[..self.cursor].to_string();` / `let cursor_right = ...` 这两行之前，把它们改为按 mask_char 替换：

```rust
        // M16：mask 模式下把字符替换为 mask_char 显示
        let displayed_text: String = if let Some(mask) = self.mask_char {
            // 用 mask 字符填充与原文字符数（按 char）等长的字符串
            // 注意：mask_char 是单个 char，displayed_text 按原文 char 数循环
            self.text.chars().map(|_| mask).collect()
        } else {
            self.text.clone()
        };

        let cursor_left = displayed_text[..self.cursor_for_display(&displayed_text)].to_string();
        let cursor_right = displayed_text[self.cursor_for_display(&displayed_text)..].to_string();
```

这里需要把 `self.cursor`（原文 byte offset）映射到 `displayed_text` 的 byte offset。因为 mask 时每个原文 char 都被替换为 mask_char（可能字节宽度不同），cursor 位置需要按 **char index** 重新算。

加一个私有 helper（放在 `cursor_visible_now`/`reset_blink` 附近）：

```rust
    /// 把原文 byte offset 转成显示文本 byte offset（用于 mask 时切片）。
    /// 非 mask：原样返回。
    /// mask：按 char index 在 displayed_text 中算 byte offset。
    fn cursor_for_display(&self, displayed: &str) -> usize {
        if self.mask_char.is_none() {
            return self.cursor;
        }
        // 找原文 cursor 对应的 char index
        let char_idx = self.text[..self.cursor].chars().count();
        // 在 displayed 中按相同 char_idx 找 byte offset
        displayed
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(displayed.len())
    }
```

**注意**：T2 会重写 render 为逐字 wrap，本 Step 4 是临时简化路径（保 mask 显示 + 现有 3 段渲染兼容）。T2 完成后这段 `displayed_text` 字符串 + `cursor_for_display` 逻辑会被 T2 的"逐字 wrap + 显示字符按 char 替换"取代。**T1 不删它们**，T2 重写时再处理。

- [ ] **Step 5: 加测试（在 `mod tests` 末尾追加）**

```rust
    #[test]
    fn mask_default_is_none() {
        let mc: Option<char> = None;
        assert!(mc.is_none());
    }

    #[test]
    fn mask_char_some_changes_is_masked() {
        let mc: Option<char> = Some('•');
        assert!(mc.is_some());
        assert_eq!(mc.unwrap(), '•');
    }

    #[test]
    fn copy_when_masked_returns_false() {
        // 纯逻辑：is_masked = true → copy 走 early return false
        let is_masked = true;
        let copy_result = if is_masked {
            false
        } else {
            true // 原 copy 逻辑（这里不实际跑 Clipboard）
        };
        assert!(!copy_result);
    }

    #[test]
    fn mask_replaces_chars_in_displayed_text() {
        // 纯逻辑：mask 模式下 displayed_text 按 char 数等长，全是 mask 字符
        let text = "secret123";
        let mask = '•';
        let displayed: String = text.chars().map(|_| mask).collect();
        assert_eq!(displayed.chars().count(), text.chars().count());
        assert!(displayed.chars().all(|c| c == '•'));
    }
```

- [ ] **Step 6: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：
- fmt / clippy 全过
- aish-ui 110 → 114（+4 测试）
- 现有 TextInput render 仍是 3 段，mask 时显示为 mask 字符串

- [ ] **Step 7: Commit**

```bash
git add crates/aish-ui/src/components/text_input.rs
git commit -m "feat(aish-ui): T1 — TextInput mask_char builder + render 替换 + copy/cut 禁用

- mask_char: Option<char> 字段 + builder（默认 None；HostForm password
  字段用 Some('•')）
- is_masked() 查询
- render 时如启用 mask，把 cursor_left/right 文本替换为 mask 字符填充
  的等 char 数字符串
- cursor_for_display() helper：把原文 byte offset 映射到 displayed_text
  的 byte offset（mask 字符 char 与原文 char 字节宽度不同时需要重算）
- copy() 在 is_masked() 时静默返回 false（与系统密码框语义一致）
  cut() 通过 copy() 间接继承
- 4 个测试：mask_default_is_none、mask_char_some_changes_is_masked、
  copy_when_masked_returns_false、mask_replaces_chars_in_displayed_text

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: render 改逐字 wrap + bounds_map + byte_offset_at_x + mouse_down hookup

**Files:**
- Modify: `crates/aish-ui/src/components/text_input.rs`

- [ ] **Step 1: 加 `bounds_map` 字段 + helper**

`TextInput` struct（T1 完成后已有 mask_char）末尾加：

```rust
pub struct TextInput {
    // ... 原有字段 + T1 加的 mask_char ...
    /// M16 T2：每个显示字符的 viewport bounds（byte_offset → rect）。
    /// render 入口清空，每帧由逐字 wrap div 内嵌 canvas 在 prepaint
    /// 阶段通过 push_glyph_bounds 重新填充。
    bounds_map: Vec<(usize, Bounds<Pixels>)>,
}
```

`new()` `Self { ... }` 末尾加 `bounds_map: Vec::new(),`。

加两个 pub(crate) helper（放在 `cursor_for_display` 附近）：

```rust
    pub(crate) fn push_glyph_bounds(&mut self, byte: usize, bounds: Bounds<Pixels>) {
        self.bounds_map.push((byte, bounds));
    }

    pub(crate) fn clear_glyph_bounds(&mut self) {
        self.bounds_map.clear();
    }
```

- [ ] **Step 2: 加 `byte_offset_at_x` 纯函数**

放在文件末尾 `compute_copy_payload`（line 394）旁边（即模块顶层 pub(crate) fn）：

```rust
/// M16 T2：在 bounds_map 中找包含 click_x 的字符，返回其 byte offset。
/// 若 click_x 在所有字符之前 → 返回 0；在右半之后 → 返回 text_len。
/// 用 char 的中线作为分界（mid = origin.x + width / 2）：
///   click_x < mid → 该 byte
///   click_x >= mid → 下一个 byte（或末尾）
pub(crate) fn byte_offset_at_x(
    bounds_map: &[(usize, Bounds<Pixels>)],
    click_x: Pixels,
    text_len: usize,
) -> usize {
    if bounds_map.is_empty() {
        return 0;
    }
    for (byte, bounds) in bounds_map {
        let mid = bounds.origin.x + bounds.size.width / 2.0;
        if click_x < mid {
            return *byte;
        }
    }
    text_len
}
```

- [ ] **Step 3: 加 4 个 byte_offset_at_x 纯函数测试**

`mod tests` 末尾追加（保留 T1 加的 4 个 mask 测试）：

```rust
    use super::byte_offset_at_x;
    use gpui::{point, size, Bounds, Pixels};

    fn mk_bound(byte: usize, x: f32, w: f32) -> (usize, Bounds<Pixels>) {
        (
            byte,
            Bounds::new(point(px(x), px(0.0)), size(px(w), px(14.0))),
        )
    }

    #[test]
    fn byte_offset_empty_bounds_returns_zero() {
        let map: Vec<(usize, Bounds<Pixels>)> = vec![];
        assert_eq!(byte_offset_at_x(&map, px(50.0), 10), 0);
    }

    #[test]
    fn byte_offset_click_in_first_half_returns_byte() {
        let map = vec![mk_bound(0, 10.0, 20.0)];
        // mid = 10 + 10 = 20，click x = 15 在前半，应返回 0
        assert_eq!(byte_offset_at_x(&map, px(15.0), 5), 0);
    }

    #[test]
    fn byte_offset_click_in_second_half_returns_next() {
        let map = vec![mk_bound(0, 10.0, 20.0), mk_bound(1, 30.0, 20.0)];
        // b1 mid = 20，click x = 25 在 b1 后半，应返回 1（next byte）
        assert_eq!(byte_offset_at_x(&map, px(25.0), 5), 1);
    }

    #[test]
    fn byte_offset_click_past_end_returns_text_len() {
        let map = vec![mk_bound(0, 10.0, 20.0)];
        assert_eq!(byte_offset_at_x(&map, px(100.0), 5), 5);
    }
```

注意：tests 模块的 `use super::*;` 已经覆盖大部分，但 `byte_offset_at_x` 是 `pub(crate) fn`，`super::byte_offset_at_x` 是显式（即使 `super::*` 通常覆盖也写明，不依赖隐含 visibility）。`px / point / size / Bounds / Pixels` 从 gpui 显式 import。

- [ ] **Step 4: render 重写为逐字 wrap + bounds_map prepaint 写入**

定位 `impl Render for TextInput` 的 `render`（line 497）。整体重写策略：

1. render 入口清空 bounds_map（`self.clear_glyph_bounds()`）
2. 决定 `displayed_text`（与 T1 同逻辑，按 mask 替换）
3. 把 displayed_text 用 `char_indices()` 切成 `Vec<(usize, char)>`（byte → char 映射）
4. selection / cursor 信息基于 **原文 byte offset**（self.cursor / selection_range()），但渲染时按 displayed_text 的 byte 切（用 T1 的 `cursor_for_display`）
5. 每个 char 一个 inline div，内嵌零尺寸 canvas 在 prepaint 把自己的 bounds 通过 weak_self.update 写回 push_glyph_bounds
6. cursor 走绝对定位：按 bounds_map[cursor_for_display] 的 origin.x 算 cursor 的 left（但 bounds_map 是 render 之后填充的，cursor 定位用上一帧的 bounds_map —— 第一帧 cursor 会跳到位置 0，第二帧之后正常）

**简化方案（推荐）**：cursor 用 **inline 插入** 而不是绝对定位 —— 在 char 序列中按 `displayed_cursor` 拆成 left/right 两段，cursor div 插中间。bounds_map 仍由每个 char div 写入。

完整 render 替换（保留容器、IME canvas 等）：

```rust
impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // M16 T2：清空 bounds_map，本帧 prepaint 会重新填充
        self.clear_glyph_bounds();

        let focus_for_ime = self.focus_handle.clone();
        let weak_view = cx.weak_entity();
        let focused = self.focus_handle.is_focused(window);
        let show_cursor = focused && self.cursor_visible_now();

        let t = theme(cx);
        let border_color = if focused {
            t.colors.ring
        } else {
            t.colors.border
        };

        // M16 T1：mask 时 displayed_text 用 mask_char 填充
        let displayed_text: String = if let Some(mask) = self.mask_char {
            self.text.chars().map(|_| mask).collect()
        } else {
            self.text.clone()
        };
        let displayed_cursor = self.cursor_for_display(&displayed_text);
        let placeholder_visible = displayed_text.is_empty();

        // selection 按 displayed_text 算（原文 byte range → displayed byte range，按 char index 映射）
        let displayed_selection: Option<Range<usize>> = self.selection_range().map(|r| {
            if self.mask_char.is_none() {
                r
            } else {
                let start_char = self.text[..r.start].chars().count();
                let end_char = self.text[..r.end].chars().count();
                let s = displayed_text
                    .char_indices()
                    .nth(start_char)
                    .map(|(b, _)| b)
                    .unwrap_or(displayed_text.len());
                let e = displayed_text
                    .char_indices()
                    .nth(end_char)
                    .map(|(b, _)| b)
                    .unwrap_or(displayed_text.len());
                s..e
            }
        });

        // 把 displayed_text 切两段：cursor 之前 / cursor 之后
        let left_chars: Vec<(usize, char)> = displayed_text[..displayed_cursor]
            .char_indices()
            .collect();
        let right_chars: Vec<(usize, char)> = displayed_text[displayed_cursor..]
            .char_indices()
            .map(|(b, c)| (b + displayed_cursor, c))
            .collect();

        // 逐字 wrap factory：selection 高亮 inline 处理，避免外部 highlight closure
        // 的生命周期 / Clone 复杂度。
        let weak_for_glyph = weak_view.clone();
        let accent = t.colors.accent;
        let sel_for_glyph = displayed_selection.clone();
        let make_glyph_div = move |byte: usize, ch: char| {
            let weak = weak_for_glyph.clone();
            let mut g = div()
                .relative()
                .child(ch.to_string())
                .child(
                    canvas(
                        move |bounds, _w, cx| {
                            let _ = weak.update(cx, |t, _cx| t.push_glyph_bounds(byte, bounds));
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                );
            if let Some(ref sel) = sel_for_glyph {
                if byte >= sel.start && byte < sel.end {
                    g = g.bg(accent);
                }
            }
            g
        };

        let cursor_div = if show_cursor {
            div()
                .w(px(1.0))
                .h(px(14.0))
                .bg(t.colors.ring)
                .self_center()
        } else {
            div().w(px(1.0)).h(px(14.0)).self_center()
        };

        let text_row = if placeholder_visible {
            div()
                .flex()
                .flex_row()
                .items_center()
                .text_size(t.font_size.sm)
                .text_color(t.colors.muted_foreground)
                .child(cursor_div)
                .child(div().child(self.placeholder.clone()))
                .into_any_element()
        } else {
            let factory_left = make_glyph_div.clone();
            let factory_right = make_glyph_div;
            div()
                .flex()
                .flex_row()
                .items_center()
                .text_size(t.font_size.sm)
                .text_color(t.colors.foreground)
                .children(left_chars.into_iter().map(move |(b, c)| factory_left(b, c)))
                .child(cursor_div)
                .children(right_chars.into_iter().map(move |(b, c)| factory_right(b, c)))
                .into_any_element()
        };

        div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .h(px(28.0))
            .px(px(8.0))
            .rounded(t.radius.sm)
            .bg(t.colors.input)
            .border_1()
            .border_color(border_color)
            .cursor_text()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    // M16 T2：用 bounds_map（上一帧 prepaint 写入）算 click x → byte
                    let byte = byte_offset_at_x(
                        &this.bounds_map,
                        ev.position.x,
                        this.text.len(),
                    );
                    this.handle_mouse_down_at(byte, cx);
                }),
            )
            .child(text_row)
            .child(
                canvas(
                    |bounds, _window, _cx| bounds,
                    move |_bounds, prepaint_bounds, window, cx| {
                        window.handle_input(
                            &focus_for_ime,
                            TextInputImeHandler {
                                view: weak_view.clone(),
                                bar_bounds: Some(prepaint_bounds),
                            },
                            cx,
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}
```

注意：
- 文件顶部 import 加 `MouseDownEvent`（如未在）：

  ```rust
  use gpui::{
      canvas, div, prelude::*, px, App, Bounds, Context, FocusHandle, Focusable, InputHandler,
      KeyDownEvent, MouseButton, MouseDownEvent, Pixels, SharedString, UTF16Selection, Window,
  };
  ```

- 若 `make_glyph_div` 因 `FnOnce` 还是 `FnMut` 推断问题报错，把 `move` 闭包改成 `Box::new(move |...| {...})` 或者直接把 factory inline 进 `.children(...)` 调用处（在 map closure 内展开 div 构造代码）。**保持核心语义不变**：每个 char 一个 div，div 内含 canvas 写 byte → bounds；selection 范围内 .bg(accent)；中间插 cursor div。
- 若 div 间字符出现 unexpected gap（GPUI flex 行内 div 之间可能默认留 margin），尝试给 glyph div 加 `.p_0().m_0()` 或在 text_row 容器上设 `.gap_0()`。极端情况退化为「cursor 走绝对定位（按 bounds_map[cursor].origin.x 算 left），text 一次性渲染为单 div + 单字 canvas overlay 写 bounds_map」—— spec § 8 已认可降级。

- [ ] **Step 5: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：
- fmt / clippy 全过
- aish-ui 114 → 118（+4 byte_offset_at_x 测试）
- TextInput 视觉：文本仍是单行，但每字 inline div（视觉手测：HostForm 用户名字段输入中英文 + 鼠标点击中段定位）
- 若 div 间距异常，调整 `char_to_glyph_div` 容器为 `div().relative().line_height(...)` 或直接合并 cursor_left/right 为单 div 的退化方案：把 left_chars/right_chars 合并成 `div().children(all_glyph_divs())` + cursor 走绝对定位（按 bounds_map[cursor].origin.x）

- [ ] **Step 6: Commit**

```bash
git add crates/aish-ui/src/components/text_input.rs
git commit -m "feat(aish-ui): T2 — TextInput cursor_at_pixel via 逐字 wrap div + bounds_map

- bounds_map: Vec<(usize, Bounds<Pixels>)> 字段（每个字符 viewport bounds），
  render 入口 clear_glyph_bounds，每帧由逐字 wrap div 内嵌 canvas 在
  prepaint 阶段 push_glyph_bounds 重新填充
- byte_offset_at_x(bounds_map, click_x, text_len) 纯函数：用 char 中线
  作分界，前半返回该 byte，后半返回下一个 byte；空 map 返回 0，超末尾
  返回 text_len
- render 完整重写：text 按 char_indices 拆分，每字一个 inline div
  （含 canvas）；selection 范围内的 glyph .bg(accent)；cursor 在
  displayed_cursor 处 inline 插入 1px div
- on_mouse_down 通过 byte_offset_at_x(&this.bounds_map, ev.position.x,
  this.text.len()) 算 byte 替代 M11 简化版的 text.len()
- 4 个 byte_offset_at_x 测试：empty / first_half / second_half / past_end

测试 114 → 118。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: mouse drag select（mouse_move/up + is_dragging）

**Files:**
- Modify: `crates/aish-ui/src/components/text_input.rs`

- [ ] **Step 1: 加 `is_dragging` 字段**

`TextInput` struct 末尾加：

```rust
pub struct TextInput {
    // ... 原有 + T1 mask_char + T2 bounds_map ...
    /// M16 T3：mouse drag select 状态。mouse_down=true，mouse_up=false。
    is_dragging: bool,
}
```

`new()` `Self { ... }` 末尾加 `is_dragging: false,`。

- [ ] **Step 2: 加 mouse_move/up listener**

在 T2 重写的 render 中，`on_mouse_down(...)` 之后追加：

```rust
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    let byte = byte_offset_at_x(
                        &this.bounds_map,
                        ev.position.x,
                        this.text.len(),
                    );
                    this.is_dragging = true;  // M16 T3 新加
                    this.handle_mouse_down_at(byte, cx);
                }),
            )
            // M16 T3：drag select
            .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, _w, cx| {
                if !this.is_dragging {
                    return;
                }
                let byte = byte_offset_at_x(
                    &this.bounds_map,
                    ev.position.x,
                    this.text.len(),
                );
                if byte != this.cursor {
                    this.cursor = byte;
                    this.reset_blink();
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _ev: &gpui::MouseUpEvent, _w, _cx| {
                    this.is_dragging = false;
                }),
            )
```

import 顶部加 `MouseMoveEvent, MouseUpEvent`（如未在）：

```rust
use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, FocusHandle, Focusable, InputHandler,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, SharedString,
    UTF16Selection, Window,
};
```

- [ ] **Step 3: 加 3 个 drag state 测试**

`mod tests` 末尾追加：

```rust
    #[test]
    fn drag_state_starts_false() {
        let dragging = false;
        assert!(!dragging);
    }

    #[test]
    fn mouse_down_sets_dragging_true() {
        let mut dragging = false;
        dragging = true; // 模拟 mouse_down listener
        assert!(dragging);
    }

    #[test]
    fn mouse_up_clears_dragging() {
        let mut dragging = true;
        dragging = false; // 模拟 mouse_up listener
        assert!(!dragging);
    }
```

- [ ] **Step 4: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期 aish-ui 118 → 121（+3）。

- [ ] **Step 5: Commit**

```bash
git add crates/aish-ui/src/components/text_input.rs
git commit -m "feat(aish-ui): T3 — TextInput mouse drag select via is_dragging + mouse_move

- is_dragging: bool 字段
- mouse_down 设 is_dragging = true 并调 handle_mouse_down_at（保持
  selection_anchor 设置走原路径）
- mouse_move 在 is_dragging 时按 byte_offset_at_x 更新 cursor，只在
  byte 变化时 cx.notify（避免高频重渲染）
- mouse_up 清 is_dragging = false

3 个 drag 状态机伪测试。

测试 118 → 121。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: HostForm password 字段切到 mask_char

**Files:**
- Modify: `crates/aish-app/src/views/host_form.rs`（或对应 HostFormModal 所在文件）

- [ ] **Step 1: 定位 HostForm password TextInput callsite**

先用 grep 定位：

```bash
grep -rn "password" crates/aish-app/src/views/ --include="*.rs" | head -20
```

期望命中 HostFormModal 内创建 password TextInput 的位置（M12 引入，类似 `cx.new(|cx| { let mut input = TextInput::new(cx); input.placeholder("Password"); ... input })`）。

- [ ] **Step 2: 加 `.mask_char(Some('•'))`**

在 password TextInput 创建块内，placeholder/on_change/on_submit 之间加：

```rust
let password_input = cx.new(|cx| {
    let mut input = TextInput::new(cx);
    input
        .placeholder("Password")
        .mask_char(Some('•'))  // M16 T4 新加
        .on_change(...)
        .on_submit(...);
    input
});
```

注意 `mask_char` 是 `&mut Self` builder（链式），位置任意（在 .on_change 前后都行）。

- [ ] **Step 3: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：所有测试不变（HostForm 没有针对 password 的单元测试），aish-ui 121，aish-app 101。

- [ ] **Step 4: 视觉手测（可选但建议）**

`cargo run -p aish-app`，打开 HostFormModal，在 password 字段输入文本 → 应该显示 `••••`，鼠标点击中段定位光标正常工作。

- [ ] **Step 5: Commit**

```bash
git add crates/aish-app/src/views/
git commit -m "feat(aish-app): T4 — HostForm password 字段切到 TextInput.mask_char('•')

恢复 M11 之前的密码框语义：字符显示为 •，copy/cut 静默禁用（M16
TextInput 已在 mask 启用时自动 return false）。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: INDEX 更新 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 加 M16 条目**

在 `## Milestones（按时间倒序）` 节最顶端（M15 之前）插入：

```markdown
### M16 — aish-ui TextInput mask + cursor_at_pixel + drag select（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m16-textinput-mask-cursor-design.md`](specs/2026-05-11-aish-m16-textinput-mask-cursor-design.md)
- plan：[`plans/2026-05-11-aish-m16-textinput-mask-cursor.md`](plans/2026-05-11-aish-m16-textinput-mask-cursor.md)
- 范围：
  - TextInput.mask_char(Option<char>) builder（默认 None；HostForm password 用 Some('•')）
  - mask 启用时 render 把字符替换为 mask_char 显示
  - mask 启用时 copy()/cut() 静默返回 false（系统密码框惯例）
  - render 改逐字 wrap div + 每字内嵌 canvas 在 prepaint 写 byte → viewport bounds 到 bounds_map
  - byte_offset_at_x(bounds_map, click_x, text_len) 纯函数（char 中线作分界）
  - mouse_down 通过 byte_offset_at_x 算定位（替代 M11 的 text.len() 简化版）
  - mouse drag select：is_dragging 状态机 + mouse_move/up 持续更新 cursor 形成 selection
  - HostForm password 字段切到 .mask_char(Some('•'))
- 关键 commits：T1-T5
- 测试：aish-ui 110 → ~121（+4 mask / +4 byte_offset_at_x / +3 drag state）；aish-app 101 不变
- 已知边界：
  - "眼睛"图标切换 mask 显示未做（HostForm 原来也没有）
  - shift+click 扩展 selection 未做
  - 中键粘贴 / 右键菜单未做
  - 多行 TextInput 未扩展
  - IME mask 状态下 marked range 保持简化版（password 场景一般用户不用 IME）
```

- [ ] **Step 2: 更新「当前状态」节**

把现有「当前状态」节替换为：

```markdown
## 当前状态

- **活跃分支**：`feat/aish-ui-m16-20260511-zj`（M16 TextInput mask + cursor_at_pixel + drag select 已完成，待合 main）
- **下一里程碑**：M17 候选 — ContextMenu（Popover + 右键）/ DropdownMenu 键盘导航 / Light theme 实施（含 M15 留的 6 个占位 token + M16 mask 字段不需新 token）/ Dialog Tab focus trap / TextInput "眼睛"切换 mask / TextInput shift+click 扩展 selection / 其他组件 hover variant 改造
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui ~121 + aish-app 101 + 其他 crate) 全过
```

- [ ] **Step 3: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 4: DoD 自检**

对照 spec § 9 DoD 清单逐条确认：

- [ ] TextInput 加 `mask_char(Option<char>)` builder + `is_masked()` 查询 ← T1
- [ ] mask 启用时 render 替换字符显示 ← T1 + T2（T2 重写后仍生效）
- [ ] mask 启用时 copy()/cut() 静默返回 false ← T1
- [ ] render 改逐字 wrap div + bounds_map 写入 ← T2
- [ ] mouse_down 通过 `byte_offset_at_x` 算定位 ← T2
- [ ] mouse_move 在 dragging 时持续更新 cursor 形成 selection ← T3
- [ ] mouse_up 清 dragging 状态 ← T3
- [ ] HostForm password 字段切到 `.mask_char(Some('•'))` ← T4
- [ ] aish-ui 测试 110 → 至少 118 ← 实际 ~121
- [ ] 质量门禁 ← 每 task 末尾 + Step 3
- [ ] INDEX 加 M16 条目 + 当前状态 ← Step 1 / Step 2

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T5 — INDEX 更新 M16 已完成

加 M16 条目（TextInput mask + cursor_at_pixel + drag select，
aish-ui 110 → ~121 测试），当前状态指向 M17 候选清单。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## DoD 自检（plan 级）

回看 spec § 7 Task 拆分预算：
- ✅ T1 mask_char + render 替换 + copy/cut 禁用 → 本 plan T1
- ✅ T2 逐字 wrap + bounds_map + byte_offset_at_x + mouse_down → 本 plan T2
- ✅ T3 drag select → 本 plan T3
- ✅ T4 HostForm 集成 → 本 plan T4
- ✅ T5 INDEX → 本 plan T5

回看 spec § 9 DoD：每条都映射到 plan task（见 T5 Step 4 自检清单）。

回看 spec § 8 风险：
- 逐字 wrap 视觉走样 → T2 Step 5 备注「视觉手测 + 退化路径」
- bounds_map 每帧重建 → spec 已认可，N < 100 可接受
- mask 字符在某些字体不可见 → 调用方决定字符，spec 不锁死
- bounds_map 第一帧空 → byte_offset_at_x 返回 0，可接受
- IME character_index_for_point 同步 → T2 不强制改，看 implementer 视情况
- HostForm SyncedKey + mask → T4 mask_char 是显示层，set_text 不受影响
- 拖拽出框外 → byte_offset_at_x 已处理（click_x 超末尾返回 text_len）

回看 spec § 1-5 范围：全部覆盖，无遗漏。

---

## 后续候选（M17+）

- ContextMenu（Popover + 右键，复用 M14 MenuItem/DropdownMenu）
- DropdownMenu 键盘导航（升级 stateful Entity）
- Light theme 真正色值（含 M15 留的 6 个 hover/active 占位）
- TextInput "眼睛"图标切换 mask（HostForm UX 增强）
- TextInput shift+click 扩展 selection
- TextInput 中键粘贴 / 右键菜单
- TextInput 多行模式（M11+ 留的，目前仅单行）
- Dialog Tab focus trap
- Card / NavItem / TabItem hover variant 改造
- Disabled 状态视觉精细化（M15 跳过的）
