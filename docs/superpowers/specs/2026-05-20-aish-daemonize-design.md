---
title: aish daemon 化探索（brainstorm）
date: 2026-05-20
status: brainstorm
related-milestones: TBD（候选 M38+）
inspired-by: paseo (https://github.com/getpaseo/paseo)
---

# aish daemon 化探索

## 0. 目标

把 aish 从「GPUI 单进程，所有状态都在 UI 进程内」演进为
「**aishd 守护进程**持有 SSH/tmux/PTY 状态，**GUI/CLI/未来的手机端**作为
client 连接到 daemon」的形态。

本文不做最终决策，只是把所有备选路径、风险、价值摊开，便于后续判断
**是否做 / 何时做 / 做到什么粒度**。

**非目标**：

- 不在本 spec 里给出实施计划（属于后续 plan 阶段）
- 不重写 GPUI UI 代码 —— UI 层 API 保持不变，只换数据源
- 不引入 cloud / 多用户 / 协作（aish 永远 local-first）
- 不替代 SSH（daemon 不做 NAT 穿透 / relay，因为 SSH 已经做到了）

---

## 1. 上下文

### 1.1 当前架构（M37 之后）

```
┌─────────────────────────────────────────────────────────┐
│  aish-app (GPUI 主进程)                                  │
│                                                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │ UI 层 (RootView / TabBar / Terminal / Modal)    │    │
│  └──────────────────────┬──────────────────────────┘    │
│                         │ GPUI cx                       │
│  ┌──────────────────────▼──────────────────────────┐    │
│  │ App 状态 (TabId / ConnectionId / HostId 三键)   │    │
│  └──────────────────────┬──────────────────────────┘    │
│                         │ tokio mpsc                    │
│  ┌──────────────────────▼──────────────────────────┐    │
│  │ Connection actor (1 个 / 连接)                  │    │
│  │  - russh PTY channel                            │    │
│  │  - alacritty Term + Grid                        │    │
│  │  - tmux attach 状态机                            │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
                          │
                          │ russh SSH
                          ▼
                  远端 sshd / tmux
```

所有运行时状态（SSH 通道、alacritty grid、tmux session 关联）都在 GPUI 进程内。
GPUI crash / 用户 cmd+Q ⇒ **所有连接断开、所有 scrollback 丢失**。

### 1.2 触发本次思考的契机

| 触发 | 说明 |
|---|---|
| paseo 项目分析 | `getpaseo/paseo` 是 daemon + 多 client 架构的成熟样本，验证了这条路径可行 |
| M37 跨平台 polish 收尾 | UI 层基本稳定，到了考虑「下一波结构性升级」的时间点 |
| CLI 需求空缺 | aish 完全没有 CLI 入口，`aish ls / aish open host` 这种诉求只能 daemon 化才优雅 |
| MCP/AI agent 集成 | 越来越多 local agent 希望能驱动用户的开发环境，daemon + MCP 是天然契合 |

### 1.3 痛点真实程度（自我反问）

**这是 daemon 化决策最重要的一节** —— 不解决真实痛点的架构升级是负债。

| 痛点 | 严重程度 | 现有 mitigation | daemon 化能否解决 |
|---|---|---|---|
| GUI crash 丢连接 | 中。GPUI 实测稳定，crash 罕见 | tmux attach（M3 之后）已经能恢复远端 session，本地 scrollback 仍丢 | ✅ 完全解决 |
| GUI 重启慢 | 低。冷启 < 1s | 无 | ✅ 不需要重新 SSH 握手 |
| 没有 CLI | 中。少数 power user 想要 `aish ssh prod`/`aish ls` | 无 | ✅ daemon 化是前提 |
| AI agent 无法驱动 aish | 中-高。趋势上越来越重要 | 无 | ✅ daemon + MCP server |
| 手机端 / 远程查看 | 低。SSH 已经够用 | 用户自己 ssh 到机器跑 tmux | ⚠️ 收益不明，且 paseo 这条路 aish 不需要 |
| 多窗口 / 多个 aish-app 共享 connection | 低。当前 use case 不明 | 无 | ✅ 顺带解决 |

**关键判断**：daemon 化的**充分理由**是「CLI + AI agent 驱动」，**不是**「GUI 不稳定」。
如果 CLI / MCP 不是真需求，单纯为了 crash 恢复做 daemon **得不偿失**。

---

## 2. 参考

- **paseo** ([getpaseo/paseo](https://github.com/getpaseo/paseo)) —
  Node.js + TypeScript，~30k 行；daemon + WebSocket + 多 client。本 spec 的
  主要灵感来源。详细分析见会话上下文（不复制到本文）
- **Zed Server** —— Zed 的 collab server 是 Rust 服务进程的工程范本（不过用途
  完全不同，仅参考结构）
- **WezTerm** —— 也是 daemon + GUI 分离（`wezterm-mux-server`），同为 Rust 终端
  项目，是更直接的对标。值得花一天通读其架构
- **mosh** / **eternal-terminal** —— 远端 session 持久化的另一种思路（serverside
  状态），跟 aish daemon 化方向相反但有借鉴价值

---

## 3. 核心决策点（ADR-style）

### ADR-001: 进程模型

**问题**：aishd 用什么进程形态？

| 选项 | 说明 | Pro | Con |
|---|---|---|---|
| A. 独立 binary | `aishd` 是单独可执行文件，GUI 启动时检查并按需 spawn | 跨进程隔离最彻底；CLI/未来 client 都同等地位 | 工程量最大；冷启动慢 |
| B. GUI 内嵌 + 子进程 fallback | aish-app 内置 daemon，需要 headless 时用 `--daemon` 模式 | 不破坏单文件分发；GUI 用户零感知 | 模式分裂，长期维护两条路径 |
| C. GUI 内 tokio task | 不分离进程，只把状态层抽到独立模块，加 IPC 接口 | 工程量最小 | 解决不了 crash 隔离；CLI 仍然需要单独进程 |

**初步倾向**：**A**。理由：
- C 不解决 CLI 需求 → 本质上做了等于没做
- B 维护两条路径长期一定走偏
- A 是 paseo / WezTerm / Docker 都验证过的路径

但 A 的工程量是 C 的 5-10 倍。**是否做 daemon 化决策 ≈ 是否接受 A 的工程量**。

### ADR-002: 通信协议（wire）

**问题**：daemon 和 client 之间用什么 IPC？

| 选项 | 说明 | Pro | Con |
|---|---|---|---|
| A. Unix Socket / Named Pipe | 跨平台抽象（`tokio::net::UnixStream` + `NamedPipeClient`） | 性能最好；权限模型清晰（fs ACL） | 不支持远程 client |
| B. localhost TCP（如 paseo 的 6767） | 一律走 TCP `127.0.0.1:xxxx` | 实现简单；测试方便 | 端口冲突；安全靠 bearer token |
| C. WebSocket on TCP | TCP + WS 帧 | 浏览器 client 友好；自带帧边界 | 协议层最重；overhead 高 |
| D. **A + B 双绑** | 默认 Unix socket，可选开 TCP 给远程 | 兼顾性能和扩展 | 复杂度上升 |

**初步倾向**：**A**（v1）→ **D**（v2）。理由：
- aish 现阶段不需要远程 client（SSH 已解决远程需求）
- paseo 选 WebSocket 是因为它要支持浏览器 client，aish 没这个需求
- 借鉴 paseo 的 `ListenTarget` 抽象（`tcp://` / `unix://` / `pipe://` 一个 enum），即使 v1 只实现 Unix socket，也保留扩展位

### ADR-003: 序列化格式

**问题**：消息体怎么编码？

| 选项 | 说明 | Pro | Con |
|---|---|---|---|
| A. JSON (serde_json) | 文本协议 | 调试方便；schema evolution 友好 | 终端流大时 CPU + 带宽双高 |
| B. bincode / postcard | Rust 原生二进制 | 性能最好 | 跨语言难（CLI 是 Rust 没事，未来手机端难） |
| C. JSON 控制 + 二进制数据帧 | paseo 的做法：JSON 配数据用 binary frame | 控制面好调试，数据面够快 | 实现复杂 |
| D. MessagePack / CBOR | 二进制 JSON | 中庸方案 | 工具链不如 JSON 成熟 |

**初步倾向**：**C**。理由：
- aish 终端流量大（每个 alacritty grid 刷新都是 KB 级），JSON 序列化是瓶颈
- 控制消息（list_hosts、open_connection 等）走 JSON，开发友好
- 终端 I/O 走 binary frame，直接抄 paseo 的 `[opcode(1)][slot(1)][payload]` 布局

### ADR-004: alacritty grid 的归属

**问题**：alacritty `Term + Grid`（终端状态机）放哪？

| 选项 | 说明 | Pro | Con |
|---|---|---|---|
| A. daemon 内 | 权威状态在 daemon | crash 恢复 / 多 client 看同一画面都自然 | 每次刷新都要序列化 cell 跨进程，CPU 开销大 |
| B. client 内 | client 维护 grid，daemon 只转发 raw PTY 字节 | 性能最好；现有 GPUI 渲染代码零改动 | crash 恢复要 daemon 回放 scrollback，scrollback 仍要存 daemon |
| C. **两边都有 + diff sync** | daemon 维护权威 grid，新 client 接入时拉 snapshot，之后只增量同步 | 兼顾两者 | 实现复杂度最高 |

**初步倾向**：**B + scrollback in daemon**。理由：
- aish 不需要 paseo 那种「多个 client 同时看一个终端」的场景（手机看 desktop）
- alacritty Term 跨进程同步是个大坑（cell 结构 / dirty region 都 GPU 友好但跨进程很糟）
- 真正要保留的是「scrollback ring」—— daemon 维护一个 raw byte ring，client 接入时 replay 就好
- C 看起来很美但复杂度爆炸，不推荐做

⚠️ **这与 paseo 的选择相反**，因为 paseo 要支持多个 client 同时围观同一个 agent 的终端，aish 没这需求。

### ADR-005: 持久化层

**问题**：host config / connection history / 已知 fingerprint 放哪？

| 选项 | 说明 | Pro | Con |
|---|---|---|---|
| A. `~/.aish/` 文件型 JSON | 抄 paseo 的 `~/.paseo/` 结构 | 简单、可读、cat 调试 | 高频写入要原子操作 |
| B. SQLite | 单文件数据库 | ACID 完备；查询能力 | 引入依赖（rusqlite）；调试不便 |
| C. sled / redb | 嵌入式 KV | 性能好；Rust 原生 | 维护投入；冷数据没必要 |

**初步倾向**：**A**。理由：
- aish 数据量小（host 数 < 100，session record < 10k）
- 现在 host config 已经是某种平铺存储（具体路径待查 aish-secrets）
- paseo 验证了「JSON file + atomic rename」对这个规模完全够用
- `tempfile::NamedTempFile::persist` 一行原子写

目录建议：
```
~/.aish/
├── config.json              # daemon listen target, ui prefs
├── aishd.pid                # PID lock + hostname/uid（抄 paseo）
├── server-id                # 稳定 daemon id, "aish_<base64url>"
├── hosts/
│   └── <host-id>.json       # 一个文件一个 host（不再依赖 secrets crate 内部存储）
├── connections/             # 持久化的 connection 元数据
│   └── <conn-id>.json
├── scrollback/              # 每个 connection 一个 raw byte ring
│   └── <conn-id>.bin
└── logs/
    └── aishd.log
```

⚠️ **secrets 怎么办**：keyring（OS 钥匙串）继续负责密码/私钥，文件型存储只放
metadata（指向 keyring 的 entry name）。aish-secrets crate 应不需要大改。

### ADR-006: capability negotiation（协议演进策略）

**问题**：daemon 和 client 版本不同步时怎么办？

**强烈推荐抄 paseo 的 `CLIENT_CAPS` 模式**：

```rust
// client hello
{
  type: "hello",
  client_id: "...",
  client_type: "gui" | "cli" | "mcp",
  protocol_version: 1,
  capabilities: ["binary_term_v1", "scrollback_replay", "tmux_attach"],
}
```

- daemon 维护 `Session::supports(cap)` 查询能力
- 协议 schema 严格 **append-only**：新字段都可选，已发字段永不删
- 新特性走能力标签，不走版本号
- 优势：daemon / client 可以独立升级，不会因为版本错配挂掉

这是 paseo 学到的最值得抄的设计模式之一，**即使 daemon 化不做也应该抄到 aish 的 host config 演进上**。

### ADR-007: 终端 PTY 跑在哪个进程

**问题**：daemon 自身有可能崩，PTY 是不是要再往下一层放？

| 选项 | 说明 |
|---|---|
| A. 跟 daemon 同进程（tokio task） | 简单 |
| B. daemon spawn 子进程跑 PTY（paseo worker 模式） | 隔离更彻底 |

**初步倾向**：**A**。理由：
- Rust + russh 远比 paseo 的 node-pty（C++ binding）稳定
- daemon 化已经够大工程，再加 worker process 边际收益低
- 真有问题以后再加（接口可以预留）

---

## 4. Wire protocol 草案（v0）

### 4.1 顶层帧

借鉴 paseo 的混合协议：JSON 文本帧（控制） + 二进制帧（终端 I/O 数据面）。

```
JSON 帧（控制）:
  { "type": "hello", ... }
  { "type": "session", "payload": { ... } }       // 一切业务 RPC 都包在 session
  { "type": "ping" } / { "type": "pong" }

二进制帧（终端流）:
  [opcode:u8][slot:u8][payload:variable]
  opcode: 0x01 Output / 0x02 Input / 0x03 Resize / 0x04 Snapshot
  slot:   ConnectionId 的紧凑映射（同一 session 内）
```

### 4.2 session message 类型（示意，非穷尽）

```rust
// 命名约定借鉴 paseo: dotted namespace + .request / .response
enum SessionMessage {
    // host 管理
    HostListRequest,
    HostListResponse(Vec<HostMetadata>),
    HostCreateRequest(HostConfig),
    HostCreateResponse(HostId),

    // connection 生命周期
    ConnectionOpenRequest { host_id: HostId, tmux_session: Option<String> },
    ConnectionOpenResponse { connection_id: ConnectionId, slot: u8 },
    ConnectionClose { connection_id: ConnectionId },

    // 状态推送（server → client）
    ConnectionStateUpdate { connection_id: ConnectionId, state: ConnState },
    HostUpdated { host_id: HostId, metadata: HostMetadata },

    // 终端控制（数据走 binary frame，命令走这里）
    TerminalSubscribe { connection_id: ConnectionId },
    TerminalUnsubscribe { connection_id: ConnectionId },
    TerminalCapture { connection_id: ConnectionId, start: Option<i32>, end: Option<i32> },

    // 错误
    RpcError { request_id: Uuid, message: String, code: ErrorCode },
}
```

### 4.3 Hello 握手

```
client → server: { type: "hello", client_id, client_type, protocol_version: 1,
                   capabilities: ["binary_term_v1", "scrollback_replay"] }
server → client: { type: "status", payload: {
                   status: "server_info", server_id, version,
                   capabilities: ["binary_term_v1", "scrollback_replay", "tmux_attach"] } }
```

---

## 5. Crate 拆分提案

```
aish/
  crates/
    aish-types/       # （现有）基础类型
    aish-secrets/     # （现有）keyring 封装
    aish-protocol/    # 🆕 wire schema + frame codec（client/daemon 共用）
    aish-ssh/         # （现有，搬到 daemon 内部）
    aish-tmux/        # （现有，搬到 daemon 内部）
    aish-sftp/        # （现有，搬到 daemon 内部）
    aish-daemon/      # 🆕 binary：aishd
    aish-cli/         # 🆕 binary：aish (CLI subcommands)
    aish-client/      # 🆕 lib：连 daemon 的 client（CLI/GUI 共用）
    aish-app/         # （现有，但 SSH/tmux/sftp 调用全部改走 aish-client）
    aish-ui/          # （现有，不动）
```

依赖图：
```
aish-app ─┐
aish-cli ─┴─→ aish-client ─→ aish-protocol ─→ aish-types
                  │ (UDS / TCP)
                  ▼
aish-daemon ──→ aish-ssh / aish-tmux / aish-sftp
```

---

## 6. 实施分期建议（每期独立 ship 价值）

> 任何一期之后停下来，aish 仍然是可用的；**最重要的设计原则**。

### Phase 0 — 不做 daemon 化，先抄低成本设计

**周期**：1-2 周。**门槛**：极低。**ROI**：很高。

不依赖 daemon 也能落地的 paseo learning：

- [ ] `~/.aish/` 持久化结构 + atomic JSON 写（替换现有 host config 存储）
- [ ] capability 标签机制（在 host config schema 加 `capabilities: Vec<String>`）
- [ ] mnemonic ID（用 `petname` crate 给 connection 起人类可读 ID）
- [ ] `ListenTarget` 抽象（即使现在没 daemon，host SSH address 解析也能用这种 enum union）

**这一期单独就值得做**，跟 daemon 化决策解耦。

### Phase 1 — `aish-protocol` + `aish-client` 抽象层

**周期**：3-4 周。**门槛**：中。**ROI**：中。

不引入新进程，只是把 aish-app 内部直接调 `aish-ssh` 的地方改成走 `aish-client::Client` trait，其中实现还在同进程内。

- [ ] 设计 wire schema（先 in-memory mpsc 实现）
- [ ] aish-app 全部业务调用走 client trait
- [ ] **不破坏当前体验** —— 用户感知零变化

**价值**：协议表面被冻结，后续切真 daemon 几乎只改 transport 层。**风险**：可能"为了 daemon 化做的准备工作，最后 daemon 化又没做"，变成纯负债。

### Phase 2 — `aishd` 二进制 + Unix Socket / Named Pipe 实现

**周期**：6-8 周。**门槛**：高。**ROI**：中-高（取决于 CLI / MCP 是否真做）。

- [ ] aishd 独立 binary，监听 UDS / NPC
- [ ] aish-app 启动时按需 spawn / attach aishd
- [ ] PID lock + 单实例保护
- [ ] 持久化层归 daemon 所有
- [ ] scrollback 持久化（关掉 GUI 再开恢复历史）

**这一期完成后** GUI 重启不丢连接，但还没有 CLI。

### Phase 3 — `aish-cli` 基础命令

**周期**：1-2 周。**门槛**：低（前提是 Phase 2 完成）。**ROI**：高。

- [ ] `aish ls` / `aish hosts` / `aish open <host>` / `aish kill <conn>`
- [ ] `aish logs <conn>` 输出 scrollback
- [ ] `aish daemon status` / `aish daemon stop`

### Phase 4（可选） — MCP server

**周期**：2-3 周。**门槛**：中。**ROI**：高（如果 AI agent 集成成为主线需求）。

- [ ] aishd 内嵌 MCP stdio endpoint
- [ ] 暴露 `list_hosts` / `open_connection` / `send_keys` / `capture_terminal` 给 Claude Code 等 agent

### Phase 5（不推荐近期做） — TCP / 远程 client

仅在「真的有远程访问 aishd 的需求」时考虑。aish 当前完全不需要。

---

## 7. Risk 表

| Risk | 严重度 | 概率 | mitigation |
|---|---|---|---|
| **工程量超预算** | 高 | 高 | 严格按 Phase 0 → Phase 4 顺序做；Phase 0 即可独立闭环；中途允许停 |
| **CLI / MCP 需求不成立，daemon 化变纯负债** | 高 | 中 | Phase 0/1 做了的东西即使不上 daemon 也有用；Phase 2 之前可以及时止损 |
| **alacritty grid 跨进程同步性能差** | 中 | 中 | ADR-004 选 B 方案（grid 留客户端），daemon 只持久化 raw scrollback |
| **Windows Named Pipe 行为 quirky** | 中 | 中-高 | 用 `tokio` 抽象 + paseo 的 `ListenTarget` 模式；早期就在 Windows 跑通 |
| **协议演进失控（v1/v2 兼容地狱）** | 中 | 中 | 严格执行「append-only schema + capability flags」规则 |
| **daemon crash 把所有连接干掉，反而更脆** | 高 | 低 | daemon 极少业务逻辑，PTY/IO 是 tokio task；test 覆盖率要求 ≥ 当前 aish-ssh |
| **keyring 跨进程访问差异** | 中 | 低 | daemon 进程持 keyring 句柄；其他 client 不直接访问 keyring |
| **单实例假设被破坏（用户启了两个 aishd）** | 中 | 中 | PID lock + 验证 hostname/uid（抄 paseo） |
| **冷启动慢（用户敲 `aish` 第一次要等 daemon 起）** | 低 | 中 | Phase 2 加入 lazy spawn + 健康检查；目标 < 200ms |
| **alacritty/russh 版本升级被 daemon 协议层 lock-in** | 中 | 低 | protocol 层不暴露内部类型，全部用稳定的 plain struct |

---

## 8. Open Questions

这些问题在 brainstorm 阶段没有答案，需要后续决策：

1. **CLI / MCP 是不是真需求？**
   - aish 用户群（包括用户自己）多大比例会用 CLI？
   - 有没有具体的 AI agent 集成 use case 已经在脑子里？
   - 如果都是「未来可能用得上」级别，不建议为它做 daemon 化

2. **要不要支持「daemon 在远程机器上跑」？**
   - 如果要，wire protocol 必须从 v1 就考虑 auth / TLS
   - 如果不要（推荐），UDS 就够，TLS / bearer token 全部不做

3. **scrollback 持久化的容量上限？**
   - 每个 connection 留多少 KB？1MB? 100MB? 不限？
   - 持久化策略：append-only + 压缩 ring，还是 SQLite-like 索引？

4. **现有 aish-secrets crate 怎么进 daemon？**
   - keyring 句柄只在 daemon 里？client 永远拿不到明文？
   - 那 GUI 显示「这个 host 用了哪个 key」之类的元信息怎么走？

5. **能不能跳过 Phase 1（client 抽象层），直接做 Phase 2？**
   - Pro：少一期纯重构
   - Con：transport 切换时业务代码要大改

6. **要不要在 daemon 里直接跑 GPUI 的渲染**（headless 渲染服务器）？
   - paseo 在 server 跑 xterm/headless 是因为多 client 看同一画面
   - aish 单 GUI 场景下不需要，**不建议**

---

## 9. 推荐结论（brainstorm 阶段）

**短期（接下来 1-2 个月）**：
- 做 **Phase 0**（不依赖 daemon 的设计模式抄过来）。这是稳赚不赔的投资
- 不立刻启动 Phase 1+，先观察 CLI / MCP 需求是否在实际使用中浮现

**中期触发条件**：
- 如果出现明确的 CLI 用例（用户自己开始想 `aish ls`），启动 Phase 1
- 如果出现明确的 AI agent 集成需求（想让 Claude Code 驱动 aish），跳过 Phase 1 直接做 Phase 2 + 4

**长期不做**：
- Phase 5（远程 daemon）—— 跟 aish 「local-first 桌面 SSH 客户端」定位冲突

**不推荐路径**：
- 为了「GUI 偶尔崩 / 重启慢」启动 daemon 化 —— ROI 太低
- 整个工程一次推完 —— 大概率拖死，必须分期可停

---

## 10. Next Step

如果决定推进，下一步是：

1. **写 Phase 0 的 plan**（`docs/superpowers/plans/YYYY-MM-DD-aish-daemonize-phase0.md`）
   - 范围明确：~/.aish 持久化 + capability flag + mnemonic ID + ListenTarget enum
   - 不触碰任何运行时状态层
2. **WezTerm 架构调研**（一天工作量）
   - 它的 `wezterm-mux-server` 跟本 spec 的 Phase 2 几乎完全等价，看它怎么做的避免重新造轮子
3. **跟自己（或团队）对齐 ADR-001 ~ ADR-007 的选择**
   - 本 spec 给出的是初步倾向，每个 ADR 都可以推翻

如果决定不推进：
- 把 Phase 0 拆成独立 milestone 推（不挂 daemon 化标签），其他 ADR 归档
- 把本 spec 标 `status: archived`，保留作为「我们考虑过 daemon 化，决定不做」的决策记录
