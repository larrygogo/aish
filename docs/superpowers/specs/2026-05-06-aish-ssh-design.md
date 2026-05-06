# aish — AI 友好的跨平台 SSH 桌面客户端

- **日期**: 2026-05-06
- **状态**: Design (approved by user, ready for implementation planning)
- **作者**: larry
- **定位**: 自用工具 + 顺便开源，YAGNI 优先

---

## 1. 背景与动机

主流 SSH 客户端（Windows Terminal、Tabby、Termius、MobaXterm、Xshell 等）在使用远程 AI CLI 工具（Claude Code、Codex、Aider 等）时存在以下痛点：

1. **tmux/screen 兼容差**：所有现存客户端把 tmux 当成黑盒终端流，用户必须背 `prefix + ?` 一堆快捷键，session/window/pane 没有可视化管理面板。
2. **附件支持缺失**：AI CLI 已经支持多模态输入（贴图、读文件），但 SSH 远端跑的时候本地附件没有顺畅的传输/注入路径，要靠 `scp` 然后手动粘路径，体验割裂。

aish 直接对准这两个核心痛点 + 一个差异化能力（**安全的远程 env 注入**，避免 API key 散落在每台 VPS 的 `.env` 里），目标是替代用户自己的 daily SSH driver。

---

## 2. 目标与非目标

### Phase 1（MVP，本 spec 范围）

| 模块 | 范围 |
|---|---|
| 跨平台 SSH 终端 | macOS / Windows / Linux，单 tab 即可 |
| tmux 可视化管理 | 自动检测、侧边栏树、attach/切换/新建/关闭 session·window·pane |
| 拖拽附件 + 路径注入 | 拖入文件 → SFTP 上传 → 路径自动回填光标 |
| 会话 env 注入 | 本地 keyring 集中存储 API key，连接时安全注入到远端 shell |

### Phase 2（架构兼容，本 spec 不实现）

多 tab、多服务器并行、远端文件浏览器、AI CLI 自动发现、剪贴板穿透。

### Phase 3（远期）

AI 输出富文本渲染（Markdown / diff / Mermaid）、端口转发 GUI、反向附件传输、内联图片预览、会话录制。

### 非目标（明确不做）

- 账号系统、订阅、付费
- 云端同步、团队协作
- Web 版、移动端
- 取代 IDE 的 Remote-SSH 能力

---

## 3. 系统架构总览

分层：UI Layer (GPUI Views) → App State (GPUI Models) → Core Services (Rust crates)

UI Layer：tmux Sidebar、Terminal View (alacritty_term)、Attachment Panel
App State：Connections / Sessions / Attachments / Env Profiles
Core Services：ssh (russh) / tmux (control mode) / sftp xfer / secrets (keyring)

底层通过 SSH (TCP) 连接 Remote Host (with tmux)。

### 分层职责

- **UI Layer**：纯 GPUI 渲染，无业务逻辑。订阅 Model 变化，渲染对应视图。
- **App State**：用 GPUI 的 `Model<T>` 管理共享状态。Models 是 single source of truth。
- **Core Services**：可独立测试的 Rust crate，每个职责单一。

### 关键设计决策

| 决策 | 理由 |
|---|---|
| **GUI: GPUI（纯 Rust）** | 用户偏好纯 Rust；GPUI 在 Zed 编辑器上证明了能做高质量终端 + Markdown + diff；Zed 源码可直接参考 |
| **终端实现：alacritty_terminal + GPUI 自绘** | 行业标准 vt100/xterm-256 实现，纯 Rust，与 GPUI 渲染层解耦 |
| **SSH：russh** | 纯 Rust，async-first，无 libssh2 C 依赖 |
| **tmux 集成：control mode（`tmux -CC`）** | tmux 主动推事件流，无需轮询；iTerm2 已验证此方案；单 channel 即可复用所有 pane 输出 |
| **GPUI/tokio 桥接：独立 tokio runtime + channel** | 两个 executor 共存，通过 mpsc / oneshot 解耦 |
| **凭证存储：OS keyring** | macOS Keychain / Windows Credential Manager / Linux Secret Service |

---

## 4. 核心数据流

### 场景：连接 → tmux 检测 → 列出 sessions → attach

1. 用户 Click "Connect"
2. SshClient::connect(host, auth) — russh 建立 TCP + 认证
3. SshClient 查询远端是否有 tmux（`command -v tmux`）
   - 没有 → 降级为普通 PTY shell channel
   - 有 → 进入步骤 4
4. TmuxController::start(ssh_client)：开 channel 启动 `tmux -CC new-session -A -s aish-default`
   - `-A` 表示 attach if exists, else create
5. 解析 control mode 输出流（tmux 启动后立即推全量状态）：
   - `%sessions-changed`
   - `%session-changed $0 main`
   - `%window-add @0` / `%window-add @1`
   - `%pane-add %0` / `%pane-add %1` ...
6. AppState.sessions = parsed_tree — GPUI Model 更新触发 UI 订阅
7. TmuxSidebar 重渲染
8. 用户 Click 某个 pane
9. TmuxController::switch_to(pane_id)：发 `switch-client -t @1` + `select-pane -t %3`
10. control channel 持续推 `%output %3 <bytes>`，路由到该 pane 对应的 alacritty_terminal Term 实例
11. GPUI TerminalView 订阅 Term 的 grid 变更，重绘屏幕

### 关键设计：单 channel 复用所有 pane 输出

一个 SSH Connection 对应：
- Channel 0：`tmux -CC` 长存活，承载所有 pane 的输入输出（通过 `%output %N` 路由），以及 send-keys / new-window / kill-pane / resize-window 命令
- Channel 1：SFTP，附件传输用，独立

**好处**：
- 一台服务器只占一个 SSH connection
- 所有 tmux pane 的输入输出都走同一个有序流，状态一致
- alacritty_terminal 只负责屏幕 grid 状态，纯 in-memory 状态机，易测试

---

## 5. Workspace 结构与核心接口

### Workspace 布局

```
aish/
├── Cargo.toml                 # workspace
├── crates/
│   ├── aish-types/            # 共享类型，零依赖
│   ├── aish-ssh/              # SSH 连接（russh 包装）
│   ├── aish-tmux/             # tmux control mode 协议
│   ├── aish-sftp/             # SFTP 附件传输
│   ├── aish-secrets/          # 跨平台 keyring 包装
│   └── aish-app/              # GPUI 主程序：Models + Views
└── docs/superpowers/specs/
```

### 依赖关系（单向，无循环）

aish-app → {aish-tmux, aish-sftp, aish-secrets, aish-ssh}
aish-tmux → aish-ssh
aish-sftp → aish-ssh
aish-ssh → aish-types
aish-secrets → aish-types

### 公开接口（核心抽象摘要）

- **aish-types**：HostId, SessionId, WindowId, PaneId, ProfileId, HostConfig, SshAuth
- **aish-ssh**：SshClient::connect / open_channel / open_sftp；Channel::request_pty / send_env / writer / reader
- **aish-tmux**：TmuxController::start / events / list_snapshot / switch_to / send_keys / resize_pane / new_window / kill_pane；TmuxEvent enum 覆盖 SessionAdded / Removed、WindowAdded、PaneAdded / Output / Died、LayoutChanged
- **aish-sftp**：SftpTransfer::upload / download / ensure_attach_dir；ProgressStream, TransferProgress
- **aish-secrets**：SecretStore::get / set / delete / list_keys，内部封装 keyring crate

### GPUI / tokio 桥接

GPUI 有自己的 executor，russh 依赖 tokio runtime——两者必须共存（Zed 也是这么做）：

- aish-app 启动时在专属线程跑 `tokio::runtime::Builder::new_multi_thread().build()`
- 所有 async 调用通过 runtime.spawn() 提交，结果通过 oneshot::channel 或 mpsc::channel 回到 GPUI
- GPUI Model 用 cx.spawn 接收 channel 并 update Model
- TmuxEvent 流：tokio::sync::mpsc → GPUI 的 cx.subscribe 风格 wrap

### 测试策略

| Crate | 测试方式 |
|---|---|
| aish-types | 单元测试 |
| aish-ssh | 集成测试用 testcontainers 起 linuxserver/openssh-server 镜像 |
| aish-tmux | 单元测试 control mode 解析（用录制的真实 tmux 输出）；集成测试连前面 ssh 容器 + tmux |
| aish-sftp | 集成测试上传/下载校验 hash |
| aish-secrets | mock keyring backend |
| aish-app | 重心放 manual + 视觉回归（参考 Zed 做法） |

---

## 6. 错误恢复与安全

### 错误恢复矩阵

| 场景 | 检测方式 | 恢复策略 | 用户感知 |
|---|---|---|---|
| **SSH TCP 断开** | russh stream EOF / keepalive timeout | 指数退避重连（1s→2s→…→60s 封顶） | 状态栏 "Reconnecting…"，pane 内容保留只读，重连后 tmux 自动 attach 回原 session |
| **tmux server 被杀** | control channel 收到 `%exit` 或解析错误 | 重新启动 `tmux -CC new-session -A -s aish-default` | toast "Remote tmux restarted"，侧边栏重建 |
| **单个 pane 死亡** | 收到 `%pane-died %N` | 从 AppState 移除该 pane Model | UI 上 pane tab 自动消失 |
| **SFTP 传输中断** | upload/download stream 异常 | 标记 attachment 为 Failed，提供"重试"按钮 | 附件面板显示红色 ⚠ + 重试按钮 |
| **control mode 协议解析错误** | 收到不识别的 `%xxx` 或格式异常 | 记录 raw 行到日志、跳过；连续 N 次 → 重启 control channel | 用户透明 |
| **认证失败** | russh 返回 AuthFailed | 不重试，弹窗提示 | 阻塞，等用户操作 |
| **远端 tmux 版本太老**（< 2.6） | start 时 `tmux -V` 检查 | 不启动 control mode，降级为 raw PTY shell | 侧边栏隐藏 + toast 提示 |
| **GPUI render panic** | panic hook | 写崩溃日志到 ~/.aish/crashes/，进程退出 | 应用退出，下次启动弹崩溃报告 |

### 安全考量

#### 凭证存储

| 数据 | 存储位置 | 说明 |
|---|---|---|
| SSH 密码 | **不存** | 强制要求用 key 或 agent；密码模式仅"输入即用即丢" |
| SSH 私钥 | 不存内容，只存**路径** | 引用 ~/.ssh/id_ed25519 等本地文件 |
| API key（env 注入用） | OS keyring | macOS Keychain / Win Credential Manager / Linux Secret Service |
| Host 配置 | 明文 JSON（~/.aish/hosts.json） | 不含任何凭证 |

#### env 注入路径

**首选：SSH SendEnv 协议**
- 客户端发 SSH_MSG_CHANNEL_REQUEST type=env
- 远端 sshd 的 AcceptEnv 白名单匹配 → 注入到子进程环境
- 优点：不进 shell history、不在命令行可见

**降级：连接后 export**
- 检测到 SendEnv 被拒绝
- 在 PTY 通道里发：`HISTCONTROL=ignorespace; export KEY=value && clear`（前导空格 + ignorespace → 不进 history；clear 抹掉屏幕痕迹）
- value 必须 shell-escape（用 shell-escape crate）
- 提示用户："此连接 env 注入走降级模式"

#### 远端命令构造（防注入）

所有发到 tmux 的命令通过 typed builder，禁止字符串拼接。例如禁止 `controller.raw_command(format!("send-keys -t {} ...", pane, user_input))`，正确的方式是 `controller.send_keys(pane_id, user_input.as_bytes())`，内部用 tmux -CC 的 hex 编码或 send-keys -l (literal) 模式。

#### 附件路径隔离

远端附件目录格式：`/tmp/aish-attach/<connection_uuid>/<timestamp>-<sha8>-<filename>`

- 应用专属前缀（aish-attach）
- 每次连接独立目录（uuid），断连后清理
- 客户端**不接受**用户指定 remote 路径，强制走隔离目录
- 文件名做 sanitize（去 ../、控制字符）
- 连接断开时由 RAII 触发清理（远端运行 rm -rf 该目录）

#### 日志与崩溃报告

- 日志默认级别 INFO，**永不打印** SSH 内容、env value、附件内容
- 崩溃报告剥离敏感字段
- 用户可手动开 DEBUG（弹窗确认"将记录敏感信息"）

---

## 7. MVP 里程碑

| 里程碑 | 周次 | 可演示状态 | 风险 |
|---|---|---|---|
| **M0 · Workspace 骨架** | W1 | cargo build 通过；CI（fmt/clippy/test）绿；6 个 crate 空壳；ADR 记下关键决策 | 低 |
| **M1 · GPUI 起步 + tokio 桥接** | W2-3 | 单窗口跑起来；左栏 + 主区基础布局；tokio runtime 与 GPUI executor 通过 channel 互通的最小例子跑通 | **高** — GPUI 学习曲线最陡的两周，预留 buffer |
| **M2 · SSH 连接 + 单 PTY 终端** | W4-5 | UI 添加 host → 连接 → 单 shell pane → 跑命令、看输出；TerminalView 处理键盘 / resize / 复制粘贴 | 中 — alacritty_terminal 与 GPUI 渲染绑定的胶水 |
| **M3 · tmux control mode** | W6-7 | 连上有现存 session 的服务器 → 侧边栏自动列出 → 点击切换 → 同窗口多 pane 共用一条 control channel 显示 | **高** — 协议解析的边角情况需要真实环境踩坑 |
| **M4 · 附件传输（上行）** | W8-9 | 拖图到窗口 → 上传进度可见 → 远端路径自动注入光标位置；附件面板可见已传文件 | 中 — GPUI drop 事件三平台兼容性 |
| **M5 · env 注入** | W10 | EnvProfileManager UI 增删 KV → keyring 持久化 → 连接时按 profile 注入 → 远端 echo $ANTHROPIC_API_KEY 看到值；SendEnv 失败自动降级 export | 低 |
| **M6 · 跨平台验证 + 打包 + 错误恢复打磨** | W11-12 | macOS / Windows / Linux 各跑通一遍；断线重连测试；崩溃日志；首个 release 出 .dmg / .msi / .AppImage | 中 — Linux 上 GPUI 稳定性是已知短板 |

### 任务拆分原则

1. **crate 实现先于 UI 打磨**：每个里程碑里，先把 aish-* 库做到能用、能测，UI 用占位，最后 M6 才统一打磨视觉。
2. **每个里程碑必须能 demo**：每周末确保 main 分支跑得起来。
3. **测试在 crate 层做扎实**：UI 测试 ROI 太低，重心放在 aish-ssh / aish-tmux / aish-sftp 的集成测试。
4. **风险任务前置**：M1 和 M3 是最难啃的，放在前半段。

### Phase 2 / 3 架构兼容点

- App State 已按 HostId 维度分组，多服务器是横向扩展
- aish-tmux 的 TmuxEvent 流和 alacritty_terminal::Term 已解耦，富文本渲染可做成 Term 输出之上的 overlay layer
- aish-sftp 已支持 download，反向附件只是 UI 接入

---

## 8. 关键技术决策摘要（ADR-style）

| ID | 决策 | 备选 | 选择理由 |
|---|---|---|---|
| ADR-1 | GUI 用 GPUI（纯 Rust） | Tauri (Rust+Web)、Iced、egui、Slint | 用户偏好纯 Rust；Zed 已证明 GPUI 能做高质量终端 + 富文本；Zed 源码可参考 |
| ADR-2 | 终端用 alacritty_terminal + GPUI 自绘 | xterm.js（需 webview）、自研解析器 | 行业标准，纯 Rust，与渲染层解耦 |
| ADR-3 | SSH 用 russh | ssh2（libssh2 binding）、thrussh | 纯 Rust、async-first、活跃维护 |
| ADR-4 | tmux 用 control mode (-CC) | 轮询 tmux list-*、自研协议 | 主动事件流、单 channel 复用所有 pane、iTerm2 已验证 |
| ADR-5 | tokio 与 GPUI executor 共存 | 完全跑在 GPUI executor 里 | russh 强依赖 tokio；用 channel 隔离避免 lifetime 灾难 |
| ADR-6 | 凭证用 OS keyring | 自研加密、明文 + 密码保护 | 跨平台标准，复用 OS 安全模型 |
| ADR-7 | env 注入：SendEnv 优先，export 降级 | 只用 export、只用 SendEnv | SendEnv 最安全但依赖 sshd 配置；export 兼容性更好但有泄露风险，做组合 |
| ADR-8 | 附件路径强制隔离到 /tmp/aish-attach/<uuid>/ | 用户指定路径、用 home 目录 | 防注入、易清理、不污染用户文件系统 |

---

## 9. 已知风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| GPUI 学习曲线陡、文档稀缺 | M1 可能拖到 W4 | 心理预期已提；Zed 源码是兜底参考 |
| Linux 上 GPUI 稳定性较差 | M6 Linux 验证可能延期 | Linux 接受 "beta 质量"，主推 macOS / Windows |
| tmux control mode 协议边角情况多 | M3 可能拖期 | 早期对接真实远端环境验证；保留降级路径 |
| 跨平台拖拽事件兼容（特别是 Windows） | M4 风险 | 早期就在三平台并行测试 |
| Mermaid 等富文本在 Phase 1 砍掉 | 用户体验缺失 | Phase 3 处理；可考虑远端预 render SVG 作为 fallback |
| 第一版周期 8-12 周对自用工具偏长 | 动力衰减风险 | 严格按里程碑切，每个 M 都能 daily driver |

---

## 10. 不在本 spec 范围内（明确边界）

- **多 tab、多服务器同时连接** — Phase 2
- **AI 输出富文本渲染** — Phase 3
- **AI CLI 自动发现** — Phase 2
- **远端文件浏览器** — Phase 2
- **端口转发 GUI** — Phase 3
- **反向附件传输 / 内联图片预览 / 剪贴板穿透** — Phase 3
- **会话录制回放** — Phase 3+
- **Snippet/Prompt 模板库** — Phase 3+

实现这些时再单独写 spec，每个走独立的 brainstorm → spec → plan → implement 循环。