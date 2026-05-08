//! Terminal 底部固定输入栏 — 图片多选 + 文字输入 + SFTP 批量上传。

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, rgb, App, Context, Entity, FocusHandle, Focusable, ImageSource,
    KeyDownEvent, ObjectFit, PathPromptOptions, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand};

struct PendingImage {
    name: String,
    path: PathBuf,
}

pub struct InputBarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    focus_handle: FocusHandle,
    images: Vec<PendingImage>,
    text: String,
    cursor: usize, // byte offset into text
}

impl InputBarView {
    pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            bridge,
            focus_handle: cx.focus_handle(),
            images: Vec::new(),
            text: String::new(),
            cursor: 0,
        }
    }

    fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    fn cursor_right(&mut self) {
        if self.cursor < self.text.len() {
            let next = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            self.cursor = next;
        }
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
        }
    }

    fn delete_forward(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.backspace();
                cx.notify();
            }
            "delete" => {
                self.delete_forward();
                cx.notify();
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
                cx.notify();
            }
            "end" => {
                self.cursor = self.text.len();
                cx.notify();
            }
            "enter" if !event.keystroke.modifiers.shift => {
                self.send(cx);
            }
            _ => {}
        }
    }

    fn pick_images(&mut self, cx: &mut Context<Self>) {
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(SharedString::from("选择图片")),
        };
        let receiver = cx.prompt_for_paths(options);
        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                this.update(cx, |this, cx| {
                    for path in paths {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image")
                            .to_string();
                        this.images.push(PendingImage { name, path });
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn remove_image(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.images.len() {
            self.images.remove(index);
            cx.notify();
        }
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => {
                self.images.clear();
                self.text.clear();
                self.cursor = 0;
                cx.notify();
                return;
            }
        };

        let text = self.text.trim().to_string();

        if self.images.is_empty() {
            if !text.is_empty() {
                if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                    let bytes = format!("{}\r", text).into_bytes();
                    self.bridge.spawn(async move {
                        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                    });
                }
            }
            self.text.clear();
            self.cursor = 0;
            cx.notify();
            return;
        }

        let mut image_data: Vec<(Vec<u8>, String)> = Vec::new();
        for img_item in &self.images {
            let ext = img_item
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png")
                .to_lowercase();
            match std::fs::read(&img_item.path) {
                Ok(bytes) => image_data.push((bytes, ext)),
                Err(e) => {
                    tracing::warn!("input_bar: 读取图片失败 {:?}: {}", img_item.path, e);
                }
            }
        }

        if image_data.is_empty() {
            if !text.is_empty() {
                if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                    let bytes = format!("{}\r", text).into_bytes();
                    self.bridge.spawn(async move {
                        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                    });
                }
            }
        } else if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
            self.bridge.spawn(async move {
                let _ = sender
                    .send(SessionCommand::UploadBatch {
                        images: image_data,
                        text,
                    })
                    .await;
            });
        }

        self.images.clear();
        self.text.clear();
        self.cursor = 0;
        cx.notify();
    }
}

impl Focusable for InputBarView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct InputBarTextHandler {
    view: gpui::WeakEntity<InputBarView>,
}

impl gpui::InputHandler for InputBarTextHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<gpui::UTF16Selection> {
        Some(gpui::UTF16Selection {
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
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

impl Render for InputBarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_handle.is_focused(window) {
            window.handle_input(
                &self.focus_handle,
                InputBarTextHandler {
                    view: cx.weak_entity(),
                },
                cx,
            );
        }

        let focused = self.focus_handle.is_focused(window);
        let border_color = if focused {
            rgb(0x6c91c2)
        } else {
            rgb(0x2d2d3f)
        };

        let images_row = if self.images.is_empty() {
            None
        } else {
            let thumbs: Vec<_> = self
                .images
                .iter()
                .enumerate()
                .map(|(i, img_item)| {
                    let path = img_item.path.clone();
                    let name = img_item.name.clone();
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(2.0))
                        .w(px(64.0))
                        .child(
                            div()
                                .relative()
                                .w(px(56.0))
                                .h(px(48.0))
                                .overflow_hidden()
                                .rounded(px(4.0))
                                .child(
                                    img(ImageSource::from(path))
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Cover),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(1.0))
                                        .right(px(1.0))
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(rgb(0x00000099))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(move |this, _, _window, cx| {
                                                this.remove_image(i, cx);
                                            }),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(9.0))
                                                .text_color(rgb(0xffffff))
                                                .child("×"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .w(px(60.0))
                                .overflow_hidden()
                                .text_size(px(9.0))
                                .text_color(rgb(0x888899))
                                .child(name),
                        )
                })
                .collect();

            Some(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(8.0))
                    .p(px(8.0))
                    .child(
                        div().flex().flex_row().gap(px(8.0)).children(thumbs).child(
                            div()
                                .w(px(56.0))
                                .h(px(48.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.0))
                                .border_1()
                                .border_color(rgb(0x444455))
                                .cursor_pointer()
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.pick_images(cx);
                                    }),
                                )
                                .child(
                                    div()
                                        .text_size(px(18.0))
                                        .text_color(rgb(0x888899))
                                        .child("+"),
                                ),
                        ),
                    ),
            )
        };

        let cursor_left = self.text[..self.cursor].to_string();
        let cursor_right = self.text[self.cursor..].to_string();
        let placeholder_visible = self.text.is_empty();

        let text_display = div()
            .flex()
            .flex_row()
            .flex_1()
            .items_center()
            .h(px(28.0))
            .px(px(8.0))
            .rounded(px(4.0))
            .bg(rgb(0x16161e))
            .border_1()
            .border_color(border_color)
            .cursor_text()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                }),
            )
            .child(if placeholder_visible {
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x555566))
                    .child("输入文字（Enter 发送，Shift+Enter 换行）")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_row()
                    .text_size(px(12.0))
                    .text_color(rgb(0xc0caf5))
                    .child(div().child(cursor_left))
                    .child(div().w(px(1.0)).h(px(14.0)).bg(rgb(0x6c91c2)).self_center())
                    .child(div().child(cursor_right))
                    .into_any_element()
            });

        let text_row = div()
            .flex()
            .flex_row()
            .h(px(40.0))
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .bg(rgb(0x2d2d3f))
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.pick_images(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(0x888899))
                            .child("+"),
                    ),
            )
            .child(text_display)
            .child(
                div()
                    .px(px(10.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .bg(rgb(0x3d59a1))
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.send(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0xc0caf5))
                            .child("发送"),
                    ),
            );

        div()
            .flex_col()
            .border_t_1()
            .border_color(rgb(0x2d2d3f))
            .bg(rgb(0x1a1b26))
            .children(images_row)
            .child(text_row)
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

    fn apply_delete_forward(text: &mut String, cursor: &mut usize) {
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
            let next = text[*cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| *cursor + i)
                .unwrap_or(text.len());
            *cursor = next;
        }
    }

    #[test]
    fn insert_str_advances_cursor() {
        let (mut text, mut cursor) = (String::new(), 0usize);
        apply_insert(&mut text, &mut cursor, "hello");
        assert_eq!(text, "hello");
        assert_eq!(cursor, 5);
    }

    #[test]
    fn backspace_removes_last_ascii_char() {
        let (mut text, mut cursor) = (String::new(), 0usize);
        apply_insert(&mut text, &mut cursor, "hi");
        apply_backspace(&mut text, &mut cursor);
        assert_eq!(text, "h");
        assert_eq!(cursor, 1);
    }

    #[test]
    fn cursor_left_right_navigates() {
        let (mut text, mut cursor) = (String::new(), 0usize);
        apply_insert(&mut text, &mut cursor, "ab");
        apply_left(&text, &mut cursor);
        assert_eq!(cursor, 1);
        apply_right(&text, &mut cursor);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn insert_at_middle_position() {
        let (mut text, mut cursor) = (String::new(), 0usize);
        apply_insert(&mut text, &mut cursor, "ac");
        apply_left(&text, &mut cursor);
        apply_insert(&mut text, &mut cursor, "b");
        assert_eq!(text, "abc");
        assert_eq!(cursor, 2);
    }

    #[test]
    fn delete_forward_removes_char_after_cursor() {
        let (mut text, mut cursor) = (String::new(), 0usize);
        apply_insert(&mut text, &mut cursor, "abc");
        cursor = 1;
        apply_delete_forward(&mut text, &mut cursor);
        assert_eq!(text, "ac");
        assert_eq!(cursor, 1);
    }
}
