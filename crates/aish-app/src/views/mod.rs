//! GPUI Views。

mod command_palette;
mod empty_terminal;
mod home;
mod host_form;
mod input_bar;
mod session_picker;
mod settings;
mod sidebar_nav;
mod tab_bar;
mod terminal_view;
// tmux_sidebar：M3c 起废弃（功能被 SessionPickerView 弹窗取代）。
#[allow(dead_code)]
mod tmux_sidebar;

pub use command_palette::CommandPaletteView;
pub use empty_terminal::EmptyTerminalGuideView;
pub use home::HomeView;
pub use host_form::HostFormModal;
pub use input_bar::InputBarView;
pub use session_picker::SessionPickerView;
pub use settings::SettingsView;
pub use sidebar_nav::SidebarNavView;
pub use tab_bar::TabBarView;
pub use terminal_view::TerminalView;
