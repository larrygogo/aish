# M30-M34 + hover leave Motion 系统手测 Checklist

**目的**：实地验证完整 motion 系统：
- M30 (Dialog/Toast 入场) / M31 (Button press+focus) / M32 (Button/IconButton
  hover) / NavItem polish (sidebar hover) / M33 (Card hover for host card)
- M34 batch (detach-detect / SSH passphrase / TabItem entity)
- hover leave fade-out (5 组件反向 lerp 补完)

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

## 10. M34 batch — TabItem hover / detach-detect / SSH passphrase

### TabItem hover (top tab bar)

**前置**：至少 1 个 host 已连接，tab bar 有 connection tab

**操作**：mouse hover 非选中的 tab；切 active tab；mouse_down 任一 tab

**期望**：
- [ ] 非 active tab mouse 移入 → 150ms bg lerp（idle card → secondary_hover）
- [ ] 非 active tab mouse 移出 → 150ms 反向 lerp（**hover leave fade-out**）
- [ ] active tab 完全不响应 hover（selected 视觉稳态）
- [ ] mouse_down 任一 tab → 150ms 0.7→1.0 opacity press feedback

### detach-detect

**前置**：连进远端 host + attach 一个 tmux session

**操作**：在 tmux 内按 `Ctrl+B d`（detach prefix+d）

**期望**：
- [ ] tmux 输出 `[detached (from session XYZ)]` 到 raw shell
- [ ] aish sidebar 该 session 的 attached 标记**自动清除**（绿色 attached
  圆点变灰，与未 attached 状态视觉一致）

**之前行为对比**：用户 detach 后 aish 不感知，sidebar 仍显示"已 attach"
标记，需重新点 attach 才同步状态。

### SSH key passphrase

**前置**：用 OpenSSH 创建加密私钥（带 passphrase）：
```bash
ssh-keygen -t ed25519 -f /tmp/aish-test-key -P "mypass123"
ssh-copy-id -i /tmp/aish-test-key.pub user@yourhost
```

**操作**：HostForm add 模式 → 选 Key File auth → 填路径 `/tmp/aish-test-key`
→ passphrase 字段填 `mypass123` → Save → 点 host 连接

**期望**：
- [ ] HostForm 在 Key File 模式下显示 `passphrase` 字段（label "passphrase"，
  placeholder "passphrase (optional, for encrypted keys)"）
- [ ] passphrase 字段是 masked input（• 显示），右侧眼睛 toggle 可见
- [ ] Save 后 keyring 写入 `{host_id}-passphrase` entry（系统 keychain 可查）
- [ ] 连接成功（aish 用 passphrase 解密私钥 → russh 认证通过）

**测试未加密私钥**：passphrase 字段留空 → save → 连接 → 仍工作（fallback
路径：SecretStore::get_passphrase NoEntry → russh load_secret_key 传 None）

---

## 11. hover leave fade-out（**新**）

**M34 hover leave 反向 lerp 补完**：所有已 entity 化组件的 hover 状态
退出时不再是 instant 切回 idle，而是 150ms 反向渐变。

**操作**：依次在 5 个组件上 mouse-hover 然后 mouse-leave

**期望**：
- [ ] Home **+ 添加 host** Button：mouse leave → 150ms primary_hover →
  primary 反向渐变
- [ ] Host card **edit / X** IconButton (Ghost variant)：mouse leave →
  150ms 实色 → 透明 alpha 反向渐变（M32 v2 fix 同样适用于 leave 方向）
- [ ] Host card 整张：mouse leave → 150ms secondary_hover → card 反向
- [ ] Sidebar NavItem：mouse leave → 150ms 反向 fg + bg 双渐变
- [ ] Tab bar TabItem (非 active)：mouse leave → 150ms secondary_hover → card 反向

**快速 enter-leave-enter 边界测试**：
- [ ] mouse 移入 button 立即移出（< 50ms）：触发 Entering → Leaving 中断
  → instant Idle（D-1 v2 防视觉抖动 — 不走 leave 动画）
- [ ] mouse 移入 button + 等 150ms 完成 enter 后立即移出：触发 Leaving
  150ms 反向 lerp（完整 enter-leave 周期）

---

## 12. 边界情况测试

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
