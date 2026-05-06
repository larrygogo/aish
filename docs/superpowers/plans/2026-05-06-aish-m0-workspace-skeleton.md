# aish M0 — Workspace 骨架实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 aish 项目的 Cargo workspace 骨架（6 个 crate）、配置 fmt/clippy/test 三件套、落地三平台 GitHub Actions CI、把 spec 里 8 条 ADR 文档化。完成后 `cargo build && cargo test && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` 全绿。

**Architecture:** 标准 Cargo workspace + resolver v2，`[workspace.dependencies]` 集中管理共享依赖版本。`aish-types` 实现真实的共享类型（HostId、SessionId 等），其余 5 个 crate 在本里程碑只是骨架（lib.rs + dummy smoke test），实际接口实现推迟到 M1-M5。CI 用 GitHub Actions 三平台矩阵（ubuntu-latest / macos-latest / windows-latest）。

**Tech Stack:** Rust stable 1.80+, Cargo workspace, GitHub Actions, ADR markdown 模板（Michael Nygard 风格）

**前置条件:** 项目目录 `C:\Users\larry\Desktop\workspace\aish` 已存在并已 `git init`，已有 `docs/superpowers/specs/2026-05-06-aish-ssh-design.md` 一个 commit。

---

## File Structure（M0 完成时的目标状态）

```
aish/
├── Cargo.toml                                    # workspace root
├── rust-toolchain.toml
├── rustfmt.toml
├── .gitignore
├── README.md
├── LICENSE
├── .github/workflows/ci.yml
├── crates/
│   ├── aish-types/{Cargo.toml, src/lib.rs}       # 真实实现：HostId/SessionId/WindowId/PaneId/ProfileId/HostConfig/SshAuth
│   ├── aish-ssh/{Cargo.toml, src/lib.rs}         # 骨架 + smoke test
│   ├── aish-tmux/{Cargo.toml, src/lib.rs}        # 骨架 + smoke test
│   ├── aish-sftp/{Cargo.toml, src/lib.rs}        # 骨架 + smoke test
│   ├── aish-secrets/{Cargo.toml, src/lib.rs}     # 骨架 + smoke test
│   └── aish-app/{Cargo.toml, src/main.rs}        # binary：tracing-subscriber + println
└── docs/
    ├── superpowers/{specs/, plans/}              # 已有 spec + 本 plan
    └── adr/                                      # 9 个 ADR 文件（元 ADR + 8 条决策）
```

---

## Task 1: 项目根元数据（.gitignore / rust-toolchain / rustfmt / README / LICENSE）

**Files:**
- Create: `.gitignore`
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`
- Create: `README.md`
- Create: `LICENSE`

- [ ] **Step 1: 创建 `.gitignore`**

```gitignore
# Rust
/target
**/*.rs.bk
Cargo.lock.bak

# IDE
.vscode/
.idea/
*.iml
*.sublime-*

# OS
.DS_Store
Thumbs.db

# aish 运行时数据（开发期可能在项目目录生成）
.aish-local/

# Logs / Crash reports
*.log
crashes/
```

- [ ] **Step 2: 创建 `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 3: 创建 `rustfmt.toml`**

```toml
edition = "2021"
max_width = 100
use_field_init_shorthand = true
use_try_shorthand = true
```

- [ ] **Step 4: 创建 `README.md`**

```markdown
# aish

AI 友好的跨平台 SSH 桌面客户端。专为远程使用 AI CLI 工具（Claude Code、Codex、Aider 等）优化。

**状态：** 早期开发中（M0：workspace 骨架）。

## 核心特性（Phase 1 目标）

- **tmux 可视化管理**：连上服务器后自动列出已有 session/window/pane，点击即可切换、关闭、新建，无需记快捷键
- **多模态附件支持**：拖拽本地文件到窗口 → 自动 SFTP 上传 → 路径回填光标，让远端 AI CLI 直接拿到文件
- **安全的远程 env 注入**：API key 集中存在本地 OS keyring，连接时按 profile 注入到远端 shell，远端文件系统不留凭证

## 技术栈

- 语言：Rust（stable 1.80+）
- GUI：[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- 终端：[alacritty_terminal](https://github.com/alacritty/alacritty/tree/master/alacritty_terminal)
- SSH：[russh](https://github.com/Eugeny/russh)
- tmux 集成：control mode（`tmux -CC`）

详见 [docs/superpowers/specs/2026-05-06-aish-ssh-design.md](docs/superpowers/specs/2026-05-06-aish-ssh-design.md)。

## 构建

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## 平台支持

macOS / Windows / Linux（Linux GPUI 稳定性是已知短板，主推 macOS / Windows）

## 许可证

MIT
```

- [ ] **Step 5: 创建 `LICENSE`（MIT）**

```
MIT License

Copyright (c) 2026 larry

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 6: 验证 + commit**

```bash
git add .gitignore rust-toolchain.toml rustfmt.toml README.md LICENSE
git status   # 期望：5 个新文件 staged
git commit -m "chore: 初始化项目元数据（gitignore / toolchain / rustfmt / README / LICENSE）"
```

---

## Task 2: Workspace Cargo.toml + 6 个 crate 骨架

**Files:**
- Create: `Cargo.toml`
- Create: `crates/aish-types/Cargo.toml`、`crates/aish-types/src/lib.rs`
- Create: `crates/aish-ssh/Cargo.toml`、`crates/aish-ssh/src/lib.rs`
- Create: `crates/aish-tmux/Cargo.toml`、`crates/aish-tmux/src/lib.rs`
- Create: `crates/aish-sftp/Cargo.toml`、`crates/aish-sftp/src/lib.rs`
- Create: `crates/aish-secrets/Cargo.toml`、`crates/aish-secrets/src/lib.rs`
- Create: `crates/aish-app/Cargo.toml`、`crates/aish-app/src/main.rs`

- [ ] **Step 1: 创建 workspace 根 `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/aish-types",
    "crates/aish-ssh",
    "crates/aish-tmux",
    "crates/aish-sftp",
    "crates/aish-secrets",
    "crates/aish-app",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["larry"]
repository = "https://github.com/larrygogo/aish"
rust-version = "1.80"

[workspace.dependencies]
# 共享版本
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "io-util", "sync", "time", "fs"] }
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
async-trait = "0.1"
bytes = "1.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.10", features = ["v4", "serde"] }

# 本地 crate
aish-types = { path = "crates/aish-types" }
aish-ssh = { path = "crates/aish-ssh" }
aish-tmux = { path = "crates/aish-tmux" }
aish-sftp = { path = "crates/aish-sftp" }
aish-secrets = { path = "crates/aish-secrets" }

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1

[profile.dev]
opt-level = 0
debug = true
```

- [ ] **Step 2: 创建 `crates/aish-types/Cargo.toml`**

```toml
[package]
name = "aish-types"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish 共享类型定义（零业务逻辑）"

[dependencies]
serde = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 3: 创建 `crates/aish-types/src/lib.rs`（最小骨架）**

```rust
//! aish 共享类型定义。Task 3 会在此基础上扩展真实类型。

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: 创建 `crates/aish-ssh/Cargo.toml` 与 `src/lib.rs`**

`crates/aish-ssh/Cargo.toml`:

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

`crates/aish-ssh/src/lib.rs`:

```rust
//! aish-ssh — SSH 连接层。M0 仅骨架，M2 起接入 russh。

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 5: 创建 `crates/aish-tmux/Cargo.toml` 与 `src/lib.rs`**

`crates/aish-tmux/Cargo.toml`:

```toml
[package]
name = "aish-tmux"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish tmux control mode 协议层（M3 实现）"

[dependencies]
aish-types = { workspace = true }
aish-ssh = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
bytes = { workspace = true }
```

`crates/aish-tmux/src/lib.rs`:

```rust
//! aish-tmux — tmux control mode (`tmux -CC`) 协议层。M0 仅骨架，M3 实现。

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 6: 创建 `crates/aish-sftp/Cargo.toml` 与 `src/lib.rs`**

`crates/aish-sftp/Cargo.toml`:

```toml
[package]
name = "aish-sftp"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish SFTP 附件传输层（M4 实现）"

[dependencies]
aish-types = { workspace = true }
aish-ssh = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
bytes = { workspace = true }
```

`crates/aish-sftp/src/lib.rs`:

```rust
//! aish-sftp — SFTP 附件双向传输。M0 仅骨架，M4 实现。

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 7: 创建 `crates/aish-secrets/Cargo.toml` 与 `src/lib.rs`**

`crates/aish-secrets/Cargo.toml`:

```toml
[package]
name = "aish-secrets"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish 跨平台 keyring 凭证存储（M5 实现）"

[dependencies]
aish-types = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

`crates/aish-secrets/src/lib.rs`:

```rust
//! aish-secrets — 跨平台 keyring 凭证存储。M0 仅骨架，M5 实现。

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 8: 创建 `crates/aish-app/Cargo.toml` 与 `src/main.rs`**

`crates/aish-app/Cargo.toml`:

```toml
[package]
name = "aish-app"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish 主应用（GPUI 在 M1 接入）"

[[bin]]
name = "aish"
path = "src/main.rs"

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
```

`crates/aish-app/src/main.rs`:

```rust
//! aish 主入口。M0 阶段只是 hello + tracing 初始化；M1 起接入 GPUI。

fn main() {
    println!("aish skeleton — M0");
}
```

- [ ] **Step 9: 验证 `cargo build`**

```bash
cargo build --workspace
```

Expected: 全部 6 个 crate 编译通过，无 warning。

- [ ] **Step 10: 验证 `cargo test`**

```bash
cargo test --workspace
```

Expected: 6 个 crate 各跑 1 个 smoke test，全部通过（共 6 passed）。

- [ ] **Step 11: 验证 `cargo fmt --check`**

```bash
cargo fmt --all -- --check
```

Expected: 退出码 0，无 diff 输出（如有 diff，运行 `cargo fmt --all` 修正后重跑）。

- [ ] **Step 12: 验证 `cargo clippy`**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 退出码 0，无 warning（如果有，按提示修正；常见的是 dead_code，可加 `#[allow(dead_code)]` 在 lib.rs 顶部，但 M0 骨架不应触发）。

- [ ] **Step 13: commit**

```bash
git add Cargo.toml crates/
git commit -m "feat: 创建 workspace 与 6 个 crate 骨架（M0）"
```

---

## Task 3: aish-types — 实现共享类型（TDD）

**Files:**
- Modify: `crates/aish-types/src/lib.rs`

**说明：** 用 TDD 节奏，每个类型先写测试 → 验证失败 → 写实现 → 验证通过。所有类型最终汇总到一个 `lib.rs`。

- [ ] **Step 1: 写 HostId 测试（先 fail）**

替换 `crates/aish-types/src/lib.rs` 内容为：

```rust
//! aish 共享类型定义。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 主机唯一标识（UUID v4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostId(pub Uuid);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_id_roundtrip() {
        let id = HostId(Uuid::new_v4());
        let json = serde_json::to_string(&id).unwrap();
        let parsed: HostId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }
}
```

- [ ] **Step 2: 验证 HostId 测试通过**

```bash
cargo test -p aish-types host_id_roundtrip
```

Expected: `1 passed`。

- [ ] **Step 3: 加 `HostId::new()` 构造函数 + Display 实现 + 测试**

在 `lib.rs` 中 `HostId` 定义之后追加：

```rust
impl HostId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for HostId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
```

在 `mod tests` 中追加：

```rust
    #[test]
    fn host_id_new_unique() {
        let a = HostId::new();
        let b = HostId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn host_id_display_is_uuid() {
        let id = HostId::new();
        assert_eq!(id.to_string(), id.0.to_string());
    }
```

- [ ] **Step 4: 验证**

```bash
cargo test -p aish-types
```

Expected: 3 passed。

- [ ] **Step 5: 写 SessionId（tmux session 名，String newtype）**

在 `HostId` 实现块之后追加：

```rust
/// tmux session 名（字符串 newtype，避免与普通 String 混淆）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
```

在 `mod tests` 中追加：

```rust
    #[test]
    fn session_id_basic() {
        let id = SessionId::new("main");
        assert_eq!(id.as_str(), "main");
        assert_eq!(id.to_string(), "main");
    }
```

- [ ] **Step 6: 验证**

```bash
cargo test -p aish-types
```

Expected: 4 passed。

- [ ] **Step 7: 写 WindowId / PaneId（tmux 内部 id，u32 newtype）**

追加：

```rust
/// tmux window 内部 id（对应 tmux 的 `@<n>` 形式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WindowId(pub u32);

/// tmux pane 内部 id（对应 tmux 的 `%<n>` 形式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u32);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{}", self.0)
    }
}
```

在 `mod tests` 追加：

```rust
    #[test]
    fn window_pane_display_matches_tmux_format() {
        assert_eq!(WindowId(3).to_string(), "@3");
        assert_eq!(PaneId(7).to_string(), "%7");
    }
```

- [ ] **Step 8: 验证**

```bash
cargo test -p aish-types
```

Expected: 5 passed。

- [ ] **Step 9: 写 ProfileId（env profile 名）**

追加：

```rust
/// env 注入 profile 的标识（用户给 profile 起的名字）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
```

在 `mod tests` 追加：

```rust
    #[test]
    fn profile_id_basic() {
        let id = ProfileId::new("default");
        assert_eq!(id.as_str(), "default");
    }
```

- [ ] **Step 10: 验证**

```bash
cargo test -p aish-types
```

Expected: 6 passed。

- [ ] **Step 11: 写 SshAuth + HostConfig**

在文件顶部 `use` 块追加：

```rust
use std::path::PathBuf;
```

在 `lib.rs` 末尾（`#[cfg(test)]` 之前）追加：

```rust
/// SSH 认证方式。Password 不持久化，仅用于"输入即用即丢"。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SshAuth {
    /// 密码模式：连接时由 UI 临时弹窗，绝不存储。
    Password,
    /// 私钥文件：只存路径，不读内容。
    KeyFile { path: PathBuf },
    /// 委托给 ssh-agent / Pageant / 1Password Agent。
    Agent,
}

/// 主机配置，序列化到 `~/.aish/hosts.json`。**不含任何凭证**。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostConfig {
    pub id: HostId,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    pub env_profile: Option<ProfileId>,
}
```

在 `mod tests` 追加：

```rust
    #[test]
    fn host_config_roundtrip() {
        let cfg = HostConfig {
            id: HostId::new(),
            label: "my dev box".to_string(),
            host: "example.com".to_string(),
            port: 22,
            user: "larry".to_string(),
            auth: SshAuth::KeyFile {
                path: PathBuf::from("/home/larry/.ssh/id_ed25519"),
            },
            env_profile: Some(ProfileId::new("default")),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: HostConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn ssh_auth_variants_serialize_distinctly() {
        let pwd = serde_json::to_string(&SshAuth::Password).unwrap();
        let agent = serde_json::to_string(&SshAuth::Agent).unwrap();
        let key = serde_json::to_string(&SshAuth::KeyFile {
            path: PathBuf::from("/tmp/k"),
        })
        .unwrap();
        assert!(pwd.contains("password"));
        assert!(agent.contains("agent"));
        assert!(key.contains("key_file"));
        assert!(key.contains("/tmp/k"));
    }
```

- [ ] **Step 12: 验证**

```bash
cargo test -p aish-types
```

Expected: 8 passed。

- [ ] **Step 13: 全 workspace fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-types/
git commit -m "feat(aish-types): 实现 HostId/SessionId/WindowId/PaneId/ProfileId/SshAuth/HostConfig"
```

---

## Task 4: aish-app — main + tracing 初始化

**Files:**
- Modify: `crates/aish-app/src/main.rs`

- [ ] **Step 1: 改写 `main.rs`**

```rust
//! aish 主入口。M0 只做：tracing 初始化 + hello。M1 起接入 GPUI。

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M0 skeleton)");
    println!("aish skeleton — see docs/superpowers/specs/ for design");
}

/// 初始化全局 tracing 订阅器。RUST_LOG 环境变量可覆盖默认 INFO 级别。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
```

- [ ] **Step 2: 验证可运行**

```bash
cargo run -p aish-app
```

Expected stdout 至少包含：
- 一行 tracing 输出，类似 `... INFO aish: aish starting (M0 skeleton)`
- 一行 `aish skeleton — see docs/superpowers/specs/ for design`

- [ ] **Step 3: 验证 RUST_LOG 起作用**

```bash
# Windows PowerShell
$env:RUST_LOG="debug"; cargo run -p aish-app; Remove-Item env:RUST_LOG

# 或 bash
RUST_LOG=debug cargo run -p aish-app
```

Expected: tracing 行的级别变为 DEBUG（或至少不报错）。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-app/
git commit -m "feat(aish-app): 添加 tracing 初始化与 hello 入口"
```

---

## Task 5: GitHub Actions CI（fmt + clippy + 三平台 test）

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: 创建 `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings

jobs:
  fmt:
    name: rustfmt check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets -- -D warnings

  test:
    name: test (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace
```

- [ ] **Step 2: 本地预演 CI 三件套**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: 三条命令全部退出码 0。

- [ ] **Step 3: commit**

```bash
git add .github/
git commit -m "ci: 添加 fmt + clippy + 三平台 test workflow"
```

> **注：** 本地无法验证 GitHub Actions 是否真正绿，需 push 到 GitHub 后在 Actions 页面查看。如果 push 后 CI 失败，针对失败 job 修复后增量 commit；不要为了"把 CI 做绿"而禁用检查。

---

## Task 6: ADR 文档（元 ADR + 8 条决策）

**Files:**
- Create: `docs/adr/0001-record-architecture-decisions.md`
- Create: `docs/adr/0002-gui-framework-gpui.md`
- Create: `docs/adr/0003-terminal-alacritty-terminal.md`
- Create: `docs/adr/0004-ssh-russh.md`
- Create: `docs/adr/0005-tmux-control-mode.md`
- Create: `docs/adr/0006-tokio-gpui-bridge.md`
- Create: `docs/adr/0007-credential-storage-keyring.md`
- Create: `docs/adr/0008-env-injection-sendenv-export.md`
- Create: `docs/adr/0009-attachment-path-isolation.md`

**模板（Michael Nygard 风格）：每个 ADR 用 Status / Context / Decision / Consequences 四节。**

- [ ] **Step 1: 创建 `0001-record-architecture-decisions.md`**

```markdown
# 0001. 用 ADR 记录架构决策

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

aish 在设计阶段做了多个绑定项目长期方向的技术选择（GUI 框架、SSH 库、tmux 集成方式等）。如果这些理由只散落在 commit message 或聊天记录里，半年后回看会无法理解"为什么当初选了 X 而不是 Y"，更不可能让贡献者快速接住上下文。

## Decision

采用 [Michael Nygard 风格的 Architecture Decision Records](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)，每条决策一个 markdown 文件，存放于 `docs/adr/`，编号递增。每个 ADR 包含：

- **Status**：Proposed / Accepted / Deprecated / Superseded by ADR-N
- **Context**：决策背景，为什么需要做这个决策
- **Decision**：实际选择是什么
- **Consequences**：选择带来的好处与代价

## Consequences

- 好处：决策可追溯，新贡献者可以从 0001 开始顺序读取了解整个系统脉络
- 代价：每个重要决策需要花 5-10 分钟写文档；不重要的决策不应该走 ADR（避免噪音）
```

- [ ] **Step 2: 创建 `0002-gui-framework-gpui.md`**

```markdown
# 0002. GUI 框架选 GPUI（纯 Rust）

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

aish 是跨平台桌面应用，需要选定 GUI 框架。候选：

- **Tauri**（Rust 后端 + Web 前端）：生态最成熟，xterm.js 是终端事实标准，富文本能力强
- **GPUI**（Zed 编辑器同款）：纯 Rust，性能极强，富文本能力已被 Zed 验证
- **Iced**：纯 Rust，Elm 架构，社区活跃，但富文本能力中等
- **egui**：即时模式，做带状态的复杂 UI 不舒服
- **Slint**：DSL + Rust，社区较小

用户偏好"纯 Rust"路线，希望避免 webview 与 TS/Web 技术栈混入。

## Decision

选 **GPUI**。理由：

1. Zed 编辑器证明了 GPUI 能做高质量终端 + Markdown + diff（这正是 aish 的核心需求）
2. 纯 Rust，无 webview / 无 Node 工具链
3. Zed 是开源的，可以直接参考源码学习

## Consequences

**好处：**
- 单一二进制，启动快，资源占用低
- 与 alacritty_terminal、russh 等纯 Rust 生态无缝衔接
- 性能上限很高（GPU 渲染）

**代价：**
- 文档稀缺，需要看 Zed 源码学习，前期 2-4 周学习曲线很陡
- API 不稳定，跟着 Zed 升级
- Linux 稳定性是已知短板（Zed 自己也在改）
- 几乎没有 Zed 之外的成功案例，aish 是早期用户
```

- [ ] **Step 3: 创建 `0003-terminal-alacritty-terminal.md`**

```markdown
# 0003. 终端实现用 alacritty_terminal + GPUI 自绘

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0002

## Context

需要在 GPUI 里渲染 vt100 / xterm-256color 终端。候选：

- **alacritty_terminal**：Alacritty 终端模拟器抽出的终端状态机库，纯 Rust，标准实现
- **xterm.js**：行业事实标准，但需要 webview，与 0002 冲突
- **wezterm-term**：WezTerm 的实现，纯 Rust，但 API 与 WezTerm 强绑定
- **自研解析器**：vt100 / DEC private modes / OSC / sixel 等遗留协议太多，不现实

## Decision

用 **alacritty_terminal** 作为终端状态机（解析 vt100 流，维护 grid 缓冲），由 GPUI 订阅 `Term` 状态变更，自绘屏幕。

## Consequences

**好处：**
- 协议解析完全交给经过实战检验的实现
- alacritty_terminal 只持有 in-memory 状态，不直接持有 socket，易测试（喂字节流 → 比对 grid 状态）
- 与渲染层解耦，未来切渲染后端不影响协议层

**代价：**
- 需要自己实现 GPUI 与 alacritty_terminal 的胶水层（订阅变更、转换坐标系、处理键盘输入回填）
- alacritty_terminal 不带字体渲染、字符宽度计算等，需要 GPUI 层接住
```

- [ ] **Step 4: 创建 `0004-ssh-russh.md`**

```markdown
# 0004. SSH 客户端用 russh

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

需要 SSH 协议实现。候选：

- **russh**：纯 Rust，async-first，活跃维护
- **ssh2**：libssh2 的 Rust binding，C 依赖，跨平台编译麻烦
- **thrussh**：russh 的前身，已不再维护
- **OpenSSH 命令行 + PTY 包装**：依赖系统装了 ssh，且无法精细控制（如 SendEnv 协议）

## Decision

用 **russh**。

## Consequences

**好处：**
- 无 C 依赖，跨平台编译简单（特别是 Windows）
- async API 与 tokio 无缝集成
- 可精细控制 SSH 协议层（自定义 channel、SendEnv、agent forward 等）

**代价：**
- 比 OpenSSH 命令行包装方案多写一些胶水代码
- 协议覆盖度偶有空白（小众算法、特殊 KEX），需关注 issues
```

- [ ] **Step 5: 创建 `0005-tmux-control-mode.md`**

```markdown
# 0005. tmux 集成用 control mode（`tmux -CC`）

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0004

## Context

aish 的核心差异化是 tmux 可视化管理：连上服务器后自动列出 session/window/pane，UI 上点击即可 attach/切换/关闭。需要选定与 tmux 通信的方式。候选：

- **轮询 `tmux list-sessions / list-windows / list-panes`**：最简单，但需要不断轮询拿最新状态，且无法在状态变化的瞬间感知
- **tmux control mode（`tmux -CC`）**：tmux 主动推事件流，单 channel 可以承载所有 pane 的 IO 与控制命令
- **自研协议**：完全不可行

## Decision

用 **control mode**。客户端在远端运行 `tmux -CC new-session -A -s aish-default`，进入 control 模式后：

- tmux 立即推送当前所有 session/window/pane 的快照
- 之后任何状态变化（新增 pane、resize、pane 死亡等）会主动推 `%xxx` 事件行
- 所有 pane 输出通过 `%output %N <bytes>` 复用同一 channel
- 客户端发送 `send-keys`、`new-window`、`kill-pane` 等命令也走同一 channel

## Consequences

**好处：**
- 实时事件流，UI 状态总是与远端一致
- 一台服务器只占一个 SSH connection（不会爆 channel）
- iTerm2 已用此方案多年，可参考其实践

**代价：**
- 解析协议有边角情况（layout string 格式、Unicode pane 名、nested tmux 等）需要踩坑
- 需要 tmux >= 2.6（control mode v2）；老版本必须降级到 raw PTY shell
```

- [ ] **Step 6: 创建 `0006-tokio-gpui-bridge.md`**

```markdown
# 0006. GPUI 与 tokio runtime 共存，用 channel 桥接

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0002, 0004

## Context

GPUI 有自己的 executor（BackgroundExecutor / ForegroundExecutor）。但 russh 强依赖 tokio runtime（其 async I/O、定时器、TLS 等都跑在 tokio 上）。两者必须共存。候选：

- **完全跑在 GPUI executor 里**：要把 russh 改成 GPUI executor 兼容，工程量巨大且不可持续
- **完全跑在 tokio 里**：UI 线程必须是 GPUI 主线程，无法替代
- **两者共存，用 channel 桥接**：Zed 自己的做法

## Decision

`aish-app` 启动时在专属线程跑一个 `tokio::runtime::Builder::new_multi_thread().build()`。所有 async 调用通过 `runtime.spawn()` 提交，结果通过 `tokio::sync::oneshot` 或 `tokio::sync::mpsc` channel 回到 GPUI。GPUI Model 用 `cx.spawn(|this, mut cx| async { ... })` 接收 channel 并 update Model。

## Consequences

**好处：**
- 两个 executor 各司其职，互不干扰
- channel 隔离了 lifetime / Send 复杂度（避免在 GPUI 内部直接 await tokio future）
- Zed 已用此方案，是经过验证的模式

**代价：**
- 多一层 channel 间接，需要小心背压（unbounded channel 可能 OOM）
- Debug 时 stack trace 跨 runtime 边界，定位问题麻烦
```

- [ ] **Step 7: 创建 `0007-credential-storage-keyring.md`**

```markdown
# 0007. 凭证用 OS keyring 存储

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

需要存储两类敏感数据：
- API key（用于 env 注入到远端 shell）
- SSH 私钥（如果支持 inline 私钥模式）

候选：
- **OS keyring**（macOS Keychain / Windows Credential Manager / Linux Secret Service）
- **自研加密**（用 master password 加密本地文件）
- **明文 + 文件权限保护**

## Decision

用 OS keyring，通过 `keyring` crate 跨平台访问。**SSH 私钥不存内容，只存路径**（引用 `~/.ssh/id_ed25519` 等本地文件）；SSH 密码完全不存（仅"输入即用即丢"）。

## Consequences

**好处：**
- 复用 OS 安全模型（生物识别、登录态绑定等）
- 用户已经习惯这种凭证管理方式
- 跨平台统一抽象

**代价：**
- Linux Secret Service 在不同发行版可用性不一致（GNOME 默认有 keyring，最简化的桌面环境可能没有），需要 fallback 处理
- keyring crate 在某些 CI 环境无 keyring backend，测试需要 mock
```

- [ ] **Step 8: 创建 `0008-env-injection-sendenv-export.md`**

```markdown
# 0008. env 注入：SendEnv 优先 + export 降级

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0004, 0007

## Context

aish 的核心差异化之一：从本地 keyring 集中管理 API key（如 `ANTHROPIC_API_KEY`），连接 SSH 时注入到远端 shell，让远端 AI CLI 工具直接读到，且远端文件系统**不存储**这些凭证。两条路径：

- **SSH SendEnv 协议**：客户端发 `SSH_MSG_CHANNEL_REQUEST type=env`，远端 sshd 按 `AcceptEnv` 白名单匹配后注入。最安全，但依赖 sshd 配置。
- **连接后 export**：在 PTY 通道里发 `export KEY=value`。兼容性最好，但有泄露风险（命令历史、屏幕残留）。

## Decision

**首选 SendEnv，失败时自动降级到 export**。降级时：
- 命令前加空格 + 设置 `HISTCONTROL=ignorespace`，确保不进 shell history
- value 用 `shell-escape` crate 转义
- 注入完立刻 `clear` 抹屏
- UI 提示用户"此连接 env 注入走降级模式，建议在 sshd_config 中配置 AcceptEnv"

## Consequences

**好处：**
- 大部分场景享受 SendEnv 的安全性（不进 history、不可见）
- 不强制用户改远端 sshd 配置（降级路径兜底）
- 用户对降级有明确感知，可主动加固

**代价：**
- 降级路径仍有理论上的泄露窗口（虽然已尽可能缩小）
- 需要维护两套注入路径的代码与测试
```

- [ ] **Step 9: 创建 `0009-attachment-path-isolation.md`**

```markdown
# 0009. 附件路径强制隔离到 `/tmp/aish-attach/<connection_uuid>/`

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0004

## Context

aish 支持本地拖拽文件 → SFTP 上传到远端 → 路径自动注入光标位置。需要决定远端落盘路径策略。候选：

- **用户指定路径**：灵活但暴露注入风险（用户输入 `../etc/passwd` 之类）
- **用户 home 目录**：污染用户文件系统
- **强制 `/tmp/aish-attach/<connection_uuid>/`**：隔离、易清理

## Decision

强制路径格式：`/tmp/aish-attach/<connection_uuid>/<timestamp>-<sha8>-<filename>`。

- `aish-attach` 前缀避免与其他工具冲突
- 每次 SSH 连接独立 UUID 子目录，断连时由 RAII 触发清理（远端运行 `rm -rf` 该目录）
- 文件名做 sanitize（去除 `../`、控制字符）
- 客户端 API **不接受**用户指定 remote 路径

## Consequences

**好处：**
- 防止路径注入
- 易清理（连接断开自动清理整个 UUID 目录）
- 不污染用户文件系统

**代价：**
- 用户如果想"上传到指定目录"必须手动 `mv`（这是有意为之，附件是临时文件，需要永久落盘的场景应该走 SFTP 文件管理 UI，未来 Phase 2）
- `/tmp` 在某些发行版上是 tmpfs（内存），大文件可能 OOM；可配置 fallback 到 `~/.cache/aish/attach/`（远期）
```

- [ ] **Step 10: commit**

```bash
git add docs/adr/
git commit -m "docs: 添加 9 条 ADR（元 ADR + 8 条 M0 决策）"
```

---

## 完成验证

执行下面所有命令，全部应该成功（退出码 0）：

```bash
cargo build --workspace
cargo test --workspace                                         # 至少 8 + 5 = 13 passed (aish-types 8 + 其他 5 个 smoke)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p aish-app                                          # 输出 hello
```

git log 应该有 6 个新 commit（Task 1-6 各一个）：

```bash
git log --oneline
# 期望（顺序从新到旧）：
# docs: 添加 9 条 ADR ...
# ci: 添加 fmt + clippy + 三平台 test workflow
# feat(aish-app): 添加 tracing 初始化与 hello 入口
# feat(aish-types): 实现 HostId/SessionId/...
# feat: 创建 workspace 与 6 个 crate 骨架（M0）
# chore: 初始化项目元数据 ...
# docs: 添加 aish 初始设计文档    （之前 spec 阶段的 commit）
```

---

## 下一步

M0 完成后即开始 M1（GPUI 起步 + tokio 桥接）的 brainstorm/plan。M1 的具体任务依赖 GPUI 学习曲线的实际进展，因此**不要**在 M0 完成前预先编写 M1 plan。