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
    /// 远程系统 /etc/os-release 的 ID 字段（如 "ubuntu" / "debian" / "centos" /
    /// "alpine" / "arch" / "fedora"...）。首次连上后由 ssh_actor 探测填入；
    /// 用于 Host 卡片显示发行版 logo。`None` = 还未探测过或探测失败。
    #[serde(default)]
    pub os_kind: Option<String>,
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
            },
            env_profile: Some(ProfileId::new("default")),
            os_kind: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: HostConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
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
        };
        assert_eq!(s.id.as_str(), "$0");
        assert_eq!(s.name, "dev");
    }
}
