//! SshError 类型，统一所有 aish-ssh 操作的错误。
//!
//! 设计要点：
//! - kind: 粗粒度分类，UI 据此选错误前缀（[error] / [info]）
//! - source: 原始错误链（russh / io / 文件系统）

use std::path::PathBuf;

use thiserror::Error;

/// 粗粒度错误分类，给 UI 决定显示样式用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    /// TCP 连接失败 / DNS 失败 / 连接 refused
    ConnectFailed,
    /// SSH 认证失败（key 不被接受 / 不存在 / 解析失败）
    AuthFailed,
    /// 已建立连接后的 IO 错误（网络断、远端关闭等）
    Io,
    /// 协议层错误（PTY 申请失败、shell 启动失败等）
    Protocol,
}

#[derive(Debug, Error)]
pub enum SshError {
    #[error("connect failed: {0}")]
    Connect(#[source] std::io::Error),

    #[error("auth failed: {0}")]
    Auth(String),

    #[error("key file {path:?} not readable: {source}")]
    KeyFileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("key file {path:?} parse failed: {source}")]
    KeyFileParse {
        path: PathBuf,
        #[source]
        source: russh_keys::Error,
    },

    #[error("ssh protocol error: {0}")]
    Protocol(#[source] russh::Error),

    #[error("io error: {0}")]
    Io(#[source] std::io::Error),
}

impl SshError {
    pub fn kind(&self) -> SshErrorKind {
        match self {
            Self::Connect(_) => SshErrorKind::ConnectFailed,
            Self::Auth(_) | Self::KeyFileRead { .. } | Self::KeyFileParse { .. } => {
                SshErrorKind::AuthFailed
            }
            Self::Io(_) => SshErrorKind::Io,
            Self::Protocol(_) => SshErrorKind::Protocol,
        }
    }
}

impl From<russh::Error> for SshError {
    fn from(err: russh::Error) -> Self {
        // russh 0.x 的 Error 没有细分 connect vs protocol，统一归 Protocol
        // 上层路径里如果是 TCP connect 阶段，应在调用处包成 SshError::Connect
        Self::Protocol(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_error_kind() {
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = SshError::Connect(io);
        assert_eq!(err.kind(), SshErrorKind::ConnectFailed);
    }

    #[test]
    fn auth_error_kind() {
        let err = SshError::Auth("server rejected".into());
        assert_eq!(err.kind(), SshErrorKind::AuthFailed);
    }

    #[test]
    fn key_file_read_error_kind_is_auth() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = SshError::KeyFileRead {
            path: PathBuf::from("/nope"),
            source: io,
        };
        assert_eq!(err.kind(), SshErrorKind::AuthFailed);
    }

    #[test]
    fn key_file_parse_error_kind_is_auth() {
        let err = SshError::KeyFileParse {
            path: PathBuf::from("/home/user/.ssh/id_ed25519"),
            source: russh_keys::Error::KeyIsEncrypted,
        };
        assert_eq!(err.kind(), SshErrorKind::AuthFailed);
    }

    #[test]
    fn io_error_kind() {
        let io = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let err = SshError::Io(io);
        assert_eq!(err.kind(), SshErrorKind::Io);
    }

    #[test]
    fn protocol_error_kind() {
        let err = SshError::Protocol(russh::Error::Disconnect);
        assert_eq!(err.kind(), SshErrorKind::Protocol);
    }

    #[test]
    fn from_russh_error_maps_to_protocol() {
        let err: SshError = russh::Error::Disconnect.into();
        assert_eq!(err.kind(), SshErrorKind::Protocol);
    }

    #[test]
    fn error_display_contains_inner_message() {
        let err = SshError::Auth("bad key".into());
        let s = format!("{}", err);
        assert!(s.contains("auth failed"));
        assert!(s.contains("bad key"));
    }
}
