# M33 — Card 升 Entity + hover transition（Plan）

**Spec**: [`../specs/2026-05-15-aish-m33-card-stateful-design.md`](../specs/2026-05-15-aish-m33-card-stateful-design.md)

---

## File Structure

```
crates/aish-ui/src/components/card.rs     (RenderOnce → Render Entity + hover transition)
crates/aish-app/src/views/home.rs         (host_cards HashMap + retain + update)
crates/aish-app/src/views/settings.rs     (3 Entity<Card> 字段 + update each frame)
```

3 个文件。

---

## Tasks

### T1: card.rs 升 Entity（含 hover transition）

- `pub struct Card { id, header, body, footer, variant, on_click, padding,
  /* M33 新增 */ hover_state, hover_anim_count, /* M31 同步加 */ pressing,
  press_count, focus_handle, focus_animated, was_focused_prev }`
- `new(id, cx)` 签名加 cx 参数构造 focus_handle / hover_state defaults
- builder `&mut self -> &mut Self`
- 加 `fire_press` / `schedule_clear_focus_anim` / `fire_hover` method
  （直接 copy button.rs 的对应实现，s/Button/Card/）
- impl Render：
  - 同 button.rs 模式：base bg = match hover_state，need_anim = pressing
    || focus_animating || hover_entering，animator wrapper 三路叠加
  - 但仅 on_click.is_some() 时挂 .on_hover / .on_mouse_down listener +
    cursor_pointer + active；无 on_click 时简化路径不挂 listener
- 删 stateless 5 个旧测试
- 加 4-5 个 hover 状态机 pure fn 单测（card 自身的，与 button 状态机
  等价但绑定 Card type — 实际上可以省略，因为 button.rs 已覆盖。
  优化：仅加 1-2 个 card-specific 验证 lerp_hsla 与 card bg 走通即可）

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试净 +2 ~ +3（-5 stateless
+7 hover）。

---

### T2: home.rs host card 改造

- HomeView 加 `host_cards: HashMap<HostId, Entity<aish_ui::Card>>` 字段
- new() 初始化 HashMap::new()
- render 顶部（已有 host_card_buttons retain 那段）扩展：同步 retain
  `host_cards`；ensure 每个 host_id 有 entry：
  ```rust
  if !self.host_cards.contains_key(&id) {
      let host_id = id;
      let weak = cx.weak_entity();
      let card = cx.new(move |cx| {
          let mut c = aish_ui::Card::new(format!("host-card-{}", host_id), cx);
          c.no_padding();
          c.on_click(move |_ev, _w, cx| {
              if let Some(this) = weak.upgrade() {
                  this.update(cx, |this, cx| this.handle_card_click(host_id, cx));
              }
          });
          c
      });
      self.host_cards.insert(host_id, card);
  }
  ```
- cards iter 内 build host_card 改造：
  - 取出 entity reference `let card = self.host_cards.get(&id).expect(...)`
  - body_row 内容 build 流程不变
  - 在 build 末尾 `card.update(cx, |c, _| { c.body(body_row); })` + 用
    `card.clone()` 嵌入外层 wrap div

**质量门禁**: fmt + clippy + test 通过；手测：添加 5 个 host → 删除 3 个，
HomeView host_cards HashMap size 与 hosts 一致；mouse hover host card 150ms
bg 平滑渐变。

---

### T3: settings.rs 3 Card 改造

- SettingsView 加 3 个字段：
  ```rust
  appearance_card: Entity<aish_ui::Card>,
  shortcuts_card: Entity<aish_ui::Card>,
  about_card: Entity<aish_ui::Card>,
  ```
- new(cx) 内 cx.new 构造，set static config（outlined + no_padding）
- render 每帧 update 重设 header / body：
  ```rust
  self.appearance_card.update(cx, |c, _| {
      c.header(section_header("Appearance", t));
      c.body(div().flex().flex_col().child(...).child(...));
  });
  ```
- render 内引用改 `self.appearance_card.clone()` / etc.

**质量门禁**: fmt + clippy + test 通过；手测 Settings 3 个 Card 显示正常，
hover 仍无切色（D-2 无 on_click）。

---

### T4: 文档 + INDEX

- spec §7 实施记录
- INDEX 顶部 + Milestones 加 M33 entry

---

## 实施顺序

```
T1 (card.rs 重构) ─→ T2 (home.rs) ─→ T3 (settings.rs) ─→ T4 (文档)
```

T1 commit 后 main 临时不可编译（callsite 还在用旧 stateless API），T2 / T3
完成后恢复。**采用临时 worktree-like 处理**：T1 commit 包含 stateless
删除 + Entity 加，T2 / T3 同 commit 内一起改 callsite — 实际上 T1 / T2 /
T3 都用一个大 commit 保证 main 始终可编译。

或：T1 旁挂模式同 M31（保留 stateless Card 标 deprecated，加 CardEntity
旁挂），T2/T3 改 callsite，T4 删 stateless rename。

**采用旁挂模式**（M31 已验证）：
- T1: 加 CardEntity 旁挂（不动 stateless Card）；测试 +2
- T2: home.rs callsite 改 CardEntity
- T3: settings.rs callsite 改 CardEntity
- T4: 删 stateless Card + rename CardEntity → Card + 删旧测试 + 文档

每 task main 可编译。

---

## 工程量估算

| Task | 文件 | 行数估 | 时间估 |
|---|---|---|---|
| T1 | card.rs | +250 / -0 | 0.5 天 |
| T2 | home.rs | +50 / -10 | 0.25 天 |
| T3 | settings.rs | +60 / -15 | 0.2 天 |
| T4 | 删 stateless + 文档 | -100 / +50 | 0.15 天 |
| **合计** | | | **~1.1 天** |
