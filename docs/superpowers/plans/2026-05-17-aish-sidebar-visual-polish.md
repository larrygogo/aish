# Plan：M35.1 Sidebar 视觉质感补强

> **里程碑代号**：M35.1（M35 视觉迭代的延续，不开新 milestone）
> **创建日期**：2026-05-17
> **状态**：待审核
> **关联**：M35 sidebar 视觉反复 5 次仍被判"丑"，参考 Warp / OrcaTerm / OpenSFTP 综合取长

---

## 1. Context

### 1.1 为什么再迭代

M35 主体完成后 sidebar 经历 v1-v5 重做（包括 shadcn-style 风），用户反复反馈"丑"、
"没审美"。此次决定**不再凭审美猜**，对照 Warp / OrcaTerm / OpenSFTP 三个产品的
**视觉质感** 取长补短，每条改动标明灵感来源 + 具体数值。

### 1.2 范围（仅视觉，不动信息架构）

**做**：bg 渐变 / item 尺寸圆角 / section header 风格 / host status dot / active state inset

**不做**：
- sidebar 信息架构改动（不动 Home/Terminal/Settings 3 nav）
- 新增 token（**复用** dark.rs 现有 token，必要时新增 1-2 个 token 也在 dark.rs）
- 动 motion 系统（保持 medium 120ms）
- ConnectionBar 相关（已在 d234fb0 删除）

### 1.3 为什么归 M35.1 而不是 M36

- 改动 < 100 行、单一主题（视觉 polish）
- 与 M35 T4-T6 同一系列工作（sidebar 视觉），归 M35 上下文更清晰
- 完成后在 INDEX.md M35 节追加 "follow-up: M35.1" 子条目

---

## 2. 设计决策（5 条改动 + 灵感来源）

### D1 — Sidebar bg 增加 elevation（OpenSFTP 借鉴）

**目标**：从纯色平面升到有"层次"的暗面

**实现**：vertical gradient `#0A0B0E → #08090A`（5% lift）

**alt 对比**：
- A. ❌ 加 right 1px border：割裂感太强，破坏 sidebar/main 一体感
- B. ✅ vertical gradient：眼睛说不出哪里变了但感觉"高级"
- C. ❌ 增加 box-shadow：不符合 dark theme 风格

**GPUI 实现路径**：GPUI 暂无 linear-gradient API；fallback 方案 = **2 层叠加 div**
（一个 #08090A 底 + 一个 vertical 5% white overlay 用 absolute 高度从 0 渐到 100%）。
**调研 task**：T0（实施前 30 分钟验证 GPUI 支持，无支持则退化到 D1-alt：sidebar bg
从 #08090A 改为 #0A0B0E 平色 + main bg 保持 #08090A，纯色但有 ΔL=2 elevation）

### D2 — NavItem 尺寸放大（OrcaTerm + shadcn 借鉴）

| 项 | 当前 | 改后 |
|---|---|---|
| radius | md (6px) | **lg (8px)** |
| 横向 padding | px_2 (8px) | **px_3 (12px)** |
| item 间距 | gap_1 (4px) | **gap_2 (8px)** |
| item 高度 | 32px | **36px** |

**为什么**："廉价感"主要来自尺寸过紧。OrcaTerm 留白宽松；shadcn sidebar 默认
height 36 + radius 8；Linear 同样 36 + 8。

**radius 选择**：lg=8 是项目现有 token，**不引入新 token**。

### D3 — Section header 重设计（Linear / shadcn 借鉴）

**「最近连接」当前**：~~13/500 Body 风~~ → 实际现状是 **11/500 MEDIUM + secondary_fg**
（plan 起草时描述有误，实施时核对代码确认为 11px MEDIUM secondary_fg）
**改后**：
- 字号：11（保持）
- 字重：MEDIUM(500) → **SEMIBOLD(600)**
- 颜色：`secondary_foreground` → **`muted_foreground`** (#8b8d97)
- ~~letter-spacing：0.5px~~ — **GPUI TextStyle 无 letter_spacing 字段**，无法实现；
  中文 "最近连接" 4 字短词本身不靠 letter-spacing 拉 hierarchy，weight + color 足够
- 上下 padding：pl_2(8) / pb_1(4)（保持）

**WCAG AA 验证**：muted_fg #8b8d97 在 sidebar 主 bg #08090A 上对比 ≈6.7:1 过 AA。
（旧代码注释里 muted 3.7:1 是 secondary 色块容器内的情况，sidebar bg 不适用。）

**字号风险**：当前 Typography 9 role 里 Micro 是 11/400，没有 11/600。
**方案选择**：
- A. ✅ **hardcode** `.text_size(px(11)).font_weight(FontWeight::SEMIBOLD)`（与 T7 home
  Caption 修红的 hardcode 模式一致）
- B. ❌ 新增 Typography role：M35 时已经 9 role，再加破坏简洁性
- C. ❌ 用现有 Micro：weight 不够，hierarchy 拉不开

**决定**：A。同时不写新 helper，inline 三行 inline。

### D4 — Host 行加 connection status dot（Warp 借鉴）

每个最近连接 host 名字**左边 6px 圆点**：

| 状态 | 颜色 | token 来源 |
|---|---|---|
| 当前 active ConnectionId 对应的 host | `success` (#4FBB72) | 现有 |
| 历史 host（无 active 连接） | `muted_foreground` 50% opacity | 现有 alpha |

**数据来源**：
- 已有 `AppState.connections: HashMap<ConnectionId, ...>` 含 host_id
- 已有 `recent_hosts: Vec<HostId>` 来自 `state.toml`
- 在 sidebar_nav.rs render 时按 `host_id` 对照 `connections` 判断 status

**为什么这条最有效**：纯静态界面 + 1 个色点 = 立刻"活"。Warp 全部"动感"都来自
status badge。

### D5 — Active state 改 inset glow（综合 shadcn + Linear）

**当前**：active = `secondary_hover` 全 fill（v5 shadcn 写法）
**改后**：

```
bg:    rgba(94, 106, 210, 0.10)   // primary 10% alpha
border: 1px solid rgba(94, 106, 210, 0.25)  // primary 25% alpha inset
icon color: primary #5E6AD2
text color: foreground #F4F5F8
```

**为什么**：fill 是低端做法，半透明 + inset border 是 Linear / Cursor / shadcn
（latest）的统一现代手法 — 比 fill 高 3 个等级。

**GPUI 验证**：`hsla(...).opacity()` 已在 M31/M32 stateful 组件用过，可行。
border 用 `.border_1().border_color(...)`。

### 决策汇总：取长来源对照

| 借自 | 借的什么 | aish 不变的（不补） |
|---|---|---|
| Warp | host status dot / 视觉"活物" | sidebar 信息架构（不动 Home/Terminal/Settings） |
| OrcaTerm | 留白 / 圆角放大 / 文字 hierarchy | Web app 风（aish 是 desktop，不抄渐变背景） |
| OpenSFTP | bg elevation 渐变 | 玻璃感（性能开销大，GPUI 也不擅长） |

---

## 3. File Structure

| 文件 | 改动 | 估算行 |
|---|---|---|
| `crates/aish-app/src/views/sidebar_nav.rs` | bg 渐变 / section header / status dot / active state 联动 | +50 / -20 |
| `crates/aish-ui/src/components/nav_item.rs` | size 放大 / radius / active state 重设 | +30 / -15 |
| `crates/aish-ui/src/theme/dark.rs` | 可能新增 `sidebar_bg_top` 1 个 token（D1 渐变需要） | +2 / -0 |
| `docs/superpowers/INDEX.md` | M35 节追加 M35.1 follow-up 子条目 | +10 |

**总计估算**：~80 行净变化、4 文件、1-2 commit。

---

## 4. Tasks（按依赖顺序）

### T0 — GPUI 渐变 API 调研（30 分钟）

**目的**：确认 D1 的实现路径
**做**：
- 检查 `gpui::div().bg(...)` 是否支持 `linear_gradient` 类型
- 跑 grep `linear_gradient` / `gradient` in zed/gpui deps
- 若不支持，确定 fallback（2-div 叠加 vs 退化纯色 ΔL=2）
**输出**：在 plan 文档注明 D1 最终方案 / 更新本 T0 结论

**质量门禁**：无（调研）

**结论（2026-05-17 完成）**：✅ **路径 A 可行 — GPUI 原生支持 linear_gradient**

- 源码：`gpui/src/color.rs:777` `pub fn linear_gradient(angle: f32, from: impl Into<LinearColorStop>, to: impl Into<LinearColorStop>) -> Background`
- 配套 `linear_color_stop(color, percentage)` 在 `color.rs:805`
- `div().bg(Background)` 全面接受（`style.rs:678` 在 paint_quad 时 dispatch `BackgroundTag::LinearGradient`）
- zed 内部 ≥10 处真实用例（agent_ui / ai_onboarding / thread_view 等）
- **angle 约定**：`0.0 = top`（指向上），CW 旋转 → **180.0 = bottom**（指向下，from 在 top）
- 颜色空间默认 sRGB；如需 OKLAB 可链 `.color_space(ColorSpace::Oklab)`

**T5 最终调用形态**：
```rust
use gpui::{linear_gradient, linear_color_stop};
.bg(linear_gradient(
    180.0,
    linear_color_stop(sidebar_bg_top,    0.0),  // #0A0B0E（5% lift）
    linear_color_stop(sidebar_bg_bottom, 1.0),  // #08090A（现有 sidebar bg）
))
```

dark.rs 仅新增 1 个 token `sidebar_bg_top: #0A0B0E`；现有 `sidebar_bg` 复用为底色。

### T1 — D2 NavItem 尺寸放大

**目的**：item 36px + radius lg + padding px_3 + gap_2
**改文件**：`nav_item.rs`
**改 sidebar_nav.rs**：调整外层 gap

**质量门禁**：
- `cargo +nightly fmt --all`
- `cargo +nightly clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

**commit**：`feat(sidebar): M35.1 D2 — NavItem 36px + radius lg + padding px_3`

### T2 — D5 Active state inset glow

**目的**：active = primary 10% bg + primary 25% border + primary icon/text
**改文件**：`nav_item.rs`

**视觉验收**：截图 active item 与未 active 区别明显但不喧宾

**质量门禁**：同 T1
**commit**：`feat(sidebar): M35.1 D5 — Active state inset glow（shadcn / Linear 风）`

### T3 — D3 Section header letter-spacing

**目的**：「最近连接」从 13/500 → 11/600 + letter-spacing 0.5px
**改文件**：`sidebar_nav.rs`

**质量门禁**：同 T1
**commit**：`feat(sidebar): M35.1 D3 — section header 11/600 letter-spaced`

### T4 — D4 Host status dot

**目的**：每个最近 host 名字左 6px 圆点（success / muted）
**改文件**：`sidebar_nav.rs`

**数据流**：
- read `app.connections` → `HashSet<HostId>` 收集 active host
- 遍历 `app.recent_hosts` 时检查是否在该集合中
- 是 → success；否 → muted_foreground 50%

**质量门禁**：同 T1
**commit**：`feat(sidebar): M35.1 D4 — host status dot（Warp 风 connection 指示）`

### T5 — D1 Sidebar bg elevation（依赖 T0 结论）

**目的**：sidebar bg 从纯 #08090A 升到 vertical gradient #0A0B0E → #08090A
**改文件**：`sidebar_nav.rs`（可能 `theme/dark.rs`）

**实施分支**（按 T0 调研结论）：
- 路径 A（GPUI 支持 gradient）：直接 `.bg(linear_gradient(...))`
- 路径 B（2-div 叠加）：sidebar 底层 #08090A + overlay div absolute 高度 100% 渐 transparent → 5% white
- 路径 C（fallback 纯色）：sidebar bg 改 #0A0B0E（ΔL=2 ≈ 比 main 略高 1 阶 "elevation"）

**质量门禁**：同 T1
**commit**：`feat(sidebar): M35.1 D1 — bg elevation gradient`

### T6 — INDEX.md 收尾

**目的**：在 M35 节追加 M35.1 follow-up 子条目，记录 commits + lesson
**改文件**：`docs/superpowers/INDEX.md`

**质量门禁**：无（文档）
**commit**：`docs(INDEX): M35.1 sidebar visual polish — 5 commits 完成`

---

## 5. Self-Review

### 5.1 风险

| 风险 | 等级 | 缓解 |
|---|---|---|
| GPUI 不支持 linear-gradient | 中 | T0 先调研，3 路径 fallback |
| host status dot 让 sidebar 变"嘈杂" | 中 | 6px 限定 / muted host 用 50% alpha 退至背景 |
| active state 半透明在 #08090A 上对比度不足 | 低 | 用 primary 10% 看不清的话提升到 12-15% |
| 11/600 hardcode 与 Typography 系统不一致 | 低 | 加 inline 注释说明（与 T7 hardcode 同模式） |
| 用户**再次**判"丑" | **高** | 每条改动**单独 commit** 可逐条 revert |

### 5.2 回退预案

每个 task 独立 commit，可针对性 revert：
- 用户嫌渐变奇怪 → revert T5
- 用户嫌 dot 嘈杂 → revert T4
- ...

### 5.3 Cross-component 检查（来自 T7 的 lesson）

- ✅ sidebar 折叠模式（48px）— 不显示最近连接 section，D3/D4 不影响折叠路径
- ✅ sidebar 展开模式（240px）— D1-D5 全部生效
- ✅ Tab bar / Connection chip — 不受影响（已在 d234fb0 解耦）
- ✅ Home 页 "Active Sessions" 列表 — sidebar 改动不影响

### 5.4 Pixel budget（来自 d234fb0 的 lesson "Earn every pixel"）

- D1 bg：**0 px 占用**（视觉层叠加）
- D2 size 放大：**每 item +4px** × 3 = +12px 占垂直空间（可接受，240px 高度宽裕）
- D3 section header：**+0 px**（只改字号字重）
- D4 status dot：**+10 px 水平**（6px dot + 4px gap），host name 区从 ~200px → ~190px，OK
- D5 active state：**+0 px**

---

## 6. Quality Gates 汇总

每条 task commit 前：
```bash
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

任一失败必须修后才能 commit。

---

## 7. Acceptance（怎么算"做完"）

- [ ] T0-T6 全部 commit
- [ ] 用户**视觉验收**通过（重点：D2 + D5 不再"廉价"，D4 让界面"活"了）
- [ ] INDEX.md 记录 commits + 关键 lesson
- [ ] 测试 559 → 559（不引入新测试，纯视觉改动）
- [ ] clippy clean

---

## 8. 不在范围（明确避免 scope creep）

- ❌ sidebar 信息架构改动（不动 Home/Terminal/Settings 3 nav 结构）
- ❌ 引入 motion 新动画（保持 medium 120ms）
- ❌ 抄 Warp vertical tabs 把 session 也搬进 sidebar（这是 M36 的题，不在此 plan）
- ❌ ConnectionBar 相关（已删，不复活）
- ❌ Light theme（M35 T17 已加实验性标签）
- ❌ 新 Typography role（hardcode 11/600 即可）

---

## Co-Authored-By

每 commit 末尾：
```
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```
