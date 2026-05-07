//! TmuxCommand — 结构化 tmux 命令 + 字节构造。

use aish_types::{PaneId, SessionId};

/// 高层 tmux 命令枚举。调用方用 enum 表达意图，
/// build_command 转成实际 `<command> <args>\n` 字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxCommand {
    /// send-keys -t %<pane> -l '<text>'  (literal 模式，无 shell 解析)
    SendText { pane: PaneId, text: String },
    /// send-keys -t %<pane> 0x<hex>  (特殊键用 hex byte)
    SendKey { pane: PaneId, key: Key },
    /// new-window -t $<session> [-n <name>]
    NewWindow {
        session: SessionId,
        name: Option<String>,
    },
    /// kill-pane -t %<pane>
    KillPane { pane: PaneId },
    /// resize-pane -t %<pane> -x <cols> -y <rows>
    ResizePane { pane: PaneId, cols: u16, rows: u16 },
    /// switch-client -t $<session>
    SwitchClient { session: SessionId },
    /// select-pane -t %<pane>
    SelectPane { pane: PaneId },
    /// attach-session -t <name>
    AttachSession { name: String },
    /// list-sessions -F '#{session_id} #{session_name}'
    ListSessions,
}

/// 特殊键 → ANSI / 控制字符 hex byte。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    CtrlC,
    CtrlD,
    CtrlZ,
    CtrlL,
    Enter,
    Tab,
    Esc,
    Backspace,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
}

impl Key {
    /// 该 Key 对应的字节序列（不含包装）。
    /// 单字节：Ctrl+X 等；多字节：方向键的 CSI 序列。
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Key::CtrlC => &[0x03],
            Key::CtrlD => &[0x04],
            Key::CtrlZ => &[0x1a],
            Key::CtrlL => &[0x0c],
            Key::Enter => &[0x0d],
            Key::Tab => &[0x09],
            Key::Esc => &[0x1b],
            Key::Backspace => &[0x7f],
            // CSI sequences (normal mode, M3a 不做 DECCKM)
            Key::ArrowUp => b"\x1b[A",
            Key::ArrowDown => b"\x1b[B",
            Key::ArrowRight => b"\x1b[C",
            Key::ArrowLeft => b"\x1b[D",
            Key::Home => b"\x1b[H",
            Key::End => b"\x1b[F",
            Key::PageUp => b"\x1b[5~",
            Key::PageDown => b"\x1b[6~",
        }
    }
}

/// 把 TmuxCommand 转成可写入 control channel 的字节流（含末尾 \n）。
pub fn build_command(cmd: &TmuxCommand) -> Vec<u8> {
    match cmd {
        TmuxCommand::SendText { pane, text } => {
            // send-keys -t %<n> -l '<escaped-text>'
            let escaped = single_quote_escape(text);
            format!("send-keys -t %{} -l '{}'\n", pane.0, escaped).into_bytes()
        }
        TmuxCommand::SendKey { pane, key } => {
            // tmux send-keys 接受 hex bytes (0xNN) 形式
            // 多字节 key 拆成多个 hex
            let hex: Vec<String> = key
                .as_bytes()
                .iter()
                .map(|b| format!("0x{:02x}", b))
                .collect();
            format!("send-keys -t %{} {}\n", pane.0, hex.join(" ")).into_bytes()
        }
        TmuxCommand::NewWindow { session, name } => match name {
            Some(n) => {
                let escaped = single_quote_escape(n);
                format!("new-window -t '{}' -n '{}'\n", session, escaped).into_bytes()
            }
            None => format!("new-window -t '{}'\n", session).into_bytes(),
        },
        TmuxCommand::KillPane { pane } => format!("kill-pane -t %{}\n", pane.0).into_bytes(),
        TmuxCommand::ResizePane { pane, cols, rows } => {
            format!("resize-pane -t %{} -x {} -y {}\n", pane.0, cols, rows).into_bytes()
        }
        TmuxCommand::SwitchClient { session } => {
            format!("switch-client -t '{}'\n", session).into_bytes()
        }
        TmuxCommand::SelectPane { pane } => format!("select-pane -t %{}\n", pane.0).into_bytes(),
        TmuxCommand::AttachSession { name } => {
            let escaped = single_quote_escape(name);
            format!("attach-session -t '{}'\n", escaped).into_bytes()
        }
        TmuxCommand::ListSessions => b"list-sessions -F '#{session_id} #{session_name}'\n".to_vec(),
    }
}

/// 把字符串放进单引号上下文：把 `'` 替换为 `'\''`（POSIX shell 标准）。
fn single_quote_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_text_literal_mode() {
        let cmd = TmuxCommand::SendText {
            pane: PaneId(3),
            text: "ls\n".into(),
        };
        let bytes = build_command(&cmd);
        assert_eq!(bytes, b"send-keys -t %3 -l 'ls\n'\n");
    }

    #[test]
    fn send_text_escapes_single_quotes() {
        let cmd = TmuxCommand::SendText {
            pane: PaneId(0),
            text: "it's".into(),
        };
        let bytes = build_command(&cmd);
        // it's → it'\''s
        assert_eq!(bytes, b"send-keys -t %0 -l 'it'\\''s'\n");
    }

    #[test]
    fn send_key_ctrl_c() {
        let cmd = TmuxCommand::SendKey {
            pane: PaneId(3),
            key: Key::CtrlC,
        };
        let bytes = build_command(&cmd);
        assert_eq!(bytes, b"send-keys -t %3 0x03\n");
    }

    #[test]
    fn send_key_arrow_up_uses_csi_bytes() {
        let cmd = TmuxCommand::SendKey {
            pane: PaneId(3),
            key: Key::ArrowUp,
        };
        let bytes = build_command(&cmd);
        // \x1b [A → 0x1b 0x5b 0x41
        assert_eq!(bytes, b"send-keys -t %3 0x1b 0x5b 0x41\n");
    }

    #[test]
    fn new_window_with_name() {
        let cmd = TmuxCommand::NewWindow {
            session: SessionId::new("$0"),
            name: Some("editor".into()),
        };
        let bytes = build_command(&cmd);
        assert_eq!(bytes, b"new-window -t '$0' -n 'editor'\n");
    }

    #[test]
    fn new_window_without_name() {
        let cmd = TmuxCommand::NewWindow {
            session: SessionId::new("$0"),
            name: None,
        };
        let bytes = build_command(&cmd);
        assert_eq!(bytes, b"new-window -t '$0'\n");
    }

    #[test]
    fn kill_pane_format() {
        let cmd = TmuxCommand::KillPane { pane: PaneId(7) };
        assert_eq!(build_command(&cmd), b"kill-pane -t %7\n");
    }

    #[test]
    fn resize_pane_format() {
        let cmd = TmuxCommand::ResizePane {
            pane: PaneId(2),
            cols: 120,
            rows: 40,
        };
        assert_eq!(build_command(&cmd), b"resize-pane -t %2 -x 120 -y 40\n");
    }

    #[test]
    fn switch_client_format() {
        let cmd = TmuxCommand::SwitchClient {
            session: SessionId::new("$1"),
        };
        assert_eq!(build_command(&cmd), b"switch-client -t '$1'\n");
    }

    #[test]
    fn select_pane_format() {
        let cmd = TmuxCommand::SelectPane { pane: PaneId(5) };
        assert_eq!(build_command(&cmd), b"select-pane -t %5\n");
    }

    #[test]
    fn attach_session_format() {
        let cmd = TmuxCommand::AttachSession {
            name: "default".into(),
        };
        assert_eq!(build_command(&cmd), b"attach-session -t 'default'\n");
    }

    #[test]
    fn list_sessions_format() {
        let cmd = TmuxCommand::ListSessions;
        assert_eq!(
            build_command(&cmd),
            b"list-sessions -F '#{session_id} #{session_name}'\n"
        );
    }

    #[test]
    fn key_ctrl_d_byte() {
        assert_eq!(Key::CtrlD.as_bytes(), &[0x04]);
    }

    #[test]
    fn key_ctrl_z_byte() {
        assert_eq!(Key::CtrlZ.as_bytes(), &[0x1a]);
    }

    #[test]
    fn key_enter_byte() {
        assert_eq!(Key::Enter.as_bytes(), &[0x0d]);
    }

    #[test]
    fn key_tab_byte() {
        assert_eq!(Key::Tab.as_bytes(), &[0x09]);
    }

    #[test]
    fn key_esc_byte() {
        assert_eq!(Key::Esc.as_bytes(), &[0x1b]);
    }

    #[test]
    fn key_backspace_byte() {
        assert_eq!(Key::Backspace.as_bytes(), &[0x7f]);
    }

    #[test]
    fn key_home_csi() {
        assert_eq!(Key::Home.as_bytes(), b"\x1b[H");
    }

    #[test]
    fn key_pagedown_csi() {
        assert_eq!(Key::PageDown.as_bytes(), b"\x1b[6~");
    }
}
