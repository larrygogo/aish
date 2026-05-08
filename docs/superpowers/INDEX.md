# aish Superpowers 索引

> 全部里程碑 plan + spec 的索引 + 当前状态 + 下一步候选。
>
> 每完成一个 milestone 更新本文件。这是 `claude-progress.md` 的替代品（后者已废弃）。

---

## 当前状态

- **活跃分支**：无（main 已合并完所有 feature；本地 main 比 origin 领先若干 commits，待推 origin）
- **下一里程碑**：未指定，等用户提需求走 brainstorm → spec → plan
- **质量门禁基线**：fmt + clippy 0 warning + test 199 全过

---

## Milestones（按时间倒序）

### M3d-resize-iter1 — 拖窗 resize 时序修复（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-tmux-resize-tweaks-design.md`](specs/2026-05-08-aish-tmux-resize-tweaks-design.md)
- plan：[`plans/2026-05-08-aish-tmux-resize-tweaks.md`](plans/2026-05-08-aish-tmux-resize-tweaks.md)
- 实际产出：debounce 100→250ms / 本地 alacritty Term resize 推迟 80ms 到 SIGWINCH 之后 / check_resize 闭包 4 段流水
- 关键 commits：`04ed0e0` `7c6bbfc`(merge)
- 决策：5 个候选薄弱点（floor 取整 / shared session / refresh-client / 时序 / debounce）只修了 #4 #5 两条确定有问题的；#1/#2/#3 实测没撞到，留观察

### M3d-ui-iter2 — 删 ConnectionChip 横条（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-remove-connection-chip-design.md`](specs/2026-05-08-aish-remove-connection-chip-design.md)
- plan：[`plans/2026-05-08-aish-remove-connection-chip.md`](plans/2026-05-08-aish-remove-connection-chip.md)
- 实际产出：删 ConnectionChipView / [SSH] 蓝胶囊并入 tab 标题 / RootView body 简化（terminal 直接占满）
- 关键 commits：`86382bf` `76553b6`(fmt) `431b0b4`(merge)
- 注：原横条上的 ▾ 折叠按钮在 UI 层暂失，恢复入口待 backlog `collapse-orphan-conn` 做完

### M3d-ui-polish — UI 整体美化（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-ui-polish-design.md`](specs/2026-05-08-aish-ui-polish-design.md)
- plan：[`plans/2026-05-08-aish-ui-polish.md`](plans/2026-05-08-aish-ui-polish.md)
- 实际产出：抽 `theme.rs` 集中色值 / 字号 / 半径 / 默认页大圆角卡片 / tab 栏 + connection chip + host form 全套 theme 应用
- 关键 commits：`24904aa` `8d5a227` `c4fc161` `0997e5e` `8097b43`

### M3c-post-cc-rework（2026-05-07）— ✅ 已完成
- spec：（无独立 spec，决策记录在 plan 内 + claude-progress.md 的"技术决策"节，已并入归档）
- plan：[`plans/2026-05-07-aish-m3c-post-cc-rework.md`](plans/2026-05-07-aish-m3c-post-cc-rework.md)
- 实际产出：connection chip / 鼠标滚轮 / tab inline 重命名 / mouse coord 修正 / SGR mouse 全事件转发
- 关键 commits：`3cebd36` `48dd5da` `679cc1f` `2fd213b` `1e3b1b5` `367d063`

### M3b — Tmux session 列表 + attach + 三栏 GUI（2026-05-07）— ⚠️ 部分作废
- spec：[`specs/2026-05-07-aish-m3b-tmux-attach-design.md`](specs/2026-05-07-aish-m3b-tmux-attach-design.md)
- plan：[`plans/2026-05-07-aish-m3b-tmux-attach.md`](plans/2026-05-07-aish-m3b-tmux-attach.md)
- 状态：原 -CC 控制模式部分已被 `ffe2cdf` 回退为 raw attach；list-sessions / GUI 三栏部分仍在使用，但三栏被 tab 系统取代

### M3a — Tmux control mode 协议层（2026-05-07）— ⚠️ M3-archived
- spec：[`specs/2026-05-07-aish-m3a-tmux-protocol-design.md`](specs/2026-05-07-aish-m3a-tmux-protocol-design.md)
- plan：[`plans/2026-05-07-aish-m3a-tmux-protocol.md`](plans/2026-05-07-aish-m3a-tmux-protocol.md)
- 状态：`aish-tmux` crate 内 controller / protocol / events / SessionTree 标 `#[allow(dead_code)]`，主路径不调用，保留待未来重启

### M2d — Auth Keyring（2026-05-07）— ✅
- spec：[`specs/2026-05-07-aish-m2d-auth-keyring-design.md`](specs/2026-05-07-aish-m2d-auth-keyring-design.md)
- plan：[`plans/2026-05-07-aish-m2d-auth-keyring.md`](plans/2026-05-07-aish-m2d-auth-keyring.md)

### M2c — Host 持久化 + GUI 增删改（2026-05-07）— ✅
- spec：[`specs/2026-05-07-aish-m2c-host-persistence-design.md`](specs/2026-05-07-aish-m2c-host-persistence-design.md)
- plan：[`plans/2026-05-07-aish-m2c-host-persistence.md`](plans/2026-05-07-aish-m2c-host-persistence.md)

### M2b1 — 终端渲染 + PTY resize（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-m2b1-terminal-rendering-design.md`](specs/2026-05-06-aish-m2b1-terminal-rendering-design.md)
- plan：[`plans/2026-05-06-aish-m2b1-terminal-rendering.md`](plans/2026-05-06-aish-m2b1-terminal-rendering.md)

### M2a — SSH 接入 + 单 PTY shell（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-m2a-ssh-bootstrap-design.md`](specs/2026-05-06-aish-m2a-ssh-bootstrap-design.md)
- plan：[`plans/2026-05-06-aish-m2a-ssh-bootstrap.md`](plans/2026-05-06-aish-m2a-ssh-bootstrap.md)

### M1 — GPUI 起步（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-m1-gpui-bootstrap-design.md`](specs/2026-05-06-aish-m1-gpui-bootstrap-design.md)
- plan：[`plans/2026-05-06-aish-m1-gpui-bootstrap.md`](plans/2026-05-06-aish-m1-gpui-bootstrap.md)

### M0 — Workspace 骨架（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-ssh-design.md`](specs/2026-05-06-aish-ssh-design.md)
- plan：[`plans/2026-05-06-aish-m0-workspace-skeleton.md`](plans/2026-05-06-aish-m0-workspace-skeleton.md)

---

## 关键决策记录（ADR）

存在 `../adr/`，按编号：

- 0001 Record architecture decisions
- 0002 GUI framework: GPUI
- 0003 Terminal: alacritty_terminal
- 0004 SSH: russh
- 0005 Tmux: control mode（**已部分回退**，见 M3c-post-cc-rework）
- 0006 Tokio / GPUI bridge
- 0007 Credential storage: keyring
- 0008 Env injection: SendEnv + export
- 0009 Attachment path isolation

后续重大决策也走 `docs/adr/00NN-<title>.md`，编号顺延。

---

## 下一步候选（M3d 起）

来自 M3c-post-cc-rework 的"下一步"+ 用户验证后新增：

| ID | 描述 | 工作量 | 优先级 |
|---|---|---|---|
| housekeeping-host-list-rs | 用 `git rm` 删占位 `views/host_list.rs` | < 5 min | 低，下次手工 |
| collapse-orphan-conn | collapse 后孤儿 connection 在默认页加"恢复"入口 | ~半天 | 中 |
| tab-reorder-keyboard | Ctrl+Shift+PageUp/Down 重排 tab | ~1 小时 | 低 |
| tab-reorder-drag | tab 拖拽（GPUI drag/drop API 调研 + 实现） | ~半天 | 低 |
| paste | Ctrl+Shift+V 粘贴 + bracketed paste mode | ~半天 | 中 |
| mouse-legacy-encoding | X10/UTF8 鼠标编码 fallback（现代默认 SGR，需求弱） | < 1 小时 | 极低 |
| mouse-on-detect | aish 启动时检测/提示远端开 `set -g mouse on` | ~1 小时 | 中 |
| detach-detect | tmux conf 注入 `set-hook -g client-detached`，aish 解析后清侧栏标记 | ~半天 | 中 |
| session-picker-meta | session picker 列表加 window 数 + 上次活跃时间 | ~1 小时 | 低 |

需要做哪条都走 brainstorm → spec → plan → implement，本表只是 backlog。

---

## 历史快照（已归档）

- `../../feature-list.json` — 2026-05-07~05-08 的 7 个 feature 的快照（不再维护）
- `../../claude-progress.md` — 同期"技术决策记录"+"会话历史摘要"快照

新里程碑请**不要**再回填这两个文件。
