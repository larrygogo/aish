# M28 — State Design（Empty / Loading / Error / Skeleton）

**日期**: 2026-05-15
**父 spec**: [`2026-05-15-aish-m24-visual-redesign-design.md`](2026-05-15-aish-m24-visual-redesign-design.md)
**目标**: 建立四类非 happy-path 状态（Empty / Loading / Error / Skeleton）的
统一视觉与组件 API，所有 view 走相同 anatomy + 文案 + icon 节奏，告别
"零散一行灰字"风。
**预计工程量**: 1.5-2 天，T1-T3 组件 + T4-T7 view 迁移

---

## 1. 动机

M26 把 typography 立体起来后，audit 当前 codebase 发现 happy-path 之外的状态
基本是**手糊一行灰字**或**零反馈**，与 Linear/Stripe Dashboard 风格差距明显。

### 现状 audit（2026-05-15 grep "is_empty|empty|loading|Connecting|Failed"）

| 场景 | 当前实现 | 问题 |
|---|---|---|
| Home 无 hosts | home.rs:633 一行 muted "还没有保存的连接 — 点上方 + 添加 host 开始" | 无 icon / 无层级 / CTA 隐含在外部 header 按钮里 |
| Home 无活跃 session | home.rs:318 直接 `None`（整个 ACTIVE SESSIONS 区块不出现） | 用户不知道这里"以后会有内容" |
| Terminal sidebar 但 tabs 空 | empty_terminal.rs（M4a 起就有，已迁 Title1） | 已经是当前最规整的，但 `>_` 字符作 icon 不够语义化、与 lucide icon 风格脱节 |
| Inbox（sidebar tab） | ComingSoonView 占位（INDEX M4a 范围内） | 写死"开发中" |
| Settings（曾经） | 同上，M4b 后已实质化 | — |
| SessionPicker 无 session | session_picker.rs:278 一行 muted "(无 session — 关闭弹窗回到 raw shell)" | 无 icon / 文案括号风 informal |
| Tmux NoTmux | state.rs:257 枚举值，但 UI 层未渲染（用户无感） | 静默 |
| Tmux QueryFailed | state.rs QueryFailed{msg}，同上无 UI | 静默 |
| SSH Connecting | terminal_view.rs:903 半透明黑遮罩 + 两行文字 | 无 spinner，用户不知是不是"卡住了"还是"在动" |
| SSH Disconnected | terminal_view.rs:933 底部 card 一行 + 重新连接按钮 | OK 但缺图标 / icon 语义；error reason 与 title 视觉差距弱 |
| SSH ConnectFailed/AuthFailed/Io/Protocol | app.rs:142 仅 toast_error | toast 易消失；连续两次同 host 失败时用户来不及看 |
| SFTP 单张上传失败 / batch abort | input_bar.rs BatchAborted toast | 同上 |
| 文件读写错误（hosts.json corrupt） | app.rs:92 仅 tracing::error! + 启动空列表 | 用户完全不知道，hosts 消失感觉是 bug |
| keyring 读密码失败 | ssh_actor 内部 tracing::error! | 同上 |
| 启动加载 hosts 中 | 同步 IO，无 skeleton | 当前 hosts 列表本地小，瞬时；将来云同步会需要 |
| host_form 校验失败 | host_form.rs 实时 error 字符串（M18） | OK，但分散在每个字段下方，缺统一组件 |

**核心结论**：4 类状态 × 14 个具体场景，缺统一组件 + 统一节奏。

### 行业参考

- **Linear**：Empty state = icon (32-40px outlined) + title (Title3 14/600)
  + description (Body 13/400/muted) + 可选 primary action
- **Stripe Dashboard**：Loading = shimmer 占位块（rounded sm 灰阶，500ms
  cubic-bezier 横向 gradient 平移）+ spinner（仅 inline action）
- **GitHub**：Error 页 = icon (AlertCircle 红) + title + reason + retry +
  "report bug" 二级 link
- **Notion**：Skeleton = 多行 placeholder block，结构 mirror 真实 content
  layout

aish 这一轮 M28 落同样四件套。

---

## 2. 决策记录（ADR-style）

### D-1: EmptyState 组件 anatomy

**采**：新组件 `aish_ui::EmptyState`，固定 4-slot anatomy：

```
┌─────────────────────────────┐
│         [icon 32px]         │  ← 圆形 secondary bg + muted_fg icon
│                             │     gap 12 ↓
│      Title3 (14/600/fg)     │  ← 必填，short label
│                             │     gap 4 ↓
│  Body (13/400/muted, max-w  │  ← 选填，长说明
│   320, text-center)         │
│                             │     gap 16 ↓
│      [Primary CTA]          │  ← 选填 button
└─────────────────────────────┘
```

API（builder）：

```rust
EmptyState::new("home-no-hosts")              // id
    .icon(IconName::Inbox)                    // 选填，默认无 icon
    .title("还没有保存的连接")
    .description("点右上角 + 添加 host 开始")  // 选填
    .action(Button::new(...).primary().label("添加 host"))  // 选填 AnyElement
```

整个组件居中（flex_col items_center justify_center），父容器决定占位区域
（size_full / py_16）。

**拒**：
- 不做 illustration / SVG 大图插画（onboarding 风），过重且依赖设计资源
- 不做 dismissible empty（"× 不再显示"），empty 是事实不是通知
- 不强制 icon 必填 —— SessionPicker dialog 内空状态没合适 icon 时只用文字

### D-2: LoadingSkeleton 组件 anatomy

**采**：新组件 `aish_ui::Skeleton`，单行占位块 + container 行变体：

```rust
// 单行 shimmer block：
Skeleton::block().w_full().h(px(16.0))           // 字段值 placeholder
Skeleton::block().w(px(120.0)).h(px(12.0))       // 短 label placeholder
Skeleton::circle().size(px(40.0))                 // avatar placeholder

// 组合：
div().flex().flex_col().gap_2()
    .child(Skeleton::block().w(px(140.0)).h(px(16.0)))  // line 1
    .child(Skeleton::block().w_full().h(px(13.0)))      // line 2
```

视觉：`secondary` 灰底 + 不带动画（v1 静态），rounded_sm。

**预留 shimmer** 但 v1 不实现：内部 `with_shimmer(bool)` builder 默认 false。
GPUI 动画 API 已用过（M20 spinner braille frame），shimmer 需要 linear-gradient
+ keyframe translate，留 v2。

**拒**：
- 不做 `Skeleton::card()` / `Skeleton::row()` 等"成品 layout" —— 真实
  layout 由 caller mirror，组件只提供 block 原语。否则要为每种业务列表
  各维护一套 skeleton，反而越做越多
- 不引入 css-style class（与 GPUI fluent builder 风不符）

### D-3: 错误页 vs 错误 toast 用法边界

**采**：按 "blocking 与否" 二分：

| 错误类型 | UI 形态 | 例子 |
|---|---|---|
| Blocking — 该 view 无法继续展示 | inline ErrorState 组件 | hosts.json corrupt → Home 列表区显 ErrorState；SSH Connect 失败 → Terminal 区显 ErrorState |
| Non-blocking — 操作失败但 view 仍可用 | toast_error | 单张图片 SFTP 失败 / 命令复制成功 / 单字段校验失败 |
| 字段级 inline | host_form 各字段下方红字（沿用 M18） | label 必填 / port 越界 |

**新组件 `aish_ui::ErrorState`** 与 EmptyState 完全同 anatomy（4-slot），
仅 default icon 改 `IconName::AlertCircle` + 红色（destructive）：

```rust
ErrorState::new("home-load-failed")
    .title("加载主机列表失败")
    .description(reason)
    .action(Button::new("retry").label("重试").primary())
```

把 EmptyState 和 ErrorState 共用一个 internal struct（`StatusView`），
公开 builder 仅在默认 icon / icon 颜色上有差。

**拒**：
- 不做 modal-style error dialog（破坏 flow，toast + inline 已覆盖）
- 不做 banner / 顶部红条（多 state 容易堆叠，先用 toast queue）

### D-4: 各场景的图标 + 文案规则

**新增 IconName**（M28 一次性扩，与 EmptyState/ErrorState 配套）：

| IconName | 用途 | 来源 |
|---|---|---|
| Inbox | 无收件 / 无活跃 session / 无 host | lucide inbox.svg |
| Server | SSH 连接相关 empty | lucide server.svg |
| WifiOff | 网络断开错误 | lucide wifi-off.svg |
| FileQuestion | 文件读取失败 / 找不到 | lucide file-question.svg |
| Loader | spinner 静态形（蹬 spinner 还用 InputBar 的 Braille 文字） | lucide loader.svg |

文案规则：

- **Title**：动词陈述事实，不用感叹号（Linear 风）
  - ✓ "还没有保存的连接"
  - ✗ "Hosts 为空！"
- **Description**：告诉用户下一步可以做什么，13/400/muted
  - ✓ "点右上角 + 添加 host 开始"
  - ✗ "Hosts.json file is empty"（暴露实现）
- **Action button label**：单个动词短语
  - ✓ "添加 host" / "重试" / "查看日志"
  - ✗ "点这里添加" / "Click to retry"

**拒**：
- 不写"Oops!" / "出错啦~" 类口语化文案（dev tool 走严肃 Linear/Stripe 风）
- 不暴露技术名词（"SIGWINCH" "PTY" "SFTP" 等仅在 Body description 选填
  且面向开发者上下文出现，比如 SSH error reason 透传）

### D-5: 不引入插画 / 不做 i18n

**采**：
- 所有 empty state 仅用 lucide line icon，无插画 SVG。Linear / Stripe 自己
  也走 minimal icon-only 风，不需要 illustration
- 文案直接中文 hardcode，与现有 codebase 一致（"添加 host" / "重新连接"
  等中文 label 已经满地）。M28 不引入 i18n 框架。

**拒**：
- onboarding 引导插画 / Lottie / 多步骤 tour 组件 —— scope 爆炸
- gettext / fluent-rs 等 i18n 库 —— 长远才考虑，单 user CN 环境 N/A

### D-6: 不重写 SSH Connecting overlay 与 host_form 字段 error

**采**：M28 范围限定 EmptyState / Skeleton / ErrorState **三个新组件 + 7
个 view 迁移**。下列 happy-path 边缘已有专属机制，不动：

- `terminal_view.rs` SSH Connecting overlay（已有 + 完整文案，Connect 中
  期约 3-5s，加 spinner 留 M29）
- `terminal_view.rs` Disconnected 底部 card（已有 + 重连按钮，仅替换
  hardcoded text_size 为 typography，归到 M26 backlog）
- `host_form.rs` 各字段下方红字 inline error（M18 已有 + 实时校验）
- `input_bar.rs` Send 按钮 Braille spinner（M20 已有）
- toast 系统（已成熟）

M28 只把"裸 grep '是不是 empty 啊？' 一行灰字"这类场景升级到统一组件。

**拒**：把 SSH Connecting overlay 重写成 EmptyState 风 —— 它是 in-progress
状态不是 empty，且已经 OK；统一化反而失个性。

### D-7: 改造范围（具体 view × 状态）

| view | 当前状态 | 改造为 |
|---|---|---|
| home.rs - hosts 列表空 | 一行灰字 | EmptyState (icon Inbox + title + description + action="+ 添加 host") |
| home.rs - active sessions 空 | `None`（区块隐藏） | 保持隐藏（active sessions 本就是 ad-hoc，无需占位） |
| empty_terminal.rs | `>_` 字符 + title + body + 按钮 | EmptyState (icon Server + title + description + action="Go to Home") |
| session_picker.rs - 无 session | 一行 muted "(无 session)" | EmptyState (无 icon + title + description=按 Esc 回 shell) |
| state.rs TmuxState::NoTmux | UI 层未渲染 | M28 不动（用户没要求暴露） |
| state.rs TmuxState::QueryFailed | UI 层未渲染 | M28 不动 |
| app.rs hosts.json load 失败 | toast (新增) + Home 列表区 ErrorState | ErrorState (icon FileQuestion + title="加载主机列表失败" + description=reason + action="重试") |
| home.rs - 启动 hosts 加载中 | 同步 IO 瞬时（无 skeleton） | M28 暂不做（无异步加载源） |

**未来用 Skeleton 的预留场景**（M28 不强制启用，但组件做好待命）：
- 远端文件管理器 list-files 期间（M29+ 计划）
- 云同步 hosts list（M30+）

**总改造 view 数：3 个 view（home / empty_terminal / session_picker）+ 1 个
ErrorState 接入点（app.rs hosts load 失败路径）**。

---

## 3. 架构变化总览

```
+---------------------------------------------------------+
| aish-ui/src/components/                                  |
|   empty_state.rs (新)                                    |
|     StatusView 内部 struct (4-slot anatomy)              |
|     EmptyState builder (default icon=Inbox, fg=muted)    |
|     ErrorState builder (default icon=AlertCircle,        |
|       fg=destructive)                                    |
|   skeleton.rs (新)                                       |
|     Skeleton::block() / Skeleton::circle()                |
|     with_shimmer(bool) builder (v1 stub)                  |
+---------------------------------------------------------+
| aish-ui/src/icons/mod.rs                                 |
|   IconName +5 (Inbox / Server / WifiOff /                |
|     FileQuestion / Loader) + 5 SVG asset                 |
+---------------------------------------------------------+
| aish-app/src/views/home.rs                               |
|   empty_hint 一行灰字 → EmptyState                       |
| aish-app/src/views/empty_terminal.rs                     |
|   '>_' 字符 → EmptyState                                  |
| aish-app/src/views/session_picker.rs                     |
|   "(无 session)" → EmptyState                            |
| aish-app/src/app.rs                                      |
|   hosts.json load_err 路径 → state.hosts_load_error     |
|   home.rs 渲染 ErrorState 替代 hosts 列表区              |
+---------------------------------------------------------+
```

state 加一个字段 `hosts_load_error: Option<String>`，启动 load_hosts 失败
写入；home.rs render 时优先看该字段，Some(err) → ErrorState；None →
正常 hosts 列表（含 EmptyState 当 hosts.is_empty()）。

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | EmptyState / ErrorState anatomy 与现有 view 父容器 layout 冲突 | 中 | 组件自身 flex_col items_center justify_center，父决定占位（size_full / py_16）；T4-T7 每个 view 迁移后手测视觉 |
| R2 | 5 个新 icon SVG 包大小 | 极低 | 每个 SVG ~1KB，总 5KB；与已有 18 个 icon 同量级 |
| R3 | Skeleton 无动画 v1 显得"假"（dead block） | 低 | 接受：v1 静态 placeholder 已比"没东西"好；shimmer 留 v2 |
| R4 | hosts_load_error 字段污染 state.rs | 低 | 单 Option<String> 字段 + 启动写入 / 用户点重试时 clear；与现有 modal/pending_session_picker 同模式 |
| R5 | session_picker 内 dialog 嵌 EmptyState 内层 layout 不居中 | 中 | session_picker dialog body 给 EmptyState 显式 min_h(px(120))；测试场景：session list = 0 时 dialog 视觉 |
| R6 | 文案误用（caller 把 description 当 title 写长句） | 低 | spec D-4 文案规则 + code review 把关 |
| R7 | Inbox icon 与 Lucide 风格其他 icon stroke width 不一致 | 低 | Lucide 全套 stroke=2，下载时统一选 default 24x24 source |
| R8 | M28 范围与未来"Inbox / Coming Soon" tab 实质化撞车 | 低 | Inbox 实质化是另一个 milestone；M28 只关心 Inbox 占位**当下**渲染（仍 ComingSoonView 或换成 EmptyState 二选一，本 spec 推荐换） |

---

## 5. Out of scope（M28 不做）

- onboarding 引导插画 / Lottie 动画 / 多步骤 product tour
- i18n 框架（gettext / fluent-rs）—— 文案直接中文 hardcode
- Skeleton shimmer 动画（v1 静态）
- SSH Connecting overlay 重写（已有专属 UI，保留）
- host_form 字段 inline error 重写（M18 已有）
- toast 系统改造（已成熟）
- Inbox / 远端 file manager / 云同步 hosts 等异步加载源（这些是 Skeleton 的
  未来用户，但 M28 仅准备组件，不接入业务）
- TmuxState::NoTmux / QueryFailed 的 UI 暴露（用户无感即可，避免提示噪音）

---

## 6. 测试策略

### 单测（aish-ui）

- `EmptyState::default()` 渲染包含 4 个语义 slot（icon / title / description /
  action）的子元素数量正确：仅 title → 1 child；title+description → 2；
  全 4 → 4
- `ErrorState` 默认 icon = AlertCircle + 颜色 = destructive
- `Skeleton::block()` / `Skeleton::circle()` 各自 rounded 与 bg 正确
- 不写"截图视觉"测试（GPUI 渲染层不接 snapshot，沿用 M11+ 测试约定）

### 集成（手测）

- Home 删光 hosts → EmptyState 出现 + icon + 居中 + 点 CTA 弹 host_form
- Terminal sidebar 关掉所有 tabs → empty_terminal 渲染 EmptyState
- SessionPicker 远端无 session（mock）→ dialog 内 EmptyState 居中
- 临时把 hosts.json 改成 invalid TOML / JSON 启动 aish →
  Home 显 ErrorState + "重试" 按钮 + 点重试触发 reload
- dark / light 主题切换看 EmptyState / ErrorState 都渲染正确
- 改造前后 5 个 view 截图对比，确认"一行灰字 → 4-slot anatomy"视觉跃迁

### 单测增量预估

- aish-ui 211 → ~220（+9：EmptyState anatomy 4 / ErrorState 3 / Skeleton 2）
- aish-app 144 → ~146（+2：hosts_load_error 字段 + render 分支）

---

## 7. Plan 引用

见 [`../plans/2026-05-15-aish-m28-state-design.md`](../plans/2026-05-15-aish-m28-state-design.md)

---

## 8. 实施记录

留待 T1-T7 完成后回填（参考 M26 模板：commits 表 + Risk 实际遇到 + 测试
增量 + 未做/跨主题验证 几节）。

---

## 7. 实施记录（2026-05-15 完成）

T1-T7 全部实施，T8 文档收尾。

### 实际 commits

| Task | Commit | 内容 |
|---|---|---|
| spec + plan | `e12c967` | 4 个并行 spec 一波合 |
| T1 | `fd5526c` | 5 个 lucide SVG (Inbox/Server/WifiOff/FileQuestion/Loader) + IconName 扩展 |
| T2 | `b59105c` | StatusView 4-slot anatomy + EmptyState/ErrorState 工厂 + 6 单测 |
| T3 | `f83e08b` | Skeleton block/circle 原语 + 5 单测 |
| T4-T7 | `283b07c` | home/empty_terminal/session_picker 接入 + hosts_load_error 字段 + 1 单测 |
| T8 | (本次) | spec 实施记录 + INDEX 加 M28 entry |

### Risk 实际命中

- **D-7 改造范围** 严格按预期 — 3 view + 1 error path，无 scope creep
- 没遇到 spec R 节列出的 risk（无 trait 冲突 / 无渲染 layout 飘移）
- Skeleton shimmer 暂留 stub（v1 静态），M30 接入后再补

### 测试增量实际

- aish-ui 211 → **222** (+11 = 6 EmptyState + 5 Skeleton)
- aish-app 144 → **145** (+1 hosts_load_error 默认)

### 未做（后续 milestone）

- shimmer 动画（依赖 M30）
- 其他 view 的 Loading state（SSH Connecting overlay 已成熟，不重写；
  SFTP 上传 spinner 现状 OK）
- onboarding 引导插画（spec D-5 明示 out of scope）
