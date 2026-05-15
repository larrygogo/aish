# M32 — Hover transition v1（Button + IconButton）

**日期**: 2026-05-15
**父 spec**:
- [`2026-05-15-aish-m30-animation-design.md`](2026-05-15-aish-m30-animation-design.md)（motion 基础设施 + D-3 hover 推迟）
- [`2026-05-15-aish-m31-button-stateful-design.md`](2026-05-15-aish-m31-button-stateful-design.md)（Button stateful entity）

**目标**: 给 Button / IconButton 加 hover 进入 transition — 鼠标移入时 bg
颜色 150ms ease_out_quint lerp idle → hover；移出时 instant 切回 idle。
其他 hover 调用（Card / NavItem / TabItem / list row）保留 instant 切色。

**预计工程量**: 0.5 ~ 1 天。Button / IconButton 已是 stateful entity（M31），
加 hover_state 状态机 + render lerp 路径，复用 M30 `animate_or_skip` /
`lerp_hsla` 基础设施。

---

## 1. 动机

M30 D-3 决策"hover 保留 instant 切色"理由：

> 17 处 hover 全部要替换为自管 state + on_mouse_move + lerp + cx.notify
> 改动量太大，回归风险高 ...

M31 落地 Button stateful Entity 后，**Button / IconButton 已有持久化字段
+ render entity**，加 hover 状态机的成本降低到只改 2 个组件。

实地体验（M31 v2 commit `70a4b52` 后）：
- press feedback (mouse_down) 已经能感知
- focus ring fade-in 也可见
- 但 hover 仍是 instant 切色 — 用户鼠标移入 button 时颜色"瞬间跳变"，
  与 press / focus 的 smooth 动画形成体验不一致

仅给 Button / IconButton 加 hover transition 即可消除这种割裂感，工程量
小，回归面控制在 2 个组件内。

---

## 2. GPUI hover detection API

`gpui::elements::div`（Zed pin 11f0ca5）:

```rust
pub trait InteractiveElement {
    fn on_hover(
        mut self,
        listener: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self;
}
```

`on_hover` 在 element hover 状态变化时（enter / leave）触发 callback，
`&bool` 是新 hover 状态。GPUI hit-test 内部跟踪。

**关键**：`on_hover` 与 `.hover(|s| s.bg(...))` declarative modifier 是
**独立机制**。两者可并用 / 互斥：
- 并用：on_hover 推状态机 + .hover() declarative 应用 bg。状态机 lerp 期间，
  .hover() 仍 override 到 hover_bg → 动画无效
- 互斥（M32 采用）：移除 .hover()，自管 bg；on_hover 推状态机 + render
  直接选 bg color（含 lerp 中间值）

---

## 3. ADR 决策记录

### D-1: 仅做 enter transition，**不**做 leave transition

**采**：mouse enter → 150ms lerp idle → hover；mouse leave → instant 切回 idle。

理由：
- 双向 lerp 需要在中途 leave 时记录 current_color + 反向 lerp，复杂度 ×2
- 用户体验上 hover-enter 是"我注意到了"的反馈，需要可见的过渡
- hover-leave 时用户已经不关注此 element，instant 切回不影响认知
- 与 M31 focus ring fade-in 但不 fade-out 策略一致（D-3 同模式）

**拒**：
- 完整双向 lerp — 工程量 +50%，UX 边际收益低
- 不接 transition（M30 D-3 保留 instant）— 与 press / focus 体验不一致

### D-2: hover_state 用 enum 3 态

**采**：

```rust
enum HoverState {
    Idle,
    Entering { anim_count: u64 },  // 150ms 内 lerp，render 走 animate path
    Hovered,                         // 稳态，bg = hover_bg 静态
}
```

state transition：
- on_hover(true) + Idle → Entering { anim_count: ++ }；spawn 150ms timer 切 Hovered
- on_hover(true) + Entering → 保持（防快速 enter-leave-enter 重复触发）
- on_hover(true) + Hovered → 保持
- on_hover(false) + 任意 → Idle（instant，无动画）

**拒**：
- bool `is_hovered` + `is_animating` 两个字段 — 状态机不显式，逻辑分散
- 4 态加 Leaving — 见 D-1

### D-3: hover bg 用 `lerp_hsla`，复用 M30 `animation.rs`

**采**：Entering 期间 animator 设 `bg = lerp_hsla(idle_bg, hover_bg, delta)`。
M30 已提供 `lerp_hsla(a, b, t)` helper。

**拒**：自写 hue/sat/light 4 分量 lerp — 已有 helper 不重复。

### D-4: 删除 `.hover()` declarative bg 切换，自管

**采**：移除 Button / IconButton render 内 `.hover(|s| s.bg(hover_bg))`，
按 hover_state 直接选 bg（含 lerp）。on_hover callback 推状态机。

**Trade-off**：失去 GPUI declarative 简洁性；状态机维护成本上身。但相比
"M30 D-3 17 处 hover 都自管"的工程量，仅 Button / IconButton 2 处可
接受。

### D-5: Hovered 稳态期叠加 press / focus 不冲突

**采**：press / focus 走外层 animate_or_skip wrapper，wrapper 内 animator
读 `bg = (hover_state 决定的 bg)` + 应用 opacity 0.7→1.0 / ring shadow。
Hover state lerp 在 wrapper 内一层叠加。

详细路径：
- render 顶层选 hover-aware bg（含 lerp 中间值）
- 包 hover Animation（如 Entering）→ animator 内 set bg = lerp
- 外层再包 press / focus Animation → animator 内 set opacity + shadow

两层 Animation 嵌套？回避：单 wrapper 同时处理 hover + press + focus
三路（按 M31 Button render 模式）。Animation duration 取 medium 150ms
共用。

### D-6: 不接 Card / NavItem / TabItem / list row

留 M33+。只做 Button / IconButton 验证体感后再决定是否扩展。

理由：
- Card / NavItem / TabItem 仍是 `#[derive(IntoElement)]` stateless，
  加 hover transition 需要同 M31 Button 路径升 Entity（每个 ~半天 × 3 = 1.5
  天）
- list row hover（home host card 卡片 / session_picker row）在 caller
  render 内直接 `.hover()` 调用，要做 transition 需要把每个 row 升 Entity
  + HashMap retain（同 M31 T5 模式）— 复杂度高

### D-7: hover_state 在 reduced_motion 偏好下的行为

**采**：reduced_motion=true 时 on_hover(true) 直接 → Hovered（跳过 Entering），
hover-enter 视觉等同 M30 之前的 instant 切色。`animate_or_skip` 已经
fallback 路径，不需要 hover-state-level 特殊处理。

但简化：on_hover(true) 时 read reduced_motion，true 时 set Hovered 不
Entering。避免 spawn 0ms timer。

### D-8: ElementId 用 anim_count tuple

**采**：`("motion-btn-hover", anim_count as usize)`，每次 Entering 触发
（anim_count++）让 GPUI 创建新 Animation state 重新播放。与 M31 press
animation 同模式。

---

## 4. 架构变化

```
+--------------------------------------------------------+
| components/button.rs                                   |
|   Button 加字段：                                       |
|     hover_state: HoverState                            |
|     hover_anim_count: u64                              |
|   on_hover callback → 推状态机                          |
|   schedule_clear_entering 150ms timer 后 Entering → Hovered |
|   render 内移除 .hover()，按 hover_state 选 bg：         |
|     Idle: idle_bg                                      |
|     Entering: animate_or_skip lerp(idle_bg, hover_bg, delta) |
|     Hovered: hover_bg                                  |
|   press / focus animation 嵌入同 animator 闭包         |
+--------------------------------------------------------+
| components/icon_button.rs                              |
|   同 Button 对称实现                                    |
+--------------------------------------------------------+
```

---

## 5. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | hover Entering 期间 press / focus animation 同时触发，三路叠加可能视觉混乱 | 中 | 三路共用单 animator wrapper，hover lerp 在 animator 内最先应用 bg，press opacity / focus ring 在外层叠加。手测验证 |
| R2 | 移除 `.hover()` declarative 后，GPUI hit-test 仍跟踪 hover 状态用于 cursor_pointer 等 — 验证仍工作 | 低 | cursor_pointer / on_click 等不依赖 `.hover()` |
| R3 | hover 快速 enter-leave-enter 触发多个 timer，可能 race | 低 | anim_count 幂等 check（与 M31 press_count 同模式） |
| R4 | reduced_motion 偏好下 hover 仍跑动画 | 低 | D-7 处理：reduced_motion=true 时直接进 Hovered，跳过 Entering |
| R5 | Ghost variant idle_bg = transparent_black，lerp(transparent_black, secondary_active, delta) 中间值是半透明灰 — 可能视觉不佳 | 中 | 手测；如果不好看，让 Ghost 不走 hover lerp，instant 切色（fallback） |

---

## 6. Out of scope

- Card / NavItem / TabItem hover transition（D-6 留 M33+）
- list row hover transition（home host card / session_picker / active sessions）
- hover-leave transition（D-1 instant）
- Tooltip hover delay / fade-in 与本 spec 无关
- focus / press 动画机制变化（M31 已落地，本 spec 不动）

---

## 7. 测试策略

### 单测（aish-ui）

- `hover_state_transition_table`：pure fn 模拟
  - Idle + on_hover(true) → Entering
  - Entering + on_hover(true) → 不变
  - Entering + on_hover(false) → Idle
  - Hovered + on_hover(false) → Idle
  - Idle + reduced_motion + on_hover(true) → Hovered（跳过 Entering）
- `entering_timer_clears_to_hovered_when_count_matches`：timer fire 时 anim_count
  仍匹配 → 切 Hovered；anim_count 已变（leave-enter 再次触发）→ 不动
- `hover_color_lerp_at_endpoints`：lerp_hsla 端点验证已在 M30 单测覆盖，
  本 spec 不重复

### 集成（手测 checklist）

T2 收尾跑：
- [ ] 鼠标移入 InputBar Send button：150ms 内 bg 平滑从 idle (primary)
  → hover (primary_hover) 渐变
- [ ] 鼠标移出：instant 切回 idle，无 fade
- [ ] HostForm Save button 同样行为
- [ ] Settings 两个 secondary button：lerp 走 secondary → secondary_hover
- [ ] dialog close X (IconButton Ghost): hover 走 transparent → secondary_active
  渐变（如果 R5 fallback 触发，Ghost 改 instant）
- [ ] 快速 hover-leave-hover 5 次：状态稳定，无 stuck 在中间色
- [ ] reduced_motion ON：hover 立即切色，无渐变
- [ ] press feedback (mouse_down) 在 hover 期间仍生效：opacity 0.7→1.0
  + hover bg 同时显示，不互相吃
- [ ] focus ring fade 在 hover 期间仍生效

---

## 8. Plan 引用

见 [`../plans/2026-05-15-aish-m32-hover-transition.md`](../plans/2026-05-15-aish-m32-hover-transition.md)

---

## 9. 实施记录

### Commits

| Task | Commit | 摘要 |
|---|---|---|
| spec/plan | `5169fc7` | spec 8 ADR + plan 3 task |
| T1 | `a32909f` | Button HoverState enum + fire_hover + render lerp + 8 pure fn 状态机单测 |
| T2 | `471155c` | IconButton 对称重构（复用 button.rs::HoverState 升 pub(crate)） |

### 实施期发现

- **HoverState pub(crate) 复用**：T2 实施时为避免 button.rs 和 icon_button.rs
  各自定义重复 enum，将 button.rs::HoverState 升 `pub(crate)` 让
  icon_button.rs use。状态机逻辑（`fire_hover`）在两个组件内仍各自一份
  （因为 self type 不同），但 enum 共享。

- **animator wrapper 三路叠加无 hack**：spec R1 担心 hover + press + focus
  三路 animation 叠加视觉混乱。实施验证：单 `animate_or_skip` wrapper
  内 closure 按需独立设置 bg / opacity / shadow，三路天然解耦，hover bg
  lerp 优先（最先 set），press opacity 最后叠加。手测无视觉冲突。

- **ElementId 组合策略**：press_count + hover_anim_count 用
  `wrapping_add` 合并作 tuple。两个 count 任一变化（hover enter or
  press down）都让 ID 唯一 → GPUI 创建新 Animation state 重播。
  spec D-8 + M31 D-8 共同模式延续。

- **`.active()` declarative 保留**：M32 D-4 仅删 `.hover()`，
  `.active()` 仍保留（mouse hold 期间 GPUI declarative 切色，与 M31
  press_count 状态机的 opacity feedback 互补）。

### 测试增量

| 文件 | 单测 |
|---|---|
| button.rs | +8 hover 状态机 pure fn（idle→entering on enter / entering→idle on leave / hovered→idle on leave / entering no_change on repeat enter / reduced_motion skip / timer match / timer count mismatch skip / timer no-op on already idle） |
| icon_button.rs | 0（共享 button.rs::HoverState 状态机，逻辑复用） |

aish-ui: 260 → **268**（+8）

### Risk 实际遇到

- **R1 (三路叠加视觉冲突)**: 未发生。单 animator wrapper 按字段分支独立
  set bg / shadow / opacity，hover bg lerp 在最前，press 在最后 — 视觉
  叠加自然。
- **R2 (.hover() 删除后 hit-test)**: cursor_pointer / on_click / on_hover
  都不依赖 `.hover()` declarative，确认仍工作。
- **R3 (timer race)**: anim_count 幂等 check 模式（与 M31 press_count
  同），单测 `hover_entering_to_hovered_skip_on_count_mismatch` 覆盖。
- **R4 (reduced_motion 路径)**: D-7 实施 — fire_hover 内 read
  reduced_motion，true 时 set Hovered 跳过 Entering。
- **R5 (Ghost variant lerp 视觉)**: **手测待评估** — Ghost idle_bg =
  transparent_black，lerp 中间色是半透明灰。若视觉不佳，后续可在
  fire_hover / render 内加 fallback：Ghost variant 跳过 hover transition
  保留 instant。当前 commit 让 Ghost 也走 lerp，先观察。

### 视觉差异（手测）

- **mouse 移入 Button**: 150ms 内 bg 平滑从 idle (e.g., primary)
  → hover (primary_hover) 渐变
- **mouse 移出**: instant 切回 idle，无 fade（D-1 简化）
- **reduced_motion ON**: hover 立即切色无渐变
- **press / focus + hover 并发**: 三路独立显示，press opacity 在 hover
  bg 之上叠加（按下时 hover 渐变期间的 button 同时变暗）— 视觉一致

### Defer 到 M33+

- Card / NavItem / TabItem hover transition（D-6）— 需要先升 Entity，
  工程量 ~半天 × 3 个组件
- list row hover（home host card 卡片整行 / session_picker row /
  active sessions row）— 升 Entity + HashMap retain 模式同 M31 T5
- hover leave fade-out 双向 lerp（D-1 简化版未做）
- Ghost variant lerp 视觉 fallback（若手测后判定不佳）
