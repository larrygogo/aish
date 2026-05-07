# aish 进度记录

> GPUI 桌面 SSH 客户端 + tmux -CC 集成。每次提交后更新此文件。

---

## 当前状态

- **总功能数**：4
- **已完成**：4 (100%)
- **进行中**：0
- **待开始**：0
- **最后更新**：2026-05-07

---

## 最近完成（最新在前）

- [2026-05-07] feature #4: tab 系统 + 卡片化默认页 + tmux session picker 弹窗（commit: 37e5895）
- [2026-05-07] feature #3: 配置与活跃连接分离，一个 host 可开多个独立连接（commit: 74b704d）
- [2026-05-07] feature #2: 放弃 tmux -CC 控制模式，改回 raw attach 让 tmux 自绘原生 UI（commit: ffe2cdf）
- [2026-05-07] feature #1: tmux -CC 客户端尺寸跟随 GPUI 窗口实时变化（commit: 9927326；后被 #2 部分回退）

---

## 下一步任务

- [ ] housekeeping：用 `git rm` 真正删除已清空的 `views/host_list.rs`（当前是占位文件）
- [ ] M3c：detach 检测（用户在 tmux 内 prefix+d 后侧栏高亮自动消失）。可选方案：tmux 启动时设 `PROMPT_COMMAND` 或 conf 加 `set-hook -g client-detached`，attach 进 raw shell 时回写特殊标记给 aish 解析。
- [ ] tab 拖拽重排序、tab 重命名（用户自定义 label）
- [ ] 终端区上方 connection chip（host label + SSH chip + 关闭/全屏按钮，参考图 7 顶部那条 chip）

---

## 已知问题 / 阻塞点

- [限制] 用户在 tmux 内 detach 后 aish 不感知，sidebar 仍高亮"已 attached"，需要点 ↻ 重新 list-sessions 才会刷新（M3c 解决）。

---

## 技术决策记录

### 2026-05-07：tab 替换 vs 新开
**背景**：点默认页 host 卡片启动连接时，是把当前 tab 内容替换成 Connection（同 id），还是新开一个 tab？
**决策**：替换当前 tab 的 content + title，保留 TabId 不变。
**理由**：① 用户的心智模型是"我在这个 tab 里点了一下，它就变成那个连接"；② 避免每点一次卡片就累积一个新 tab；③ 想多开就用 + 按钮新建一个默认页再点卡片。

### 2026-05-07：session picker 触发时机
**背景**：用户期望"连接后弹窗显示 sessions 让选"。但 list-sessions 是异步的（actor 启动后单独 exec），可能在用户已经切到别的 tab 后才返回。
**决策**：弹窗触发条件 = `sessions 非空 && current_connection() == Some(conn)`。事件到时若用户已经切走，不弹（避免打扰）；用户切回去时也不再追溯弹（再点 ↻ 刷新可手动召回）。
**理由**：① 简单；② 不打扰其他 tab 操作；③ TmuxState::Detected 仍持有结果，未来如果想做"无打扰提示"可在 connection tab 顶部加 chip。

### 2026-05-07：配置与活跃连接分离的键空间设计
**背景**：用户希望"一个保存的配置可以打开多个连接"（参考移动端 SSH 客户端的"活跃会话 + 已保存连接"两区设计）。原来 `HostId` 既是配置键又是运行时连接键，1:1 绑死。
**决策**：引入 `ConnectionId(Uuid)` 作为运行时连接的唯一标识；`HostId` 仅用于配置（持久化）和 keyring 索引。所有 per-runtime map（sessions / host_pty_term / host_pty_dimensions / tmux_state）以 ConnectionId 为键。
**理由**：① newtype 强制类型隔离，编译期防止把 host_id 当 conn_id 用；② keyring 仍按 HostId 让多连接共享密码（密码是配置属性而非连接属性）；③ 重启不持久化连接，hosts.json 兼容性零破坏。
**留作未来**：① detach 检测（M3c）让用户在 tmux 内 prefix+d 后侧栏自动取消高亮；② 连接重命名（用户自定义 label）。

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

- **键空间**：`HostId`（持久化配置 + keyring 索引）vs `ConnectionId`（运行时连接，每个独立 actor/PTY/Term/tmux 状态）vs `TabId`（UI 视图，指向 Default 或某个 ConnectionId）
- **UI 顶层**：RootView = TabBar（顶） + body（按 current_tab.content 切 DefaultPage / Terminal）+ overlay（HostFormModal / SessionPickerView）
- **新连接流程**：default page 卡片点击 → `state.open_connection(host_id)` → `state.replace_current_tab(Connection(c), label)` → `bridge.spawn_session` → 等 actor 异步 list-sessions → `pending_session_picker = Some(c)`（如果有 sessions）→ 弹 picker
- **actor 模式**：单一 raw shell 模式（M3-archived：之前的 `ActorMode::TmuxAttached` 已删）
- **PTY 链路**：GPUI `check_resize` → `SessionCommand::Resize` → `chan.window_change` SIGWINCH → 远端 shell（或 tmux client → 远端 pane shell）
- **GPUI → actor 通道**：`SessionCommand` mpsc，包括 `SendBytes / Resize / QueryTmuxSessions / AttachTmux / Disconnect`
- **AttachTmux 语义**：actor 在当前 channel 发字节 `tmux attach -t '<sess>'\r`，不切 mode、不开新 channel
- **新连接入口**：点 sidebar 的"已保存的连接"中某个 host → `state.open_connection(host_id)` 生成新 ConnectionId + 自动 label `"<host.label> #N"` + 自动选中 → `bridge.spawn_session(conn, config, ...)` 启动 actor
- **render 决策**：`terminal_view::term_for_render(state, conn)` 直接返回 `host_pty_term.get(&conn)`
- **GPUI resize debounce**：`terminal_view::check_resize` 在 canvas prepaint 的下一帧检测 bounds 变化，100ms debounce 后发 `SessionCommand::Resize`
- **侧栏高亮**：`TmuxState::Detected { sessions, attached: Option<SessionId> }`，attach 后写 attached 字段，UI 用 ●/○ marker 区分

---

## 会话历史摘要

- [2026-05-07] 初次会话：① 诊断 + 修复 tmux -CC 客户端尺寸不跟随 GPUI 窗口（feature #1，commit 9927326）；② 用户给参考图希望见到 tmux 原生绿色状态栏，反向重构放弃 -CC 改回 raw attach（feature #2，commit ffe2cdf）；③ 配置与活跃连接分离 —— 引入 ConnectionId、host_list 拆两段、× 按钮断连（feature #3，commit 74b704d）；④ tab 系统 + 卡片化默认页 + tmux session picker 弹窗（feature #4，commit 37e5895）；初始化项目级模板。
