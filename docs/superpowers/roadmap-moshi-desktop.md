# 桌面版 Moshi Roadmap

> 长期愿景活文档。aish → 桌面版 Moshi 的路线图。
>
> 每个 milestone 完成后回头打勾、修订估算、补充教训。本文件**不替代**单个 milestone
> 的 spec/plan（仍在 `specs/` `plans/`），仅作为跨多个 milestone 的总览索引。
>
> 重大决策仍走 `docs/adr/00NN-*.md`，本文件只列指针。

**最后更新**：2026-05-08（M4a brainstorm）

---

## 总愿景

aish 当前是桌面端 SSH + tmux 终端 GUI 客户端（GPUI / Windows-first / Rust）。
[Moshi](https://getmoshi.app/docs) 是 iOS/iPadOS 移动终端，定位为"远程驱动你机器
上长运行的 AI coding agent + shell + tmux"，差异化卖点不在终端基础体验上，而在
对 AI agent 会话的专属 UI（Inbox / Activity / 图片粘贴 / 语音输入 / 配额）。

**aish 的演进方向**：把"桌面版 Moshi"作为长期目标——保留通用 SSH/tmux 能力作
为底层，但 UI 与交互重组为 AI-agent-first，让用户在桌面上获得 Moshi 等价
甚至更好的远程 agent 驾驶体验。

**与 Moshi 的差异点**（不是 1:1 复刻）：
- 桌面端，多 tab 并列查看会话，不限制单会话
- Windows 优先（Moshi 是 iOS only）
- 通用 SSH/tmux 能力保留，不强绑定 agent

---

## 子项目拆解

依赖图：A 是 UI 容器地基；B 是 agent 系列地基；C/D/G/H 都需要 B。E/F 独立。

| ID | 名称 | 依赖 | 状态 | milestone | 估算 |
|---|---|---|---|---|---|
| **A** | 4-tab 信息架构（Sidebar + Home/Terminal/Inbox/Settings） | – | 🟡 进行中 | M4a / M4b | 3-4d |
| **B** | AI agent 会话识别（Claude Code / Codex / Cursor 等） | – | ⏸ 未开始 | M5 | 1-2d |
| **C** | Activity 实时活动条（agent 当前任务进度） | B | ⏸ 未开始 | M6 | 2-3d |
| **D** | Inbox 事件中心（agent 工具完成 / 任务完成 / 需要批准） | B, C | ⏸ 未开始 | M7 | 2-3d |
| **E** | 图片粘贴 / 拖拽到 prompt | – | ⏸ 未开始 | M8 | 1-2d |
| **F** | 语音输入（Whisper.cpp / 云引擎） | – | ⏸ 未开始 | M9 | 2d |
| **G** | 配额 / Token 用量解析 | B | ⏸ 未开始 | M10 | 1d |
| **H** | Approval / Prompt 卡片（agent 询问 → 浮卡批准） | B, C | ⏸ 未开始 | M11 | 2-3d |

总估算：~16-23 天工程时间，分多个 milestone 渐进交付。

---

## Sub-project A · 4-tab 信息架构（详细愿景）

### M4a（当前 milestone）— 仅地基
- 左侧 48px 纯 icon 4-tab 侧边栏（Home / Terminal / Inbox / Settings）
- Home：Hosts grid + Active Sessions + Quick Actions
- Terminal：复用现有 tab 系统；空状态用 EmptyTerminalGuideView
- Inbox / Settings：ComingSoon placeholder（链回本 roadmap）
- spec：[`specs/2026-05-08-aish-m4a-info-arch-design.md`](specs/2026-05-08-aish-m4a-info-arch-design.md)

### M4b — Recent + Settings 起步
- Home 加 Recent 区（持久化 host 上次使用时间到 `~/.config/aish/state.toml`）
- Settings 实质内容：应用信息、版本、shortcut 列表（只读）
- 估算：1-2d

### 远期 Home 区块（不属于 M4 系列，等 agent 集成）
- **Token 用量 widget**（依赖 G）：Home 顶部条 / Settings 都可放
- **Agent 任务进度卡片**（依赖 C）：Home 列出"远端正在跑的 agent 任务"
- **Pinned hosts**（书签）：用户手动标 ⭐ 的 host 排在最上
- **Search**（host 多了之后必要）：顶部搜索条 fuzzy match host name

### 远期 Inbox 内容（依赖 B+C+D）
事件类型：
- `tool-finished` / `tool-failed`：agent 调用某工具完成 / 失败
- `approval-requested`：agent 请求批准（rm / push / commit 等）
- `task-completed`：长任务结束
- `quota-warning`：Token 即将耗尽

UI：
- 左 list（事件流，时间戳分组：今天 / 昨天 / 上周）+ 右详情面板
- 红点 badge 在 Sidebar Inbox icon
- 桌面 Toast（Windows toast / Linux notify-send / macOS NSUserNotification）
- 点详情可"跳到来源会话"（切 sidebar=Terminal + 激活对应 tab + 滚动到事件位置）

### 远期 Settings 内容
设置树：
- **Appearance**：主题切换（dark/light/auto）、字体族 / 字号、accent 色
- **Input**：键位（粘贴 / 语音触发键 / Ctrl+1..4）、bracketed paste 模式默认
- **Notifications**：哪些 Inbox 事件触发桌面 Toast、是否声音
- **Hosts**：host 全局默认（默认 shell、默认 PTY 大小）
- **Agents**（依赖 B）：每个 agent 自定义识别规则、prompt 模板
- **Advanced**：日志级别、telemetry 开关、SSH 选项

---

## Sub-project B · AI agent 会话识别（设计思路）

**目标**：在一个 SSH 连接里识别"这是 Claude Code / Codex / Cursor / Gemini /
OpenCode 的会话"，给 tab 加 agent 图标，让后续 C/D/G/H 知道用哪个解析器。

**识别策略**（多层级 fallback）：
1. **进程级**：远端 `ps`/`pgrep` 检查 `claude / codex / cursor-agent` 进程
2. **输出特征**：解析终端 buffer 找特征字符串（`╭─ ` 框线、`> claude`
   prompt、`---` 分隔等）
3. **用户手动标**：tab 右键 → "标记为 Claude Code / Codex / …"
4. **配置 / 别名**：HostConfig 加 `default_agent: Option<AgentKind>`

**风险**：各家协议不一致，统一抽象层（`AgentEvent` enum）的 surface 设计是关键。
预研：M5 第一天先调研 Claude Code 的 stream-json 输出和 Codex 的 SSE，看共性。

---

## Sub-project E · 图片粘贴（设计思路）

**目标**：截图（Win+Shift+S → 剪贴板）或拖拽图片文件 → aish 把图片注入到当前
agent 的 prompt。

**注入方式按 agent 不同**：
- **Claude Code**：base64 嵌入 stream-json 的 `image` content block；命令行
  也支持 `claude --image path/to.png`
- **Codex CLI**：通过 `codex --attach` 或粘贴 base64 data URL
- **Cursor**：HTTP API 形式的 image upload
- **泛用 fallback**：把图片保存到远端 `/tmp/aish-clip-<ts>.png` 然后 echo
  路径到 prompt，让用户手动 `@/tmp/aish-clip-...png`

**实现要点**：剪贴板图片读取在 GPUI / arboard 都已经有；难点在远端注入路径。
M8 立项时再细化。

---

## Sub-project F · 语音输入（设计思路）

**目标**：Push-to-talk（如长按 F12）→ 录音 → 转文字 → 注入到当前 prompt。

**引擎候选**：
- **Whisper.cpp**（本地）：~140MB base 模型 / ~1.5GB large 模型；离线、隐私好；启动稍慢
- **Azure Speech / Google Cloud Speech**（云）：实时低延迟；需要密钥配置；隐私问题
- **Windows SpeechRecognizer**（系统自带）：Windows 10+ 有；中文支持一般

策略：默认 Whisper.cpp（small 模型），高级用户配置云引擎。

---

## Sub-project H · Approval / Prompt 卡片（设计思路）

**问题**：agent 经常问"是否运行 `rm -rf foo/`?"或"批准 push 到 main 吗?"，
用户在终端里要找到那行 `[y/n]` 不友好。

**方案**：解析 agent 输出识别 approval prompt，浮一张卡片在 terminal 上方：
- 卡片显示：操作摘要（`rm -rf foo/`）、危险等级、原始 stdout 链接
- 两个按钮：[Allow] / [Deny]，点击 → 把 `y\n` / `n\n` 注入 PTY
- 与 Inbox 联动：approval 也是一种 Inbox 事件

依赖 B（识别会话）和 C（解析输出）。

---

## 已知风险 / 调研项

| 风险 | 影响 | 缓解 |
|---|---|---|
| AI agent 协议各家不一 | 子项目 B 抽象层难度高 | M5 第一天先做调研报告，再写 spec |
| 语音输入 Whisper 模型体积大 | F 启动慢 / 安装包大 | small 模型默认；可下载更大模型 |
| 桌面 Toast 跨平台 API 不同 | D 实现复杂度 | 用现成 crate（`notify-rust`） |
| 图片粘贴每家 agent 注入路径不同 | E 兼容性 | 先做 Claude Code，泛用 fallback 兜底 |
| Inbox 数据持久化 vs 仅内存 | D 跨会话恢复 | 第一版仅内存，后续加 sqlite |

---

## ADR 指针

跟桌面版 Moshi 路径相关的关键决策（如有新增）：

- 0010 Sidebar 4-tab 信息架构（M4a 立项时如确立写）
- 0011 AI agent 会话识别策略（M5 立项时写）
- 0012 Inbox 持久化方案（D 立项时写）

---

## 历史里程碑（已合并到本 roadmap）

跟桌面版 Moshi 路径相关的之前的工作：

- **M3d-ui-polish**（2026-05-08，已完成）— UI 视觉抄了一些 Moshi 风（圆角卡片 / 暗色 / chip），是本 roadmap 的视觉前奏。
- **M3d-ui-iter2**（2026-05-08，已完成）— 删了 ConnectionChip 横条，简化 RootView 给 4-tab 架构腾位。
