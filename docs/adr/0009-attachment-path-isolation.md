# 0009. 附件路径强制隔离到 `/tmp/aish-attach/<uuid>/`

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

aish 支持本地拖拽文件 → SFTP 上传到远端 → 路径自动注入光标位置。需要决定远端落盘路径策略。候选：

- **用户指定路径**：灵活但暴露注入风险（用户输入 `../etc/passwd` 之类）
- **用户 home 目录**：污染用户文件系统
- **强制 `/tmp/aish-attach/<connection_uuid>/`**：隔离、易清理

## Decision

强制路径格式：`/tmp/aish-attach/<connection_uuid>/<timestamp>-<sha8>-<filename>`。

- `aish-attach` 前缀避免与其他工具冲突
- 每次 SSH 连接独立 UUID 子目录，断连时由 RAII 触发清理（远端运行 `rm -rf` 该目录）
- 文件名做 sanitize（去除 `../`、控制字符）
- 客户端 API **不接受**用户指定 remote 路径

## Consequences

**好处：**
- 防止路径注入
- 易清理（连接断开自动清理整个 UUID 目录）
- 不污染用户文件系统

**代价：**
- 用户如果想"上传到指定目录"必须手动 `mv`（这是有意为之，附件是临时文件，需要永久落盘的场景应该走 SFTP 文件管理 UI，未来 Phase 2）
- `/tmp` 在某些发行版上是 tmpfs（内存），大文件可能 OOM；可配置 fallback 到 `~/.cache/aish/attach/`（远期）
