# M8 — 图片粘贴 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ctrl+Shift+V 检测剪贴板内容：有图片则通过 SFTP 上传到远端 `/tmp/aish-clip-<ts>.png` 并把路径 echo 到 PTY；有文字则走现有文字粘贴逻辑。

**Architecture:** TerminalView 用 `arboard` 读剪贴板检测图片 → 发 `SessionCommand::UploadImage` 给 ssh_actor → actor spawn 子 task 调用 `SshClient::sftp_upload` → 成功后 `SshEvent::ImageUploaded` 推回 GPUI → app.rs 把路径 send_bytes 到 PTY。

**Tech Stack:** arboard 3.x（Windows 剪贴板图片读取）、image 0.25（RGBA → PNG 编码，仅 png feature）、russh-sftp 2.1（SFTP 客户端，在 aish-ssh crate 内封装）。

---

## 文件结构

| 操作 | 路径 | 说明 |
|---|---|---|
| 修改 | `Cargo.toml`（workspace） | 加 `russh-sftp`, `arboard`, `image` workspace dep |
| 修改 | `crates/aish-ssh/Cargo.toml` | 加 `russh-sftp` |
| 修改 | `crates/aish-ssh/src/error.rs` | 加 `SshError::Sftp(String)` variant |
| 修改 | `crates/aish-ssh/src/client.rs` | 加 `SshClient::sftp_upload` 方法 |
| 修改 | `crates/aish-app/Cargo.toml` | 加 `arboard`, `image` |
| 修改 | `crates/aish-app/src/state.rs` | 加 `SessionCommand::UploadImage`, `SshEvent::ImageUploaded`, `SshEvent::ImageUploadFailed` |
| 新建 | `crates/aish-app/src/terminal/image.rs` | `encode_rgba_to_png` 辅助函数 |
| 修改 | `crates/aish-app/src/terminal/mod.rs` | 加 `pub mod image;` |
| 修改 | `crates/aish-app/src/views/terminal_view.rs` | 修改 `paste()` 在文字粘贴前检测图片 |
| 修改 | `crates/aish-app/src/ssh_actor.rs` | 加 `UploadImage` match arm |
| 修改 | `crates/aish-app/src/app.rs` | 加 `ImageUploaded` / `ImageUploadFailed` 处理 |
| 修改 | `docs/superpowers/INDEX.md` | 更新里程碑状态 |

---

### Task 1: 添加 Workspace 依赖

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 在 `[workspace.dependencies]` 末尾追加三行**

打开 `Cargo.toml`，在 `alacritty_terminal = "0.26"` 之后加：

```toml
russh-sftp = "2.1"
arboard = "3.4"
image = { version = "0.25", default-features = false, features = ["png"] }
```

- [ ] **Step 2: 验证 workspace 能解析依赖**

```bash
cargo +nightly clippy --workspace --all-targets -- -D warnings 2>&1 | head -20
```

预期：编译继续，无 "no matching package" 错误（新 dep 还没被 crate 引用，不影响编译）。

- [ ] **Step 3: 提交**

```bash
git add Cargo.toml
git commit -m "build: workspace 依赖加 russh-sftp / arboard / image（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 2: aish-ssh crate 依赖 + SshError::Sftp

**Files:**
- Modify: `crates/aish-ssh/Cargo.toml`
- Modify: `crates/aish-ssh/src/error.rs`

- [ ] **Step 1: 写失败测试**

在 `crates/aish-ssh/src/error.rs` 的 `tests` 模块末尾加：

```rust
#[test]
fn sftp_error_kind_is_io() {
    let err = SshError::Sftp("timeout".into());
    assert_eq!(err.kind(), SshErrorKind::Io);
}

#[test]
fn sftp_error_display_contains_message() {
    let err = SshError::Sftp("permission denied".into());
    assert!(format!("{}", err).contains("sftp error"));
    assert!(format!("{}", err).contains("permission denied"));
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p aish-ssh sftp_error 2>&1 | tail -20
```

预期：编译失败 `SshError::Sftp` not found。

- [ ] **Step 3: 加 SshError::Sftp variant**

在 `crates/aish-ssh/src/error.rs` 的 `SshError` enum 里，在 `Io` variant 之后加：

```rust
    #[error("sftp error: {0}")]
    Sftp(String),
```

在 `kind()` 方法的 match 里加：

```rust
            Self::Sftp(_) => SshErrorKind::Io,
```

最终 `kind()` 方法完整代码：

```rust
pub fn kind(&self) -> SshErrorKind {
    match self {
        Self::Connect(_) => SshErrorKind::ConnectFailed,
        Self::Auth(_) | Self::KeyFileRead { .. } | Self::KeyFileParse { .. } => {
            SshErrorKind::AuthFailed
        }
        Self::Io(_) => SshErrorKind::Io,
        Self::Sftp(_) => SshErrorKind::Io,
        Self::Protocol(_) => SshErrorKind::Protocol,
    }
}
```

- [ ] **Step 4: 在 aish-ssh/Cargo.toml 加 russh-sftp**

```toml
russh-sftp = { workspace = true }
```

- [ ] **Step 5: 运行测试验证通过**

```bash
cargo test -p aish-ssh sftp_error 2>&1 | tail -10
```

预期：`test sftp_error_kind_is_io ... ok`，`test sftp_error_display_contains_message ... ok`。

- [ ] **Step 6: 提交**

```bash
git add crates/aish-ssh/Cargo.toml crates/aish-ssh/src/error.rs
git commit -m "feat(ssh): SshError::Sftp variant + russh-sftp 依赖（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 3: SshClient::sftp_upload 方法

**Files:**
- Modify: `crates/aish-ssh/src/client.rs`

- [ ] **Step 1: 在 client.rs 顶部导入**

在 `use crate::error::SshError;` 之后加：

```rust
use russh_sftp::client::SftpSession;
```

- [ ] **Step 2: 在 SshClient impl 块末尾加 sftp_upload 方法**

在 `pub async fn close(&self)` 之后加：

```rust
/// 通过 SFTP 把 `data`（PNG bytes）写到远端 `remote_path`。
/// 内部 fork 一条独立 SFTP channel，与主 PTY channel 不冲突。
pub async fn sftp_upload(&self, remote_path: &str, data: &[u8]) -> Result<(), SshError> {
    let channel = self
        .handle
        .channel_open_session()
        .await
        .map_err(SshError::Protocol)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(SshError::Protocol)?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| SshError::Sftp(e.to_string()))?;
    let mut file = sftp
        .create(remote_path)
        .await
        .map_err(|e| SshError::Sftp(e.to_string()))?;
    use tokio::io::AsyncWriteExt;
    file.write_all(data)
        .await
        .map_err(|e| SshError::Sftp(e.to_string()))?;
    file.flush()
        .await
        .map_err(|e| SshError::Sftp(e.to_string()))?;
    drop(file);
    sftp.close()
        .await
        .map_err(|e| SshError::Sftp(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 3: 运行 clippy 验证编译**

```bash
cargo +nightly clippy -p aish-ssh -- -D warnings 2>&1 | tail -20
```

预期：0 warnings，0 errors。

- [ ] **Step 4: 提交**

```bash
git add crates/aish-ssh/src/client.rs
git commit -m "feat(ssh): SshClient::sftp_upload（SFTP channel fork，M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 4: state.rs — 新增 enum variants

**Files:**
- Modify: `crates/aish-app/src/state.rs`

- [ ] **Step 1: 写失败测试**

在 `crates/aish-app/src/state.rs` 的 `tests` 模块末尾加：

```rust
#[test]
fn upload_image_command_constructible() {
    let cmd = SessionCommand::UploadImage { data: vec![0u8, 1, 2, 3] };
    match cmd {
        SessionCommand::UploadImage { data } => assert_eq!(data.len(), 4),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn image_uploaded_event_carries_path() {
    use aish_types::ConnectionId;
    let conn = ConnectionId::new();
    let event = SshEvent::ImageUploaded {
        conn,
        path: "/tmp/aish-clip-123.png".into(),
    };
    match event {
        SshEvent::ImageUploaded { conn: c, path } => {
            assert_eq!(c, conn);
            assert_eq!(path, "/tmp/aish-clip-123.png");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn image_upload_failed_event_carries_msg() {
    use aish_types::ConnectionId;
    let conn = ConnectionId::new();
    let event = SshEvent::ImageUploadFailed {
        conn,
        msg: "permission denied".into(),
    };
    match event {
        SshEvent::ImageUploadFailed { conn: c, msg } => {
            assert_eq!(c, conn);
            assert!(msg.contains("permission"));
        }
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p aish-app upload_image 2>&1 | tail -10
cargo test -p aish-app image_upload 2>&1 | tail -10
```

预期：编译失败，variants not found。

- [ ] **Step 3: 在 SessionCommand enum 末尾加 UploadImage**

找到 `crates/aish-app/src/state.rs` 中 `SessionCommand` 的 `AttachTmux` variant 之后，加：

```rust
    /// 上传本地剪贴板图片（PNG bytes）到远端 /tmp。
    UploadImage { data: Vec<u8> },
```

- [ ] **Step 4: 在 SshEvent enum 末尾加 ImageUploaded / ImageUploadFailed**

找到 `TmuxAttached` variant 之后，加：

```rust
    /// SFTP 上传成功，path 是远端绝对路径（如 /tmp/aish-clip-123456.png）。
    ImageUploaded {
        conn: ConnectionId,
        path: String,
    },
    /// SFTP 上传失败，msg 是错误描述。
    ImageUploadFailed {
        conn: ConnectionId,
        msg: String,
    },
```

- [ ] **Step 5: 运行测试验证通过**

```bash
cargo test -p aish-app upload_image_command_constructible image_uploaded_event_carries_path image_upload_failed_event_carries_msg 2>&1 | tail -10
```

预期：3 个测试全部 ok。

- [ ] **Step 6: 提交**

```bash
git add crates/aish-app/src/state.rs
git commit -m "feat(state): UploadImage 命令 + ImageUploaded/Failed 事件（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 5: terminal/image.rs — encode_rgba_to_png

**Files:**
- Create: `crates/aish-app/src/terminal/image.rs`
- Modify: `crates/aish-app/src/terminal/mod.rs`
- Modify: `crates/aish-app/Cargo.toml`

- [ ] **Step 1: 在 aish-app/Cargo.toml 加 arboard + image 依赖**

在 `[dependencies]` 末尾加：

```toml
arboard = { workspace = true }
image = { workspace = true }
```

- [ ] **Step 2: 新建 terminal/image.rs，先写测试**

创建文件 `crates/aish-app/src/terminal/image.rs`，内容：

```rust
//! 剪贴板图片编码辅助。

use std::io::Cursor;

use image::{ImageFormat, RgbaImage};

/// 把 RGBA 像素 buffer（arboard::ImageData.bytes）编码为 PNG bytes。
///
/// `rgba` 长度必须恰好为 `w * h * 4`，否则返回 Err。
pub fn encode_rgba_to_png(w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<Vec<u8>> {
    let img = RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or_else(|| {
        anyhow::anyhow!(
            "RGBA buffer size mismatch: expected {} bytes, got {}",
            w * h * 4,
            rgba.len()
        )
    })?;
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_rgba_to_png_produces_valid_png_header() {
        // 2×2 RGBA 图片（红、绿、蓝、黄各一像素）
        let rgba = vec![
            255u8, 0, 0, 255, // red
            0, 255, 0, 255,   // green
            0, 0, 255, 255,   // blue
            255, 255, 0, 255, // yellow
        ];
        let png = encode_rgba_to_png(2, 2, &rgba).expect("encode should succeed");
        // PNG 文件以固定签名开头：\x89PNG\r\n\x1a\n
        assert_eq!(&png[0..4], b"\x89PNG", "output must start with PNG signature");
        assert!(png.len() > 30, "PNG must have at least header + IHDR");
    }

    #[test]
    fn encode_rgba_to_png_size_mismatch_returns_error() {
        // 只有 10 字节，但声称是 4×4 图
        let bad = vec![0u8; 10];
        let result = encode_rgba_to_png(4, 4, &bad);
        assert!(result.is_err(), "must fail on buffer size mismatch");
        assert!(
            result.unwrap_err().to_string().contains("mismatch"),
            "error must mention mismatch"
        );
    }

    #[test]
    fn encode_rgba_to_png_1x1_red_pixel() {
        let rgba = vec![255u8, 0, 0, 255]; // 1×1 红色像素
        let png = encode_rgba_to_png(1, 1, &rgba).unwrap();
        // 能用 image crate 解码回来，宽高=1
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!(decoded.width(), 1);
        assert_eq!(decoded.height(), 1);
    }
}
```

- [ ] **Step 3: 在 terminal/mod.rs 加 `pub mod image;`**

在 `pub mod selection;` 之后加：

```rust
pub mod image;
```

- [ ] **Step 4: 运行测试验证通过**

```bash
cargo test -p aish-app terminal::image 2>&1 | tail -15
```

预期：3 个测试全部 ok。

- [ ] **Step 5: 提交**

```bash
git add crates/aish-app/Cargo.toml crates/aish-app/src/terminal/image.rs crates/aish-app/src/terminal/mod.rs
git commit -m "feat(terminal): encode_rgba_to_png 辅助函数（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 6: terminal_view — paste() 图片检测

**Files:**
- Modify: `crates/aish-app/src/views/terminal_view.rs`

当前 `paste()` 签名在 `terminal_view.rs` 约第 246 行：
```rust
fn paste(&mut self, conn: aish_types::ConnectionId, cx: &mut Context<Self>) {
```

- [ ] **Step 1: 在 paste() 方法第一行（进入后，existing text paste logic 前）插入图片检测代码**

在 `fn paste` 方法体开头（`use alacritty_terminal::term::TermMode;` 之前），插入：

```rust
    // 先检测剪贴板是否有图片（arboard 直接读 Windows 剪贴板）
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(img) = cb.get_image() {
            match crate::terminal::image::encode_rgba_to_png(
                img.width as u32,
                img.height as u32,
                &img.bytes,
            ) {
                Ok(png) => {
                    if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                        self.bridge.spawn(async move {
                            let _ = sender.send(SessionCommand::UploadImage { data: png }).await;
                        });
                    }
                    return; // 图片路径已提交给 actor，不走文字粘贴
                }
                Err(e) => {
                    tracing::warn!("图片粘贴：PNG 编码失败：{}", e);
                    return; // 剪贴板有图但编码失败，不 fallback 文字（内容是图不是文）
                }
            }
        }
    }
```

完整修改后的 `paste()` 方法（从 `fn paste` 到方法结束），供参考对照：

```rust
fn paste(&mut self, conn: aish_types::ConnectionId, cx: &mut Context<Self>) {
    // 先检测剪贴板是否有图片（arboard 直接读 Windows 剪贴板）
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(img) = cb.get_image() {
            match crate::terminal::image::encode_rgba_to_png(
                img.width as u32,
                img.height as u32,
                &img.bytes,
            ) {
                Ok(png) => {
                    if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                        self.bridge.spawn(async move {
                            let _ = sender.send(SessionCommand::UploadImage { data: png }).await;
                        });
                    }
                    return;
                }
                Err(e) => {
                    tracing::warn!("图片粘贴：PNG 编码失败：{}", e);
                    return;
                }
            }
        }
    }

    // 无图片，走现有文字粘贴逻辑（以下为原有代码，不改动）
    use alacritty_terminal::term::TermMode;

    let Some(item) = cx.read_from_clipboard() else {
        return;
    };
    let Some(text) = item.text() else {
        return;
    };
    if text.is_empty() {
        return;
    }

    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                normalized.push('\r');
                if matches!(chars.peek(), Some('\n')) {
                    chars.next();
                }
            }
            '\n' => normalized.push('\r'),
            other => normalized.push(other),
        }
    }

    let bracketed = self
        .state
        .read(cx)
        .term_of(conn)
        .map(|t| t.mode().contains(TermMode::BRACKETED_PASTE))
        .unwrap_or(false);

    let bytes = if bracketed {
        let mut out = Vec::with_capacity(normalized.len() + 12);
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(normalized.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        normalized.into_bytes()
    };

    let sender = match self.state.read(cx).sessions.get(&conn).cloned() {
        Some(s) => s,
        None => return,
    };
    self.bridge.spawn(async move {
        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
    });
}
```

- [ ] **Step 2: 运行 clippy 验证编译**

```bash
cargo +nightly clippy -p aish-app -- -D warnings 2>&1 | tail -20
```

预期：0 errors，0 warnings（tracing::warn! 宏用到 tracing，已在 Cargo.toml 引入）。

- [ ] **Step 3: 提交**

```bash
git add crates/aish-app/src/views/terminal_view.rs
git commit -m "feat(ui): paste() 优先检测剪贴板图片，有图走 SFTP 上传（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 7: ssh_actor — UploadImage match arm

**Files:**
- Modify: `crates/aish-app/src/ssh_actor.rs`

在 `connection_task` 的 `cmd = cmd_rx.recv()` match 分支里，找到 `Some(SessionCommand::Disconnect) | None =>` 之前，加 UploadImage 分支。

- [ ] **Step 1: 找到插入位置**

在 `ssh_actor.rs` 约第 209 行 `Some(SessionCommand::AttachTmux { session: sess_id }) =>` 分支结束后，在 `Some(SessionCommand::Disconnect) | None =>` 之前，插入：

```rust
                Some(SessionCommand::UploadImage { data }) => {
                    let conn_id = conn;
                    let session_for_sftp = session.clone();
                    let tx_for_sftp = event_tx.clone();
                    tokio::spawn(async move {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        let remote_path = format!("/tmp/aish-clip-{}.png", ts);
                        match session_for_sftp.sftp_upload(&remote_path, &data).await {
                            Ok(()) => {
                                let _ = tx_for_sftp
                                    .send(SshEvent::ImageUploaded {
                                        conn: conn_id,
                                        path: remote_path,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx_for_sftp
                                    .send(SshEvent::ImageUploadFailed {
                                        conn: conn_id,
                                        msg: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    });
                }
```

- [ ] **Step 2: 在 use 声明处加 aish_ssh 引用（如尚未 use）**

`connection_task` 顶部已有 `use aish_ssh::{ChannelMsg, SshClient};`，无需改动。`sftp_upload` 是 `SshClient` 的方法，直接调用即可。

- [ ] **Step 3: 运行 clippy 验证编译**

```bash
cargo +nightly clippy -p aish-app -- -D warnings 2>&1 | tail -20
```

预期：0 errors，0 warnings。

- [ ] **Step 4: 提交**

```bash
git add crates/aish-app/src/ssh_actor.rs
git commit -m "feat(actor): 处理 UploadImage — spawn SFTP 子 task（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 8: app.rs — ImageUploaded / ImageUploadFailed 事件处理

**Files:**
- Modify: `crates/aish-app/src/app.rs`

- [ ] **Step 1: 在 app.rs 事件循环 match 末尾加两个 arm**

找到 `SshEvent::TmuxAttached { conn, session } =>` 分支之后（约第 95 行），在 `});` 闭合之前，插入：

```rust
                    SshEvent::ImageUploaded { conn, path } => {
                        // 把远端路径输入到当前命令行（不带 \r，让用户自己按 Enter）
                        if let Some(sender) = state.sessions.get(&conn).cloned() {
                            let bytes = path.into_bytes();
                            let _ = sender.try_send(SessionCommand::SendBytes(bytes));
                        }
                    }
                    SshEvent::ImageUploadFailed { conn, msg } => {
                        // 在终端里打印红色错误提示（不干扰当前命令行）
                        if let Some(sender) = state.sessions.get(&conn).cloned() {
                            let err = format!(
                                "\x1b[31m[aish] 图片上传失败: {}\x1b[0m\r\n",
                                msg
                            );
                            let _ = sender.try_send(SessionCommand::SendBytes(err.into_bytes()));
                        }
                    }
```

- [ ] **Step 2: 确认 SessionCommand 已在 app.rs use 中可见**

顶部已有：
```rust
use crate::state::{AppState, SidebarTab, SshEvent};
```

`SessionCommand` 在 `state` 模块中，需要在 app.rs 里添加：

在 `use crate::state::{AppState, SidebarTab, SshEvent};` 这行改为：

```rust
use crate::state::{AppState, SessionCommand, SidebarTab, SshEvent};
```

- [ ] **Step 3: 运行完整测试套件（质量门禁）**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：fmt 无改动，clippy 0 warnings，所有测试通过（目标 ≥ 92 个）。

- [ ] **Step 4: 提交**

```bash
git add crates/aish-app/src/app.rs
git commit -m "feat(app): 处理 ImageUploaded → echo 路径 / ImageUploadFailed → 红字提示（M8）

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 9: INDEX 更新

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 更新 INDEX.md**

在 `## Milestones（按时间倒序）` 区域，在 M4b 条目之前插入 M8 条目：

```markdown
### M8 — 图片粘贴（Ctrl+Shift+V + SFTP + echo path）（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m8-image-paste-design.md`](specs/2026-05-08-aish-m8-image-paste-design.md)
- plan：[`plans/2026-05-08-aish-m8-image-paste.md`](plans/2026-05-08-aish-m8-image-paste.md)
- 范围：Ctrl+Shift+V 检测剪贴板类型 → 图片走 arboard 读取 + PNG 编码 + SshClient::sftp_upload → 远端 /tmp + echo 路径到 PTY；文字走现有 bracketed paste 逻辑
- 关键 commits：
  - M8-task-1：workspace 依赖加 russh-sftp / arboard / image
  - M8-task-2/3：SshError::Sftp + SshClient::sftp_upload
  - M8-task-4：state.rs UploadImage + ImageUploaded/Failed
  - M8-task-5：terminal/image.rs encode_rgba_to_png
  - M8-task-6：terminal_view paste() 图片检测
  - M8-task-7：ssh_actor UploadImage match arm
  - M8-task-8：app.rs 事件处理 + 质量门禁
```

同时更新 `## 当前状态` 区域的活跃分支描述和下一里程碑：

```markdown
- **活跃分支**：`main`（M8 图片粘贴已完成）
- **下一里程碑**：M6 Activity 实时活动条 或 backlog 条目
```

- [ ] **Step 2: 提交**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs: INDEX 更新，M8 图片粘贴已完成

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## 手工验证清单

在实现完成后手工测试：

1. **正常路径**：截图（Win+Shift+S）→ 在 aish 终端按 Ctrl+Shift+V → 应看到命令行出现 `/tmp/aish-clip-<ts>.png`（无 Enter）
2. **文字粘贴**：复制一段文字 → Ctrl+Shift+V → 文字粘贴到命令行（原有行为不变）
3. **上传失败**：断开 SSH 后（或连接中断时）按 Ctrl+Shift+V 粘贴图片 → 应看到红色错误行
4. **非连接 tab**：在无连接的 tab 按 Ctrl+Shift+V → 无反应（`sessions.get(&conn)` 返回 None）

## Self-Review

**Spec 覆盖检查：**
- ✅ Ctrl+Shift+V 触发 → Task 6
- ✅ arboard 图片检测 → Task 5+6
- ✅ PNG 编码 → Task 5
- ✅ SessionCommand::UploadImage → Task 4+7
- ✅ SFTP 上传 → Task 3
- ✅ SshEvent::ImageUploaded / ImageUploadFailed → Task 4+8
- ✅ 路径 echo 到 PTY（不带 \r） → Task 8
- ✅ 上传失败红字提示 → Task 8
- ✅ 文字 fallback → Task 6（现有 paste 逻辑保留）

**占位符扫描：** 无 TBD / TODO。

**类型一致性：**
- `SessionCommand::UploadImage { data: Vec<u8> }` — Task 4 定义，Task 6 生产，Task 7 消费 ✓
- `SshEvent::ImageUploaded { conn: ConnectionId, path: String }` — Task 4 定义，Task 7 生产，Task 8 消费 ✓
- `SshClient::sftp_upload(&self, remote_path: &str, data: &[u8]) -> Result<(), SshError>` — Task 3 定义，Task 7 调用 ✓
