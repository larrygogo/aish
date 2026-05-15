# M27 — Component Anatomy 规范

**日期**: 2026-05-15
**父 spec**: [`2026-05-15-aish-m24-visual-redesign-design.md`](2026-05-15-aish-m24-visual-redesign-design.md)
**前置 milestone**: M24 色彩 / M25 加密度 / M26 Typography hierarchy
**目标**: 定义 Card / Dialog / List row / Form field / TabBar / Toast / Tooltip
等核心组件的**内部 padding / gap / divider 统一规则**，消除 caller 各自手糊
导致的视觉不对齐
**预计工程量**: 1-2 天，T1 anatomy token + T2-T7 按组件分批改造

---

## 1. 动机

M24/M25/M26 完成了**色彩 / elevation / typography** 三层语义化，但
组件内部空间度量仍是各处 caller 散乱写死。当前 codebase audit 显示
明显不一致：

### 不一致样例（实测当前 codebase）

#### Card 组件 — 完全无内置 padding，全靠 caller

`crates/aish-ui/src/components/card.rs:128-130`：

```rust
el.when_some(self.header, |d, h| d.child(div().child(h)))
    .when_some(self.body, |d, b| d.child(div().flex_1().child(b)))
    .when_some(self.footer, |d, f| d.child(div().child(f)))
```

→ **每个 caller 自己决定 Card 内 padding**，于是：

- `home.rs:553-560` host card body: `px_4 py_3 gap_3`（16/12/12）
- `home.rs:371-377` active session row（伪 card）: `px_4 py_2p5 gap_3`（16/10/12）
- `settings.rs:40-47` section_header: `px_4 py_3`（16/12，**用作 Card header**）
- `settings.rs:56-61` two_column_row: `px_4 py(10)`（16/**10**，与 host card 12 不同）
- `settings.rs:96-102` control_row: `px_4 py(10)`（16/10）

→ Card header 是 12，但 Card row 是 10 — 同一卡片内 header 与 row 行高
不一致；host card 是 12（更紧），settings row 是 10（更松），**两者
应统一**。

#### Dialog 组件 — header 与 body 用不同 token，但内嵌写死

`crates/aish-ui/src/components/dialog.rs:218-244`：

```rust
.child(
    div()
        .px(t.spacing.px_4)  // 16
        .py(t.spacing.px_3)  // 12
        // header
)
.child(
    div()
        .p(t.spacing.px_4)   // 16/16 四向均匀
        .flex_1()
        // body
)
```

→ Dialog header py=12 / body p=16 是合理的（header 更紧，body 内容多
需要喘息空间）。**但写死 component 内**，caller 完全无法调整 — 例如
session_picker 在 body 内又包了一层 `div.flex_col.gap(px_1)` 让 row
间紧凑，等于 Dialog body 16px padding 上还内嵌 row gap 1px，**双重
spacing 没有规则**。

#### List row — 5 个不同来源 5 种不同度量

| 来源 | h | px | py | gap |
|---|---|---|---|---|
| `dropdown_menu.rs:91-96` | **26** | 12 | (h 控) | 8 |
| `session_picker.rs:217-225` | (auto) | 12 | **8** | 12 |
| `nav_item.rs:108-115` horizontal | **32** | 12 | (h 控) | 8 |
| `tab_item.rs:80-93` | **40** | 16 | (h 控) | 8 |
| `home.rs:553-560` host card row | (auto) | 16 | **12** | 12 |
| `home.rs:371-377` active session row | (auto) | 16 | **10** | 12 |
| `settings.rs:97-102` control_row | (auto) | 16 | **10** | - |

→ "list row" 在 codebase 有 **5 种 px 度量**（10/12/16）和 **3 种 h
固定值**（26/32/40），**根本没有统一规则**。Dev tool（Linear / Warp /
Stripe）共同 pattern 是 list row 三档：

- **紧凑（dense）**: h=24-28，适合密集导航 / dropdown
- **标准（comfortable）**: h=32-36，适合 sidebar / Tabs
- **松弛（spacious）**: h=40-44，适合 file list / card row

aish 当前 26 / 32 / 40 / auto 实际上**已经在用三档**，但没有命名 token
caller 也不知道何时用哪档。

#### Toast — 完全 hardcoded 不走 spacing token

`crates/aish-ui/src/components/toast.rs:172-183`：

```rust
.px(px(12.0))
.py(px(12.0))
.gap(px(12.0))
```

→ 12 是 `spacing.px_3`，但写死不接 Theme — dark/light 切换时 token 自身
不变，所以视觉无 bug，但**与项目"Theme token 唯一真相源"原则违背**。

#### Form field gap — 单 form 内 3 种 gap

`crates/aish-app/src/views/host_form.rs`：

- L452 / L479: form column gap = `spacing.px_3`（12，**字段之间**）
- L543: field 内 label-error column gap = `spacing.px_1`（4，**字段内**）
- L549 / L561 / L587: field 内 label-input row gap = `spacing.px_3`（12，**字段内 row**）
- L613: footer buttons gap = `spacing.px_2`（8）

→ 4 个不同 gap 在同一 form 内，**没有"field 内 vs field 间 vs footer"
的明确语义**。

#### Page outer padding — page 之间也不一致

| Page | header px | header py | content px | content py | gap |
|---|---|---|---|---|---|
| home.rs:290-293 | 32 | pt 24 / pb 12 | 32 | pb 24 | 12 |
| settings.rs:248-249 | (page 整体) | (page 整体) | 32 | 24 | 16 |
| empty_terminal.rs:41 | - | - | - | - | 12 |

home 与 settings 都用 32px 横边距（一致），**但 page header 与 content
之间的 gap**：home pb_3=12，settings pb_6=24（**page_title 自己 pb_6 而非容器**），
不一致。

---

### 后果

1. **视觉对齐失败** — Card 内 row 高度 10/12/16 三档跳变，眼睛能看到
   抖动；不同 view 看起来不像"同一个产品"
2. **caller 心智负担** — 写新 view 时要凭感觉挑 px_3 还是 px_4，没有
   统一参考
3. **修改难** — 想全局把 list 高度收紧 2px 要扫所有 caller，没有 single
   source of truth
4. **review 难** — code review 看到 `py(10)` vs `py_3` 无法判断是有意
   还是手抖

Linear / Warp / Stripe Dashboard 共同点：**anatomy 即组件契约**，每个
组件标注 header/body/footer 各自的 padding，row 高度按密度档分级，
form gap 按"字段间 vs 字段内"严格区分。

---

## 2. 决策记录（ADR-style）

### D-1: anatomy 作为 Theme 第五层 token

**采**：`Theme` 加 `anatomy: Anatomy` 字段（与 ColorTokens / Radius /
Spacing / FontSize / Typography 并列）。Anatomy 内分 6 个 sub-struct：

```rust
pub struct Anatomy {
    pub card: CardAnatomy,        // header/body/footer padding
    pub dialog: DialogAnatomy,    // header/body/footer padding
    pub list_row: ListRowAnatomy, // 三档 h / px / gap
    pub form: FormAnatomy,        // field_gap / row_gap / inline_gap
    pub page: PageAnatomy,        // outer_px / outer_py / section_gap
    pub overlay: OverlayAnatomy,  // toast / tooltip 内 padding
}
```

每个 sub-struct 都是简单数值 token（`Pixels`），不是 trait / fn。

**拒**：把 anatomy 散在各 component 内做常量（M24/M25 当前做法）—
caller 无法 introspect / 主题切换无法整体调；用 `.spacing.px_3` 直接
调用（caller 仍要选择 px_3 vs px_4，无语义）。

### D-2: Card anatomy — header py=10 / body p=16 / footer py=12 + 内置 padding

**采**：Card 组件**内置 padding**（M27 与 M11 设计反向），caller 不再
手糊 px/py：

| Slot | px | py | 备注 |
|---|---|---|---|
| header | 16 | **10** | 与 list row 标准档同 py，hierarchy 明确 |
| body | 16 | **12** | 内容主体，py 比 header 多一档 |
| footer | 16 | **10** | 与 header 对称 |

caller 仍可 `.no_padding()` builder opt-out（少数特殊场景，如 host card
自己 row 已 padding 不要再嵌一层）。**默认 padding 启用**。

**理由**：Card 是 anatomy 不一致最严重的源头（5 个 caller 5 种度量），
强制内置 + opt-out 才能根治。

**拒**：仍由 caller 控制（保持 M11 现状）— 不一致根因没解决；caller
传 `padding: Padding` 字段 — 太自由，仍可乱写。

### D-3: Dialog anatomy — 沿用现有 header px_4 py_3 / body p_4 但接 anatomy token

**采**：Dialog 已经内置 padding（M12 加），只是写死 `t.spacing.px_4`，
M27 改成 `t.anatomy.dialog.header_py` 等命名 token：

| Slot | px | py | 备注 |
|---|---|---|---|
| header | 16 | 12 | 现状 |
| body | 16 | 16 | 现状（四向均匀） |
| footer | 16 | 12 | 现状 + 新增显式 footer slot anatomy |

**Dialog body 之所以 16/16**（比 Card body 16/12 更松）：Dialog 是
modal 弹层，需要"主角感"，padding 更松强化层级。Card 是页面内卡片，
紧凑更好。**两者刻意不同，不是 bug**。

**拒**：Dialog body 也改 16/12 与 Card 对齐 — 失去 modal 的视觉重量。

### D-4: ListRow anatomy — 三档密度 token

**采**：`ListRowAnatomy` 提供三档 size：

| Density | h | px | gap | 用途 |
|---|---|---|---|---|
| `dense` | **26** | 12 | 8 | dropdown / context menu / 高密度命令面板 |
| `comfortable` | **32** | 12 | 8 | nav_item / Tabs tab / sidebar |
| `spacious` | **40** | 16 | 12 | TabItem / host card row / file list |

每档对应字段 `h_dense / h_comfortable / h_spacious` + 配对 `px_*` /
`gap_*`。当前 26/32/40 三档已是事实标准，M27 只是把它**命名 + 文档化**，
不改值。

**列表 row 例外**：高度 auto（如 host card 内多行内容）时 py 走对应档
的"等效 py"：

- dense → py_2（8）
- comfortable → py_2p5（10）
- spacious → py_3（12）

→ 把当前散乱的 `py(10)` / `py(12)` 收敛到 `anatomy.list_row.py_*`。

**拒**：单一 list row 度量 — 不同密度场景天然需求不同（dropdown 紧 vs
TabItem 松）；4 档以上 — 过度设计，业界 3 档够用。

### D-5: Form anatomy — 3 个 gap 语义化

**采**：`FormAnatomy` 三个明确 gap：

| Token | 值 | 用途 |
|---|---|---|
| `field_gap` | 12 | 字段之间（form column） |
| `row_gap` | 12 | 字段内 label-input row 之间 |
| `inline_gap` | 4 | 字段内 label 与 error 之间 / 字段内子元素 |
| `footer_gap` | 8 | 底部按钮 row gap |

field_gap 与 row_gap 都是 12 但语义不同（form 整体 column gap vs 字段
内 row gap），允许将来分离。当前值相同是设计意图，不是巧合。

**拒**：单一 form_gap = 12 — caller 无法区分语义；4 个以上 — 过度。

### D-6: Page anatomy — outer + section + header

**采**：`PageAnatomy`：

| Token | 值 | 用途 |
|---|---|---|
| `outer_px` | 32 | 页面横向边距（与 sidebar/content 主轴对齐） |
| `outer_py_top` | 24 | 页面顶部边距 |
| `outer_py_bottom` | 24 | 页面底部边距（之前 home pb_6=24 / settings pb_6=24，统一） |
| `section_gap` | 16 | section 之间 gap（settings card 之间） |
| `header_to_content_gap` | 16 | page title 与第一个 section 之间 |

home 与 settings 当前 outer_px=32 一致，**已经对齐**，M27 只是命名化。
header_to_content_gap 当前 home pb_3=12 / settings pb_6=24，**统一到
16**（M26 Title1 字号 20 已经够大，下面 16 缓冲足）。

**拒**：响应式 page padding（小窗口缩到 24）— 留 M28+ 响应式独立 milestone。

### D-7: Overlay anatomy — Toast / Tooltip / Popover 走 anatomy 不再 hardcoded

**采**：`OverlayAnatomy`：

| Token | 值 | 用途 |
|---|---|---|
| `toast_p` | 12 | Toast 四向 padding |
| `toast_gap` | 12 | Toast icon-content gap |
| `tooltip_px` | 8 | Tooltip 横向 |
| `tooltip_py` | 4 | Tooltip 纵向 |

Toast 当前 hardcoded `px(12) py(12) gap(12)` → 改 `anatomy.overlay.toast_*`。
Tooltip 当前用 `spacing.px_2 / px_1` → 改 `anatomy.overlay.tooltip_*`
（值不变，但语义更明确，方便将来调）。

### D-8: 改造范围 — 内置 + 渐进迁移

**采**：T1 加 anatomy token + Card / Dialog 内置 padding（强制变化）。
T2-T6 各 view 渐进改 — 每个 view 一个 task：

- T2: home host card / active session row → 用 anatomy.list_row.spacious
- T3: session_picker row → 用 anatomy.list_row.dense
- T4: settings two_column_row / control_row → 用 anatomy.list_row.comfortable
- T5: host_form field → 用 anatomy.form.*
- T6: Toast / Tooltip → 用 anatomy.overlay.*

视图组件本身（NavItem / TabItem / Tabs / dropdown_menu）当前度量已是
事实三档，T1 在 anatomy 内反向引用 component 内的值即可，**不强迫改 component**
（避免改坏现有视觉，单测过即可）。

**拒**：一次性强迫所有 component 都改 — 工作量大，回归风险高；只加
token 不改 caller — anatomy 形同虚设。

### D-9: aish-ui crate 不暴露 Anatomy 细节给 aish-app

**采**：`pub use aish_ui::theme::anatomy::{Anatomy, ListRowDensity, ...}`
通过 lib.rs re-export，caller `t.anatomy.card.header_py` 即可访问。
**不**暴露 sub-struct 内部字段的 setter（Theme 整体 immutable，要改
走 `Theme::dark()` / `Theme::light()` 内部）。

---

## 3. 架构变化总览

```
+-------------------------------------------------------+
| crates/aish-ui/src/theme/anatomy.rs (新增)              |
|   pub struct Anatomy { card, dialog, list_row,        |
|                       form, page, overlay }            |
|   pub struct CardAnatomy { header_px header_py        |
|                            body_px body_py            |
|                            footer_px footer_py }      |
|   ...类似 Dialog/ListRow/Form/Page/Overlay anatomy    |
|   impl Default for Anatomy (D-1~D-7 数值)               |
+-------------------------------------------------------+
| crates/aish-ui/src/theme/mod.rs                         |
|   pub use anatomy::*                                    |
+-------------------------------------------------------+
| crates/aish-ui/src/theme/tokens.rs                      |
|   Theme 加 anatomy: Anatomy 字段                        |
+-------------------------------------------------------+
| crates/aish-ui/src/components/card.rs                   |
|   render 内插 padding (D-2) + .no_padding() opt-out    |
+-------------------------------------------------------+
| crates/aish-ui/src/components/dialog.rs                 |
|   header/body padding 改读 t.anatomy.dialog.*          |
+-------------------------------------------------------+
| crates/aish-ui/src/components/toast.rs                  |
|   hardcoded px(12) 改 t.anatomy.overlay.toast_*        |
+-------------------------------------------------------+
| crates/aish-ui/src/components/tooltip.rs                |
|   t.spacing.px_2 改 t.anatomy.overlay.tooltip_*        |
+-------------------------------------------------------+
| crates/aish-app/src/views/*.rs                          |
|   各 view 按 list_row / form / page 改 token            |
+-------------------------------------------------------+
```

---

## 4. Risk 表

| ID | Risk | 严重度 | 缓解 |
|---|---|---|---|
| R1 | Card 内置 padding 破坏现有 caller 视觉（host card 自带 row padding） | 高 | `.no_padding()` opt-out + T2 改造时手测 host card 不双重 padding |
| R2 | Dialog body p_4 改 anatomy token 值不变但 hash 变 | 低 | 单测断言 `Anatomy::default().dialog.body_px == px(16.0)`；视觉无变化 |
| R3 | settings row 从 py(10) 改 list_row.comfortable.py(10) 看似无变 | 中 | 单测加 ListRowAnatomy 值断言；视觉 nochange 即成功 |
| R4 | host card row py(12) 改 spacious py(12) 看似无变；但 spacious 内含 px=16 与现 px_4=16 一致 | 低 | 数值都不变，仅命名 |
| R5 | dark/light theme 共享同 Anatomy::default() 是否合适 | 低 | anatomy 是几何度量与色彩无关；测试两 theme 切换布局不抖 |
| R6 | form 3-gap 语义新 caller 可能滥用 | 中 | spec D-5 表格清晰；新增 view code review 把关 |
| R7 | tmux_sidebar 已废弃但 px/py 混乱，是否要改 | 低 | 不改（已 archived），M27 不动 |
| R8 | input_bar hardcoded `px(8) py(4)` 等不一致是否纳入 | 中 | input_bar 是高定制 view（含 spinner / drag preview / 缩略图），M27 仅做主轴 anatomy；input_bar 留 backlog |
| R9 | 改 Card padding 后 settings card body 内嵌的 `flex_col` row（无 div padding）会"塌"到 Card 自带 16/12 | 中 | T4 改 settings 时手测 + 调整 — 移除外层 div padding，复用 Card 自带 |

---

## 5. Out of scope（M27 不做）

- input_bar 内部 padding 统一（高定制 view，留 backlog）
- tmux_sidebar 改造（已 archived）
- 响应式 padding（小窗口收紧）— M28+
- Card / Dialog Border-radius 统一（Radius token 已存在，本 milestone 不动）
- divider（border-bottom）颜色 / 厚度调整 — 已用 `border` token，不动
- 子组件（Button / IconButton / Badge / Checkbox / Switch）内部度量 —
  这些是 atomic 元素，M11/M14/M15 已定，M27 只做组合容器
- terminal_view 内的 input_bar / 终端 grid padding — 留独立 milestone
- 字体 line_height（M26 未做，M28+）

---

## 6. 测试策略

### 单测（aish-ui）

- `Anatomy::default()` 各字段值正确（数值断言全字段）
- `CardAnatomy::default().header_py == px(10.0)` / `body_py == px(12.0)`
- `DialogAnatomy::default().body_px == px(16.0)`（M11 现状不变）
- `ListRowAnatomy` 三档单调：`h_dense < h_comfortable < h_spacious`
- `ListRowAnatomy` 三档 py 单调：`py_dense < py_comfortable < py_spacious`
- `FormAnatomy::default()` 4 字段：field_gap=row_gap=12 / inline_gap=4 / footer_gap=8
- `PageAnatomy::default().outer_px == px(32.0)`
- `Theme::dark().anatomy == Theme::light().anatomy`（geometry 跨主题相同）

### 集成（手测）

- 切到 dark / light 看 home host card / settings card / dialog 是否对齐
- 改造前后截图对比，确认：
  - host card 内 row 不双重 padding（Card 自带 + caller 自包之前的 px_4 删掉）
  - settings card 内 row 视觉与 host card row "感觉" 一致（spacious 与
    comfortable 各自档明确）
  - dialog body 视觉与 M11 一致（值不变，仅命名）
- 全部组件单元测试 + clippy 0 warning

### 回归断言（assertEq 风格）

T1 token 加好后，跑前先记录基线（aish-ui 211 + aish-app 144 = 355）。
每条 T2-T6 task commit 后跑全测试 + clippy，net +N 单测应在 T1 处。

---

## 7. Plan 引用

见 [`../plans/2026-05-15-aish-m27-component-anatomy.md`](../plans/2026-05-15-aish-m27-component-anatomy.md)

---

## 8. 实施记录（待 plan 完成后回填）

T1 ~ T7 实施 commits 等待填入。

---

## 8. 实施记录（2026-05-15 完成）

T1 完整 + T2-T4 + T7 部分接入 + page anatomy 接入。T5/T6 (session_picker
dense list + host_form form anatomy) 推迟到 M29 host_form 重设计一起做
（避免与 M29 改动重叠）。

### 实际 commits

| Task | Commit | 内容 |
|---|---|---|
| spec + plan | `e12c967` | M27/M28/M29/M30 4 并行 spec 一波合 |
| T1 | `37ca0c4` | anatomy.rs + 6 sub-struct + Theme 字段 + 8 单测 |
| T2+T3+T4 | `ef82d75` | Card 内置 padding + .no_padding() + caller opt-out + 2 单测 |
| anatomy.page | `de7960f` | home / settings page padding 走 anatomy.page token |
| T7 | (本次) | Toast / Tooltip 走 anatomy.overlay |
| T8 | (本次) | INDEX + spec 实施记录 |

### 测试增量

- aish-ui 222 → **232** (+10 = 8 anatomy T1 + 2 Card padding)
- aish-app 145 不变

### Risk 实际命中

- **R1 双重 padding 叠加**：T2 加内置 padding 后 settings 3 个 Card +
  home host card 必须 .no_padding() opt-out 才不双 padding。已 mitigation。
- caller 适配工作量低于预期 — opt-out 模式比 plan 原版"section_header
  删 px_4 让 Card 自带"简单得多，零视觉回归。

### 未做（推迟到 M29 一起）

- T5 session_picker row 走 list_row.dense（涉及 row 数据流不动）
- T6 host_form 用 form anatomy.field_gap/row_gap/inline_gap/footer_gap
  （会被 M29 重设计覆盖，避免重复改）
