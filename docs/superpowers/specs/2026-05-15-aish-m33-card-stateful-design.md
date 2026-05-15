# M33 — Card 升 Entity + hover transition

**日期**: 2026-05-15
**父 spec**:
- [`2026-05-15-aish-m31-button-stateful-design.md`](2026-05-15-aish-m31-button-stateful-design.md)（Button 升 Entity 路径，本 spec 简化复用）
- [`2026-05-15-aish-m32-hover-transition-design.md`](2026-05-15-aish-m32-hover-transition-design.md)（hover transition 状态机）

**目标**: 把 `aish_ui::Card` 从 stateless `RenderOnce` 升 stateful `Entity`，
落地 hover transition（mouse 移入 on_click Card 时 bg 150ms lerp idle →
hover）。范围限 Card 单组件（NavItem / TabItem 留 M34+，按需评估）。

**预计工程量**: ~1 ~ 1.5 天，4 处 callsite + AnyElement 每帧 update 模式
（同 Dialog body）。

---

## 1. 动机

M32 给 Button / IconButton 加 hover transition 后，**home host card 仍是
instant 切色** — 用户最频繁交互的元素（host 列表 + 点击进入）反而没有
M32 体验。

Card 升 Entity 后获得：
- hover transition（home host card 体验改进，每张卡片 mouse enter 150ms
  bg 渐变）
- press feedback（mouse_down 80→1.0 opacity，与 Button 一致）
- focus ring fade-in（虽然 Card 通常不参与 Tab focus，但 entity 框架统一）

**Settings 3 Card 没 on_click**：升 Entity 后无 hover 切色 / 无 hover transition，
仍是装饰容器。但 entity 框架统一让 caller API 一致，运行时开销低（entity
持轻量字段，无 timer）。

---

## 2. ADR

### D-1: Card 升 `Render`，slots 每帧 `.update()` 重设

**采**：同 M31 Button — `cx.new(|cx| Card::new(id, cx))` + builder
`&mut self -> &mut Self`。slots（header/body/footer）是 `AnyElement` 不可
Clone，caller 每帧 `card.update(cx, |c, _| c.header(h).body(b).footer(f))`
重新塞（同 Dialog body 模式）。

**Trade-off**：caller 需在 render 内 update + clone，比 stateless
right-value `Card::new()` 多 1 行。但 callsite 仅 4 处可接受。

### D-2: 仅在 `on_click` 设置时启用 hover transition

**采**：Card 内部 `if on_click.is_some() { 走 hover 状态机 + animate path }
else { 走 stateless 简化 render，无 hover state 推进 }`。

理由：
- Card 无 on_click 时 stateless 时代 `.hover()` 也不调用（card.rs 现有
  逻辑：`if let Some(handler) = self.on_click { hover + active + listener }`）
- 升 Entity 后保持同语义：on_click 决定是否启用 hover transition
- Settings 3 Card 走简化路径，无 hover overhead

### D-3: 复用 button.rs `HoverState` enum

**采**：直接 `use crate::components::button::HoverState`。M32 已升
pub(crate)，跨组件复用。

**拒**：Card 单独定义 — 重复代码无收益。

### D-4: Vec 渲染（home host card）走 HashMap retain

**采**：HomeView 加 `host_cards: HashMap<HostId, Entity<Card>>`，render
前 `retain_alive_entities` 清死 host，再 entry().or_insert_with(cx.new(...))
ensure 活 host。每帧 update card 的 slots（host card body row）。

模式与 M31 T5 `host_card_buttons` 完全一致。

### D-5: Settings 3 Card 直接持字段

**采**：SettingsView 加 3 个 `Entity<Card>` 字段（appearance_card /
shortcuts_card / about_card）。new() 内 cx.new 构造，render 每帧 update
header/body 内容。

### D-6: hover_bg_at 复用 M32 v2 — Card variant 单一不需要 Ghost

Card 没 Ghost variant；3 variant (Default / Outlined / Elevated) bg 都是
`t.colors.card`（实色）。hover_bg = `t.colors.secondary_hover`（实色）。
`lerp_hsla` 中间值视觉自然，无 M32 R5 紫粉色 bug。直接 lerp_hsla 即可，
不需要 hover_bg_at 分支。

### D-7: 删除 stateless Card，rename 即原名

同 M31 Button 模式：直接 rename 升级，旧 stateless 不保留。callsite 改
造在同一个 commit / task 内完成（4 处可控）。

---

## 3. 实施范围 / 不动

**做**：
- card.rs Card stateless → stateful Entity（含 hover transition + press
  feedback + focus ring fade）
- home.rs host card callsite（per-host HashMap retain）
- settings.rs 3 Card callsite（3 字段）
- card.rs 单测：删旧 stateless 5 个测试，加 4-5 个 hover 状态机 pure fn

**不动**（留 M34+）：
- NavItem 升 Entity + hover transition
- TabItem 升 Entity + hover transition + indicator slide（M30 T6 defer）
- list row hover transition（home host card 外层 wrap div 的 .hover()
  / session_picker row / active sessions row）

---

## 4. Risk

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | AnyElement 每帧 update 性能（home N 个 host card 都要 take + 重设） | 低 | AnyElement::take 仅 mem::replace，O(1)；重设 slot 是 builder fn，每帧 < 1ms |
| R2 | host_cards HashMap 内存泄漏（host 删除后 entity drop） | 中 | retain_alive_entities 同 M31 T5 验证模式，host_card_buttons 已用 |
| R3 | Card 没 on_click 走简化路径但仍是 Entity，开销？ | 低 | Entity 是 RC 引用，无 on_click 时 render 不挂 listener / timer，开销可忽略 |
| R4 | 同帧 render 调 update + clone 与 GPUI element tree path 冲突 | 中 | Dialog body 已采同模式无问题，照搬 |

---

## 5. 测试策略

- 新增 4-5 个 hover 状态机 pure fn 单测（复用 button.rs HoverState，
  涉及 Card 自身的状态变化路径）
- 删除 stateless Card 的 5 个旧测试（new_defaults / variant_chain /
  slots_can_be_set / on_click_stored / padding 系列）— 旧 stateless API
  不存在
- 手测：
  - [ ] home host card mouse 移入 → 150ms bg 平滑渐变 secondary_hover
  - [ ] mouse 移出 → instant 切回
  - [ ] mouse_down → 150ms opacity 0.7→1.0
  - [ ] Settings 3 Card：无 on_click，hover 不变色（同 stateless 时代）
  - [ ] 添加 host → 新 entity 创建；删 host → host_cards HashMap retain
    清掉

---

## 6. Plan 引用

见 [`../plans/2026-05-15-aish-m33-card-stateful.md`](../plans/2026-05-15-aish-m33-card-stateful.md)

---

## 7. 实施记录

（M33 实施后填）
