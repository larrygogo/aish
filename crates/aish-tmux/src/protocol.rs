//! tmux control mode 协议行解析。
//!
//! ParsedEvent 是 parser 输出，与 TmuxEvent 解耦：
//!   - ParsedEvent 精确对应协议字段
//!   - controller 把 ParsedEvent 转 TmuxEvent + 更新 SessionTree

use aish_types::{PaneId, SessionId, WindowId};
use bytes::Bytes;

use crate::error::TmuxError;

/// parser 解析一行后的输出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedEvent {
    /// %begin <ts> <num> <flags>
    Begin { ts: u64, num: u64, flags: u64 },
    /// %end <ts> <num> <flags>
    End { ts: u64, num: u64, flags: u64 },
    /// %error <ts> <num> <flags> + 后续可能有错误消息（M3a 简化处理）
    Error { ts: u64, num: u64, flags: u64 },

    /// %output %<pane> <hex-bytes>
    Output { pane: PaneId, data: Bytes },

    /// %sessions-changed
    SessionsChanged,
    /// %session-changed $<id> <name>
    SessionChanged { id: SessionId, name: String },
    /// %session-renamed $<id> <name>
    SessionRenamed { id: SessionId, name: String },

    /// %window-add @<id>
    WindowAdd { window: WindowId },
    /// %window-close @<id>
    WindowClose { window: WindowId },
    /// %window-renamed @<id> <name>
    WindowRenamed { window: WindowId, name: String },

    /// %layout-change @<id> <layout-string>  (后续字段忽略)
    LayoutChange { window: WindowId, layout: String },

    /// %pane-mode-changed %<id> — M3a 仅识别但不派生 TmuxEvent
    PaneModeChanged { pane: PaneId },

    /// %client-detached <client>
    ClientDetached,

    /// %exit [<reason>]
    Exit { reason: String },

    /// 命令响应内容行（在 %begin/%end 之间，不以 % 开头）
    /// M3a 不解析具体内容，只标记
    CommandOutput(String),
}

/// 解析单行协议输入。返回 None 表示空行或忽略行（如纯 \r）。
pub fn parse_line(line: &str) -> Result<Option<ParsedEvent>, TmuxError> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return Ok(None);
    }

    if !line.starts_with('%') {
        // 命令响应内容（在 %begin/%end 之间）
        return Ok(Some(ParsedEvent::CommandOutput(line.to_string())));
    }

    // %xxx 事件分发
    let mut parts = line.splitn(2, ' ');
    let head = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");

    let event = match head {
        "%begin" => parse_begin_end(rest, "begin")?,
        "%end" => parse_begin_end(rest, "end")?,
        "%error" => parse_begin_end(rest, "error")?,
        "%output" => parse_output(rest, line)?,
        "%sessions-changed" => ParsedEvent::SessionsChanged,
        "%session-changed" => parse_session_changed(rest)?,
        "%session-renamed" => parse_session_renamed(rest)?,
        "%window-add" => parse_window_add(rest)?,
        "%window-close" => parse_window_close(rest)?,
        "%window-renamed" => parse_window_renamed(rest)?,
        "%layout-change" => parse_layout_change(rest)?,
        "%pane-mode-changed" => parse_pane_mode_changed(rest)?,
        "%client-detached" => ParsedEvent::ClientDetached,
        "%exit" => ParsedEvent::Exit {
            reason: rest.to_string(),
        },
        _ => return Err(TmuxError::UnknownEvent(line.to_string())),
    };

    Ok(Some(event))
}

fn parse_begin_end(rest: &str, kind: &str) -> Result<ParsedEvent, TmuxError> {
    // <ts> <num> <flags>
    let mut iter = rest.split(' ');
    let ts: u64 = iter
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%{} missing ts: {}", kind, rest)))?;
    let num: u64 = iter
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%{} missing num: {}", kind, rest)))?;
    let flags: u64 = iter
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%{} missing flags: {}", kind, rest)))?;
    Ok(match kind {
        "begin" => ParsedEvent::Begin { ts, num, flags },
        "end" => ParsedEvent::End { ts, num, flags },
        "error" => ParsedEvent::Error { ts, num, flags },
        _ => unreachable!(),
    })
}

fn parse_output(rest: &str, full_line: &str) -> Result<ParsedEvent, TmuxError> {
    // %<pane-id> <hex-bytes>
    let mut iter = rest.splitn(2, ' ');
    let pane_str = iter
        .next()
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%output missing pane: {}", rest)))?;
    let hex_str = iter.next().unwrap_or("");

    let pane = parse_pane_id(pane_str)?;
    let data =
        hex_decode(hex_str).map_err(|_| TmuxError::HexDecodeFailed(full_line.to_string()))?;
    Ok(ParsedEvent::Output {
        pane,
        data: Bytes::from(data),
    })
}

fn parse_session_changed(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let mut iter = rest.splitn(2, ' ');
    let id_str = iter
        .next()
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%session-changed missing id: {}", rest)))?;
    let name = iter.next().unwrap_or("").to_string();
    let id = parse_session_id(id_str)?;
    Ok(ParsedEvent::SessionChanged { id, name })
}

fn parse_session_renamed(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let mut iter = rest.splitn(2, ' ');
    let id_str = iter
        .next()
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%session-renamed missing id: {}", rest)))?;
    let name = iter.next().unwrap_or("").to_string();
    let id = parse_session_id(id_str)?;
    Ok(ParsedEvent::SessionRenamed { id, name })
}

fn parse_window_add(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let window = parse_window_id(rest.trim())?;
    Ok(ParsedEvent::WindowAdd { window })
}

fn parse_window_close(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let window = parse_window_id(rest.trim())?;
    Ok(ParsedEvent::WindowClose { window })
}

fn parse_window_renamed(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let mut iter = rest.splitn(2, ' ');
    let id_str = iter
        .next()
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%window-renamed missing id: {}", rest)))?;
    let name = iter.next().unwrap_or("").to_string();
    let window = parse_window_id(id_str)?;
    Ok(ParsedEvent::WindowRenamed { window, name })
}

fn parse_layout_change(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let mut iter = rest.splitn(2, ' ');
    let id_str = iter
        .next()
        .ok_or_else(|| TmuxError::UnknownEvent(format!("%layout-change missing id: {}", rest)))?;
    let layout_and_extra = iter.next().unwrap_or("");
    // layout-change 后续可能有 visible-layout / window-flags，M3a 仅取第一个空白前的部分
    let layout = layout_and_extra.split(' ').next().unwrap_or("").to_string();
    let window = parse_window_id(id_str)?;
    Ok(ParsedEvent::LayoutChange { window, layout })
}

fn parse_pane_mode_changed(rest: &str) -> Result<ParsedEvent, TmuxError> {
    let pane = parse_pane_id(rest.trim())?;
    Ok(ParsedEvent::PaneModeChanged { pane })
}

/// 解析 `$<n>` 形式的 SessionId（tmux 内部 id）。
fn parse_session_id(s: &str) -> Result<SessionId, TmuxError> {
    if s.starts_with('$') {
        // $0, $1 等。SessionId 接受任意 String，直接传整个 "$0"
        Ok(SessionId::new(s))
    } else {
        Err(TmuxError::UnknownEvent(format!(
            "session id must start with $: {}",
            s
        )))
    }
}

/// 解析 `@<n>` 形式的 WindowId。
fn parse_window_id(s: &str) -> Result<WindowId, TmuxError> {
    let s = s.trim_start_matches('@');
    s.parse::<u32>()
        .map(WindowId)
        .map_err(|_| TmuxError::UnknownEvent(format!("invalid window id: {}", s)))
}

/// 解析 `%<n>` 形式的 PaneId。
fn parse_pane_id(s: &str) -> Result<PaneId, TmuxError> {
    let s = s.trim_start_matches('%');
    s.parse::<u32>()
        .map(PaneId)
        .map_err(|_| TmuxError::UnknownEvent(format!("invalid pane id: {}", s)))
}

/// hex 解码：每两个 char → 1 byte。
#[allow(clippy::result_unit_err)]
pub fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_char(bytes[i])?;
        let lo = hex_char(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_char(c: u8) -> Result<u8, ()> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_line_returns_none() {
        assert_eq!(parse_line("").unwrap(), None);
        assert_eq!(parse_line("\r\n").unwrap(), None);
    }

    #[test]
    fn unknown_event_returns_err() {
        let r = parse_line("%foobar 1 2");
        assert!(r.is_err());
    }

    #[test]
    fn parse_begin() {
        let ev = parse_line("%begin 1234 7 0").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::Begin {
                ts: 1234,
                num: 7,
                flags: 0
            }
        );
    }

    #[test]
    fn parse_end() {
        let ev = parse_line("%end 1234 7 0").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::End {
                ts: 1234,
                num: 7,
                flags: 0
            }
        );
    }

    #[test]
    fn parse_output_decodes_hex() {
        // "hi" = 0x68 0x69
        let ev = parse_line("%output %3 6869").unwrap().unwrap();
        if let ParsedEvent::Output { pane, data } = ev {
            assert_eq!(pane, PaneId(3));
            assert_eq!(&data[..], b"hi");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn parse_output_invalid_hex_errors() {
        let r = parse_line("%output %3 not-hex");
        assert!(r.is_err());
    }

    #[test]
    fn parse_sessions_changed() {
        let ev = parse_line("%sessions-changed").unwrap().unwrap();
        assert_eq!(ev, ParsedEvent::SessionsChanged);
    }

    #[test]
    fn parse_session_changed() {
        let ev = parse_line("%session-changed $0 default").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::SessionChanged {
                id: SessionId::new("$0"),
                name: "default".into(),
            }
        );
    }

    #[test]
    fn parse_window_add() {
        let ev = parse_line("%window-add @5").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::WindowAdd {
                window: WindowId(5)
            }
        );
    }

    #[test]
    fn parse_window_close() {
        let ev = parse_line("%window-close @5").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::WindowClose {
                window: WindowId(5)
            }
        );
    }

    #[test]
    fn parse_window_renamed() {
        let ev = parse_line("%window-renamed @5 main").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::WindowRenamed {
                window: WindowId(5),
                name: "main".into(),
            }
        );
    }

    #[test]
    fn parse_layout_change_takes_first_field() {
        let ev = parse_line("%layout-change @5 bb62,278x67,0,0,1 visible flags")
            .unwrap()
            .unwrap();
        assert_eq!(
            ev,
            ParsedEvent::LayoutChange {
                window: WindowId(5),
                layout: "bb62,278x67,0,0,1".into(),
            }
        );
    }

    #[test]
    fn parse_pane_mode_changed() {
        let ev = parse_line("%pane-mode-changed %7").unwrap().unwrap();
        assert_eq!(ev, ParsedEvent::PaneModeChanged { pane: PaneId(7) });
    }

    #[test]
    fn parse_client_detached() {
        let ev = parse_line("%client-detached cli-1").unwrap().unwrap();
        assert_eq!(ev, ParsedEvent::ClientDetached);
    }

    #[test]
    fn parse_exit_with_reason() {
        let ev = parse_line("%exit shutdown").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::Exit {
                reason: "shutdown".into()
            }
        );
    }

    #[test]
    fn parse_exit_without_reason() {
        let ev = parse_line("%exit").unwrap().unwrap();
        assert_eq!(ev, ParsedEvent::Exit { reason: "".into() });
    }

    #[test]
    fn parse_error_event() {
        let ev = parse_line("%error 1234 7 0").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::Error {
                ts: 1234,
                num: 7,
                flags: 0
            }
        );
    }

    #[test]
    fn parse_command_output_no_percent_prefix() {
        let ev = parse_line("$0: 1 windows (created Mon)").unwrap().unwrap();
        assert_eq!(
            ev,
            ParsedEvent::CommandOutput("$0: 1 windows (created Mon)".into())
        );
    }

    #[test]
    fn hex_decode_basic() {
        assert_eq!(hex_decode("00").unwrap(), vec![0x00]);
        assert_eq!(hex_decode("ff").unwrap(), vec![0xff]);
        assert_eq!(
            hex_decode("1234abcd").unwrap(),
            vec![0x12, 0x34, 0xab, 0xcd]
        );
    }

    #[test]
    fn hex_decode_uppercase() {
        assert_eq!(
            hex_decode("DEADBEEF").unwrap(),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
    }

    #[test]
    fn hex_decode_odd_length_errors() {
        assert!(hex_decode("0").is_err());
        assert!(hex_decode("123").is_err());
    }

    #[test]
    fn hex_decode_invalid_char_errors() {
        assert!(hex_decode("xx").is_err());
        assert!(hex_decode("0g").is_err());
    }
}
