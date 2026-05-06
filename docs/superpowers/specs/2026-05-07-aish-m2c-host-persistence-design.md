# aish M2c — host 持久化 + GUI 增删改设计

- **日期**: 2026-05-07
- **状态**: Design (approved by user, ready for implementation planning)
- **里程碑**: M2c（M2 第三阶段；M2 整体拆 M2a / M2b1 / M2b2 / M2c — 实际实施顺序：M2a → M2b1 → **M2c** → M2b2，因 M2c 解决 onboarding 痛点）
- **前置**: M2b1 已完成（commit `ea977ba` 之后；终端渲染 + PTY resize + mouse 选中复制 + 方向键就绪）
- **作者**: larry
- **预计周期**: ~1 周

---

## 1. 实施顺序调整背景

主 spec section 7 原计划是 M2b → M2c。但 M2b1 完成后用户实测时卡在"必须配环境变量才能跑"——这是 onboarding 阻塞而非 daily 体验问题。M2c（GUI 增删改 host + hosts.json 持久化）解决这个痛点的优先级**高于** M2b2（scrollback 滚动 UI + DECCKM + F1-12）。

故调整顺序：M2c 先做，M2b2 再做。M2b1 spec section 10 的"M2b1 → M2b2 演进路径"仍然适用，只是中间多了一站 M2c。

---

## 2. M2c 范围与目标

### Phase 1（M2c，本 spec 范围）

| 模块 | 范围 |
|---|---|
| `~/.aish/hosts.json` 读写 | 用 `dirs` crate 跨平台 path；启动 load；增删改立即写（原子 tmp+rename） |
| GUI 添加 host | 左栏顶部 "+" 按钮 → modal dialog 弹出表单（label/host/port/user/key path）→ Save/Cancel |
| GUI 编辑 host | 左栏 row hover 出现 ✏️ icon → 同 modal（不同 title） |
| GUI 删除 host | 左栏 row hover 出现 🗑 icon → 弹"确认删除？"→ Yes/Cancel |
| 删 fixtures.rs | M2c 完成后 dev 也走 hosts.json，环境变量读取下线 |

### 不在范围内（明确边界）

- **Password 认证 / 弹窗输密码** → 后续按需（spec ADR-1 决定 M2c 仍只 KeyFile）
- **SSH Agent 认证（Pageant / 1Password Agent）** → 后续按需
- **SecretStore 集成**（aish-secrets crate）→ M5
- **原生文件选择器（OS file dialog 选 key path）** → follow-up（M2c 用手输文本框，简单可靠）
- **导入/导出 hosts.json**（备份/同步）→ Phase 2
- **多 profile（dev/prod 不同 host group）** → Phase 2
- **Scrollback 滚动 UI / DECCKM / F1-12** → M2b2
- **链接识别 / 图像内联 / Bell** → 远期

---

## 3. 关键技术决策（M2c-ADR）

| ID | 决策 | 备选 | 理由 |
|---|---|---|---|
| M2c-ADR-1 | **最小 M2c：仍只 KeyFile，不加 Password/Agent** | 含 Password 弹窗 / 含 Agent | YAGNI；user VPS 已用 KeyFile；Password 在 modern 服务器场景少；Agent 是高级用户场景。需要时再扩 |
| M2c-ADR-2 | **modal dialog 添加/编辑 host** | inline 表单 / 独立窗口 / settings panel | 行业标准（VS Code/iTerm2）；inline 在小窗口下视觉糟；独立窗口太重；settings panel 适合多设置项场景，M2c 只有 host 一种过度 |
| M2c-ADR-3 | **删除按钮在左栏 row 上 hover 显现** | 右键菜单 / 全局 toolbar | 直接、可发现、不依赖右键事件（GPUI 右键支持按平台可能不一致） |
| M2c-ADR-4 | **hosts.json 路径用 dirs crate 跨平台** | 写死 `~/.aish/` | dirs 自动处理 Windows AppData / macOS Application Support / Linux XDG，符合各平台惯例 |
| M2c-ADR-5 | **每次改动立即写 + 原子 tmp+rename** | debounce / 显式 Save 按钮 | 防数据丢失（崩溃时已生效的改动不丢）；tmp+rename 防半写损坏；user 不必记得点 Save |
| M2c-ADR-6 | **key path 用手输文本框，不上原生文件选择器** | OS file picker | GPUI 文件 dialog API 跨平台稳定性未知；手输 + path validation 简单可靠；文件 picker 留 follow-up |
| M2c-ADR-7 | **M2c 完成后删除 fixtures.rs** | 保留作 dev 备用 | 双轨制污染（dev "为什么这个 host 没出现"是 fixtures 还是 json？）；hosts.json 是 M2c 之后唯一来源 |
| M2c-ADR-8 | **HostConfig 序列化复用 aish_types 已有的 Serialize/Deserialize derive** | 单独定义 PersistedHostConfig | DRY；M0 已经为 SshAuth/HostConfig 加了 serde derive，直接复用 |
| M2c-ADR-9 | **modal 用 RootView 持有的 overlay 实现，不用 Zed Workspace::toggle_modal** | 引入 Zed workspace crate | aish 不用 Zed workspace 框架；自己在 RootView 里加 modal Entity 字段 + z-index 高的 overlay div 层即可 |

---

## 4. 系统架构

### 总览

```
┌──────────────────────────────────────────────────────────────┐
│                  GPUI Application Process                     │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │             RootView (扩展含 modal 状态)               │   │
│  │  ┌─────────────┐ ┌────────────────┐ ┌──────────────┐ │   │
│  │  │ HostListView │ │ TerminalView   │ │ HostFormModal│ │   │
│  │  │ (左栏)       │ │ (主区，不变)    │ │ (overlay)    │ │   │
│  │  │ + "+"按钮    │ │                │ │ Add/Edit 表单 │ │   │
│  │  │ + ✏️/🗑 icons│ │                │ │              │ │   │
│  │  └──────┬──────┘ └────────────────┘ └──────┬───────┘ │   │
│  └─────────┼──────────────────────────────────┼────────┘   │
│            │                                  │             │
│  ┌─────────▼──────────────────────────────────▼────────┐   │
│  │              AppState (Model<T>)                     │   │
│  │  hosts: Vec<HostConfig>  ← 改：从 hosts.json 加载    │   │
│  │  selected / sessions / pane_terminals (M2b1 不变)    │   │
│  │  modal: Option<HostFormState>  ← 新                  │   │
│  └─────────┬───────────────────────────────────────────┘   │
│            │ AppState 改动 → persistence::save 立即触发     │
│  ┌─────────▼───────────────────────────────────────────┐   │
│  │           persistence (新模块)                        │   │
│  │   load_hosts() / save_hosts(&[HostConfig])           │   │
│  │   原子 tmp+rename / dirs::config_dir()               │   │
│  └─────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

### 关键消息类型

```rust
// AppState 内的 modal 状态
#[derive(Debug)]
pub enum HostFormState {
    Adding(HostFormDraft),
    Editing { id: HostId, draft: HostFormDraft },
}

// 表单中间状态（与 HostConfig 不同：未提交可能含无效字段）
#[derive(Debug, Default)]
pub struct HostFormDraft {
    pub label: String,
    pub host: String,
    pub port: String,      // String 让用户能临时输入非数字，提交时校验
    pub user: String,
    pub key_path: String,
    pub error: Option<String>, // 校验错误显示在 modal 底部
}

impl HostFormDraft {
    pub fn from_config(cfg: &HostConfig) -> Self { ... }
    pub fn into_config(&self, id: Option<HostId>) -> Result<HostConfig, String> { ... }
}
```

### Modal 数据流

```
用户点 "+" (HostListView)
  → AppState.update {
      state.modal = Some(HostFormState::Adding(HostFormDraft::default()));
      cx.notify();
    }
  → RootView 订阅 → 渲染时检测 modal 存在 → 在 host_list/terminal 之上 paint HostFormModal overlay

modal 内表单输入
  → HostFormModal::on_input → 更新 state.modal.draft.label 等字段

modal Save 按钮
  → HostFormModal::on_save:
      let cfg = draft.into_config(id_or_none)?;  // 校验
      AppState.update {
        state.add_or_replace_host(cfg);
        state.modal = None;
        cx.notify();
      }
  → AppState 改动 → cx.spawn 触发 persistence::save_hosts(&state.hosts)

modal Cancel 按钮 / Esc 键
  → AppState.modal = None
```

### 删除流程

```
用户 hover host row → ✏️/🗑 icons 显现（GPUI hover state）
点 🗑 → 弹小 confirm modal（同 HostFormModal 框架，不同 body 内容）
确认 → AppState.remove_host(id) → cx.notify → persistence::save_hosts
```

简化方案：删除不弹 modal，直接弹 GPUI 内置的简单提示？— **不**，统一用我们自己的 modal 框架，避免依赖 GPUI Toast/Notification API（可能未稳定）。

### 启动流程

```
app::run()
  → register_bundled_font(cx)  (M2b1 已有)
  → let hosts = persistence::load_hosts().unwrap_or_default();
  → state = cx.new(|_| AppState::with_hosts(hosts));
  → 后续 M2b1 不变
```

如果 hosts.json 不存在：load_hosts 返回 Ok(vec![])（空列表），不报错。如果存在但 parse 失败：返回 Err，启动时弹一个全局 toast 或 modal 提示，然后 fall back 到 vec![]（避免阻塞启动）。

---

## 5. File Structure（M2c 完成时）

```
aish/
├── Cargo.toml                        # 加 dirs workspace dep
├── crates/aish-app/
│   ├── Cargo.toml                    # 加 dirs
│   └── src/
│       ├── main.rs                   # 修改：删 mod fixtures，加 mod persistence + mod views::host_form
│       ├── app.rs                    # 修改：启动 load hosts.json
│       ├── state.rs                  # 修改：加 modal: Option<HostFormState> + add_host/update_host/remove_host
│       ├── persistence.rs            # 新：load_hosts / save_hosts / 原子写
│       ├── bridge.rs                 # 不变
│       ├── ssh_actor.rs              # 不变
│       ├── fixtures.rs               # ❌ 删除
│       ├── terminal/                 # 不变（M2b1 完整）
│       └── views/
│           ├── mod.rs                # 修改：reexport HostFormModal
│           ├── host_list.rs          # 修改：加 "+" 按钮 + hover icons + 调 modal 开启
│           ├── host_form.rs          # 新：HostFormModal modal view
│           └── terminal_view.rs      # 不变
```

新增 2 个文件：persistence.rs / views/host_form.rs
删除 1 个：fixtures.rs
修改 6 个：main.rs / app.rs / state.rs / views/mod.rs / views/host_list.rs + Cargo.toml
（terminal_view / ssh_actor / bridge / 整个 terminal/ 模块不动）

---

## 6. 核心数据流（端到端：用户首次启动 → 添加 host → 连）

```
用户 cargo run（首次，~/.config/aish/hosts.json 不存在）
  → app::run
  → persistence::load_hosts() → 文件不存在 → Ok(vec![])
  → AppState::with_hosts(vec![])
  → 窗口启动，左栏空 + "(无 host)" 提示 + 顶部 "+" 按钮

用户点 "+"
  → HostListView::handle_add_click
  → AppState.modal = Some(HostFormState::Adding(default_draft))
  → notify
  → RootView 重渲染 → HostFormModal overlay 出现，含空表单

用户填表单（如 label="my-vps", host="1.2.3.4", port="22", user="root", key="C:\Users\larry\.ssh\id_ed25519"）
  → 每次 keypress 调 HostFormModal::on_input → AppState.modal.draft 字段更新

用户点 Save
  → HostFormModal::on_save
  → draft.into_config(None) — 校验 port parsable / fields non-empty / key path 文件存在
  → 校验 OK → cfg = HostConfig { id: HostId::new(), ... }
  → AppState.update {
       state.hosts.push(cfg);
       state.modal = None;
       cx.notify();
     }
  → cx.spawn { persistence::save_hosts(&state.hosts).await };
  → 持久化完成（hosts.json 已含此 host）

用户点新 host → 走 M2a/M2b1 已有的 spawn_session → SshClient::connect → ...

用户重启应用
  → persistence::load_hosts() → 解析 hosts.json → vec![my-vps]
  → AppState 启动时左栏即有 my-vps
```

---

## 7. 错误处理矩阵

| 场景 | 处理 |
|---|---|
| **hosts.json 不存在** | load_hosts 返回 Ok(vec![])；正常启动 |
| **hosts.json 存在但 parse 失败** | load_hosts 返回 Err(serde_json::Error)；app 启动时通过 modal 显示"hosts.json 损坏：<错误>，已使用空列表"，用户点击关闭后正常进入；不覆盖原文件（避免数据丢失），重启再看 |
| **hosts.json 写入失败**（磁盘满 / 权限） | tracing::error! + 弹 modal 提示"保存失败：<错误>"；内存 state 已更新，下次重启会丢，但避免静默 |
| **add_host 校验失败**（port 非数字 / fields 空 / key 文件不存在） | HostFormDraft.error = Some(msg)，modal 底部显示红字；不关 modal |
| **删除唯一选中的 host** | sessions[host] drop（actor 自动退出）；selected = None；主区显示"请从左侧选择主机" |
| **编辑 host 修改了 host/user/key** | 对应 session 立即 disconnect（下次 click 会用新 config 连）；pane_terminals 保留 scrollback |
| **modal 期间用户点窗口外 / 关窗口** | modal **不**自动关（避免误丢数据）；必须显式 Cancel/Save/Esc |
| **磁盘配置目录不存在** | save_hosts 自动 mkdir -p ~/.config/aish/ |

---

## 8. 测试策略

| 模块 | 测试方式 |
|---|---|
| `persistence::load_hosts` | 单元测试：tempdir 准备 hosts.json → load → 比对内容 |
| `persistence::save_hosts` | 单元测试：tempdir → save → 读回 → 比对 |
| `persistence::save_hosts` 原子写 | 单元测试：save 后检查 .tmp 文件是否被清理（rename 已成功）|
| `state::AppState::add_host / update_host / remove_host` | 单元测试 mutation |
| `HostFormDraft::from_config / into_config` | 单元测试：roundtrip + port 解析失败 + 空字段失败 |
| GUI（host_list / host_form modal） | 手动视觉验证（与 spec 一致，UI 自动化测试 ROI 太低） |

CI 仍只 build / test / fmt / clippy。

---

## 9. M2c 完成验证（demo 标准）

执行 `cargo run -p aish-app`（**不**设环境变量）：

1. ✅ 窗口启动，左栏空 + "(无 host)" 提示 + 顶部 "+" 按钮（不再要求 AISH_DEV_HOST）
2. ✅ 点 "+" → modal 出现含表单（label/host/port/user/key path + Save/Cancel）
3. ✅ 输入完整信息（用你的真 VPS）+ Save → modal 关闭 + 左栏出现新 host (`○ <label>`)
4. ✅ 点新 host → 连接 + shell prompt（M2a/M2b1 路径都正常）
5. ✅ Esc → modal 关闭（无变化）
6. ✅ Cancel → modal 关闭（无变化）
7. ✅ 校验：port 输入 "abc" → Save → modal 底部红字 "port 必须是数字"
8. ✅ 校验：key path 不存在 → Save → 红字 "key 文件不存在"
9. ✅ Hover host row → 出现 ✏️ + 🗑 icons
10. ✅ 点 ✏️ → modal 出现含**预填**字段；改 label → Save → 左栏 row 文本更新
11. ✅ 点 🗑 → 确认 modal 出现"确认删除 my-vps?" → Yes → row 消失
12. ✅ 点 🗑 → Cancel → 不删
13. ✅ 关窗口 → 再启动 → 左栏自动恢复刚才的 host（hosts.json 持久化生效）
14. ✅ 手动改 hosts.json 加一个 garbage 字段使 parse 失败 → 启动 → modal 提示"hosts.json 损坏，使用空列表"
15. ✅ `cargo test --workspace` 全绿
16. ✅ `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` 全绿
17. ✅ CI 三平台 test job 全绿

---

## 10. M2c → M2b2 演进路径

M2c 完成后回到 M2b2（scrollback UI + DECCKM + F1-12 / IME）。M2c 留下的 onramp：

- AppState.hosts 已是真 host 来源；M2b2 不动 host 管理
- modal 框架可复用（M2b2 如要"键盘 cheatsheet" / "settings" 等弹窗）
- persistence 模块为 M2b2 / M3+ 配置（如 settings / theme）铺路

---

## 11. 已知风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| **GPUI modal overlay 实现复杂** | M2c 工期延长 | 参考 Zed crates/picker / crates/file_finder 看 modal 怎么做；如果太难，降级为"侧边滑出表单"（仍是 overlay 但不居中） |
| **hover icons 在小窗口或 host 名长时遮挡 label** | 视觉差 | 加最小 px 宽度 + label truncate；M2c 接受小瑕疵，未来可调 |
| **hosts.json 损坏后 modal 提示不清** | user 不知道怎么修 | 错误信息含具体行/列号（serde_json::Error 自带）+ 提示 "可手动编辑后重启" |
| **Windows path 含反斜杠 + JSON 转义** | hosts.json 里 `C:\\Users\\...` 显得乱 | 接受；如果反人类可后续改 PathBuf 序列化为 forward slash |
| **dirs crate 的路径在企业代理用户上奇怪**（如 OneDrive 重定向 AppData） | hosts.json 落到错位置 | 未知，遇到再处理 |
| **M2c 过程中误删用户已有 hosts.json** | 数据丢失 | implementer test 用 tempdir，不接触 user 真路径；user 在 M2c demo 前手动备份 hosts.json（如果有）|

---

## 12. 不在本 spec 范围内（边界提醒）

- Password / Agent 认证 → 后续按需
- SecretStore / aish-secrets → M5
- 原生文件选择器（OS file dialog） → follow-up
- 导入/导出 hosts.json / 多 profile → Phase 2
- Scrollback 滚动 UI / DECCKM / F1-12 / IME → M2b2
- 链接识别 / 图像内联 / Bell → 远期
- 主题 / 字号配置 → Phase 2
