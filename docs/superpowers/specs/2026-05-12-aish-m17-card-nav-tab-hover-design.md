# M17 — Card / NavItem / TabItem hover 改造 + accent_active Spec

> 状态：草案（待用户审）
> 父 spec：[`2026-05-09-aish-ui-architecture-design.md`](2026-05-09-aish-ui-architecture-design.md)
> 关联：M15 给 Button/IconButton 加了按 variant 分色的 hover/active token；M17 是把剩下 3 个有 hover 的组件跟上，并兑现 M15 D-2 当时"Ghost 不引入 accent_active"决策的回退

---

## 1. 目标

补完 aish-ui 的 hover/active 状态体系：

- **新加 `accent_active` token**：Card / NavItem / TabItem 这类 "accent 风格" 容器组件按下时的视觉反馈色
- **NavItem hover 补 bg**：当前 hover 只改 text_color，加 bg(accent) 与 Card / TabItem 一致
- **三组件接 `.active()` modifier**：mouse-down 时 bg 切到 accent_active
- **HostForm 之类的 callsite 不动**：所有改动限于 aish-ui crate 内组件 + theme

---

## 2. 范围 & 不做事项

### 范围内

- `crates/aish-ui/src/theme/tokens.rs`：`ColorTokens` 加 1 个字段 `accent_active`
- `crates/aish-ui/src/theme/dark.rs`：填 accent_active 色值 + 1 个 lightness 断言
- `crates/aish-ui/src/theme/light.rs`：占位 + TODO 注释更新（接 M15/M16 既有占位）
- `crates/aish-ui/src/components/card.rs`：on_click 启用时加 `.active(|s| s.bg(accent_active))`
- `crates/aish-ui/src/components/nav_item.rs`：active=false 路径 hover 补 `.bg(accent)`，加 `.active(|s| s.bg(accent_active))`
- `crates/aish-ui/src/components/tab_item.rs`：selected=false 路径加 `.active(|s| s.bg(accent_active))`
- 各组件相应测试
- `INDEX.md`：M17 条目 + 当前状态推进

### 不做事项

- **Button / IconButton Ghost variant 接 accent_active**：M15 D-2 决策范围内，M17 不动这两个组件。Ghost 的 accent_active 兑现留 backlog（M18+ 单独 milestone）
- **给三组件加 ButtonVariant 字段**：它们是 layout/nav/container，不是 action 组件，YAGNI
- **NavItem / TabItem selected (active=true) 路径视觉调整**：保现状（NavItem selected bg=card，TabItem selected bg=secondary），hover/active 不接管 selected 路径
- **focus_handle / focus ring**：这 3 组件 caller 一般不需键盘 focus，不在范围
- **Light theme 真正色值**：M17 新加的 accent_active 占位 = dark 同值，下个 light theme milestone 真正实现

---

## 3. ADR-style 决策记录

### D-1：新加 `accent_active` token 而非复用 secondary

**决策**：ColorTokens 加显式字段 `accent_active`，dark theme 给比 accent 略深一档的色。

**考虑过**：
- A) 新加 token ✅ 选中
- B) 复用 secondary（Tokyo Night secondary #2d2d3f 比 accent #6c91c2 暗，语义上可作"按下"反馈）
- C) 算法 darken accent (Hsla::darken helper)

**理由**：
- 与 M15 的 `primary_active` / `secondary_active` / `destructive_active` 命名一致（虽然方向相反，见 D-3）
- B 方案语义不一致：hover=accent / active=secondary 会让维护者困惑"为什么按下时跳到另一系列"
- C 方案需要引入 helper 函数，M15 当时已考虑过被 D-1 拒绝，这里保持一致

### D-2：M15 D-2 决策正式回退

M15 当时的 D-2 决策：「Ghost variant 不引入 accent_active token，保持简单」。M17 把 accent_active 加进来。**这是有意识的回退**。

**理由**：M15 当时 D-2 的"简单"前提是 Button Ghost 是唯一用 accent 的地方。M17 范围扩到 Card/NavItem/TabItem 后，3 个组件都需要按下反馈，accent_active token 的复用价值显现。

**遗留**：Button/IconButton 的 Ghost variant 当前仍走 hover=accent / active=accent（无区别）。**M17 不改它们**（避免范围膨胀），留 backlog 在 M18+ 兑现。INDEX 显式记录这点。

**命名冲突澄清**：`accent_active` token 中的 `_active` 沿用 M15 的命名约定，对应 GPUI `.active()` modifier 的语义（mouse-down 持续按下）。这与 `NavItem::active(bool)` / `TabItem::active(bool)` builder 的内部 `active` 字段（指 "selected 当前激活的 tab/nav 项"）是**两个不同 namespace 的概念**：token 层 `_active` = "pressed"，组件 API 层 `.active()` = "selected"。这两个 namespace 在代码中不会交叉（NavItem selected 路径 D-5 决策已声明不接 token 的 active 状态）。

### D-3：accent_active 在 dark theme 比 accent **更深**（lightness 方向与 primary/destructive 相反）

**决策**：dark theme 给 accent_active 比 accent **更深一档**（lightness 减少）。M15 的 primary/secondary/destructive 系列是 hover→active **更亮**。

**理由**：
- accent 系列用作"中性容器 hover"（Card/NavItem/TabItem 这种 layout 组件）。按下时直觉是"沉下去、暗化"
- primary/destructive 系列用作"主动按钮"。按下时直觉是"更鲜艳、强调"
- 这是 UI 设计层面的语义差异，不是 bug
- spec 显式记录避免维护者按统一规则照搬

**测试断言方向**：`t.colors.accent_active.l < t.colors.accent.l`（与 M15 的 primary_active.l > primary.l 相反）。

### D-4：NavItem hover 补 bg 改变现有视觉

**决策**：active=false 路径 hover 补 `.bg(accent)`，原有 `.text_color(secondary_foreground)` 保留。

**风险**：NavItem 当前 hover 是 minimal（只改 text_color），sidebar 4 个 nav 项视觉更"重"。

**理由**：
- 让三组件 hover 风格统一（Card / TabItem 都有 bg 变化）
- 用户体验上 hover 加 bg 提供更明确的"可点击"反馈
- 若视觉手测发现 sidebar 变化过重，可降级用 secondary 替代 accent（但 spec 阶段不预先降级，避免猜测）

### D-5：三组件 selected/active=true 路径完全不动

**决策**：NavItem selected (bg=card)、TabItem selected (bg=secondary) 状态下，hover 和 mouse-down 不接管视觉。

**理由**：
- selected 状态视觉强（已有 indicator 条 + bg 变化），hover/active 再叠加容易过载
- selected NavItem/TabItem 是"信息态"（指示当前位置），不是"待操作态"，不需要按下反馈
- 范围最小，避免动到 caller 视觉

---

## 4. 视觉规范

### 4.1 三组件状态色表

| 组件 | 状态 | bg | text |
|---|---|---|---|
| **Card** | idle | `card` | （body 自己控）|
| Card (on_click) | hover | `accent` | — |
| Card (on_click) | active | `accent_active` | — |
| **NavItem** (active=false) | idle | transparent | `muted_foreground` |
| NavItem (active=false) | hover | **`accent`**（新）| `secondary_foreground` |
| NavItem (active=false) | active(pressed) | `accent_active` | `secondary_foreground` |
| NavItem (active=true) | selected | `card` | `foreground` |
| NavItem (active=true) | hover / pressed | 同 selected（不接管）| 同 selected |
| **TabItem** (active=false) | idle | transparent | `muted_foreground` |
| TabItem (active=false) | hover | `accent` | — |
| TabItem (active=false) | active(pressed) | `accent_active` | — |
| TabItem (active=true) | selected | `secondary` | `foreground` |
| TabItem (active=true) | hover / pressed | 同 selected（不接管）| 同 selected |

### 4.2 Dark theme accent_active 色

参考阶梯（与 accent 的 lightness 关系）：

```
accent           #6c91c2   (h≈220°, l~56%)  现有
accent_active    #4a7099   (h≈220°, l~45%)  暗 ~11%
```

具体 hex 由 T1 implementer 用 HSL picker 选，**验证测试只断言 `accent_active.l < accent.l`**（单调降）。

---

## 5. API 改动

### 5.1 ColorTokens（破坏性？否，加字段）

```rust
pub struct ColorTokens {
    // ... 原 27 个字段（含 M15 的 6 个 hover/active）...
    pub accent_active: Hsla,  // M17 新加
}
```

字段位置：放在 `destructive_active`（M15 最后字段）之后；保持 "原有 → M15 加 → M17 加" 的时间序。

### 5.2 Dark / Light theme

`dark.rs` 加：

```rust
            // M17 新加：accent 按下反馈，比 accent 更深
            accent_active: hex(0x4a7099),
```

`light.rs` 更新 TODO 注释，加上 accent_active：

```rust
// TODO(light-theme): M15/M16/M17 给 ColorTokens 加的 hover/active 字段
// （primary/secondary/destructive_hover/_active、accent_active）当前 light()
// 是 unimplemented! stub，未构造 struct literal。下个 light theme milestone
// 真正实现 light() 时按 light 配色手挑这些色（dark 那批对照参考在 theme/dark.rs）。
```

### 5.3 Card render hookup

`card.rs` 现有 on_click 启用路径（line ~108-115）：

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

### 5.4 NavItem render hookup

`nav_item.rs` 现有 active=false 路径（line ~129-131）：

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

注意：`active=true` 路径（line ~125-127 `if active && orientation == Vertical { el = el.bg(t.colors.card); }`）**不动**。selected NavItem 不接 hover/active。

### 5.5 TabItem render hookup

`tab_item.rs` 现有 active=false 路径（line ~92-95）：

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

注意：`active=true` 路径（line ~73-75 selected bg=secondary）**不动**。

---

## 6. 测试计划

### 6.1 dark.rs：1 个新断言

```rust
#[test]
fn dark_accent_active_is_darker_than_accent() {
    let t = Theme::dark();
    assert!(t.colors.accent_active.l < t.colors.accent.l);
}
```

注意方向：M15 测试都是 `_active.l > _.l`（更亮），M17 这个测试反向（更暗）。

### 6.2 card.rs / nav_item.rs / tab_item.rs

GPUI render path 没有 unit test API（既有 codebase pattern），但 builder chain / 状态机层面可加伪测试：

- card: 无需新测试（active 接入是单行 .active() 调用，与现有 hover 同模式）
- nav_item: 加一个 `hover_only_when_inactive` 伪测试，验证 active=true 时不进入 hover 分支
- tab_item: 同 nav_item，加一个 `hover_only_when_inactive` 伪测试

预期 aish-ui 121 → 124（dark.rs +1 / nav_item.rs +1 / tab_item.rs +1）。

### 6.3 视觉手测

`cargo run -p aish-app` 观察：

- **Home page host card** hover → bg 变 accent；按下 → bg 变 accent_active；松开 → 回 hover
- **Sidebar 4 个 NavItem**：active=false 的 hover 加上 bg(accent) 后视觉是否过重？若过重在 T3 阶段考虑改为 secondary token
- **TabBar tab 项** hover → accent；按下 → accent_active
- **NavItem / TabItem selected** 项：hover / 按下时**视觉无变化**（验证 D-5 决策落地）

---

## 7. Task 拆分预算

| Task | 范围 | 预计 |
|---|---|---|
| T1 | ColorTokens +accent_active + Dark 填值 + Light 占位更新 + 1 个 lightness 断言 | 0.2 天 |
| T2 | Card on_click 路径加 `.active()` | 0.15 天 |
| T3 | NavItem active=false 路径 hover 补 bg + `.active()` + 1 个测试 | 0.25 天 |
| T4 | TabItem active=false 路径加 `.active()` + 1 个测试 | 0.2 天 |
| T5 | INDEX 更新 + DoD + 视觉手测建议 | 0.2 天 |

合计 ~1 天。

---

## 8. 风险 / 已知边界

- **NavItem hover 加 bg 视觉过重**：当前 NavItem hover 只改 text_color，sidebar 4 项一起加 bg 后可能显得"喧宾夺主"。手测后若问题严重，T3 阶段可降级到 secondary token（hover bg = secondary 而非 accent）。spec 阶段先按 accent 走
- **accent_active lightness 方向相反**：与 M15 primary/secondary/destructive_active 方向不一致，可能让维护者按 M15 模式照搬。spec § 3 D-3 + § 6.1 测试方向显式记录避免误读
- **Button/IconButton Ghost 未同步**：当前仍走 hover=accent / active=accent（无区别）。INDEX 显式记录这是 M17 已知未做项，留 M18+ 兑现。否则 accent 系列在 codebase 内不一致（容器有 active 反馈，Button Ghost 没有）
- **selected NavItem/TabItem 完全不接 hover/active**：用户用鼠标移到已选中的 tab 上时无视觉变化，可能让人误以为 "hover 没生效"。这是 D-5 决策的内在 trade-off，spec 接受

---

## 9. DoD（Definition of Done）

- [ ] ColorTokens 加 `accent_active` 字段
- [ ] Dark theme 填值（lightness 比 accent 小）
- [ ] Light theme TODO 注释包含 accent_active
- [ ] Card on_click 路径加 `.active(|s| s.bg(accent_active))`
- [ ] NavItem active=false 路径 hover 补 bg(accent) + `.active(|s| s.bg(accent_active))`
- [ ] TabItem active=false 路径加 `.active(|s| s.bg(accent_active))`
- [ ] NavItem / TabItem selected 路径完全不动
- [ ] dark.rs `dark_accent_active_is_darker_than_accent` 测试通过
- [ ] aish-ui 测试 121 → 至少 123
- [ ] 质量门禁：fmt + clippy 0 warning + workspace test 全过
- [ ] INDEX 加 M17 条目 + 当前状态指向 M18 候选（含 Button Ghost accent_active 兑现）
- [ ] 视觉手测（可选）：sidebar / home / tabbar 三处 hover+active 反馈正常
