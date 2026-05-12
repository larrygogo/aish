# M17 — Card / NavItem / TabItem hover 改造 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ColorTokens 加 `accent_active` token，Card / NavItem / TabItem 三个 "accent 风格" 容器组件 hover bg 统一为 accent，并补 `.active()` modifier 设 bg=accent_active 提供 mouse-down 反馈。

**Architecture:** 新 token `accent_active` 在 dark theme 给比 accent **更深**的色（lightness 单调降，与 M15 primary/destructive_active 变亮方向相反，因 accent 系列是容器 hover 不是 action）。三组件 render 复用 GPUI `.hover()` / `.active()` modifier（M15 已验证可用），idle/hover/active 三态 bg 直接 bind。NavItem / TabItem 的 selected (active=true) 路径完全不接管。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- gpui（workspace dep）
- 测试：`cargo test --workspace`

**Spec ref:** `docs/superpowers/specs/2026-05-12-aish-m17-card-nav-tab-hover-design.md`

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
| `crates/aish-ui/src/theme/tokens.rs` | modify | `ColorTokens` 加 `accent_active` 字段 |
| `crates/aish-ui/src/theme/dark.rs` | modify | accent_active 填值 + 1 个单调断言测试 |
| `crates/aish-ui/src/theme/light.rs` | modify | TODO 注释更新含 accent_active |
| `crates/aish-ui/src/components/card.rs` | modify (T2) | on_click 路径加 `.active()` |
| `crates/aish-ui/src/components/nav_item.rs` | modify (T3) | active=false 路径 hover 补 bg + 加 `.active()` + 1 测试 |
| `crates/aish-ui/src/components/tab_item.rs` | modify (T4) | active=false 路径加 `.active()` + 1 测试 |
| `docs/superpowers/INDEX.md` | modify (T5) | M17 条目 + 当前状态推进 |

---

## Task 1: ColorTokens 扩展 + Dark/Light 填值

**Files:**
- Modify: `crates/aish-ui/src/theme/tokens.rs`
- Modify: `crates/aish-ui/src/theme/dark.rs`
- Modify: `crates/aish-ui/src/theme/light.rs`

- [ ] **Step 1: 给 ColorTokens 加 accent_active 字段**

文件 `crates/aish-ui/src/theme/tokens.rs`。M15 加了 6 个字段在 `warning` 之后，最后一个是 `destructive_active`。本 T1 在 `destructive_active` 之后追加：

```rust
pub struct ColorTokens {
    // ... 原有 21 + M15 加的 6 = 27 字段 ...
    pub destructive_hover: Hsla,
    pub destructive_active: Hsla,
    // M17 新加：accent 系列容器按下反馈
    pub accent_active: Hsla,
}
```

- [ ] **Step 2: Dark theme 填 accent_active 色**

文件 `crates/aish-ui/src/theme/dark.rs`。在 M15 的 `destructive_active: hex(0xff9cb5),` 之后追加：

```rust
                // M17 新加：accent 按下反馈，比 accent 更深（lightness 与 M15 系列方向相反）
                accent_active: hex(0x4a7099),
```

参考阶梯：
- accent `#6c91c2` (l~56%)
- accent_active `#4a7099` (l~45%) — **更深**

如果 Step 5 的 `dark_accent_active_is_darker_than_accent` 测试失败，回到这一步调整 hex（保持 hue 接近 accent，仅降 lightness）。

- [ ] **Step 3: Light theme 更新 TODO 注释**

文件 `crates/aish-ui/src/theme/light.rs`。当前 M16 完成后的 TODO 注释覆盖了 M15 的 6 字段。本 T1 把 accent_active 加进 TODO 列表。

Read 文件看现状，把现有 TODO 注释块（在 `impl Theme {` 之前）扩展为：

```rust
// TODO(light-theme): M15/M16/M17 给 ColorTokens 加的 hover/active 字段
// （primary_hover/_active、secondary_hover/_active、destructive_hover/_active、
// accent_active）当前 light() 是 unimplemented! stub，未构造 struct literal，
// 这些字段也没有对应值。下个 light theme milestone 真正实现 light() 时按
// light 配色手挑这些色（dark 那批对照参考在 theme/dark.rs）。
```

（如果现有 TODO 注释措辞略有不同，保持现有句式，仅在字段列表中加 `accent_active`。）

- [ ] **Step 4: 加 1 个单调 lightness 断言测试**

`crates/aish-ui/src/theme/dark.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn dark_accent_active_is_darker_than_accent() {
        let t = Theme::dark();
        // M17：accent_active 比 accent 更深（容器按下"沉下去"，与 M15 系列变亮方向相反）
        assert!(t.colors.accent_active.l < t.colors.accent.l);
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
- aish-ui 121 → 122（dark.rs +1）

若 `dark_accent_active_is_darker_than_accent` 失败，回 Step 2 调整 hex（保持 hue 接近原 accent，仅降 lightness 至 < 0.56）。

- [ ] **Step 6: Commit**

```bash
git add crates/aish-ui/src/theme
git commit -m "feat(aish-ui): T1 — ColorTokens 加 accent_active token + dark 填值 + light 占位

- accent_active: Hsla 字段，与 M15 加的 6 个 hover/active 在同一列
- Dark theme accent #6c91c2 → accent_active #4a7099（lightness ~56% → ~45%，
  方向与 M15 primary/destructive_active 的变亮相反，因 accent 系列是容器
  hover 不是 action 按钮，按下时直觉是\"沉下去\"）
- Light theme TODO 注释加上 accent_active 字段名
- 单调 lightness 断言测试：dark_accent_active_is_darker_than_accent

测试 121 → 122。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Card on_click 加 .active() modifier

**Files:**
- Modify: `crates/aish-ui/src/components/card.rs`

- [ ] **Step 1: 改 on_click 路径加 .active()**

文件 `crates/aish-ui/src/components/card.rs`。定位 `impl RenderOnce for Card` 内 on_click 启用块（line ~108-115）：

```rust
        if let Some(handler) = self.on_click {
            let accent = t.colors.accent;
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(accent))
                .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
        }
```

改为：

```rust
        if let Some(handler) = self.on_click {
            let accent = t.colors.accent;
            let accent_active = t.colors.accent_active;
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(accent))
                .active(move |s| s.bg(accent_active))
                .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
        }
```

注意：closure 内 `accent_active` 用 `move`，每次 render 都会从 `t.colors.accent_active` 取值，与 `accent` 模式对称。

- [ ] **Step 2: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：aish-ui 122 不变（Card 测试为 builder 路径，render 改动不引入新测试）。

- [ ] **Step 3: Commit**

```bash
git add crates/aish-ui/src/components/card.rs
git commit -m "feat(aish-ui): T2 — Card on_click 路径加 .active(accent_active) mouse-down 反馈

on_click 启用时 hover bg=accent（保持现状）+ 新增 active bg=accent_active。
现有 Card callsite（home host card 等）按下时自动获得视觉反馈，无需调整
caller。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: NavItem hover 补 bg + 加 .active()

**Files:**
- Modify: `crates/aish-ui/src/components/nav_item.rs`

- [ ] **Step 1: 改 active=false 路径 hover 补 bg + 加 .active()**

文件 `crates/aish-ui/src/components/nav_item.rs`。定位 `impl RenderOnce for NavItem` 内 active=false 块（line ~129-131）：

```rust
        if !active {
            let hover_fg = t.colors.secondary_foreground;
            el = el.hover(move |s| s.text_color(hover_fg));
        }
```

改为：

```rust
        if !active {
            let hover_fg = t.colors.secondary_foreground;
            let hover_bg = t.colors.accent;
            let active_bg = t.colors.accent_active;
            el = el
                .hover(move |s| s.text_color(hover_fg).bg(hover_bg))
                .active(move |s| s.bg(active_bg));
        }
```

注意：
- `active=true` 路径（line ~125-127 `if active && orientation == NavItemOrientation::Vertical { el = el.bg(t.colors.card); }`）**完全不动**，spec D-5 决策
- hover closure 同时改 `text_color` + `bg`（链式调）
- active closure 只改 `bg`（pressed 时 text_color 沿用 hover 时的 secondary_foreground，因为 GPUI active 状态隐含 hover 状态）

- [ ] **Step 2: 加 1 个测试验证 hover_only_when_inactive**

`crates/aish-ui/src/components/nav_item.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn hover_only_when_inactive() {
        // 验证 NavItem 的 active 字段是否决定 hover 路径分支：
        // active=false → 走 hover bg/text 改变 + active bg
        // active=true → 不接管 hover/active（spec D-5）
        let inactive = NavItem::new("a").active(false);
        let active = NavItem::new("a").active(true);
        assert!(!inactive.active);
        assert!(active.active);
    }
```

这是 builder 路径伪测试（与 codebase 其他 NavItem 测试同 pattern），实际 hover 分支语义由 render 内 `if !active { ... }` 守护，运行时无 unit test 方法测 render 路径。

- [ ] **Step 3: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：aish-ui 122 → 123。

- [ ] **Step 4: Commit**

```bash
git add crates/aish-ui/src/components/nav_item.rs
git commit -m "feat(aish-ui): T3 — NavItem active=false 路径 hover 补 bg(accent) + .active(accent_active)

- 原 hover 只改 text_color(secondary_foreground)，现在叠加 bg(accent)
  与 Card / TabItem 视觉一致
- 新增 .active(|s| s.bg(accent_active)) 提供 mouse-down 反馈
- selected (active=true) 路径完全不动（spec D-5）：sidebar 选中的 nav
  项 hover / 按下时无视觉变化，保持 indicator 条 + bg(card) 不被叠加

测试 +1（hover_only_when_inactive 伪测试），122 → 123。

视觉手测点：sidebar 4 个 NavItem hover 加 bg 后是否过重；若过重在后续
milestone 降级到 secondary token（spec § 8 已记录降级路径）。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: TabItem 加 .active()

**Files:**
- Modify: `crates/aish-ui/src/components/tab_item.rs`

- [ ] **Step 1: 改 active=false 路径加 .active()**

文件 `crates/aish-ui/src/components/tab_item.rs`。定位 `impl RenderOnce for TabItem` 内 active=false 块（line ~92-95）：

```rust
        if !active {
            let hover_bg = t.colors.accent;
            el = el.hover(move |s| s.bg(hover_bg));
        }
```

改为：

```rust
        if !active {
            let hover_bg = t.colors.accent;
            let active_bg = t.colors.accent_active;
            el = el
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
        }
```

注意：`active=true` 路径（line ~73-75 selected bg=secondary）**完全不动**。

- [ ] **Step 2: 加 1 个测试 hover_only_when_inactive**

`crates/aish-ui/src/components/tab_item.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn hover_only_when_inactive() {
        // 验证 TabItem 的 active 字段是否决定 hover 路径分支：
        // active=false → 走 hover bg + active bg
        // active=true → 不接管 hover/active（spec D-5）
        let inactive = TabItem::new("a").active(false);
        let active = TabItem::new("a").active(true);
        assert!(!inactive.active);
        assert!(active.active);
    }
```

- [ ] **Step 3: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期 aish-ui 123 → 124。

- [ ] **Step 4: Commit**

```bash
git add crates/aish-ui/src/components/tab_item.rs
git commit -m "feat(aish-ui): T4 — TabItem active=false 路径加 .active(accent_active)

hover bg=accent（保持现状）+ 新增 active bg=accent_active 提供 mouse-down
反馈。selected (active=true) 路径完全不动（spec D-5）：当前选中的 tab
hover / 按下时保持 bg(secondary) + 底 2px primary 条不变。

测试 +1（hover_only_when_inactive），123 → 124。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: INDEX 更新 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 加 M17 条目**

打开 `docs/superpowers/INDEX.md`。在 `## Milestones（按时间倒序）` 节最顶端（M16 之前）插入：

```markdown
### M17 — aish-ui Card / NavItem / TabItem hover 改造 + accent_active token（2026-05-12）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-12-aish-m17-card-nav-tab-hover-design.md`](specs/2026-05-12-aish-m17-card-nav-tab-hover-design.md)
- plan：[`plans/2026-05-12-aish-m17-card-nav-tab-hover.md`](plans/2026-05-12-aish-m17-card-nav-tab-hover.md)
- 范围：
  - ColorTokens 加 `accent_active` 字段（M15 D-2 决策正式回退）
  - Dark theme accent_active = #4a7099（比 accent #6c91c2 更深 / lightness ~45% vs 56%；与 M15 系列变亮方向**相反**，因 accent 系列是容器 hover 不是 action）
  - Light theme TODO 注释加 accent_active 字段名
  - Card on_click 路径加 `.active(accent_active)` mouse-down 反馈
  - NavItem active=false 路径 hover 补 bg(accent)（与 Card/TabItem 视觉一致）+ `.active(accent_active)`
  - TabItem active=false 路径加 `.active(accent_active)`
  - NavItem / TabItem selected (active=true) 路径完全不动（保持现有 indicator 条 + bg）
- 关键 commits：T1-T5
- 测试：aish-ui 121 → 124（净 +3：dark.rs +1 / nav_item.rs +1 / tab_item.rs +1）；aish-app 101 不变
- 命名 namespace 澄清：token 层 `_active` = "pressed"（GPUI `.active()` modifier）；组件 API 层 `.active(bool)` = "selected"。两个 namespace 在代码中不交叉
- 已知边界：
  - Button / IconButton Ghost variant **未同步**接 accent_active，仍走 hover=accent / active=accent（无区别）。M17 不动，留 M18+ 兑现
  - NavItem hover 补 bg 后 sidebar 4 项视觉较前更"重"，手测后若问题严重可降级到 secondary token
  - selected NavItem / TabItem hover/按下时无视觉变化（D-5 决策内在 trade-off）
  - Light theme 6+1 个新 token 仍占位，真正色值留下个 light theme milestone
```

- [ ] **Step 2: 更新「当前状态」节**

把现有「当前状态」节替换为：

```markdown
## 当前状态

- **活跃分支**：`feat/aish-ui-m17-20260512-zj`（M17 Card / NavItem / TabItem hover 改造 + accent_active 已完成，待合 main）
- **下一里程碑**：M18 候选 — Button/IconButton Ghost variant 接 accent_active（M17 留的）/ ContextMenu（Popover + 右键）/ DropdownMenu 键盘导航 / Light theme 实施（含 M15/M16/M17 共 7 个占位 token）/ Dialog Tab focus trap / TextInput "眼睛"切换 mask / TextInput shift+click 扩展 selection / TextInput 多行 / Disabled 状态视觉精细化
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui 124 + aish-app 101 + 其他 crate) 全过
```

- [ ] **Step 3: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 4: DoD 自检**

对照 spec § 9 DoD 清单逐条确认：

- [ ] ColorTokens 加 `accent_active` 字段 ← T1
- [ ] Dark theme 填值（lightness 比 accent 小）← T1
- [ ] Light theme TODO 注释包含 accent_active ← T1
- [ ] Card on_click 路径加 `.active(|s| s.bg(accent_active))` ← T2
- [ ] NavItem active=false 路径 hover 补 bg(accent) + `.active(|s| s.bg(accent_active))` ← T3
- [ ] TabItem active=false 路径加 `.active(|s| s.bg(accent_active))` ← T4
- [ ] NavItem / TabItem selected 路径完全不动 ← T3 / T4 不改 `if active` 分支
- [ ] dark.rs `dark_accent_active_is_darker_than_accent` 测试通过 ← T1
- [ ] aish-ui 测试 121 → 至少 123（实际 124）← T1/T3/T4
- [ ] 质量门禁全过 ← 每 task 末尾 + Step 3
- [ ] INDEX 加 M17 条目 + 当前状态指向 M18 候选 ← Step 1 / Step 2

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T5 — INDEX 更新 M17 已完成

加 M17 条目（Card / NavItem / TabItem hover + accent_active，aish-ui
121 → 124 测试），当前状态指向 M18 候选清单（首选 Button Ghost 接
accent_active 兑现 M15 D-2 回退）。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## DoD 自检（plan 级）

回看 spec § 7 Task 拆分预算：
- ✅ T1 ColorTokens + Dark + Light → 本 plan T1
- ✅ T2 Card .active() → 本 plan T2
- ✅ T3 NavItem hover bg + .active() + 测试 → 本 plan T3
- ✅ T4 TabItem .active() + 测试 → 本 plan T4
- ✅ T5 INDEX → 本 plan T5

回看 spec § 9 DoD：每条都映射到 plan task（见 T5 Step 4 自检清单）。

回看 spec § 8 风险：
- NavItem hover bg 过重 → T3 commit message 已记录降级路径
- accent_active 方向相反 → T1 Step 4 测试断言方向明确 + spec § 6.1 已记录
- Button/IconButton Ghost 未同步 → T5 INDEX 显式记录
- selected 路径无视觉变化 → T3/T4 设计上不接管，spec D-5

回看 spec § 1-5 范围：全部覆盖。

---

## 后续候选（M18+）

- **Button / IconButton Ghost variant 接 accent_active**（M17 留的，M18 首选兑现 M15 D-2 回退）
- ContextMenu（Popover + 右键）
- DropdownMenu 键盘导航
- Light theme 真正色值（含 M15/M16/M17 共 7 个占位）
- TextInput "眼睛"切换 mask / shift+click 扩展 selection / 多行
- Dialog Tab focus trap
- Disabled 状态视觉精细化
- TextInput cursor_at_pixel + mask 在 multiline 扩展（M16 留）
