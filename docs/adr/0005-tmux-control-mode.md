# 0005. tmux 集成用 control mode（`tmux -CC`）

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0004

## Context

aish 的核心差异化是 tmux 可视化管理：连上服务器后自动列出 session/window/pane，UI 上点击即可 attach/切换/关闭。需要选定与 tmux 通信的方式。候选：

- **轮询 `tmux list-sessions / list-windows / list-panes`**：最简单，但需要不断轮询拿最新状态，且无法在状态变化的瞬间感知
- **tmux control mode（`tmux -CC`）**：tmux 主动推事件流，单 channel 可以承载所有 pane 的 IO 与控制命令
- **自研协议**：完全不可行

## Decision

用 **control mode**。客户端在远端运行 `tmux -CC new-session -A -s aish-default`，进入 control 模式后：

- tmux 立即推送当前所有 session/window/pane 的快照
- 之后任何状态变化（新增 pane、resize、pane 死亡等）会主动推 `%xxx` 事件行
- 所有 pane 输出通过 `%output %N <bytes>` 复用同一 channel
- 客户端发送 `send-keys`、`new-window`、`kill-pane` 等命令也走同一 channel

## Consequences

**好处：**
- 实时事件流，UI 状态总是与远端一致
- 一台服务器只占一个 SSH connection（不会爆 channel）
- iTerm2 已用此方案多年，可参考其实践

**代价：**
- 解析协议有边角情况（layout string 格式、Unicode pane 名、nested tmux 等）需要踩坑
- 需要 tmux >= 2.6（control mode v2）；老版本必须降级到 raw PTY shell
