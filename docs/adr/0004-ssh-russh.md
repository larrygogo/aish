# 0004. SSH 客户端用 russh

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

需要 SSH 协议实现。候选：

- **russh**：纯 Rust，async-first，活跃维护
- **ssh2**：libssh2 的 Rust binding，C 依赖，跨平台编译麻烦
- **thrussh**：russh 的前身，已不再维护
- **OpenSSH 命令行 + PTY 包装**：依赖系统装了 ssh，且无法精细控制（如 SendEnv 协议）

## Decision

用 **russh**。

## Consequences

**好处：**
- 无 C 依赖，跨平台编译简单（特别是 Windows）
- async API 与 tokio 无缝集成
- 可精细控制 SSH 协议层（自定义 channel、SendEnv、agent forward 等）

**代价：**
- 比 OpenSSH 命令行包装方案多写一些胶水代码
- 协议覆盖度偶有空白（小众算法、特殊 KEX），需关注 issues
