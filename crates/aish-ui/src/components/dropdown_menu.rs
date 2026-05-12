//! DropdownMenu — 菜单项列表。
//!
//! 本身 builder + RenderOnce 不管 open/close，作为 Popover content 传入。
//! 上层负责 Popover open 切换 + trigger element。
//!
//! M14 简化版：不接键盘导航（无内部 active index 状态机），只支持鼠标
//! click 选项。M15+ 可升级为 stateful Entity 加键盘 ↑↓。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Pixels, Window,
};

use crate::components::MenuItem;
use crate::icons::icon;
use crate::theme::theme;

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct DropdownMenu {
    id: ElementId,
    items: Vec<MenuItem>,
    on_select: Option<SelectHandler>,
    min_width: Option<Pixels>,
}

impl DropdownMenu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            on_select: None,
            min_width: None,
        }
    }

    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    pub fn on_select(mut self, h: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(h));
        self
    }

    pub fn min_width(mut self, w: Pixels) -> Self {
        self.min_width = Some(w);
        self
    }
}

impl RenderOnce for DropdownMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let on_select = self.on_select;

        let mut container = div().id(self.id).flex().flex_col().py(t.spacing.px_1);

        if let Some(w) = self.min_width {
            container = container.min_w(w);
        }

        container.children(self.items.into_iter().enumerate().map(|(i, item)| {
            let is_disabled = item.disabled;
            let fg = if is_disabled {
                t.colors.muted_foreground
            } else {
                t.colors.popover_foreground
            };

            let mut row = div()
                .h(gpui::px(28.0))
                .px(t.spacing.px_3)
                .flex()
                .flex_row()
                .items_center()
                .gap(t.spacing.px_2)
                .text_size(t.font_size.sm)
                .text_color(fg);

            if let Some(icon_name) = item.icon {
                row = row.child(icon(icon_name).size(gpui::px(14.0)).text_color(fg));
            }
            row = row.child(div().flex_1().child(item.label.clone()));
            if let Some(sc) = item.shortcut {
                row = row.child(
                    div()
                        .text_color(t.colors.muted_foreground)
                        .text_size(t.font_size.xs)
                        .child(sc),
                );
            }

            if !is_disabled {
                // hover 走 secondary 灰阶（与 Card / TabItem / Ghost button 等大容器
                // 一致），不再用 accent 暗绿染色
                let hover_bg = t.colors.secondary_hover;
                row = row.cursor_pointer().hover(move |s| s.bg(hover_bg));
                if let Some(handler) = on_select.clone() {
                    row = row.on_mouse_down(
                        MouseButton::Left,
                        move |_ev: &MouseDownEvent, window, cx| {
                            handler(&i, window, cx);
                        },
                    );
                }
            }

            row
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let m = DropdownMenu::new("test");
        assert!(m.items.is_empty());
        assert!(m.on_select.is_none());
        assert!(m.min_width.is_none());
    }

    #[test]
    fn items_chain() {
        let items = vec![MenuItem::new("a"), MenuItem::new("b")];
        let m = DropdownMenu::new("test").items(items);
        assert_eq!(m.items.len(), 2);
    }

    #[test]
    fn on_select_stored() {
        let m = DropdownMenu::new("test").on_select(|_, _, _| {});
        assert!(m.on_select.is_some());
    }
}
