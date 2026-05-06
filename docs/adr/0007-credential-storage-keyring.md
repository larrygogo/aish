# 0007. 凭证用 OS keyring 存储

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

需要存储两类敏感数据：
- API key（用于 env 注入到远端 shell）
- SSH 私钥（如果支持 inline 私钥模式）

候选：
- **OS keyring**（macOS Keychain / Windows Credential Manager / Linux Secret Service）
- **自研加密**（用 master password 加密本地文件）
- **明文 + 文件权限保护**

## Decision

用 OS keyring，通过 `keyring` crate 跨平台访问。**SSH 私钥不存内容，只存路径**（引用 `~/.ssh/id_ed25519` 等本地文件）；SSH 密码完全不存（仅"输入即用即丢"）。

## Consequences

**好处：**
- 复用 OS 安全模型（生物识别、登录态绑定等）
- 用户已经习惯这种凭证管理方式
- 跨平台统一抽象

**代价：**
- Linux Secret Service 在不同发行版可用性不一致（GNOME 默认有 keyring，最简化的桌面环境可能没有），需要 fallback 处理
- keyring crate 在某些 CI 环境无 keyring backend，测试需要 mock
