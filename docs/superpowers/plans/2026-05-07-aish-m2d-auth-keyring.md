# aish M2d — Password 认证 + OS keyring 凭证存储 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 SSH 密码认证 + 把密码存到 OS keyring（不进 hosts.json），GUI 加 auth 类型切换 + 密码字段。

**Architecture:** aish-secrets 提供 `SecretStore` 跨平台 keyring API；aish-types::SshAuth::Password 加 `password` 字段（`#[serde(skip_serializing)]`，运行时持有，不进 hosts.json）；persistence.rs 保存时把 password 写 keyring，加载时**不**预读；ssh_actor 在 connect 前 lazy 从 keyring 取密码；host_form GUI 加 auth radio + 密码字段（mask + 👁 toggle）+ 编辑模式留空保留原密码。

**Tech Stack:** keyring crate 3.x (sync API, `apple-native`/`windows-native`/`sync-secret-service` features), serde, russh `authenticate_password`, GPUI。

**主线分支：** main（直接 push，不开 PR）。

---

## File Structure

| 文件 | 责任 |
|---|---|
| `crates/aish-secrets/Cargo.toml` | + keyring deps |
| `crates/aish-secrets/src/lib.rs` | SecretStore + SecretError，mock 测试初始化 |
| `crates/aish-types/src/lib.rs` | SshAuth::Password 加 password 字段 |
| `crates/aish-ssh/src/client.rs` | password 认证实现 |
| `crates/aish-app/Cargo.toml` | （已含 aish-secrets dep，确认） |
| `crates/aish-app/src/persistence.rs` | save_hosts 时写 keyring + delete_secret_for helper |
| `crates/aish-app/src/state.rs` | HostFormDraft 加 auth_kind/password/password_visible 字段 + into_config 适配 + remove_host 调 delete_secret_for |
| `crates/aish-app/src/ssh_actor.rs` | connect 前 SecretStore::get 填回 password |
| `crates/aish-app/src/views/host_form.rs` | auth radio + password input + 👁 toggle + FocusField 扩展 |

---

## Task 1: aish-secrets — keyring 集成 + SecretStore API

**Files:**
- Modify: `crates/aish-secrets/Cargo.toml`
- Modify: `crates/aish-secrets/src/lib.rs`

- [ ] **Step 1: 改写 `Cargo.toml`，加 keyring + dev-dep**

```toml
[package]
name = "aish-secrets"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish 跨平台 keyring 凭证存储（M2d 实现）"

[dependencies]
aish-types = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
keyring = { version = "3", default-features = false, features = ["apple-native", "windows-native", "sync-secret-service", "crypto-rust"] }

[dev-dependencies]
keyring = { version = "3", default-features = false, features = ["apple-native", "windows-native", "sync-secret-service", "crypto-rust", "std-secret-store"] }
```

注：`std-secret-store` 是 keyring 3.x 的 mock backend（in-memory）。如果 feature 名不对，implementer 用 `cargo doc -p keyring --open` 确认或在 keyring docs.rs 找正确名字。备选：`mock`。

- [ ] **Step 2: 改写 `src/lib.rs`，实现 SecretStore + SecretError + 测试初始化**

```rust
//! aish-secrets — 跨平台 keyring 凭证存储。
//!
//! service 固定 "aish"，username 用 HostId 的 UUID 字符串。
//! 调用方在 macOS Keychain Access.app / Windows 凭据管理器 搜 "aish" 能列出全部条目。

use aish_types::HostId;
use keyring::Entry;

/// 固定 service name；user 在 OS keyring 里看到的应用名。
const SERVICE: &str = "aish";

/// 跨平台 keyring 凭证存储。
///
/// 所有方法都是同步的（keyring crate 3.x 的 sync API）— 操作通常 < 10ms。
/// 调用方应当在异步上下文里用 `tokio::task::spawn_blocking` 或在 cx.spawn 内部调用。
pub struct SecretStore;

impl SecretStore {
    /// 把 password 存到 keyring，绑定到 host_id。
    /// 已存在则覆盖。
    pub fn set(host_id: HostId, password: &str) -> Result<(), SecretError> {
        let entry = Entry::new(SERVICE, &host_id.to_string())?;
        entry.set_password(password)?;
        Ok(())
    }

    /// 取出 host_id 对应的 password。
    /// 不存在返回 `SecretError::NoEntry`。
    pub fn get(host_id: HostId) -> Result<String, SecretError> {
        let entry = Entry::new(SERVICE, &host_id.to_string())?;
        match entry.get_password() {
            Ok(p) => Ok(p),
            Err(keyring::Error::NoEntry) => Err(SecretError::NoEntry),
            Err(e) => Err(SecretError::Keyring(e)),
        }
    }

    /// 删除 host_id 对应的 entry。
    /// 不存在视为成功（调用方期望 idempotent 删除）。
    pub fn delete(host_id: HostId) -> Result<(), SecretError> {
        let entry = Entry::new(SERVICE, &host_id.to_string())?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Keyring(e)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring 操作失败: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("条目不存在")]
    NoEntry,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    /// 全局一次性初始化 mock keyring backend。
    /// keyring::set_default_credential_builder 只能调用一次，所以用 Once 守卫。
    static INIT: Once = Once::new();

    fn init_mock() {
        INIT.call_once(|| {
            // keyring 3.x 的 mock builder API；如果 import 路径不对，
            // implementer 用 cargo doc -p keyring 找 mock module 的正确入口。
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    #[test]
    fn set_then_get_returns_value() {
        init_mock();
        let host = HostId::new();
        SecretStore::set(host, "p@ss123").unwrap();
        let got = SecretStore::get(host).unwrap();
        assert_eq!(got, "p@ss123");
    }

    #[test]
    fn get_nonexistent_returns_no_entry() {
        init_mock();
        let host = HostId::new(); // 全新 id，未 set
        let r = SecretStore::get(host);
        assert!(matches!(r, Err(SecretError::NoEntry)));
    }

    #[test]
    fn delete_then_get_returns_no_entry() {
        init_mock();
        let host = HostId::new();
        SecretStore::set(host, "x").unwrap();
        SecretStore::delete(host).unwrap();
        let r = SecretStore::get(host);
        assert!(matches!(r, Err(SecretError::NoEntry)));
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        init_mock();
        let host = HostId::new();
        // 没 set 直接 delete — 应当 Ok（idempotent）
        SecretStore::delete(host).unwrap();
    }

    #[test]
    fn set_overwrites_existing() {
        init_mock();
        let host = HostId::new();
        SecretStore::set(host, "old").unwrap();
        SecretStore::set(host, "new").unwrap();
        assert_eq!(SecretStore::get(host).unwrap(), "new");
    }
}
```

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-secrets 2>&1 | tail -5
cargo test -p aish-secrets 2>&1 | tail -10
cargo clippy -p aish-secrets --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: 5 个测试全部 passed，clippy 全绿。

⚠️ **如果 mock backend feature 名称不对**（编译失败 "feature `std-secret-store` not found"）：
1. 跑 `cargo search keyring` + 查 docs.rs/keyring 找 3.x 的 mock feature 名
2. 在 keyring 3.x，mock 通常通过 `keyring = { version = "3", default-features = false }`（无任何 native backend）+ `keyring::set_default_credential_builder(keyring::mock::default_credential_builder())` 启用
3. 如果 `keyring::mock` 不可访问，可能需要 `features = ["mock"]` 或类似。implementer 现场调

- [ ] **Step 4: commit**

```bash
git add crates/aish-secrets/
git commit -m "feat(aish-secrets): SecretStore + SecretError 跨平台 keyring 实现"
```

---

## Task 2: aish-types — SshAuth::Password 加 password 字段

**Files:**
- Modify: `crates/aish-types/src/lib.rs:90-100`（SshAuth enum）
- Modify: `crates/aish-types/src/lib.rs` 的 `tests` mod（更新现有测试）

- [ ] **Step 1: 改 SshAuth enum**

把 `crates/aish-types/src/lib.rs:90-100` 的：

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
```

替换为：

```rust
/// SSH 认证方式。Password 的 `password` 字段不序列化 — 仅运行时持有；
/// 持久化到 OS keyring（aish-secrets::SecretStore），hosts.json 只标 kind。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SshAuth {
    /// 密码模式：password 字段不进 hosts.json，存 OS keyring。
    /// 加载时 password == ""，由 ssh_actor 在 connect 前从 SecretStore 填回。
    Password {
        #[serde(default, skip_serializing)]
        password: String,
    },
    /// 私钥文件：只存路径，不读内容。
    KeyFile { path: PathBuf },
    /// 委托给 ssh-agent / Pageant / 1Password Agent。
    Agent,
}
```

- [ ] **Step 2: 更新现有测试 + 加 4 个新测试**

把 `crates/aish-types/src/lib.rs` 的 `tests` mod 里这一段：

```rust
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

替换为：

```rust
    #[test]
    fn ssh_auth_variants_serialize_distinctly() {
        let pwd = serde_json::to_string(&SshAuth::Password {
            password: "ignored".into(),
        })
        .unwrap();
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

    #[test]
    fn password_serialize_omits_field() {
        // password 字段标 #[serde(skip_serializing)] —— JSON 里不应出现 "password" key
        let auth = SshAuth::Password {
            password: "very-secret".into(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        // tag "kind":"password" 必须在
        assert!(json.contains("\"kind\":\"password\""));
        // 但 "very-secret" 不能泄漏
        assert!(!json.contains("very-secret"));
        // password 字段名也不应出现（防止有人误以为字段进了 JSON）
        assert!(
            !json.contains("\"password\":"),
            "password field should be skipped from serialization, got: {}",
            json
        );
    }

    #[test]
    fn password_deserialize_defaults_empty() {
        // 新 hosts.json 里 password kind 没有 password 字段 → 应 deserialize 为 password=""
        let json = r#"{"kind":"password"}"#;
        let auth: SshAuth = serde_json::from_str(json).unwrap();
        match auth {
            SshAuth::Password { password } => assert_eq!(password, ""),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn password_deserialize_ignores_password_field_if_present() {
        // 即使 hosts.json 不该有 password 字段，万一旧文件残留，也应能正常解析
        let json = r#"{"kind":"password","password":"leftover"}"#;
        let auth: SshAuth = serde_json::from_str(json).unwrap();
        match auth {
            SshAuth::Password { password } => assert_eq!(password, "leftover"),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn key_file_unchanged_compat() {
        // 旧 KeyFile entries 必须照常反 / 序列化
        let original = SshAuth::KeyFile {
            path: PathBuf::from("/home/larry/.ssh/id_ed25519"),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: SshAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
```

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-types 2>&1 | tail -10
cargo clippy -p aish-types --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: aish-types 测试 ≥ 11 passed (原 7 + 新 4)。clippy 全绿。

- [ ] **Step 4: 检查下游 crate 是否还能编译（SshAuth::Password 现在带字段，旧用法会报 missing field）**

```bash
cargo build --workspace 2>&1 | tail -20
```

预期：会有编译错误指向 aish-ssh/client.rs 的 `SshAuth::Password =>` match arm（unit pattern 现在不能匹配 struct variant）。**这是 Task 3 要修的，先记下编译错误再继续。**

- [ ] **Step 5: commit**

```bash
git add crates/aish-types/
git commit -m "feat(aish-types): SshAuth::Password 加 password 字段（serde skip 不入 hosts.json）"
```

---

## Task 3: aish-ssh — password 认证实现

**Files:**
- Modify: `crates/aish-ssh/src/client.rs`（match arm + tests）

- [ ] **Step 1: 修 client.rs 的 SshAuth match**

把 `crates/aish-ssh/src/client.rs:39-79` 的认证 match block 替换为：

```rust
        // 认证
        match &cfg.auth {
            SshAuth::KeyFile { path } => {
                // load_secret_key 内部自行读文件 + 解析，无需手动 fs::read
                let key_pair = russh::keys::load_secret_key(path, None).map_err(|source| {
                    // 先判断是否是 IO（文件不可读）
                    if let russh_keys::Error::IO(ref io_err) = source {
                        SshError::KeyFileRead {
                            path: path.clone(),
                            source: std::io::Error::new(io_err.kind(), io_err.to_string()),
                        }
                    } else {
                        SshError::KeyFileParse {
                            path: path.clone(),
                            source,
                        }
                    }
                })?;

                let auth_res = handle
                    .authenticate_publickey(&cfg.user, Arc::new(key_pair))
                    .await
                    .map_err(SshError::from)?;

                if !auth_res {
                    return Err(SshError::Auth(format!(
                        "server rejected publickey for user {}",
                        cfg.user
                    )));
                }
            }
            SshAuth::Password { password } => {
                if password.is_empty() {
                    return Err(SshError::Auth(
                        "密码为空（keyring 未取到值或未设置）".into(),
                    ));
                }
                let auth_res = handle
                    .authenticate_password(&cfg.user, password)
                    .await
                    .map_err(SshError::from)?;
                if !auth_res {
                    return Err(SshError::Auth(format!(
                        "server rejected password for user {}",
                        cfg.user
                    )));
                }
            }
            SshAuth::Agent => {
                return Err(SshError::Auth(
                    "Agent auth not supported (M5+)".into(),
                ));
            }
        }
```

- [ ] **Step 2: 更新现有 password 测试 + 加 1 个 empty password 测试**

把 `crates/aish-ssh/src/client.rs` tests mod 里的：

```rust
    #[tokio::test]
    async fn connect_with_password_auth_returns_unsupported_error() {
        let cfg = mk_cfg(SshAuth::Password);
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        // timeout 或 Err 都接受
        match result {
            Err(_timeout) => {} // tokio timeout，也 OK
            Ok(Err(SshError::Auth(msg))) => {
                assert!(
                    msg.contains("KeyFile"),
                    "expected KeyFile mention, got: {}",
                    msg
                );
            }
            Ok(Err(_)) => {} // TCP connect 先失败（127.0.0.1:22 没 sshd），也接受
            Ok(Ok(_)) => panic!("expected error for Password auth"),
        }
    }
```

替换为：

```rust
    #[tokio::test]
    async fn connect_with_empty_password_returns_auth_error() {
        // 空 password = keyring 未取到值的情况，应当 fail-fast
        let cfg = mk_cfg(SshAuth::Password {
            password: String::new(),
        });
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        match result {
            Err(_timeout) => {} // tokio timeout，OK
            Ok(Err(SshError::Auth(msg))) => {
                // 期望提到密码为空
                assert!(
                    msg.contains("密码为空") || msg.contains("empty"),
                    "expected empty password mention, got: {}",
                    msg
                );
            }
            Ok(Err(_)) => {} // TCP connect 先失败（127.0.0.1:22 没 sshd），也接受
            Ok(Ok(_)) => panic!("expected error for empty Password"),
        }
    }

    #[tokio::test]
    async fn connect_with_nonempty_password_attempts_auth() {
        // 非空 password — TCP 多半连不上 127.0.0.1:22，但绝不应该是
        // "密码为空"或 "not supported" 的错误（因为我们要走到 authenticate_password）
        let cfg = mk_cfg(SshAuth::Password {
            password: "definitely-wrong".into(),
        });
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        match result {
            Err(_timeout) => {} // tokio timeout，OK
            Ok(Err(SshError::Auth(msg))) => {
                // 不应是「密码为空」也不应是「not supported」
                assert!(
                    !msg.contains("密码为空") && !msg.contains("not supported"),
                    "should attempt actual auth, got: {}",
                    msg
                );
            }
            Ok(Err(_)) => {} // 任何其他错误（TCP 失败等）OK
            Ok(Ok(_)) => panic!("local 127.0.0.1:22 不应能用任意密码登录"),
        }
    }
```

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-ssh 2>&1 | tail -10
cargo clippy -p aish-ssh --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: aish-ssh 测试全绿（包括上面 2 个新测试）。clippy 全绿。

- [ ] **Step 4: commit**

```bash
git add crates/aish-ssh/
git commit -m "feat(aish-ssh): 实现 password 认证（authenticate_password）"
```

---

## Task 4: persistence.rs — save 时写 keyring + delete_secret_for helper

**Files:**
- Modify: `crates/aish-app/src/persistence.rs`

- [ ] **Step 1: 加 imports + SaveError 加 Secret variant**

替换 `crates/aish-app/src/persistence.rs:11-17` 的 imports + 加 use：

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aish_secrets::{SecretError, SecretStore};
use aish_types::{HostConfig, HostId, SshAuth};
```

替换 `crates/aish-app/src/persistence.rs:80-88` 的 SaveError：

```rust
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("config directory not found")]
    NoConfigDir,
    #[error("io error: {0}")]
    Io(#[source] io::Error),
    #[error("serialize failed: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("keyring 写入失败: {0}")]
    Secret(#[source] SecretError),
}
```

- [ ] **Step 2: 改写 save_hosts / save_hosts_to 实现 keyring 写入**

替换 `crates/aish-app/src/persistence.rs:49-68`（save_hosts + save_hosts_to）：

```rust
/// 保存 hosts.json（原子 tmp+rename）。自动 mkdir -p 父目录。
///
/// 副作用：对每个 SshAuth::Password 且 password 非空的 host，把密码写到 OS keyring。
/// password 为空表示「编辑模式不改密码」，不动 keyring。
pub fn save_hosts(hosts: &[HostConfig]) -> Result<(), SaveError> {
    let path = hosts_json_path().ok_or(SaveError::NoConfigDir)?;
    save_hosts_to(&path, hosts)
}

/// 测试用：保存到指定 path。除了 IO 部分，keyring 行为和 save_hosts 一致。
pub fn save_hosts_to(path: &Path, hosts: &[HostConfig]) -> Result<(), SaveError> {
    // Step 1: 把每个 Password host 的密码写 keyring
    for host in hosts {
        if let SshAuth::Password { password } = &host.auth {
            if !password.is_empty() {
                SecretStore::set(host.id, password).map_err(SaveError::Secret)?;
            }
            // password.is_empty() = 编辑时未改密码 → 不动 keyring
        }
    }

    // Step 2: 把 hosts（password 字段不会被序列化）写到 hosts.json
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SaveError::Io)?;
    }

    let json = serde_json::to_string_pretty(hosts).map_err(SaveError::Serialize)?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json).map_err(SaveError::Io)?;
    fs::rename(&tmp_path, path).map_err(SaveError::Io)?;

    Ok(())
}

/// 删除指定 host 在 keyring 里的 entry。NoEntry 视为成功（idempotent）。
/// 其他错误打 warn 但不阻塞调用方（删除流程已不可回退，仅记录）。
pub fn delete_secret_for(host_id: HostId) {
    if let Err(e) = SecretStore::delete(host_id) {
        if !matches!(e, SecretError::NoEntry) {
            tracing::warn!(?host_id, "failed to delete keyring entry: {}", e);
        }
    }
}
```

- [ ] **Step 3: 加 4 个 keyring 相关测试**

在 `crates/aish-app/src/persistence.rs` 的 tests mod 末尾（`#[test] fn hosts_json_path_returns_some_on_typical_env` 之后）加：

```rust
    use std::sync::Once;

    /// 全局一次性初始化 mock keyring backend（同 aish-secrets 的测试模式）。
    static KEYRING_INIT: Once = Once::new();

    fn init_mock_keyring() {
        KEYRING_INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    fn mk_password_host(label: &str, password: &str) -> HostConfig {
        HostConfig {
            id: HostId::new(),
            label: label.into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: SshAuth::Password {
                password: password.into(),
            },
            env_profile: None,
        }
    }

    #[test]
    fn save_password_writes_to_keyring() {
        init_mock_keyring();
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let host = mk_password_host("v", "p@ss");
        let id = host.id;

        save_hosts_to(&path, &[host]).unwrap();

        // keyring 里应该有这个条目
        assert_eq!(aish_secrets::SecretStore::get(id).unwrap(), "p@ss");
    }

    #[test]
    fn save_password_empty_skips_keyring() {
        init_mock_keyring();
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let host = mk_password_host("v", "");
        let id = host.id;

        save_hosts_to(&path, &[host]).unwrap();

        // 空 password 不应写 keyring
        assert!(matches!(
            aish_secrets::SecretStore::get(id),
            Err(aish_secrets::SecretError::NoEntry)
        ));
    }

    #[test]
    fn save_then_load_password_keeps_password_empty() {
        init_mock_keyring();
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let host = mk_password_host("v", "secret");

        save_hosts_to(&path, &[host]).unwrap();
        let loaded = load_hosts_from(&path).unwrap();

        assert_eq!(loaded.len(), 1);
        match &loaded[0].auth {
            SshAuth::Password { password } => assert_eq!(password, ""),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn delete_secret_for_idempotent_on_nonexistent() {
        init_mock_keyring();
        let id = HostId::new(); // 全新 id
        // 不应 panic
        delete_secret_for(id);
    }
```

- [ ] **Step 4: aish-app/Cargo.toml 加 keyring dev-dep（mock backend）**

测试需要 keyring crate（用 mock backend）。`crates/aish-app/Cargo.toml` 的 `[dev-dependencies]` 段加：

```toml
[dev-dependencies]
tempfile = { workspace = true }
keyring = { version = "3", default-features = false, features = ["apple-native", "windows-native", "sync-secret-service", "crypto-rust"] }
```

注：dev-dep 的 keyring 必须 enable mock module 可访问。如果 `keyring::mock::default_credential_builder` 在默认 features 下不可用，调整 features。同 Task 1 的 fallback 策略。

- [ ] **Step 5: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-app persistence 2>&1 | tail -15
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: persistence 测试 ≥ 10 passed（原 6 + 新 4）。clippy 全绿。

⚠️ persistence tests 与 aish-secrets tests 都用 `keyring::set_default_credential_builder` — 同进程只能调一次。两组测试在不同 crate 不会冲突（cargo test 默认每 crate 独立进程）。

- [ ] **Step 6: commit**

```bash
git add crates/aish-app/src/persistence.rs crates/aish-app/Cargo.toml
git commit -m "feat(aish-app): persistence 保存时写 keyring + delete_secret_for helper"
```

---

## Task 5: ssh_actor.rs — connect 前 lazy 读 keyring

**Files:**
- Modify: `crates/aish-app/src/ssh_actor.rs`

- [ ] **Step 1: 修改 host_session_task，在 SshClient::connect 前从 keyring 取密码**

替换 `crates/aish-app/src/ssh_actor.rs:28-58` 的 host_session_task 开头：

```rust
async fn host_session_task(
    host: HostId,
    config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    use aish_secrets::{SecretError, SecretStore};
    use aish_ssh::{ChannelMsg, SshClient};
    use aish_types::SshAuth;

    use crate::state::SshErrorKind;

    // 0. 如果是 Password auth 且 password 为空（来自 hosts.json），从 keyring 取
    let mut effective_config = config.clone();
    if let SshAuth::Password { password } = &mut effective_config.auth {
        if password.is_empty() {
            match SecretStore::get(host) {
                Ok(p) => {
                    *password = p;
                }
                Err(SecretError::NoEntry) => {
                    let _ = event_tx
                        .send(SshEvent::Error {
                            host,
                            kind: SshErrorKind::AuthFailed,
                            msg: "keyring 中没有该 host 的密码（请重新在 GUI 中输入并保存）".into(),
                        })
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(SshEvent::Error {
                            host,
                            kind: SshErrorKind::AuthFailed,
                            msg: format!("从 keyring 读取密码失败: {}", e),
                        })
                        .await;
                    return;
                }
            }
        }
    }

    // 1. 连接 + 认证
    let session = match SshClient::connect(&effective_config).await {
        Ok(s) => s,
        Err(err) => {
            // 把 aish_ssh::SshErrorKind 映射到 aish-app::state::SshErrorKind
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
```

注意：原来用 `&config`，现在改用 `&effective_config`。`config` 字段不再被后续用到（effective 已克隆），但 fn signature 不能改（spawn_session 调用方传的）— 在 fn 体内即时 shadow / clone。

- [ ] **Step 2: 添加 1 个验证用的单元测试**

ssh_actor 是异步 + 真 SSH，不便单测。但可以测「password 为空 + keyring 没条目 → 立即发 Error」。

加一个简单测试到 `crates/aish-app/src/ssh_actor.rs` tests mod 里（`#[test] fn encode_normal_chars` 之前）：

```rust
    use std::sync::Once;
    static KEYRING_INIT: Once = Once::new();
    fn init_mock_keyring() {
        KEYRING_INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    #[tokio::test]
    async fn password_empty_no_keyring_entry_emits_auth_error() {
        init_mock_keyring();

        use aish_types::{HostConfig, SshAuth};
        let cfg = HostConfig {
            id: aish_types::HostId::new(),
            label: "no-pwd".into(),
            host: "127.0.0.1".into(),
            port: 22,
            user: "root".into(),
            auth: SshAuth::Password {
                password: String::new(),
            },
            env_profile: None,
        };
        let host_id = cfg.id;

        let (event_tx, mut event_rx) = mpsc::channel::<SshEvent>(8);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(8);

        // 直接调 host_session_task（绕过 spawn）
        // 因为没 keyring 条目，期望立即 emit Error 然后 return
        let task_handle = tokio::spawn(host_session_task(host_id, cfg, cmd_rx, event_tx));

        // 期望第一个事件是 Error
        let evt = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timed out waiting for Error event");
        match evt {
            Some(SshEvent::Error {
                kind: crate::state::SshErrorKind::AuthFailed,
                msg,
                ..
            }) => {
                assert!(
                    msg.contains("keyring") || msg.contains("没有"),
                    "expected keyring/没有 in msg, got: {}",
                    msg
                );
            }
            other => panic!("expected AuthFailed Error, got: {:?}", other),
        }

        // task 应该 return（不 panic）
        task_handle.await.unwrap();
    }
```

注意：`host_session_task` 是 module-private fn，但同 crate 同 module 的 test 能访问 `super::host_session_task`。如果 `host_session_task` 无 `pub(crate)`，加上：在 `async fn host_session_task(...)` 改为 `pub(crate) async fn host_session_task(...)`（仅修可见性）。

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-app ssh_actor 2>&1 | tail -10
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: ssh_actor 测试 ≥ 8 passed（原 7 + 新 1）。clippy 全绿。

- [ ] **Step 4: commit**

```bash
git add crates/aish-app/src/ssh_actor.rs
git commit -m "feat(aish-app): ssh_actor 在 connect 前 lazy 从 keyring 读密码"
```

---

## Task 6: state.rs — HostFormDraft 加字段 + remove_host 调 delete_secret_for

**Files:**
- Modify: `crates/aish-app/src/state.rs`

- [ ] **Step 1: 加 AuthKind 枚举 + HostFormDraft 加 3 字段**

在 `crates/aish-app/src/state.rs:75`（`pub enum HostFormState` 之前）插入：

```rust
/// 表单中选中的认证类型（radio 控件）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    KeyFile,
    Password,
}
```

替换 `crates/aish-app/src/state.rs:84-93` 的 HostFormDraft：

```rust
/// 表单中间状态。port 用 String 让用户能临时输入非数字，提交时校验。
#[derive(Debug, Default, Clone)]
pub struct HostFormDraft {
    pub label: String,
    pub host: String,
    pub port: String,
    pub user: String,
    /// 当前选中的 auth 类型（radio）
    pub auth_kind: AuthKind,
    /// auth_kind == KeyFile 时使用
    pub key_path: String,
    /// auth_kind == Password 时使用。
    /// 编辑模式下默认 ""，留空表示「不改密码」（保留 keyring 现有值）。
    pub password: String,
    /// 控制密码字段 mask / 明文 显示（👁 toggle）
    pub password_visible: bool,
    /// 校验失败时显示在 modal 底部的红字。
    pub error: Option<String>,
}
```

- [ ] **Step 2: 改 from_config 适配 Password variant**

替换 `crates/aish-app/src/state.rs:95-110` 的 `from_config`：

```rust
    /// 从已有 HostConfig 填充（用于编辑）。
    /// 注意：Password 模式下 password 字段保持 ""，placeholder 提示「(unchanged)」；
    /// 不从 keyring 预读密码（最小化内存暴露 + 编辑保存空 = 不动 keyring）。
    pub fn from_config(cfg: &HostConfig) -> Self {
        let (auth_kind, key_path) = match &cfg.auth {
            aish_types::SshAuth::KeyFile { path } => {
                (AuthKind::KeyFile, path.display().to_string())
            }
            aish_types::SshAuth::Password { .. } => (AuthKind::Password, String::new()),
            aish_types::SshAuth::Agent => (AuthKind::KeyFile, String::new()),
        };
        Self {
            label: cfg.label.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            auth_kind,
            key_path,
            password: String::new(), // 编辑时不预填，留空 = 不改
            password_visible: false,
            error: None,
        }
    }
```

- [ ] **Step 3: 改 into_config 按 auth_kind 分支验证**

替换 `crates/aish-app/src/state.rs:112-148` 的 `into_config`：

```rust
    /// 校验并转回 HostConfig。`id` Some 表示编辑（保留原 id）/ None 表示新建。
    ///
    /// auth_kind 决定走 KeyFile 还是 Password 校验路径：
    ///   - KeyFile: 校验 key path 非空 + 文件存在
    ///   - Password: 校验 password 非空（**编辑模式例外**：编辑时空 password 表示「不改」，
    ///     由 caller 在 save 流程中区分；into_config 这里要求新建模式必须填密码）
    #[allow(clippy::wrong_self_convention)]
    pub fn into_config(&self, id: Option<HostId>) -> Result<HostConfig, String> {
        if self.label.trim().is_empty() {
            return Err("label 不能为空".into());
        }
        if self.host.trim().is_empty() {
            return Err("host 不能为空".into());
        }
        let port: u16 = self
            .port
            .trim()
            .parse()
            .map_err(|_| "port 必须是 1-65535 的数字".to_string())?;
        if self.user.trim().is_empty() {
            return Err("user 不能为空".into());
        }

        let auth = match self.auth_kind {
            AuthKind::KeyFile => {
                let key_path = self.key_path.trim();
                if key_path.is_empty() {
                    return Err("key path 不能为空".into());
                }
                let key_pathbuf = std::path::PathBuf::from(key_path);
                if !key_pathbuf.exists() {
                    return Err(format!("key 文件不存在: {}", key_path));
                }
                aish_types::SshAuth::KeyFile { path: key_pathbuf }
            }
            AuthKind::Password => {
                // 新建模式：必须填密码
                // 编辑模式：留空表示「不改」（caller 处理 — into_config 不知道是新建还是编辑就只能要求一致）
                // 这里 id.is_none() 检测新建
                if id.is_none() && self.password.is_empty() {
                    return Err("password 不能为空".into());
                }
                // 编辑时 password 可以为空（不改），传入 SshAuth::Password { password: "" }
                // persistence.save_hosts 看到空 password 就不动 keyring
                aish_types::SshAuth::Password {
                    password: self.password.clone(),
                }
            }
        };

        Ok(HostConfig {
            id: id.unwrap_or_else(|| HostId(uuid::Uuid::new_v4())),
            label: self.label.trim().into(),
            host: self.host.trim().into(),
            port,
            user: self.user.trim().into(),
            auth,
            env_profile: None,
        })
    }
```

- [ ] **Step 4: remove_host 调 persistence::delete_secret_for**

`crates/aish-app/src/state.rs::remove_host` 不能直接调 persistence（state.rs 是 lower layer，不应反向依赖 persistence）。改在调用方处理。

实际策略：在 host_form.rs 的 save 函数里（处理 DeleteConfirm 分支时）多调一行 `persistence::delete_secret_for(id)`。Task 7 的 host_form 改动里包含。state.rs::remove_host **不改**（保持 layer 干净）。

但是为了让 state.rs::remove_host 的语义清楚，加一个小注释。在 `crates/aish-app/src/state.rs::remove_host`（约第 254 行的 `/// 删除 host。同步清理 sessions / pane_terminals / pane_dimensions / 重置 selected。`）后追加一行：

```rust
    /// 删除 host。同步清理 sessions / pane_terminals / pane_dimensions / 重置 selected。
    /// 返回 true = 成功删除，false = 未找到。
    ///
    /// **注意**：此函数**不**清理 keyring 条目 — 调用方（host_form save）
    /// 在调本函数前/后调 `persistence::delete_secret_for(id)`。
    pub fn remove_host(&mut self, id: HostId) -> bool {
```

- [ ] **Step 5: 更新现有 HostFormDraft 测试 + 加 5 个新测试**

替换 `crates/aish-app/src/state.rs` tests mod 里现有 5 个 draft_* 测试为下面这一组（删旧加新；保留 mk_host / write_temp_key_file 等 helper 不变）：

```rust
    #[test]
    fn draft_keyfile_into_config_validates_empty_label() {
        let draft = HostFormDraft {
            label: "".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: "/tmp/x".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("label"));
    }

    #[test]
    fn draft_keyfile_into_config_validates_port_non_numeric() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "abc".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: "/tmp/x".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        assert!(draft.into_config(None).unwrap_err().contains("port"));
    }

    #[test]
    fn draft_keyfile_into_config_validates_key_file_exists() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: "/nonexistent/path/aish_test_only".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        assert!(draft.into_config(None).unwrap_err().contains("key 文件不存在"));
    }

    #[test]
    fn draft_keyfile_into_config_succeeds_with_existing_key() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: key.path().display().to_string(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let cfg = draft.into_config(None).unwrap();
        assert_eq!(cfg.label, "v");
        assert!(matches!(cfg.auth, aish_types::SshAuth::KeyFile { .. }));
    }

    #[test]
    fn draft_keyfile_into_config_preserves_id_when_provided() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::KeyFile,
            key_path: key.path().display().to_string(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let id = HostId(Uuid::new_v4());
        let cfg = draft.into_config(Some(id)).unwrap();
        assert_eq!(cfg.id, id);
    }

    #[test]
    fn draft_password_new_requires_nonempty_password() {
        // 新建模式 + Password + password 为空 → 报错
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::Password,
            key_path: "".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let r = draft.into_config(None); // None = 新建
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("password"));
    }

    #[test]
    fn draft_password_new_succeeds_with_password() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::Password,
            key_path: "".into(),
            password: "secret".into(),
            password_visible: false,
            error: None,
        };
        let cfg = draft.into_config(None).unwrap();
        match cfg.auth {
            aish_types::SshAuth::Password { password } => assert_eq!(password, "secret"),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn draft_password_edit_allows_empty_password() {
        // 编辑模式 + Password + 空密码 → OK（caller 解释为「不改」）
        let id = HostId(Uuid::new_v4());
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            auth_kind: AuthKind::Password,
            key_path: "".into(),
            password: "".into(),
            password_visible: false,
            error: None,
        };
        let cfg = draft.into_config(Some(id)).unwrap(); // Some = 编辑
        match cfg.auth {
            aish_types::SshAuth::Password { password } => assert_eq!(password, ""),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn draft_from_config_password_keeps_password_empty() {
        let host = HostConfig {
            id: HostId::new(),
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: aish_types::SshAuth::Password {
                password: "this-should-be-ignored".into(),
            },
            env_profile: None,
        };
        let draft = HostFormDraft::from_config(&host);
        assert_eq!(draft.auth_kind, AuthKind::Password);
        assert_eq!(draft.password, ""); // **关键**：不预填
    }

    #[test]
    fn draft_from_config_keyfile_extracts_path() {
        let h = mk_host("v");
        let draft = HostFormDraft::from_config(&h);
        assert_eq!(draft.auth_kind, AuthKind::KeyFile);
        assert!(draft.key_path.contains("/tmp/k") || draft.key_path.contains("\\tmp\\k"));
    }
```

注：上面新测试集**完全替换**原 5 个 draft 测试（旧 `draft_into_config_*` / `draft_from_config_*` 都要被这 10 个新测试取代）。

- [ ] **Step 6: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-app state 2>&1 | tail -20
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: state 测试 ≥ 21 passed（原非-draft 测试 11 + 新 draft 测试 10）。clippy 全绿。

- [ ] **Step 7: commit**

```bash
git add crates/aish-app/src/state.rs
git commit -m "feat(aish-app): HostFormDraft 加 AuthKind/password/password_visible，into_config 双路径校验"
```

---

## Task 7: host_form.rs — auth radio + password 字段 + 👁 toggle + FocusField 扩展

**Files:**
- Modify: `crates/aish-app/src/views/host_form.rs`（render + handle_key + FocusField）

- [ ] **Step 1: 扩展 FocusField + 调整 next() 跳过非当前 auth_kind 字段**

替换 `crates/aish-app/src/views/host_form.rs:23-42` 的 FocusField + impl：

```rust
/// 当前 focus 的 input 字段。auth_kind == KeyFile 走 KeyPath；== Password 走 Password。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Label,
    Host,
    Port,
    User,
    KeyPath,
    Password,
}

impl FocusField {
    /// 给定当前 auth_kind，跳到下一个有效字段（跳过当前 auth 不需要的）。
    fn next(self, auth_kind: crate::state::AuthKind) -> Self {
        use crate::state::AuthKind;
        // 顺序：Label → Host → Port → User → (KeyPath | Password) → Label
        match (self, auth_kind) {
            (FocusField::Label, _) => FocusField::Host,
            (FocusField::Host, _) => FocusField::Port,
            (FocusField::Port, _) => FocusField::User,
            (FocusField::User, AuthKind::KeyFile) => FocusField::KeyPath,
            (FocusField::User, AuthKind::Password) => FocusField::Password,
            (FocusField::KeyPath, _) => FocusField::Label,
            (FocusField::Password, _) => FocusField::Label,
        }
    }
}
```

- [ ] **Step 2: 修 append_char / backspace 加 KeyPath/Password 分支**

替换 `crates/aish-app/src/views/host_form.rs:71-113` 的 append_char + backspace：

```rust
    /// 把字符 append 到当前 focused 字段。
    fn append_char(&mut self, ch: char, cx: &mut Context<Self>) {
        let field = self.focus_field;
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                let target = match field {
                    FocusField::Label => &mut draft.label,
                    FocusField::Host => &mut draft.host,
                    FocusField::Port => &mut draft.port,
                    FocusField::User => &mut draft.user,
                    FocusField::KeyPath => &mut draft.key_path,
                    FocusField::Password => &mut draft.password,
                };
                target.push(ch);
                draft.error = None;
                cx.notify();
            }
        });
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        let field = self.focus_field;
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                let target = match field {
                    FocusField::Label => &mut draft.label,
                    FocusField::Host => &mut draft.host,
                    FocusField::Port => &mut draft.port,
                    FocusField::User => &mut draft.user,
                    FocusField::KeyPath => &mut draft.key_path,
                    FocusField::Password => &mut draft.password,
                };
                target.pop();
                draft.error = None;
                cx.notify();
            }
        });
    }
```

- [ ] **Step 3: 修 cycle_focus 传 auth_kind**

替换 `crates/aish-app/src/views/host_form.rs:115-118` 的 cycle_focus：

```rust
    fn cycle_focus(&mut self, cx: &mut Context<Self>) {
        // 取当前 modal 的 draft.auth_kind 决定 next() 跳到哪
        let auth_kind = self
            .state
            .read(cx)
            .modal
            .as_ref()
            .and_then(|m| match m {
                HostFormState::Adding(d) => Some(d.auth_kind),
                HostFormState::Editing { draft: d, .. } => Some(d.auth_kind),
                HostFormState::DeleteConfirm { .. } => None,
            })
            .unwrap_or(crate::state::AuthKind::KeyFile);
        self.focus_field = self.focus_field.next(auth_kind);
        cx.notify();
    }
```

- [ ] **Step 4: save 函数 — DeleteConfirm 分支调 delete_secret_for**

替换 `crates/aish-app/src/views/host_form.rs:127-180` 的 save：

```rust
    /// 保存（添加 / 编辑 / 删除确认）。返回是否需要持久化。
    fn save(&mut self, cx: &mut Context<Self>) {
        // 把 modal 取出（同时清空），决定后续动作
        let action = self.state.update(cx, |state, _cx| state.modal.take());

        let needs_persist = match action {
            Some(HostFormState::DeleteConfirm { id, .. }) => {
                self.state.update(cx, |state, cx| {
                    state.remove_host(id);
                    cx.notify();
                });
                // 删 host 同步删 keyring（idempotent，NoEntry 不报错）
                crate::persistence::delete_secret_for(id);
                true
            }
            Some(HostFormState::Adding(draft)) => {
                self.handle_add_or_edit(None, draft, cx)
            }
            Some(HostFormState::Editing { id, draft }) => {
                self.handle_add_or_edit(Some(id), draft, cx)
            }
            None => false,
        };

        if needs_persist {
            let hosts = self.state.read(cx).hosts.clone();
            self.bridge.spawn(async move {
                if let Err(e) = persistence::save_hosts(&hosts) {
                    tracing::error!("save hosts.json failed: {}", e);
                }
            });
        }
    }

    /// 处理添加/编辑保存：校验失败重新塞回 modal 并显示红字。返回是否成功。
    fn handle_add_or_edit(
        &mut self,
        id: Option<aish_types::HostId>,
        draft: HostFormDraft,
        cx: &mut Context<Self>,
    ) -> bool {
        match draft.into_config(id) {
            Ok(cfg) => {
                self.state.update(cx, |state, cx| {
                    if let Some(id) = id {
                        state.update_host(id, cfg);
                    } else {
                        state.add_host(cfg);
                    }
                    cx.notify();
                });
                true
            }
            Err(err) => {
                let mut new_draft = draft.clone();
                new_draft.error = Some(err);
                self.state.update(cx, |state, cx| {
                    state.modal = match id {
                        Some(id) => Some(HostFormState::Editing {
                            id,
                            draft: new_draft,
                        }),
                        None => Some(HostFormState::Adding(new_draft)),
                    };
                    cx.notify();
                });
                false
            }
        }
    }
```

- [ ] **Step 5: render_form_body 加 auth radio + password 字段**

替换 `crates/aish-app/src/views/host_form.rs:269-327` 的 `render_form_body`（整个函数）：

```rust
fn render_form_body(
    title: &str,
    draft: &HostFormDraft,
    focus_field: FocusField,
) -> gpui::AnyElement {
    let title_str = title.to_string();
    let mut col = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(15.0))
                .child(title_str),
        )
        .child(field_row(
            "label",
            &draft.label,
            focus_field == FocusField::Label,
        ))
        .child(field_row(
            "host",
            &draft.host,
            focus_field == FocusField::Host,
        ))
        .child(field_row(
            "port",
            &draft.port,
            focus_field == FocusField::Port,
        ))
        .child(field_row(
            "user",
            &draft.user,
            focus_field == FocusField::User,
        ));

    // auth radio（当前选中: ● 否则 ○ — 显示 only，无交互）
    // 切换 auth_kind 通过点击 row 触发 — 但 row 自己不能持有 closure 引用 self；
    // 因为这是个静态 fn。最简方案：把 auth radio 的点击事件从 form modal level 派发。
    // 实现里用 IntoElement 返回 div 后由调用方包 listener。
    // 这里返回静态展示 row（点击用 caller 提供的 closure）。
    let auth_kind = draft.auth_kind;
    let kf_marker = if auth_kind == crate::state::AuthKind::KeyFile {
        "● 密钥"
    } else {
        "○ 密钥"
    };
    let pw_marker = if auth_kind == crate::state::AuthKind::Password {
        "● 密码"
    } else {
        "○ 密码"
    };
    col = col.child(
        div()
            .flex()
            .flex_row()
            .gap_4()
            .py_1()
            .child(
                div()
                    .id("auth_kf")
                    .px_2()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .child(kf_marker),
            )
            .child(
                div()
                    .id("auth_pw")
                    .px_2()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .child(pw_marker),
            ),
    );

    // 根据 auth_kind 显示 KeyPath 或 Password 字段
    use crate::state::AuthKind;
    col = match auth_kind {
        AuthKind::KeyFile => col.child(field_row(
            "key path",
            &draft.key_path,
            focus_field == FocusField::KeyPath,
        )),
        AuthKind::Password => col.child(password_field_row(
            &draft.password,
            draft.password_visible,
            focus_field == FocusField::Password,
        )),
    };

    if let Some(err) = &draft.error {
        col = col.child(
            div()
                .text_color(rgb(0xff6666))
                .text_size(px(12.0))
                .child(err.clone()),
        );
    }

    col.child(
        div()
            .text_color(rgb(0x888888))
            .text_size(px(11.0))
            .child("Tab 切换字段，Ctrl+T 切 auth 类型，Ctrl+E 切密码可见，Enter 保存，Esc 取消"),
    )
    .into_any_element()
}

/// 密码字段行：input(mask/明文) + 👁 toggle 图标。
/// 编辑模式下 password 为空时显示 placeholder「(unchanged)」提示。
fn password_field_row(
    password: &str,
    visible: bool,
    focused: bool,
) -> gpui::AnyElement {
    let display: SharedString = if password.is_empty() {
        SharedString::from("(unchanged) 输入新密码所换")
    } else if visible {
        SharedString::from(password.to_string())
    } else {
        SharedString::from("•".repeat(password.chars().count()))
    };
    let border_color = if focused {
        rgb(0x4a90e2)
    } else {
        rgb(0x444444)
    };
    let text_color = if password.is_empty() {
        rgb(0x555555) // placeholder 灰色
    } else {
        rgb(0xeeeeee)
    };
    let eye = if visible { "👁" } else { "👁‍🗨" };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(72.0))
                .text_color(rgb(0xaaaaaa))
                .text_size(px(13.0))
                .child("password"),
        )
        .child(
            div()
                .flex_1()
                .px_2()
                .py_1()
                .bg(rgb(0x1d1d1d))
                .border_1()
                .border_color(border_color)
                .rounded_sm()
                .text_color(text_color)
                .text_size(px(13.0))
                .child(display),
        )
        .child(
            div()
                .id("pw_eye")
                .px_2()
                .text_color(rgb(0xaaaaaa))
                .text_size(px(14.0))
                .cursor_pointer()
                .child(eye),
        )
        .into_any_element()
}
```

- [ ] **Step 6: 加 keyboard shortcut — Ctrl+T 切 auth_kind / Ctrl+E 切 password_visible**

GPUI 鼠标点击没法在 fn 体里轻易传递（render_form_body 是 static fn），最实用的解法：用 keyboard shortcut。

替换 `crates/aish-app/src/views/host_form.rs:182-203` 的 `handle_key`：

```rust
    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let ctrl = event.keystroke.modifiers.control;

        match key {
            "escape" => self.cancel(cx),
            "tab" => self.cycle_focus(cx),
            "enter" => self.save(cx),
            "backspace" => self.backspace(cx),
            // Ctrl+T: 切换 auth_kind
            "t" if ctrl => self.toggle_auth_kind(cx),
            // Ctrl+E: 切换 password_visible
            "e" if ctrl => self.toggle_password_visible(cx),
            _ => {
                // 优先使用 key_char（系统 IME / 布局感知字符），退回到 key 本身（长度==1时）
                if let Some(ch_str) = event.keystroke.key_char.as_deref() {
                    if let Some(ch) = ch_str.chars().next() {
                        self.append_char(ch, cx);
                    }
                } else if key.len() == 1 {
                    if let Some(ch) = key.chars().next() {
                        self.append_char(ch, cx);
                    }
                }
            }
        }
    }

    /// 切换 auth_kind（KeyFile ↔ Password）。focus 重置到 Label 避免 focus_field 指向不可见字段。
    fn toggle_auth_kind(&mut self, cx: &mut Context<Self>) {
        use crate::state::AuthKind;
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                draft.auth_kind = match draft.auth_kind {
                    AuthKind::KeyFile => AuthKind::Password,
                    AuthKind::Password => AuthKind::KeyFile,
                };
                draft.error = None;
                cx.notify();
            }
        });
        self.focus_field = FocusField::Label;
    }

    /// 切换 password_visible（mask ↔ 明文）。
    fn toggle_password_visible(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                draft.password_visible = !draft.password_visible;
                cx.notify();
            }
        });
    }
```

注：
- 鼠标点击 auth radio / 👁 toggle 也想要支持的话，需要把 closure 从 render_form_body 移到 RootView/Modal 层。M2d 第一版用键盘快捷键即可（hint 在 modal 底部已说明）。
- 如果需要 mouse click 支持，**M2d 后续小迭代**再加（不阻塞这次主线）。

- [ ] **Step 7: 整文件清理 — 检查 imports**

`crates/aish-app/src/views/host_form.rs` 的 `use` 应该已经包含必要 import。如果 Step 5 的 `password_field_row` 用了新 import，加：

```rust
// 确认已有：
use gpui::{
    div, hsla, opaque_grey, prelude::*, px, rgb, App, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, SharedString, Window,
};
```

如果 Step 5 用 `.id("auth_kf")` 等 element id（用于 GPUI 的 element tree），需要 `gpui::ElementId` import；通常 `prelude::*` 已经带了。

- [ ] **Step 8: 验证（编译 + 手动 cargo run）**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -10
cargo test -p aish-app 2>&1 | grep -E "test result"
cargo fmt --all -- --check
cargo clippy -p aish-app --all-targets -- -D warnings 2>&1 | tail -3
```

Expected:
- build PASS
- aish-app 全部测试 passed
- fmt PASS
- clippy 全绿

UI 层无单元测试（GPUI render 难单测），通过 Task 8 端到端验证覆盖。

- [ ] **Step 9: commit**

```bash
git add crates/aish-app/src/views/host_form.rs
git commit -m "feat(aish-app): host_form 加 auth radio + password 字段 + 👁 toggle (Ctrl+T/Ctrl+E)"
```

---

## Task 8: 端到端验证 + push

**Files:** 无文件改动；只验证 + push。

- [ ] **Step 1: 全 workspace 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build --workspace 2>&1 | tail -5
cargo test --workspace 2>&1 | grep -E "test result"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: 全部退出码 0；test 总数 ≥ 160 (M2d 之前 148 + aish-secrets 5 + aish-types 4 + aish-ssh 1 + persistence 4 + state 5 + ssh_actor 1 ≈ 148 + 20 = 168)。

- [ ] **Step 2: 手动 GUI 验证清单**

```bash
cargo run -p aish-app
```

按下面顺序操作 + 确认每步行为：

1. 启动后能看到现有 test host（label="test"）
2. **修复**现有 test host 的 auth 类型为密码：
   - 鼠标 hover test 行，看到 ✏ 编辑图标（M3a 修复后该 hover 工作正常）
   - 点 ✏ 打开编辑 modal
   - 按 Ctrl+T 切到 Password 模式 — 应看到 password 字段出现，key path 字段消失
   - 输入真实密码（输入时 mask 显示 ••••）
   - 按 Ctrl+E 切 visible — 应看到明文
   - 按 Ctrl+E 再切回 mask
   - Enter 保存 — modal 关闭，回到列表
3. 点 test 行连接 — 应能连上（看到 SSH banner），grid 显示远端输出
4. 退出 GUI（关窗口）后重启
5. 再次点 test 行连接 — 应能直接连上（密码从 keyring 自动取）
6. 测试编辑模式不预填密码：
   - 再 hover test 行 → 点 ✏
   - password 字段应为空，placeholder 显示「(unchanged) 输入新密码所换」
   - 不输入任何密码，直接 Enter 保存
   - 再次连接 — 应仍能连上（keyring 密码未被空字符串覆盖）
7. 测试删除时清理 keyring：
   - hover test → 点 🗑 删除确认
   - Enter 确认
   - host 消失
   - 在 macOS Keychain Access / Windows 凭据管理器搜 "aish" — 应该看不到该 host_id 的条目了

如果任一步失败，**STOP 不要 push**；记录失败现象后回到对应 task 修。

- [ ] **Step 3: push**

```bash
git config http.postBuffer 524288000
git push origin main
```

如失败重试 `git push origin main`。

- [ ] **Step 4: 等 CI**

```bash
gh run list --limit 1
```

CI 应当能跑（aish-secrets 的 keyring tests 在 CI Linux 上用 mock backend 不依赖真 secret-service）。

如果 CI 失败因为 Linux keyring backend 问题，调整 aish-secrets/Cargo.toml 的 features（去掉 sync-secret-service 或加 default features）后再 push fix commit。

- [ ] **Step 5: 完成报告**

输出：

```
M2d STATUS: DONE

Commits:
- aish-secrets keyring 实现
- aish-types Password 字段
- aish-ssh password 认证
- aish-app persistence keyring 写入
- aish-app ssh_actor lazy 读
- aish-app HostFormDraft 字段扩展
- aish-app host_form UI

Tests: ~168 passed
GUI 手测: 7/7 通过
CI: success
```

---

## 完成验证（M2d 整体）

```bash
cargo build --workspace
cargo test --workspace          # ~168 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

git log 应有 8 个新 feat/fix commit + spec + plan = 10 个新 commit（含 spec / plan 文档）。

---

## 下一步

M2d 完成后回到 M3 路线：

- M3b：ssh_actor 持有 TmuxController per-host + GUI 加 TmuxSidebarView 渲染 SessionTree
- M3c：tmux 多 pane 真实交互（点击切 pane / send-keys / 重连恢复）

M2d 给 M3 路径解锁了：以后能用密码登录任何 SSH 服务器测试 tmux 集成。

---

## Self-Review

### Spec 覆盖
- ✅ aish-secrets SecretStore (Task 1)
- ✅ SshAuth::Password 字段 + serde (Task 2)
- ✅ aish-ssh password 认证 (Task 3)
- ✅ persistence save 写 keyring + delete_secret_for (Task 4)
- ✅ ssh_actor lazy 读 keyring (Task 5)
- ✅ HostFormDraft 字段扩展 + into_config 双路径 (Task 6)
- ✅ host_form UI radio + password + 👁 + 编辑留空 (Task 7)
- ✅ 端到端手测 + push (Task 8)
- ✅ 删除 host 同步删 keyring（Task 7 save 函数 DeleteConfirm 分支）

### Placeholder 扫描
- 无 TBD/TODO/implement later
- 无 "add appropriate" 类模糊描述
- 测试代码完整给出
- Step 7 提到「鼠标点击需要的话 M2d 后续小迭代」— 这是明确范围切割，不是 placeholder

### Type 一致性
- AuthKind 定义在 state.rs，引用在 host_form.rs（`crate::state::AuthKind`）— 一致
- HostFormDraft 字段：label/host/port/user/auth_kind/key_path/password/password_visible/error — 各 task 一致
- SshAuth::Password { password: String } — 各 task 一致
- SecretStore::set/get/delete 签名 — 各 task 一致
- SaveError 加 Secret variant — 一致

### 依赖顺序
- Task 1 (aish-secrets) 不依赖其他 → 可独立做
- Task 2 (aish-types) 不依赖其他 → 可独立做。但 Task 2 完成后 aish-ssh 会编译失败 → Task 3 必须紧跟
- Task 3 (aish-ssh) 依赖 Task 2
- Task 4 (persistence) 依赖 Task 1 + Task 2
- Task 5 (ssh_actor) 依赖 Task 1 + Task 3
- Task 6 (state) 依赖 Task 2
- Task 7 (host_form) 依赖 Task 6
- Task 8 (验证 + push) 依赖前 7 都完成

执行顺序按 Task 1 → 8 顺序进行（subagent-driven）。
