//! Light theme — M11 留 stub，M11+ 之外 milestone 实现。

use super::tokens::Theme;

// TODO(light-theme): M15 新加 6 个 hover/active 字段（primary_hover/_active、
// secondary_hover/_active、destructive_hover/_active）当前 = dark 同值占位，
// 下个 light theme milestone 真正手挑配色时替换。
impl Theme {
    /// **未实现**：M11 范围外。调用会 panic。
    pub fn light() -> Self {
        unimplemented!("Light theme not implemented in M11; see aish-ui architecture spec");
    }
}
