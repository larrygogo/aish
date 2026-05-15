use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use aish_types::HostId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const APP_DIR_NAME: &str = "aish";
const APP_STATE_FILE: &str = "app_state.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppStateFile {
    #[serde(default)]
    pub recent: HashMap<String, u64>,
    /// 主题种类。"dark" / "light"，未指定时 None（启动用默认 dark）。
    /// SettingsView 切换 Dark mode switch 后写入此字段并 save 让重启保留。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// M30：accessibility - "减少动画"偏好。None = 默认 false（启用动画）。
    /// SettingsView Switch 写盘后，启动时回灌到 Theme.reduced_motion。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    /// M35 T9: sidebar 是否展开（220px 含「最近连接」list）。
    /// None = 默认 false（折叠 / 64px icon-only — 保留 v0.next muscle memory）。
    /// 用户点 logo 区域 toggle 后写盘。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar_expanded: Option<bool>,
}

pub fn app_state_path() -> Option<PathBuf> {
    let mut p = dirs::config_dir()?;
    p.push(APP_DIR_NAME);
    p.push(APP_STATE_FILE);
    Some(p)
}

/// 配置目录（不含具体文件）— Settings "打开配置目录" 按钮 reveal_path 用。
/// 同 dirs::config_dir() + APP_DIR_NAME 路径，对应 hosts.json / app_state.toml
/// 所在目录。
pub fn config_dir() -> Option<PathBuf> {
    let mut p = dirs::config_dir()?;
    p.push(APP_DIR_NAME);
    Some(p)
}

pub fn load_app_state_from(path: &Path) -> AppStateFile {
    match fs::read_to_string(path) {
        Ok(s) => match toml::from_str(&s) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("app_state.toml parse error: {} — using defaults", e);
                AppStateFile::default()
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => AppStateFile::default(),
        Err(e) => {
            tracing::warn!("app_state.toml read error: {} — using defaults", e);
            AppStateFile::default()
        }
    }
}

pub fn save_app_state_to(path: &Path, s: &AppStateFile) {
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!("save_app_state: mkdir failed: {}", e);
            return;
        }
    }
    let content = match toml::to_string(s) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("save_app_state: serialize failed: {}", e);
            return;
        }
    };
    let tmp = path.with_extension("toml.tmp");
    if let Err(e) = fs::write(&tmp, &content) {
        tracing::warn!("save_app_state: write tmp failed: {}", e);
        return;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        tracing::warn!("save_app_state: rename failed: {}", e);
        let _ = fs::remove_file(&tmp);
    }
}

pub fn load_app_state() -> AppStateFile {
    match app_state_path() {
        Some(p) => load_app_state_from(&p),
        None => {
            tracing::warn!("app_state_path: config dir not found");
            AppStateFile::default()
        }
    }
}

pub fn save_app_state(s: &AppStateFile) {
    match app_state_path() {
        Some(p) => save_app_state_to(&p, s),
        None => tracing::warn!("save_app_state: config dir not found"),
    }
}

impl AppStateFile {
    pub fn into_last_connected(self) -> HashMap<HostId, SystemTime> {
        self.recent
            .into_iter()
            .filter_map(|(k, v)| {
                let uuid = Uuid::parse_str(&k).ok()?;
                let time = SystemTime::UNIX_EPOCH + Duration::from_secs(v);
                Some((HostId(uuid), time))
            })
            .collect()
    }

    /// 把 last_connected 合并到现有 AppStateFile（保留 theme 等其他字段）。
    pub fn merge_last_connected(mut self, last_connected: &HashMap<HostId, SystemTime>) -> Self {
        self.recent = last_connected
            .iter()
            .filter_map(|(host_id, time)| {
                let secs = time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
                Some((host_id.0.to_string(), secs))
            })
            .collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_state.toml");
        let loaded = load_app_state_from(&path);
        assert!(loaded.recent.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_state.toml");
        let id = Uuid::new_v4().to_string();
        let mut state = AppStateFile::default();
        state.recent.insert(id.clone(), 1715174400u64);
        save_app_state_to(&path, &state);
        let loaded = load_app_state_from(&path);
        assert_eq!(loaded.recent.get(&id), Some(&1715174400u64));
    }

    #[test]
    fn load_corrupt_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_state.toml");
        fs::write(&path, b"[[[corrupt toml").unwrap();
        let loaded = load_app_state_from(&path);
        assert!(loaded.recent.is_empty());
    }

    #[test]
    fn save_atomic_no_tmp_remains() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_state.toml");
        let tmp = path.with_extension("toml.tmp");
        save_app_state_to(&path, &AppStateFile::default());
        assert!(!tmp.exists(), "tmp file should not remain after rename");
        assert!(path.exists());
    }

    #[test]
    fn into_last_connected_converts_valid_uuid() {
        let id = HostId(Uuid::new_v4());
        let secs = 1715174400u64;
        let mut state = AppStateFile::default();
        state.recent.insert(id.0.to_string(), secs);
        let lc = state.into_last_connected();
        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        assert_eq!(lc.get(&id), Some(&expected));
    }

    #[test]
    fn merge_last_connected_snapshot() {
        let id = HostId(Uuid::new_v4());
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1715174400);
        let mut lc = HashMap::new();
        lc.insert(id, t);
        let state = AppStateFile::default().merge_last_connected(&lc);
        assert_eq!(state.recent.get(&id.0.to_string()), Some(&1715174400u64));
    }

    /// M30：reduced_motion None / Some(true) / Some(false) 都能 roundtrip。
    /// None 时 skip_serializing 不写盘，加载回来仍是 None（默认行为）。
    #[test]
    fn reduced_motion_roundtrip_some_true() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_state.toml");
        let state = AppStateFile {
            reduced_motion: Some(true),
            ..AppStateFile::default()
        };
        save_app_state_to(&path, &state);
        let loaded = load_app_state_from(&path);
        assert_eq!(loaded.reduced_motion, Some(true));
    }

    #[test]
    fn reduced_motion_default_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_state.toml");
        let loaded = load_app_state_from(&path);
        // 文件不存在时返回 default，reduced_motion None = 默认 false
        assert_eq!(loaded.reduced_motion, None);
    }
}
