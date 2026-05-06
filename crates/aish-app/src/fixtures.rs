//! M2a 临时方案：从环境变量读 host config，避免 commit 真凭证。
//!
//! 环境变量：
//!   - AISH_DEV_HOST: 主机名或 IP
//!   - AISH_DEV_PORT: SSH 端口（可选，默认 22）
//!   - AISH_DEV_USER: 登录用户名
//!   - AISH_DEV_KEY_PATH: 私钥本地路径
//!   - AISH_DEV_LABEL: 显示标签（可选，默认 "dev"）
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
