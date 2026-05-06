//! aish-app 内部 App State：M1 阶段的 Model。
//!
//! 注意：此处的 `HostId` 是 M1 的 mock 类型（u32 newtype），
//! 与 `aish_types::HostId`（UUID）不冲突——M2 接入真实 SSH 时再切换。

// M1 阶段类型仅在测试中使用，暂时允许 dead_code。
#![allow(dead_code)]

use std::collections::HashMap;

/// M1 阶段的 mock host 标识（u32 newtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostId(pub u32);

/// M1 阶段的 mock host（M2 时换成 aish_types::HostConfig）。
#[derive(Debug, Clone)]
pub struct MockHost {
    pub id: HostId,
    pub label: String,
}

/// 从 bridge 推回 GPUI 的事件（M2 时会扩展更多 variant）。
#[derive(Debug, Clone)]
pub enum MockEvent {
    PaneOutput { host: HostId, line: String },
}

/// 单一 root Model：所有 UI 共享状态的 source of truth。
#[derive(Debug, Default)]
pub struct AppState {
    pub hosts: Vec<MockHost>,
    pub selected: Option<HostId>,
    pub pane_logs: HashMap<HostId, Vec<String>>,
}

impl AppState {
    /// 用三个固定 mock host 初始化。
    pub fn with_mock_hosts() -> Self {
        Self {
            hosts: vec![
                MockHost {
                    id: HostId(1),
                    label: "server-A (mock)".into(),
                },
                MockHost {
                    id: HostId(2),
                    label: "server-B (mock)".into(),
                },
                MockHost {
                    id: HostId(3),
                    label: "server-C (mock)".into(),
                },
            ],
            selected: None,
            pane_logs: HashMap::new(),
        }
    }

    /// 切换选中 host。
    pub fn select_host(&mut self, id: HostId) {
        self.selected = Some(id);
    }

    /// 追加一行到指定 host 的 pane log。
    pub fn append_log(&mut self, host: HostId, line: String) {
        self.pane_logs.entry(host).or_default().push(line);
    }

    /// 读指定 host 的 pane log（若无则返回空切片）。
    pub fn logs_of(&self, host: HostId) -> &[String] {
        self.pane_logs
            .get(&host)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_mock_hosts_returns_three() {
        let state = AppState::with_mock_hosts();
        assert_eq!(state.hosts.len(), 3);
        assert_eq!(state.hosts[0].id, HostId(1));
        assert_eq!(state.hosts[2].label, "server-C (mock)");
        assert!(state.selected.is_none());
        assert!(state.pane_logs.is_empty());
    }

    #[test]
    fn select_host_sets_selected() {
        let mut state = AppState::with_mock_hosts();
        state.select_host(HostId(2));
        assert_eq!(state.selected, Some(HostId(2)));
    }

    #[test]
    fn append_log_creates_entry_for_new_host() {
        let mut state = AppState::default();
        state.append_log(HostId(7), "hello".into());
        assert_eq!(state.logs_of(HostId(7)), &["hello".to_string()]);
    }

    #[test]
    fn append_log_accumulates_per_host() {
        let mut state = AppState::default();
        state.append_log(HostId(1), "line A1".into());
        state.append_log(HostId(2), "line B1".into());
        state.append_log(HostId(1), "line A2".into());
        assert_eq!(
            state.logs_of(HostId(1)),
            &["line A1".to_string(), "line A2".into()]
        );
        assert_eq!(state.logs_of(HostId(2)), &["line B1".to_string()]);
    }

    #[test]
    fn logs_of_missing_host_returns_empty_slice() {
        let state = AppState::default();
        assert!(state.logs_of(HostId(99)).is_empty());
    }
}
