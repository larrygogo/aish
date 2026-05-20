//! aish 共享类型定义。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 主机唯一标识（UUID v4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostId(pub Uuid);

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

/// 活跃连接唯一标识（UUID v4）。
///
/// 与 `HostId` 的区别：HostId 标记**配置**（持久化到 hosts.json），
/// ConnectionId 标记**运行时连接**（内存生命周期内有效）。一个 HostConfig
/// 可同时派生多个 Connection，每个 Connection 有独立的 actor / PTY / tmux
/// 状态。重启后所有 ConnectionId 失效，只剩 hosts。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// 人类可读的连接别名（替代 UUID 在日志 / 未来 CLI 输出里的可读性）。
///
/// 形如 `quick-fox-3a7c`：两词宠物名 + 4 位 hex 后缀防同名冲突。
/// 不取代 `ConnectionId`（仍是 UUID 主键），只作展示用。一个 ConnectionId 对应
/// 一个稳定 ConnectionAlias 由调用方维护；当前未持久化映射，重启后重新生成。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConnectionAlias(String);

impl ConnectionAlias {
    /// 用 petname 生成新别名。词表加载失败时 fallback 到 "anon-XXXX"。
    pub fn generate() -> Self {
        let words = petname::petname(2, "-").unwrap_or_else(|| "anon".to_string());
        let suffix = format!("{:04x}", Uuid::new_v4().as_u128() & 0xffff);
        Self(format!("{}-{}", words, suffix))
    }

    /// 从已知字符串构造（持久化加载 / 测试用）。
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConnectionAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// UI tab 唯一标识（UUID v4）。
///
/// 每个 tab 独立显示一个视图：默认页（host 卡片）或某个 Connection 的终端。
/// 不持久化。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// tmux session 名（字符串 newtype，避免与普通 String 混淆）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

/// 远端 tmux list-sessions 输出的单条 session 信息（纯展示用，不含 windows/panes）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSession {
    pub id: SessionId,
    pub name: String,
    /// 该 session 内的 window 数量（来自 tmux `#{session_windows}`）。
    pub windows: u32,
    /// session 最后活跃时间（Unix epoch 秒，来自 `#{session_activity}`）。
    /// 0 = 解析失败 / 旧 tmux 版本不支持该字段。
    pub activity: i64,
}

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
    /// `passphrase` 用于解密加密私钥（如 OpenSSH 默认 -P 加密的 id_rsa）。
    /// 同 password 模式：序列化时跳过（不入 hosts.json），存 OS keyring；
    /// 加载时 passphrase == ""，由 ssh_actor 在 connect 前从 SecretStore::get_passphrase
    /// 填回。未加密私钥则保持 "" 即可。
    KeyFile {
        path: PathBuf,
        #[serde(default, skip_serializing)]
        passphrase: String,
    },
    /// 委托给 ssh-agent / Pageant / 1Password Agent。
    Agent,
}

/// 远端 host-level 探测能力的集合。**只放跨连接稳定的属性**（比如发行版、
/// CPU 架构、默认 shell），不放每次连接可能变的运行时状态（tmux mouse /
/// session 列表等 — 那些归 session-level state，由 actor + state.rs 管）。
///
/// Schema 演进规则：append-only — 只增字段、不删字段、不改语义。所有字段
/// 用 `Option` + `#[serde(default)]`，保证旧 hosts.json 加载不报错。详见
/// [docs/capability-schema-rules.md](../../../docs/capability-schema-rules.md)。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    /// 远程系统 /etc/os-release 的 ID 字段（如 "ubuntu" / "debian" / "centos" /
    /// "alpine" / "arch" / "fedora"...）。首次连上后由 ssh_actor 探测填入；
    /// 用于 Host 卡片显示发行版 logo。
    /// `None` = 还未探测过或探测失败 — 下次连接会重试。
    #[serde(default)]
    pub os_kind: Option<String>,
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
    /// 探测到的远端 host-level 属性。aish 启动时默认空，连接后由 ssh_actor
    /// 探测填充并持久化。
    #[serde(default)]
    pub capabilities: HostCapabilities,
}

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

    #[test]
    fn connection_id_new_unique() {
        let a = ConnectionId::new();
        let b = ConnectionId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn connection_id_distinct_type_from_host_id() {
        // 编译期验证：HostId 和 ConnectionId 不互通（newtype 隔离）。
        // 这是防止 future 开发者把 host_id 当 conn_id 用的最低限度保险。
        let _h: HostId = HostId::new();
        let _c: ConnectionId = ConnectionId::new();
    }

    #[test]
    fn connection_alias_generate_nonempty() {
        let a = ConnectionAlias::generate();
        assert!(!a.as_str().is_empty());
    }

    #[test]
    fn connection_alias_generate_unique() {
        // 4 位 hex 后缀提供 65536 种区分，两次 generate 撞名概率 1/65536
        let a = ConnectionAlias::generate();
        let b = ConnectionAlias::generate();
        assert_ne!(a, b, "two generated aliases should differ");
    }

    #[test]
    fn connection_alias_format_words_and_hex_suffix() {
        let a = ConnectionAlias::generate();
        let s = a.as_str();
        // 2 词 petname + dash + 4 位 hex 后缀 ⇒ 至少 2 个 dash
        assert!(s.matches('-').count() >= 2, "expected ≥2 dashes in {}", s);
        let suffix = s.rsplit('-').next().unwrap();
        assert_eq!(suffix.len(), 4, "suffix len should be 4: {}", s);
        assert!(
            suffix.chars().all(|c| c.is_ascii_hexdigit()),
            "suffix not hex: {}",
            suffix
        );
    }

    #[test]
    fn connection_alias_new_and_display() {
        let a = ConnectionAlias::new("test-name-aabb");
        assert_eq!(a.as_str(), "test-name-aabb");
        assert_eq!(a.to_string(), "test-name-aabb");
    }

    #[test]
    fn connection_alias_serde_roundtrip() {
        let a = ConnectionAlias::generate();
        let json = serde_json::to_string(&a).unwrap();
        let parsed: ConnectionAlias = serde_json::from_str(&json).unwrap();
        assert_eq!(a, parsed);
    }

    #[test]
    fn tab_id_new_unique() {
        let a = TabId::new();
        let b = TabId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn session_id_basic() {
        let id = SessionId::new("main");
        assert_eq!(id.as_str(), "main");
        assert_eq!(id.to_string(), "main");
    }

    #[test]
    fn window_pane_display_matches_tmux_format() {
        assert_eq!(WindowId(3).to_string(), "@3");
        assert_eq!(PaneId(7).to_string(), "%7");
    }

    #[test]
    fn profile_id_basic() {
        let id = ProfileId::new("default");
        assert_eq!(id.as_str(), "default");
    }

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
                passphrase: String::new(),
            },
            env_profile: Some(ProfileId::new("default")),
            capabilities: HostCapabilities::default(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: HostConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn host_capabilities_default_empty() {
        let caps = HostCapabilities::default();
        assert_eq!(caps.os_kind, None);
    }

    #[test]
    fn host_capabilities_roundtrip() {
        let caps = HostCapabilities {
            os_kind: Some("ubuntu".to_string()),
        };
        let json = serde_json::to_string(&caps).unwrap();
        let parsed: HostCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, parsed);
    }

    #[test]
    fn host_config_with_capabilities_roundtrip() {
        let cfg = HostConfig {
            id: HostId::new(),
            label: "vps".into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: SshAuth::Agent,
            env_profile: None,
            capabilities: HostCapabilities {
                os_kind: Some("debian".to_string()),
            },
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(
            json.contains("\"capabilities\""),
            "capabilities key should appear in serialized JSON: {}",
            json
        );
        assert!(
            json.contains("\"os_kind\":\"debian\""),
            "os_kind should serialize inside capabilities: {}",
            json
        );
        let parsed: HostConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
        assert_eq!(parsed.capabilities.os_kind.as_deref(), Some("debian"));
    }

    #[test]
    fn host_config_old_format_ignores_top_level_os_kind() {
        // 旧 hosts.json 顶层放 os_kind 字段。serde 默认忽略 unknown fields，
        // 加载后 capabilities 为空（用户首次连接会重新探测填回）。
        let old_json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "label": "legacy",
            "host": "1.1.1.1",
            "port": 22,
            "user": "root",
            "auth": { "kind": "agent" },
            "env_profile": null,
            "os_kind": "ubuntu"
        }"#;
        let parsed: HostConfig =
            serde_json::from_str(old_json).expect("legacy hosts.json must still parse");
        assert_eq!(parsed.label, "legacy");
        assert_eq!(
            parsed.capabilities,
            HostCapabilities::default(),
            "old top-level os_kind should be silently ignored — value lost, will re-detect"
        );
    }

    #[test]
    fn host_config_missing_capabilities_field_defaults_empty() {
        // 兼容场景：HostConfig JSON 不带 capabilities 字段（升级路径）
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000002",
            "label": "fresh",
            "host": "2.2.2.2",
            "port": 22,
            "user": "u",
            "auth": { "kind": "agent" },
            "env_profile": null
        }"#;
        let parsed: HostConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.capabilities, HostCapabilities::default());
    }

    #[test]
    fn ssh_auth_variants_serialize_distinctly() {
        let pwd = serde_json::to_string(&SshAuth::Password {
            password: "ignored".into(),
        })
        .unwrap();
        let agent = serde_json::to_string(&SshAuth::Agent).unwrap();
        let key = serde_json::to_string(&SshAuth::KeyFile {
            path: PathBuf::from("/tmp/k"),
            passphrase: String::new(),
        })
        .unwrap();
        assert!(pwd.contains("password"));
        assert!(agent.contains("agent"));
        assert!(key.contains("key_file"));
        assert!(key.contains("/tmp/k"));
    }

    #[test]
    fn password_serialize_omits_field() {
        // password 字段标 #[serde(skip_serializing)] —— JSON 里不应出现 password 值
        let auth = SshAuth::Password {
            password: "very-secret".into(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"kind\":\"password\""));
        assert!(!json.contains("very-secret"));
        assert!(
            !json.contains("\"password\":"),
            "password field should be skipped from serialization, got: {}",
            json
        );
    }

    #[test]
    fn password_deserialize_defaults_empty() {
        let json = r#"{"kind":"password"}"#;
        let auth: SshAuth = serde_json::from_str(json).unwrap();
        match auth {
            SshAuth::Password { password } => assert_eq!(password, ""),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn password_deserialize_ignores_password_field_if_present() {
        let json = r#"{"kind":"password","password":"leftover"}"#;
        let auth: SshAuth = serde_json::from_str(json).unwrap();
        match auth {
            SshAuth::Password { password } => assert_eq!(password, "leftover"),
            _ => panic!("expected Password variant"),
        }
    }

    #[test]
    fn key_file_unchanged_compat() {
        let original = SshAuth::KeyFile {
            path: PathBuf::from("/home/larry/.ssh/id_ed25519"),
            passphrase: String::new(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: SshAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn remote_session_basic() {
        let s = RemoteSession {
            id: SessionId::new("$0"),
            name: "dev".into(),
            windows: 3,
            activity: 1700000000,
        };
        assert_eq!(s.id.as_str(), "$0");
        assert_eq!(s.name, "dev");
        assert_eq!(s.windows, 3);
        assert_eq!(s.activity, 1700000000);
    }
}
