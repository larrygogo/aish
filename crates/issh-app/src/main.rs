//! issh 主入口。M2a 起接入真 SSH。

mod app;
mod app_state_file;
mod avatar;
mod bridge;
mod logo;
mod migration;
mod persistence;
mod ssh_actor;
mod state;
mod terminal;
mod views;

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("issh starting (M2a)");
    migration::migrate_legacy_config_dir();
    app::run();
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
