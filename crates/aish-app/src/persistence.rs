//! hosts.json 跨平台持久化读写。
//!
//! 路径：{config_dir}/aish/hosts.json
//!   - Windows: %APPDATA%\aish\hosts.json
//!   - macOS:   ~/Library/Application Support/aish/hosts.json
//!   - Linux:   ~/.config/aish/hosts.json
//!
//! 写入用原子 tmp+rename 防半写损坏。

#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aish_types::HostConfig;

/// 配置子目录名。
const APP_DIR_NAME: &str = "aish";
/// hosts 文件名。
const HOSTS_FILE: &str = "hosts.json";

/// 拿 hosts.json 完整路径（不保证父目录存在）。
pub fn hosts_json_path() -> Option<PathBuf> {
    let mut p = dirs::config_dir()?;
    p.push(APP_DIR_NAME);
    p.push(HOSTS_FILE);
    Some(p)
}

/// 加载 hosts.json。
///
/// - 文件不存在 → Ok(vec![])
/// - 文件存在但 parse 失败 → Err
pub fn load_hosts() -> Result<Vec<HostConfig>, LoadError> {
    let path = hosts_json_path().ok_or(LoadError::NoConfigDir)?;
    load_hosts_from(&path)
}

/// 测试用：从指定 path 加载（绕过 dirs::config_dir）。
pub fn load_hosts_from(path: &Path) -> Result<Vec<HostConfig>, LoadError> {
    match fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map_err(LoadError::Parse),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(LoadError::Io(e)),
    }
}

/// 保存 hosts.json（原子 tmp+rename）。自动 mkdir -p 父目录。
pub fn save_hosts(hosts: &[HostConfig]) -> Result<(), SaveError> {
    let path = hosts_json_path().ok_or(SaveError::NoConfigDir)?;
    save_hosts_to(&path, hosts)
}

/// 测试用：保存到指定 path。
pub fn save_hosts_to(path: &Path, hosts: &[HostConfig]) -> Result<(), SaveError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SaveError::Io)?;
    }

    let json = serde_json::to_string_pretty(hosts).map_err(SaveError::Serialize)?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json).map_err(SaveError::Io)?;
    fs::rename(&tmp_path, path).map_err(SaveError::Io)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("config directory not found (HOME / APPDATA unset?)")]
    NoConfigDir,
    #[error("read hosts.json failed: {0}")]
    Io(#[source] io::Error),
    #[error("parse hosts.json failed: {0}")]
    Parse(#[source] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("config directory not found")]
    NoConfigDir,
    #[error("io error: {0}")]
    Io(#[source] io::Error),
    #[error("serialize failed: {0}")]
    Serialize(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_types::{HostId, SshAuth};
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn mk_host(label: &str) -> HostConfig {
        HostConfig {
            id: HostId(Uuid::new_v4()),
            label: label.into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: SshAuth::KeyFile {
                path: PathBuf::from("/home/me/.ssh/id_ed25519"),
            },
            env_profile: None,
        }
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let result = load_hosts_from(&path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("aish").join("hosts.json");
        let original = vec![mk_host("vps-1"), mk_host("vps-2")];

        save_hosts_to(&path, &original).unwrap();

        let loaded = load_hosts_from(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].label, "vps-1");
        assert_eq!(loaded[1].label, "vps-2");
        assert_eq!(loaded[0].id, original[0].id);
    }

    #[test]
    fn save_creates_parent_dir_if_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("c").join("hosts.json");
        let hosts = vec![mk_host("vps")];
        save_hosts_to(&path, &hosts).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn save_atomic_cleans_up_tmp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let tmp = path.with_extension("json.tmp");
        let hosts = vec![mk_host("vps")];

        save_hosts_to(&path, &hosts).unwrap();

        // tmp 文件应被 rename 消除
        assert!(!tmp.exists(), "tmp file should not remain after rename");
        assert!(path.exists());
    }

    #[test]
    fn load_returns_parse_error_on_corrupt_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        fs::write(&path, b"this is not json").unwrap();

        let result = load_hosts_from(&path);
        assert!(matches!(result, Err(LoadError::Parse(_))));
    }

    #[test]
    fn save_then_load_empty_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let original: Vec<HostConfig> = Vec::new();

        save_hosts_to(&path, &original).unwrap();
        let loaded = load_hosts_from(&path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn hosts_json_path_returns_some_on_typical_env() {
        // CI 与开发机都应该能拿到（HOME / APPDATA 必存在）
        let p = hosts_json_path();
        assert!(p.is_some());
        let p = p.unwrap();
        let s = p.display().to_string();
        assert!(
            s.ends_with("aish/hosts.json") || s.ends_with("aish\\hosts.json"),
            "unexpected path: {}",
            s
        );
    }
}
