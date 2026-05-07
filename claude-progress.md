# aish 进度记录

> GPUI 桌面 SSH 客户端 + tmux -CC 集成。每次提交后更新此文件。

---

## 当前状态

- **总功能数**：2
- **已完成**：2 (100%)
- **进行中**：0
- **待开始**：0
- **最后更新**：2026-05-07

---

## 最近完成（最新在前）

- [2026-05-07] feature #2: 放弃 tmux -CC 控制模式，改回 raw attach 让 tmux 自绘原生 UI（commit: ffe2cdf）
- [2026-05-07] feature #1: tmux -CC 客户端尺寸跟随 GPUI 窗口实时变化（commit: 9927326；后被 #2 部分回退）

---

## 下一步任务

- [ ] M3c：detach 检测（用户在 tmux 内 prefix+d 后 sidebar 高亮自动消失）。可选方案：tmux 启动时设 `PROMPT_COMMAND` 或在 tmux conf 里加 `set-hook -g client-detached`，让 attach 进 raw shell 时回写一行特殊标记给 aish 解析。
- [ ] 移动端参考图里的 "Tmux Sessions" 弹窗（连接后自动列 + "跳过"按钮）。当前是侧栏始终展示。

---

## 已知问题 / 阻塞点

- [限制] 用户在 tmux 内 detach 后 aish 不感知，sidebar 仍高亮"已 attached"，需要点 ↻ 重新 list-sessions 才会刷新（M3c 解决）。

---

## 技术决策记录

### 2026-05-07：放弃 tmux -CC 控制模式，回归 raw attach
**背景**：之前实现的 tmux -CC 控制模式接管 pane 渲染（aish-tmux crate 解析 `%output` / `%layout-change` 等事件 + 维护 SessionTree + per-pane alacritty Term），结果丢失了 tmux 自身的状态栏（绿条）+ 窗口列表 + pane 边框 —— 这些在 -CC 协议里被默认抑制，期望 GUI 客户端自渲染。
**决策**：actor 永远走 raw shell 单一路径；`AttachTmux` 命令改为在 raw shell channel 里发送字节 `tmux attach -t '<sess>'\r`，让远端 tmux 接管 PTY 渲染。
**原因**：① 用户期望看到熟悉的 tmux 原生 UI（含底部绿色 status line）；② 代码量减少 ~300 行，认知负担显著降低；③ 失去的"per-pane 独立渲染"能力当前并无产品价值（用户在 tmux 内用 prefix 键切换即可）。
**保留**：`aish-tmux` crate 内的 controller/protocol/events/SessionTree 标注 M3-archived 但不删，未来要做"多 pane GPUI 端独立渲染"或"detach 联动"时可重启用。

### 2026-05-07：tmux 尺寸由 actor 内部状态驱动而非 AppState（已部分作废）
**背景**：feature #1 阶段引入 `current_size` 跟踪 -CC 客户端尺寸，feature #2 删 -CC 后简化为只在初始化用 DEFAULT。
**当前状态**：actor 不再跟踪 size；`SessionCommand::Resize` 直接 `chan.window_change`，SIGWINCH 透传到远端 PTY；tmux 自身根据 PTY size 重排 pane。

---

## 架构关键点（速查）

- **actor 模式**：单一 raw shell 模式（M3-archived：之前的 `ActorMode::TmuxAttached` 已删）
- **PTY 链路**：GPUI `check_resize` → `SessionCommand::Resize` → `chan.window_change` SIGWINCH → 远端 shell（或 tmux client → 远端 pane shell）
- **GPUI → actor 通道**：`SessionCommand` mpsc，包括 `SendBytes / Resize / QueryTmuxSessions / AttachTmux / Disconnect`
- **AttachTmux 语义**：actor 在当前 channel 发字节 `tmux attach -t '<sess>'\r`，不切 mode、不开新 channel
- **render 决策**：`terminal_view::term_for_render` 直接返回 `host_pty_term.get(&host)`
- **GPUI resize debounce**：`terminal_view::check_resize` 在 canvas prepaint 的下一帧检测 bounds 变化，100ms debounce 后发 `SessionCommand::Resize`
- **侧栏高亮**：`TmuxState::Detected { sessions, attached: Option<SessionId> }`，attach 后写 attached 字段，UI 用 ●/○ marker 区分

---

## 会话历史摘要

- [2026-05-07] 初次会话：① 诊断 + 修复 tmux -CC 客户端尺寸不跟随 GPUI 窗口（feature #1，commit 9927326）；② 用户给参考图希望见到 tmux 原生绿色状态栏，反向重构放弃 -CC 改回 raw attach（feature #2，commit ffe2cdf）；初始化项目级模板。
