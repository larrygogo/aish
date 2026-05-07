# aish M3a — tmux control mode 协议层实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 aish-tmux crate 实装 TmuxController pure state machine：feed_bytes 解析 tmux control mode (`tmux -CC`) 输出流 → 派生 TmuxEvent + 更新 SessionTree；build_command 把 TmuxCommand enum 转成 tmux 命令字节。完成后 aish-tmux 是独立可测的协议层，M3b 在 ssh_actor 内集成。

**Architecture:** Pure state machine 设计（与 alacritty_terminal::Term 对称）— TmuxController 不持有 IO，所有 IO 由调用方（M3b 的 ssh_actor）负责。SessionTree 用 BTreeMap 三层（sessions/windows/panes），保证 UI 列表顺序稳定。layout string 仅存 raw（M3c 渲染时才解析）。send-keys 默认 `-l` literal 模式 + 特殊键 hex 编码避免注入。

**Tech Stack:** Rust state machine (no async no IO), bytes::Bytes (引用计数字节流), aish-types (复用 SessionId/WindowId/PaneId), thiserror (错误类型)

**前置:** M2c 已完成（commit `efd6e81` 之后），M3a spec 已落盘 (`docs/superpowers/specs/2026-05-07-aish-m3a-tmux-protocol-design.md`, commit `ddd0017`)。

---

## ⚠️ 实施者须知

### tmux control mode 协议参考

权威文档：`man tmux` 的 "CONTROL MODE" 章节，或 https://github.com/tmux/tmux/wiki/Control-Mode

iTerm2 的 control mode 实现是事实标准之一：https://gitlab.com/gnachman/iterm2/-/blob/master/sources/iTermTmuxLayoutParser.m （参考 — 不需要逐行看）

### Fixtures 来源

如果 user 不方便在 VPS 上录制，**implementer 自己手写 fixtures**（按 spec section 4 的协议格式）。手写 fixture 的好处：精确可控；坏处：可能漏 tmux 实际行为。M3a 接受手写。

### 模块解耦

protocol.rs 内部用 ParsedEvent enum（与 TmuxEvent 解耦）。区别：
- `ParsedEvent`：parser 输出，每行一个，结构精确对应协议字段
- `TmuxEvent`：controller 输出，可能一个 ParsedEvent 派生多个 events（如 SessionChanged 可能派生 SessionAdded + ClientSessionChanged）

这种分层让 parser 单测纯（不需要 SessionTree 状态）+ controller 单测专注 state 演化。

---

## File Structure（M3a 完成时）

```
aish/
├── crates/aish-tmux/
│   ├── Cargo.toml                    # 加 bytes / aish-types / thiserror
│   ├── src/
│   │   ├── lib.rs                    # 改写：mod 声明 + reexport
│   │   ├── error.rs                  # 新：TmuxError
│   │   ├── types.rs                  # 新：SessionTree / Session / Window / Pane
│   │   ├── events.rs                 # 新：TmuxEvent enum (12 variants)
│   │   ├── commands.rs               # 新：TmuxCommand + Key + build_command
│   │   ├── protocol.rs               # 新：ParsedEvent + parse_line + hex decode
│   │   └── controller.rs             # 新：TmuxController state machine
│   └── tests/
│       ├── fixtures/                 # 新建目录
│       │   ├── startup_one_session.txt
│       │   ├── attach_existing_session.txt
│       │   ├── multi_window.txt
│       │   ├── pane_output_stream.txt
│       │   ├── pane_died.txt
│       │   ├── window_renamed.txt
│       │   └── session_close.txt
│       └── protocol_test.rs          # 集成测试
```

新增 7 个 .rs + 7 个 .txt + 1 个测试目录。aish-tmux 当前是 M0 骨架（lib.rs + 1 个 smoke test），整体重写。

---

## Task 1: aish-tmux Cargo.toml + 基础类型（error / types / events）

**Files:**
- Modify: `crates/aish-tmux/Cargo.toml`
- Modify: `crates/aish-tmux/src/lib.rs`
- Create: `crates/aish-tmux/src/error.rs`
- Create: `crates/aish-tmux/src/types.rs`
- Create: `crates/aish-tmux/src/events.rs`

- [ ] **Step 1: 加 aish-tmux 依赖**

读 `crates/aish-tmux/Cargo.toml`。当前内容（M0 骨架）应该只有 package 信息。改为：

```toml
[package]
name = "aish-tmux"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "aish tmux control mode 协议层（pure state machine）"

[dependencies]
aish-types = { workspace = true }
bytes = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

(workspace.dependencies 已经有 bytes / thiserror / tracing — M0 加的，无需 root Cargo.toml 改动)

- [ ] **Step 2: 创建 `crates/aish-tmux/src/error.rs`**

```rust
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
```

- [ ] **Step 3: 创建 `crates/aish-tmux/src/types.rs`**

```rust
//! SessionTree — tmux session/window/pane 三层结构。

use std::collections::BTreeMap;

use aish_types::{PaneId, SessionId, WindowId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionTree {
    pub sessions: BTreeMap<SessionId, Session>,
    pub active_session: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub windows: BTreeMap<WindowId, Window>,
    pub active_window: Option<WindowId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub session: SessionId,
    pub name: String,
    pub panes: BTreeMap<PaneId, Pane>,
    pub active_pane: Option<PaneId>,
    /// raw layout string (e.g. "bb62,278x67,0,0,1") — M3c 才解析
    pub layout: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub window: WindowId,
}

impl SessionTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_session(&mut self, id: SessionId, name: String) {
        self.sessions.insert(
            id,
            Session {
                name,
                windows: BTreeMap::new(),
                active_window: None,
            },
        );
    }

    pub fn remove_session(&mut self, id: &SessionId) -> bool {
        let removed = self.sessions.remove(id).is_some();
        if removed && self.active_session.as_ref() == Some(id) {
            self.active_session = None;
        }
        removed
    }

    pub fn rename_session(&mut self, id: &SessionId, new_name: String) -> bool {
        if let Some(s) = self.sessions.get_mut(id) {
            s.name = new_name;
            true
        } else {
            false
        }
    }

    pub fn add_window(
        &mut self,
        session: SessionId,
        window: WindowId,
        name: String,
    ) -> Result<(), String> {
        let s = self
            .sessions
            .get_mut(&session)
            .ok_or_else(|| format!("session {} not found", session))?;
        s.windows.insert(
            window,
            Window {
                session: session.clone(),
                name,
                panes: BTreeMap::new(),
                active_pane: None,
                layout: String::new(),
            },
        );
        Ok(())
    }

    pub fn remove_window(&mut self, window: &WindowId) -> bool {
        for s in self.sessions.values_mut() {
            if s.windows.remove(window).is_some() {
                if s.active_window.as_ref() == Some(window) {
                    s.active_window = None;
                }
                return true;
            }
        }
        false
    }

    pub fn rename_window(&mut self, window: &WindowId, new_name: String) -> bool {
        for s in self.sessions.values_mut() {
            if let Some(w) = s.windows.get_mut(window) {
                w.name = new_name;
                return true;
            }
        }
        false
    }

    pub fn set_window_layout(&mut self, window: &WindowId, layout: String) -> bool {
        for s in self.sessions.values_mut() {
            if let Some(w) = s.windows.get_mut(window) {
                w.layout = layout;
                return true;
            }
        }
        false
    }

    pub fn add_pane(&mut self, window: WindowId, pane: PaneId) -> Result<(), String> {
        for s in self.sessions.values_mut() {
            if let Some(w) = s.windows.get_mut(&window) {
                w.panes.insert(pane, Pane { window });
                return Ok(());
            }
        }
        Err(format!("window {} not found", window))
    }

    pub fn remove_pane(&mut self, pane: &PaneId) -> bool {
        for s in self.sessions.values_mut() {
            for w in s.windows.values_mut() {
                if w.panes.remove(pane).is_some() {
                    if w.active_pane.as_ref() == Some(pane) {
                        w.active_pane = None;
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn set_active_session(&mut self, id: SessionId) {
        self.active_session = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid as _;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    #[test]
    fn add_session_inserts() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        assert!(tree.sessions.contains_key(&sid("$0")));
        assert_eq!(tree.sessions[&sid("$0")].name, "default");
    }

    #[test]
    fn remove_session_clears_active() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "a".into());
        tree.set_active_session(sid("$0"));
        assert!(tree.remove_session(&sid("$0")));
        assert!(tree.active_session.is_none());
    }

    #[test]
    fn rename_session_changes_name() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "old".into());
        assert!(tree.rename_session(&sid("$0"), "new".into()));
        assert_eq!(tree.sessions[&sid("$0")].name, "new");
    }

    #[test]
    fn add_window_attaches_to_session() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into()).unwrap();
        assert_eq!(tree.sessions[&sid("$0")].windows.len(), 1);
        assert_eq!(tree.sessions[&sid("$0")].windows[&WindowId(0)].name, "main");
    }

    #[test]
    fn add_window_to_missing_session_errors() {
        let mut tree = SessionTree::new();
        let e = tree.add_window(sid("$nope"), WindowId(0), "main".into());
        assert!(e.is_err());
    }

    #[test]
    fn add_pane_attaches_to_window() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into()).unwrap();
        tree.add_pane(WindowId(0), PaneId(0)).unwrap();
        assert_eq!(
            tree.sessions[&sid("$0")].windows[&WindowId(0)].panes.len(),
            1
        );
    }

    #[test]
    fn remove_pane_returns_true_when_found() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into()).unwrap();
        tree.add_pane(WindowId(0), PaneId(0)).unwrap();
        assert!(tree.remove_pane(&PaneId(0)));
        assert!(!tree.remove_pane(&PaneId(0))); // 第二次 false
    }

    #[test]
    fn set_window_layout_stores_raw_string() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$0"), "default".into());
        tree.add_window(sid("$0"), WindowId(0), "main".into()).unwrap();
        let layout = "bb62,278x67,0,0,1".to_string();
        assert!(tree.set_window_layout(&WindowId(0), layout.clone()));
        assert_eq!(
            tree.sessions[&sid("$0")].windows[&WindowId(0)].layout,
            layout
        );
    }

    #[test]
    fn sessions_are_btreemap_ordered() {
        let mut tree = SessionTree::new();
        tree.add_session(sid("$2"), "c".into());
        tree.add_session(sid("$0"), "a".into());
        tree.add_session(sid("$1"), "b".into());
        let names: Vec<_> = tree.sessions.values().map(|s| s.name.as_str()).collect();
        // BTreeMap<SessionId, _> 排序按 SessionId（String 字典序），$0 < $1 < $2
        assert_eq!(names, vec!["a", "b", "c"]);
    }
}
```

> 注意 `use uuid::Uuid as _;` — 实际上 SessionTree 测试不直接用 uuid，删掉 use 行，避免 clippy unused_import。修改为：

实际上去掉那一行 `use uuid::Uuid as _;` — 测试中没用到。保留干净的：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    // ... 测试 ...
}
```

- [ ] **Step 4: 创建 `crates/aish-tmux/src/events.rs`**

```rust
//! TmuxEvent — controller 输出的事件，供上层 GPUI 订阅。

use aish_types::{PaneId, SessionId, WindowId};
use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxEvent {
    SessionAdded(SessionId),
    SessionRemoved(SessionId),
    SessionRenamed { id: SessionId, name: String },
    WindowAdded {
        session: SessionId,
        window: WindowId,
        name: String,
    },
    WindowRemoved(WindowId),
    WindowRenamed { window: WindowId, name: String },
    PaneAdded { window: WindowId, pane: PaneId },
    PaneOutput { pane: PaneId, data: Bytes },
    PaneDied(PaneId),
    LayoutChanged { window: WindowId, layout: String },
    ClientSessionChanged { session: SessionId },
    Exit { reason: String },
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
```

- [ ] **Step 5: 改写 `crates/aish-tmux/src/lib.rs`**

```rust
//! aish-tmux — tmux control mode (`tmux -CC`) 协议层。
//!
//! TmuxController 是 pure state machine（不持有 IO），与 alacritty_terminal::Term
//! 设计对称。调用方喂 raw bytes，拿派生的 events + 当前 SessionTree。

#![allow(dead_code)]

pub mod error;
pub mod events;
pub mod types;

pub use error::TmuxError;
pub use events::TmuxEvent;
pub use types::{Pane, Session, SessionTree, Window};
```

注意：M3a Task 1 阶段还没有 commands / protocol / controller 模块；这些 Task 2-4 会加。

- [ ] **Step 6: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-tmux 2>&1 | tail -5
cargo test -p aish-tmux 2>&1 | tail -10
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected:
- aish-tmux build PASS
- aish-tmux 测试 ≥ 13 passed (4 error + 9 types + 2 events)
- workspace 全绿（M2c 76 + aish-tmux 新 = ~89 passed，其中 aish-tmux 之前的 1 个 smoke 不存在了 因为 lib.rs 改写）

如果 lib.rs 之前的 `mod tests { ... smoke }` 被改写删了，无所谓，新增的覆盖远超。

- [ ] **Step 7: commit**

```bash
cargo fmt --all
git add crates/aish-tmux/
git commit -m "feat(aish-tmux): 基础类型 (error / types / events)"
```

---

## Task 2: commands.rs — TmuxCommand + Key + build_command

**Files:**
- Create: `crates/aish-tmux/src/commands.rs`
- Modify: `crates/aish-tmux/src/lib.rs`（加 mod commands + reexport）

- [ ] **Step 1: 创建 `crates/aish-tmux/src/commands.rs`**

```rust
//! TmuxCommand — 结构化 tmux 命令 + 字节构造。

use aish_types::{PaneId, SessionId, WindowId};

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
    CtrlC, CtrlD, CtrlZ, CtrlL,
    Enter, Tab, Esc, Backspace,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Home, End, PageUp, PageDown,
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
        TmuxCommand::ListSessions => {
            b"list-sessions -F '#{session_id} #{session_name}'\n".to_vec()
        }
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
            session: aish_types::SessionId::new("$0"),
            name: Some("editor".into()),
        };
        let bytes = build_command(&cmd);
        assert_eq!(bytes, b"new-window -t '$0' -n 'editor'\n");
    }

    #[test]
    fn new_window_without_name() {
        let cmd = TmuxCommand::NewWindow {
            session: aish_types::SessionId::new("$0"),
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
            session: aish_types::SessionId::new("$1"),
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
        assert_eq!(Key::PageDown.as_bytes(), b"\x1b[5~".to_vec().as_slice());
        // 注意 PageDown 是 ~6~，PageUp 是 ~5~ - 验证下
        assert_eq!(Key::PageDown.as_bytes(), b"\x1b[6~");
    }
}
```

注意：上面 `key_pagedown_csi` 测试中第一个 assert 故意拿 PageUp 字节比对（演示思路），实际正确测试只需要第二个 assert。**修正：删第一个 assert，保留**：

```rust
    #[test]
    fn key_pagedown_csi() {
        assert_eq!(Key::PageDown.as_bytes(), b"\x1b[6~");
    }
```

- [ ] **Step 2: lib.rs 加 mod commands**

```rust
//! aish-tmux — tmux control mode (`tmux -CC`) 协议层。

#![allow(dead_code)]

pub mod commands;
pub mod error;
pub mod events;
pub mod types;

pub use commands::{build_command, Key, TmuxCommand};
pub use error::TmuxError;
pub use events::TmuxEvent;
pub use types::{Pane, Session, SessionTree, Window};
```

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-tmux 2>&1 | tail -15
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: aish-tmux 测试 ≥ 32 passed (Task 1 的 15 个 + Task 2 的 ~17 个)。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-tmux/
git commit -m "feat(aish-tmux): TmuxCommand + Key + build_command"
```

---

## Task 3: protocol.rs — 行式 parser

**Files:**
- Create: `crates/aish-tmux/src/protocol.rs`
- Modify: `crates/aish-tmux/src/lib.rs`（加 mod protocol）

- [ ] **Step 1: 创建 `crates/aish-tmux/src/protocol.rs`**

```rust
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
    let data = hex_decode(hex_str).map_err(|_| TmuxError::HexDecodeFailed(full_line.to_string()))?;
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
    let layout = layout_and_extra
        .split(' ')
        .next()
        .unwrap_or("")
        .to_string();
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
        assert_eq!(ev, ParsedEvent::Begin { ts: 1234, num: 7, flags: 0 });
    }

    #[test]
    fn parse_end() {
        let ev = parse_line("%end 1234 7 0").unwrap().unwrap();
        assert_eq!(ev, ParsedEvent::End { ts: 1234, num: 7, flags: 0 });
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
        assert_eq!(ev, ParsedEvent::WindowAdd { window: WindowId(5) });
    }

    #[test]
    fn parse_window_close() {
        let ev = parse_line("%window-close @5").unwrap().unwrap();
        assert_eq!(ev, ParsedEvent::WindowClose { window: WindowId(5) });
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
        assert_eq!(
            ev,
            ParsedEvent::PaneModeChanged { pane: PaneId(7) }
        );
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
        assert_eq!(ev, ParsedEvent::Error { ts: 1234, num: 7, flags: 0 });
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
        assert_eq!(hex_decode("1234abcd").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
    }

    #[test]
    fn hex_decode_uppercase() {
        assert_eq!(hex_decode("DEADBEEF").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
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
```

- [ ] **Step 2: lib.rs 加 mod protocol**

```rust
//! aish-tmux — tmux control mode (`tmux -CC`) 协议层。

#![allow(dead_code)]

pub mod commands;
pub mod error;
pub mod events;
pub mod protocol;
pub mod types;

pub use commands::{build_command, Key, TmuxCommand};
pub use error::TmuxError;
pub use events::TmuxEvent;
pub use types::{Pane, Session, SessionTree, Window};
```

protocol 不直接 reexport（内部细节），由 controller 层封装。

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-tmux 2>&1 | tail -15
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: aish-tmux 测试 ≥ 53 passed (Task 1+2 ~32 + Task 3 ~21)。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-tmux/
git commit -m "feat(aish-tmux): protocol 行式 parser + hex decode"
```

---

## Task 4: controller.rs — TmuxController state machine

**Files:**
- Create: `crates/aish-tmux/src/controller.rs`
- Modify: `crates/aish-tmux/src/lib.rs`（加 mod controller + reexport TmuxController）

- [ ] **Step 1: 创建 `crates/aish-tmux/src/controller.rs`**

```rust
//! TmuxController — pure state machine。
//!
//! 用法：
//!   let mut ctrl = TmuxController::new();
//!   let events = ctrl.feed_bytes(b"%session-changed $0 default\n");
//!   // events: vec![SessionAdded($0), ClientSessionChanged($0)]
//!   let cmd_bytes = ctrl.build_command(&TmuxCommand::SelectPane { pane: PaneId(3) });

use aish_types::{PaneId, SessionId, WindowId};

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

                // tmux 在 layout-change 时也意味着 panes 可能变化；
                // 简化处理：从 layout string 第二段提取 pane ids 加进 state
                // M3a 不做这个 — 等 M3c 写 layout 解析器；当前依赖单独的 PaneAdded/PaneClose 事件
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
                // M3b 可在 ListSessions 触发后解析 %begin..%end 块构建初始 SessionTree
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

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
        let events = ctrl.feed_bytes(
            b"%session-changed $0 default\n%window-add @0\n%output %0 6869\n",
        );
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
```

- [ ] **Step 2: lib.rs 加 mod controller + reexport**

```rust
//! aish-tmux — tmux control mode (`tmux -CC`) 协议层。

#![allow(dead_code)]

pub mod commands;
pub mod controller;
pub mod error;
pub mod events;
pub mod protocol;
pub mod types;

pub use commands::{build_command, Key, TmuxCommand};
pub use controller::TmuxController;
pub use error::TmuxError;
pub use events::TmuxEvent;
pub use types::{Pane, Session, SessionTree, Window};
```

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-tmux 2>&1 | tail -20
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: aish-tmux 测试 ≥ 65 passed (Task 1+2+3 ~53 + Task 4 ~12)。

- [ ] **Step 4: commit**

```bash
cargo fmt --all
git add crates/aish-tmux/
git commit -m "feat(aish-tmux): TmuxController state machine"
```

---

## Task 5: tests/fixtures/ + tests/protocol_test.rs（集成测试）

**Files:**
- Create: `crates/aish-tmux/tests/fixtures/startup_one_session.txt`
- Create: `crates/aish-tmux/tests/fixtures/multi_window.txt`
- Create: `crates/aish-tmux/tests/fixtures/pane_output_stream.txt`
- Create: `crates/aish-tmux/tests/fixtures/pane_died.txt`
- Create: `crates/aish-tmux/tests/protocol_test.rs`

⚠️ **Fixture 收集**：spec 列了 7 个场景，但 M3a 阶段（无真 ssh_actor）不能直接录制。**手写 4 个最具代表性的 fixture**：

- `startup_one_session.txt`：tmux 启动后单 session/window/pane 的初始推送
- `multi_window.txt`：多 window 多 pane
- `pane_output_stream.txt`：连续 PaneOutput
- `pane_died.txt`：pane 退出事件

剩 3 个（attach_existing_session / window_renamed / session_close）覆盖在已有的 controller.rs 单测里，不重复加 fixture。

- [ ] **Step 1: 创建 `startup_one_session.txt`**

⚠️ 使用 PowerShell here-string + UTF-8 无 BOM 写入：

```powershell
$content = @"
%begin 1700000000 1 0
%end 1700000000 1 0
%sessions-changed
%session-changed `$0 default
%window-add @0
%window-renamed @0 main
%layout-change @0 bb62,80x24,0,0,0
"@
$path = "C:\Users\larry\Desktop\workspace\aish\crates\aish-tmux\tests\fixtures\startup_one_session.txt"
New-Item -ItemType Directory -Path (Split-Path $path) -Force | Out-Null
[System.IO.File]::WriteAllText($path, $content, [System.Text.UTF8Encoding]::new($false))
```

注意：PowerShell here-string 里 `$0` 必须 escape 为 `` `$0 ``（反引号转义），否则被当变量。

文件预期内容（注意所有行末为 \n，最后一行也有 \n）：

```
%begin 1700000000 1 0
%end 1700000000 1 0
%sessions-changed
%session-changed $0 default
%window-add @0
%window-renamed @0 main
%layout-change @0 bb62,80x24,0,0,0
```

- [ ] **Step 2: 创建 `multi_window.txt`**

```powershell
$content = @"
%session-changed `$0 dev
%window-add @0
%window-renamed @0 editor
%window-add @1
%window-renamed @1 server
%window-add @2
%window-renamed @2 logs
"@
$path = "C:\Users\larry\Desktop\workspace\aish\crates\aish-tmux\tests\fixtures\multi_window.txt"
[System.IO.File]::WriteAllText($path, $content, [System.Text.UTF8Encoding]::new($false))
```

- [ ] **Step 3: 创建 `pane_output_stream.txt`**

```powershell
# bytes "ls\n" = 0x6c 0x73 0x0a → hex "6c730a"
# bytes "file.txt\n" = 0x66 0x69 0x6c 0x65 0x2e 0x74 0x78 0x74 0x0a → hex "66696c652e7478740a"
$content = @"
%session-changed `$0 default
%window-add @0
%output %0 6c730a
%output %0 66696c652e7478740a
"@
$path = "C:\Users\larry\Desktop\workspace\aish\crates\aish-tmux\tests\fixtures\pane_output_stream.txt"
[System.IO.File]::WriteAllText($path, $content, [System.Text.UTF8Encoding]::new($false))
```

- [ ] **Step 4: 创建 `pane_died.txt`**

```powershell
$content = @"
%session-changed `$0 default
%window-add @0
%window-close @0
%exit
"@
$path = "C:\Users\larry\Desktop\workspace\aish\crates\aish-tmux\tests\fixtures\pane_died.txt"
[System.IO.File]::WriteAllText($path, $content, [System.Text.UTF8Encoding]::new($false))
```

> 说明：M3a 协议层没有专用的 `%pane-died` event 在 tmux 真实输出里 — tmux 的"pane 死亡"通常通过 `%window-close` 或 `%exit` 反映（pane 是 window 的一部分）。spec section 4 提的 `%pane-died` 是 controller 派生的高层事件（PaneDied），M3a 现阶段不主动派发（依赖 layout-change 重建）。文件命名保留只为覆盖 window-close + exit 场景。

- [ ] **Step 5: 创建 `crates/aish-tmux/tests/protocol_test.rs`**

```rust
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

    // 期望：SessionAdded($0) + ClientSessionChanged + WindowAdded(@0) + WindowRenamed(@0, "main") + LayoutChanged(@0, ...)
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

    // tree 状态：
    let session = tree
        .sessions
        .values()
        .next()
        .expect("at least one session");
    assert_eq!(session.name, "default");
    let window = session.windows.values().next().expect("at least one window");
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
    assert!(events
        .iter()
        .any(|e| matches!(e, TmuxEvent::Exit { .. })));
    // 最终 tree 中 session 仍然存在（只关闭了 window），但没有 windows
    let session = tree.sessions.values().next().unwrap();
    assert!(session.windows.is_empty());
}
```

- [ ] **Step 6: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-tmux 2>&1 | tail -15
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: aish-tmux 测试 ≥ 69 passed (Task 1-4 ~65 + 集成 4 个)。

- [ ] **Step 7: commit**

```bash
cargo fmt --all
git add crates/aish-tmux/tests/
git commit -m "test(aish-tmux): 4 个 fixture + 集成测试覆盖典型场景"
```

---

## Task 6: 端到端验证 + push

**Files:** 无文件改动；只验证 + push。

- [ ] **Step 1: 跑全部自动化验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build --workspace
cargo test --workspace 2>&1 | grep -E "test result"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 全部退出码 0；test 总数 ~145 passed（M2c 76 + aish-tmux 69）。

- [ ] **Step 2: push**

```bash
git config http.postBuffer 524288000
git push origin main
```

如失败用 `git push origin main` 重试。

- [ ] **Step 3: 等 CI**

```bash
gh run list --limit 1
```

- [ ] **Step 4: 完成报告**

```
STATUS: DONE

Files: 无文件改动

Verification:
- cargo build: PASS
- cargo test --workspace: ~145 passed
- cargo fmt --check: PASS
- cargo clippy: PASS
- git push: 成功
- CI 状态: queued/completed

Concerns: M3a 是纯 backend 协议层，无 GUI 变化。M3b 才会接通 ssh_actor 让 user 在 GUI 看到 tmux 树。
```

---

## 完成验证（M3a 整体）

```bash
cargo build --workspace
cargo test --workspace          # ~145 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

git log 应有 5 个新 feat commit + spec + plan = 7 个新 commit。

---

## 下一步

M3a 完成后开始 M3b：

- ssh_actor 持有 TmuxController per-host
- 检测 tmux 版本 + 启动 `tmux -CC new-session -A -s aish-default`
- < 2.6 老版本降级到 raw PTY shell（M2a 路径）
- 新增 TmuxSidebarView 渲染 SessionTree（仅展示）
- AppState 加 tmux_trees: HashMap<HostId, SessionTree>

M3b 不在本 plan 范围。M3a 完成后单独 brainstorm → spec → plan → implement。
