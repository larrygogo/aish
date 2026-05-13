//! ssh_actor — connection_task：每个连接一个 tokio task，own SshSession + PTY。
//!
//! 每个 task 内部 select! 在两个 future 之间：
//!   - chan.wait() — PTY 远端输出，转 SshEvent::PaneOutput 推回 GPUI
//!   - cmd_rx.recv() — GPUI 端的键盘输入命令，写入 chan.data()
//!
//! 关键 id 区分：
//!   - `conn: ConnectionId` — 该 task 自身的连接标识，所有 SshEvent 都用它寻址
//!   - `config.id: HostId` — 配置标识，keyring 密码按 HostId 索引（同一 host 的
//!     多个连接共享 keyring 条目）

#![allow(dead_code)]

use aish_types::{ConnectionId, HostConfig, RemoteSession};
use tokio::sync::mpsc;

use crate::state::{DisconnectReason, SessionCommand, SshEvent};

/// 在 tokio runtime 上 spawn 一个连接的 actor task。
///
/// 返回 SessionCommand sender — caller 把它存进 AppState.sessions。
pub fn spawn_session(
    runtime: tokio::runtime::Handle,
    conn: ConnectionId,
    config: HostConfig,
    event_tx: mpsc::Sender<SshEvent>,
) -> mpsc::Sender<SessionCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(64);
    runtime.spawn(connection_task(conn, config, cmd_rx, event_tx));
    cmd_tx
}

pub(crate) async fn connection_task(
    conn: ConnectionId,
    config: HostConfig,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    event_tx: mpsc::Sender<SshEvent>,
) {
    use aish_secrets::{SecretError, SecretStore};
    use aish_ssh::{ChannelMsg, SshClient};
    use aish_types::SshAuth;

    use crate::state::{SshErrorKind, DEFAULT_COLS, DEFAULT_ROWS};

    let host_id = config.id; // keyring 索引用

    // 初始 PTY 尺寸用 DEFAULT 占位 —— GPUI 第一次 layout 后立即通过
    // SessionCommand::Resize 触发 chan.window_change，把 SIGWINCH 透传到远端
    // shell；tmux attach 后 tmux 自身根据 PTY size 变化重排 pane（SIGWINCH
    // 链路：本地 → SSH → 远端 PTY → tmux server → tmux client → pane shell）。

    // 0. 如果是 Password auth 且 password 为空（来自 hosts.json），从 keyring 取
    let mut effective_config = config.clone();
    if let SshAuth::Password { password } = &mut effective_config.auth {
        if password.is_empty() {
            match SecretStore::get(host_id) {
                Ok(p) => {
                    *password = p;
                }
                Err(SecretError::NoEntry) => {
                    let _ = event_tx
                        .send(SshEvent::Error {
                            conn,
                            kind: SshErrorKind::AuthFailed,
                            msg: "keyring 中没有该 host 的密码（请重新在 GUI 中输入并保存）".into(),
                        })
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(SshEvent::Error {
                            conn,
                            kind: SshErrorKind::AuthFailed,
                            msg: format!("从 keyring 读取密码失败: {}", e),
                        })
                        .await;
                    return;
                }
            }
        }
    }

    // 1. 连接 + 认证
    let session = match SshClient::connect(&effective_config).await {
        Ok(s) => s,
        Err(err) => {
            let kind = match err.kind() {
                aish_ssh::SshErrorKind::ConnectFailed => SshErrorKind::ConnectFailed,
                aish_ssh::SshErrorKind::AuthFailed => SshErrorKind::AuthFailed,
                aish_ssh::SshErrorKind::Io => SshErrorKind::Io,
                aish_ssh::SshErrorKind::Protocol => SshErrorKind::Protocol,
            };
            let _ = event_tx
                .send(SshEvent::Error {
                    conn,
                    kind,
                    msg: err.to_string(),
                })
                .await;
            return;
        }
    };

    // 2. 开 raw PTY channel（M2 行为）
    let mut chan = match session.open_channel().await {
        Ok(c) => c,
        Err(err) => {
            let _ = event_tx
                .send(SshEvent::Error {
                    conn,
                    kind: SshErrorKind::Protocol,
                    msg: format!("open_channel: {}", err),
                })
                .await;
            return;
        }
    };
    if let Err(err) = chan
        .request_pty(DEFAULT_COLS, DEFAULT_ROWS, "xterm-256color")
        .await
    {
        let _ = event_tx
            .send(SshEvent::Error {
                conn,
                kind: SshErrorKind::Protocol,
                msg: format!("request_pty: {}", err),
            })
            .await;
        return;
    }
    if let Err(err) = chan.shell().await {
        let _ = event_tx
            .send(SshEvent::Error {
                conn,
                kind: SshErrorKind::Protocol,
                msg: format!("shell: {}", err),
            })
            .await;
        return;
    }
    let _ = event_tx.send(SshEvent::Connected { conn }).await;

    // 3. spawn 后台 list-sessions（独立 SSH channel）
    let session_for_query = session.clone();
    let tx_for_query = event_tx.clone();
    tokio::spawn(tmux_query_task(conn, session_for_query, tx_for_query));

    // 3.5 spawn 后台 OS 探测（独立 SSH channel，与 list-sessions 并行）
    let session_for_os = session.clone();
    let tx_for_os = event_tx.clone();
    let host_id_for_os = host_id;
    tokio::spawn(os_detect_task(
        conn,
        host_id_for_os,
        session_for_os,
        tx_for_os,
    ));

    // 4. 主循环：raw shell 单一模式。tmux attach 不再切换协议，只是往 channel
    //    发送 `tmux attach -t '<sess>'\r` 字节，让远端 tmux 接管 PTY 渲染。
    loop {
        tokio::select! {
            msg = chan.wait() => match msg {
                Some(ChannelMsg::ExtendedData { ref data, ext }) => {
                    tracing::warn!(?conn, ext, len = data.len(), payload = %String::from_utf8_lossy(data), "actor: channel ExtendedData");
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    tracing::warn!(?conn, exit_status, "actor: channel ExitStatus");
                }
                Some(ChannelMsg::ExitSignal { ref signal_name, .. }) => {
                    tracing::warn!(?conn, signal = ?signal_name, "actor: channel ExitSignal");
                }
                Some(ChannelMsg::Data { data }) => {
                    let _ = event_tx
                        .send(SshEvent::PaneOutput {
                            conn,
                            bytes: data.to_vec(),
                        })
                        .await;
                }
                Some(ChannelMsg::Eof) | None => {
                    tracing::warn!(?conn, "actor: channel closed (Eof/None)");
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            conn,
                            reason: DisconnectReason::RemoteExited,
                        })
                        .await;
                    break;
                }
                Some(other) => {
                    tracing::info!(?conn, ?other, "actor: other ChannelMsg");
                }
            },
            cmd = cmd_rx.recv() => match cmd {
                Some(SessionCommand::SendBytes(bytes)) => {
                    if let Err(e) = chan.data(&bytes[..]).await {
                        let _ = event_tx
                            .send(SshEvent::Disconnected {
                                conn,
                                reason: DisconnectReason::NetworkError(e.to_string()),
                            })
                            .await;
                        break;
                    }
                }
                Some(SessionCommand::Resize { cols, rows }) => {
                    // SIGWINCH 透传到远端 PTY；raw shell 模式下到 shell，tmux attach
                    // 模式下到 tmux client（tmux client 自己根据 PTY size 重排 panes）。
                    if let Err(e) = chan.window_change(cols as u32, rows as u32, 0, 0).await {
                        tracing::warn!("PTY resize failed: {}", e);
                    }
                }
                Some(SessionCommand::QueryTmuxSessions) => {
                    let session_for_query = session.clone();
                    let tx_for_query = event_tx.clone();
                    tokio::spawn(tmux_query_task(conn, session_for_query, tx_for_query));
                }
                Some(SessionCommand::AttachTmux { session: sess_id }) => {
                    // 在当前 raw shell PTY 里发 `tmux attach -t '<sess>'\r`。
                    // tmux 接管渲染（自带状态栏/窗口列表/pane 边框），用户在 tmux
                    // 内 prefix+d detach 后自然回到 shell 提示符。aish 不再用 -CC
                    // 控制模式（M3-archived），SessionTree / pane 树等也不维护。
                    let escaped = sess_id.as_str().replace('\'', r"'\''");
                    let attach_bytes = format!("tmux attach -t '{}'\r", escaped).into_bytes();
                    tracing::info!(
                        ?conn,
                        sess = sess_id.as_str(),
                        "actor: AttachTmux → send-keys to raw shell"
                    );
                    if let Err(e) = chan.data(&attach_bytes[..]).await {
                        tracing::warn!(?conn, "actor: send tmux attach failed: {}", e);
                        continue;
                    }
                    let _ = event_tx
                        .send(SshEvent::TmuxAttached {
                            conn,
                            session: sess_id,
                        })
                        .await;
                }
                Some(SessionCommand::UploadImage { data }) => {
                    let session_for_sftp = session.clone();
                    let tx_for_sftp = event_tx.clone();
                    tokio::spawn(async move {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        let remote_path = format!("/tmp/aish-clip-{}.png", ts);
                        match session_for_sftp.sftp_upload(&remote_path, &data).await {
                            Ok(()) => {
                                let _ = tx_for_sftp
                                    .send(SshEvent::ImageUploaded {
                                        conn,
                                        path: remote_path,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx_for_sftp
                                    .send(SshEvent::ImageUploadFailed {
                                        conn,
                                        msg: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    });
                }
                Some(SessionCommand::UploadBatch { images, text }) => {
                    // 流式上传：每张图独立上传，成功/失败立即发对应单图事件
                    // （app.rs 把成功的 path 立即 append 到 PTY，失败的只 toast
                    // 不插入），每完成一张发 BatchProgress 更新 input_bar 进度，
                    // 全部结束发 BatchDone 让 app.rs append text payload。
                    // 失败不早退 —— 继续上传剩余的（与旧 BatchUploadFailed 早退行为不同）。
                    let session_for_batch = session.clone();
                    let tx_for_batch = event_tx.clone();
                    tokio::spawn(async move {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        let total = images.len();
                        for (i, (bytes, ext)) in images.iter().enumerate() {
                            let remote_path = format!("/tmp/aish-clip-{}-{}.{}", ts, i, ext);
                            match session_for_batch.sftp_upload(&remote_path, bytes).await {
                                Ok(()) => {
                                    let _ = tx_for_batch
                                        .send(SshEvent::ImageUploaded {
                                            conn,
                                            path: remote_path,
                                        })
                                        .await;
                                }
                                Err(e) => {
                                    let _ = tx_for_batch
                                        .send(SshEvent::ImageUploadFailed {
                                            conn,
                                            msg: e.to_string(),
                                        })
                                        .await;
                                }
                            }
                            let _ = tx_for_batch
                                .send(SshEvent::BatchProgress {
                                    conn,
                                    done: i + 1,
                                    total,
                                })
                                .await;
                        }
                        let _ = tx_for_batch
                            .send(SshEvent::BatchDone { conn, text })
                            .await;
                    });
                }
                Some(SessionCommand::Disconnect) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            conn,
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

/// 键盘事件 → PTY 字节流编码。
///
/// Alt 修饰：在基础序列前加 ESC（\x1b），实现标准终端 Meta 键行为。
/// 例：Alt+F → \x1bf（bash readline forward-word）。
pub fn encode_key(key: &str, ctrl: bool, alt: bool) -> Vec<u8> {
    if ctrl {
        if let Some(c) = key.chars().next() {
            let upper = c.to_ascii_uppercase();
            if upper.is_ascii_uppercase() {
                let byte = (upper as u8) - 0x40;
                // Alt+Ctrl+key：极少见，仅发 Ctrl 序列（不加 ESC 前缀）
                return vec![byte];
            }
        }
        return Vec::new();
    }

    // 用 lowercased key 匹配特殊键名（GPUI 可能给 "Up" / "ArrowUp" / "up"）
    let base: Vec<u8> = match key.to_lowercase().as_str() {
        "enter" => vec![b'\r'],
        "backspace" => vec![0x7f],
        "tab" => vec![b'\t'],
        "escape" | "esc" => vec![0x1b],
        // GPUI 空格 keystroke.key 是 "space"（5 字符），不是 " "，单字符
        // 分支不会匹配，需独立处理。否则空格按下不发字节、远端收不到。
        "space" => vec![b' '],

        // 方向键 (normal mode CSI)
        "up" | "arrowup" => b"\x1b[A".to_vec(),
        "down" | "arrowdown" => b"\x1b[B".to_vec(),
        "right" | "arrowright" => b"\x1b[C".to_vec(),
        "left" | "arrowleft" => b"\x1b[D".to_vec(),

        // 导航键
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        "insert" => b"\x1b[2~".to_vec(),

        // 单字符 — 保留原 key（不 lowercased，避免 "Z" → "z"）
        s if s.len() == 1 => key.as_bytes().to_vec(),

        _ => Vec::new(),
    };

    if base.is_empty() {
        return Vec::new();
    }

    // Alt：在基础序列前加 ESC 前缀（标准 Meta 键编码）
    if alt {
        let mut out = Vec::with_capacity(base.len() + 1);
        out.push(0x1b);
        out.extend_from_slice(&base);
        out
    } else {
        base
    }
}

/// 解析 tmux list-sessions -F '#{session_id}|#{session_name}|#{session_windows}|#{session_activity}'
/// 的 stdout。windows / activity 解析失败时 fallback 0（旧 tmux 版本或字段缺失）。
fn parse_session_list(stdout: &[u8]) -> Vec<RemoteSession> {
    let s = String::from_utf8_lossy(stdout);
    s.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '|');
            let id_trimmed = parts.next()?.trim();
            let name_trimmed = parts.next()?.trim();
            if id_trimmed.is_empty() {
                return None;
            }
            let windows: u32 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let activity: i64 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            Some(RemoteSession {
                id: aish_types::SessionId::new(id_trimmed),
                name: name_trimmed.to_string(),
                windows,
                activity,
            })
        })
        .collect()
}

/// 在独立 SSH channel 跑 tmux list-sessions，结果通过 SshEvent 推回。
/// 解析 /etc/os-release 输出，提取 `ID=...` 字段（小写、去引号）。
/// 返回 `Some("ubuntu")` / `Some("debian")` / `None`（找不到 ID 行）。
fn parse_os_release(stdout: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(stdout).ok()?;
    for line in s.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("ID=") {
            // 去掉两端引号
            let val = rest.trim().trim_matches(|c| c == '"' || c == '\'');
            if !val.is_empty() {
                return Some(val.to_ascii_lowercase());
            }
        }
    }
    None
}

async fn os_detect_task(
    conn: ConnectionId,
    host_id: aish_types::HostId,
    client: aish_ssh::SshClient,
    event_tx: mpsc::Sender<SshEvent>,
) {
    // /etc/os-release 是 systemd 标准（Ubuntu/Debian/CentOS/Fedora/Arch/Alpine 等都有）。
    // macOS 没该文件，cat 会 fail，os_kind = None，UI 走 fallback 首字母 avatar。
    let result = client.exec_command("cat /etc/os-release 2>/dev/null").await;
    let os_kind = match result {
        Ok(r) if r.exit_code == 0 => parse_os_release(&r.stdout),
        _ => None,
    };
    let _ = event_tx
        .send(SshEvent::OsDetected {
            conn,
            host_id,
            os_kind,
        })
        .await;
}

async fn tmux_query_task(
    conn: ConnectionId,
    client: aish_ssh::SshClient,
    event_tx: mpsc::Sender<SshEvent>,
) {
    let _ = event_tx.send(SshEvent::TmuxQueryStarted { conn }).await;
    // session_windows / session_activity 加进 format 给 SessionPicker 展示
    // 元信息（windows 数 + 上次活跃时间）。两者在 tmux 1.6+ 都可用。
    let result = client
        .exec_command(
            "tmux list-sessions -F '#{session_id}|#{session_name}|#{session_windows}|#{session_activity}'",
        )
        .await;
    match result {
        Ok(r) if r.exit_code == 0 => {
            let sessions = parse_session_list(&r.stdout);
            let _ = event_tx
                .send(SshEvent::TmuxSessionsListed { conn, sessions })
                .await;
            // tmux 确实在 → 检查 mouse 是否开启。'no server running' 分支不查
            // （tmux 还没 server，show-options 也跑不通）。
            tokio::spawn(tmux_mouse_check_task(
                conn,
                client.clone(),
                event_tx.clone(),
            ));
        }
        Ok(r) => {
            let s = String::from_utf8_lossy(&r.stderr).to_string();
            if s.contains("command not found") || s.contains("not found") {
                let _ = event_tx.send(SshEvent::TmuxNoTmux { conn }).await;
            } else if s.contains("no server running") || s.contains("no sessions") {
                let _ = event_tx
                    .send(SshEvent::TmuxSessionsListed {
                        conn,
                        sessions: vec![],
                    })
                    .await;
                // 'no server running' = tmux 装了但没启 server。还是查一次
                // mouse global option（show-options -g 在没 server 时会启动
                // 一个临时 server 读 ~/.tmux.conf），可以提前知道用户配置。
                tokio::spawn(tmux_mouse_check_task(
                    conn,
                    client.clone(),
                    event_tx.clone(),
                ));
            } else {
                let trimmed = s.trim();
                let msg = if trimmed.is_empty() {
                    format!("tmux list-sessions exit {}", r.exit_code)
                } else {
                    trimmed.to_string()
                };
                let _ = event_tx.send(SshEvent::TmuxQueryFailed { conn, msg }).await;
            }
        }
        Err(e) => {
            let _ = event_tx
                .send(SshEvent::TmuxQueryFailed {
                    conn,
                    msg: e.to_string(),
                })
                .await;
        }
    }
}

/// 检查远端 tmux 全局 mouse 选项。'on' 静默；其他值或 exec 失败时发
/// TmuxMouseDisabled 让 UI 弹 toast 引导用户加 `set -g mouse on`。
///
/// 仅在 tmux 确认装了之后才 spawn（看 tmux_query_task 调用点）。
/// 用 `2>/dev/null` 吞 stderr 避免 'unknown option' 等版本差异报错污染。
async fn tmux_mouse_check_task(
    conn: ConnectionId,
    client: aish_ssh::SshClient,
    event_tx: mpsc::Sender<SshEvent>,
) {
    let result = client
        .exec_command("tmux show-options -gv mouse 2>/dev/null")
        .await;
    let (mouse_on, dbg) = match &result {
        Ok(r) => {
            let stdout = String::from_utf8_lossy(&r.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&r.stderr).trim().to_string();
            let on = r.exit_code == 0 && stdout.eq_ignore_ascii_case("on");
            (
                on,
                format!(
                    "exit={} stdout={:?} stderr={:?}",
                    r.exit_code, stdout, stderr
                ),
            )
        }
        Err(e) => (false, format!("err={}", e)),
    };
    tracing::info!(?conn, mouse_on, dbg = %dbg, "tmux_mouse_check_task result");
    if !mouse_on {
        let _ = event_tx.send(SshEvent::TmuxMouseDisabled { conn }).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_list_basic() {
        let s = b"$0|dev\n$1|work\n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id.as_str(), "$0");
        assert_eq!(result[0].name, "dev");
        assert_eq!(result[1].id.as_str(), "$1");
        assert_eq!(result[1].name, "work");
    }

    #[test]
    fn parse_session_list_empty_stdout() {
        let result = parse_session_list(b"");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_session_list_trims_whitespace() {
        let s = b"  $0  |  dev with spaces  \n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.as_str(), "$0");
        assert_eq!(result[0].name, "dev with spaces");
    }

    #[test]
    fn parse_session_list_skips_lines_without_pipe() {
        let s = b"$0|dev\nbroken-line\n$1|work\n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "dev");
        assert_eq!(result[1].name, "work");
    }

    #[test]
    fn parse_session_list_with_windows_and_activity() {
        let s = b"$0|dev|3|1700000000\n$1|work|5|1700001234\n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].windows, 3);
        assert_eq!(result[0].activity, 1700000000);
        assert_eq!(result[1].windows, 5);
        assert_eq!(result[1].activity, 1700001234);
    }

    #[test]
    fn parse_session_list_missing_extra_fields_fallback_zero() {
        // 旧 tmux 版本 / format 字段缺失：windows / activity 失败时 fallback 0，
        // id + name 仍然解析（向后兼容）。
        let s = b"$0|dev\n$1|work|2\n";
        let result = parse_session_list(s);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].windows, 0);
        assert_eq!(result[0].activity, 0);
        assert_eq!(result[1].windows, 2);
        assert_eq!(result[1].activity, 0);
    }

    #[test]
    fn parse_os_release_ubuntu() {
        let s = b"NAME=\"Ubuntu\"\nVERSION=\"24.04 LTS\"\nID=ubuntu\nID_LIKE=debian\n";
        assert_eq!(parse_os_release(s).as_deref(), Some("ubuntu"));
    }

    #[test]
    fn parse_os_release_unquoted_id() {
        let s = b"NAME=Arch Linux\nID=arch\n";
        assert_eq!(parse_os_release(s).as_deref(), Some("arch"));
    }

    #[test]
    fn parse_os_release_single_quoted_id() {
        let s = b"ID='alpine'\n";
        assert_eq!(parse_os_release(s).as_deref(), Some("alpine"));
    }

    #[test]
    fn parse_os_release_lowercases() {
        let s = b"ID=CentOS\n";
        assert_eq!(parse_os_release(s).as_deref(), Some("centos"));
    }

    #[test]
    fn parse_os_release_no_id_line_returns_none() {
        let s = b"NAME=\"Ubuntu\"\nVERSION=\"24.04\"\n";
        assert_eq!(parse_os_release(s), None);
    }

    #[test]
    fn parse_os_release_empty_returns_none() {
        assert_eq!(parse_os_release(b""), None);
    }

    #[test]
    fn parse_os_release_ignores_id_like() {
        // ID_LIKE 不应该被误匹配（必须严格前缀 "ID="）
        let s = b"ID_LIKE=debian\nID=ubuntu\n";
        assert_eq!(parse_os_release(s).as_deref(), Some("ubuntu"));
    }

    #[tokio::test]
    async fn password_empty_no_keyring_entry_emits_auth_error() {
        // password 为空 + keyring 无该 host 条目 → 期望立即 emit AuthFailed Error
        use aish_types::{ConnectionId, HostConfig, SshAuth};

        let cfg = HostConfig {
            id: aish_types::HostId::new(),
            label: "no-pwd".into(),
            host: "127.0.0.1".into(),
            port: 22,
            user: "root".into(),
            auth: SshAuth::Password {
                password: String::new(),
            },
            env_profile: None,
            os_kind: None,
        };
        let conn = ConnectionId::new();

        let (event_tx, mut event_rx) = mpsc::channel::<SshEvent>(8);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(8);

        // 直接调 connection_task — 因为没 keyring 条目应立即 emit Error 然后 return
        let task_handle: tokio::task::JoinHandle<()> =
            tokio::spawn(connection_task(conn, cfg, cmd_rx, event_tx));

        let evt = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timed out waiting for Error event");
        match evt {
            Some(SshEvent::Error {
                kind: crate::state::SshErrorKind::AuthFailed,
                msg,
                ..
            }) => {
                assert!(
                    msg.contains("keyring") || msg.contains("没有"),
                    "expected keyring/没有 in msg, got: {}",
                    msg
                );
            }
            other => panic!("expected AuthFailed Error, got: {:?}", other),
        }

        task_handle.await.unwrap();
    }

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
    }

    #[test]
    fn encode_arrow_keys_normal_mode() {
        assert_eq!(encode_key("up", false, false), b"\x1b[A".to_vec());
        assert_eq!(encode_key("ArrowUp", false, false), b"\x1b[A".to_vec());
        assert_eq!(encode_key("down", false, false), b"\x1b[B".to_vec());
        assert_eq!(encode_key("right", false, false), b"\x1b[C".to_vec());
        assert_eq!(encode_key("left", false, false), b"\x1b[D".to_vec());
    }

    #[test]
    fn encode_navigation_keys() {
        assert_eq!(encode_key("home", false, false), b"\x1b[H".to_vec());
        assert_eq!(encode_key("end", false, false), b"\x1b[F".to_vec());
        assert_eq!(encode_key("pageup", false, false), b"\x1b[5~".to_vec());
        assert_eq!(encode_key("pagedown", false, false), b"\x1b[6~".to_vec());
        assert_eq!(encode_key("delete", false, false), b"\x1b[3~".to_vec());
        assert_eq!(encode_key("insert", false, false), b"\x1b[2~".to_vec());
    }

    #[test]
    fn encode_uppercase_chars_preserve_case() {
        assert_eq!(encode_key("Z", false, false), b"Z");
        assert_eq!(encode_key("A", false, false), b"A");
    }
}
