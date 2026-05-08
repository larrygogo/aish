# M8 — 图片粘贴 设计文档

> **里程碑**：M8（Sub-project E · 图片粘贴）
> **日期**：2026-05-08
> **范围**：Ctrl+Shift+V 检测剪贴板类型 → 图片走 SFTP 上传远端 /tmp → echo 路径到 PTY；文字走普通粘贴。

---

## 目标

用户在 aish 终端按 Ctrl+Shift+V，若剪贴板含图片（截图或复制的图片文件），自动将图片上传到远端 `/tmp/aish-clip-<ts>.png` 并把路径输入到当前命令行，让用户决定是否按 Enter 提交给 agent。若剪贴板是文字，走普通文字粘贴（顺带实现 backlog paste 条目）。

**Out-of-scope（M8 不做）：**
- 文件大小限制提示
- 上传进度条 / 状态提示
- Agent-specific 注入（Claude Code `--image`、Codex base64 等）
- 文件拖拽到窗口
- Bracketed paste mode（文字粘贴用简单 send_bytes，不包 `\x1b[200~...\x1b[201~`）

---

## 架构

### 数据流

```
[用户按 Ctrl+Shift+V]
       │
       ▼
TerminalView::handle_key
  ├─ arboard::Clipboard::get_image() 成功
  │     └─ encode_rgba_to_png → PNG bytes
  │           └─ SessionCommand::UploadImage { data: Vec<u8> }
  │                    │
  │               ssh_actor（tokio::spawn 子 task）
  │                    ├─ sftp_upload("/tmp/aish-clip-<ts>.png", data)
  │                    ├─ 成功 → SshEvent::ImageUploaded { conn, path }
  │                    └─ 失败 → SshEvent::ImageUploadFailed { conn, msg }
  │
  └─ arboard::Clipboard::get_text() 成功
        └─ send_bytes(text.as_bytes())  ← 普通粘贴

[app.rs 收到 SshEvent::ImageUploaded]
  └─ send_bytes(path.as_bytes())   ← 不带 \r，用户自行按 Enter

[app.rs 收到 SshEvent::ImageUploadFailed]
  └─ send_bytes("\x1b[31m[aish] 图片上传失败: {msg}\x1b[0m\r\n")
```

### 新增 enum variants

```rust
// state.rs — SessionCommand
UploadImage { data: Vec<u8> }   // PNG 编码后的图片 bytes

// state.rs — SshEvent
ImageUploaded { conn: ConnectionId, path: String }
ImageUploadFailed { conn: ConnectionId, msg: String }
```

---

## 各模块设计

### 1. 剪贴板读取（TerminalView）

在 `handle_key` Ctrl+Shift+V 分支调用 `arboard`，**在 GPUI 主线程（Windows COM STA）直接调用**，不需要额外线程：

```rust
// Ctrl+Shift+V
let mut cb = arboard::Clipboard::new()?;

if let Ok(img) = cb.get_image() {
    // img: arboard::ImageData { width: usize, height: usize, bytes: Cow<[u8]> }
    // bytes 是 RGBA，每像素 4 字节
    let png = crate::terminal::image::encode_rgba_to_png(
        img.width as u32,
        img.height as u32,
        &img.bytes,
    )?;
    bridge.send(conn, SessionCommand::UploadImage { data: png });
    return;
}

if let Ok(text) = cb.get_text() {
    bridge.send_bytes(conn, text.as_bytes());
}
```

### 2. PNG 编码辅助（terminal/image.rs）

```rust
use image::{ImageFormat, RgbaImage};
use std::io::Cursor;

pub fn encode_rgba_to_png(w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<Vec<u8>> {
    let img = RgbaImage::from_raw(w, h, rgba.to_vec())
        .ok_or_else(|| anyhow::anyhow!("RGBA buffer size mismatch"))?;
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)?;
    Ok(buf)
}
```

### 3. SFTP 上传（aish-sftp/src/lib.rs）

依赖 `russh-sftp 2.x`（与 workspace russh 0.46 配套）：

```rust
use russh::client::Handle;
use russh_sftp::client::SftpSession;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SftpError {
    #[error("channel open failed: {0}")]
    Channel(#[from] russh::Error),
    #[error("sftp error: {0}")]
    Sftp(#[from] russh_sftp::client::error::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn sftp_upload<H: russh::client::Handler>(
    session: &Handle<H>,
    remote_path: &str,
    data: &[u8],
) -> Result<(), SftpError> {
    let channel = session.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    let sftp = SftpSession::new(channel.into_stream()).await?;
    let mut file = sftp.create(remote_path).await?;
    use tokio::io::AsyncWriteExt;
    file.write_all(data).await?;
    file.close().await?;
    Ok(())
}
```

### 4. ssh_actor 处理 UploadImage

```rust
// ssh_actor.rs — actor 主 loop match 分支
SessionCommand::UploadImage { data } => {
    let conn = self.conn;
    let session = self.session.clone();
    let tx = self.event_tx.clone();

    tokio::spawn(async move {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let remote_path = format!("/tmp/aish-clip-{}.png", ts);

        match aish_sftp::sftp_upload(&session, &remote_path, &data).await {
            Ok(()) => {
                let _ = tx.send(SshEvent::ImageUploaded { conn, path: remote_path }).await;
            }
            Err(e) => {
                let _ = tx.send(SshEvent::ImageUploadFailed {
                    conn,
                    msg: e.to_string(),
                }).await;
            }
        }
    });
}
```

### 5. app.rs 事件处理

```rust
SshEvent::ImageUploaded { conn, path } => {
    if let Some(sender) = state.sessions.get(&conn) {
        let _ = sender.try_send(SessionCommand::SendBytes(path.into_bytes()));
    }
}
SshEvent::ImageUploadFailed { conn, msg } => {
    let err = format!("\x1b[31m[aish] 图片上传失败: {}\x1b[0m\r\n", msg);
    if let Some(sender) = state.sessions.get(&conn) {
        let _ = sender.try_send(SessionCommand::SendBytes(err.into_bytes()));
    }
}
```

---

## 文件结构

| 操作 | 路径 | 说明 |
|---|---|---|
| 修改 | `crates/aish-app/src/state.rs` | 新增 `SessionCommand::UploadImage` / `SshEvent::ImageUploaded` / `ImageUploadFailed` |
| 修改 | `crates/aish-app/src/views/terminal_view.rs` | Ctrl+Shift+V 分支：arboard → encode → 发命令；文字 fallback |
| 新建 | `crates/aish-app/src/terminal/image.rs` | `encode_rgba_to_png` 辅助函数 |
| 修改 | `crates/aish-app/src/terminal/mod.rs` | `pub mod image;` |
| 修改 | `crates/aish-app/src/ssh_actor.rs` | 处理 `UploadImage`，spawn SFTP task |
| 修改 | `crates/aish-app/src/app.rs` | 处理 `ImageUploaded` → send path；`ImageUploadFailed` → send 错误行 |
| 实现 | `crates/aish-sftp/src/lib.rs` | `sftp_upload` 函数 |
| 修改 | `crates/aish-app/Cargo.toml` | 加 `arboard`、`image` |
| 修改 | `crates/aish-sftp/Cargo.toml` | 加 `russh-sftp` |
| 修改 | `Cargo.toml`（workspace） | 加 `russh-sftp`、`arboard`、`image` 到 workspace deps |

---

## 风险与已知限制

| 风险 | 缓解 |
|---|---|
| russh-sftp 2.x 与 russh 0.46 API 兼容性 | 实现前先写 smoke test 验证 `channel_open_session` + `request_subsystem("sftp")` |
| arboard 在 GPUI 主线程的 COM 初始化 | arboard Windows 后端自行初始化 COM，实测无冲突；如有问题改为 `std::thread::spawn` + channel 同步 |
| 图片体积无上限 | M8 不做限制，后续加 4MB 上限提示 |
| 远端 /tmp 无写权限 | `ImageUploadFailed` 报错，用户可见 |

---

## 测试策略

- `terminal/image.rs`：单测 `encode_rgba_to_png` 输出为合法 PNG（读回宽高匹配）
- `aish-sftp`：单测 `sftp_upload` 用 mock channel（或跳过，由集成测试覆盖）
- `state.rs`：新 variants 的构造/匹配单测（顺延现有模式）
- 手工验证：截图 → Ctrl+Shift+V → 看路径出现在命令行；上传失败场景（断连后粘贴）
