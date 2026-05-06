# aish M2b1 — 终端渲染 + PTY resize 设计

- **日期**: 2026-05-06
- **状态**: Design (approved by user, ready for implementation planning)
- **里程碑**: M2b1（M2 第二阶段第一部分；M2 整体拆 M2a / M2b1 / M2b2 / M2c）
- **前置**: M2a 已完成（commit `d717956` 之后；真 SSH 接通 + 键盘 echo + 字符流 utf8_lossy 显示）
- **作者**: larry
- **预计周期**: ~2 周

---

## 1. M2b 拆分背景

按 brainstorming scope check，M2b 演进路径太大（alacritty_terminal 集成 / 自绘 grid / 字体度量 / 颜色 / 光标 / resize / 完整键盘编码 / 滚动 UI = 8 项），单一 spec 装不下。拆为：

- **M2b1（本 spec）**：1-6 + mouse 选中复制 — 终端渲染基础 + 视觉一致 + 复制能用
- **M2b2**：完整 xterm 键盘编码（方向键 / Home/End / F1-12 / Alt+ 等）+ scrollback 滚动 UI

每个独立可 demo。

---

## 2. M2b1 范围与目标

### Phase 1（M2b1，本 spec 范围）

| 模块 | 范围 |
|---|---|
| `alacritty_terminal::Term` 集成 | 替换 `pane_logs: Vec<String>` 为 `pane_terminals: HashMap<HostId, Term>`；ssh_actor 输出 bytes 经 GPUI 端 feed 到 Term |
| 字体加载 | `bundle JetBrains Mono Nerd Font Regular` 进 `crates/aish-app/assets/`，启动时通过 GPUI text_system 加载 |
| GPUI 自绘字符 grid | HostPaneView 替换为 TerminalView：逐字符绘 + 颜色 + 光标，参考 Zed `crates/terminal_view` |
| 颜色映射 | alacritty Term 输出的 cell.fg/bg → GPUI rgb；支持 16 色 palette + 256 color + truecolor |
| 光标渲染 | 方块光标 + 闪烁 600ms + 失焦时空心 |
| PTY resize | GPUI window resize → 字体度量 → 算 cols/rows → debounce 100ms → SessionCommand::Resize → chan.window_change |
| Mouse 选中 + 复制 | mouse drag 标记 alacritty Term 的 selection；Ctrl+Shift+C 取选中文本 → GPUI 系统剪贴板 |
| 方向键 / Home / End / PageUp / PageDown 的 normal mode 编码 | 在 `ssh_actor::encode_key` 表加 normal mode (CSI) 序列：↑→↓← → `\x1b[A/B/C/D`；Home/End → `\x1b[H/F`；PageUp/PageDown → `\x1b[5~/6~`。**仅 normal mode**，DECCKM 应用键模式切换留 M2b2 |

### 不在范围内（明确边界）

- **DECCKM 应用键模式切换**（vim/less 在某些 mode 下方向键编码会从 `\x1b[A` 切到 `\x1bOA`） → M2b2
- **F1-F12 / Alt+ / Shift+Tab 等扩展键** → M2b2
- **scrollback 滚动 UI**（鼠标滚轮 / 滚动条）→ M2b2（buffer 已存在，只差 UI）
- **字体大小 / 主题用户配置** → M2c+
- **链接识别**（mailto / http URL 高亮 + click 打开） → 远期
- **图像内联**（sixel / iTerm2 inline image） → Phase 3
- **Bell / 标题更新等 OSC 序列** → Phase 2
- **ConPTY / Windows 特定终端 quirks** → 必要时再处理（russh PTY 是 *nix-flavor，Windows server 端少见）
- **hosts.json 持久化 / 添加 host UI** → M2c

---

## 3. 关键技术决策（M2b1-ADR）

| ID | 决策 | 备选 | 理由 |
|---|---|---|---|
| M2b1-ADR-1 | **拆 M2b1 + M2b2 而非一次到位** | 一次 3-4 周 / 三拆 | 单 milestone 3-4 周中间无 checkpoint，bug 累积风险大；M2b1 是"看起来像真终端"最小完整体；三拆的 M2b1（黑白文字）demo 价值低 |
| M2b1-ADR-2 | **bundle JetBrains Mono Nerd Font Regular** | 系统默认字体 / 不带 Nerd Font / 用户配置 | 跨平台视觉一致；支持 Nerd Font 图标（modern shell prompt 标配如 starship / powerlevel10k）；400KB 一次性成本可接受 |
| M2b1-ADR-3 | **M2b1 含 mouse 选中 + Ctrl+Shift+C 复制** | 留 M2b2 / 更后 | 没复制能力的"终端"不能 daily 用，demo 价值打折；alacritty Term 自带 selection state，工作量 +3-4 天可控 |
| M2b1-ADR-4 | **scrollback buffer 10,000 行** | 5,000 / 50,000 / 无限 | 现代终端默认；M2b1 不做滚动 UI 但 buffer 存好留 M2b2 用；10K 行内存占用可控（每行 ~500 bytes × 10K = 5MB / host） |
| M2b1-ADR-5 | **直接用 alacritty_terminal 默认颜色主题** | 自定义主题 / 用户配置 | YAGNI；alacritty 默认是经过设计的（黑底 #1d1f21 + xterm 16 色 + 256 color + truecolor），M2c+ 才做主题 |
| M2b1-ADR-6 | **方块光标 + 闪烁 600ms + 失焦时空心** | 竖线 / 下划线 / 不闪烁 | macOS Terminal / iTerm2 default；闪烁周期 600ms 是行业标准，1Hz 太慢，500ms 偏快 |
| M2b1-ADR-7 | **PTY resize debounce 100ms** | 0ms 立即 / 200ms / 500ms | 拖窗口过程中每帧 resize 会风暴 chan.window_change（每秒可能 60+ 次）；100ms 是惯例（VS Code 终端 / iTerm2 都是这量级） |
| M2b1-ADR-8 | **选中高亮 #3a3a8a 80% opacity** | 反色 / 浅蓝 / 黄色 | 与深色背景 (#1d1f21) contrast 够；macOS / VS Code 选中色风格 |
| M2b1-ADR-9 | **字号 14pt hardcoded** | 12 / 16 / 用户配置 | YAGNI；14pt 是现代终端默认（VS Code / Hyper / Warp 默认）；M2c+ 才做配置 |
| M2b1-ADR-10 | **TerminalView 渲染参考 Zed `crates/terminal_view`** | 自研 / 用 zed 的 fork | Zed 已经做完字体度量 + 自绘 grid + 选中 + 光标的所有难活，参考其代码模式而非全新自研 |
| M2b1-ADR-11 | **bytes feed 在 GPUI 端而非 ssh_actor 端** | ssh_actor 持有 Term | alacritty_terminal::Term 与 GPUI Model 紧绑（渲染要 read Term 状态），ssh_actor 持有 Term 反而要回传引用；让 GPUI 端持有 Term，ssh_actor 仅 emit raw bytes 流 |
| M2b1-ADR-12 | **方向键 / Home/End / PageUp/Down 的 normal mode 编码加入 M2b1** | 留 M2b2 一并做完整键盘 | demo 价值差别大（bash 历史 / cd Tab 补全 / vim insert 方向键导航 — daily 必备）；工作量小（encode_key 表扩展几行 + 测试 +0.5-1 天）；M2b2 仍有 DECCKM / F1-12 / Alt+ / IME / scrollback UI 等重活，不会被掏空 |

---

## 4. 系统架构

### 数据流

```
┌────────────────────────────────────────────────────────────────┐
│                  GPUI Application Process                       │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │              Views (M2a 复用 + 新 TerminalView)            │ │
│  │  HostListView (左栏，不变)                                │ │
│  │  TerminalView (主区，替换 HostPaneView)                   │ │
│  │   ├─ TextSystem 字体度量                                  │ │
│  │   ├─ GridRenderer 自绘 cell                               │ │
│  │   ├─ CursorRenderer 光标 + 闪烁                           │ │
│  │   ├─ SelectionRenderer mouse drag → Term selection        │ │
│  │   └─ KeyHandler (M2a encode_key 兼容版，M2b2 重写)        │ │
│  └────────────┬─────────────────────────────────────────────┘ │
│               │ subscribe + key/mouse event                    │
│  ┌────────────▼─────────────────────────────────────────────┐ │
│  │            AppState (Model<T>)                            │ │
│  │  hosts / selected / sessions  (M2a 不变)                  │ │
│  │  pane_terminals: HashMap<HostId, alacritty_terminal::Term>│ │ ← 新
│  │  pane_dimensions: HashMap<HostId, (cols, rows)>           │ │ ← 新
│  └────────────┬─────────────────────────────────────────────┘ │
│               │ cx.spawn 接收 SshEvent → feed bytes 到 Term     │
│               │ window resize → debounce → SessionCommand::Resize│
│  ┌────────────▼─────────────────────────────────────────────┐ │
│  │           Bridge (tokio runtime, M2a 不变)                │ │
│  │   per-host actor task (own SshSession + PTY)              │ │
│  │   + 新增 SessionCommand::Resize 处理：chan.window_change   │ │
│  └─────────────────┬────────────────────────────────────────┘ │
└────────────────────┼───────────────────────────────────────────┘
                     │ TCP (russh)
                     ▼
               ┌─────────────┐
               │ Remote Host │
               └─────────────┘
```

### 关键消息类型变更

```rust
// SessionCommand 加一个 variant
pub enum SessionCommand {
    SendBytes(Vec<u8>),
    Resize { cols: u16, rows: u16 },  // ← 新增
    Disconnect,
}

// SshEvent 不变（仍 emit raw bytes）
pub enum SshEvent {
    Connected { host: HostId },
    PaneOutput { host: HostId, bytes: Vec<u8> },
    Disconnected { host: HostId, reason: DisconnectReason },
    Error { host: HostId, kind: SshErrorKind, msg: String },
}
```

### Term 的 Send 问题

`alacritty_terminal::Term<L>` 中 `L: EventListener` 是关联类型。GPUI Model 要 Send + 'static。alacritty_terminal 提供 `VoidListener`（只丢弃事件），它是 ZST + Send，可以满足。

但 Term 的某些方法会修改内部状态（如 `feed` / `resize` / 鼠标 selection），需要 `&mut Term`。GPUI Model<T>::update 的 closure 已经给 `&mut T`，匹配。

### TerminalView 渲染细节

参考 Zed `crates/terminal_view/src/terminal_element.rs`：

1. **每帧渲染流程**：
   - `let term = state.read(cx).pane_terminals.get(&host)`
   - `let grid = term.grid()`
   - 遍历 `grid.display_iter()` 得每个 cell（pos / char / fg / bg / flags）
   - 计算 cell 在 GPUI 坐标的 (x, y) = (col × cell_width, row × cell_height)
   - 用 GPUI 的 paint API 绘字符 + 背景色 + 选中高亮
   - 单独绘光标（在 grid.cursor.point 位置 + 闪烁 alpha）

2. **字体度量**：
   - `cx.text_system().font_metrics(...)` 拿 monospace 字符的 advance_width / line_height
   - cell_width = advance_width，cell_height = line_height
   - 两者随字号变化（字号 hardcoded 14pt）

3. **GPUI 低层 paint API**：
   - 不是每个 cell 一个 div（性能不行：80 × 24 = 1920 divs/frame）
   - 用 `Window::paint_layer` + `text_system.shape_line` 批量绘
   - Zed 自己也是这么做的（看 terminal_element.rs `paint_layout` 函数）

### Selection 数据流

```
mouse_down → TerminalView::on_mouse_down
  → 算字符 grid 坐标 (col, row) = (pixel_pos - offset) / cell_size
  → state.update(cx, |state, cx| {
       term.selection = Some(Selection::new(SelectionType::Simple, ..));
       cx.notify();
     })

mouse_move (drag) → 更新 selection.end → notify
mouse_up → selection 锁定，保留高亮

Ctrl+Shift+C 按下 → KeyHandler 触发：
  let selected_text = term.selection_to_string();
  cx.write_to_clipboard(selected_text);
```

`alacritty_terminal::Term::selection_to_string()` 已经处理了 wrap line / 多行选中等边界，复用即可。

### Resize 数据流

```
GPUI Window::on_resize → TerminalView 收到新主区 px size
  → cell_width / cell_height 已知
  → new_cols = floor(width / cell_width)
  → new_rows = floor(height / cell_height)
  → debounce_timer.cancel + spawn 100ms timer
  → 100ms 内无新 resize → trigger:
      state.update(cx, |state, cx| {
        let term = state.pane_terminals.get_mut(&host)?;
        term.resize(new_cols, new_rows);
        let sender = state.sessions.get(&host)?;
        bridge.spawn(async move {
          sender.send(SessionCommand::Resize { cols, rows }).await;
        });
        cx.notify();
      });

ssh_actor 收到 SessionCommand::Resize:
  chan.window_change(cols as u32, rows as u32, 0, 0).await
```

debounce 用 GPUI executor 自带的 timer / cx.spawn + tokio::time::sleep 或 simple Instant::elapsed 方式。

---

## 5. File Structure（M2b1 完成时）

```
aish/
├── Cargo.toml                       # 加 alacritty_terminal workspace dep
├── crates/
│   ├── aish-app/
│   │   ├── Cargo.toml               # 加 alacritty_terminal + arrayvec 等
│   │   ├── assets/                  # 新建目录
│   │   │   └── JetBrainsMonoNerdFont-Regular.ttf  # 新增（~400KB）
│   │   └── src/
│   │       ├── main.rs              # 修改：mod terminal
│   │       ├── app.rs               # 修改：PaneOutput 处理 — feed bytes 到 Term
│   │       ├── state.rs             # 修改：pane_logs → pane_terminals + pane_dimensions
│   │       ├── bridge.rs            # 不变
│   │       ├── ssh_actor.rs         # 修改：SessionCommand::Resize 处理 + chan.window_change
│   │       ├── fixtures.rs          # 不变
│   │       ├── terminal/            # 新建模块
│   │       │   ├── mod.rs
│   │       │   ├── font.rs          # 加载 bundled font + 字体度量
│   │       │   ├── grid_renderer.rs # 自绘 cell + 颜色映射
│   │       │   ├── cursor.rs        # 光标渲染 + 闪烁
│   │       │   ├── selection.rs     # mouse drag → Term selection
│   │       │   └── colors.rs        # alacritty 颜色 → GPUI rgb 映射
│   │       └── views/
│   │           ├── mod.rs           # reexport TerminalView
│   │           ├── host_list.rs     # 不变
│   │           ├── host_pane.rs     # ❌ 删除（被 TerminalView 替代）
│   │           └── terminal_view.rs # 新：替换 host_pane.rs
│   └── （aish-types / ssh / tmux / sftp / secrets 不变）
```

新增 7 个文件 + 1 个字体资源
删除 1 个文件（host_pane.rs）
修改 5 个文件（main.rs / app.rs / state.rs / ssh_actor.rs / views/mod.rs）
修改 2 个 Cargo.toml

---

## 6. 核心数据流（用户输入 / 远端输出 / resize 完整循环）

### 6.1 远端输出 → 屏幕显示

```
远端 echo "hello" 后 PTY 写入 ANSI bytes "hello\r\n"
  ↓ russh channel.wait()
ssh_actor:
  ChannelMsg::Data { data } → SshEvent::PaneOutput { host, bytes }
  ↓ mpsc channel
GPUI cx.spawn loop:
  state.update(cx, |state, cx| {
    let term = state.pane_terminals.entry(host).or_insert_with(|| make_term(...));
    for byte in &bytes {
      term.parser.feed(*byte, &mut term.handler);
      // alacritty Term 内部 vt100 状态机更新 grid
    }
    cx.notify();
  });
  ↓ subscribe
TerminalView::render:
  let term = state.read(cx).pane_terminals.get(&host)?;
  for cell in term.grid().display_iter() {
    paint cell at (col*cw, row*ch)
  }
  paint cursor at (term.grid.cursor.point.col * cw, ...)
```

### 6.2 用户键盘输入

M2b1 扩展 `ssh_actor::encode_key`，**新增** normal mode CSI 编码：

| Key | Bytes (M2b1) |
|---|---|
| ArrowUp | `\x1b[A` |
| ArrowDown | `\x1b[B` |
| ArrowRight | `\x1b[C` |
| ArrowLeft | `\x1b[D` |
| Home | `\x1b[H` |
| End | `\x1b[F` |
| PageUp | `\x1b[5~` |
| PageDown | `\x1b[6~` |
| Delete | `\x1b[3~` |
| Insert | `\x1b[2~` |

GPUI 的 `event.keystroke.key` 字符串名按 Zed editor / GPUI examples 的实际 key 名为准（可能是 `up` / `arrowup` / `Up` 之一，按 KeyDownEvent 实际值调整）。

M2a 已有的（普通字符 / Enter / Backspace / Tab / Esc / Ctrl+A-Z）保留不变。

> **DECCKM 应用键模式切换留 M2b2**：vim/less 等切到 application cursor key mode 时方向键编码应是 `\x1bO{A,B,C,D}`，M2b1 不做此切换，统一发 normal mode (CSI)。后果：bash / vim insert mode / cd Tab 补全 — work；某些 TUI 在 app mode 下方向键失效是预期。

流程同 M2a：

```
key_down → encode_key(key, ctrl, alt) → bytes
  → bridge.spawn(async move {
       sessions[host].send(SessionCommand::SendBytes(bytes)).await
     })
  → ssh_actor: chan.data(bytes).await
  → 远端 PTY 收到 → 通常会 echo 回来 → 走路径 6.1
```

特殊键 **Ctrl+Shift+C**（不发到远端，本地复制）：

```
KeyDownEvent { key: "c", modifiers: { control: true, shift: true } }
  → 不调 encode_key
  → 取 term.selection_to_string()
  → cx.write_to_clipboard(text)
  → 显示 toast "Copied N chars"（可选）
```

### 6.3 Window resize → PTY resize

```
GPUI Window resize event
  → TerminalView 收到新 main_pane_pixel_size (w, h)
  → new_cols = (w / cell_width).floor() as u16
  → new_rows = (h / cell_height).floor() as u16
  → 取消上次的 debounce_timer
  → cx.spawn(async move {
       tokio::time::sleep(100ms).await;
       state.update(...);
     })
  → 100ms 内无新 resize 才执行：
    state.update(cx, |state, cx| {
      let host = state.selected?;
      let term = state.pane_terminals.get_mut(&host)?;
      term.resize(TermSize::new(new_cols, new_rows));
      state.pane_dimensions.insert(host, (new_cols, new_rows));
      let sender = state.sessions.get(&host)?.clone();
      cx.notify();
      bridge.spawn(async move {
        let _ = sender.send(SessionCommand::Resize {
          cols: new_cols, rows: new_rows,
        }).await;
      });
    });
  → ssh_actor 收 Resize → chan.window_change(cols, rows, 0, 0).await
  → 远端 sshd 给 shell 发 SIGWINCH
  → 远端 stty / tput cols / tput lines 看到新值
```

切换 host 时：每个 host 维护自己的 (cols, rows)。如果 main pane size 不同（实际上窗口只有一个，size 一致），切回 host 时 term.resize 用当前 (cols, rows) 重新对齐——alacritty Term::resize 是 idempotent 的。

### 6.4 Mouse 选中 → 复制

```
mouse_down at (px, py) on TerminalView:
  col = ((px - pad_x) / cell_width).floor() as i32
  row = ((py - pad_y) / cell_height).floor() as i32
  state.update(cx, |state, cx| {
    let term = state.pane_terminals.get_mut(&host)?;
    term.selection = Some(Selection::new(
      SelectionType::Simple,
      Point::new(Line(row), Column(col)),
      Side::Left,
    ));
    cx.notify();
  });

mouse_move (button held):
  state.update(cx, |state, cx| {
    let term = state.pane_terminals.get_mut(&host)?;
    if let Some(ref mut sel) = term.selection {
      sel.update(Point::new(Line(row), Column(col)), Side::Right);
    }
    cx.notify();
  });

mouse_up: selection 锁定（不清掉，下次点击或 ESC 才清）

KeyDownEvent { key: "c", ctrl: true, shift: true }:
  let term = state.read(cx).pane_terminals.get(&host)?;
  let text = term.selection_to_string()?;
  cx.write_to_clipboard(text);
```

---

## 7. 错误处理与边界

| 场景 | 处理 |
|---|---|
| **字体加载失败** | startup panic + 写崩溃日志（字体是 critical 资产） |
| **GPUI text_system 不可用** | 同上 panic |
| **alacritty_terminal Term::resize 失败** | resize 返回 Result；M2a 风格 — fail silently（log warn）+ pane_dimensions 不更新 |
| **selection_to_string 返回空** | Ctrl+Shift+C 不写剪贴板（避免清空已有剪贴板内容） |
| **Term feed bytes 异常** | alacritty 内部容错；如果真出问题 panic（vt100 解析器极少 panic） |
| **窗口最小化 resize 到 0** | clamp cols/rows 最小值 1，避免 SIGWINCH 给远端无效值 |
| **Term 的 grid 为空（host 刚创建未 connect）** | 渲染空白主区（同 M2a 的"等待 Connecting"逻辑） |
| **clipboard 写入失败** | log warn + 不影响 UI |

---

## 8. 测试策略

| 模块 | 测试方式 |
|---|---|
| `terminal/colors.rs` | 单元测试 alacritty Color → GPUI rgb 映射（含 16/256/truecolor 分支） |
| `terminal/font.rs` | 单元测试字体加载成功 + 字体 metric 合理（non-zero advance_width）|
| `terminal/grid_renderer.rs` | 难单元测（依赖 GPUI paint）；手动视觉验证 |
| `terminal/cursor.rs` | 单元测试光标位置计算（grid → pixel）|
| `terminal/selection.rs` | 单元测试 mouse pixel → grid coords 映射 + Term selection state mutation |
| `app::state` | 单元测试 pane_terminals + pane_dimensions HashMap 增删 |
| `ssh_actor` | 单元测试 SessionCommand::Resize 路径（mock chan.window_change） |
| 端到端 demo | **user 手动验证 demo 11 项**（见 Section 9） |

CI 仍只跑 build / test / fmt / clippy；GUI 自动化测试 ROI 太低（与 spec 一致）。

---

## 9. M2b1 完成验证（demo 标准）

设环境变量后 `cargo run -p aish-app`，按以下手动核对：

1. ✅ 启动后窗口含 JetBrains Mono Nerd Font 字体（与 M2a 系统字体显示有视觉差异）
2. ✅ 点 my-vps 连接 → shell prompt **不再有 `[01;34m` 等乱码**，颜色正确（如目录蓝色 / 可执行文件绿色 / symlink 青色）
3. ✅ 跑 `ls --color` → 看到带颜色的目录列表（与 M2a 对比，从乱码到正确显示是核心 demo 价值）
4. ✅ 跑 `vim /tmp/test.txt` → 看到完整 vim 编辑界面（行号、状态栏、`-- INSERT --` 提示等）；按 `i` 进入编辑模式 → 输入字符 → Esc → `:wq` 保存退出
5. ✅ 跑 `top` → 看到刷新表格、CPU/MEM 颜色 bar；按 `q` 退出
6. ✅ 光标可见：方块形 + 闪烁（约 600ms 周期）；点窗口外失焦后变空心方块
7. ✅ Mouse drag 选中一段输出 → 选中区域有半透明蓝色高亮
8. ✅ Ctrl+Shift+C → 选中文本进剪贴板；在外部应用 (Notepad / 浏览器地址栏) Ctrl+V → 看到原文
9. ✅ 拖动主窗口大小 → 主区跟随 → 远端 `tput cols && tput lines` 输出新值（远端 SIGWINCH 收到）
10. ✅ Ctrl+C 中断 `top` 类长跑命令
11. ✅ bash 中按 ↑ → 看到上一条命令；按 ↓ → 看到下一条；连续 ↑ 翻历史
12. ✅ 长命令行（超过窗口宽度）按 Home → 光标到行首；End → 行尾
13. ✅ `cargo test --workspace` 全绿（应增加 ~10-15 个 unit test，total ~50 passed）
14. ✅ `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` 全绿
15. ✅ CI 三平台 test job 全绿

---

## 10. M2b1 → M2b2 演进路径

M2b2 接的事：

| M2b1 现状 | → M2b2 |
|---|---|
| `ssh_actor::encode_key` 含 normal mode 方向键 / Home/End / PageUp/Down 等 | 加 DECCKM 应用键模式判断（读 `term.mode().contains(TermMode::APP_CURSOR)` 切 `\x1bO{A,B,C,D}`） |
| 无 F1-F12 / Alt+ / Shift+Tab 支持 | 加完整支持（含 modifier 组合编码）|
| 无 IME 输入法（中文等） | 接 GPUI IME composition 事件 |
| Term 有 10K 行 scrollback buffer 但不能滚动 | 加滚轮 / Page Up/Down 滚动 viewport / 滚动条 UI |
| 选中只能在当前 visible viewport | 选中可跨 scrollback |
| 光标只渲染当前 viewport | scrollback 时显示 viewport 滚动指示 |

M2b1 留下的 onramp：
- TerminalView 已有完整渲染框架 + selection；M2b2 主要扩展 KeyHandler + 加 ScrollHandler
- alacritty Term 的 scrollback API 已经内置（grid().display_offset / scroll_display），M2b2 接渲染层即可

---

## 11. 已知风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| **GPUI 自绘 grid 性能不达标** | 大尺寸（200+ cols）或快速刷新（top）卡顿 | 参考 Zed terminal_element 的 paint_layer 批量绘策略；如果仍卡，alacritty Term 可降帧（每 16ms 才 commit 一次 cx.notify） |
| **alacritty_terminal 与 GPUI 字体度量不一致** | grid cell 与渲染像素错位（字符之间有缝） | M2b1 用 GPUI 自己的 metric（cw/ch），完全 ignore alacritty 的 SizeInfo 内部缩放；alacritty Term 只用作 vt100 解析器 + selection state |
| **Term::feed 在 GPUI Model.update 中可能 hot path 慢** | 大量输出（cat 大文件）卡 UI | M2b1 接受；M2b2 优化（攒一批 bytes 再 feed） |
| **JetBrains Mono Nerd Font 文件体积** | 资源 ~400KB 进 binary | 接受；可后续 strip 不用的 glyphs（但 Nerd Font 主要价值就是 glyph 多） |
| **Windows Terminal 输入兼容** | Windows console quirks（如 Ctrl+Space → \0 vs ^@） | M2a encode_key 已经避开复杂键；M2b1 继续用 |
| **Linux GPUI 仍有稳定性问题** | M2b1 demo 在 Linux 可能渲染异常 | 与主 spec 一致：Linux 接受 beta，主推 macOS / Windows |
| **demo 验证依赖 user 真 VPS** | implementer subagent 跑不了 | 由 user 在 Task 8 手动验证；subagent 只验证 build + 启动无 panic |

---

## 12. 不在本 spec 范围内（边界提醒）

- DECCKM 应用键模式切换（vim/less 在 app mode 下方向键 `\x1bO{A,B,C,D}`） → M2b2
- F1-F12 / Alt+ / Shift+Tab / Modifier 组合键编码 → M2b2
- IME 输入法（中文 / 日文 / 韩文） → M2b2
- Scrollback 滚动 UI（鼠标滚轮 / 滚动条 / Page Up/Down 翻 viewport） → M2b2
- 字体大小 / 主题用户配置 → M2c+
- 链接识别（mailto / http URL 高亮 + click 打开） → 远期
- 图像内联（sixel / iTerm2 inline image） → Phase 3
- Bell / 标题更新等 OSC 序列 → Phase 2
- ConPTY / Windows 特定终端 quirks → 必要时再处理
- hosts.json 持久化 / 添加 host UI → M2c
- 多 tab / 分屏 → Phase 2
