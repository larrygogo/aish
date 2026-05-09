# M4b — Recent 持久化 + Settings 起步 Design Spec

**日期**：2026-05-08  
**里程碑**：M4b  
**依赖**：M4a（4-tab 信息架构，已完成）  
**估算**：1-2 天

---

## 1. 目标

1. Host 卡片显示上次连接时间（持久化到 `app_state.toml`）
2. Settings tab 替换占位页，展示 App 信息 / 快捷键 / 关于三段内容

---

## 2. 数据层 — `app_state.toml`

### 2.1 文件路径

```
{config_dir}/aish/app_state.toml
  Windows：%APPDATA%\aish\app_state.toml
  macOS：  ~/Library/Application Support/aish/app_state.toml
  Linux：  ~/.config/aish/app_state.toml
```

与 `hosts.json` 同目录，分离配置（hosts）与运行时状态（recent）。

### 2.2 文件格式

```toml
[recent]
# HostId（UUID 字符串） = Unix timestamp（秒，u64）
"550e8400-e29b-41d4-a716-446655440000" = 1715174400
```

### 2.3 新模块 `crates/aish-app/src/app_state_file.rs`

```rust
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppStateFile {
    #[serde(default)]
    pub recent: HashMap<String, u64>,   // HostId.to_string() → secs since UNIX_EPOCH
}

/// 路径：{config_dir}/aish/app_state.toml
pub fn app_state_path() -> Option<PathBuf>;
pub fn load_app_state() -> AppStateFile;          // 文件不存在 → Default
pub fn save_app_state(s: &AppStateFile);          // 原子 tmp+rename，失败仅 warn
```

- `load_app_state` 失败（解析错误 / IO 错误）时 `tracing::warn` 并返回 `Default`，不 panic
- `save_app_state` 失败时 `tracing::warn`，不向上传播（recent 是 best-effort）
- 依赖：`toml` crate（`serde` feature）加入 `aish-app/Cargo.toml`

### 2.4 `AppState` 新字段

```rust
// state.rs
pub last_connected: HashMap<HostId, SystemTime>,
```

- `with_hosts` 构造时传入（从 `app_state_file::load_app_state()` 读取）
- 写入时机：`HomeView::handle_card_click` 连接后（记录用户意图，不等 SSH 握手）

### 2.5 写入流程

```
HomeView::handle_card_click(host_id)
  → state.last_connected.insert(host_id, SystemTime::now())
  → app_state_file::save_app_state(&snapshot)   // 后台 best-effort
```

`save_app_state` 在 UI 线程同步调用（文件小，写入 < 1ms，不需要 spawn）。

---

## 3. HomeView — Host 卡片时间戳

### 3.1 显示规则

- `last_connected` 有该 `HostId` 的记录 → 在 host/port 行下方显示时间字符串
- 无记录 → 不显示，不占位

### 3.2 时间格式

复用与 `humanize_opened_at` 相同的阈值逻辑，函数名 `humanize_last_connected`，独立实现（接受 `SystemTime` 参数）：

| elapsed | 显示 |
|---------|------|
| < 60s | `just now` |
| < 3600s | `Xm ago` |
| < 86400s | `Xh ago` |
| < 172800s | `yesterday` |
| ≥ 172800s | `Xd ago` |

### 3.3 卡片布局变化

```
┌─────────────────────────────────────────────────┐
│ [A]  服务器A  SSH  ● 2 活跃                     │
│      root@1.2.3.4:22                            │
│      上次连接 3h ago                       ✎ ×  │  ← 新增行（仅有记录时）
└─────────────────────────────────────────────────┘
```

- 样式：`TEXT_MUTED`，`px(11.0)`，前缀 `"上次连接 "`
- 位置：host_text 行下方，与 avatar / chip / chevron 同一卡片内

---

## 4. SettingsView

### 4.1 文件

新建 `crates/aish-app/src/views/settings.rs`，替换 `app.rs` 中 Settings tab 的 `ComingSoonView`。

`SettingsView` 无状态（不依赖 `Entity<AppState>`）：

```rust
pub struct SettingsView;
impl SettingsView { pub fn new() -> Self { Self } }
```

### 4.2 布局（三段只读）

```
SETTINGS
─────────────────────────────

APP INFO
  aish  v{CARGO_PKG_VERSION}
  Built 2026-05-08

KEYBOARD SHORTCUTS
  Ctrl+Shift+V    粘贴
  Ctrl+W          关闭 tab
  Ctrl+T          新 tab
  Ctrl+1          Home
  Ctrl+2          Terminal
  Ctrl+3          Inbox
  Ctrl+4          Settings

ABOUT
  github.com/larrygogo/aish
  MIT License
```

- 版本号：`env!("CARGO_PKG_VERSION")`，编译期注入
- 构建日期：静态字符串 `"2026-05-08"`，M4b 不引入 `build.rs`
- Ctrl+1..4 快捷键仅展示，实际绑定在后续 milestone 实现
- GitHub URL 和 License 为纯文字，不需要可点击链接（GPUI 超链接较复杂，留后）

### 4.3 样式

- 区段标题：`TEXT_MUTED`，`px(11.0)`，全大写，`pt_4 pb_1`
- 内容行：`TEXT_PRIMARY` / `TEXT_SECONDARY`，`px(14.0)`
- 快捷键采用两列对齐（`flex_row` + `w(px(180.0))` 左列键名）
- 整体 `overflow_y_scroll`，`px_8 py_6`

---

## 5. Coming Soon 清理

`coming_soon.rs` 目前 Settings 和 Inbox 都用 emoji（`⚙` / `✉`）：

- **Settings**：随 SettingsView 的替换自动消失
- **Inbox**：`coming_soon.rs` 里把 `"✉"` 换成 Nerd Font 字形 `"\u{f01c}"`（fa-inbox），与 sidebar icon 保持一致

---

## 6. 文件结构

**新增：**
- `crates/aish-app/src/app_state_file.rs` — app_state.toml 读写
- `crates/aish-app/src/views/settings.rs` — SettingsView

**修改：**
- `crates/aish-app/Cargo.toml` — 加 `toml` 依赖
- `crates/aish-app/src/state.rs` — 加 `last_connected` 字段，`with_hosts` 接受参数
- `crates/aish-app/src/app.rs` — `with_hosts` 传入 loaded state；Settings tab 换 SettingsView
- `crates/aish-app/src/views/home.rs` — 卡片加时间戳；handle_card_click 写 recent
- `crates/aish-app/src/views/mod.rs` — 暴露 SettingsView
- `crates/aish-app/src/views/coming_soon.rs` — Inbox icon 换 Nerd Font

---

## 7. 测试计划

**单测（`app_state_file.rs`）：**
- `load_returns_default_when_missing` — 文件不存在返回 Default
- `save_then_load_roundtrip` — 写后读值一致
- `load_corrupt_returns_default` — parse 失败返回 Default（不 panic）
- `save_atomic_no_tmp_remains` — tmp 文件在 rename 后消失

**单测（`state.rs`）：**
- `last_connected_inserted_on_connect` — 验证 `last_connected` 字段在 `with_hosts` 后可访问

**手动验收：**
- 点击 host 卡片 → 重启 aish → 卡片显示 "just now" / 正确时间
- Settings tab 显示三段内容，版本号与 Cargo.toml 一致
- Inbox ComingSoon 图标从 emoji 变为 Nerd Font 字形

---

## 8. Out of Scope

- Settings 实际可编辑项（Appearance / Input / Notifications）— 后续 milestone
- Ctrl+1..4 键绑定实际功能 — 后续 milestone
- Recent 清除 / 上限截断（如超过 N 条）— 暂不做，HashMap 按需增长
- 构建时间戳动态注入（`build.rs`）— 暂不做
