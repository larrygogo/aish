# M27 — Component Anatomy 规范（Plan）

**Spec**: [`../specs/2026-05-15-aish-m27-component-anatomy-design.md`](../specs/2026-05-15-aish-m27-component-anatomy-design.md)

---

## File Structure

```
crates/aish-ui/src/theme/anatomy.rs                 (新文件 - 6 sub-struct + Default)
crates/aish-ui/src/theme/mod.rs                     (pub use anatomy)
crates/aish-ui/src/theme/tokens.rs                  (Theme 加 anatomy 字段)
crates/aish-ui/src/lib.rs                           (pub re-export Anatomy / 子结构)
crates/aish-ui/src/components/card.rs               (内置 padding + .no_padding() opt-out)
crates/aish-ui/src/components/dialog.rs             (改 anatomy.dialog.* token)
crates/aish-ui/src/components/toast.rs              (改 anatomy.overlay.toast_*)
crates/aish-ui/src/components/tooltip.rs            (改 anatomy.overlay.tooltip_*)
crates/aish-app/src/views/home.rs                   (T2 host card / active session row)
crates/aish-app/src/views/session_picker.rs         (T3 session row dense)
crates/aish-app/src/views/settings.rs               (T4 two_column_row / control_row + page anatomy)
crates/aish-app/src/views/host_form.rs              (T5 form gap)
```

---

## Tasks（顺序，每条独立 commit）

### T1: anatomy.rs + 6 sub-struct + Theme 字段

- 新文件 `theme/anatomy.rs`：
  - `pub struct Anatomy { card, dialog, list_row, form, page, overlay }`
  - 6 个 sub-struct + 各自 `impl Default`：
    - `CardAnatomy { header_px, header_py, body_px, body_py, footer_px, footer_py }`
      （Default: 16/10, 16/12, 16/10 — spec D-2）
    - `DialogAnatomy { header_px, header_py, body_px, body_py, footer_px, footer_py }`
      （Default: 16/12, 16/16, 16/12 — spec D-3）
    - `ListRowAnatomy { h_dense, h_comfortable, h_spacious, px_dense,
      px_comfortable, px_spacious, gap_dense, gap_comfortable, gap_spacious,
      py_dense, py_comfortable, py_spacious }`
      （Default: 26/32/40, 12/12/16, 8/8/12, 8/10/12 — spec D-4）
    - `FormAnatomy { field_gap, row_gap, inline_gap, footer_gap }`
      （Default: 12/12/4/8 — spec D-5）
    - `PageAnatomy { outer_px, outer_py_top, outer_py_bottom, section_gap,
      header_to_content_gap }`
      （Default: 32/24/24/16/16 — spec D-6）
    - `OverlayAnatomy { toast_p, toast_gap, tooltip_px, tooltip_py }`
      （Default: 12/12/8/4 — spec D-7）
- `theme/tokens.rs`: `Theme` 加 `anatomy: Anatomy` 字段 + `dark()`/`light()`
  填 `Anatomy::default()`（跨主题相同）
- `theme/mod.rs` + `lib.rs` re-export
- 单测覆盖：
  - `default_card_anatomy_values`：6 字段全断言
  - `default_dialog_anatomy_values`：6 字段全断言
  - `default_list_row_anatomy_monotone`：h_dense<h_comfortable<h_spacious
    + py 单调
  - `default_form_anatomy_values`：4 字段断言
  - `default_page_anatomy_values`：5 字段断言
  - `default_overlay_anatomy_values`：4 字段断言
  - `anatomy_shared_across_themes`：dark.anatomy == light.anatomy 字段相等
    （逐 sub-struct 比对）

**质量门禁**: fmt + clippy + test 通过；预计 aish-ui 211 → ~219（+8）。

---

### T2: Card 内置 padding + .no_padding() opt-out

- `components/card.rs`：
  - 加字段 `padding: bool`（默认 true）
  - 加 builder `.no_padding(self) -> Self { self.padding = false }`
  - `RenderOnce::render` 内：
    - 当 `self.padding == true`：
      - header slot `div().px(t.anatomy.card.header_px).py(t.anatomy.card.header_py)`
      - body slot `div().px(t.anatomy.card.body_px).py(t.anatomy.card.body_py)`
      - footer slot `div().px(t.anatomy.card.footer_px).py(t.anatomy.card.footer_py)`
    - 当 `false`：保持 M11 现状（无内置 padding）
  - 加 module 注释说明 anatomy 规则 + opt-out 场景（host card 自身 row 已 padded）
- 单测：
  - `padding_default_true`：`Card::new("a").padding == true`
  - `no_padding_chains`：`.no_padding().padding == false`

**质量门禁**: fmt + clippy + test 通过。**注意**：本 commit 不改 caller，
故 settings 三个 Card / home host Card 视觉**会变化**（突然多出 padding） —
T3 / T4 紧跟做 caller 适配。**临时视觉不一致**接受（task 链内）。

---

### T3: home host card / active session row 走 list_row.spacious

- `views/home.rs:553-560` host card body row:
  - `px_4 py_3 gap_3` → 改读 `anatomy.list_row.px_spacious /
    py_spacious / gap_spacious`（值 16/12/12 不变）
  - **关键**：host card 用 `Card::new(...).body(body_row).no_padding()`
    防 Card 内置 padding 与 body_row 自带 padding 双重叠加
- `views/home.rs:371-377` active session row（伪 list row）:
  - `px_4 py_2p5 gap_3` → `anatomy.list_row.px_spacious / py 改成
    anatomy.list_row.py_comfortable (10)` 让 active session 视觉略
    紧（不是 card 而是 row list） + `gap_spacious`
- page header section：`home.rs:290-293` 的 `px_8 pt_6 pb_3` →
  改用 `anatomy.page.outer_px / outer_py_top / header_to_content_gap`
  （值 32/24/16；当前 pb_3=12 调到 16 与 settings 对齐）
- HOSTS section 容器 `home.rs:671-674` 的 `px_8 pb_6 gap_3` →
  `anatomy.page.outer_px / outer_py_bottom / anatomy.list_row.gap_spacious`

**质量门禁**: 手测 home 整体视觉：host card padding 不双重 / active
section row 视觉略紧但不抖。

---

### T4: settings two_column_row / control_row / section_header + page anatomy

- `views/settings.rs:40-47` section_header（用作 Card header）：
  - `px_4 py_3` 删掉（Card 自带）；section_header 只剩 typography
    + border_b
  - **或者**改成 `Card::new(...).header(section_header(...))` 让 Card
    自带 padding 包住 section_header，section_header 只画 typography +
    text — 这样更简洁
- `views/settings.rs:56-61` two_column_row:
  - `px_4 py(10)` → 改读 `anatomy.list_row.px_comfortable /
    py_comfortable`（12/10；px 从 16 改 12 视觉**略变紧**，符合
    "settings row 是 comfortable" 语义）
  - **或保持 px=16**：caller 加 wrapper `div().px(card.body_px).py(...)`
    把 row 包在 Card body 自带 padding 内。**Plan 采后者**：Card body
    自带 px=16，row 自己只管 py，更符合"row 在 card body 内"语义
- `views/settings.rs:96-102` control_row 同样：删 px_4，仅 py(10)
  改 `anatomy.list_row.py_comfortable`
- `views/settings.rs:239-258` ScrollPage 改 `anatomy.page.outer_px /
  outer_py_top` + section 之间 gap 改 `anatomy.page.section_gap`（16
  不变）
- `views/settings.rs:130-133` page_title `pb_6` → `pb(anatomy.page.header_to_content_gap)`（16）
- Settings 三个 Card body 内 `flex_col` 容器**移除**自带 padding（之前
  每行自带 px_4，现 Card body 自带），改 caller 用 `Card::new(...).body(div()
  .flex_col().children(rows))` — Card body 自身已 px_4 py_3 包了

**质量门禁**: 手测 settings：appearance / shortcuts / about 三个 Card
视觉对齐，row 间无错位；page_title 与第一个 Card 之间 gap 16；Card 之间
gap 16。

---

### T5: session_picker row 走 list_row.dense

- `views/session_picker.rs:217-225` session row:
  - `px(spacing.px_3) py(spacing.px_2) gap(spacing.px_3)` →
    `anatomy.list_row.px_dense / py_dense / gap_dense`（值 12/8/8；
    gap 从 12 改 8 略紧）
  - 或保持 gap=12（视觉适合，可只接 px/py token，gap 单独）—
    Plan 采用 `gap_dense`（8）符合"dense 列表 gap 也紧"语义
- session row 容器 `gap(spacing.px_1)` 不动（rows 之间间隔不属 list_row anatomy）
- Dialog body 内嵌的 `py(spacing.px_4)` 空 state（L281）→ 改 Dialog body
  内空 state Body + 居中（不调 padding，让 Dialog body 自带 16/16 撑住）

**质量门禁**: 手测 session picker 弹窗：row 视觉紧凑（dense） + 整体
节奏与 dropdown_menu 一致。

---

### T6: host_form field gap 走 form anatomy

- `views/host_form.rs:452 / L479` 外层 column `gap(spacing.px_3)` →
  `anatomy.form.field_gap`（12 不变）
- `views/host_form.rs:543` field 内 column `gap(spacing.px_1)` →
  `anatomy.form.inline_gap`（4 不变）
- `views/host_form.rs:549 / L561 / L587` field 内 row `gap(spacing.px_3)`
  → `anatomy.form.row_gap`（12 不变）
- `views/host_form.rs:613` buttons row `gap(spacing_px_2)` →
  `anatomy.form.footer_gap`（8 不变）
- Dialog body 内嵌 form caller 不需要 padding（Dialog body 自带 16/16）
- 删 L579-L582 / L606-L609 的 `let spacing_px_3 = { let t = theme(cx); ... }`
  局部变量 hack，直接 inline `theme(cx).anatomy.form.row_gap`

**质量门禁**: 手测 host_form：字段视觉对齐（field_gap = row_gap = 12
但语义清晰）；error 内联在 field 内 inline_gap=4 紧贴 label。

---

### T7: Toast / Tooltip overlay anatomy

- `components/toast.rs:172-183`:
  - `.px(px(12.0))` / `.py(px(12.0))` → `.px(t.anatomy.overlay.toast_p)
    / .py(t.anatomy.overlay.toast_p)`
  - `.gap(px(12.0))` → `.gap(t.anatomy.overlay.toast_gap)`
- `components/tooltip.rs:49-50`:
  - `.px(t.spacing.px_2)` → `.px(t.anatomy.overlay.tooltip_px)`
  - `.py(t.spacing.px_1)` → `.py(t.anatomy.overlay.tooltip_py)`
- 单测：Tooltip / Toast 旧测试不变（视觉值都不变，仅命名）

**质量门禁**: fmt + clippy + test；Toast / Tooltip 视觉与之前完全一致。

---

### T8: 文档 + INDEX

- 更新 spec `2026-05-15-aish-m27-component-anatomy-design.md` 末尾"实施
  记录"节，填入 T1-T7 commits + 测试增量
- 更新 `docs/superpowers/INDEX.md`：
  - "当前状态" 更新到 "2026-05-15 完成 M27"
  - "Milestones" 顶部加 M27 entry，参考 M26 entry 格式
  - "下一里程碑" 候选改为 State design (empty/loading/error/skeleton)
    / icon stroke 统一 / input_bar anatomy 收尾

---

## Self-Review Checklist

- [ ] D-1 ~ D-9 每条决策都对应 task 实施
- [ ] R1 (Card 双 padding) 在 T2 (no_padding opt-out) + T3 (host card 使用)
      落地
- [ ] R3 (settings row 改后值不变看似无变) 在 T1 单测 + T4 手测 双重
      保险
- [ ] R5 (跨主题 anatomy 相同) 在 T1 `anatomy_shared_across_themes` 单测
- [ ] R9 (settings card 内 row 塌陷) 在 T4 显式说明删 row 自带 px_4
- [ ] T1 之后 Card 与 Dialog 在 caller 适配前会有"突然有 padding"现象 —
      已在 T2 commit message 中说明并紧跟 T3 / T4 修复
- [ ] T6 删除 host_form 内 `let spacing_px_X = {...}` hack 减少 boilerplate
- [ ] Out-of-scope 中 input_bar / tmux_sidebar 明确不动
- [ ] commits 严格按 task 顺序

---

## 实施顺序与依赖

```
T1 (anatomy token + Theme 字段) ────┐
                                    ↓
T2 (Card 内置 padding + no_padding) ── T3 (home 使用)
                                    ├── T4 (settings 使用)
                                    └── T5 (session_picker 使用)
                                          ↓
T6 (host_form form anatomy) ── T7 (Toast/Tooltip overlay anatomy)
                                          ↓
                                       T8 (文档)
```

**关键依赖**：T1 必须最先（其余 task 都要引用 anatomy token）。T2 必须
在 T3/T4/T5 之前（caller 改造依赖 `.no_padding()` opt-out 才能不双重
padding）。T3/T4/T5 之间互相独立可并行（不同 view）。T6/T7 与 T2-T5
独立。

**视觉回归窗口**：T2 commit 后，T3 commit 前的 codebase 状态下 host
card / settings card 会有"突然多出 padding"现象 —— 实践上把 T2 + T3
作为同一 commit（先改 component 后改 caller）也可，但 plan 拆开方便
review。**实施时按 plan 顺序逐 task commit，T2 commit 之后立刻跑 T3
commit 修视觉，中间不留 untested 状态过夜**。

---

## 工作量预估

| Task | 预估 | 复杂度 |
|---|---|---|
| T1 | 1.5h | 中（6 sub-struct + 8 单测） |
| T2 | 1h | 中（Card render 改 + 单测） |
| T3 | 1h | 中（home 两处 + page anatomy） |
| T4 | 1.5h | 高（settings 三 Card + page，最容易撞 R9 塌陷） |
| T5 | 0.5h | 低（session_picker 单处 row） |
| T6 | 0.5h | 低（host_form 4 处 gap） |
| T7 | 0.3h | 低（Toast/Tooltip 各一处） |
| T8 | 0.5h | 低（文档） |
| **合计** | **~7h** | 1 整天工作量（不含手测） |

---

## 预期测试增量

- aish-ui 211 → ~221（+10：Anatomy 6 子单测 + 跨主题 + Card padding 字段 2）
- aish-app 144 不变（view 改造无新单元逻辑）
- clippy 0 warning
