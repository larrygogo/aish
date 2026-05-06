# 0002. GUI 框架选 GPUI（纯 Rust）

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

aish 是跨平台桌面应用，需要选定 GUI 框架。候选：

- **Tauri**（Rust 后端 + Web 前端）：生态最成熟，xterm.js 是终端事实标准，富文本能力强
- **GPUI**（Zed 编辑器同款）：纯 Rust，性能极强，富文本能力已被 Zed 验证
- **Iced**：纯 Rust，Elm 架构，社区活跃，但富文本能力中等
- **egui**：即时模式，做带状态的复杂 UI 不舒服
- **Slint**：DSL + Rust，社区较小

用户偏好"纯 Rust"路线，希望避免 webview 与 TS/Web 技术栈混入。

## Decision

选 **GPUI**。理由：

1. Zed 编辑器证明了 GPUI 能做高质量终端 + Markdown + diff（这正是 aish 的核心需求）
2. 纯 Rust，无 webview / 无 Node 工具链
3. Zed 是开源的，可以直接参考源码学习

## Consequences

**好处：**
- 单一二进制，启动快，资源占用低
- 与 alacritty_terminal、russh 等纯 Rust 生态无缝衔接
- 性能上限很高（GPU 渲染）

**代价：**
- 文档稀缺，需要看 Zed 源码学习，前期 2-4 周学习曲线很陡
- API 不稳定，跟着 Zed 升级
- Linux 稳定性是已知短板（Zed 自己也在改）
- 几乎没有 Zed 之外的成功案例，aish 是早期用户
