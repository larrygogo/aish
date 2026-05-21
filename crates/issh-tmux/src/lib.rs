//! issh-tmux — tmux control mode (`tmux -CC`) 协议层。
//!
//! TmuxController 是 pure state machine（不持有 IO），与 alacritty_terminal::Term
//! 设计对称。调用方喂 raw bytes，拿派生的 events + 当前 SessionTree。
//!
//! # M3-archived（2026-05-07）
//!
//! issh-app 已切换到 raw `tmux attach` 路径（让 tmux 自身画状态栏 + 窗口栏），
//! `controller` / `protocol` / `events` / `types::SessionTree` 这套 -CC 控制
//! 模式的状态机当前没有 caller。代码保留在仓库里：
//! - 如未来要做"多 pane 在 GPUI 端独立渲染"或"远端 detach 联动"，可重启用
//! - 单元测试持续运行，保证 parser 行为不退化
//!
//! `commands::TmuxCommand` 部分变体（如 `RefreshClient`、`AttachSession`）
//! 同期未被使用，但同样保留待用。

#![allow(dead_code)]

pub mod commands;
pub mod controller;
pub mod error;
pub mod events;
pub mod protocol;
pub mod types;

pub use commands::{build_command, Key, TmuxCommand};
pub use controller::{extract_pane_ids, TmuxController};
pub use error::TmuxError;
pub use events::TmuxEvent;
pub use types::{Pane, Session, SessionTree, Window};
