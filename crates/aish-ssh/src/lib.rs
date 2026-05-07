//! aish-ssh — SSH 客户端层（russh 包装）。

#![allow(dead_code)]

mod channel;
mod client;
mod error;

pub use channel::Channel;
pub use client::{ExecResult, SshClient};
pub use error::{SshError, SshErrorKind};

// 重新 re-export russh 的 ChannelMsg，让上层不直接依赖 russh
pub use russh::ChannelMsg;

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
