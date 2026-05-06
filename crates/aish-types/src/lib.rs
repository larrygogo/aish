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
}
