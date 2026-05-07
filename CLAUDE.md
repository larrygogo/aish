# aish 项目工作流（覆盖全局）

> 本文件 **覆盖** `~/.claude/CLAUDE.md` 里 `feature-list.json` / `claude-progress.md`
> 那一套全局工作流。本仓走 superpowers 流：每个里程碑独立 spec + plan，存在
> `docs/superpowers/{specs,plans}/`。

---

## 工作流（每次任务必走）

按顺序：**Brainstorm → Spec → Plan → Implement → Commit**。

| 阶段 | 输出 | 路径 |
|---|---|---|
| Brainstorm | 与人对话明确目标、约束、UX | （无文件，对话即可）|
| **Spec** | 设计文档（数据模型、架构图、Risk 表）| `docs/superpowers/specs/YYYY-MM-DD-aish-<milestone>-design.md` |
| **Plan** | 实施计划（File Structure / 编号 Task / Self-Review）| `docs/superpowers/plans/YYYY-MM-DD-aish-<milestone>.md` |
| Implement | 按 Plan 的 Task 顺序执行，每条 Task 跑质量门禁 | 代码 |
| Commit | 中文描述提交，每条独立 commit，逐个 Task 提 | git |

**简单 fix（< 50 行 / 单文件）** 可跳过 Spec，直接简短 Plan + Implement + Commit。

**重大架构变化** 必须先在 Spec 里画 ADR-style 决策记录。

`docs/superpowers/INDEX.md` 是**唯一**的 milestone 索引：当前在做什么、已完成什么、
下一步候选。每完成一个里程碑就更新它。

---

## 已废弃的工作流（不要再用）

- ❌ `feature-list.json` — 历史快照，不再维护，新需求请走 superpowers/plans
- ❌ `claude-progress.md` — 同上，索引功能转给 `docs/superpowers/INDEX.md`

两个文件**保留为历史快照**，不要删（hook 阻挡 git rm；保留也方便回看 2026-05-07
那一波快速迭代的上下文）。

---

## 质量门禁（每条 Task 完成后跑）

```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

任一失败必须修后才能 commit。

---

## Git 提交规范

- branch 命名：`<type>/<desc>-YYYYMMDD-zj`（type ∈ feat/fix/refactor/chore/docs/test/ci/build/perf/style/revert）
- commit message：中文描述
- 每条 commit 末尾加：
  ```
  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  ```
- 每个独立的逻辑变化一个 commit，不要把多个 feature 合在一起

---

## 上下文恢复（每次会话开始）

1. 读 `docs/superpowers/INDEX.md` 看最近做了啥、当前状态
2. `git log --oneline -20` 看最近 commits
3. 如果用户提的需求覆盖某个 milestone，去 `docs/superpowers/plans/` 找对应 plan 看决策记录

不再读 `claude-progress.md` / `feature-list.json`（已废弃）。

---

## 技术栈速查

- Rust stable（edition 2021）+ nightly fmt/clippy
- workspace：`aish-types` / `aish-ssh` / `aish-tmux` / `aish-sftp` / `aish-secrets` / `aish-app`
- GUI：GPUI（git dep，pin Zed main）
- 终端：alacritty_terminal（解析 + grid + scrollback）
- SSH：russh
- Tmux：raw `tmux attach`（M3-archived 的 -CC 控制模式见 `aish-tmux/src/lib.rs` 顶注释）
- 异步：tokio multi-thread runtime + GPUI bridge（mpsc）

---

## 关键架构常识（看代码前先知道）

- **键空间**：
  - `HostId`（持久化配置 + keyring 索引）
  - `ConnectionId`（运行时 SSH 连接，每个独立 actor / PTY / Term / tmux 状态）
  - `TabId`（UI 视图，指向 Default 或 Connection）
- **actor 模式**：单一 raw shell，`AttachTmux` = 在 channel 发 `tmux attach -t '<sess>'\r`
- **UI 顶层**：RootView = TabBar + body（按 current_tab.content 切 DefaultPage / Terminal+ConnectionChip）+ overlay (HostFormModal / SessionPickerView)
- **鼠标事件**：alacritty Term mode `MOUSE_MODE+SGR_MOUSE` → 转 SGR escape 给远端；否则本地 selection / scroll
- **远端 tmux 需要 `set -g mouse on`** 才能让 aish 的鼠标 click/drag/wheel 在 tmux 内生效
