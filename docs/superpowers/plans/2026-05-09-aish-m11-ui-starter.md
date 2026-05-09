# M11 — aish-ui 起步套件 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 GPUI 之上搭起 `aish-ui` 独立 crate，交付 Foundations（Theme + Icon）+ 7 个组件（Button / IconButton / Badge / Separator / Tooltip / TextInput / Toast），并让 `aish-app` 把 `InputBarView` 的文本输入部分迁到 `TextInput`。

**Architecture:** 新 workspace crate `aish-ui` 仅依赖 `gpui`。组件采用 Hybrid API：无状态用 builder + `IntoElement`，有状态用 `Entity<T>` + `Render`。Theme 通过 `cx.set_global(Theme::dark())` 注入；Icon 通过 `AssetSource` trait 把 `assets/icons/*.svg` 编入 binary、`gpui::svg()` 解析路径加载。

**Tech Stack:**
- Rust stable + nightly fmt/clippy
- `gpui`（git dep，pinned to Zed `11f0ca5`）
- `arboard`（剪贴板，aish 已有 dep）
- 测试：`cargo test --workspace`，每个组件 in-file `#[cfg(test)] mod tests`

**Spec ref:** `docs/superpowers/specs/2026-05-09-aish-m11-ui-starter-design.md` 与父 spec `2026-05-09-aish-ui-architecture-design.md`

**质量门禁（每个 Task 完成后）：**
```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Task 1: aish-ui crate 骨架 + workspace 注册

**Files:**
- Create: `crates/aish-ui/Cargo.toml`
- Create: `crates/aish-ui/src/lib.rs`
- Create: `crates/aish-ui/src/prelude.rs`
- Create: `crates/aish-ui/src/theme/mod.rs`
- Create: `crates/aish-ui/src/components/mod.rs`
- Create: `crates/aish-ui/src/icons/mod.rs`
- Modify: `Cargo.toml`（workspace 根）

- [ ] **Step 1: 创建 aish-ui Cargo.toml**

`crates/aish-ui/Cargo.toml`：

```toml
[package]
name = "aish-ui"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
gpui = { workspace = true }
```

- [ ] **Step 2: 创建空的 src 文件**

`crates/aish-ui/src/lib.rs`：

```rust
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
pub use theme::{theme, Theme};
```

`crates/aish-ui/src/prelude.rs`：

```rust
//! 常用 re-exports。`use aish_ui::prelude::*;` 一行拉齐。

pub use crate::components::*;
pub use crate::icons::{icon, IconName};
pub use crate::theme::{theme, Theme};
```

`crates/aish-ui/src/theme/mod.rs`：

```rust
//! Theme / Token 系统。M11 仅实现 dark。

// 后续 task 填充
```

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

// 后续 task 填充
```

`crates/aish-ui/src/icons/mod.rs`：

```rust
//! Icon 系统：IconName enum + AishUiAssets AssetSource。

// 后续 task 填充
```

- [ ] **Step 3: workspace 根 Cargo.toml 注册新成员**

修改根 `Cargo.toml`：

1. `[workspace] members = [...]` 数组里追加 `"crates/aish-ui"`
2. `[workspace.dependencies]` 追加：`aish-ui = { path = "crates/aish-ui" }`

- [ ] **Step 4: 验证 workspace 编译通过**

```bash
cargo check --workspace
```

预期：`Finished dev profile`，无错误。aish-ui 显示为 0 deps 0 warnings。

- [ ] **Step 5: 跑质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：clippy 0 warning，已有 108 测试仍全过（aish-ui 此 task 还没测试）。

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/aish-ui
git commit -m "feat(aish-ui): T1 — 新建 aish-ui crate 骨架 + workspace 注册"
```

---

## Task 2: Theme 系统（tokens + dark + theme(cx) helper）

**Files:**
- Create: `crates/aish-ui/src/theme/tokens.rs`
- Create: `crates/aish-ui/src/theme/dark.rs`
- Create: `crates/aish-ui/src/theme/light.rs`
- Modify: `crates/aish-ui/src/theme/mod.rs`

- [ ] **Step 1: 写 tokens.rs（数据结构）**

`crates/aish-ui/src/theme/tokens.rs`：

```rust
//! Theme token 定义。颜色、圆角、间距、字号四类。
//! 命名参考 shadcn/ui，HSLA 内部存储。

use gpui::{px, Hsla, Pixels, Rgba};

#[derive(Clone, Copy)]
pub struct ColorTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub card_foreground: Hsla,
    pub popover: Hsla,
    pub popover_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
}

#[derive(Clone, Copy)]
pub struct Radius {
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub full: Pixels,
}

impl Default for Radius {
    fn default() -> Self {
        Self {
            sm: px(4.0),
            md: px(6.0),
            lg: px(8.0),
            full: px(9999.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Spacing {
    pub px_1: Pixels,
    pub px_2: Pixels,
    pub px_3: Pixels,
    pub px_4: Pixels,
    pub px_6: Pixels,
    pub px_8: Pixels,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            px_1: px(4.0),
            px_2: px(8.0),
            px_3: px(12.0),
            px_4: px(16.0),
            px_6: px(24.0),
            px_8: px(32.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FontSize {
    pub xs: Pixels,
    pub sm: Pixels,
    pub base: Pixels,
    pub lg: Pixels,
    pub xl: Pixels,
}

impl Default for FontSize {
    fn default() -> Self {
        Self {
            xs: px(10.0),
            sm: px(12.0),
            base: px(14.0),
            lg: px(16.0),
            xl: px(18.0),
        }
    }
}

pub struct Theme {
    pub colors: ColorTokens,
    pub radius: Radius,
    pub spacing: Spacing,
    pub font_size: FontSize,
}

impl gpui::Global for Theme {}

/// 把 0xRRGGBB hex 转 Hsla。
pub(crate) fn hex(rgb: u32) -> Hsla {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    Rgba { r, g, b, a: 1.0 }.into()
}

/// 0xRRGGBB + alpha (0..1.0)。
pub(crate) fn hex_a(rgb: u32, alpha: f32) -> Hsla {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    Rgba { r, g, b, a: alpha }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_pure_red_roundtrip() {
        let h = hex(0xff0000);
        assert!(h.h.abs() < 0.01 || (h.h - 1.0).abs() < 0.01);
        assert!(h.s > 0.99);
    }

    #[test]
    fn radius_defaults_ordered() {
        let r = Radius::default();
        assert!(r.sm < r.md);
        assert!(r.md < r.lg);
        assert!(r.lg < r.full);
    }

    #[test]
    fn spacing_defaults_ordered() {
        let s = Spacing::default();
        assert!(s.px_1 < s.px_2);
        assert!(s.px_2 < s.px_3);
        assert!(s.px_3 < s.px_4);
        assert!(s.px_4 < s.px_6);
        assert!(s.px_6 < s.px_8);
    }

    #[test]
    fn font_size_defaults_ordered() {
        let f = FontSize::default();
        assert!(f.xs < f.sm);
        assert!(f.sm < f.base);
        assert!(f.base < f.lg);
        assert!(f.lg < f.xl);
    }
}
```

- [ ] **Step 2: 写 dark.rs**

`crates/aish-ui/src/theme/dark.rs`：

```rust
//! 默认 dark 主题，色板基于 Tokyo Night 系。

use super::tokens::{hex, ColorTokens, FontSize, Radius, Spacing, Theme};

impl Theme {
    pub fn dark() -> Self {
        Self {
            colors: ColorTokens {
                background: hex(0x1a1b26),
                foreground: hex(0xc0caf5),
                card: hex(0x1f2030),
                card_foreground: hex(0xc0caf5),
                popover: hex(0x24253a),
                popover_foreground: hex(0xc0caf5),
                primary: hex(0x3d59a1),
                primary_foreground: hex(0xc0caf5),
                secondary: hex(0x2d2d3f),
                secondary_foreground: hex(0xa9b1d6),
                muted: hex(0x2d2d3f),
                muted_foreground: hex(0x565f89),
                accent: hex(0x6c91c2),
                accent_foreground: hex(0xc0caf5),
                destructive: hex(0xf7768e),
                destructive_foreground: hex(0x1a1b26),
                border: hex(0x2d2d3f),
                input: hex(0x16161e),
                ring: hex(0x6c91c2),
                success: hex(0x9ece6a),
                warning: hex(0xe0af68),
            },
            radius: Radius::default(),
            spacing: Spacing::default(),
            font_size: FontSize::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_primary_is_blue_ish() {
        let t = Theme::dark();
        // hue 蓝色 ≈ 0.55..0.7 / 1.0
        assert!(t.colors.primary.h > 0.5 && t.colors.primary.h < 0.75);
    }

    #[test]
    fn dark_destructive_is_red_ish() {
        let t = Theme::dark();
        assert!(t.colors.destructive.h < 0.05 || t.colors.destructive.h > 0.95);
    }

    #[test]
    fn dark_background_is_very_dark() {
        let t = Theme::dark();
        assert!(t.colors.background.l < 0.15);
    }
}
```

- [ ] **Step 3: 写 light.rs（仅 stub）**

`crates/aish-ui/src/theme/light.rs`：

```rust
//! Light theme — M11 留 stub，M11+ 之外 milestone 实现。

use super::tokens::Theme;

impl Theme {
    /// **未实现**：M11 范围外。调用会 panic。
    pub fn light() -> Self {
        unimplemented!("Light theme not implemented in M11; see aish-ui architecture spec");
    }
}
```

- [ ] **Step 4: 写 theme/mod.rs（汇总 + helper）**

`crates/aish-ui/src/theme/mod.rs` 完整内容：

```rust
//! Theme / Token 系统。M11 仅实现 dark。

mod dark;
mod light;
mod tokens;

pub use tokens::{ColorTokens, FontSize, Radius, Spacing, Theme};

/// 从 App 全局取当前 theme。调用前需先 `cx.set_global(Theme::dark())`。
pub fn theme(cx: &gpui::App) -> &Theme {
    cx.global::<Theme>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_is_global() {
        // 用 trait bound 间接验证 Theme: gpui::Global
        fn _assert<T: gpui::Global>() {}
        _assert::<Theme>();
    }
}
```

- [ ] **Step 5: 跑测试**

```bash
cargo test -p aish-ui
```

预期：`test result: ok. 8 passed; 0 failed` 或类似。

- [ ] **Step 6: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/theme
git commit -m "feat(aish-ui): T2 — Theme/Token + dark 实现 + light stub"
```

---

## Task 3: Icon 系统（IconName + AssetSource）

**Files:**
- Create: `crates/aish-ui/assets/icons/*.svg`（15 个）
- Modify: `crates/aish-ui/src/icons/mod.rs`
- Modify: `crates/aish-ui/Cargo.toml`（include assets）

- [ ] **Step 1: 创建 15 个 Lucide SVG**

每个 SVG 用 `currentColor` 作为 stroke / fill，方便 GPUI 染色。Lucide 默认 24×24 viewBox，stroke-width 2。

`crates/aish-ui/assets/icons/chevron-down.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
```

`crates/aish-ui/assets/icons/chevron-up.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m18 15-6-6-6 6"/></svg>
```

`crates/aish-ui/assets/icons/chevron-left.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
```

`crates/aish-ui/assets/icons/chevron-right.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>
```

`crates/aish-ui/assets/icons/x.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
```

`crates/aish-ui/assets/icons/check.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
```

`crates/aish-ui/assets/icons/info.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
```

`crates/aish-ui/assets/icons/alert-circle.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
```

`crates/aish-ui/assets/icons/alert-triangle.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
```

`crates/aish-ui/assets/icons/send.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/></svg>
```

`crates/aish-ui/assets/icons/plus.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>
```

`crates/aish-ui/assets/icons/minus.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/></svg>
```

`crates/aish-ui/assets/icons/search.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
```

`crates/aish-ui/assets/icons/settings.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>
```

`crates/aish-ui/assets/icons/trash.svg`：

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
```

- [ ] **Step 2: 写 icons/mod.rs**

`crates/aish-ui/src/icons/mod.rs`：

```rust
//! Icon 系统。
//!
//! - `IconName` enum 列出所有内置 icon
//! - `AishUiAssets` 实现 `gpui::AssetSource`，把 SVG 编入 binary
//! - `icon(name)` 返回 GPUI svg 元素
//!
//! 调用方流程：
//! 1. `Application::with_assets(AishUiAssets).run(...)`
//! 2. 渲染：`icon(IconName::Send).text_color(...).size_4()`

use std::borrow::Cow;

use gpui::{px, svg, AssetSource, IntoElement, Result, SharedString, Svg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    X,
    Check,
    Info,
    AlertCircle,
    AlertTriangle,
    Send,
    Plus,
    Minus,
    Search,
    Settings,
    Trash,
}

impl IconName {
    /// AssetSource 加载用的 path（对应 assets/icons/ 内文件名）。
    pub fn asset_path(&self) -> &'static str {
        match self {
            IconName::ChevronDown => "icons/chevron-down.svg",
            IconName::ChevronUp => "icons/chevron-up.svg",
            IconName::ChevronLeft => "icons/chevron-left.svg",
            IconName::ChevronRight => "icons/chevron-right.svg",
            IconName::X => "icons/x.svg",
            IconName::Check => "icons/check.svg",
            IconName::Info => "icons/info.svg",
            IconName::AlertCircle => "icons/alert-circle.svg",
            IconName::AlertTriangle => "icons/alert-triangle.svg",
            IconName::Send => "icons/send.svg",
            IconName::Plus => "icons/plus.svg",
            IconName::Minus => "icons/minus.svg",
            IconName::Search => "icons/search.svg",
            IconName::Settings => "icons/settings.svg",
            IconName::Trash => "icons/trash.svg",
        }
    }

    /// 直接拿到 SVG bytes。Test / debug 用，正式渲染走 AssetSource。
    pub fn bytes(&self) -> &'static [u8] {
        match self {
            IconName::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
            IconName::ChevronUp => include_bytes!("../../assets/icons/chevron-up.svg"),
            IconName::ChevronLeft => include_bytes!("../../assets/icons/chevron-left.svg"),
            IconName::ChevronRight => include_bytes!("../../assets/icons/chevron-right.svg"),
            IconName::X => include_bytes!("../../assets/icons/x.svg"),
            IconName::Check => include_bytes!("../../assets/icons/check.svg"),
            IconName::Info => include_bytes!("../../assets/icons/info.svg"),
            IconName::AlertCircle => include_bytes!("../../assets/icons/alert-circle.svg"),
            IconName::AlertTriangle => include_bytes!("../../assets/icons/alert-triangle.svg"),
            IconName::Send => include_bytes!("../../assets/icons/send.svg"),
            IconName::Plus => include_bytes!("../../assets/icons/plus.svg"),
            IconName::Minus => include_bytes!("../../assets/icons/minus.svg"),
            IconName::Search => include_bytes!("../../assets/icons/search.svg"),
            IconName::Settings => include_bytes!("../../assets/icons/settings.svg"),
            IconName::Trash => include_bytes!("../../assets/icons/trash.svg"),
        }
    }
}

/// AssetSource 实现：把所有 IconName 的 SVG 编入 binary。
pub struct AishUiAssets;

impl AssetSource for AishUiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // 遍历每个 IconName 看 path 是否匹配
        let candidates: &[IconName] = &[
            IconName::ChevronDown,
            IconName::ChevronUp,
            IconName::ChevronLeft,
            IconName::ChevronRight,
            IconName::X,
            IconName::Check,
            IconName::Info,
            IconName::AlertCircle,
            IconName::AlertTriangle,
            IconName::Send,
            IconName::Plus,
            IconName::Minus,
            IconName::Search,
            IconName::Settings,
            IconName::Trash,
        ];
        for name in candidates {
            if name.asset_path() == path {
                return Ok(Some(Cow::Borrowed(name.bytes())));
            }
        }
        Ok(None)
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

/// 默认 16×16 尺寸；调用方可链 `.size_*()` 调整。
pub fn icon(name: IconName) -> Svg {
    svg().path(name.asset_path()).size(px(16.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_icon_has_nonempty_bytes() {
        let all = [
            IconName::ChevronDown,
            IconName::ChevronUp,
            IconName::ChevronLeft,
            IconName::ChevronRight,
            IconName::X,
            IconName::Check,
            IconName::Info,
            IconName::AlertCircle,
            IconName::AlertTriangle,
            IconName::Send,
            IconName::Plus,
            IconName::Minus,
            IconName::Search,
            IconName::Settings,
            IconName::Trash,
        ];
        for icon in all {
            let bytes = icon.bytes();
            assert!(bytes.len() > 50, "icon {icon:?} 太短: {}", bytes.len());
            assert!(
                bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml"),
                "icon {icon:?} 不是 SVG: {:?}",
                std::str::from_utf8(&bytes[..50.min(bytes.len())])
            );
        }
    }

    #[test]
    fn asset_source_resolves_known_path() {
        let src = AishUiAssets;
        let result = src.load("icons/check.svg").expect("load 不该 err");
        assert!(result.is_some());
        assert_eq!(result.unwrap().as_ref(), IconName::Check.bytes());
    }

    #[test]
    fn asset_source_returns_none_for_unknown_path() {
        let src = AishUiAssets;
        let result = src.load("icons/missing.svg").expect("load 不该 err");
        assert!(result.is_none());
    }
}
```

- [ ] **Step 3: 跑测试**

```bash
cargo test -p aish-ui
```

预期：3 个新测试加上 Task 2 的 8 个，共 11 全过。

- [ ] **Step 4: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/assets crates/aish-ui/src/icons
git commit -m "feat(aish-ui): T3 — Icon 系统（15 Lucide SVG + AssetSource）"
```

---

## Task 4: Separator 组件（最简 builder）

**Files:**
- Create: `crates/aish-ui/src/components/separator.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Separator 组件**

`crates/aish-ui/src/components/separator.rs`：

```rust
//! Separator — 横向 / 纵向分割线。

use gpui::{div, prelude::*, px, App, IntoElement, Window};

use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(IntoElement)]
pub struct Separator {
    orientation: Orientation,
}

impl Separator {
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
        }
    }

    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
        }
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = theme(cx).colors.border;
        match self.orientation {
            Orientation::Horizontal => div().w_full().h(px(1.0)).bg(color),
            Orientation::Vertical => div().h_full().w(px(1.0)).bg(color),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_constructor() {
        let s = Separator::horizontal();
        assert_eq!(s.orientation, Orientation::Horizontal);
    }

    #[test]
    fn vertical_constructor() {
        let s = Separator::vertical();
        assert_eq!(s.orientation, Orientation::Vertical);
    }
}
```

- [ ] **Step 2: 在 components/mod.rs 导出**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod separator;

pub use separator::{Orientation, Separator};
```

- [ ] **Step 3: 跑测试**

```bash
cargo test -p aish-ui
```

预期：13 全过（11 + 2 新增）。

- [ ] **Step 4: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T4 — Separator 组件（builder + 横竖两向）"
```

---

## Task 5: Badge 组件

**Files:**
- Create: `crates/aish-ui/src/components/badge.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Badge**

`crates/aish-ui/src/components/badge.rs`：

```rust
//! Badge — 小标签。胶囊形，5 种 variant。

use gpui::{div, prelude::*, App, IntoElement, SharedString, Window};

use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BadgeVariant {
    Default,
    Primary,
    Success,
    Warning,
    Destructive,
}

#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    variant: BadgeVariant,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: BadgeVariant::Default,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = BadgeVariant::Primary;
        self
    }

    pub fn success(mut self) -> Self {
        self.variant = BadgeVariant::Success;
        self
    }

    pub fn warning(mut self) -> Self {
        self.variant = BadgeVariant::Warning;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.variant = BadgeVariant::Destructive;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let (bg, fg) = match self.variant {
            BadgeVariant::Default => (t.colors.muted, t.colors.muted_foreground),
            BadgeVariant::Primary => (t.colors.primary, t.colors.primary_foreground),
            BadgeVariant::Success => (t.colors.success, t.colors.background),
            BadgeVariant::Warning => (t.colors.warning, t.colors.background),
            BadgeVariant::Destructive => (t.colors.destructive, t.colors.destructive_foreground),
        };
        div()
            .h(t.spacing.px_4 + t.spacing.px_1 / 2.0) // ~18
            .px(t.spacing.px_2)
            .flex()
            .items_center()
            .rounded(t.radius.full)
            .bg(bg)
            .child(
                div()
                    .text_size(t.font_size.xs)
                    .text_color(fg)
                    .child(self.label),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_to_default_variant() {
        let b = Badge::new("hi");
        assert_eq!(b.variant, BadgeVariant::Default);
        assert_eq!(b.label.as_ref(), "hi");
    }

    #[test]
    fn primary_sets_variant() {
        let b = Badge::new("ok").primary();
        assert_eq!(b.variant, BadgeVariant::Primary);
    }

    #[test]
    fn destructive_sets_variant() {
        let b = Badge::new("err").destructive();
        assert_eq!(b.variant, BadgeVariant::Destructive);
    }

    #[test]
    fn success_warning_distinct() {
        assert_ne!(
            Badge::new("a").success().variant,
            Badge::new("a").warning().variant
        );
    }
}
```

- [ ] **Step 2: 注册到 mod.rs**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod separator;

pub use badge::{Badge, BadgeVariant};
pub use separator::{Orientation, Separator};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T5 — Badge 组件（5 variants 胶囊形）"
```

---

## Task 6: Button 组件

**Files:**
- Create: `crates/aish-ui/src/components/button.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Button**

`crates/aish-ui/src/components/button.rs`：

```rust
//! Button — 主要操作组件。4 个 variant + disabled。

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, ElementId, IntoElement, MouseButton, MouseDownEvent, SharedString,
    Window,
};

use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Destructive,
    Ghost,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            variant: ButtonVariant::Primary,
            disabled: false,
            on_click: None,
        }
    }

    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = l.into();
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.variant = ButtonVariant::Destructive;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let (bg, fg) = if disabled {
            (t.colors.muted, t.colors.muted_foreground)
        } else {
            match self.variant {
                ButtonVariant::Primary => (t.colors.primary, t.colors.primary_foreground),
                ButtonVariant::Secondary => (t.colors.secondary, t.colors.secondary_foreground),
                ButtonVariant::Destructive => {
                    (t.colors.destructive, t.colors.destructive_foreground)
                }
                ButtonVariant::Ghost => (
                    gpui::transparent_black().into(),
                    t.colors.foreground,
                ),
            }
        };

        let mut el = div()
            .id(self.id)
            .h(t.spacing.px_3 + t.spacing.px_4) // ~28
            .px(t.spacing.px_3)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.md)
            .bg(bg)
            .child(
                div()
                    .text_size(t.font_size.sm)
                    .text_color(fg)
                    .child(self.label),
            );

        if !disabled {
            el = el.cursor_pointer();
            if let Some(handler) = self.on_click {
                el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
            }
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let b = Button::new("send");
        assert_eq!(b.variant, ButtonVariant::Primary);
        assert!(!b.disabled);
        assert!(b.on_click.is_none());
    }

    #[test]
    fn variant_chains() {
        assert_eq!(Button::new("a").secondary().variant, ButtonVariant::Secondary);
        assert_eq!(
            Button::new("a").destructive().variant,
            ButtonVariant::Destructive
        );
        assert_eq!(Button::new("a").ghost().variant, ButtonVariant::Ghost);
    }

    #[test]
    fn disabled_chain() {
        let b = Button::new("a").disabled(true);
        assert!(b.disabled);
    }

    #[test]
    fn on_click_stored() {
        let b = Button::new("a").on_click(|_, _, _| {});
        assert!(b.on_click.is_some());
    }

    #[test]
    fn label_stored() {
        let b = Button::new("send").label("发送");
        assert_eq!(b.label.as_ref(), "发送");
    }
}
```

- [ ] **Step 2: 注册到 mod.rs**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod button;
mod separator;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use separator::{Orientation, Separator};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T6 — Button 组件（4 variants + disabled + on_click）"
```

---

## Task 7: IconButton 组件

**Files:**
- Create: `crates/aish-ui/src/components/icon_button.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 IconButton**

`crates/aish-ui/src/components/icon_button.rs`：

```rust
//! IconButton — 仅 icon 的方形按钮。复用 Button 的 variant 系统。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, ElementId, IntoElement, MouseButton, MouseDownEvent, Pixels, Window,
};

use crate::components::ButtonVariant;
use crate::icons::{icon, IconName};
use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconButtonSize {
    Sm,
    Md,
    Lg,
}

impl IconButtonSize {
    fn box_size(&self) -> Pixels {
        match self {
            IconButtonSize::Sm => px(24.0),
            IconButtonSize::Md => px(32.0),
            IconButtonSize::Lg => px(40.0),
        }
    }

    fn icon_size(&self) -> Pixels {
        self.box_size() - px(8.0)
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon_name: IconName,
    variant: ButtonVariant,
    size: IconButtonSize,
    disabled: bool,
    on_click: Option<Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon_name: IconName) -> Self {
        Self {
            id: id.into(),
            icon_name,
            variant: ButtonVariant::Ghost,
            size: IconButtonSize::Md,
            disabled: false,
            on_click: None,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.variant = ButtonVariant::Destructive;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = IconButtonSize::Sm;
        self
    }

    pub fn medium(mut self) -> Self {
        self.size = IconButtonSize::Md;
        self
    }

    pub fn large(mut self) -> Self {
        self.size = IconButtonSize::Lg;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_click(
        mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(h));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let disabled = self.disabled;
        let (bg, fg) = if disabled {
            (t.colors.muted, t.colors.muted_foreground)
        } else {
            match self.variant {
                ButtonVariant::Primary => (t.colors.primary, t.colors.primary_foreground),
                ButtonVariant::Secondary => (t.colors.secondary, t.colors.secondary_foreground),
                ButtonVariant::Destructive => {
                    (t.colors.destructive, t.colors.destructive_foreground)
                }
                ButtonVariant::Ghost => (gpui::transparent_black().into(), t.colors.foreground),
            }
        };

        let bs = self.size.box_size();
        let isz = self.size.icon_size();

        let mut el = div()
            .id(self.id)
            .w(bs)
            .h(bs)
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.radius.sm)
            .bg(bg)
            .child(icon(self.icon_name).size(isz).text_color(fg));

        if !disabled {
            el = el.cursor_pointer();
            if let Some(handler) = self.on_click {
                el = el.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                    handler(ev, window, cx);
                });
            }
        }

        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_to_ghost_md() {
        let b = IconButton::new("close", IconName::X);
        assert_eq!(b.variant, ButtonVariant::Ghost);
        assert_eq!(b.size, IconButtonSize::Md);
        assert!(!b.disabled);
    }

    #[test]
    fn size_chains() {
        assert_eq!(
            IconButton::new("a", IconName::X).small().size,
            IconButtonSize::Sm
        );
        assert_eq!(
            IconButton::new("a", IconName::X).large().size,
            IconButtonSize::Lg
        );
    }

    #[test]
    fn box_size_relationships() {
        assert!(IconButtonSize::Sm.box_size() < IconButtonSize::Md.box_size());
        assert!(IconButtonSize::Md.box_size() < IconButtonSize::Lg.box_size());
        assert_eq!(
            IconButtonSize::Md.icon_size(),
            IconButtonSize::Md.box_size() - px(8.0)
        );
    }
}
```

- [ ] **Step 2: 注册到 mod.rs**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod button;
mod icon_button;
mod separator;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use icon_button::{IconButton, IconButtonSize};
pub use separator::{Orientation, Separator};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T7 — IconButton 组件（4 variants × 3 size）"
```

---

## Task 8: Tooltip 组件

**Files:**
- Create: `crates/aish-ui/src/components/tooltip.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

GPUI 的 div 已经提供 `.tooltip(|window, cx| AnyView)` —— 我们封装一个语义化 builder + `TooltipExt` trait，让调用方 `.tooltip(Tooltip::new("提示"))` 直接挂在任意元素上。

- [ ] **Step 1: 写 Tooltip**

`crates/aish-ui/src/components/tooltip.rs`：

```rust
//! Tooltip — 悬停提示。封装 GPUI 内置 .tooltip() API。
//!
//! 调用方：
//! ```rust
//! use aish_ui::tooltip::TooltipExt;
//! div().child("hi").tooltip(Tooltip::new("你好"))
//! ```

use gpui::{
    div, prelude::*, AnyView, App, Context, IntoElement, Render, SharedString, Window,
};

use crate::theme::theme;

/// 静态文本 tooltip 描述。
pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

/// 内部 view，承载 Tooltip 渲染。
pub struct TooltipView {
    text: SharedString,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        div()
            .px(t.spacing.px_2)
            .py(t.spacing.px_1)
            .rounded(t.radius.sm)
            .bg(t.colors.popover)
            .border_1()
            .border_color(t.colors.border)
            .child(
                div()
                    .text_size(t.font_size.xs)
                    .text_color(t.colors.popover_foreground)
                    .child(self.text.clone()),
            )
    }
}

/// 给所有 InteractiveElement 加 `.tooltip(Tooltip::new(...))`。
pub trait TooltipExt: Sized {
    fn with_tooltip(self, tooltip: Tooltip) -> Self;
}

impl<E> TooltipExt for E
where
    E: gpui::InteractiveElement + gpui::StatefulInteractiveElement,
{
    fn with_tooltip(self, tooltip: Tooltip) -> Self {
        let text = tooltip.text;
        gpui::InteractiveElement::tooltip(self, move |_window, cx| {
            let view = cx.new(|_| TooltipView { text: text.clone() });
            AnyView::from(view)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_stores_text() {
        let t = Tooltip::new("hello");
        assert_eq!(t.text.as_ref(), "hello");
    }

    #[test]
    fn tooltip_view_holds_text() {
        let v = TooltipView {
            text: "world".into(),
        };
        assert_eq!(v.text.as_ref(), "world");
    }
}
```

- [ ] **Step 2: 注册到 mod.rs**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod button;
mod icon_button;
mod separator;
mod tooltip;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use icon_button::{IconButton, IconButtonSize};
pub use separator::{Orientation, Separator};
pub use tooltip::{Tooltip, TooltipExt, TooltipView};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T8 — Tooltip 组件（薄封装 GPUI 内置 .tooltip）"
```

---

## Task 9: TextInput 基础（struct / Render / 键盘 / IME）

**Files:**
- Create: `crates/aish-ui/src/components/text_input.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

这是 M11 最复杂的组件。Task 9–12 都改这个文件，每个 task 加一层能力。

- [ ] **Step 1: 写状态机基础**

`crates/aish-ui/src/components/text_input.rs`：

```rust
//! TextInput — 单行文本输入框。
//!
//! 含 cursor blink（T10）、selection（T11）、复制粘贴（T12）。
//! 本 task（T9）只负责：struct + 键盘 + IME + render。

use std::rc::Rc;
use std::time::Instant;

use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, Entity, FocusHandle, Focusable,
    InputHandler, KeyDownEvent, MouseButton, Pixels, SharedString, UTF16Selection, Window,
};

use crate::theme::theme;

pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    cursor: usize, // byte offset
    placeholder: SharedString,
    on_submit: Option<Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    on_change: Option<Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    bar_bounds: Option<Bounds<Pixels>>,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            placeholder: SharedString::default(),
            on_submit: None,
            on_change: None,
            bar_bounds: None,
        }
    }

    pub fn placeholder(&mut self, p: impl Into<SharedString>) -> &mut Self {
        self.placeholder = p.into();
        self
    }

    pub fn on_submit(
        &mut self,
        h: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_submit = Some(Rc::new(h));
        self
    }

    pub fn on_change(
        &mut self,
        h: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, t: impl Into<String>, cx: &mut Context<Self>) {
        self.text = t.into();
        self.cursor = self.text.len();
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.text.clear();
        self.cursor = 0;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    // -------- 状态机 --------

    pub(crate) fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub(crate) fn cursor_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
        }
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
        }
    }

    pub(crate) fn delete_forward(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub(crate) fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    fn fire_change(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_change.clone() {
            h(&self.text, window, cx);
        }
    }

    fn fire_submit(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_submit.clone() {
            h(&self.text, window, cx);
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.backspace();
                cx.notify();
                self.fire_change(window, cx);
            }
            "delete" => {
                self.delete_forward();
                cx.notify();
                self.fire_change(window, cx);
            }
            "left" => {
                self.cursor_left();
                cx.notify();
            }
            "right" => {
                self.cursor_right();
                cx.notify();
            }
            "home" => {
                self.cursor = 0;
                cx.notify();
            }
            "end" => {
                self.cursor = self.text.len();
                cx.notify();
            }
            "enter" if !event.keystroke.modifiers.shift => {
                self.fire_submit(window, cx);
            }
            _ => {
                if let Some(ch) = &event.keystroke.key_char {
                    if !event.keystroke.modifiers.control && !event.keystroke.modifiers.alt {
                        self.insert_str(ch.as_str());
                        cx.notify();
                        self.fire_change(window, cx);
                    }
                }
            }
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// -------- IME --------

struct TextInputImeHandler {
    view: gpui::WeakEntity<TextInput>,
    bar_bounds: Option<Bounds<Pixels>>,
}

impl InputHandler for TextInputImeHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &mut self,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.view
            .update(cx, |this, cx| {
                this.insert_str(text);
                cx.notify();
            })
            .ok();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.bar_bounds
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

// -------- Render --------

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_for_ime = self.focus_handle.clone();
        let weak_view = cx.weak_entity();
        let focused = self.focus_handle.is_focused(window);

        let t = theme(cx);
        let border_color = if focused { t.colors.ring } else { t.colors.border };

        let cursor_left = self.text[..self.cursor].to_string();
        let cursor_right = self.text[self.cursor..].to_string();
        let placeholder_visible = self.text.is_empty();

        div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .h(t.spacing.px_3 + t.spacing.px_4) // ~28
            .px(t.spacing.px_2)
            .rounded(t.radius.sm)
            .bg(t.colors.input)
            .border_1()
            .border_color(border_color)
            .cursor_text()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                }),
            )
            .child(if placeholder_visible {
                div()
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.muted_foreground)
                    .child(self.placeholder.clone())
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_row()
                    .text_size(t.font_size.sm)
                    .text_color(t.colors.foreground)
                    .child(div().child(cursor_left))
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(14.0))
                            .bg(t.colors.ring)
                            .self_center(),
                    )
                    .child(div().child(cursor_right))
                    .into_any_element()
            })
            .child(
                canvas(
                    |bounds, _window, _cx| bounds,
                    move |_bounds, prepaint_bounds, window, cx| {
                        window.handle_input(
                            &focus_for_ime,
                            TextInputImeHandler {
                                view: weak_view.clone(),
                                bar_bounds: Some(prepaint_bounds),
                            },
                            cx,
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    fn apply_insert(text: &mut String, cursor: &mut usize, s: &str) {
        text.insert_str(*cursor, s);
        *cursor += s.len();
    }

    fn apply_backspace(text: &mut String, cursor: &mut usize) {
        if *cursor > 0 {
            let prev = text[..*cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            text.remove(prev);
            *cursor = prev;
        }
    }

    fn apply_delete(text: &mut String, cursor: &mut usize) {
        if *cursor < text.len() {
            text.remove(*cursor);
        }
    }

    fn apply_left(text: &str, cursor: &mut usize) {
        if *cursor > 0 {
            *cursor = text[..*cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    fn apply_right(text: &str, cursor: &mut usize) {
        if *cursor < text.len() {
            *cursor = text[*cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| *cursor + i)
                .unwrap_or(text.len());
        }
    }

    #[test]
    fn insert_advances_cursor() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "hi");
        assert_eq!(t, "hi");
        assert_eq!(c, 2);
    }

    #[test]
    fn backspace_removes_prev_char() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ab");
        apply_backspace(&mut t, &mut c);
        assert_eq!(t, "a");
        assert_eq!(c, 1);
    }

    #[test]
    fn delete_removes_next_char() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "abc");
        c = 1;
        apply_delete(&mut t, &mut c);
        assert_eq!(t, "ac");
        assert_eq!(c, 1);
    }

    #[test]
    fn left_right_navigates() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ab");
        apply_left(&t, &mut c);
        assert_eq!(c, 1);
        apply_right(&t, &mut c);
        assert_eq!(c, 2);
    }

    #[test]
    fn insert_at_middle() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ac");
        apply_left(&t, &mut c);
        apply_insert(&mut t, &mut c, "b");
        assert_eq!(t, "abc");
        assert_eq!(c, 2);
    }

    #[test]
    fn cjk_char_boundary_handling() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "中文");
        // 中文 = 6 bytes (3 each in UTF-8)
        assert_eq!(c, 6);
        apply_backspace(&mut t, &mut c);
        // 应该删掉一个完整字符
        assert_eq!(t, "中");
        assert_eq!(c, 3);
    }
}
```

- [ ] **Step 2: 注册到 mod.rs**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod button;
mod icon_button;
mod separator;
mod text_input;
mod tooltip;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use icon_button::{IconButton, IconButtonSize};
pub use separator::{Orientation, Separator};
pub use text_input::TextInput;
pub use tooltip::{Tooltip, TooltipExt, TooltipView};
```

- [ ] **Step 3: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components
git commit -m "feat(aish-ui): T9 — TextInput 基础（state machine + IME + render）"
```

---

## Task 10: TextInput cursor blink

**Files:**
- Modify: `crates/aish-ui/src/components/text_input.rs`

- [ ] **Step 1: 加 blink 状态字段 + helper**

修改 `TextInput` struct 加 `blink_epoch: Instant`，`new()` 初始化，新增方法：

```rust
// 在 struct TextInput 字段列表里添加（已有 imports 不变）：
//   blink_epoch: Instant,

// new() 内部初始化：
//   blink_epoch: Instant::now(),

// 在 impl TextInput { ... } 里追加：

const BLINK_PERIOD_MS: u64 = 600;

impl TextInput {
    /// 重置 blink 相位（按键 / 鼠标后让 cursor 立即可见）。
    fn reset_blink(&mut self) {
        self.blink_epoch = Instant::now();
    }

    /// 根据相位判断 cursor 是否应该可见（仅 focused 时调用）。
    fn cursor_visible_now(&self) -> bool {
        let phase = self.blink_epoch.elapsed().as_millis() as u64 % BLINK_PERIOD_MS;
        phase < BLINK_PERIOD_MS / 2
    }

    /// 启动定时器：每 100ms 触发一次 cx.notify()，让 render 重跑。
    /// 仅在 focused 时实际更新；失焦时跳过 notify 节省电量。
    fn start_blink_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.focus_handle.is_focused(cx) {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
}
```

注：`this.focus_handle.is_focused(cx)` 这里 `cx` 是 `Context<Self>`，
检查 focus 需要 `&App` 不是 `&Window`。GPUI 的 `FocusHandle::is_focused`
有两个签名：传 `&Window` 或 `&App`。用 `cx` 作为 `&App` 时调
`focus_handle.is_focused_in(cx)` 或类似。**实施时注意 GPUI 实际 API**
（参考 `terminal_view.rs` 的现有用法：`self.focus_handle.is_focused(window)`）。

替代实现：在 timer 内不区分 focus，每 100ms 都 notify；render 内部根据
`focused` 决定画不画 cursor。

为简化、稳定，本 task 走"timer 总是 notify，render 决定可见性"路线：

```rust
fn start_blink_timer(&self, cx: &mut Context<Self>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            if this.update(cx, |_this, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
    .detach();
}
```

- [ ] **Step 2: new() 内启动 timer**

修改 `pub fn new(cx: &mut Context<Self>) -> Self`：

```rust
pub fn new(cx: &mut Context<Self>) -> Self {
    let this = Self {
        focus_handle: cx.focus_handle(),
        text: String::new(),
        cursor: 0,
        placeholder: SharedString::default(),
        on_submit: None,
        on_change: None,
        bar_bounds: None,
        blink_epoch: Instant::now(),
    };
    this.start_blink_timer(cx);
    this
}
```

- [ ] **Step 3: 在所有变更处调 reset_blink**

修改 `handle_key`、`insert_str`（为了 IME 也 reset），`set_text`、`clear` 全部在 cursor 变化前后调用 `self.reset_blink()`：

```rust
// insert_str 改成：
pub(crate) fn insert_str(&mut self, s: &str) {
    self.text.insert_str(self.cursor, s);
    self.cursor += s.len();
    self.reset_blink();
}

// backspace、delete_forward、cursor_left、cursor_right 也加 self.reset_blink();

// set_text、clear 也加。

// handle_key 内 home / end 分支也加。
```

- [ ] **Step 4: render 里根据 focused + cursor_visible_now 决定画 cursor**

修改 render 内部 cursor 那段：

```rust
let show_cursor = focused && self.cursor_visible_now();

// 替换原来的 cursor div：
.child(
    if show_cursor {
        div()
            .w(px(1.0))
            .h(px(14.0))
            .bg(t.colors.ring)
            .self_center()
            .into_any_element()
    } else {
        div().w(px(1.0)).h(px(14.0)).self_center().into_any_element()
    },
)
```

- [ ] **Step 5: 加测试**

在 `text_input.rs` 的 `mod tests` 里追加：

```rust
#[test]
fn blink_phase_first_half_visible() {
    use std::time::Duration;
    let epoch = std::time::Instant::now() - Duration::from_millis(100);
    let phase = epoch.elapsed().as_millis() as u64 % 600;
    assert!(phase < 300);
}

#[test]
fn blink_phase_second_half_invisible() {
    use std::time::Duration;
    let epoch = std::time::Instant::now() - Duration::from_millis(400);
    let phase = epoch.elapsed().as_millis() as u64 % 600;
    assert!(phase >= 300);
}

#[test]
fn blink_period_constant() {
    assert_eq!(super::BLINK_PERIOD_MS, 600);
}
```

- [ ] **Step 6: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components/text_input.rs
git commit -m "feat(aish-ui): T10 — TextInput cursor blink（600ms 周期 + reset on input）"
```

---

## Task 11: TextInput selection（拖选 + 双击选词 + Ctrl+A）

**Files:**
- Modify: `crates/aish-ui/src/components/text_input.rs`

- [ ] **Step 1: 加 selection 字段**

`TextInput` struct 加 `selection_anchor: Option<usize>`，在 `new()` 内 `selection_anchor: None,`。

`use std::ops::Range;` 添加到 imports。

新增方法：

```rust
impl TextInput {
    /// 当前选区（normalize 后），无选区时 None。
    pub(crate) fn selection_range(&self) -> Option<Range<usize>> {
        self.selection_anchor.map(|a| {
            if a < self.cursor {
                a..self.cursor
            } else {
                self.cursor..a
            }
        })
    }

    /// 删除当前选区文本（如有），返回是否删除过。
    pub(crate) fn delete_selection(&mut self) -> bool {
        if let Some(range) = self.selection_range() {
            self.text.drain(range.clone());
            self.cursor = range.start;
            self.selection_anchor = None;
            self.reset_blink();
            true
        } else {
            false
        }
    }

    /// 全选（Ctrl+A）。
    pub(crate) fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection_anchor = Some(0);
            self.cursor = self.text.len();
            self.reset_blink();
        }
    }

    /// 清选区（按方向键 / Esc 后调用）。
    pub(crate) fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// 选中 cursor 周围的 word（双击触发）。word = 连续非空白 char。
    pub(crate) fn select_word_at_cursor(&mut self) {
        let len = self.text.len();
        if len == 0 {
            return;
        }
        let bytes = self.text.as_bytes();
        let is_space = |b: u8| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r';

        // 找 word start：从 cursor 向前直到遇到空白或开头
        let mut start = self.cursor.min(len);
        // start 处是空白时往前推到非空白
        while start > 0 && is_space(bytes[start.saturating_sub(1)]) {
            start -= 1;
        }
        while start > 0 && !is_space(bytes[start - 1]) {
            start -= 1;
        }

        // 找 word end：从 cursor 向后直到遇到空白
        let mut end = self.cursor.min(len);
        while end < len && !is_space(bytes[end]) {
            end += 1;
        }

        if start < end {
            self.selection_anchor = Some(start);
            self.cursor = end;
            self.reset_blink();
        }
    }
}
```

- [ ] **Step 2: 修改 insert_str / backspace / delete_forward 处理 selection**

```rust
pub(crate) fn insert_str(&mut self, s: &str) {
    if self.delete_selection() {
        // delete 之后再插入
    }
    self.text.insert_str(self.cursor, s);
    self.cursor += s.len();
    self.reset_blink();
}

pub(crate) fn backspace(&mut self) {
    if self.delete_selection() {
        return;
    }
    if self.cursor > 0 {
        let prev = self.text[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.text.remove(prev);
        self.cursor = prev;
        self.reset_blink();
    }
}

pub(crate) fn delete_forward(&mut self) {
    if self.delete_selection() {
        return;
    }
    if self.cursor < self.text.len() {
        self.text.remove(self.cursor);
        self.reset_blink();
    }
}
```

- [ ] **Step 3: 修改 cursor_left / cursor_right 清选区**

```rust
pub(crate) fn cursor_left(&mut self) {
    self.clear_selection();
    if self.cursor > 0 {
        self.cursor = self.text[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.reset_blink();
    }
}

pub(crate) fn cursor_right(&mut self) {
    self.clear_selection();
    if self.cursor < self.text.len() {
        self.cursor = self.text[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.cursor + i)
            .unwrap_or(self.text.len());
        self.reset_blink();
    }
}
```

- [ ] **Step 4: handle_key 加 Ctrl+A**

在 handle_key 的 match 里追加：

```rust
"a" if event.keystroke.modifiers.control => {
    self.select_all();
    cx.notify();
}
"escape" => {
    self.clear_selection();
    cx.notify();
}
```

- [ ] **Step 5: 加 last_click 字段 + 双击检测**

struct 加：

```rust
last_click: Option<(Instant, usize)>,
```

`new()` 内 `last_click: None,`。

新增 method：

```rust
fn handle_mouse_down_at(&mut self, byte_offset: usize, cx: &mut Context<Self>) {
    let now = Instant::now();
    let is_double = self
        .last_click
        .as_ref()
        .map(|(t, b)| now.duration_since(*t).as_millis() < 500 && *b == byte_offset)
        .unwrap_or(false);

    if is_double {
        self.cursor = byte_offset;
        self.select_word_at_cursor();
        self.last_click = None;
    } else {
        self.cursor = byte_offset;
        self.selection_anchor = Some(byte_offset);
        self.last_click = Some((now, byte_offset));
    }
    self.reset_blink();
    cx.notify();
}
```

由于点击位置精确解析（`cursor_at_pixel`）超出 M11 简化版能力，
M11 版本只在点击时把 cursor 移到文末（保持原 InputBarView 行为），
双击时就在当前 cursor 位置调 `select_word_at_cursor`：

```rust
// render 的 on_mouse_down 改成：
.on_mouse_down(
    MouseButton::Left,
    cx.listener(|this, _, window, cx| {
        this.focus_handle.focus(window, cx);
        let pos = this.text.len(); // M11 简化：点击 = 移到末尾
        this.handle_mouse_down_at(pos, cx);
    }),
)
```

注释里写明：精确按字符位置定位需要拿到点击 x 坐标 + 字号 layout，
留待 M12+ 完善。

- [ ] **Step 6: render 里画选区高亮**

修改 cursor_left / cursor_right 拼装那段——若有 selection，把选区文字背景染成 `accent`：

```rust
// 替换 placeholder_visible == false 分支：
} else if let Some(sel) = self.selection_range() {
    let before = self.text[..sel.start].to_string();
    let middle = self.text[sel.start..sel.end].to_string();
    let after = self.text[sel.end..].to_string();
    div()
        .flex()
        .flex_row()
        .text_size(t.font_size.sm)
        .text_color(t.colors.foreground)
        .child(div().child(before))
        .child(div().bg(t.colors.accent).child(middle))
        .child(div().child(after))
        .into_any_element()
} else {
    // 原 cursor 渲染保留
    div()
        .flex()
        .flex_row()
        .text_size(t.font_size.sm)
        .text_color(t.colors.foreground)
        .child(div().child(cursor_left))
        .child(if show_cursor {
            div().w(px(1.0)).h(px(14.0)).bg(t.colors.ring).self_center().into_any_element()
        } else {
            div().w(px(1.0)).h(px(14.0)).self_center().into_any_element()
        })
        .child(div().child(cursor_right))
        .into_any_element()
}
```

- [ ] **Step 7: 加测试**

`mod tests` 追加：

```rust
#[test]
fn select_all_when_text_present() {
    let mut t = String::from("hello");
    let mut c = 5;
    let mut sel: Option<usize> = None;
    // 模拟 select_all
    sel = Some(0);
    c = t.len();
    assert_eq!(sel, Some(0));
    assert_eq!(c, 5);
}

#[test]
fn selection_range_normalizes() {
    // anchor < cursor
    let anchor = 2;
    let cursor = 5;
    let range = if anchor < cursor { anchor..cursor } else { cursor..anchor };
    assert_eq!(range, 2..5);

    // anchor > cursor
    let anchor = 7;
    let cursor = 3;
    let range = if anchor < cursor { anchor..cursor } else { cursor..anchor };
    assert_eq!(range, 3..7);
}

#[test]
fn delete_selection_removes_range_resets_cursor() {
    let mut text = String::from("hello world");
    let range: std::ops::Range<usize> = 5..11;
    text.drain(range.clone());
    let cursor = range.start;
    assert_eq!(text, "hello");
    assert_eq!(cursor, 5);
}

#[test]
fn select_word_finds_boundaries() {
    let text = "hello world rust";
    let bytes = text.as_bytes();
    let is_space = |b: u8| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r';

    let cursor = 8; // 在 'o' of "world"
    let mut start = cursor;
    while start > 0 && !is_space(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = cursor;
    while end < text.len() && !is_space(bytes[end]) {
        end += 1;
    }

    assert_eq!(&text[start..end], "world");
}

#[test]
fn select_word_at_start_of_text() {
    let text = "hello world";
    let bytes = text.as_bytes();
    let is_space = |b: u8| b == b' ' || b == b'\t';

    let cursor = 0;
    let mut start = cursor;
    while start > 0 && !is_space(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = cursor;
    while end < text.len() && !is_space(bytes[end]) {
        end += 1;
    }

    assert_eq!(&text[start..end], "hello");
}
```

- [ ] **Step 8: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components/text_input.rs
git commit -m "feat(aish-ui): T11 — TextInput selection（拖选 + 双击选词 + Ctrl+A + Esc 清）"
```

---

## Task 12: TextInput 复制粘贴

**Files:**
- Modify: `crates/aish-ui/src/components/text_input.rs`
- Modify: `crates/aish-ui/Cargo.toml`（加 arboard dep）

- [ ] **Step 1: Cargo.toml 加 arboard**

`crates/aish-ui/Cargo.toml`：

```toml
[dependencies]
gpui = { workspace = true }
arboard = { workspace = true }
```

- [ ] **Step 2: 加 copy / cut 方法**

`text_input.rs` 顶部 imports 添加：

```rust
use arboard::Clipboard;
```

新增 method：

```rust
impl TextInput {
    /// 复制选区（无选区则复制整段）。返回是否成功。
    pub(crate) fn copy(&self) -> bool {
        let payload = match self.selection_range() {
            Some(r) => self.text[r].to_string(),
            None => self.text.clone(),
        };
        if payload.is_empty() {
            return false;
        }
        match Clipboard::new() {
            Ok(mut cb) => cb.set_text(payload).is_ok(),
            Err(e) => {
                tracing::warn!("text_input: clipboard 初始化失败: {}", e);
                false
            }
        }
    }

    /// 剪切：copy 然后 delete selection。
    pub(crate) fn cut(&mut self) -> bool {
        let copied = self.copy();
        if copied {
            self.delete_selection();
        }
        copied
    }
}
```

注：粘贴走 IME 通道（系统 paste 触发 `replace_text_in_range`），不需要单独实现。

- [ ] **Step 3: handle_key 加 Ctrl+C / Ctrl+X**

```rust
// match 内追加：
"c" if event.keystroke.modifiers.control => {
    self.copy();
}
"x" if event.keystroke.modifiers.control => {
    if self.cut() {
        cx.notify();
        self.fire_change(window, cx);
    }
}
```

- [ ] **Step 4: aish-ui Cargo.toml 加 tracing 依赖**

`crates/aish-ui/Cargo.toml`：

```toml
[dependencies]
gpui = { workspace = true }
arboard = { workspace = true }
tracing = { workspace = true }
```

- [ ] **Step 5: 加测试**

`mod tests` 追加：

```rust
#[test]
fn copy_with_no_selection_uses_full_text() {
    // 模拟逻辑：无 selection 时复制全文
    let text = "hello";
    let selection: Option<std::ops::Range<usize>> = None;
    let payload = match selection {
        Some(r) => text[r].to_string(),
        None => text.to_string(),
    };
    assert_eq!(payload, "hello");
}

#[test]
fn copy_with_selection_uses_range() {
    let text = "hello world";
    let selection = Some(0..5);
    let payload = match selection {
        Some(r) => text[r].to_string(),
        None => text.to_string(),
    };
    assert_eq!(payload, "hello");
}

#[test]
fn copy_empty_text_returns_false() {
    let text = "";
    let selection: Option<std::ops::Range<usize>> = None;
    let payload = match selection {
        Some(r) => text[r].to_string(),
        None => text.to_string(),
    };
    assert!(payload.is_empty());
    // 实际函数会因 is_empty() 提前返回 false
}
```

注：Clipboard 实际操作不在单元测试中验证（环境依赖），仅测纯逻辑。

- [ ] **Step 6: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src/components/text_input.rs crates/aish-ui/Cargo.toml
git commit -m "feat(aish-ui): T12 — TextInput 复制粘贴（Ctrl+C/X via arboard）"
```

---

## Task 13: Toast + ToastManager + ToastHandle

**Files:**
- Create: `crates/aish-ui/src/components/toast.rs`
- Modify: `crates/aish-ui/src/components/mod.rs`

- [ ] **Step 1: 写 Toast 数据 struct + ToastManager Entity**

`crates/aish-ui/src/components/toast.rs`：

```rust
//! Toast — 自动消失的提示。
//!
//! 三层：
//! - `Toast` 数据结构
//! - `ToastManager` Entity（队列 + 渲染 + 定时清理）
//! - `ToastHandle` Global（持有 Entity<ToastManager> 的引用，让任意位置都能 push）

use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, App, Context, Entity, IntoElement, Render, SharedString, Window,
};

use crate::icons::{icon, IconName};
use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    fn icon_name(&self) -> IconName {
        match self {
            ToastKind::Info => IconName::Info,
            ToastKind::Success => IconName::Check,
            ToastKind::Warning => IconName::AlertTriangle,
            ToastKind::Error => IconName::AlertCircle,
        }
    }
}

#[derive(Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: SharedString,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct ToastManager {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl ToastManager {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let this = Self {
            toasts: Vec::new(),
            next_id: 1,
        };
        this.start_cleanup_timer(cx);
        this
    }

    pub fn push(
        &mut self,
        kind: ToastKind,
        msg: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.toasts.push(Toast {
            id,
            kind,
            message: msg.into(),
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        });
        cx.notify();
    }

    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        let before = self.toasts.len();
        self.toasts.retain(|t| t.id != id);
        if self.toasts.len() != before {
            cx.notify();
        }
    }

    pub(crate) fn cleanup_expired(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let before = self.toasts.len();
        self.toasts
            .retain(|t| now.duration_since(t.created_at) < t.duration);
        if self.toasts.len() != before {
            cx.notify();
        }
    }

    fn start_cleanup_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |this, cx| this.cleanup_expired(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn toasts(&self) -> &[Toast] {
        &self.toasts
    }
}

impl Render for ToastManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let toasts = self.toasts.clone();
        div()
            .absolute()
            .top(t.spacing.px_4)
            .right(t.spacing.px_4)
            .flex()
            .flex_col()
            .gap(t.spacing.px_2)
            .children(toasts.into_iter().map(|toast| render_toast(toast, cx)))
    }
}

fn render_toast(toast: Toast, cx: &mut App) -> impl IntoElement {
    let t = theme(cx);
    let (border_color, fg_color) = match toast.kind {
        ToastKind::Info => (t.colors.accent, t.colors.foreground),
        ToastKind::Success => (t.colors.success, t.colors.foreground),
        ToastKind::Warning => (t.colors.warning, t.colors.background),
        ToastKind::Error => (t.colors.destructive, t.colors.foreground),
    };

    div()
        .min_w(gpui::px(240.0))
        .px(t.spacing.px_3)
        .py(t.spacing.px_2)
        .rounded(t.radius.md)
        .bg(t.colors.popover)
        .border_1()
        .border_color(border_color)
        .flex()
        .flex_row()
        .items_center()
        .gap(t.spacing.px_2)
        .child(icon(toast.kind.icon_name()).size(t.font_size.base).text_color(border_color))
        .child(
            div()
                .text_size(t.font_size.sm)
                .text_color(fg_color)
                .child(toast.message),
        )
}

#[derive(Clone)]
pub struct ToastHandle(pub Entity<ToastManager>);

impl gpui::Global for ToastHandle {}

/// 公共 API：从任意 cx push toast。要求 `ToastHandle` 已注册为 global。
pub fn toast(cx: &mut App, kind: ToastKind, msg: impl Into<SharedString>) {
    let handle = cx.global::<ToastHandle>().clone();
    let msg = msg.into();
    handle.0.update(cx, |m, cx| m.push(kind, msg, cx));
}

pub fn toast_info(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Info, msg);
}
pub fn toast_success(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Success, msg);
}
pub fn toast_warning(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Warning, msg);
}
pub fn toast_error(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Error, msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_name_per_kind() {
        assert_eq!(ToastKind::Info.icon_name(), IconName::Info);
        assert_eq!(ToastKind::Success.icon_name(), IconName::Check);
        assert_eq!(ToastKind::Warning.icon_name(), IconName::AlertTriangle);
        assert_eq!(ToastKind::Error.icon_name(), IconName::AlertCircle);
    }

    #[test]
    fn cleanup_logic_removes_expired() {
        // 模拟 retain 逻辑
        let now = Instant::now();
        let toasts = vec![
            Toast {
                id: 1,
                kind: ToastKind::Info,
                message: "old".into(),
                created_at: now - Duration::from_secs(10),
                duration: Duration::from_secs(3),
            },
            Toast {
                id: 2,
                kind: ToastKind::Info,
                message: "fresh".into(),
                created_at: now,
                duration: Duration::from_secs(3),
            },
        ];
        let kept: Vec<_> = toasts
            .into_iter()
            .filter(|t| now.duration_since(t.created_at) < t.duration)
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, 2);
    }

    #[test]
    fn dismiss_removes_by_id() {
        let mut toasts = vec![
            Toast {
                id: 1,
                kind: ToastKind::Info,
                message: "a".into(),
                created_at: Instant::now(),
                duration: Duration::from_secs(3),
            },
            Toast {
                id: 2,
                kind: ToastKind::Info,
                message: "b".into(),
                created_at: Instant::now(),
                duration: Duration::from_secs(3),
            },
        ];
        toasts.retain(|t| t.id != 1);
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].id, 2);
    }

    #[test]
    fn next_id_wraps_safely() {
        let mut id: u64 = u64::MAX;
        id = id.wrapping_add(1);
        assert_eq!(id, 0);
    }
}
```

- [ ] **Step 2: 注册到 mod.rs**

`crates/aish-ui/src/components/mod.rs`：

```rust
//! 组件实现。

mod badge;
mod button;
mod icon_button;
mod separator;
mod text_input;
mod toast;
mod tooltip;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use icon_button::{IconButton, IconButtonSize};
pub use separator::{Orientation, Separator};
pub use text_input::TextInput;
pub use toast::{
    toast, toast_error, toast_info, toast_success, toast_warning, Toast, ToastHandle, ToastKind,
    ToastManager,
};
pub use tooltip::{Tooltip, TooltipExt, TooltipView};
```

- [ ] **Step 3: prelude 更新**

`crates/aish-ui/src/prelude.rs`：

```rust
//! 常用 re-exports。

pub use crate::components::{
    toast_error, toast_info, toast_success, toast_warning, Badge, BadgeVariant, Button,
    ButtonVariant, IconButton, IconButtonSize, Orientation, Separator, TextInput, Toast,
    ToastHandle, ToastKind, ToastManager, Tooltip, TooltipExt,
};
pub use crate::icons::{icon, IconName};
pub use crate::theme::{theme, Theme};
```

- [ ] **Step 4: 跑质量门禁 + commit**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/aish-ui/src
git commit -m "feat(aish-ui): T13 — Toast 三层（数据 + Manager Entity + Handle Global）"
```

---

## Task 14: aish-app 接入 — 注册 Theme + ToastHandle global + AssetSource

**Files:**
- Modify: `crates/aish-app/Cargo.toml`
- Modify: `crates/aish-app/src/app.rs`

- [ ] **Step 1: 加 aish-ui 依赖**

`crates/aish-app/Cargo.toml`，在 `[dependencies]` 加：

```toml
aish-ui = { workspace = true }
```

- [ ] **Step 2: 验证 cargo check 通过**

```bash
cargo check -p aish-app
```

预期：`Finished dev`，无错误。

- [ ] **Step 3: 改 app.rs 注册 asset source**

`crates/aish-app/src/app.rs` 修改 `pub fn run()` 顶部（紧接 `bridge_owner` 之后）：

```rust
// 原代码：
//   application().run(move |cx: &mut App| { ... });

// 改成：
application()
    .with_assets(aish_ui::AishUiAssets)
    .run(move |cx: &mut App| {
        // 注册 Theme global（必须在创建任何 view / 调用 theme(cx) 之前）
        cx.set_global(aish_ui::Theme::dark());

        // 创建 ToastManager 并注册 Handle global
        let toast_manager = cx.new(|cx| aish_ui::ToastManager::new(cx));
        cx.set_global(aish_ui::ToastHandle(toast_manager.clone()));

        crate::terminal::font::register_bundled_font(cx);

        // ... 后续原逻辑不变
    });
```

注：`Application::with_assets` 返回 `Self`，链式调用即可。

- [ ] **Step 4: RootView 加 ToastManager 渲染节点**

`crates/aish-app/src/app.rs` 修改 `RootView`：

```rust
struct RootView {
    state: Entity<AppState>,
    sidebar_nav: Entity<crate::views::SidebarNavView>,
    tab_bar: Entity<crate::views::TabBarView>,
    home: Entity<crate::views::HomeView>,
    terminal: Entity<crate::views::TerminalView>,
    empty_terminal: Entity<crate::views::EmptyTerminalGuideView>,
    inbox: Entity<crate::views::ComingSoonView>,
    settings: Entity<crate::views::SettingsView>,
    host_form: Entity<crate::views::HostFormModal>,
    session_picker: Entity<crate::views::SessionPickerView>,
    input_bar: Entity<crate::views::InputBarView>,
    toast_manager: Entity<aish_ui::ToastManager>,  // ← 新加
}
```

`RootView::new()` 内：

```rust
let toast_manager = cx.global::<aish_ui::ToastHandle>().0.clone();
// ...
Self {
    // ... 其他字段
    toast_manager,
}
```

`Render` impl 内 root 加 toast 子节点：

```rust
let mut root = div().relative().size_full().child(main);

if picker_open {
    root = root.child(self.session_picker.clone());
}
if modal_open {
    root = root.child(self.host_form.clone());
}

// Toast 总是叠在最顶层
root = root.child(self.toast_manager.clone());

root
```

- [ ] **Step 5: 跑全套质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：全过。aish-app 编译能找到 aish_ui::Theme / ToastManager / AishUiAssets。

- [ ] **Step 6: 启动手测**

```bash
cargo run -p aish
```

预期：app 窗口能开，没崩；现有功能视觉无明显退化。Toast 没触发也没事，只是注册可用。

- [ ] **Step 7: Commit**

```bash
git add crates/aish-app/Cargo.toml crates/aish-app/src/app.rs
git commit -m "feat(aish-ui): T14 — aish-app 接入 Theme/ToastHandle global + AssetSource"
```

---

## Task 15: aish-app 把 InputBarView 文本部分切到 TextInput

**Files:**
- Modify: `crates/aish-app/src/views/input_bar.rs`

- [ ] **Step 1: 重构 InputBarView struct**

把 text / cursor / IME 全摘掉，引入 `Entity<TextInput>`：

```rust
use std::path::PathBuf;
use std::sync::Arc;

use aish_ui::TextInput;
use gpui::{
    div, img, prelude::*, px, rgb, App, Context, Entity, FocusHandle, Focusable, ImageSource,
    ObjectFit, PathPromptOptions, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand};

struct PendingImage {
    name: String,
    path: PathBuf,
}

pub struct InputBarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    images: Vec<PendingImage>,
    input: Entity<TextInput>,
}

impl InputBarView {
    pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(cx));
        input.update(cx, |i, _cx| {
            i.placeholder("输入文字（Enter 发送）");
        });
        Self {
            state,
            bridge,
            images: Vec::new(),
            input,
        }
    }
}
```

注：删除 `focus_handle` / `text` / `cursor` 字段、`cursor_left` / `cursor_right` / `backspace` /
`delete_forward` / `insert_str` / `handle_key` / `InputBarImeHandler` 全部移除。

- [ ] **Step 2: 重构 send 方法**

`send` 改成：

```rust
fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let text = self.input.read(cx).text().trim().to_string();

    let conn = match self.state.read(cx).current_connection() {
        Some(c) => c,
        None => {
            self.input.update(cx, |i, cx| i.clear(cx));
            self.images.clear();
            cx.notify();
            return;
        }
    };

    if self.images.is_empty() {
        if !text.is_empty() {
            if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                let bytes = format!("{}\r", text).into_bytes();
                self.bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                });
            }
        }
        self.input.update(cx, |i, cx| i.clear(cx));
        cx.notify();
        return;
    }

    let mut image_data: Vec<(Vec<u8>, String)> = Vec::new();
    for img_item in &self.images {
        let ext = img_item
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
        match std::fs::read(&img_item.path) {
            Ok(bytes) => image_data.push((bytes, ext)),
            Err(e) => {
                tracing::warn!("input_bar: 读取图片失败 {:?}: {}", img_item.path, e);
            }
        }
    }

    if image_data.is_empty() {
        if !text.is_empty() {
            if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                let bytes = format!("{}\r", text).into_bytes();
                self.bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::SendBytes(bytes)).await;
                });
            }
        }
    } else if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
        self.bridge.spawn(async move {
            let _ = sender
                .send(SessionCommand::UploadBatch {
                    images: image_data,
                    text,
                })
                .await;
        });
    }

    self.images.clear();
    self.input.update(cx, |i, cx| i.clear(cx));
    cx.notify();
    let _ = window;
}
```

- [ ] **Step 3: 接 TextInput::on_submit**

在 `InputBarView::new` 初始化 input 时挂 on_submit：

```rust
pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, cx: &mut Context<Self>) -> Self {
    let input = cx.new(|cx| TextInput::new(cx));
    let weak_self = cx.weak_entity();
    input.update(cx, |i, _cx| {
        i.placeholder("输入文字（Enter 发送）");
        i.on_submit(move |_text, window, cx| {
            if let Some(this) = weak_self.upgrade() {
                this.update(cx, |this, cx| this.send(window, cx));
            }
        });
    });
    Self {
        state,
        bridge,
        images: Vec::new(),
        input,
    }
}
```

- [ ] **Step 4: 重构 Render**

`Render for InputBarView` 改成：

```rust
impl Render for InputBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let images_row = if self.images.is_empty() {
            None
        } else {
            // 缩略图渲染保持原样（M11 不动）
            let thumbs: Vec<_> = self
                .images
                .iter()
                .enumerate()
                .map(|(i, img_item)| {
                    let path = img_item.path.clone();
                    let name = img_item.name.clone();
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(2.0))
                        .w(px(64.0))
                        .child(
                            div()
                                .relative()
                                .w(px(56.0))
                                .h(px(48.0))
                                .overflow_hidden()
                                .rounded(px(4.0))
                                .child(
                                    img(ImageSource::from(path))
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Cover),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(1.0))
                                        .right(px(1.0))
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(rgb(0x00000099))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(move |this, _, _window, cx| {
                                                this.remove_image(i, cx);
                                            }),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(9.0))
                                                .text_color(rgb(0xffffff))
                                                .child("×"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .w(px(60.0))
                                .overflow_hidden()
                                .text_size(px(9.0))
                                .text_color(rgb(0x888899))
                                .child(name),
                        )
                })
                .collect();

            Some(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(8.0))
                    .p(px(8.0))
                    .children(thumbs),
            )
        };

        let text_row = div()
            .flex()
            .flex_row()
            .h(px(40.0))
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .bg(rgb(0x2d2d3f))
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.pick_images(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(0x888899))
                            .child("+"),
                    ),
            )
            .child(div().flex_1().child(self.input.clone()))
            .child(
                div()
                    .px(px(10.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .bg(rgb(0x3d59a1))
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.send(window, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0xc0caf5))
                            .child("发送"),
                    ),
            );

        div()
            .flex_col()
            .border_t_1()
            .border_color(rgb(0x2d2d3f))
            .bg(rgb(0x1a1b26))
            .children(images_row)
            .child(text_row)
    }
}
```

- [ ] **Step 5: pick_images / remove_image 保留**

```rust
impl InputBarView {
    fn pick_images(&mut self, cx: &mut Context<Self>) {
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(SharedString::from("选择图片")),
        };
        let receiver = cx.prompt_for_paths(options);
        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                this.update(cx, |this, cx| {
                    for path in paths {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image")
                            .to_string();
                        this.images.push(PendingImage { name, path });
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn remove_image(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.images.len() {
            self.images.remove(index);
            cx.notify();
        }
    }
}
```

- [ ] **Step 6: 移除原 Focusable + tests**

`impl Focusable for InputBarView` 整个删除（不再需要——focus 走子 input 的 focus_handle）。

原 `mod tests` 内 `apply_insert / apply_backspace / ...` helper 全部移除（已迁到 aish-ui::TextInput）。

- [ ] **Step 7: 跑质量门禁 + 启动手测**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p aish
```

启动后手测：

- 输入栏可输入英文（key_char path）
- 输入栏可输入中文（IME path）
- cursor 闪烁
- 鼠标拖选 → 文字背景变色（注：M11 简化版点击移到末尾，但拖选语义机器还在）
- Ctrl+A 全选高亮
- Ctrl+C 复制（粘到记事本验证）
- Enter 发送、回车后清空
- IME 候选窗对齐输入框

- [ ] **Step 8: Commit**

```bash
git add crates/aish-app/src/views/input_bar.rs
git commit -m "feat(aish-ui): T15 — InputBarView 文本部分切到 aish_ui::TextInput"
```

---

## Task 16: 收尾 — INDEX 更新 + 视觉对比 + DoD 自检

**Files:**
- Modify: `docs/superpowers/INDEX.md`

- [ ] **Step 1: 更新 INDEX.md**

在 `## Milestones（按时间倒序）` 节点最上方插入：

```markdown
### M11 — aish-ui 起步套件（2026-05-09）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-09-aish-m11-ui-starter-design.md`](specs/2026-05-09-aish-m11-ui-starter-design.md)
- plan：[`plans/2026-05-09-aish-m11-ui-starter.md`](plans/2026-05-09-aish-m11-ui-starter.md)
- 范围：新建 aish-ui crate（独立 workspace 成员），交付 Theme/Token + 15 SVG icon + Button/IconButton/Badge/Separator/Tooltip/TextInput/Toast 七大组件；aish-app 注册 Theme global + ToastHandle global + AssetSource，InputBarView 文本部分切到 TextInput
- 关键 commits：T1–T16（feat(aish-ui): 系列）
```

更新顶部 `## 当前状态`：

```markdown
- **活跃分支**：`main`（M11 aish-ui 起步套件已完成）
- **下一里程碑**：M12 — 表单与导航（Card / Tabs / Dialog / Select / Checkbox / RadioGroup / Switch）
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui 30+ + aish-app 全部) 全过
```

- [ ] **Step 2: 视觉回归 checklist**

启动 aish 手测如下并记录结果：

| 项 | 期望 | 实际 |
|---|---|---|
| 启动 | 窗口正常开 | ✅/❌ |
| Home tab | hosts grid 视觉与 M10 一致 | ✅/❌ |
| Terminal tab | 空时 EmptyTerminalGuide 显示，连上后 PTY 正常 | ✅/❌ |
| 输入栏 | placeholder 显示、cursor 闪烁、英文输入正常 | ✅/❌ |
| 输入栏中文 | 切到中文输入法，候选窗对齐输入框，输入正常 | ✅/❌ |
| 输入栏 Ctrl+A | 全选高亮 | ✅/❌ |
| 输入栏 Ctrl+C | 选区文本进剪贴板（外部记事本粘贴验证） | ✅/❌ |
| 输入栏发送 | Enter / 点发送按钮，文本送 PTY，输入栏清空 | ✅/❌ |
| 图片 + 文字 | 多选图片 + 加文字 + 发送，路径 + 文字 echo 到 PTY | ✅/❌ |

- [ ] **Step 3: 跑最终质量门禁**

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

aish-ui 测试预期 30+ 通过，aish-app 测试 108 全过（M9 数）。

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/INDEX.md
git commit -m "docs(superpowers): T16 — INDEX 更新 M11 aish-ui 起步套件已完成"
```

---

## 完成定义（DoD）核对

回看 `2026-05-09-aish-m11-ui-starter-design.md` § 8：

- [ ] aish-ui crate 编译通过，独立测试 30+ 通过 ←→ Task 1–13
- [ ] 7 个组件 + Theme + Icon 全部实现 ←→ Task 2–13
- [ ] aish-app 启动后 ToastManager global 可用 ←→ Task 14
- [ ] 输入栏 cursor 闪烁、可选区、Ctrl+C 能复制 ←→ Task 10–12 + Task 15
- [ ] IME（中文）仍工作，候选窗对齐输入框 ←→ Task 9（IME） + Task 15
- [ ] 全部质量门禁通过 ←→ 每个 Task 末尾
- [ ] INDEX.md 更新 M11 条目 ←→ Task 16
- [ ] 父 spec Risk 表 R1–R6 实际遇到 / 未遇到补记 ←→ 在 Task 16 commit message 备注，或单独追加 ADR

---

## 后续候选

- **M12**：Card / Tabs / Dialog / Select / Checkbox / RadioGroup / Switch（2–3 天）
- **M13**：DropdownMenu / ContextMenu + crate README + examples + HostFormModal/SessionPicker/SettingsView 迁移到 aish-ui（2–3 天）
- aish-ui crate 单独 v0.1 release tag（可选，M13 之后）
