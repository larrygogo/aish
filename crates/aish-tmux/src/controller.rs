//! TmuxController — pure state machine。
//!
//! 用法：
//!   let mut ctrl = TmuxController::new();
//!   let events = ctrl.feed_bytes(b"%session-changed $0 default\n");
//!   // events: vec![SessionAdded($0), ClientSessionChanged($0)]
//!   let cmd_bytes = ctrl.build_command(&TmuxCommand::SelectPane { pane: PaneId(3) });

use crate::commands::{build_command, TmuxCommand};
use crate::events::TmuxEvent;
use crate::protocol::{parse_line, ParsedEvent};
use crate::types::SessionTree;

pub struct TmuxController {
    state: SessionTree,
    /// 累积未完整的行（按 \n 切分）
    parser_buf: Vec<u8>,
    /// 标记当前是否在 %begin/%end 块内（命令响应内容）
    in_command_response: bool,
}

impl Default for TmuxController {
    fn default() -> Self {
        Self::new()
    }
}

impl TmuxController {
    pub fn new() -> Self {
        Self {
            state: SessionTree::new(),
            parser_buf: Vec::new(),
            in_command_response: false,
        }
    }

    /// 喂入 raw bytes（来自 tmux control channel），返回派生的 events。
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<TmuxEvent> {
        self.parser_buf.extend_from_slice(bytes);
        let mut events = Vec::new();

        // 按 \n 切完整的行
        while let Some(pos) = self.parser_buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.parser_buf.drain(..=pos).collect();
            let line = match std::str::from_utf8(&line_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    tracing::warn!("non-utf8 line in tmux protocol; skipping");
                    continue;
                }
            };

            match parse_line(&line) {
                Ok(Some(parsed)) => {
                    self.handle_parsed(parsed, &mut events);
                }
                Ok(None) => {
                    // 空行，忽略
                }
                Err(e) => {
                    tracing::warn!("tmux parse error: {}", e);
                    // 容错：跳过该行继续
                }
            }
        }

        events
    }

    /// 当前 SessionTree 只读快照。
    pub fn session_tree(&self) -> &SessionTree {
        &self.state
    }

    /// 把 TmuxCommand 转字节流（含末尾 \n），调用方写入 control channel。
    pub fn build_command(&self, cmd: &TmuxCommand) -> Vec<u8> {
        build_command(cmd)
    }

    fn handle_parsed(&mut self, ev: ParsedEvent, events: &mut Vec<TmuxEvent>) {
        match ev {
            ParsedEvent::Begin { .. } => {
                self.in_command_response = true;
            }
            ParsedEvent::End { .. } => {
                self.in_command_response = false;
            }
            ParsedEvent::Error { ts, num, flags: _ } => {
                events.push(TmuxEvent::Exit {
                    reason: format!("tmux error ts={} num={}", ts, num),
                });
            }
            ParsedEvent::Output { pane, data } => {
                events.push(TmuxEvent::PaneOutput { pane, data });
            }
            ParsedEvent::SessionsChanged => {
                // 整体变化的 hint；M3a 不主动同步（M3b 可触发 ListSessions）
            }
            ParsedEvent::SessionChanged { id, name } => {
                let was_new = !self.state.sessions.contains_key(&id);
                if was_new {
                    self.state.add_session(id.clone(), name.clone());
                    events.push(TmuxEvent::SessionAdded(id.clone()));
                }
                self.state.set_active_session(id.clone());
                events.push(TmuxEvent::ClientSessionChanged { session: id });
            }
            ParsedEvent::SessionRenamed { id, name } => {
                if self.state.rename_session(&id, name.clone()) {
                    events.push(TmuxEvent::SessionRenamed { id, name });
                }
            }
            ParsedEvent::WindowAdd { window } => {
                let session_id = match self.state.active_session.clone() {
                    Some(s) => s,
                    None => {
                        tracing::warn!("WindowAdd received but no active session");
                        return;
                    }
                };
                if self
                    .state
                    .add_window(session_id.clone(), window, String::new())
                    .is_ok()
                {
                    events.push(TmuxEvent::WindowAdded {
                        session: session_id,
                        window,
                        name: String::new(),
                    });
                }
            }
            ParsedEvent::WindowClose { window } => {
                if self.state.remove_window(&window) {
                    events.push(TmuxEvent::WindowRemoved(window));
                }
            }
            ParsedEvent::WindowRenamed { window, name } => {
                if self.state.rename_window(&window, name.clone()) {
                    events.push(TmuxEvent::WindowRenamed { window, name });
                }
            }
            ParsedEvent::LayoutChange { window, layout } => {
                if self.state.set_window_layout(&window, layout.clone()) {
                    events.push(TmuxEvent::LayoutChanged { window, layout });
                }
            }
            ParsedEvent::PaneModeChanged { .. } => {
                // M3a 不派生 event
            }
            ParsedEvent::ClientDetached => {
                // M3a 不派生 event；future: TmuxEvent::ClientDetached
            }
            ParsedEvent::Exit { reason } => {
                events.push(TmuxEvent::Exit { reason });
            }
            ParsedEvent::CommandOutput(_) => {
                // 命令响应内容；M3a 不解析
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use aish_types::{PaneId, SessionId, WindowId};

    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    #[test]
    fn feed_session_changed_creates_session_and_active() {
        let mut ctrl = TmuxController::new();
        let events = ctrl.feed_bytes(b"%session-changed $0 default\n");
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], TmuxEvent::SessionAdded(_)));
        assert!(matches!(events[1], TmuxEvent::ClientSessionChanged { .. }));
        assert_eq!(ctrl.session_tree().active_session, Some(sid("$0")));
        assert!(ctrl.session_tree().sessions.contains_key(&sid("$0")));
    }

    #[test]
    fn feed_session_changed_repeat_only_emits_client_changed() {
        let mut ctrl = TmuxController::new();
        ctrl.feed_bytes(b"%session-changed $0 default\n");
        let events = ctrl.feed_bytes(b"%session-changed $0 default\n");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TmuxEvent::ClientSessionChanged { .. }));
    }

    #[test]
    fn feed_window_add_after_session() {
        let mut ctrl = TmuxController::new();
        ctrl.feed_bytes(b"%session-changed $0 default\n%window-add @0\n");
        let tree = ctrl.session_tree();
        assert!(tree.sessions[&sid("$0")].windows.contains_key(&WindowId(0)));
    }

    #[test]
    fn feed_window_add_without_session_is_noop() {
        let mut ctrl = TmuxController::new();
        let events = ctrl.feed_bytes(b"%window-add @0\n");
        // 没 active session，加不进去，无 event
        assert!(events.is_empty());
    }

    #[test]
    fn feed_pane_output_decodes_hex() {
        let mut ctrl = TmuxController::new();
        // "hi" = 0x68 0x69
        let events = ctrl.feed_bytes(b"%output %3 6869\n");
        assert_eq!(events.len(), 1);
        if let TmuxEvent::PaneOutput { pane, data } = &events[0] {
            assert_eq!(*pane, PaneId(3));
            assert_eq!(&data[..], b"hi");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn feed_partial_line_buffers_until_newline() {
        let mut ctrl = TmuxController::new();
        let e1 = ctrl.feed_bytes(b"%session-changed $0 ");
        assert!(e1.is_empty()); // 还没 \n，buffer
        let e2 = ctrl.feed_bytes(b"default\n");
        assert_eq!(e2.len(), 2); // 现在完整
    }

    #[test]
    fn feed_multi_lines_in_one_call() {
        let mut ctrl = TmuxController::new();
        let events =
            ctrl.feed_bytes(b"%session-changed $0 default\n%window-add @0\n%output %0 6869\n");
        assert_eq!(events.len(), 4); // SessionAdded + ClientSessionChanged + WindowAdded + PaneOutput
    }

    #[test]
    fn feed_unknown_event_skipped_with_warn() {
        let mut ctrl = TmuxController::new();
        let events = ctrl.feed_bytes(b"%fakeevent abc\n%session-changed $0 d\n");
        // 第一行被 warn 跳过；第二行正常解析
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn feed_window_close_removes_from_state() {
        let mut ctrl = TmuxController::new();
        ctrl.feed_bytes(b"%session-changed $0 d\n%window-add @0\n");
        let events = ctrl.feed_bytes(b"%window-close @0\n");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TmuxEvent::WindowRemoved(WindowId(0))));
        assert!(ctrl.session_tree().sessions[&sid("$0")].windows.is_empty());
    }

    #[test]
    fn feed_layout_change_stores_raw_string() {
        let mut ctrl = TmuxController::new();
        ctrl.feed_bytes(b"%session-changed $0 d\n%window-add @0\n");
        let events = ctrl.feed_bytes(b"%layout-change @0 bb62,278x67,0,0,1 extra\n");
        assert_eq!(events.len(), 1);
        if let TmuxEvent::LayoutChanged { window, layout } = &events[0] {
            assert_eq!(*window, WindowId(0));
            assert_eq!(layout, "bb62,278x67,0,0,1");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn feed_exit_emits_exit_event() {
        let mut ctrl = TmuxController::new();
        let events = ctrl.feed_bytes(b"%exit shutdown\n");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TmuxEvent::Exit { .. }));
    }

    #[test]
    fn build_command_select_pane_round_trip() {
        let ctrl = TmuxController::new();
        let bytes = ctrl.build_command(&TmuxCommand::SelectPane { pane: PaneId(5) });
        assert_eq!(bytes, b"select-pane -t %5\n");
    }
}
