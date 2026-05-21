# issh 设计原则

> 这份文档是 issh 视觉与交互设计的**北极星**。所有 PR review、design decision、
> 新组件设计、color/spacing token 取舍都应回到这三条原则来判断。
>
> 立于 M35（2026-05-15），从 M22-M34 design system 累积经验沉淀。

---

## 定位一句话

**issh 是给开发者用的、有审美的 SSH 客户端 — 不是终端，是工作站。**

| 我们不是 | 我们是 |
|---|---|
| iTerm2 / Alacritty 那种纯粹的渲染器 | Linear / Warp 那种 *opinionated* 的工作台 |
| Termius 那种主推「跨设备 sync」的云产品 | 本地优先、单人开发者掌控感 |
| Royal TSX 那种企业 dashboard | 单人 daily-driver 的桌面 app |

每次有"加什么新功能 / 改什么视觉"的犹豫，先回到这个**目标用户 + 场景**画像：
**深夜独自工作的全栈开发者，本地 Mac/Windows，多 host 多 session 跨切换，要快、要清、要不打扰**。

---

## 三条原则

### 1. Technician with taste — 工程师的克制审美

**不要可爱，不要 corporate。**

- 色彩饱和度永远比 marketing site **低一档**。`accent` 是 indigo 不是 violet；`primary` 是深 emerald 不是 bright green；hover_bg 是 secondary 灰阶不是 hue-shifted color。
- 圆角永远比 consumer app **小一档**。Card / Dialog 用 `radius.lg`(8px)，Button 用 `radius.md`(6px)，list row 用 `radius.md`。**不用** 12px+ 的「胖圆角」。
- Emoji **仅出现在用户输入的内容里**（如 host label "🏠 家用 VM"）。UI chrome 里**永远不用** emoji 当 icon — 用 lucide icon set + Linux brand SVG。
- Layout 优先**直角、严格对齐**。不做 diagonal slice、不做 wave divider。Section divider 是 1px Separator 或 spacing 留白，**不是装饰元素**。

**反例**：
- ❌ Sidebar 紫色 glow ring（已删 commit `1bf2627`）
- ❌ Card hover lift（设计阶段排除）
- ❌ "Welcome!" 这种 marketing 文案 — 用「继续工作」「保存的主机」这种 verb/noun

**正例**：
- ✅ Linux 发行版 SVG 用品牌色但**饱和度 -15%** 适配 dark theme
- ✅ ACTIVE SESSIONS section 上方一条 muted Separator —— 仅一条 1px 灰线分区
- ✅ host card 上 `larry@1.2.3.4:22` 用 `TypeRole::Code` JetBrains Mono

---

### 2. Earn every pixel — 每个像素必须 justify

**每个像素必须服务一个具体的认知任务**：识别 / 操作 / 状态指示。**装饰性元素零容忍**。

判断标准：
- 这个元素如果**删掉**，用户能否完成任务？
  - 能 → 删掉
  - 不能 → 保留
- 这个元素能否被**已有的元素**兼任？
  - 能 → 合并

**反例**：
- ❌ Sidebar 60px 窄柱配 icon-only — icon 不自明（用户不知道齿轮是 Settings 还是 Tools），又没腾出空间放更有价值的列表
- ❌ Card 内 header / body / footer 间挂多余 divider line 当装饰
- ❌ Empty state 配大段引导文字 — 开发者会跳过

**正例**：
- ✅ ConnectionChip 升级后含 status dot（状态）+ host name（识别）+ tmux session（识别）+ click→tooltip（操作）—— 4 个像素任务挤在 28px 高
- ✅ host card avatar 一格（识别 OS）+ label 一行（识别 host）+ user@host:port 一行（识别连接信息）+ 上次连接时间一行（状态）—— 4 task 一卡片
- ✅ HostForm Label 字段移到底部 + 标「可选」—— 因为 90% 用户不填 label，不应占首位

---

### 3. Motion is feedback, not decoration — 动画必须回答问题

每段动画必须能回答用户的**一个具体问题**：

| 动画 | 回答什么问题 |
|---|---|
| Button mouse_down 0.7→1.0 opacity | "我点了吗？" |
| Card hover bg 150ms lerp | "这是 actionable 吗？" |
| TabItem active indicator fade-in | "我切换成功了吗？" |
| Dialog 150ms fade-in | "modal 加载完成了吗？" |
| Toast 250ms slow fade-in | "这是 transient 的，不要打扰我" |
| NavItem hover transition | "这是导航容器，不是 button" |

**禁止**：
- 纯装饰的 entrance 动画（卡片首次出现时的 stagger fade-in）
- bounce / spring / elastic（这是 marketing 视觉）
- scale up（hover lift），Linear/Vercel 已弃
- 超过 250ms 的微交互（150ms 为 medium，250ms 仅 Toast 用，更长一律 reduced）

**reduced_motion 兼容**：所有 with_animation 调用都走 `animate_or_skip(theme, ...)` 包装，
用户开 Settings → 「减少动画」switch 后所有动画 instant 跳过，但 press feedback
opacity 0.7 保留（物理触觉反馈不该被 accessibility 偏好移除）。

---

## 应用到决策

下次面对设计取舍，问自己：

1. 这符合 **technician with taste** 吗？（不可爱、不 corporate、克制）
2. 每个像素都 **earn 自己的存在** 吗？（删了能不能用）
3. 加的动画在 **回答用户问题** 吗？（不是装饰）

3 条全过 → 合理。任一不过 → 重新设计或不做。

---

## 历史决策追溯

每次做出符合本三原则的设计决策，应在对应 spec 顶部注脚引用本文件。
反例（违反原则但落地的决策）应在 INDEX.md 「已废弃」记录原因，
作为下次 review 的负面案例库。

参考：
- M22 motion 系统设计：原则 #3
- M29 HostForm redesign：原则 #2（label 移底部）
- M31 Card focus ring 不做：原则 #1（容器不画 ring）
- post-M34 NavItem ring 删除 (`1bf2627`)：原则 #1 + #2（sidebar 窄柱视觉过载）
- M35 ⌘K Palette：原则 #2（开发者期待的快速访问 — earn）+ 原则 #1（不做 fancy result preview）
