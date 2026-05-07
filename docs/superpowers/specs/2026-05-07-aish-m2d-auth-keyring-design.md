# aish M2d — Password 认证 + OS keyring 凭证存储 设计

> Status: Spec | Drafted 2026-05-07

## 0. 背景

M2a 实现了 SSH KeyFile 认证；M2c 加了 GUI host CRUD（label/host/port/user/key_path 字段）。
但实际使用中 user 大量场景没有现成的 SSH key，需要密码认证。

aish-types 当前 SshAuth 已经预留 `Password` 和 `Agent` variant 但未实现，
注释说"Password 不持久化，仅用于'输入即用即丢'"。本 milestone 重新定位 Password：
**密码持久化到 OS keyring**（macOS Keychain / Windows Credential Manager / Linux Secret Service），
hosts.json 只标记「该 host 用密码认证」不存密码本身。

## 1. 目标 / 非目标

### 目标
- aish-secrets crate 提供跨平台 keyring 凭证存储
- aish-types::SshAuth::Password { password } 加字段（运行时持有，不序列化）
- aish-app GUI host_form 加 auth 类型 radio + 密码字段（mask + 👁 toggle）
- aish-app 持久化层：保存时把密码写 keyring；加载时**不**预读（lazy 在 connect 时读）
- aish-ssh 实现 password 认证
- 删除 host 时同步删 keyring entry

### 非目标（M2d 不做）
- 密钥+密码的两层加密（passphrase-protected key file）
- ssh-agent 集成（仍是 M5+ 的 stretch）
- keyring 不可用时降级到加密文件（fallback 违背安全初衷）
- 密码强度检查 / 复杂度提示
- Touch ID / Windows Hello 二次确认

## 2. 架构概述

```
┌─────────────────────────────────────────────────────────────┐
│  GUI (host_form.rs)                                          │
│  - radio: 密钥 / 密码                                          │
│  - password input（mask + 👁 toggle）                         │
│  - 编辑模式：留空表示「不改密码」                                │
└────────────────────┬─────────────────────────────────────────┘
                     │ save
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  persistence.rs                                              │
│  - save_hosts(hosts): 把每个 Password host 的密码写 keyring，│
│    然后把 hosts (无密码) 写 hosts.json                        │
│  - load_hosts(): 从 hosts.json 读结构，**不**读 keyring     │
└────────────────────┬─────────────────────────────────────────┘
                     │
       ┌─────────────┴─────────────┐
       ▼                           ▼
┌──────────────┐          ┌────────────────────┐
│ hosts.json   │          │  aish-secrets      │
│ (no passwd)  │          │  SecretStore       │
└──────────────┘          │  - set/get/delete  │
                          │  - keyring crate    │
                          └────────────────────┘
                                   ▲
                                   │ lazy read
                                   │
┌─────────────────────────────────────────────────────────────┐
│  ssh_actor.rs / aish-ssh::client                             │
│  - connect 时如果 host.auth == Password，                     │
│    SecretStore::get(host_id) 读密码后立即用，握手完丢弃        │
└─────────────────────────────────────────────────────────────┘
```

## 3. 详细设计

### 3.1 aish-secrets crate

**Cargo.toml**：加 `keyring = "3"` 依赖（zbus 默认 feature 用 platform native；Linux 需要 `keyring = { version = "3", features = ["sync-secret-service"] }`，但 aish 主要 Win/macOS，Linux 可以延后）。

实际依赖：
```toml
keyring = { version = "3", default-features = false, features = ["apple-native", "windows-native", "sync-secret-service", "crypto-rust"] }
```

**API**：
```rust
pub struct SecretStore;

impl SecretStore {
    /// service name 固定为 "aish"，username 为 host_id 的 UUID 字符串。
    pub fn set(host_id: HostId, password: &str) -> Result<(), SecretError>;
    pub fn get(host_id: HostId) -> Result<String, SecretError>;
    pub fn delete(host_id: HostId) -> Result<(), SecretError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring 操作失败: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("条目不存在")]
    NoEntry,
}
```

NoEntry 单独提供以便 caller 区分「entry 不存在」vs「IO 错误」（删除时 NoEntry 视为成功）。

### 3.2 aish-types::SshAuth 演进

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SshAuth {
    /// 密码认证。password 字段不序列化（仅运行时）。
    /// 持久化时持有空字符串（"")，加载后由 ssh_actor::connect 从 keyring 取值填回。
    Password {
        #[serde(default, skip_serializing)]
        password: String,
    },
    KeyFile { path: PathBuf },
    Agent,
}
```

**schema 兼容**：
- 旧 `{"kind": "key_file", "path": "..."}` 仍然 deserialize 成 `KeyFile { path }`
- 旧 `{"kind": "password"}` deserialize 成 `Password { password: "" }`（默认值）
- 新写出 `{"kind": "password"}`（password 字段 skip_serializing）

### 3.3 persistence.rs 改动

**save_hosts(hosts) 新行为**：
```rust
pub fn save_hosts(hosts: &[HostConfig]) -> Result<(), PersistError> {
    // 第一步：把每个 Password host 的密码写 keyring
    for host in hosts {
        if let SshAuth::Password { password } = &host.auth {
            if !password.is_empty() {
                SecretStore::set(host.id, password)
                    .map_err(PersistError::Secret)?;
            }
            // password.is_empty() = 编辑时未改密码 → 不动 keyring
        }
    }

    // 第二步：把 hosts（password 字段不会被序列化）写到 hosts.json
    let json = serde_json::to_string_pretty(hosts)?;
    atomic_write(&hosts_path()?, json.as_bytes())?;
    Ok(())
}
```

**load_hosts() 不变**：直接从 hosts.json 读，Password variant 的 password 字段保持空字符串。

**delete_host(id) 新增**（或在调用方处理）：
```rust
pub fn delete_secret_for(host_id: HostId) {
    if let Err(e) = SecretStore::delete(host_id) {
        if !matches!(e, SecretError::NoEntry) {
            tracing::warn!(?host_id, "failed to delete keyring entry: {}", e);
        }
    }
}
```

state.rs 的 `remove_host` 调用方在持久化前调一次 delete_secret_for。

### 3.4 aish-ssh::client 改动

```rust
match &cfg.auth {
    SshAuth::KeyFile { path } => { /* 现有 */ }
    SshAuth::Password { password } => {
        if password.is_empty() {
            return Err(SshError::Auth("密码为空（keyring 未取到值）".into()));
        }
        let auth_result = handle.authenticate_password(&cfg.user, password).await?;
        if !auth_result.success() {
            return Err(SshError::Auth("密码认证被拒".into()));
        }
    }
    SshAuth::Agent => Err(SshError::Auth("Agent auth not supported (M5+)".into())),
}
```

**关键**：password 在握手完成后随 fn scope drop。SshClient 不存 password。

### 3.5 ssh_actor.rs 改动

connect 流程加一步 keyring 读取：
```rust
async fn connect(host: HostConfig, ...) {
    // 如果是 Password 且 password 为空（来自 hosts.json），从 keyring 取
    let mut effective_cfg = host.clone();
    if let SshAuth::Password { password } = &mut effective_cfg.auth {
        if password.is_empty() {
            *password = SecretStore::get(host.id)
                .map_err(|e| SshActorError::SecretLookup(e))?;
        }
    }
    // 用 effective_cfg 调用 SshClient::connect
}
```

### 3.6 host_form.rs UI 改动

**新增表单字段**：
- `auth_kind` — `enum AuthKind { KeyFile, Password }`，radio 控件
- `password_input` — 文本，仅 auth_kind == Password 时显示
- `password_visible` — bool，控制 mask / 明文（👁 toggle）

**编辑模式特殊处理**：
- 打开 modal 时 `password_input = ""`（不预填）
- placeholder 文本：「(unchanged) 输入新密码所换」
- 保存时如果 `password_input.is_empty()` 且是 Editing 模式且原 host.auth 是 Password → 不调用 SecretStore::set（保留 keyring 现有值）

**FocusField 扩展**：
```rust
enum FocusField {
    Label, Host, Port, User,
    KeyPath,    // auth_kind == KeyFile 时可达
    Password,   // auth_kind == Password 时可达
}
```

next() 跳过当前 auth_kind 不需要的字段。

**radio 控件**（GPUI 没有内置 radio，自绘）：
```rust
fn auth_radio(current: AuthKind, on_change: impl Fn(AuthKind)) -> AnyElement {
    div().flex_row().gap_3()
        .child(radio_button("密钥", current == AuthKind::KeyFile, on_change(KeyFile)))
        .child(radio_button("密码", current == AuthKind::Password, on_change(Password)))
}

fn radio_button(label: &str, selected: bool, on_click: ...) -> ... {
    let dot = if selected { "●" } else { "○" };
    div().flex_row().items_center().gap_1()
        .child(format!("{} {}", dot, label))
        .on_mouse_down(...)
}
```

**密码字段 mask**：
```rust
let display = if password_visible {
    password.clone()
} else {
    "•".repeat(password.chars().count())
};
let placeholder = if is_editing && password.is_empty() {
    "(unchanged) 输入新密码所换"
} else {
    ""
};
```

**👁 toggle**：在 password input 右侧 div 上加 click handler 翻转 password_visible。

### 3.7 错误显示

modal 底部已经有 `draft.error: Option<String>` 红字区。SecretStore / SSH 错误统一进这里：
- 保存时 SecretStore::set 失败：「凭证写入 keyring 失败: {msg}」
- 连接时 SecretStore::get 失败：在 host_list 行 hover/连接日志显示（不在 modal 因为 modal 已关）

## 4. ADR

### ADR-1: keyring crate 选型 — keyring 3.x
- **背景**：keyring crate 2.x 是 sync API；3.x 重构有 async 选项 + 更细的 platform feature flag
- **决策**：用 3.x 的 sync API（aish-secrets 是 thin wrapper，无需 async）
- **替代**：security-framework (macOS only) / windows crate (Windows only) — 重新发明跨平台
- **影响**：单一依赖（keyring），跨平台一致 API。若未来需要 async 可平滑切

### ADR-2: keyring entry naming — service="aish", username=host_id
- **背景**：keyring API 形式 `Entry::new(service, username)` — 需要决定如何 namespace
- **决策**：service 固定 "aish"，username 用 HostId 的 UUID 字符串
- **替代**：service="aish-{host_id}" 把 host_id 嵌 service — 不利于用户在 OS keyring 管理工具里筛选
- **影响**：在 macOS Keychain Access.app / Windows 凭据管理器搜 "aish" 能列出全部 aish 条目

### ADR-3: lazy 读取 — connect 时才读 keyring
- **背景**：load_hosts() 时是否预读所有密码到 hosts 列表？
- **决策**：**不预读**。Password::password 在 hosts.json 加载后保持空字符串，ssh_actor::connect 时才 SecretStore::get
- **理由**：
  - 密码不长期常驻内存（最小化暴露窗口）
  - 启动快（避免 N 次 keyring API 调用阻塞）
  - 即使 user 多 host，也只为正在连接的那个读密码
- **替代**：load_hosts 时 batch 读 — 启动慢且密码常驻
- **影响**：ssh_actor 需要 SecretError → SshActorError 的转换

### ADR-4: 编辑模式留空 = 不改密码
- **背景**：编辑现有 Password host 时密码字段是否预填？
- **决策**：**不预填**，placeholder 显示「(unchanged) 输入新密码所换」
- **理由**：
  - 与 1Password / Bitwarden / GitHub PAT 编辑等主流模式一致
  - 不把密码从 keyring 读出到 GUI 内存（最小暴露）
  - 防止意外覆盖：user 编辑其他字段保存时不会误改密码
- **替代**：打开时从 keyring 预填 — 增加内存暴露 + UX 不一致
- **影响**：persistence.rs save 逻辑要识别空字符串 = "skip"，不要把空写进 keyring

### ADR-5: keyring 失败不 fallback 到明文
- **背景**：keyring 写失败（OS 权限拒绝、Linux 无 secret-service 等）怎么办？
- **决策**：**报错**，modal 红字提示，user 自己处理
- **理由**：
  - fallback 到明文 hosts.json 违背选 keyring 的安全初衷
  - 静默 fallback 让 user 误以为密码加密了，更危险
- **替代**：fallback + WARN 日志 — 见上理由
- **影响**：Linux 上 user 必须装 gnome-keyring / KWallet 等 secret-service provider

### ADR-6: 删除 host 同步删 keyring
- **背景**：state.rs::remove_host 时 keyring entry 怎么处理？
- **决策**：同步 SecretStore::delete，NoEntry 错误视为成功
- **理由**：避免 keyring 累积孤儿条目（多个 host_id 不再使用但密码还在）
- **替代**：保留 keyring 条目以防误删 — 但 host_id 是 UUID 不可恢复，"误删后撤销"不现实
- **影响**：删除流程多一步 IO（毫秒级，可接受）

### ADR-7: 密码 mask 默认 + 👁 toggle
- **背景**：input 显示策略 — 一直 mask / toggle / 输入时短暂明文
- **决策**：默认 mask，加 👁 按钮（user 主动点切明文）
- **理由**：
  - 主流密码 UI（macOS, Windows, GitHub Settings）都是这个模式
  - 给 user 调试能力（输错时能看一眼）但不默认暴露
- **替代**：始终 mask（M2d 简化） — user 已选 toggle，按 user 选择
- **影响**：host_form 加一个 password_visible state 字段

### ADR-8: persistence error → modal error string，不 panic
- **背景**：保存失败如何向 user 反馈
- **决策**：所有 persistence/SecretStore 错误转 String 进 draft.error，modal 显示红字，user 看了能改
- **替代**：panic / 弹 OS 错误框 — 不 user-friendly
- **影响**：HostFormModal::save 改返回 Result，错误路径写 draft.error 后保留 modal 不关

### ADR-9: aish-secrets 直接用 keyring 全 sync API
- **背景**：keyring crate 3.x 同时支持 sync 和 async
- **决策**：用 sync API，aish-secrets 函数都是普通 fn 不是 async fn
- **理由**：
  - 操作快（< 10ms 在主流 OS）
  - persistence 已经在 cx.spawn 异步上下文里调用 — 阻塞几 ms 无感
  - 减少代码复杂度（不用引入 tokio dep 到 aish-secrets）
- **替代**：async API — 收益小成本大
- **影响**：aish-secrets Cargo.toml 不依赖 tokio

## 5. 测试策略

### 单元测试

**aish-secrets**：
- `set_then_get_returns_value` — 写后读
- `delete_then_get_returns_no_entry` — 删后读应返回 NoEntry
- `get_nonexistent_returns_no_entry` — 没存过的 host_id

⚠️ keyring 测试在 CI 上需要 `KEYRING_BACKEND_MOCK=1` 或类似 — 实际 CI Linux 环境通常没 secret-service。**测试用 `keyring::mock::default_credential_builder` 注入 mock backend**（keyring crate 3.x 提供）。

**aish-types**：
- `password_serialize_omits_field` — `SshAuth::Password { password: "secret" }` → JSON 不含 "password" key
- `password_deserialize_defaults_empty` — `{"kind": "password"}` → password 字段 == ""
- `key_file_unchanged` — 旧 KeyFile entries 仍然反/序列化正确

**aish-ssh**：现有测试结构里加：
- `password_auth_empty_returns_error` — Password { password: "" } 返回 Auth 错误
- `password_auth_nonempty_calls_authenticate_password` — mock russh handle 验证调用

**persistence**：
- `save_password_writes_to_keyring` — 用 mock SecretStore
- `save_password_empty_skips_keyring` — 空密码不调用 set
- `load_keeps_password_empty` — load 后 Password.password == ""

**host_form** (集成性较强，覆盖关键路径)：
- `auth_kind_toggle_changes_visible_fields` — 切 radio 影响显示
- `password_visible_toggle_swaps_display` — 👁 toggle
- `editing_with_empty_password_doesnt_overwrite_keyring` — 编辑保存空 password

### 集成测试
- 不在 CI 跑（依赖真实 keyring）
- 本地手测：开 GUI → 添加 Password host → 保存 → 重启 → 编辑能 hover 到 host → 连接成功

## 6. 兼容性 / 迁移

- **hosts.json schema**：旧 KeyFile entries **零改动**正常工作；旧 Password unit variant（`"password"` 字符串形式）现在 deserialize 到 `Password { password: "" }`（兼容）
- **现有 user 数据**：larry 当前 hosts.json 里只有 1 个 KeyFile host（path="/" 无效），M2d 完成后 user 可以编辑这个 host 改 auth_kind=Password 继续用
- **keyring 首次使用**：第一次写时 OS 可能弹「允许 aish 访问 keychain」对话框，user 同意后续静默

## 7. 待 plan 阶段细化

- 各 task 的具体文件 / 函数签名 / 单元测试 case 列表
- aish-app Cargo.toml 加 aish-secrets workspace dep
- HostFormDraft 新字段（auth_kind / password / password_visible）的 default + from_config 逻辑
- 验证步骤（手测 checklist）
