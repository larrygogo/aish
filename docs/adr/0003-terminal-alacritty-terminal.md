# 0003. 终端实现用 alacritty_terminal + GPUI 自绘

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0002

## Context

需要在 GPUI 里渲染 vt100 / xterm-256color 终端。候选：

- **alacritty_terminal**：Alacritty 编辑器抽出的终端解析器，纯 Rust，标准实现
- **xterm.js**：行业事实标准，但需要 webview，与 0002 冲突
- **wezterm-term**：WezTerm 的实现，纯 Rust，但 API 与 WezTerm 强绑定
- **自研解析器**：vt100 / DEC private modes / OSC / sixel 等遗留协议太多，不现实

## Decision

用 **alacritty_terminal** 作为终端状态机（解析 vt100 流，维护 grid 缓冲），由 GPUI 订阅 `Term` 状态变更，自绘屏幕。

## Consequences

**好处：**
- 协议解析完全交给经过实战检验的实现
- alacritty_terminal 只持有 in-memory 状态，不直接持有 socket，易测试（喂字节流 → 比对 grid 状态）
- 与渲染层解耦，未来切渲染后端不影响协议层

**代价：**
- 需要自己实现 GPUI 与 alacritty_terminal 的胶水层（订阅变更、转换坐标系、处理键盘输入回填）
- alacritty_terminal 不带字体渲染、字符宽度计算等，需要 GPUI 层接住
