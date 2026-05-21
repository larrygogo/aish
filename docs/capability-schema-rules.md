# Capability Schema 演进规则

> issh 的远端能力探测数据怎么定义、怎么演进、怎么不踩兼容性坑。
>
> 触发本文件的 milestone：2026-05-20 RemoteCapabilities 重构
> （[plan](superpowers/plans/2026-05-20-aish-remote-capabilities.md)）。

---

## 1. 什么是 capability

issh 在 SSH 连接建立后，会探测远端的各种属性来调整本地行为。比如：

- 知道远端是 ubuntu → Host 卡片显示 Ubuntu logo
- 知道远端 tmux 没开 mouse → 弹 toast 引导用户
- 未来：知道远端支持 OSC 52 → 启用跨终端剪贴板透传
- 未来：知道远端架构是 arm64 → 推荐对应预编译二进制

这些**探测出来的远端属性**统称 **capability**。

---

## 2. 两种 capability，分两层管

### Host-level capability — 跨连接稳定，**持久化**

特征：
- 对同一个 host 几乎不变（除非用户重装系统、换发行版）
- 探测一次后可以缓存，下次连接不用重测
- 例子：`os_kind` / `arch` / 默认 `shell`

存放：`HostConfig.capabilities: HostCapabilities`（在 `issh-types/src/lib.rs`）
持久化：随 `hosts.json` 一起 atomic write

### Session-level capability — 每次连接重测，**不持久化**

特征：
- 每次 SSH 连接可能变（用户改了 `~/.tmux.conf`、装/卸 tmux、shell 切换）
- 必须每次连接重新探测
- 例子：`tmux_installed` / `tmux_mouse_on` / `tmux_version` / `osc52_supported`

存放：`SshEvent` 推送 + `state.rs` 内存状态（如 `tmux_state: HashMap<ConnectionId, TmuxState>`）
持久化：**不持久化**

**判断准则**：「用户重启 issh 后这个能力还成立吗？」
- 是 → host-level，写 `HostCapabilities`
- 不一定 → session-level，走 event + state

---

## 3. Schema 演进规则（append-only）

issh 的 `hosts.json` 没有 migration 框架，靠 schema 自身的**前向兼容**保证升级
不破坏。规则借鉴 paseo「append-only schema + capability flag」+ WezTerm
「append-only enum + ident 永不复用」的组合。

### 3.1 只增字段，不删字段

旧字段即使废弃也保留：删字段 ≡ 删数据。如果一个 capability 不再有意义：
- 加注释 `/// DEPRECATED since vX.X — 改用 new_field`
- 反序列化继续接受
- 序列化时同步写 new_field（双写过渡期）
- 真要清掉至少跨一个 minor release，且让用户至少打开 issh 一次完成迁移

### 3.2 只放 `Option<T>` + `#[serde(default)]`

`HostCapabilities` 里所有字段都必须：

```rust
#[derive(Default, ...)]
pub struct HostCapabilities {
    #[serde(default)]
    pub some_capability: Option<T>,  // None = 还未探测或探测失败
}
```

理由：
- 旧 `hosts.json` 不带该字段 → 加载成 `None`，不报错
- `None` 不是「探测过 + 没有」，是「还未探测过」—— 下次连接自动重试

### 3.3 不改语义

如果某个 capability 字段语义变了（比如 `os_kind` 从 `/etc/os-release` 的
ID 改成 `uname -s` 的输出），**新加一个字段，不要原地改**：

```rust
pub struct HostCapabilities {
    // 老语义保留
    #[serde(default)]
    pub os_kind: Option<String>,         // /etc/os-release ID
    #[serde(default)]
    pub uname_system: Option<String>,    // uname -s 输出
}
```

读取方根据需要选择字段。如果觉得字段过多累赘，参照 3.1 的弃用流程。

### 3.4 新字段命名空间

`HostCapabilities` 字段名跟 `HostConfig` 自身字段不能撞。当前 `HostConfig`
平铺字段：`id / label / host / port / user / auth / env_profile / capabilities`。
未来 `HostCapabilities` 内新字段避免使用这些名字。

> 注：当前 `HostCapabilities` 是嵌套字段（`capabilities: {...}`），命名冲突
> 影响小；但 future-proofing 仍然要求避免，万一以后改成 `#[serde(flatten)]`
> 不踩坑。

---

## 4. 加新 capability 的 checklist

每次往 `HostCapabilities` 加字段，按这个清单走：

- [ ] 字段类型用 `Option<T>` 或 `T: Default`
- [ ] 加 `#[serde(default)]`
- [ ] doc comment 写清：
  - 探测时机（哪个 actor task / 什么命令）
  - `None` 的语义（一定是「未探测」，不是「未安装」）
  - 用途（哪个 UI / 行为决策用它）
- [ ] 在 `issh-types/src/lib.rs` 加 roundtrip 测试
- [ ] 在 `issh-types/src/lib.rs` 加「字段缺失时 default 为 None」测试
- [ ] 探测函数加单元测试（在 `ssh_actor.rs` 或专门的 `capabilities/` 模块）
- [ ] **不**为新字段加版本号或迁移逻辑 —— 走 append-only

---

## 5. 跟 daemon 化 spec 的关系

本文档定义的规则适用于 **issh 当前单进程架构**。如果未来推进
[daemon 化](superpowers/specs/2026-05-20-aish-daemonize-design.md)（spec
ADR-006），还会有一层 **wire-level capability**（daemon ↔ client 协议能力
协商），那是另一码事：
- 本文档的 capability = 远端 SSH server 的能力
- wire-level capability = 本地 daemon ↔ client 的协议版本

两层不要混在一起。

---

## 6. 当前状态（v0.1.0）

`HostCapabilities` 只有一个字段：

```rust
pub struct HostCapabilities {
    #[serde(default)]
    pub os_kind: Option<String>,
}
```

session-level capability 暂未结构化，仍走 `SshEvent` + `state.rs::TmuxState`。
真等到 session-level 字段数 ≥ 3 或者跨连接共享需求出现时，再抽
`SessionCapabilities` struct。当前不抽 = 避免过度设计。
