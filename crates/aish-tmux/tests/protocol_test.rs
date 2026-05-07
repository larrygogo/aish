//! 集成测试：用 fixtures 跑 TmuxController 端到端。

use aish_tmux::{TmuxController, TmuxEvent};

fn run_fixture(name: &str) -> (Vec<TmuxEvent>, aish_tmux::SessionTree) {
    let path = format!("tests/fixtures/{}", name);
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("missing fixture: {}", path));
    let mut ctrl = TmuxController::new();
    let events = ctrl.feed_bytes(&bytes);
    (events, ctrl.session_tree().clone())
}

#[test]
fn startup_one_session_creates_default() {
    let (events, tree) = run_fixture("startup_one_session.txt");

    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::SessionAdded(_))));
    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::ClientSessionChanged { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::WindowAdded { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::WindowRenamed { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::LayoutChanged { .. })));

    let session = tree.sessions.values().next().expect("at least one session");
    assert_eq!(session.name, "default");
    let window = session
        .windows
        .values()
        .next()
        .expect("at least one window");
    assert_eq!(window.name, "main");
    assert!(!window.layout.is_empty());
}

#[test]
fn multi_window_creates_three_windows() {
    let (_events, tree) = run_fixture("multi_window.txt");
    let session = tree.sessions.values().next().unwrap();
    assert_eq!(session.name, "dev");
    assert_eq!(session.windows.len(), 3);
    let names: Vec<_> = session.windows.values().map(|w| w.name.as_str()).collect();
    // BTreeMap 按 WindowId(u32) 排序：@0 @1 @2 → editor/server/logs
    assert_eq!(names, vec!["editor", "server", "logs"]);
}

#[test]
fn pane_output_stream_emits_two_outputs() {
    let (events, _tree) = run_fixture("pane_output_stream.txt");
    let outputs: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let TmuxEvent::PaneOutput { data, .. } = e {
                Some(data.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(outputs.len(), 2);
    assert_eq!(&outputs[0][..], b"ls\n");
    assert_eq!(&outputs[1][..], b"file.txt\n");
}

#[test]
fn pane_died_window_close_then_exit() {
    let (events, tree) = run_fixture("pane_died.txt");
    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::WindowRemoved(_))));
    assert!(events.iter().any(|e| matches!(e, TmuxEvent::Exit { .. })));
    let session = tree.sessions.values().next().unwrap();
    assert!(session.windows.is_empty());
}
