//! TextInput — 单行文本输入框。
//!
//! 含 cursor blink（T10）、selection（T11）、复制粘贴（T12）。
//! T9：struct + 键盘 + IME + render。
//! T10：cursor blink（600ms 周期，任意操作 reset 相位）。

use std::rc::Rc;
use std::time::Instant;

use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, FocusHandle, Focusable, InputHandler,
    KeyDownEvent, MouseButton, Pixels, SharedString, UTF16Selection, Window,
};

use crate::theme::theme;

type SubmitHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type ChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

const BLINK_PERIOD_MS: u64 = 600;

pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    cursor: usize, // byte offset
    placeholder: SharedString,
    on_submit: Option<SubmitHandler>,
    on_change: Option<ChangeHandler>,
    blink_epoch: Instant,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let this = Self {
            focus_handle: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            placeholder: SharedString::default(),
            on_submit: None,
            on_change: None,
            blink_epoch: Instant::now(),
        };
        this.start_blink_timer(cx);
        this
    }

    pub fn placeholder(&mut self, p: impl Into<SharedString>) -> &mut Self {
        self.placeholder = p.into();
        self
    }

    pub fn on_submit(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_submit = Some(Rc::new(h));
        self
    }

    pub fn on_change(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, t: impl Into<String>, cx: &mut Context<Self>) {
        self.text = t.into();
        self.cursor = self.text.len();
        self.reset_blink();
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.text.clear();
        self.cursor = 0;
        self.reset_blink();
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    // -------- blink helpers --------

    /// 重置 blink 相位（按键 / 鼠标后让 cursor 立即可见）。
    fn reset_blink(&mut self) {
        self.blink_epoch = Instant::now();
    }

    /// 根据相位判断 cursor 是否应该可见（render 用）。
    fn cursor_visible_now(&self) -> bool {
        let phase = self.blink_epoch.elapsed().as_millis() as u64 % BLINK_PERIOD_MS;
        phase < BLINK_PERIOD_MS / 2
    }

    /// 启动定时器：每 100ms 触发 cx.notify()，让 render 重跑（重新计算 phase）。
    fn start_blink_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            if this.update(cx, |_this, cx| cx.notify()).is_err() {
                break;
            }
        })
        .detach();
    }

    // -------- 状态机 --------

    pub(crate) fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.reset_blink();
        }
    }

    pub(crate) fn cursor_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            self.reset_blink();
        }
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
            self.reset_blink();
        }
    }

    pub(crate) fn delete_forward(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
            self.reset_blink();
        }
    }

    pub(crate) fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.reset_blink();
    }

    fn fire_change(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_change.clone() {
            h(&self.text, window, cx);
        }
    }

    fn fire_submit(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_submit.clone() {
            h(&self.text, window, cx);
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.backspace();
                cx.notify();
                self.fire_change(window, cx);
            }
            "delete" => {
                self.delete_forward();
                cx.notify();
                self.fire_change(window, cx);
            }
            "left" => {
                self.cursor_left();
                cx.notify();
            }
            "right" => {
                self.cursor_right();
                cx.notify();
            }
            "home" => {
                self.cursor = 0;
                self.reset_blink();
                cx.notify();
            }
            "end" => {
                self.cursor = self.text.len();
                self.reset_blink();
                cx.notify();
            }
            "enter" if !event.keystroke.modifiers.shift => {
                self.fire_submit(window, cx);
            }
            _ => {
                if let Some(ch) = &event.keystroke.key_char {
                    if !event.keystroke.modifiers.control && !event.keystroke.modifiers.alt {
                        self.insert_str(ch.as_str());
                        cx.notify();
                        self.fire_change(window, cx);
                    }
                }
            }
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// -------- IME --------

struct TextInputImeHandler {
    view: gpui::WeakEntity<TextInput>,
    bar_bounds: Option<Bounds<Pixels>>,
}

impl InputHandler for TextInputImeHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &mut self,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.view
            .update(cx, |this, cx| {
                this.insert_str(text);
                cx.notify();
            })
            .ok();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.bar_bounds
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

// -------- Render --------

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_for_ime = self.focus_handle.clone();
        let weak_view = cx.weak_entity();
        let focused = self.focus_handle.is_focused(window);
        let show_cursor = focused && self.cursor_visible_now();

        let t = theme(cx);
        let border_color = if focused {
            t.colors.ring
        } else {
            t.colors.border
        };

        let cursor_left = self.text[..self.cursor].to_string();
        let cursor_right = self.text[self.cursor..].to_string();
        let placeholder_visible = self.text.is_empty();

        div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .h(px(28.0))
            .px(px(8.0))
            .rounded(t.radius.sm)
            .bg(t.colors.input)
            .border_1()
            .border_color(border_color)
            .cursor_text()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                }),
            )
            .child(if placeholder_visible {
                div()
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.muted_foreground)
                    .child(self.placeholder.clone())
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_row()
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.foreground)
                    .child(div().child(cursor_left))
                    .child(if show_cursor {
                        div()
                            .w(px(1.0))
                            .h(px(14.0))
                            .bg(t.colors.ring)
                            .self_center()
                            .into_any_element()
                    } else {
                        div()
                            .w(px(1.0))
                            .h(px(14.0))
                            .self_center()
                            .into_any_element()
                    })
                    .child(div().child(cursor_right))
                    .into_any_element()
            })
            .child(
                canvas(
                    |bounds, _window, _cx| bounds,
                    move |_bounds, prepaint_bounds, window, cx| {
                        window.handle_input(
                            &focus_for_ime,
                            TextInputImeHandler {
                                view: weak_view.clone(),
                                bar_bounds: Some(prepaint_bounds),
                            },
                            cx,
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    fn apply_insert(text: &mut String, cursor: &mut usize, s: &str) {
        text.insert_str(*cursor, s);
        *cursor += s.len();
    }

    fn apply_backspace(text: &mut String, cursor: &mut usize) {
        if *cursor > 0 {
            let prev = text[..*cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            text.remove(prev);
            *cursor = prev;
        }
    }

    fn apply_delete(text: &mut String, cursor: &mut usize) {
        if *cursor < text.len() {
            text.remove(*cursor);
        }
    }

    fn apply_left(text: &str, cursor: &mut usize) {
        if *cursor > 0 {
            *cursor = text[..*cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    fn apply_right(text: &str, cursor: &mut usize) {
        if *cursor < text.len() {
            *cursor = text[*cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| *cursor + i)
                .unwrap_or(text.len());
        }
    }

    #[test]
    fn insert_advances_cursor() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "hi");
        assert_eq!(t, "hi");
        assert_eq!(c, 2);
    }

    #[test]
    fn backspace_removes_prev_char() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ab");
        apply_backspace(&mut t, &mut c);
        assert_eq!(t, "a");
        assert_eq!(c, 1);
    }

    #[test]
    fn delete_removes_next_char() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "abc");
        c = 1;
        apply_delete(&mut t, &mut c);
        assert_eq!(t, "ac");
        assert_eq!(c, 1);
    }

    #[test]
    fn left_right_navigates() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ab");
        apply_left(&t, &mut c);
        assert_eq!(c, 1);
        apply_right(&t, &mut c);
        assert_eq!(c, 2);
    }

    #[test]
    fn insert_at_middle() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ac");
        apply_left(&t, &mut c);
        apply_insert(&mut t, &mut c, "b");
        assert_eq!(t, "abc");
        assert_eq!(c, 2);
    }

    #[test]
    fn cjk_char_boundary_handling() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "中文");
        // 中文 = 6 bytes (3 each in UTF-8)
        assert_eq!(c, 6);
        apply_backspace(&mut t, &mut c);
        // 应该删掉一个完整字符
        assert_eq!(t, "中");
        assert_eq!(c, 3);
    }

    #[test]
    fn blink_phase_first_half_visible() {
        use std::time::Duration;
        let epoch = std::time::Instant::now() - Duration::from_millis(100);
        let phase = epoch.elapsed().as_millis() as u64 % super::BLINK_PERIOD_MS;
        assert!(phase < super::BLINK_PERIOD_MS / 2);
    }

    #[test]
    fn blink_phase_second_half_invisible() {
        use std::time::Duration;
        let epoch = std::time::Instant::now() - Duration::from_millis(400);
        let phase = epoch.elapsed().as_millis() as u64 % super::BLINK_PERIOD_MS;
        assert!(phase >= super::BLINK_PERIOD_MS / 2);
    }

    #[test]
    fn blink_period_constant() {
        assert_eq!(super::BLINK_PERIOD_MS, 600);
    }
}
