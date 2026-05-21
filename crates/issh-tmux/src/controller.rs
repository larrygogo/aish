//! TmuxController — pure state machine。
//!
//! 用法：
//!   let mut ctrl = TmuxController::new();
//!   let events = ctrl.feed_bytes(b"%session-changed $0 default\n");
//!   // events: vec![SessionAdded($0), ClientSessionChanged($0)]
//!   let cmd_bytes = ctrl.build_command(&TmuxCommand::SelectPane { pane: PaneId(3) });

use issh_types::PaneId;

use crate::commands::{build_command, TmuxCommand};
use crate::events::TmuxEvent;
use crate::protocol::{parse_line, ParsedEvent};
use crate::types::SessionTree;

/// 从 tmux layout 字串提取所有 pane id（公开供 actor 用）。
///
/// layout 例：
///   - 单 pane: `bb62,80x24,0,0,1`
///   - 水平/垂直 split: `f3a4,80x24,0,0{40x24,0,0,1,40x24,40,0,2}`
///   - 垂直 stack: `cd34,80x24,0,0[80x12,0,0,1,80x12,0,12,2]`
///
/// 每个 leaf 形如 `WxH,X,Y,ID`。算法：把 `{`/`}`/`[`/`]` 当 `,` 切，
/// 扫 token 流找连续 `WxH, N, N, N` 4-token 序列，最后 N 即 pane id。
pub fn extract_pane_ids(layout: &str) -> Vec<PaneId> {
    let normalized: String = layout
        .chars()
        .map(|c| match c {
            '{' | '}' | '[' | ']' => ',',
            other => other,
        })
        .collect();
    let tokens: Vec<&str> = normalized
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();

    let mut ids = Vec::new();
    let mut i = 0;
    while i + 3 < tokens.len() {
        if is_wxh(tokens[i])
            && is_num(tokens[i + 1])
            && is_num(tokens[i + 2])
            && is_num(tokens[i + 3])
        {
            if let Ok(n) = tokens[i + 3].parse::<u32>() {
                ids.push(PaneId(n));
            }
            i += 4;
        } else {
            i += 1;
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn is_wxh(s: &str) -> bool {
    let mut parts = s.split('x');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(a), Some(b), None) if a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok()
    )
}

fn is_num(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

pub struct TmuxController {
    state: SessionTree,
    /// 累积未完整的行（按 \n 切分）
    parser_buf: Vec<u8>,
    /// 标记当前是否在 %begin/%end 块内（命令响应内容）
    in_command_response: bool,
    /// 当前命令响应：(num, 已收 lines)。在 %begin 时建，%end 时取出 emit。
    current_reply: Option<(u64, Vec<String>)>,
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
            current_reply: None,
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
            ParsedEvent::Begin { num, .. } => {
                self.in_command_response = true;
                self.current_reply = Some((num, Vec::new()));
            }
            ParsedEvent::End { num, .. } => {
                self.in_command_response = false;
                if let Some((reply_num, content)) = self.current_reply.take() {
                    if reply_num == num {
                        events.push(TmuxEvent::CommandReply {
                            num: reply_num,
                            content,
                        });
                    }
                }
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
                // attach 现有 session 时 tmux 不会发 %window-add（那只在新建 window 时发），
                // 但会发 %layout-change。如果 window 还没建，从此处补建并提取 pane ids。
                let mut newly_added = false;
                if !self
                    .state
                    .sessions
                    .values()
                    .any(|s| s.windows.contains_key(&window))
                {
                    if let Some(active) = self.state.active_session.clone() {
                        if self
                            .state
                            .add_window(active.clone(), window, String::new())
                            .is_ok()
                        {
                            newly_added = true;
                            events.push(TmuxEvent::WindowAdded {
                                session: active,
                                window,
                                name: String::new(),
                            });
                        }
                    }
                }
                if newly_added {
                    for pane in extract_pane_ids(&layout) {
                        let _ = self.state.add_pane(window, pane);
                    }
                }
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
            ParsedEvent::CommandOutput(line) => {
                if let Some((_, ref mut content)) = self.current_reply {
                    content.push(line);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use issh_types::{PaneId, SessionId, WindowId};

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
    fn feed_pane_output_decodes_octal_escape() {
        let mut ctrl = TmuxController::new();
        // tmux -CC: 字面 + \NNN octal
        let events = ctrl.feed_bytes(b"%output %3 hi\\012\n");
        assert_eq!(events.len(), 1);
        if let TmuxEvent::PaneOutput { pane, data } = &events[0] {
            assert_eq!(*pane, PaneId(3));
            assert_eq!(&data[..], b"hi\n");
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
            ctrl.feed_bytes(b"%session-changed $0 default\n%window-add @0\n%output %0 hi\n");
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

    #[test]
    fn extract_pane_ids_single_pane() {
        let ids = super::extract_pane_ids("bb62,278x67,0,0,1");
        assert_eq!(ids, vec![PaneId(1)]);
    }

    #[test]
    fn extract_pane_ids_horizontal_split() {
        let ids = super::extract_pane_ids("f3a4,80x24,0,0{40x24,0,0,1,40x24,40,0,2}");
        assert_eq!(ids, vec![PaneId(1), PaneId(2)]);
    }

    #[test]
    fn extract_pane_ids_vertical_stack() {
        let ids = super::extract_pane_ids("cd34,80x24,0,0[80x12,0,0,1,80x12,0,12,2]");
        assert_eq!(ids, vec![PaneId(1), PaneId(2)]);
    }

    #[test]
    fn extract_pane_ids_empty_when_no_match() {
        let ids = super::extract_pane_ids("garbage-no-numbers");
        assert!(ids.is_empty());
    }

    #[test]
    fn feed_layout_change_auto_creates_window_and_pane() {
        // attach 现有 session 时 tmux 不发 %window-add，只发 %layout-change。
        // controller 应当从 layout 自动建 window + pane。
        let mut ctrl = TmuxController::new();
        ctrl.feed_bytes(b"%session-changed $0 d\n");
        let events = ctrl.feed_bytes(b"%layout-change @5 bb62,80x24,0,0,7\n");
        // 期望 events: WindowAdded + LayoutChanged
        assert_eq!(events.len(), 2);
        let tree = ctrl.session_tree();
        let sess = &tree.sessions[&sid("$0")];
        assert!(sess.windows.contains_key(&WindowId(5)));
        let win = &sess.windows[&WindowId(5)];
        assert!(win.panes.contains_key(&PaneId(7)));
    }
}
