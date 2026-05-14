//! Light theme — M11 留 stub，M11+ 之外 milestone 实现。

use super::tokens::Theme;

// TODO(light-theme): M15 / M17 / M18 给 `ColorTokens` 加的 hover/active 字段
// （primary_hover/_active、secondary_hover/_active、destructive_hover/_active、
// accent_active、secondary_strongest）。当前 `light()` 整体是 unimplemented! stub，
// 未构造 struct literal，这些新字段也没有对应值。下个 light theme milestone 真正
// 实现 `light()` 时，需要按 light 配色手挑这些 hover/active 色（dark 那批对照参考
// 在 theme/dark.rs）。
// 注：M16（TextInput mask + cursor_at_pixel）未给 ColorTokens 加字段，与本注释无关。
impl Theme {
    /// **未实现**：M11 范围外。调用会 panic。
    pub fn light() -> Self {
        unimplemented!("Light theme not implemented in M11; see aish-ui architecture spec");
    }
}
