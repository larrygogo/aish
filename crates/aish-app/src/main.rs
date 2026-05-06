//! aish 主入口。M1 起接入 GPUI。

mod bridge;
mod mock;
mod state;

use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    init_logging();
    tracing::info!("aish starting (M1 skeleton)");
    println!("aish skeleton — see docs/superpowers/specs/ for design");
}

/// 初始化全局 tracing 订阅器。RUST_LOG 环境变量可覆盖默认 INFO 级别。
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(true).init();
}
