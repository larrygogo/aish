//! Bridge：把 tokio runtime 与 GPUI executor 解耦。

#![allow(dead_code)]

use std::future::Future;
use std::sync::Arc;

use aish_types::{ConnectionId, HostConfig};
use tokio::sync::mpsc;

use crate::state::{SessionCommand, SshEvent};

pub struct EventChannel {
    pub tx: mpsc::Sender<SshEvent>,
    pub rx: mpsc::Receiver<SshEvent>,
}

impl EventChannel {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(64);
        Self { tx, rx }
    }
}

pub struct Bridge {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Bridge {
    pub fn start() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("aish-tokio")
            .build()?;
        Ok(Self {
            runtime: Arc::new(rt),
        })
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// 启动一个连接的 actor task。每个 ConnectionId 对应独立的 SSH session +
    /// PTY，同一 HostConfig 可以并发启多个连接。
    pub fn spawn_session(
        &self,
        conn: ConnectionId,
        config: HostConfig,
        event_tx: mpsc::Sender<SshEvent>,
    ) -> mpsc::Sender<SessionCommand> {
        crate::ssh_actor::spawn_session(self.runtime.handle().clone(), conn, config, event_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_channel_capacity_is_64() {
        let chan = EventChannel::new();
        let tx = chan.tx;
        for i in 0..64u32 {
            tx.try_send(SshEvent::PaneOutput {
                conn: ConnectionId::new(),
                bytes: vec![i as u8],
            })
            .expect("buffer of 64 should accept 64 sends without blocking");
        }
        assert!(tx
            .try_send(SshEvent::PaneOutput {
                conn: ConnectionId::new(),
                bytes: vec![65],
            })
            .is_err());
    }

    #[test]
    fn bridge_starts() {
        let bridge = Bridge::start().expect("runtime should start");
        let _handle = bridge.handle();
    }
}
