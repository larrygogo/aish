//! 一次性升级迁移：项目从 aish 改名为 issh（2026-05-21）。
//!
//! 老用户配置目录在 `{config_dir}/aish/`（含 hosts.json / app_state.toml）。
//! 启动时若发现老目录存在但新目录 `{config_dir}/issh/` 不存在，整体 rename
//! 过去，保留全部用户数据 + 历史 last_connected。已迁移过的（新目录已存在）
//! 跳过 — 不覆盖、不合并。
//!
//! keyring service 名保留为 "aish"（见 issh-secrets/lib.rs 注释），不在此处理。

use std::fs;
use std::path::PathBuf;

const LEGACY_DIR: &str = "aish";
const NEW_DIR: &str = "issh";

pub fn migrate_legacy_config_dir() {
    let Some(base) = dirs::config_dir() else {
        return;
    };
    let legacy: PathBuf = base.join(LEGACY_DIR);
    let new: PathBuf = base.join(NEW_DIR);

    if !legacy.exists() {
        return;
    }
    if new.exists() {
        // 已迁移过或新装并发起过 issh — 保留新目录，老目录不动（避免覆盖）。
        return;
    }
    match fs::rename(&legacy, &new) {
        Ok(()) => tracing::info!(
            "migrated config dir: {} → {}",
            legacy.display(),
            new.display()
        ),
        Err(e) => tracing::warn!(
            "migrate config dir failed ({} → {}): {}",
            legacy.display(),
            new.display(),
            e
        ),
    }
}
