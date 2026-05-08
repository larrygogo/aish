# 删掉 ConnectionChip 横条（M3d-ui-iter2）— Design Spec

**Goal**：移除终端区上方的 ConnectionChip 横条，把横条独有的 `[SSH]` 类型徽章并入
tab 标题里。

**Non-Goal**：
- 不重新设计 tab 栏整体布局（高度 / 圆角 / 间距维持现状）
- 不动 actor / state / SSH 层
- 不补"折叠保留连接"的新 UI 入口（功能在 UI 层暂时丢失，等 M3c+ 默认页"活跃连接区"再恢复）

**用户决策**：
- 横条删除方案：**A**（直接删，[SSH] 移到 tab 标题）
- SSH 标识：**保留蓝色胶囊**（沿用 ConnectionChip 上原样式：`px_2p5 py_0p5 text_xs ACCENT_BLUE bg CHIP_BLUE_BG rounded_full`）

---

## 1. 触发原因

用户反馈："不懂顶部那个横条的意义是什么"。盘点后确认横条与 tab 栏 99% 信息重复：

| 信息 | Tab 栏 | ConnectionChip 横条 |
|---|---|---|
| 活跃绿点 ● | ✅ | ✅ |
| 标题 / label | ✅ | ✅ |
| 关闭 × | ✅ | ✅ |
| `[SSH]` 类型徽章 | ❌ | ✅ |
| `▾` 折叠（保留连接回首页） | ❌ | ✅ |

横条独有的 `▾` 折叠功能在代码注释里明确写"等 M3c+ 默认页加活跃连接入口才闭环"——
当前点 `▾` 后默认页**没有任何入口**能重新进入仍在跑的 connection（默认页只有 host
列表，点 host 是新开连接），等于一个"丢失入口"按钮，体验比 `×` 关闭更糟。

→ 综合"信息重复 + 未闭环按钮"两点，整条横条价值低于占用的 36px 屏幕高度，删除。

---

## 2. 改动范围

### 2.1 删除

- **文件**：`crates/aish-app/src/views/connection_chip.rs` 整个删除
- **mod 引用**：`crates/aish-app/src/views/mod.rs` 删 `pub mod connection_chip;` 和
  对 `ConnectionChipView` 的 re-export

### 2.2 修改 RootView（`crates/aish-app/src/app.rs`）

删除：
- `ConnectionChipView` 的 `Entity::new` 实例化
- `RootView::render` 里 `tab.content == TabContent::Connection(_)` 分支下的
  `ConnectionChipView` 渲染节点

效果：连接 tab 的 body 直接由 `TerminalView` 占满，没有上方横条。

### 2.3 修改 Tab 栏（`crates/aish-app/src/views/tab_bar.rs`）

在 connection tab 的元素链中插入 SSH 胶囊：

```
当前：[prefix(●/空)] [title]                  [close ×]
新：  [prefix(●/空)] [title] [SSH chip(仅连接tab)] [close ×]
```

胶囊样式直接复用 ConnectionChip 上的实现（迁移过去）：
```rust
let ssh_chip = div()
    .px_2p5().py_0p5()
    .text_size(theme::text_xs())
    .text_color(rgb(theme::ACCENT_BLUE))
    .bg(rgb(theme::CHIP_BLUE_BG))
    .rounded_full()
    .child("SSH");
```

判定条件：`matches!(t.content, TabContent::Connection(_))`（已有 `is_connection`
变量复用即可）。默认页 tab 不渲染胶囊。

### 2.4 保留不动

- `AppState::replace_current_tab` —— 该方法被 `default_page.rs:46` 用于"点 host
  卡片 → 当前 tab 转成 connection"，删 connection_chip 后仍是活跃路径，**无需
  任何改动**（包括注释）
- `connection_chip` 之外的 theme tokens / 其它 view 全部不动

---

## 3. ASCII Mockup

**改造前**：
```
┌─[teste #3 ×]─[+]──────────────────────────┐  ← TabBar 40px
├────────────────────────────────────────────┤
│ ● teste #3  [SSH]              ▾   ×       │  ← ConnectionChip 36px
├────────────────────────────────────────────┤
│ (terminal)                                 │
│                                            │
└────────────────────────────────────────────┘
```

**改造后**：
```
┌─[● teste #3 [SSH] ×]─[+]──────────────────┐  ← TabBar 40px（含 SSH 胶囊）
├────────────────────────────────────────────┤
│ (terminal)                                 │  ← 多 36px 终端高度
│                                            │
└────────────────────────────────────────────┘
```

---

## 4. 已知风险

| 风险 | 应对 |
|---|---|
| Tab 宽度膨胀（SSH 胶囊 ~36-40px） | 接受。当前 tab 无 max-width，多个 connection 时挤压在所难免，留作未来项 |
| Tab 高度 40px 放不下胶囊 | 胶囊 `text_xs + py_0p5` 实测 18-20px，竖向居中放得下 |
| `▾` 折叠功能丢失，用户找不到入口 | 短期接受。actor 不会泄漏（`×` 关闭仍正常 disconnect + remove_connection）；M3c+ 加"活跃连接区"后恢复语义 |

---

## 5. 验证

- `cargo run` 启动 → 连一个 host → 确认终端区上方无横条 → tab 上有 `● label [SSH] ×`
- 多个 connection tab 横排 → 视觉上能容纳，不至于完全挤压不可读
- `cargo +nightly fmt --all` / `cargo +nightly clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` 均通过

无新增自动化测试（纯 UI 删除 + 元素重排，逻辑无变化）。

---

## 6. 实施拆解

详见 plan。预计 1 个 commit（删 chip 文件 + mod + app.rs + tab_bar.rs 一起，逻辑
不可拆分；分多 commit 反而中间状态会编译失败）。
