# M15 — Button + IconButton 精细化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 aish-ui 的 `Button` / `IconButton` 加 hover / active 按 variant 分色，以及可选 focus ring 接 Tab focus。

**Architecture:** `ColorTokens` 加 6 个 named hover/active token（primary / secondary / destructive 各一对），Ghost 保持现状用 `accent`。`Button` / `IconButton` render 内 match variant 取对应 idle/hover/active 三态 bg，配合 GPUI 的 `.hover()` / `.active()` modifier。Focus ring 通过 caller 可选传入 `FocusHandle` + 在 stateful element 上挂 `.track_focus()` 触发，render 时根据 `handle.is_focused(window)` 决定是否加外圈视觉。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- gpui（workspace dep）
- 测试：`cargo test --workspace`，每文件 `#[cfg(test)] mod tests`

**Spec ref:** `docs/superpowers/specs/2026-05-11-aish-m15-button-polish-design.md`

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
| `crates/aish-ui/src/theme/tokens.rs` | modify | `ColorTokens` 加 6 字段 |
| `crates/aish-ui/src/theme/dark.rs` | modify | Dark theme 填 6 个新色 + 测试 |
| `crates/aish-ui/src/theme/light.rs` | modify | Light theme 6 个新字段占位（=dark 值，标 TODO） |
| `crates/aish-ui/src/components/button.rs` | modify | hover/active per variant + 可选 focus_handle + 测试 |
| `crates/aish-ui/src/components/icon_button.rs` | modify | 同步上面改造 + 测试 |
| `docs/superpowers/INDEX.md` | modify | M15 条目 + 当前状态 |

---

## Task 1: ColorTokens 扩展 + Dark/Light 填值

**Files:**
- Modify: `crates/aish-ui/src/theme/tokens.rs`（加 6 字段）
- Modify: `crates/aish-ui/src/theme/dark.rs`（填值 + 测试）
- Modify: `crates/aish-ui/src/theme/light.rs`（占位）

- [ ] **Step 1: 给 ColorTokens 加 6 字段**

文件 `crates/aish-ui/src/theme/tokens.rs`，把现有 `pub warning: Hsla,` 这一行之后追加 6 字段。最终 `ColorTokens` 结构如下（仅展示新加部分，前 21 字段不动）：

```rust
pub struct ColorTokens {
    // ... 原 21 个字段保持原顺序 ...
    pub success: Hsla,
    pub warning: Hsla,
    // 新加：按 variant 的 hover / active 状态色
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub destructive_hover: Hsla,
    pub destructive_active: Hsla,
}
```

- [ ] **Step 2: Dark theme 填 6 个新色**

文件 `crates/aish-ui/src/theme/dark.rs`，把现有 `warning: hex(0xe0af68),` 之后追加 6 行。最终 `Theme::dark()` 中 `ColorTokens { ... }` 字面量为（仅展示新加，前面 21 个不动）：

```rust
        Self {
            colors: ColorTokens {
                // ... 原有字段 ...
                success: hex(0x9ece6a),
                warning: hex(0xe0af68),
                // M15 新加：Tokyo Night 阶梯
                primary_hover: hex(0x4a6ab3),
                primary_active: hex(0x5a7bc8),
                secondary_hover: hex(0x3a3a52),
                secondary_active: hex(0x444460),
                destructive_hover: hex(0xff8aa1),
                destructive_active: hex(0xff9cb5),
            },
            // ...
        }
```

- [ ] **Step 3: Light theme 填占位（=dark 同值，标 TODO）**

先 `Read` `crates/aish-ui/src/theme/light.rs` 看现状。如果文件存在且有 `Theme::light()` 实现（可能是 `unimplemented!()` stub 也可能有部分值），保 6 个新字段 = dark 同值。如果是 `unimplemented!()` 整体 stub，那 6 个新字段进不到 struct 字面量里，但**还是要在文件里加 `// TODO(light-theme): M15 占位，6 个 hover/active 字段在真正 light 配色时手挑`** 注释，作为未来 light theme milestone 的提醒锚。

具体编辑策略：
- 若 light.rs 有真实 `ColorTokens { ... }` 字面量 → 加 6 个新字段（=dark 值）+ 顶部注释
- 若 light.rs 是 stub（`unimplemented!()`）→ 仅加顶部 TODO 注释

注释模板（无论哪种）：

```rust
// TODO(light-theme): 以下 6 个 M15 新加字段（primary_hover/_active、
// secondary_hover/_active、destructive_hover/_active）当前 = dark 同值
// 占位，下个 light theme milestone 真正手挑配色时替换。
```

- [ ] **Step 4: 给 dark.rs 加 6 个新测试**

在 `crates/aish-ui/src/theme/dark.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn dark_primary_hover_is_lighter_than_primary() {
        let t = Theme::dark();
        assert!(t.colors.primary_hover.l > t.colors.primary.l);
    }

    #[test]
    fn dark_primary_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.primary_active.l > t.colors.primary_hover.l);
    }

    #[test]
    fn dark_secondary_hover_is_lighter_than_secondary() {
        let t = Theme::dark();
        assert!(t.colors.secondary_hover.l > t.colors.secondary.l);
    }

    #[test]
    fn dark_secondary_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.secondary_active.l > t.colors.secondary_hover.l);
    }

    #[test]
    fn dark_destructive_hover_is_lighter_than_destructive() {
        let t = Theme::dark();
        assert!(t.colors.destructive_hover.l > t.colors.destructive.l);
    }

    #[test]
    fn dark_destructive_active_is_lighter_than_hover() {
        let t = Theme::dark();
        assert!(t.colors.destructive_active.l > t.colors.destructive_hover.l);
    }
```

- [ ] **Step 5: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：
- fmt / clippy 全过
- aish-ui 测试 100 → 106（dark.rs +6）
- 旧有 Button / IconButton 测试不变（render 路径还没改）

如果某个 `dark_*_is_lighter_*` 测试失败，**说明 Step 2 选的 hex 值 HSL lightness 不单调递增**。回到 Step 2 调整 hex 值（用在线 HSL picker 把 lightness 阶梯化），重跑测试，直到通过。

- [ ] **Step 6: Commit**

```bash
git add crates/aish-ui/src/theme
git commit -m "feat(aish-ui): T1 — ColorTokens 加 6 个 hover/active 状态色 + dark 填值 + light 占位

Tokyo Night 阶梯：
- primary  #3d59a1 → hover #4a6ab3 → active #5a7bc8
- secondary #2d2d3f → hover #3a3a52 → active #444460
- destructive #f7768e → hover #ff8aa1 → active #ff9cb5

Dark theme 加 6 个单调 lightness 断言测试。Light theme 6 个字段
占位 = dark 同值，加 TODO 注释，下个 light theme milestone 真正手挑。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Button hover/active per variant + focus_handle

**Files:**
- Modify: `crates/aish-ui/src/components/button.rs`

- [ ] **Step 1: 给 Button 加 focus_handle 字段 + builder**

文件 `crates/aish-ui/src/components/button.rs`。把 `Button` struct 加一个 `focus_handle: Option<FocusHandle>` 字段（在最后）：

```rust
use gpui::{
    div, prelude::*, App, ElementId, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
    SharedString, Window,
};
// ↑ 加 FocusHandle 到 import 列表

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<ClickHandler>,
    focus_handle: Option<FocusHandle>,  // 新加
}
```

`Button::new()` 内初始化 `focus_handle: None`：

```rust
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            variant: ButtonVariant::Primary,
            disabled: false,
            on_click: None,
            focus_handle: None,  // 新加
        }
    }
```

在 `on_click` builder 之后加 `focus_handle` builder：

```rust
    pub fn focus_handle(mut self, h: FocusHandle) -> Self {
        self.focus_handle = Some(h);
        self
    }
```

- [ ] **Step 2: 改 Button::render 实现 hover/active per variant**

把现有 `impl RenderOnce for Button` 的 `render` 函数体替换为：

```rust
impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;

        // (idle_bg, hover_bg, active_bg, fg)
        let (idle_bg, hover_bg, active_bg, fg) = if disabled {
            (
                t.colors.muted,
                t.colors.muted,
                t.colors.muted,
                t.colors.muted_foreground,
            )
        } else {
            match self.variant {
                ButtonVariant::Primary => (
                    t.colors.primary,
                    t.colors.primary_hover,
                    t.colors.primary_active,
                    t.colors.primary_foreground,
                ),
                ButtonVariant::Secondary => (
                    t.colors.secondary,
                    t.colors.secondary_hover,
                    t.colors.secondary_active,
                    t.colors.secondary_foreground,
                ),
                ButtonVariant::Destructive => (
                    t.colors.destructive,
                    t.colors.destructive_hover,
                    t.colors.destructive_active,
                    t.colors.destructive_foreground,
                ),
                ButtonVariant::Ghost => (
                    gpui::transparent_black(),
                    t.colors.accent,
                    t.colors.accent,
                    t.colors.foreground,
                ),
            }
        };

        let ring = t.colors.ring;
        let is_focused = self
            .focus_handle
            .as_ref()
            .map(|h| h.is_focused(window))
            .unwrap_or(false);

        let mut el = div()
            .id(self.id)
            .h(t.spacing.px_3 + t.spacing.px_4) // ~28
            .px(t.spacing.px_3)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.md)
            .bg(idle_bg)
            .child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(fg)
                    .child(self.label),
            );

        if let Some(handle) = &self.focus_handle {
            el = el.track_focus(handle);
        }

        if is_focused {
            // outer 2px ring：用 box_shadow 实现外圈
            el = el.shadow(vec![gpui::BoxShadow {
                color: ring,
                offset: gpui::point(gpui::px(0.0), gpui::px(0.0)),
                blur_radius: gpui::px(0.0),
                spread_radius: gpui::px(2.0),
            }]);
        }

        if !disabled {
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
            if let Some(handler) = self.on_click {
                el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
            }
        }

        el
    }
}
```

注意：
- `render` 签名第一个参数从 `_window` 改成 `window: &mut Window`（要传给 `handle.is_focused(window)`）
- 若 gpui 实际 `.active(closure)` API 名称不同（如 `.on_active()`、`.pressed()` 之类），调整 callsite 即可，不动 spec 含义
- 若 `gpui::BoxShadow` 不可达 / 字段名不同，**降级方案**：用 `el.border_2().border_color(ring)` 把 button border 改成 ring 色（视觉略缩内，但保证功能可用）。降级时把 spread_radius 那段替换为：
  ```rust
  if is_focused {
      el = el.border_2().border_color(ring);
  }
  ```

- [ ] **Step 3: 加 2 个新测试**

`FocusHandle` 不能在 unit test 里直接构造（需要 `cx.focus_handle()`），所以测试只验证字段默认值 + 字段类型存在。

在 `mod tests` 末尾追加（如 `super::*` 没 re-export `FocusHandle`，需要显式 `use gpui::FocusHandle;` 到 tests 顶部）：

```rust
    #[test]
    fn focus_handle_default_none() {
        let b = Button::new("a");
        assert!(b.focus_handle.is_none());
    }

    #[test]
    fn focus_handle_field_exists() {
        // 验证字段路径可访问；真实 handle 由集成测试或 caller 提供
        let b = Button::new("a");
        let _: &Option<FocusHandle> = &b.focus_handle;
    }
```

- [ ] **Step 4: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：
- aish-ui 106 → 108（button.rs +2）
- 如果 gpui `.active()` API 找不到：clippy/rustc 报 method not found，回 Step 2 调整 API 调用
- 如果 `BoxShadow` 字段名不同：rustc 报 field not found，回 Step 2 走「降级方案」

- [ ] **Step 5: Commit**

```bash
git add crates/aish-ui/src/components/button.rs
git commit -m "feat(aish-ui): T2 — Button hover/active 按 variant 分色 + 可选 focus ring

idle/hover/active 三态 bg：
- Primary → primary / primary_hover / primary_active
- Secondary → secondary / secondary_hover / secondary_active
- Destructive → destructive / destructive_hover / destructive_active
- Ghost → transparent / accent / accent（不动）
- Disabled (任意 variant) → muted（无 hover/active）

新加 focus_handle 可选 builder，caller 传入后 render 检查
handle.is_focused(window)，true 时画 2px outer ring（box_shadow，
spread_radius 2px，color t.colors.ring）。现有 callsite 不传 handle
即向后兼容，不显示 ring。

测试 +2（focus_handle_default_none、focus_handle_field_exists）。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: IconButton 同步处理

**Files:**
- Modify: `crates/aish-ui/src/components/icon_button.rs`

- [ ] **Step 1: 加 focus_handle 字段 + builder**

文件 `crates/aish-ui/src/components/icon_button.rs`。把 `FocusHandle` 加到 import：

```rust
use gpui::{
    div, prelude::*, px, App, ElementId, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
    Pixels, Window,
};
```

`IconButton` struct 加字段：

```rust
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon_name: IconName,
    variant: ButtonVariant,
    size: IconButtonSize,
    disabled: bool,
    on_click: Option<ClickHandler>,
    focus_handle: Option<FocusHandle>,  // 新加
}
```

`IconButton::new()` 初始化 `focus_handle: None`。在 `on_click` builder 之后加：

```rust
    pub fn focus_handle(mut self, h: FocusHandle) -> Self {
        self.focus_handle = Some(h);
        self
    }
```

- [ ] **Step 2: 改 IconButton::render**

替换现有 `impl RenderOnce for IconButton` 的 `render` 函数体为：

```rust
impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;

        let (idle_bg, hover_bg, active_bg, fg) = if disabled {
            (
                t.colors.muted,
                t.colors.muted,
                t.colors.muted,
                t.colors.muted_foreground,
            )
        } else {
            match self.variant {
                ButtonVariant::Primary => (
                    t.colors.primary,
                    t.colors.primary_hover,
                    t.colors.primary_active,
                    t.colors.primary_foreground,
                ),
                ButtonVariant::Secondary => (
                    t.colors.secondary,
                    t.colors.secondary_hover,
                    t.colors.secondary_active,
                    t.colors.secondary_foreground,
                ),
                ButtonVariant::Destructive => (
                    t.colors.destructive,
                    t.colors.destructive_hover,
                    t.colors.destructive_active,
                    t.colors.destructive_foreground,
                ),
                ButtonVariant::Ghost => (
                    gpui::transparent_black(),
                    t.colors.accent,
                    t.colors.accent,
                    t.colors.foreground,
                ),
            }
        };

        let ring = t.colors.ring;
        let is_focused = self
            .focus_handle
            .as_ref()
            .map(|h| h.is_focused(window))
            .unwrap_or(false);

        let bs = self.size.box_size();
        let isz = self.size.icon_size();

        let mut el = div()
            .id(self.id)
            .w(bs)
            .h(bs)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.sm)
            .bg(idle_bg)
            .child(icon(self.icon_name).size(isz).text_color(fg));

        if let Some(handle) = &self.focus_handle {
            el = el.track_focus(handle);
        }

        if is_focused {
            el = el.shadow(vec![gpui::BoxShadow {
                color: ring,
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(2.0),
            }]);
        }

        if !disabled {
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
            if let Some(handler) = self.on_click {
                el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
            }
        }

        el
    }
}
```

注：与 Button 相同 API 假设。若需降级（`.active()` 或 `BoxShadow` API 不可用），在 T2 已用同方案，沿用即可。

- [ ] **Step 3: 加 2 个测试**

在 `mod tests` 末尾追加：

```rust
    #[test]
    fn focus_handle_default_none() {
        let b = IconButton::new("close", IconName::X);
        assert!(b.focus_handle.is_none());
    }

    #[test]
    fn focus_handle_field_exists() {
        let b = IconButton::new("close", IconName::X);
        let _: &Option<gpui::FocusHandle> = &b.focus_handle;
    }
```

- [ ] **Step 4: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期 aish-ui 108 → 110（icon_button.rs +2）。

- [ ] **Step 5: Commit**

```bash
git add crates/aish-ui/src/components/icon_button.rs
git commit -m "feat(aish-ui): T3 — IconButton 同步 hover/active per variant + 可选 focus ring

与 Button (T2) 完全一致的 idle/hover/active 三态 bg per variant，
Ghost 不动用 accent，Disabled 用 muted 无 hover/active。

focus_handle 可选 builder + track_focus + box_shadow 2px ring。

测试 +2。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: INDEX 更新 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 加 M15 条目**

打开 `docs/superpowers/INDEX.md`。在 `## Milestones（按时间倒序）` 节最顶端（M14 之前）插入：

```markdown
### M15 — aish-ui Button + IconButton 精细化（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m15-button-polish-design.md`](specs/2026-05-11-aish-m15-button-polish-design.md)
- plan：[`plans/2026-05-11-aish-m15-button-polish.md`](plans/2026-05-11-aish-m15-button-polish.md)
- 范围：
  - ColorTokens 加 6 个 hover/active 状态色（primary/secondary/destructive 各一对）
  - Dark theme 填 Tokyo Night 阶梯（lightness 单调递增 idle → hover → active）
  - Light theme 6 个新字段占位（=dark 值，加 TODO 注释，下个 light theme milestone 真正手挑）
  - Button hover/active 按 variant 分色（Ghost 仍用 accent；Disabled 用 muted 无 hover/active）
  - Button 加可选 focus_handle(handle) builder，caller 传入后 render 检查 handle.is_focused 画 2px outer ring（box_shadow + ring 色）
  - IconButton 同步处理
- 关键 commits：T1-T4
- 测试：aish-ui 100 → 110（dark.rs +6 单调 lightness 断言 / Button +2 / IconButton +2）
- 已知边界：
  - Ghost variant hover/active 未拆 token（用 accent 单色）
  - Disabled 状态视觉不精细化（保持 muted bg + muted_foreground）
  - Light theme 6 个新 token 仅占位，真正配色留 light theme milestone
  - focus ring 不区分键鼠 focus 路径（focus-visible 留 backlog）
  - 现有 Button / IconButton callsite 不传 focus_handle，向后兼容；具体接入由后续 milestone 在需要的场景按需做
  - 若 GPUI 的 `.active()` modifier 或 `BoxShadow` API 不可用，T2/T3 已记录降级路径（active 退用 hover 同色 / ring 用 border_color 替代）
```

- [ ] **Step 2: 更新「当前状态」节**

把现有「当前状态」节替换为：

```markdown
## 当前状态

- **活跃分支**：`feat/aish-ui-m15-20260511-zj`（M15 Button + IconButton 精细化已完成，待合 main）
- **下一里程碑**：M16 候选 — ContextMenu（Popover + 右键） / DropdownMenu 键盘导航 / Light theme 实施 / TextInput mask + cursor_at_pixel / Dialog Tab focus trap
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui 110 + aish-app 101 + 其他 crate) 全过
```

- [ ] **Step 3: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：全过，aish-ui 110，workspace 整体不退化。

- [ ] **Step 4: DoD 自检**

回看 spec § 9 DoD 清单，逐条确认：

- [ ] ColorTokens 加 6 字段 ← T1 完成
- [ ] Dark theme 填色 ← T1 完成
- [ ] Light theme 占位 ← T1 完成
- [ ] Button hover/active 按 variant 取 token，Ghost 保持现状 ← T2 完成
- [ ] IconButton 同步处理 ← T3 完成
- [ ] Button + IconButton 加可选 focus_handle builder，传入后渲染 ring ← T2 / T3 完成
- [ ] aish-ui 测试 100 → 至少 108 ← 实际 110，达成
- [ ] 质量门禁全过 ← 每 task 末尾 + Step 3 二次确认
- [ ] INDEX.md 加 M15 条目 + 更新当前状态 ← Step 1 / Step 2 完成

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T4 — INDEX 更新 M15 已完成

加 M15 条目（Button + IconButton 精细化，aish-ui 100 → 110 测试），
当前状态指向 M16 候选清单。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## DoD 自检（plan 级）

回看 spec § 7 Task 拆分预算：
- ✅ T1 ColorTokens + Dark/Light + 测试 → 本 plan T1
- ✅ T2 Button → 本 plan T2
- ✅ T3 IconButton → 本 plan T3
- ✅ T4 INDEX → 本 plan T4

回看 spec § 9 DoD：每条都映射到 plan task（见 T4 Step 4 自检清单）。

回看 spec § 8 风险：
- GPUI `.active()` API → T2 Step 4 + Step 2 标注「若不存在调整 callsite」
- focus ring 实现 → T2 Step 2 给了 box_shadow 主路径 + border 降级
- Light theme 占位 → T1 Step 3 处理
- Tokyo Night hex 由 implementer 挑 → T1 Step 2 给参考值 + Step 5 测试只断言 lightness 单调（不卡死具体 hex）
- callsite 不动 → 全 plan 未修改任何 callsite，向后兼容

---

## 后续候选（M16+）

- **ContextMenu**（Popover + 右键，复用 M14 MenuItem/DropdownMenu）
- **DropdownMenu 键盘导航**（升级 stateful Entity）
- **Light theme 真正色值**（填本 milestone 的 6 个占位 token，配新 light 配色）
- **TextInput mask 模式**（HostForm password 字段恢复隐藏）
- **TextInput cursor_at_pixel**（鼠标点击定位光标）
- **Dialog Tab focus trap**（M12 留的；可顺便给 Dialog 内 Button 接 focus_handle）
- **Disabled 状态精细化**（M15 跳过的）
- **Card / NavItem / TabItem hover variant 改造**（其他组件 hover 也走 named token）
