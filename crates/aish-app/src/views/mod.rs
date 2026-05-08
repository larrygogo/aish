//! GPUI Views。

#![allow(dead_code)]

mod default_page;
mod home;
mod host_form;
mod session_picker;
mod sidebar_nav;
mod tab_bar;
mod terminal_view;
// tmux_sidebar：M3c 起废弃（功能被 SessionPickerView 弹窗取代）。模块保留备用，不再 pub use。
#[allow(dead_code)]
mod tmux_sidebar;

pub use default_page::DefaultPageView;
pub use host_form::HostFormModal;
pub use session_picker::SessionPickerView;
pub use tab_bar::TabBarView;
pub use terminal_view::TerminalView;
