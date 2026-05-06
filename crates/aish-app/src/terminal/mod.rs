//! 终端渲染模块。
//!
//! - `colors`: alacritty Color → GPUI rgb
//! - `font`: 加载 bundled JetBrains Mono Nerd Font (Task 2)
//! - `grid_renderer`: 自绘字符 grid (Task 4)
//! - `cursor`: 光标渲染 + 闪烁 (Task 4)
//! - `selection`: mouse drag → Term selection (Task 6)

#![allow(dead_code)]

pub mod colors;
