# 0008. env 注入：SendEnv 优先 + export 降级

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0004, 0007

## Context

aish 的核心差异化之一：从本地 keyring 集中管理 API key（如 `ANTHROPIC_API_KEY`），连接 SSH 时注入到远端 shell，让远端 AI CLI 工具直接读到，且远端文件系统**不存储**这些凭证。两条路径：

- **SSH SendEnv 协议**：客户端发 `SSH_MSG_CHANNEL_REQUEST type=env`，远端 sshd 按 `AcceptEnv` 白名单匹配后注入。最安全，但依赖 sshd 配置。
- **连接后 export**：在 PTY 通道里发 `export KEY=value`。兼容性最好，但有泄露风险（命令历史、屏幕残留）。

## Decision

**首选 SendEnv，失败时自动降级到 export**。降级时：
- 命令前加空格 + 设置 `HISTCONTROL=ignorespace`，确保不进 shell history
- value 用 `shell-escape` crate 转义
- 注入完立刻 `clear` 抹屏
- UI 提示用户"此连接 env 注入走降级模式，建议在 sshd_config 中配置 AcceptEnv"

## Consequences

**好处：**
- 大部分场景享受 SendEnv 的安全性（不进 history、不可见）
- 不强制用户改远端 sshd 配置（降级路径兜底）
- 用户对降级有明确感知，可主动加固

**代价：**
- 降级路径仍有理论上的泄露窗口（虽然已尽可能缩小）
- 需要维护两套注入路径的代码与测试
