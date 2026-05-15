# M30-M33 Motion 系统手测 Checklist

**目的**：实地验证 M30 (Dialog/Toast 入场) / M31 (Button press+focus) /
M32 (Button/IconButton hover) / NavItem polish (sidebar hover) /
M33 (Card hover for host card) 的实际视觉效果。

**前置**：
```bash
cargo run --bin aish
```

无需准备远端 host — 大部分测试在 Home / Settings 页面就能验证。

---

## 1. M32/NavItem — sidebar 4-tab hover transition

**操作**：鼠标移到左侧 sidebar 的 Home / Terminal / Settings icon 上

**期望**：
- [ ] mouse 移入 → 150ms 内 bg + fg 颜色**平滑渐变**（不是瞬间切色）
  - idle: 灰色 icon，无 bg
  - hover: 稍亮的 fg + secondary_hover bg
- [ ] mouse 移出 → **瞬间**切回 idle（无 fade）
- [ ] 当前 active 的 sidebar tab（高亮 accent bg）hover **不变色**（active 视觉保持稳态）

---

## 2. M31 — Button press feedback (Home 顶部 + 添加 host)

**操作**：点击 Home 右上角 **+ 添加 host** primary button

**期望**：
- [ ] mouse_down 瞬间 button **明显变暗** (opacity 0.70，30% 变化)
- [ ] 150ms 内 ease-out 慢慢恢复回 opacity 1.0
- [ ] 即使 dialog 立刻弹出，按下瞬间的暗化**用户能看到**（M31 v2 修复点）

---

## 3. M32 — Button hover transition

**操作**：鼠标移到 **+ 添加 host** button 上

**期望**：
- [ ] mouse 移入 → 150ms bg 颜色 lerp（primary indigo → primary_hover 稍亮）
- [ ] mouse 移出 → 瞬间切回 idle
- [ ] Settings 页面里的 **打开配置目录** / **查看 GitHub** secondary buttons 同样行为

---

## 4. M33 — Host card hover transition + press feedback

**前置**：Home 添加至少 1 个 host（任意 label，host 不用真连接得通）

**操作 A** — host card hover：
- [ ] mouse 移到 host card 上 → 150ms bg lerp（idle card → secondary_hover）
- [ ] mouse 移出 → 瞬间切回
- [ ] hover 期间 edit (✏) / delete (X) IconButton 浮现（group_hover 透明度切换）

**操作 B** — host card press feedback：
- [ ] 点击 host card → 150ms opacity 0.7→1.0 暗化恢复
- [ ] 同时 host card 触发连接 actor 启动（看 tab bar 新增 tab）

---

## 5. M32 — IconButton hover + press (host card 内 ✏ / X)

**操作**：hover host card 让 ✏ / X 浮现后，依次 hover ✏ / X / press 它们

**期望**：
- [ ] IconButton hover 150ms bg lerp（**Ghost variant** 走 alpha-only — 从透明渐入实色，**无紫粉色 flash**，M32 v2 修复）
- [ ] press 0.7→1.0 opacity
- [ ] Edit 触发 HostForm modal；X 触发 delete confirm modal

---

## 6. M30 — Dialog 入场动画

**操作**：点击 **+ 添加 host** 或 host card 上的 ✏ 触发 HostForm modal

**期望**：
- [ ] Modal **150ms fade in**（backdrop + content opacity 0→1 ease_out_quint）
- [ ] **不是 instant 弹出**
- [ ] 关闭（Esc / 点 backdrop / 点 X）→ modal **fade out 150ms** 后 unmount

---

## 7. M31 — Focus ring fade-in (Dialog 内 button Tab 切换)

**前置**：HostForm modal 打开（add 或 edit）

**操作**：按 Tab 键在 label / host / port / user / keyfile / password input + Save / Cancel button 之间循环

**期望**：
- [ ] focus 切到 button (Save / Cancel / Delete) 时 ring **150ms 渐显** indigo glow
- [ ] focus 离开 button 时 ring **瞬间消失**（M31 D-3 simplify）
- [ ] focus 切到 TextInput 时 ring 渐显（input border + outline）

---

## 8. M30 — Toast 入场动画

**触发**：让 toast 显示 — 例如：
- 点 Settings 的 **查看 GitHub** button → toast "已复制：..." (或 url 打开成功的 toast)
- HostForm 输入空 label 直接 Save → toast 显示校验失败

**期望**：
- [ ] Toast 从右下角 **250ms slow opacity 0→1** fade in
- [ ] 多 toast 时 flex_col_reverse 自然下移，新 toast 出现在底部
- [ ] 3s 后自动消失（cleanup_expired），消失时 instant（M30 D-1 不做 exit 动画）
- [ ] X 关闭按钮 hover + press 同 M31/M32 反馈

---

## 9. reduced_motion 偏好测试

**操作**：进 Settings → Appearance → 打开 **减少动画** Switch

**期望（toggle ON 后）**：
- [ ] Dialog open/close **瞬间**显隐，无 fade
- [ ] Toast 出现**瞬间**完全可见，无 250ms 渐显
- [ ] Button hover **瞬间**切色，无 150ms lerp
- [ ] Button press **依然有 opacity 0.7 暗化**（press 是物理反馈，reduced_motion 不影响）
- [ ] Focus ring **瞬间**显隐
- [ ] Sidebar NavItem hover **瞬间**切色
- [ ] Host card hover **瞬间**切色

**关闭 Switch 后**：所有动画恢复。下次冷启 aish 偏好保留（app_state.toml）。

---

## 10. 边界情况测试

- [ ] **快速 hover-leave-hover** Button 5 次：hover_state 不卡顿，hover_anim_count 幂等 check 防 stale timer 工作
- [ ] **连点 Button 5 次**：press feedback 每次都播 + 不死锁（press_count++ 模式）
- [ ] **添加 5 个 host → 删除 3 个**：HomeView host_cards HashMap retain 同步清，无 entity 泄漏（可在 task manager 观察内存不爆涨）
- [ ] **dark / light 主题切换**：hover/press 渐变色按主题自适应
- [ ] **Ghost IconButton 在 dialog 内**（dialog close X / toast close X）hover 渐入 — **无紫粉色 flash**（M32 v2 修复）

---

## 已知不完整 / Defer 范围

| 内容 | 状态 |
|---|---|
| TabItem (top tab bar) hover transition | ⛔ Defer — 需 tab_bar render split |
| list row hover (session_picker row / active sessions row) | ⛔ Defer — 同 home split 模式 |
| Settings 3 Card 升 Entity | 📌 不做（无 on_click 无效） |
| hover leave fade-out | 📌 D-1 简化版不做 |
| Tab indicator slide | ⛔ M30 T6 Defer |

---

## 反馈

测后任何 motion 视觉问题（变化幅度 / timing / 视觉抖动 / 色阶不匹配）请记录到
后续 milestone polish 候选。
