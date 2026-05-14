# M24 — 视觉重塑（Plan）

**Spec**: [`../specs/2026-05-15-aish-m24-visual-redesign-design.md`](../specs/2026-05-15-aish-m24-visual-redesign-design.md)
**实施目标**: D-1 ~ D-9 落地，每条 Task 独立 commit

---

## File Structure

```
crates/aish-ui/src/theme/dark.rs           (色值重排)
crates/aish-ui/src/theme/light.rs          (色值重排)
crates/aish-ui/src/theme/tokens.rs         (radius lg 调 + 可选加 elevation 常量)
crates/aish-ui/src/components/toast.rs     (shadow elevation 调整)
crates/aish-ui/src/components/card.rs      (shadow 加 elevation-1)
crates/aish-ui/src/components/dialog.rs    (shadow elevation-3)
crates/aish-ui/src/components/popover.rs   (shadow elevation-2)
crates/aish-ui/src/components/button.rs    (focus ring alpha)
crates/aish-app/src/terminal/colors.rs     (selection_bg 用 accent at alpha)
```

---

## Tasks（顺序，每条独立 commit）

### T1: ColorTokens dark 重排（D-2 + D-3 + D-5 + D-9）

- dark.rs 全色值替换：
  - primary / primary_hover / primary_active 改 indigo 阶梯
  - accent / accent_active 改 indigo 阶梯（去绿）
  - background / card / popover / secondary / muted / border 改 neutral 灰阶 L0-L7
  - foreground / muted_foreground / secondary_foreground 调亮 + 冷调
  - destructive / destructive_hover / destructive_active 改 #E5484D 系
  - success / warning desaturate
  - ring 改 indigo
- 单测 hue 断言：primary.h ≈ 0.66 ± 0.05（indigo）；destructive.h < 0.05 || > 0.95（红）
- 现有阶梯单调性测试（dark_primary_hover_is_lighter_than_primary 等）自动适配
- 跑 fmt + clippy + test

**质量门禁**: fmt + clippy 0 warning + 测试 0 失败。

---

### T2: ColorTokens light 重排（D-4 + D-5）

- light.rs 全色值替换：与 dark 对称 + 反向阶梯（hover 加深而非提亮）
- 单测同样断言 hue
- 跑 fmt + clippy + test

---

### T3: Radius lg 12 → 8（D-6）

- tokens.rs `lg: px(12.0)` → `px(8.0)`
- 现有 callsite 用 `t.radius.lg` 的自动 cascade — 不用改 caller
- 视觉验证 Card / Dialog / Toast / 大圆角处仍美观
- fmt + clippy + test

---

### T4: Shadow elevation 系统（D-7）

- tokens.rs 加 elevation helper（可选 — pub fn shadow_elevation_2(theme_kind) -> Vec<BoxShadow>）
- 或者直接在各组件 inline shadow vec（更简单）
- Toast: 改成 elevation-3 alpha 4 / dark 翻倍
- Popover: elevation-2
- Dialog: elevation-3
- Card: 不默认带 shadow（保持 minimal），仅 elevated variant 用 elevation-1

---

### T5: Focus ring alpha glow（D-9）

- Button / IconButton focus ring：当前 1px box-shadow 用 colors.ring
  改 2px alpha 0.6 box-shadow（accent 色，Linear glow 风）
- TextInput border focus 已用 ring 色 — 改 ring border + 外加 1-2px alpha glow box-shadow

---

### T6: 终端 selection_bg alpha + 兼容性

- terminal/colors.rs selection_bg 改用 accent at alpha 0.3（之前 hardcode）
- 终端文字 cursor 色已是 default_fg 不动

---

### T7: 全应用视觉验证（手测）

- 切 dark 走一遍每个视图：Home / Terminal / Settings / 各 modal
- 切 light 同
- 截图对比前后（可选）
- 修发现的硬编码色 / 不一致细节

---

### T8: 文档 + INDEX 更新

- 更新 INDEX 加 M24 entry
- 更新 spec 末尾"已实现"标记
- 写 commits 汇总

---

## Self-Review Checklist

- [ ] D-1 ~ D-9 决策每条都对应 task（D-8 已 out of scope）
- [ ] Risk R1-R6 在 task 内有 mitigation 落地
- [ ] aish-ui 测试调整后仍全过（断言 hue / lightness 阶梯）
- [ ] 终端 ANSI palette 不动（D-1 决策外）
- [ ] hardcoded `#E81123` close 红保留
- [ ] commit 严格按 task 顺序，每 task 1 commit

---

## 实施顺序与依赖

```
T1 (dark tokens) ─┐
T2 (light tokens) ─┴→ T3 (radius) ─→ T4 (shadow) ─→ T5 (focus ring) ─→ T6 (selection) ─→ T7 (QA) ─→ T8 (文档)
```

T1/T2 可以一个 commit 完成（同质改动），或分两个让 git history 更细。
T4/T5 互相独立可并行。
