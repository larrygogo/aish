# M43 流行主题包 + Shell 内 ANSI 联动

> 2026-05-21 用户需求：扩展主题为业界流行 10 个主题（Termius 风），主题
> 同时覆盖 shell 内 ANSI 样式。

## 决策

| 项 | 选择 |
|---|---|
| 架构 | 一体化（UI chrome + ANSI palette 一起切） |
| 现有主题 | 删除 Midnight / Warp Aurora（用户决定） |
| 主题数量 | 10 个（6 dark + 4 light）+ 默认 dark/light（共 12） |

## 主题列表

**DARK**：Moshi / Dracula / Nord / Solarized Dark / Gruvbox / Catppuccin Mocha
**LIGHT**：Solarized Light / Catppuccin Latte / GitHub Light / Rosé Pine Dawn

每主题需提供：
- 16 色 ANSI palette（terminal viewport 显示）
- 5-8 个核心 UI hex 颜色（background / foreground / accent / destructive / success / warning）
- ColorTokens 其余字段由 helper fn 从核心色派生（lighten/darken）

## 实施阶段

1. **清理**：删 dark_midnight.rs / dark_warp.rs / aurora_a/b token / midnight/warp 相关 settings option 路径
2. **架构**：ThemeKind enum 扩展（10 + Dark + Light = 12 variant）+ palette_to_theme helper
3. **数据**：10 主题 Theme::xxx() factory + 各自 ANSI palette
4. **联动**：terminal/colors.rs palette_for(kind) 按 ThemeKind 路由
5. **UI**：Settings 主题选择从 select dropdown → DARK/LIGHT 分组列表（参照用户截图样式），每项显示 5 色块预览
6. **持久化**：app_state.toml theme 字段从字符串保留兼容（"dracula" / "nord" 等）

## 风险

| 风险 | 缓解 |
|---|---|
| 派生 UI tokens 不如手调精致 | 核心色手调，secondary tokens 派生 |
| 老用户 "midnight"/"warp" 主题字段无法 load | fallback 到默认 dark |
| 工作量大 | 分阶段 commit，先架构后数据，每主题完成可单独 verify |
