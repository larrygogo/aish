//! Theme / Token 系统。M11 仅实现 dark。

mod dark;
mod light;
mod tokens;
mod typography;

pub use tokens::{
    elevation_1, elevation_2, elevation_3, ColorTokens, FontSize, Radius, Spacing, Theme, ThemeKind,
};
pub use typography::{ColorRole, TypeRole, TypeStyle, Typography, TypographyExt};

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
