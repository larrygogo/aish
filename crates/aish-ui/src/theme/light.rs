//! Light theme — M11 留 stub，M11+ 之外 milestone 实现。

use super::tokens::Theme;

// TODO(light-theme): M15 给 `ColorTokens` 加了 6 个 hover/active 字段
// （primary_hover/_active、secondary_hover/_active、destructive_hover/_active）。
// 当前 `light()` 整体是 unimplemented! stub，未构造 struct literal，6 个新字段
// 也没有对应值。下个 light theme milestone 真正实现 `light()` 时，需要按 light
// 配色手挑这 6 个 hover/active 色（dark 那 6 个的对照参考在 theme/dark.rs）。
impl Theme {
    /// **未实现**：M11 范围外。调用会 panic。
    pub fn light() -> Self {
        unimplemented!("Light theme not implemented in M11; see aish-ui architecture spec");
    }
}
