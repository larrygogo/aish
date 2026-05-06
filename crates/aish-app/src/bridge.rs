//! Bridge：把 tokio runtime 与 GPUI executor 解耦。
//!
//! 启动一个 multi-thread runtime 在专属 worker thread 上，所有 SSH/SFTP/timer 类
//! async 工作都通过 `Bridge::spawn` 提交。runtime 通过 `tokio::sync::mpsc::Sender`
//! 把事件推回 GPUI 端，由 GPUI 用 `cx.spawn` 跑 async 块接收。

#![allow(dead_code)]

use std::future::Future;
use std::sync::Arc;

use crate::state::MockEvent;

/// 与 GPUI 端共享的事件 channel 对端。
///
/// `tx` 给 tokio task 用来发事件；`rx` 在 GPUI 端用 `cx.spawn` 接收。
pub struct EventChannel {
    pub tx: tokio::sync::mpsc::Sender<MockEvent>,
    pub rx: tokio::sync::mpsc::Receiver<MockEvent>,
}

impl EventChannel {
    /// 创建一个容量 64 的有限 channel（防止 OOM；M1 mock 流量不会满）。
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        Self { tx, rx }
    }
}

/// tokio runtime 包装。Drop 时 runtime 会优雅 shutdown。
pub struct Bridge {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Bridge {
    /// 启动 multi-thread runtime（默认 worker 数 = 物理核数）。
    pub fn start() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("aish-tokio")
            .build()?;
        Ok(Self {
            runtime: Arc::new(rt),
        })
    }

    /// 在 runtime 上提交一个 future。
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }

    /// 拿一个 runtime handle 用于跨线程 spawn（如果调用方不持有 Bridge 引用）。
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bridge_can_spawn_and_send_events() {
        let bridge = Bridge::start().expect("runtime should start");
        let mut chan = EventChannel::new();
        let tx = chan.tx.clone();

        bridge.spawn(async move {
            for i in 1..=3 {
                tx.send(MockEvent::PaneOutput {
                    host: crate::state::HostId(i),
                    line: format!("line {}", i),
                })
                .await
                .ok();
            }
        });

        // 同步等待 3 个事件到达（最多等 1 秒）
        let received = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut got = Vec::new();
                while got.len() < 3 {
                    match tokio::time::timeout(Duration::from_secs(1), chan.rx.recv()).await {
                        Ok(Some(ev)) => got.push(ev),
                        _ => break,
                    }
                }
                got
            })
        })
        .join()
        .unwrap();

        assert_eq!(received.len(), 3);
        let MockEvent::PaneOutput { host, line } = &received[0];
        assert_eq!(host.0, 1);
        assert_eq!(line, "line 1");
    }

    #[test]
    fn event_channel_capacity_is_64() {
        // 不直接测容量，而是测 64 个 send 可以连续完成（buffer 装得下）
        let chan = EventChannel::new();
        let tx = chan.tx;
        // 用 try_send 避免 await：未满会成功
        for i in 0..64 {
            tx.try_send(MockEvent::PaneOutput {
                host: crate::state::HostId(i),
                line: "x".into(),
            })
            .expect("buffer of 64 should accept 64 sends without blocking");
        }
        // 第 65 个应该满
        assert!(tx
            .try_send(MockEvent::PaneOutput {
                host: crate::state::HostId(65),
                line: "x".into(),
            })
            .is_err());
    }
}
