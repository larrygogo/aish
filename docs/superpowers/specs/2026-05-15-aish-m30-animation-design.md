# M30 — 动画 / micro-interaction 体系

**日期**: 2026-05-15
**父 spec**: [`2026-05-15-aish-m24-visual-redesign-design.md`](2026-05-15-aish-m24-visual-redesign-design.md)
**目标**: 把当前所有 hover / focus / open / close 的 instant snap 行为
升级到 Linear / Warp 风的 subtle transition（hover ~80ms、dialog open ~150ms、
toast slide ~200ms 等），同时建立可扩展的 duration / easing token 体系
**预计工程量**: 2 ~ 3 天，T1 调研 + 原型 → T2 token + 基础设施 → T3-T6 接入

---

## 1. 动机

aish 视觉骨架（M24 配色 / M25 加密度 + elevation / M26 typography）已成型，
但**所有交互反馈仍是 instant snap**：
- Button / IconButton 的 `.hover(|s| s.bg(hover_bg))` 鼠标进入瞬间换色
- Card / NavItem / TabItem 同上
- Dialog `open()` 立刻显示，没有 fade / scale 入场
- Toast 出现 / 消失瞬间切换，没有 slide / fade
- TabItem active 切换 indicator 条无 transition
- 上传 batch 完成、连接状态切换全是 hard cut

对比 Linear / Warp / VS Code 等成熟商业级 dev tool：所有 hover 都有 60-100ms
的 ease-out 过渡，dialog / popover 入场带 100-200ms 的 fade-scale，toast
slide-in 150-250ms。这种 subtle motion 让界面"活"起来，而不是 stop-motion 动画。

**codebase audit**（grep `.hover(` + `.active(`）：
- 17 文件含 hover / active 调用（aish-ui 6 个组件 + aish-app 5 个 view + spinner / scroll_page / select / input_bar 几处）
- 全部用 `Styled::hover` / `Styled::active` 即时切色，无任何 transition
- aish-ui 内**没有**任何对 `gpui::Animation` / `AnimationExt::with_animation` 的调用
- 唯一接近"动画"的是 InputBarView 的 Braille spinner（手写 80ms timer +
  cx.notify 推 phase 0..10）、TextInput cursor blink（600ms 周期 + 100ms
  notify timer）、Toast 自动 dismiss（100ms cleanup timer + 3s duration） —
  全是 phase / step 推进，**没有连续值插值**

---

## 2. GPUI 动画能力调研结论

### 2.1 内置 API（确认可用）

源码 `gpui/src/elements/animation.rs`（zed main 11f0ca5）

**核心 API**:

```rust
pub struct Animation {
    pub duration: Duration,
    pub oneshot: bool,
    pub easing: Rc<dyn Fn(f32) -> f32>,
}

pub trait AnimationExt {
    fn with_animation(
        self,
        id: impl Into<ElementId>,
        animation: Animation,
        animator: impl Fn(Self, f32) -> Self + 'static,
    ) -> AnimationElement<Self>;

    fn with_animations(/* chain version */) -> AnimationElement<Self>;
}

impl<E: IntoElement + 'static> AnimationExt for E {}
```

**内置 easing**（`gpui::easing::*`）:
- `linear(d) = d`
- `quadratic(d) = d * d`
- `ease_in_out(d)`（quad in-out）
- `ease_out_quint() -> impl Fn`（1 - (1-d)^5，常用 UI ease-out）
- `bounce(easing)`（先正后逆 trip）
- `pulsating_between(min, max)`（sin 呼吸曲线）

**工作机制**:
1. `with_animation(id, anim, animator)` 包成 AnimationElement，element 必须有 id
2. 每次 paint 时 `Element::request_layout` 内基于 `Instant::now() - state.start` 算 delta ∈ [0, 1]
3. 调 `easing(delta)` 得 eased delta，传给 animator 闭包 `Fn(Self, f32) -> Self`
4. 内部调 `window.request_animation_frame()` 直到 delta > 1（oneshot 模式停在 1.0；非 oneshot `delta %= 1.0` 循环）
5. delta 由 element state 持有，相同 ElementId 跨帧复用

**用法示例**:
```rust
div()
    .id("my-card")
    .bg(t.colors.card)
    .with_animation(
        "card-fade-in",
        Animation::new(Duration::from_millis(150)).with_easing(ease_out_quint()),
        |this, delta| this.opacity(delta),
    )
```

### 2.2 关键限制（重要！）

**L1 — 仅 enter 动画，无 exit / reverse**

`AnimationElement` 一旦元素从树上 take 掉，state（start Instant）就消失。
**没有原生 exit 动画机制** —— 要做 dialog close / toast slide-out 需要：
- 自己维护 `state: Open / Closing / Closed` 三态
- Closing 期间元素**仍渲染**（保持挂在树上），用 `with_animation` 跑反向 lerp
- Animation 到 1.0 后用 cx.spawn timer 切到 Closed 真正 unmount
- 复杂度高，原型 PoC 后决定哪些组件值得做 exit 动画

**L2 — 无 hover 触发的过渡**

`Styled::hover(|s| s)` 是 instant style swap（gpui style refinement），
GPUI 内置**没有** CSS `transition: bg 80ms` 这种"状态变化时插值到新值"机制。
要做 hover transition 需：
- 在 view 内自己存 `hover_state: bool` + `hover_change_at: Instant`
- on_mouse_move 上 set state、record now
- render 用 `(now - change_at) / duration` 算 delta，lerp idle → hover 颜色
- 复杂度极高，不可能在 17 处全部手写。

**M30 决策**：放弃 hover transition，**仅做 enter 动画 + 可见状态变化的动画**
（dialog open / toast slide-in / button press-feedback 等）。hover 保留 instant
切色（D-3 详述）。

**L3 — 无 lerp / color interpolation helper**

`Hsla { h, s, l, a }` 4 个 f32 字段公开，自己写 `lerp_hsla(a, b, t)` 是 9 行
代码。没有 GPUI 提供。Pixels 同理 `lerp(a, b, t)`。

**L4 — Animation 一次性 ID 复用问题**

`with_animation(id, anim, ...)` 的 ElementId 在 element state 内绑定 start
Instant。**同一帧内同 ID 复用 state**，跨条件分支 unmount/remount 时
state 重置 —— 这意味着"再次 mount 就再次播放"，正合 dialog open 用例。

**L5 — request_animation_frame 节流**

`window.request_animation_frame()` 走 `cx.notify(entity)` 触发下一帧 redraw，
**理论上 60-120 FPS**（platform 决定），实测在 Win 上稳 60Hz。150ms 动画
约 9 帧、80ms 约 5 帧，足够顺滑。

### 2.3 不可行 / 不做

- **物理动画（spring）**：GPUI 无内置 spring，要么手写 damped harmonic
  oscillator（lerp 升级到二阶 ODE），要么接 `interpolation` crate —— M30
  Out of scope，先 ease-out 起步
- **layout transition**（width / height / position 持续插值）：GPUI taffy
  layout 不支持中间值 fluid resize；Animation 改 `width()` 会让父 layout
  每帧重算，性能未测，先不做
- **shared element transition**（hero animation）：需要跨 element 关联，
  GPUI 无 layoutId 机制，不做
- **基于 transform 的位移**：GPUI `Styled::translate_x/y` 存在，理论上
  可 lerp，但与 layout flex 混用细节多。M30 仅 opacity / scale 安全

---

## 3. 决策记录（ADR-style）

### D-1: duration token — 4 档语义

**采**：在 `theme/motion.rs` 加 `Motion` struct，4 档命名 duration token：

| Token | 值 | 用途 |
|---|---|---|
| `instant` | 0 ms | opt-out / accessibility reduced motion 模式 / 测试 |
| `fast` | 80 ms | press-feedback / focus ring 出现 / icon swap |
| `medium` | 150 ms | dialog / popover open / fade-in / scale-in |
| `slow` | 250 ms | toast slide-in / page transition（保留扩展位） |

理由：4 档对应主流 UI 系统（Material `short1 50ms / short2 100ms / medium1 150ms / long1 250ms` 简化版、
Tailwind `duration-75/150/200/300`、shadcn）。`fast 80ms` 是
Linear hover transition 经验值；`medium 150ms` 是 dialog enter；
`slow 250ms` 给 toast slide。`instant 0ms` 关键 —— D-5 减少动画偏好时
所有动画 fallback 到这一档（直接 skip Animation 包装，详见 D-5）。

**拒**：
- 单值常量（`const HOVER_MS: u64 = 80`） — 不语义化，难扩展 dark/light 主题差异（未来可能 light 主题略慢提升清晰）
- 5 档以上 — 选择困难，对 dev tool 风非动画驱动产品过度
- 命名 `xs/sm/base/lg`（与 FontSize 撞 namespace）

### D-2: easing token — 默认 ease_out_quint

**采**：`Motion.easing_standard` = GPUI 内置 `ease_out_quint()`（1 - (1-d)^5）。

理由：UI 动画 95% 场景是 ease-out（启动快、停止慢，符合"物体从外飞入屏幕"
直觉，Material 称 standard easing）。Quint 比 Quad 后半段更平稳，常用于
Linear / Stripe。GPUI 内置不用自己写。

附加：
- `Motion.easing_standard_in` = `quadratic()`（淡出场景，速度慢→快）
- `Motion.easing_emphasized`（保留 None，先用 standard，将来按需加）

**拒**：每个动画 caller 自己传 easing —— 失去 design system 统一性。

### D-3: hover transition 不做，仅做 press / enter

**采**：M30 **不动** `.hover(|s| s.bg(...))` 现有代码。理由见 L2：
- 17 处 hover 全部要替换为自管 state + on_mouse_move + lerp + cx.notify
  改动量太大，回归风险高
- hover 80ms 在 dev tool 场景增益边际（最直观的反馈来自 click），
  不如先把 click feedback / dialog open 做完看效果再回头
- GPUI 的 `Styled::hover` 是声明式 style refinement，与 imperative Animation
  机制不兼容，强行融合需要修改 div.rs 上游

**M30 做的**：
- press-feedback（D-4）：Button mouse_down 时 scale(0.97) ease-out 80ms 再回弹（与 hover 解耦，靠 `.active()` 之外加 wrapper）
- dialog enter：opacity 0→1 + scale 0.96→1.0 medium 150ms ease-out
- toast slide-in：translate_x +20px→0 + opacity 0→1 slow 250ms ease-out
- focus ring fade-in：opacity 0→1 fast 80ms（M15 focus ring 现在直接 box_shadow，需重构）

**留**：hover transition 留 M31+；先把 enter / press 做完看体感。

### D-4: 接入范围（5 个组件）

**Phase 1 — 基础设施**（T1 + T2）:
- `theme/motion.rs` Motion token
- `motion::Animated` helper / Macro 包一层：根据 reduced_motion 决定走 Animation 或 skip
- `lerp_hsla(a, b, t)` / `lerp_px(a, b, t)` 工具函数

**Phase 2 — Dialog open animation**（T3）:
- `dialog.rs` 内 `open()` 状态从 bool 升级为 `OpenState::Closed / Opening / Open / Closing`
- Opening / Closing 期间用 `with_animation` opacity + scale lerp
- Esc / backdrop close 走 Closing 路径，medium 150ms 后 unmount

**Phase 3 — Toast slide-in**（T4）:
- `toast.rs` render_toast 每条包 `with_animation` slow 250ms enter
  （translate_x + opacity）
- exit 不做（toast 自动 dismiss 时直接消失，避免 L1 三态机复杂度爆炸；
  X 按钮 click 同样直接 dismiss）
- 多 toast 堆叠时新 toast 单独 enter，旧 toast 不变（flex_col_reverse 内
  flow 自然下移）

**Phase 4 — Button press feedback**（T5）:
- `button.rs` / `icon_button.rs` render 时如果 `mouse_down` 状态为 true，
  外层包 `with_animation` fast 80ms scale 0.97→1.0 ease-out
- 不接入 hover（D-3）
- focus ring 出现也走 fast 80ms opacity fade（M15 是 hard box_shadow，
  这里改成 `with_animation` 80ms 0→1.0 alpha）

**Phase 5 — TabItem active indicator slide**（T6，可选）:
- 当前 TabItem 底部 2px primary line 是 instant 切换 active tab
- 加 medium 150ms ease-out 让 indicator 在 tab 之间"滑过去"
- **风险高**：indicator 在不同 tab 元素内，跨 element 关联难（L1 shared transition 限制），
  实现可能要在 TabBar 外层维护 active_index_animated state + 绝对定位
  indicator —— 工程量超出 M30，先 spike 评估再决定是否进 M30 / 延后

### D-5: opt-out 机制（accessibility — reduced motion）

**采**：Theme 加 `reduced_motion: bool` 字段（默认 false），存入 `app_state.toml`，
Settings 加切换。所有 motion 工具函数 / Animation 包装入口处先看 reduced_motion：
- true → 跳过 `with_animation`，直接渲染 end-state 元素（duration = 0ms，
  视觉等于现在的 snap 行为）
- false → 走 Animation 路径

实现：抽 `fn maybe_animate<E>(element: E, t: &Theme, anim: Animation, animator: F) -> AnyElement`
helper：

```rust
pub fn animate_or_skip<E, F>(el: E, t: &Theme, anim: Animation, animator: F) -> AnyElement
where
    E: IntoElement + 'static,
    F: Fn(E, f32) -> E + 'static,
{
    if t.reduced_motion {
        animator(el, 1.0).into_any_element()
    } else {
        el.with_animation("...", anim, animator).into_any_element()
    }
}
```

理由：
- WCAG 2.3.3 "Animation from Interactions" 指明 essential motion 应可禁用
- 系统级"减少动画"偏好（macOS NSAccessibilityRequestUserAttention / Win
  SystemParametersInfo SPI_GETCLIENTAREAANIMATION）M30 不自动检测（Out
  of scope），先做应用内手动 toggle，下个 milestone 加系统检测

**拒**：每 caller 自己 if reduced_motion — 重复 17 次模板代码，违反 DRY。

### D-6: Animation ID 命名规范

**采**：所有 ElementId 用 `"motion-{component}-{purpose}"` 前缀：
- `"motion-dialog-enter"`、`"motion-toast-{toast_id}-enter"`、
  `"motion-button-press-{btn_id}"`、`"motion-tab-indicator"`

理由：
- ElementId 在 GPUI element tree 内必须唯一（同帧内）
- 前缀 `motion-` 让 inspector / 调试日志一眼能挑出动画元素
- toast / button 这类多实例的，附加自身 id 防冲突

### D-7: 不引入新 dependency

GPUI 内置 Animation + easing 已够覆盖 D-1 ~ D-6 全场景。
拒绝引入 `interpolation` / `springy` 等 spring 库 —— Out of scope。
新增代码仅在 `aish-ui/src/theme/motion.rs` + `aish-ui/src/animation.rs`
（lerp helper）两个文件，零外部 crate。

---

## 4. 架构变化总览

```
+--------------------------------------------------------+
| theme/motion.rs (新增)                                  |
|   pub struct Motion {                                  |
|     instant: Duration (0ms)                            |
|     fast: Duration (80ms)                              |
|     medium: Duration (150ms)                           |
|     slow: Duration (250ms)                             |
|     easing_standard: Rc<dyn Fn(f32) -> f32>            |
|     easing_standard_in: Rc<dyn Fn(f32) -> f32>         |
|   }                                                    |
|   pub fn animate_or_skip<E, F>(el, t, anim, f)         |
|     -> AnyElement                                      |
+--------------------------------------------------------+
| theme/tokens.rs                                        |
|   Theme 加 motion: Motion 字段                           |
|   Theme 加 reduced_motion: bool（默认 false）             |
+--------------------------------------------------------+
| animation.rs (新增)                                     |
|   pub fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla   |
|   pub fn lerp_px(a: Pixels, b: Pixels, t: f32) -> Pixels|
|   单测覆盖端点 + 中点                                     |
+--------------------------------------------------------+
| components/dialog.rs                                   |
|   OpenState { Closed / Opening / Open / Closing }      |
|   render: Opening/Closing 期 animate_or_skip(...)      |
|   open()/close() 推状态机                                |
+--------------------------------------------------------+
| components/toast.rs                                    |
|   render_toast 每条包 animate_or_skip slide-in         |
+--------------------------------------------------------+
| components/button.rs + icon_button.rs                  |
|   字段加 pressing: bool（mouse_down 推 true）            |
|   pressing=true 时 animate_or_skip scale 0.97→1.0      |
|   focus ring 改 animate_or_skip opacity 0→1            |
+--------------------------------------------------------+
| views/settings.rs                                      |
|   Appearance section 加 "减少动画" Switch                |
|   存 app_state.reduced_motion → 写盘                    |
+--------------------------------------------------------+
| app_state_file.rs                                      |
|   AppStateFile 加 reduced_motion: Option<bool>          |
+--------------------------------------------------------+
```

---

## 5. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | Animation 跨帧 60 FPS 在 17 处同时触发性能瓶颈 | 低 | M30 仅做 enter / press 一次性动画，不持续；每次只有 0-2 个动画并发；GPUI request_animation_frame 是 view-local notify 不全树重画 |
| R2 | Dialog Closing 三态机 bug — 用户快速 open/close 触发竞态 | 高 | OpenState 状态机内 transition 表显式列出（Closing→Opening 中断保留 delta 反向）+ 单测覆盖 4 种边沿（open-during-closing / close-during-opening / close-during-closing / open-during-open） |
| R3 | Toast 多条同时 slide-in 视觉乱（堆叠错位） | 中 | flex_col_reverse + 每条独立 with_animation id（含 toast_id）— 测试 3+ toast 连续推入观察 |
| R4 | Button press feedback scale 影响父 flex 布局（layout reflow） | 中 | scale 仅在 render 末端套 wrapper，wrapper 本身保持原 bounds；如果 caller 给 button 固定 w/h 不变；如果是 flex_1 撑开的 button，scale 0.97 可能短暂"塌陷" — T5 内手测 InputBar Send 按钮 / HostForm Save 按钮两处 |
| R5 | reduced_motion 切换后未即时生效 | 低 | Theme 是 global，写 settings 后 cx.set_global 整树 re-render；T5 测试切换 toggle 后下一次开 dialog 验证 |
| R6 | tab indicator slide (T6) 跨 element 关联复杂 | 高 | T6 标 optional —— T1 调研期做 spike，若 1 天内出不来可工作 PoC 就 defer 到 M31 |
| R7 | Animation ElementId 同帧冲突 | 中 | D-6 命名规范 + 多实例附加自身 id；T2 单测断言 id 不撞 |
| R8 | TextInput cursor blink / Spinner timer 与 Motion token 混用 | 低 | M30 不动现有 timer-based 动画（spinner / cursor blink），与 Animation 走两条路径；motion 仅管 Animation 包装的 enter / press 动画 |

---

## 6. Out of scope（M30 不做）

- hover transition（D-3 决策，留 M31+）
- exit / close 动画的全面铺开（仅 Dialog 做闭环；Toast 只做 enter；其他组件不做 exit）
- 物理动画（spring / bouncy）
- layout transition（width / height / position 持续 fluid resize）
- shared element / hero transition
- 系统级 reduced motion 自动检测（NSAccessibilityXxx / SPI_GETCLIENTAREAANIMATION），只做应用内 toggle
- TabItem active indicator slide（T6 标 optional，spike 后决定）
- 终端内容滚动动画 / 字符渐显（终端就是终端）
- 拖拽 tab 时的占位 slide / 重排动画（已有 drag preview 即可）
- 图标 icon swap 动画（fast 80ms 切换实测视觉无感，性价比低）
- DropdownMenu / Popover open animation（与 Dialog 同模式，M30 先做 Dialog，验证后 M31 复制粘贴接入；Popover 数量多 17 处 callsite，先稳一稳）

---

## 7. 测试策略

### 单测（aish-ui）

- `motion::Motion::default()` 4 个 duration 值正确（0/80/150/250 ms）
- `motion::Motion::default()` easing_standard 单调递增（delta 0 / 0.5 / 1.0
  对应 ease_out_quint 值递增到 1）
- `lerp_hsla(a, b, 0.0) == a` / `lerp_hsla(a, b, 1.0) == b` / `lerp_hsla(a, b, 0.5)` 在 a/b 中间
- `lerp_px(a, b, 0.5)` 同上
- `animate_or_skip` 在 `reduced_motion = true` 时不调用 `with_animation`
  （用 flag counter trick 间接断言：animator 在 reduced 模式下被 call 一次且 delta = 1.0）
- Dialog `OpenState` 状态机 transition 表：8 种 (current_state × event) 全覆盖
- Toast render_toast 在 reduced_motion = true 时不带 Animation wrapper

### 集成（手测）

- 切 dark / light + reduced_motion ON/OFF 四组合
- Dialog open / close 5 次连续点击观察是否闪屏 / 状态错乱
- Toast 连推 5 条观察 slide-in 是否清晰、堆叠是否正确
- Button click 时观察 press-feedback 时长合适不顿挫
- 切 Settings → 减少动画 ON → 再开 Dialog → 验证瞬时显示
- 截图对比：M29 vs M30 同样操作的 GIF（dialog open / toast slide / button press）

### 性能基线

- M30 完工后 `cargo run --release` 启动并打开 Home，cpu idle 时应保持 < 2% CPU
  （现状 Toast cleanup timer 100ms + cursor blink 100ms 已经在跑，
  Animation 仅 enter 一次性触发，不应增加 idle 负载）

---

## 8. Plan 引用

见 [`../plans/2026-05-15-aish-m30-animation.md`](../plans/2026-05-15-aish-m30-animation.md)

---

## 9. 实施记录

### Commits

| Task | Commit | 摘要 |
|---|---|---|
| T1 | （无 commit） | GPUI Animation 调研沉淀进 spec §2，PoC 验证 API 可用 |
| T2 | `857f456` | Motion token + animate_or_skip helper + lerp_hsla/px |
| T3 | `b88b7bd` | Dialog open/close 4 态机器 + fade 动画 |
| T4 | `e696d29` | Toast enter 动画 opacity 0→1 slow 250ms |
| T5 | **defer M31** | Button press feedback — stateless 限制无法持 timer，详见下文 |
| T6 | **defer M31** | TabItem active indicator slide — spec 已 mark optional |
| T7 | `41807c4` | Settings 减少动画 toggle + app_state 持久化 |

### 实际遇到的 Risk / 限制

- **R-spec L4 + R4 落地 — GPUI transform 不可用**：spec L4 说 transform translate
  "理论可用"；实施 T4 时验证 GPUI div **不**支持 `.translate_x/y` /
  `.with_transformation`（仅 svg 有）。Toast 原方案 opacity + translate 简化
  为 opacity-only。视觉上手测 250ms ease_out_quint 仍够 subtle。
- **R-T3 状态机 race condition**：Dialog 4 态机器引入 `schedule_state_transition`
  helper，timer fire 时做幂等 check（state == expected_prev 才推进），
  覆盖了 close→open→close 短间隔的 stale timer 场景。Self-Review 单测
  8 case 全 pass。Closing 期间禁用键鼠 listener 避免再次 close 进入死循环。
- **R-T5 stateless 组件限制（spec 未充分预料）**：Button / IconButton 是
  `#[derive(IntoElement)]` stateless 组件，无 Entity 持 `pressing: bool` +
  spawn timer。GPUI `.active()` 是 declarative style refinement，**不**支持
  嵌套 `with_animation`。要做 press scale 反馈必须把 Button 重构成 stateful
  Entity，工程量超 M30 — defer M31。
- **R-T6 跨 element 关联难度**：TabItem indicator slide 需要 TabBar 顶层
  维护 active_index_animated + 绝对定位 indicator，需要 prepaint canvas
  写入每 tab bounds 全局 map。spec 已 mark optional，按 spike 评估
  defer M31。

### 测试增量

| 文件 | 新增单测 |
|---|---|
| `theme/motion.rs` | 5（duration 默认 / 有序 / easing 单调 / quadratic 端点 / Rc clone 安全） |
| `animation.rs` | 5（lerp_hsla 端点+中点+clamp / lerp_px 端点+clamp） |
| `components/dialog.rs` | 9（M30 transition table 8 case + reduced_motion path） |
| `crates/aish-app/src/app_state_file.rs` | 2（reduced_motion roundtrip / default None） |

aish-ui: 242 → **261**（+19）
aish-app: 145 → **147**（+2）

### 视觉 / 行为差异（手测）

- **Dialog**：HostForm / SessionPicker open 时 backdrop + content 150ms 淡入；
  close 时同样淡出（之前 instant snap）
- **Toast**：右下角新 toast 250ms 淡入；多 toast 同时推时各自独立 enter
  动画 ID 不冲突
- **reduced_motion ON**：所有动画跳过，视觉等于现在的 instant snap；
  下次 cold start 也保留偏好

### Defer 到 M31 的 task

- T5：Button / IconButton press feedback（含 focus ring fade）— 需先把
  Button 重构成 stateful Entity，或换 GPUI 升级路径
- T6：TabItem active indicator slide — 跨 element 关联工程量超 M30
