//! Switch — iOS 风格开关。受控。

use std::rc::Rc;

use gpui::{div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Window};

use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            disabled: false,
            on_change: None,
        }
    }

    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_change(mut self, h: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let checked = self.checked;

        let track_bg = if disabled {
            t.colors.muted
        } else if checked {
            t.colors.primary
        } else {
            t.colors.muted
        };

        let mut track = div()
            .id(self.id)
            .w(gpui::px(36.0))
            .h(gpui::px(20.0))
            .rounded(t.radius.full)
            .bg(track_bg)
            .flex()
            .items_center()
            .px(gpui::px(2.0));

        // 圆点位置：未选中时左侧，选中时右侧
        if checked {
            // 用 justify_end 把圆点推到右侧
            track = track.justify_end();
        } else {
            track = track.justify_start();
        }

        track = track.child(
            div()
                .w(gpui::px(16.0))
                .h(gpui::px(16.0))
                .rounded(t.radius.full)
                .bg(t.colors.foreground),
        );

        if !disabled {
            track = track.cursor_pointer();
            if let Some(handler) = self.on_change {
                let new_value = !checked;
                track = track.on_mouse_down(
                    MouseButton::Left,
                    move |_ev: &MouseDownEvent, window, cx| {
                        handler(&new_value, window, cx);
                    },
                );
            }
        }

        track
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let s = Switch::new("dark");
        assert!(!s.checked);
        assert!(!s.disabled);
        assert!(s.on_change.is_none());
    }

    #[test]
    fn checked_chain() {
        let s = Switch::new("a").checked(true);
        assert!(s.checked);
    }

    #[test]
    fn disabled_chain() {
        let s = Switch::new("a").disabled(true);
        assert!(s.disabled);
    }

    #[test]
    fn on_change_stored() {
        let s = Switch::new("a").on_change(|_, _, _| {});
        assert!(s.on_change.is_some());
    }
}
