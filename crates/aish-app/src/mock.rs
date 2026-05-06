//! Mock SSH 行为：模拟真实 SSH 连接的延迟与输出。
//!
//! M2 接入真实 `aish_ssh::SshClient` 时整体替换此模块。

#![allow(dead_code)]

use std::time::Duration;

use tokio::sync::mpsc::Sender;

use crate::state::{HostId, MockEvent};

/// 模拟"连上 server，3 秒后收到 welcome 消息"。
///
/// 实际产生效果：
///   t=0     立即返回（caller 可继续做事）
///   t+3s    通过 channel send 一行 "Welcome to <label>!"
pub async fn mock_ssh_task(host: HostId, label: String, tx: Sender<MockEvent>) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    let _ = tx
        .send(MockEvent::PaneOutput {
            host,
            line: format!("Welcome to {}! (mocked SSH output)", label),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mock_ssh_task_emits_welcome_after_three_seconds() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let started = Instant::now();
        tokio::spawn(mock_ssh_task(HostId(42), "test-server".into(), tx));

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("should not timeout")
            .expect("channel should yield event");
        let elapsed = started.elapsed();

        // 时间窗口宽松：2.8s ~ 4s 都算正常（CI 环境抖动）
        assert!(
            elapsed >= Duration::from_millis(2800) && elapsed < Duration::from_secs(4),
            "expected ~3s, got {:?}",
            elapsed
        );

        let MockEvent::PaneOutput { host, line } = event;
        assert_eq!(host, HostId(42));
        assert!(line.contains("test-server"));
        assert!(line.contains("mocked SSH output"));
    }
}
