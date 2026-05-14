# M22 — InputBar per-connection draft 隔离（设计）

> 状态：✅ 已实施（2026-05-14）
> Plan：[`../plans/2026-05-14-aish-m22-inputbar-per-connection.md`](../plans/2026-05-14-aish-m22-inputbar-per-connection.md)

## 1. 现状（why now）

`InputBarView` 在 `RootView` 是**全局单例**（M9 落地，M19/M20 仅做 UX
迭代未触动单例假设）：

```rust
// crates/aish-app/src/app.rs（M22 之前）
struct RootView {
    // ...
    input_bar: Entity<crate::views::InputBarView>,
}
```

所有 SSH connection tab 共享同一份：
- `input: Entity<TextInput>` —— 未发送文字
- `images: Vec<PendingImage>` —— 未发送图片缩略图
- TextInput 自身 cursor / IME composition / selection state

切 tab 时上一个 shell 写到一半的内容会显示在另一 shell 的输入栏里。
用户报告："inputbar 是多个 shell 共享的，要改成 shell 独享"。

注意 `pending_uploads` / `last_aborted_batch` 已经按 `ConnectionId` 分桶
（`state.rs:457/462`），所以"上传中状态"切 tab 切换时**已能**正确恢复 —
M22 仅补齐**草稿层**隔离。

## 2. ADR — 三个候选方案对比

| 方案 | 思路 | 优 | 劣 |
|---|---|---|---|
| **A. per-conn InputBarView entity** ✅ 采用 | RootView 持 `HashMap<ConnectionId, Entity<InputBarView>>`，每 conn 一个 entity；切 tab 时 render 拿对应 entity | 彻底隔离：TextInput cursor / IME / spinner / images 全 per-conn；conn 销毁时 entity 自动 drop 释放 timer + 内存；新加 conn 不影响已有 conn | 多个 spinner / drag polling timer 并存（2N 个），但 timer 内只读 self.conn 不互相干扰；off-screen entity 不 paint |
| B. 单例 InputBarView + draft Map | InputBarView 内 `HashMap<ConnectionId, (text, images)>`，切 tab 时 load 当前 conn 的 draft 到唯一 TextInput entity | 内存稍省（一个 TextInput） | TextInput cursor / IME / selection 全得手动 save/restore，IME composition 中切 tab 会被打断；逻辑复杂 |
| C. Draft 进 AppState | text_drafts / image_drafts 进 AppState，与 pending_uploads 同层 | 状态全局可见，便于 debug | 同 B 的 TextInput state 问题；AppState 字段膨胀 |

**采用 A**。GPUI Entity 生命周期模型天然适合 per-conn 隔离 —— Entity drop
自动清理内部 observe / cx.spawn task；HashMap retain 是同步 stale 的最干净
入口。

## 3. 生命周期

| 事件 | input_bars HashMap 行为 |
|---|---|
| `open_connection` → 新 ConnectionId 加入 `connections` | 不立即建 entity；等用户切到该 tab、render 内 lazy create |
| 切 tab 到该 conn（首次） | render 内 `cx.new(InputBarView::new(conn, ...))` + insert |
| 切 tab 到该 conn（已存在） | render 内 lookup HashMap，直接 clone entity |
| `drop_session`（actor sender 销，conn 元数据保留） | HashMap 不变；entity 内 render 读 `connection_phases.get(self.conn)` 得 Disconnected，Send 按钮 disabled |
| `reopen_connection`（同 ConnectionId 重连） | HashMap 不变；草稿保留；render 重新拿 Connected，Send 恢复 |
| `remove_connection`（conn 元数据销，tab 一起关） | RootView observe(state) 回调内 `retain_alive_entities` 检测 `state.connections.contains_key` false → drop entity → Drop chain 清理 spinner / drag timer / TextInput 子 entity / images |
| Default tab（current_connection() == None） | render 内 `if let Some(conn) =` 守护，主体不挂 InputBar |

## 4. Disconnected 视觉决策

Plan 阶段澄清两个边界：

**Q1：Default tab 上是否显示 InputBar？**
- 决策：不渲染。Default tab 没有对应 conn，原 InputBar 是 noop（send 时
  `current_connection() == None` 直接 silent return），视觉空 input bar 给
  用户错误暗示。M22 后 main_body Terminal 分支 `if let Some(conn) =` gate。

**Q2：Disconnected 状态保留 InputBar 还是隐藏？**
- 决策：保留 + Send 按钮 disabled。TextInput / `+` 按钮 / 缩略图保留可编辑
  能力 —— 让用户在断网期间继续写草稿，双击 tab 重连后草稿原样保留可发送。
- 实现：新 `is_connected` 通道 `matches!(connection_phases.get(self.conn),
  Some(Connected))`，仅作用于 Send 按钮 `.disabled(is_uploading ||
  !is_connected)`。
- 不复用 is_uploading 通道：避免 Disconnected 时 Send label 错乱显示
  "上传中 0/0"（label 仍是 "发送"，disabled 灰已表达不可点）。

## 5. 风险表

| 风险 | 触发 | 缓解 |
|---|---|---|
| render 内 `cx.new(...)` + `self.input_bars.insert(...)` 双 mut borrow | rustc borrow check | 局部变量分两步：先 cx.new 得 entity，再 insert（已落地 `app.rs:474`） |
| observe(state) 回调每次 state.notify 都跑 retain | `feed_bytes` 每秒数十次 notify | retain O(N) 且 N=同时 alive 的 conn 数（通常 < 10），无 alloc，可忽略 |
| spinner / drag polling timer 在 entity drop 后多跑 1 tick | 80-100ms 内 | 现有 `this.update().unwrap_or(false) → break` 模式自动退出 |
| Default → Connection tab 首次切换 InputBar 创建延迟 | 首次进入该 conn | `InputBarView::new` 仅栈构造，亚毫秒，肉眼无感 |
| reopen_connection 复用同 ConnectionId | Disconnected → 双击 tab 重连 | 草稿自动保留（feature） |
| 多 conn 并存时 2N 个 timer 持续轮询 | N 个 conn alive | 每 timer 仅读 self.conn 互不干扰；CPU 微不足道（80ms / 100ms 间隔） |

## 6. 决策 ADR 索引（项目 ADR 章节）

本 milestone 不引入新的项目级 ADR — 选择 A 方案是 GPUI Entity 模型的
自然延伸，不属于"GUI framework / SSH library"那一档重大决策。决策记录在
本 spec 内即可。
