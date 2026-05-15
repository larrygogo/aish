//! aish-ui — 受 shadcn 启发的 GPUI 组件库。
//!
//! - `theme` — Token / Theme struct + dark 实现
//! - `components` — Button / TextInput / Toast 等组件
//! - `icons` — IconName + 内置 SVG 资源 + AssetSource
//! - `animation` — M30 lerp 工具（与 theme::motion 配对）

pub mod animation;
pub mod components;
pub mod icons;
pub mod prelude;
pub mod theme;

pub use animation::{lerp_hsla, lerp_px};
pub use components::*;
pub use icons::{icon, AishUiAssets, IconName};
pub use theme::{
    animate_or_skip, elevation_1, elevation_2, elevation_3, theme, Anatomy, CardAnatomy, ColorRole,
    DialogAnatomy, EasingFn, FormAnatomy, ListRowAnatomy, Motion, OverlayAnatomy, PageAnatomy,
    Theme, ThemeKind, TypeRole, TypeStyle, Typography, TypographyExt,
};
