# Spec：M36 Home Launchpad（信息架构重设计）

> **里程碑代号**：M36
> **创建日期**：2026-05-17
> **状态**：待审核
> **关联**：M35.1 sidebar polish 完成后 home 视觉滞后，启动 M36 重做
> **预估**：3-5 天 / ~430 行净变化（详 §9）/ ~12 新 pure-fn 测试

---

## 1. Context

### 1.1 起源

M35.1 sidebar 视觉补强完成（5 改动 + 1 fixup + 用户反馈"好很多"）后，
user 提出 "Home 页视觉上一个台阶" 诉求。第一轮澄清 "泛视觉提升 / 没特定点"
判定为 M35 sidebar 反复 5 次仍判丑的同款陷阱 — 触发 plan 风险表里 "再次
判丑" high 风险。

锁定方法学：**不凭审美猜，对照参照系**（同 M35.1 三参照系做法）。
用户在 4 个候选参照系（Warp / Raycast / Linear / TablePlus）中全选，
转为雄心档位收紧 — 用户选 **大重做（信息架构重设计）**。

进而在 3 个 layout 方向（A split panel / B Warp launchpad / C Raycast
launcher）中选 **B Warp launchpad**。中途追加 "shell 缩略图 + 兜底" 要求
（Warp launchpad 招牌 visual hook）。

### 1.2 范围

**做**：
- active session **大卡**（含 shell 缩略图 + 4 phase 兜底）
- saved hosts **grid 卡重设计**（vertical layout / 紧凑 / hover edit-delete 右下角）
- 5 个 empty / error / edge state 兜底
- ~12 个 pure-fn 单元测试

**不做**：
- ANSI color preservation in preview（v2 backlog）
- render throttle（v1 实测后再决定）
- 改 `aish_ui::EmptyState` / `ErrorState` / `Kbd` / `Spinner` 通用组件
- sidebar / settings / hostform / commandpalette 改动
- 信息架构层级（不动 sidebar tab 体系）
- light theme（M35 T17 实验性标签状态延续）

### 1.3 为什么开 M36 而不归 M35.x

- 改动 300-400 行（M35.1 80 行的 4-5 倍）
- 信息架构改动（active hero + saved grid 重设计），不是单一视觉补强
- 引入新技术风险（Term grid 接入 + 性能基线），需要正式 spec
- 完成后在 INDEX.md 开 ### M36 一节，与 M35.x 并列

---

## 2. 关键决策记录（ADR-style）

| ID | 决策 | 替代方案 | 选择理由 |
|---|---|---|---|
| D1 | 走 launchpad 风（方向 B） | A split panel / C raycast launcher | aish 是终端工具，核心动作"接 + 继续 session"；不引入双 sidebar；改动可控 |
| D2 | shell 缩略图 v1 dim 统一色 | v2 ANSI 保色 | 视觉权重 < header 要求 dim；先 ship 实测；ANSI 实现成本高 |
| D3 | 不加 render throttle | 60Hz throttle | 实测先；terminal_view 1680 char 基线已稳定 |
| D4 | saved 卡 vertical layout | horizontal layout（现状） | grid 列窄、vertical 紧凑 |
| D5 | saved 卡保留 `● 活跃` chip | 隐藏 active host | M35 T7 revert lesson — 跨组件删除前 trace 默认状态路径 |
| D6 | 整卡 click = Attach (active) / Connect (saved) | 仅 button 触发 | 用户习惯：单击卡片 = 进入 |
| D7 | edit/delete IconButton 移到右下绝对定位 | 保持现状右侧 inline | 与文字内容不争位 |
| D8 | 复用 M28/M35 已有 EmptyState/ErrorState/Spinner/Kbd | 重写 | 不动通用组件，保持 cross-app 一致性 |
| D9 | preview 6 行 × 10px Code | 4 行 × 12px / 8 行 × 8px | 行数撑卡片 hero feel；10px JetBrains Mono dim 不抢戏 |
| D10 | hover state 同 sidebar M35.1 D5 inset glow | secondary_hover fill | 与 sidebar 视觉语言一致；与 active 大卡 hero 不冲突 |

---

## 3. 整体 Layout & 信息架构

### 3.1 Page 结构（窗口宽 ≥ 1000px）

```
┌──────────────────────────────────────────────────────────────────┐
│  Continue your work                                + 添加 host  │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Active sessions (N)                                             │
│  ┌─ Active Card (大) ─────────────┐  ┌─ Active Card (大) ─────┐ │
│  │  ● host · ⌧ tmux:dev          │  │  ●host                 │ │
│  │  user@host:22 · 12h active     │  │  user@host:22 · 2h     │ │
│  │  ┌─ shell preview 6 行 ────┐  │  │  ┌─ Connecting... ──┐ │ │
│  │  │ $ npm run dev          │  │  │  │ (spinner)        │ │ │
│  │  │ ✓ Server on :3000      │  │  │  │                  │ │ │
│  │  │ $ █                    │  │  │  └──────────────────┘ │ │
│  │  └────────────────────────┘  │  │           [Attach ↵] │ │
│  │                 [Attach ↵]   │  └─────────────────────────┘ │
│  └─────────────────────────────────┘                            │
│                                                                  │
│  ──────────────── separator ──────────────                       │
│                                                                  │
│  Saved hosts (N)                                       ⌘K        │
│  ┌─ saved card ─┐ ┌─ saved card ─┐ ┌─ saved card ─┐             │
│  │ 🐧           │ │ 🍏           │ │ 🐧           │             │
│  │ prod-db-1    │ │ web-1        │ │ stage-db     │             │
│  │ user@h:22    │ │ user@h:22    │ │ user@h:22    │             │
│  │ 12h ago      │ │ 2d ago       │ │ 5d ago       │             │
│  └──────────────┘ └──────────────┘ └──────────────┘             │
└──────────────────────────────────────────────────────────────────┘
```

### 3.2 响应式列数（按窗口宽度切换）

| 窗口宽 | active 列数 | saved 列数 |
|---|---|---|
| ≥ 1000 | 2 | 4 |
| 700-1000 | 2 | 3 |
| < 700 | 1 | 2 |

### 3.3 顺序与可见性

- Section 顺序：active → separator → saved（与 M35 T4 一致，不改）
- separator 仅当 active 非空 **且** saved 非空时画（M35 T4 现有逻辑保留）
- active 区为空时整段隐藏（active section label + cards + separator 都不渲染）
- saved 区为空时显示 M28 EmptyState 占满 page（hero center）
- hosts.json 加载失败时 saved 区位置替换为 M28 ErrorState（含 retry_btn）

---

## 4. Active Session 大卡 Anatomy

### 4.1 视觉规格

| 项 | 值 |
|---|---|
| min_h | 240px |
| max_h | 280px |
| padding | px(16) py(16) |
| radius | lg (8px) |
| border | 1px solid（idle: transparent / hover: primary 25%） |
| bg | card (#101113 dark) — idle / card + primary 5% blend — hover |

### 4.2 三段 layout

**Header row** — host 主身份
- `● phase dot` 6px circle（success/warning/destructive/loader 按 connection_phase）
- host name (Title3 14/600/foreground)
- `· ` 中点（muted_foreground dim）
- **tmux session** Code chip：`⌧ tmux:dev`（无 tmux 时元素不渲染）

**Meta row** — 连接细节
- user@host:port (Code typography / secondary_foreground)
- `· ` muted 中点
- 存活时长 (Caption 12/400/muted) — `5m active` / `12h active` / `2d active`

**Shell preview** — visual hook
- 容器：rounded md / 1px secondary_strongest border / bg `background` (比 card 暗一档 → "凹下去"嵌入感)
- 字体：Code (JetBrains Mono) **xs size = 10px**
- 行数：**固定 6 行**
- 内容：取 `host_pty_term[conn_id]` 的 last 6 rows cells
- v1：所有 cells 统一 `muted_foreground` 色（ANSI 保色留 v2）
- Cursor `█`：仅 Online + 有 buffer + cursor 在 last 6 rows 窗口内时画

**Actions row**
- `[Attach ↵]` 右对齐 — Button **primary** size **medium**
- `↵` 用 M35 T14 `aish_ui::Kbd` chip inline 在 label 内
- **整卡 click 等价 Attach 按钮**（D6）

### 4.3 4 phase 兜底（preview 区）

| connection_phase | preview 内容 | bg | 备注 |
|---|---|---|---|
| `Online` + cell buffer 非空 | 实际 last-6-rows | background | 主路径 |
| `Online` + cell buffer 空 | center "等待输出..." Caption | background | 刚连上未输出 |
| `Connecting` / `Reconnecting` | center loader icon + "Connecting..." | background | 含 spinner 动画 |
| `Disconnected` | center ⚠ + "Disconnected · 点击重连" | destructive 5% tint | 整卡 click 触发重连 |

### 4.4 Hover state（与 M35.1 D5 sidebar 同语言）

- idle: bg `card`、border `transparent`（永久 border_1 防 layout shift）
- hover: bg `card` + primary 5% blend、border `primary.opacity(0.25)`
- transition: medium 120ms ease（M35 T1 motion 标准）

---

## 5. Saved Host 卡 Anatomy

### 5.1 视觉规格

| 项 | 值 |
|---|---|
| min_h | 150px |
| padding | px(12) py(12) |
| radius | md (6px) |
| border | 1px solid（idle: transparent / hover: primary 25%） |
| bg | card / hover card+primary 5% blend（同 active 大卡） |

### 5.2 与 active 大卡的视觉差异（hierarchy）

| 维度 | active 大卡 | saved 卡 |
|---|---|---|
| min_h | 240 | 150 |
| padding | 16 | 12 |
| radius | lg (8) | md (6) |
| visual hook | shell 缩略图 | distros avatar |
| 主 action | `[Attach ↵]` Button | 整卡 click |
| 信息 row 数 | 2 (header / meta) | 3 (name / conn / time) |

### 5.3 内部 layout（vertical）

```
┌─ Card ──────────────────────────────┐
│  ┌────┐                              │
│  │ 🐧 │  ← distros avatar 40px      │
│  └────┘     top-left                 │
│                                      │
│  prod-db-1                ← Title3   │
│  user@db.prod:22          ← Code dim │
│  12h ago · ● 活跃         ← Caption  │
│              + active chip 弱视觉    │
│                                      │
│                  [hover: ✏  ⌫]       │
│                  ← 右下绝对定位       │
└──────────────────────────────────────┘
```

### 5.4 「● 活跃」chip

- host 在 active 大卡区出现时：saved 卡 time 行尾追 `· ● 活跃` chip（11/SEMIBOLD/success，弱视觉）
- host 无 active connection：仅 time（`12h ago`）
- 决策 D5：不藏 active host — saved 是 "可启动 host 全集" 语义；藏会让 count 变化、用户找不到

### 5.5 Edit / Delete IconButton

- 位置：**右下角 absolute**（D7 — 与文字内容不争位）
- 现状 `group_hover` 显形 pattern 保留
- IconButton size sm（与现状一致）
- 现有 `host_card_buttons: HashMap<HostId, HostCardButtons>` entity 池继续用

### 5.6 Avatar

- distros icon (M35 T16 backlog 的 distros SVG 集，目前已有 macos.svg；其他 8 个 distros 标 blocked)
- fallback：字母圆形（host name 首字母 + primary tint bg）
- size 40px

### 5.7 Hover & Click

- hover state 与 active 大卡一致（inset glow，M35.1 D5 同语言）
- 整卡 click → 调 `open_connection(host_id)` 开新 connection + 切 Terminal tab
- click edit IconButton → 打开 HostForm modal edit 模式
- click delete IconButton → confirm dialog → 删除 host

---

## 6. Data Flow & 缩略图实现

### 6.1 数据链路

```
SSH actor (per ConnectionId)
   │ raw bytes
   ▼
AnsiProcessor  (host_pty_processor[id])
   │ parse ANSI → Term commands
   ▼
Term<TitleListener>  (host_pty_term[id])
   │ apply commands → cell grid 更新
   │
   │ ── SshEvent::PtyData ──→ AppState.notify
   ▼
home rerender (home.rs:74 cx.observe(&state) 已接通)
```

**关键事实**：home.rs:74 已 `cx.observe(&state, |_, _, cx| cx.notify()).detach()`
— actor 每条 PtyData / state 变化都 trigger home rerender。**无需新增订阅**。

### 6.2 Preview content 取法（Phase A owned snapshot）

借用规则 — phase A read app borrow scope 内 owned 出 snapshot，drop borrow
后 phase B 用 snapshot 构造大卡 inner，与 home.rs 现有 3-phase 模式一致：

```rust
struct PreviewSnapshot {
    phase: ConnectionPhase,
    preview: Vec<String>,       // 6 行 cell content（trim trailing whitespace）
    cursor_in_window: Option<(usize, usize)>,  // (row, col) in last-6-window
}

// home.rs Phase A 内（read app borrow scope）
let active_previews: HashMap<ConnectionId, PreviewSnapshot> = ...;
```

**"preview 空" 的精确定义**：`take_last_n_rows` 返回的 `Vec<String>`
对每行 trim trailing whitespace 后，如果 **6 行全部为 empty string**（即用户
连上但完全没输出）则判 "preview 空" → Section 4.3 兜底分支 "等待输出..."。
非空判定 = 任意一行 trim 后非 empty。

### 6.3 v1 vs v2 — ANSI color 处理

| 维度 | v1 (本 spec) | v2 (backlog) |
|---|---|---|
| fg color | 统一 `muted_foreground` | 保留 ANSI fg |
| bg color | preview 容器统一 background | 保留 ANSI bg |
| 字体 | Code typography | 同 |
| 实现成本 | 低（char + 单色 div） | 中（cell→Hsla + multi-color render） |
| 选择理由（D2）| 视觉权重 < header 要求 dim；先 ship 实测 | 留升级路径 |

### 6.4 Performance estimates & throttle 决策

| 量级 | 估算 |
|---|---|
| cell 总数 / render | 5 active × 6 行 × 80 char = ~2400 cells |
| 对照基线 | terminal_view 70×24 = 1680 chars（已稳定运行） |
| GPUI text render 2400 char | 估实测 < 1ms |
| home rerender 频率 | actor PtyData 每条触发，无 throttle 30-60Hz |

**决策 D3**：v1 **不加 throttle** 先 ship；若实测性能不达标，v1.5 加 60Hz 节流。

### 6.5 数据风险

| 风险 | 等级 | 缓解 |
|---|---|---|
| `host_pty_term[id]` 在连接 close 残留旧 buffer | 低 | phase = Disconnected 走兜底分支，旧 buffer 不渲染 |
| Term grid 取 row 触发 deep clone | 中 | spec 实施前 benchmark；如有问题改 borrow grid 在 phase A 内完成 String 提取 |
| alacritty `display_iter` API 稳定性 | 中 | spec 实施前在 isolated 小程序验证 |
| 5+ active 时 rerender 频率过高 | 中 | 留 v1.5 throttle 选项 |
| Term cursor 在 6 行窗口外 | 低 | `cursor_in_window` 字段 None 时不画 cursor |

---

## 7. Empty / Error / Edge States

### 7.1 5 个状态全集

| # | 场景 | 视觉 |
|---|---|---|
| 1 | first-time（无 saved，无 active） | M28 EmptyState 占满 page hero center，含 `+ 添加 host` 大按钮（empty_add_btn entity） |
| 2 | 有 saved，无 active | active 区整段隐藏（无 hero card 区，无 separator）；saved grid 直接顶上来 |
| 3 | 有 active，无 saved | 不可能（active session 一定 from 某 saved host） |
| 4 | hosts.json 加载失败 | M28 ErrorState 显示在 saved section 位置（retry_btn entity）；active 区不受影响 |
| 5 | 某 active connection Disconnected | active 大卡内自身兜底（preview 区 ⚠ + "Disconnected · 点击重连"） |

### 7.2 状态 5 重连行为

- 整 Disconnected 大卡 click → 触发 `open_connection(host_id)` + `Bridge.spawn_session` 路径
- 无 modal、无 toast，直接走重连 → phase 切回 Connecting → 大卡 preview 切到 spinner

### 7.3 与已有组件的关系

| 组件 | 改动 | 原因 |
|---|---|---|
| `aish_ui::EmptyState` (M28) | 不改 | 通用组件，home 复用 |
| `aish_ui::ErrorState` (M28) | 不改 | 同上 |
| `aish_ui::Spinner` / loader icon | 不改 | active 大卡 Connecting 态复用 |
| `aish_ui::Kbd` (M35 T14) | 不改 | active 大卡 `[Attach ↵]` 用 |

---

## 8. Testing Strategy

### 8.1 沿用 M22-M35 惯例

| 层 | 策略 |
|---|---|
| Pure-fn 逻辑 | 写 unit test（cargo test 接入） |
| GPUI render 层 | manual visual 验收（aish 项目无 GUI 自动化 framework） |
| Integration / e2e | 不写（与现有 milestone 一致） |

### 8.2 Pure-fn 单元测试候选

| 函数 | 单元测试覆盖 | 数量 |
|---|---|---|
| `take_last_n_rows(grid, n) -> Vec<String>` | empty grid / grid < n / grid ≥ n / trailing whitespace / cursor in window vs out | ~5 |
| `preview_branch_for_phase(phase, preview_empty) -> PreviewBranch` | Online+非空 / Online+空 / Connecting / Reconnecting / Disconnected | ~4 |
| `format_active_duration(connected_at, now) -> String` | "5m active" / "12h active" / "2d active" / 边界 < 1m | ~3-4 |
| **总计** | | **~12 新测试** |

### 8.3 测试基线

- 当前 **571 全过**（M35.1 收尾后）
- 预估 M36 后 **~583 全过**

### 8.4 Manual visual 验收 checklist

1. active 4 phase 渲染（启 SSH 看 Connecting → Online；kill 看 Disconnected；进 shell 不输入看"等待输出"）
2. 整卡 click = Attach 触发 attach 流程
3. saved 卡 click = connect，hover edit/delete 出现
4. 响应式列数切换（< 700 / 700-1000 / ≥ 1000）
5. empty state — 删空 hosts.json
6. error state — corrupt hosts.json

### 8.5 不测的边界

| 项 | 原因 |
|---|---|
| GPUI div / layout 渲染本身 | 上层 framework，aish 不 own |
| alacritty Term grid API 语义 | 第三方 crate，依赖版本号信任 |
| SSH actor / Bridge spawn 流程 | M3 / M22 已稳定 |

### 8.6 风险类验证（不归测试但要做）

- alacritty_terminal API `display_iter` / `iter_from` / `cursor.point` 在当前 dep version 行为验证 → spec 实施前在 isolation 小程序确认
- 性能基线（5 active session 实测 home rerender 时长）→ 不达标加 throttle，spec 阶段不预先决定

---

## 9. File Structure 改动预估

| 文件 | 改动 | 估算行 |
|---|---|---|
| `crates/aish-app/src/views/home.rs` | 主要改动 — active 大卡 + saved 卡重构、preview 接入、phase 兜底 | +250 / -150 |
| `crates/aish-app/src/views/home_preview.rs`（新文件） | pure-fn 抽出：`take_last_n_rows` / `preview_branch_for_phase` / `format_active_duration` + 12 单元测试 | +150 / 0 |
| `crates/aish-types/src/lib.rs`（可能） | 如需扩 `ConnectionPhase` enum 或加 `PreviewSnapshot` 公共类型 | +20 / 0 |
| `crates/aish-app/src/state.rs`（可能） | 确认 `connection_phases` 字段已覆盖 4 phase 兜底；如缺 `Disconnected` 标记需补 | +10 / 0 |

**总计估算**：~430 行净变化、3-4 文件、~12 新测试、5-8 commit。

---

## 10. Acceptance（怎么算"做完"）

- [ ] 所有 Section 3-7 design 实现（active 大卡 / saved 卡 / 4 phase 兜底 / empty/error）
- [ ] 12 个 pure-fn 单元测试通过
- [ ] 6 项 manual visual 验收通过
- [ ] cargo +nightly fmt --all 通过
- [ ] cargo +nightly clippy --workspace --all-targets -- -D warnings 通过
- [ ] cargo test --workspace 全过（~583）
- [ ] alacritty API 验证（isolation 程序）完成、性能 baseline 实测无问题
- [ ] INDEX.md 追加 `### M36 Home Launchpad` 段
- [ ] 用户视觉验收通过

---

## 11. 不在范围（明确避免 scope creep）

- ❌ ANSI color preservation in preview（v2 backlog）
- ❌ render throttle (v1 实测后决定 v1.5)
- ❌ 改 `aish_ui::EmptyState` / `ErrorState` / `Kbd` / `Spinner` 通用组件
- ❌ sidebar / settings / hostform / commandpalette 改动
- ❌ 信息架构层级（不动 sidebar tab 体系）
- ❌ light theme（M35 T17 实验性标签延续）
- ❌ Active session 卡上加 "Disconnect" button / "Open in new tab" 等多 action（保持单一 Attach action 简洁）
- ❌ saved 卡 grid → list 切换（D4 选择 vertical grid，不引入 layout toggle）

---

## 12. 后续 plan 落地策略

本 spec 通过后开 plan：`docs/superpowers/plans/2026-05-17-aish-m36-home-launchpad.md`。
plan 内含 task 顺序（建议先做 alacritty API 验证 → pure-fn 抽出 + 测试 →
active 大卡 → saved 卡 → 兜底 → INDEX 收尾），每 task 独立 commit。
