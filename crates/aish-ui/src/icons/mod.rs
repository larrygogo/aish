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

use gpui::{px, svg, AssetSource, Result, SharedString, Styled, Svg};

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
    Home,
    Terminal,
    Tmux,
    Trash,
    Pencil,
    Eye,
    EyeOff,
    // Linux 发行版 logo（来源：simpleicons.org，单色 path，由 svg
    // text_color 注入颜色；调用方在 host 卡片 avatar 用"品牌色背景 + 白色
    // logo"组合渲染）。其他识别但未列入这里的发行版（mint/manjaro 等）
    // 走 avatar::os_avatar_for 的 Letter 分支（首字母 + 品牌色）。
    DistroUbuntu,
    DistroDebian,
    DistroArch,
    DistroAlpine,
    DistroCentos,
    DistroFedora,
    DistroRedhat,
    DistroMacos,
    // M28 state design：EmptyState / ErrorState / LoadingSkeleton 默认 icon
    Inbox,
    Server,
    WifiOff,
    FileQuestion,
    Loader,
}

impl IconName {
    /// 全部 IconName 的清单。新增 variant 时**必须**追加到此数组，
    /// AssetSource::load / 各类测试都依赖这个常量遍历。
    pub const ALL: &'static [IconName] = &[
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
        IconName::Home,
        IconName::Terminal,
        IconName::Tmux,
        IconName::Trash,
        IconName::Pencil,
        IconName::Eye,
        IconName::EyeOff,
        IconName::DistroUbuntu,
        IconName::DistroDebian,
        IconName::DistroArch,
        IconName::DistroAlpine,
        IconName::DistroCentos,
        IconName::DistroFedora,
        IconName::DistroRedhat,
        IconName::DistroMacos,
        IconName::Inbox,
        IconName::Server,
        IconName::WifiOff,
        IconName::FileQuestion,
        IconName::Loader,
    ];

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
            IconName::Home => "icons/home.svg",
            IconName::Terminal => "icons/terminal.svg",
            IconName::Tmux => "icons/tmux.svg",
            IconName::Trash => "icons/trash.svg",
            IconName::Pencil => "icons/pencil.svg",
            IconName::Eye => "icons/eye.svg",
            IconName::EyeOff => "icons/eye-off.svg",
            IconName::DistroUbuntu => "icons/distros/ubuntu.svg",
            IconName::DistroDebian => "icons/distros/debian.svg",
            IconName::DistroArch => "icons/distros/arch.svg",
            IconName::DistroAlpine => "icons/distros/alpine.svg",
            IconName::DistroCentos => "icons/distros/centos.svg",
            IconName::DistroFedora => "icons/distros/fedora.svg",
            IconName::DistroRedhat => "icons/distros/redhat.svg",
            IconName::DistroMacos => "icons/distros/macos.svg",
            IconName::Inbox => "icons/inbox.svg",
            IconName::Server => "icons/server.svg",
            IconName::WifiOff => "icons/wifi-off.svg",
            IconName::FileQuestion => "icons/file-question.svg",
            IconName::Loader => "icons/loader.svg",
        }
    }

    /// 直接拿到 SVG bytes。Test / debug 用，正式渲染走 AssetSource。
    pub(crate) fn bytes(&self) -> &'static [u8] {
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
            IconName::Home => include_bytes!("../../assets/icons/home.svg"),
            IconName::Terminal => include_bytes!("../../assets/icons/terminal.svg"),
            IconName::Tmux => include_bytes!("../../assets/icons/tmux.svg"),
            IconName::Trash => include_bytes!("../../assets/icons/trash.svg"),
            IconName::Pencil => include_bytes!("../../assets/icons/pencil.svg"),
            IconName::Eye => include_bytes!("../../assets/icons/eye.svg"),
            IconName::EyeOff => include_bytes!("../../assets/icons/eye-off.svg"),
            IconName::DistroUbuntu => {
                include_bytes!("../../assets/icons/distros/ubuntu.svg")
            }
            IconName::DistroDebian => {
                include_bytes!("../../assets/icons/distros/debian.svg")
            }
            IconName::DistroArch => include_bytes!("../../assets/icons/distros/arch.svg"),
            IconName::DistroAlpine => {
                include_bytes!("../../assets/icons/distros/alpine.svg")
            }
            IconName::DistroCentos => {
                include_bytes!("../../assets/icons/distros/centos.svg")
            }
            IconName::DistroFedora => {
                include_bytes!("../../assets/icons/distros/fedora.svg")
            }
            IconName::DistroRedhat => {
                include_bytes!("../../assets/icons/distros/redhat.svg")
            }
            IconName::DistroMacos => {
                include_bytes!("../../assets/icons/distros/macos.svg")
            }
            IconName::Inbox => include_bytes!("../../assets/icons/inbox.svg"),
            IconName::Server => include_bytes!("../../assets/icons/server.svg"),
            IconName::WifiOff => include_bytes!("../../assets/icons/wifi-off.svg"),
            IconName::FileQuestion => include_bytes!("../../assets/icons/file-question.svg"),
            IconName::Loader => include_bytes!("../../assets/icons/loader.svg"),
        }
    }
}

/// AssetSource 实现：把所有 IconName 的 SVG 编入 binary。
pub struct AishUiAssets;

impl AssetSource for AishUiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for name in IconName::ALL {
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

/// 默认 16×16 尺寸；调用方可链 `.size(...)` 调整。
///
/// flex_shrink_0 防御：GPUI svg 在 flex 容器内默认 shrink=1，当父容器空间不够
/// 时会被压到 width=0（即便 .size() 已固定），触发 'can't render at a zero
/// size' 报错刷屏。固定 svg 不参与 flex 收缩，svg 尺寸严格按 .size() 渲染。
pub fn icon(name: IconName) -> Svg {
    svg().path(name.asset_path()).size(px(16.0)).flex_shrink_0()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_icon_has_nonempty_bytes() {
        for icon in IconName::ALL {
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
