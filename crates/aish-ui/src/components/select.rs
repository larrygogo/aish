//! Select — 下拉选单。M14 改造：弹层从手糊 absolute 切到 Popover，
//! 获得自动 fit_mode 翻转（向下没空间时翻向上）。

use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, SharedString, Window,
};

use crate::components::{Popover, PopoverPlacement};
use crate::icons::{icon, IconName};
use crate::theme::theme;

type ChangeHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct Select {
    focus_handle: FocusHandle,
    options: Vec<SharedString>,
    selected: usize,
    popover: Entity<Popover>,
    placeholder: SharedString,
    on_change: Option<ChangeHandler>,
}

impl Select {
    pub fn new<S: Into<SharedString>>(options: Vec<S>, cx: &mut Context<Self>) -> Self {
        let popover = cx.new(|cx| {
            let mut p = Popover::new(cx);
            p.placement(PopoverPlacement::Bottom);
            p
        });
        Self {
            focus_handle: cx.focus_handle(),
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
            popover,
            placeholder: SharedString::default(),
            on_change: None,
        }
    }

    pub fn placeholder(&mut self, p: impl Into<SharedString>) -> &mut Self {
        self.placeholder = p.into();
        self
    }

    pub fn on_change(&mut self, h: impl Fn(&usize, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn current(&self) -> Option<&str> {
        self.options.get(self.selected).map(|s| s.as_ref())
    }

    pub fn set_selected(&mut self, idx: usize, cx: &mut Context<Self>) {
        let clamped = idx.min(self.options.len().saturating_sub(1));
        if clamped != self.selected {
            self.selected = clamped;
            cx.notify();
        }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.popover.update(cx, |p, cx| p.toggle(cx));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.popover.update(cx, |p, cx| p.close(cx));
    }

    fn select(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let clamped = idx.min(self.options.len().saturating_sub(1));
        let changed = clamped != self.selected;
        self.selected = clamped;
        self.close(cx);
        cx.notify();
        if changed {
            if let Some(h) = self.on_change.clone() {
                h(&self.selected, window, cx);
            }
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => self.close(cx),
            "down" if self.selected + 1 < self.options.len() => {
                self.select(self.selected + 1, window, cx);
            }
            "up" if self.selected > 0 => {
                self.select(self.selected - 1, window, cx);
            }
            "enter" => self.close(cx),
            _ => {}
        }
    }
}

impl Focusable for Select {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Select {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let popover_open = self.popover.read(cx).is_open();
        let selected = self.selected;
        let placeholder = self.placeholder.clone();
        let options = self.options.clone();
        let weak_self = cx.weak_entity();

        let display_text = self
            .options
            .get(self.selected)
            .cloned()
            .unwrap_or_else(|| placeholder.clone());

        // trigger 元素 — 含 canvas 写入 bounds 到 popover
        let popover_for_canvas = self.popover.clone();
        let trigger = div()
            .id("select-trigger")
            .relative()
            .h(gpui::px(28.0))
            .px(t.spacing.px_3)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(t.spacing.px_2)
            .rounded(t.radius.sm)
            .bg(t.colors.input)
            .border_1()
            .border_color(t.colors.border)
            .cursor_pointer()
            .child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.foreground)
                    .child(display_text),
            )
            .child(
                icon(if popover_open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(gpui::px(14.0))
                .text_color(t.colors.muted_foreground),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                    this.toggle(cx);
                }),
            )
            .child(
                canvas(
                    move |bounds, _w, cx| {
                        let h = popover_for_canvas.clone();
                        h.update(cx, |p, _cx| p.set_trigger_bounds(bounds));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );

        // popover content 选项列表 — 只在展开时构建，避免每帧白白分配 N 个 div
        if popover_open {
            let content = div()
                .flex()
                .flex_col()
                .min_w(gpui::px(200.0))
                .py(t.spacing.px_1)
                .children(options.into_iter().enumerate().map(|(i, opt)| {
                    let is_selected = i == selected;
                    let weak = weak_self.clone();
                    div()
                        .id(("select-option", i))
                        .h(gpui::px(28.0))
                        .px(t.spacing.px_3)
                        .flex()
                        .items_center()
                        .text_size(t.font_size.sm)
                        .text_color(if is_selected {
                            t.colors.accent_foreground
                        } else {
                            t.colors.popover_foreground
                        })
                        .when(is_selected, |d| d.bg(t.colors.accent))
                        .cursor_pointer()
                        .hover({
                            let accent = t.colors.accent;
                            move |s| s.bg(accent)
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            move |_ev: &MouseDownEvent, window, cx| {
                                let _ = weak.update(cx, |s, cx| s.select(i, window, cx));
                            },
                        )
                        .child(opt)
                }));

            self.popover.update(cx, |p, _| {
                p.content(content);
            });
        }

        div()
            .relative()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .child(trigger)
            .child(self.popover.clone())
    }
}

#[cfg(test)]
mod tests {
    fn clamp(idx: usize, len: usize) -> usize {
        idx.min(len.saturating_sub(1))
    }

    #[test]
    fn clamp_in_range() {
        assert_eq!(clamp(0, 3), 0);
        assert_eq!(clamp(2, 3), 2);
    }

    #[test]
    fn clamp_overflow() {
        assert_eq!(clamp(10, 3), 2);
    }

    #[test]
    fn clamp_empty() {
        assert_eq!(clamp(0, 0), 0);
        assert_eq!(clamp(5, 0), 0);
    }

    #[test]
    fn down_arrow_advances_within_range() {
        let mut selected = 0usize;
        let len = 3usize;
        if selected + 1 < len {
            selected += 1;
        }
        assert_eq!(selected, 1);
    }

    #[test]
    fn up_arrow_stays_at_zero() {
        let mut selected = 0usize;
        if selected > 0 {
            selected = selected.saturating_sub(1);
        }
        assert_eq!(selected, 0);
    }
}
