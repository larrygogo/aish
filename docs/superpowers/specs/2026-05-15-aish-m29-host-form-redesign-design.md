# M29 — HostForm Dialog 视觉重设计

**日期**: 2026-05-15
**父 spec**:
- [`2026-05-15-aish-m24-visual-redesign-design.md`](2026-05-15-aish-m24-visual-redesign-design.md)（视觉基线）
- [`2026-05-15-aish-m26-typography-hierarchy-design.md`](2026-05-15-aish-m26-typography-hierarchy-design.md)（type role）
- [`2026-05-09-aish-m12-forms-nav-design.md`](2026-05-09-aish-m12-forms-nav-design.md)（HostForm v1 落地）

**目标**: 把 HostForm Modal 从 M12 时期的"label 左 80px 栅格 + Tabs 切 auth +
delete 走同 dialog"风格升级到 Linear/Stripe 商业级 form modal 风格 — label 在
input 上方 / Radio 选 auth / Delete 拆独立 destructive dialog / 按钮 footer
两端对齐 + 分隔线 / 视觉间距与字号统一走 M26 token。

**预计工程量**: 1 天，T1 layout 重构 + T2 Radio + T3 Delete 独立 + T4 footer
按钮规范 + T5 inline error 视觉 + T6 dialog 容器细节。

---

## 1. 动机 — 当前 HostForm 视觉与交互问题

直接读完 `host_form.rs` 644 行后审计出的具体问题。每条引用代码位置，避免 hand-wavy。

### 1.1 Label 左 80px 固定宽度（label-on-side 栅格）

- `field_label` (host_form.rs:523) — `.w(gpui::px(80.0))` + `TypeRole::Label`
  + `.text_color(secondary_foreground)` override
- 6 个 label 长度："label / host / port / user / key path / password" —
  最长 8 char 最短 4 char，80px 宽全部撑得下但左侧大片留白
- 中文化（如果未来 i18n 改"主机名 / 端口 / 用户"等 4-6 char 中文）80px 又
  会撑爆
- Linear "Connect to Service" / Stripe "Add API Key" / Vercel "New Project"
  等 form modal 一致用 **label 在 input 上方** 风格，省掉左 80px 浪费
  + 自动适应任意 label 长度

### 1.2 Label 颜色被 secondary_foreground override 弱化

- `field_label` (host_form.rs:528-529)：M26 把 label 应用 `TypeRole::Label`
  (13/500/fg) 后又 `.text_color(t.colors.secondary_foreground)` 拉回弱化
- 实测视觉接近 placeholder，label 与 placeholder 没区分度，用户分不清"标
  签"和"提示"
- M26 spec D-4 说 caller 可 override default color_role，**但 form label
  本身就是 fg 强调用途**，弱化与设计意图相反
- Linear / Stripe label 一律用 fg + 500 weight 而非 muted（弱化是给"上次
  连接 X"这种 meta 信息，不是给 label）

### 1.3 Tabs 切 KeyFile / Password 视觉过重

- `auth_tabs` (host_form.rs:114) Tabs 行整行 ~40px 高，含 underline / 内边距
- 仅 2 个选项，Tabs 的"承载多面板切换"语义大材小用
- Linear "Authentication" / 1Password "New item" 等 2-3 选项一律走 **Radio
  Button 横排** 或 **Segmented Control**，紧凑度高 50%
- 此外 Tabs Entity（auth_tabs）占字段 + observe + set_active 逻辑可省

### 1.4 Inline error 字号 12 太小

- `field_row` 内 error 行 (host_form.rs:567) — `TypeRole::Caption` (12/400)
  + `destructive` color override
- 12px destructive 红字 + 与 input 同 column 但加 80px 缩进，可读性偏弱
- Stripe / Linear 一律用 ≥13px **body size** + destructive color 做 inline
  error（比 input 字号小但不缩到 caption 12）

### 1.5 Delete 确认走同一 Dialog 内 body 分支

- `body` (host_form.rs:447-469) — DeleteConfirm 分支与 add/edit 共用 460px
  dialog
- delete 是不可逆 destructive 操作，应该独立 confirm dialog：
  - 更窄（~360-400px）— 集中注意力
  - title 改"删除 Host?" 强语义
  - primary 按钮 destructive 红
  - body 仅"将永久删除：{label}" 一行 + meta 提示 "Enter 确认 · Esc 取消"
- 当前实现 dialog title "确认删除" 是 16/600，但 body 字号、按钮都与 add/edit
  共用，destructive 信号弱

### 1.6 Edit 模式 Delete + Save 同 row 右对齐

- `buttons_row` (host_form.rs:614-642) — show_delete 时 row 末尾追加
  Delete button（destructive），再追 Save。两按钮都右对齐
- Material / Linear / GitHub 一律用 **destructive 左 / primary 右** footer
  两端对齐（justify_between）：
  - 用户视线从左向右扫，destructive 在左侧形成"危险区"暗示
  - primary action 在右侧符合"主流确认按钮"惯例
- 当前 row 右对齐 + Delete 紧邻 Save 风险点击错（destructive 应远离 primary）

### 1.7 Dialog footer 没有 border-top 分隔线

- `buttons_row` (host_form.rs:613) — 直接 `flex.justify_end`，与上方 form
  body 同 padding，无 border-top 分隔
- Dialog 视觉 anatomy 应是 **header / body / footer 三段**，footer 与 body
  之间需 1px border + secondary 背景或者至少视觉留白拉开层级
- 现在 footer 紧贴最后一个 input 看上去像"还有一个字段"而不是"操作区"

### 1.8 字段 group gap 用 px_3 偏小（label-on-top 风格下）

- `body` flex_col gap (host_form.rs:479) — `gap(spacing.px_3)` = 12px
- label-on-side 时 12px gap OK；改 label-on-top 后**单字段内 label-input gap
  = 6**，但**字段间 gap 需要 12-16** 才能避免和 label-input 间距混淆
- 现在改造前 12px 单 gap 用于所有 row，改造后需要 nested gap (字段内 + 字段间)

### 1.9 Dialog 宽度 460 + body padding 16 在 label-on-top 下偏紧

- `dialog.width(gpui::px(460.0))` (host_form.rs:105) — 460 是 label-left 时
  的合理宽
- label-on-top 后 input 宽度独占整行，460 - 16*2 padding = 428 input 宽
  仍 OK 但偏窄；Linear "Add Source" 等通常 480-520 宽
- Dialog 内部 body padding `t.spacing.px_4` = 16 (dialog.rs:241)，Stripe /
  Linear 多用 20-24

### 1.10 Dialog 内 cx mut borrow chain 不能稳定持 &Theme（M26 遗留注释）

- (host_form.rs:425-433) — `let (colors, font_size, spacing) = { ... }` 提
  前拷贝避免 borrow 冲突
- M26 注释说"无法稳定持 &Theme，因此本 view 内文字仍 inline `.text_size +
  .text_color`，等价 typography role (Body/Caption/Label) 但不走
  `.typography()` ext"
- M29 改造可机会一并把 inline 字号 / 颜色全切到 `.typography(...)` ext —
  helper fn 改成 `fn(cx)` 内 `theme(cx)` 取一次 t 用完即丢，不持久

### 1.11 其他次要问题

- "Enter 确认 · Esc 取消" hint (host_form.rs:464) 字面"·"分隔符在 12px
  下毛刺，可改成"或 Esc 取消"或者两个 inline keyboard chip
- "key path" label 含空格，其他都单 word（不一致）
- placeholder "22" / "root" 是 default 而非示例 — 用户可能误认为"已经填了"
  → 改用 example 风格 "如 example.com 22 / root"（实际 default 22 / root
  由 fill_inputs_from_modal 第 254 行预填，可保留）

---

## 2. 决策记录（ADR-style）

### D-1: Label 位置 — input **上方** + 6px gap

**采**: label 在 input **上方**，间距 6px（spacing.px_1=4 偏紧 / px_2=8
偏松，6 是 M29 专属"label-input gap"硬编码）。label 走 `TypeRole::Label`
(13/500/fg)，**不**再 secondary_foreground override。

理由：
- 适应任意 label 长度（中英文 / 长 short）
- 与 Linear / Stripe / Vercel form modal 视觉一致
- fg 强调让 label 与 placeholder 区分清晰
- 6px gap 让"label 是 input 的 caption"语义紧贴，不混入字段间 gap

**拒**:
- label 左 80px 栅格（现状）— 浪费空间 + 不适应 i18n
- 4px gap — label 几乎贴 input border，视觉拥挤
- 8px gap — 与字段间 12px gap 区分度不够

### D-2: Field group gap = 16，form padding 内 = 20

**采**:
- 字段间 gap：`spacing.px_4` = 16（旧 px_3 = 12 偏紧）
- Dialog body padding：`spacing.px_4` × 1.25 ≈ 20（M29 在 dialog.rs `p()`
  传入手算的 px(20.0)，**不**改 spacing token；px_4 = 16 仍是默认
  spacing scale，dialog 内的 20 是 form-specific override）

理由：
- 16 字段间 gap > 6 label-input gap > 1px border —— 三层间距清晰区分
  "字段内 / 字段间 / 视觉分隔"
- Dialog padding 20 让宽度 480 form 内有 440 input 宽，长 host 字符串
  （如 `user@host.example.com:22`）不挤

**拒**:
- 字段 gap 用 px_3 = 12（现状）— label-on-top 风格下与 label-input gap 6
  比例 2:1 太小，视觉粘连
- 全部 dialog padding 改 spacing.px_5 = 20（新加 token）— 仅 host_form 一
  处用，不值得动 token；inline px(20.0) 即可

### D-3: Auth 方式选择 — Radio Button 横排

**采**: 删除 `auth_tabs` Tabs Entity，改 **Radio Button 横排**（两个圆形
radio + label，dot 选中态 primary 色）。Radio 行 + 选中状态对应的字段（key
path 或 password）紧跟其下。

新增 `aish_ui::Radio` 组件（M29 内独立 commit）：
- `Radio::new(id).label("...").checked(bool).on_click(...)`
- 视觉：14px 圆 + 1px border + 选中态内 6px primary dot
- 一组 radio caller 自己用 HashMap 管 selected_id，组件不持组合状态

理由：
- 2 选项 Radio 比 Tabs 紧凑（仅 ~24px 高 vs Tabs ~40px）
- Radio 表达"互斥单选"语义比 Tabs 更准确（Tabs 暗示"切换不同面板内容
  视图"，Radio 才是"在选项内单选"）
- form 内常见模式，Linear "Add API Key" 选 scope / Stripe "Payment method"
  选 card/bank 都走 Radio 行

**拒**:
- Segmented Control（连接 2 button 视觉合体）— 实现复杂（border 共享 +
  active 切换 + animation），M29 出 Radio 即可
- 保持 Tabs — 见 1.3
- Select 下拉 — 2 选项不值得下拉

### D-4: Delete 确认拆独立 destructive Dialog

**采**:
- HostFormModal 仍 observe `state.modal`，但 DeleteConfirm 分支在 render
  内**用第二个 Dialog Entity** 渲染（HostFormModal 持 `dialog: Dialog` +
  `delete_dialog: Dialog`，按 modal 状态 open 哪个）
- delete_dialog 配置：
  - title: "删除 Host?"
  - width: 380px（窄）
  - body: "将永久删除 \"{label}\"，此操作不可撤销。" + 12px gap + 2 行
    keyboard hint "Enter 确认 · Esc 取消"
  - primary: destructive 红 "删除"
  - footer 仍带"取消"按钮（destructive 场景"二次确认"惯例）
  - body padding 仍 20

理由：
- destructive 操作需要专属容器拉开注意力
- 窄 dialog 集中视线
- 文案明确"不可撤销"提示
- 与 add/edit 在 modal type 维度本来就分离（HostFormState::DeleteConfirm
  独立 variant），UI 层也应该独立

**拒**:
- DeleteConfirm 走同一 dialog（现状）— 视觉信号弱
- 走系统 prompt（os 原生）— 跨平台样式不一致 + 阻塞主线程

### D-5: 按钮顺序 — Cancel 左 / Primary 右（add/edit）；Delete 左 / Cancel 中 / Primary 右（不存在该形态）

**采** add/edit footer:
- 横向 `justify_between`
- **左**: Cancel（Ghost variant，弱化）
- **右**: Save（Primary）
- 间距：左右两端贴 footer 内 padding，中间自动撑

**采** delete confirm footer:
- 横向 `justify_end`
- **从左到右**: Cancel（Ghost）+ 删除（Destructive）
- destructive 在 primary 位（右），与 add/edit Save 位置同 —— 用户肌肉记
  忆"右下 = 主操作"一致

**采** add/edit Edit 模式追加 Delete 按钮:
- 删除按钮**移到 footer 最左端**（destructive 区，远离 Save）
- 左 destructive | 中弹簧 | 右 Cancel + Save

  ```
  [Delete]                       [Cancel] [Save]
  ```
- M12 的"右对齐三连按钮 Delete + Cancel + Save"改成两端对齐 + Delete 单
  独 left

理由：
- destructive 与 primary 分离，物理距离防误点
- footer 两端对齐是 Material / Linear / GitHub form modal 标配
- Cancel 显式回归（M12 删过 Cancel 让 X 唯一关闭路径；M29 改回 Cancel +
  X 双入口 — Cancel 是"焦点在 footer 时键盘可达 + Tab 序末尾自然回归"，
  X 只在视觉容器右上）

**拒**:
- 全右对齐三连（现状）— Delete 紧邻 Save 误点风险
- destructive 用 ghost + 红字（非填色）— 视觉权重不够，destructive 就该
  填色

### D-6: Inline 实时校验视觉规范

**采**:
- 错误文字：`TypeRole::Body` (13/400) + `text_color(destructive)`（**不**
  用 Caption 12）
- 出现位置：input 下方 4px gap（与 label-input 6px gap 平行，错误文字略
  紧贴 input 暗示"针对该 input"）
- 出现时机：on_change 后立即（现状已是）
- 不出现：input 空时（validate_host/port 已 return None）
- input border 颜色：有 error 时切 `destructive`（M29 新加在 TextInput 上
  的 `.error(bool)` builder，否则保持现 border / focus ring）

理由：
- 13px 比 12 更易读
- input 左下 4px 紧贴形成视觉"针对性"
- input 边框变红是 Stripe / Linear / Material 一致做法，仅文字红信号不够

**拒**:
- 保持 12px caption（现状）
- 错误消息走 toast 浮层 — inline 校验本质是 inline
- input 边框走 destructive_hover 弱化 — 错误就该高对比

### D-7: TextInput `.error(bool)` 新 builder

**采**: aish-ui `TextInput` 加 `pub fn error(self, e: bool) -> Self` builder。
内部存 `error: bool` 字段。render 时 error=true 切 border_color =
destructive，focus ring color = destructive。

理由：M29 视觉需要 input 边框跟 inline error 状态联动；TextInput 已有
border / focus 逻辑（M11 起），加 `error` 字段 1-bit 状态最干净。

**拒**: caller 用 wrapper div + border 包裹 — TextInput 内部 border 与外
border 视觉冲突；改 TextInput 本体最干净。

### D-8: Dialog 视觉容器细节 — header / body / footer 三段 + 宽度 480 + body padding 20

**采**:
- HostFormModal 的 dialog width 460 → 480
- delete_dialog width = 380
- dialog body padding 从全局 `t.spacing.px_4` = 16 改局部 20
  - 实现：HostFormModal 自己 wrap dialog body 时外加一层 `.px(px(20.0))`
    `.py(px(20.0))`，**不动** dialog.rs（其他 dialog 仍 px_4，M29 out of
    scope）
- footer:
  - 上方加 1px border `t.colors.border`
  - footer 自身 px(20) py(12) 视觉与 body padding 对齐
  - footer 在 body 同 column 内（dialog body 不需要改，HostFormModal 自
    己渲染 body 时包 `flex_col` + `border_t` separator + footer row）

理由：
- 480 给 host 字段（example.com / 192.168.1.1:22 等）更舒展
- body padding 20 是 Linear / Stripe 标配，16 在 480 宽内视觉拥挤
- header/body/footer 分隔线让 dialog "三段"语义清晰
- 改动局限在 HostFormModal 自己，不动 Dialog 组件 — 避免 SessionPicker /
  Settings 等其他 dialog 受连锁影响（M29 仅 HostForm 范围）

**拒**:
- 把 padding 20 推到 dialog.rs `p()` — 影响所有 dialog，超 M29 scope
- 全局 spacing 加 px_5 = 20 token — 仅一处用不值得
- footer 不加 border — 视觉拥挤如 1.7 所述

### D-9: Focus 流 — Dialog open 后第一个 input 自动 focus

**采**:
- 现状 (host_form.rs:226-236) focus_chain 已注册 6 个 input，但 Dialog
  open 后 needs_focus 把 focus 给的是 dialog focus_handle 自己（dialog.rs:165
  `self.focus_handle.focus(window, cx)`），用户要再按一次 Tab 才进 label_input
- M29 改：HostFormModal 在 dialog.open(cx) **之后**用 cx.spawn / cx.defer
  把 focus 显式给 label_input（add/edit 第一个字段）；delete_dialog open 后
  focus 给 destructive button（确保 Enter 直接确认）

实现层面：M29 给 Dialog 加 `initial_focus(handle: FocusHandle)` builder
（与现有 focus_chain 不同），open 后用 initial_focus 替代 self.focus_handle
作 needs_focus 目标。focus_chain 仅作 Tab 循环用，与 initial focus 解耦。

理由：
- M12 实现里 Dialog open 后 focus 在 dialog 容器自己，与所有现代 form
  modal 体感不一致（Linear / Stripe open 后 cursor 立即在第一个 input）
- 解耦 initial_focus vs focus_chain 让 caller 显式控制开屏行为

**拒**:
- 默认让 focus_chain 第一个元素自动 focus — focus_chain 是 Tab 循环用，
  语义不应包含 open 时机；显式 initial_focus 更清晰
- HostFormModal 自己 cx.defer 调 input.focus_handle.focus — 也能工作但
  把 Dialog 内部状态机的 "open + focus" 时序拆到 caller，不优雅

### D-10: 保持 SyncedKey 与 HostFormDraft 数据流不动

**采**: M29 仅改 UI 层 — `HostFormState` / `HostFormDraft` / `SyncedKey`
/ `into_config` / persistence 调用全部不动。collect_draft 不动。修复后
的 form 仍走 6 个 TextInput + auth_kind = AuthKind::KeyFile/Password 的
逻辑流，仅把"读 auth_kind"从 `auth_tabs.read(cx).active()` 改成"读 self
内部新加的 `auth_kind: AuthKind` 字段（由 Radio on_click 写入）"。

理由：M29 是视觉重塑，数据 / 持久化 / keyring 全部不动 — 风险最小，回
归只在 UI 层。

---

## 3. 架构变化总览

```
┌──────────────────────────────────────────────────────────────┐
│ aish-ui                                                       │
│   components/radio.rs (新增)                                   │
│     pub struct Radio { id, label, checked, on_click }         │
│   components/text_input.rs                                     │
│     +pub fn error(self, e: bool) -> Self                       │
│     render: error=true 切 border destructive                   │
│   components/dialog.rs                                         │
│     +pub fn initial_focus(handle: FocusHandle) -> &mut Self    │
│     render: open + needs_focus 时 focus initial_focus 而非 self │
└──────────────────────────────────────────────────────────────┘
                              ↓
┌──────────────────────────────────────────────────────────────┐
│ aish-app/views/host_form.rs                                    │
│   字段:                                                         │
│     -auth_tabs: Entity<Tabs>                                   │
│     +auth_kind: AuthKind                                       │
│     +delete_dialog: Entity<Dialog>                             │
│   render:                                                       │
│     match SyncedKey                                            │
│       Adding/Editing → dialog (480w)                           │
│         header (Dialog 内置)                                    │
│         body (px(20))                                          │
│           label-on-top × 6 字段 + Radio auth                    │
│           inline error (Body destructive + input.error)        │
│         border-top                                             │
│         footer (px(20), justify_between)                       │
│           [Delete?] | [Cancel] [Save]                          │
│       DeleteConfirm → delete_dialog (380w)                     │
│         body: "将永久删除..." + keyboard hint                   │
│         footer: [Cancel] [删除 (destructive)]                  │
│   helper:                                                       │
│     -field_label (删，label-on-top 不再共用)                    │
│     field_row 重写 → label-on-top 风                            │
│     buttons_row 重写 → footer 两端对齐 + border-top              │
│     +radio_row (auth 选择)                                      │
│     +delete_dialog_body                                         │
└──────────────────────────────────────────────────────────────┘
```

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | 数据流污染 — SyncedKey / fill_inputs_from_modal / collect_draft 改坏 | 高 | D-10 显式规定不动；改造仅替换 `auth_tabs.read.active()` 为 `self.auth_kind`，fill_inputs 内 set_active 改写 self.auth_kind |
| R2 | Radio 组件首次出现，无现成 callsite 参考 | 中 | T2 加 Radio 时附 3+ 单测（默认未选 / 单选切换 / disabled 视觉占位）；M29 仅 host_form 用 Radio，组件 API 简单不易出错 |
| R3 | TextInput.error() 改 border 与现 focus ring 视觉冲突 | 中 | T1 实现先看 TextInput::render border_color 与 focus ring 互动逻辑（M11/M15），error=true 时**优先级最高**覆盖 border + ring；加单测确认 error + focus 同时为 true 时 destructive 边框稳定 |
| R4 | Dialog.initial_focus 与 needs_focus 时序冲突 | 中 | T1 改 Dialog 时 needs_focus 分支：有 initial_focus 用 initial focus，没有 fallback 旧 self.focus_handle.focus（向后兼容 SessionPicker / Settings Dialog）；加单测断言 initial_focus 设置后 needs_focus 路径调用 initial 而非 self |
| R5 | delete_dialog 是第二个 Dialog Entity，可能两 Dialog 同时 open | 高 | render 内严格 match SyncedKey 单分支：Adding/Editing → 只 open dialog（close delete_dialog）；DeleteConfirm → 只 open delete_dialog（close dialog）。sync_from_state 内 SyncedKey 切换路径加 close-the-other 行 |
| R6 | label-on-top 后 dialog 高度增加触发 dialog max_h(640) 裁切 | 低 | 6 字段 label-on-top（每字段 ~50px：label 18 + gap 6 + input 28）+ Radio 24 + Auth 字段 50 + 错误 ~20 + footer 56 ≈ 350-400 + dialog header 48 = 总 ~450，远小于 max_h 640 |
| R7 | 现有 host_form 单测 / aish-app 集成测试基于旧 UI 结构 | 低 | host_form 现无 UI 层单测（M12 至今没补），仅 state.rs HostFormDraft::into_config 有逻辑测试；M29 不动 into_config，测试照旧通过 |
| R8 | Radio 字段（key_path / password）切换时清空对方输入 vs 保留 | 中 | 沿用现状（Tabs 切换不清 keyfile_input / password_input 内容；用户切回原 auth 仍能看到旧值）。Radio 切换同行为，collect_draft 仅取当前 auth_kind 对应字段 |
| R9 | Linear/Stripe 风把 Cancel 放 footer 左 - 但 M12 删过 Cancel 是用户偏好 | 中 | M12 删 Cancel 理由是"X 已经在 header 右上"，M29 改 footer 两端对齐后**键盘可达性**要求 Cancel 在 Tab 序末尾 — 与 X 是冗余但有键盘流必要性；如用户反馈仍可去 — 留 toggle |
| R10 | initial_focus 在 delete_dialog 给 destructive button — 用户按 Enter 即删可能太激进 | 中 | Enter 立即删是 destructive confirm dialog 通用惯例（git 命令 `--yes-i-really-mean-it`、systemd `--no-ask-password` 等），但 M29 改成 initial_focus 给 **Cancel** 按钮（更保守，destructive 操作需鼠标或 Tab+Enter 二次确认）。Spec D-9 落地这条 — 见 D-9 修订 |

> **D-9 修订**: delete_dialog initial_focus 给 **Cancel** 而非 destructive
> button（destructive 二次确认惯例 + R10）。Enter 不会立即触发删除。

---

## 5. Out of scope（M29 不做）

- 不新增 HostFormDraft 字段（如 jump_host / proxy / 自定义 known_hosts）
- 不动 keyring 逻辑（save_secret_for / delete_secret_for）
- 不动 persistence（hosts.json 读写）
- 不动 HostFormState 枚举形态
- 不做 form 字段联动校验（如 host + port 组合合法性）
- 不做 SSH 试连按钮（"Test Connection"）
- 不做高级展开（"Advanced settings"折叠区）
- 不一次性把 Dialog body padding 20 推到 dialog.rs（D-8 决定局部）
- 不动 SessionPicker / Settings 内其他 Dialog 视觉

---

## 6. 测试策略

### 单测（aish-ui）

T1 / T2 加：
- `Radio::default_unchecked`：默认 checked = false
- `Radio::toggle_on_click`：on_click 调用后 checked 状态机翻转（模拟）
- `Radio::render_check_dot_when_checked`：checked = true 时 child 含 dot
  div（结构断言）
- `TextInput::error_border`：error = true 时 render 内 border_color =
  destructive（结构断言）
- `TextInput::error_overrides_focus`：error + focused 同时 true，border
  优先 destructive（不被 focus ring 覆盖）
- `Dialog::initial_focus_when_open`：set initial_focus + open → needs_focus
  路径取 initial 而非 self focus_handle

### 单测（aish-app）

- 现 host_form 无 UI 层单测，M29 也不补（render 路径多 cx.listener 难
  unit test，照 M12 风格走集成 / 手测）

### 集成（手测 checklist）

T6 收尾时跑：
- [ ] 点 Home 中 "+" 添加 host：dialog 480 宽 / focus 在 label input /
  label 在 input 上方 fg 加粗
- [ ] 输入 "ssh://host" → host 字段下方 13px 红字 + input border 红
- [ ] 输入 "abc" → port 字段下方红字 + border 红
- [ ] 清空 host → 红字消失 + border 复原
- [ ] 点 Radio "Password" → key path 字段 hide，password 字段 show
- [ ] 切回 "Key File" → password 字段 hide，key path 字段 show（保留旧值）
- [ ] 点 Save 全空：dialog 内底部 destructive 红字 "label 不能为空"
- [ ] 编辑 host 切到 edit modal：footer 左 [Delete] 右 [Cancel] [Save]
- [ ] 点 [Delete]：切到 delete_dialog 380 宽 / title "删除 Host?" / focus
  在 Cancel / Enter 不立即删
- [ ] delete dialog 内按 Tab：focus 走到 destructive 按钮 + Enter 删除
- [ ] add 时按 Tab：focus 在 6 input + Radio + 2 button 间循环
- [ ] dark / light 主题切换：所有色用 token 自适应
- [ ] Esc 任意 dialog 关闭

### 质量门禁

每个 task commit 跑：
```
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### 截图对比

M29 完成后，spec 末尾"实施记录"贴：
- before：M28 末态 host_form
- after：M29 末态 host_form (add) + delete confirm dialog
- 两张 form 视觉对比说明 hierarchy 改善

---

## 7. Plan 引用

见 [`../plans/2026-05-15-aish-m29-host-form-redesign.md`](../plans/2026-05-15-aish-m29-host-form-redesign.md)

---

## 8. 实施记录

（M29 实施后填）
