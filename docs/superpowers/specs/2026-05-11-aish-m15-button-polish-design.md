# M15 — Button + IconButton 精细化（hover/active 分 variant + focus ring）Spec

> 状态：草案（待用户审）
> 父 spec：[`2026-05-09-aish-ui-architecture-design.md`](2026-05-09-aish-ui-architecture-design.md)
> 关联：M11 起点（Button/IconButton 初版）、M14 收尾 review 留下的 backlog 项

---

## 1. 目标

aish-ui 的 `Button` / `IconButton` 当前所有 variant 共用同一 hover 色（`t.colors.accent`），按下也无视觉反馈，键盘 Tab 进焦点没有 ring 提示。M15 把这块视觉规范补齐：

- **hover/active 按 variant 分色**：Primary / Secondary / Destructive 用各自的 hover / active 阶梯；Ghost 保持现状（用 `accent` 单色）
- **focus ring**：caller 显式传 `FocusHandle` 时画 outer 2px ring（`t.colors.ring` 色）
- **disabled 视觉**：本里程碑不动

---

## 2. 范围 & 不做事项

### 范围内

- `crates/aish-ui/src/theme/tokens.rs`：`ColorTokens` 加 6 个字段
- `crates/aish-ui/src/theme/dark.rs`：填新字段的 Tokyo Night 色阶值
- `crates/aish-ui/src/theme/light.rs`：新字段占位（复制 dark 值，标 TODO）
- `crates/aish-ui/src/components/button.rs`：hover/active per-variant + 可选 FocusHandle
- `crates/aish-ui/src/components/icon_button.rs`：同步上面的改造
- 测试：每个组件 +N unit test
- `INDEX.md`：M15 条目 + 状态推进

### 不做事项

- **Disabled 状态视觉精细化**（保持现 muted bg + muted_foreground）
- **Ghost variant hover/active 改造**（继续用 `accent` 单色，避免再加 2 个 token）
- **Light theme 真正色值**（仅占位复制 dark；下个 light theme milestone 真正填）
- **focus-visible 区分键鼠路径**（鼠标 click 拿到 focus 也会显示 ring；shadcn 默认行为）
- **其他组件 hover/active 改造**（Card on_click hover / NavItem / TabItem 等不动；以后 visual polish milestone 统一）

---

## 3. ADR-style 决策记录

### D-1：用显式 token 而非算法 darken

**决策**：在 `ColorTokens` 加 6 个具名字段，由 Theme 显式给值。

**考虑过**：
- A) 算法 `Hsla::darken(f)` helper — token 表精简，light theme 也不用复制
- B) 显式 6 个 token ✅ 选中
- C) 混合（destructive 显式，其他算法）

**理由**：
- aish 用 Tokyo Night 调色板，destructive `#f7768e` 是粉红色，**算法 darken 会偏暗紫**（h 不变 + l 降低），不符合"按下时更刺激更红"的预期。设计师挑色更可控
- token 表 21 → 27 不算严重膨胀；M14 已建立"用 named token"风格
- light theme 多复制 6 行是一次性成本，下个 milestone 真正填值时也是手挑

### D-2：Ghost variant 不加 token

**决策**：Ghost 的 hover/active 继续用 `t.colors.accent` 单色。

**理由**：
- Ghost idle 是透明，"hover 时变 accent" 已经形成足够对比；按下时再加深 accent 视觉收益弱
- 加 `accent_hover` / `accent_active` 会让 token 表更乱（accent 本身就是给 Ghost 用的）；不如保持简单

### D-3：Button focus_handle 走可选传入

**决策**：`Button::focus_handle(FocusHandle)` builder 方法可选，不传不画 ring。

**考虑过**：
- A) Button 升级为 Entity 持 FocusHandle — 大改，所有 callsite 都要适配 cx.new(...)
- B) 可选传入 ✅ 选中
- C) 完全不做 focus ring，留 backlog — 用户 brainstorm 时选了全套，舍弃

**理由**：
- Button 当前所有 callsite 不需要 focus（Enter 提交 / 鼠标点击为主）
- 可选传入 = 向后兼容现有 callsite + 给少数需要 Tab focus 的场景（如 Dialog 内表单按钮）开口

### D-4：Light theme 占位 = 复制 dark 值

**决策**：M15 在 `light.rs` 给 6 个新 token 填 dark 同值，加 TODO 注释。下个 light theme milestone 真正手挑 light 配色。

**理由**：避免本 milestone 范围膨胀；光看 light theme 也没人在用（现状还是 `unimplemented!()` stub）。

---

## 4. 视觉规范

### 4.1 Button / IconButton 状态色

| Variant | idle bg | hover bg | active bg | text (idle/hover/active 同) |
|---|---|---|---|---|
| Primary | `primary` | `primary_hover` | `primary_active` | `primary_foreground` |
| Secondary | `secondary` | `secondary_hover` | `secondary_active` | `secondary_foreground` |
| Destructive | `destructive` | `destructive_hover` | `destructive_active` | `destructive_foreground` |
| Ghost | `transparent` | `accent` | `accent` | `foreground` |
| Disabled (任意 variant) | `muted` | — | — | `muted_foreground` |

### 4.2 Dark theme 6 个新色（Tokyo Night 阶梯）

```
primary           #3d59a1   (h=222°, l=44%)  现有
primary_hover     #4a6ab3   (h=222°, l=50%)  亮 6%
primary_active    #5a7bc8   (h=222°, l=57%)  再亮 7%

secondary         #2d2d3f   (h=240°, l=21%)  现有
secondary_hover   #3a3a52   (h=240°, l=27%)
secondary_active  #444460   (h=240°, l=32%)

destructive       #f7768e   (h=350°, l=72%)  现有
destructive_hover #ff8aa1   (h=350°, l=78%)
destructive_active #ff9cb5  (h=350°, l=83%)
```

**注**：具体 hex 由 implementer 用 HSL 阶梯调整；spec 仅给方向。validation 测试只断言「hover.l > idle.l, active.l > hover.l」（dark 主题 hover 比 idle **亮**，active 更亮）。

### 4.3 Focus Ring

- 仅 `Button::focus_handle(handle)` / `IconButton::focus_handle(handle)` 传入后启用
- 视觉：outer 2px ring，色 `t.colors.ring`，与 button 外缘紧贴或留 1px offset（视实现 API 而定）
- Button / IconButton 当前**无 border**，所以 ring 独立画在 button 外侧，不与 border 冲突
- GPUI 实现路径：在 stateful element 上挂 `.track_focus(&handle)`，render 内根据 `handle.is_focused(window)` 通过 `box-shadow` 或类似 API 画 2px 外圈
- **实现备选**：若 GPUI 的 outline/shadow API 实现外圈 ring 困难，退化方案为「focus 时给 button 加 2px ring 色的 border」（视觉略缩内但保证可用）。implementer 优先 outer ring，遇 API 阻碍可降级，spec 不卡死

---

## 5. API 改动

### 5.1 ColorTokens（破坏性？否）

```rust
pub struct ColorTokens {
    // ... 原 21 个字段 ...
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub destructive_hover: Hsla,
    pub destructive_active: Hsla,
}
```

**兼容性**：`ColorTokens` 是 aish-ui 内部 struct，外部不该手工构造（都走 `Theme::dark()` / `Theme::light()`）。加字段属于内部演进，不破坏 user API。

### 5.2 Button

```rust
impl Button {
    // 新增
    pub fn focus_handle(mut self, h: FocusHandle) -> Self {
        self.focus_handle = Some(h);
        self
    }
}

pub struct Button {
    // ... 原字段 ...
    focus_handle: Option<FocusHandle>,
}
```

`Button::new()` 默认 `focus_handle = None`。其它现有方法、变体不变。

### 5.3 IconButton

同 Button 加 `focus_handle(handle)` builder + `focus_handle: Option<FocusHandle>` 字段。

### 5.4 render 改动（伪代码）

```rust
// Button::render
let (idle_bg, hover_bg, active_bg, fg) = if disabled {
    (t.colors.muted, t.colors.muted, t.colors.muted, t.colors.muted_foreground)
} else {
    match variant {
        Primary => (t.colors.primary, t.colors.primary_hover, t.colors.primary_active, t.colors.primary_foreground),
        Secondary => (t.colors.secondary, t.colors.secondary_hover, t.colors.secondary_active, t.colors.secondary_foreground),
        Destructive => (t.colors.destructive, t.colors.destructive_hover, t.colors.destructive_active, t.colors.destructive_foreground),
        Ghost => (transparent, t.colors.accent, t.colors.accent, t.colors.foreground),
    }
};

let mut el = div().bg(idle_bg) ...;

if !disabled {
    el = el.cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .active(move |s| s.bg(active_bg));  // GPUI active modifier
    // on_click 不变
}

if let Some(handle) = &focus_handle {
    el = el.track_focus(handle);
    // focus 视觉：让 implementer 用 GPUI box-shadow / outline 画 outer ring
}
```

GPUI 的 `.active()` modifier 与 `.hover()` 对应（在 stateful element 上有效）。

---

## 6. 测试计划

### 6.1 ColorTokens (theme/dark.rs)

- `dark_primary_hover_is_lighter_than_primary`：`t.colors.primary_hover.l > t.colors.primary.l`
- `dark_primary_active_is_lighter_than_hover`：`t.colors.primary_active.l > t.colors.primary_hover.l`
- 同样 3 组：secondary / destructive

预期：dark.rs 测试 +6（M11 的 3 个 dark_* test 保留）

### 6.2 Button (button.rs)

保留现有 5 个测试。新增：

- `focus_handle_default_none`：`Button::new("a").focus_handle.is_none()`
- `focus_handle_chain_stores`：传入 handle 后字段非空

预期：button.rs 测试 5 → 7

不写「per-variant hover bg = primary_hover」之类的运行时测试，因为 GPUI render 路径无 entity tree assert API，与现有 codebase test pattern 一致（局部变量伪测试 + builder chain 测试为主）。

### 6.3 IconButton (icon_button.rs)

保留现有 3 个。新增：

- `focus_handle_default_none`
- `focus_handle_chain_stores`

预期：icon_button.rs 3 → 5

### 6.4 总测试增量

- aish-ui：100 → ~110（dark.rs +6，button.rs +2，icon_button.rs +2，整体净 +10）

---

## 7. Task 拆分预算

| Task | 范围 | 预计 |
|---|---|---|
| T1 | ColorTokens +6 + Dark 填色 + Light 占位 + theme test | 0.25 天 |
| T2 | Button hover/active per variant + focus_handle 可选 + 测试 | 0.5 天 |
| T3 | IconButton 同步处理 + 测试 | 0.25 天 |
| T4 | INDEX 更新 + DoD 自检 + 视觉手测 | 0.25 天 |

合计 ~1.25 天。比 M14（~2.5 天）轻一档，符合 visual polish 类 milestone 的体量。

---

## 8. 风险 / 已知边界

- **GPUI active modifier API 未验证**：spec 假设 `.active(closure)` 与 `.hover()` 对称存在；如 GPUI 实际 API 名为 `.pressed()` / `.on_active()` 等，T2 implementer 调整 callsite，保 spec 表面意图不变
- **focus ring 视觉实现路径未验证**：spec 给 outer ring 偏好，允许 implementer 退化为 inner border 着色（spec § 4.3 备选）
- **Light theme 6 个新 token = dark 值占位**：是已知技术债，下个 light theme milestone 必须填，否则 light theme 仍 `unimplemented!()` 状态保持
- **Tokyo Night 阶梯色具体 hex 由 implementer 挑**：spec 仅给方向「hover 比 idle 亮、active 更亮」，validation test 仅断言 lightness 单调；实际色由 HSL 调参得来
- **HostFormModal 等现有 Button callsite 不接 focus**：本 milestone 不动 callsite，focus_handle 接入是后续 milestone（可能与 Dialog Tab focus trap 一起做）

---

## 9. DoD（Definition of Done）

- [ ] ColorTokens 加 6 字段，Dark theme 填色，Light theme 占位
- [ ] Button hover/active 按 variant 取对应 token，Ghost 保持现状
- [ ] IconButton 同步处理
- [ ] Button + IconButton 加可选 `focus_handle(FocusHandle)` builder，传入后渲染 focus ring
- [ ] aish-ui 测试 100 → 至少 108
- [ ] 质量门禁：fmt + clippy 0 warning + workspace test 全过
- [ ] `INDEX.md` 加 M15 条目，更新当前状态
- [ ] 手测（可选）：实际启动 aish，观察 send button / settings switch 等场景的 hover/active/focus 视觉
