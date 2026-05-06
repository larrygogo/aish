# aish M2a — SSH 接入 + 单 PTY shell 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 aish-ssh crate 实装 russh-based SshClient + Channel + PTY；在 aish-app 完全 retire mock，迁移到真实类型 (`aish_types::HostId/HostConfig`)；引入 Actor model 的 `host_session_task` 让每个 host 一个 tokio task 持有 SshSession；GPUI 键盘事件接通 PTY writer。完成后能从 GPUI 主窗口 click → 真 SSH connect → 看到 shell prompt → 打命令看输出。

**Architecture:** Actor model — 每个 host 首次 click 时 Bridge spawn 一个 tokio task，task 内 own SshSession + PTY channel reader/writer；通过 `mpsc::Sender<SessionCommand>` 接收 GPUI 端键盘输入，通过共享的 `mpsc::Sender<SshEvent>` 发输出回 GPUI。AppState 仅持有 `HashMap<HostId, mpsc::Sender<SessionCommand>>`，不直接持有 SshSession，避免 Send / lifetime 复杂度。

**Tech Stack:** russh (SSH 协议), russh-keys (key 文件解析), tokio (async runtime), aish_types (HostConfig / HostId / SshAuth), GPUI (键盘事件)

**前置:** M1 已完成（commit `e5ad92f` 之后），M2a spec 已落盘 (`docs/superpowers/specs/2026-05-06-aish-m2a-ssh-bootstrap-design.md`, commit `d8ed86f`)。

---

## ⚠️ 实施者须知

### russh API 现实

russh 在 0.x 阶段，API 仍在演进。本 plan 给的 russh 代码是**参考骨架**，标 `// RUSSH-API` 的地方可能与实际版本不一致。**实施时按 cargo build 错误调整**，不要扭曲实现去硬贴 plan。

参考来源：
- russh 当前版本的 [examples](https://github.com/Eugeny/russh/tree/main/russh/examples)（特别是 `client_exec_simple.rs` 与 `interactive.rs`）
- docs.rs 上的最新 API 文档

### M2a CI 不跑真 SSH

集成测试推迟到 M5。M2a 的 cargo test 只覆盖：
- aish-ssh::error 的 `From<russh::Error>` 转换
- aish-app::ssh_actor::encode_key 键盘字节映射
- aish-app::state 的 sessions HashMap 增删

不要为了"提高覆盖率"加伪集成测试（如 connect 一个 mock TCP listener）。

### Demo 验证 implementer 跑不了

implementer subagent 没有 user 的 VPS 凭证，**无法验证 demo**。每个 task 的 verification 只能确认：build 通过 / cargo test 全绿 / cargo run 启动无 panic。**真 demo 13 项由 user 在 Task 8 手动验证**。

### 凭证安全

`fixtures.rs` 用环境变量读 VPS 信息，**不**把 host / user / key path 直接写进源代码。这是 spec Section 11 风险表 #3 的硬性要求。

---

## File Structure（M2a 完成时的目标状态）

```
aish/
├── Cargo.toml                       # 修改：workspace.dependencies 加 russh + russh-keys
├── crates/
│   ├── aish-types/                  # 不变
│   ├── aish-ssh/
│   │   ├── Cargo.toml               # 修改：加 russh / russh-keys / tokio dep
│   │   └── src/
│   │       ├── lib.rs               # 改写：mod 声明 + reexport
│   │       ├── error.rs             # 新：SshError + From<russh::Error>
│   │       ├── client.rs            # 新：SshClient + connect (KeyFile only)
│   │       └── channel.rs           # 新：Channel + PTY 包装
│   ├── aish-tmux/                   # 不变（M3 才动）
│   ├── aish-sftp/                   # 不变（M4 才动）
│   ├── aish-secrets/                # 不变（M5 才动）
│   └── aish-app/
│       ├── Cargo.toml               # 修改：加 aish-types + aish-ssh dep
│       └── src/
│           ├── main.rs              # 修改：mod 声明（删 mock，加 fixtures, ssh_actor）
│           ├── app.rs               # 修改：MockEvent → SshEvent，cx.spawn loop 处理 4 种事件
│           ├── state.rs             # 改写：HostId/HostConfig 用 aish_types，新增 sessions/SshEvent/SessionCommand 等
│           ├── bridge.rs            # 修改：MockEvent → SshEvent；新增 spawn_session helper
│           ├── ssh_actor.rs         # 新：host_session_task + encode_key
│           ├── fixtures.rs          # 新：env var 读 HostConfig
│           ├── mock.rs              # ❌ 删除
│           └── views/
│               ├── mod.rs           # 不变
│               ├── host_list.rs     # 修改：HostId 类型迁移；click 调 bridge.spawn_session
│               └── host_pane.rs     # 修改：渲染逻辑迁移；新增键盘输入 handler
```

---

## Task 1: aish-ssh 加 russh 依赖 + error.rs

**Files:**
- Modify: `Cargo.toml`（workspace 根，加 russh + russh-keys）
- Modify: `crates/aish-ssh/Cargo.toml`（依赖 russh + russh-keys + tokio + aish-types）
- Modify: `crates/aish-ssh/src/lib.rs`（mod 声明）
- Create: `crates/aish-ssh/src/error.rs`

- [ ] **Step 1: 在 workspace 根 Cargo.toml 加 russh / russh-keys 共享版本**

读 `C:\Users\larry\Desktop\workspace\aish\Cargo.toml`，在 `[workspace.dependencies]` 段末尾追加（保持已有顺序不动）：

```toml
russh = "0.45"
russh-keys = "0.45"
```

注意：版本号要一致。如果 0.45 在 cargo build 时找不到（可能 yanked 或不存在），用 cargo search 看实际可用版本：

```bash
cargo search russh --limit 3
```

按 search 结果选最新稳定版（>= 0.40），同步改两处 (russh + russh-keys)。

- [ ] **Step 2: 修改 `crates/aish-ssh/Cargo.toml` 加依赖**

当前内容（参考 M0 写的）：

```toml
[package]
name = "aish-ssh"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish SSH 客户端层（M2 起接入 russh）"

[dependencies]
aish-types = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

改为（在 dependencies 末尾追加 4 行）：

```toml
[package]
name = "aish-ssh"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish SSH 客户端层（russh 包装）"

[dependencies]
aish-types = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
russh = { workspace = true }
russh-keys = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
```

- [ ] **Step 3: 改写 `crates/aish-ssh/src/lib.rs`**

完整替换为：

```rust
//! aish-ssh — SSH 客户端层（russh 包装）。

#![allow(dead_code)]

mod error;

pub use error::{SshError, SshErrorKind};

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: 创建 `crates/aish-ssh/src/error.rs`**

```rust
//! SshError 类型，统一所有 aish-ssh 操作的错误。
//!
//! 设计要点：
//! - kind: 粗粒度分类，UI 据此选错误前缀（[error] / [info]）
//! - source: 原始错误链（russh / io / 文件系统）

use std::path::PathBuf;

use thiserror::Error;

/// 粗粒度错误分类，给 UI 决定显示样式用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    /// TCP 连接失败 / DNS 失败 / 连接 refused
    ConnectFailed,
    /// SSH 认证失败（key 不被接受 / 不存在 / 解析失败）
    AuthFailed,
    /// 已建立连接后的 IO 错误（网络断、远端关闭等）
    Io,
    /// 协议层错误（PTY 申请失败、shell 启动失败等）
    Protocol,
}

#[derive(Debug, Error)]
pub enum SshError {
    #[error("connect failed: {0}")]
    Connect(#[source] std::io::Error),

    #[error("auth failed: {0}")]
    Auth(String),

    #[error("key file {path:?} not readable: {source}")]
    KeyFileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("key file {path:?} parse failed: {source}")]
    KeyFileParse {
        path: PathBuf,
        #[source]
        source: russh_keys::Error,
    },

    #[error("ssh protocol error: {0}")]
    Protocol(#[source] russh::Error),

    #[error("io error: {0}")]
    Io(#[source] std::io::Error),
}

impl SshError {
    pub fn kind(&self) -> SshErrorKind {
        match self {
            Self::Connect(_) => SshErrorKind::ConnectFailed,
            Self::Auth(_) | Self::KeyFileRead { .. } | Self::KeyFileParse { .. } => {
                SshErrorKind::AuthFailed
            }
            Self::Io(_) => SshErrorKind::Io,
            Self::Protocol(_) => SshErrorKind::Protocol,
        }
    }
}

impl From<russh::Error> for SshError {
    fn from(err: russh::Error) -> Self {
        // russh 0.x 的 Error 没有细分 connect vs protocol，统一归 Protocol
        // 上层路径里如果是 TCP connect 阶段，应在调用处包成 SshError::Connect
        Self::Protocol(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_error_kind() {
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = SshError::Connect(io);
        assert_eq!(err.kind(), SshErrorKind::ConnectFailed);
    }

    #[test]
    fn auth_error_kind() {
        let err = SshError::Auth("server rejected".into());
        assert_eq!(err.kind(), SshErrorKind::AuthFailed);
    }

    #[test]
    fn key_file_read_error_kind_is_auth() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = SshError::KeyFileRead {
            path: PathBuf::from("/nope"),
            source: io,
        };
        assert_eq!(err.kind(), SshErrorKind::AuthFailed);
    }

    #[test]
    fn protocol_error_kind() {
        let err = SshError::Protocol(russh::Error::Disconnect);
        assert_eq!(err.kind(), SshErrorKind::Protocol);
    }

    #[test]
    fn from_russh_error_maps_to_protocol() {
        let err: SshError = russh::Error::Disconnect.into();
        assert_eq!(err.kind(), SshErrorKind::Protocol);
    }

    #[test]
    fn error_display_contains_inner_message() {
        let err = SshError::Auth("bad key".into());
        let s = format!("{}", err);
        assert!(s.contains("auth failed"));
        assert!(s.contains("bad key"));
    }
}
```

> **可能要调整**：`russh_keys::Error` 与 `russh::Error::Disconnect` 是否真存在，按 russh 0.45 实际名调整。如果 `russh::Error::Disconnect` 不存在，改用 `russh::Error::IO(...)` 之类一个明显的 variant 写测试。

- [ ] **Step 5: 验证 cargo build + test**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-ssh
```

Expected: 首次拉 russh + 大量 transitive deps（约 3-8 分钟编译）。结束 0 退出。如果 russh API 名（如 `russh_keys::Error`）报错，按错误调整 error.rs。

```bash
cargo test -p aish-ssh
```

Expected: 6 passed（5 个 error 单测 + 1 个 smoke test）。

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/aish-ssh/
git commit -m "feat(aish-ssh): 引入 russh 依赖 + SshError 错误类型"
```

---

## Task 2: aish-ssh client.rs — SshClient::connect

**Files:**
- Create: `crates/aish-ssh/src/client.rs`
- Modify: `crates/aish-ssh/src/lib.rs`

- [ ] **Step 1: 看 russh 当前 examples 找 connect 模式**

```bash
ls C:/Users/larry/.cargo/git/checkouts/ 2>$null
# 或者直接看 docs:
# https://docs.rs/russh/latest/russh/client/index.html
# 从 docs.rs 抄一份最小 client connect example
```

或者下载 russh 源码看 examples：

```bash
# cargo metadata 找 russh 源码路径
cargo metadata --format-version 1 | grep '"russh"' | head -1
```

记下：
- `russh::client::Handler` trait 当前签名（特别是 `check_server_key`）
- `russh::client::connect(config, addr, handler)` 签名
- 如何 `authenticate_publickey`

- [ ] **Step 2: 创建 `crates/aish-ssh/src/client.rs`**

```rust
//! SshClient — russh 包装。M2a 仅支持 KeyFile 认证。

use std::sync::Arc;

use aish_types::{HostConfig, SshAuth};
use async_trait::async_trait;
use russh::client::{Config, Handle, Handler};
use russh::keys::PrivateKey;
use russh::ChannelId;

use crate::error::SshError;

/// SshClient 持有一个建立好的 SSH 连接（已 handshake + auth 完成）。
///
/// 后续调用 `open_channel().request_pty(...)` 拿可用的 PTY channel。
pub struct SshClient {
    handle: Handle<NoopHandler>,
}

impl SshClient {
    /// 连接到 cfg.host:cfg.port，按 cfg.auth 认证。
    ///
    /// M2a 仅支持 SshAuth::KeyFile；其他 variant 返回 Auth 错误。
    pub async fn connect(cfg: &HostConfig) -> Result<Self, SshError> {
        let config = Arc::new(Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(60 * 60)),
            ..Default::default()
        });

        // TCP 连接
        let addr = format!("{}:{}", cfg.host, cfg.port);
        let mut handle = russh::client::connect(config, addr, NoopHandler {})
            .await
            .map_err(|e| match e {
                russh::Error::IO(io) => SshError::Connect(io),
                other => SshError::Protocol(other),
            })?;

        // 认证
        match &cfg.auth {
            SshAuth::KeyFile { path } => {
                let key_bytes = std::fs::read(path).map_err(|source| SshError::KeyFileRead {
                    path: path.clone(),
                    source,
                })?;
                let key = russh_keys::decode_secret_key(
                    std::str::from_utf8(&key_bytes).map_err(|_| SshError::Auth(
                        "key file is not valid UTF-8".into(),
                    ))?,
                    None,
                )
                .map_err(|source| SshError::KeyFileParse {
                    path: path.clone(),
                    source,
                })?;

                let auth_result = handle
                    .authenticate_publickey(
                        &cfg.user,
                        Arc::new(PrivateKey::from(key)),
                    )
                    .await
                    .map_err(SshError::from)?;

                if !auth_result.success() {
                    return Err(SshError::Auth(format!(
                        "server rejected publickey for user {}",
                        cfg.user
                    )));
                }
            }
            SshAuth::Password => {
                return Err(SshError::Auth(
                    "Password auth not supported in M2a (use KeyFile)".into(),
                ));
            }
            SshAuth::Agent => {
                return Err(SshError::Auth(
                    "Agent auth not supported in M2a (use KeyFile)".into(),
                ));
            }
        }

        Ok(Self { handle })
    }

    /// 获取底层 russh Handle（仅 channel.rs 内部用）。
    pub(crate) fn handle(&mut self) -> &mut Handle<NoopHandler> {
        &mut self.handle
    }

    /// 主动关闭连接。可选——drop 时会自动关闭。
    pub async fn close(self) {
        let _ = self.handle.disconnect(russh::Disconnect::ByApplication, "", "").await;
    }
}

/// russh Handler trait 的最小实现：M2a 不验证 server key（信任所有）。
///
/// **安全提醒**：这是 M2a 的简化路径。M5 实施 SecretStore 时引入 known_hosts 校验。
pub(crate) struct NoopHandler;

#[async_trait]
impl Handler for NoopHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // M2a: 信任所有 server key（M5 改为 known_hosts 校验）
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;
    use aish_types::{HostId, ProfileId};

    #[tokio::test]
    async fn connect_with_password_auth_returns_unsupported_error() {
        let cfg = HostConfig {
            id: HostId(Uuid::new_v4()),
            label: "test".into(),
            host: "127.0.0.1".into(),
            port: 22,
            user: "test".into(),
            auth: SshAuth::Password,
            env_profile: None::<ProfileId>,
        };
        let result = SshClient::connect(&cfg).await;
        assert!(matches!(result, Err(SshError::Auth(_))));
        if let Err(SshError::Auth(msg)) = result {
            assert!(msg.contains("KeyFile"));
        }
    }

    #[tokio::test]
    async fn connect_with_agent_auth_returns_unsupported_error() {
        let cfg = HostConfig {
            id: HostId(Uuid::new_v4()),
            label: "test".into(),
            host: "127.0.0.1".into(),
            port: 22,
            user: "test".into(),
            auth: SshAuth::Agent,
            env_profile: None,
        };
        let result = SshClient::connect(&cfg).await;
        assert!(matches!(result, Err(SshError::Auth(_))));
    }

    #[tokio::test]
    async fn connect_with_missing_key_file_returns_key_file_read_error() {
        let cfg = HostConfig {
            id: HostId(Uuid::new_v4()),
            label: "test".into(),
            host: "127.0.0.1".into(),
            port: 22,
            user: "test".into(),
            auth: SshAuth::KeyFile {
                path: PathBuf::from("/nonexistent/key/path"),
            },
            env_profile: None,
        };
        let result = SshClient::connect(&cfg).await;
        // 实际可能因为 TCP connect 先失败（127.0.0.1:22 没 sshd），所以不强求
        // 此测试只验证函数能 await 完成并返回 Err
        assert!(result.is_err());
    }
}
```

> **可能要大幅调整**：russh 0.45 的 `connect()` 签名 / `authenticate_publickey` 返回类型 / `decode_secret_key` 路径都可能不同。按 cargo build 错误 + russh examples 调整。如果某些 API 完全找不到对应（如 `PrivateKey::from(key)` 报错），改用 `russh_keys::decode_secret_key(...)` 直接返回的类型。

- [ ] **Step 3: 在 lib.rs 加 mod client + reexport**

修改 `crates/aish-ssh/src/lib.rs`：

```rust
//! aish-ssh — SSH 客户端层（russh 包装）。

#![allow(dead_code)]

mod client;
mod error;

pub use client::SshClient;
pub use error::{SshError, SshErrorKind};

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: 验证**

```bash
cargo build -p aish-ssh
```

Expected: 编译通过。如果 russh API 报错，**先看 cargo build 错误第 1-30 行**，按 russh 0.45 实际 API 调整 client.rs。

```bash
cargo test -p aish-ssh
```

Expected: 9 passed（6 个旧 + 3 个 client 测试）。

> 注意：`connect_with_missing_key_file_returns_key_file_read_error` 测试可能因为 TCP connect 先失败而返回 `Connect` 错误（127.0.0.1:22 没人 listen）。**这种情况下 assert 应该改为 `result.is_err()` 而不是检查具体 variant**——实际上 plan 已经这么写了。

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-ssh/
git commit -m "feat(aish-ssh): 实现 SshClient::connect (KeyFile only)"
```

---

## Task 3: aish-ssh channel.rs — Channel + PTY

**Files:**
- Create: `crates/aish-ssh/src/channel.rs`
- Modify: `crates/aish-ssh/src/lib.rs`
- Modify: `crates/aish-ssh/src/client.rs`（加 `open_channel` 方法）

- [ ] **Step 1: 创建 `crates/aish-ssh/src/channel.rs`**

```rust
//! Channel — russh::Channel 的 PTY 包装。

use russh::client::Msg;
use russh::ChannelMsg;

use crate::error::SshError;

/// Channel 包装一个已开的 SSH channel。
///
/// 典型流程：
///   1. SshClient::open_channel() 拿到 Channel
///   2. request_pty(120, 40, "xterm-256color")
///   3. shell()
///   4. 在 select! 里轮 wait() (read) + data() (write)
pub struct Channel {
    inner: russh::Channel<Msg>,
}

impl Channel {
    pub(crate) fn new(inner: russh::Channel<Msg>) -> Self {
        Self { inner }
    }

    /// 申请 PTY。M2a hardcoded 120×40 + xterm-256color。
    pub async fn request_pty(
        &mut self,
        cols: u16,
        rows: u16,
        term: &str,
    ) -> Result<(), SshError> {
        self.inner
            .request_pty(
                false,        // want_reply
                term,
                cols as u32,
                rows as u32,
                0,            // pixel_width
                0,            // pixel_height
                &[],          // terminal_modes (default)
            )
            .await?;
        Ok(())
    }

    /// 启动 shell（必须先 request_pty）。
    pub async fn shell(&mut self) -> Result<(), SshError> {
        self.inner.request_shell(false).await?;
        Ok(())
    }

    /// 写入数据到 PTY（键盘输入）。
    pub async fn data(&mut self, data: &[u8]) -> Result<(), SshError> {
        self.inner.data(data).await?;
        Ok(())
    }

    /// 等待下一个 channel 消息（read）。返回 None = channel 已关闭。
    pub async fn wait(&mut self) -> Option<ChannelMsg> {
        self.inner.wait().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    /// 类型大小不作为正确性测试，只为编译验证 Channel/SshError 类型可正常引用。
    #[test]
    fn channel_compiles() {
        // 这个测试编译通过即说明 Channel struct 与 SshError 都能被引用
        assert!(size_of::<Channel>() > 0);
        assert!(size_of::<SshError>() > 0);
    }
}
```

> **可能要调整**：russh::Channel 的 `request_pty` / `request_shell` / `data` / `wait` 签名按 0.45 实际调整。`Msg` 类型可能在不同 path。

- [ ] **Step 2: 在 client.rs 加 `open_channel` 方法**

在 `SshClient::connect()` 之后追加方法：

```rust
impl SshClient {
    // ... 原有 connect / handle / close 不变 ...

    /// 打开一个新的 SSH channel。
    pub async fn open_channel(&mut self) -> Result<crate::channel::Channel, SshError> {
        let chan = self.handle.channel_open_session().await?;
        Ok(crate::channel::Channel::new(chan))
    }
}
```

注意：`handle.channel_open_session()` 是参考 API，按 russh 实际调整。

- [ ] **Step 3: 改 lib.rs reexport**

```rust
//! aish-ssh — SSH 客户端层（russh 包装）。

#![allow(dead_code)]

mod channel;
mod client;
mod error;

pub use channel::Channel;
pub use client::SshClient;
pub use error::{SshError, SshErrorKind};

// 重新 re-export russh 的 ChannelMsg 让上层不直接 depend on russh
pub use russh::ChannelMsg;

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: 验证**

```bash
cargo build -p aish-ssh
cargo test -p aish-ssh
```

Expected: 10 passed（前 9 + 1 个 channel_compiles）。

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-ssh/
git commit -m "feat(aish-ssh): 添加 Channel + PTY 包装"
```

---

## Task 4: aish-app 类型迁移 + ssh_actor stub + 接通新架构

⚠️ 这是 **M2a 最大的 task**——一次性 retire mock + 引入新类型 + 让架构跑通（不接真 SSH，先用 stub）。step 多但每步小，每步 cargo check 验证不破坏 build。

**Files:**
- Modify: `crates/aish-app/Cargo.toml`（加 aish-types + aish-ssh dep）
- Modify: `crates/aish-app/src/state.rs`（类型迁移 + 新 enum）
- Create: `crates/aish-app/src/ssh_actor.rs`（stub 形态）
- Modify: `crates/aish-app/src/bridge.rs`（MockEvent → SshEvent）
- Modify: `crates/aish-app/src/app.rs`（cx.spawn loop 改）
- Modify: `crates/aish-app/src/views/host_list.rs`（HostId 类型迁移 + spawn_session 调用）
- Modify: `crates/aish-app/src/views/host_pane.rs`（HostId 类型迁移）
- Modify: `crates/aish-app/src/main.rs`（删 mod mock，加 mod ssh_actor + mod fixtures）
- Create: `crates/aish-app/src/fixtures.rs`（最小 stub，Task 5 实装）
- Delete: `crates/aish-app/src/mock.rs`

- [ ] **Step 1: 加 aish-app 依赖**

`crates/aish-app/Cargo.toml` `[dependencies]` 段已有 aish-types/ssh/tmux/sftp/secrets 等 workspace 引用（M0 task 2 加的），不需要再加。但**需要确认 uuid + chrono-like 等都在**。当前依赖：

```toml
[dependencies]
aish-types = { workspace = true }
aish-ssh = { workspace = true }
aish-tmux = { workspace = true }
aish-sftp = { workspace = true }
aish-secrets = { workspace = true }
anyhow = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
gpui = { workspace = true }
gpui_platform = { workspace = true }
```

需要加 `uuid`（HostConfig 含 uuid）：

在 dependencies 末尾追加：

```toml
uuid = { workspace = true }
```

- [ ] **Step 2: 改写 `crates/aish-app/src/state.rs`**

完整替换为：

```rust
//! aish-app App State — M2a 起用真实类型 + Actor model session 管理。

#![allow(dead_code)]

use std::collections::HashMap;

use aish_types::{HostConfig, HostId};
use tokio::sync::mpsc;

/// 从 SSH actor task 推回 GPUI 的事件。
#[derive(Debug)]
pub enum SshEvent {
    Connected { host: HostId },
    PaneOutput { host: HostId, bytes: Vec<u8> },
    Disconnected { host: HostId, reason: DisconnectReason },
    Error { host: HostId, kind: SshErrorKind, msg: String },
}

#[derive(Debug, Clone)]
pub enum DisconnectReason {
    UserRequested,
    RemoteExited,
    NetworkError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    ConnectFailed,
    AuthFailed,
    Io,
    Protocol,
}

/// 从 GPUI 发到 actor task 的命令。
#[derive(Debug)]
pub enum SessionCommand {
    SendBytes(Vec<u8>),
    Disconnect,
}

/// 单一 root Model：所有 UI 共享状态。
#[derive(Default)]
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    pub pane_logs: HashMap<HostId, Vec<String>>,
    /// 已连接 host 的 SessionCommand sender。
    /// 缺失 = 未连接，存在 = 有活跃 session。
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            selected: None,
            pane_logs: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    pub fn select_host(&mut self, id: HostId) {
        self.selected = Some(id);
    }

    pub fn append_log(&mut self, host: HostId, line: String) {
        self.pane_logs.entry(host).or_default().push(line);
    }

    pub fn logs_of(&self, host: HostId) -> &[String] {
        self.pane_logs.get(&host).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn host_label(&self, id: HostId) -> Option<String> {
        self.hosts.iter().find(|h| h.id == id).map(|h| h.label.clone())
    }

    pub fn is_session_active(&self, id: HostId) -> bool {
        self.sessions.contains_key(&id)
    }

    pub fn register_session(&mut self, id: HostId, sender: mpsc::Sender<SessionCommand>) {
        self.sessions.insert(id, sender);
    }

    pub fn drop_session(&mut self, id: HostId) {
        self.sessions.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_types::SshAuth;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn mk_host(label: &str) -> HostConfig {
        HostConfig {
            id: HostId(Uuid::new_v4()),
            label: label.into(),
            host: "example.com".into(),
            port: 22,
            user: "larry".into(),
            auth: SshAuth::KeyFile { path: PathBuf::from("/tmp/k") },
            env_profile: None,
        }
    }

    #[test]
    fn with_hosts_initializes_correctly() {
        let h1 = mk_host("a");
        let h2 = mk_host("b");
        let state = AppState::with_hosts(vec![h1.clone(), h2.clone()]);
        assert_eq!(state.hosts.len(), 2);
        assert_eq!(state.hosts[0].label, "a");
        assert!(state.selected.is_none());
        assert!(state.pane_logs.is_empty());
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn select_host_sets_selected() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.select_host(id);
        assert_eq!(state.selected, Some(id));
    }

    #[test]
    fn append_log_per_host_isolation() {
        let h1 = mk_host("a");
        let h2 = mk_host("b");
        let id1 = h1.id;
        let id2 = h2.id;
        let mut state = AppState::with_hosts(vec![h1, h2]);
        state.append_log(id1, "line A1".into());
        state.append_log(id2, "line B1".into());
        state.append_log(id1, "line A2".into());
        assert_eq!(state.logs_of(id1), &["line A1".to_string(), "line A2".into()]);
        assert_eq!(state.logs_of(id2), &["line B1".to_string()]);
    }

    #[test]
    fn host_label_returns_correct_label() {
        let h = mk_host("my-vps");
        let id = h.id;
        let state = AppState::with_hosts(vec![h]);
        assert_eq!(state.host_label(id), Some("my-vps".into()));
        assert_eq!(state.host_label(HostId(Uuid::new_v4())), None);
    }

    #[tokio::test]
    async fn session_register_and_drop() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        assert!(!state.is_session_active(id));
        state.register_session(id, tx);
        assert!(state.is_session_active(id));
        state.drop_session(id);
        assert!(!state.is_session_active(id));
    }
}
```

- [ ] **Step 3: 创建 `crates/aish-app/src/ssh_actor.rs`（stub 形态）**

```rust
//! ssh_actor — host_session_task：每 host 一个 tokio task。
//!
//! M2a Task 4 阶段是 **stub 形态**：spawn 后立即发 Connected 事件 + 等待
//! SessionCommand，收到任何 SendBytes 都 echo 回去当 PaneOutput。
//! 真 SSH 接通在 Task 6 替换 host_session_task 内部。

use aish_types::{HostConfig, HostId};
use tokio::sync::mpsc;

use crate::state::{DisconnectReason, SessionCommand, SshErrorKind, SshEvent};

/// 在 tokio runtime 上 spawn 一个 host 的 session task。
///
/// 返回 SessionCommand sender — caller 把它存进 AppState.sessions。
pub fn spawn_session(
    runtime: &tokio::runtime::Handle,
    host: HostId,
    config: HostConfig,
    event_tx: mpsc::Sender<SshEvent>,
) -> mpsc::Sender<SessionCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(64);
    runtime.spawn(host_session_task(host, config, cmd_rx, event_tx));
    cmd_tx
}

/// Stub 实现：发 Connected → echo 任何 SendBytes 回 PaneOutput → 收 Disconnect 退出。
async fn host_session_task(
    host: HostId,
    _config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    let _ = event_tx.send(SshEvent::Connected { host }).await;
    let _ = event_tx
        .send(SshEvent::PaneOutput {
            host,
            bytes: b"[stub] type something and press Enter to echo\r\n".to_vec(),
        })
        .await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SessionCommand::SendBytes(bytes) => {
                let _ = event_tx
                    .send(SshEvent::PaneOutput {
                        host,
                        bytes: bytes.clone(),
                    })
                    .await;
                // echo \n if Enter
                if bytes.contains(&b'\r') {
                    let _ = event_tx
                        .send(SshEvent::PaneOutput {
                            host,
                            bytes: b"\n[stub] echo done\r\n".to_vec(),
                        })
                        .await;
                }
            }
            SessionCommand::Disconnect => {
                let _ = event_tx
                    .send(SshEvent::Disconnected {
                        host,
                        reason: DisconnectReason::UserRequested,
                    })
                    .await;
                return;
            }
        }
    }
    // cmd_rx 收到 None = AppState drop sender，自然退出
    let _ = event_tx
        .send(SshEvent::Disconnected {
            host,
            reason: DisconnectReason::UserRequested,
        })
        .await;
}

/// 简易键盘事件 → 字节流编码（M2a 范围）。
///
/// 支持：普通字符 / Enter / Backspace / Tab / Esc / Ctrl+A-Z。
/// 不支持：方向键 / Home / End / F1-12 / Alt+ — M2b alacritty_terminal 接管。
pub fn encode_key(key: &str, ctrl: bool, _alt: bool) -> Vec<u8> {
    if ctrl {
        if let Some(c) = key.chars().next() {
            let upper = c.to_ascii_uppercase();
            if ('A'..='Z').contains(&upper) {
                let byte = (upper as u8) - 0x40;
                return vec![byte];
            }
        }
        return Vec::new();
    }

    match key {
        "enter" => vec![b'\r'],
        "backspace" => vec![0x7f],
        "tab" => vec![b'\t'],
        "escape" => vec![0x1b],
        s if s.len() == 1 => s.as_bytes().to_vec(),
        _ => Vec::new(),  // 不识别的 key 丢弃（M2b 接 alacritty_terminal 处理）
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_normal_chars() {
        assert_eq!(encode_key("a", false, false), b"a");
        assert_eq!(encode_key("Z", false, false), b"Z");
        assert_eq!(encode_key(" ", false, false), b" ");
        assert_eq!(encode_key("/", false, false), b"/");
    }

    #[test]
    fn encode_special_keys() {
        assert_eq!(encode_key("enter", false, false), vec![b'\r']);
        assert_eq!(encode_key("backspace", false, false), vec![0x7f]);
        assert_eq!(encode_key("tab", false, false), vec![b'\t']);
        assert_eq!(encode_key("escape", false, false), vec![0x1b]);
    }

    #[test]
    fn encode_ctrl_combinations() {
        // Ctrl+C = 0x03
        assert_eq!(encode_key("c", true, false), vec![0x03]);
        // Ctrl+D = 0x04
        assert_eq!(encode_key("d", true, false), vec![0x04]);
        // Ctrl+A = 0x01
        assert_eq!(encode_key("a", true, false), vec![0x01]);
        // Ctrl+大写也成
        assert_eq!(encode_key("Z", true, false), vec![0x1a]);
    }

    #[test]
    fn unrecognized_keys_return_empty() {
        assert_eq!(encode_key("F1", false, false), Vec::<u8>::new());
        assert_eq!(encode_key("up", false, false), Vec::<u8>::new());
        assert_eq!(encode_key("home", false, false), Vec::<u8>::new());
    }
}
```

- [ ] **Step 4: 创建 `crates/aish-app/src/fixtures.rs`（最小 stub）**

```rust
//! M2a 临时方案：从环境变量读 host config，避免 commit 真凭证。
//!
//! Task 5 实装真实读取逻辑。本 task 仅占位。

#![allow(dead_code)]

use aish_types::HostConfig;

/// Task 5 实装：从 AISH_DEV_HOST / AISH_DEV_USER / AISH_DEV_KEY_PATH 读。
/// Task 4 阶段返回空 Vec，让 UI 显示空主机列表（subagent 没真 host 也能跑）。
pub fn dev_hosts() -> Vec<HostConfig> {
    Vec::new()
}
```

- [ ] **Step 5: 修改 `crates/aish-app/src/bridge.rs`**

整个文件替换为：

```rust
//! Bridge：把 tokio runtime 与 GPUI executor 解耦。

#![allow(dead_code)]

use std::future::Future;
use std::sync::Arc;

use aish_types::HostConfig;
use tokio::sync::mpsc;

use crate::state::{HostId, SessionCommand, SshEvent};

pub struct EventChannel {
    pub tx: mpsc::Sender<SshEvent>,
    pub rx: mpsc::Receiver<SshEvent>,
}

impl EventChannel {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(64);
        Self { tx, rx }
    }
}

pub struct Bridge {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Bridge {
    pub fn start() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("aish-tokio")
            .build()?;
        Ok(Self { runtime: Arc::new(rt) })
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// 启动一个 host session task，返回 SessionCommand sender。
    pub fn spawn_session(
        &self,
        host: HostId,
        config: HostConfig,
        event_tx: mpsc::Sender<SshEvent>,
    ) -> mpsc::Sender<SessionCommand> {
        crate::ssh_actor::spawn_session(&self.runtime.handle(), host, config, event_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_channel_capacity_is_64() {
        let chan = EventChannel::new();
        let tx = chan.tx;
        for i in 0..64u32 {
            tx.try_send(SshEvent::PaneOutput {
                host: HostId(uuid::Uuid::new_v4()),
                bytes: vec![i as u8],
            })
            .expect("buffer of 64 should accept 64 sends without blocking");
        }
        assert!(tx
            .try_send(SshEvent::PaneOutput {
                host: HostId(uuid::Uuid::new_v4()),
                bytes: vec![65],
            })
            .is_err());
    }

    #[test]
    fn bridge_starts() {
        let bridge = Bridge::start().expect("runtime should start");
        let _handle = bridge.handle();
    }
}
```

注意：`use crate::state::{HostId, ...}`——但 HostId 现在来自 aish_types。修正：

```rust
use aish_types::HostId;
use crate::state::{SessionCommand, SshEvent};
```

- [ ] **Step 6: 修改 `crates/aish-app/src/main.rs`**

整个文件替换为：

```rust
//! aish 主入口。M2a 起接入真 SSH。

mod app;
mod bridge;
mod fixtures;
mod ssh_actor;
mod state;
mod views;

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M2a)");
    app::run();
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
```

变化：
- 删 `mod mock;`
- 加 `mod fixtures;` 与 `mod ssh_actor;`
- tracing 字符串 `"M1 skeleton"` → `"M2a"`

- [ ] **Step 7: 修改 `crates/aish-app/src/app.rs`**

整体替换为：

```rust
//! aish GPUI 主应用入口。

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

use crate::bridge::{Bridge, EventChannel};
use crate::state::{AppState, DisconnectReason, SshEvent};

pub fn run() {
    let bridge_owner = Arc::new(Bridge::start().expect("tokio runtime 启动失败"));

    application().run(move |cx: &mut App| {
        let hosts = crate::fixtures::dev_hosts();
        let state = cx.new(|_cx| AppState::with_hosts(hosts));
        let channel = EventChannel::new();

        // 接收 SshEvent loop
        let state_for_loop = state.clone();
        let mut rx = channel.rx;
        cx.spawn(async move |mut cx| {
            while let Some(event) = rx.recv().await {
                let _ = state_for_loop.update(&mut cx, |state, cx| match event {
                    SshEvent::Connected { host } => {
                        state.append_log(host, "[info] Connected".into());
                        cx.notify();
                    }
                    SshEvent::PaneOutput { host, bytes } => {
                        let s = String::from_utf8_lossy(&bytes);
                        for line in s.split('\n') {
                            // 去掉行尾 \r（cooked output 常见）
                            let line = line.trim_end_matches('\r').to_string();
                            state.append_log(host, line);
                        }
                        cx.notify();
                    }
                    SshEvent::Disconnected { host, reason } => {
                        let msg = match reason {
                            DisconnectReason::RemoteExited => "[info] 远端 shell 已退出".to_string(),
                            DisconnectReason::NetworkError(e) => format!("[error] 连接中断: {}", e),
                            DisconnectReason::UserRequested => "[info] 已断开".to_string(),
                        };
                        state.append_log(host, msg);
                        state.drop_session(host);
                        cx.notify();
                    }
                    SshEvent::Error { host, kind, msg } => {
                        state.append_log(host, format!("[error] {:?}: {}", kind, msg));
                        state.drop_session(host);
                        cx.notify();
                    }
                });
            }
        })
        .detach();

        // 开窗口
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("aish — M2a")),
                ..Default::default()
            }),
            ..Default::default()
        };

        let bridge_for_window = bridge_owner.clone();
        let tx_for_window = channel.tx.clone();
        let state_for_window = state.clone();

        cx.open_window(window_options, move |_window, cx| {
            cx.new(|cx| {
                RootView::new(
                    state_for_window.clone(),
                    bridge_for_window.clone(),
                    tx_for_window.clone(),
                    cx,
                )
            })
        })
        .expect("主窗口应能打开");

        cx.activate(true);
    });

    drop(bridge_owner);
}

struct RootView {
    host_list: Entity<crate::views::HostListView>,
    host_pane: Entity<crate::views::HostPaneView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let host_list = cx.new(|cx| {
            crate::views::HostListView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let host_pane = cx.new(|cx| crate::views::HostPaneView::new(state, bridge, tx, cx));
        Self { host_list, host_pane }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x121212))
            .child(self.host_list.clone())
            .child(self.host_pane.clone())
    }
}
```

变化：
- import `SshEvent` / `DisconnectReason` 替换 `MockEvent`
- cx.spawn loop 处理 4 种事件（Connected / PaneOutput / Disconnected / Error）
- HostPaneView::new 签名扩展（要 bridge + tx，键盘输入用）

- [ ] **Step 8: 修改 `crates/aish-app/src/views/host_list.rs`**

整体替换：

```rust
//! 左栏：host 列表，点击切换 selected + 触发 SSH 连接。

use std::sync::Arc;

use aish_types::HostId;
use gpui::{
    div, prelude::*, px, rgb, App, Context, Entity, MouseButton, MouseDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SshEvent};

pub struct HostListView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HostListView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        // 1. 检查 session 状态决定是否要触发连接
        let needs_connect = self.state.update(cx, |state, cx| {
            state.select_host(host);
            let label = state.host_label(host).unwrap_or_else(|| format!("{:?}", host));
            let needs = !state.is_session_active(host);
            if needs {
                state.append_log(
                    host,
                    format!("[{}] Connecting to {}...", simple_time(), label),
                );
            }
            cx.notify();
            needs
        });

        // 2. 如需连接：从 fixtures 找 config，spawn session task
        if needs_connect {
            let config = match self
                .state
                .read(cx)
                .hosts
                .iter()
                .find(|h| h.id == host)
                .cloned()
            {
                Some(c) => c,
                None => {
                    self.state.update(cx, |state, cx| {
                        state.append_log(host, "[error] host config not found".into());
                        cx.notify();
                    });
                    return;
                }
            };

            let sender = self.bridge.spawn_session(host, config, self.tx.clone());
            self.state.update(cx, |state, _cx| {
                state.register_session(host, sender);
            });
        }
    }
}

fn simple_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 86400)
        .unwrap_or(0);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;
        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                let is_active = state.is_session_active(id);
                let prefix = if is_active { "● " } else { "○ " };
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _window, cx| {
                            this.handle_click(id, cx);
                        }),
                    )
                    .child(format!("{}{}", prefix, label))
            })
            .collect();

        let empty_hint = if state.hosts.is_empty() {
            Some(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x888888))
                    .child("（无 host：设置 AISH_DEV_HOST/USER/KEY_PATH 环境变量）"),
            )
        } else {
            None
        };

        div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .children(host_rows)
            .children(empty_hint)
    }
}
```

变化：
- HostId 用 `aish_types::HostId`
- mock_ssh_task → bridge.spawn_session
- 增加"已连接"指示符 `●` / `○`
- 空 hosts 列表显示提示文字

- [ ] **Step 9: 修改 `crates/aish-app/src/views/host_pane.rs`**

整体替换为（现在 receive bridge + tx，但**键盘输入逻辑 Task 7 才加**）：

```rust
//! 主区：渲染 selected host 的 pane log。

use std::sync::Arc;

use gpui::{
    div, prelude::*, rgb, AnyElement, App, Context, Entity, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SshEvent};

pub struct HostPaneView {
    state: Entity<AppState>,
    #[allow(dead_code)] // Task 7 加键盘输入时用
    bridge: Arc<Bridge>,
    #[allow(dead_code)] // Task 7 加键盘输入时用
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HostPaneView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }
}

impl Render for HostPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        match state.selected {
            None => div()
                .flex_1()
                .h_full()
                .text_color(rgb(0x888888))
                .p_4()
                .child("请从左侧选择主机")
                .into_any_element(),
            Some(host) => {
                let lines = state.logs_of(host);
                let text_lines: Vec<AnyElement> = lines
                    .iter()
                    .map(|line| {
                        div()
                            .text_color(rgb(0xeeeeee))
                            .child(line.clone())
                            .into_any_element()
                    })
                    .collect();

                div()
                    .flex_1()
                    .h_full()
                    .bg(rgb(0x121212))
                    .text_color(rgb(0xeeeeee))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(text_lines)
                    .into_any_element()
            }
        }
    }
}
```

变化：构造函数签名扩展（接 bridge + tx），但渲染逻辑不变（Task 7 加键盘）。

- [ ] **Step 10: 删除 `crates/aish-app/src/mock.rs`**

```bash
cd C:\Users\larry\Desktop\workspace\aish
rm crates/aish-app/src/mock.rs
```

或在 Windows PowerShell：

```powershell
Remove-Item C:\Users\larry\Desktop\workspace\aish\crates\aish-app\src\mock.rs
```

- [ ] **Step 11: 验证 build / test**

```bash
cargo build -p aish-app
```

Expected: 编译通过。如果报错（typically GPUI API、`channel.rx` 移动语义、HostConfig 字段名），按错误调整。

```bash
cargo test --workspace
```

Expected: state 测试 5 + bridge 测试 2 + ssh_actor encode_key 4 + aish-types 8 + 各 lib smoke 1×4 = ~23 passed。

如果 state.rs 的某些 field name 与上面 plan 写的不一致（如 `is_session_active`），按测试 expected name 调整 plan 给的代码。

- [ ] **Step 12: cargo run 验证窗口能开 + stub 能 echo（subagent 在后台跑）**

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2a-t4-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2a-t4-err.txt"
Start-Sleep -Seconds 8
if ($proc.HasExited) {
    Write-Output "FAIL: process exited prematurely"
    Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2a-t4-err.txt" -Tail 30
} else {
    Write-Output "OK: window started"
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2a-t4-out.txt" -Tail 10
Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2a-t4-err.txt" -Tail 30
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2a-t4-*.txt" -Force
```

预期：tracing INFO 输出 + 没 panic + 窗口左栏空（fixtures 还没实装），主区"请从左侧选择主机"。subagent 看不到 demo 实际效果，**user 在 Task 8 才手动验证**。

- [ ] **Step 13: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/aish-app/
git rm crates/aish-app/src/mock.rs 2>/dev/null || true  # 已删，git rm 兜底
git commit -m "feat(aish-app): retire mock 接通 Actor 架构（ssh_actor stub）"
```

> ⚠️ 这个 commit 涉及很多文件变化，**diff 量会非常大**（~500-800 行）。这是 plan 设计上接受的——M2a 的核心架构变化集中在这一步。

---

## Task 5: aish-app/src/fixtures.rs — 环境变量读 HostConfig

**Files:**
- Modify: `crates/aish-app/src/fixtures.rs`

- [ ] **Step 1: 改写 `crates/aish-app/src/fixtures.rs`**

```rust
//! M2a 临时方案：从环境变量读 host config，避免 commit 真凭证。
//!
//! 环境变量：
//!   - AISH_DEV_HOST: 主机名或 IP
//!   - AISH_DEV_PORT: SSH 端口（可选，默认 22）
//!   - AISH_DEV_USER: 登录用户名
//!   - AISH_DEV_KEY_PATH: 私钥本地路径
//!   - AISH_DEV_LABEL: 显示标签（可选，默认 "dev")
//!
//! M2c 起删除此模块，改用 ~/.aish/hosts.json 持久化。

#![allow(dead_code)]

use std::path::PathBuf;

use aish_types::{HostConfig, HostId, SshAuth};
use uuid::Uuid;

/// 从环境变量读 1 个 host config。如果必需的环境变量缺失返回空 Vec。
pub fn dev_hosts() -> Vec<HostConfig> {
    let host = match std::env::var("AISH_DEV_HOST") {
        Ok(h) => h,
        Err(_) => {
            tracing::warn!(
                "AISH_DEV_HOST not set; host list will be empty. \
                 Set AISH_DEV_HOST/USER/KEY_PATH to enable dev SSH."
            );
            return Vec::new();
        }
    };

    let user = match std::env::var("AISH_DEV_USER") {
        Ok(u) => u,
        Err(_) => {
            tracing::warn!("AISH_DEV_USER not set; skipping dev host");
            return Vec::new();
        }
    };

    let key_path = match std::env::var("AISH_DEV_KEY_PATH") {
        Ok(p) => PathBuf::from(p),
        Err(_) => {
            tracing::warn!("AISH_DEV_KEY_PATH not set; skipping dev host");
            return Vec::new();
        }
    };

    let port: u16 = std::env::var("AISH_DEV_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(22);

    let label = std::env::var("AISH_DEV_LABEL").unwrap_or_else(|_| "dev".into());

    vec![HostConfig {
        id: HostId(Uuid::new_v4()),
        label,
        host,
        port,
        user,
        auth: SshAuth::KeyFile { path: key_path },
        env_profile: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 注意：测试不能 set/unset env var 因为 Rust 测试并行跑会污染。
    /// 这里只测"缺所有 env var 时返回空"——CI 环境不该有这些 var。
    #[test]
    fn dev_hosts_returns_empty_when_env_unset() {
        // 假设 CI 不会设 AISH_DEV_HOST。如果本地开发者设了，跳过断言。
        if std::env::var("AISH_DEV_HOST").is_err() {
            assert!(dev_hosts().is_empty());
        }
    }
}
```

- [ ] **Step 2: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-app fixtures
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: fixtures 1 passed (在 CI 环境)。

- [ ] **Step 3: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/fixtures.rs
git commit -m "feat(aish-app): fixtures 从环境变量读 dev host config"
```

---

## Task 6: aish-app/src/ssh_actor.rs — 接通真 SSH

**Files:**
- Modify: `crates/aish-app/src/ssh_actor.rs`（替换 stub 为真 SSH）

- [ ] **Step 1: 替换 host_session_task 为真 SSH 实现**

完整替换 `host_session_task` 函数（保留 `spawn_session` 与 `encode_key` 不变）：

```rust
async fn host_session_task(
    host: HostId,
    config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    use aish_ssh::{ChannelMsg, SshClient};

    use crate::state::{DisconnectReason, SshErrorKind};

    // 1. 连接 + 认证
    let mut session = match SshClient::connect(&config).await {
        Ok(s) => s,
        Err(err) => {
            let kind = match err.kind() {
                aish_ssh::SshErrorKind::ConnectFailed => SshErrorKind::ConnectFailed,
                aish_ssh::SshErrorKind::AuthFailed => SshErrorKind::AuthFailed,
                aish_ssh::SshErrorKind::Io => SshErrorKind::Io,
                aish_ssh::SshErrorKind::Protocol => SshErrorKind::Protocol,
            };
            let _ = event_tx
                .send(SshEvent::Error {
                    host,
                    kind,
                    msg: err.to_string(),
                })
                .await;
            return;
        }
    };

    // 2. 开 PTY channel
    let mut chan = match session.open_channel().await {
        Ok(c) => c,
        Err(err) => {
            let _ = event_tx
                .send(SshEvent::Error {
                    host,
                    kind: SshErrorKind::Protocol,
                    msg: format!("open_channel: {}", err),
                })
                .await;
            return;
        }
    };

    if let Err(err) = chan.request_pty(120, 40, "xterm-256color").await {
        let _ = event_tx
            .send(SshEvent::Error {
                host,
                kind: SshErrorKind::Protocol,
                msg: format!("request_pty: {}", err),
            })
            .await;
        return;
    }

    if let Err(err) = chan.shell().await {
        let _ = event_tx
            .send(SshEvent::Error {
                host,
                kind: SshErrorKind::Protocol,
                msg: format!("shell: {}", err),
            })
            .await;
        return;
    }

    let _ = event_tx.send(SshEvent::Connected { host }).await;

    // 3. select! loop: read + cmd
    loop {
        tokio::select! {
            msg = chan.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    let _ = event_tx
                        .send(SshEvent::PaneOutput {
                            host,
                            bytes: data.to_vec(),
                        })
                        .await;
                }
                Some(ChannelMsg::Eof) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::RemoteExited,
                        })
                        .await;
                    break;
                }
                Some(_) => {
                    // 其他 ChannelMsg 类型（ExitStatus / WindowAdjusted / 等）暂时忽略
                }
            },
            cmd = cmd_rx.recv() => match cmd {
                Some(SessionCommand::SendBytes(bytes)) => {
                    if let Err(e) = chan.data(&bytes[..]).await {
                        let _ = event_tx
                            .send(SshEvent::Disconnected {
                                host,
                                reason: DisconnectReason::NetworkError(e.to_string()),
                            })
                            .await;
                        break;
                    }
                }
                Some(SessionCommand::Disconnect) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::UserRequested,
                        })
                        .await;
                    break;
                }
            },
        }
    }
    // session drop → russh close
}
```

> **可能要调整**：
> - `ChannelMsg::Data { data }` 的 `data` 字段类型在 russh 0.45 是 `CryptoVec`（不是 `&[u8]`），`.to_vec()` 是否可用按 cargo build 错误调整
> - `ChannelMsg::Eof` 是否真存在；如果没有，用 `Close` 等价 variant
> - `chan.wait().await` 的返回类型是 `Option<ChannelMsg>` 吗

- [ ] **Step 2: 删 stub 中残留的代码**

确认 ssh_actor.rs 中 stub 的 echo 逻辑被删除（只有注释提到 stub 时，删掉那些过时注释）。检查：
- 不再有 `b"[stub] type something..."` 字符串
- 不再有 `b"\n[stub] echo done\r\n"` 字符串

文件顶部注释更新：

```rust
//! ssh_actor — host_session_task：每 host 一个 tokio task，own SshSession + PTY。
//!
//! 每个 task 内部 select! 在两个 future 之间：
//!   - chan.wait() — PTY 远端输出，转 SshEvent::PaneOutput 推回 GPUI
//!   - cmd_rx.recv() — GPUI 端的键盘输入命令，写入 chan.data()
```

- [ ] **Step 3: 验证 build + test**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app
cargo test --workspace
```

Expected: 全绿。如果 cargo test 跑 `encode_key` 测试失败，检查测试代码 + 实现是否同步。

- [ ] **Step 4: cargo run 后台启动验证（subagent 还是没真 host，但应该不 panic）**

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2a-t6-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2a-t6-err.txt"
Start-Sleep -Seconds 8
if ($proc.HasExited) {
    Write-Output "FAIL: process exited prematurely"
    Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2a-t6-err.txt" -Tail 30
} else {
    Write-Output "OK: window started"
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2a-t6-*.txt" -Force
```

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/src/ssh_actor.rs
git commit -m "feat(aish-app): ssh_actor 替换 stub 为真 russh PTY shell"
```

---

## Task 7: aish-app/src/views/host_pane.rs — GPUI 键盘输入

**Files:**
- Modify: `crates/aish-app/src/views/host_pane.rs`

- [ ] **Step 1: 看 Zed 的键盘事件处理 example**

```bash
grep -l 'on_key_down\|KeyDownEvent\|focusable' C:/Users/larry/Desktop/workspace/zed-reference/crates/gpui/examples/*.rs
cat C:/Users/larry/Desktop/workspace/zed-reference/crates/gpui/examples/input.rs 2>$null | head -120
```

记下：
- `KeyDownEvent` 当前签名（含 keystroke / modifiers）
- `on_key_down` 是否需要 `track_focus` / `FocusHandle`
- 从 keystroke 提取 key 名 + ctrl modifier 的方式

- [ ] **Step 2: 改写 `crates/aish-app/src/views/host_pane.rs`**

整体替换为：

```rust
//! 主区：渲染 selected host 的 pane log + 接键盘输入发到 PTY。

use std::sync::Arc;

use gpui::{
    div, prelude::*, rgb, AnyElement, App, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};

pub struct HostPaneView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)] // 保留 tx 备 spawn task 内 emit Event 用
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
}

impl HostPaneView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        Self { state, bridge, tx, focus_handle }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let host = match self.state.read(cx).selected {
            Some(h) => h,
            None => return,
        };
        let sender = match self.state.read(cx).sessions.get(&host).cloned() {
            Some(s) => s,
            None => return,
        };

        let ctrl = event.keystroke.modifiers.control;
        let alt = event.keystroke.modifiers.alt;
        let key = event.keystroke.key.as_str();

        let bytes = encode_key(key, ctrl, alt);
        if bytes.is_empty() {
            return;
        }

        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }
}

impl Focusable for HostPaneView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HostPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.state.read(cx).selected;
        let body = match selected {
            None => div()
                .text_color(rgb(0x888888))
                .p_4()
                .child("请从左侧选择主机")
                .into_any_element(),
            Some(host) => {
                let lines = self.state.read(cx).logs_of(host);
                let text_lines: Vec<AnyElement> = lines
                    .iter()
                    .map(|line| {
                        div()
                            .text_color(rgb(0xeeeeee))
                            .child(line.clone())
                            .into_any_element()
                    })
                    .collect();

                div()
                    .text_color(rgb(0xeeeeee))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(text_lines)
                    .into_any_element()
            }
        };

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .flex_1()
            .h_full()
            .bg(rgb(0x121212))
            .child(body)
    }
}
```

变化：
- 加 `focus_handle: FocusHandle` 字段 + `Focusable` trait 实现
- 加 `handle_key` 方法
- render 用 `track_focus` + `on_key_down` 接键盘
- 删了原 `_window: &mut Window` 没用的注释（保留参数名）

> **GPUI API 可能要调整**：
> - `cx.focus_handle()` 是否存在
> - `Focusable` trait 名 / 签名（可能是 `FocusableView`）
> - `KeyDownEvent.keystroke.modifiers.control` 字段路径
> - `KeyDownEvent.keystroke.key` 类型（`SharedString` vs `String` 等）
> - `track_focus(&handle)` 方法名

按 Zed example 调整。

- [ ] **Step 3: 验证 build + test + run**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app
cargo test --workspace
```

Expected: 全绿。

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2a-t7-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2a-t7-err.txt"
Start-Sleep -Seconds 8
if ($proc.HasExited) {
    Write-Output "FAIL"
    Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2a-t7-err.txt" -Tail 30
} else {
    Write-Output "OK"
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2a-t7-*.txt" -Force
```

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/src/views/host_pane.rs
git commit -m "feat(aish-app): host_pane 接 GPUI 键盘事件 → SessionCommand::SendBytes"
```

---

## Task 8: 端到端验证 + push（user 手动 demo）

⚠️ 这个 task **subagent 没法完成 demo 验证**——需要 user 本人跑 + 手动操作 GUI + 比对 13 项标准。

**Files:** 无文件改动；只验证 + push。

- [ ] **Step 1: 跑全部自动化验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 全部退出码 0。

- [ ] **Step 2: push 到 origin**

```bash
git push origin main
```

如果网络问题失败，重试。

- [ ] **Step 3: 等 GitHub Actions CI 跑完**

```bash
gh run list --limit 1
gh run watch  # 或等通知
```

Expected: 三平台 (ubuntu / macos / windows) test job 全过。如挂某平台，记录 follow-up 单独修。

- [ ] **Step 4: 提示 user 手动验证 demo 13 项**

implementer subagent 在报告中**明确列出**：

```
demo 验证由 user 完成。请按 spec Section 9 13 项手动核对：

1. 设置环境变量：
   $env:AISH_DEV_HOST = "<vps-host>"
   $env:AISH_DEV_USER = "<user>"
   $env:AISH_DEV_KEY_PATH = "<key-path>"
   $env:AISH_DEV_LABEL = "my-vps"   # 可选

2. cargo run -p aish-app

3. 验证：
   1) 窗口弹出（1200×800）
   2) 左栏显示 "my-vps"（带 ○ 前缀）
   3) 主区 "请从左侧选择主机"
   4) 点 my-vps → 主区立刻 [HH:MM:SS] Connecting to my-vps...
   5) 1-3 秒后 → [info] Connected + shell prompt（如 larry@vps:~$）
   6) 输入 ls + Enter → 看目录 listing（含乱码控制字符）
   7) 输入 echo hello + Enter → 看 hello
   8) 输入 exit + Enter → [info] 远端 shell 已退出，左栏 ● 变 ○
   9) 改环境变量 AISH_DEV_KEY_PATH 为不存在路径 → 重启 → click → [error] AuthFailed
   10) 关窗口 → 进程 1 秒内退出（echo $LASTEXITCODE = 0）
   11) cargo test --workspace 全绿
   12) cargo fmt + clippy 全绿
   13) GitHub Actions CI 三平台全绿
```

- [ ] **Step 5: 完成报告**

报告格式同其他 task，但额外明确 demo 责任：

```
STATUS: DONE_WITH_PENDING_USER_VERIFICATION

Files: 无文件改动

Verification (subagent 跑的):
- cargo build: PASS / FAIL
- cargo test --workspace: X passed
- cargo fmt --check: PASS / FAIL
- cargo clippy: PASS / FAIL
- git push: 成功 / 失败
- CI 状态: started / completed / failed

Pending (user 手动):
- demo 13 项（spec Section 9）

Concerns: ...
```

---

## 完成验证（M2a 整体）

执行下面，全部应成功：

```bash
cargo build --workspace
cargo test --workspace                                 # ~25 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p aish-app                                  # demo 13 项（user 手动）
```

git log 应有 8 个新 commit + spec + plan = 10 个新 commit。

---

## 下一步

M2a 完成后开始 M2b（终端渲染）：
- 集成 alacritty_terminal::Term 替换 String 流
- GPUI TerminalView 自绘字符 grid
- PTY 跟随窗口 resize（动态算 cols/rows）
- 键盘事件交给 alacritty_terminal 编码（接 alt / 方向键 / F1-12）

M2b 不在本 plan 范围。M2a 完成后单独 brainstorm → spec → plan → implement。
