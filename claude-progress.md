# aish 进度记录

> GPUI 桌面 SSH 客户端 + tmux -CC 集成。每次提交后更新此文件。

---

## 当前状态

- **总功能数**：1
- **已完成**：1 (100%)
- **进行中**：0
- **待开始**：0
- **最后更新**：2026-05-07

---

## 最近完成（最新在前）

- [2026-05-07] feature #1: tmux -CC 客户端尺寸跟随 GPUI 窗口实时变化（commit: 9927326）

---

## 下一步任务

（暂无；等下次需求）

---

## 已知问题 / 阻塞点

- [限制] 远端 tmux server 若被多个 client 同时 attach（如另一个 SSH 终端），pane 大小会按"最小 client"裁剪，除非 server 端配置 `set-option -g aggressive-resize on`。本地 aish 单独 attach 时不受影响。

---

## 技术决策记录

### 2026-05-07：tmux 尺寸由 actor 内部状态驱动而非 AppState
**背景**：GPUI render 路径不能轻易把 host_pty_dimensions 推给 tokio actor；actor 启动时 GPUI 还没第一次 layout。
**决策**：actor 内部局部变量 `current_size: (u16, u16)` 初始 `DEFAULT_COLS/ROWS`，等第一个 `SessionCommand::Resize` 自动校准，覆盖默认占位。
**原因**：避免 spawn_session 签名增 cols/rows 参数（牵涉 register_session、bridge），且 GPUI debounce 100ms 后真实尺寸就到了，默认占位窗口期可忽略。

---

## 架构关键点（速查）

- **mode 状态机**：`ssh_actor::ActorMode::{RawShell, TmuxAttached(TmuxController)}`
- **PTY 路径**：raw shell 用 `chan.window_change` SIGWINCH；tmux -CC 用 `refresh-client -C <c>x<r>` 给 server
- **GPUI → actor 通道**：`SessionCommand` mpsc，包括 `SendBytes / Resize / QueryTmuxSessions / AttachTmux / Disconnect`
- **render 决策**：`terminal_view::term_for_render` 在 TmuxAttached 模式下优先 `last_active_pane`，fallback SessionTree first pane
- **GPUI resize debounce**：`terminal_view::check_resize` 在 canvas prepaint 的下一帧检测 bounds 变化，100ms debounce 后发 `SessionCommand::Resize`

---

## 会话历史摘要

- [2026-05-07] 初次会话：诊断 + 修复 tmux 客户端尺寸不跟随 GPUI 窗口的 bug；初始化项目级模板。
