---
title: aish 视觉语言 Warp 风重做（brainstorm）
date: 2026-05-20
status: brainstorm
related-milestones: TBD（候选 M39+）
inspired-by: Warp.dev terminal
supersedes-direction: M24 Linear indigo + 准黑白 minimal（部分）
---

# aish 视觉语言 Warp 风重做

## 0. 目标

把 aish 从「**Linear indigo + 准黑白 minimal**」（M24 起的设计语言）演进为
「**Warp 风温暖 + gradient + half-glass**」 — 但**不替换** motion / typography /
anatomy / hover 子系统（这些跟视觉色相无关，已经成熟）。

**非目标**：

- 不重做 typography 9 个 type role / anatomy 6 类 / motion 4 档 / hover 三态色阶
- 不替换 Button / Card / Dialog / Toast 等 25 个 primitive 的 API
- 不破坏现有 view 结构（只改 ColorTokens + aurora 配色 + 圆角档位）
- 不直接抹除默认 dark（保留作为「素净 dark」选项，加 Warp variant 并列）

---

## 1. 现状盘点 + Warp 视觉特征解构

### 1.1 aish 当前视觉（M37 之后）

| 维度 | 现状 |
|---|---|
| 主 accent | `#5E6AD2` Linear indigo（冷紫蓝，中饱和） |
| Aurora 背景 | indigo + cyan 双冷色，opacity 0.18 / 0.14（低饱和、不易察觉）|
| Glass cards | `hsla(0,0,0.04,0.75)` 纯黑 75% 半透明，aurora 透出有限 |
| 圆角 | sm 4 / md 6 / lg 8 / full（保守，「小一档」原则）|
| Brand identity | 无 — accent 只是按钮色，不作 logo / hero 装饰 |
| 整体调性 | 「克制 / 冷峻 / dev tool 惯例」 |

### 1.2 Warp 视觉特征（凭印象 + 行业认知）

Warp 的视觉 DNA：

- **brand mark**：紫到粉的渐变 logo，整个产品 identity 围绕这条 gradient
- **accent**：紫色 + 粉色双色系（不是单 accent），紫做主要 / 粉做点缀
- **背景 aurora**：暖色 gradient 域，**饱和度比 aish 高 30-50%**，对比明显
- **terminal panels**：圆角较大（10-12px），背景半透明 + dark glass
- **command block**：每个命令一个圆角 block 视觉分隔（aish 没有此概念）
- **悬浮元素**（命令面板 / AI 抽屉）：强 glass + blur，浮层有明显存在感
- **typography**：保留等宽字体在 input，UI chrome 用 sans
- **整体调性**：现代 / 温暖 / 有 brand 存在感 / 不冷峻

### 1.3 关键差距

aish → Warp 风需要改的部分（按影响面排）：

| 差距 | 影响面 | 改造规模 |
|---|---|---|
| accent 单色 → 紫+粉双色 brand | 全局 | 大 |
| aurora 冷低饱和 → 暖高饱和 | 全局背景 | 中 |
| 圆角小一档 → 中一档 | 所有 card / button / dialog | 中 |
| 无 brand mark → gradient logo | hero 区 / sidebar 顶 | 小 |
| 缺「command block」视觉分隔 | terminal 区（如果做）| 大 |
| Glass opacity 0.75 → 0.5-0.6 | 所有 glass card | 小 |

---

## 2. 参考

- **Warp** (warp.dev) — 主要灵感来源
- **Raycast** — 另一个 modern brand-strong UI，但调性更冷
- **Arc Browser** — 极致 brand identity + spatial UI
- **Linear** — aish M24 当前路线，保守对照组
- **Cursor** — 接近 Warp 但更冷一档，Code 工具感

---

## 3. 核心决策点（ADR-style，每条列选项让用户选）

### ADR-001 主 accent 色 ⭐

**问题**：当前 Linear indigo `#5E6AD2` 是冷紫蓝，Warp 风需要更暖更紫。

| 选项 | 色值 | hue | 调性 |
|---|---|---|---|
| A 保留 indigo | `#5E6AD2` | 234° | 冷紫蓝，现状 |
| B Warp 紫 | `#7C5CFC` | 252° | 紫偏 magenta，更亮、更暖 |
| C Cursor 紫 | `#6E56CF` | 256° | 介于 A 和 B，温和过渡 |
| D 双色 brand | `#7C5CFC` + `#FF7B9F`（粉） | 252° + 343° | Warp 实际 brand mark 是双色 gradient |

**初步推荐**：**B**（单色升级），D 可作 hero / logo 专用 gradient。理由：单
accent 切换风险小，所有 Button primary / focus ring 一行改色就生效。D 留给后续
brand 强化阶段。

### ADR-002 Aurora 背景配色 ⭐

**问题**：当前 indigo + cyan 冷双色 opacity 0.18/0.14。Warp 风需要暖、饱和、明显。

| 选项 | 配色 | opacity | 调性 |
|---|---|---|---|
| A 保持冷双色 | indigo + cyan | 0.18 / 0.14 | 现状 |
| B 暖三色 | 紫 `#7C5CFC` + 品红 `#FF5CAA` + 琥珀 `#FFA85C` | 0.25 / 0.20 / 0.15 | Warp brand 三色 |
| C 紫+粉双色 | 紫 + 粉 `#FF7B9F` | 0.25 / 0.20 | 双色 brand，简化版 B |
| D 极致 Warp | 紫 + 粉，opacity 0.35 / 0.30 | — | 饱和度最高，最 brand |

**初步推荐**：**C**（紫+粉双色）。理由：三色容易花俏，C 跟 ADR-001 D 双色
brand 呼应；opacity 0.25 比当前 0.18 高一档但不爆，安全增量。

### ADR-003 Glass card opacity ⭐

**问题**：当前 `hsla(0,0,0.04,0.75)` —— 75% opacity 让 aurora 几乎透不过来。

| 选项 | opacity | 渗透度 |
|---|---|---|
| A 现状 | 0.75 | aurora 微弱 |
| B 中等 | 0.55 | aurora 明显 |
| C 强渗透 | 0.40 | aurora 主导，card 仅作前景容器 |

**初步推荐**：**B**（0.55）。理由：GPUI 无 backdrop blur，太透明会让 card
文字跟背景 aurora 冲突；0.55 是经验值（Warp 实际接近此值）。

### ADR-004 圆角档位 ⭐

**问题**：aish principles.md #1 明确「圆角比 consumer app 小一档」，Warp
风需要中一档。

| 选项 | 调整 |
|---|---|
| A 不动 | sm 4 / md 6 / lg 8 / full |
| B 全局 +2 | sm 6 / md 8 / lg 10 / xl 12 / full |
| C 仅 card + dialog +2 | card 8→10 / dialog 8→12，其他不动 |
| D Warp 极端 | sm 8 / md 10 / lg 14 / xl 18 |

**初步推荐**：**C**（仅 card + dialog 拉大）。理由：button 小圆角是 dev tool
精确感（保留），card / dialog 是「容器」可以软一档对接 Warp 风。B 全局 +2
改动面太大，D 太激进违反 principles.md。

### ADR-005 brand mark / hero 装饰 ⭐

**问题**：aish 当前 sidebar 顶部 logo 是单色 PNG，无 gradient brand mark。
Warp 用 gradient 作为 logo / brand 的核心识别。

| 选项 |
|---|
| A 保留单色 logo |
| B sidebar logo 加 gradient（紫到粉 conic / linear） |
| C 加 hero 区（Home / About 显眼位置）放 gradient brand block |
| D B + C 全套 |

**初步推荐**：**B + C 渐进**。先做 B（sidebar logo gradient，影响小），
验证后再决定 C。

### ADR-006 部署策略 ⭐⭐（最关键）

**问题**：直接替换默认 dark 风险大（用户审美差异），怎么演进？

| 选项 | 策略 |
|---|---|
| A 直接替换 | 默认 dark 立即变 Warp 风，midnight 保留作冷选项 |
| B 新增 variant | 加「Warp Aurora」作第三个 dark variant，跟 default / midnight 并列，用户选 |
| C 渐进迁移 | Phase 1 加 variant → 用户验收 → Phase 2 决定是否升为默认 |

**初步推荐**：**C**（渐进迁移）。理由：1-2 周大工程，先做 variant 不动默认
最安全，等用户跑一段时间，满意了再 Phase 2 升默认。midnight commit 34a931a
已经验证了「加 variant 不替换默认」的可行性。

### ADR-007 子系统是否动 typography / motion / anatomy / hover ⭐

| 选项 |
|---|
| A 不动（仅改 color + 圆角）|
| B 微调 anatomy padding 拉宽 |
| C 推翻重做（强烈反对）|

**初步推荐**：**A**（不动）。理由：这些子系统 M22-M37 调了 16 milestone，跟视觉
色相无关。Warp 风核心是「色 + glass + brand」，不是「layout 节奏」。

---

## 4. 实施分期建议

按 **ADR-006 推荐方案 C**：

### Phase 1 — Warp Aurora dark variant（不替换默认）

**周期**：3-5 天。**门槛**：低（基于 dark_midnight 模板）。**ROI**：高。

- [ ] 新文件 `aish-ui/src/theme/dark_warp.rs` — `Theme::dark_warp()` 构造器
- [ ] ColorTokens 全部按 ADR-001 / 003 调整（accent 紫 #7C5CFC / glass 更透）
- [ ] ThemeKind 加 `DarkWarp` variant（跟 DarkMidnight 并列）
- [ ] aish-app 启动支持 `theme = "warp"` 加载
- [ ] Settings 深色变体 Select 加第三选项「Warp Aurora」
- [ ] 测试 + 守护断言（is_dark / accent hue / saturation 等）

**用户验收**：跑 aish 切到 Warp Aurora，看是否符合期望。

### Phase 2 — Aurora 配色升级（按 ADR-002 推荐 C）

**周期**：1-2 天。**门槛**：中。

- [ ] app.rs aurora layer 1/2 配色按 ADR-002 调整
- [ ] 但 Aurora 是**全局**层，不能仅 Warp variant 用 — 需要让 aurora 配色
      跟当前 theme.kind 联动
- [ ] 加 `aurora_colors_for(kind: ThemeKind) -> AuroraColors` helper

### Phase 3 — 圆角拉大（按 ADR-004 推荐 C，仅 card + dialog）

**周期**：2-3 天。**门槛**：中（需调多个 view callsite）。

- [ ] anatomy.rs card.radius / dialog.radius 拉大
- [ ] 视觉验收 + 必要 polish

### Phase 4 — Brand mark gradient（按 ADR-005 推荐 B）

**周期**：1-2 天。**门槛**：低。

- [ ] sidebar logo 加 gradient overlay
- [ ] Hero / About 等显眼位置评估

### Phase 5（可选）— 升为默认

仅在 Phase 1-4 用户验收满意后。

---

## 5. Risk 表

| Risk | 严重度 | 概率 | mitigation |
|---|---|---|---|
| **审美 trade-off 跟用户不一致** | 高 | 中 | Phase 1 做 variant 不替换默认，验收后再决定 |
| **饱和度过高破坏「克制」原则** | 中 | 中 | ADR-002 选 C 不选 D；保留 default dark 选项 |
| **Aurora 全局层跨 theme 协调难** | 中 | 高 | 加 aurora_colors_for helper 跟 theme.kind 联动 |
| **半透明 card 文字可读性下降** | 中 | 中 | ADR-003 选 B 0.55 不选 C 0.40；测试 contrast ratio ≥ 4.5 |
| **圆角拉大破坏精确感** | 低 | 中 | ADR-004 选 C 仅 card+dialog 不全局 |
| **Logo gradient 显得 marketing** | 低 | 低 | Phase 4 验收，不满意可回退 |
| **多 variant 维护负担** | 低 | 高 | 接受 — variant 是 token 集合，无 runtime cost |

---

## 6. Open Questions

1. **Warp 实际配色具体值**：本 spec 凭印象列了紫 `#7C5CFC` / 粉 `#FF7B9F`，
   是否需要先用 Warp 截图采色？
2. **Phase 1 之后**是否给「Warp Aurora」做单独 Aurora 背景方案，还是用全局
   一致 aurora？（关系 Phase 2 是否分裂 aurora 配色）
3. **是否引入「command block」概念**到 terminal 区？这是 Warp 最强差异化但
   工程量极大（重写 terminal 渲染），不在本 spec 范围
4. **Light theme 是否同步做 Warp light**？还是 Light 留实验性不动？
5. **用户是否能接受**「设置 → 外观 → 深色变体」select 出现「Warp Aurora /
   Midnight / 默认」3 选 1？

---

## 7. 推荐结论（brainstorm 阶段）

**短期 1-2 周**：

1. **现在**：你审本 spec 七个 ADR，每条选你倾向的选项（A/B/C/D），有疑虑的标
   出来
2. **审完后**：根据你的选项写 implementation plan（`docs/superpowers/plans/`）
3. **实施 Phase 1**（3-5 天）：Warp Aurora variant 落地，你验收
4. **如果 Phase 1 OK**：继续 Phase 2-4；不 OK 调整

**中期 1-2 月**：

- 如果 Warp Aurora 受用户欢迎，考虑 Phase 5 升为默认
- 否则保留 variant 让喜欢 Linear 风的用户继续用默认 dark

**长期**：

- 「command block」（Warp 最大差异化）评估单独 spec，工程量太大不在本次范围

---

## 8. Next Step

如果你想继续推进，最自然的下一步：

1. **你回复 ADR 选项**（如：「001-B / 002-C / 003-B / 004-C / 005-B+C 渐进 /
   006-C 渐进迁移 / 007-A 不动」），或对某条提出第五选项
2. 我根据你的选项写 plan
3. 实施 Phase 1

或者：

- 你直接说「就按你推荐的全套来」，我用 spec 里所有「初步推荐」开 Phase 1
- 或者「停一下，先做点别的」，spec 留 brainstorm 状态等触发
