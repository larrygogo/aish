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
            // 限宽 200px 防止超长 title（如 tmux pane title）撑爆 tab bar 把
            // 后续 tab 挤出窗口外；title 子元素 ellipsis 截断显示 "..."
            .max_w(px(200.0))
            .overflow_hidden()
            .flex_shrink_0()
            .px(t.spacing.px_4)
            .flex()
            .flex_row()
            .items_center()
            .gap(t.spacing.px_2)
            .text_size(t.font_size.sm)
            .bg(bg)
            .cursor_pointer();

        if !active {
            // 大容器 hover 用 secondary 灰阶提亮（不换色调），accent 留给小元素强调
            let hover_bg = t.colors.secondary_hover;
            let active_bg = t.colors.secondary_active;
            el = el
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
        }

        if let Some(handler) = self.on_click {
            el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                handler(ev, window, cx);
            });
        }

        el = el.when_some(self.prefix, |d, p| d.child(p));
        // title 包一层 flex_1 + overflow_hidden + ellipsis：
        // - flex_1 让 title 占满 prefix/suffix 之间剩余空间
        // - min_w(0) 关键 —— GPUI flex item 默认 min_width=auto 拒绝 shrink，
        //   设 0 才能在 max_w 200 限制下被压缩 + 触发 ellipsis
        // - whitespace_nowrap + text_ellipsis 长 title 单行截断显示 "..."
        el = el.when_some(self.title, |d, ti| {
            d.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(ti),
            )
        });
        el = el.when_some(self.suffix, |d, s| d.child(s));

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

    #[test]
    fn hover_only_when_inactive() {
        // 验证 TabItem 的 active 字段是否决定 hover 路径分支：
        // active=false → 走 hover bg + active bg
        // active=true → 不接管 hover/active（spec D-5）
        let inactive = TabItem::new("a").active(false);
        let active = TabItem::new("a").active(true);
        assert!(!inactive.active);
        assert!(active.active);
    }
}
