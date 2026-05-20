---
title: RemoteCapabilities 重构 — 把 host-level 远端探测能力结构化
date: 2026-05-20
status: planned
related: 2026-05-20-aish-daemonize-design.md / 2026-05-20-aish-wezterm-research-notes.md
---

# RemoteCapabilities 重构

## 范围

把 `HostConfig.os_kind` 从平铺字段重构成嵌套 `HostCapabilities` struct，
为未来添加 host-level 远端能力探测（CPU arch / shell / SFTP 支持等）提供
标准位置。同时把「append-only schema + capability flag」演进规则文档化。

## 非范围

- **不动 session-level 运行时探测**（tmux mouse / installed / version 等）。
  这些是每次连接重新探测的状态，不持久化，归 actor + state.rs 管，跟本次
  重构正交。session-level 结构化等真有第二个 session 字段触发再做
- **不引入新的探测点**（不加 CPU arch / shell / SFTP 探测）。本次只重构
  现有 os_kind，避免范围蔓延
- **不向后兼容旧 hosts.json**。aish 仍 v0.1.0，用户基数小，os_kind 重新
  探测代价低（首次连接 200ms 自动恢复）。旧 hosts.json 顶层 `os_kind`
  字段加载时被 serde 忽略，不报错

## 文件结构

```
crates/aish-types/src/lib.rs                # HostCapabilities + 修改 HostConfig
crates/aish-app/src/persistence.rs          # mk_host 测试 helper 更新
crates/aish-app/src/ssh_actor.rs            # OsDetected 事件后处理调用方
crates/aish-app/src/app.rs                  # SshEvent::OsDetected 处理调用方
docs/capability-schema-rules.md             # 🆕 schema 演进规则文档
```

## Tasks

### Task 1 — aish-types 加 HostCapabilities

- 在 `aish-types/src/lib.rs` 加：
  ```rust
  #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
  pub struct HostCapabilities {
      /// 远程系统 /etc/os-release 的 ID 字段（如 "ubuntu" / "debian" / ...
      /// macOS → "macos"，无 os-release 的 Linux → "linux"）。
      /// None = 还未探测或探测失败。
      #[serde(default)]
      pub os_kind: Option<String>,
  }
  ```
- 修改 `HostConfig`：移除 `pub os_kind: Option<String>`，加
  `#[serde(default)] pub capabilities: HostCapabilities`
- 保留 `#[serde(default)]` 确保旧 hosts.json（无 capabilities 字段）能加载
  成 `HostCapabilities::default()`

### Task 2 — 更新所有 os_kind 访问点

grep 范围内的 5 处：
- `aish-app/src/app.rs:177-195`（SshEvent::OsDetected 处理）
- `aish-app/src/persistence.rs:151,247`（mk_host 测试 helper）
- `aish-app/src/ssh_actor.rs:852`（unit test 构造）
- `aish-types/src/lib.rs:275`（host_config_roundtrip 测试）

所有 `host.os_kind` → `host.capabilities.os_kind`。

### Task 3 — 写 capability schema 演进规则文档

`docs/capability-schema-rules.md`：

- 什么是 capability：远端属性，aish 探测后用于 UI / 行为决策
- host-level vs session-level 区分原则
- 演进规则：append-only（只增字段、不删字段、不改语义）+ Option default
- 新加 capability 字段的 checklist：
  - 默认 Option/Default
  - serde default attribute
  - 文档化探测时机
  - roundtrip 测试

短文（< 100 行），明确规则，给未来人/agent 加新 capability 时有参照。

### Task 4 — 测试

- `aish-types` 加 `host_capabilities_default_empty` / `host_capabilities_roundtrip` /
  `host_config_with_capabilities_roundtrip` / `host_config_old_format_ignores_top_level_os_kind`
  四个测试
- 已有 `host_config_roundtrip` 测试更新（构造 HostCapabilities 而非 os_kind 字段）

### Task 5 — 质量门禁 + commit

- cargo +nightly fmt --all
- cargo +nightly clippy --workspace --all-targets -- -D warnings
- cargo test --workspace（基线 601 测试 + 本次新增）
- commit message: `refactor(types): 把 HostConfig.os_kind 重构成 HostCapabilities`

---

## Self-Review

- [ ] 旧 hosts.json（顶层 os_kind）加载不 panic，capabilities 为空
- [ ] 新 hosts.json（capabilities.os_kind）serde roundtrip 正确
- [ ] 所有 os_kind 引用迁移完整（5 处全改）
- [ ] avatar.rs::os_avatar_for 调用点适配（os_kind 还是 &str 输入，应该不用改）
- [ ] schema-rules 文档清晰可执行
- [ ] commit message 中文 + Co-Authored-By 行
