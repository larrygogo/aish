---
title: M13 — aish-ui Card / NavItem / TabItem + 全 view 切组件
date: 2026-05-11
status: approved
parent: 2026-05-09-aish-ui-architecture-design.md
---

# M13 — aish-ui Card / NavItem / TabItem + 全 view 切组件

## 0. 关系

本 spec 是 [aish-ui 架构总览](2026-05-09-aish-ui-architecture-design.md) 下的第三个里程碑。M11/M12 已交付：

- M11：Foundations（Theme + Icon + 7 起步组件）
- M12：Forms & Nav（Checkbox / Switch / Tabs / Dialog / Select + HostFormModal/SessionPicker/Settings 迁移）

M13 目标：把 aish-app 剩下手糊的复合 view 元素（home host 卡片 / sidebar 4-tab / tab_bar tab 项）也切到 aish-ui 组件。

## 1. 范围

### 1.1 Scope（in）

3 个新组件：

- **Card**（builder + slot）—— header / body / footer / actions 四 slot + variant + on_click
- **NavItem**（builder）—— horizontal + vertical 双模，icon + label 任意 IntoElement，含 active indicator
- **TabItem**（builder + slot）—— 薄布局，prefix / title / suffix 三 slot + active + on_click（透 click_count）

3 处 view 迁移：

- **home.rs** host 卡片 → `Card`
- **sidebar_nav.rs** 4 tab → `NavItem.vertical()`
- **tab_bar.rs** tab 项 → `TabItem`

### 1.2 Out of scope（顺延 M14+）

- DropdownMenu / ContextMenu —— M11 spec 已列入 M13 候选，但 aish 当前 view 没明确使用场景，延后
- Light theme 实际实现 —— `Theme::light()` 仍 `unimplemented!()`，Settings Switch UI 演示不变
- TextInput mask 模式 —— HostForm password 字段需要，但与 Card/Nav/Tab 主线无关
- TextInput 精确点击定位（cursor_at_pixel）—— 同上
- Toast 关闭按钮 —— 现仅自动消失，独立小改动
- Dialog Tab focus trap —— 现 Esc + backdrop close 已足够
- Select 弹层方向自适应 —— 现固定向下弹

### 1.3 视觉非目标

不做样式重新设计——所有视觉效果与现有 view 看上去**一致**或更细，仅是结构换层。

## 2. 文件结构

```
crates/aish-ui/src/components/
  card.rs              # 新
  nav_item.rs          # 新
  tab_item.rs          # 新
  mod.rs               # 追加 mod / pub use

crates/aish-ui/src/prelude.rs       # 自动 re-export via pub use components::*

crates/aish-app/src/views/home.rs        # 重写 host 卡片
crates/aish-app/src/views/sidebar_nav.rs # 重写 4-tab
crates/aish-app/src/views/tab_bar.rs     # 重写 tab 项
```

## 3. 组件详细设计

### 3.1 Card

```rust
//! Card — 卡片容器。header / body / footer / actions 四 slot。
//!
//! 因为 `AnyElement` 不可 Clone，Card 走 `#[derive(IntoElement)] + RenderOnce`
//! 一次性消费（与 Dialog body 同模式）。每帧 render 时调用方通过 builder
//! 重新构造 Card 实例。

use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardVariant {
    Default,    // bg=card，无 border
    Outlined,   // bg=card + border-1 border
    Elevated,   // bg=card + border-1 ring (M13 暂以加强 border 模拟 shadow)
}

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    header: Option<AnyElement>,
    body: Option<AnyElement>,
    footer: Option<AnyElement>,
    /// actions slot — hover 时显示（group_hover 透明度切换）。
    /// 通常放编辑/删除等次级操作。
    actions: Option<AnyElement>,
    variant: CardVariant,
    on_click: Option<ClickHandler>,
}

impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self;
    pub fn header(self, h: impl IntoElement) -> Self;
    pub fn body(self, b: impl IntoElement) -> Self;
    pub fn footer(self, f: impl IntoElement) -> Self;
    pub fn actions(self, a: impl IntoElement) -> Self;
    pub fn variant(self, v: CardVariant) -> Self;
    pub fn outlined(self) -> Self { self.variant(CardVariant::Outlined) }
    pub fn elevated(self) -> Self { self.variant(CardVariant::Elevated) }
    pub fn on_click(self, h: impl Fn(...) + 'static) -> Self;
}

impl RenderOnce for Card { ... }
```

**视觉**：

- bg = `colors.card`
- rounded = `radius.lg`
- 整体 padding 由调用方在 slot 内部排（Card 不假设 px/py）
- variant Outlined：`.border_1().border_color(colors.border)`
- variant Elevated：`.border_1().border_color(colors.ring)`（GPUI 无 shadow，先用 ring 边框替代）
- on_click 存在时：`.cursor_pointer().hover(|s| s.bg(colors.accent))`
- actions slot 用 `.group("card-N").group_hover("card-N", |s| s.opacity(1.0))`，让 actions 在卡片 hover 时才显示（默认 opacity 0）

**布局**（render 内部）：

```rust
div().id(id).group(group_id)
    .flex().flex_col()
    .bg(colors.card)
    .rounded(radius.lg)
    .when(self.outlined / elevated, |d| d.border(...))
    .when(on_click_some, |d| d.cursor_pointer().hover(...).on_mouse_down(...))
    .when_some(header, |d, h| d.child(div().child(h)))
    .when_some(body,   |d, b| d.child(div().flex_1().child(b)))
    .when_some(footer, |d, f| d.child(div().child(f)))
    .when_some(actions, |d, a| d.child(
        div().opacity(0.0).group_hover(group_id, |s| s.opacity(1.0)).child(a)
    ))
```

注：`group("card-N")` 的 N 用 element id 派生避免多卡片相互影响。GPUI group API 需要 spike 验证。

### 3.2 NavItem

```rust
//! NavItem — 导航项。Horizontal（顶部栏）+ Vertical（侧栏）双模。
//!
//! icon 接受任意 IntoElement（SVG / Nerd Font / 纯文字），label 可选。
//! active 时画 indicator：vertical 在左侧 2px primary 条，
//! horizontal 在底部 2px primary 条。

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavItemOrientation { Vertical, Horizontal }

#[derive(IntoElement)]
pub struct NavItem {
    id: ElementId,
    icon: Option<AnyElement>,
    label: Option<SharedString>,
    active: bool,
    orientation: NavItemOrientation,
    on_click: Option<ClickHandler>,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>) -> Self;
    pub fn icon(self, i: impl IntoElement) -> Self;
    pub fn label(self, l: impl Into<SharedString>) -> Self;
    pub fn active(self, a: bool) -> Self;
    pub fn vertical(self) -> Self;
    pub fn horizontal(self) -> Self;
    pub fn on_click(self, h: impl Fn(...) + 'static) -> Self;
}
```

**视觉**：

| 状态 / 方向 | Vertical | Horizontal |
|---|---|---|
| 整体 | `w_full / py(14) / flex_col / items_center / gap(4)` | `h(36) / px(12) / flex_row / items_center / gap(8)` |
| Active indicator | 左侧 `.border_l_2().border_color(primary)` | 底部 `.border_b_2().border_color(primary)` |
| Active bg | `colors.card` | （无背景变化，仅底部线） |
| Inactive 占位 indicator | `.border_l_2().border_color(transparent)`（防 layout 跳动） | `.border_b_2().border_color(transparent)` |
| Icon / Label 色 | active = `foreground`，inactive = `muted_foreground` | 同 |
| Hover（inactive）| text → `secondary_foreground` | 同 |
| Cursor | pointer | pointer |

### 3.3 TabItem

```rust
//! TabItem — 顶部 tab 栏单项。薄布局 + 3 slot。
//!
//! Tab item 业务多变（连接状态 dot / inline rename / SSH chip / close 按钮），
//! TabItem 不试图通用化所有细节，只提供 prefix / title / suffix 三 slot + active
//! + on_click（透传 click_count）让调用方在 slot 内拼自己业务。

#[derive(IntoElement)]
pub struct TabItem {
    id: ElementId,
    prefix: Option<AnyElement>,
    title: Option<AnyElement>,
    suffix: Option<AnyElement>,
    active: bool,
    on_click: Option<ClickHandler>,  // 闭包参数含 MouseDownEvent.click_count
}

impl TabItem {
    pub fn new(id: impl Into<ElementId>) -> Self;
    pub fn prefix(self, p: impl IntoElement) -> Self;
    pub fn title(self, t: impl IntoElement) -> Self;
    pub fn suffix(self, s: impl IntoElement) -> Self;
    pub fn active(self, a: bool) -> Self;
    pub fn on_click(self, h: impl Fn(...) + 'static) -> Self;
}
```

**视觉**：

- 整体 `h(40) / px(16) / flex_row / items_center / gap(8) / text_size(sm)`
- Active：`bg(colors.background)` + 绝对定位底部 2px primary 横线
- Inactive：`bg(colors.card)` + hover `bg(colors.accent)`
- 内部 slot：prefix / title / suffix 顺序排
- `.relative()` 让绝对定位 bottom-line 相对自身

### 3.4 prelude

`pub use components::*;` 已经 glob，新增 3 个 component 自动 re-export。无需手动加。

## 4. View 迁移设计

### 4.1 home.rs host 卡片

**当前结构**（行 282-449）：
- avatar (40×40 圆角 + 首字母)
- label + SSH chip + active_chip
- host text (user@host:port)
- last_connected text
- actions（edit + delete IconButton，opacity 0 + group_hover 显示）
- chevron `›`

**迁移后**：

```rust
let header_row = div()
    .flex().flex_row().items_center().gap_3()
    .child(avatar)
    .child(
        div().flex_1().flex().flex_col().gap_0p5()
            .child(div().flex().flex_row().gap_2().items_center()
                .child(div().text_color(...).text_size(...).child(label))
                .child(aish_ui::Badge::new("SSH").primary())
                .children(active_chip))
            .child(div().text_color(...).text_size(...).child(host_text))
            .children(last_conn_str_div))
    .child(chevron);

let actions = div().flex().flex_row().gap_1()
    .child(edit_icon_button)
    .child(delete_icon_button);

aish_ui::Card::new(("host-card", host_id))
    .body(header_row)
    .actions(actions)
    .on_click(...)
```

注：原 host card 在 actions 用 `.group("host_card") + .group_hover(...)`——Card 现在自己管 group，调用方不再需要。

### 4.2 sidebar_nav.rs

**当前**：4 个手糊 div nav_item，含 active 高亮 + 左侧 2px primary。

**迁移后**：

```rust
let make_item = |tab, icon_char, cx: &mut Context<Self>| {
    let icon = div().font_family(FONT_NAME).text_size(px(16.0)).child(icon_char);
    aish_ui::NavItem::new(("sidebar-nav", tab as usize))
        .vertical()
        .icon(icon)
        .active(current == tab)
        .on_click(cx.listener(move |this, _, _, cx| this.handle_click(tab, cx)))
};

div().w(px(48.0)).h_full().flex().flex_col()
    .bg(colors.background)
    .border_r_1().border_color(colors.border)
    .child(make_item(SidebarTab::Home, "\u{f015}", cx))
    .child(make_item(SidebarTab::Terminal, "\u{f120}", cx))
    .child(make_item(SidebarTab::Inbox, "\u{f01c}", cx))
    .child(div().flex_1().flex().flex_col().justify_end()
        .child(make_item(SidebarTab::Settings, "\u{f013}", cx)))
```

**注**：`icon` 传入 `div().font_family(FONT_NAME).child(unicode)` —— NavItem 接受 `impl IntoElement`，把字符包成 div 即可。

### 4.3 tab_bar.rs

**当前**：tab item 手糊 + 选中态 bg 切换 + bottom-line + prefix dot + SSH chip + close 按钮。

**迁移后**：

```rust
let prefix = if is_connection {
    let dot_color = if is_alive { colors.success } else { colors.muted_foreground };
    div().text_color(dot_color).text_size(font_size.xs).child("●").into_any_element()
} else {
    div().into_any_element()
};

let title = if is_editing {
    div().border_1().border_color(colors.ring).rounded_md().px_1p5()
        .child(edit_buffer.clone()).into_any_element()
} else {
    div().text_color(...).child(title_text).into_any_element()
};

let suffix = div().flex().flex_row().items_center().gap_2()
    .when(is_connection, |d| d.child(aish_ui::Badge::new("SSH").primary()))
    .child(close_icon_button)
    .into_any_element();

aish_ui::TabItem::new(("tab", id))
    .prefix(prefix)
    .title(title)
    .suffix(suffix)
    .active(is_selected)
    .on_click(cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
        this.handle_tab_click(id, ev.click_count, w, cx);
    }))
```

inline rename buffer / close 按钮的 stop_propagation 仍保留在 close_icon_button 上。

## 5. 单元测试矩阵

| 组件 | 测试 |
|---|---|
| Card | 4-5 个：new_defaults / variant_chain / outlined/elevated shortcuts / slot 占位（header.is_some 等）/ on_click 存储 |
| NavItem | 4-5 个：new_defaults / orientation_chain（vertical/horizontal） / active_chain / icon/label 存储 / on_click 存储 |
| TabItem | 3-4 个：new_defaults / active_chain / slot 存储（prefix/title/suffix） / on_click 存储 |

预计 aish-ui 77 → ~88（+11）。

## 6. Risk

| ID | 风险 | 应对 |
|---|---|---|
| R1 | GPUI group / group_hover API 实际签名 | T1 起手 spike `.group(name)` + `.group_hover(name, |s| ...)` 是否能跨 element 协作 |
| R2 | TabItem on_click 透传 MouseDownEvent.click_count 给 caller | 验证 GPUI on_mouse_down 闭包内 `ev: &MouseDownEvent` 是否含 click_count（M11 已用过，应该 ok） |
| R3 | sidebar Nerd Font icon 包成 IntoElement | div().font_family(FONT_NAME).child(unicode) 试运行 |
| R4 | Card actions slot 默认 opacity 0 + hover 显示需要内部 group | M11 已有 home 卡片用过这模式，参考即可 |
| R5 | TabItem inline rename 状态机仍在 caller（tab_bar.rs），不迁组件 | 文档已说明这是有意为之 |

## 7. Milestone 阶段拆分

| 阶段 | 内容 | 工期 |
|---|---|---|
| 阶段一 | Card + NavItem + TabItem 三组件 + 测试 | 1.5 天 |
| 阶段二 | home 卡片迁 Card + sidebar_nav 迁 NavItem + tab_bar 迁 TabItem | 1 天 |
| 阶段三 | INDEX 更新 + 视觉手测 + DoD | 0.5 天 |

总计 ~3 天。

## 8. 完成定义（DoD）

- [ ] aish-ui 3 个新组件实现 + 11+ 单元测试
- [ ] home host 卡片视觉与原版一致或更好，edit/delete hover 显示正确
- [ ] sidebar 4 tab 视觉与原版一致，active indicator 正常
- [ ] tab_bar tab 项视觉与原版一致，rename 流程仍可用，close 按钮正常
- [ ] 质量门禁：fmt + clippy 0 warning + test 全过
- [ ] INDEX.md 更新 M13 条目
- [ ] aish-app 内手糊"复合 view 元素"清单完毕，剩余全部是 terminal_view（终端本体，不动）/ sidebar_nav 包装外（已在迁移内）/ tmux_sidebar（已废弃）

## 9. 后续候选（M14+）

- DropdownMenu / ContextMenu（需要 popover 定位 + 键盘导航）
- Light theme 实际实现 + Settings Switch 真切换
- TextInput mask 模式（HostForm password）
- TextInput 精确点击定位（cursor_at_pixel）
- Toast 关闭按钮 + 手动 dismiss API
- Dialog Tab focus trap
- Select 弹层方向自适应
- Button hover variant 精细化（Primary hover +10% 而非统一 accent）
