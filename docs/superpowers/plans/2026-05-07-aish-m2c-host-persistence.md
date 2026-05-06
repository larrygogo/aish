# aish M2c — host 持久化 + GUI 增删改实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 引入 `dirs` crate 跨平台 host 持久化路径，新建 `persistence.rs`（load/save hosts.json + 原子 tmp+rename），AppState 加 modal 状态 + add/update/remove host 方法，新建 `views/host_form.rs`（modal 表单 + 删除确认 + 校验），HostListView 加"+"按钮 + hover ✏️/🗑 icons，app.rs 启动时读 hosts.json 替代 fixtures.rs。完成后用户**不设环境变量**也能在 GUI 添加 host 并连。

**Architecture:** 新增 `persistence` 纯函数模块负责 JSON 读写（无状态，便于单元测试）。AppState 加 `modal: Option<HostFormState>` 字段，open/close modal 都是 GPUI Model 状态变化，Views 订阅自动重渲染。Modal 渲染走 RootView 的 overlay 层（z-index 高），不依赖 Zed Workspace::toggle_modal。每次 add/update/remove 都通过 cx.spawn 立即触发 persistence::save。

**Tech Stack:** dirs (跨平台 config dir), serde_json (JSON), tempfile (单元测试用), aish_types::HostConfig (复用 M0 已有 serde derive), GPUI overlay layer

**前置:** M2b1 已完成（commit `ea977ba` 之后），M2c spec 已落盘 (`docs/superpowers/specs/2026-05-07-aish-m2c-host-persistence-design.md`, commit `1212804`)。

---

## ⚠️ 实施者须知

### dirs crate

跨平台 config 目录用 `dirs::config_dir()`：
- Windows: `%APPDATA%` = `C:\Users\<user>\AppData\Roaming`
- macOS: `~/Library/Application Support`
- Linux: `~/.config`

aish hosts.json 完整路径 = `{config_dir}/aish/hosts.json`。

### Modal overlay 的 GPUI 实现

参考 Zed 的 picker / modal 模式：

```bash
cat C:/Users/larry/Desktop/workspace/zed-reference/crates/picker/src/picker.rs 2>$null | head -120
grep -rn 'fn render.*modal\|on_action.*Cancel\|absolute' C:/Users/larry/Desktop/workspace/zed-reference/crates/picker/src/ 2>$null | head -20
```

GPUI 提供 `absolute()` + `inset_0()` 让 div 定位到父容器全屏；半透明背景 + 居中表单。无需 z-index 概念，div 后写就在前层。

### Demo 验证 implementer 跑不了

implementer subagent 没有 GUI 环境（cargo run 启动后窗口在远程不可见），demo 由 user 在 Task 6 手动 17 项验证。subagent 验证只确认 build / test / fmt / clippy / 启动无 panic。

### key path 校验

校验"key 文件存在"用 `std::path::Path::exists()`。如果 path 含相对路径 / `~`，**不展开**（用户输什么校验什么；M2c 接受手输绝对路径）。

---

## File Structure（M2c 完成时）

```
aish/
├── Cargo.toml                     # 加 dirs + tempfile workspace dep
├── crates/aish-app/
│   ├── Cargo.toml                 # 加 dirs（dev-dep tempfile 在 [dev-dependencies]）
│   └── src/
│       ├── main.rs                # 修改：删 mod fixtures，加 mod persistence
│       ├── app.rs                 # 修改：启动 persistence::load_hosts；RootView 加 modal overlay
│       ├── state.rs               # 修改：加 HostFormState/HostFormDraft + add_host/update_host/remove_host
│       ├── persistence.rs         # 新：load_hosts + save_hosts + 原子 tmp+rename
│       ├── bridge.rs              # 不变
│       ├── ssh_actor.rs           # 不变
│       ├── fixtures.rs            # ❌ 删除
│       ├── terminal/              # 不变
│       └── views/
│           ├── mod.rs             # 修改：reexport HostFormModal
│           ├── host_list.rs       # 修改：加 "+" 按钮 + hover ✏️/🗑 icons
│           ├── host_form.rs       # 新：HostFormModal + DeleteConfirmModal
│           └── terminal_view.rs   # 不变
```

新增 2 文件：persistence.rs / views/host_form.rs
删除 1 文件：fixtures.rs
修改 6 文件 + 2 个 Cargo.toml

---

## Task 1: dirs dep + persistence.rs

**Files:**
- Modify: `Cargo.toml`（workspace.dependencies 加 dirs + tempfile）
- Modify: `crates/aish-app/Cargo.toml`（加 dirs + dev-dep tempfile）
- Create: `crates/aish-app/src/persistence.rs`
- Modify: `crates/aish-app/src/main.rs`（加 mod persistence）

- [ ] **Step 1: workspace 根 Cargo.toml 加 dirs + tempfile**

读 `Cargo.toml`，在 `[workspace.dependencies]` 段末尾追加：

```toml
dirs = "5.0"
tempfile = "3.10"
```

如果某版本不存在，`cargo search dirs --limit 3` 看实际可用版本。

- [ ] **Step 2: aish-app/Cargo.toml 加 dirs + dev tempfile**

在 `[dependencies]` 末尾加：

```toml
dirs = { workspace = true }
serde_json = { workspace = true }
```

注意：serde_json 在 workspace.dependencies 已经有（M0 加的），但 aish-app 之前没用过；persistence 要用所以现在加。

如果 aish-app/Cargo.toml 没有 `[dev-dependencies]` section，加：

```toml
[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 3: 创建 `crates/aish-app/src/persistence.rs`**

```rust
//! hosts.json 跨平台持久化读写。
//!
//! 路径：{config_dir}/aish/hosts.json
//!   - Windows: %APPDATA%\aish\hosts.json
//!   - macOS:   ~/Library/Application Support/aish/hosts.json
//!   - Linux:   ~/.config/aish/hosts.json
//!
//! 写入用原子 tmp+rename 防半写损坏。

#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aish_types::HostConfig;

/// 配置子目录名。
const APP_DIR_NAME: &str = "aish";
/// hosts 文件名。
const HOSTS_FILE: &str = "hosts.json";

/// 拿 hosts.json 完整路径（不保证父目录存在）。
pub fn hosts_json_path() -> Option<PathBuf> {
    let mut p = dirs::config_dir()?;
    p.push(APP_DIR_NAME);
    p.push(HOSTS_FILE);
    Some(p)
}

/// 加载 hosts.json。
///
/// - 文件不存在 → Ok(vec![])
/// - 文件存在但 parse 失败 → Err(io / serde_json error wrap 在 LoadError 里)
pub fn load_hosts() -> Result<Vec<HostConfig>, LoadError> {
    let path = hosts_json_path().ok_or(LoadError::NoConfigDir)?;
    load_hosts_from(&path)
}

/// 测试用：从指定 path 加载（绕过 dirs::config_dir）。
pub fn load_hosts_from(path: &Path) -> Result<Vec<HostConfig>, LoadError> {
    match fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map_err(LoadError::Parse),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(LoadError::Io(e)),
    }
}

/// 保存 hosts.json（原子 tmp+rename）。
///
/// 自动 mkdir -p 父目录。
pub fn save_hosts(hosts: &[HostConfig]) -> Result<(), SaveError> {
    let path = hosts_json_path().ok_or(SaveError::NoConfigDir)?;
    save_hosts_to(&path, hosts)
}

/// 测试用：保存到指定 path。
pub fn save_hosts_to(path: &Path, hosts: &[HostConfig]) -> Result<(), SaveError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SaveError::Io)?;
    }

    let json = serde_json::to_string_pretty(hosts).map_err(SaveError::Serialize)?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json).map_err(SaveError::Io)?;
    fs::rename(&tmp_path, path).map_err(SaveError::Io)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("config directory not found (HOME / APPDATA unset?)")]
    NoConfigDir,
    #[error("read hosts.json failed: {0}")]
    Io(#[source] io::Error),
    #[error("parse hosts.json failed: {0}")]
    Parse(#[source] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("config directory not found")]
    NoConfigDir,
    #[error("io error: {0}")]
    Io(#[source] io::Error),
    #[error("serialize failed: {0}")]
    Serialize(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use aish_types::{HostId, SshAuth};
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn mk_host(label: &str) -> HostConfig {
        HostConfig {
            id: HostId(Uuid::new_v4()),
            label: label.into(),
            host: "1.2.3.4".into(),
            port: 22,
            user: "root".into(),
            auth: SshAuth::KeyFile {
                path: PathBuf::from("/home/me/.ssh/id_ed25519"),
            },
            env_profile: None,
        }
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let result = load_hosts_from(&path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("aish").join("hosts.json");
        let original = vec![mk_host("vps-1"), mk_host("vps-2")];

        save_hosts_to(&path, &original).unwrap();

        let loaded = load_hosts_from(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].label, "vps-1");
        assert_eq!(loaded[1].label, "vps-2");
        assert_eq!(loaded[0].id, original[0].id);
    }

    #[test]
    fn save_creates_parent_dir_if_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("c").join("hosts.json");
        let hosts = vec![mk_host("vps")];
        save_hosts_to(&path, &hosts).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn save_atomic_cleans_up_tmp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let tmp = path.with_extension("json.tmp");
        let hosts = vec![mk_host("vps")];

        save_hosts_to(&path, &hosts).unwrap();

        // tmp 文件应被 rename 消除
        assert!(!tmp.exists(), "tmp file should not remain after rename");
        assert!(path.exists());
    }

    #[test]
    fn load_returns_parse_error_on_corrupt_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        fs::write(&path, b"this is not json").unwrap();

        let result = load_hosts_from(&path);
        assert!(matches!(result, Err(LoadError::Parse(_))));
    }

    #[test]
    fn save_then_load_empty_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let original: Vec<HostConfig> = Vec::new();

        save_hosts_to(&path, &original).unwrap();
        let loaded = load_hosts_from(&path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn hosts_json_path_returns_some_on_typical_env() {
        // CI 与开发机都应该能拿到（HOME / APPDATA 必存在）
        let p = hosts_json_path();
        assert!(p.is_some());
        let p = p.unwrap();
        assert!(p.ends_with("aish/hosts.json") || p.ends_with("aish\\hosts.json"));
    }
}
```

- [ ] **Step 4: main.rs 加 mod persistence**

读 main.rs 当前 mod 列表：

```rust
mod app;
mod bridge;
mod fixtures;
mod ssh_actor;
mod state;
mod terminal;
mod views;
```

加 `mod persistence;`（按字母序在 mod fixtures 后、mod ssh_actor 前）：

```rust
mod app;
mod bridge;
mod fixtures;
mod persistence;
mod ssh_actor;
mod state;
mod terminal;
mod views;
```

- [ ] **Step 5: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -5
cargo test -p aish-app persistence 2>&1 | tail -10
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 7 个 persistence 测试通过 + workspace 全绿（59 → 66 passed）。

- [ ] **Step 6: commit**

```bash
cargo fmt --all
git add Cargo.toml Cargo.lock crates/aish-app/Cargo.toml crates/aish-app/src/persistence.rs crates/aish-app/src/main.rs
git commit -m "feat(aish-app): persistence 模块（hosts.json 原子读写）"
```

---

## Task 2: state.rs 加 modal 状态 + add/update/remove host 方法

**Files:**
- Modify: `crates/aish-app/src/state.rs`

- [ ] **Step 1: 加新类型 HostFormState + HostFormDraft**

读 `crates/aish-app/src/state.rs`，在文件末尾的 `#[cfg(test)] mod tests` **之前**追加：

```rust
/// modal 状态：当前是否在添加 / 编辑 host。
#[derive(Debug)]
pub enum HostFormState {
    Adding(HostFormDraft),
    Editing { id: HostId, draft: HostFormDraft },
    DeleteConfirm { id: HostId, label: String },
}

/// 表单中间状态。port 用 String 让用户能临时输入非数字，提交时校验。
#[derive(Debug, Default, Clone)]
pub struct HostFormDraft {
    pub label: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub key_path: String,
    /// 校验失败时显示在 modal 底部的红字。
    pub error: Option<String>,
}

impl HostFormDraft {
    /// 从已有 HostConfig 填充（用于编辑）。
    pub fn from_config(cfg: &HostConfig) -> Self {
        let key_path = match &cfg.auth {
            aish_types::SshAuth::KeyFile { path } => path.display().to_string(),
            _ => String::new(),
        };
        Self {
            label: cfg.label.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            key_path,
            error: None,
        }
    }

    /// 校验并转回 HostConfig。`id` Some 表示编辑（保留原 id）/ None 表示新建。
    pub fn into_config(&self, id: Option<HostId>) -> Result<HostConfig, String> {
        if self.label.trim().is_empty() {
            return Err("label 不能为空".into());
        }
        if self.host.trim().is_empty() {
            return Err("host 不能为空".into());
        }
        let port: u16 = self
            .port
            .trim()
            .parse()
            .map_err(|_| "port 必须是 1-65535 的数字".to_string())?;
        if self.user.trim().is_empty() {
            return Err("user 不能为空".into());
        }
        let key_path = self.key_path.trim();
        if key_path.is_empty() {
            return Err("key path 不能为空".into());
        }
        let key_pathbuf = std::path::PathBuf::from(key_path);
        if !key_pathbuf.exists() {
            return Err(format!("key 文件不存在: {}", key_path));
        }

        Ok(HostConfig {
            id: id.unwrap_or_else(HostId::new),
            label: self.label.trim().into(),
            host: self.host.trim().into(),
            port,
            user: self.user.trim().into(),
            auth: aish_types::SshAuth::KeyFile { path: key_pathbuf },
            env_profile: None,
        })
    }
}
```

注意：`HostId::new()` — aish_types 是否有这个构造？看 aish-types/src/lib.rs，如果没有，加 `aish_types::HostId(uuid::Uuid::new_v4())` 直接构造（因为 HostId 是 `pub struct HostId(pub Uuid)`，pub 字段）。如果 HostId::new 已经存在就用它。

读 `crates/aish-types/src/lib.rs` 确认 HostId 形态：

```bash
cat crates/aish-types/src/lib.rs | head -50
```

如果没 `new()`，用直接构造：

```rust
HostId(uuid::Uuid::new_v4())
```

替换 plan 中的 `HostId::new()`。

- [ ] **Step 2: AppState 加 modal 字段 + 方法**

在 AppState struct 加 modal 字段：

```rust
#[derive(Default)]
pub struct AppState {
    pub hosts: Vec<HostConfig>,
    pub selected: Option<HostId>,
    pub pane_terminals: HashMap<HostId, Term<VoidListener>>,
    pub pane_dimensions: HashMap<HostId, (u16, u16)>,
    pub sessions: HashMap<HostId, mpsc::Sender<SessionCommand>>,
    /// 当前打开的 modal（添加/编辑/删除确认）；None = 无 modal
    pub modal: Option<HostFormState>,
}
```

在 `impl AppState` 末尾追加方法：

```rust
    /// 添加一个新 host。
    pub fn add_host(&mut self, cfg: HostConfig) {
        self.hosts.push(cfg);
    }

    /// 替换已有 host（保持 id 不变；新 cfg.id 应等于 id）。
    /// 返回 true = 成功替换，false = id 未找到。
    pub fn update_host(&mut self, id: HostId, cfg: HostConfig) -> bool {
        if let Some(slot) = self.hosts.iter_mut().find(|h| h.id == id) {
            *slot = cfg;
            true
        } else {
            false
        }
    }

    /// 删除 host。同步清理 sessions / pane_terminals / pane_dimensions / 重置 selected。
    /// 返回 true = 成功删除，false = 未找到。
    pub fn remove_host(&mut self, id: HostId) -> bool {
        let idx = match self.hosts.iter().position(|h| h.id == id) {
            Some(i) => i,
            None => return false,
        };
        self.hosts.remove(idx);
        self.sessions.remove(&id);
        self.pane_terminals.remove(&id);
        self.pane_dimensions.remove(&id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        true
    }
}
```

- [ ] **Step 3: 加单元测试**

在 `mod tests` 末尾追加（在 `}` 之前）：

```rust
    fn write_temp_key_file() -> tempfile::NamedTempFile {
        tempfile::NamedTempFile::new().expect("temp file")
    }

    #[test]
    fn draft_into_config_validates_empty_label() {
        let draft = HostFormDraft {
            label: "".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: "/tmp/x".into(),
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("label"));
    }

    #[test]
    fn draft_into_config_validates_port_non_numeric() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "abc".into(),
            user: "root".into(),
            key_path: "/tmp/x".into(),
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.unwrap_err().contains("port"));
    }

    #[test]
    fn draft_into_config_validates_key_file_exists() {
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: "/nonexistent/path".into(),
            error: None,
        };
        let r = draft.into_config(None);
        assert!(r.unwrap_err().contains("key 文件不存在"));
    }

    #[test]
    fn draft_into_config_succeeds_with_existing_key() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: key.path().display().to_string(),
            error: None,
        };
        let cfg = draft.into_config(None).unwrap();
        assert_eq!(cfg.label, "v");
        assert_eq!(cfg.port, 22);
    }

    #[test]
    fn draft_into_config_preserves_id_when_provided() {
        let key = write_temp_key_file();
        let draft = HostFormDraft {
            label: "v".into(),
            host: "1.2.3.4".into(),
            port: "22".into(),
            user: "root".into(),
            key_path: key.path().display().to_string(),
            error: None,
        };
        let id = HostId(Uuid::new_v4());
        let cfg = draft.into_config(Some(id)).unwrap();
        assert_eq!(cfg.id, id);
    }

    #[test]
    fn draft_from_config_extracts_key_path() {
        let h = mk_host("v");
        let draft = HostFormDraft::from_config(&h);
        assert_eq!(draft.label, "v");
        assert_eq!(draft.port, "22");
        assert!(draft.key_path.contains("id_ed25519"));
    }

    #[test]
    fn add_host_appends() {
        let mut state = AppState::with_hosts(vec![]);
        let h = mk_host("v");
        state.add_host(h.clone());
        assert_eq!(state.hosts.len(), 1);
        assert_eq!(state.hosts[0].id, h.id);
    }

    #[test]
    fn update_host_replaces_in_place() {
        let h1 = mk_host("orig");
        let id = h1.id;
        let mut state = AppState::with_hosts(vec![h1]);

        let mut new_cfg = mk_host("renamed");
        new_cfg.id = id; // 保持 id
        let ok = state.update_host(id, new_cfg);
        assert!(ok);
        assert_eq!(state.hosts[0].label, "renamed");
    }

    #[test]
    fn update_host_returns_false_for_unknown_id() {
        let mut state = AppState::with_hosts(vec![]);
        let unknown = HostId(Uuid::new_v4());
        let cfg = mk_host("x");
        let ok = state.update_host(unknown, cfg);
        assert!(!ok);
    }

    #[test]
    fn remove_host_clears_related_state() {
        let h = mk_host("v");
        let id = h.id;
        let mut state = AppState::with_hosts(vec![h]);
        state.feed_bytes(id, b"hello"); // 创建 Term
        let (tx, _rx) = mpsc::channel::<SessionCommand>(8);
        state.register_session(id, tx);
        state.select_host(id);

        let ok = state.remove_host(id);
        assert!(ok);
        assert!(state.hosts.is_empty());
        assert!(state.pane_terminals.get(&id).is_none());
        assert!(state.pane_dimensions.get(&id).is_none());
        assert!(!state.is_session_active(id));
        assert_eq!(state.selected, None);
    }

    #[test]
    fn remove_host_returns_false_for_unknown_id() {
        let mut state = AppState::with_hosts(vec![]);
        let unknown = HostId(Uuid::new_v4());
        assert!(!state.remove_host(unknown));
    }
```

注意 `write_temp_key_file` 用 `tempfile::NamedTempFile`，需要 dev-dependencies 含 tempfile（Task 1 已加）。

- [ ] **Step 2.5: 在测试 mod 顶部加 use tempfile**

如果 `mod tests` 顶部还没 `use tempfile;`，加上 `use tempfile;`（实际上 `tempfile::NamedTempFile::new()` 用全限定路径就不用 use，下方代码用的是 `tempfile::NamedTempFile`，OK）。

- [ ] **Step 4: 验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -5
cargo test -p aish-app state 2>&1 | tail -15
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: state 测试 6 旧 + 11 新 = 17 个；workspace 全绿（66 → 77 passed）。

- [ ] **Step 5: commit**

```bash
git add crates/aish-app/src/state.rs
git commit -m "feat(aish-app): AppState 加 modal + add/update/remove_host 方法"
```

---

## Task 3: host_form.rs modal view + 删除确认

**Files:**
- Create: `crates/aish-app/src/views/host_form.rs`
- Modify: `crates/aish-app/src/views/mod.rs`

- [ ] **Step 1: 看 GPUI 表单输入 API**

看 Zed 的简单 input field 模式：

```bash
grep -rn 'TextField\|TextInput\|on_input\|on_text_changed' C:/Users/larry/Desktop/workspace/zed-reference/crates/gpui/examples/ 2>$null | head -10
grep -rn 'pub struct.*input\|struct TextField' C:/Users/larry/Desktop/workspace/zed-reference/crates/ui/src/ 2>$null | head -10
```

⚠️ GPUI text input 历史上是 Zed 自己造的（复杂），M2c 用最简方案：

**简化方案 — `gpui::TextField` 不存在时**：用 div 渲染当前 draft 字段值 + KeyDownEvent 自己处理字符输入。每个字段一个 div，FocusHandle 控制焦点。

**或更简方案**：用一个全局 KeyDownEvent 把字符 append 到当前 focused 字段。

按 Zed examples 看到的实际 API 选最简形态。如果实在没现成 input，自实现一个 SimpleTextInput struct。

- [ ] **Step 2: 创建 `crates/aish-app/src/views/host_form.rs`**

```rust
//! 添加/编辑/删除确认 modal。
//!
//! 三种状态由 AppState.modal 决定：
//!   - HostFormState::Adding(draft) — 添加模式
//!   - HostFormState::Editing { id, draft } — 编辑模式
//!   - HostFormState::DeleteConfirm { id, label } — 删除确认

use std::sync::Arc;

use aish_types::HostId;
use gpui::{
    div, hsla, prelude::*, px, rgb, App, Bounds, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, MouseButton, MouseDownEvent, Pixels, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::persistence;
use crate::state::{AppState, HostFormDraft, HostFormState};

/// 当前 focus 的 input 字段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Label,
    Host,
    Port,
    User,
    KeyPath,
}

impl FocusField {
    fn next(self) -> Self {
        match self {
            FocusField::Label => FocusField::Host,
            FocusField::Host => FocusField::Port,
            FocusField::Port => FocusField::User,
            FocusField::User => FocusField::KeyPath,
            FocusField::KeyPath => FocusField::Label,
        }
    }
}

pub struct HostFormModal {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    focus_handle: FocusHandle,
    focus_field: FocusField,
}

impl HostFormModal {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self {
            state,
            bridge,
            focus_handle: cx.focus_handle(),
            focus_field: FocusField::Label,
        }
    }

    /// 把字符 append 到当前 focused 字段。
    fn append_char(&mut self, c: char, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                let target = match self.focus_field {
                    FocusField::Label => &mut draft.label,
                    FocusField::Host => &mut draft.host,
                    FocusField::Port => &mut draft.port,
                    FocusField::User => &mut draft.user,
                    FocusField::KeyPath => &mut draft.key_path,
                };
                target.push(c);
                draft.error = None;
                cx.notify();
            }
        });
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                let target = match self.focus_field {
                    FocusField::Label => &mut draft.label,
                    FocusField::Host => &mut draft.host,
                    FocusField::Port => &mut draft.port,
                    FocusField::User => &mut draft.user,
                    FocusField::KeyPath => &mut draft.key_path,
                };
                target.pop();
                draft.error = None;
                cx.notify();
            }
        });
    }

    fn cycle_focus(&mut self, cx: &mut Context<Self>) {
        self.focus_field = self.focus_field.next();
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.modal = None;
            cx.notify();
        });
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let result = self.state.update(cx, |state, cx| {
            let modal = state.modal.take();
            let action = match modal {
                Some(HostFormState::Adding(draft)) => Some(("add", None, draft)),
                Some(HostFormState::Editing { id, draft }) => Some(("edit", Some(id), draft)),
                Some(HostFormState::DeleteConfirm { id, label }) => {
                    state.remove_host(id);
                    cx.notify();
                    return Some(Ok(format!("removed {}", label)));
                }
                None => None,
            };

            if let Some((kind, id_opt, draft)) = action {
                match draft.into_config(id_opt) {
                    Ok(cfg) => {
                        if kind == "add" {
                            state.add_host(cfg);
                        } else if let Some(id) = id_opt {
                            state.update_host(id, cfg);
                        }
                        cx.notify();
                        Some(Ok(format!("saved")))
                    }
                    Err(err) => {
                        // 校验失败：把 modal 放回去含 error，不关闭
                        let mut new_draft = draft.clone();
                        new_draft.error = Some(err);
                        state.modal = Some(if let Some(id) = id_opt {
                            HostFormState::Editing { id, draft: new_draft }
                        } else {
                            HostFormState::Adding(new_draft)
                        });
                        cx.notify();
                        Some(Err(()))
                    }
                }
            } else {
                None
            }
        });

        if let Some(Ok(_)) = result {
            // 触发持久化
            let hosts = self.state.read(cx).hosts.clone();
            self.bridge.spawn(async move {
                if let Err(e) = persistence::save_hosts(&hosts) {
                    tracing::error!("save hosts.json failed: {}", e);
                }
            });
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str().to_lowercase();
        match key.as_str() {
            "escape" | "esc" => self.cancel(cx),
            "tab" => self.cycle_focus(cx),
            "enter" => self.save(cx),
            "backspace" => self.backspace(cx),
            s if s.len() == 1 => {
                if let Some(c) = s.chars().next() {
                    // 用原 key 保留大小写
                    let real = event.keystroke.key.as_str();
                    if let Some(ch) = real.chars().next() {
                        self.append_char(ch, cx);
                    } else {
                        self.append_char(c, cx);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Focusable for HostFormModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HostFormModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal = self.state.read(cx).modal.as_ref();
        let body = match modal {
            None => return div().into_any_element(), // 不该到这（caller 应该检查 modal Some）
            Some(HostFormState::DeleteConfirm { label, .. }) => render_delete_body(label),
            Some(HostFormState::Adding(draft)) => render_form_body("添加 host", draft, self.focus_field),
            Some(HostFormState::Editing { draft, .. }) => render_form_body("编辑 host", draft, self.focus_field),
        };

        // 全屏半透明背景 + 居中 modal 卡片
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _w, cx| this.handle_key(ev, cx)))
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.5)) // 半透明黑遮罩
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(420.0))
                    .bg(rgb(0x252525))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(0x444444))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(body)
                    .child(render_buttons(modal, cx, self.focus_handle.clone())),
            )
            .into_any_element()
    }
}

fn render_form_body(
    title: &str,
    draft: &HostFormDraft,
    focus_field: FocusField,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(16.0))
                .child(title.to_string()),
        )
        .child(field_row("label", &draft.label, focus_field == FocusField::Label))
        .child(field_row("host", &draft.host, focus_field == FocusField::Host))
        .child(field_row("port", &draft.port, focus_field == FocusField::Port))
        .child(field_row("user", &draft.user, focus_field == FocusField::User))
        .child(field_row("key path", &draft.key_path, focus_field == FocusField::KeyPath))
        .child(
            // 错误提示行
            if let Some(err) = &draft.error {
                div()
                    .text_color(rgb(0xff6666))
                    .text_size(px(12.0))
                    .child(err.clone())
                    .into_any_element()
            } else {
                div().into_any_element()
            },
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("Tab 切换字段，Enter 保存，Esc 取消"),
        )
        .into_any_element()
}

fn field_row(label: &str, value: &str, focused: bool) -> gpui::AnyElement {
    let display: SharedString = if value.is_empty() {
        "(空)".into()
    } else {
        value.to_string().into()
    };
    let border_color = if focused { rgb(0x4a90e2) } else { rgb(0x444444) };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(80.0))
                .text_color(rgb(0xaaaaaa))
                .text_size(px(13.0))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .px_2()
                .py_1()
                .bg(rgb(0x1d1d1d))
                .border_1()
                .border_color(border_color)
                .rounded(px(4.0))
                .text_color(rgb(0xeeeeee))
                .text_size(px(13.0))
                .child(display),
        )
        .into_any_element()
}

fn render_delete_body(label: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(16.0))
                .child("确认删除？"),
        )
        .child(
            div()
                .text_color(rgb(0xcccccc))
                .text_size(px(13.0))
                .child(format!("将永久删除 host: {}", label)),
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("Enter 确认，Esc 取消"),
        )
        .into_any_element()
}

fn render_buttons(
    modal: Option<&HostFormState>,
    _cx: &mut Context<HostFormModal>,
    _focus: FocusHandle,
) -> gpui::AnyElement {
    let primary_text = match modal {
        Some(HostFormState::DeleteConfirm { .. }) => "Delete",
        _ => "Save",
    };
    div()
        .flex()
        .flex_row()
        .gap_2()
        .justify_end()
        .child(
            div()
                .px_4()
                .py_1()
                .bg(rgb(0x444444))
                .text_color(rgb(0xeeeeee))
                .rounded(px(4.0))
                .child("Cancel (Esc)"),
        )
        .child(
            div()
                .px_4()
                .py_1()
                .bg(rgb(0x4a90e2))
                .text_color(rgb(0xffffff))
                .rounded(px(4.0))
                .child(format!("{} (Enter)", primary_text)),
        )
        .into_any_element()
}
```

⚠️ **设计取舍**：

- **没用真 text input control** — GPUI 没现成的简单 TextField，自己实现一个完整的（含光标 / 选中 / 复制粘贴）超出 M2c 范围。降级方案：用 div 显示当前字段值 + 全局 KeyDownEvent 把字符 append。**键盘 only 输入**，用户不能用鼠标点字段编辑（Tab 切字段）。
- 这是 M2c 接受的限制；M2b2 或后续可加真 TextField（Zed 用 `crates/ui/src/components/input` 之类，可参考）
- Save / Cancel 按钮仅显示文字，**键盘 Enter/Esc 触发**（不接 mouse_down click — 简化）
- Delete confirm 也走同 modal 框架

如果 implementer 看 Zed 找到更简单的 TextField，**优先用 Zed 的**而非 div 替代。

- [ ] **Step 3: views/mod.rs 加 reexport**

读 `crates/aish-app/src/views/mod.rs`，加 mod 声明 + reexport：

```rust
//! GPUI Views。

#![allow(dead_code)]

mod host_form;
mod host_list;
mod terminal_view;

pub use host_form::HostFormModal;
pub use host_list::HostListView;
pub use terminal_view::TerminalView;
```

- [ ] **Step 4: 验证 build**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -30
```

如失败按 GPUI API 调整。常见问题：
- `absolute()` / `inset_0()` 是 div 的方法，名字可能不同（Zed 用 Position absolute）
- `hsla(h, s, l, a)` 参数顺序
- `border_1()` 是否存在
- `gap_2()` / `gap_3()` 命名

成功后：

```bash
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 测试不变（host_form 没新增测试，因为 GUI 测试 ROI 太低），workspace 全绿。

- [ ] **Step 5: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/views/host_form.rs crates/aish-app/src/views/mod.rs
git commit -m "feat(aish-app): HostFormModal modal view（添加/编辑/删除确认）"
```

---

## Task 4: host_list.rs 加 "+" 按钮 + hover icons + 触发 modal

**Files:**
- Modify: `crates/aish-app/src/views/host_list.rs`

- [ ] **Step 1: 改写 host_list.rs 加 + 按钮 + hover icons**

读当前 host_list.rs（M2a/M2b1 实现），整体替换为（保留 handle_click 逻辑 + 新增方法）：

```rust
//! 左栏：host 列表，"+" 按钮 + hover ✏️/🗑 icons + 点击触发 SSH 连接。

use std::sync::Arc;

use aish_types::HostId;
use gpui::{
    div, prelude::*, px, rgb, App, Context, Entity, MouseButton, MouseDownEvent, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, HostFormDraft, HostFormState, SshEvent};

pub struct HostListView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    tx: tokio::sync::mpsc::Sender<SshEvent>,
}

impl HostListView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self { state, bridge, tx }
    }

    fn handle_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        let needs_connect = self.state.update(cx, |state, cx| {
            state.select_host(host);
            let needs = !state.is_session_active(host);
            cx.notify();
            needs
        });

        if needs_connect {
            let config = match self
                .state
                .read(cx)
                .hosts
                .iter()
                .find(|h| h.id == host)
                .cloned()
            {
                Some(c) => c,
                None => {
                    tracing::error!(?host, "host config not found");
                    return;
                }
            };

            let sender = self.bridge.spawn_session(host, config, self.tx.clone());
            self.state.update(cx, |state, _cx| {
                state.register_session(host, sender);
            });
        }
    }

    fn handle_add_click(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.modal = Some(HostFormState::Adding(HostFormDraft::default()));
            cx.notify();
        });
    }

    fn handle_edit_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(cfg) = state.hosts.iter().find(|h| h.id == host).cloned() {
                state.modal = Some(HostFormState::Editing {
                    id: host,
                    draft: HostFormDraft::from_config(&cfg),
                });
                cx.notify();
            }
        });
    }

    fn handle_delete_click(&mut self, host: HostId, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(cfg) = state.hosts.iter().find(|h| h.id == host).cloned() {
                state.modal = Some(HostFormState::DeleteConfirm {
                    id: host,
                    label: cfg.label,
                });
                cx.notify();
            }
        });
    }
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected;

        let plus_button = div()
            .px_3()
            .py_2()
            .text_color(rgb(0xaaaaaa))
            .bg(rgb(0x1e1e1e))
            .border_b_1()
            .border_color(rgb(0x333333))
            .hover(|s| s.bg(rgb(0x2a2a2a)).cursor_pointer())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_add_click(cx)),
            )
            .child("+ 添加 host");

        let host_rows: Vec<_> = state
            .hosts
            .iter()
            .map(|h| {
                let id = h.id;
                let label = h.label.clone();
                let is_selected = selected == Some(id);
                let is_active = state.is_session_active(id);
                let prefix = if is_active { "● " } else { "○ " };

                let edit_btn = div()
                    .px_1()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xffffff)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_edit_click(id, cx);
                        }),
                    )
                    .child("✏");

                let delete_btn = div()
                    .px_1()
                    .text_color(rgb(0xaaaaaa))
                    .hover(|s| s.text_color(rgb(0xff6666)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_delete_click(id, cx);
                        }),
                    )
                    .child("🗑");

                div()
                    .group("host_row")
                    .px_3()
                    .py_2()
                    .text_color(rgb(if is_selected { 0xffffff } else { 0xcccccc }))
                    .bg(rgb(if is_selected { 0x2a2a2a } else { 0x1e1e1e }))
                    .hover(|s| s.bg(rgb(0x252525)).cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_click(id, cx);
                        }),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().flex_1().child(format!("{}{}", prefix, label)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .visible_on_hover("host_row")
                            .child(edit_btn)
                            .child(delete_btn),
                    )
            })
            .collect();

        let empty_hint = if state.hosts.is_empty() {
            Some(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x888888))
                    .text_size(px(12.0))
                    .child("(无 host：点上方 + 添加)"),
            )
        } else {
            None
        };

        div()
            .w(px(220.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x333333))
            .flex()
            .flex_col()
            .child(plus_button)
            .children(host_rows)
            .children(empty_hint)
    }
}
```

⚠️ **GPUI-API 不确定点**：
- `.group("name")` + `.visible_on_hover("name")` 是否存在 — Tailwind/GPUI 的 group hover 模式
- 如果不存在，简化方案：始终显示 ✏️/🗑（不 hover），visual 略丑但 work
- `text_size(px(12.0))` 方法名按当前 GPUI 调整

降级方案（如 group hover 不行）：删 `.group("host_row")` + `.visible_on_hover("host_row")`，让 icons 始终可见。

- [ ] **Step 2: 验证 build**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build -p aish-app 2>&1 | tail -20
```

如 group/visible_on_hover 不存在，按错误调整（删掉这两行让 icons 始终可见）。

```bash
cargo test --workspace 2>&1 | grep -E "test result" | head -5
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 3: commit**

```bash
cargo fmt --all
git add crates/aish-app/src/views/host_list.rs
git commit -m "feat(aish-app): HostListView 加 + 按钮 + hover ✏️/🗑 icons"
```

---

## Task 5: app.rs 启动 load + RootView 含 modal overlay + 删 fixtures.rs

**Files:**
- Modify: `crates/aish-app/src/app.rs`
- Modify: `crates/aish-app/src/main.rs`（删 mod fixtures）
- Delete: `crates/aish-app/src/fixtures.rs`

- [ ] **Step 1: 改写 app.rs**

读 app.rs。需要 3 处改动：

**1. run() 中替换 fixtures::dev_hosts 为 persistence::load_hosts**

找到：

```rust
let hosts = crate::fixtures::dev_hosts();
let state = cx.new(|_cx| AppState::with_hosts(hosts));
```

替换为：

```rust
let hosts = match crate::persistence::load_hosts() {
    Ok(h) => h,
    Err(e) => {
        tracing::error!("load hosts.json failed: {} — starting with empty list", e);
        Vec::new()
    }
};
let state = cx.new(|_cx| AppState::with_hosts(hosts));
```

**2. RootView 加 modal entity 字段**

找到 RootView struct + new + Render impl，整体替换为：

```rust
struct RootView {
    state: Entity<AppState>,
    host_list: Entity<crate::views::HostListView>,
    terminal: Entity<crate::views::TerminalView>,
    host_form: Entity<crate::views::HostFormModal>,
}

impl RootView {
    fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        let host_list = cx.new(|cx| {
            crate::views::HostListView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let terminal = cx.new(|cx| {
            crate::views::TerminalView::new(state.clone(), bridge.clone(), tx.clone(), cx)
        });
        let host_form = cx.new(|cx| {
            crate::views::HostFormModal::new(state.clone(), bridge.clone(), cx)
        });
        Self { state, host_list, terminal, host_form }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal_open = self.state.read(cx).modal.is_some();

        let main = div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x1d1f21))
            .child(self.host_list.clone())
            .child(self.terminal.clone());

        let mut root = div().relative().size_full().child(main);

        // modal overlay 仅在 modal 打开时渲染
        if modal_open {
            root = root.child(self.host_form.clone());
        }

        root
    }
}
```

注意：
- `relative()` 给父 div 设置定位上下文，让 modal 的 `.absolute().inset_0()` 相对它定位
- modal 后于 main 添加 → 在视觉层之上（GPUI 后绘的在上）

**3. 加 use 引入 HostFormModal**

确保 app.rs 顶部 use 含 crate::views::HostFormModal（或者用 `crate::views::HostFormModal` 全限定也行）。如果通过 `crate::views` 模块访问就不用单独 use。

- [ ] **Step 2: 删 main.rs 的 mod fixtures**

读 main.rs，找 `mod fixtures;` 那行，删除。其余不动。

- [ ] **Step 3: 删除 fixtures.rs**

```bash
cd C:\Users\larry\Desktop\workspace\aish
git rm crates/aish-app/src/fixtures.rs
```

- [ ] **Step 4: 验证 build / test / run**

```bash
cargo build -p aish-app 2>&1 | tail -20
cargo test --workspace 2>&1 | grep -E "test result" | head -10
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 全绿。

后台启动验证：

```powershell
$proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","aish-app") -PassThru -RedirectStandardOutput "C:\Users\larry\Desktop\workspace\aish\target\m2c-t5-out.txt" -RedirectStandardError "C:\Users\larry\Desktop\workspace\aish\target\m2c-t5-err.txt"
Start-Sleep -Seconds 8
if ($proc.HasExited) {
    Write-Output "FAIL"
    Get-Content "C:\Users\larry\Desktop\workspace\aish\target\m2c-t5-err.txt" -Tail 30
} else {
    Write-Output "OK"
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Remove-Item "C:\Users\larry\Desktop\workspace\aish\target\m2c-t5-*.txt" -Force
```

预期：窗口能开 + 没 panic + 无 host（如果 hosts.json 不存在）+ 顶部 "+" 按钮可见。

- [ ] **Step 5: commit**

```bash
git add -A crates/aish-app/
git commit -m "feat(aish-app): app 启动 load hosts.json + RootView 含 modal overlay; 删 fixtures"
```

---

## Task 6: 端到端验证 + push（user 手动 demo）

**Files:** 无文件改动；只验证 + push。

- [ ] **Step 1: 跑全部自动化验证**

```bash
cd C:\Users\larry\Desktop\workspace\aish
cargo build --workspace
cargo test --workspace 2>&1 | grep -E "test result"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 全绿。test ~77 passed (M2b1 59 + persistence 7 + state 11 = 77)。

- [ ] **Step 2: push 到 origin**

```bash
git push origin main
```

如网络问题失败可用 `git config http.postBuffer 524288000` 后重试。

- [ ] **Step 3: 等 CI**

```bash
gh run list --limit 1
```

- [ ] **Step 4: user 手动验证 demo 17 项**

implementer 在报告中明确列出（与 spec Section 9 一致）：

```
demo 验证由 user 完成。**不**设环境变量，按 spec Section 9 17 项手动核对：

1. cargo run -p aish-app
2. 验证：
   1) 窗口启动，左栏空 + "(无 host：点上方 + 添加)" 提示 + 顶部 "+ 添加 host"
   2) 点 "+" → modal overlay 出现含表单
   3) 输入 label/host/port/user/key path（用真 VPS）→ Enter Save → modal 关 + 左栏新 host
   4) 点新 host → 连接 + shell prompt（M2a/M2b1 路径正常）
   5) 点 "+" → Esc → modal 关无变化
   6) 点 "+" → Cancel 文字按钮（如果 mouse 不通就 Esc）
   7) port 输 "abc" → Enter Save → modal 底部红字 "port 必须是 1-65535 的数字"
   8) key path 不存在 → Save → 红字 "key 文件不存在"
   9) Hover host row → ✏️/🗑 icons 显现（如降级方案则始终显示）
   10) 点 ✏️ → modal 含预填字段；改 label → Save → 左栏 row 文本更新
   11) 点 🗑 → 确认 modal 出现 → Enter → row 消失
   12) 点 🗑 → Esc → 不删
   13) 关窗口 → 重启 → 左栏自动恢复 host（hosts.json 持久化）
   14) 手动改 hosts.json 加 garbage 字段 → 重启 → 启动正常但 hosts 空，tracing 日志含 parse 错误
   15) cargo test --workspace 全绿
   16) cargo fmt + clippy 全绿
   17) GitHub Actions CI 三平台全绿
```

- [ ] **Step 5: 完成报告**

```
STATUS: DONE_WITH_PENDING_USER_VERIFICATION

Files: 无文件改动

Verification (subagent 跑的):
- cargo build: PASS
- cargo test --workspace: X passed
- cargo fmt --check: PASS
- cargo clippy: PASS
- git push: 成功 / 失败
- CI 状态: started / completed / failed

Pending (user 手动):
- demo 17 项（spec Section 9）

Concerns: ...
```

---

## 完成验证（M2c 整体）

```bash
cargo build --workspace
cargo test --workspace                                 # ~77 passed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p aish-app                                  # demo 17 项 user 手动
```

git log 应有 5 个新 commit + spec + plan = 7 个新 commit。

---

## 下一步

M2c 完成后回到 M2b2（DECCKM 应用键模式 + F1-F12 / Alt+ / Shift+Tab + IME + scrollback 滚动 UI）。

M2c 留下的 onramp：
- Modal 框架可复用：M2b2 如要"键盘 cheatsheet" 弹窗、settings 弹窗等
- persistence 模块为后续 settings / theme 配置铺路
- AppState.hosts 已是真持久化来源；M2b2 不动 host 管理

M2b2 不在本 plan 范围。M2c 完成后单独 brainstorm → spec → plan → implement。
