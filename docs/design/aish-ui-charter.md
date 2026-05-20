# aish-ui Charter

> 这是 aish 视觉与交互设计的**执行手册**。对照 paseo design.md 的章节结构，
> 内容是 aish 自己的设计语言 — M22-M37 累积下来的具体做法 + forbidden 清单。
>
> 跟 [`principles.md`](./principles.md) 配套：principles 是**北极星 (为什么)**，
> charter 是**执行手册 (怎么做)**。决策时先回 principles，落地时查 charter。

---

## 1. Character / 调性

aish 是「**给独立开发者的深夜工作站**」。视觉一句话：

> **静、密、克制。配色低饱和，圆角小一档，零装饰。**

判断是否符合调性，问 3 个 yes/no：

- 字号 / 色块 / 圆角是否比 marketing site 低一档？
- 屏上每个像素能不能 justify「识别 / 操作 / 状态指示」之一？
- 这段动画是不是在回答用户的一个具体问题？

详细原则见 [principles.md](./principles.md)。

---

## 2. Token 系统总览

aish-ui 把设计 token 分 6 层（按抽象高度倒序）：

| 层 | 文件 | 内容 |
|---|---|---|
| **Theme** | `theme/dark.rs` / `theme/light.rs` | 27 个 color token 完整 dark / light 配色 |
| **Anatomy** | `theme/anatomy.rs` | Card / Dialog / List / Form / Page / Overlay 内部 padding / gap 规则（M27） |
| **Typography** | `theme/typography.rs` | 9 个语义 type role：size × weight × color_role（M26） |
| **Motion** | `theme/motion.rs` | 4 档 duration + 2 个 easing + `animate_or_skip` accessibility（M30）|
| **Tokens** | `theme/tokens.rs` | ColorTokens / Radius / Spacing / FontSize 原始定义 |
| **Components** | `components/*.rs` | 25 个 primitive：Button / Card / Dialog / Toast / Tooltip / ... |

**找东西规则**：
- 改颜色 → `theme/dark.rs` / `theme/light.rs`
- 改间距节奏 → `theme/anatomy.rs`
- 改字体层级 → `theme/typography.rs`
- 改动效时长 → `theme/motion.rs`
- 加新视觉元素 → 先翻 `components/`，**99% 已经存在**

---

## 3. Hierarchy / 层级表达

**aish 用 typography 9 个 role 表达层级，不靠手写 font_size / font_weight。**
所有 view 都走 `.typography(TypeRole::X, theme)` 一行 apply。

```
Display 层 — 页面标题 / Hero
  Title1   20/600/fg      Home / Settings 页面标题
  Title2   16/600/fg      Dialog 标题 / sidebar 分组标题
  Title3   14/600/fg      Card / section header / list group 标题

Structural 层 — 结构性标签
  Label    13/500/fg      form field label / 表头
  BodyStrong 13/500/fg    inline 强调 / 当前选中项

Content 层 — 内容正文
  Body     13/400/fg      列表项 / Card 内容正文 / Button 文字
  Caption  12/400/muted   meta 信息 / helper / 上次连接时间
  Micro    11/400/muted   快捷键 / badge / 角标 micro 文字
  Code     13/400/fg      inline code / 路径文字（JetBrains Mono）
```

**规则**：
- 「命名一个 surface 或 group」的文字 → Structural 层（Label / Title3）
- 「在 surface 或 group 内部」的文字 → Content 层（Body / Caption）
- 「在 page 顶部」的文字 → Display 层（Title1）

**foreground vs muted_foreground**：
- `foreground` 是被操作的对象（行标题、当前选中项）
- `muted_foreground` 是上下文（hint、二级 metadata、placeholder、未选中项）

详见 `theme/typography.rs` doc comment。

---

## 4. Buttons / 按钮 4 个 variant

aish 的按钮 primitive 是 `aish_ui::Button`，**4 个 variant**（不是 5 个）：

| Variant | 用途 | 颜色 |
|---|---|---|
| **Primary** | 一页 ≤1 个的主要 CTA | filled `primary` (indigo #5E6AD2) |
| **Secondary** | 默认 variant，与 Primary 配对 / 中性操作 | filled `secondary` (灰阶) |
| **Destructive** | 删除 / 退出 / 危险操作 | filled `destructive` (#e5484d) |
| **Ghost** | 结构性、非决策性（back arrow / header toggle / icon trigger） | 透明 + hover bg |

**注意**：aish **没有** Outline variant —— Ghost + 在 row 末尾的视觉位置已经覆盖
"低频 action 沿行尾"场景。如果需要带边框的低频 action，加 Ghost + `border_color(border)`，
不要单独搞 Outline。

**Sizes**：button 内部 size 走 `anatomy.button` 规则，caller 不手写 padding。

**Forbidden**：
- ❌ 自己写 `div().px_3().py_1_5().bg(...).hover(...).child("Save")` —— 用 `Button::primary("Save")`
- ❌ Primary + Secondary 同一行出现 ≥2 个 —— 视觉打架
- ❌ Destructive 直接出现在主页面 —— 只在 `Dialog` 内部 confirm 步骤

---

## 5. Borders / 边框三种用法

借鉴 paseo §5，aish 也是三种用法：

1. **Group rows in card** — Card 包一组相关 row，**一个** border 围着整组。Row
   内部用 `Separator` 分割，**不再加单独的 border**。
2. **Pane chrome** — TerminalView header / Dialog header 用 **底部一条 border**
   分隔头与内容。没有 shadow，只 1px 线。
3. **Outline emphasis（罕见）** — `border_color(primary)` 仅用于 focused input
   ring。其他场景禁用 primary 当 border 色。

**Forbidden**：
- ❌ 给一个单独的元素加 border（如果只有一个东西，要么用 Card，要么不要边框）
- ❌ shadow + border 同时用（用 `elevation_1/2/3` 时不再加 border）

---

## 6. Pickers / 5 种弹层 primitive

| Primitive | 文件 | 用途 |
|---|---|---|
| **Popover** | `components/popover.rs` | 锚定 trigger 的小 fixed 集合（≤8 项），无搜索 |
| **DropdownMenu** | `components/dropdown_menu.rs` | 同 Popover 但用 MenuItem 渲染 + 键盘导航 |
| **ContextMenu** | `components/context_menu.rs` | 右键 / 长按触发的隐藏菜单 |
| **Dialog** | `components/dialog.rs` | 全屏 backdrop + 居中卡片，多字段表单 / confirm |
| **Tooltip** | `components/tooltip.rs` | hover 触发的纯信息 hint（非交互） |

**选择树**：
- 3 个选项给固定 trigger → **Popover**
- 30 个选项需要搜索 → 暂无 Combobox primitive，**后续 milestone 触发再加**
- 「确定要删除吗」→ **Dialog** with Destructive button
- 一段说明文字 → **Tooltip**
- 右键 row → **ContextMenu**

**Forbidden**：
- ❌ 自己写 floating div 模拟弹层 —— 用上面 5 个 primitive
- ❌ Dialog 内嵌 Dialog 嵌套 —— 复杂流程拆步骤而不是叠 modal

---

## 7. Density / 节奏 (whitespace is the design)

aish 用 `anatomy.rs` 控密度，三级 rhythm：

| 层级 | 内部 spacing | 外部 spacing | 用 token |
|---|---|---|---|
| **Page** | 32px outer padding | — | `anatomy.page.padding` |
| **Section** | 24px gap between sections | — | `anatomy.page.section_gap` |
| **Card** | 16px inner padding，row 间紧贴只靠 Separator | 16px 卡片间 gap | `anatomy.card.padding` / `card.gap` |
| **Row** | 12-16px vertical / 16px horizontal | 0（卡片内）/ 8px（sidebar list） | `anatomy.list.row_padding` |

**核心规则**：
- **不能为塞内容压缩行高**。塞不下意味着更多 section，不是更小 row
- Card 内 row 间**不留 gap**，只一条 Separator
- Section 间留充足空白，不是装饰，是分割

**Forbidden**：
- ❌ `padding: px(10.0)` / `padding: px(20.0)` —— 不在 4px scale 内
- ❌ Card 内 row 之间加 margin —— 用 Separator
- ❌ section 自己设 margin_bottom —— 让父布局控 gap

---

## 8. Responsiveness / 响应式

aish **桌面 only**（macOS / Windows / Linux），不做 mobile / web。但 GUI 内有
两个 form factor：

| 维度 | 紧凑模式 | 标准模式 |
|---|---|---|
| **Sidebar** | 64px icon-only (折叠) | 220px 含「最近连接」list（展开） |
| **Tab bar** | 紧贴 sidebar 顶部，icon-only | 全宽，icon + label |
| **Terminal pane** | 全宽 | 全宽（terminal 永远是工作主区） |
| **Settings / Home** | 模态弹出（不影响 terminal） | 居中 720px 列 |

sidebar 折叠状态由用户控制（点 logo 区域 toggle，写盘 app_state.toml）。

**Forbidden**：
- ❌ 在 view 内用 `if window.width() < 800` 判断 —— sidebar 折叠是用户偏好，不是 width 判断

---

## 9. Copy / 文案风格（中文）

aish 是中文 UI，规则跟 paseo 英文 sentence case 不同，但「一致 + 简短」原则一致：

### 按钮：动词词组

| ❌ 别这么写 | ✅ 这么写 |
|---|---|
| 「连接管理」 | 「保存」「连接」「取消」 |
| 「数据导出功能」 | 「导出」「导入」 |
| 「删除主机选项」 | 「删除」 |

### Toast：「[主语]：[动作 / 状态] — [建议或下一步]」

```
✅ vps-1: 连接已断开 — 双击 tab 可重连
✅ home-mac: SSH 密码错误
✅ prod: 上传完成 — 3 张图片
❌ Error: Failed to connect            （不要英文 Error 前缀）
❌ 连接失败！                            （感叹号过多）
```

### Empty state：名词短语 + 可选 ghost button

```
✅ 「还没有保存的主机」    + [添加主机]
✅ 「该会话无快照」
❌ 「您还没有任何主机，请点击下方按钮添加」    （太长）
```

### In-flight：动词 + 「中...」

```
✅ 「连接中...」「保存中...」「下载中...」
❌ 「正在连接...」（多 1 字）
❌ 「Loading...」（不要英文）
```

### Error：直陈状态 + 可选恢复建议

```
✅ 「连接失败 — 双击 tab 重试」
✅ 「私钥读取失败 — 检查路径是否正确」
❌ 「抱歉，连接出错了」（不要道歉，直接陈述）
```

### 术语统一

| 概念 | 用 | 不用 |
|---|---|---|
| 远端机器 | 主机 / host | 服务器、机器、box |
| SSH 通道 | 连接 | session（session 留给 tmux）|
| tmux 会话 | session | 屏幕、会话窗 |
| 配置文件 | host 配置 | 主机配置文件、节点 |

**Forbidden**：
- ❌ 感叹号（「成功！」「错误！」）—— 平和陈述
- ❌ 句末多余的「了」（「保存了」→「已保存」，「连接了」→「已连接」）
- ❌ 中英混杂（「打开 Settings」→「打开设置」，除非「tmux」「SSH」这种专有名词）

---

## 10. States / 状态呈现

aish 的状态呈现已经成体系（M28 state design），4 类：

| 状态 | 用 | 文件 |
|---|---|---|
| **Page-level 通知** | `toast_info` / `toast_warning` / `toast_error` | `aish_ui::toast` |
| **Empty / Error 占位** | `EmptyState::new(id)` / `ErrorState::new(id)` | `components/empty_state.rs` |
| **Inline loading** | `Skeleton`（list 行）或 `Body + muted_fg + "加载中..."` 文字 | `components/skeleton.rs` |
| **Inline field error** | `InlineError::new("msg")` —— 字段下方单行 Caption + destructive | `components/inline_error.rs` |

**Disabled state**：所有 disabled 走 **opacity 50%**，绝不改色。
```rust
.when(disabled, |el| el.opacity(0.5))   // ✅
.when(disabled, |el| el.text_color(gray))  // ❌ 不要改色
```

**Forbidden**：
- ❌ disabled 改字色 / 改背景色 —— 用 opacity 50%
- ❌ 用 toast 显示 form 字段错误 —— 字段错误应该 inline 在字段下方
- ❌ Empty state 配大段引导文字 —— 1-2 行名词短语就够

---

## 11. List rows / 行 anatomy

`ListRow` (`components/list_row.rs`) 是统一的行 primitive。anatomy：

```
┌──────────────────────────────────────────────────┐
│ [leading]  primary line                  [trail] │  ← 16px py
│            secondary line (caption muted)        │
└──────────────────────────────────────────────────┘
            ↑                                ↑
            主行：Body 13/400/fg              chevron / kebab / switch
            副行：Caption 12/400/muted        / status badge / dot
```

**Trailing slot 用法**：
- **Chevron** = 导航（点行进入 detail）
- **Kebab (MoreVertical)** = 该行的 actions menu
- **Switch / Toggle** = 该行的状态切换
- **Status dot / Badge** = 该行的状态指示

一行可以同时有 chevron + kebab（kebab 在前 / chevron 在后），其他不要混。

**Selected state**：
- Sidebar list：`secondary` 背景
- Desktop list+detail：`secondary_hover` 背景（更亮一档）

---

## 12. Status indicators / 状态视觉

aish 已有 `Badge` (`components/badge.rs`)，5 个 variant：

| Variant | 颜色 | 用途 |
|---|---|---|
| **Default** | muted bg | 中性标签（如 tmux 版本号） |
| **Primary** | primary bg | 高亮标识（如 active 状态） |
| **Success** | success bg | 在线 / 已连接 / 已保存 |
| **Warning** | warning bg | 等待 / 鼠标未开 / 配置缺失 |
| **Destructive** | destructive bg | 错误 / 已断开 / 不可用 |

**Status dot** 模式：8px filled circle，颜色对应 status 色。配在 sidebar host
行 leading 位置，比 Badge 更紧凑。

**Forbidden**：
- ❌ 自己 `div().w(px(8.0)).h(px(8.0)).bg(some_color).rounded_full()` —— 用 `StatusDot` helper（若无则 PR 加）
- ❌ 用 emoji 当状态图标（✅ ❌ ⚠️） —— 用 Badge / Dot / icon

---

## 13. Forbidden / 不要这么干（aish 自己踩过的坑）

以下每条都对应一个真实踩过的坑（commit 引用在括号里）。**新代码请确认不在这个列表内**。

### Color / Theme

- ❌ **硬编码 hex 在 view 文件** —— 全部走 `theme(cx).colors.X`。需要新颜色加 `ColorTokens` 字段
- ❌ **disabled 改字色或 bg** —— 用 `.opacity(0.5)`（principles.md #3）
- ❌ **emoji 当 UI icon** —— 用 `IconName::*` (lucide) + Linux 发行版 SVG（principles.md #1）
- ❌ **状态色用饱和度过高** —— success/warning/destructive 已经 desaturate 一档，不要回填鲜艳色（M24 决策）

### Typography

- ❌ **手写 `.text_size(px(N))` / `.font_weight(FontWeight::MEDIUM)`** —— 用 `.typography(TypeRole::X, theme)`
- ❌ **`TypeRole::Body` 加 `.font_weight(...)`** —— 改用 `TypeRole::BodyStrong`
- ❌ **row title 用 medium weight** —— 内容层永远 normal，medium 是结构性标签（如 group label）专属

### Layout / Spacing

- ❌ **`.px(px(10.0))` / `.gap(px(20.0))` 等不在 4px scale 的值** —— 用 `theme.spacing.px_N`
- ❌ **Card 内 row 之间加 margin** —— 用 `Separator`
- ❌ **section 自己写 `.mb_8()`** —— 让父布局走 `anatomy.page.section_gap`

### Icon / Opacity

- ❌ **icon 大小硬编码 `px(14.0)` / `px(16.0)`** —— 用 `theme.icon_size.{xs/sm/md/lg/xl}`（12/14/16/18/20）
- ❌ **disabled 用 `opacity(0.5)` 硬编码** —— 用 `theme.opacity.disabled`
- ❌ **press 反馈用 `opacity(0.8)` 硬编码** —— 用 `theme.opacity.press`
- ⚠️ **视觉 overlay opacity（0.05 / 0.25 / 0.4 等）保持 view-level 硬编码** —— 这是 caller 决定的视觉效果，不归 token

### Animation / Motion

- ❌ **直接调 `.with_animation(...)`** —— 用 `animate_or_skip(theme, ...)` 包装，否则 reduced_motion 失效
- ❌ **bounce / spring / elastic easing** —— 用 `motion.easing_standard`（ease_out_quint）（principles.md #3）
- ❌ **超过 250ms 的微交互** —— 4 档 motion token 已覆盖，slow=250ms 是上限
- ❌ **stagger fade-in 入场（装饰性）** —— 动画必须回答用户问题，纯装饰禁用

### Interaction / Hover

- ❌ **`.on_mouse_move` 手动维护 hover 状态** —— 用 GPUI `.hover()` 或 `StatefulInteractiveElement`
- ❌ **hover 触发 lift / scale_up** —— Linear/Vercel 已弃，用 bg + border 表达
- ❌ **Button focus ring 用 `border_color(primary)`** —— 走 `ring` token + alpha glow（M15 决策）

### Component / Primitive

- ❌ **`div().bg(...).hover(...).child("Save")` 模拟 button** —— 用 `Button::primary("Save")`
- ❌ **自己写 floating div 做弹层** —— 用 Popover / DropdownMenu / Dialog / Tooltip / ContextMenu
- ❌ **bespoke status pill** —— 用 `Badge`
- ❌ **bespoke empty state** —— 用 `EmptyState::new(id).title("...").description("...")`

### Copy / Voice

- ❌ **感叹号 / 中英混杂 / 道歉式 error**（详见 §9）
- ❌ **toast 显示字段错误** —— 字段错误 inline 显示

---

## 14. Canonical surfaces / 经典面孔

| 模式 | 参考实现 |
|---|---|
| Home Launchpad（active 卡 + saved 卡） | `aish-app/src/views/home.rs` (M36) |
| Settings 页（720 居中 row list） | `aish-app/src/views/settings.rs` |
| Host 表单 Dialog | `aish-app/src/views/host_form.rs` |
| Sidebar workspace list | `aish-app/src/views/sidebar.rs` |
| Terminal pane + ConnectionChip | `aish-app/src/views/terminal_view.rs` |
| Session picker overlay | `aish-app/src/views/session_picker.rs` |
| Tab bar | `aish-app/src/views/tab_bar.rs` |
| Toast 通知 | `aish_ui::toast_*` API |
| Confirm Dialog | `aish-app/src/views/host_form.rs::confirm_delete` |

新功能要做 list+detail 时，**复制 Settings 的 shell**，不发明第三种布局。

---

## 15. 如何加新设计 token / primitive

按这个流程：

1. **先翻 `components/`** —— 99% 已经存在（25 个 primitive）
2. **不存在 + 用 ≥3 个地方** → 加 primitive
3. **不存在 + 用 1 处** → 直接写在 view 里，未来用 ≥3 处时抽
4. 加 primitive 时：
   - 在 `aish-ui/src/components/<name>.rs` 实现
   - `IntoElement` derive + Builder pattern (`Foo::new().variant_x().size_y()`)
   - 走 `theme(cx).colors.X` / `.typography(...)` / `.anatomy(...)` —— 不硬编码
   - 顶部 doc comment 写**用途 + 反例**
   - 单元测试（如果有可测的逻辑）
   - 在本文件 §14 加入引用

---

## 16. 关联文档

- [`principles.md`](./principles.md) —— 三条北极星原则（为什么）
- [`docs/capability-schema-rules.md`](../capability-schema-rules.md) —— host capability schema 演进规则
- `crates/aish-ui/src/theme/{tokens,typography,anatomy,motion}.rs` —— token 实现源码（每个文件顶部 doc comment 是权威）
- `docs/superpowers/specs/2026-05-20-aish-paseo-ui-borrowing-notes.md` —— paseo UI 美学借鉴笔记（本 charter 的灵感来源）
