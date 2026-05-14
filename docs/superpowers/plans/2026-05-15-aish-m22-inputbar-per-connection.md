# M22 — InputBar per-connection draft 隔离（实施计划）

> 状态：✅ 已完成（2026-05-15）
> Spec：[`../specs/2026-05-15-aish-m22-inputbar-per-connection-design.md`](../specs/2026-05-15-aish-m22-inputbar-per-connection-design.md)

## 范围

把 `InputBarView` 从 RootView 单例改成 per-ConnectionId 实例，让每个 SSH
shell tab 拥有独立的未发送文字 + 图片缩略图 + TextInput cursor / IME。

## File Structure

| 文件 | 改动性质 |
|---|---|
| `crates/aish-app/src/views/input_bar.rs` | 加 `conn: ConnectionId` 字段 + new 签名变更 + 5 处 `current_connection()` 改 `self.conn` + 新 `is_connected` 通道 + Send 按钮 `disabled(is_uploading \|\| !is_connected)` |
| `crates/aish-app/src/app.rs` | RootView 单例 → `HashMap<ConnectionId, Entity<InputBarView>>` + observe reconcile + Default tab 不挂 InputBar + 提取 `retain_alive_entities` helper + 4 个单测 |
| `crates/aish-app/src/state.rs` | 只读：`current_connection()` / `ConnectionPhase` / `connections` 字段 |

## Tasks（按 commit 顺序）

### T1 — InputBarView 接 conn + RootView HashMap 化

**Commit**：`403adb9`

**改动**：
- `InputBarView` 加 `conn: ConnectionId` 字段
- `pub fn new(conn, state, bridge, cx)` 签名变更
- 5 处 `state.current_connection()` 全部改 `self.conn`：
  - `is_uploading()` —— 改 `state.pending_uploads.contains_key(&self.conn)`
  - `send()` —— 直接用 self.conn，删 `current_connection() → None` 兜底分支
  - spinner timer —— 仅看 self.conn 是否在上传
  - render 内 `upload_progress` —— 直接 `pending_uploads.get(&self.conn)`
  - render 内 BatchAborted 边沿 —— 同上
- `RootView.input_bar: Entity<InputBarView>` 删
- `RootView.input_bars: HashMap<ConnectionId, Entity<InputBarView>>` 新增
- `RootView.bridge: Arc<Bridge>` 字段（lazy create 时用）
- `RootView::new` 不再预创建 input_bar
- 现有 `cx.observe(&state, ...)` 回调加 retain：清掉 connections.remove 后的 stale entity
- render Terminal 分支：
  - `current_conn = app.current_connection()`
  - `Some(conn)`：lazy create + insert（分两步避免 cx.new + insert 双 mut borrow）→ 挂对应 entity
  - `None`（Default tab）：不挂 InputBar，main_body 只 tab_bar + terminal_area

**Self-Review 完成**：
- [x] `cargo +nightly fmt --all` 已跑
- [x] `cargo +nightly clippy --workspace --all-targets -- -D warnings` 0 warning
- [x] `cargo test --workspace` 全过
- [x] borrow checker 走分两步 insert 路径

### T2 — Disconnected 状态 Send 按钮 disabled

**Commit**：`c15b8e7`

**改动**：
- import `ConnectionPhase`
- render 顶部计算 `is_connected = matches!(connection_phases[self.conn], Connected)`
- Send 按钮 `.disabled(is_uploading || !is_connected)`
- TextInput / `+` 按钮 / 缩略图 不动（草稿可编辑等重连）
- label 不改

**Self-Review 完成**：
- [x] cargo 三件套
- [x] is_connected 不复用 is_uploading 通道（避免 label 错乱）

### T3 — retain_alive_entities helper + 4 个单测

**Commit**：`b0dd3fc`

**改动**：
- 抽 `pub(crate) fn retain_alive_entities<K, V>(map: &mut HashMap<K, V>, mut is_alive: impl FnMut(&K) -> bool)`
- RootView observe 回调改调它
- 4 个单测：`keeps_all_when_all_alive` / `drops_all_when_none_alive` /
  `drops_only_stale` / `handles_empty_map`

**Self-Review 完成**：
- [x] cargo 三件套
- [x] aish-app 140 → 144 tests

### T4 — Spec + Plan + INDEX 文档

**Commit**：本 commit

**改动**：
- 新建 `specs/2026-05-15-aish-m22-inputbar-per-connection-design.md`（含 ADR / 生命周期表 / Disconnected 决策 / 风险表）
- 新建 `plans/2026-05-15-aish-m22-inputbar-per-connection.md`（本文件）
- `INDEX.md` 顶部当前状态 + Milestones 列表追加 M22 entry

## Verification（手动 end-to-end）

1. 构建：`cargo run -p aish-app`
2. 开 host-A 连接 → conn-A tab
3. InputBar 输 "AAA" + 添加一张图（`+` 按钮选）
4. Home 开 host-B 连接 → conn-B tab，应**无** "AAA" 和图
5. InputBar 输 "BBB"
6. 切回 conn-A tab → 应仍显示 "AAA" + 图
7. 点 Send → 远端收到 "AAA" + 图，InputBar 清空
8. conn-A 拔网模拟 NetworkError → tab 进 Disconnected → Send 按钮变灰 / TextInput 仍可输入
9. TextInput 输 "after-disc"，双击 tab 重连成功 → Send 恢复 / "after-disc" 仍在 → 点 Send 发出
10. Home sidebar 切回 Terminal、tab 列表空 → EmptyTerminalGuideView 无 InputBar
11. 开 host-A，关 tab，**重新**开 host-A —— 新草稿空白（新 ConnectionId）
12. `cargo test --workspace` 全过；retain_alive_entities 4 个测试通过

## 已知边界

- 多 conn 并存时 2N 个 spinner / drag polling timer 并存（每 conn 2 个）。
  每 timer 仅读 self.conn 互不干扰；CPU 微不足道。
- reopen_connection 复用同 ConnectionId 时草稿自动保留 —— 副作用，
  与"Disconnected 时草稿不丢"用户预期一致。
