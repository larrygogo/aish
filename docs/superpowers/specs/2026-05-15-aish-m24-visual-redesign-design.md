# M24 — 视觉重塑：Warp / Linear 风商业感设计系统

**日期**: 2026-05-15
**父 spec**: [`2026-05-09-aish-ui-architecture-design.md`](2026-05-09-aish-ui-architecture-design.md)
**目标**: 把整体视觉从"终端绿 hacker 风"切到 Warp / Linear 风（准黑白 +
单 indigo accent），整体产品观感升级到 dev-tool 商业级标杆
**预计工程量**: 1-2 天，分阶段 commit

---

## 1. 动机

当前色板核心是终端绿（primary `#00CC33` + ring `#00E63A` + accent `#2F6E3E`
都是绿系）。"终端 hacker 风"在 M11 / M12 起步时合理 — 现在产品成型，目标
往商业级 dev tool 靠拢，绿色调与 Tabby / Termius / Warp / Linear 等成熟
SSH/dev 产品视觉差距大：

- 绿色 accent 在 sidebar / nav indicator / focus ring 上过亮，分散注意力
- Tokyo Night destructive `#F7768E` 偏粉淡，与"危险/警示"语义不匹配
- Card / popover / border 的灰阶分档不够细，多层 elevation 难以表达
- 圆角 lg=12 偏大，与 Warp/Linear 偏 hairline + small radius 风不符

参考产品（截图对比已 brainstorm）：
- **Linear**: 准黑底 + #5E6AD2 indigo accent + hairline border + 大量灰阶
  elevation
- **Warp**: 黑/暗灰底 + #6868EE indigo accent + 紧凑高密度 + glow focus

---

## 2. 决策记录（ADR-style）

### D-1: 设计基调 — Warp / Linear

**采**：参考 Linear / Warp 视觉语言：
- 准黑白 + 单 accent（indigo 系，非绿）
- 大面积中性灰阶 elevation，accent 仅在 CTA / focus / 关键状态
- 偏紧凑高密度（与 dev tool 用户期待匹配）
- hairline 1px border 普遍存在
- subtle shadow，多档 elevation alpha 极低

**拒**：保留终端绿（hacker 风与商业感冲突）；多色 accent（Tabby 风彩色装饰
与极简灰阶 minimal 冲突）。

### D-2: Accent 选 Linear Indigo `#5E6AD2`

**采**：dark primary = light primary = `#5E6AD2`（Linear indigo），dark/light
hover/active 各自走阶梯（dark 提亮 / light 加深）。

- `#5E6AD2` 是 Linear 主色，indigo 偏冷理性，与 dev tool 气质契合
- 同色覆盖 dark/light 减少配色复杂度（Warp 也是单色 accent）
- 状态色（success/warning/destructive）独立 token 不与 accent 混用

**拒**：Warp `#6868EE` —— 几乎同色（hue 差 < 5）但 Linear 更知名，统一记忆
点更强。

### D-3: Neutral 灰阶 ramp 细分（dark）

**采**：8 档灰阶覆盖 elevation，每档差距 ~3-5% lightness（Linear / Warp
分档实测）：

| 层 | 用途 | 当前 dark | 新 dark |
|---|---|---|---|
| L0 | bg 最底 | `#050505` | `#08090A` |
| L1 | card / panel | `#0D0D0D` | `#101113` |
| L2 | popover / dialog | `#161616` | `#191B1F` |
| L3 | input bg | `#0D0D0D` | `#101113`（=L1） |
| L4 | secondary / muted bg | `#1F1F1F` | `#26282D` |
| L5 | hover bg | `#2A2A2A` | `#2E3036` |
| L6 | active bg | `#404040` | `#3A3D44` |
| L7 | border / hairline | `#1F1F1F` | `#26282D`（=L4） |

文字色：
- foreground: `#E0E0E0` → `#F4F5F8`（更亮，dev tool 高对比惯例）
- muted_foreground: `#808080` → `#8B8D97`（hue 偏冷不再纯灰）
- secondary_foreground: `#BFBFBF` → `#C8CACF`

**拒**：完全黑 `#000` —— OLED 友好但与 popover/dialog 各层 elevation 区分不
开；`#08090A` 与 Linear 一致足够低亮度。

### D-4: Light theme — Linear 风浅灰

**采**：

| 层 | 用途 | 当前 light | 新 light |
|---|---|---|---|
| L0 | bg | `#FAFAFA` | `#FAFBFC` |
| L1 | card | `#FFFFFF` | `#FFFFFF` |
| L2 | popover | `#FFFFFF` | `#FFFFFF` |
| L4 | secondary | `#F1F1F1` | `#F3F4F6` |
| L5 | hover | `#E5E5E5` | `#E6E8EC` |
| L6 | active | `#D4D4D4` | `#D9DBDF` |
| L7 | border | `#E5E5E5` | `#E6E8EC`（=L5） |

文字：
- foreground: `#0A0A0A` → `#0D0E10`
- muted_foreground: `#737373` → `#6B6E78`（同 dark 反向 hue）

### D-5: 状态色（success / warning / destructive）desaturate

**采**：
- success: dark `#9ECE6A` → `#4FBB72`（Linear "Done" 绿，desat 一档）
- warning: dark `#E0AF68` → `#E8A658`（橙）
- destructive: dark `#F7768E` → `#E5484D`（Linear "Error" 红 — 真红不偏粉）
- light 镜像（success `#16A34A` / warning `#D97706` / destructive `#DC2626`）

**拒**：保留 Tokyo Night 高饱和（与整体 desat 风格冲突）。

### D-6: 圆角系统调整

**采**：

| Token | 当前 | 新 |
|---|---|---|
| sm | 4px | 4px（不变） |
| md | 6px | 6px（不变） |
| lg | 12px | 8px（更紧凑，Linear 风） |
| full | 9999 | 9999（不变） |

Card / Dialog 大量用 lg — 从 12px 收 8px 视觉更利落。Button / Input 仍 sm/md。

### D-7: Shadow elevation 多层 subtle

**采**：3 档 shadow（black alpha 极低 + 偏移小）：
- elevation-1（hover card）: `0 1px 2px rgba(0,0,0,0.08)`
- elevation-2（popover/dropdown）: `0 4px 12px rgba(0,0,0,0.25)`
- elevation-3（modal/toast）: `0 8px 24px rgba(0,0,0,0.4)`

dark 下 alpha 翻倍（黑底上 shadow 不显，加深让 elevation 可见）。

**拒**：单 shadow 全用 —— elevation 层级表达不出。

### D-8: Typography 不动 + 加密度

不引新字体（沿用系统 sans + monospace）。但收紧 line-height + padding：
- TextInput 行高 line_h 20px → 18px（更紧凑高密度）
- Card padding 12 → 10
- 表格 / 列表行高 28 → 24

**Out of scope** —— D-8 留 M25 评估，本次仅做色彩 / shadow / radius。

### D-9: Focus ring 颜色

**采**：focus ring = accent 色（`#5E6AD2`）但带 alpha 0.6 + 2px 厚 box-shadow
而非 1px border（Linear 风 glow），软光晕不抢眼。

---

## 3. 架构变化总览

```
+-----------------------------------------------------------------+
| Theme tokens 改动（仅色值，结构不动）                              |
|   ColorTokens: primary / accent / 各状态色全替                    |
|   Radius: lg 12→8                                                |
|   neutral L0-L7 ramp 重排                                         |
+-----------------------------------------------------------------+
| 组件视觉细节调整                                                   |
|   - Toast: shadow alpha 调整 + border_color desat                |
|   - Card: shadow 从无到 elevation-1                              |
|   - Popover/Dialog: shadow elevation-2 / elevation-3             |
|   - Button focus ring: accent + alpha 0.6                        |
+-----------------------------------------------------------------+
| 终端区视觉                                                         |
|   - 终端 bg = L0（与 app bg 同色）                                 |
|   - cursor 色 = foreground（不变，原本就对）                        |
|   - selection_bg = accent at alpha 0.3                            |
+-----------------------------------------------------------------+
```

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | 用户期待保留绿系 hacker 风 | 中 | 用户已明示 Warp/Linear 方向；保留 light theme accent 但减饱和 |
| R2 | accent 色变更影响所有 button / focus 视觉 | 中 | 自动 cascade — token 改 button 自动跟，无需逐组件调 |
| R3 | shadow elevation 在 light 下不显 | 中 | light alpha 翻倍 (~0.15 / ~0.30 / ~0.45) |
| R4 | 测试断言绿色 hue（如 destructive_hover_lighter_than_destructive）失败 | 低 | 改 token 后断言以新色重算；色彩单调性 lemma 不动 |
| R5 | 终端 ANSI palette 与新主题不匹配（user 终端绿可能跟 accent 撞色） | 低 | 终端 palette 是 VS Code Dark+ 独立色板，不受 theme accent 影响 |
| R6 | 一次性全改可能漏掉某处硬编码色（如 close 红 `#E81123`） | 中 | grep `0x[0-9a-f]{6}` 全 audit；hardcoded 警示色保留（titlebar close） |

---

## 5. Out of scope（M24 不做）

- 字体替换（D-8 推 M25）
- 终端 ANSI palette 调整（已是 VS Code Dark+，足够）
- icon 替换（Lucide stroke 风 OK，不动）
- 动画 / micro-interaction（hover transition 等）
- frosted glass / 透明效果（GPUI 不支持，且与 Linear 风不符）

---

## 6. 测试策略

### 单测（aish-ui）

- ColorTokens 单调性 lemma 自动适配新色值（hover.l > base.l 等不变）
- 加 token 视觉契约：primary.h ≈ indigo hue（~0.66 ± 0.05）
- shadow 数值 hardcode 测（提取 elevation constants 后断言数值）

### 集成（手测）

- 切到 dark theme 全屏检查每个 view（Home / Terminal / Settings / 各 modal）
- 切 light theme 重做
- 终端区 cursor / selection / 文字对比度
- focus ring on Button / TextInput 各种 variant

---

## 7. Plan 引用

见 [`../plans/2026-05-15-aish-m24-visual-redesign.md`](../plans/2026-05-15-aish-m24-visual-redesign.md)
