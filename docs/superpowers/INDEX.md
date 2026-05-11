# aish Superpowers 索引

> 全部里程碑 plan + spec 的索引 + 当前状态 + 下一步候选。
>
> 每完成一个 milestone 更新本文件。这是 `claude-progress.md` 的替代品（后者已废弃）。
>
> **长期愿景** → [桌面版 Moshi Roadmap](roadmap-moshi-desktop.md)（跨多里程碑活文档）

---

## 当前状态

- **活跃分支**：`feat/aish-ui-m13-20260511-zj`（M13 Card/NavItem/TabItem + 全 view 切组件已完成）
- **下一里程碑**：M14 — DropdownMenu/ContextMenu + Light theme 实现 + 其他 M11-M13 遗留（TextInput mask / Toast 关闭按钮 / Dialog Tab focus trap / Select 弹层翻转 / Button hover variant 精细化 等）
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui 90 + aish-app 101 + 其他 crate) 全过

---

## Milestones（按时间倒序）

### M13 — aish-ui Card / NavItem / TabItem + 全 view 切组件（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m13-cards-nav-design.md`](specs/2026-05-11-aish-m13-cards-nav-design.md)
- plan：[`plans/2026-05-11-aish-m13-cards-nav.md`](plans/2026-05-11-aish-m13-cards-nav.md)
- 范围：3 个新组件 + 3 处 view 迁移
  - Card（4 slot: header/body/footer/actions + 3 variant: Default/Outlined/Elevated + on_click + group_hover actions 悬停显隐）
  - NavItem（vertical+horizontal 双模 + icon 接受任意 IntoElement + label + active indicator: 左 2px vertical / 底 2px horizontal）
  - TabItem（3 slot: prefix/title/suffix + active 切 bg + 绝对定位底部 2px primary line + on_click 透 click_count）
  - home host 卡片 → Card（actions 浮在右上角 hover 显示）
  - sidebar_nav 4 tab → NavItem.vertical()（Nerd Font icon 通过 div+font_family 包装传入）
  - tab_bar tab 项 → TabItem（rename 状态机和 close stop_propagation 仍在 caller）
- 关键 commits：
  - `311dc0b` — T1 Card
  - `8dc83f3` — T2 NavItem
  - `5f74da1` — T3 TabItem
  - `2431e00` — T4 home host 卡片切 Card（Card 内部加 .relative()）
  - `417d487` — T5 sidebar_nav 切 NavItem
  - `4290846` — T6 tab_bar 切 TabItem
- 测试：aish-ui 77 → **90**（+13：Card 4 / NavItem 5 / TabItem 4）；aish-app 101 不变
- 收尾：aish-app 内复合 view 元素全部组件化（仅 terminal_view 本体 + 已废弃 tmux_sidebar 保留手糊）

### M12 — aish-ui 表单与导航 + HostForm/SessionPicker 迁移（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-09-aish-m12-forms-nav-design.md`](specs/2026-05-09-aish-m12-forms-nav-design.md)
- plan：[`plans/2026-05-09-aish-m12-forms-nav.md`](plans/2026-05-09-aish-m12-forms-nav.md)
- 范围：5 个新组件（Checkbox / Switch / Tabs / Dialog / Select）+ HostFormModal 重写为 Dialog+Tabs+6 TextInput（-294 行）+ SessionPickerView 外壳迁 Dialog + SettingsView 加 Appearance section 含 Dark mode Switch（点 Light 弹 toast warning + 视觉回弹）
- 关键 commits：
  - `20c106f` — T1 Checkbox（builder + 受控）
  - `122ac00` — T2 Switch（iOS 风胶囊）
  - `022e59b` — T3 Tabs（Entity + 键盘 ←/→）
  - `3bbc6e1` — T4 Dialog（Entity + Esc/backdrop close + needs_focus）
  - `72e992f` — T5 Select（Entity + 下拉 + ↑/↓/Enter/Esc）
  - （T6 prelude 验证：components::* glob 自动覆盖，无 commit）
  - `f52719d` — T7 HostFormModal 重写（删 FocusField/cycle_focus，引入 SyncedKey + 6 TextInput）
  - `9c617c0` — T8 SessionPickerView 外壳迁 Dialog
  - `8247f82` — T9 SettingsView Appearance section + Dark mode Switch
- 测试：aish-ui 51 → **77**（+26：Checkbox 5 / Switch 4 / Tabs 5 / Dialog 6 / Select 6）；aish-app 101 不变
- 已知边界：Dialog Tab 循环 focus trap 留 M13；Select 弹层只向下；Light theme 仍 unimplemented! stub；HostForm 的 password mask toggle 不再支持（M11 TextInput 暂无 mask 模式）；Edit 模式下"Tab 切字段"键盘流不接（依赖 Dialog focus trap）

### M11 — aish-ui 起步套件（2026-05-09）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-09-aish-m11-ui-starter-design.md`](specs/2026-05-09-aish-m11-ui-starter-design.md)
- plan：[`plans/2026-05-09-aish-m11-ui-starter.md`](plans/2026-05-09-aish-m11-ui-starter.md)
- 范围：新建 aish-ui 独立 crate（依赖仅 gpui）+ Theme/Token 系统（21 ColorTokens + Radius/Spacing/FontSize scale + Theme::dark）+ 15 个 Lucide SVG icon + AssetSource 实现 + 7 个组件（Button/IconButton/Badge/Separator/Tooltip/TextInput/Toast）+ aish-app 接入：注册 Theme global / ToastHandle global / AssetSource，InputBarView 文本部分迁到 TextInput
- 关键 commits：
  - `4f025cc` — T1 crate 骨架 + workspace 注册
  - `90de327` — T2 Theme/Token + dark + light stub（amend 修 hex_a 死代码）
  - `405c27b` — T3 Icon 系统（15 SVG + AssetSource，amend 修 IconName::ALL 常量）
  - `2a75c62` — T4 Separator
  - `429747f` — T5 Badge
  - `5290bfd` — T6 Button
  - `0496cac` — T7 IconButton
  - `e3685e0` — T8 Tooltip
  - `81e12b6` — T9 TextInput 基础
  - `3f65fed` — T10 TextInput cursor blink
  - `cad165a` — T11 TextInput selection（amend 修 selection_range 空 range bug）
  - `ea5ee69` — T12 TextInput 复制粘贴（amend 修 IME on_change + compute_copy_payload 抽取）
  - `8ea5322` — T13 Toast 三层（amend 修 Warning fg_color）
  - `fd4a57c` — T14 aish-app 接入 Theme/ToastHandle/AssetSource
  - `7b84e8d` — T15 InputBarView 文本部分切到 aish_ui::TextInput
- 测试：aish-ui crate 51 tests + aish-app 101 tests + 其他 crate 不变（合计 268 全过）
- 已知边界：M11 简化版 mouse click 不解析 x 坐标（点击 = 末尾），cursor_at_pixel 留 M12+

### M10 — App 图标（跨平台 SVG→PNG/ICO/ICNS + Build 集成）（2026-05-09）— ✅ 已完成
- spec：[`specs/2026-05-09-aish-m10-icon-design.md`](specs/2026-05-09-aish-m10-icon-design.md)
- plan：[`plans/2026-05-09-aish-m10-icon.md`](plans/2026-05-09-aish-m10-icon.md)
- 实现状态：代码已完成并通过质量门禁 (fmt / clippy / test)，文档已补充
- 范围：SVG 单一真相源（终端像素风 `>_` 设计）→ 8 级 PNG 套装 + Windows ICO (6 尺寸) + macOS ICNS (7 尺寸)；Node.js 生成脚本 (@resvg/resvg-js + png-to-ico + @fiahfy/icns)；Windows build.rs 编译期集成；macOS Info.plist 配置；Linux .desktop 文件
- 关键任务：
  - T1：SVG 主设计源（已完成，assets/icons/aish.svg）
  - T2：Bun 生成脚本（已完成，scripts/gen-icons.js）
  - T3：运行脚本生成 PNG/ICO/ICNS（✅）
  - T4：Windows build.rs 集成（✅）
  - T5：macOS Info.plist（✅）
  - T6：Linux desktop 文件（✅）
  - T7：INDEX.md 更新（✅）
- 产出文件：
  - `assets/icons/aish-{16,32,48,64,128,256,512,1024}.png`
  - `assets/aish.ico`
  - `assets/aish.icns`
  - `crates/aish-app/build.rs`
  - `packaging/macos/Info.plist`
  - `packaging/linux/aish.desktop`

### M9 — Agent 输入栏（图片多选 + 文字 + SFTP 批量上传）（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m9-input-bar-design.md`](specs/2026-05-08-aish-m9-input-bar-design.md)
- plan：[`plans/2026-05-08-aish-m9-input-bar.md`](plans/2026-05-08-aish-m9-input-bar.md)
- 范围：终端视图下方固定底栏；[+] 按钮 GPUI 系统文件选择器多选图片；缩略图预览 + × 关闭；文字输入（Enter 发送）；Send → 批量 SFTP 上传 → paths + text echo 到 PTY
- 关键 commits：
  - `7ac8865` — state.rs UploadBatch + BatchUploaded/Failed + actor + app.rs 事件处理（T1-T3）
  - `9742fc5` — InputBarView 完整实现 + 集成到 RootView（T4-T5）

### M8 — 图片粘贴（Ctrl+Shift+V + SFTP + echo path）（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m8-image-paste-design.md`](specs/2026-05-08-aish-m8-image-paste-design.md)
- plan：[`plans/2026-05-08-aish-m8-image-paste.md`](plans/2026-05-08-aish-m8-image-paste.md)
- 范围：Ctrl+Shift+V 检测剪贴板类型 → 图片走 arboard 读取 + PNG 编码 + SshClient::sftp_upload → 远端 /tmp + echo 路径到 PTY；文字走现有 bracketed paste 逻辑
- 关键 commits：
  - `19e4fc3` — build: workspace 依赖加 russh-sftp / arboard / image
  - `96752d7` / `2a95493` / `366d852` — SshError::Sftp + SshClient::sftp_upload
  - `d589727` — state.rs UploadImage + ImageUploaded/Failed
  - `8cd880e` — terminal/image.rs encode_rgba_to_png
  - `fd7caad` — terminal_view paste() 图片检测
  - `41d4baf` — ssh_actor UploadImage match arm
  - `dd96d4c` — app.rs 事件处理 + 质量门禁

### M4b — Recent 持久化 + Settings 起步（2026-05-08）— ✅ 已完成
- spec：（无独立 spec，架构设计见 M4a spec）
- plan：[`plans/2026-05-08-aish-m4b-recent-settings.md`](plans/2026-05-08-aish-m4b-recent-settings.md)
- 范围：TOML 持久化 / app_state_file / last_connected 字段 + humanize / SettingsView 三段布局（Version / App Info / Legal）/ Inbox icon 换 Nerd Font / 启动加载 last_connected
- 关键 commits：
  - M4b-task-1：添加 toml + serde 依赖
  - M4b-task-2：app_state_file.rs 实现 TOML 读写
  - M4b-task-3：state.rs 添加 last_connected + humanize
  - M4b-task-4：SettingsView 三段布局
  - M4b-task-5：接入 SettingsView，启动加载
  - M4b-task-6：Home 卡片时间戳 + 写 recent
  - M4b-task-7：Inbox icon Nerd Font
  - M4b-task-8：质量门禁 + INDEX 更新

### M4a — 信息架构 4-tab 化（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m4a-info-arch-design.md`](specs/2026-05-08-aish-m4a-info-arch-design.md)
- plan：[`plans/2026-05-08-aish-m4a-info-arch.md`](plans/2026-05-08-aish-m4a-info-arch.md)
- 长期上下文：[桌面版 Moshi Roadmap](roadmap-moshi-desktop.md) · 子项目 A
- 范围：左侧 48px sidebar 4-tab + Home(hosts grid + active sessions + quick action) + Inbox/Settings ComingSoon placeholder + EmptyTerminalGuideView
- Out-of-scope：Recent 持久化 / Settings 实质内容 / 任何 AI agent 集成 / 键盘快捷键 Ctrl+1..4
- 关键 commits：
  - `426b0b6` — feat(state): SidebarTab + sidebar 字段
  - `84ee15f` — feat(theme): sidebar 常量
  - `5e6352c` — feat(ui): SidebarNavView
  - `f354b34` — feat(ui): HomeView
  - `159ab9a` — feat(ui): EmptyTerminalGuideView + ComingSoonView
  - `9f19375` — feat(ui): RootView 重构
  - `973b8d1` — refactor(ui): 删除 DefaultPageView

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
