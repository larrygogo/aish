//! Channel — russh::Channel 的 PTY 包装。

use russh::client::Msg;
use russh::ChannelMsg;

use crate::error::SshError;

/// Channel 包装一个已开的 SSH channel。
///
/// 典型流程：
///   1. SshClient::open_channel() 拿到 Channel
///   2. request_pty(120, 40, "xterm-256color")
///   3. shell()
///   4. 在 select! 里轮 wait() (read) + data() (write)
pub struct Channel {
    inner: russh::Channel<Msg>,
}

impl Channel {
    pub(crate) fn new(inner: russh::Channel<Msg>) -> Self {
        Self { inner }
    }

    /// 申请 PTY。
    ///
    /// russh 0.46 签名：request_pty(&self, want_reply, term, col_width, row_height, pix_width, pix_height, terminal_modes)
    pub async fn request_pty(&self, cols: u16, rows: u16, term: &str) -> Result<(), SshError> {
        self.inner
            .request_pty(
                false, // want_reply
                term,
                cols as u32, // col_width
                rows as u32, // row_height
                0,           // pix_width
                0,           // pix_height
                &[],         // terminal_modes
            )
            .await
            .map_err(SshError::Protocol)
    }

    /// 启动 shell（必须先 request_pty）。
    pub async fn shell(&self) -> Result<(), SshError> {
        self.inner
            .request_shell(false)
            .await
            .map_err(SshError::Protocol)
    }

    /// 写入数据到 PTY（键盘输入）。
    ///
    /// russh 0.46 的 data() 接受 AsyncRead + Unpin，用 Cursor 包装 &[u8]。
    pub async fn data(&self, bytes: &[u8]) -> Result<(), SshError> {
        use std::io::Cursor;
        self.inner
            .data(Cursor::new(bytes))
            .await
            .map_err(SshError::Protocol)
    }

    /// 等待下一个 channel 消息（read）。返回 None = channel 已关闭。
    pub async fn wait(&mut self) -> Option<ChannelMsg> {
        self.inner.wait().await
    }

    /// 发送 EOF 信号。
    pub async fn eof(&self) -> Result<(), SshError> {
        self.inner.eof().await.map_err(SshError::Protocol)
    }

    /// 关闭 channel。
    pub async fn close(&self) -> Result<(), SshError> {
        self.inner.close().await.map_err(SshError::Protocol)
    }

    /// 在 channel 上执行命令（非交互式，无 PTY）。
    ///
    /// `want_reply` 传 true 等待服务端确认。
    /// russh 0.46 Channel::exec(want_reply, command) wrapper。
    pub async fn run_cmd(
        &mut self,
        want_reply: bool,
        command: impl Into<String>,
    ) -> Result<(), SshError> {
        self.inner
            .exec(want_reply, command.into())
            .await
            .map_err(SshError::Protocol)
    }

    /// 通知远端 PTY 大小变化（SIGWINCH）。
    ///
    /// russh 0.46 签名：window_change(col_width, row_height, pix_width, pix_height)
    pub async fn window_change(
        &self,
        cols: u32,
        rows: u32,
        pix_width: u32,
        pix_height: u32,
    ) -> Result<(), SshError> {
        self.inner
            .window_change(cols, rows, pix_width, pix_height)
            .await
            .map_err(SshError::Protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    /// 类型大小不作为正确性测试，只为编译验证 Channel/SshError 类型可正常引用。
    #[test]
    fn channel_compiles() {
        assert!(size_of::<Channel>() > 0);
        assert!(size_of::<SshError>() > 0);
    }
}
