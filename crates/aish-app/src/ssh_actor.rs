//! ssh_actor — host_session_task：每 host 一个 tokio task，own SshSession + PTY。
//!
//! 每个 task 内部 select! 在两个 future 之间：
//!   - chan.wait() — PTY 远端输出，转 SshEvent::PaneOutput 推回 GPUI
//!   - cmd_rx.recv() — GPUI 端的键盘输入命令，写入 chan.data()

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

async fn host_session_task(
    host: HostId,
    config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    use aish_ssh::{ChannelMsg, SshClient};

    use crate::state::SshErrorKind;

    // 1. 连接 + 认证
    let session = match SshClient::connect(&config).await {
        Ok(s) => s,
        Err(err) => {
            // 把 aish_ssh::SshErrorKind 映射到 aish-app::state::SshErrorKind
            let kind = match err.kind() {
                aish_ssh::SshErrorKind::ConnectFailed => SshErrorKind::ConnectFailed,
                aish_ssh::SshErrorKind::AuthFailed => SshErrorKind::AuthFailed,
                aish_ssh::SshErrorKind::Io => SshErrorKind::Io,
                aish_ssh::SshErrorKind::Protocol => SshErrorKind::Protocol,
            };
            let _ = event_tx
                .send(SshEvent::Error {
                    host,
                    kind,
                    msg: err.to_string(),
                })
                .await;
            return;
        }
    };

    // 2. 开 PTY channel
    let mut chan = match session.open_channel().await {
        Ok(c) => c,
        Err(err) => {
            let _ = event_tx
                .send(SshEvent::Error {
                    host,
                    kind: SshErrorKind::Protocol,
                    msg: format!("open_channel: {}", err),
                })
                .await;
            return;
        }
    };

    if let Err(err) = chan.request_pty(120, 40, "xterm-256color").await {
        let _ = event_tx
            .send(SshEvent::Error {
                host,
                kind: SshErrorKind::Protocol,
                msg: format!("request_pty: {}", err),
            })
            .await;
        return;
    }

    if let Err(err) = chan.shell().await {
        let _ = event_tx
            .send(SshEvent::Error {
                host,
                kind: SshErrorKind::Protocol,
                msg: format!("shell: {}", err),
            })
            .await;
        return;
    }

    let _ = event_tx.send(SshEvent::Connected { host }).await;

    // 3. select! loop: read + cmd
    loop {
        tokio::select! {
            msg = chan.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    // CryptoVec 实现了 Deref<Target=[u8]>，to_vec() 经由 Deref 调用
                    let _ = event_tx
                        .send(SshEvent::PaneOutput {
                            host,
                            bytes: data.to_vec(),
                        })
                        .await;
                }
                Some(ChannelMsg::Eof) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::RemoteExited,
                        })
                        .await;
                    break;
                }
                Some(_) => {
                    // 其他 ChannelMsg 类型（ExitStatus / WindowAdjusted / 等）暂时忽略
                }
            },
            cmd = cmd_rx.recv() => match cmd {
                Some(SessionCommand::SendBytes(bytes)) => {
                    if let Err(e) = chan.data(&bytes[..]).await {
                        let _ = event_tx
                            .send(SshEvent::Disconnected {
                                host,
                                reason: DisconnectReason::NetworkError(e.to_string()),
                            })
                            .await;
                        break;
                    }
                }
                Some(SessionCommand::Resize { cols, rows }) => {
                    if let Err(e) = chan
                        .window_change(cols as u32, rows as u32, 0, 0)
                        .await
                    {
                        tracing::warn!("PTY resize failed: {}", e);
                        // resize 失败不致命，继续运行
                    }
                }
                Some(SessionCommand::Disconnect) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::UserRequested,
                        })
                        .await;
                    break;
                }
            },
        }
    }
    // session drop → russh close
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
