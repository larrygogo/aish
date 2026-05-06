# aish M2b1 — 终端渲染 + PTY resize 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 引入 alacritty_terminal::Term 解析 PTY 字节流，bundle JetBrains Mono Nerd Font，自绘字符 grid 替换 M2a 的"每行一个 div"渲染；加方块光标 + 闪烁、PTY 跟随窗口 resize、mouse 选中 + Ctrl+Shift+C 复制、方向键 normal mode 编码。完成后 vim/top 等 TUI 显示完整、bash 方向键看历史可用。

**Architecture:** alacritty_terminal::Term 由 GPUI Model<AppState> 持有（每 host 一个 Term），ssh_actor 仅 emit raw bytes，feed Term 在 GPUI cx.spawn loop 内（避免跨 Send 边界）。TerminalView 用 GPUI 低层 paint API 自绘 grid（参考 Zed `crates/terminal_view`），字体度量来自 GPUI text_system。SessionCommand 加 Resize variant 让 GPUI window resize 通过 actor 触发 chan.window_change。

**Tech Stack:** alacritty_terminal (vt100 解析 + grid + scrollback buffer), GPUI text_system + low-level paint API, JetBrains Mono Nerd Font (bundled), russh::Channel::window_change

**前置:** M2a 已完成（commit `d717956`，真 SSH + 键盘 echo + utf8_lossy 显示），M2b1 spec 已落盘 (`docs/superpowers/specs/2026-05-06-aish-m2b1-terminal-rendering-design.md`, commit `7d2930c`)。

---

## ⚠️ 实施者须知

### alacritty_terminal API

alacritty_terminal 是 alacritty 终端模拟器抽出的状态机库。本 plan 标 `// ALAC-API` 的代码按当前版本调整：

- 看 `<zed-reference>/crates/terminal/src/` —— Zed 自己集成 alacritty_terminal 的代码（最权威参考）
- docs.rs/alacritty_terminal 上的 API 文档

### GPUI 自绘 API

参考 `<zed-reference>/crates/terminal_view/src/terminal_element.rs` 中的 `paint` 方法。**不要每个 cell 一个 div**（性能不行）；用 `Window::paint_layer` + `text_system.shape_line` 批量绘。

### bundle 字体来源

JetBrains Mono Nerd Font Regular `.ttf` 从 https://github.com/ryanoasis/nerd-fonts/releases 下载 `JetBrainsMono.zip`，解压取 `JetBrainsMonoNerdFont-Regular.ttf`（约 400KB），放进 `crates/aish-app/assets/`。**不要 commit zip**，只 commit `.ttf`。

### Demo 验证 implementer 跑不了

implementer subagent 没用户 VPS 凭证，不能验证 demo。每个 task 的 verification 只能确认：build / cargo test / cargo run 启动无 panic。**真 demo 15 项由 user 在 Task 8 手动验证**。

---

## File Structure（M2b1 完成时）

```
aish/
├── Cargo.toml                             # 加 alacritty_terminal workspace dep
├── crates/aish-app/
│   ├── Cargo.toml                         # 加 alacritty_terminal
│   ├── assets/                            # 新建
│   │   └── JetBrainsMonoNerdFont-Regular.ttf  # 新增 (~400KB)
│   └── src/
│       ├── main.rs                        # 修改：mod terminal
│       ├── app.rs                         # 修改：PaneOutput 处理 feed Term
│       ├── state.rs                       # 修改：pane_logs → pane_terminals + pane_dimensions
│       ├── bridge.rs                      # 不变
│       ├── ssh_actor.rs                   # 修改：SessionCommand::Resize 处理 + 扩展 encode_key 表
│       ├── fixtures.rs                    # 不变
│       ├── terminal/                      # 新建模块
│       │   ├── mod.rs
│       │   ├── colors.rs                  # 新：alacritty Color → GPUI rgb
│       │   ├── font.rs                    # 新：加载 bundled font
│       │   ├── grid_renderer.rs           # 新：自绘 cell + 颜色 + 背景
│       │   ├── cursor.rs                  # 新：光标渲染 + 闪烁
│       │   └── selection.rs               # 新：mouse drag → Term selection
│       └── views/
│           ├── mod.rs                     # 修改：reexport TerminalView (替换 HostPaneView)
│           ├── host_list.rs               # 不变
│           ├── host_pane.rs               # ❌ 删除
│           └── terminal_view.rs           # 新：替换 host_pane.rs
```

新增 8 个文件 + 1 个字体资源
删除 1 个文件
修改 6 个文件 + 2 个 Cargo.toml

---

## Task 1: alacritty_terminal 依赖 + colors.rs（颜色映射）

**Files:**
- Modify: `Cargo.toml`（workspace.dependencies 加 alacritty_terminal）
- Modify: `crates/aish-app/Cargo.toml`（加 alacritty_terminal）
- Create: `crates/aish-app/src/terminal/mod.rs`
- Create: `crates/aish-app/src/terminal/colors.rs`
- Modify: `crates/aish-app/src/main.rs`（加 mod terminal）

- [ ] **Step 1: 在 workspace 根 Cargo.toml 加 alacritty_terminal**

读 `Cargo.toml`，在 `[workspace.dependencies]` 段末尾追加：

```toml
alacritty_terminal = "0.25"
```

如果 0.25 不存在，`cargo search alacritty_terminal` 看实际版本，选最新稳定 (>= 0.20)。

- [ ] **Step 2: 在 aish-app/Cargo.toml 加依赖**

在 `[dependencies]` 段末尾追加：

```toml
alacritty_terminal = { workspace = true }
```

- [ ] **Step 3: 创建 `crates/aish-app/src/terminal/mod.rs`**

```rust
//! 终端渲染模块。
//!
//! - `colors`: alacritty Color → GPUI rgb
//! - `font`: 加载 bundled JetBrains Mono Nerd Font
//! - `grid_renderer`: 自绘字符 grid
//! - `cursor`: 光标渲染 + 闪烁
//! - `selection`: mouse drag → Term selection

#![allow(dead_code)]

pub mod colors;
```

- [ ] **Step 4: 创建 `crates/aish-app/src/terminal/colors.rs`**

```rust
//! alacritty Color → GPUI rgba。
//!
//! alacritty_terminal::vte::ansi::Color 有三种 variant：
//!   - Named(NamedColor) — 16 个标准 ANSI 名（Black/Red/Green/...）
//!   - Spec(Rgb) — 任意 RGB
//!   - Indexed(u8) — 256 color palette index

use alacritty_terminal::vte::ansi::{Color as AlacColor, NamedColor, Rgb};
use gpui::{rgb, Hsla, Rgba};

/// 默认 16 色 ANSI palette（与 alacritty 默认主题一致）。
pub const DEFAULT_PALETTE: [u32; 16] = [
    0x1d1f21, // 0  Black
    0xcc6666, // 1  Red
    0xb5bd68, // 2  Green
    0xf0c674, // 3  Yellow
    0x81a2be, // 4  Blue
    0xb294bb, // 5  Magenta
    0x8abeb7, // 6  Cyan
    0xc5c8c6, // 7  White
    0x969896, // 8  BrightBlack
    0xde935f, // 9  BrightRed
    0xb5bd68, // 10 BrightGreen (与 Green 同；alacritty 默认就是这样)
    0xf0c674, // 11 BrightYellow (同上)
    0x81a2be, // 12 BrightBlue
    0xb294bb, // 13 BrightMagenta
    0x8abeb7, // 14 BrightCyan
    0xffffff, // 15 BrightWhite
];

pub const DEFAULT_FOREGROUND: u32 = 0xc5c8c6;
pub const DEFAULT_BACKGROUND: u32 = 0x1d1f21;

/// 主入口：把 alacritty Color 转成 GPUI Hsla。
pub fn to_gpui(color: AlacColor, is_fg: bool) -> Hsla {
    match color {
        AlacColor::Named(named) => named_to_gpui(named, is_fg),
        AlacColor::Spec(rgb_color) => rgb_to_gpui(rgb_color),
        AlacColor::Indexed(idx) => indexed_to_gpui(idx, is_fg),
    }
}

fn named_to_gpui(named: NamedColor, is_fg: bool) -> Hsla {
    let hex = match named {
        NamedColor::Black => DEFAULT_PALETTE[0],
        NamedColor::Red => DEFAULT_PALETTE[1],
        NamedColor::Green => DEFAULT_PALETTE[2],
        NamedColor::Yellow => DEFAULT_PALETTE[3],
        NamedColor::Blue => DEFAULT_PALETTE[4],
        NamedColor::Magenta => DEFAULT_PALETTE[5],
        NamedColor::Cyan => DEFAULT_PALETTE[6],
        NamedColor::White => DEFAULT_PALETTE[7],
        NamedColor::BrightBlack => DEFAULT_PALETTE[8],
        NamedColor::BrightRed => DEFAULT_PALETTE[9],
        NamedColor::BrightGreen => DEFAULT_PALETTE[10],
        NamedColor::BrightYellow => DEFAULT_PALETTE[11],
        NamedColor::BrightBlue => DEFAULT_PALETTE[12],
        NamedColor::BrightMagenta => DEFAULT_PALETTE[13],
        NamedColor::BrightCyan => DEFAULT_PALETTE[14],
        NamedColor::BrightWhite => DEFAULT_PALETTE[15],
        NamedColor::Foreground => {
            if is_fg {
                DEFAULT_FOREGROUND
            } else {
                DEFAULT_BACKGROUND
            }
        }
        NamedColor::Background => DEFAULT_BACKGROUND,
        NamedColor::Cursor => DEFAULT_FOREGROUND,
        // ALAC-API: 还有更多 variants（DimBlack 等）按版本调整
        _ => {
            if is_fg {
                DEFAULT_FOREGROUND
            } else {
                DEFAULT_BACKGROUND
            }
        }
    };
    Rgba::from(rgb(hex)).into()
}

fn rgb_to_gpui(c: Rgb) -> Hsla {
    let hex = ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
    Rgba::from(rgb(hex)).into()
}

/// 256 color palette: 0-15 是 16 色名，16-231 是 6×6×6 cube，232-255 是 24 灰阶。
fn indexed_to_gpui(idx: u8, is_fg: bool) -> Hsla {
    if idx < 16 {
        named_to_gpui(NAMED_BY_IDX[idx as usize], is_fg)
    } else if idx < 232 {
        // 6×6×6 cube
        let i = idx - 16;
        let r = (i / 36) % 6;
        let g = (i / 6) % 6;
        let b = i % 6;
        let scale = |c: u8| -> u32 {
            if c == 0 {
                0
            } else {
                (40 * c as u32) + 55
            }
        };
        let hex = (scale(r) << 16) | (scale(g) << 8) | scale(b);
        Rgba::from(rgb(hex)).into()
    } else {
        // 24 灰阶
        let v = (8 + 10 * (idx as u32 - 232)) & 0xff;
        let hex = (v << 16) | (v << 8) | v;
        Rgba::from(rgb(hex)).into()
    }
}

const NAMED_BY_IDX: [NamedColor; 16] = [
    NamedColor::Black,
    NamedColor::Red,
    NamedColor::Green,
    NamedColor::Yellow,
    NamedColor::Blue,
    NamedColor::Magenta,
    NamedColor::Cyan,
    NamedColor::White,
    NamedColor::BrightBlack,
    NamedColor::BrightRed,
    NamedColor::BrightGreen,
    NamedColor::BrightYellow,
    NamedColor::BrightBlue,
    NamedColor::BrightMagenta,
    NamedColor::BrightCyan,
    NamedColor::BrightWhite,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba_hex(c: Hsla) -> u32 {
        let rgba = Rgba::from(c);
        let r = (rgba.r * 255.0).round() as u32;
        let g = (rgba.g * 255.0).round() as u32;
        let b = (rgba.b * 255.0).round() as u32;
        (r << 16) | (g << 8) | b
    }

    #[test]
    fn named_red_maps_to_palette() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Red), true);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE[1]);
    }

    #[test]
    fn named_foreground_returns_default_fg_when_is_fg() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Foreground), true);
        assert_eq!(rgba_hex(hsla), DEFAULT_FOREGROUND);
    }

    #[test]
    fn named_background_returns_default_bg() {
        let hsla = to_gpui(AlacColor::Named(NamedColor::Background), false);
        assert_eq!(rgba_hex(hsla), DEFAULT_BACKGROUND);
    }

    #[test]
    fn spec_rgb_preserves_components() {
        let hsla = to_gpui(
            AlacColor::Spec(Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            }),
            true,
        );
        assert_eq!(rgba_hex(hsla), 0x123456);
    }

    #[test]
    fn indexed_15_maps_to_bright_white() {
        let hsla = to_gpui(AlacColor::Indexed(15), true);
        assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE[15]);
    }

    #[test]
    fn indexed_0_to_15_use_named_palette() {
        for i in 0u8..16 {
            let hsla = to_gpui(AlacColor::Indexed(i), true);
            assert_eq!(rgba_hex(hsla), DEFAULT_PALETTE[i as usize]);
        }
    }

    #[test]
    fn indexed_232_to_255_grayscale() {
        let dark = to_gpui(AlacColor::Indexed(232), true);
        let bright = to_gpui(AlacColor::Indexed(255), true);
        let dark_hex = rgba_hex(dark);
        let bright_hex = rgba_hex(bright);
        // 232 应该接近黑（rgb 8,8,8），255 接近白（rgb 238,238,238）
        let dark_r = (dark_hex >> 16) & 0xff;
        let bright_r = (bright_hex >> 16) & 0xff;
        assert!(dark_r < 20);
        assert!(bright_r > 220);
    }

    #[test]
    fn indexed_cube_16_is_pure_black() {
        // 16 是 6×6×6 cube 的起点 = (0,0,0)
        let hsla = to_gpui(AlacColor::Indexed(16), true);
        assert_eq!(rgba_hex(hsla), 0x000000);
    }
}
```

> **可能要调整**：
> - alacritty_terminal 0.25 中 `Color`/`NamedColor`/`Rgb` 的 path 可能是 `alacritty_terminal::ansi::Color` 而不是 `vte::ansi::Color` — 按 cargo build 错误调整 import path
> - GPUI 的 `Hsla` / `Rgba` / `rgb` 都是稳定 API（M1/M2 已用）

- [ ] **Step 5: 在 main.rs 加 mod terminal**

`main.rs` 当前 mod 列表（保持字母序）：

```rust
mod app;
mod bridge;
mod fixtures;
mod ssh_actor;
mod state;
mod views;
```

加 `mod terminal;`：

```rust
mod app;
mod bridge;
mod fixtures;
mod ssh_actor;
mod state;
mod terminal;
mod views;
```

- [ ] **Step 6: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -10
cargo test -p aish-app 2>&1 | tail -5
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: build 通过（首次拉 alacritty_terminal 编译 1-3 分钟）+ 8 个 colors 测试 + workspace test 全绿。

- [ ] **Step 7: commit**

```bash
git add Cargo.toml Cargo.lock crates/aish-app/Cargo.toml crates/aish-app/src/terminal/ crates/aish-app/src/main.rs
git commit -m "feat(aish-app): 引入 alacritty_terminal 依赖 + colors 模块"
```

---

## Task 2: bundle 字体 + font.rs

**Files:**
- Create: `crates/aish-app/assets/JetBrainsMonoNerdFont-Regular.ttf`（手动下载）
- Modify: `crates/aish-app/Cargo.toml`（声明 assets/ 包含在 crate）
- Create: `crates/aish-app/src/terminal/font.rs`
- Modify: `crates/aish-app/src/terminal/mod.rs`（加 pub mod font）

- [ ] **Step 1: 下载 JetBrainsMono Nerd Font**

PowerShell：

```powershell
$assetsDir = "C:\Users\larry\Desktop\workspace\aish\crates\aish-app\assets"
New-Item -ItemType Directory -Path $assetsDir -Force | Out-Null

$url = "https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip"
$zipPath = Join-Path $assetsDir "JetBrainsMono.zip"
Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

# 解压 + 仅保留 Regular
Expand-Archive -Path $zipPath -DestinationPath (Join-Path $assetsDir "_unzipped") -Force
$src = Get-ChildItem -Path (Join-Path $assetsDir "_unzipped") -Filter "JetBrainsMonoNerdFont-Regular.ttf" -Recurse | Select-Object -First 1
if (-not $src) {
    # 如果文件名带空格变体（"JetBrainsMono Nerd Font Regular.ttf"），按 pattern 找
    $src = Get-ChildItem -Path (Join-Path $assetsDir "_unzipped") -Filter "*NerdFont-Regular.ttf" -Recurse | Select-Object -First 1
}
if (-not $src) {
    Write-Error "Cannot find Regular font in unzipped archive"
} else {
    Copy-Item -Path $src.FullName -Destination (Join-Path $assetsDir "JetBrainsMonoNerdFont-Regular.ttf") -Force
}

# 清理
Remove-Item $zipPath -Force
Remove-Item (Join-Path $assetsDir "_unzipped") -Recurse -Force

# 验证
Get-Item (Join-Path $assetsDir "JetBrainsMonoNerdFont-Regular.ttf") | Select-Object Name, Length
```

如果 `JetBrainsMono.zip` 实际下载下来文件名带空格（如 `JetBrainsMono Nerd Font Regular.ttf`），重命名为 `JetBrainsMonoNerdFont-Regular.ttf` 以保证后续 include_bytes! 路径稳定。

> 如果网络问题下载失败，BLOCKED 报告。可以手动让 user 下载放进去。

- [ ] **Step 2: 在 aish-app/Cargo.toml 声明 include**

Cargo 默认 include 是 src 目录里的所有 .rs。assets/ 会通过 `include_bytes!()` 编译时嵌入，不需要 Cargo.toml 额外声明。但为了 cargo package 时不丢失，加 `include = [...]`：

读 `crates/aish-app/Cargo.toml`，在 `[package]` section（authors 之后）追加：

```toml
include = ["src/**/*", "assets/**/*", "Cargo.toml"]
```

- [ ] **Step 3: 创建 `crates/aish-app/src/terminal/font.rs`**

```rust
//! 字体加载：bundle JetBrains Mono Nerd Font Regular。

use gpui::{App, Pixels};

/// 字体名称，用于 GPUI text_system font 查找。
pub const FONT_NAME: &str = "JetBrainsMono Nerd Font";

/// 终端字号（M2c 才做用户配置）。
pub const FONT_SIZE: f32 = 14.0;

/// bundle 的 .ttf bytes。
const FONT_BYTES: &[u8] = include_bytes!("../../assets/JetBrainsMonoNerdFont-Regular.ttf");

/// 在 GPUI App 启动时调用：把 bundled font 注册进 text_system。
///
/// 必须在创建任何使用此字体的 view 之前调用（典型是 app::run() 内）。
pub fn register_bundled_font(cx: &mut App) {
    let bytes = FONT_BYTES.to_vec();
    cx.text_system()
        .add_fonts(vec![bytes.into()])
        .expect("bundled font should load");
}

/// 拿 (cell_width, cell_height) — 单字符 advance 与行高。
///
/// FONT_SIZE 字号下 monospace 的 cell 度量。
pub fn cell_size(cx: &App) -> (Pixels, Pixels) {
    // GPUI-API: text_system().font_metrics + .typographic_bounds
    // 实际 API 名以 zed-reference/crates/terminal_view/.../terminal_element.rs 为准
    let _ = cx;
    // 占位实现：用一个合理的 hardcoded 度量，Task 4 真正接通 GPUI text_system 时替换
    (Pixels(8.4), Pixels(18.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_bytes_is_non_empty() {
        // 验证 include_bytes! 真嵌入了内容
        assert!(FONT_BYTES.len() > 100_000); // ttf 至少 100KB
        assert!(FONT_BYTES.len() < 2_000_000); // 不该 >2MB
    }

    #[test]
    fn font_bytes_starts_with_ttf_or_otf_magic() {
        // TrueType 文件首 4 字节通常是 0x00010000 或 "OTTO" / "true" / "typ1"
        // 至少不应该全 0
        let magic = &FONT_BYTES[..4];
        assert!(magic.iter().any(|b| *b != 0));
    }

    #[test]
    fn font_name_is_jetbrains() {
        assert_eq!(FONT_NAME, "JetBrainsMono Nerd Font");
    }

    #[test]
    fn font_size_is_14pt() {
        assert!((FONT_SIZE - 14.0).abs() < 0.01);
    }
}
```

> **关于 `cell_size`**：Task 2 只是占位实现（hardcoded 8.4 × 18.0），Task 4（TerminalView 渲染）真正接通 GPUI text_system 测度量时替换。

- [ ] **Step 4: 在 terminal/mod.rs 加 pub mod font**

```rust
//! 终端渲染模块。

#![allow(dead_code)]

pub mod colors;
pub mod font;
```

- [ ] **Step 5: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app
cargo test -p aish-app font 2>&1 | tail -5
cargo test --workspace 2>&1 | tail -5
```

Expected: 4 个 font 测试通过 + 全 workspace 全绿。

- [ ] **Step 6: commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/Cargo.toml crates/aish-app/assets/ crates/aish-app/src/terminal/
git commit -m "feat(aish-app): bundle JetBrains Mono Nerd Font + font 模块"
```

---

## Task 3: retire pane_logs + 接通 Term + TerminalView 骨架（big task）

⚠️ M2b1 最大的 task — 一次性把 pane_logs 改 pane_terminals + 改 ssh_actor + 改 app.rs feed loop + 创建 TerminalView 骨架（先渲染空白）+ 删 host_pane.rs。Task 4 才让 TerminalView 真正绘 grid。

**Files:**
- Modify: `crates/aish-app/src/state.rs`
- Modify: `crates/aish-app/src/ssh_actor.rs`
- Modify: `crates/aish-app/src/app.rs`
- Modify: `crates/aish-app/src/views/mod.rs`
- Create: `crates/aish-app/src/views/terminal_view.rs`（骨架版）
- Delete: `crates/aish-app/src/views/host_pane.rs`

- [ ] **Step 1: 改写 `crates/aish-app/src/state.rs`**

整体替换为：

```rust
//! aish-app App State — M2b1 起持有 alacritty_terminal::Term per host。

#![allow(dead_code)]

use std::collections::HashMap;

use aish_types::{HostConfig, HostId};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::Term;
use tokio::sync::mpsc;

/// 从 SSH actor task 推回 GPUI 的事件。
#[derive(Debug)]
pub enum SshEvent {
    Connected { host: HostId },
    PaneOutput { host: HostId, bytes: Vec<u8> },
    Disconnected { host: HostId, reason: DisconnectReason },
    Error { host: HostId, kind: SshErrorKind, msg: String },
}

#[derive(Debug, Clone)]
pub enum DisconnectReason {
    UserRequested,
    RemoteExited,
    NetworkError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    ConnectFailed,
    AuthFailed,
    Io,
    Protocol,
}

/// 从 GPUI 发到 actor task 的命令。
#[derive(Debug)]
pub enum SessionCommand {
    SendBytes(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Disconnect,
}

/// 默认 PTY 大小（首次 connect 时用，后续按窗口 resize 调整）。
pub const DEFAULT_COLS: u16 = 120;
pub const DEFAULT_ROWS: u16 = 40;

/// scrollback buffer 大小。
const SCROLLBACK_LINES: usize = 10_000;

/// 创建一个空 Term（M2b1 用 VoidListener — 不接收 alacritty 事件）。
pub fn make_term(cols: u16, rows: u16) -> Term<VoidListener> {
    let size = TermSize::new(cols as usize, rows as usize);
    let mut config = TermConfig::default();
    config.scrolling_history = SCROLLBACK_LINES;
    Term::new(config, &size, VoidListener)
}

/// 单一 root Model：所有 UI 共享状态。
#[derive(Default)]
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    /// per-host alacritty Term（vt100 状态机 + grid + scrollback）
    pub pane_terminals: HashMap<HostId, Term<VoidListener>>,
    /// per-host 当前 PTY 大小（cols, rows）
    pub pane_dimensions: HashMap<HostId, (u16, u16)>,
    /// 已连接 host 的 SessionCommand sender
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
}

impl AppState {
    pub fn with_hosts(hosts: Vec<HostConfig>) -> Self {
        Self {
            hosts,
            selected: None,
            pane_terminals: HashMap::new(),
            pane_dimensions: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    pub fn select_host(&mut self, id: HostId) {
        self.selected = Some(id);
    }

    pub fn host_label(&self, id: HostId) -> Option<String> {
        self.hosts.iter().find(|h| h.id == id).map(|h| h.label.clone())
    }

    pub fn is_session_active(&self, id: HostId) -> bool {
        self.sessions.contains_key(&id)
    }

    pub fn register_session(&mut self, id: HostId, sender: mpsc::Sender<SessionCommand>) {
        self.sessions.insert(id, sender);
        self.pane_dimensions.insert(id, (DEFAULT_COLS, DEFAULT_ROWS));
    }

    pub fn drop_session(&mut self, id: HostId) {
        self.sessions.remove(&id);
        // 不删 pane_terminals — 保留 scrollback，重连时用户能看到旧输出
    }

    /// feed bytes 到指定 host 的 Term（VT100 状态机）。
    /// 如果 Term 不存在则创建。
    pub fn feed_bytes(&mut self, host: HostId, bytes: &[u8]) {
        let (cols, rows) = self
            .pane_dimensions
            .get(&host)
            .copied()
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        let term = self
            .pane_terminals
            .entry(host)
            .or_insert_with(|| make_term(cols, rows));
        // ALAC-API: alacritty_terminal 0.25 的 feed 通过 vte::Parser 处理
        // Term 自己不直接 feed，需要 Parser::advance(&mut Term, byte)
        // 实际 API 看 zed-reference/crates/terminal/src/lib.rs 的 ProcessHandler 部分
        let mut parser = alacritty_terminal::vte::Parser::new();
        for &byte in bytes {
            parser.advance(term, byte);
        }
    }

    /// 取指定 host 的 Term（只读）。
    pub fn term_of(&self, host: HostId) -> Option<&Term<VoidListener>> {
        self.pane_terminals.get(&host)
    }

    /// resize 指定 host 的 Term（同步 alacritty grid）。
    pub fn resize_term(&mut self, host: HostId, cols: u16, rows: u16) {
        if let Some(term) = self.pane_terminals.get_mut(&host) {
            let size = TermSize::new(cols as usize, rows as usize);
            term.resize(size);
        }
        self.pane_dimensions.insert(host, (cols, rows));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_types::SshAuth;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn mk_host(label: &str) -> HostConfig {
        HostConfig {
            id: HostId(Uuid::new_v4()),
            label: label.into(),
            host: "example.com".into(),
            port: 22,
            user: "larry".into(),
            auth: SshAuth::KeyFile {
                path: PathBuf::from("/tmp/k"),
            },
            env_profile: None,
        }
    }

    #[test]
    fn with_hosts_initializes() {
        let h = mk_host("a");
        let state = AppState::with_hosts(vec![h]);
        assert_eq!(state.hosts.len(), 1);
        assert!(state.pane_terminals.is_empty());
        assert!(state.pane_dimensions.is_empty());
    }

    #[test]
    fn feed_bytes_creates_term_on_demand() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"hello\r\n");
        assert!(state.pane_terminals.contains_key(&id));
    }

    #[test]
    fn feed_bytes_reflects_in_term_grid() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"abc");
        let term = state.term_of(id).unwrap();
        // 验证 grid 第一行前 3 个 cell 是 a/b/c
        let grid = term.grid();
        let first_row = &grid[alacritty_terminal::index::Line(0)];
        assert_eq!(first_row[alacritty_terminal::index::Column(0)].c, 'a');
        assert_eq!(first_row[alacritty_terminal::index::Column(1)].c, 'b');
        assert_eq!(first_row[alacritty_terminal::index::Column(2)].c, 'c');
    }

    #[tokio::test]
    async fn register_session_inits_dimensions() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        assert_eq!(
            state.pane_dimensions.get(&id),
            Some(&(DEFAULT_COLS, DEFAULT_ROWS))
        );
    }

    #[tokio::test]
    async fn drop_session_keeps_terminal() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        state.feed_bytes(id, b"x");
        state.drop_session(id);
        // 不删 Term — scrollback 保留
        assert!(state.pane_terminals.contains_key(&id));
        assert!(!state.is_session_active(id));
    }

    #[test]
    fn resize_updates_dimensions() {
        let h = mk_host("a");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b""); // 创建 Term
        state.resize_term(id, 100, 30);
        assert_eq!(state.pane_dimensions.get(&id), Some(&(100, 30)));
    }
}
```

> **关键 ALAC-API 不确定点**：
> - `alacritty_terminal::vte::Parser::new()` 还是别的路径
> - `parser.advance(term, byte)` 签名
> - `Term::new(config, &size, listener)` 参数顺序
> - `Term::grid()` / `Term::resize()` 方法名
> 
> 看 zed-reference/crates/terminal/src/lib.rs 的实际用法（grep `vte::Parser` / `Term::new`）。

- [ ] **Step 2: 修改 ssh_actor.rs — SessionCommand::Resize 处理**

读 ssh_actor.rs，找到 `tokio::select!` 块里的 `SendBytes` 分支（M2a 写的），在它附近增加 `Resize` 分支：

```rust
            cmd = cmd_rx.recv() => match cmd {
                Some(SessionCommand::SendBytes(bytes)) => {
                    if let Err(e) = chan.data(&bytes[..]).await {
                        let _ = event_tx
                            .send(SshEvent::Disconnected {
                                host,
                                reason: DisconnectReason::NetworkError(e.to_string()),
                            })
                            .await;
                        break;
                    }
                }
                Some(SessionCommand::Resize { cols, rows }) => {
                    // ALAC-API: russh::Channel::window_change 签名是
                    // window_change(col_width: u32, row_height: u32, pix_width: u32, pix_height: u32)
                    if let Err(e) = chan
                        .window_change(cols as u32, rows as u32, 0, 0)
                        .await
                    {
                        tracing::warn!("PTY resize failed: {}", e);
                        // resize 失败不致命，继续运行
                    }
                }
                Some(SessionCommand::Disconnect) | None => {
                    let _ = event_tx
                        .send(SshEvent::Disconnected {
                            host,
                            reason: DisconnectReason::UserRequested,
                        })
                        .await;
                    break;
                }
            },
```

注意：`chan.window_change` 是 aish-ssh::Channel 包装的。需要在 aish-ssh/src/channel.rs 加这个方法。

如果 Channel 没有 window_change 方法，先去 aish-ssh/src/channel.rs 加上：

读 `crates/aish-ssh/src/channel.rs`，在 impl Channel 末尾追加：

```rust
    /// 通知远端 PTY 大小变化（SIGWINCH）。
    pub async fn window_change(
        &self,
        cols: u32,
        rows: u32,
        pix_width: u32,
        pix_height: u32,
    ) -> Result<(), SshError> {
        self.inner
            .window_change(cols, rows, pix_width, pix_height)
            .await
            .map_err(SshError::Protocol)
    }
```

> ALAC-API: 实际 russh::Channel API 名按 0.46 调整（应该是 `window_change`，但参数顺序按实际 trait 看）。

- [ ] **Step 3: 修改 app.rs cx.spawn loop 改为 feed bytes**

读 app.rs，找到 `SshEvent::PaneOutput { host, bytes }` 分支（M2a 写的 utf8_lossy + split），整个分支替换为：

```rust
                    SshEvent::PaneOutput { host, bytes } => {
                        state.feed_bytes(host, &bytes);
                        cx.notify();
                    }
```

`SshEvent::Connected` / `Disconnected` / `Error` 分支保留 M2a 的逻辑，但 `append_log` 调用要改 — 因为 `pane_logs` 不存在了。

简化方案：M2b1 阶段 Connected/Disconnected/Error 不显示在主区（demo 里通过 host_list 的 ●/○ 状态指示连接），等 M2b2 决定怎么把这些事件 inline 到终端里。

修改 app.rs：

```rust
                    SshEvent::Connected { host: _ } => {
                        // M2b1: 状态变更通过 host_list 的 ●/○ 显示，不写 pane
                        cx.notify();
                    }
                    SshEvent::PaneOutput { host, bytes } => {
                        state.feed_bytes(host, &bytes);
                        cx.notify();
                    }
                    SshEvent::Disconnected { host, reason: _ } => {
                        state.drop_session(host);
                        cx.notify();
                    }
                    SshEvent::Error { host, kind: _, msg } => {
                        // M2b1: 错误暂时通过 tracing log 记录；主区不显示（M2b2 接 alacritty + ANSI 颜色后再 inline 错误行）
                        tracing::error!(?host, msg, "SSH error");
                        state.drop_session(host);
                        cx.notify();
                    }
```

注意：因为 `host_label` 不再用于"Connecting..." 提示（pane_logs 没了），但 host_list.rs 的 handle_click 仍写 "Connecting..." 行用 append_log — 那也要改。

读 host_list.rs 的 handle_click，找到 `state.append_log(host, format!("[{}] Connecting to {}...", ...))`，删除这一行。`select_host` 仍调用，但不写 log。

- [ ] **Step 4: 修改 host_list.rs 删 Connecting log + 移除对 append_log 的引用**

读 host_list.rs。找到：

```rust
        let needs_connect = self.state.update(cx, |state, cx| {
            state.select_host(host);
            let label = state.host_label(host).unwrap_or_else(|| format!("{:?}", host));
            let needs = !state.is_session_active(host);
            if needs {
                state.append_log(
                    host,
                    format!("[{}] Connecting to {}...", simple_time(), label),
                );
            }
            cx.notify();
            needs
        });
```

替换为：

```rust
        let needs_connect = self.state.update(cx, |state, cx| {
            state.select_host(host);
            let needs = !state.is_session_active(host);
            cx.notify();
            needs
        });
```

并删除 `simple_time()` 函数（已无引用）和它的 `use std::time::{SystemTime, UNIX_EPOCH}` 导入。

- [ ] **Step 5: 创建 `crates/aish-app/src/views/terminal_view.rs`（骨架）**

```rust
//! 主区终端视图。M2b1 Task 3 阶段渲染空白；Task 4 实现真正的 grid 绘制。

use std::sync::Arc;

use gpui::{
    div, prelude::*, rgb, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};

pub struct TerminalView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
}

impl TerminalView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        Self {
            state,
            bridge,
            tx,
            focus_handle,
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let host = match self.state.read(cx).selected {
            Some(h) => h,
            None => return,
        };
        let sender = match self.state.read(cx).sessions.get(&host).cloned() {
            Some(s) => s,
            None => return,
        };

        let ctrl = event.keystroke.modifiers.control;
        let alt = event.keystroke.modifiers.alt;
        let key = event.keystroke.key.as_str();

        let bytes = encode_key(key, ctrl, alt);
        if bytes.is_empty() {
            return;
        }

        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.state.read(cx).selected;
        let placeholder = match selected {
            None => "请从左侧选择主机",
            Some(_) => "(终端渲染将在 Task 4 实装)",
        };

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .flex_1()
            .h_full()
            .bg(rgb(0x1d1f21))
            .text_color(rgb(0xc5c8c6))
            .p_4()
            .child(placeholder)
    }
}
```

- [ ] **Step 6: 修改 `crates/aish-app/src/views/mod.rs` reexport**

替换为：

```rust
//! GPUI Views。

#![allow(dead_code)]

mod host_list;
mod terminal_view;

pub use host_list::HostListView;
pub use terminal_view::TerminalView;
```

- [ ] **Step 7: 修改 app.rs RootView 用 TerminalView**

读 app.rs，找到 `host_pane: Entity<crate::views::HostPaneView>` 改为 `terminal: Entity<crate::views::TerminalView>`。`RootView::new` 中创建 host_pane 改为 terminal：

```rust
struct RootView {
    host_list: Entity<crate::views::HostListView>,
    terminal: Entity<crate::views::TerminalView>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let host_list = cx.new(|cx| {
            crate::views::HostListView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let terminal = cx.new(|cx| crate::views::TerminalView::new(state, bridge, tx, cx));
        Self { host_list, terminal }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x1d1f21))  // 改为 alacritty 默认背景
            .child(self.host_list.clone())
            .child(self.terminal.clone())
    }
}
```

- [ ] **Step 8: 删除 `crates/aish-app/src/views/host_pane.rs`**

```bash
git rm crates/aish-app/src/views/host_pane.rs
```

- [ ] **Step 9: 删除 state.rs 残留 append_log / logs_of**

回到 state.rs（Step 1 替换的版本），确认其中**已经没有** `append_log` / `logs_of` 方法。如果误留了，删除。

- [ ] **Step 10: 验证 build / test**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -30
```

如果失败，按错误调整：
- alacritty_terminal API 路径（`alacritty_terminal::vte::Parser` 还是 `alacritty_terminal::ansi::Parser`）
- `Term::new` 参数顺序
- `Term::resize(TermSize)` vs `Term::resize(usize, usize)`
- VoidListener 路径

成功后：

```bash
cargo test --workspace 2>&1 | tail -10
```

Expected: state 测试 6 个新 + workspace 全绿。

- [ ] **Step 11: cargo run 后台启动**

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t3-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t3-err.txt"
Start-Sleep -Seconds 8
if ($proc.HasExited) {
    Write-Output "FAIL"
    Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t3-err.txt" -Tail 30
} else {
    Write-Output "OK: window started, killing now"
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t3-*.txt" -Force
```

预期：窗口能开 + 没 panic + 主区显示 "请从左侧选择主机"。

- [ ] **Step 12: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add -A crates/aish-app/ crates/aish-ssh/
git commit -m "feat(aish-app): retire pane_logs 改用 alacritty Term + TerminalView 骨架"
```

---

## Task 4: TerminalView 完整渲染（grid + 颜色 + 光标 + 闪烁）

⚠️ M2b1 第二个 sizable task — TerminalView 替换 placeholder 为真自绘。

**Files:**
- Modify: `crates/aish-app/src/views/terminal_view.rs`
- Create: `crates/aish-app/src/terminal/grid_renderer.rs`
- Create: `crates/aish-app/src/terminal/cursor.rs`
- Modify: `crates/aish-app/src/terminal/mod.rs`
- Modify: `crates/aish-app/src/terminal/font.rs`（接通真 cell_size）

- [ ] **Step 1: 看 Zed terminal_element.rs 学渲染模式**

```bash
cat C:/Users/larry/Desktop/workspace/zed-reference/crates/terminal_view/src/terminal_element.rs | head -200
grep -n 'fn paint\|paint_layer\|shape_line\|font_metrics' C:/Users/larry/Desktop/workspace/zed-reference/crates/terminal_view/src/terminal_element.rs
```

记下：
- GPUI 字体度量 API：`cx.text_system().font_metrics(...)` 或 `font_id` 体系
- 自绘单字符：`Window::paint_text` / `text_system.shape_line`
- 自绘背景矩形：`Window::paint_quad`
- Element trait 实现（如果用 custom Element 替代 Render）

- [ ] **Step 2: 接通 font::cell_size 真实现**

读 `crates/aish-app/src/terminal/font.rs`，把 Task 2 的 hardcoded `cell_size` 替换为真实度量。**具体 GPUI API 按 Zed 看到的调整**。参考：

```rust
pub fn cell_size(cx: &App) -> (Pixels, Pixels) {
    use gpui::{font, FontWeight, FontStyle, Pixels};
    let font_id = cx
        .text_system()
        .font_id(&font(FONT_NAME).weight(FontWeight::NORMAL).style(FontStyle::Normal))
        .expect("font should be registered");
    let font_size: Pixels = Pixels(FONT_SIZE);
    // 单字符 monospace 度量
    let metrics = cx
        .text_system()
        .font_metrics(font_id, font_size);
    let cell_width = metrics.advance(' '); // monospace 任意字符宽度相同
    let cell_height = font_size * metrics.line_height_factor();
    (cell_width, cell_height)
}
```

> **GPUI-API**：实际方法名可能不同。看 Zed `terminal_element.rs` 中 `font_pixels` / `line_height` / `glyph_for_char` 的真实用法。

- [ ] **Step 3: 创建 `crates/aish-app/src/terminal/grid_renderer.rs`**

```rust
//! 自绘 alacritty Term grid。
//!
//! 关键设计：用 GPUI 低层 paint API（不每个 cell 一个 div），参考 Zed
//! `crates/terminal_view/src/terminal_element.rs`。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::index::Point;
use alacritty_terminal::Term;
use gpui::{Hsla, Pixels, Window};

use super::colors;

pub struct GridLayout {
    pub cell_width: Pixels,
    pub cell_height: Pixels,
    pub origin_x: Pixels,
    pub origin_y: Pixels,
}

impl GridLayout {
    pub fn cell_to_pixel(&self, point: Point) -> (Pixels, Pixels) {
        (
            self.origin_x + self.cell_width * point.column.0 as f32,
            self.origin_y + self.cell_height * point.line.0 as f32,
        )
    }
}

/// 渲染 Term 的可见 grid 到 GPUI Window。
///
/// 此函数应该在 Element::paint 阶段调用（非 Render::render）。
pub fn paint_grid(
    term: &Term<VoidListener>,
    layout: &GridLayout,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    use super::font;

    // 1. 拿字体 + font_id
    // GPUI-API: 方式按 Zed terminal_element 实际写
    let _font_size = font::FONT_SIZE;
    let _ = (window, cx);

    // 2. 遍历 grid display_iter（只画可见 viewport）
    let grid = term.grid();
    for cell in grid.display_iter() {
        let point = cell.point;
        let (px, py) = layout.cell_to_pixel(point);

        // 2a. 算前景/背景色
        let fg = colors::to_gpui(cell.fg, true);
        let bg = colors::to_gpui(cell.bg, false);

        // 2b. 画背景矩形（如果不是默认背景）
        if !is_default_bg(bg) {
            // GPUI-API: window.paint_quad(Bounds::new(point(px, py), size(cell_width, cell_height)), bg, ...)
            // 参考 Zed terminal_element 的 paint 实现
            let _ = (px, py, fg);
        }

        // 2c. 画字符
        if cell.c != ' ' && cell.c != '\0' {
            // GPUI-API: window.paint_text 或 line.paint
            let _ = cell.c;
        }
    }
}

fn is_default_bg(_color: Hsla) -> bool {
    // 简化：M2b1 暂时所有 bg 都画（不优化）
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::{Column, Line};
    use gpui::Pixels;

    #[test]
    fn cell_to_pixel_at_origin() {
        let layout = GridLayout {
            cell_width: Pixels(10.0),
            cell_height: Pixels(20.0),
            origin_x: Pixels(0.0),
            origin_y: Pixels(0.0),
        };
        let (x, y) = layout.cell_to_pixel(Point::new(Line(0), Column(0)));
        assert_eq!(x, Pixels(0.0));
        assert_eq!(y, Pixels(0.0));
    }

    #[test]
    fn cell_to_pixel_at_offset() {
        let layout = GridLayout {
            cell_width: Pixels(10.0),
            cell_height: Pixels(20.0),
            origin_x: Pixels(5.0),
            origin_y: Pixels(10.0),
        };
        let (x, y) = layout.cell_to_pixel(Point::new(Line(2), Column(3)));
        assert_eq!(x, Pixels(5.0 + 30.0));
        assert_eq!(y, Pixels(10.0 + 40.0));
    }
}
```

⚠️ 这个 grid_renderer.rs 是**接口骨架** + cell_to_pixel 完整实现。真正 paint_grid 内的 paint_quad / paint_text 调用，按 Zed terminal_element 实际 API 填进去。如果 implementer 看 Zed 后觉得用 Element trait 比 Render 更合适（terminal 渲染 Zed 是用 custom Element），那就把 TerminalView 重构为 Element。

- [ ] **Step 4: 创建 `crates/aish-app/src/terminal/cursor.rs`**

```rust
//! 光标渲染：方块 + 闪烁 600ms + 失焦时空心。

use std::time::{Duration, Instant};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::Term;
use gpui::{Hsla, Pixels, Window};

use super::colors;
use super::grid_renderer::GridLayout;

/// 闪烁周期（500ms 显示 + 100ms 不显示 + 100ms 不显示 + 500ms 显示...）
/// 简化为 600ms 周期：前 300ms 显示，后 300ms 不显示
pub const BLINK_PERIOD_MS: u64 = 600;

#[derive(Clone, Copy)]
pub struct CursorState {
    /// 创建时间，用于算闪烁相位
    pub epoch: Instant,
    /// 是否聚焦（影响光标样式：实心 vs 空心）
    pub focused: bool,
}

impl CursorState {
    pub fn new(focused: bool) -> Self {
        Self {
            epoch: Instant::now(),
            focused,
        }
    }

    /// 当前是否应该显示光标（根据闪烁相位）。
    pub fn is_visible_now(&self) -> bool {
        let elapsed_ms = self.epoch.elapsed().as_millis() as u64;
        let phase = elapsed_ms % BLINK_PERIOD_MS;
        phase < (BLINK_PERIOD_MS / 2)
    }
}

/// 在 GPUI Window 上画光标。
///
/// 实心方块（聚焦）：bg = 前景色，char 渲染为 bg 色
/// 空心方块（失焦）：仅描边
pub fn paint_cursor(
    term: &Term<VoidListener>,
    cursor_state: &CursorState,
    layout: &GridLayout,
    window: &mut Window,
    _cx: &mut gpui::App,
) {
    if !cursor_state.is_visible_now() {
        return;
    }

    let cursor_point = term.grid().cursor.point;
    let (_px, _py) = layout.cell_to_pixel(cursor_point);
    let _color: Hsla =
        colors::to_gpui(alacritty_terminal::vte::ansi::Color::Named(
            alacritty_terminal::vte::ansi::NamedColor::Foreground,
        ), true);

    // GPUI-API: window.paint_quad 画矩形
    if cursor_state.focused {
        // 实心：fill cell with cursor color
    } else {
        // 空心：仅描边
    }
    let _ = (window, _px, _py, _color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_visible_first_300ms() {
        let mut state = CursorState::new(true);
        // 把 epoch 改为现在 — 应该 visible
        state.epoch = Instant::now();
        assert!(state.is_visible_now());
    }

    #[test]
    fn blink_period_constant() {
        assert_eq!(BLINK_PERIOD_MS, 600);
    }
}
```

⚠️ paint_cursor 也是骨架；具体 paint_quad 调用按 Zed cursor 渲染实际写。

- [ ] **Step 5: 加 mod 声明**

`crates/aish-app/src/terminal/mod.rs`：

```rust
//! 终端渲染模块。

#![allow(dead_code)]

pub mod colors;
pub mod cursor;
pub mod font;
pub mod grid_renderer;
```

- [ ] **Step 6: 改写 `crates/aish-app/src/views/terminal_view.rs`** — 真渲染

整体替换 Render 实现部分（保留 struct + new + handle_key + Focusable）。**这部分最依赖 GPUI API 实际形态**：

```rust
//! 主区终端视图。M2b1 真自绘 grid + 光标 + 闪烁。

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, Entity, FocusHandle, Focusable, KeyDownEvent,
    Pixels, Window,
};

use crate::bridge::Bridge;
use crate::ssh_actor::encode_key;
use crate::state::{AppState, SessionCommand, SshEvent};
use crate::terminal::{cursor::CursorState, font, grid_renderer};

pub struct TerminalView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
    cursor_state: CursorState,
}

impl TerminalView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        let cursor_state = CursorState::new(true);

        // 启动闪烁定时器：每 300ms 触发 cx.notify 重绘
        cx.spawn(async move |this, mut cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                if this
                    .update(&mut cx, |_this, cx| {
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Self {
            state,
            bridge,
            tx,
            focus_handle,
            cursor_state,
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let host = match self.state.read(cx).selected {
            Some(h) => h,
            None => return,
        };
        let sender = match self.state.read(cx).sessions.get(&host).cloned() {
            Some(s) => s,
            None => return,
        };

        let ctrl = event.keystroke.modifiers.control;
        let alt = event.keystroke.modifiers.alt;
        let key = event.keystroke.key.as_str();

        let bytes = encode_key(key, ctrl, alt);
        if bytes.is_empty() {
            return;
        }

        self.bridge.spawn(async move {
            let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
        });
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.state.read(cx).selected;
        let cursor_state = self.cursor_state;
        let state_entity = self.state.clone();

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key(event, cx);
            }))
            .flex_1()
            .h_full()
            .bg(gpui::rgb(0x1d1f21))
            .child(canvas(
                move |_bounds, _window, _cx| {},
                move |bounds: Bounds<Pixels>, _drawn, window, cx| {
                    // 真自绘逻辑放这里 — Element painter callback
                    paint_terminal(host, &state_entity, &cursor_state, bounds, window, cx);
                },
            )
            .size_full())
    }
}

fn paint_terminal(
    host: Option<aish_types::HostId>,
    state: &Entity<AppState>,
    cursor_state: &CursorState,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let host = match host {
        Some(h) => h,
        None => {
            // 渲染 placeholder（GPUI-API: window.paint_text）
            return;
        }
    };
    let app_state = state.read(cx);
    let term = match app_state.term_of(host) {
        Some(t) => t,
        None => return,
    };

    let (cell_width, cell_height) = font::cell_size(cx);
    let layout = grid_renderer::GridLayout {
        cell_width,
        cell_height,
        origin_x: bounds.origin.x + px(8.0),
        origin_y: bounds.origin.y + px(8.0),
    };

    grid_renderer::paint_grid(term, &layout, window, cx);
    crate::terminal::cursor::paint_cursor(term, cursor_state, &layout, window, cx);
}
```

> ⚠️ **GPUI canvas API**：实际 GPUI 提供 `canvas()` 元素或类似 API 让你给两个 closure（layout + paint）。看 Zed terminal_element 实际怎么用。如果 GPUI 没有 `canvas`，用 `Element` trait 直接 impl。
>
> ⚠️ **font::register_bundled_font**：必须在 app::run() 启动时调一次（在创建任何 view 之前）。修改 app.rs 的 run() 函数，在 application().run(...) closure 头部加：
> 
> ```rust
> application().run(move |cx: &mut App| {
>     crate::terminal::font::register_bundled_font(cx);
>     // ... 原有代码
> });
> ```

- [ ] **Step 7: 在 app.rs 注册字体**

读 app.rs，找 `application().run(move |cx: &mut App| {` 之后的第一行，插入：

```rust
        crate::terminal::font::register_bundled_font(cx);
```

- [ ] **Step 8: 验证 build / test / run**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -30
```

如失败按 GPUI / alacritty_terminal API 调整。

```bash
cargo test --workspace 2>&1 | tail -10
```

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t4-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t4-err.txt"
Start-Sleep -Seconds 8
if ($proc.HasExited) {
    Write-Output "FAIL"
    Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t4-err.txt" -Tail 30
} else {
    Write-Output "OK"
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t4-*.txt" -Force
```

预期：窗口能开 + 没 panic。subagent 没 host 凭证，看不到真渲染，但 build + 启动 OK。

- [ ] **Step 9: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/
git commit -m "feat(aish-app): TerminalView 自绘 grid + 颜色 + 光标闪烁"
```

---

## Task 5: PTY resize 链路（debounce + cell metric → SessionCommand::Resize）

**Files:**
- Modify: `crates/aish-app/src/views/terminal_view.rs`（加 resize 检测）

- [ ] **Step 1: 在 TerminalView 加 last_size 跟踪 + debounce**

读 terminal_view.rs，在 struct TerminalView 加字段：

```rust
pub struct TerminalView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
    cursor_state: CursorState,
    last_pane_size: Option<gpui::Size<gpui::Pixels>>,
    pending_resize: Option<gpui::Task<()>>, // GPUI-API: 取消的 task handle
}
```

- [ ] **Step 2: 在 render 中检测 size 变化触发 debounce resize**

修改 `Render::render` 的 canvas painter callback。在 paint_terminal 调用前后加 size 检测：

```rust
.child(canvas(
    move |bounds: Bounds<Pixels>, window, cx| {
        // layout phase: 检测 size 变化
        // 由 implementer 接通 GPUI api
    },
    move |bounds: Bounds<Pixels>, _drawn, window, cx| {
        paint_terminal(host, &state_entity, &cursor_state, bounds, window, cx);
    },
))
```

具体 resize 检测逻辑放在 view 自身的方法里，由 cx.subscribe window resize 事件触发：

```rust
impl TerminalView {
    fn check_resize(&mut self, new_size: gpui::Size<gpui::Pixels>, cx: &mut Context<Self>) {
        if Some(new_size) == self.last_pane_size {
            return;
        }
        self.last_pane_size = Some(new_size);

        // 取消上次 pending
        self.pending_resize.take();

        // 算 cols/rows
        let (cw, ch) = crate::terminal::font::cell_size(cx);
        if cw.0 <= 0.0 || ch.0 <= 0.0 {
            return;
        }
        let cols = ((new_size.width.0 - 16.0) / cw.0).floor().max(1.0) as u16;
        let rows = ((new_size.height.0 - 16.0) / ch.0).floor().max(1.0) as u16;

        let host = match self.state.read(cx).selected {
            Some(h) => h,
            None => return,
        };

        let sender = self.state.read(cx).sessions.get(&host).cloned();
        let state = self.state.clone();
        let bridge = self.bridge.clone();

        // 启 100ms debounce
        let task = cx.spawn(async move |_this, mut cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;

            // 真触发 resize
            let _ = state.update(&mut cx, |state, cx| {
                state.resize_term(host, cols, rows);
                cx.notify();
            });

            if let Some(sender) = sender {
                bridge.spawn(async move {
                    let _ = sender
                        .send(SessionCommand::Resize { cols, rows })
                        .await;
                });
            }
        });
        self.pending_resize = Some(task);
    }
}
```

> **GPUI-API**：`gpui::Task` 类型可能在不同 path（`gpui::AsyncTask` / `Task<()>`）。如果它没有 Drop = Cancel 的语义，就用 `tokio::sync::Notify` + spawn task 自检方式。看 Zed Editor 的 debounce 实现（grep `debounce` in zed-reference）。

- [ ] **Step 3: 在 render 中调用 check_resize**

修改 render：

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // ... 现有逻辑 ...

    div()
        .on_children_prepainted(cx.listener(|this, _, window, cx| {
            // 拿到这个 div 的实际 bounds
            // GPUI-API: window.bounds 或 element bounds
            // 这里用一个简化方案：通过 canvas 的 layout callback 拿 size，再调 check_resize
        }))
        // ... 其他不变 ...
}
```

**或者更简单**：在 canvas 的 prepaint callback 里 check_resize（cx 借不借得到看 GPUI Element trait）。如果 GPUI canvas 不能在 prepaint 调 cx.update，**改用 Element trait 直接 impl**——参考 Zed terminal_element。

⚠️ 这个 Step 要 implementer 看 Zed 实际 resize 处理方式调整。

- [ ] **Step 4: 验证 build + run**

```bash
cargo build -p aish-app
cargo test --workspace
```

后台跑窗口验证不 panic（resize 行为要 user 手动验证）：

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t5-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t5-err.txt"
Start-Sleep -Seconds 8
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2b1-t5-*.txt" -Force
```

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/src/views/terminal_view.rs
git commit -m "feat(aish-app): PTY 跟随窗口 resize（100ms debounce）"
```

---

## Task 6: Mouse selection + Ctrl+Shift+C 复制

**Files:**
- Create: `crates/aish-app/src/terminal/selection.rs`
- Modify: `crates/aish-app/src/terminal/mod.rs`
- Modify: `crates/aish-app/src/views/terminal_view.rs`

- [ ] **Step 1: 创建 `crates/aish-app/src/terminal/selection.rs`**

```rust
//! Mouse drag → alacritty Term selection 状态机。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::Term;
use gpui::Pixels;

use super::grid_renderer::GridLayout;

/// 把 pixel 坐标 → Term grid 坐标。
///
/// 返回 (Line, Column, Side) — Side 是 Left/Right，决定该 cell 是否完整在选中范围。
pub fn pixel_to_grid(
    px: Pixels,
    py: Pixels,
    layout: &GridLayout,
    grid_lines: usize,
    grid_cols: usize,
) -> (Line, Column, Side) {
    let local_x = (px - layout.origin_x).0;
    let local_y = (py - layout.origin_y).0;

    let row = (local_y / layout.cell_height.0)
        .floor()
        .clamp(0.0, (grid_lines as f32 - 1.0).max(0.0)) as i32;
    let col_f = (local_x / layout.cell_width.0).clamp(0.0, grid_cols as f32);
    let col = col_f.floor().clamp(0.0, (grid_cols as f32 - 1.0).max(0.0)) as usize;

    // Side：x 落在 cell 左半边 → Left，右半边 → Right
    let frac = col_f - col_f.floor();
    let side = if frac < 0.5 { Side::Left } else { Side::Right };

    (Line(row), Column(col), side)
}

/// 开始一次 mouse selection。
pub fn start_selection(term: &mut Term<VoidListener>, line: Line, col: Column, side: Side) {
    term.selection = Some(Selection::new(SelectionType::Simple, Point::new(line, col), side));
}

/// 拖拽过程中更新选中末端。
pub fn update_selection(term: &mut Term<VoidListener>, line: Line, col: Column, side: Side) {
    if let Some(ref mut sel) = term.selection {
        sel.update(Point::new(line, col), side);
    }
}

/// 清除 selection。
pub fn clear_selection(term: &mut Term<VoidListener>) {
    term.selection = None;
}

/// 取选中的文本（用于复制到剪贴板）。空选中或无 selection 返回 None。
pub fn selected_text(term: &Term<VoidListener>) -> Option<String> {
    let sel = term.selection.as_ref()?;
    let range = sel.to_range(term)?;
    let mut text = String::new();
    let grid = term.grid();
    for line in range.start.line.0..=range.end.line.0 {
        let row = &grid[Line(line)];
        let start_col = if line == range.start.line.0 {
            range.start.column.0
        } else {
            0
        };
        let end_col = if line == range.end.line.0 {
            range.end.column.0
        } else {
            grid.columns() - 1
        };
        for col in start_col..=end_col {
            text.push(row[Column(col)].c);
        }
        if line < range.end.line.0 {
            text.push('\n');
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    fn mk_layout() -> GridLayout {
        GridLayout {
            cell_width: Pixels(10.0),
            cell_height: Pixels(20.0),
            origin_x: Pixels(0.0),
            origin_y: Pixels(0.0),
        }
    }

    #[test]
    fn pixel_at_origin_is_grid_zero_zero() {
        let (line, col, side) = pixel_to_grid(px(0.0), px(0.0), &mk_layout(), 24, 80);
        assert_eq!(line, Line(0));
        assert_eq!(col, Column(0));
        assert_eq!(side, Side::Left);
    }

    #[test]
    fn pixel_at_cell_2_3_center() {
        // 列 3 中心 = x = 35
        let (line, col, side) = pixel_to_grid(px(35.0), px(45.0), &mk_layout(), 24, 80);
        assert_eq!(line, Line(2));
        assert_eq!(col, Column(3));
        assert_eq!(side, Side::Right);
    }

    #[test]
    fn pixel_clamp_to_grid_max() {
        let (line, col, _) = pixel_to_grid(px(10000.0), px(10000.0), &mk_layout(), 24, 80);
        assert_eq!(line, Line(23));
        assert_eq!(col, Column(79));
    }
}
```

> ALAC-API: `Selection::new` / `to_range` / `term.selection` 字段实际签名按 alacritty 0.25 调整。看 zed-reference/crates/terminal/src/lib.rs 的 selection 处理。

- [ ] **Step 2: 加 mod 声明**

`crates/aish-app/src/terminal/mod.rs`：

```rust
pub mod colors;
pub mod cursor;
pub mod font;
pub mod grid_renderer;
pub mod selection;
```

- [ ] **Step 3: TerminalView 加 mouse 事件 + Ctrl+Shift+C handler**

读 terminal_view.rs，在 handle_key 里加 Ctrl+Shift+C 拦截（在 encode_key 调用之前）：

```rust
fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
    let host = match self.state.read(cx).selected {
        Some(h) => h,
        None => return,
    };

    let ctrl = event.keystroke.modifiers.control;
    let shift = event.keystroke.modifiers.shift;
    let alt = event.keystroke.modifiers.alt;
    let key = event.keystroke.key.as_str();

    // Ctrl+Shift+C: 复制选中（不发到远端）
    if ctrl && shift && (key == "c" || key == "C") {
        let text = self.state.read(cx)
            .term_of(host)
            .and_then(crate::terminal::selection::selected_text);
        if let Some(text) = text {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        return;
    }

    let sender = match self.state.read(cx).sessions.get(&host).cloned() {
        Some(s) => s,
        None => return,
    };

    let bytes = encode_key(key, ctrl, alt);
    if bytes.is_empty() {
        return;
    }

    self.bridge.spawn(async move {
        let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
    });
}
```

> GPUI-API: `cx.write_to_clipboard` / `ClipboardItem::new_string` 实际签名按 GPUI 当前调整。看 zed-reference 的 clipboard 用法。

- [ ] **Step 4: 加 mouse_down / mouse_move / mouse_up handler**

在 TerminalView 的 Render::render 中，在 div() 链上加：

```rust
.on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
    this.handle_mouse_down(ev, cx);
}))
.on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
    if ev.dragging() {
        this.handle_mouse_move(ev, cx);
    }
}))
```

并加方法：

```rust
fn handle_mouse_down(&mut self, ev: &gpui::MouseDownEvent, cx: &mut Context<Self>) {
    let host = match self.state.read(cx).selected {
        Some(h) => h,
        None => return,
    };
    let layout = self.current_layout(cx);
    let (line, col, side) = crate::terminal::selection::pixel_to_grid(
        ev.position.x, ev.position.y, &layout, 40, 120,  // grid_lines/cols 应来自 dimensions
    );
    self.state.update(cx, |state, cx| {
        if let Some(term) = state.pane_terminals.get_mut(&host) {
            crate::terminal::selection::start_selection(term, line, col, side);
        }
        cx.notify();
    });
}

fn handle_mouse_move(&mut self, ev: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
    let host = match self.state.read(cx).selected {
        Some(h) => h,
        None => return,
    };
    let layout = self.current_layout(cx);
    let (line, col, side) = crate::terminal::selection::pixel_to_grid(
        ev.position.x, ev.position.y, &layout, 40, 120,
    );
    self.state.update(cx, |state, cx| {
        if let Some(term) = state.pane_terminals.get_mut(&host) {
            crate::terminal::selection::update_selection(term, line, col, side);
        }
        cx.notify();
    });
}

fn current_layout(&self, cx: &App) -> grid_renderer::GridLayout {
    let (cw, ch) = font::cell_size(cx);
    grid_renderer::GridLayout {
        cell_width: cw,
        cell_height: ch,
        origin_x: gpui::px(8.0),
        origin_y: gpui::px(8.0),
    }
}
```

注意：`grid_lines` / `grid_cols` 实际值来自 state.pane_dimensions[host]，简化代码先用 hardcoded 40/120，后续接入。

- [ ] **Step 5: 在 grid_renderer.rs 加 selection 高亮渲染**

Selection 的高亮是覆盖在普通背景上的半透明蓝色 (#3a3a8a 80% opacity)。Task 4 的 paint_grid 函数中，每个 cell 渲染前检查是否在 selection range，是的话画选中色背景：

修改 grid_renderer.rs 的 paint_grid：

```rust
pub fn paint_grid(
    term: &Term<VoidListener>,
    layout: &GridLayout,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let selection_range = term
        .selection
        .as_ref()
        .and_then(|sel| sel.to_range(term));

    let grid = term.grid();
    for cell in grid.display_iter() {
        let point = cell.point;
        let (px, py) = layout.cell_to_pixel(point);

        let fg = colors::to_gpui(cell.fg, true);
        let bg = colors::to_gpui(cell.bg, false);

        // 选中高亮覆盖
        let is_selected = selection_range
            .as_ref()
            .map(|r| in_range(point, r))
            .unwrap_or(false);
        let final_bg = if is_selected {
            // 选中色：#3a3a8a 80% opacity
            gpui::hsla(0.66, 0.4, 0.38, 0.8)
        } else {
            bg
        };

        // 画背景（按需）+ 字符
        // GPUI-API 实际 paint
        let _ = (px, py, fg, final_bg, window, cx);
    }
}

fn in_range(
    point: Point,
    range: &alacritty_terminal::selection::SelectionRange,
) -> bool {
    let p = (point.line.0, point.column.0);
    let s = (range.start.line.0, range.start.column.0);
    let e = (range.end.line.0, range.end.column.0);
    p >= s && p <= e
}
```

- [ ] **Step 6: 验证 build / test**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app
cargo test --workspace 2>&1 | tail -10
```

Expected: selection 3 个测试通过 + workspace 全绿。

- [ ] **Step 7: fmt + clippy + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/aish-app/src/terminal/selection.rs crates/aish-app/src/terminal/mod.rs crates/aish-app/src/terminal/grid_renderer.rs crates/aish-app/src/views/terminal_view.rs
git commit -m "feat(aish-app): mouse 选中 + Ctrl+Shift+C 复制"
```

---

## Task 7: 扩展 encode_key — 方向键 / Home/End/PageUp/Down

**Files:**
- Modify: `crates/aish-app/src/ssh_actor.rs`

- [ ] **Step 1: 扩展 encode_key 函数 + 加测试**

读 ssh_actor.rs，找 encode_key 函数。修改为：

```rust
pub fn encode_key(key: &str, ctrl: bool, _alt: bool) -> Vec<u8> {
    if ctrl {
        if let Some(c) = key.chars().next() {
            let upper = c.to_ascii_uppercase();
            if upper.is_ascii_uppercase() {
                let byte = (upper as u8) - 0x40;
                return vec![byte];
            }
        }
        return Vec::new();
    }

    match key.to_lowercase().as_str() {
        "enter" => vec![b'\r'],
        "backspace" => vec![0x7f],
        "tab" => vec![b'\t'],
        "escape" | "esc" => vec![0x1b],

        // 方向键 (normal mode CSI)
        "up" | "arrowup" => b"\x1b[A".to_vec(),
        "down" | "arrowdown" => b"\x1b[B".to_vec(),
        "right" | "arrowright" => b"\x1b[C".to_vec(),
        "left" | "arrowleft" => b"\x1b[D".to_vec(),

        // 导航键
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        "insert" => b"\x1b[2~".to_vec(),

        s if s.len() == 1 => key.as_bytes().to_vec(), // 用原 key 而非 lowercased
        _ => Vec::new(),
    }
}
```

**关键变化**：
- 用 `key.to_lowercase()` 比较，让 GPUI 是 "Up" / "ArrowUp" / "up" 都能匹配
- 但单字符仍用原 `key` 不 lowercased（避免 "A" 变 "a"）
- 加 10 个新 key → ANSI 序列

- [ ] **Step 2: 加测试**

在 mod tests 末尾追加：

```rust
    #[test]
    fn encode_arrow_keys_normal_mode() {
        assert_eq!(encode_key("up", false, false), b"\x1b[A".to_vec());
        assert_eq!(encode_key("ArrowUp", false, false), b"\x1b[A".to_vec());
        assert_eq!(encode_key("down", false, false), b"\x1b[B".to_vec());
        assert_eq!(encode_key("right", false, false), b"\x1b[C".to_vec());
        assert_eq!(encode_key("left", false, false), b"\x1b[D".to_vec());
    }

    #[test]
    fn encode_navigation_keys() {
        assert_eq!(encode_key("home", false, false), b"\x1b[H".to_vec());
        assert_eq!(encode_key("end", false, false), b"\x1b[F".to_vec());
        assert_eq!(encode_key("pageup", false, false), b"\x1b[5~".to_vec());
        assert_eq!(encode_key("pagedown", false, false), b"\x1b[6~".to_vec());
        assert_eq!(encode_key("delete", false, false), b"\x1b[3~".to_vec());
        assert_eq!(encode_key("insert", false, false), b"\x1b[2~".to_vec());
    }

    #[test]
    fn encode_uppercase_chars_preserve_case() {
        assert_eq!(encode_key("Z", false, false), b"Z");
        assert_eq!(encode_key("A", false, false), b"A");
    }
```

- [ ] **Step 3: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo test -p aish-app encode 2>&1 | tail -10
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: encode_key 测试 7 个（4 旧 + 3 新）+ workspace 全绿。

- [ ] **Step 4: commit**

```bash
git add crates/aish-app/src/ssh_actor.rs
git commit -m "feat(aish-app): encode_key 加方向键 / Home/End/PageUp/Down 等导航键"
```

---

## Task 8: 端到端验证 + push（user 手动 demo）

⚠️ subagent 没法完成 demo 验证 — 由 user 本人按 spec Section 9 15 项手动核对。

**Files:** 无文件改动，只验证 + push。

- [ ] **Step 1: 跑全部自动化验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 全部退出码 0。test 数量比 M2a 增加 ~15-25 个（colors 8 + font 4 + grid_renderer 2 + cursor 2 + selection 3 + state 5-6 + encode 3）。

- [ ] **Step 2: push 到 origin**

```bash
git push origin main
```

如网络问题失败重试。

- [ ] **Step 3: 等 GitHub Actions CI 跑完**

```bash
gh run list --limit 1
gh run watch
```

如 Linux GPUI 失败、russh window_change 不可用等问题，记录 follow-up。

- [ ] **Step 4: 提示 user 手动验证 demo 15 项**

implementer 在报告中明确列出（与 spec Section 9 一致）：

```
demo 验证由 user 完成。请按 spec Section 9 15 项手动核对：

1. 设置环境变量：
   $env:AISH_DEV_HOST = "<vps-host>"
   $env:AISH_DEV_USER = "<user>"
   $env:AISH_DEV_KEY_PATH = "<key-path>"
   $env:AISH_DEV_LABEL = "my-vps"

2. cargo run -p aish-app

3. 验证：
   1) 字体是 JetBrains Mono Nerd Font（与 M2a 系统字体有视觉差异）
   2) 点 my-vps → shell prompt 无乱码，颜色正确（目录蓝/可执行绿/symlink 青）
   3) ls --color → 看到带颜色的目录列表
   4) vim /tmp/test.txt → 完整 vim 界面 + 行号 + 状态栏；i 进 insert + 输入 + Esc + :wq
   5) top → 刷新表格 + CPU/MEM 颜色 bar；q 退出
   6) 光标方块 + 闪烁约 600ms 周期；点窗口外失焦后变空心
   7) Mouse drag 选中 → 半透明蓝色高亮
   8) Ctrl+Shift+C → 选中文本进剪贴板；外部应用 Ctrl+V 粘贴看到原文
   9) 拖窗口大小 → 主区跟随 → 远端 tput cols && tput lines 输出新值
   10) Ctrl+C 中断 top 类长跑命令
   11) bash 中按 ↑ → 上一条命令；连续 ↑ 翻历史
   12) 长命令行按 Home → 行首；End → 行尾
   13) cargo test --workspace 全绿
   14) cargo fmt + clippy 全绿
   15) GitHub Actions CI 三平台全绿
```

- [ ] **Step 5: 完成报告**

```
STATUS: DONE_WITH_PENDING_USER_VERIFICATION

Files: 无文件改动

Verification (subagent 跑的):
- cargo build: PASS
- cargo test --workspace: X passed
- cargo fmt --check: PASS
- cargo clippy: PASS
- git push: 成功 / 失败
- CI 状态: started / completed / failed

Pending (user 手动):
- demo 15 项（spec Section 9）

Concerns: ...
```

---

## 完成验证（M2b1 整体）

```bash
cargo build --workspace
cargo test --workspace                                 # ~50 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p aish-app                                  # demo 15 项 user 手动
```

git log 应有 8 个新 commit + spec + plan = 10 个新 commit。

---

## 下一步

M2b1 完成后开始 M2b2（完整 xterm 键盘 + scrollback 滚动 UI）。M2b1 留下的 onramp：
- alacritty Term 持有完整 scrollback buffer，仅缺 viewport scroll API + UI
- encode_key 已支持 normal mode；M2b2 加 DECCKM 应用键模式判断
- TerminalView 渲染框架已建立；M2b2 加滚轮事件 + 滚动条

M2b2 不在本 plan 范围。M2b1 完成后单独 brainstorm → spec → plan → implement。
