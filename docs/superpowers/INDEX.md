# aish Superpowers 索引

> 全部里程碑 plan + spec 的索引 + 当前状态 + 下一步候选。
>
> 每完成一个 milestone 更新本文件。这是 `claude-progress.md` 的替代品（后者已废弃）。
>
> **长期愿景** → [桌面版 Moshi Roadmap](roadmap-moshi-desktop.md)（跨多里程碑活文档）

---

## 当前状态

- **活跃分支**：main（2026-05-13 完成 M17-polish 一轮 35 commits 连续迭代，已 push origin/main）
- **下一里程碑**：M18 候选 — Button/IconButton Ghost variant 接 accent_active（M17 留的，兑现 M15 D-2 回退）/ ContextMenu（Popover + 右键）/ DropdownMenu 键盘导航 / Light theme 实施（含 M15/M17 共 7 个占位 token）/ Dialog Tab focus trap / TextInput "眼睛"切换 mask / TextInput shift+click 扩展 selection / TextInput 多行 / Disabled 状态视觉精细化
- **质量门禁基线**：fmt + clippy 0 warning + test (aish-ui **143** + aish-app 101 + 其他 crate) 全过

---

## Milestones（按时间倒序）

### M17-polish — UI / TextInput / 终端 / SSH 连续迭代（2026-05-12 ~ 2026-05-13）— ✅ 已完成
- 性质：不走 spec/plan 的用户反馈驱动 polish pass。35 commits 跨 9 个主题，每条独立 commit，本节做汇总索引便于回看。
- 主题 1：Tab inline rename 从无到有重做（10 commits，`d9ea7be` ~ `11d6f9a`）
  - 用 `aish_ui::TextInput` Entity 替代手糊 div+key handler（自动获得 IME / 中文 / 选区 / 复制粘贴）
  - 双击进入编辑 + select_all、Esc 取消、失焦自动 commit（与浏览器 inline rename 体感一致）
  - editing 时跳过 TabItem 单独渲染 240px 宽 inline editor（TabItem max_w(200)+overflow_hidden 会裁掉 input cursor）
  - TabItem mouse_down 抢 input 点击的根因：TextInput mouse_down 加 `cx.stop_propagation()` + handle_tab_click editing 同 id 入口 return 防御性兜底
  - 视觉外壳：editing tab box bg=input + 1px primary border + cursor_text，TextInput 用 borderless 不嵌套两套盒子
- 主题 2：Tab 横向溢出滚动 + < > 箭头（10 commits，`faa7418` ~ `dc713dd`）
  - Chrome / Edge 模式 < > 箭头，scroll 容器 GPUI ScrollHandle 精确 offset 同步
  - 多个 GPUI flex pitfall：`min_w(0)` 让 flex_1 真生效 + svg icon 自己设 text_color + max_offset 首帧 0 用 200ms 心跳 cx.notify
  - overflow_hidden 关掉 GPUI 内置 scroll，自己的 wheel handler 唯一接管（60px/tick）
- 主题 3：TextInput 水平 scroll + drag-select 完善（3 commits，`6d2d7b7` `3782eef` `1734ae4`）
  - 字段 scroll_offset + canvas prepaint callback 算 cursor_x vs viewport → 调 margin-left
  - drag-to-edge auto-scroll：30ms timer 在 drag 期间检测鼠标接近边沿，主动扩 cursor + 滚（鼠标停在边沿不动也会持续）
  - 全局 mouse_up 监听：`window.on_mouse_event::<MouseUpEvent>` 兜底 drag 拖出 input 外松开的场景
- 主题 4：终端颜色 palette VS Code Dark+ + bold→bright（1 commit，`e11b9ff`）
  - 旧 Tomorrow Night palette 在纯黑底上偏灰白，bright 系与 normal 相同看不出差
  - 新 palette：每 bright 都明显亮于 normal；加 `bold_promote()` 把 normal 色升级到 bright，grid_renderer 在 `cell.flags.BOLD` 时调用 → Ubuntu PS1 `\033[01;32m` 鲜亮绿正确显示
- 主题 5：终端 wheel 滚动速度（3 commits，`ee44c46` `e578448` 等）
  - 本地 alacritty 滚动 cap ±3 行/tick；SGR mouse 模式只发 1 个 event 让 tmux 决定步长（避免 ×3×5 = 45 行/tick）
- 主题 6：Tab 动态标题 OSC 0/1/2 + 手动 rename lock（1 commit，`d7a1207`）
  - 远端 escape sequence 改 tab title 实时同步；用户手动重命名后锁定不再被远端覆盖
- 主题 7：SSH idle disconnect 修复（1 commit，`9f75961`）
  - 误设 `inactivity_timeout: Some(1h)` 让 client 自己关连接；改回 None + 加 30s keepalive
  - UI 翻译：'协议错误'→'会话异常'；msg 含 'Disconnect' 时改'连接已断开 — 双击 tab 可重连'
- 主题 8：连接 phase 可视化 + 断开重连（1 commit，`0a47a7e`）
  - 连接中转圈反馈 + Disconnected 后允许双击 tab 重连
- 主题 9：Logo / Titlebar polish（2 commits，`2cee52f` `7123418`）
  - logo 换项目真品牌 SVG（取代 Nerd Font `>_` 字符），用 PNG + img() 保留多色（svg() monochrome 渲染丢色）
  - host-form 删 footer Cancel（Dialog 顶部 X 已经有）
  - ring / titlebar / settings 细化（`412c33a`）
- 测试：未新增（这一轮全是 UI/UX/integration 修复，不引入新单元逻辑）；aish-ui 143 / aish-app 101 不变
- 关键技术 lessons（沉淀供未来 milestone 参考）：
  - **GPUI flex `min_w(0)` pitfall** 这轮出现 3+ 次（TabItem title ellipsis / tab_bar scroll 容器 / root main flex_1）：flex item 默认 `min-width: auto` 拒绝 shrink，必须显式 `min_w(px(0.0))`
  - **GPUI svg() 是 monochrome** 必须自己 `.text_color()` 设；父 div text_color 不会 inherit
  - **GPUI ScrollHandle 符号**：`offset.x ∈ [-max_offset.x, 0]`，负数 = 向右滚；`max_offset.x ≥ 0`
  - **GPUI mouse_up 不全局**：element 内 `on_mouse_up` 鼠标拖出去松开收不到，需要 `window.on_mouse_event::<MouseUpEvent>` 兜底
  - **borderless input 视觉规则**：当 input 嵌在带 bg/border 的卡片里时用 borderless（让父卡片当外壳），否则用默认 bg/border 让"是个 input"明确

### M17 — aish-ui Card / NavItem / TabItem hover 改造 + accent_active token（2026-05-12）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-12-aish-m17-card-nav-tab-hover-design.md`](specs/2026-05-12-aish-m17-card-nav-tab-hover-design.md)
- plan：[`plans/2026-05-12-aish-m17-card-nav-tab-hover.md`](plans/2026-05-12-aish-m17-card-nav-tab-hover.md)
- 范围：
  - ColorTokens 加 `accent_active` 字段（M15 D-2 决策正式回退）
  - Dark theme accent_active = #4a7099（比 accent #6c91c2 更深 / lightness ~45% vs ~56%；与 M15 系列变亮方向**相反**，因 accent 系列是容器 hover 不是 action）
  - Light theme TODO 注释加 accent_active 字段名（M16 不涉及 ColorTokens 的事实也已澄清）
  - Card on_click 路径加 `.active(accent_active)` mouse-down 反馈
  - NavItem active=false 路径 hover 补 bg(accent)（与 Card/TabItem 视觉一致）+ `.active(accent_active)`
  - TabItem active=false 路径加 `.active(accent_active)`
  - NavItem / TabItem selected (active=true) 路径完全不动（保持现有 indicator 条 + bg）
- 关键 commits：
  - `a25cdeb` — T1 ColorTokens +accent_active + Dark 填值 + Light 占位 + 1 个 lightness 反向断言（amend 修 light.rs 误引 M16）
  - `9e80471` — T2 Card on_click 加 .active(accent_active)
  - `b556d2c` — T3 NavItem hover 补 bg + .active() + hover_only_when_inactive 测试
  - `eb9ff47` — T4 TabItem .active() + hover_only_when_inactive 测试
- 测试：aish-ui 121 → **124**（净 +3：dark.rs +1 / nav_item.rs +1 / tab_item.rs +1）；aish-app 101 不变
- 命名 namespace 澄清：token 层 `_active` = "pressed"（GPUI `.active()` modifier）；组件 API 层 `.active(bool)` = "selected"。两个 namespace 在代码中不交叉
- 已知边界：
  - Button / IconButton Ghost variant **未同步**接 accent_active，仍走 hover=accent / active=accent（无区别）。M17 不动，留 M18+ 兑现
  - NavItem hover 补 bg 后 sidebar 4 项视觉较前更"重"，手测后若问题严重可降级到 secondary token
  - selected NavItem / TabItem hover/按下时无视觉变化（D-5 决策内在 trade-off）
  - Light theme 7 个新 token 仍占位，真正色值留下个 light theme milestone

### M16 — aish-ui TextInput mask + cursor_at_pixel + drag select（2026-05-12）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m16-textinput-mask-cursor-design.md`](specs/2026-05-11-aish-m16-textinput-mask-cursor-design.md)
- plan：[`plans/2026-05-11-aish-m16-textinput-mask-cursor.md`](plans/2026-05-11-aish-m16-textinput-mask-cursor.md)
- 范围：
  - TextInput.mask_char(Option<char>) builder + is_masked() 查询（默认 None；HostForm password 用 Some('•')）
  - mask 启用时 render 把字符替换为 mask_char 显示
  - mask 启用时 copy()/cut() 静默返回 false（系统密码框惯例）
  - cursor_for_display() helper 把原文 byte offset 映射到 displayed_text byte offset（mask 字符与原文 char 字节宽度不同时需要重算）
  - render 改逐字 wrap div（每字一个 inline div 含 zero-size canvas 在 prepaint 写入 bounds_map）
  - byte_offset_at_x(bounds_map, click_x, text_len) 纯函数（char 中线作分界；空 map 返回 0，超末尾返回 text_len）
  - on_mouse_down 通过 byte_offset_at_x 算 byte 替代 M11 简化版的 text.len()
  - is_dragging 字段 + on_mouse_move 在 dragging 时持续更新 cursor + on_mouse_up 清状态：mouse_down 设 selection_anchor（沿用 handle_mouse_down_at），drag 期间只动 cursor，anchor 不变，selection_range() 自然形成
  - glyph_div(byte, ch, weak, sel, accent) 关联函数：消除 left/right 两段同构 map 重复
  - HostForm password 字段切到 .mask_char(Some('•'))
- 关键 commits：
  - `8801f94` — T1 mask_char + render 替换 + copy/cut 禁用 + cursor_for_display（amend 含字段行内注释）
  - `ac9b8b0` — T2 cursor_at_pixel：bounds_map + byte_offset_at_x + render 重写为逐字 wrap（amend 含时序注释 + width==0 注释）
  - `bfce36f` — T3 drag select + glyph_div helper 抽取（amend 含 anchor 注释 + mouse_up cx.notify）
  - `520ab3d` — T4 HostForm password 字段 .mask_char('•')
- 测试：aish-ui 110 → **121**（净 +11：mask +4 / byte_offset_at_x +4 / drag state +3）；aish-app 101 不变
- 后续修复（2026-05-12 下午一次性合入 main，cherry-pick 后 SHA 重写，下列原 SHA 见 reflog）：
  - `70885dc` 补 Ctrl+V 键盘粘贴 —— M16 漏掉、fallback `!ctrl && !alt` 守卫把 Ctrl+V 也吞了。加 paste() + compute_paste_payload（多行截到首行）+ 7 个单测；masked 状态允许 paste（密码框惯例）。
  - `aa6da68` 修字符双输入 —— M16 render 改逐字 wrap div 后，IME canvas (prepaint 注册 InputHandler) 稳定触发 WM_CHAR → replace_text_in_range，而 handle_key default 分支也 insert_str，两侧都插入导致每字符变两个。删 KeyDown 普通字符路径，唯一交给 IME path（与 terminal_view.rs:214-223 同样模式）。
  - `74fe9f5` 修 mask 模式鼠标点击 + backspace panic —— bounds_map 写入的 byte 来自 displayed_text 空间（`•`=3B），mouse_down 直接当 self.cursor 用导致 cursor 超出 self.text.len()，下次按 backspace 时 `self.text[..self.cursor]` slice 越界。加 cursor_from_click 做 displayed→source 映射（与 cursor_for_display 反向，对称）+ 5 个单测。HostForm password 字段是最典型触发点。
  - `217fd01` SSH 登录/连接失败弹 toast —— `app.rs` 收到 `SshEvent::Error` 原本只 `tracing::error!` + drop_session，UI 无任何反馈。按 SshErrorKind 分四类（连接/登录/IO/协议）调 `aish_ui::toast_error`，文案含 connection label。`SshEvent::Disconnected` 区分 reason：UserRequested/RemoteExited 静默，NetworkError 弹 toast。利用现有 ToastHandle global（无新组件）。
  - `ede5315` selection_anchor invariant 全面修复 —— 上面 4 个 fix 上线后仍残留 panic：mouse_down 在 anchor==cursor 状态下设 anchor + IME insert_str 在 anchor==cursor 时 delete_selection 走 false 分支不清 anchor，cursor 推进后 anchor 残留旧位置；之后 backspace remove 字符（也不清 anchor）让 text 变短到 anchor 之外，再按 Backspace 时 selection_range 返回越界 range → drain panic。修：所有 self.text mutating 方法（set_text / clear / backspace / delete_forward / insert_str）显式 anchor=None；selection_range() 内 clamp anchor/cursor 到 text.len() 兜底（defense-in-depth）。+6 单测覆盖 clamp 与 anchor 清除路径。aish-ui 137 → **143**。
- 已知边界：
  - "眼睛"图标切换 mask 显示未做（HostForm 原来也没有）
  - shift+click 扩展 selection 未做
  - 中键粘贴 / 右键菜单未做（Ctrl+V 已补）
  - 多行 TextInput 未扩展
  - IME mask 状态下 marked range 保持简化版（password 场景一般用户不用 IME）
  - bounds_map 第一帧空 → mouse_down 返回 0（首帧 click 极少发生，可接受）
  - cursor_for_display + displayed_selection 转换调用 char_indices().nth() 是 O(n)，password / 单行短文本无影响，多行扩展时可优化为一次遍历

### M15 — aish-ui Button + IconButton 精细化（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m15-button-polish-design.md`](specs/2026-05-11-aish-m15-button-polish-design.md)
- plan：[`plans/2026-05-11-aish-m15-button-polish.md`](plans/2026-05-11-aish-m15-button-polish.md)
- 范围：
  - ColorTokens 加 6 个 hover/active 状态色（primary/secondary/destructive 各一对，Ghost 不动用 accent，Disabled 用 muted）
  - Dark theme 填 Tokyo Night 阶梯（lightness 单调递增 idle → hover → active）
  - Light theme 6 个新字段占位 = dark 同值 + TODO 注释（下个 light theme milestone 真正手挑）
  - Button hover/active 按 variant 分色（GPUI `.hover()` + `.active()` modifier）
  - Button 加可选 `focus_handle(handle)` builder，传入后 render 用 `is_focused(window)` 判定，true 时画 2px outer ring（box_shadow，spread 2px，color t.colors.ring）
  - IconButton 同步处理（与 Button 完全对称的 variant 三态 + focus ring）
- 关键 commits：
  - `32e3506` — T1 ColorTokens +6 + Dark 填值 + Light 占位 + 6 个单调 lightness 断言测试
  - `34ca591` — T2 Button hover/active per variant + focus_handle（amend 含 disabled 三写占位 + Ghost active 同色决策注释）
  - `6f38227` — T3 IconButton 同步处理（amend 含 BoxShadow/point/FocusHandle import 一致性 + 注释对齐）
- 测试：aish-ui 100 → **110**（净 +10：dark.rs +6 / button.rs +2 / icon_button.rs +2）
- 已知边界：
  - Ghost variant hover/active 未拆 token（用 accent 单色）
  - Disabled 状态视觉不精细化（保持 muted）
  - Light theme 6 个新 token 仅占位，真正配色留下个 light theme milestone
  - focus ring 不区分键鼠 focus 路径（focus-visible 留 backlog）
  - 现有 Button / IconButton callsite 不传 focus_handle，向后兼容；具体接入由后续 milestone 在需要的场景按需做

### M14 — aish-ui Popover / DropdownMenu + Select 改造 + Toast 关闭（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m14-popover-design.md`](specs/2026-05-11-aish-m14-popover-design.md)
- plan：[`plans/2026-05-11-aish-m14-popover.md`](plans/2026-05-11-aish-m14-popover.md)
- 范围：
  - Popover Entity（click + programmatic 触发 + GPUI anchored Window mode + canvas prepaint 写入 trigger bounds + SwitchAnchor 自动翻转 + Esc/backdrop close + occlude）
  - MenuItem 数据 struct + DropdownMenu builder（作为 Popover content 使用）
  - Select 弹层从手糊 absolute 切到 Popover（自动获得 fit_mode 翻转，向下没空间时翻向上）
  - Toast 每条加 X 关闭按钮（IconButton + weak_entity().upgrade().dismiss）
- 关键 commits：
  - `ca9ac72` — T1 Popover（amend 修 fit_mode dead code + handle_key 抽离 + 模块注释补 trigger_bounds 前提）
  - `5d1e8e9` — T2 MenuItem + DropdownMenu
  - `d791baa` — T3 Select 改 Popover（amend 修 if popover_open guard + 删冗余 popover_handle）
  - `f2faf42` — T4 Toast X 关闭按钮（amend 删冗余测试 + 注释 usize 转换假设）
- 测试：aish-ui 90 → **100**（净 +10：Popover 5 / MenuItem 3 / DropdownMenu 3 / Select 6→5 / Toast 不变）；aish-app 101 不变
- 已知边界：
  - DropdownMenu 不接键盘导航（M14 简化版，M15+ 升级为 stateful Entity）
  - ContextMenu（右键触发）未做，留 M15+
  - `fit_mode` builder 已删除：gpui `anchored()` API 不允许 runtime 切换 fit mode，默认 SwitchAnchor 已包含

### M13 — aish-ui Card / NavItem / TabItem + 全 view 切组件（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-11-aish-m13-cards-nav-design.md`](specs/2026-05-11-aish-m13-cards-nav-design.md)
- plan：[`plans/2026-05-11-aish-m13-cards-nav.md`](plans/2026-05-11-aish-m13-cards-nav.md)
- 范围：3 个新组件 + 3 处 view 迁移
  - Card（4 slot: header/body/footer/actions + 3 variant: Default/Outlined/Elevated + on_click + group_hover actions 悬停显隐）
  - NavItem（vertical+horizontal 双模 + icon 接受任意 IntoElement + label + active indicator: 左 2px vertical / 底 2px horizontal）
  - TabItem（3 slot: prefix/title/suffix + active 切 bg + 绝对定位底部 2px primary line + on_click 透 click_count）
  - home host 卡片 → Card（actions 浮在右上角 hover 显示）
  - sidebar_nav 4 tab → NavItem.vertical()（Nerd Font icon 通过 div+font_family 包装传入）
  - tab_bar tab 项 → TabItem（rename 状态机和 close stop_propagation 仍在 caller）
- 关键 commits：
  - `311dc0b` — T1 Card
  - `8dc83f3` — T2 NavItem
  - `5f74da1` — T3 TabItem
  - `2431e00` — T4 home host 卡片切 Card（Card 内部加 .relative()）
  - `417d487` — T5 sidebar_nav 切 NavItem
  - `4290846` — T6 tab_bar 切 TabItem
- 测试：aish-ui 77 → **90**（+13：Card 4 / NavItem 5 / TabItem 4）；aish-app 101 不变
- 收尾：aish-app 内复合 view 元素全部组件化（仅 terminal_view 本体 + 已废弃 tmux_sidebar 保留手糊）

### M12 — aish-ui 表单与导航 + HostForm/SessionPicker 迁移（2026-05-11）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-09-aish-m12-forms-nav-design.md`](specs/2026-05-09-aish-m12-forms-nav-design.md)
- plan：[`plans/2026-05-09-aish-m12-forms-nav.md`](plans/2026-05-09-aish-m12-forms-nav.md)
- 范围：5 个新组件（Checkbox / Switch / Tabs / Dialog / Select）+ HostFormModal 重写为 Dialog+Tabs+6 TextInput（-294 行）+ SessionPickerView 外壳迁 Dialog + SettingsView 加 Appearance section 含 Dark mode Switch（点 Light 弹 toast warning + 视觉回弹）
- 关键 commits：
  - `20c106f` — T1 Checkbox（builder + 受控）
  - `122ac00` — T2 Switch（iOS 风胶囊）
  - `022e59b` — T3 Tabs（Entity + 键盘 ←/→）
  - `3bbc6e1` — T4 Dialog（Entity + Esc/backdrop close + needs_focus）
  - `72e992f` — T5 Select（Entity + 下拉 + ↑/↓/Enter/Esc）
  - （T6 prelude 验证：components::* glob 自动覆盖，无 commit）
  - `f52719d` — T7 HostFormModal 重写（删 FocusField/cycle_focus，引入 SyncedKey + 6 TextInput）
  - `9c617c0` — T8 SessionPickerView 外壳迁 Dialog
  - `8247f82` — T9 SettingsView Appearance section + Dark mode Switch
- 测试：aish-ui 51 → **77**（+26：Checkbox 5 / Switch 4 / Tabs 5 / Dialog 6 / Select 6）；aish-app 101 不变
- 已知边界：Dialog Tab 循环 focus trap 留 M13；Select 弹层只向下；Light theme 仍 unimplemented! stub；HostForm 的 password mask toggle 不再支持（M11 TextInput 暂无 mask 模式）；Edit 模式下"Tab 切字段"键盘流不接（依赖 Dialog focus trap）

### M11 — aish-ui 起步套件（2026-05-09）— ✅ 已完成
- 父 spec：[`specs/2026-05-09-aish-ui-architecture-design.md`](specs/2026-05-09-aish-ui-architecture-design.md)
- spec：[`specs/2026-05-09-aish-m11-ui-starter-design.md`](specs/2026-05-09-aish-m11-ui-starter-design.md)
- plan：[`plans/2026-05-09-aish-m11-ui-starter.md`](plans/2026-05-09-aish-m11-ui-starter.md)
- 范围：新建 aish-ui 独立 crate（依赖仅 gpui）+ Theme/Token 系统（21 ColorTokens + Radius/Spacing/FontSize scale + Theme::dark）+ 15 个 Lucide SVG icon + AssetSource 实现 + 7 个组件（Button/IconButton/Badge/Separator/Tooltip/TextInput/Toast）+ aish-app 接入：注册 Theme global / ToastHandle global / AssetSource，InputBarView 文本部分迁到 TextInput
- 关键 commits：
  - `4f025cc` — T1 crate 骨架 + workspace 注册
  - `90de327` — T2 Theme/Token + dark + light stub（amend 修 hex_a 死代码）
  - `405c27b` — T3 Icon 系统（15 SVG + AssetSource，amend 修 IconName::ALL 常量）
  - `2a75c62` — T4 Separator
  - `429747f` — T5 Badge
  - `5290bfd` — T6 Button
  - `0496cac` — T7 IconButton
  - `e3685e0` — T8 Tooltip
  - `81e12b6` — T9 TextInput 基础
  - `3f65fed` — T10 TextInput cursor blink
  - `cad165a` — T11 TextInput selection（amend 修 selection_range 空 range bug）
  - `ea5ee69` — T12 TextInput 复制粘贴（amend 修 IME on_change + compute_copy_payload 抽取）
  - `8ea5322` — T13 Toast 三层（amend 修 Warning fg_color）
  - `fd4a57c` — T14 aish-app 接入 Theme/ToastHandle/AssetSource
  - `7b84e8d` — T15 InputBarView 文本部分切到 aish_ui::TextInput
- 测试：aish-ui crate 51 tests + aish-app 101 tests + 其他 crate 不变（合计 268 全过）
- 已知边界：M11 简化版 mouse click 不解析 x 坐标（点击 = 末尾），cursor_at_pixel 留 M12+

### M10 — App 图标（跨平台 SVG→PNG/ICO/ICNS + Build 集成）（2026-05-09）— ✅ 已完成
- spec：[`specs/2026-05-09-aish-m10-icon-design.md`](specs/2026-05-09-aish-m10-icon-design.md)
- plan：[`plans/2026-05-09-aish-m10-icon.md`](plans/2026-05-09-aish-m10-icon.md)
- 实现状态：代码已完成并通过质量门禁 (fmt / clippy / test)，文档已补充
- 范围：SVG 单一真相源（终端像素风 `>_` 设计）→ 8 级 PNG 套装 + Windows ICO (6 尺寸) + macOS ICNS (7 尺寸)；Node.js 生成脚本 (@resvg/resvg-js + png-to-ico + @fiahfy/icns)；Windows build.rs 编译期集成；macOS Info.plist 配置；Linux .desktop 文件
- 关键任务：
  - T1：SVG 主设计源（已完成，assets/icons/aish.svg）
  - T2：Bun 生成脚本（已完成，scripts/gen-icons.js）
  - T3：运行脚本生成 PNG/ICO/ICNS（✅）
  - T4：Windows build.rs 集成（✅）
  - T5：macOS Info.plist（✅）
  - T6：Linux desktop 文件（✅）
  - T7：INDEX.md 更新（✅）
- 产出文件：
  - `assets/icons/aish-{16,32,48,64,128,256,512,1024}.png`
  - `assets/aish.ico`
  - `assets/aish.icns`
  - `crates/aish-app/build.rs`
  - `packaging/macos/Info.plist`
  - `packaging/linux/aish.desktop`

### M9 — Agent 输入栏（图片多选 + 文字 + SFTP 批量上传）（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m9-input-bar-design.md`](specs/2026-05-08-aish-m9-input-bar-design.md)
- plan：[`plans/2026-05-08-aish-m9-input-bar.md`](plans/2026-05-08-aish-m9-input-bar.md)
- 范围：终端视图下方固定底栏；[+] 按钮 GPUI 系统文件选择器多选图片；缩略图预览 + × 关闭；文字输入（Enter 发送）；Send → 批量 SFTP 上传 → paths + text echo 到 PTY
- 关键 commits：
  - `7ac8865` — state.rs UploadBatch + BatchUploaded/Failed + actor + app.rs 事件处理（T1-T3）
  - `9742fc5` — InputBarView 完整实现 + 集成到 RootView（T4-T5）

### M8 — 图片粘贴（Ctrl+Shift+V + SFTP + echo path）（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m8-image-paste-design.md`](specs/2026-05-08-aish-m8-image-paste-design.md)
- plan：[`plans/2026-05-08-aish-m8-image-paste.md`](plans/2026-05-08-aish-m8-image-paste.md)
- 范围：Ctrl+Shift+V 检测剪贴板类型 → 图片走 arboard 读取 + PNG 编码 + SshClient::sftp_upload → 远端 /tmp + echo 路径到 PTY；文字走现有 bracketed paste 逻辑
- 关键 commits：
  - `19e4fc3` — build: workspace 依赖加 russh-sftp / arboard / image
  - `96752d7` / `2a95493` / `366d852` — SshError::Sftp + SshClient::sftp_upload
  - `d589727` — state.rs UploadImage + ImageUploaded/Failed
  - `8cd880e` — terminal/image.rs encode_rgba_to_png
  - `fd7caad` — terminal_view paste() 图片检测
  - `41d4baf` — ssh_actor UploadImage match arm
  - `dd96d4c` — app.rs 事件处理 + 质量门禁

### M4b — Recent 持久化 + Settings 起步（2026-05-08）— ✅ 已完成
- spec：（无独立 spec，架构设计见 M4a spec）
- plan：[`plans/2026-05-08-aish-m4b-recent-settings.md`](plans/2026-05-08-aish-m4b-recent-settings.md)
- 范围：TOML 持久化 / app_state_file / last_connected 字段 + humanize / SettingsView 三段布局（Version / App Info / Legal）/ Inbox icon 换 Nerd Font / 启动加载 last_connected
- 关键 commits：
  - M4b-task-1：添加 toml + serde 依赖
  - M4b-task-2：app_state_file.rs 实现 TOML 读写
  - M4b-task-3：state.rs 添加 last_connected + humanize
  - M4b-task-4：SettingsView 三段布局
  - M4b-task-5：接入 SettingsView，启动加载
  - M4b-task-6：Home 卡片时间戳 + 写 recent
  - M4b-task-7：Inbox icon Nerd Font
  - M4b-task-8：质量门禁 + INDEX 更新

### M4a — 信息架构 4-tab 化（2026-05-08）— ✅ 已完成
- spec：[`specs/2026-05-08-aish-m4a-info-arch-design.md`](specs/2026-05-08-aish-m4a-info-arch-design.md)
- plan：[`plans/2026-05-08-aish-m4a-info-arch.md`](plans/2026-05-08-aish-m4a-info-arch.md)
- 长期上下文：[桌面版 Moshi Roadmap](roadmap-moshi-desktop.md) · 子项目 A
- 范围：左侧 48px sidebar 4-tab + Home(hosts grid + active sessions + quick action) + Inbox/Settings ComingSoon placeholder + EmptyTerminalGuideView
- Out-of-scope：Recent 持久化 / Settings 实质内容 / 任何 AI agent 集成 / 键盘快捷键 Ctrl+1..4
- 关键 commits：
  - `426b0b6` — feat(state): SidebarTab + sidebar 字段
  - `84ee15f` — feat(theme): sidebar 常量
  - `5e6352c` — feat(ui): SidebarNavView
  - `f354b34` — feat(ui): HomeView
  - `159ab9a` — feat(ui): EmptyTerminalGuideView + ComingSoonView
  - `9f19375` — feat(ui): RootView 重构
  - `973b8d1` — refactor(ui): 删除 DefaultPageView

### M3d-resize-iter1 — 拖窗 resize 时序修复（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-tmux-resize-tweaks-design.md`](specs/2026-05-08-aish-tmux-resize-tweaks-design.md)
- plan：[`plans/2026-05-08-aish-tmux-resize-tweaks.md`](plans/2026-05-08-aish-tmux-resize-tweaks.md)
- 实际产出：debounce 100→250ms / 本地 alacritty Term resize 推迟 80ms 到 SIGWINCH 之后 / check_resize 闭包 4 段流水
- 关键 commits：`04ed0e0` `7c6bbfc`(merge)
- 决策：5 个候选薄弱点（floor 取整 / shared session / refresh-client / 时序 / debounce）只修了 #4 #5 两条确定有问题的；#1/#2/#3 实测没撞到，留观察

### M3d-ui-iter2 — 删 ConnectionChip 横条（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-remove-connection-chip-design.md`](specs/2026-05-08-aish-remove-connection-chip-design.md)
- plan：[`plans/2026-05-08-aish-remove-connection-chip.md`](plans/2026-05-08-aish-remove-connection-chip.md)
- 实际产出：删 ConnectionChipView / [SSH] 蓝胶囊并入 tab 标题 / RootView body 简化（terminal 直接占满）
- 关键 commits：`86382bf` `76553b6`(fmt) `431b0b4`(merge)
- 注：原横条上的 ▾ 折叠按钮在 UI 层暂失，恢复入口待 backlog `collapse-orphan-conn` 做完

### M3d-ui-polish — UI 整体美化（2026-05-08）— ✅
- spec：[`specs/2026-05-08-aish-ui-polish-design.md`](specs/2026-05-08-aish-ui-polish-design.md)
- plan：[`plans/2026-05-08-aish-ui-polish.md`](plans/2026-05-08-aish-ui-polish.md)
- 实际产出：抽 `theme.rs` 集中色值 / 字号 / 半径 / 默认页大圆角卡片 / tab 栏 + connection chip + host form 全套 theme 应用
- 关键 commits：`24904aa` `8d5a227` `c4fc161` `0997e5e` `8097b43`

### M3c-post-cc-rework（2026-05-07）— ✅ 已完成
- spec：（无独立 spec，决策记录在 plan 内 + claude-progress.md 的"技术决策"节，已并入归档）
- plan：[`plans/2026-05-07-aish-m3c-post-cc-rework.md`](plans/2026-05-07-aish-m3c-post-cc-rework.md)
- 实际产出：connection chip / 鼠标滚轮 / tab inline 重命名 / mouse coord 修正 / SGR mouse 全事件转发
- 关键 commits：`3cebd36` `48dd5da` `679cc1f` `2fd213b` `1e3b1b5` `367d063`

### M3b — Tmux session 列表 + attach + 三栏 GUI（2026-05-07）— ⚠️ 部分作废
- spec：[`specs/2026-05-07-aish-m3b-tmux-attach-design.md`](specs/2026-05-07-aish-m3b-tmux-attach-design.md)
- plan：[`plans/2026-05-07-aish-m3b-tmux-attach.md`](plans/2026-05-07-aish-m3b-tmux-attach.md)
- 状态：原 -CC 控制模式部分已被 `ffe2cdf` 回退为 raw attach；list-sessions / GUI 三栏部分仍在使用，但三栏被 tab 系统取代

### M3a — Tmux control mode 协议层（2026-05-07）— ⚠️ M3-archived
- spec：[`specs/2026-05-07-aish-m3a-tmux-protocol-design.md`](specs/2026-05-07-aish-m3a-tmux-protocol-design.md)
- plan：[`plans/2026-05-07-aish-m3a-tmux-protocol.md`](plans/2026-05-07-aish-m3a-tmux-protocol.md)
- 状态：`aish-tmux` crate 内 controller / protocol / events / SessionTree 标 `#[allow(dead_code)]`，主路径不调用，保留待未来重启

### M2d — Auth Keyring（2026-05-07）— ✅
- spec：[`specs/2026-05-07-aish-m2d-auth-keyring-design.md`](specs/2026-05-07-aish-m2d-auth-keyring-design.md)
- plan：[`plans/2026-05-07-aish-m2d-auth-keyring.md`](plans/2026-05-07-aish-m2d-auth-keyring.md)

### M2c — Host 持久化 + GUI 增删改（2026-05-07）— ✅
- spec：[`specs/2026-05-07-aish-m2c-host-persistence-design.md`](specs/2026-05-07-aish-m2c-host-persistence-design.md)
- plan：[`plans/2026-05-07-aish-m2c-host-persistence.md`](plans/2026-05-07-aish-m2c-host-persistence.md)

### M2b1 — 终端渲染 + PTY resize（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-m2b1-terminal-rendering-design.md`](specs/2026-05-06-aish-m2b1-terminal-rendering-design.md)
- plan：[`plans/2026-05-06-aish-m2b1-terminal-rendering.md`](plans/2026-05-06-aish-m2b1-terminal-rendering.md)

### M2a — SSH 接入 + 单 PTY shell（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-m2a-ssh-bootstrap-design.md`](specs/2026-05-06-aish-m2a-ssh-bootstrap-design.md)
- plan：[`plans/2026-05-06-aish-m2a-ssh-bootstrap.md`](plans/2026-05-06-aish-m2a-ssh-bootstrap.md)

### M1 — GPUI 起步（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-m1-gpui-bootstrap-design.md`](specs/2026-05-06-aish-m1-gpui-bootstrap-design.md)
- plan：[`plans/2026-05-06-aish-m1-gpui-bootstrap.md`](plans/2026-05-06-aish-m1-gpui-bootstrap.md)

### M0 — Workspace 骨架（2026-05-06）— ✅
- spec：[`specs/2026-05-06-aish-ssh-design.md`](specs/2026-05-06-aish-ssh-design.md)
- plan：[`plans/2026-05-06-aish-m0-workspace-skeleton.md`](plans/2026-05-06-aish-m0-workspace-skeleton.md)

---

## 关键决策记录（ADR）

存在 `../adr/`，按编号：

- 0001 Record architecture decisions
- 0002 GUI framework: GPUI
- 0003 Terminal: alacritty_terminal
- 0004 SSH: russh
- 0005 Tmux: control mode（**已部分回退**，见 M3c-post-cc-rework）
- 0006 Tokio / GPUI bridge
- 0007 Credential storage: keyring
- 0008 Env injection: SendEnv + export
- 0009 Attachment path isolation

后续重大决策也走 `docs/adr/00NN-<title>.md`，编号顺延。

---

## 下一步候选（M3d 起）

来自 M3c-post-cc-rework 的"下一步"+ 用户验证后新增：

| ID | 描述 | 工作量 | 优先级 |
|---|---|---|---|
| housekeeping-host-list-rs | 用 `git rm` 删占位 `views/host_list.rs` | < 5 min | 低，下次手工 |
| collapse-orphan-conn | collapse 后孤儿 connection 在默认页加"恢复"入口 | ~半天 | 中 |
| tab-reorder-keyboard | Ctrl+Shift+PageUp/Down 重排 tab | ~1 小时 | 低 |
| tab-reorder-drag | tab 拖拽（GPUI drag/drop API 调研 + 实现） | ~半天 | 低 |
| paste | Ctrl+Shift+V 粘贴 + bracketed paste mode | ~半天 | 中 |
| mouse-legacy-encoding | X10/UTF8 鼠标编码 fallback（现代默认 SGR，需求弱） | < 1 小时 | 极低 |
| mouse-on-detect | aish 启动时检测/提示远端开 `set -g mouse on` | ~1 小时 | 中 |
| detach-detect | tmux conf 注入 `set-hook -g client-detached`，aish 解析后清侧栏标记 | ~半天 | 中 |
| session-picker-meta | session picker 列表加 window 数 + 上次活跃时间 | ~1 小时 | 低 |

需要做哪条都走 brainstorm → spec → plan → implement，本表只是 backlog。

---

## 历史快照（已归档）

- `../../feature-list.json` — 2026-05-07~05-08 的 7 个 feature 的快照（不再维护）
- `../../claude-progress.md` — 同期"技术决策记录"+"会话历史摘要"快照

新里程碑请**不要**再回填这两个文件。
