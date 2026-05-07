# aish M3c — Post-CC 重构与 UX 收尾（修订版）

> **修订说明**：原 M3b 的"下一步"指向 click-to-switch-pane / per-pane send-keys / detach 回 raw shell / 重连恢复 SessionTree —— 这些都依赖 tmux -CC 控制模式。本里程碑实际开工前，路线已变更：
>
> - **commit `ffe2cdf`** 放弃 -CC 模式，回退到 raw `tmux attach`，让 tmux 自身画 UI（含状态栏 + 窗口列表 + pane 边框）
> - **commit `74b704d`** 配置与运行时连接分离，引入 `ConnectionId`，一个 `HostConfig` 可派生 N 个独立连接
> - **commit `37e5895`** 顶部 tab 系统 + 卡片化默认页 + tmux session picker 弹窗（参考用户给的移动端 SSH 客户端截图）
>
> 因此 M3c 的实际目标重新定义为：**Post-CC 路径下的 UX 收尾**。原 plan 里"per-pane 操作"由 tmux 自身（prefix 键 + 鼠标）承担，aish 不再需要参与。

**Goal:** 在 raw attach + tab + 卡片页架构上补齐三条 UX 缺口（connection chip / 鼠标滚轮 / tab 重命名），并把过时的 -CC 路径在文档与代码层面彻底归档。

**Architecture:** 沿用 M3b 之后已落地的三层模型：
- **键空间**：`HostId`（持久化配置 + keyring）/ `ConnectionId`（运行时 actor + PTY + Term）/ `TabId`（UI 视图）
- **actor 模式**：单一 raw shell，`AttachTmux` 命令 = 在当前 channel 发 `tmux attach -t '<sess>'\r`
- **UI 顶层**：RootView = TabBar（顶）+ body（按 current_tab.content 切 DefaultPage 或 Terminal+ConnectionChip 组合）+ overlay（HostFormModal / SessionPickerView）

**Tech Stack:** GPUI 自绘（FocusHandle/KeyDownEvent/ScrollWheelEvent/MouseDownEvent.click_count）；alacritty `Term::scroll_display(Scroll::Delta)`；其余复用 M2/M3a/M3b 已有依赖。

**前置:** M3b 已完成（commit `7532555` 之后），M3b 之后的三次架构调整已合入 `fix/tmux-client-size-follow-window-20260507-zj` 分支。

---

## ⚠️ 实施者须知

### 文档与代码的一致性

本里程碑没有独立的 spec 文件。设计决策直接落在 `claude-progress.md` 的"技术决策记录"节：
- "tab 替换 vs 新开"
- "session picker 触发时机"
- "配置与活跃连接分离的键空间设计"
- "放弃 tmux -CC 控制模式，回归 raw attach"

未来如果重启 -CC 路径，先看 `crates/aish-tmux/src/lib.rs` 顶部的"M3-archived"块，再决定是否撤销。

### tab 重命名的输入实现

GPUI 当前没有官方 simple text input widget；本里程碑用最小输入循环（KeyDownEvent + buffer + 光标 `|`），不支持光标移动 / 选择 / IME。后续如果有更复杂输入需求，应统一抽 `views/text_input.rs`。

### 鼠标滚轮方向约定

`alacritty_terminal::grid::Scroll::Delta(n)`：
- `n > 0` = 视口向上（看更老内容）
- `n < 0` = 向下（回到当前 prompt）

GPUI `ScrollDelta::Lines(p).y > 0` = 用户向上滚 → 正向传给 alacritty。如果未来发现某些 OS 滚轮方向相反，加 platform 配置开关而非反转语义。

---

## File Structure（M3c 修订版完成时）

| 文件 | 责任 | 状态 |
|---|---|---|
| `crates/aish-app/src/views/connection_chip.rs` | 终端上方 connection 信息条（label / SSH chip / ⊖ / ×） | NEW |
| `crates/aish-app/src/views/tab_bar.rs` | tab 列表 + 双击重命名（editing_tab + edit_buffer + focus_handle） | 扩展 |
| `crates/aish-app/src/views/terminal_view.rs` | 加 handle_scroll 调用 alacritty scroll_display | 扩展 |
| `crates/aish-app/src/views/mod.rs` | pub use ConnectionChipView | 扩展 |
| `crates/aish-app/src/app.rs` | RootView 在 connection tab body 包 chip + terminal | 扩展 |
| `docs/superpowers/plans/2026-05-07-aish-m3c-post-cc-rework.md` | 本文件 | NEW |

---

## Task 1: 终端区 connection chip — 已完成（commit `3cebd36`）

- [x] 新建 `views/connection_chip.rs`：32px 高 chip，含 ●/label/[SSH]/⊖/×
- [x] `views/mod.rs` 暴露 `ConnectionChipView`
- [x] `app.rs::RootView` 在 connection tab 模式下叠 chip + terminal

**视觉对照**：
```
┌──────────────────────────────────────────────────┐
│ [TabBar]                                         │
├──────────────────────────────────────────────────┤
│ ● 腾讯云 #1 [SSH]                  [⊖] [×]       │  ← connection chip (32px)
├──────────────────────────────────────────────────┤
│                                                  │
│              terminal_view (flex_1)              │
│                                                  │
└──────────────────────────────────────────────────┘
```

**⊖ collapse 语义**：把当前 tab.content 改为 Default + title 重置为 "新连接"，但 actor / Term / 后台连接全部**保留**在 `state.connections` 里。用户重新点 host 卡片**不会**回到原 connection（会开新的），原 connection 在 connections map 里成为孤儿。

> **遗留**：collapse 后的孤儿 connection 当前没有 UI 入口让用户回到。M3d 候选改进：默认页加"已活跃但脱离 tab 的 connection"区段，类似浏览器"最近关闭的标签页"。

## Task 2: 鼠标滚轮 scrollback — 已完成（commit `3cebd36`）

- [x] `terminal_view.rs::handle_scroll` 监听 `ScrollWheelEvent`
- [x] `ScrollDelta::Lines` / `ScrollDelta::Pixels` 统一换算成行数
- [x] `term.scroll_display(Scroll::Delta(lines * 3))` 每 tick 滚 3 行
- [x] 元素绑 `.on_scroll_wheel(...)`

**未做**：横向滚动（alacritty Term 不支持，PTY 也不感知）；触摸板 momentum（需要 OS 平台层支持，当前 GPUI 已经处理就用了，不需要额外代码）。

## Task 3: tab 双击 inline 重命名 — 已完成（commit `3cebd36`）

- [x] `TabBarView` 加 `editing_tab: Option<TabId>` / `edit_buffer: String` / `focus_handle: FocusHandle`
- [x] `handle_tab_click` 用 `MouseDownEvent.click_count` 分流（1=select / 2+=enter edit）
- [x] `handle_edit_key` 处理 Enter / Escape / Backspace / 字符
- [x] 编辑中渲染 `<buffer>|` + 蓝色下划线表示 focus

**视觉对照**：
```
普通态：    [● 腾讯云 #1 ×]
编辑态：    [● 我的开发机|×]    （下划线蓝色高亮）
```

**已知限制**：
- 不支持 IME（中文输入法只能从 keystroke.key_char 拿已 commit 的字符）
- 不支持光标移动 / 文本选择 / 复制粘贴
- focus 抢占：editing 时 tab_bar 接收 KeyDownEvent，可能与 terminal_view 焦点冲突 —— 实测下来 GPUI track_focus 能正确路由（最后一个调 focus_handle.focus 的视图获得焦点）

## Task 4: 文档归档 — 本文件即归档

- [x] 写本文件，把 M3c 实际做了什么 / 哪里没做 / 为什么改路径说清楚
- [ ] 单独 commit `chore: 归档 M3c 修订版 plan`（接下来一个 commit）

---

## 完成验证（M3c 修订版整体）

| 项 | 验证方式 | 状态 |
|---|---|---|
| connection chip 显示 host 名 + SSH 标签 | 真机：点 host 卡片连接，看终端上方 | ⏳ user |
| ⊖ collapse 切回默认页但 actor 不死 | 真机：点 ⊖ 后再点 host 卡片，应该开新连接而非复用 | ⏳ user |
| × close 完整断连 | 真机：点 × 后看活跃连接消失 | ⏳ user |
| 鼠标滚轮翻 scrollback | 真机：跑 `seq 1 1000` 后滚轮往上看 1-500 范围 | ⏳ user |
| 双击 tab 进入重命名 | 真机：双击某 tab 标题，输几个字，按 Enter | ⏳ user |
| Escape 取消重命名 | 真机：双击 → 输几个字 → Escape，标题不变 | ⏳ user |
| workspace 编译 + 测试 | `cargo +nightly clippy --workspace --all-targets -- -D warnings && cargo test --workspace` | ✅ exit 0，197 全过 |

---

## 下一步（M3d 候选）

按当前路径，下一里程碑可考虑：

1. **collapse 孤儿 connection 入口**：默认页加"已脱离 tab 的连接"区，列出 `state.connections` 里没被任何 tab 引用的；点击恢复到当前 tab。
2. **tab 拖拽重排序**：GPUI drag/drop API 体验 + 文档调研，先做 keyboard 版（Ctrl+Shift+PageUp/Down）。
3. **session picker 优化**：sessions 列表加 window 数 / 上次活跃时间（需要远端 `display-message -p '#{session_windows} #{session_activity}'`）。
4. **detach 检测**：tmux conf 注入 `set-hook -g client-detached 'send-message ...'`，捕获 detach 后清侧栏 attached 标记。
5. **粘贴**：Ctrl+Shift+V 把剪贴板内容写入 PTY；考虑 bracketed paste mode (`\x1b[200~ ... \x1b[201~`)。

每条独立 brainstorm → spec → plan → implement，**不并行**。

---

## Self-Review

### 路径变更覆盖
- ✅ -CC 模式废弃文档化（aish-tmux/lib.rs + 本文件）
- ✅ ConnectionId / TabId 引入文档化（aish-types + claude-progress.md）
- ✅ 三个新 view（chip / tab_bar 重命名 / scroll）落代码 + 本 plan
- ✅ 原 M3b "下一步"中 click-to-pane / per-pane send-keys / detach 检测 / SessionTree 重连四条标记为已废弃 / 推后

### Placeholder 扫描
- collapse 后孤儿 connection 没 UI 入口 — 已在 Task 1 标"遗留"
- tab 重命名输入循环简陋 — 已在"实施者须知"标限制
- 鼠标滚轮方向约定 — 已写注释 + 本 plan

### Type 一致性
- `ConnectionId` / `TabId` / `HostId` 三者编译期隔离（aish-types 单测 `connection_id_distinct_type_from_host_id`）
- `Connection.id == ConnectionId` / `Connection.host_id == HostId` 命名一致

### 依赖顺序
1. M3b 完成 ✅
2. -CC 废弃 + raw attach（commit `ffe2cdf`）
3. ConnectionId 分离（commit `74b704d`）
4. tab 系统 + picker（commit `37e5895`）
5. 本 plan 三条 UX 改进（commit `3cebd36`）
6. 本文件归档（下一个 commit）
