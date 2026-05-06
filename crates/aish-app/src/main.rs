//! aish 主入口。M1 起接入 GPUI。

mod app;
mod bridge;
mod mock;
mod state;
mod views;

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M1)");
    app::run();
}

/// 初始化全局 tracing 订阅器。RUST_LOG 环境变量可覆盖默认 INFO 级别。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
