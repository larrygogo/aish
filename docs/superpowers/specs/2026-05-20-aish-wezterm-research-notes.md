---
title: WezTerm 架构调研 — daemon 化对标参考
date: 2026-05-20
status: research
related: 2026-05-20-aish-daemonize-design.md
source: https://github.com/wez/wezterm (shallow clone @ workspace/wezterm-reference)
---

# WezTerm 架构调研 — daemon 化对标参考

> 给 [aish daemon 化 brainstorm spec](./2026-05-20-aish-daemonize-design.md) 的
> 七个 ADR 提供来自 Rust 同类项目的直接对标证据。WezTerm 在 `wezterm-mux-server`
> 上做的事，几乎就是 spec §6 的 Phase 2 + Phase 4。

---

## 1. 项目尺度速览

| 维度 | 数据 |
|---|---|
| 总 crate 数 | 50+（terminal + GUI + mux + ssh + lua + 等等） |
| 关键 crate | `wezterm-mux-server` / `wezterm-mux-server-impl` / `mux` / `wezterm-client` / `codec` / `wezterm-uds` |
| 协议层 | 自定义二进制 codec（leb128 帧头 + bincode 数据） |
| 当前 CODEC_VERSION | 45 |
| PDU variants 数 | 53 |

WezTerm 比 paseo（Node.js，~30k 行）大一个量级，且核心 mux 部分是 Rust，**对 aish 的参考价值比 paseo 直接得多**。

---

## 2. 与 spec 七个 ADR 的对照

### ADR-001 进程模型 — ✅ 与 spec 倾向一致（A: 独立 binary）

`wezterm-mux-server` 是单独的 Rust binary（不是 GUI 进程的 thread）。

- 入口：`wezterm-mux-server/src/main.rs` 用 clap 解析参数
- `--daemonize` 让它 fork 进后台（Unix `daemon(3)` 风格）
- GUI 端通过 `UnixDomain::serve_command()` 自动 spawn daemon：
  ```rust
  // config/src/unix.rs:117-130
  pub fn serve_command(&self) -> anyhow::Result<Vec<OsString>> {
      match self.serve_command.as_ref() {
          Some(cmd) => Ok(cmd.iter().map(Into::into).collect()),
          None => Ok(vec![
              std::env::current_exe()?
                  .with_file_name(if cfg!(windows) {
                      "wezterm-mux-server.exe"
                  } else {
                      "wezterm-mux-server"
                  })
                  .into_os_string(),
              OsString::from("--daemonize"),
          ]),
      }
  }
  ```

**对 aish 的直接启示**：`aishd` 应该跟 `aish` (GUI binary) 同目录、同 release 包，GUI 用 `current_exe().with_file_name("aishd")` 找到它。这比 PATH lookup 稳。

### ADR-002 通信协议 — ✅ 与 spec 倾向一致（A: UDS / 命名管道）

`wezterm-uds` crate 把 Unix Domain Socket 包装成跨平台抽象：

- Unix：`std::os::unix::net::UnixStream/UnixListener`
- Windows：`uds_windows` crate（第三方提供 UDS-like API on Windows，**不是 named pipe**）

```rust
// wezterm-uds/src/lib.rs
#[cfg(unix)]
use std::os::unix::net::UnixStream as StreamImpl;
#[cfg(windows)]
use uds_windows::UnixStream as StreamImpl;
```

**关键决策点**：Windows 上用 `uds_windows` 而**非** Named Pipe（`\\.\pipe\xxx`）。原因可能是 protocol 层不想分裂 —— UDS 抽象同一套字节流即可。

但 `uds_windows` 是非官方 crate（依赖 AFUNIX socket，Windows 10 1803+ 才稳定）。**aish 选择时要权衡**：
- `uds_windows`：代码统一，但依赖较新 Windows
- Named Pipe（`tokio::net::windows::named_pipe`）：原生支持，但要写两套代码

paseo 用 named pipe 路径（`\\.\pipe\paseo-...`），WezTerm 用 uds_windows 路径。**对 aish 我倾向 named pipe**：tokio 原生支持，无 uds_windows 第三方依赖。

**socket path 权限校验**（必抄）：

```rust
// wezterm-mux-server-impl/src/local.rs:42-65
// 检查父目录权限不能允许 other-user 写
let permissions = meta.permissions();
if (permissions.mode() & 0o22) != 0 {
    anyhow::bail!("permissions for {} are insecure", sock_dir.display());
}
```

UDS 在 `/tmp` 等公共目录的话，权限是安全底线。aish 必须照做。

**默认 socket 路径**：`RUNTIME_DIR/sock`
- Linux: `$XDG_RUNTIME_DIR/wezterm/sock`（即 `/run/user/$UID/wezterm/sock`）
- macOS: tmpdir 子目录
- Windows: 用 named shared memory 注册 path（**复杂**，见下文）

### ADR-003 序列化格式 — ⚠️ WezTerm 与 spec 推荐方案不同

spec 推荐 **C（JSON 控制 + 二进制数据帧）**，paseo 也是这个路径。

**WezTerm 选了纯二进制全栈**：

```
帧格式 (codec/src/lib.rs):
  [tagged_len: leb128]   # 帧总长（高位 bit = is_compressed）
  [serial: leb128]       # 请求-响应 correlation id
  [ident: leb128]        # PDU variant 标识符
  [data bytes]           # bincode 序列化的具体 variant payload
```

- **leb128** 变长整数：小 ident 用 1 字节，大的逐步增长
- **per-variant `ident` 数字**（`ListPanes: 3, SpawnV2: 36, ...`）= **不依赖 serde 的 tag 字段**
- **CODEC_VERSION: usize = 45**（全局版本号）+ 老 ident 永不复用 = **append-only enum**

```rust
// codec/src/lib.rs:441-505 节选
pub const CODEC_VERSION: usize = 45;

pdu! {
    ErrorResponse: 0,
    Ping: 1,
    Pong: 2,
    ListPanes: 3,
    ListPanesResponse: 4,
    SpawnResponse: 8,        // 注意 5,6,7 跳号 — 老 variant 移除后 ident 不复用
    WriteToPane: 9,
    // ... (53 个 variants)
    AdjustPaneSize: 62,
}
```

**这是与 paseo capability flag 等价的演进策略**，两者解决同一问题（client/server 版本错配），方法不同：

| 方案 | paseo | WezTerm |
|---|---|---|
| 携带的元数据 | client `capabilities: Vec<String>` | global `CODEC_VERSION: usize` |
| schema 演进 | append-only field + 能力 flag 门控 | append-only enum + ident 永不复用 |
| 适合 | JS 弱类型 union + Zod 验证 | Rust sealed enum + serde + bincode |
| 客户端老于服务端 | 服务端 fallback 老 shape | 服务端拒绝 (`Invalid{ident}` 占位) |

**对 aish 的启示**：Rust 项目应该走 **WezTerm 风格**：
- 用 enum + 显式 ident 数字（`pdu!` 风格宏）
- bincode 或 postcard 做 payload 序列化
- 不用 JSON（除非要给浏览器 client 直接用）

**重要修订**：spec ADR-003 的 C 方案（JSON 控制 + 二进制数据）应该被推翻成 **D 方案：全二进制 + ident-based enum**，理由：
- aish 不需要 JS / 浏览器 client
- bincode 比 JSON 小 30-50%、解析快 5-10 倍
- 终端流和控制消息都是结构化数据，没必要分两套

### ADR-004 alacritty grid 归属 — ⚠️ WezTerm 与 spec 倾向相反

spec 推荐 **B（grid 在 client）**，理由是 aish 单 client 不需要 multi-client 同步。

**WezTerm 选了 A（grid 在 daemon）+ 主动推送 + 增量同步**：

```rust
// codec PDU 片段
GetPaneRenderChanges: 24,
GetPaneRenderChangesResponse: 25,
GetLines: 22,
GetLinesResponse: 23,
```

- terminal 状态机（wezterm-term）在 mux daemon 里
- client 拉 `GetLines` 拿当前 viewport，订阅 `GetPaneRenderChanges` 增量
- 多个 client 看同一 pane 看到一致内容

**为什么 WezTerm 选 A 而 aish 该选 B**：

| 场景 | WezTerm | aish |
|---|---|---|
| GUI 同时多开看同一 pane | ✅ 必须 | ❌ 当前无场景 |
| SSH 远程 attach 到本地 mux | ✅ 主线功能 | ❌ SSH 已经解决 |
| client 是 web 浏览器 | ⚠️ 未来 | ❌ 不在规划 |
| client crash 后 grid 仍活 | ✅ | ⚠️ 仅 scrollback 价值 |

**结论**：spec ADR-004 的 B 方案对 aish 仍然合理。WezTerm A 方案的复杂度（grid 跨进程序列化、SequenceNo 同步）aish 不需要。

但有个**折中方案值得考虑**：daemon 维护 raw byte ring（scrollback），client crash 后重连 replay。这就是 spec ADR-004 的 B 方案 + scrollback 持久化部分。

### ADR-005 持久化层 — WezTerm 用 lua 配置 + state file

WezTerm 持久化分两块：

- **配置**：lua 文件（`~/.config/wezterm/wezterm.lua`） — aish 现在 JSON/TOML，更简单，不抄
- **运行时状态**：mux 内部内存 + workspace state file（如 `~/.local/share/wezterm/state/workspace.json`）

aish 当前持久化已经够好（`dirs::config_dir()/aish/hosts.json` + atomic write），**无需调整**。

但 WezTerm 有个值得借鉴的点：**socket 路径用 `RUNTIME_DIR` 而非 `config_dir`**
- Linux: `$XDG_RUNTIME_DIR`（重启后自动清空，systemd 管）
- macOS: 临时目录
- Windows: 见下文

**aish 抄**：daemon 的 PID lock + socket path 应该用 runtime dir（`dirs::runtime_dir()`），而不是配置目录。stale PID file 在重启后自动消失，少一类故障。

### ADR-006 capability negotiation — WezTerm 走全局版本号

见 ADR-003 分析：WezTerm 的 `CODEC_VERSION` 替代了 paseo 的 capability flag。

但 WezTerm **也有 client capability 概念**：

```rust
// codec PDU 片段
SetClientId: 40,            // client 上报身份信息
GetClientList: 41,          // server 列出所有连接的 client
GetClientListResponse: 42,
SetWindowWorkspace: 43,
```

`SetClientId` 携带 `ClientInfo`：

```rust
// mux/src/client.rs (推断，未直接验证)
pub struct ClientInfo {
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub pid: Option<u32>,
    // ...
}
```

这是 client 上报身份信息让 server 做审计 / 显示「来自哪台机器」。**aish 不需要**这层（单机单 client）。

### ADR-007 PTY 隔离 — ✅ 与 spec 倾向一致（A: 同进程）

WezTerm 用 `portable_pty` crate 直接 spawn child process，**不开 worker process**。

```rust
// mux/src/localpane.rs（简化）
let child = portable_pty::native_pty_system().openpty(...)?.spawn_command(cmd)?;
// child 跟 mux server 同进程
```

理由跟 spec 一致：Rust + 稳定的 PTY crate 不需要 paseo 那种 worker process 隔离。

---

## 3. 值得直接抄的额外设计

### 3.1 Domain trait 抽象（强烈推荐）

```rust
// mux/src/domain.rs:49-72
#[async_trait(?Send)]
pub trait Domain: Downcast + Send + Sync {
    async fn spawn(&self, size, command, command_dir, window) -> Result<Arc<Tab>>;
    async fn spawn_pane(&self, size, command, command_dir) -> Result<Arc<dyn Pane>>;
    async fn split_pane(&self, source, tab, pane_id, request) -> Result<Arc<dyn Pane>>;
    fn domain_id(&self) -> DomainId;
    fn domain_name(&self) -> &str;
    fn state(&self) -> DomainState;
    // ...
}
```

实现者：`LocalDomain`（本地 PTY）、`TlsDomain`（远程 mux over TLS）、`SshDomain`（SSH 到远端跑 wezterm-mux-server）。

**对 aish 启示**：可以抽 `Connection` trait，目前只有一个实现 `SshConnection`（russh-based），未来扩展 `LocalConnection`（本地 shell）/ `WslConnection`（WSL 直连）极方便。

但**当前不建议提前抽**：aish 只有 SSH 一种 backend，提前抽是过度设计。等真有第二种 backend 触发再抽。

### 3.2 PDU 的 `is_user_input()` 标志（背压策略用）

```rust
// codec/src/lib.rs:507-523
impl Pdu {
    pub fn is_user_input(&self) -> bool {
        match self {
            Self::WriteToPane(_)
            | Self::SendKeyDown(_)
            | Self::SendMouseEvent(_)
            | Self::SendPaste(_)
            | Self::Resize(_)
            | Self::SetClipboard(_)
            | Self::SetPaneZoomed(_)
            | Self::SpawnV2(_) => true,
            _ => false,
        }
    }
}
```

用户输入（键盘、鼠标）优先级 > 后台 render delta。**aish 抄**：未来 wire protocol 加同样标志，client → daemon 队列拥塞时优先送用户输入。

### 3.3 `serial` 字段做请求-响应 correlation

每条 PDU 带 leb128 的 `serial`，client 发请求时分配 serial，server 响应时 echo 回来。

paseo 用 `request_id: Uuid`，WezTerm 用 `serial: u64`（leb128 编码）。**leb128 + u64 比 UUID 省 15 字节左右**，对终端流量频繁的场景有意义。

aish 抄 leb128 + u64。

### 3.4 帧头压缩位

```
masked_len 的最高 bit = is_compressed
```

支持透明压缩（zstd 或 zlib），但不强制。**对 aish**：第一版不做（终端流量本就小），留协议位即可。

---

## 4. 对 spec 七个 ADR 的最终建议

| ADR | spec 倾向 | WezTerm 印证 | 调研后建议 |
|---|---|---|---|
| 001 进程模型 | A（独立 binary） | ✅ A | **A，无变化** |
| 002 通信协议 | A → D（UDS v1，TCP 可加） | ✅ A（UDS），但 Windows 用 uds_windows | **A，但 Windows 走 named pipe**（不引入 uds_windows 依赖） |
| 003 序列化 | C（JSON + binary） | ❌ 全二进制 + ident enum | **改 D：全二进制（bincode/postcard）+ ident-based enum**，抄 WezTerm 的 `pdu!` 宏 |
| 004 grid 归属 | B（client 内） | ❌ A（daemon 内） | **B 不变**（aish 没有 multi-client 场景，但加 daemon 侧 scrollback ring） |
| 005 持久化 | A（文件型 JSON） | 配置 lua + state file | **A 不变**，但 socket/PID 移到 `dirs::runtime_dir()` 而非 config_dir |
| 006 capability | paseo flag 模式 | 全局 CODEC_VERSION + ident append-only | **改用 WezTerm 风格**（Rust 项目更自然） |
| 007 PTY 隔离 | A（同进程） | ✅ A | **A，无变化** |

---

## 5. 实施建议增量

针对 spec §6 Phase 1（wire schema），调研后的具体建议：

```rust
// aish-protocol crate 草图
use serde::{Serialize, Deserialize};

// 每个 variant 一个稳定 ident（u64）
// 老 ident 永不复用，新 variant 分配下一个未用 ident
#[derive(Debug, Serialize, Deserialize)]
pub enum Pdu {
    Hello(Hello) = 1,
    HelloAck(HelloAck) = 2,
    Ping = 3,
    Pong = 4,
    ListHosts(ListHosts) = 10,
    ListHostsResponse(ListHostsResponse) = 11,
    OpenConnection(OpenConnection) = 12,
    // ...
}

pub const CODEC_VERSION: u32 = 1;

// 帧格式
// [tagged_len: leb128 (high bit = compressed)]
// [serial: leb128]
// [ident: leb128]
// [bincode-encoded payload]
```

**比 spec 草案的修订**：
- 抛弃「JSON 控制 + 二进制数据帧」分层 → 全二进制
- 抛弃 capability flag 字符串数组 → 全局 CODEC_VERSION + ident 永不复用
- 保留 `serial` 请求-响应关联
- 终端 I/O 跟控制消息走同一个 PDU enum（用 `is_user_input()` 区分背压）

---

## 6. 不应该抄的部分

| WezTerm 设计 | 不抄的理由 |
|---|---|
| Lua 配置 | aish JSON/TOML 足够，引入 lua 是负债 |
| `uds_windows` crate | tokio 原生 named pipe 更轻 |
| Multi-domain / SSH-mux 嵌套 | aish 只有一种 backend（直接 ssh），不需要 |
| Grid 在 daemon 跨进程同步 | aish 单 client 不需要 |
| TLS 远程 mux | SSH 已经解决远程，aish 不重复造 |
| `wezterm-blob-leases` | aish 没有 blob 存储需求 |
| Shared memory client discovery on Windows | named pipe + 固定路径更简单 |

---

## 7. 调研结论

1. **WezTerm 验证了 spec §6 Phase 2 的可行性**：Rust + UDS + bincode 这条路在生产项目跑了多年，PDU enum 演进到 53 variants 仍稳定。

2. **三个 ADR 修订**：
   - ADR-003（序列化）：从「JSON + 二进制」改为「全二进制 + ident enum」
   - ADR-006（capability 协商）：从 paseo string-array 风格改为 WezTerm 全局版本号 + ident append-only
   - ADR-002（通信）：Windows 倾向 named pipe 而非 uds_windows

3. **Domain trait** 是个值得留意的抽象，但不立即抄（aish 只有一种 backend）。

4. **socket 路径用 `runtime_dir`** 而非 `config_dir`，stale PID 自动清理。

5. **`is_user_input()` 标志 + leb128 `serial`** 这俩小细节是 WezTerm 的工程亮点，可直接抄到未来的 wire protocol。

6. **WezTerm 跟 paseo 的对比** 是这次调研最有价值的部分：两者解决相同问题（client/server 版本协商）选了相反路径，aish 作为 Rust 项目应该走 WezTerm 路径。

---

## 8. Next Step

如果 Phase 1（wire schema）启动，按本次调研结论：

1. 创建 `aish-protocol` crate，参考 WezTerm `codec` crate 实现 `pdu!` 宏
2. 在 spec §3 ADR-003 / ADR-006 上加调研结论 → 更新决策
3. 把 spec §4 wire protocol 草案改写成 ident-based enum 形式

如果 Phase 1 不启动，本笔记作为：
- 未来 aish 演进的设计参考
- daemon 化决策的「我们调研过 Rust 同类项目」证据
