//! M2a 临时方案：从环境变量读 host config，避免 commit 真凭证。
//!
//! Task 5 实装真实读取逻辑。本 task 仅占位返回空。

#![allow(dead_code)]

use aish_types::HostConfig;

/// Task 5 实装：从 AISH_DEV_HOST / AISH_DEV_USER / AISH_DEV_KEY_PATH 读。
pub fn dev_hosts() -> Vec<HostConfig> {
    Vec::new()
}
