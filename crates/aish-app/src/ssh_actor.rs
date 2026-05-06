//! ssh_actor — host_session_task：每 host 一个 tokio task。
//!
//! M2a Task 4 阶段是 **stub 形态**：spawn 后立即发 Connected 事件 + 等待
//! SessionCommand，收到任何 SendBytes 都 echo 回去当 PaneOutput。
//! 真 SSH 接通在 Task 6 替换 host_session_task 内部。

#![allow(dead_code)]

use aish_types::{HostConfig, HostId};
use tokio::sync::mpsc;

use crate::state::{DisconnectReason, SessionCommand, SshEvent};

/// 在 tokio runtime 上 spawn 一个 host 的 session task。
///
/// 返回 SessionCommand sender — caller 把它存进 AppState.sessions。
pub fn spawn_session(
    runtime: tokio::runtime::Handle,
    host: HostId,
    config: HostConfig,
    event_tx: mpsc::Sender<SshEvent>,
) -> mpsc::Sender<SessionCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(64);
    runtime.spawn(host_session_task(host, config, cmd_rx, event_tx));
    cmd_tx
}

/// Stub 实现：发 Connected → echo 任何 SendBytes 回 PaneOutput → 收 Disconnect 退出。
async fn host_session_task(
    host: HostId,
    _config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    let _ = event_tx.send(SshEvent::Connected { host }).await;
    let _ = event_tx
        .send(SshEvent::PaneOutput {
            host,
            bytes: b"[stub] type something and press Enter to echo\r\n".to_vec(),
        })
        .await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SessionCommand::SendBytes(bytes) => {
                let _ = event_tx
                    .send(SshEvent::PaneOutput {
                        host,
                        bytes: bytes.clone(),
                    })
                    .await;
                if bytes.contains(&b'\r') {
                    let _ = event_tx
                        .send(SshEvent::PaneOutput {
                            host,
                            bytes: b"\n[stub] echo done\r\n".to_vec(),
                        })
                        .await;
                }
            }
            SessionCommand::Disconnect => {
                let _ = event_tx
                    .send(SshEvent::Disconnected {
                        host,
                        reason: DisconnectReason::UserRequested,
                    })
                    .await;
                return;
            }
        }
    }
    // cmd_rx 收到 None = AppState drop sender，自然退出
    let _ = event_tx
        .send(SshEvent::Disconnected {
            host,
            reason: DisconnectReason::UserRequested,
        })
        .await;
}

/// 简易键盘事件 → 字节流编码（M2a 范围）。
///
/// 支持：普通字符 / Enter / Backspace / Tab / Esc / Ctrl+A-Z。
/// 不支持：方向键 / Home / End / F1-12 / Alt+ — M2b alacritty_terminal 接管。
pub fn encode_key(key: &str, ctrl: bool, _alt: bool) -> Vec<u8> {
    if ctrl {
        if let Some(c) = key.chars().next() {
            let upper = c.to_ascii_uppercase();
            if upper.is_ascii_uppercase() {
                let byte = (upper as u8) - 0x40;
                return vec![byte];
            }
        }
        return Vec::new();
    }

    match key {
        "enter" => vec![b'\r'],
        "backspace" => vec![0x7f],
        "tab" => vec![b'\t'],
        "escape" => vec![0x1b],
        s if s.len() == 1 => s.as_bytes().to_vec(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_normal_chars() {
        assert_eq!(encode_key("a", false, false), b"a");
        assert_eq!(encode_key("Z", false, false), b"Z");
        assert_eq!(encode_key(" ", false, false), b" ");
        assert_eq!(encode_key("/", false, false), b"/");
    }

    #[test]
    fn encode_special_keys() {
        assert_eq!(encode_key("enter", false, false), vec![b'\r']);
        assert_eq!(encode_key("backspace", false, false), vec![0x7f]);
        assert_eq!(encode_key("tab", false, false), vec![b'\t']);
        assert_eq!(encode_key("escape", false, false), vec![0x1b]);
    }

    #[test]
    fn encode_ctrl_combinations() {
        assert_eq!(encode_key("c", true, false), vec![0x03]);
        assert_eq!(encode_key("d", true, false), vec![0x04]);
        assert_eq!(encode_key("a", true, false), vec![0x01]);
        assert_eq!(encode_key("Z", true, false), vec![0x1a]);
    }

    #[test]
    fn unrecognized_keys_return_empty() {
        assert_eq!(encode_key("F1", false, false), Vec::<u8>::new());
        assert_eq!(encode_key("up", false, false), Vec::<u8>::new());
        assert_eq!(encode_key("home", false, false), Vec::<u8>::new());
    }
}
