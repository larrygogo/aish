# M13 — aish-ui Card / NavItem / TabItem + 全 view 切组件 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 加 Card / NavItem / TabItem 三个 aish-ui 组件，把 aish-app 中 home host 卡片 / sidebar_nav 4 tab / tab_bar tab 项三处手糊"复合 view 元素"切到组件，完成 aish-app 全 view 组件化。

**Architecture:** 三组件均为 builder + `#[derive(IntoElement)]` + `RenderOnce`，slot 用 `Option<AnyElement>`（不可 Clone，每帧调用方重构 builder）。视觉效果保持与现有 view 一致或更精细。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- gpui (workspace dep)
- 测试：`cargo test --workspace`，每组件 in-file `#[cfg(test)] mod tests`

**Spec ref:** `docs/superpowers/specs/2026-05-11-aish-m13-cards-nav-design.md`

**质量门禁（每个 Task 完成后）：**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Task 1: Card 组件

**Files:**
- Create: `crates/aish-ui/src/components/card.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 card.rs**

```rust
//! Card — 卡片容器。header / body / footer / actions 四 slot。
//!
//! `AnyElement` 不可 Clone，Card 走 `#[derive(IntoElement)] + RenderOnce`
//! 一次性消费（与 Dialog body 同模式）。每帧调用方通过 builder 重新构造。

use std::rc::Rc;

use gpui::{
    div, prelude::*, AnyElement, App, ElementId, IntoElement, MouseButton, MouseDownEvent,
    SharedString, Window,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardVariant {
    Default,
    Outlined,
    Elevated,
}

#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    header: Option<AnyElement>,
    body: Option<AnyElement>,
    footer: Option<AnyElement>,
    /// actions slot — hover 时显示（group_hover 透明度切换）。
    actions: Option<AnyElement>,
    variant: CardVariant,
    on_click: Option<ClickHandler>,
}

impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            header: None,
            body: None,
            footer: None,
            actions: None,
            variant: CardVariant::Default,
            on_click: None,
        }
    }

    pub fn header(mut self, h: impl IntoElement) -> Self {
        self.header = Some(h.into_any_element());
        self
    }

    pub fn body(mut self, b: impl IntoElement) -> Self {
        self.body = Some(b.into_any_element());
        self
    }

    pub fn footer(mut self, f: impl IntoElement) -> Self {
        self.footer = Some(f.into_any_element());
        self
    }

    pub fn actions(mut self, a: impl IntoElement) -> Self {
        self.actions = Some(a.into_any_element());
        self
    }

    pub fn variant(mut self, v: CardVariant) -> Self {
        self.variant = v;
        self
    }

    pub fn outlined(self) -> Self {
        self.variant(CardVariant::Outlined)
    }

    pub fn elevated(self) -> Self {
        self.variant(CardVariant::Elevated)
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let id = self.id;
        let variant = self.variant;
        let on_click = self.on_click;
        let header = self.header;
        let body = self.body;
        let footer = self.footer;
        let actions = self.actions;

        // 用 id 派生 group 名，避免多 Card 实例 hover 互相影响
        let group_name = SharedString::from(format!("card-group-{:?}", &id));

        let mut el = div()
            .id(id)
            .group(group_name.clone())
            .flex()
            .flex_col()
            .bg(t.colors.card)
            .rounded(t.radius.lg);

        match variant {
            CardVariant::Default => {}
            CardVariant::Outlined => {
                el = el.border_1().border_color(t.colors.border);
            }
            CardVariant::Elevated => {
                el = el.border_1().border_color(t.colors.ring);
            }
        }

        if let Some(handler) = on_click {
            let accent = t.colors.accent;
            el = el
                .cursor_pointer()
                .hover(move |s| s.bg(accent))
                .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
        }

        // 内容布局：header 顶部 / body 中间 flex_1 / footer 底部 / actions 浮层
        el = el
            .when_some(header, |d, h| d.child(div().child(h)))
            .when_some(body, |d, b| d.child(div().flex_1().child(b)))
            .when_some(footer, |d, f| d.child(div().child(f)));

        if let Some(a) = actions {
            el = el.child(
                div()
                    .opacity(0.0)
                    .group_hover(group_name, |s| s.opacity(1.0))
                    .child(a),
            );
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let c = Card::new("test");
        assert_eq!(c.variant, CardVariant::Default);
        assert!(c.header.is_none());
        assert!(c.body.is_none());
        assert!(c.footer.is_none());
        assert!(c.actions.is_none());
        assert!(c.on_click.is_none());
    }

    #[test]
    fn variant_chain() {
        assert_eq!(Card::new("a").outlined().variant, CardVariant::Outlined);
        assert_eq!(Card::new("a").elevated().variant, CardVariant::Elevated);
    }

    #[test]
    fn slots_can_be_set() {
        let c = Card::new("a")
            .header(gpui::div())
            .body(gpui::div())
            .footer(gpui::div())
            .actions(gpui::div());
        assert!(c.header.is_some());
        assert!(c.body.is_some());
        assert!(c.footer.is_some());
        assert!(c.actions.is_some());
    }

    #[test]
    fn on_click_stored() {
        let c = Card::new("a").on_click(|_, _, _| {});
        assert!(c.on_click.is_some());
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

`crates/aish-ui/src/components/mod.rs` 加（字母序：badge 之后、button 之前）：

```rust
mod card;
pub use card::{Card, CardVariant};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T1 — Card 组件（4 slot + 3 variant + on_click + group_hover actions）"
```

预期 aish-ui 77 → 81。

---

## Task 2: NavItem 组件

**Files:**
- Create: `crates/aish-ui/src/components/nav_item.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 nav_item.rs**

```rust
//! NavItem — 导航项。Horizontal（顶部栏）+ Vertical（侧栏）双模。
//!
//! icon 接受任意 IntoElement（SVG / Nerd Font / 纯文字），label 可选。
//! active 时画 indicator：vertical 在左侧 2px primary 条，
//! horizontal 在底部 2px primary 条。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, IntoElement, MouseButton, MouseDownEvent,
    SharedString, Window,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavItemOrientation {
    Vertical,
    Horizontal,
}

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
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            label: None,
            active: false,
            orientation: NavItemOrientation::Vertical,
            on_click: None,
        }
    }

    pub fn icon(mut self, i: impl IntoElement) -> Self {
        self.icon = Some(i.into_any_element());
        self
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    pub fn active(mut self, a: bool) -> Self {
        self.active = a;
        self
    }

    pub fn vertical(mut self) -> Self {
        self.orientation = NavItemOrientation::Vertical;
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.orientation = NavItemOrientation::Horizontal;
        self
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let active = self.active;
        let orientation = self.orientation;
        let icon = self.icon;
        let label = self.label;
        let on_click = self.on_click;

        let fg = if active {
            t.colors.foreground
        } else {
            t.colors.muted_foreground
        };

        let indicator_color = if active {
            t.colors.primary
        } else {
            gpui::transparent_black()
        };

        let mut el = div().id(self.id).text_color(fg).cursor_pointer();

        // 方向相关样式
        el = match orientation {
            NavItemOrientation::Vertical => el
                .w_full()
                .py(px(14.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .border_l_2()
                .border_color(indicator_color),
            NavItemOrientation::Horizontal => el
                .h(px(36.0))
                .px(t.spacing.px_3)
                .flex()
                .flex_row()
                .items_center()
                .gap(t.spacing.px_2)
                .border_b_2()
                .border_color(indicator_color),
        };

        // active 时 vertical 给 card bg；horizontal 仅靠底部线
        if active && orientation == NavItemOrientation::Vertical {
            el = el.bg(t.colors.card);
        }

        // hover：inactive 时文字色变 secondary_foreground
        if !active {
            let hover_fg = t.colors.secondary_foreground;
            el = el.hover(move |s| s.text_color(hover_fg));
        }

        if let Some(handler) = on_click {
            el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                handler(ev, window, cx);
            });
        }

        el = el.when_some(icon, |d, i| d.child(i));
        el = el.when_some(label, |d, l| {
            d.child(div().text_size(t.font_size.sm).child(l))
        });

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let n = NavItem::new("test");
        assert!(!n.active);
        assert_eq!(n.orientation, NavItemOrientation::Vertical);
        assert!(n.icon.is_none());
        assert!(n.label.is_none());
        assert!(n.on_click.is_none());
    }

    #[test]
    fn orientation_chain() {
        assert_eq!(
            NavItem::new("a").horizontal().orientation,
            NavItemOrientation::Horizontal
        );
        assert_eq!(
            NavItem::new("a").vertical().orientation,
            NavItemOrientation::Vertical
        );
    }

    #[test]
    fn active_chain() {
        let n = NavItem::new("a").active(true);
        assert!(n.active);
    }

    #[test]
    fn icon_and_label_stored() {
        let n = NavItem::new("a").icon(gpui::div()).label("Home");
        assert!(n.icon.is_some());
        assert_eq!(n.label.as_ref().unwrap().as_ref(), "Home");
    }

    #[test]
    fn on_click_stored() {
        let n = NavItem::new("a").on_click(|_, _, _| {});
        assert!(n.on_click.is_some());
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

字母序：nav_item 放在 icon_button 之后、separator 之前。

```rust
mod nav_item;
pub use nav_item::{NavItem, NavItemOrientation};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T2 — NavItem 组件（vertical+horizontal 双模 + icon + active indicator）"
```

预期 aish-ui 81 → 86。

---

## Task 3: TabItem 组件

**Files:**
- Create: `crates/aish-ui/src/components/tab_item.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 tab_item.rs**

```rust
//! TabItem — 顶部 tab 栏单项。薄布局 + 3 slot。
//!
//! Tab item 业务多变（连接状态 dot / inline rename / SSH chip / close 按钮），
//! TabItem 不试图通用化所有细节，只提供 prefix / title / suffix 三 slot + active
//! + on_click（透传 click_count）让调用方在 slot 内拼自己业务。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, IntoElement, MouseButton, MouseDownEvent,
    Window,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct TabItem {
    id: ElementId,
    prefix: Option<AnyElement>,
    title: Option<AnyElement>,
    suffix: Option<AnyElement>,
    active: bool,
    on_click: Option<ClickHandler>,
}

impl TabItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            prefix: None,
            title: None,
            suffix: None,
            active: false,
            on_click: None,
        }
    }

    pub fn prefix(mut self, p: impl IntoElement) -> Self {
        self.prefix = Some(p.into_any_element());
        self
    }

    pub fn title(mut self, t: impl IntoElement) -> Self {
        self.title = Some(t.into_any_element());
        self
    }

    pub fn suffix(mut self, s: impl IntoElement) -> Self {
        self.suffix = Some(s.into_any_element());
        self
    }

    pub fn active(mut self, a: bool) -> Self {
        self.active = a;
        self
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for TabItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let active = self.active;
        let bg = if active {
            t.colors.background
        } else {
            t.colors.card
        };

        let mut el = div()
            .id(self.id)
            .relative()
            .h(px(40.0))
            .px(t.spacing.px_4)
            .flex()
            .flex_row()
            .items_center()
            .gap(t.spacing.px_2)
            .text_size(t.font_size.sm)
            .bg(bg)
            .cursor_pointer();

        if !active {
            let hover_bg = t.colors.accent;
            el = el.hover(move |s| s.bg(hover_bg));
        }

        if let Some(handler) = self.on_click {
            el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                handler(ev, window, cx);
            });
        }

        el = el.when_some(self.prefix, |d, p| d.child(p));
        el = el.when_some(self.title, |d, ti| d.child(ti));
        el = el.when_some(self.suffix, |d, s| d.child(s));

        // active 时底部 2px primary 横线（绝对定位贴底）
        if active {
            el = el.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(2.0))
                    .bg(t.colors.primary),
            );
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let t = TabItem::new("test");
        assert!(!t.active);
        assert!(t.prefix.is_none());
        assert!(t.title.is_none());
        assert!(t.suffix.is_none());
        assert!(t.on_click.is_none());
    }

    #[test]
    fn active_chain() {
        let t = TabItem::new("a").active(true);
        assert!(t.active);
    }

    #[test]
    fn slots_can_be_set() {
        let t = TabItem::new("a")
            .prefix(gpui::div())
            .title(gpui::div())
            .suffix(gpui::div());
        assert!(t.prefix.is_some());
        assert!(t.title.is_some());
        assert!(t.suffix.is_some());
    }

    #[test]
    fn on_click_stored() {
        let t = TabItem::new("a").on_click(|_, _, _| {});
        assert!(t.on_click.is_some());
    }
}
```

- [ ] **Step 2: 注册 mod.rs**

字母序：tab_item 放在 switch 之后、tabs 之前（注意有 `tabs.rs` 同名前缀，确认 tab_item < tabs）。

```rust
mod tab_item;
pub use tab_item::TabItem;
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T3 — TabItem 组件（3 slot + active + on_click 透 click_count）"
```

预期 aish-ui 86 → 90。

---

## Task 4: home host 卡片切到 Card

**Files:**
- Modify: `crates/aish-app/src/views/home.rs`

- [ ] **Step 1: 重写 host 卡片渲染**

替换 home.rs 内 `// ───── Hosts grid ─────` 注释下方 host 卡片渲染（约 line 279-450 的 `cards: Vec<_>` 那段）。

原代码：手糊 `div().group("host_card").bg(card).rounded_2xl().on_mouse_down(...).flex_row()` + actions opacity + group_hover。

新代码（关键替换）：

```rust
let cards: Vec<_> = app
    .hosts
    .iter()
    .map(|h| {
        let id = h.id;
        let label = h.label.clone();
        let host_text = format!("{}@{}:{}", h.user, h.host, h.port);
        let last_conn_str: Option<String> = app
            .last_connected
            .get(&id)
            .map(|t| humanize_last_connected(*t));

        let active_count = app.connections.values().filter(|c| c.host_id == id).count();

        // avatar
        let initial = label.chars().next().unwrap_or('?').to_uppercase().to_string();
        let avatar_bg = crate::avatar::avatar_color_for(&label);
        let avatar = div()
            .w(px(40.0)).h(px(40.0))
            .flex().items_center().justify_center()
            .bg(rgb(avatar_bg))
            .rounded_xl()
            .text_color(colors.primary_foreground)
            .text_size(font_size.lg)
            .child(initial);

        // active chip（仅当 active_count > 0）
        let active_chip: Option<gpui::AnyElement> = if active_count > 0 {
            Some(aish_ui::Badge::new(format!("● {} 活跃", active_count)).success().into_any_element())
        } else {
            None
        };

        // chevron
        let chevron = div()
            .text_color(colors.muted_foreground)
            .text_size(font_size.lg)
            .child("›");

        // header body：avatar + (label + chips) + (host text) + chevron
        let body_row = div()
            .flex().flex_row().items_center().gap_3()
            .px_4().py_3p5()  // padding 仍由调用方控（Card 不假设）
            .child(avatar)
            .child(
                div()
                    .flex_1().flex().flex_col().gap_0p5()
                    .child(
                        div().flex().flex_row().gap_2().items_center()
                            .child(div().text_color(colors.foreground).text_size(font_size.lg).child(label))
                            .child(aish_ui::Badge::new("SSH").primary())
                            .children(active_chip),
                    )
                    .child(div().text_color(colors.secondary_foreground).text_size(font_size.sm).child(host_text))
                    .children(last_conn_str.map(|s| {
                        div()
                            .text_color(colors.muted_foreground)
                            .text_size(px(11.0))
                            .child(format!("上次连接 {}", s))
                    })),
            )
            .child(chevron);

        // edit / delete icon buttons
        let edit_btn = aish_ui::IconButton::new(
            gpui::SharedString::from(format!("host-edit-{}", id)),
            aish_ui::IconName::Pencil,
        )
        .small()
        .ghost()
        .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
            cx.stop_propagation();
            this.handle_edit_click(id, cx);
        }));

        let delete_btn = aish_ui::IconButton::new(
            gpui::SharedString::from(format!("host-delete-{}", id)),
            aish_ui::IconName::X,
        )
        .small()
        .destructive()
        .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
            cx.stop_propagation();
            this.handle_delete_click(id, cx);
        }));

        let actions = div()
            .absolute()
            .top_2()
            .right_2()
            .flex()
            .flex_row()
            .gap_1()
            .child(edit_btn)
            .child(delete_btn);

        aish_ui::Card::new(gpui::SharedString::from(format!("host-card-{}", id)))
            .body(body_row)
            .actions(actions)
            .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                this.handle_card_click(id, cx);
            }))
            .into_any_element()
    })
    .collect();
```

注意：
- `actions` 用 `.absolute().top_2().right_2()` 浮在卡片右上角；Card 内部 group_hover 控显隐
- Card body 内的 padding（px_4 py_3p5）由调用方在 body row 自己排
- Card 整体 rounded 走 `radius.lg`（不是原来的 `rounded_2xl`，视觉略小一档；可接受）

- [ ] **Step 2: 检查未使用的 import / 旧代码清理**

旧代码 host card 内含 `.group("host_card") + .group_hover("host_card", ...)`，迁移后 group 由 Card 管，调用方不需要。

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-app/src/views/home.rs
git commit -m "refactor(aish-app): home host 卡片切到 aish_ui::Card"
```

---

## Task 5: sidebar_nav 4 tab 切到 NavItem

**Files:**
- Modify: `crates/aish-app/src/views/sidebar_nav.rs`

- [ ] **Step 1: 重写 sidebar_nav.rs**

替换文件内 `nav_item` 闭包 + sidebar 渲染：

```rust
//! SidebarNav：左侧 48px iconfont 4-tab 导航（M4a 信息架构）。
//!
//! M13 重写为用 aish_ui::NavItem.vertical()，icon 通过 div+font_family 包装传入。

use gpui::{div, prelude::*, px, Context, Entity, MouseDownEvent, Window};

use crate::state::{AppState, SidebarTab};
use crate::terminal::font::FONT_NAME;

const SIDEBAR_WIDTH: f32 = 48.0;

pub struct SidebarNavView {
    state: Entity<AppState>,
}

impl SidebarNavView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }

    fn handle_click(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.sidebar = tab;
            cx.notify();
        });
    }
}

impl Render for SidebarNavView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.state.read(cx).sidebar;
        let colors = aish_ui::theme(cx).colors;

        let make_item = |tab: SidebarTab,
                         icon_char: &'static str,
                         tag: &'static str,
                         cx: &mut Context<SidebarNavView>| {
            let icon = div()
                .font_family(FONT_NAME)
                .text_size(px(16.0))
                .child(icon_char);
            aish_ui::NavItem::new(("sidebar-nav", tag))
                .vertical()
                .icon(icon)
                .active(current == tab)
                .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_click(tab, cx);
                }))
        };

        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .border_r_1()
            .border_color(colors.border)
            .child(make_item(SidebarTab::Home, "\u{f015}", "home", cx))
            .child(make_item(SidebarTab::Terminal, "\u{f120}", "terminal", cx))
            .child(make_item(SidebarTab::Inbox, "\u{f01c}", "inbox", cx))
            .child(
                div().flex_1().flex().flex_col().justify_end().child(make_item(
                    SidebarTab::Settings,
                    "\u{f013}",
                    "settings",
                    cx,
                )),
            )
    }
}
```

注：`("sidebar-nav", tag)` 用 (str, str) tuple 作为 ElementId，4 个 tab 各有 unique tag。

- [ ] **Step 2: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-app/src/views/sidebar_nav.rs
git commit -m "refactor(aish-app): sidebar_nav 4 tab 切到 aish_ui::NavItem.vertical()"
```

---

## Task 6: tab_bar tab 项切到 TabItem

**Files:**
- Modify: `crates/aish-app/src/views/tab_bar.rs`

- [ ] **Step 1: 重写 tab_items 渲染**

替换 tab_bar.rs 内 `tab_items: Vec<_>` map 内部逻辑（约 line 205-336）。**rename 状态机和 close 按钮逻辑保留不变**，只把外层 div 换成 TabItem + 3 slot。

```rust
let tab_items: Vec<_> = app
    .tabs
    .iter()
    .map(|t| {
        let id = t.id;
        let title_text = t.title.clone();
        let is_selected = selected == Some(id);
        let is_connection = matches!(t.content, TabContent::Connection(_));
        let is_editing = editing_tab == Some(id);
        let is_alive = match t.content {
            TabContent::Connection(c) => app.is_session_active(c),
            _ => false,
        };

        // prefix: connection alive/dead dot
        let prefix: gpui::AnyElement = if is_connection {
            let dot_color = if is_alive { colors.success } else { colors.muted_foreground };
            div().text_color(dot_color).text_size(font_size.xs).child("●").into_any_element()
        } else {
            div().into_any_element()
        };

        // title: rename buffer 或 plain 文字
        let title_el: gpui::AnyElement = if is_editing {
            div()
                .text_color(colors.foreground)
                .border_1().border_color(colors.ring).rounded_md().px_1p5()
                .child(edit_buffer.clone())
                .into_any_element()
        } else {
            let title_color = if is_connection && !is_alive {
                colors.muted_foreground
            } else if is_selected {
                colors.foreground
            } else {
                colors.secondary_foreground
            };
            div().text_color(title_color).child(title_text).into_any_element()
        };

        // suffix: SSH chip + close button
        let close_btn = aish_ui::IconButton::new(
            gpui::SharedString::from(format!("tab-close-{}", id)),
            aish_ui::IconName::X,
        )
        .small()
        .ghost()
        .on_click(cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
            cx.stop_propagation();
            this.handle_close(id, cx);
        }));

        let suffix = div()
            .flex().flex_row().items_center().gap_2()
            .when(is_connection, |d| d.child(aish_ui::Badge::new("SSH").primary()))
            .child(close_btn)
            .into_any_element();

        aish_ui::TabItem::new(gpui::SharedString::from(format!("tab-{}", id)))
            .prefix(prefix)
            .title(title_el)
            .suffix(suffix)
            .active(is_selected)
            .on_click(cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                this.handle_tab_click(id, ev.click_count, w, cx);
            }))
            .into_any_element()
    })
    .collect();
```

- [ ] **Step 2: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-app/src/views/tab_bar.rs
git commit -m "refactor(aish-app): tab_bar tab 项切到 aish_ui::TabItem"
```

---

## Task 7: INDEX 更新 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 加 M13 条目到 Milestones 列表最顶端**

```markdown
### M13 — aish-ui Card / NavItem / TabItem + 全 view 切组件（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m13-cards-nav-design.md`](specs/2026-05-11-aish-m13-cards-nav-design.md)
- plan：[`plans/2026-05-11-aish-m13-cards-nav.md`](plans/2026-05-11-aish-m13-cards-nav.md)
- 范围：3 个新组件（Card / NavItem / TabItem，全部 builder + slot）+ 3 处 view 迁移（home host 卡片 / sidebar_nav 4 tab / tab_bar tab 项）。aish-app 内的复合 view 元素全部组件化（仅 terminal_view 本体 + 已废弃 tmux_sidebar 保留手糊）
- 关键 commits：T1-T7
- 测试：aish-ui 77 → ~90 + aish-app 101 不变
- 已知边界：Light theme / DropdownMenu / TextInput mask 等仍顺延 M14+

更新 `## 当前状态`：
- **活跃分支**：`main`（M13 已完成）
- **下一里程碑**：M14 — DropdownMenu/ContextMenu + Light theme + TextInput mask（视用户优先级而定）
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui ~90 + aish-app 101 + 其他) 全过
```

- [ ] **Step 2: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T7 — INDEX 更新 M13 已完成"
```

---

## DoD 自检

回看 spec § 8：

- [ ] aish-ui 3 个新组件 + 11+ 测试 ←→ T1-T3
- [ ] home host 卡片视觉与原版一致，edit/delete hover 显示正确 ←→ T4
- [ ] sidebar 4 tab 视觉与原版一致，active indicator 正常 ←→ T5
- [ ] tab_bar tab 项视觉与原版一致，rename 流程可用，close 正常 ←→ T6
- [ ] 质量门禁全过 ←→ 每 task 末尾
- [ ] INDEX.md 更新 M13 ←→ T7
- [ ] aish-app 手糊"复合 view 元素"清单完毕 ←→ 收尾验证

---

## 后续候选（M14+）

- DropdownMenu / ContextMenu（需要 popover 定位）
- Light theme 实际实现 + Settings Switch 真切
- TextInput mask 模式（HostForm password）
- TextInput cursor_at_pixel
- Toast 关闭按钮
- Dialog Tab focus trap
- Select 弹层方向自适应
- Button hover variant 精细化
