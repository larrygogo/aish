//! aish-ui — 受 shadcn 启发的 GPUI 组件库。
//!
//! - `theme` — Token / Theme struct + dark 实现
//! - `components` — Button / TextInput / Toast 等组件
//! - `icons` — IconName + 内置 SVG 资源 + AssetSource

pub mod components;
pub mod icons;
pub mod prelude;
pub mod theme;

pub use theme::{theme, Theme};
