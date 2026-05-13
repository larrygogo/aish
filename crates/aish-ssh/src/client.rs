//! SshClient — russh 包装。M2a 仅支持 KeyFile 认证。

use std::sync::Arc;

use aish_types::{HostConfig, SshAuth};
use async_trait::async_trait;
use russh::client::{Config, Handle, Handler};
use russh::keys::key::PublicKey;

use crate::error::SshError;
use russh_sftp::client::SftpSession;

/// SshClient 持有一个建立好的 SSH 连接（已 handshake + auth 完成）。
///
/// 后续调用 `open_channel().request_pty(...)` 拿可用的 PTY channel。
pub struct SshClient {
    handle: Arc<Handle<NoopHandler>>,
}

impl Clone for SshClient {
    fn clone(&self) -> Self {
        // russh::client::Handle 内部是 Arc，clone 是引用计数克隆，共享底层连接
        // Arc clone：引用计数 +1，共享底层连接
        Self {
            handle: Arc::clone(&self.handle),
        }
    }
}

/// 远端命令执行结果。
#[derive(Debug)]
pub struct ExecResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: u32,
}

impl SshClient {
    /// 连接到 cfg.host:cfg.port，按 cfg.auth 认证。
    ///
    /// M2a 仅支持 SshAuth::KeyFile；其他 variant 返回 Auth 错误。
    pub async fn connect(cfg: &HostConfig) -> Result<Self, SshError> {
        // russh 默认 inactivity_timeout=None（不因 idle 主动断）。之前误设
        // 1 小时，导致用户在 tmux 里挂着不动超 1h 后，russh 客户端自己 close
        // 连接，UI 显示 'ssh protocol error: Disconnected' —— 误报为'服务器
        // 主动断'，实际是 client 自己关。改回 None。
        //
        // keepalive_interval=30s：每 30s 客户端发 SSH keep-alive 给 server，
        // 一是防 NAT / 中间路由因长 idle RST 连接，二是 server 端确认 client
        // 还活着。keepalive_max=3（默认）= 90s 内连续 3 次无响应才视为断。
        let config = Arc::new(Config {
            inactivity_timeout: None,
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        });

        // TCP 连接 + SSH handshake
        let addr = format!("{}:{}", cfg.host, cfg.port);
        let mut handle = russh::client::connect(config, addr, NoopHandler {})
            .await
            .map_err(|e| match e {
                russh::Error::IO(io) => SshError::Connect(io),
                other => SshError::Protocol(other),
            })?;

        // 认证
        match &cfg.auth {
            SshAuth::KeyFile { path } => {
                // load_secret_key 内部自行读文件 + 解析，无需手动 fs::read
                let key_pair = russh::keys::load_secret_key(path, None).map_err(|source| {
                    // 先判断是否是 IO（文件不可读）
                    if let russh_keys::Error::IO(ref io_err) = source {
                        SshError::KeyFileRead {
                            path: path.clone(),
                            source: std::io::Error::new(io_err.kind(), io_err.to_string()),
                        }
                    } else {
                        SshError::KeyFileParse {
                            path: path.clone(),
                            source,
                        }
                    }
                })?;

                let auth_res = handle
                    .authenticate_publickey(&cfg.user, Arc::new(key_pair))
                    .await
                    .map_err(SshError::from)?;

                if !auth_res {
                    return Err(SshError::Auth(format!(
                        "server rejected publickey for user {}",
                        cfg.user
                    )));
                }
            }
            SshAuth::Password { password } => {
                if password.is_empty() {
                    return Err(SshError::Auth(
                        "密码为空（keyring 未取到值或未设置）".into(),
                    ));
                }
                let auth_res = handle
                    .authenticate_password(&cfg.user, password)
                    .await
                    .map_err(SshError::from)?;
                if !auth_res {
                    return Err(SshError::Auth(format!(
                        "server rejected password for user {}",
                        cfg.user
                    )));
                }
            }
            SshAuth::Agent => {
                return Err(SshError::Auth("Agent auth not supported (M5+)".into()));
            }
        }

        Ok(Self {
            handle: Arc::new(handle),
        })
    }

    /// 获取底层 russh Handle（仅 channel.rs 内部用）。
    pub(crate) fn handle(&self) -> &Handle<NoopHandler> {
        &self.handle
    }

    /// 跑一条远端命令并等其完成。封装 channel_open + exec + 收 stdout/stderr/exit-code。
    /// 用于 tmux list-sessions 等短命令；不适合长跑（用 open_channel + shell）。
    pub async fn exec_command(&self, command: &str) -> Result<ExecResult, SshError> {
        use russh::ChannelMsg;
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(SshError::Protocol)?;
        channel
            .exec(true, command)
            .await
            .map_err(SshError::Protocol)?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code: Option<u32> = None;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, ext: 1 } => stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                ChannelMsg::Eof => {}
                ChannelMsg::Close => break,
                _ => {}
            }
        }

        Ok(ExecResult {
            stdout,
            stderr,
            exit_code: exit_code.unwrap_or(255),
        })
    }

    /// 主动关闭连接。可选——drop 时会自动关闭。
    pub async fn close(&self) {
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await;
    }

    /// 通过 SFTP 把 `data`（PNG bytes）写到远端 `remote_path`。
    /// 内部 fork 一条独立 SFTP channel，与主 PTY channel 不冲突。
    pub async fn sftp_upload(&self, remote_path: &str, data: &[u8]) -> Result<(), SshError> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(SshError::Protocol)?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(SshError::Protocol)?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| SshError::Sftp(format!("sftp session init failed: {}", e)))?;
        let mut file = sftp
            .create(remote_path)
            .await
            .map_err(|e| SshError::Sftp(format!("sftp create '{}' failed: {}", remote_path, e)))?;
        use tokio::io::AsyncWriteExt;
        file.write_all(data)
            .await
            .map_err(|e| SshError::Sftp(format!("sftp write failed: {}", e)))?;
        file.flush()
            .await
            .map_err(|e| SshError::Sftp(format!("sftp flush failed: {}", e)))?;
        drop(file);
        sftp.close()
            .await
            .map_err(|e| SshError::Sftp(format!("sftp close failed: {}", e)))?;
        Ok(())
    }

    /// 打开一个新的 SSH session channel。
    ///
    /// 返回 Channel 包装，调用方随后可 request_pty + shell。
    pub async fn open_channel(&self) -> Result<crate::channel::Channel, SshError> {
        let chan = self
            .handle
            .channel_open_session()
            .await
            .map_err(SshError::Protocol)?;
        Ok(crate::channel::Channel::new(chan))
    }
}

/// russh Handler trait 的最小实现：M2a 不验证 server key（信任所有）。
///
/// **安全提醒**：这是 M2a 的简化路径。M5 实施 SecretStore 时引入 known_hosts 校验。
pub(crate) struct NoopHandler;

#[async_trait]
impl Handler for NoopHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // M2a: 信任所有 server key（M5 改为 known_hosts 校验）
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use aish_types::HostId;

    use super::*;

    fn mk_cfg(auth: SshAuth) -> HostConfig {
        HostConfig {
            id: HostId::new(),
            label: "test".into(),
            host: "127.0.0.1".into(),
            port: 22,
            user: "test".into(),
            auth,
            env_profile: None,
            os_kind: None,
        }
    }

    #[tokio::test]
    async fn connect_with_empty_password_returns_auth_error() {
        // 空 password = keyring 未取到值的情况，应当 fail-fast
        let cfg = mk_cfg(SshAuth::Password {
            password: String::new(),
        });
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        match result {
            Err(_timeout) => {} // tokio timeout，OK
            Ok(Err(SshError::Auth(msg))) => {
                assert!(
                    msg.contains("密码为空") || msg.contains("empty"),
                    "expected empty password mention, got: {}",
                    msg
                );
            }
            Ok(Err(_)) => {} // TCP connect 先失败（127.0.0.1:22 没 sshd），也接受
            Ok(Ok(_)) => panic!("expected error for empty Password"),
        }
    }

    #[tokio::test]
    async fn connect_with_nonempty_password_attempts_auth() {
        // 非空 password — TCP 多半连不上 127.0.0.1:22，但绝不应该是
        // 「密码为空」或 「not supported」 的错误（因为我们要走到 authenticate_password）
        let cfg = mk_cfg(SshAuth::Password {
            password: "definitely-wrong".into(),
        });
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        match result {
            Err(_timeout) => {} // tokio timeout，OK
            Ok(Err(SshError::Auth(msg))) => {
                assert!(
                    !msg.contains("密码为空") && !msg.contains("not supported"),
                    "should attempt actual auth, got: {}",
                    msg
                );
            }
            Ok(Err(_)) => {} // 任何其他错误（TCP 失败等）OK
            Ok(Ok(_)) => panic!("local 127.0.0.1:22 不应能用任意密码登录"),
        }
    }

    #[tokio::test]
    async fn connect_with_agent_auth_returns_unsupported_error() {
        let cfg = mk_cfg(SshAuth::Agent);
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        match result {
            Err(_timeout) => {}
            Ok(Err(SshError::Auth(msg))) => {
                assert!(
                    msg.contains("Agent") || msg.contains("KeyFile"),
                    "unexpected msg: {}",
                    msg
                );
            }
            Ok(Err(_)) => {}
            Ok(Ok(_)) => panic!("expected error for Agent auth"),
        }
    }

    #[tokio::test]
    async fn connect_with_missing_key_file_returns_error() {
        let cfg = mk_cfg(SshAuth::KeyFile {
            path: PathBuf::from("/nonexistent/key/path/aish_test"),
        });
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), SshClient::connect(&cfg)).await;
        // 期望是 Connect（TCP 先失败）或 KeyFileRead 错误
        match result {
            Err(_timeout) => {}
            Ok(Err(_)) => {} // 任意错误都可接受
            Ok(Ok(_)) => panic!("expected error for missing key"),
        }
    }
}
