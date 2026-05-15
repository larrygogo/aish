# M28 — State Design（Plan）

**Spec**: [`../specs/2026-05-15-aish-m28-state-design-design.md`](../specs/2026-05-15-aish-m28-state-design-design.md)

---

## File Structure

```
crates/aish-ui/src/components/empty_state.rs       (新文件 StatusView + EmptyState + ErrorState)
crates/aish-ui/src/components/skeleton.rs          (新文件 Skeleton::block/circle)
crates/aish-ui/src/components/mod.rs               (pub mod + pub use)
crates/aish-ui/src/icons/mod.rs                    (+5 IconName + ALL 数组追加)
crates/aish-ui/assets/icons/inbox.svg              (新增 lucide)
crates/aish-ui/assets/icons/server.svg             (新增 lucide)
crates/aish-ui/assets/icons/wifi-off.svg           (新增 lucide)
crates/aish-ui/assets/icons/file-question.svg      (新增 lucide)
crates/aish-ui/assets/icons/loader.svg             (新增 lucide)
crates/aish-ui/src/lib.rs                          (pub re-export EmptyState / ErrorState / Skeleton)
crates/aish-app/src/state.rs                       (AppState +hosts_load_error 字段)
crates/aish-app/src/app.rs                         (load_hosts 失败写入 hosts_load_error)
crates/aish-app/src/views/home.rs                  (empty_hint → EmptyState + load_error → ErrorState)
crates/aish-app/src/views/empty_terminal.rs        ('>_' 字符 → EmptyState)
crates/aish-app/src/views/session_picker.rs        ('(无 session)' → EmptyState)
```

---

## Tasks（顺序，每条独立 commit）

### T1: 新 IconName +5 + SVG assets

- `aish-ui/src/icons/mod.rs`：
  - `IconName` 加 `Inbox / Server / WifiOff / FileQuestion / Loader`
  - `IconName::ALL` 数组追加（保持与变体一一对应）
  - `IconName::file_name()` match arm 加 5 个新映射（`"inbox.svg"` 等）
- `aish-ui/assets/icons/` 加 5 个 lucide SVG（从 https://lucide.dev 下
  default 24x24 stroke=2 版本）
- 单测：现有 `all_icons_have_assets` 自动覆盖（遍历 `IconName::ALL` 调
  `AssetSource::load`），不需新增

**质量门禁**: fmt + clippy + test 通过；手动 `cargo test --package aish-ui
icons` 确认 5 个新 icon load 成功。

---

### T2: EmptyState + ErrorState 组件

- 新文件 `components/empty_state.rs`：
  - 内部 `struct StatusView` 持 `id / icon / icon_color / icon_bg /
    title / description / action`
  - `impl StatusView` 渲染 4-slot anatomy（spec D-1 图示）：
    - icon container = w_8 h_8 rounded_full bg(secondary) +
      flex center + icon
    - gap 12 → title (Title3 typography)
    - gap 4 → description (Body typography + muted_fg + max_w(px(320)) +
      text_align_center)
    - gap 16 → action (AnyElement child)
  - pub `EmptyState::new(id)` -> StatusView { default icon = None, color =
    muted_foreground, bg = secondary }
  - pub `ErrorState::new(id)` -> StatusView { default icon =
    AlertCircle, color = destructive, bg = secondary }
  - builder：`.icon(IconName)` / `.title(impl Into<SharedString>)` /
    `.description(...)` / `.action(impl IntoElement)`
- `impl Render for StatusView` 输出固定布局
- 单测：
  - `empty_state_minimum_title_only`：仅 title 渲染时子元素 count = 1
  - `empty_state_full_anatomy`：4 slot 全填 count = 4
  - `error_state_default_icon_is_alert_circle`
  - `error_state_default_color_is_destructive`
- mod.rs + lib.rs pub re-export

**质量门禁**: fmt + clippy + test 通过；新增 ~6 个单测。

---

### T3: Skeleton 组件

- 新文件 `components/skeleton.rs`：
  - `pub struct Skeleton { shape, size, shimmer: bool }`
  - `pub enum Shape { Block, Circle }`
  - `pub fn block() -> Skeleton` 默认 shape=Block + rounded_sm
  - `pub fn circle() -> Skeleton` 默认 shape=Circle + rounded_full
  - `.with_shimmer(bool)` builder（v1：true 时无副作用，预留 hook）
  - `impl IntoElement` 输出单 div bg(secondary) + 对应 rounded
  - 提供 `.w/.h/.size` builder 透传（或直接 caller 在 IntoElement 外层
    wrap div 控尺寸 — 选前者更易用）
- 单测：
  - `skeleton_block_default_rounded_sm`：渲染包含 rounded class（GPUI
    style 字段断言）
  - `skeleton_circle_default_rounded_full`
- mod.rs + lib.rs pub re-export

**质量门禁**: fmt + clippy + test 通过；新增 ~2 个单测。

---

### T4: home.rs 迁 EmptyState（hosts 为空）

- `views/home.rs`：
  - 删 `empty_hint` 那 13 行手糊 div
  - 替换为 `EmptyState::new("home-no-hosts")
      .icon(IconName::Inbox)
      .title("还没有保存的连接")
      .description("点右上角 + 添加 host 开始")
      .action(Button::new(...).primary().label("添加 host").on_click(...))`
  - action button on_click 复用现有 `handle_add_click`
- 容器层：原 `empty_hint` 位置（px_4 py_8）保留作 outer 占位，EmptyState
  自身居中

**质量门禁**: 删光 hosts 启动 → 看到 4-slot anatomy 居中显示 + 点 action
弹 host_form。

---

### T5: empty_terminal.rs 迁 EmptyState

- `views/empty_terminal.rs`：
  - 删 `>_` 字符 div + 现有 3 个 child（title / subtitle / button）
  - 替换为：
    ```rust
    EmptyState::new("empty-terminal")
        .icon(IconName::Server)
        .title("No active sessions yet")
        .description("Pick a host from Home to get started.")
        .action(go_home_btn)
    ```
  - 保留外层 size_full + bg(colors.background) 容器（EmptyState 自身不
    决定 bg）

**质量门禁**: 切到 Terminal sidebar 但无 tabs 时显示新 EmptyState；点
action 跳 Home 仍工作。

---

### T6: session_picker.rs 迁 EmptyState（dialog 内）

- `views/session_picker.rs`：
  - sessions.is_empty() 分支：删一行 muted "(无 session — 关闭弹窗回到
    raw shell)"
  - 替换为 `EmptyState::new("session-picker-empty")
      .title("无 tmux session")
      .description("按 Esc 关闭弹窗回到 raw shell")`
    （无 icon，dialog 内紧凑）
  - dialog body 给 EmptyState 加 wrapper `div().min_h(px(120))`，避免
    dialog 高度塌缩
- 测试：mock TmuxState::Detected { sessions: vec![] } 让 picker 打开看
  EmptyState 渲染

**质量门禁**: 手测 session 数=0 dialog 视觉居中，无 wrapping awkward。

---

### T7: ErrorState for hosts.json load 失败

- `state.rs`：`AppState` 加 `hosts_load_error: Option<String>` 字段，
  default None；`new()` 内 init None
- `app.rs`：现有 `load hosts.json failed: ... — starting with empty list`
  改为：
  ```rust
  Err(e) => {
      tracing::error!("load hosts.json failed: {}", e);
      state.hosts_load_error = Some(format!("{}", e));
      Vec::new()
  }
  ```
- `views/home.rs` render：
  - hosts_load_error == Some(err) → 渲染 ErrorState 替代 hosts 列表
    + EmptyState：
    ```rust
    ErrorState::new("home-hosts-load-failed")
        .icon(IconName::FileQuestion)
        .title("加载主机列表失败")
        .description(err)
        .action(Button::new(...).label("重试").primary()
            .on_click(... self.handle_retry_load(cx)))
    ```
  - `handle_retry_load`：调 `crate::persistence::load_hosts()` →
    成功设 `hosts_load_error = None` + `state.hosts = vec` + cx.notify()；
    失败更新 error message
- 单测：`state.rs` 加 1 个 `hosts_load_error_default_is_none`

**质量门禁**: 临时把 hosts.json 写坏 → 启动 aish → Home 显 ErrorState +
点重试可恢复（先恢复 json 内容再点）。

---

### T8: 文档 + INDEX

- 回填 spec 末尾"实施记录"节（commits 表 + Risk 实际遇到 + 测试增量 +
  未做 / 跨主题验证）
- 更新 `docs/superpowers/INDEX.md`：
  - "当前状态" 节 → "M28 已完成（2026-05-15）"
  - "下一里程碑" 候选清单删 "State design"
  - "Milestones（按时间倒序）" 顶部插入 M28 entry（范围 / 关键 commits /
    测试数 / 已知边界）
- 不写 .md 报告文件，最后 commit 收尾即可

---

## Self-Review Checklist

- [ ] D-1 EmptyState anatomy（4-slot）由 T2 落地
- [ ] D-2 Skeleton block/circle 由 T3 落地（shimmer 留 stub）
- [ ] D-3 错误页 vs toast 边界：T7 落 inline ErrorState，toast 系统不动
- [ ] D-4 5 个新 IconName + 文案规则各 view 改造时遵守
- [ ] D-5 不引入插画 / i18n（无新依赖）
- [ ] D-6 SSH Connecting overlay / host_form 不动（T1-T7 文件清单不含）
- [ ] D-7 改造范围限定 3 view + 1 error path
- [ ] Risk R1-R8 各有 mitigation 落地或显式接受
- [ ] T1-T3 每条带单测覆盖
- [ ] T4-T7 每条 view 改造后手测视觉清单
- [ ] commits 严格按 task 顺序，每条独立

---

## 实施顺序与依赖

```
T1 (icons +5) ──┐
                 ├→ T2 (EmptyState / ErrorState) ──┐
T3 (Skeleton)   ─┘                                  ├→ T4 (home empty)
                                                    ├→ T5 (empty_terminal)
                                                    ├→ T6 (session_picker)
                                                    └→ T7 (hosts ErrorState)
                                                          ↓
                                                        T8 (docs)
```

- T1 必须先（T2 / T4-T7 用到新 IconName）
- T2 + T3 独立可并行（但 T2 优先：T4-T7 都依赖它）
- T4-T7 互相独立，按"用户最先看到 → 后看到"排：T4 (Home) > T5
  (Terminal) > T6 (SessionPicker dialog) > T7 (Error path 仅在 json
  corrupt 时触发，频率最低)
- T8 收尾在最后

---

## 测试增量预估

| Task | aish-ui | aish-app |
|---|---|---|
| T1 | 0（asset 测试已有覆盖） | 0 |
| T2 | +4-6（EmptyState / ErrorState anatomy） | 0 |
| T3 | +2（Skeleton block/circle） | 0 |
| T4 | 0（手测） | 0 |
| T5 | 0 | 0 |
| T6 | 0 | 0 |
| T7 | 0 | +1-2（hosts_load_error 默认 / retry 逻辑） |
| T8 | 0 | 0 |
| **总** | **+6-8** | **+1-2** |

预期：aish-ui 211 → ~218；aish-app 144 → ~146。
