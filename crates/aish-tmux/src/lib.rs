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
