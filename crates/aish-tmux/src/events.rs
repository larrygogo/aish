//! TmuxEvent — controller 输出的事件，供上层 GPUI 订阅。

use aish_types::{PaneId, SessionId, WindowId};
use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxEvent {
    SessionAdded(SessionId),
    SessionRemoved(SessionId),
    SessionRenamed {
        id: SessionId,
        name: String,
    },
    WindowAdded {
        session: SessionId,
        window: WindowId,
        name: String,
    },
    WindowRemoved(WindowId),
    WindowRenamed {
        window: WindowId,
        name: String,
    },
    PaneAdded {
        window: WindowId,
        pane: PaneId,
    },
    PaneOutput {
        pane: PaneId,
        data: Bytes,
    },
    PaneDied(PaneId),
    LayoutChanged {
        window: WindowId,
        layout: String,
    },
    ClientSessionChanged {
        session: SessionId,
    },
    Exit {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_output_carries_bytes() {
        let ev = TmuxEvent::PaneOutput {
            pane: PaneId(3),
            data: Bytes::from_static(b"hello"),
        };
        if let TmuxEvent::PaneOutput { pane, data } = ev {
            assert_eq!(pane, PaneId(3));
            assert_eq!(&data[..], b"hello");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn events_are_clonable_and_eq() {
        let a = TmuxEvent::PaneDied(PaneId(7));
        let b = a.clone();
        assert_eq!(a, b);
    }
}
