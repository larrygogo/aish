//! Separator — 横向 / 纵向分割线。

use gpui::{div, prelude::*, px, App, IntoElement, Window};

use crate::theme::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(IntoElement)]
pub struct Separator {
    orientation: Orientation,
}

impl Separator {
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
        }
    }

    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
        }
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = theme(cx).colors.border;
        match self.orientation {
            Orientation::Horizontal => div().w_full().h(px(1.0)).bg(color),
            Orientation::Vertical => div().h_full().w(px(1.0)).bg(color),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_constructor() {
        let s = Separator::horizontal();
        assert_eq!(s.orientation, Orientation::Horizontal);
    }

    #[test]
    fn vertical_constructor() {
        let s = Separator::vertical();
        assert_eq!(s.orientation, Orientation::Vertical);
    }
}
