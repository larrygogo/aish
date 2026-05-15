# M35 — UI/UX 整体提升 design

**日期**: 2026-05-15
**父 spec**:
- [`2026-05-15-aish-m24-visual-redesign-design.md`](2026-05-15-aish-m24-visual-redesign-design.md)（首次视觉重构基线）
- [`2026-05-15-aish-m26-typography-hierarchy-design.md`](2026-05-15-aish-m26-typography-hierarchy-design.md)（9 TypeRole 体系）
- [`2026-05-15-aish-m27-component-anatomy-design.md`](2026-05-15-aish-m27-component-anatomy-design.md)（Anatomy spacing token）
- [`2026-05-15-aish-m30-animation-design.md`](2026-05-15-aish-m30-animation-design.md)（motion token + reduced_motion）
- M31 / M32 / M33 / M34（6 个 stateful entity + motion 系统完整收尾）

**目标**: 在 M22-M34 已建立的 design tokens + motion 系统底子上做**视觉层级与
信息密度的密度提升 + 定位明确化**。当前产品的 token / typography / motion
已达商业级，但视觉层级偏「平」、sidebar 60px 窄柱信息密度低、Home 页主次
关系不清、SSH 主题相关 typography 没专用 token，整体仍带「developer demo」
气质。本 spec 把这些问题编号记录 + 给出可执行设计决策。

**预计工程量**: 分 3 个 release，累计 11-14 天。Phase A（v0.next）1-2 天为
高 ROI 小批量，可独立 ship；B / C 是后续大改不阻塞 A。

---

## 1. 定位与设计哲学（写进未来 `docs/design/principles.md`）

**一句话定位**：*aish 是给开发者用的、有审美的 SSH 客户端 — 不是终端，是工作站。*

| 不是 | 是 |
|---|---|
| iTerm2 / Alacritty 纯粹的渲染器 | Linear / Warp 那种 opinionated 的工作台 |
| Termius 主推「跨设备 sync」的云产品 | 本地优先、单人开发者掌控感 |
| Royal TSX 企业 dashboard | 单人 daily-driver 桌面 app |

**三条原则**：
1. **Technician with taste** — 工程师的克制审美。不要可爱，不要 corporate。色彩饱和度永远比 marketing site 低一档；圆角永远比 consumer app 小一档；emoji 仅出现在用户输入的内容里。
2. **Earn every pixel** — 每个像素必须服务一个具体的认知任务（识别 / 操作 / 状态指示）。装饰性元素零容忍。
3. **Motion is feedback, not decoration** — 动画必须回答用户的一个问题。不做 entrance / bounce / scale up 这种「看起来很 cool」的效果。

---

## 2. 问题清单（P-1..P-13）

按**结构性**问题（P-1..P-3）/ **Design system 精化**（P-4..P-8）/ **关键 flow**（P-9..P-13）三组分类。

### 结构性问题（最高优先）

#### P-1: 视觉层级太「平」

Home 页的 **HOSTS 区**与**ACTIVE SESSIONS 区**用同样的字号、间距、caption divider label。用户**扫不出主次** — 「Active Sessions」应该是「立即可恢复的工作」（首要 CTA），Hosts 是「备选起点」（次要列表），但视觉权重 1:1。

**症状**：用户打开 Home 第一眼不知道该看哪里。

**根因**：section divider label 都用 `TypeRole::Caption` (12/400/muted)。

#### P-2: Sidebar 60px 窄柱信息密度低

3 个 icon-only nav（Home / Terminal / Settings）占满全屏高度的左侧 60px 窄柱。
- 既不能像 macOS Finder 那样靠 icon 自明（缺 tooltip + label）
- 也没腾出 240px 空间放真正有用的快捷列表（最近连接 / 主机收藏）

**症状**：开发者目标用户感觉 sidebar 是"浪费空间"。Warp 用顶 tab、Linear 用左 240px 多层级，aish 现在的 60px 是夹缝。

#### P-3: Terminal 页面 ConnectionChip 视觉权重不足

Terminal tab 切进去就是黑屏 + cursor，缺一个**始终在场的、有视觉权重的** connection chip（host 名 / SSH 用户 / 当前 tmux session / 连接耗时）。代码里有 ConnectionChip 组件但仅 text 无 bg/border，5 个 tab 间跳时识别成本高。

### Design System 精化

#### P-4: Motion 时长 150ms 在密集操作时累积感知为「卡顿」

`Motion::medium = 150ms` 当前驱动所有 hover/press/fade。Linear / Warp 的微交互都在 80-150ms 区间。连点 5 个 host 切换时 150ms 累积感知为"反应慢"。

**修法**：medium 降到 120ms，全 6 个 stateful entity 同步受益（改一个 token 值）。

#### P-5: 缺 `TypeRole::Code` — SSH 主题信息没专用 typography

aish 是 SSH 工具，`host name / ssh command / passphrase / paths` 这些等宽内容散落各处，目前都用 `TypeRole::Body`（proportional font）渲染。

**修法**：补 `TypeRole::Code` = JetBrains Mono 12px/400/foreground。**一个改动让产品立刻有 developer tool 气质**。

#### P-6: Border token 在 dark theme 下对比度不足

Dark theme 下 Card outline 几乎看不见。`colors.border` 与 `colors.card` 差距太小。

#### P-7: Accent (active nav 紫) 饱和度偏高

NavItem active=true 时 bg=accent，但 accent 当前 S 值偏高，仍略喧宾。前几个 commit 已移除 focus ring glow，accent bg 是仅剩的视觉权重源 — 应该**降一档饱和度** (S -10%) 配合。

#### P-8: 缺 `elevation_focus` shadow token

Dialog / Popover 当前 shadow 与 Card 等高，开 modal 时**没有 z 维度差异**。需要更深的 blur（24） + 大 alpha（0.4）作为 floating layer 专用。

#### P-9: Light theme 7 个 token 仍 dark fallback

INDEX 历史记录显示 M15/M17 有 7 个 light theme token 用 dark 占位 + TODO。SSH client 极少用户使用 light，工程量不值得做完。需要决策：**接受现状 + Settings 加「实验性」标签** 或 **正式废弃 light theme**。

### 关键 Flow 问题

#### P-10: HostForm 信息组织顺序错

当前 4 个字段（label / host / port / user）按 hosts.json schema 顺序排，但开发者**最熟悉的格式是 `user@host:port`**（SSH 原生）。应该有「快速输入」单行字段支持这个格式，4 字段表单作 fallback。

另外：
- 认证方法是 Tabs（隐藏"还有 2 种"信息），改 inline radio 一眼看全
- Label（显示名）90% 用户不在乎，应可选 + 折叠到底部

#### P-11: SSH 连接失败错误展示太轻

当前 SSH 连接失败仅一行 toast 红字。错误信息是 SSH 客户端**核心场景之一**，toast 不够份量。

**修法**：升级到 inline ErrorState 嵌在 terminal viewport 中央（保留 terminal 边框 + connection chip），含 [重试] [编辑 host] [复制错误] 三个 button。

#### P-12: Settings 缺关键页面

- **快捷键展示页** — 开发者会找这个（Ctrl+Shift+T / Ctrl+Shift+V 等）
- **关于页**真正的 logo + version + license + GitHub link（LOGO_128 资产已在 logo.rs 预留但无 caller）

#### P-13: 全局缺 ⌘K Command Palette

当前 host 数量 > 5 时用户需要滚动查找。开发者期待 ⌘K / Ctrl+P 风格的 fuzzy search（Warp / VS Code / Linear 都有）。这是 power user 留存的关键功能。

---

## 3. ADR 决策记录

### D-1: 不推倒重来，build on M22-M34 既有 token

**采**：所有改动都从既有 `Theme.colors / spacing / typography / motion / anatomy` 派生，**不引入全新 token 体系**。新增项是补充（如 `Code` role / `elevation_focus`），不是替换。

**理由**：M22-M34 累计 12 个 milestone 沉淀的 design system 是商业级底子，推翻成本高 + 风险高 + 也不必要。「不好」不是 token 不对，是**用法**不对（视觉层级、信息组织）。

### D-2: Phase A 全部改动控制在 `theme/` + `views/` 局部，不动 component

**采**：Phase A（v0.next）的 5 个改动都不改 `crates/aish-ui/src/components/*.rs`，仅：
- `theme/motion.rs` 改 `medium = 120ms`
- `theme/typography.rs` 补 `Code` role
- `theme/tokens.rs` 调 accent / border 色值
- `views/home.rs` 改 section label + 加 separator + 改名
- `views/sidebar_nav.rs` 加 label slot

**理由**：让 Phase A 风险最低、可独立 ship、effort < 2 天，给后续大改腾时间。

### D-3: Sidebar 升 220px 含「最近连接」是 Phase B 的 single largest change

**采**：把 Home 页里的「继续工作」列表升到 sidebar，sidebar 从 60px icon-only 升 220px 浅 nav + 最近连接列表。

**Trade-off**：
- ✅ 信息密度大幅提升，符合"开发者 daily-driver"定位
- ✅ Home 页腾出更多空间给「保存的主机」grid
- ❌ 全屏横向空间减少 160px — terminal viewport 减小约 12%
- ❌ 用户对 sidebar layout 有 muscle memory（之前 60px），改动后需要重新适应

**Mitigation**：sidebar **可折叠**（点 logo 收起到 48px icon-only）。默认展开，用户偏好持久化到 app_state.toml。

### D-4: ⌘K Command Palette 单独立项做 MVP，不和 layout 改动绑

**采**：CommandPalette MVP 范围限定：
- 全局 Ctrl+P / Cmd+P 触发
- fuzzy search **已保存的 hosts**（不做 commands / settings 索引）
- Enter 直接打开新 connection tab

**Trade-off**：MVP 不含「最近用过的命令」「Open settings」等 — 那些是 v1 范围，可后续渐进加。MVP 至少**解决 host > 5 个时的查找问题**。

### D-5: Light theme 决策 — 接受现状 + 加「实验性」标签

**采**：不投入工程量补完 7 个 light token，Settings 里 Light theme switch label 改成「Light（实验性）」+ tooltip 解释「部分色彩未完整调优」。

**理由**：SSH client 用户 95%+ 用 dark theme（与 IDE 习惯一致）。工程预算应投到 dark theme 的精细化。

### D-6: 单行 `user@host:port` 输入用解析 fallback 而非替换 4 字段表单

**采**：HostForm 顶部加一个新字段 `connection: TextInput`，placeholder `user@host:port`。`on_change` 时用 regex 解析：
- 成功 → 自动填到下方 user / host / port 字段（保留 4 字段可单独调）
- 失败 → 不报错，让用户继续在 4 字段表单填

**理由**：兼容**复杂场景**（user 含特殊字符、port 非数字 fallback）+ **新手友好**（保留 4 字段引导）。

### D-7: Inline ErrorState for SSH 连接失败 — 替换 terminal viewport 中央内容

**采**：当 `ConnectionState::Failed { kind, msg }` 时，terminal_view 不再 paint grid，改 paint 一个 ErrorState：
- icon: AlertOctagon
- title: 「连接失败」+ host 名
- description: msg (Code typography)
- actions: 3 个 Button：[重试连接] [编辑 host] [复制错误]

ConnectionChip + tab bar 仍保留（用户能切其它 tab）。

---

## 4. Risk 表

| Risk | 影响 | 概率 | Mitigation |
|---|---|---|---|
| Phase B sidebar 大改破用户 muscle memory | 用户首次开 v0.next+1 找不到 nav | 中 | sidebar 可折叠 + 默认展开 + 首次显示 "新 sidebar" toast 一次性 onboarding |
| Motion 时长降到 120ms 后某些动画显得"太快没看清" | 用户反馈 motion 不明显 | 低 | Phase A 落地后实测 1-2 天，若反馈差再回 150ms（改 1 个 token 值） |
| ⌘K palette 与 OS 快捷键冲突 | macOS ⌘K 部分 app 已占用 | 低 | 同时支持 Ctrl+P fallback；首次启动检测平台 |
| `TypeRole::Code` 字体在 Windows 没安装 fallback | 等宽字体显示成 proportional | 中 | bundled JetBrains Mono Nerd Font 已在 build.rs 嵌入，强制用 bundle font |
| Phase C "关于页" 需要新 page route | 当前 Settings 是单页，需加 sub-page | 低 | 简化为 Settings 内 Card + EmptyState 风样式，不开 sub-page |
| Inline ErrorState 替换 terminal viewport 中央 — 用户错以为「terminal 没了」 | 用户重启 app | 低 | ErrorState description 明确写「terminal 暂未启动」+ 保留 ConnectionChip 让用户能切回其它 tab |

---

## 5. 非目标 (Non-Goals)

明确**不做**的事，避免范围爆炸：

1. **不做暗黑紫 / 霓虹蓝 hero gradient** — 这是 marketing site 视觉，不是 daily-driver。
2. **不做 hover lift / scale up 卡片** — Linear / Vercel 都已放弃，现认为「过时」。
3. **不做 onboarding tour** — 开发者会 dismiss 它，且新手用 SSH 时已知道想干什么。
4. **不做插件市场** — 用户基数不支撑，SSH client 不是好的插件平台。
5. **不做 AI 命令提示**（仿 Warp）— 是另一个产品方向，独立立项不和 motion / layout 混。
6. **不做新主题色系** — accent / primary 是 OpenClaw 生态品牌色，保留。

---

## 6. 成功指标（如何判断 M35 完成）

定性（用户感知）：
- [ ] 打开 Home 第一眼能看出"继续工作"是首要操作（视觉层级 P-1 解决）
- [ ] sidebar 不再"看起来浪费"（P-2 解决）
- [ ] host > 10 个时能用 ⌘K 1 秒找到目标（P-13 解决）
- [ ] SSH 连接失败时知道下一步该做什么（P-11 解决）
- [ ] 整体视觉「年轻了 2 年」（综合）

定量（数字门禁）：
- [ ] cargo +nightly fmt --check ✅
- [ ] cargo +nightly clippy --workspace --all-targets -- -D warnings ✅
- [ ] cargo test --workspace: 全过（新增组件含 ≥ 3 个 pure-fn 测试）
- [ ] 大文件行数：Phase B 拆完后 home.rs / tab_bar.rs / terminal_view.rs 任一文件 < 800 行（如可能）

---

## 7. Open Questions

1. **CommandPalette 是否要单独立 spec？** 当前 P-13 + D-4 仅 sketch MVP 范围，但实际实施可能需要新 component + global key listener + result ranking 算法 — 工程量可能逼近 2 天，足够立独立 spec。**建议**：先在 Plan 的 Phase B 用一个 milestone 跑，若 effort 超 2 天 → 拆 spec。

2. **Sidebar 升级到 220px 是否需要 user opt-in？** 默认展开后老用户可能不喜欢。**建议**：默认展开 + 顶部一次性 toast 提示「点 logo 可折叠回紧凑模式」，偏好持久化。

3. **`TypeRole::Code` 用 JetBrains Mono 还是 SF Mono on macOS？** bundle 路径已含 JetBrains Mono Nerd Font。**建议**：直接用 bundled font 跨平台一致，不走 system fallback。
