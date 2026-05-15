# M30 — 动画 / micro-interaction（Plan）

**Spec**: [`../specs/2026-05-15-aish-m30-animation-design.md`](../specs/2026-05-15-aish-m30-animation-design.md)

---

## File Structure

```
crates/aish-ui/src/theme/motion.rs              (新文件 Motion token + animate_or_skip helper)
crates/aish-ui/src/animation.rs                 (新文件 lerp_hsla / lerp_px 工具)
crates/aish-ui/src/theme/mod.rs                 (pub use Motion / animate_or_skip)
crates/aish-ui/src/theme/tokens.rs              (Theme 加 motion + reduced_motion 字段)
crates/aish-ui/src/lib.rs                       (pub re-export Motion / animate_or_skip / lerp_* )
crates/aish-ui/src/components/dialog.rs         (OpenState 状态机 + opening/closing 动画)
crates/aish-ui/src/components/toast.rs          (render_toast slide-in 包 animate_or_skip)
crates/aish-ui/src/components/button.rs         (pressing 字段 + press-feedback scale + focus ring fade)
crates/aish-ui/src/components/icon_button.rs    (同上)
crates/aish-app/src/app_state_file.rs           (AppStateFile 加 reduced_motion: Option<bool>)
crates/aish-app/src/views/settings.rs           (Appearance section 加"减少动画" Switch)
```

---

## Tasks（顺序，每条独立 commit）

### T1: GPUI Animation 调研 + PoC spike

> **不写产线代码**，纯 PoC。结论已沉淀到 spec §2，本 task 仅做：
>
> 1. 在 sandbox view（home.rs 临时挂载）写一个最小 demo：div().bg() + with_animation
>    150ms opacity 0→1 ease_out_quint，验证渲染顺滑、CPU 占用稳
> 2. 写另一个 demo：手动维护 OpenState 三态机，open / closing 反向 lerp
>    delta，确认状态机思路可行
> 3. （optional）TabItem indicator slide spike — 若 1 天搞不定就 mark
>    D-T6 defer，更新 spec Out of scope
>
> commit message: `chore(m30): GPUI Animation 调研 + PoC spike（无产线代码）`
> 实际不 commit，作为 spec §2 调研结论的 inline 验证

**质量门禁**: 演示帧率稳 60Hz / 无 panic / 关掉 demo 后无残留 entity。

---

### T2: motion.rs + Motion token + animate_or_skip + lerp helper

- 新文件 `theme/motion.rs`：
  - `pub struct Motion { instant: Duration, fast: Duration, medium: Duration, slow: Duration, easing_standard: Rc<dyn Fn(f32) -> f32>, easing_standard_in: Rc<dyn Fn(f32) -> f32> }`
  - `impl Default for Motion`，4 档 `Duration::from_millis(0/80/150/250)` + easing 用 GPUI 内置 `ease_out_quint()` / `quadratic`
  - `pub fn animate_or_skip<E, F>(el: E, t: &Theme, id: impl Into<ElementId>, anim: Animation, animator: F) -> AnyElement where E: IntoElement + 'static, F: Fn(E, f32) -> E + 'static`
    - reduced_motion = true → animator(el, 1.0).into_any_element()（直接出 end-state）
    - reduced_motion = false → el.with_animation(id, anim, animator).into_any_element()
- 新文件 `animation.rs`：
  - `pub fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla`（4 个分量分别 lerp，clamp t ∈ [0, 1]）
  - `pub fn lerp_px(a: Pixels, b: Pixels, t: f32) -> Pixels`
- `theme/tokens.rs`:
  - `Theme` 加 `motion: Motion` 字段（与 typography / font_size 并存）
  - `Theme` 加 `reduced_motion: bool` 字段（默认 false）
  - `Theme::dark()` / `Theme::light()` 初始化时填 `Motion::default()` + reduced_motion: false
- `theme/mod.rs` + `lib.rs` pub re-export `Motion`、`animate_or_skip`、`lerp_hsla`、`lerp_px`
- 单测（aish-ui）：
  - `motion_defaults_correct_durations`：4 档 ms 值断言
  - `motion_easing_standard_monotone`：delta 0 / 0.5 / 1.0 → ease_out_quint 单调递增到 1
  - `lerp_hsla_endpoints_and_midpoint`：t=0 → a / t=1 → b / t=0.5 在中间
  - `lerp_px_endpoints_and_midpoint`
  - `animate_or_skip_reduced_motion_skips_animation`：reduced=true 时 animator 仅 call 一次 delta=1.0（用 RefCell<u32> counter）
  - `animate_or_skip_normal_returns_animation_element`：reduced=false 时返回 AnyElement 类型断言

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试 +5。

---

### T3: Dialog open / close 动画

- `components/dialog.rs` 现有字段 `open: bool` 升级为 `state: OpenState`：
  ```rust
  enum OpenState {
      Closed,
      Opening { started_at: Instant },
      Open,
      Closing { started_at: Instant },
  }
  ```
- 状态机 transition：
  | 当前 | 事件 | 新状态 |
  |---|---|---|
  | Closed | open() | Opening { now } |
  | Opening | open() | Opening（保留 started_at，no-op） |
  | Open | open() | Open（no-op） |
  | Closing | open() | Opening { now }（直接重启 enter，简化版） |
  | Closed | close() | Closed |
  | Opening | close() | Closing { now } |
  | Open | close() | Closing { now } |
  | Closing | close() | Closing（no-op） |
- `render()` 逻辑：
  - Closed → 返回 empty div
  - Opening → animate_or_skip medium 150ms ease_out_quint，animator 包 backdrop + content 设 opacity = delta, transform scale = lerp_px(0.96, 1.0, delta)
  - Open → 直接渲染 backdrop + content（无动画）
  - Closing → animate_or_skip medium 150ms ease_out_quint，animator 设 opacity = 1.0 - delta, scale = lerp_px(1.0, 0.96, delta)
  - Closing 完成（delta = 1.0）需要切到 Closed：cx.spawn 起 150ms timer，到时 update 进入 Closed + cx.notify
- Esc / backdrop close → 调 `close()` 走 Closing 路径，**不**直接置 Closed
- 单测：
  - `open_state_transition_table`：8 种 (state × event) 全枚举断言
  - `opening_then_close_starts_closing_from_current_state`：Opening 中 close() 切 Closing（简化版重置 timer，不做 delta 反向继承）
- 手测：HostForm 打开 / 关闭、SessionPicker 打开 / 关闭，观察 fade-scale 是否 subtle 不闪

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试 +2；手测 5 次开关 dialog 无残留 / 无闪。

---

### T4: Toast slide-in 动画

- `components/toast.rs` `render_toast(toast, cx, weak_mgr)`：
  - 取 `t.motion.slow` (250ms) + `t.motion.easing_standard`
  - 外层 div 包 `animate_or_skip` enter 动画：
    - animator: `|el, delta| el.opacity(delta).translate_x(lerp_px(px(20.0), px(0.0), delta))`
    - id: `("motion-toast-enter", toast.id as usize)`
- 退出不做动画（决策 D-4 Phase 3：直接消失，避免 L1 三态机 toast 队列复杂度）
- 单测：
  - `render_toast_with_reduced_motion_skips_animation`：reduced=true 时 animator 立即 delta=1.0 出 end-state（用 toast.rs 内 unit 测，间接断言：构造 ToastManager + Theme reduced_motion = true，render 调用不 panic 且无 with_animation 元素 — 实际靠 type-level check 而非 runtime；用 animate_or_skip 已有 T2 单测，T4 不再重测，仅手测）
- 手测：连续推 3 条 toast 观察依次 slide-in；reduced_motion ON 时直接出

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试不变（动画包装无新逻辑）；手测 toast slide 视觉顺滑。

---

### T5: Button / IconButton press feedback + focus ring fade

- `components/button.rs`：
  - 字段加 `pressing: bool`
  - on_mouse_down 内 set `pressing = true` + cx.notify；同时 cx.spawn 80ms timer 到时 set `pressing = false` + cx.notify
  - render 时若 pressing=true，外层套 animate_or_skip fast 80ms ease_out_quint，animator: `|el, delta| { let scale = lerp_px(px(0.97), px(1.0), delta); /* 用 transform 或 scale style */ el }`
    - **注意**：GPUI Styled 没有 `.scale(f32)` API；查 Styled trait 实测。如果没有，用 `translate` + 视觉等价 PoC 或先放弃 scale 用 opacity 0.8→1.0 替代（PoC T1 验证）
- focus ring（M15 现状是 `is_focused(window) → box_shadow(ring color, spread 2px)` 硬切）：
  - 加 `focus_animated: bool` flag 或干脆每帧 read focus 状态
  - 若 is_focused：包 animate_or_skip fast 80ms ease_out_quint opacity 0→1 让 ring 渐显
  - 失焦：直接消失（简化版，不做反向）
- `components/icon_button.rs` 同步处理
- 单测：
  - `button_pressing_flag_set_on_mouse_down`：模拟 mouse_down，pressing=true，timer 后 false（用 fake clock 或简化版断言 pressing 字段访问）
  - 实际 GPUI mouse 事件单测难度高，可能改为：测试 `set_pressing(true)` API 后 80ms 内 render 走 animate 分支（type-level 断言）
  - 弱化：T2 已覆盖 animate_or_skip，T5 仅断言 pressing 字段存在 + 默认 false
- 手测：Button click 时观察 press 反馈；focus tab 切到 Button 时 ring 渐显

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试 +1 ~ +2；手测 InputBar Send / HostForm Save / Settings reduce-motion Switch 三处按钮 press 视觉。

**风险点（spec R4）**：scale wrapper 不能影响父 flex 布局。如果发现 button 短暂"塌陷"挤压周围元素，回退到 opacity 0.85→1.0 单维度反馈。T1 PoC 期验证。

---

### T6 (optional): TabItem active indicator slide

> 标 optional —— T1 spike 若 1 天搞不定就 defer 到 M31

- 在 `views/tab_bar.rs` 或 `components/tab_item.rs` 内：
  - 当前每个 TabItem 自己渲染底部 2px primary line
  - 改为 TabBar 顶层维护 `active_index: usize` + `last_active_index: usize` + `active_changed_at: Instant`
  - 绝对定位一条 indicator 在 TabBar 内，位置 = lerp_px(last_active_x, current_active_x, delta) 用 medium 150ms
  - 每个 TabItem 不再自画 indicator，只画 bg / label
- 难点：
  - tab 宽度可变（rename 状态可能变宽），需要 prepaint 拿到每个 tab 的 bounds
  - 跨 element 关联，可能要 canvas 写入全局 map
- 单测：lerp_px 已在 T2 覆盖，indicator 数学逻辑用纯函数抽出来单测
- 手测：tab 切换观察 indicator 横向滑动

**质量门禁**: fmt + clippy + test 通过；spike 失败时本 task 跳过，spec / INDEX 标 defer。

---

### T7: Settings 加"减少动画"toggle + app_state 持久化

- `crates/aish-app/src/app_state_file.rs`：
  - struct 加 `pub reduced_motion: Option<bool>`（Option 兼容旧 toml）
  - load / save 路径处理 None 默认 false
- `crates/aish-app/src/views/settings.rs`：
  - Appearance section（M12 加的）下加新 row："减少动画" + Switch
  - on_change：写 app_state + cx.set_global 更新 Theme.reduced_motion
- 启动加载（app.rs 启动逻辑）：读 app_state.reduced_motion 写入 Theme.reduced_motion
- Theme 切 dark / light 不影响 reduced_motion（保持独立 bool）
- 单测：
  - `app_state_file_reduced_motion_roundtrip`：写盘 true / 读回 true / 默认 None → false

**质量门禁**: fmt + clippy + test 通过；aish-app 测试 +1；手测切换 toggle 后下一次开 dialog 验证瞬时

---

### T8: 文档 + INDEX

- 更新 spec §9 实施记录（commits + 测试增量 + 实际 risk）
- INDEX 加 M30 entry，记录：
  - 最终 task 编号 + commit SHA
  - 测试增量（aish-ui XXX → YYY，aish-app XXX → YYY）
  - GPUI 调研关键发现 take-aways
  - T6 是否做了，如未做 mark defer
- 更新"当前状态"段落

---

## Self-Review Checklist

- [ ] D-1 ~ D-7 决策每条都对应 task
- [ ] Risk R1-R8 在 task 内有 mitigation 落地
- [ ] T1 PoC 验证 GPUI Animation API 可工作 + scale 是否支持
- [ ] T2 单测覆盖 lerp + animate_or_skip + Motion default
- [ ] T3 Dialog 状态机 transition 表 8 种全覆盖
- [ ] T5 Button press scale 不破坏父 flex 布局（手测 InputBar Send）
- [ ] T7 reduced_motion 整树生效（cx.set_global 强制全树 re-render）
- [ ] commits 严格按 task 顺序，中文 message
- [ ] 每 commit 加 `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`

---

## 实施顺序与依赖

```
T1 (PoC spike) ──┐
                 ↓
                T2 (motion token + animate_or_skip + lerp)
                 ↓
         ┌───────┼──────────┐
         ↓       ↓          ↓
        T3      T4         T5
       Dialog  Toast     Button/IconButton
         └───────┼──────────┘
                 ↓
              T6 (optional, TabItem indicator)
                 ↓
              T7 (Settings toggle + persist)
                 ↓
              T8 (文档 + INDEX)
```

T3 / T4 / T5 互相独立可并行实施，但顺序由"用户最易感知 → 难感知"决定
（Dialog 最常打开，Toast 次之，Button press 最微小但最频繁）。T7 必须
在前面任一动画 task 完成后才有实际意义（否则 toggle 关掉无可观察）。

---

## 关键不变量（实施期持续校验）

- **Animation 不动现有 hover / active 行为**：M30 严格遵守 D-3，不替换
  `.hover()` / `.active()`，仅在 Dialog open / Toast enter / Button press
  这种**可见状态变化**节点加 Animation 包装
- **reduced_motion 是 binary switch**：true 时所有 Animation 直接 skip 到 end-state，
  不允许部分组件忽略；T2 的 animate_or_skip helper 是唯一入口
- **AnyElement 替换不破坏父 flex_1 / w_full**：T2 / T3 / T4 / T5 实施期手测
  父布局尺寸不变；Button scale 包装是最高风险点（spec R4）
- **现有 timer-based 动画（spinner / cursor blink / Toast cleanup）不动**：M30 仅管
  GPUI Animation 体系，与 spawn timer 并行存在
