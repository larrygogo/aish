# M32 — Hover transition v1（Plan）

**Spec**: [`../specs/2026-05-15-aish-m32-hover-transition-design.md`](../specs/2026-05-15-aish-m32-hover-transition-design.md)

---

## File Structure

```
crates/aish-ui/src/components/button.rs           (HoverState enum + on_hover + render lerp)
crates/aish-ui/src/components/icon_button.rs      (同 Button 对称)
```

仅 2 个文件。

---

## Tasks（顺序，每条独立 commit）

### T1: Button hover transition

**对应 ADR**: D-1 / D-2 / D-3 / D-4 / D-5 / D-7 / D-8

- `button.rs` 加 enum：
  ```rust
  enum HoverState {
      Idle,
      Entering { anim_count: u64 },
      Hovered,
  }
  ```
- Button 字段：
  - 删 `cursor_pointer / hover modifier`（render 内仍调用 cursor_pointer，但
    `.hover(|s| s.bg(hover_bg))` 移除）
  - 加 `hover_state: HoverState`（默认 `Idle`）
  - 加 `hover_anim_count: u64`（每次 Idle → Entering 时 +1，作 ElementId 区分）
- new() 内初始化 `hover_state = Idle`, `hover_anim_count = 0`
- 加 method `fire_hover(hovered: bool, cx: &mut Context<Self>)`：
  ```rust
  fn fire_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
      use HoverState::*;
      if hovered {
          if matches!(self.hover_state, Idle) {
              let reduced = theme(cx).reduced_motion;
              if reduced {
                  self.hover_state = Hovered;
              } else {
                  self.hover_anim_count = self.hover_anim_count.wrapping_add(1);
                  let expected = self.hover_anim_count;
                  self.hover_state = Entering { anim_count: expected };
                  let dur = theme(cx).motion.medium;
                  cx.spawn(async move |this, cx| {
                      cx.background_executor().timer(dur).await;
                      let _ = this.update(cx, |this, cx| {
                          // 幂等 check：anim_count 未变才 → Hovered
                          if matches!(this.hover_state, Entering { anim_count } if anim_count == expected) {
                              this.hover_state = Hovered;
                              cx.notify();
                          }
                      });
                  }).detach();
              }
              cx.notify();
          }
      } else {
          // 任意 → Idle（instant，D-1）
          if !matches!(self.hover_state, Idle) {
              self.hover_state = Idle;
              cx.notify();
          }
      }
  }
  ```
- render 内：
  - 移除 `.hover(|s| s.bg(hover_bg))` 调用
  - 加 `.on_hover(cx.listener(|this, &hovered, _w, cx| this.fire_hover(hovered, cx)))`
  - 选 hover-aware base bg：
    - `Idle` → `idle_bg`
    - `Entering` → 包 animate path，animator 内 `el.bg(lerp_hsla(idle_bg, hover_bg, delta))`
    - `Hovered` → `hover_bg`
  - 已有的 press / focus 三路 animator wrapper 内**也**需要 set bg（按 hover state 决定起始色）

  实施技巧：让 animator wrapper 内根据 hover_state + press + focus 综合算 bg / opacity / shadow。
- 单测：
  - `hover_idle_to_entering_on_enter`
  - `hover_entering_to_idle_on_leave`（中断 enter）
  - `hover_entering_to_hovered_on_timer_match`
  - `hover_entering_to_hovered_skip_on_count_mismatch`（leave-enter 期间）
  - `hover_reduced_motion_skips_entering`

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试 +4 ~ +5；手测 InputBar
Send / HostForm Save：mouse enter 150ms bg lerp 平滑 / mouse leave instant
切回。

---

### T2: IconButton hover transition

**对应 ADR**: 同 T1（D-1 ~ D-8 对称）

- icon_button.rs 同 T1 模式：加 hover_state / fire_hover / render lerp
- 单测：与 Button 共用 enum 逻辑，状态机模拟测试已被 T1 覆盖，
  icon_button.rs 不重复，仅靠 box_size_relationships 等基础测试
- 验证 R5：Ghost variant idle_bg = transparent_black，lerp 中间色
  半透明灰 — 手测 dialog close X / toast close X / host card
  edit/delete 等多处 Ghost IconButton

  若 Ghost lerp 视觉不佳，fallback：Ghost variant 跳过 hover transition，
  保留 instant（在 render 内 `if matches!(self.variant, Ghost) { ... instant 路径 }`）。

**质量门禁**: fmt + clippy + test 通过；手测 dialog close X / host card
edit/delete / tab close 各处 IconButton 都正常。

---

### T3: 文档 + INDEX

- 更新 spec §9 实施记录（commits + 实际 R5 Ghost lerp 评估 + 测试增量）
- INDEX 顶部当前状态 + Milestones 加 M32 entry
- backlog `hover-transition` 标 ✅ Button + IconButton 已落地，
  Card / NavItem / TabItem / list row 留 M33+（D-6）

---

## Self-Review Checklist

- [ ] D-1 ~ D-8 决策每条都对应 task
- [ ] R1-R5 mitigation 落地（特别 R5 Ghost lerp 视觉评估）
- [ ] T1 加 5 个 hover 状态机单测
- [ ] T1/T2 移除 `.hover(|s| s.bg(...))` 调用，自管 bg
- [ ] T1/T2 加 `.on_hover(...)` callback + fire_hover method
- [ ] render 内 animator wrapper 同时处理 hover lerp + press opacity + focus ring shadow
- [ ] commits 严格按 task 顺序；每个 commit fmt + clippy + test 通过
- [ ] commit message 末尾 `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`

---

## 实施顺序

```
T1 (Button) ─→ T2 (IconButton) ─→ T3 (文档)
```

T2 完全是 T1 的 mirror，按相同结构实施。T3 收尾。

---

## 工程量估算

| Task | 文件 | 行数估 | 时间估 |
|---|---|---|---|
| T1 | button.rs | +80 / -10（删 .hover） + 5 测试 | 0.5 天 |
| T2 | icon_button.rs | +80 / -10 | 0.25 天 |
| T3 | spec + INDEX | +50 | 0.1 天 |
| **合计** | | | **~0.85 天** |
