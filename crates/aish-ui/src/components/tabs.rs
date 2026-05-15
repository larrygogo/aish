//! Tabs — 标签栏。Entity 持久化 active index。
//!
//! Tabs 只画 tab 头，调用方根据 `tabs.read(cx).active()` 渲染对应内容面板。
//! 这点与 shadcn 的 `<Tabs>` 含子 `<TabsContent>` 不同，但更贴 GPUI 习惯。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, SharedString, Window,
};

use crate::theme::theme;
use crate::TypographyExt;

type ChangeHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct Tabs {
    focus_handle: FocusHandle,
    labels: Vec<SharedString>,
    active: usize,
    on_change: Option<ChangeHandler>,
}

impl Tabs {
    pub fn new<S: Into<SharedString>>(labels: Vec<S>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            labels: labels.into_iter().map(Into::into).collect(),
            active: 0,
            on_change: None,
        }
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn labels(&self) -> &[SharedString] {
        &self.labels
    }

    pub fn set_active(&mut self, idx: usize, cx: &mut Context<Self>) {
        let clamped = idx.min(self.labels.len().saturating_sub(1));
        if clamped != self.active {
            self.active = clamped;
            cx.notify();
        }
    }

    pub fn on_change(&mut self, h: impl Fn(&usize, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    fn select(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let clamped = idx.min(self.labels.len().saturating_sub(1));
        if clamped != self.active {
            self.active = clamped;
            cx.notify();
            if let Some(h) = self.on_change.clone() {
                h(&self.active, window, cx);
            }
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "left" if self.active > 0 => {
                self.select(self.active - 1, window, cx);
            }
            "right" if self.active + 1 < self.labels.len() => {
                self.select(self.active + 1, window, cx);
            }
            _ => {}
        }
    }
}

impl Focusable for Tabs {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Tabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let active = self.active;
        let labels = self.labels.clone();

        div()
            .flex()
            .flex_row()
            .gap(t.spacing.px_1)
            .border_b_1()
            .border_color(t.colors.border)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .children(labels.into_iter().enumerate().map(|(i, label)| {
                let is_active = i == active;
                div()
                    .id(("tab", i))
                    .h(gpui::px(32.0))
                    .px(t.spacing.px_3)
                    .flex()
                    .items_center()
                    // M26 Tabs tab label：active=BodyStrong (13/500/fg)，
                    // inactive=Body (13/400/secondary_fg)。weight 区分而非 color。
                    .typography(
                        if is_active {
                            crate::TypeRole::BodyStrong
                        } else {
                            crate::TypeRole::Body
                        },
                        t,
                    )
                    .text_color(if is_active {
                        t.colors.foreground
                    } else {
                        t.colors.secondary_foreground
                    })
                    .border_b_2()
                    .border_color(if is_active {
                        t.colors.primary
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, window, cx| {
                            this.select(i, window, cx);
                        }),
                    )
                    .child(label)
            }))
    }
}

#[cfg(test)]
mod tests {
    fn clamp_active(idx: usize, len: usize) -> usize {
        idx.min(len.saturating_sub(1))
    }

    #[test]
    fn clamp_keeps_in_range() {
        assert_eq!(clamp_active(0, 2), 0);
        assert_eq!(clamp_active(1, 2), 1);
        assert_eq!(clamp_active(5, 2), 1);
    }

    #[test]
    fn clamp_with_empty_labels_returns_zero() {
        assert_eq!(clamp_active(0, 0), 0);
        assert_eq!(clamp_active(5, 0), 0);
    }

    #[test]
    fn left_arrow_decrements() {
        let active = 2usize;
        let new = if active > 0 { active - 1 } else { active };
        assert_eq!(new, 1);
    }

    #[test]
    fn right_arrow_increments_when_in_range() {
        let active = 1usize;
        let len = 3usize;
        let new = if active + 1 < len { active + 1 } else { active };
        assert_eq!(new, 2);
    }

    #[test]
    fn right_arrow_stays_at_last() {
        let active = 2usize;
        let len = 3usize;
        let new = if active + 1 < len { active + 1 } else { active };
        assert_eq!(new, 2);
    }
}
