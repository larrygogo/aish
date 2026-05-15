# M26 — Typography × Information Hierarchy（Plan）

**Spec**: [`../specs/2026-05-15-aish-m26-typography-hierarchy-design.md`](../specs/2026-05-15-aish-m26-typography-hierarchy-design.md)

---

## File Structure

```
crates/aish-ui/src/theme/typography.rs           (新文件 token + trait ext)
crates/aish-ui/src/theme/mod.rs                  (pub use 新 token)
crates/aish-ui/src/theme/tokens.rs               (Theme 加 typography 字段)
crates/aish-ui/src/lib.rs                        (pub re-export TypeRole / Typography)
crates/aish-app/src/views/home.rs                (T2 page title + T3 card header + T4 host card)
crates/aish-app/src/views/settings.rs            (T2 page title + T3 section header + T5 form-like)
crates/aish-app/src/views/empty_terminal.rs      (T2 page title)
crates/aish-app/src/views/session_picker.rs      (T4 list item)
crates/aish-app/src/views/host_form.rs           (T5 form labels)
crates/aish-ui/src/components/dropdown_menu.rs   (T4 menu item label)
```

---

## Tasks（顺序，每条独立 commit）

### T1: typography.rs + TypeRole + ext trait

- 新文件 `theme/typography.rs`：
  - `pub enum TypeRole { Micro, Caption, Body, BodyStrong, Label, Title3,
    Title2, Title1, Code }`
  - `pub struct TypeStyle { size: Pixels, weight: FontWeight,
    default_color_role: ColorRole }`
  - `pub enum ColorRole { Foreground, MutedForeground, SecondaryForeground }`
  - `pub struct Typography { /* 9 个字段 */ }`
  - `impl Default for Typography { fn default() -> Self {
    /* spec D-1 表格值 */ } }`
- `theme/tokens.rs`:
  - `Theme` 加 `typography: Typography` 字段
- `theme/mod.rs` + `lib.rs` pub re-export `TypeRole / Typography /
  TypographyExt`
- ext trait：
  ```rust
  pub trait TypographyExt: Styled {
      fn typography(self, role: TypeRole, t: &Theme) -> Self;
  }
  impl<E: Styled + IntoElement> TypographyExt for E { ... }
  ```
  内部 match role + apply `text_size + font_weight + text_color`
- 单测：
  - `default_typography_size_monotone`：micro<caption<body=body_strong<
    title_3<title_2<title_1
  - `default_typography_weight_progression`：caption/body=400,
    label/body_strong=500, title_*=600
  - `code_uses_body_size`：code.size == body.size

**质量门禁**: fmt + clippy + test 通过。

---

### T2: page title 用 title_1 (3 view)

- home.rs: "Home" 标题 → typography(Title1, t)
- settings.rs: "Settings" 标题 → typography(Title1, t)
- empty_terminal.rs: 主标题 → typography(Title1, t)

**质量门禁**: 手测三页 title 视觉一致 + 比旧版 hierarchy 更清晰。

---

### T3: Card / section header 用 title_3

- settings.rs: section_header helper 用 typography(Title3, t)
- home.rs: HOSTS section label + Active Sessions label →
  typography(Title3, t)（之前是 muted_fg xs，过弱）

**质量门禁**: section header 视觉与 page title 拉开层级。

---

### T4: list item label 用 body / body_strong

- home.rs: host card label → typography(BodyStrong, t)（之前 xl semibold
  抢眼）；host_text → typography(Body, t)
- session_picker.rs: session name → typography(Body, t)；windows count /
  activity → typography(Caption, t)
- dropdown_menu.rs: item label → typography(Body, t)；shortcut →
  typography(Micro, t)

**质量门禁**: 列表视觉密度提升，labels 不抢眼。

---

### T5: form label 用 label role + footer/meta 用 caption

- host_form.rs: 各字段 label → typography(Label, t)
- settings.rs: two_column_row left (label) → typography(Label, t);
  right (value) → typography(Body, t)
- home.rs: "上次连接 X" / "未连接" → typography(Caption, t)

**质量门禁**: form 视觉对齐，meta 信息 muted 不抢眼。

---

### T6: 文档 + INDEX

- 更新 spec 末尾"已实现"
- INDEX 加 M26 entry
- 写 commits 汇总

---

## Self-Review Checklist

- [ ] D-1 ~ D-7 决策每条都对应 task
- [ ] Risk R1-R5 在 task 内有 mitigation 落地
- [ ] T1 内单测覆盖 size + weight 单调性
- [ ] T2-T5 每个 view 改造后视觉 hierarchy 明显加强
- [ ] 旧 FontSize 5 档保留作 fallback（不强制全删）
- [ ] commits 严格按 task 顺序

---

## 实施顺序与依赖

```
T1 (token + ext) ──┐
                   ↓
T2 (page title) ── T3 (section header) ── T4 (list item) ── T5 (form/meta)
                                                                ↓
                                                              T6 (文档)
```

T2-T5 互相独立可并行，但顺序由"用户最先看到 → 后看到"决定（page title
最重要）。
