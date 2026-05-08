# tmux 缩放跳动修复（M3d-resize-iter1）— Design Spec

**Goal**：减少拖窗 / 远端 tmux attach 时的字符跳动 / 错位，靠两个改动：
延长 PTY resize debounce + 把本地 Term resize 延后到 SIGWINCH 之后。

**Non-Goal**：
- 不动 alacritty 的 grid floor 取整公式（#1 — 公式正确，未确认是 bug）
- 不动 tmux attach -x/-y 参数（#2 — raw attach 单 client 默认 latest，未撞到）
- 不引入 refresh-client（#3 — raw attach 模式 SIGWINCH 已够，是 -CC 模式专用）
- 不做可配置（debounce / delay 写死 const）

**用户决策**：只修 #4 + #5（"先复现再修"路径与"全部预防性修"之间的中庸方案）。

---

## 1. 触发原因

用户反馈："tmux 的缩放适配兼容的并不好"。

复盘 5 个候选薄弱点（前一轮对话已分析），最终聚焦在 #4 + #5：

| # | 问题 | 状态 |
|---|---|---|
| 1 | `floor` 取整边界吃行 | 公式正确，留观察 |
| 2 | shared session min size | tmux 默认 latest，单 client 不会撞 |
| 3 | 没 refresh-client 兜底 | raw attach 模式 SIGWINCH 已够 |
| 4 | **本地 Term vs 远端时序** | ✅ 真问题 |
| 5 | **debounce 100ms 太短** | ✅ 真问题 |

#4 #5 共同导致拖窗时的"跳动"症状。

---

## 2. 改动设计

### 2.1 #5 — debounce 100ms → 250ms

**文件**：`crates/aish-app/src/views/terminal_view.rs:471`

```rust
// 原 100ms 拖窗时每帧重排远端，250ms 等拖动稳定后再发 SIGWINCH
const PTY_RESIZE_DEBOUNCE_MS: u64 = 250;
```

放文件顶部 `use` 区下方，作为 module-level const。原代码里 `Duration::from_millis(100)`
改读这个常量。

### 2.2 #4 — 本地 Term resize 延后到 SIGWINCH 之后

**问题本质**：

```
当前流程：
T0   debounce 完成
T0   state.resize_term(本地 alacritty Term 立即按新 size 排)
T0+ε SessionCommand::Resize → ssh_actor → chan.window_change (SIGWINCH)
T0+RTT 远端 tmux 收 SIGWINCH 重排
T0+RTT+ε 新 size 字节流到达本地

→ T0..T0+RTT 之间：远端按旧 size 吐字节、本地按新 size 排 → ANSI 错位
```

**修法**：把 `state.resize_term`（本地）从 `SessionCommand::Resize`（远端）**之前**
挪到**之后**，并补一个 ~80ms 短延迟覆盖 SSH RTT。

```rust
// terminal_view.rs:443~491 闭包重排成两段计时：
// 1. PTY_RESIZE_DEBOUNCE_MS — 等拖动稳定
// 2. 立刻发 SessionCommand::Resize (SIGWINCH 透传到远端)
// 3. LOCAL_TERM_RESIZE_DELAY_MS — 等远端 RTT 估值，避免本地 Term 超前
// 4. state.resize_term — 本地 alacritty Term 按新 size 排

const PTY_RESIZE_DEBOUNCE_MS: u64 = 250;
const LOCAL_TERM_RESIZE_DELAY_MS: u64 = 80;

let task = cx.spawn(async move |_this, cx| {
    // 1. debounce
    cx.background_executor()
        .timer(Duration::from_millis(PTY_RESIZE_DEBOUNCE_MS))
        .await;

    // 2. 先发 SIGWINCH 给远端
    let sender_opt = cx.update(|app| {
        state.update(app, |s, _cx| s.sessions.get(&conn).cloned())
    });
    if let Some(sender) = sender_opt {
        bridge.spawn(async move {
            let _ = sender.send(SessionCommand::Resize { cols, rows }).await;
        });
    }

    // 3. 等远端 RTT 落地（粗估）
    cx.background_executor()
        .timer(Duration::from_millis(LOCAL_TERM_RESIZE_DELAY_MS))
        .await;

    // 4. 本地 Term 跟随 size，让后续到达的字节按新 size 排
    let _ = cx.update(|app| {
        state.update(app, |s, cx| {
            s.resize_term(conn, cols, rows);
            cx.notify();
        })
    });
});
```

### 2.3 不动

- `last_pty_size` 缓存 / debounce 取消（drop pending_resize）逻辑保留
- `alacritty_terminal::term::resize` 调用本身不变
- `chan.window_change` SSH 调用本身不变
- `make_term`、`host_pty_dimensions` 等 state 字段不变

---

## 3. 时序对比

```
原流程（用户拖窗一次）：
─┬──────────────────────────────────────────
 │ bounds 变化
 ├─ 100ms 后
 │    ├─ resize_term（本地 Term 新 size）
 │    └─ SIGWINCH 发出
 │       ├─ ~RTT 后远端开始吐新字节
 │       └─ 本地按新 size 排，远端旧字节错位 ←❌

新流程：
─┬──────────────────────────────────────────
 │ bounds 变化
 ├─ 250ms 后
 │    ├─ SIGWINCH 发出
 │    └─ 80ms 后
 │       └─ resize_term（本地 Term 新 size）
 │          └─ 此时远端已按新 size 吐字节，本地按新 size 排，对齐 ✅
```

---

## 4. 已知风险

| 风险 | 应对 |
|---|---|
| 80ms 太短覆盖不到高延迟（越洋 200ms+ RTT） | 接受。常见局域网 / 国内云 RTT < 50ms；高延迟用户可后续加可配置环境变量 |
| 250 + 80 = 330ms 总响应时间，拖结束后视觉滞后 | 人眼对 < 500ms 拖结束反馈不敏感；优先消除"跳动"比"即时响应"价值高 |
| 本地 Term 推迟 resize → 这段时间多吃了"旧 size 字节" | 这正是目标——这些字节本来就是远端按旧 size 算的 ANSI，本地按旧 size 排版才**正确**对齐 |
| `cx.update` / `cx.spawn` 在闭包内做多次 await，pending_resize task 被 drop 时取消 | 现有 drop 取消机制保留；中途 drop 会让 SIGWINCH 已发但本地 Term 不 resize — 下次 check_resize 仍会触发，最终一致 |

---

## 5. 验证

### 5.1 自动化

- `cargo +nightly fmt --all` — 0 diff
- `cargo +nightly clippy --workspace --all-targets -- -D warnings` — 0 warning
- `cargo test --workspace` — 现有 199 个 test 不应失败

无新增 unit test：涉及多段 timer + 远端时序，单测无法准确覆盖；唯一关键参数是两个常量值，写死且有注释说明依据。

### 5.2 手测

- `cargo run -p aish-app` → 连一个 host → tmux attach
- 拖动窗口边缘改变大小：
  - 拖动**期间** SIGWINCH 不发（每 250ms 才发一次，松手前没到第二次）
  - 松手 ~330ms 后远端 + 本地一起按新 size 排，**无字符跳动 / 错位**
- 远端跑 vim / less / btop 等大量 ANSI 重定位的程序，拖窗后内容**对齐**

---

## 6. 实施分解

详见 plan。预计 1 个 commit（两个常量改动 + 闭包重排，逻辑同一段，分割反而碎）。
