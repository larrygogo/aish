//! 终端渲染模块。
//!
//! - `colors`: alacritty Color → GPUI rgb
//! - `font`: 加载 bundled JetBrains Mono Nerd Font
//! - `grid_renderer`: 自绘字符 grid
//! - `cursor`: 光标渲染 + 闪烁
//! - `selection`: mouse drag → Term selection (Task 6)

#![allow(dead_code)]

pub mod colors;
pub mod cursor;
pub mod font;
pub mod grid_renderer;
pub mod selection;
