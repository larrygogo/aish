# M35 — UI/UX 整体提升（Plan）

**Spec**: [`../specs/2026-05-15-aish-m35-uiux-overhaul-design.md`](../specs/2026-05-15-aish-m35-uiux-overhaul-design.md)

---

## File Structure

```
crates/aish-ui/src/theme/
├── motion.rs          (Phase A T1: medium 150 → 120ms)
├── typography.rs      (Phase A T2: 加 Code role)
├── tokens.rs          (Phase A T3: 调 accent S / border 对比度 / 加 elevation_focus)
└── anatomy.rs         (Phase A T5: outer_py_spacious token)

crates/aish-ui/src/components/
├── command_palette.rs  (新 Phase B T8: ⌘K MVP，~300 行)
├── status_bar.rs       (新 Phase C T13: 底栏 latency / 连接状态, ~150 行)
└── kbd.rs              (新 Phase C T14: KbdShortcut chip 视觉, ~80 行)

crates/aish-app/src/views/
├── home.rs            (Phase A T4: 改名 + separator + hero "继续工作" section
                       Phase B T7: 削减「最近连接」list (移到 sidebar))
├── sidebar_nav.rs     (Phase A T6: icon + Caption label slot
                       Phase B T9: 升 220px 含「最近连接」list)
├── host_form.rs       (Phase B T10: 单行 user@host:port 输入 + inline radio)
├── terminal_view.rs   (Phase B T11: ConnectionChip 视觉权重 + Phase B T12: inline ErrorState 替换)
├── settings.rs        (Phase C T15: 快捷键展示 section + 关于页 card)
└── command_palette_view.rs  (新 Phase B T8 配套)

docs/design/
└── principles.md      (Phase A T0: 写产品三条设计原则)
```

总计：4 个新文件 + 6 个现存文件改动 + 1 个 doc。

---

## Phase A — v0.next（1-2 天，高 ROI 小批量）

每个 task 独立可 commit，全部不动 component 内部 API，仅改 token / view 局部。

### T0: 写设计原则 doc（30min）

**File**: `docs/design/principles.md`（新）

把 spec §1 的「定位 + 三原则」拷成独立 doc 供未来 reviewer / 新人查阅。
**Acceptance**: 文件存在、引用从 INDEX.md 和 spec 顶部。

### T1: Motion `medium` 150 → 120ms

**File**: `crates/aish-ui/src/theme/motion.rs`

改 `Motion::medium` 单 token 值。6 个 stateful entity 全部受益。
**质量门禁**：cargo test --workspace 全过 + 实测 5 个 hover 切换不显「太快」。
**Acceptance**：手测 Button / Card hover 速度感更利落，不卡。

### T2: 补 `TypeRole::Code`

**File**: `crates/aish-ui/src/theme/typography.rs`

```rust
pub enum TypeRole {
    // ... 9 existing ...
    Code,  // 新增
}

impl Typography {
    pub fn for_role(&self, r: TypeRole) -> TypographySpec {
        match r {
            // ... existing ...
            TypeRole::Code => TypographySpec {
                font: aish_ui::FONT_CODE_NAME,  // JetBrains Mono Nerd Font
                size: px(12.0),
                weight: FontWeight::NORMAL,
                color_role: ColorRole::Foreground,
            },
        }
    }
}
```

随后在 host card / Settings 显示路径处用 `.typography(TypeRole::Code, theme)` 替换原 Body。
**Acceptance**：host card `user@host:port` 等宽 + Settings 「打开配置目录」路径等宽。

### T3: 调 color tokens（accent / border / elevation_focus）

**File**: `crates/aish-ui/src/theme/tokens.rs`

- `accent` dark：S 值 -10%（具体 HSL 数值仍需测试，目标视觉「不再喧宾」）
- `border` dark：L 值 +5%（与 card bg 对比度从 ΔL=4 → ΔL=9）
- 新增 `elevation_focus(kind) -> Vec<BoxShadow>`：blur 24 / spread 0 / α 0.4

仅 token 改动，所有 caller 自动受益。
**Acceptance**：dark theme 下 outlined Card 边框可见；active sidebar nav bg 不刺眼；Dialog 开起来时 backdrop 视觉「漂浮」（需 T4 配套用 elevation_focus）。

### T4: Home 页改名 + separator + hero「继续工作」section

**File**: `crates/aish-app/src/views/home.rs`

改动点：
1. 「ACTIVE SESSIONS」section label 改 `Title3` (16/600/foreground)，内容改名「⏎ 继续工作」
2. 「HOSTS」section label 改 `Title3`，内容改名「保存的主机 ({count})」+ 右上加 `⌘K 搜索` Caption hint（不挂事件，先 visual only）
3. 两 section 之间加 Separator（aish_ui::Separator 已存在）
4. host card 内 `user@host:port` 改用 `TypeRole::Code`（依赖 T2）

**Acceptance**：Home 视觉层级清晰，「继续工作」是第一视觉重点；section divider 不再是 muted Caption。

### T5: Anatomy `outer_py_spacious` token

**File**: `crates/aish-ui/src/theme/anatomy.rs`

```rust
pub struct Page {
    // ... existing outer_px / outer_py_top / outer_py_bottom ...
    pub outer_py_spacious: Pixels,  // 新增 32.0
}
```

home.rs 顶部 page header 用 spacious 模式（与 hero section 配合）。
**Acceptance**：Home 顶部留白更舒展，符合 Linear / Vercel 风格。

### T6: Sidebar icon + Caption label（同 60px 不增宽）

**File**: `crates/aish-app/src/views/sidebar_nav.rs`

每个 NavItem 内部 vertical layout 加 Caption 标签（icon 上 + 「Home」「Terminal」「Settings」标签下）。**保持 60px 宽不变** — 仅在 icon 下方加 11px Caption label 配合 4px gap。

NavItem 现已支持 `.label()` slot，本任务仅在 ctor 调用时挂 label。
**Acceptance**：sidebar 一眼看清 3 个 nav 是什么，不再"图形猜谜"。

### Phase A Commit 计划

```
- chore(theme): motion medium 150 → 120ms (T1)
- feat(theme): TypeRole::Code + JetBrains Mono 等宽 typography (T2)
- feat(theme): accent / border 视觉调优 + elevation_focus token (T3, T5)
- feat(home): 改名「继续工作」/「保存的主机」+ section hierarchy 升 Title3 (T4)
- feat(sidebar): icon 下方加 Caption label (T6)
- docs: M35 设计原则 + INDEX 状态更新 (T0)
```

7 个 commit；每个独立可 ship；总改动 ~150 行。

---

## Phase B — v0.next+1（5-7 天，大改）

### T7: Home 削减「最近连接」section

**File**: `crates/aish-app/src/views/home.rs`

把当前 ACTIVE SESSIONS 的 list 行迁移走（移到 sidebar T9），Home 仅保留「保存的主机」+ Quick Actions header。
**依赖**：T9（sidebar 升级）完成后做此 cleanup。

### T8: CommandPalette MVP（⌘K）

**Files**:
- `crates/aish-ui/src/components/command_palette.rs`（新）
- `crates/aish-app/src/views/command_palette_view.rs`（新）

范围：
- 全局 Ctrl+P / Cmd+P 触发（在 RootView 加 global key listener）
- 显示 Dialog-like overlay (centered, width 540px)
- TextInput 输入 → fuzzy match hosts.label / host / user
- 结果列表用 ListRow（复用已存在）
- ↑/↓ 选中，Enter 触发 `open_connection(host_id)` + 关 palette
- Esc 关 palette

不做的（v1 范围）：commands index / settings / 最近用过的命令。
**质量门禁**：单测 3 个（fuzzy match scorer / 选中 idx wrap / Enter handler）。
**Acceptance**：host > 5 个时按 ⌘K 能 1 秒找到目标。

### T9: Sidebar 升 220px 含「最近连接」list

**File**: `crates/aish-app/src/views/sidebar_nav.rs`（大改）

改动：
1. 加 `collapsed: bool` 状态字段（默认 false，持久化到 app_state.toml）
2. 展开时 220px 含：
   - 顶部 logo + name + 折叠按钮
   - 3 个 NavItem（横向 layout 含 icon + label，复用 NavItem horizontal mode）
   - separator
   - 「最近连接」section（Caption divider + ListRow * N，复用 list_row）
3. 折叠时 48px icon-only + tooltip（用现有 aish_ui::Tooltip）

需要 app_state.toml schema 加 `sidebar_collapsed: bool`。
**质量门禁**：单测 sidebar_collapsed roundtrip。
**Acceptance**：sidebar 默认展开 220px 含 nav + 最近连接；点 logo 切折叠；偏好持久化。

### T10: HostForm 单行 user@host:port 输入 + inline radio

**File**: `crates/aish-app/src/views/host_form.rs`

改动：
1. 顶部加 `connection_input: Entity<TextInput>`，placeholder `user@host:port`
2. `parse_connection_string(s) -> Option<(user, host, port)>` 解析函数 + 4 个 pure-fn 单测
3. on_change 解析成功 → 自动填 user / host / port，并在下方 4 字段 readonly mode preview
4. on_change 解析失败 → 不报错，user 继续在 4 字段填
5. AuthKind Tabs 改 inline Radio 3 选项（aish_ui::Radio 已存在）
6. Label 字段移到底部 + 标 `(可选)`

**质量门禁**：parse_connection_string 单测 6 个（典型 / 无 port / 含 IPv6 / 含 - 字符 / 空字符串 / 含特殊字符）。
**Acceptance**：粘贴 `larry@1.2.3.4:22` 自动填表。

### T11: ConnectionChip 视觉权重升级

**File**: `crates/aish-app/src/views/terminal_view.rs`（找 ConnectionChip 用法）

当前 ConnectionChip 是 inline text。升级为有 bg + border 的 chip 组件：
- 左侧 status dot（success / warning / error）
- host name (Code typography)
- 右侧 tmux session badge（如有）
- 整体可 hover 显完整信息 tooltip

**Acceptance**：5 个 tab 同时开时一眼看出哪个 tab 连哪个 host。

### T12: Inline ErrorState 替换 terminal viewport

**File**: `crates/aish-app/src/views/terminal_view.rs`

当 `ConnectionState::Failed { kind, msg }` 时不再 paint grid，改 paint 居中 ErrorState：
- icon: `AlertOctagon`
- title: 「连接失败」+ host 名
- description: msg（Code typography）
- actions: `Button("重试连接")` + `Button("编辑 host")` + `Button("复制错误")`

保留 ConnectionChip + tab bar（用户能切其它 tab 不被卡死）。
**Acceptance**：SSH 认证失败时用户能看清错误 + 一步触发重试 / 编辑。

### Phase B Commit 计划

```
- feat(command-palette): ⌘K MVP fuzzy host search (T8)
- feat(sidebar): 升 220px 含「最近连接」list + 折叠模式 (T9)
- chore(home): 「最近连接」移到 sidebar 后 home 清理 (T7)
- feat(host-form): 单行 user@host:port 输入 + inline radio (T10)
- feat(terminal): ConnectionChip 视觉权重升级 (T11)
- feat(terminal): SSH 失败 inline ErrorState 替换 toast (T12)
```

6 个 commit；总改动 ~600 行（CommandPalette 占大头）。

---

## Phase C — v0.next+2（5 天，收尾）

### T13: StatusBar 组件（底栏）

**File**: `crates/aish-ui/src/components/status_bar.rs`（新）

最简 status bar：底部 24px 高，3 个 slot（左 / 中 / 右）。
- 左：connection latency（如 「ping 12ms」）
- 中：current tmux mode 提示（如「tmux 鼠标模式 ON」）
- 右：reduced_motion / theme indicator

只是 visual component，caller 控制内容。RootView 在 ScrollPage 下方挂一个。
**Acceptance**：StatusBar 不喧宾，但用户能瞄一眼知道当前连接质量。

### T14: KbdShortcut chip 视觉组件

**File**: `crates/aish-ui/src/components/kbd.rs`（新）

`<Kbd>⌘ K</Kbd>` 风格的 chip。用在快捷键展示页和 EmptyState description 里。
样式：8px padding / border 1 / radius 4 / Code typography / 浅 bg。
**Acceptance**：3 处复用（Settings 快捷键列表 / Home ⌘K hint / EmptyState description）。

### T15: Settings 快捷键展示 + 关于页

**File**: `crates/aish-app/src/views/settings.rs`

加 2 个新 Card：
1. **快捷键**：列出 ~10 个核心快捷键（用 KbdShortcut chip）。table 风格。
2. **关于**：用 LOGO_128 资产 + version + license + GitHub link（point to repo）

LOGO_128 删 `#[allow(dead_code)]`，关于页是首个 caller。
**Acceptance**：Settings 总 5 张 Card（外观 / 数据 / 安全 / **快捷键** / **关于**）。

### T16: Linux brand icon 补全

**File**: 找当前 host card avatar 涉及的代码（home.rs）

INDEX 提过：当前 7 个 OS 已有 SVG（ubuntu/debian/arch/alpine/centos/fedora/redhat）。补：
- rocky / mint / manjaro / nixos / gentoo / opensuse / raspbian / elementary 8 个

每个加 SVG 资产到 `build.rs` 嵌入路径 + os_kind match arm 加分支。
**Acceptance**：8 个新 OS host 显 SVG icon 而非单字母 fallback。

### T17: Light theme decision — 「实验性」标签

**File**: `crates/aish-app/src/views/settings.rs`

Light theme switch label 改：「Light（实验性）」+ tooltip「部分色彩未完整调优」。
**Acceptance**：用户知道 light theme 不是 first-class。

### T18: 大文件可能性评估

**Files**: `views/home.rs` / `views/tab_bar.rs` / `views/terminal_view.rs`

若 Phase B 完成后这 3 个文件仍 > 800 行，本 task 尝试**轻量拆分**（仅抽出 helper / pure fn 到 sibling mod，不动核心 render 结构）。如评估拆出后维护成本反升 → 放弃，写入 INDEX 「保留大文件」注释。
**质量门禁**：拆完测试不变 + clippy 不变。

### Phase C Commit 计划

```
- feat(ui): StatusBar 组件 + 接入 RootView (T13)
- feat(ui): KbdShortcut chip 组件 (T14)
- feat(settings): 快捷键展示 + 关于页 (T15)
- feat(home): Linux brand icon 补完 8 个 OS (T16)
- chore(settings): Light theme 「实验性」标签 (T17)
- refactor(views): 大文件评估 + 可能拆分 (T18, 若动手则单独 commit)
```

6 个 commit；总改动 ~400 行。

---

## Self-Review

### 风险评估

| Phase | 主风险 | Mitigation |
|---|---|---|
| A | Motion 120ms 太快用户反馈不明显 | 改 1 token 值，回滚成本极低 |
| A | accent 饱和度 -10% 后视觉对比不足 | 落地后实测 1 天，调到合适值 |
| B | Sidebar 220px 改动破 muscle memory | 折叠按钮 + 首次 toast 提示 |
| B | CommandPalette 与 OS 快捷键冲突 | 双绑 Ctrl+P / Cmd+P，OS-detect 自动 |
| C | Linux SVG 资产版权 | 仅用 simpleicons / 各发行版官方 CC0 资产 |

### 依赖关系

```
T0 (docs)         ─┐
T1 (motion)       ─┤  Phase A 6 task 全部独立可并行
T2 (Code role)    ─┼─→ T4 (home) 依赖 T2 完成
T3 (color)        ─┤
T5 (anatomy)      ─┤
T6 (sidebar label)─┘

T8 (palette MVP)  ─┐
T9 (sidebar 220)  ─┼─→ T7 (home 清理) 依赖 T9 完成
T10 (host form)   ─┤
T11 (chip)        ─┤
T12 (inline err)  ─┘  Phase B 5 个独立 + 1 个依赖

T13 (statusbar)   ─┐
T14 (kbd)         ─┤
T15 (settings)    ─┼─→ T15 依赖 T14 (KbdShortcut chip)
T16 (linux icon)  ─┤
T17 (light theme) ─┤
T18 (拆文件)      ─┘
```

### 工程量分布

| Phase | Tasks | 工程量 | 累计 |
|---|---|---|---|
| A | 6 | 1-2 天 | 1-2 天 |
| B | 6 | 5-7 天 | 6-9 天 |
| C | 6 | 5 天 | 11-14 天 |

每个 task 完成后跑质量门禁（cargo +nightly fmt --check + clippy --workspace -D warnings + test --workspace），任一失败必须修后才能 commit。

### 不在本 plan 范围

- AI 命令提示（仿 Warp） — 单独立项
- 插件市场 — 不做
- Onboarding tour — 不做
- 移动端 / web 版本 — 不在本产品 roadmap

### 与 INDEX backlog 关系

本 plan 完成后 INDEX backlog 状态：

| Backlog item | 状态 |
|---|---|
| skeleton-business-usecase | 仍 valid，独立项 |
| button-entity-test-harness | 仍 valid，独立项 |
| mouse-legacy-encoding | 仍极低优，可不做 |
| tab-indicator-slide (真 slide) | Phase A 已落地 fade 简化版，slide 不做 |

---

## Acceptance Criteria（整体 M35）

定性：
- [ ] 打开 Home 第一眼能看出"继续工作"是首要操作
- [ ] sidebar 不再"看起来浪费"
- [ ] host > 10 个时能用 ⌘K 1 秒找到目标
- [ ] SSH 连接失败时知道下一步该做什么
- [ ] 整体视觉「年轻了 2 年」

定量：
- [ ] cargo +nightly fmt --check ✅
- [ ] cargo +nightly clippy --workspace --all-targets -- -D warnings ✅
- [ ] cargo test --workspace 全过（新增组件含 ≥ 3 个 pure-fn 测试）
- [ ] 18 个 task 全部 commit + push
- [ ] INDEX.md 更新 M35 落地记录
