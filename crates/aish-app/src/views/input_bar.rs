//! Terminal 底部固定输入栏 — 图片多选 + 文字输入 + SFTP 批量上传。
//!
//! 文字输入部分使用 aish_ui::TextInput（含 cursor blink / selection / IME / 复制粘贴）。
//! 本 view 负责：图片多选 + 缩略图渲染 + Send 按钮 + UploadBatch 派发。

use std::path::PathBuf;
use std::sync::Arc;

use aish_ui::TextInput;
use gpui::{
    div, img, prelude::*, px, rgb, Context, Entity, ImageSource, ObjectFit, PathPromptOptions,
    SharedString, Window,
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
    images: Vec<PendingImage>,
    input: Entity<TextInput>,
}

impl InputBarView {
    pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(TextInput::new);
        let weak_self = cx.weak_entity();
        input.update(cx, |i, _cx| {
            i.placeholder("输入文字（Enter 发送）");
            i.on_submit(move |_text, window, cx| {
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, |this, cx| this.send(window, cx));
                }
            });
        });
        Self {
            state,
            bridge,
            images: Vec::new(),
            input,
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

    fn send(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();

        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => {
                self.input.update(cx, |i, cx| i.clear(cx));
                self.images.clear();
                cx.notify();
                return;
            }
        };

        if self.images.is_empty() {
            if !text.is_empty() {
                if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                    let bytes = format!("{}\r", text).into_bytes();
                    self.bridge.spawn(async move {
                        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                    });
                }
            }
            self.input.update(cx, |i, cx| i.clear(cx));
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
        self.input.update(cx, |i, cx| i.clear(cx));
        cx.notify();
    }
}

impl Render for InputBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .children(thumbs),
            )
        };

        let text_row = div()
            .flex()
            .flex_row()
            .h(px(40.0))
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
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
            .child(div().flex_1().child(self.input.clone()))
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
                        cx.listener(|this, _, window, cx| {
                            this.send(window, cx);
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
