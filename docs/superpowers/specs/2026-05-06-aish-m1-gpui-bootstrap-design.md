# aish M1 — GPUI 起步 + tokio 桥接设计

- **日期**: 2026-05-06
- **状态**: Design (approved by user, ready for implementation planning)
- **里程碑**: M1（属于 spec 主文档 `2026-05-06-aish-ssh-design.md` 的 Phase 1 第二个里程碑）
- **前置**: M0 已完成（commit `9f19f68` 之后）
- **作者**: larry
- **预计周期**: W2-3（2 周，可能拖到 4 周）

---

## 1. 范围与目标

M1 是 aish 项目最难的里程碑——攻克 GPUI 学习曲线 + 验证 GPUI/tokio executor 桥接模式。**完全不接入真实 SSH**，全部数据用 mock。完成后才有信心进 M2 接 russh。

### Phase 1（M1，本 spec 范围）

| 模块 | 范围 |
|---|---|
| GPUI 应用基础 | 单窗口、左栏 + 主区两栏布局、可 resize |
| App State | 单一 root `Model<AppState>`，含 mock host 列表、当前选中、per-host pane log |
| Views | HostListView（左栏 List）+ HostPaneView（主区 log 流） |
| tokio 桥接 | 独立 worker thread 跑 multi-thread runtime；mpsc channel 把 mock 事件推回 GPUI |
| Mock SSH | mock_ssh_task：3 秒后通过 channel 推一行假 PaneOutput |
| 学习参考 | Zed 仓库 clone 到 `C:\Users\larry\Desktop\workspace\zed-reference`，作 GPUI 学习金矿 |

### 不在范围内（明确边界）

- 真实 SSH 连接（M2）
- 真实终端渲染（alacritty_terminal 集成，M2）
- tmux control mode 协议（M3）
- 附件传输（M4）
- env 注入（M5）
- 多 tab、多窗口、菜单栏、设置面板（Phase 2+）
- hosts.json 持久化（M2 才引入）
- AI CLI 输出富文本渲染（Phase 3）

---

## 2. 关键技术决策

| ID | 决策 | 理由 |
|---|---|---|
| M1-ADR-1 | **Demo 范围 = B（中等）** | A（纯 hello）学不到 GPUI 核心抽象；C（接 hosts.json）引入持久化让焦点偏离学习目标。B 覆盖 GPUI List + Model + cx.spawn + tokio 桥接，无真实 IO 副作用 |
| M1-ADR-2 | **Zed 完整 clone 到本地** | GPUI 文档稀缺，看 Zed 源码（特别是 `crates/gpui/examples/`）是唯一系统学习方式；IDE 跳定义、grep API、看 trait 实现都需要本地代码 |
| M1-ADR-3 | **GPUI 依赖：pin 到 Zed main 的具体 commit** | Zed 没专门的 GPUI release tag，跟 main HEAD 太脆弱（每次 cargo update 可能破）。pin 一个 commit 给手动升级窗口 |
| M1-ADR-4 | **App State：单 root Model<AppState>** | M1 数据结构简单，单 Model 最直观；M2 接真实 SSH 时再按需拆 Model（不预先过度设计） |
| M1-ADR-5 | **tokio runtime：独立 worker thread + multi_thread builder** | 与主 spec ADR-5 一致；GPUI executor 与 tokio runtime 不共享线程，避免 Send/lifetime 灾难 |
| M1-ADR-6 | **桥接走 tokio::sync::mpsc** | tokio 端 send，GPUI 端 cx.spawn 跑 async block 接收；channel buffered（容量 64）防止 OOM |
| M1-ADR-7 | **per-host pane state 隔离用 HashMap<HostId, Vec<String>>** | 切换 host 时之前的 mock 输出保留；既验证状态隔离，又为 M2 终端 buffer 隔离铺路 |
| M1-ADR-8 | **不预先抽象 HostProvider / SshClient trait** | YAGNI；M2 接真实 SSH 时整体 Model 大概率重构，提前抽的 trait 大概率是错的 |

---

## 3. 系统架构

```
┌─────────────────────────────────────────────────────┐
│            GPUI Application Process                  │
│                                                      │
│  ┌─────────────────────────────────────────────┐   │
│  │                Views (GPUI)                  │   │
│  │  ┌──────────────┐ ┌─────────────────────┐   │   │
│  │  │ HostListView │ │ HostPaneView         │   │   │
│  │  │ (左栏 220px) │ │ (主区 fill)          │   │   │
│  │  └──────┬───────┘ └─────────┬───────────┘   │   │
│  └─────────┼─────────────────────┼─────────────┘   │
│            │ subscribe            │ subscribe        │
│  ┌─────────▼─────────────────────▼─────────────┐   │
│  │      AppState (single Model<T>)              │   │
│  │  hosts: Vec<MockHost>                        │   │
│  │  selected: Option<HostId>                    │   │
│  │  pane_logs: HashMap<HostId, Vec<String>>     │   │
│  └─────────────────┬───────────────────────────┘   │
│                    │ cx.spawn 提交 task             │
│                    ▼                                 │
│  ┌─────────────────────────────────────────────┐   │
│  │   Bridge (tokio runtime on worker thread)    │   │
│  │   - rt::Builder::new_multi_thread().build()  │   │
│  │   - mock_ssh_task: sleep 3s + emit event     │   │
│  └─────────────────┬───────────────────────────┘   │
│                    │ mpsc::Sender<MockEvent>        │
│                    ▼                                 │
│  ┌─────────────────────────────────────────────┐   │
│  │  GPUI cx.spawn 接收 mpsc::Receiver           │   │
│  │  → cx.update_model 改 AppState               │   │
│  │  → 订阅者自动重渲染                           │   │
│  └─────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### 分层职责

- **Views**：纯 GPUI 渲染逻辑，订阅 Model 变化重绘；不持有业务状态
- **AppState (Model)**：所有 UI 共享状态的 single source of truth；用 GPUI 的 `Model<T>` 包装
- **Bridge**：tokio runtime 启动器 + channel 收发 helper；UI 与 tokio 之间的唯一接口
- **Mock**：模拟 SSH 行为的 async function，纯计算/sleep，无真实网络

---

## 4. File Structure

M1 在 `aish-app` crate 内新增：

```
crates/aish-app/
├── Cargo.toml                  # 修改：加 gpui git dep + 调整 tokio features
└── src/
    ├── main.rs                 # 修改：启 tokio runtime + 调 app::run()
    ├── app.rs                  # 新：App 入口（创建 GPUI app + 主窗口 + 启 bridge）
    ├── state.rs                # 新：AppState Model + MockHost / MockEvent / HostId(u32)
    ├── bridge.rs               # 新：tokio runtime 启动 + mpsc channel 桥接 helper
    ├── mock.rs                 # 新：mock_ssh_task（sleep 3s + emit PaneOutput）
    └── views/
        ├── mod.rs              # 新：reexport HostListView / HostPaneView
        ├── host_list.rs        # 新：左栏 List 渲染（点击切换 selected）
        └── host_pane.rs        # 新：主区 pane log 渲染（订阅 selected + pane_logs）
```

8 个文件，每个职责单一，预期最大不超过 150 行。

**注：** M1 的 `HostId` 暂用 `u32`（mock 主机 1/2/3），不复用 `aish-types::HostId`（UUID）。M2 引入真实 `HostConfig` 时再切换为 `aish-types::HostId`。这是 ADR-8 "不预先抽象" 的具体体现。

---

## 5. 核心数据流（用户点击 server-A 时序）

```
User Click on "server-A" in HostListView
       │
       ▼
HostListView 的 click handler 调：
  1. cx.update_model(&app_state, |state, cx| {
       state.selected = Some(host_a_id);
       state.pane_logs
           .entry(host_a_id)
           .or_default()
           .push(format!("[{}] Connecting to server-A...", now()));
       cx.notify();
     })
       │  Model 立即变更 → HostPaneView 订阅触发重渲染
       │  用户看到 "Connecting..."
       │
  2. bridge.spawn_mock_ssh(host_a_id)
       │  通过 tokio runtime 提交 task
       ▼
async fn mock_ssh_task(host_id, tx) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    tx.send(MockEvent::PaneOutput {
        host: host_id,
        line: "Welcome to server-A! (mocked SSH output)".to_string(),
    }).await.ok();
}
       │  3 秒后 send 到 mpsc channel
       ▼
GPUI 全局事件循环（在 app::run 启动时由一次性 cx.spawn 创建，常驻）：
  cx.spawn(|mut cx| async move {
      while let Some(event) = rx.recv().await {
          cx.update_model(&app_state, |state, cx| {
              match event {
                  MockEvent::PaneOutput { host, line } => {
                      state.pane_logs.entry(host).or_default().push(line);
                      cx.notify();
                  }
              }
          })?;
      }
      Ok(())
  }).detach();
       │
       ▼
HostPaneView 订阅 AppState 变更 → 重渲染 → 用户看到 Welcome 行追加
```

---

## 6. M1 → M2 演进路径

M2 接入真实 SSH 时的最小改动（**不预先抽象**）：

| M1 | → M2 |
|---|---|
| `state::HostId(u32)` + `MockHost` | 换为 `aish_types::HostId(Uuid)` + `aish_types::HostConfig` |
| `bridge::spawn_mock_ssh()` | 改为 `bridge::spawn_ssh_connect()`，内部走 `aish_ssh::SshClient::connect` |
| `MockEvent::PaneOutput` | 复用 enum 名，添加更多 variant；接 `aish_tmux::TmuxEvent::PaneOutput` 转换 |
| `HashMap<HostId, Vec<String>>` | 改为 `HashMap<HostId, alacritty_terminal::Term>`（log 改为终端 grid） |
| `views/host_pane.rs` 渲染纯文本行 | 改为渲染终端 grid（订阅 Term 状态） |

预计 M2 开始时这 5 处会有改动；变更范围明确、可控。

---

## 7. 测试策略

| 文件 | 测试方式 |
|---|---|
| `bridge.rs` | 单元测试：起 tokio runtime → 通过 channel send/recv 一些 event → 验证 receiver 收到的内容与顺序 |
| `mock.rs` | 单元测试：spawn mock_ssh_task → tokio::time::pause + advance 3s → 验证 channel 收到正确 PaneOutput |
| `state.rs` | 单元测试 AppState 的纯状态 mutation 函数（如 `select_host`, `append_log`），不需要 GPUI runtime |
| `app.rs` / `views/` | 手动视觉验证（GPUI UI 自动化测试 ROI 太低，与主 spec 一致） |

测试目标：M1 完成时 `cargo test --workspace` 仍全绿，且至少有 4-6 个新增的非 GPUI 单元测试覆盖 bridge / mock / state。

---

## 8. 错误处理

M1 阶段错误源很少，主要：

| 场景 | 处理 |
|---|---|
| GPUI 创建窗口失败 | log error + 进程退出（非常少见，通常是 GPU 驱动问题） |
| tokio runtime 启动失败 | 同上 |
| mpsc channel 满 | M1 channel buffer 容量 64，mock 事件量极小不会满；但代码上仍 `.await` send（背压） |
| GPUI render panic | 主 spec 已定：panic hook + crash log 到 `~/.aish/crashes/` + 进程退出 |
| 关窗口时 tokio task 泄漏 | App drop 时显式 shutdown runtime；mock task 都是有限时长（3s sleep + 一次 send），最多让 worker thread 多活 3s |

---

## 9. M1 完成验证（demo 标准）

执行 `cargo run -p aish-app`，按以下步骤验证：

1. ✅ 窗口弹出（默认 1200×800，可 resize）
2. ✅ 左栏显示三行：`server-A (mock)` / `server-B (mock)` / `server-C (mock)`
3. ✅ 主区初始为空 + 提示文字"请从左侧选择主机"
4. ✅ 点击 `server-A` → 主区立刻显示 `[<时间>] Connecting to server-A...`
5. ✅ 等 3 秒 → 主区追加 `Welcome to server-A! (mocked SSH output)`
6. ✅ 切到 `server-B` → 主区切换；快速点回 A → server-A 之前的两行**仍在**
7. ✅ 立刻点 A 然后立刻点 B（间隔 < 1s）→ 3 秒后两个 host 各自的 Welcome 行**都到位**（验证 tokio multi-thread）
8. ✅ 关窗口 → 进程在 1 秒内干净退出（`echo $?` 在 bash 是 0，无 zombie 进程）
9. ✅ `cargo test --workspace` 仍全绿
10. ✅ `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` 全绿
11. ✅ CI 三平台 test job 全绿（push 到 GitHub 后由 Actions 验证）

---

## 10. 已知风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| **GPUI 学习曲线陡** | 前 3-5 天产出感很弱，可能拖期 | 心理预期已提；Zed 完整 clone 是兜底参考 |
| **GPUI commit pin 选错** | Zed 该 commit 后破坏 API 或不稳定 | Plan 阶段查"最近 30 天内 GPUI crate 改动小的 commit"；遇问题及时退到上一个 commit |
| **tokio/GPUI 桥接的 cx.spawn 与 mpsc::Receiver 协作模式** | 错的写法可能编译过但运行 deadlock 或 task 泄漏 | 找 Zed 类似模式的 example（如 zed-industries/zed 的 worktree fs watcher）作参考 |
| **Windows 上 GPUI 稳定性** | 撞 GPUI Windows 特定 bug 可能阻塞 | 主开发用 Windows 试；如果撞硬墙考虑临时主开发切 macOS（可走 Mac mini / VM） |
| **2 周时间窗口偏紧** | 实际可能 3-4 周 | 接受；M1 是项目最艰难的里程碑，宁可拖期也要把基础打牢 |

---

## 11. 后续里程碑预告

M1 完成后开始 M2（SSH 连接 + 单 PTY 终端）：
- 接入 `aish_ssh::SshClient::connect`
- 集成 `alacritty_terminal::Term` 替换 mock pane log
- 引入 `aish-types::HostConfig` 替换 `MockHost`
- 引入 `~/.aish/hosts.json` 持久化主机列表

M2 不在本 spec 范围。M1 完成后会单独走 brainstorm → spec → plan → implement 循环。