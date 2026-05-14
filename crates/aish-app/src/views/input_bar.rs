//! Terminal 底部固定输入栏 — 图片多选 + 文字输入 + SFTP 批量上传。
//!
//! 文字输入部分使用 aish_ui::TextInput（含 cursor blink / selection / IME / 复制粘贴）。
//! 本 view 负责：图片多选 + 缩略图渲染 + Send 按钮 + UploadBatch 派发。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use aish_ui::{theme, TextInput};
use gpui::{
    div, img, prelude::*, px, rgb, Context, Entity, ExternalPaths, ImageSource, ObjectFit,
    PathPromptOptions, SharedString, Window,
};

use aish_types::ConnectionId;

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand};

struct PendingImage {
    name: String,
    path: PathBuf,
}

/// 判 path 是否常见图片扩展名（不区分大小写）。drag-drop 场景外部 OS 不限
/// 文件类型，必须 caller 自己过滤防混入 PDF / txt。SVG 走光栅化失败兜底
/// (SFTP 上传 / 远端 cat -> 文本)，所以也算"图片"放过。
fn is_image_path(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "ico" | "svg")
    )
}

pub struct InputBarView {
    /// 本视图绑定的 connection。每个 ConnectionId 一个独立 InputBarView entity，
    /// 草稿（input 文字 + images 缩略图）天然按 conn 隔离。M22 之前 InputBarView
    /// 是全局单例 + 内部读 `state.current_connection()`，导致多个 shell tab 共
    /// 享同一份草稿；现改成 RootView 持 `HashMap<ConnectionId, Entity<Self>>`
    /// 按 conn lazy 创建。conn 销毁（remove_connection）→ RootView observe 内
    /// retain → entity drop → 草稿一并丢弃。
    conn: ConnectionId,
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    images: Vec<PendingImage>,
    input: Entity<TextInput>,
    /// 上一帧的上传中状态（state.pending_uploads has self conn）。render 内
    /// 边沿检测：上一帧 true 且本帧 false → BatchDone 完成 → 清 images + input。
    /// 避免 send 瞬间立即清掉造成"看着已发送但实际还在上传"误导。
    last_uploading: bool,
    /// Braille spinner 当前帧（0..10）。上传中每 80ms +1 mod 10，render 内取
    /// SPINNER_FRAMES[phase] 拼到 Send 按钮 label 给 loading 动画反馈。
    spinner_phase: u8,
    /// 当前窗口是否有外部 drag-in（GPUI cx.has_active_drag）。100ms polling
    /// timer 同步：状态变化时 cx.notify 触发 re-render 显示/隐藏 drop hint。
    /// GPUI active_drag 是 pub(crate) 没观察 API，render 内直接 read cx 不
    /// 触发自身 re-render — 必须 polling + 持久字段。
    has_external_drag: bool,
}

/// CLI 标准 Braille spinner 10 帧（与 npm `ora` / `cli-spinners` 的 `dots`
/// preset 一致）。80ms / frame = 12.5 fps。
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl InputBarView {
    pub fn new(
        conn: ConnectionId,
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        cx: &mut Context<Self>,
    ) -> Self {
        // observe state → BatchProgress / BatchDone 改 pending_uploads 时本视图
        // 同步重绘（显示/隐藏进度行 + 切换发送按钮 disabled 状态）。
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();

        let input = cx.new(TextInput::new);
        let weak_self = cx.weak_entity();
        let weak_for_paste = cx.weak_entity();
        let weak_for_files = cx.weak_entity();
        input.update(cx, |i, _cx| {
            i.placeholder("输入文字（Enter 换行，Ctrl+Enter 发送）")
                .multiline(true)
                .max_lines(6)
                // 关键：borderless 让 TextInput 不画自己的 bg / border，
                // 视觉框由外层 InputBar card 提供（统一 chat-card 美感，
                // 避免 input border + InputBar border 双层框）。
                .borderless(true);
            // 把 callback 收到的 text 直接传给 send，避免 send 内再
            // self.input.read(cx) —— Enter 调用链里 TextInput entity 已被
            // listener mut-borrow，read 同一 entity 会触发 double_lease panic。
            //
            // M19: multiline=true 下 Enter 插 \\n、Ctrl+Enter 触发 fire_submit
            // 路由到这里。on_submit 行为不变，caller 透明。
            i.on_submit(move |text, window, cx| {
                let text = text.to_string();
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, move |this, cx| this.send(text, window, cx));
                }
            });
            // Ctrl+V 剪贴板含图片时：写临时 PNG + 入 self.images，跳过 text paste。
            // 文本 / 文件路径仍走 TextInput 默认 text paste（这里不消费）。
            i.on_paste_image(move |w, h, rgba, _window, cx| {
                if let Some(this) = weak_for_paste.upgrade() {
                    this.update(cx, |this, cx| this.add_image_from_rgba(w, h, rgba, cx))
                } else {
                    false
                }
            });
            // Ctrl+V 剪贴板含 file 路径（用户从文件管理器 Ctrl+C 图片场景）：
            // 走 add_image_paths 同 + 按钮 / drag-drop 队列。**总是 return true**
            // 即便 filter 全过滤掉也不让 text paste 把路径字符串当文字插进 input
            // （用户期望是入队图片，不是 paste 文件路径文本）。
            i.on_paste_files(move |paths, _window, cx| {
                if let Some(this) = weak_for_files.upgrade() {
                    this.update(cx, |this, cx| this.add_image_paths(paths, cx));
                }
                true
            });
        });
        // observe input entity：input mouse_down 获焦 / typed 时 cx.notify
        // input → 链路传到 InputBar re-render，读最新 is_focused 决定 card
        // border 颜色（textarea-like focus 反馈）。
        cx.observe(&input, |_this, _input, cx| cx.notify()).detach();

        // Braille spinner timer：80ms / frame 推进 spinner_phase 0..10。
        // 仅 is_uploading 时 cx.notify 触发 re-render（空闲时不 notify
        // 避免空转重渲），phase 仍递增让恢复上传时从下一帧无缝继续。
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            let cont = this
                .update(cx, |this, cx| {
                    this.spinner_phase = (this.spinner_phase + 1) % SPINNER_FRAMES.len() as u8;
                    // 仅看本 conn（self.conn）是否在上传；M22 之前读
                    // current_connection 会让其他 tab 的上传也驱动本 spinner。
                    let uploading = this.state.read(cx).pending_uploads.contains_key(&this.conn);
                    if uploading {
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if !cont {
                break;
            }
        })
        .detach();
        // 100ms polling timer 同步外部 drag-in 状态：cx.has_active_drag()
        // 变化时 cx.notify 让 InputBar re-render 显示 / 隐藏 drop hint。
        // 状态不变 → 不 notify（避免空转重渲）。
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let cont = this
                .update(cx, |this, cx| {
                    let now = cx.has_active_drag();
                    if now != this.has_external_drag {
                        this.has_external_drag = now;
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if !cont {
                break;
            }
        })
        .detach();

        Self {
            conn,
            state,
            bridge,
            images: Vec::new(),
            input,
            last_uploading: false,
            spinner_phase: 0,
            has_external_drag: false,
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
                    this.add_image_paths(paths, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// 共用 image 入队路径：+ 按钮 prompt / drag-drop / 未来其他入口都走它。
    /// - 上传中拒绝（防止 batch 进行中混入新图）
    /// - 过滤非图片扩展名（drop 场景常见混入 PDF / 文本）
    /// - 给每张图算 file_name 作 PendingImage.name
    fn add_image_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.is_uploading(cx) {
            return;
        }
        let mut added = 0usize;
        for path in paths {
            if !is_image_path(&path) {
                tracing::debug!(path = ?path, "input_bar: 跳过非图片文件");
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image")
                .to_string();
            self.images.push(PendingImage { name, path });
            added += 1;
        }
        if added > 0 {
            cx.notify();
        }
    }

    /// Ctrl+V 路径专属：剪贴板拿来的 RGBA 字节 → 编码 PNG → 写临时文件 →
    /// 入队 self.images。返回 true 表示已消费图片（TextInput 跳过 text paste）。
    /// 写文件 / 编码 / 上传中失败都返回 false 让 caller 走 text paste 兜底。
    fn add_image_from_rgba(
        &mut self,
        w: u32,
        h: u32,
        rgba: Vec<u8>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_uploading(cx) {
            return false;
        }
        let png = match crate::terminal::image::encode_rgba_to_png(w, h, &rgba) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("input_bar: 剪贴板图片 PNG 编码失败: {}", e);
                return false;
            }
        };
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = std::env::temp_dir().join(format!("aish-paste-{}.png", ts));
        if let Err(e) = std::fs::write(&path, &png) {
            tracing::warn!("input_bar: 写临时图片文件失败 {}: {}", path.display(), e);
            return false;
        }
        let name = format!("clipboard-{}.png", ts);
        self.images.push(PendingImage { name, path });
        cx.notify();
        true
    }

    /// 本 conn（self.conn）是否有进行中的批量上传。drop / paste image 时拒绝
    /// 入队避免 batch 进行中混入新图。
    fn is_uploading(&self, cx: &Context<Self>) -> bool {
        self.state.read(cx).pending_uploads.contains_key(&self.conn)
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

        // M22：每个 InputBarView 绑死 self.conn，不再从 current_connection 派生
        // —— per-conn entity 必有 conn，原 None 兜底分支删除。RootView 在 Default
        // tab（current_connection == None）下根本不挂 InputBar 元素。
        let conn = self.conn;

        // 发送期间禁止再次按发送：当前 conn 有上传中 → 直接 return，不再发起新
        // 一轮 UploadBatch / SendBytes。Enter 路径 + 按钮 click 路径都走 send，
        // 都在这里被拦下。按钮自身也是 disabled 视觉，这里是逻辑兜底（防快捷键
        // / 自动化双击触发）。
        if self.state.read(cx).pending_uploads.contains_key(&conn) {
            return;
        }

        // 无图片：纯文字 → 立即发送 + 清 input（无上传过程不存在"半途撤销"）
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
            // 所有图片读取失败 → 退化到纯文字路径
            if !text.is_empty() {
                if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                    let bytes = format!("{}\r", text).into_bytes();
                    self.bridge.spawn(async move {
                        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                    });
                }
            }
            self.defer_clear_input(cx);
            self.images.clear();
            cx.notify();
        } else if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
            // 有图片：派发 UploadBatch。**不**立即清 images + input —— 等
            // BatchDone（render 内 last_uploading 边沿检测）后再清，保证：
            // - 缩略图依次消失（render 时按 done 数 skip 前 N 张）
            // - 文字内容直到完整上传完成才清空，给用户"上传中"的可见反馈
            // - 任一图片失败仍保留 input bar 状态用于 retry（M19 stage 2）
            //
            // 立即设 pending_uploads(0, total) 标记进入上传状态 —— actor 端
            // 第一张 BatchProgress 在第一张完成后才到（可能数秒），不立即
            // 设的话 send 按钮 / 缩略图 / loading label 在第一张完成前都看
            // 不出"在上传"。
            let total = image_data.len();
            self.bridge.spawn(async move {
                let _ = sender
                    .send(SessionCommand::UploadBatch {
                        images: image_data,
                        text,
                    })
                    .await;
            });
            self.state.update(cx, |s, cx| {
                s.pending_uploads.insert(conn, (0, total));
                cx.notify();
            });
        }
    }
}

impl Render for InputBarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 先把 theme 字段 copy 出来释放 cx immut borrow（下面 BatchDone
        // 边沿检测要 mut borrow cx 调 defer_clear_input）
        let (colors, radius) = {
            let t = theme(cx);
            (t.colors, t.radius)
        };
        // input focused 时 card border 改 ring 色（textarea-like 焦点反馈）。
        // borderless 模式下 input 自身没视觉 focus indicator，靠 card 给。
        let input_focused = self.input.read(cx).is_focused(window);
        let card_border_color = if input_focused {
            colors.ring
        } else {
            colors.border
        };

        // 本 conn（self.conn）的批量上传进度（BatchProgress 更新 / BatchDone
        // 清除）。Some((done, total)) 时显示进度行 + 发送按钮 disabled。
        let upload_progress: Option<(usize, usize)> =
            self.state.read(cx).pending_uploads.get(&self.conn).copied();
        let is_uploading = upload_progress.is_some();

        // BatchDone / BatchAborted 边沿检测：上一帧 uploading=true & 本帧 false → 结束。
        // - last_aborted_batch 命中 → batch 中途失败（含 30s 超时）：
        //   drain 前 succeeded 张（已成功 path 已 append PTY），保留剩余 images
        //   + 用户文字供 retry。消费 last_aborted_batch entry。
        // - 否则正常完成：清 images + defer clear input（render 内不能直接 mut input）。
        if self.last_uploading && !is_uploading {
            let aborted = self
                .state
                .read(cx)
                .last_aborted_batch
                .get(&self.conn)
                .copied();
            if let Some((succeeded, _total)) = aborted {
                // 失败 retry 路径：移除前 N 张已传成功的缩略图，保留剩余 + text
                let drain_n = succeeded.min(self.images.len());
                if drain_n > 0 {
                    self.images.drain(0..drain_n);
                }
                let conn = self.conn;
                self.state.update(cx, |s, _| {
                    s.last_aborted_batch.remove(&conn);
                });
            } else {
                self.images.clear();
                self.defer_clear_input(cx);
            }
        }
        self.last_uploading = is_uploading;

        // 上传中锁定 TextInput：键盘 / IME 输入跳过，但 mouse 仍可选择复制。
        // 同步设到 input entity（每帧 sync 因为 is_uploading 可能变化）。
        self.input.update(cx, |i, _cx| {
            i.disabled(is_uploading);
        });

        let progress_row = upload_progress.map(|(done, total)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .py(px(4.0))
                .text_size(px(11.0))
                .text_color(colors.muted_foreground)
                .child(format!("上传中  {}/{}", done, total))
        });

        // 上传中已完成的图片数（视觉跳过前 done 张缩略图，实现"发完一个去掉一个"）
        let done_count = upload_progress.map(|(d, _)| d).unwrap_or(0);

        let images_row = if self.images.len() <= done_count {
            None
        } else {
            let thumbs: Vec<_> = self
                .images
                .iter()
                .enumerate()
                .skip(done_count)
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
                                // × 删除按钮：上传中不渲染（锁定缩略图）
                                .when(!is_uploading, |d| {
                                    d.child(
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
                                    )
                                }),
                        )
                        .child(
                            div()
                                .w(px(60.0))
                                .overflow_hidden()
                                .text_size(px(9.0))
                                .text_color(colors.muted_foreground)
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
            // items_end 让按钮始终贴 row 底部（chat-app 标准，与 input 末行
            // baseline 对齐）。多行时按钮跟 input 最后一行齐底，单行时跟唯一
            // 一行齐底（input min_h=28 跟 Send 按钮等高，+ 24 略矮贴底）。
            .items_end()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(8.0))
            .child(
                aish_ui::IconButton::new("input-bar-pick", aish_ui::IconName::Plus)
                    .small()
                    .secondary()
                    // 上传中锁定：禁止再添加图片（与 Send 按钮同步 disabled）
                    .disabled(is_uploading)
                    .on_click(
                        cx.listener(|this, _ev: &gpui::MouseDownEvent, _window, cx| {
                            this.pick_images(cx);
                        }),
                    ),
            )
            .child(div().flex_1().child(self.input.clone()))
            .child(
                aish_ui::Button::new("input-bar-send")
                    // 上传中：Braille spinner + "上传中 X/Y"；空闲态 "发送"
                    .label(if let Some((d, n)) = upload_progress {
                        let spinner = SPINNER_FRAMES[self.spinner_phase as usize];
                        SharedString::from(format!("{} 上传中 {}/{}", spinner, d, n))
                    } else {
                        SharedString::from("发送")
                    })
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

        // InputBar 整体改 chat-card 风：rounded + border + bg(card)。
        // input 自身 borderless，视觉框由 outer card 提供；缩略图 / 进度 / +
        // / input / Send 全在一个 card 内，统一组件感。focused 时 border
        // 改 ring 色作为 textarea-like focus 反馈。
        //
        // drag-over 双重反馈：
        // - card 顶部 drop_hint：外部 drag 进窗口（self.has_external_drag）
        //   立即显示一行"拖到这里上传图片"，让用户知道 input bar 是 drop
        //   target。100ms polling timer 把 cx.has_active_drag 同步到字段。
        // - drag_over::<ExternalPaths> 鼠标真的 hover 在 card 上时 border
        //   切 primary + bg 加深一档（visible affordance）。GPUI .drag_over
        //   仅在 hover 触发，不依赖 entity re-render。
        let primary = colors.primary;
        let drop_target_bg = colors.accent;
        let drop_hint = if self.has_external_drag {
            let muted_fg = colors.muted_foreground;
            Some(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .py(px(6.0))
                    .text_size(px(11.0))
                    .text_color(muted_fg)
                    .child("⬇")
                    .child("拖到这里加入图片"),
            )
        } else {
            None
        };
        div()
            .flex_col()
            .mx(px(8.0))
            .mb(px(8.0))
            .mt(px(4.0))
            .rounded(radius.md)
            .border_1()
            .border_color(card_border_color)
            .bg(colors.card)
            // closure 接 StyleRefinement 返回 StyleRefinement，链式 builder。
            // border 加粗到 2px 让 hover 反馈更显眼（占用 1px 内边距可接受）。
            .drag_over::<ExternalPaths>(move |s, _paths, _w, _cx| {
                s.border_color(primary).bg(drop_target_bg)
            })
            // 外部文件 drag-drop：OS 文件管理器 / 浏览器拖图片到 card 区域。
            // GPUI 把 FileDropEvent 转 internal drag，Submit 时 .on_drop::<T>
            // 命中（T=ExternalPaths）。过滤图片扩展名后入 self.images，与
            // + 按钮 prompt 同一队列。上传中 is_uploading 拒绝（add_image_paths
            // 内兜底）。
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this.add_image_paths(paths.paths().to_vec(), cx);
            }))
            .children(drop_hint)
            .children(progress_row)
            .children(images_row)
            .child(text_row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_image_path_accepts_common_raster() {
        for ext in [
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "ico", "svg",
        ] {
            let p = PathBuf::from(format!("foo.{ext}"));
            assert!(is_image_path(&p), "{} should be image", ext);
        }
    }

    #[test]
    fn is_image_path_case_insensitive() {
        assert!(is_image_path(&PathBuf::from("photo.JPG")));
        assert!(is_image_path(&PathBuf::from("Screenshot.PNG")));
        assert!(is_image_path(&PathBuf::from("icon.IcO")));
    }

    #[test]
    fn is_image_path_rejects_non_image() {
        assert!(!is_image_path(&PathBuf::from("doc.pdf")));
        assert!(!is_image_path(&PathBuf::from("readme.txt")));
        assert!(!is_image_path(&PathBuf::from("archive.zip")));
        assert!(!is_image_path(&PathBuf::from("video.mp4")));
    }

    #[test]
    fn is_image_path_rejects_no_extension() {
        assert!(!is_image_path(&PathBuf::from("Makefile")));
        assert!(!is_image_path(&PathBuf::from("noext")));
    }

    #[test]
    fn is_image_path_accepts_full_path() {
        assert!(is_image_path(&PathBuf::from("/tmp/foo/bar.png")));
        assert!(is_image_path(&PathBuf::from("C:\\Users\\me\\pic.jpg")));
    }
}
