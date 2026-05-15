# M31 — Button / IconButton stateful 重构 + press / focus 动画

**日期**: 2026-05-15
**父 spec**:
- [`2026-05-15-aish-m30-animation-design.md`](2026-05-15-aish-m30-animation-design.md)（M30 motion 基础设施 + defer T5/T6）
- [`2026-05-11-aish-m15-button-polish-design.md`](2026-05-11-aish-m15-button-polish-design.md)（M15 hover/active 切色 + focus_handle injection 基础）

**目标**: 把 `aish_ui::Button` / `IconButton` 从 `#[derive(IntoElement)]` stateless
组件升级为 stateful `Entity`（GPUI `Render`），落地 M30 defer 的 press
feedback（opacity 0.85→1.0 fast 80ms ease_out_quint）+ focus ring fade-in
（opacity 0→1 fast 80ms）。

**预计工程量**: 3 ~ 5 天，35 处 callsite 改造 + aish-ui 双组件重构 + Vec 持
Entity 模式（retain_alive_entities 复用 M22）。

---

## 1. 动机

M30 落地 Dialog / Toast 入场动画时已确认：

> **Button stateless 限制（M30 spec R-T5 落地）**：`#[derive(IntoElement)]`
> 组件无法持 `pressing: bool` + spawn timer 自身 re-render。GPUI `.active()`
> 是 declarative style refinement，不可嵌套 `with_animation`。要做 press
> 反馈动画 / focus ring fade 必须重构成 stateful Entity，工程量超 M30。

press feedback 在 spec D-4 评估时被划为"最微小但最频繁"用户感知点 — 30
次/分钟级别的点击交互，每次缺少 80ms ease-out 反馈是细微但持续的"廉价"
感受。M31 还掉这笔技术债。

**何时不该做**：如果只想要"按下变暗" instant feedback，GPUI `.active(|s|
s.opacity(0.85))` 一行 stateless 改动即可，无需 Entity 重构。M31 做的是
**80ms 过渡动画**，必须 stateful。

---

## 2. 当前架构 + Callsite 调研

### 2.1 现状

`Button` / `IconButton` 当前实现：

```rust
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<Rc<dyn Fn(...)>>,
    focus_handle: Option<FocusHandle>,  // M15 加，可选注入
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self { ... }
    // builder：.label/.variant/.disabled/.on_click/.focus_handle...
}

impl RenderOnce for Button {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement { ... }
}
```

**Caller 模式**（典型 home.rs add_btn）:

```rust
let add_btn = Button::new("home-add-host-btn")
    .label("添加 Host")
    .primary()
    .on_click(cx.listener(|this, _ev, _w, cx| this.handle_add(cx)));
// add_btn 是右值，直接传 .child(add_btn)
```

每次父 view render 都重建 Button — stateless 优势是不需要 caller 持有，
劣势是不能持 pressing / focus_animated 状态。

### 2.2 Callsite 全量清单（35 处）

`grep -rn "Button::new\|IconButton::new" crates/{aish-app,aish-ui}/src`

| 文件 | callsites | 场景 |
|---|---|---|
| `aish-app/views/empty_terminal.rs` | 1 | go-home button |
| `aish-app/views/home.rs` | 5 | add_btn × 2 / per-card open + edit + delete / retry |
| `aish-app/views/host_form.rs` | 6 | delete-cancel / delete-confirm / pick-keyfile / host-delete / host-cancel / host-save |
| `aish-app/views/input_bar.rs` | 2 | pick / send |
| `aish-app/views/settings.rs` | 2 | open-config-dir / open-github |
| `aish-app/views/tab_bar.rs` | 1 | per-tab close |
| `aish-app/views/terminal_view.rs` | 1 | terminal-reconnect |
| `aish-ui/components/dialog.rs` | 1 | dialog-close（X 按钮） |
| `aish-ui/components/toast.rs` | 1 | per-toast close X |
| `aish-ui/components/button.rs` | 8 | 单测 |
| `aish-ui/components/icon_button.rs` | 6 | 单测 |

**生产代码 callsite**：19 处生产 + 14 处单测 = 33 处。
**真正难改的**：Vec / HashMap 渲染的 button（home host card × N / tab_bar
close × N / toast 列表 × N）— 需要 per-key Entity HashMap + retain。

### 2.3 GPUI Animation 复用 M30 基础设施

`crates/aish-ui/src/theme/motion.rs`:
- `Motion::fast = 80ms`
- `easing_standard = ease_out_quint`
- `animate_or_skip(el, t, id, anim, animator)` — reduced_motion 自动跳过

`crates/aish-ui/src/animation.rs`:
- `lerp_px` / `lerp_hsla`（M31 主要用 opacity 不需要 lerp，留作未来）

M31 不引入新 motion 基础设施，纯复用。

---

## 3. ADR 决策记录

### D-1: Button 升 `Render` 而非 `RenderOnce`

**采**：`impl Render for Button`，caller 通过 `cx.new(|cx| Button::new(id, cx))`
构造 `Entity<Button>`，render 时 `button.clone()` 嵌入元素树。

**拒**：
- 保留 `RenderOnce` + 加 `pressing` 字段 — RenderOnce 每帧重建实例，
  `pressing` 状态无法跨帧保留
- 自定义 `Element` impl 手动管 state — GPUI 内置 `with_animation` 已经管 state，
  自定义 Element 是 over-engineering

**Trade-off**：caller 从 right-value `Button::new(...)` 改为持 `Entity<Button>`
字段，所有 19 处生产 callsite 必须改造，是 M31 最大成本。

### D-2: press feedback 用 opacity 0.85 → 1.0，**不**用 scale

**采**：mouse_down 进入 `pressing=true` 状态 → render 包 `animate_or_skip(
el, t, "motion-btn-press-{id}", Animation::new(fast).with_easing(ease_out_quint),
|el, delta| el.opacity(0.85 + 0.15 * delta))`。80ms 后 timer 清 pressing → 下一帧
回到无 wrap 路径。

**拒**：scale 0.97 → 1.0 — GPUI `div` **不支持** transform translate / scale
（仅 svg 有 `with_transformation`），spec L4 / M30 T4 已验证。

**数值 0.85 选择理由**：
- 0.95 太弱看不出按下感
- 0.7 太强像被禁用
- 0.85 ≈ alpha 减 15%，与 GPUI 默认 disabled 0.6 + active 切色叠加视觉
  不冲突

### D-3: focus ring fade-in，**不**做 fade-out

**采**：`is_focused(window)` 在 render 内计算 → 与上一帧 `was_focused_prev`
比较 → 若 false → true 触发 `focus_animated=true` 80ms timer fade in；
true → false 直接清 `focus_animated=false`（ring 直接消失）。

**拒**：
- 双向 fade（focus 失去 fade out）— 复杂度 ×2，UX 边际收益低
- 不接 fade，保留 M15 hard box_shadow — M31 价值核心就是 fade，跳过等于
  没做

### D-4: 一次性全量改造，**不**分批 + 不 deprecate 老 API

**采**：M31 一个分支内把 35 处 callsite 全改完，旧 stateless Button
直接删除。worktree 隔离实施，主分支 main 保持可编译，merge 时 squash 进 main。

**拒**：
- 双 API 并存（`Button` stateless + `AnimatedButton` stateful） — 让 caller
  困惑用哪个；spec 文档维护成本翻倍
- 分批迁移（per-view PR）— callsite 之间互相耦合（如 home.rs 的 add_btn
  在 page header + empty state 两处用同 helper），分批难拆

**Risk**: 单 commit 改 35 处过于巨型，需要分 task：
- T2 加 aish-ui Button Entity（旧 stateless 暂保留，标 `#[deprecated]`）
- T3 改 aish-ui 内 dialog / toast 两处 IconButton callsite（用 new Entity API）
- T4 改 aish-app 9 个 view 的所有 callsite
- T5 删 stateless `RenderOnce` impl，单测改 Entity 版

每 task 独立 commit，期间 main 可编译。

### D-5: per-card / per-tab Button Entity 用 HashMap + retain_alive_entities

**采**：home.rs 的 host card Vec 渲染 / tab_bar 的 tab close button 等
"按 key 动态增减" 场景，view 持 `HashMap<HostId, [Entity<Button>; 3]>` 或
`HashMap<TabId, Entity<IconButton>>`，每帧 `retain_alive_entities` 同步
key 集合（M22 已抽出 helper，crate `aish-app/src/state.rs` 内）。

例：

```rust
pub struct HomeView {
    state: Entity<AppState>,
    host_card_buttons: HashMap<HostId, HostCardButtons>,
    add_btn: Entity<Button>,
    retry_btn: Option<Entity<Button>>,
}

struct HostCardButtons {
    open: Entity<Button>,
    edit: Entity<IconButton>,
    delete: Entity<IconButton>,
}
```

render 内：

```rust
let host_ids: HashSet<HostId> = hosts.iter().map(|h| h.id).collect();
retain_alive_entities(&mut self.host_card_buttons, &host_ids);
for host in hosts {
    let btns = self.host_card_buttons.entry(host.id).or_insert_with(|| {
        HostCardButtons {
            open: cx.new(|cx| Button::new(("open", host.id), cx)),
            edit: cx.new(|cx| IconButton::new(("edit", host.id), IconName::Pencil, cx)),
            delete: cx.new(|cx| IconButton::new(("delete", host.id), IconName::Trash, cx)),
        }
    });
    // 每帧用 .update() apply 配置（label 文案随 host 变化等）
    btns.open.update(cx, |b, _| { b.label(host.label.clone()); });
    ...
}
```

**拒**：
- 把 Entity 持在 AppState — 跨 view 共享耦合
- 用 `cx.entity_ref` 一次性 cache — GPUI 无此机制
- 每帧 cx.new 重建 — entity drop 让 Animation state 消失，press 反馈失效

### D-6: IconButton 同步重构（不只升 Button）

**采**：Button 和 IconButton 对称重构。理由：
- 8 callsite 用 IconButton（toast close / dialog close / host card edit/delete /
  tab close / input bar pick / dropdown close）— 都是高频点击
- API / 内部实现高度对称，单独留 IconButton stateless 让 design system
  不一致

**拒**：先升 Button 看效果再升 IconButton — 折半工程量但仅 1.5 天节省，
不值得制造 API 不对称的临时态。

### D-7: builder API 保留 — `cx.new(|cx| Button::new(id, cx).label(...).primary())`

**采**：`new(id, cx)` 签名加 cx 参数（用于 cx.focus_handle）+ 保留 builder
chain，caller 在 `cx.new` 闭包里用 builder 一次性配置。新 API：

```rust
let btn = cx.new(|cx| {
    Button::new("home-add", cx)
        .label("添加 Host")
        .primary()
        .on_click(cx.listener(|this, _ev, _w, cx| this.handle_add(cx)))
});
```

builder 方法签名从 `mut self -> Self`（消费）改为 `&mut self -> &mut Self`
（不消费），以便 `.update(cx, |b, _| b.label(new_text))` 每帧更新配置。

**拒**：
- 全部改成 `Button::new(id, cx, ButtonConfig { ... })` struct 构造 — 失去
  builder 流畅性
- 把 builder 留在外面 `cx.new(...).update(cx, |b, _| b.label(...))` — 双调用
  繁琐

### D-8: M31 不接 hover transition

延续 M30 D-3 — hover 仍 GPUI `.hover()` instant 切色。M31 改的是 press +
focus，hover 留 M32+。

### D-9: 测试策略 — Entity 难做 unit test，纯函数模拟 + 行为单测

**采**：
- 状态机用 pure fn 抽出（如 `fn next_pressing_state(cur: bool, mouse_down: bool) -> bool`），用 pure fn 单测覆盖
- 字段访问 / builder 配置仍可单测（创建 Entity 时 cx.new 在 test 内不
  好搞 — 看 GPUI 是否有 test app harness）
- 复杂行为（动画播放 / focus ring fade）靠手测覆盖

**拒**：引入 GPUI test harness 单独跑 view test — GPUI 测试基建薄，aish-ui
现有单测都是 pure fn 模拟（见 dialog.rs M30 transition table 测试模式）

---

## 4. 架构变化总览

```
+-----------------------------------------------------+
| aish-ui/src/components/button.rs                    |
|   pub struct Button {                               |
|     id: ElementId,                                  |
|     label, variant, disabled, on_click,             |
|     focus_handle: FocusHandle,    (cx.focus_handle) |
|     pressing: bool,               (M31)             |
|     focus_animated: bool,         (M31)             |
|     was_focused_prev: bool,       (M31)             |
|   }                                                 |
|   impl Render for Button { ... }                    |
+-----------------------------------------------------+
| aish-ui/src/components/icon_button.rs               |
|   同 Button 对称重构                                  |
+-----------------------------------------------------+
| aish-app callsite × 19：                            |
|   - per-view 单例 button → view struct 持 Entity     |
|   - Vec/HashMap 渲染（home / tab_bar）→ retain helper|
+-----------------------------------------------------+
| Animation：                                          |
|   - press feedback: animate_or_skip 80ms opacity     |
|     0.85→1.0 ease_out_quint                         |
|   - focus ring fade: animate_or_skip 80ms opacity    |
|     0→1 ease_out_quint                              |
+-----------------------------------------------------+
```

---

## 5. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | 35 callsite 改造遗漏 / 编译破坏 | 中 | T2 加 entity 但保留 stateless deprecated，逐 task 改 callsite；T5 删 stateless 时编译器报错兜底所有 missing callsite |
| R2 | per-key Entity HashMap 内存泄漏（host 删除后 Button entity 不释放） | 中 | retain_alive_entities(M22) helper 每帧扫除 stale key — host_form.rs 已有先例 |
| R3 | pressing 80ms timer race — 用户连点 5 次 button，5 个 timer pending | 低 | 进入 pressing=true 时不重置 timer（timer fire 时 check pressing 仍 true 才清）+ cx.spawn detach 模式（weak.update 失败安全） |
| R4 | focus ring fade 在 Tab 焦点链快速移动（host_form 6 input + 2 button Tab 循环）时 8 个 Button 同时 animate | 低 | 每个 animate 仅 80ms 8 个 button 并发 < 200µs/frame，GPUI 测试无瓶颈；reduced_motion ON 时全跳过 |
| R5 | 重构期 main 临时不可编译（巨大 PR 中途 push） | 中 | 用 worktree 隔离实施分支，验证可编译再 merge；不允许 partial commit 跨 task |
| R6 | per-card Button Entity 频繁创建 / 销毁性能（home 100 hosts 滚动） | 低 | 当前 home 无虚拟滚动，hosts 全量 render，HashMap 持 entity 一对一稳定；100 entity 创建 < 1ms |
| R7 | `.focus_handle()` API 变化破坏 host_form M29 D-9 initial_focus 依赖 | 高 | Button.focus_handle() 改为返回 `&FocusHandle`（Entity 内字段持有，不再 Option），caller 现有 `.focus_handle(handle)` 注入 API 改为 `.set_focus_handle(handle)` 或保留 builder 但内部 set Entity 自身 focus_handle |
| R8 | Animation ElementId `("motion-btn-press", button_id)` 冲突 — 多 button 同 id 在不同 view | 低 | ElementId 内嵌 button.id（caller 给的），按 GPUI 规则同 element-tree-path 下唯一即可；toast/host_form 已经用 `(name, runtime_id)` tuple 模式 |

---

## 6. Out of scope

- **hover transition** — 保留 GPUI `.hover()` instant 切色（M30 D-3 / M31 D-8）
- **focus ring fade-out** — 失焦直接消失（D-3 简化）
- **scale press feedback** — GPUI 限制（D-2 用 opacity）
- **TabItem indicator slide** — M30 T6 defer，留 M32+
- **Dropdown / Select / Switch 等其他 stateless 组件** — 不在 M31 范围
- **新 Light theme polish** — 现有 dark/light token 不动
- **Button 接 IconLabel 双 slot**（icon + label 并列）— 当前 Button 只接 label，不引入新 layout

---

## 7. 测试策略

### 单测（aish-ui）

T2 加：
- `Button::default_not_pressing`：构造后 pressing=false
- `pressing_state_transition`：pure fn 模拟 mouse_down → pressing=true，
  timer fire → false
- `focus_ring_animator_starts_only_on_focus_gained`：pure fn 模拟
  prev=false, cur=true → animate; prev=true, cur=true → no animate; cur=false → no animate
- `press_opacity_animator_at_endpoints`：animator(el, 0.0) → opacity 0.85，
  animator(el, 1.0) → opacity 1.0 — 数学断言（用 lerp_px 验证或 inline）
- `entity_dropped_drops_animations`：测试 Entity drop → cx.spawn weak.update fail
  → timer 自动安全退出（type-level / pure fn 模拟）

### 集成（手测 checklist）

T5 收尾时跑：
- [ ] 点 InputBar Send：button opacity 80ms 暗→亮过渡
- [ ] Tab 焦点切到 HostForm Save：focus ring 80ms 渐显（虚线 ring 围着）
- [ ] reduced_motion ON：所有 button instant，无动画
- [ ] home 10+ host card 滚动：每张 card 的 open/edit/delete button press 都正常
- [ ] tab_bar 多 tab 关闭 button：close 后 entity 释放（retain 生效，无残留）
- [ ] 连点 5 次 Save button：timer 不死锁，pressing 状态机正常

### 质量门禁

每个 task commit 跑：
```
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 8. Plan 引用

见 [`../plans/2026-05-15-aish-m31-button-stateful.md`](../plans/2026-05-15-aish-m31-button-stateful.md)

---

## 9. 实施记录

### Commits

| Task | Commit | 摘要 |
|---|---|---|
| T1 | `f1d9bb2` | ButtonEntity 旁挂 + 9 个 pure fn 单测（pressing state / focus animator / opacity endpoints） |
| T2 | `98a380c` | IconButtonEntity 旁挂同 T1 模式，复用 button.rs 的 pick_button_colors / press_opacity_at helper |
| T3 | `a171d0d` | aish-ui 内 dialog/toast 2 callsite 迁新 API；toast close_buttons HashMap retain 同步 |
| T4(p1) | `4ff730d` | aish-app 5 view 单例 9 callsite（empty_terminal / settings / terminal_view / input_bar / home single） |
| T4(p2) | `76f899a` | host_form 6 callsite 收尾；delete_cancel_focus 改从 entity.focus_handle() 取 |
| T5 | `91f1979` | home host_card_buttons + session_open_buttons / tab_bar close_buttons HashMap + retain helper 模式 |
| T6 | `4608899` | 删 stateless Button/IconButton + rename Entity → 简洁名；callsite sed 批量 rename |

### 实施期发现 / 调整

- **plan T1 调整为旁挂模式**：原 spec D-4 设想 "rename stateless → ButtonLegacy
  + entity 沿用 Button 名" 会让 T1 commit 后 main 立即不可编译，违背 CLAUDE.md
  "每 task 跑质量门禁"。提交 `ad6cae6` 改 plan：T1-T5 期间 stateless `Button` +
  `ButtonEntity` 并存，T6 删 stateless 同时 rename Entity 回 Button。每 task
  完成 main 可编译。

- **R7 实际验证**：M29 D-9 `Dialog::initial_focus(handle)` 依赖 Button 的
  focus_handle。新 entity 内置 focus_handle (cx.focus_handle() in new())，
  caller 通过 `button.read(cx).focus_handle()` 取出作为 dialog initial_focus
  参数 — 兼容 M29 行为。host_form delete_cancel_focus 字段被移除，改从
  delete_cancel_btn.read(cx).focus_handle() 直接取。

- **R-spec-render：AnyElement 不可二次 with_animation**：原计划 press
  feedback + focus ring fade 各自一层 animate_or_skip。实施 T1 时验证
  `AnyElement` 不实现 Styled，无法 chain `.opacity()` / `.shadow()`。改为
  单 animate_or_skip wrapper 内 closure 同时处理两路（pressing → opacity，
  focus_animating → shadow alpha 0→0.4）；ring_show_static 路径在 closure
  内静态挂 shadow。两态 80ms duration 共用同 animator。

- **press feedback 用 opacity 0.85→1.0**：spec D-2 已决策（GPUI 限制
  L4 / R4 — div 无 transform translate / scale，仅 svg 有 with_transformation）。
  实施验证 — opacity 视觉清晰，无副作用。

- **input_bar Send button 动态 disabled**：每帧 render 时
  `self.send_btn.update(cx, |b, _| b.label(...).disabled(...))` 同步状态，
  builder 改 `&mut self -> &mut Self` 是关键（D-7）。

- **R5 worktree 不需要**：plan T1 旁挂模式让每 task main 可编译，全程在
  main 实施，无需 worktree。

### 测试增量

| 文件 | 新增 / 删除 |
|---|---|
| button.rs | +9 M31 pure fn（pressing / focus_animator / press_opacity_at），-7 旧 stateless 单测 |
| icon_button.rs | -4 旧 stateless 单测（new_defaults / size_chains / focus_handle_* 等），保留 box_size_relationships |
| 净增 | aish-ui 262 → 260（-2，删 11 旧测 + 加 9 新测） |

### 视觉差异（手测）

- **press feedback**：所有 button mouse_down 触发 80ms ease_out_quint
  opacity 0.85 → 1.0 渐显，松开自动复位
- **focus ring fade-in**：Tab 切换 focus 时 ring 80ms opacity 0 → 0.4
  渐显（之前 instant box_shadow）
- **reduced_motion ON**：所有 button press / focus 立即出 end-state，
  无动画过渡
- **hover transition 不动**：保留 GPUI `.hover()` instant 切色（D-8）

### 不变量校验

- ✅ 35 callsite 全数迁移（grep `Button::new\|IconButton::new` 全部走
  `cx.new(|cx| Button::new(id, cx).label(...)...)` 模式）
- ✅ Vec/HashMap 渲染（home host card × N / tab_bar tab × N / toast × N）
  全部 retain_alive_entities 同步避免 entity 泄漏
- ✅ HostForm M29 D-9 `Dialog::initial_focus(button.focus_handle())` 兼容
- ✅ Settings reduced_motion toggle 切换后所有 button 同步生效
