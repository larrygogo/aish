# M29 — HostForm Dialog 视觉重设计（Plan）

**Spec**: [`../specs/2026-05-15-aish-m29-host-form-redesign-design.md`](../specs/2026-05-15-aish-m29-host-form-redesign-design.md)

---

## File Structure

```
crates/aish-ui/src/components/radio.rs                  (新增)
crates/aish-ui/src/components/text_input.rs             (+error builder)
crates/aish-ui/src/components/dialog.rs                 (+initial_focus builder + 改 needs_focus 路径)
crates/aish-ui/src/components/mod.rs                    (pub mod radio)
crates/aish-ui/src/lib.rs                               (pub use Radio)
crates/aish-app/src/views/host_form.rs                  (整 view 重写 layout + Radio + delete_dialog)
docs/superpowers/INDEX.md                               (M29 entry)
```

---

## Tasks（顺序，每条独立 commit）

### T1: aish-ui — `TextInput.error(bool)` + `Dialog.initial_focus(handle)`

**对应 ADR**: D-7 / D-9（修订版）

- `components/text_input.rs`:
  - 加 `error: bool` 字段（默认 false）
  - 加 `pub fn error(mut self, e: bool) -> Self`（一致 builder 风）
  - render 内 border_color 选择：`if self.error { destructive } else if focused { ring } else { input/border }`
  - focus ring `box_shadow` color 同上 — error 时 ring 也走 destructive（alpha 0.4 维持 M24 风）
- `components/dialog.rs`:
  - 加 `initial_focus: Option<FocusHandle>` 字段
  - 加 `pub fn initial_focus(&mut self, h: FocusHandle) -> &mut Self`
  - render 内 `if self.needs_focus { ... }` 路径：
    ```rust
    if let Some(h) = self.initial_focus.as_ref() {
        h.focus(window, cx);
    } else {
        self.focus_handle.focus(window, cx);
    }
    ```
  - 不影响现有 `focus_chain` 行为（Tab 循环独立）
- 单测（新增 5 个，aish-ui）：
  - `text_input_error_border_color_destructive`：error=true，render 结构断言 border 走 destructive token
  - `text_input_error_overrides_focus_ring`：error+focused=true，border 仍走 destructive（不被 ring 覆盖）
  - `text_input_error_default_false`：默认未调 error builder 时 error=false
  - `dialog_initial_focus_setter_stores_handle`：set initial_focus 后字段非 None
  - `dialog_open_with_initial_focus_path_chosen`：open 后 needs_focus=true，且有 initial_focus 时 path 走 initial 分支（模拟分支断言，不真起 GPUI）

**质量门禁**: fmt + clippy + test 通过（aish-ui 211 → 216）。

---

### T2: aish-ui — Radio 组件

**对应 ADR**: D-3

- 新建 `components/radio.rs`：
  ```rust
  #[derive(IntoElement)]
  pub struct Radio {
      id: ElementId,
      label: SharedString,
      checked: bool,
      disabled: bool,
      on_click: Option<Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>>,
  }
  ```
  - `Radio::new(id)` 默认 unchecked / enabled
  - `.label(s)` / `.checked(b)` / `.disabled(b)` / `.on_click(h)` builder
- `RenderOnce::render`:
  - 横向 flex 行：14px 圆 + 1px border + 8px gap + label
  - checked = true: 圆内 6px primary dot（center 居中）
  - checked = false: 圆内空
  - disabled: opacity 0.5
  - hover (`.hover()` modifier): border 切 accent / dot 隐微调
  - on_click 走 `.on_mouse_down(MouseButton::Left, ...)`，调用 handler
- 单测（aish-ui）：
  - `radio_default_unchecked`：new(id) 后 checked = false
  - `radio_with_label_text`：.label("foo") 后 self.label = "foo"
  - `radio_checked_builder`：.checked(true) 后字段 = true
  - `radio_on_click_handler_stored`：.on_click(...) 后字段 Some

**质量门禁**: aish-ui 216 → 220（+4）。

---

### T3: host_form.rs — auth Radio + 删 Tabs Entity

**对应 ADR**: D-3 / D-10

- 删字段 `auth_tabs: Entity<Tabs>`
- 加字段 `auth_kind: AuthKind`（默认 AuthKind::KeyFile）
- `new` 内删 auth_tabs 构造
- `fill_inputs_from_modal`：把"set_active(active)" 改成 "self.auth_kind = draft.auth_kind"
- `collect_draft`：把"if auth_tabs.read.active()==0" 改成"match self.auth_kind"
- render body 内 `let active = self.auth_tabs.read(cx).active()` 改成 `let active = self.auth_kind`
- render 内原"Tabs row"位置改一个 `radio_row(self.auth_kind, cx)` helper：
  ```
  [○ Key File]    [○ Password]
  ```
  两个 Radio 横排 + 8px gap，on_click 写 self.auth_kind + cx.notify

- 暂保持其他视觉不变（layout 仍 label-on-left）— T3 仅做"换 Radio + 删 Tabs"，layout 在 T4。

**质量门禁**: fmt + clippy + test；手测 add/edit 切 auth 后 collect_draft 正确取 auth_kind。

---

### T4: host_form.rs — label-on-top layout + 字段 gap 16

**对应 ADR**: D-1 / D-2 / D-8 / D-10

- 删 `field_label` helper（不再共用 80px 左 label）
- 重写 `field_row`：
  ```rust
  fn field_row(
      cx: &App,
      label: &'static str,
      input: Entity<TextInput>,
      error: Option<&'static str>,
  ) -> impl IntoElement {
      let t = theme(cx);
      div()
          .flex().flex_col().gap(gpui::px(6.0))  // label-input gap
          .child(
              div()
                  .typography(TypeRole::Label, t)  // fg + 500，不 secondary override
                  .child(label)
          )
          .child(
              // input 自身。M29 加 error 联动：input.update(cx, |i, _| i.error(error.is_some()))
              // 因 input 是 Entity，外部 wrap 时需更新内部状态（在 render 内 dispatch）
              input  // caller 在外面写 input.update(cx, |i, _| i.error(error.is_some()))
          )
          .when_some(error, |d, msg| {
              d.child(
                  div()
                      .typography(TypeRole::Body, t)  // 13/400
                      .text_color(t.colors.destructive)
                      .child(msg)
              )
          })
  }
  ```
- render 内 form body flex_col `gap` 从 px_3 改 `gpui::px(16.0)` 显式 16（与 spacing.px_4 等价）
- render 之前对 host_input / port_input 通过 `i.error(...)` 联动 inline error 状态（用 self.host_error / self.port_error）：
  ```rust
  let host_err = self.host_error.is_some();
  self.host_input.update(cx, |i, _| { i.error(host_err); });
  // 同 port_input
  ```
- dialog body 外加自定义 px(20) py(20) wrapper（D-8 局部 padding 20）：
  - 由于 Dialog body 是 `dialog.update(cx, |d, _| d.body(...))`，HostFormModal
    自己组装 body 时最外层 div 加 `.p(px(20.0))` 替代默认 Dialog 内 `p(t.spacing.px_4)` —
    会和 Dialog 内 px_4 叠加 → **改 dialog body wrapper 时不再用 dialog 默认 padding**：M29 简单做法 = dialog body 不加额外 padding，让 Dialog 内 px_4=16 仍生效；如果视觉测下来 16 仍偏紧再补 + px(4)，避免动 dialog.rs
  - **决定**：T4 仅改 form gap 16，**dialog padding 暂保留 16**；T6 手测后看是否需要补 px(4) wrapper
- dialog width 460 → 480：`d.width(gpui::px(480.0))`

**质量门禁**: 手测 add 模式：label 在 input 上方 / 字段间 16 / dialog 480 宽 / inline error red border + 13px 红字。

---

### T5: host_form.rs — delete_dialog 拆独立

**对应 ADR**: D-4 / D-5（delete confirm 部分）/ D-9 修订 / R5

- 加字段 `delete_dialog: Entity<Dialog>`
- `new` 内构造：
  ```rust
  let delete_dialog = cx.new(Dialog::new);
  let weak = cx.weak_entity();
  delete_dialog.update(cx, move |d, _cx| {
      d.title("删除 Host?");
      d.width(gpui::px(380.0));
      d.on_close(move |_w, cx| {
          if let Some(this) = weak.upgrade() {
              this.update(cx, |this, cx| this.cancel(cx));
          }
      });
  });
  ```
- `sync_from_state` 内增加分支：
  ```rust
  // 决定 open 哪个 dialog，close 另一个（R5 防双 open）
  match next {
      SyncedKey::Adding | SyncedKey::Editing(_) => {
          self.delete_dialog.update(cx, |d, cx| d.close(cx));
          // ... 原 dialog open 逻辑
      }
      SyncedKey::DeleteConfirm(_) => {
          self.dialog.update(cx, |d, cx| d.close(cx));
          // open delete_dialog
          self.delete_dialog.update(cx, |d, cx| {
              d.body(self.build_delete_body(cx));
              d.open(cx);
          });
      }
      _ => {}
  }
  ```
- render 内 match SyncedKey 返回：
  - Adding/Editing → self.dialog.clone().into_any_element()（add/edit body 走老逻辑改 label-on-top）
  - DeleteConfirm → self.delete_dialog.clone().into_any_element()
- delete_dialog body 内容：
  ```
  ┌───────────────────────────────┐
  │ 删除 Host?                  X │
  ├───────────────────────────────┤
  │ 将永久删除 "{label}"，          │
  │ 此操作不可撤销。               │
  │                               │
  │ Enter 确认 · Esc 取消          │
  ├───────────────────────────────┤
  │              [Cancel] [删除]  │
  └───────────────────────────────┘
  ```
  - body 文字用 `TypeRole::Body` + foreground
  - keyboard hint 用 `TypeRole::Caption` + muted（保持现有）
  - footer 右对齐：Cancel (Ghost) + 删除 (Destructive)
  - **initial_focus** 给 Cancel button 的 FocusHandle（D-9 修订 — Enter
    不立即删）
  - Cancel button on_click → self.cancel(cx)
  - 删除 button on_click → self.save(cx)（save 内 DeleteConfirm 分支已存在
    第 320 行）

**质量门禁**: 手测 edit → Delete → delete_dialog 380 宽 / focus 在 Cancel
/ Enter 触发 Cancel（不删除）/ 鼠标点 删除 才真删 / Esc 关闭。

---

### T6: host_form.rs — footer 重构（add/edit 两端对齐 + border-top）

**对应 ADR**: D-5 / D-7 / D-8（footer 部分）

- 重写 `buttons_row` 为 add/edit footer：
  ```rust
  fn buttons_row(...) -> impl IntoElement {
      div()
          .flex().flex_row()
          .justify_between()  // 两端对齐
          .items_center()
          .border_t_1()
          .border_color(t.colors.border)
          .pt(t.spacing.px_3)  // footer 内顶 padding 12
          .mt(t.spacing.px_3)  // 与上方 body 间 12 间距
          .child(
              // 左侧：edit 时 Delete，否则空 div 占位（保持 justify_between 布局）
              if show_delete && !is_delete {
                  Delete button (destructive)
              } else {
                  div().into_any_element()  // 空占位
              }
          )
          .child(
              // 右侧：Cancel + Save 横排
              div().flex().flex_row().gap(spacing.px_2)
                  .child(Cancel button (ghost))
                  .child(Save button (primary, disabled when save_disabled))
          )
  }
  ```
- Cancel button on_click → self.cancel(cx)
- Save button 不变（已存在）
- Delete button on_click 把 modal 切到 DeleteConfirm（已存在）— 触发 T5
  的 delete_dialog

- 删除 footer 之前的 inline error display（"label 不能为空"等 into_config
  报错，现在仍在 modal.error 字段。位置不变 — 仍在 footer 上方 body 末尾，
  字号改 Body 13 + destructive）：
  ```rust
  .when_some(err, |d, e| {
      d.child(
          div()
              .typography(TypeRole::Body, t)  // 13/400
              .text_color(t.colors.destructive)
              .mt(t.spacing.px_2)
              .child(e)
      )
  })
  ```

**质量门禁**: 手测 add 模式 footer：左空 / 右 [Cancel] [Save]；edit 模式：
左 [Delete] / 右 [Cancel] [Save]；点 Save 全空，红字提示在 footer 上方
body 末尾。

---

### T7: host_form.rs — dialog initial_focus 给 label_input

**对应 ADR**: D-9

- `sync_from_state` 内 dialog.open(cx) 之前 set `initial_focus` 给
  `label_input.read(cx).focus_handle(cx)`：
  ```rust
  let label_fh = self.label_input.read(cx).focus_handle(cx);
  self.dialog.update(cx, |d, cx| {
      d.focus_chain(chain);
      d.initial_focus(label_fh);
      d.open(cx);
  });
  ```
- delete_dialog open 时给 Cancel button focus handle —— 这里复杂：Cancel
  button 是 RenderOnce（无 Entity 持 FocusHandle）。解决：给 HostFormModal
  加 `delete_cancel_focus: FocusHandle` 字段（cx.focus_handle() in new），
  Cancel button 用 Button::focus_handle(self.delete_cancel_focus.clone())
  注入。delete_dialog 的 initial_focus 给这个 handle。

**质量门禁**: 手测 add 时立即 focus 在 label input cursor 闪；delete confirm
立即 focus 在 Cancel button（虚线 ring 围着）。

---

### T8: 文档 + INDEX

- 更新 `specs/2026-05-15-aish-m29-host-form-redesign-design.md` 末尾"实施
  记录"：贴 commits 表 + Risk 实际遇到 + 测试增量
- `docs/superpowers/INDEX.md`：
  - 顶部"当前状态"改 — 当前活跃分支 / 下一里程碑候选
  - "Milestones（按时间倒序）"加 M29 entry
- 截图：spec 末尾贴 before/after

---

## Self-Review Checklist

- [ ] D-1 ~ D-10 决策每条都对应 task
- [ ] Risk R1-R10 在 task 内有 mitigation 落地
- [ ] T1 加 5 个 aish-ui 单测（error border / initial_focus 路径）
- [ ] T2 加 4 个 Radio 单测
- [ ] T3 不动 HostFormDraft / SyncedKey 数据流（R1 / D-10）
- [ ] T5 sync_from_state 切换路径 close-the-other（R5）
- [ ] T7 delete_dialog initial_focus = Cancel（D-9 修订 / R10）
- [ ] T4 dialog padding 16 暂保留，T6 手测后视觉评估是否补 wrapper（D-8 决定）
- [ ] commits 严格按 task 顺序
- [ ] 每条 commit 跑 fmt + clippy + test
- [ ] commit message 末尾加 Co-Authored-By Claude Opus

---

## 实施顺序与依赖

```
T1 (TextInput.error + Dialog.initial_focus, aish-ui 底层) ─┐
                                                            ↓
T2 (Radio 组件, aish-ui)                                    │
                                                            ↓
T3 (host_form auth_kind + Radio 接入) ──── T4 (label-on-top layout)
                                                            ↓
T5 (delete_dialog 拆出) ──── T6 (footer 两端对齐) ──── T7 (initial_focus)
                                                            ↓
                                                          T8 (文档)
```

T1 + T2 可并行（不同文件），但都需先于 T3-T7。
T3-T7 依赖关系：T3 是数据层切换前置；T4-T6 视觉层；T7 焦点收尾。
T8 收尾文档 / INDEX。

---

## 工程量估算

| Task | 文件 | 行数估 | 时间估 |
|---|---|---|---|
| T1 | text_input.rs / dialog.rs | +30 / +20 + 5 测试 | 1h |
| T2 | radio.rs (新) | ~120 + 4 测试 | 1.5h |
| T3 | host_form.rs | -30 / +20 | 0.5h |
| T4 | host_form.rs | 重写 field_row + body gap | 1h |
| T5 | host_form.rs | +50（delete_dialog 构造 + sync 分支 + body） | 1.5h |
| T6 | host_form.rs | 重写 buttons_row | 1h |
| T7 | host_form.rs | +10 line（initial_focus 接线） | 0.5h |
| T8 | docs/INDEX | +50 line | 0.5h |
| **合计** | | | **~7.5h** |

约 **1 工作日**。质量门禁每 task 必跑。

---

## 已知 backlog（M29 后留下次）

- TextInput `.error()` 视觉 ring 与 focus ring 同色（destructive alpha 0.4）
  与现有 ring（M24 ring alpha 0.4）层次区分 — 实操中可能需要不同 alpha
- Radio Group 组合管理（M29 caller 手管 selected）；未来若 form 有 ≥3 个
  RadioGroup callsite 抽 RadioGroup Entity
- Dialog body padding token 化（M29 局部 px(20) 决定；后续若多 dialog 共
  用 20 再加 spacing.px_5）
- HostForm "Test Connection" 按钮（试连按钮，在 footer 中间）— 独立 M30+
- HostForm Advanced 折叠区（jump_host / known_hosts / 自定义 env）— 独立
  M30+
