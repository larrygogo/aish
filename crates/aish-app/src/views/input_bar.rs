//! Terminal 底部固定输入栏 — 图片多选 + 文字输入 + SFTP 批量上传。
//!
//! 文字输入部分使用 aish_ui::TextInput（含 cursor blink / selection / IME / 复制粘贴）。
//! 本 view 负责：图片多选 + 缩略图渲染 + Send 按钮 + UploadBatch 派发。

use std::path::PathBuf;
use std::sync::Arc;

use aish_ui::{theme, TextInput};
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
        // observe state → BatchProgress / BatchDone 改 pending_uploads 时本视图
        // 同步重绘（显示/隐藏进度行 + 切换发送按钮 disabled 状态）。
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();

        let input = cx.new(TextInput::new);
        let weak_self = cx.weak_entity();
        input.update(cx, |i, _cx| {
            i.placeholder("输入文字（Enter 发送）");
            // 把 callback 收到的 text 直接传给 send，避免 send 内再
            // self.input.read(cx) —— Enter 调用链里 TextInput entity 已被
            // listener mut-borrow，read 同一 entity 会触发 double_lease panic。
            i.on_submit(move |text, window, cx| {
                let text = text.to_string();
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, move |this, cx| this.send(text, window, cx));
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

    /// 把 `input.clear()` 延后到下一轮 event loop。
    ///
    /// **必要性**：send() 可能从 TextInput 自己的 on_submit 回调（enter 键）
    /// 触发，此时 TextInput entity 的 listener mut-borrow 还活着（在
    /// handle_key → fire_submit → on_submit callback 调用链里）。如果在这条
    /// 路径上同步调 `self.input.update(...)` → GPUI 检测到同一 entity 的
    /// 嵌套 update → borrow_mut 失败 panic（"shell 底部输入框按回车死程序"）。
    ///
    /// 发送按钮 click 路径本来不会嵌套（InputBarView 的 listener，cx 是
    /// 自己的 Context，与 input 不同 entity），但为了一致性 + 防御未来类似
    /// bug，所有 clear 都走 defer 路径。
    fn defer_clear_input(&self, cx: &mut Context<Self>) {
        let input = self.input.clone();
        cx.spawn(async move |_view, cx| {
            // input 是 Entity（strong ref），update 在 async cx 上返回 ()
            // 而非 Result（entity 一直 alive）；clippy 因此抱怨 let_unit_value
            input.update(cx, |i, cx| i.clear(cx));
        })
        .detach();
    }

    /// `text` 由调用方传入 —— 发送按钮 click 路径在 listener 内 read 自己
    /// input 是安全的；Enter 路径必须从 on_submit callback 参数拿（见 new()
    /// 注释）。本函数内**不再** self.input.read(cx)，避免 double_lease panic。
    fn send(&mut self, text: String, _window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim().to_string();

        let conn = match self.state.read(cx).current_connection() {
            Some(c) => c,
            None => {
                self.defer_clear_input(cx);
                self.images.clear();
                cx.notify();
                return;
            }
        };

        // 发送期间禁止再次按发送：当前 conn 有上传中 → 直接 return，不再发起新
        // 一轮 UploadBatch / SendBytes。Enter 路径 + 按钮 click 路径都走 send，
        // 都在这里被拦下。按钮自身也是 disabled 视觉，这里是逻辑兜底（防快捷键
        // / 自动化双击触发）。
        if self.state.read(cx).pending_uploads.contains_key(&conn) {
            return;
        }

        if self.images.is_empty() {
            if !text.is_empty() {
                if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                    let bytes = format!("{}\r", text).into_bytes();
                    self.bridge.spawn(async move {
                        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                    });
                }
            }
            self.defer_clear_input(cx);
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
        self.defer_clear_input(cx);
        cx.notify();
    }
}

impl Render for InputBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);

        // 当前 conn 的批量上传进度（BatchProgress 更新 / BatchDone 清除）。
        // Some((done, total)) 时显示进度行 + 发送按钮 disabled。
        let upload_progress: Option<(usize, usize)> = self
            .state
            .read(cx)
            .current_connection()
            .and_then(|c| self.state.read(cx).pending_uploads.get(&c).copied());
        let is_uploading = upload_progress.is_some();

        let progress_row = upload_progress.map(|(done, total)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .py(px(4.0))
                .text_size(px(11.0))
                .text_color(t.colors.muted_foreground)
                .child(format!("上传中  {}/{}", done, total))
        });

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
                aish_ui::IconButton::new("input-bar-pick", aish_ui::IconName::Plus)
                    .small()
                    .secondary()
                    .on_click(
                        cx.listener(|this, _ev: &gpui::MouseDownEvent, _window, cx| {
                            this.pick_images(cx);
                        }),
                    ),
            )
            .child(div().flex_1().child(self.input.clone()))
            .child(
                aish_ui::Button::new("input-bar-send")
                    .label("发送")
                    .primary()
                    // 上传期间发送按钮 disabled：视觉灰化 + 不响应 click。
                    // send() 内部还有一层 pending_uploads 检查兜底（防 Enter
                    // / 快捷键绕过按钮 disabled 直接走 send）。
                    .disabled(is_uploading)
                    .on_click(cx.listener(|this, _ev: &gpui::MouseDownEvent, window, cx| {
                        // 按钮 click 路径不嵌套（InputBarView listener，
                        // input 是别的 entity，read 是首次借用 OK）
                        let text = this.input.read(cx).text().to_string();
                        this.send(text, window, cx);
                    })),
            );

        div()
            .flex_col()
            .border_t_1()
            .border_color(t.colors.border)
            .bg(t.colors.background)
            .children(progress_row)
            .children(images_row)
            .child(text_row)
    }
}
