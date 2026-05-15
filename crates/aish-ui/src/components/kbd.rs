//! KbdShortcut — 键盘快捷键视觉 chip。
//!
//! `<Kbd>⌘ K</Kbd>` 风格的小 chip，用于：
//! - Settings 快捷键展示页（列表中视觉化按键）
//! - EmptyState description 提示快捷键
//! - Home 「⌘K 搜索」hint 等
//!
//! 视觉：8px padding / radius 4 / 1px border / Code typography / 浅 bg。
//! stateless RenderOnce — 无 hover / press / state，纯展示。

use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};

use crate::theme::theme;
use crate::TypographyExt;

#[derive(IntoElement)]
pub struct Kbd {
    id: ElementId,
    keys: SharedString,
}

impl Kbd {
    pub fn new(id: impl Into<ElementId>, keys: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            keys: keys.into(),
        }
    }
}

impl RenderOnce for Kbd {
    fn render(self, _w: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let colors = t.colors;
        div()
            .id(self.id)
            .px(px(6.0))
            .py(px(1.0))
            .rounded(t.radius.sm)
            .border_1()
            .border_color(colors.border)
            .bg(colors.secondary)
            .typography(crate::TypeRole::Code, t)
            .text_color(colors.muted_foreground)
            .child(self.keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kbd_new_holds_keys() {
        let k = Kbd::new("k1", "⌘ K");
        assert_eq!(k.keys.as_ref(), "⌘ K");
    }

    #[test]
    fn kbd_id_distinct() {
        let _k1 = Kbd::new("a", "Esc");
        let _k2 = Kbd::new("b", "Enter");
        // 编译 + ctor 无 panic 即可，主要测 API 形态稳定
    }

    #[test]
    fn kbd_accepts_various_key_strings() {
        let _a = Kbd::new("a", "⌘ K");
        let _b = Kbd::new("b", "Ctrl+Shift+T");
        let _c = Kbd::new("c", "Esc");
        let _d = Kbd::new("d", "↑");
    }
}
