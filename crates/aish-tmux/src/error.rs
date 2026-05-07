//! TmuxError — 协议解析与状态机错误。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TmuxError {
    /// 协议行不识别（含原始字节用于调试）
    #[error("unknown event line: {0}")]
    UnknownEvent(String),

    /// %output 的 hex 字段解码失败
    #[error("hex decode failed for line: {0}")]
    HexDecodeFailed(String),

    /// `%error` event（tmux 主动报错）
    #[error("tmux protocol error (ts={ts} num={num}): {message}")]
    TmuxProtocolError { ts: u64, num: u64, message: String },

    /// 内部状态不一致（例如 PaneAdded 但 Window 不存在）
    #[error("inconsistent state: {0}")]
    InconsistentState(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_event_display() {
        let e = TmuxError::UnknownEvent("%foobar 1 2".into());
        let s = format!("{}", e);
        assert!(s.contains("unknown event"));
        assert!(s.contains("%foobar"));
    }

    #[test]
    fn protocol_error_includes_message() {
        let e = TmuxError::TmuxProtocolError {
            ts: 42,
            num: 7,
            message: "no such session".into(),
        };
        let s = format!("{}", e);
        assert!(s.contains("ts=42"));
        assert!(s.contains("no such session"));
    }

    #[test]
    fn hex_decode_failed_carries_input() {
        let e = TmuxError::HexDecodeFailed("not-hex".into());
        let s = format!("{}", e);
        assert!(s.contains("not-hex"));
    }

    #[test]
    fn inconsistent_state_shows_detail() {
        let e = TmuxError::InconsistentState("pane @99 missing".into());
        let s = format!("{}", e);
        assert!(s.contains("pane @99 missing"));
    }
}
