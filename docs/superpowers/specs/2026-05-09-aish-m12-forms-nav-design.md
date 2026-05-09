---
title: M12 — aish-ui 表单与导航 + HostForm/SessionPicker 迁移
date: 2026-05-09
status: approved
parent: 2026-05-09-aish-ui-architecture-design.md
---

# M12 — aish-ui 表单与导航 + HostForm/SessionPicker 迁移

## 0. 关系

本 spec 是 [`aish-ui 架构总览`](2026-05-09-aish-ui-architecture-design.md) 下的第二个里程碑。
M11（[`m11 spec`](2026-05-09-aish-m11-ui-starter-design.md)）已交付 Foundations + 7 个起步组件
（Button/IconButton/Badge/Separator/Tooltip/TextInput/Toast）+ aish-app 全 view
切到 `aish_ui::theme`。

M12 延续：交付 5 个表单与导航组件，**同步**用新组件重写 aish-app 的 HostFormModal +
SessionPickerView，并给 Settings 加 Light/Dark toggle 的 UI 演示位。

## 1. 范围

### 1.1 Scope（in）

5 个组件：

- **Dialog**（Entity）—— 居中 modal，含 focus trap + Esc/外侧点击关闭 + 自定义 body
- **Select**（Entity）—— 下拉选单，触发器（带选中文字）+ 弹层选项列表
- **Checkbox**（builder）—— 受控勾选框，含 label
- **Switch**（builder）—— 受控滑动开关，类似 iOS toggle
- **Tabs**（Entity）—— 标签栏 + 内容面板切换，受控 active 索引

aish-app 视图迁移：

- **HostFormModal** 重写：外壳走 Dialog，KeyFile/Password 切换走 Tabs，输入字段走 TextInput（M11 已有）
- **SessionPickerView** 迁移：外壳走 Dialog，列表保留手写（不强迁 Select）
- **SettingsView** 加 Section "Appearance"：内嵌 Switch 控件，演示 Dark/Light toggle UI

### 1.2 Out of scope

- Card / RadioGroup —— 父 spec 列入 M12 但实际未必用得上，按需在 M13 加（HostFormModal 走 Tabs 不走 RadioGroup）
- DropdownMenu / ContextMenu —— M13
- Light theme **实际实现** —— 仍 `unimplemented!()` stub。Settings Switch 仅 UI 演示，点 Light 触发 toast warning，不调 `cx.set_global`
- Select 弹层方向自适应（向上 / 向左）—— M12 简化只向下弹，超出视口暂不处理
- Dialog 多种变体（confirm / info / form-wrapper） —— M12 只做通用 form-wrapper Dialog，confirm/alert 等变体按需后续加
- HostFormModal 的 SSH 业务逻辑改动（save / delete / connect） —— 行为保持，只换外壳和字段控件

## 2. 文件结构

```
crates/aish-ui/src/components/
  ├ checkbox.rs       # Checkbox + builder
  ├ dialog.rs         # Dialog Entity + Render + focus trap
  ├ select.rs         # Select Entity + DropdownState
  ├ switch.rs         # Switch + builder
  └ tabs.rs           # Tabs Entity + active_index + on_change
  mod.rs              # 追加 5 个 mod / pub use

crates/aish-ui/src/lib.rs              # 自动 re-export via pub use components::*
crates/aish-ui/src/prelude.rs          # 追加 5 个名字（Checkbox/Switch/Dialog/Select/Tabs）

crates/aish-app/src/views/host_form.rs       # 重写
crates/aish-app/src/views/session_picker.rs  # 迁外壳 + 用 Dialog
crates/aish-app/src/views/settings.rs        # 加 Appearance section + Switch
```

## 3. 组件详细设计

### 3.1 Checkbox

```rust
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: SharedString,
    disabled: bool,
    on_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>) -> Self;
    pub fn checked(self, c: bool) -> Self;
    pub fn label(self, l: impl Into<SharedString>) -> Self;
    pub fn disabled(self, d: bool) -> Self;
    pub fn on_change(self, h: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self;
}
```

**视觉**：

- 外框 16×16 px，`radius.sm`，`border` 颜色
- 选中：bg = `primary`，内画 ✓ icon (IconName::Check, size 12)
- 未选中：bg 透明
- disabled：bg = `muted`，icon = `muted_foreground`，无 hover/click
- label 间距 8px，与外框水平对齐

**交互**：点击外框或 label 触发 `on_change(!checked)`。受控组件，调用方负责更新 `checked`。

### 3.2 Switch

```rust
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self;
    pub fn checked(self, c: bool) -> Self;
    pub fn disabled(self, d: bool) -> Self;
    pub fn on_change(self, h: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self;
}
```

**视觉**（参 iOS / shadcn）：

- 整体 36×20 px 胶囊（`radius.full`）
- on：bg = `primary`，圆点 16×16 在右
- off：bg = `muted`，圆点 16×16 在左
- 圆点 padding 2px、`radius.full`、bg = `foreground`
- disabled：opacity 50%

**交互**：点击触发 `on_change(!checked)`。受控。

### 3.3 Dialog

```rust
pub struct Dialog {
    focus_handle: FocusHandle,
    open: bool,
    title: SharedString,
    body: Option<gpui::AnyElement>,
    width: Option<Pixels>,           // 默认 480px
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Dialog {
    pub fn new(cx: &mut Context<Self>) -> Self;
    pub fn title(&mut self, t: impl Into<SharedString>) -> &mut Self;
    pub fn body(&mut self, body: impl IntoElement) -> &mut Self;
    pub fn width(&mut self, w: Pixels) -> &mut Self;
    pub fn on_close(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self;
    pub fn open(&mut self, cx: &mut Context<Self>);
    pub fn close(&mut self, cx: &mut Context<Self>);
    pub fn is_open(&self) -> bool;
}

impl Render for Dialog { ... }
impl Focusable for Dialog { ... }
```

**视觉**：

- 全屏 backdrop（`background`，opacity 60%），点击 backdrop 关闭
- 居中卡片：bg = `popover`，`radius.lg`，`border` border-1，`width` 默认 480px
- 内部纵向 layout：title (`font_size.lg` + `foreground`) + body slot + footer slot（调用方在 body 内自行排）
- 顶部右侧 IconButton(IconName::X) 关闭

**交互**：

- focus trap：用户聚焦在 dialog 内的输入控件 / 按钮，Tab 键循环但不出 dialog
- Esc 触发 `on_close`
- backdrop click 触发 `on_close`
- `open()` 调用时立即 `focus_handle.focus(window, cx)` 拿焦点

**实现要点**：

- 渲染时 `if !self.open { return Empty }`
- focus trap 通过 KeyDown 拦截 Tab 键 + 检查 focus 是否还在 Dialog 子树（如不在则强行 focus 第一个 focusable）
- M12 简化版 focus trap：仅处理 Esc 关闭 + backdrop click，不强制 Tab 循环（Tab 循环留 M13 加固）

### 3.4 Select

```rust
pub struct Select {
    focus_handle: FocusHandle,
    options: Vec<SharedString>,
    selected: usize,
    open: bool,
    placeholder: SharedString,
    on_change: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl Select {
    pub fn new(cx: &mut Context<Self>, options: Vec<impl Into<SharedString>>) -> Self;
    pub fn selected(&mut self, idx: usize, cx: &mut Context<Self>);
    pub fn placeholder(&mut self, p: impl Into<SharedString>) -> &mut Self;
    pub fn on_change(&mut self, h: impl Fn(&usize, &mut Window, &mut App) + 'static) -> &mut Self;
    pub fn current(&self) -> Option<&str>;
    fn toggle(&mut self, cx: &mut Context<Self>);
    fn select(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>);
}

impl Render for Select { ... }
impl Focusable for Select { ... }
```

**视觉**：

- 触发器：高 28px、`radius.sm`、bg = `input`、`border` border-1
- 触发器内：选中文字（左对齐）+ ChevronDown icon（右对齐）
- 弹层：在触发器下方，bg = `popover`、`radius.md`、shadow（暂不做 GPUI 阴影限制，用 `border` 替代）
- 弹层项：高 28px、padding-x 12、hover 时 bg = `accent`、选中项前缀 ✓ icon（也可省）

**交互**：

- 点击触发器 → `toggle(open)`
- 点击弹层项 → `select(idx)` + `on_change(idx)` + 关闭弹层
- Esc / 点弹层外 → 关闭弹层
- 键盘：聚焦时 ↑/↓ 切换 selected，Enter 关闭弹层

**M12 简化**：弹层只向下弹，无空间也不翻转。

### 3.5 Tabs

```rust
pub struct Tabs {
    focus_handle: FocusHandle,
    labels: Vec<SharedString>,
    active: usize,
    on_change: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl Tabs {
    pub fn new(cx: &mut Context<Self>, labels: Vec<impl Into<SharedString>>) -> Self;
    pub fn active(&self) -> usize;
    pub fn set_active(&mut self, idx: usize, cx: &mut Context<Self>);
    pub fn on_change(&mut self, h: impl Fn(&usize, &mut Window, &mut App) + 'static) -> &mut Self;
}

impl Render for Tabs { ... }
impl Focusable for Tabs { ... }
```

**视觉**（横向，仅 horizontal）：

- 整体宽度 = 父容器宽度（fit-content 也可，调用方决定）
- 每个 tab 头：高 32px、padding-x 12px、`font_size.sm` `secondary_foreground`
- 选中 tab：底部 2px 横线 = `primary`，文字色变 `foreground`
- hover：bg = `accent` 半透明（M12 简化省 hover 高亮）
- 内容面板：在 tab 头下方，调用方手动渲染：`tabs.read(cx).active()` 拿索引选 child

**交互**：

- 点击 tab 头切换 active
- 键盘 ←/→（聚焦时）切换 active
- on_change 回调

**注**：Tabs 只画 tab 头，不画内容。调用方根据 `active()` 渲染对应 panel。这点与 shadcn 的 `<Tabs>` 含子 `<TabsContent>` 不同，但更贴 GPUI 习惯（避免 `Children` slot 复杂）。

## 4. View 迁移

### 4.1 HostFormModal

**当前**：805 行，自实现 modal + 6 字段 + 单选按钮切 KeyFile/Password。

**迁移后**：

```rust
pub struct HostFormModal {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: Sender<SshEvent>,
    dialog: Entity<Dialog>,
    auth_tabs: Entity<Tabs>,           // 0: KeyFile, 1: Password
    label_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
    user_input: Entity<TextInput>,
    keyfile_input: Entity<TextInput>,  // 实际是 readonly + 文件 picker 触发
    password_input: Entity<TextInput>,
    draft_id: Option<HostId>,           // 新建 vs 编辑
}

impl HostFormModal {
    pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, tx: Sender<SshEvent>, cx: &mut Context<Self>) -> Self {
        let dialog = cx.new(|cx| Dialog::new(cx));
        let auth_tabs = cx.new(|cx| Tabs::new(cx, vec!["Key File", "Password"]));
        // 6 个 TextInput cx.new(...)
        // dialog 上挂 on_close 调 close_modal
        Self { ... }
    }

    pub fn open_for_new(&mut self, cx: &mut Context<Self>);
    pub fn open_for_edit(&mut self, host: HostConfig, cx: &mut Context<Self>);
    pub fn close(&mut self, cx: &mut Context<Self>);
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn delete(&mut self, cx: &mut Context<Self>);
    fn pick_keyfile(&mut self, cx: &mut Context<Self>);
}

impl Render for HostFormModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // dialog 自动处理 open/close 和 backdrop
        let active = self.auth_tabs.read(cx).active();
        let body = div()
            .flex_col()
            .gap(theme(cx).spacing.px_3)
            .child(self.label_input.clone())
            .child(self.host_input.clone())
            .child(self.port_input.clone())
            .child(self.user_input.clone())
            .child(self.auth_tabs.clone())
            .child(if active == 0 {
                div().child(self.keyfile_input.clone()).into_any_element()
            } else {
                div().child(self.password_input.clone()).into_any_element()
            })
            .child(/* save / delete / cancel buttons */);
        self.dialog.update(cx, |d, _| d.body(body));
        self.dialog.clone()
    }
}
```

预期减少 ~250 行。

### 4.2 SessionPickerView

**当前**：197 行，自实现 modal + 列表 + 键盘导航。

**迁移后**：

```rust
pub struct SessionPickerView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: Sender<SshEvent>,
    dialog: Entity<Dialog>,
    selected_index: usize,
    // sessions 从 state 读，无需自己存
}

impl Render for SessionPickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.state.read(cx);
        let sessions = /* extract from app.tmux_state */;
        let selected = self.selected_index;
        let list = div().flex_col().children(
            sessions.iter().enumerate().map(|(i, s)| {
                div()
                    .h(px(28.0))
                    .px(theme(cx).spacing.px_3)
                    .bg(if i == selected { theme(cx).colors.accent } else { transparent })
                    .child(/* session label */)
                    .on_mouse_down(... attach on click)
            })
        );
        self.dialog.update(cx, |d, _| d.body(list));
        self.dialog.clone()
    }
}
```

预期减少 ~30-50 行。

### 4.3 SettingsView 加 Appearance Section

```rust
// 已有 SettingsView struct，加一个 dark_switch 字段：
pub struct SettingsView {
    // ... 原有字段
    dark_switch_state: bool,    // 本地状态，默认 true（dark）
}

// 在 render 内 Section 列表加 "Appearance" 块：
.child(section_header(cx, "Appearance"))
.child(
    div()
        .flex().flex_row().items_center().justify_between()
        .child(div().text_color(theme(cx).colors.foreground).child("Dark mode"))
        .child(
            Switch::new("dark-mode-switch")
                .checked(self.dark_switch_state)
                .on_change(cx.listener(|this, new_value: &bool, _, cx| {
                    if !*new_value {
                        // 用户切到 Light
                        aish_ui::toast_warning(cx, "Light theme not yet implemented");
                        // 不更新 state，Switch 视觉回滚下一帧
                        cx.notify();
                    } else {
                        this.dark_switch_state = *new_value;
                        cx.notify();
                    }
                }))
        )
)
```

注：M12 简化版 Switch 是受控组件，调用方决定是否 commit 新值。这里点 Light（new_value=false）时，回调内不更新 `dark_switch_state`，下一帧 render 时 Switch 拿 `checked(true)` 自动回弹。

## 5. 单元测试矩阵

| 模块 | 测试 |
|---|---|
| `checkbox` | 构造默认 / `.checked(true)` / `.disabled(true)` / `.label("text")` 字段断言 |
| `switch` | 构造默认 / `.checked` / `.disabled` 字段断言 |
| `dialog` | `open()` / `close()` 状态机 / `is_open()` |
| `select` | 构造 / `set_selected(idx)` / `current()` 返回正确字符串 / `toggle()` 切换 open |
| `tabs` | 构造 / `set_active(idx)` / `active()` getter / 边界（idx >= len 时 clamp） |

预计 25+ 测试。aish-ui 51 → ~76。

## 6. Risk

| ID | 风险 | 应对 |
|---|---|---|
| R1 | Dialog focus trap GPUI 实现成本 | M12 简化只做 Esc + backdrop close，Tab 循环留 M13 |
| R2 | Select 弹层定位（弹下方 / 翻转向上） | M12 只弹下方，超出视口不做翻转。M13 评估改进 |
| R3 | HostFormModal SSH 行为回归 | 迁移前后跑手测：新建主机、编辑、连接、删除四条路径 |
| R4 | Tabs 键盘 ←/→ 与 HostFormModal 表单字段焦点冲突 | Tabs 只在自身聚焦时响应方向键，表单字段聚焦时方向键归属 TextInput |
| R5 | Switch on_change 不真正切 theme 用户体验 | UI 上 Switch 视觉立即回弹，toast 提示 "Light theme not yet implemented" |
| R6 | Dialog body slot 借用问题（render 时 update self.dialog 改 body） | render 内调 `self.dialog.update(cx, |d, _| d.body(body))` 是合法的 entity update 模式 |

## 7. Milestone 阶段拆分（plan 输入参考）

| 阶段 | 内容 | 工期 |
|---|---|---|
| **阶段一**：5 组件 | Checkbox / Switch / Tabs / Dialog / Select 各自独立 task | 1.5 天 |
| **阶段二**：HostFormModal 重写 | 用 Dialog + Tabs + TextInput | 1 天 |
| **阶段三**：SessionPicker 迁 + Settings Switch | 含 Appearance section | 0.5 天 |
| **阶段四**：收尾 | INDEX 更新 + 视觉回归 + DoD | 0.5 天 |

总计 3.5 天。

## 8. 完成定义（DoD）

- [ ] aish-ui 5 个新组件实现 + 25+ 单元测试
- [ ] aish-app 启动后 HostFormModal 视觉与原版相近，新建/编辑/删除/连接四条路径手测通过
- [ ] SessionPickerView 视觉相近，attach 流程正常
- [ ] Settings 有 Appearance section 含 Dark mode Switch，点 Light 弹 toast warning 且 Switch 视觉回弹
- [ ] 全部质量门禁通过：fmt + clippy 0 warning + test (aish-ui ~76 + aish-app ~101) 全过
- [ ] INDEX.md 更新 M12 条目
- [ ] 父 spec Risk R1 (focus trap)、R2 (Select 翻转)、R5 (Switch UX) 实际选择记录

## 9. 后续候选（M13 范围）

- DropdownMenu / ContextMenu
- Card / RadioGroup（M12 backlog 没用上）
- Light theme 实际实现（color tokens 重映射）
- Dialog Tab 键 focus 循环加固
- Select 弹层方向自适应（向上/向左）
- Button hover state（M11 遗留）
- Toast 关闭按钮（M11 遗留）
- aish-ui crate-level README + examples/
