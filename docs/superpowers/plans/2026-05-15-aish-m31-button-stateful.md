# M31 — Button stateful 重构 + press / focus 动画（Plan）

**Spec**: [`../specs/2026-05-15-aish-m31-button-stateful-design.md`](../specs/2026-05-15-aish-m31-button-stateful-design.md)

---

## File Structure

```
crates/aish-ui/src/components/button.rs           (RenderOnce → Render Entity)
crates/aish-ui/src/components/icon_button.rs      (同 Button 对称重构)
crates/aish-ui/src/components/mod.rs              (pub use 更新如需要)
crates/aish-ui/src/lib.rs                         (pub re-export 不变)

crates/aish-app/src/views/empty_terminal.rs       (1 callsite)
crates/aish-app/src/views/home.rs                 (5 callsite，含 per-card Vec)
crates/aish-app/src/views/host_form.rs            (6 callsite)
crates/aish-app/src/views/input_bar.rs            (2 callsite)
crates/aish-app/src/views/settings.rs             (2 callsite)
crates/aish-app/src/views/tab_bar.rs              (1 callsite，per-tab Vec)
crates/aish-app/src/views/terminal_view.rs        (1 callsite)
crates/aish-ui/src/components/dialog.rs           (1 callsite，dialog-close)
crates/aish-ui/src/components/toast.rs            (1 callsite，per-toast)
```

---

## Tasks（顺序，每条独立 commit）

### T1: 新增 ButtonEntity（旁挂，保留 stateless Button 不改）

**对应 ADR**: D-1 / D-2 / D-7 / D-9

**关键调整（vs spec 原设计）**：spec D-4 原说"rename stateless → `ButtonLegacy`
+ 新 entity 沿用名 `Button`"，会让 T1 commit 后 main 立即不可编译（违背
CLAUDE.md "每 task 跑质量门禁"）。**plan 改：ButtonEntity 旁挂模式** —
T1-T5 期间新旧两套 type 并存（不同名），T6 删 stateless 同时 rename
ButtonEntity → Button。每 task 完成 main 都可编译。

- 在 `button.rs` 内**并存**两套：
  - 旧：`pub struct Button` + `impl RenderOnce` **保持不动**（callsite 仍可用）
  - 新：`pub struct ButtonEntity { id, label, variant, disabled, on_click,
    focus_handle: FocusHandle, pressing: bool, focus_animated: bool,
    was_focused_prev: bool }` + `impl Render`
- 新 `ButtonEntity::new(id, cx)` 构造：cx.focus_handle()，pressing/focus_animated false
- builder 方法签名 `&mut self -> &mut Self`：caller 在 cx.new 闭包内 chain
- render 内部：
  - 计算 `now_focused = self.focus_handle.is_focused(window)`
  - 若 `!self.was_focused_prev && now_focused`：set `focus_animated=true` + spawn 80ms timer 清 false
  - 若 `self.was_focused_prev && !now_focused`：set `focus_animated=false`
  - update `self.was_focused_prev = now_focused`
  - 包动画路径（press + focus 两层）
- 加私有 helper `schedule_clear_pressing` / `schedule_clear_focus_animated`（cx.spawn weak.update 模式，与 M30 dialog.rs schedule_state_transition 一致）
- 加 pure fn 单测（D-9）：
  - `next_pressing_state(cur, mouse_down) -> bool`：mouse_down=true → true，
    mouse_down=false 且 cur=true → false
  - `focus_animator_should_start(prev, cur)`：(false, true)=true，其他 false
  - `press_opacity_animator_at_endpoints`：0.0→0.85，1.0→1.0
  - `press_opacity_clamped`：负值 / 超 1 时 opacity 保留端点
- **不**改 aish-app 的 callsite，旧 stateless `Button` 仍工作

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试 +4 ~ +5；aish-app 旧 callsite 不动 — 整个 main 仍可编译。

---

### T2: IconButtonEntity 旁挂（旧 stateless IconButton 不变）

**对应 ADR**: D-6

- `icon_button.rs` 同 T1 模式：旧 stateless `IconButton` 保持，新 `IconButtonEntity`
- pressing / focus_animated 字段 + 80ms animate 包装相同
- size (Small / Default / Large) / variant 字段保留不变
- 单测 +3：`icon_btn_pressing_default_false` / `icon_btn_focus_anim_transition` / press 端点

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试 +3；旧 callsite 不动 main 可编译。

---

### T3: aish-ui 内 dialog.rs + toast.rs 两处 IconButton callsite 迁新 API

**对应 ADR**: D-4 / D-5

- `dialog.rs` 内 `IconButton::new("dialog-close", IconName::X)` 改为 cx.new entity
  - Dialog 加字段 `close_btn: Entity<IconButton>`
  - new() 内构造 + cx.listener wire close handler
  - render 改用 `self.close_btn.clone()`
- `toast.rs` 的 per-toast close X 是 `Vec<Toast>` 渲染 — 用 D-5 模式：
  - ToastManager 加 `close_buttons: HashMap<u64, Entity<IconButton>>` （key = toast_id）
  - render 内 retain_alive_entities（按 toast_id 集合）+ HashMap entry 模式
  - 因 toast 已经在 aish-ui crate 内，retain_alive_entities helper 需提供给 aish-ui — 抽到 `crates/aish-ui/src/util.rs` 或先 inline 简化版

**质量门禁**: fmt + clippy + test 通过；手测 dialog X 关闭 / toast 关闭按钮仍工作 + 动画生效。

---

### T4: aish-app callsite 改造批 1 — 单例 button（11 处）

**对应 ADR**: D-4 / D-7

按 view 列表逐个改：
- `empty_terminal.rs` — go-home btn 单例：EmptyTerminalView 加 `go_home_btn: Option<Entity<Button>>`，按 conn 是否存在 lazy create
- `home.rs` 单例部分：`add_btn: Entity<Button>` + `retry_btn: Option<Entity<Button>>`
- `host_form.rs` 6 处：HostFormModal 加 6 个字段
  - `delete_cancel_btn: Entity<Button>` / `delete_confirm_btn: Entity<Button>`
  - `pick_keyfile_btn: Entity<Button>`
  - `host_delete_btn: Entity<Button>` / `host_cancel_btn: Entity<Button>` / `host_save_btn: Entity<Button>`
  - render 时按需 .update apply label / disabled 状态
- `input_bar.rs` 2 处：InputBarView 加 `pick_btn` / `send_btn`
- `settings.rs` 2 处：SettingsView 加 `open_config_btn` / `open_github_btn`
- `terminal_view.rs` 1 处：reconnect btn — Disconnected 状态显示
- 本 task 后只剩 Vec/HashMap 渲染场景未改（T5 处理）

**质量门禁**: fmt + clippy + test 通过；手测 add host / save host / settings buttons 都正常；press feedback 在每个 button 80ms 暗→亮可见。

---

### T5: aish-app Vec / HashMap 渲染 button 改造（8 处）

**对应 ADR**: D-5

- `home.rs` per-card 3 button：
  - HomeView 加 `host_card_buttons: HashMap<HostId, HostCardButtons>`
  - struct HostCardButtons { open, edit, delete }
  - render 前 retain_alive_entities(host_ids) + entry().or_insert_with(cx.new(...))
- `tab_bar.rs` per-tab close：
  - TabBarView 加 `close_buttons: HashMap<TabId, Entity<IconButton>>`
  - render 前 retain（tab_ids 集合）+ entry pattern
- 验证 host 删除 / tab 关闭后 entity drop（无 Animation state 残留）

**质量门禁**: fmt + clippy + test 通过；手测：连续添加 5 个 host → 删除 3 个，HomeView host_card_buttons HashMap size 与 hosts 一致；连续开关 3 个 tab，close_buttons 同步清。

---

### T6: 删 stateless Button / IconButton + rename Entity → 简洁名

**对应 ADR**: D-4

- 删除 `pub struct Button` (stateless `RenderOnce`) + 单测
- 删除 `pub struct IconButton` (stateless `RenderOnce`) + 单测
- rename `ButtonEntity → Button`、`IconButtonEntity → IconButton`（VS Code
  symbol rename 或 sed 批量）— callsite 在 T3-T5 已用旁挂名，rename 后
  callsite 走 `Button::new(id, cx)` 新签名
- 同步 mod.rs / lib.rs re-export 收敛到单一 `Button` / `IconButton`
- 单测改用 Entity 构造（D-9 pure fn 模拟模式）— 或留少量 pure fn 替代

**质量门禁**: fmt + clippy + test 通过；aish-ui 测试不少于 T2 末态；clippy 0 warning。

---

### T7: 文档 + INDEX

- 更新 spec §9 实施记录（commits 表 + 实际 Risk + 测试增量）
- INDEX 顶部当前状态 + Milestones 加 M31 entry
- M30 INDEX entry 的 "Defer 到 M31" 部分标 "✅ M31 落地"

---

## Self-Review Checklist

- [ ] D-1 ~ D-9 决策每条都对应 task
- [ ] Risk R1-R8 在 task 内有 mitigation 落地
- [ ] T1 / T2 加 7~8 个 aish-ui 单测（pressing state / focus animator / opacity endpoints）
- [ ] T3 toast retain_alive_entities — 验证 toast 列表变化时无 stale entity
- [ ] T4 / T5 callsite 全数迁 ButtonEntity / IconButtonEntity，T6 rename 后无 stateless 残留
- [ ] T5 home host_card_buttons / tab_bar close_buttons HashMap 都用 retain_alive_entities
- [ ] commits 严格按 task 顺序；每个 commit fmt + clippy + test 通过
- [ ] commit message 末尾加 `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`
- [ ] **不走** worktree（plan T1 调整：旁挂模式让每 task main 都可编译）
- [ ] T6 删 stateless + rename 后 main 编译 + callsite 无 stateless 残留

---

## 实施顺序与依赖

```
T1 (ButtonEntity 旁挂，旧 stateless Button 不动) ──┐
                                                    ↓
T2 (IconButtonEntity 旁挂同 T1 模式) ───────────────┘
                                                       ↓
            ┌──────────────────────────────────────────┘
            ↓
T3 (aish-ui 内 dialog/toast callsite 迁新 API)
            ↓
T4 (aish-app 单例 button 迁移 11 处)
            ↓
T5 (aish-app Vec/HashMap 渲染 8 处 + retain helper)
            ↓
T6 (删 Legacy + 单测改造)
            ↓
T7 (文档 + INDEX)
```

T1 / T2 互不依赖可并行实施（不同文件），但 T3 必须等两者 done。T4 / T5
依赖 T3 才能用新 API。T6 是清理收尾，必须最后。

---

## 工程量估算

| Task | 文件 | 行数估 | 时间估 |
|---|---|---|---|
| T1 | button.rs | +200 (Entity + animator) / 删 0 | 0.5 天 |
| T2 | icon_button.rs | +180 同 T1 | 0.5 天 |
| T3 | dialog.rs / toast.rs | +30 / +50 (toast HashMap retain) | 0.5 天 |
| T4 | 7 个 view 文件 | +200 / -150 | 1 天 |
| T5 | home.rs / tab_bar.rs | +120 / -50 + retain helper | 0.5 天 |
| T6 | button.rs / icon_button.rs | -200（删 stateless + rename）+ test 重写 | 0.5 天 |
| T7 | spec + INDEX | +60 | 0.25 天 |
| **合计** | | | **~3.75 天** |

**实际估算可能**：3 ~ 5 天，取决于 GPUI test harness 探索难度（D-9 单测
怎么做 Entity 的 test）+ 实际遇到的 GPUI Animation API 边界。

---

## 关键不变量（实施期持续校验）

- **API surface 简洁性**：caller 改造后 builder chain 仍流畅
  `cx.new(|cx| Button::new(id, cx).label(...).primary().on_click(...))`，不
  让 D-7 builder 流畅性退化
- **reduced_motion 整树生效**：所有 animate_or_skip 调用走 motion token，
  Settings toggle reduced_motion 后 button press / focus 立即跳过动画
- **focus_handle 兼容 M29 D-9**：HostForm dialog `.initial_focus(label_input.focus_handle)`
  仍工作 — Button.focus_handle() 返回 self.focus_handle.clone()（与 M15
  外部注入语义保持向后兼容）
- **callsite 改造期 main 可编译**：T1 旁挂模式让所有 task 完成 main 都
  能编译 + 通过质量门禁，不允许 partial state 跨 task
- **press timer 不死锁**：spawn timer 用 weak entity update + check pressing
  仍 true 才清，连点不死锁（R3）
