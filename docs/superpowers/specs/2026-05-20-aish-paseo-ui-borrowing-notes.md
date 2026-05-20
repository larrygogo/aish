---
title: paseo UI 美学借鉴笔记
date: 2026-05-20
status: research
source: workspace/paseo-reference/docs/design.md + packages/app/src/styles/theme.ts
target: aish-ui crate（M22-M37 已积累的设计系统）
---

# paseo UI 美学借鉴笔记

> 对照 paseo 的 `docs/design.md`（13 章设计宪法）+ `theme.ts`（token 系统）
> 跟 aish-ui 当前状态（M22-M37 已有 typography / anatomy / motion / hover
> 子系统），找可借鉴 / 已对齐 / 不该抄 三类，按 ROI 排落地优先级。

---

## 0. 一句话总结

paseo **设计宪法的可读性 + 多 dark theme + 严格 forbidden 清单**最值得抄。
aish 已经有完整的 component anatomy / motion / typography 三大子系统，**比
paseo 在动效维度更深**，缺的是「写下来的顶层设计原则」+ 几个零散 token 补全。

---

## 1. paseo 设计宪法 13 章浓缩

| 章节 | 核心规则 |
|---|---|
| §1 Character | minimal / spacious / quiet / confident。「whitespace is the design」 |
| §2 Reuse | 「相同语义元素出现 ≥3 处即 primitive」，1 次是 screen |
| §3 Hierarchy | **weight + color**，不靠字号；3 档 weight：screen titles 400/300 desktop / structural labels medium / content normal |
| §4 Buttons | 5 个 variant：default(accent) / secondary(surface3) / outline / ghost / destructive。每页 ≤1 个 default |
| §5 Borders | group rows in card / separate pane header / borderAccent for outline button — 三种用法 |
| §6 Pickers | 5 个 primitive：DropdownMenu / Combobox / ContextMenu / AdaptiveModalSheet / confirmDialog。按选项数 + 是否需搜 + 锚定方式选 |
| §7 Density | settings/projects 720px 居中列；workspace/chat 全宽。card 内行紧贴只靠 divider 分 |
| §8 Responsiveness | compact-first。list+detail shell 复用，**不发明第三种布局** |
| §9 Copy | sentence case / no trailing period on row titles / imperative buttons / present-participle in-flight |
| §10 States | inline loading 14px / page loading large center / card loading 一行文 / empty noun phrase / inline error red-300 xs / page Alert / disabled = opacity only |
| §11 Rows | content 列 + trailing slot；chevron = nav / kebab = action；hover-revealed on web / always on native |
| §12 Pills | `<StatusBadge>` 是唯一 pill：palette[300] fg + 10% alpha bg。bespoke pill = drift |
| §13 Forbidden | **17 项明确禁令**，几乎每条都是踩过的坑 |

---

## 2. aish-ui 当前状态盘点（M22-M37 已经在做的）

### 已有且系统化

| 系统 | aish 实现 | 对照 paseo |
|---|---|---|
| Typography | 9 个 type role（M26：display×3 / structural×3 / content×3） | 比 paseo 的 3 档 weight 更精细 |
| Anatomy | Card / Dialog / List / Form / Page / Overlay padding+gap 规则（M27） | paseo 没有等价的统一抽象，散落在 settings.ts |
| Motion | duration + easing 4 档 token + animate_or_skip + reduced_motion accessibility（M30-M34） | **paseo 没有结构化 motion token**，aish 更深 |
| Hover/Press/Focus | M17 容器按下沉色 / M18 ghost active 跳一档 / M31 button stateful / M32-M34 hover transition | paseo §8 hover 规则是 onHoverIn/Out 模式，aish 在动效层更细 |
| Theme kind | dark + light（暂同 dark 配色） | paseo 5 个 dark variant + 1 light |
| Color tokens | 27 个 token（background/card/popover/primary/secondary/muted/accent + 各自 hover/active + sidebar_bg_top） | paseo 30+ 个，分层更细（surface0-4） |

### 缺失但 paseo 有

| 缺失项 | paseo 实现 | 严重度 |
|---|---|---|
| **顶层设计宪法文档** | docs/design.md 248 行可读宪法 | 高 — aish 子系统多了，缺整体 charter |
| **IconSize token** | xs/sm/md/lg = 12/14/16/20 | 中 — aish 现在 icon 大小手写 px |
| **BorderWidth token** | 0/1/2 | 低 — aish 现状默认 1px 够用 |
| **Opacity token** | 0/50/100 | 中 — disabled state 已用，但散落 hex |
| **Surface 阶梯** | surface0-4 + surfaceSidebar/Hover/Workspace | 中 — aish 6 个 surface，少一档「workspace」语义 |
| **StatusBadge primitive** | palette[300] fg + 10% alpha bg | 中 — aish 没有 reusable pill |
| **多 dark theme variant** | paseo / zinc / midnight / claude / ghostty | 低 — 用户基数小不必抄全 |
| **inline error + empty state 规范** | red-300 xs under field / noun phrase centered muted | 中 — aish 仅有 toast，无 inline |
| **Forbidden 清单** | 17 项明确禁令 | 中 — 知识在脑子里，没写下来 |

### aish 有但 paseo 没有

| aish 特色 | 价值 |
|---|---|
| Typography 9 个 type role | 比 paseo 3 档 weight 更精细 |
| Anatomy 抽象（Card/Dialog/...）| paseo 是 settings.ts 局部约定 |
| Motion 完整子系统 + reduced_motion | paseo 没有 motion token |
| primary/secondary/destructive 各自 hover/active 三态色阶 | paseo 仅 default/hover 两态 |
| sidebar_bg_top 渐变（M35.1）| paseo 是单色 sidebar |
| OS-native polish（M37 macOS / Win 各自键盘 / 鼠标行为） | paseo 是 RN 跨端，不到这一层 |

---

## 3. 真正值得借鉴的 10 个具体点（按落地价值排）

### A. 写一份「aish-ui 设计宪法」文档 ⭐⭐⭐

paseo 的 design.md 是**「Designer / Engineer / AI agent 都能读的执行手册」**。aish
当前 motion / typography / anatomy / hover 知识散在四份子文档 + commit message
里，缺一份顶层 charter 把「Character / Hierarchy / 命名约定 / Forbidden」串起来。

**落地**：写 `docs/aish-ui-charter.md`（≈ 200-300 行），分章节复用 paseo 结构，
但内容是 aish 自己的设计语言（不直接抄 paseo 配色 / 命名）。

**工作量**：半天 - 1 天。**ROI 最高**。

### B. 补齐 IconSize / Opacity / BorderWidth token ⭐⭐⭐

- `IconSize { xs: 12, sm: 14, md: 16, lg: 20 }` — 现在 svg icon 用 `px(14.)` /
  `px(16.)` 手写散落，统一到 token
- `Opacity { disabled: 0.5, transparent: 0.0, full: 1.0 }` — disabled 用 `0.5`
  到处出现
- `BorderWidth { thin: 1, thick: 2 }` — 实际只用到 1，但留 token 便于未来

**落地**：在 aish-ui/src/theme/tokens.rs 加三个 struct，更新 caller。

**工作量**：1 天（包含 callsite 迁移）。

### C. 加 `StatusBadge` primitive ⭐⭐

paseo §12：`<StatusBadge>` 是 pill 唯一实现，4 个 variant（success/warning/danger/muted），
配色规则 `palette[color][300]` foreground + `rgba(color, 0.1)` background。

aish 现在没有 status pill —— Host 卡片状态、tmux session 状态、连接状态都用
ad-hoc 小色块。统一成 primitive 后调用更整洁。

**落地**：`aish-ui/src/components/status_badge.rs` + 4 variant（success/warning/destructive/muted）。

**工作量**：1 天。

### D. Forbidden 清单文档 ⭐⭐

paseo §13 的 17 项 forbidden 是「我曾经踩过这个坑」级别的明确禁令：
- `<Pressable>` wrapping `<Text>` = wrong (用 Button)
- `onPointerEnter` / `onPointerLeave` = wrong on native
- Color changes for disabled state = wrong（用 opacity）
- Spacing 不在 scale 内 = wrong
- 等等

aish 也有等价的「踩过的坑」（M13/M15/M18/M30 commit message 里有），但**没写成清单**。

**落地**：写 `docs/aish-ui-forbidden.md`（80-100 行），列 aish 自己踩过的坑：
- ColorTokens hex 不能硬编码在 view（要走 theme(cx).colors.X）
- hover 不能用 `.on_mouse_move` 自己维护，要用 GPUI `.hover()` 或 stateful
- div 嵌套用 div_uniform_pad 走 anatomy
- 等等

**工作量**：半天。

### E. 加一档 surface「workspace」语义 ⭐⭐

paseo 在 surface 阶梯上有 `surfaceWorkspace = surface1`，明确表示「workspace 主区
背景」语义。aish 现在 terminal 区背景是 `background`，跟 Home / Settings 主区
同色 — **但 terminal 区在视觉上其实想有微妙不同**（有 tmux 时 / 全屏时）。

**落地**：在 ColorTokens 加 `surface_workspace: Hsla`，初始等同 background，
未来需要时调整。

**工作量**：1-2 小时。

### F. inline error / empty state 规范化 ⭐⭐

paseo §10 把状态呈现分级：
- inline loading: 14px spinner + muted color
- page loading: large centered
- card loading: 一行短文字（**不用 spinner**）
- empty state: noun phrase, centered, muted
- inline error: `palette.red[300]` xs under field

aish 现在的 toast 系统（`aish_ui::toast_error` / `toast_warning`）只覆盖 page-
level 通知，**inline 错误 / empty state 没有规范**。Host 卡片的「未探测 OS」、
Settings 输入框的「字段错误」都是 ad-hoc。

**落地**：
- `aish-ui/src/components/empty_state.rs`：noun phrase + muted color + optional ghost button
- `aish-ui/src/components/inline_error.rs`：单行 destructive color xs，用于表单字段下方

**工作量**：1-2 天。

### G. 加 1 个 dark theme variant 作为「midnight」选项 ⭐

paseo 提供 5 个 dark theme，aish 现在只有 1 个 dark（indigo accent）。**不抄 5
个，只加 1 个**作为实验性 alternative。

候选：
- **midnight**（深紫蓝 surface + indigo accent 加亮）—— 对应 paseo midnightDark
- 或 **claude**（暖橙 surface + orange accent）

**落地**：在 dark.rs 旁加 dark_midnight.rs 实现，Settings 加切换。

**工作量**：1 天。

### H. List+detail shell 复用 ⭐

paseo §8 把 list+detail 在 Settings / Projects / Sessions 三处复用同一个 shell。
aish 目前 Settings / Home / Inbox 风格不完全统一（Home 是 launchpad style，
Settings 是 row 风格，Inbox TBD）。

**落地**：**不立即做**。这是大重构（3-5 天），且 aish 的 Home 是 launchpad 特意
跟 Settings 不同（M36 决策）。等真有需要时再统一。

**工作量**：3-5 天（暂不推荐）。

### I. Density 规则文档化 ⭐

paseo §7：「**The whitespace is the design**」、「page → spacious; section →
spacious; card → tight」、「rows 16px padding， sidebar 8-12px」、「不能压缩行
高去塞内容」。

aish anatomy 已经有 padding/gap 规则，但**没有「page/section/card 三级 rhythm」
的明文叙述**。

**落地**：把 density 规则补进 anatomy.rs 的 doc comment 或写进设计宪法 §7。

**工作量**：包含在 A 里。

### J. Copy 风格规范（中文版） ⭐

paseo §9 是英文规则（sentence case / no trailing periods / imperative）。aish
是中文，需要 reimagine：
- 按钮：动词词组（「连接」「保存」「取消」），不是名词（「连接管理」）
- toast：「[主语]：[动作] — [简短建议]」三段式
- empty：名词短语（「没有连接」「未配置 host」）
- inflight：动词 + 「中...」（「连接中...」「保存中...」）
- error：直陈状态 + 建议（「连接失败 — 双击 tab 重试」）

aish 已经隐含遵守了大部分，**写下来即正式化**。

**落地**：写进设计宪法 §9。

**工作量**：包含在 A 里。

---

## 4. 不该抄的部分

| paseo 设计 | 不抄理由 |
|---|---|
| 5 个 dark theme variant | 维护成本高，aish 用户基数小 |
| 5 个 picker primitive | aish 已有 Popover/Modal/ContextMenu，加 Combobox 够，再加 5 个过度 |
| terminology 术语表 | aish 是 SSH 客户端，词汇完全不同（无 workspace / agent / provider） |
| sidebar callout 系统 | aish 是单机桌面 app，无跨设备 / 跨 worktree 通知场景 |
| `<AdaptiveModalSheet>` 兼容 mobile sheet | aish 仅桌面，不需要 sheet form factor |
| FONT_SIZE 8 档 | aish 5 档 + Typography 9 个 role 已经覆盖，加多反而碎 |

---

## 5. 落地优先级表

| Pri | 项 | 工作量 | 依赖 |
|---|---|---|---|
| **P0** | A. aish-ui 设计宪法文档 | 0.5-1 天 | 无 |
| **P0** | B. IconSize / Opacity / BorderWidth token | 1 天 | 无 |
| **P1** | C. StatusBadge primitive | 1 天 | 无 |
| **P1** | D. Forbidden 清单文档 | 0.5 天 | 推荐在 A 之后 |
| **P1** | E. surface_workspace 语义 | 0.1 天 | 无 |
| **P2** | F. inline_error + empty_state component | 1-2 天 | C 完成后更协调 |
| **P2** | G. midnight dark theme variant | 1 天 | 无 |
| **P3** | H. List+detail shell 统一 | 3-5 天 | 不推荐近期做 |
| 自动 | I. Density 规则文档化 | 包含在 A | — |
| 自动 | J. Copy 风格规范（中文版） | 包含在 A | — |

---

## 6. 推荐推进顺序

如果你想真正落地一波 paseo 美学借鉴，最自然的次序：

1. **A + I + J**（设计宪法 + density + copy 规则）—— 一份文档搞定，半天
2. **B**（token 补全）—— 1 天
3. **D**（Forbidden 清单）—— 半天
4. **E**（surface_workspace）—— 2 小时
5. **C**（StatusBadge）—— 1 天
6. **F**（inline_error + empty_state）—— 1-2 天
7. **G**（midnight theme variant）—— 1 天（可选实验）

总计约 5-7 天工作量，分批进 commit，每批独立 ship。

---

## 7. 推荐结论

**短期立即推**：A + B + D + E（约 2 天）—— 全是文档 + 小 token 补全，零 UI
风险，提升设计系统可读性。

**中期看需求**：C + F（约 2-3 天）—— 引入两个新 primitive，需要 caller 迁移。
等真有用例触发再做。

**长期实验**：G（约 1 天）—— midnight theme 是好玩点，但不影响主线。

**不做**：H（list+detail 统一）—— Home launchpad 是 M36 明确决策的差异化，
不该抹平。
