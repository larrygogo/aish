//! aish-ui — 受 shadcn 启发的 GPUI 组件库。
//!
//! - `theme` — Token / Theme struct + dark 实现
//! - `components` — Button / TextInput / Toast 等组件
//! - `icons` — IconName + 内置 SVG 资源 + AssetSource

pub mod components;
pub mod icons;
pub mod prelude;
pub mod theme;

pub use components::*;
pub use icons::{icon, AishUiAssets, IconName};
pub use theme::{
    elevation_1, elevation_2, elevation_3, theme, Anatomy, CardAnatomy, ColorRole, DialogAnatomy,
    FormAnatomy, ListRowAnatomy, OverlayAnatomy, PageAnatomy, Theme, ThemeKind, TypeRole,
    TypeStyle, Typography, TypographyExt,
};
