# aish UI 美化（M3d-ui-polish）— Design Spec

**Goal:** 把当前略粗糙的 GPUI 界面调到接近用户给的移动端参考图（图 9）的视觉
质量，但适配桌面横屏。覆盖 4 个区域：默认页、tab 栏、connection chip、host 表单。

**Non-Goal:** 不做整套设计系统抽象（CSS variables 等），不做主题切换（暗色/亮色），
不做动效（GPUI 0.x 动效 API 仍在演进，能稳定的就 hover 透明度过渡）。

**用户决策**：
- **风格基线**：移动端 iOS 风（参考图 9 截图：纯黑底 + 圆角卡片 + 蓝色/绿色 chip + 一致字号层次）
- **范围**：默认页 + tab 栏 + connection chip + host 表单 modal 全部 4 个

---

## 1. 当前状态盘点

| 区域 | 当前实现 | 问题点 |
|---|---|---|
| RootView 背景 | `0x1d1f21` | 不够深，跟卡片区分弱 |
| Tab 栏 | `0x1a1a1a` 底 + 直角 + `0x333333` 右 border | 直角扎眼 / 选中态高亮不明显 / `+` 按钮无视觉权重 |
| Default page | `0x141414` 底 + `0x1e1e1e` 卡片 + `0x2a2a2a` border + rounded_lg | 卡片间距过密 / 文字层次单薄 / 按钮位置默认隐藏不易发现 |
| Connection chip | 32px 高 / 一行平铺 | 颜色和 terminal 没分层 / 按钮纯文字符号 |
| Host form modal | 圆角 + 输入框带边框 / 按钮 | 输入框样式平淡 / segmented control 缺失 / 字段间距密 |
| Session picker | 半透明遮罩 + 居中 / rounded_lg | 已经能用，但和默认页不一致 |
| 字号层次 | 11/12/13/14 散点 | 没规则，需统一 |
| 图标 | emoji `✏ 🗑 × ↻ ⊖ ●` 混用 | 风格不一，部分 emoji 在某些字体下变形 |

---

## 2. 视觉规范

### 2.1 配色（基于参考图 9）

```
/* 背景层（深 → 浅） */
--bg-base       #0a0a0c    （RootView 全局底，比当前 #1d1f21 深一档）
--bg-elevated   #15161a    （卡片 / chip 填充）
--bg-hover      #1f2128    （hover 高亮）
--bg-selected   #26282f    （选中态 / 按下态）

/* 边框 */
--border-subtle #1f2128    （卡片默认边框，几乎贴 elevated 不刺眼）
--border-strong #2f323a    （hover / focus 边框）

/* 文本 */
--text-primary   #eaeaee   （host label / 标题）
--text-secondary #888a93   （host:port、user · time 副信息）
--text-muted     #5b5d66   （placeholder / 占位 / disabled）

/* 强调色 */
--accent-blue   #4a9eff    （SSH chip / 输入 focus / 选中 tab 边）
--accent-green  #4ec9b0    （活跃指示 ● / 成功 chip）
--accent-red    #ff6b6b    （删除 / × hover / 错误）
--accent-yellow #f5c242    （连接中 / 警告，未启用）

/* 蓝色 chip 背景（基于 accent-blue 调出半透明深色调） */
--chip-blue-bg  #1f3a5c    （SSH chip 底，已用值，保留）
--chip-green-bg #16382f    （活跃 chip 底，新增）
```

### 2.2 圆角

| 元素 | 半径 | GPUI |
|---|---|---|
| 卡片（host card / picker dialog） | 12px | `rounded_xl` 等价 |
| chip / 按钮 / 输入框 | 6px | `rounded_md` |
| Tab 顶部 | 8px 上半圆 | `rounded_t_lg`，下边平 |
| Connection chip 整条 | 0（贴在 terminal 上方，不圆） | — |

### 2.3 字号 + Weight

只用三档：

```
--text-xl    16px / 600   （modal 标题）
--text-lg    14px / 500   （host label，tab 标题）
--text-sm    12px / 400   （副信息、按钮文字、chip）
--text-xs    11px / 400   （时间戳、占位说明）
```

GPUI 的 font_weight 在当前 build 是否稳定有 weight 选项需要 trial；如果不稳定，
fallback 到只调 size，weight 用颜色对比代偿（primary 亮 / secondary 灰）。

### 2.4 间距

| 场景 | 值 |
|---|---|
| 卡片内部 padding | `px_4 py_3`（16/12px）|
| 卡片之间 gap | `gap_3`（12px）|
| section 标题与列表 | `pt_5 pb_3`（20/12px）|
| modal padding | `px_6 py_5`（24/20px）|
| 输入框 padding | `px_3 py_2`（12/8px）|
| chip padding | `px_2 py_0p5`（8/2px，已有，保留）|

### 2.5 图标

统一规则：
- **互动按钮** 用 SVG 字符或 Unicode 几何符号，不用 emoji（emoji 在不同字体下渲染差异大）
  - 关闭：`×`（U+00D7，已用）
  - 添加：`+`（已用）
  - 折叠：`▾`（替代当前 `⊖`）
  - 编辑：`✎`（U+270E，替代 emoji `✏`）
  - 删除：`🗑` 替换为彩色 `×` 或保留 emoji（trial 决定）
- **状态指示** 用纯色实心圆点 `●`（已用）

### 2.6 微交互

- 卡片 hover：`bg-elevated → bg-hover`，border 同步变 strong
- 按钮 hover：text-secondary → text-primary，无背景色
- 选中 tab：底部 2px 蓝色高亮线（`accent-blue`）+ bg-selected
- focus ring：focused input 用 1px `accent-blue` 边框替换 subtle

---

## 3. 各区域改造方案

### 3.1 默认页（views/default_page.rs）

```
┌──────────────────────────────────────────┐
│  已保存的连接          + 添加 host       │  ← header pt_6 px_8 (16px → 24px 加大)
│                                          │
│  ┌────────────────────────────────┐      │  ← gap_3
│  │  腾讯云新加坡开发      [SSH]    │      │
│  │  larry@53.62.x.x:22       ✎ 🗑 │      │  ← icon hover 显示
│  └────────────────────────────────┘      │
│                                          │
│  ┌────────────────────────────────┐      │
│  │  腾讯云新加坡开发      [SSH]    │      │
│  │  ...                            │      │
│  └────────────────────────────────┘      │
└──────────────────────────────────────────┘
```

- 底色 `bg-base` 全屏
- 卡片 `bg-elevated` + `border-subtle` + `rounded_xl` + `px_4 py_3`
- hover: `bg-hover` + `border-strong`
- label 用 `text-lg` primary，`user@host:port` 用 `text-sm` secondary（次行，gap_1）
- SSH chip `chip-blue-bg` + `text-sm accent-blue` + `rounded_md`
- 编辑/删除按钮 hover 出现，不再用 emoji，改 `✎` / `×`

### 3.2 Tab 栏（views/tab_bar.rs）

```
┌──[● 腾讯云 #1   ×]──[ 新连接   ×]──[+]─────────────┐
│ ↑ 选中 tab：bg-selected + 底部 2px 蓝线         │
└──────────────────────────────────────────────────┘
```

- 高度从 36px → 40px（呼吸感）
- 每个 tab 顶部 8px 圆角（`rounded_t_lg`），左右间距 1px gap
- 选中态：`bg-selected` + 底部 `border_b_2` `accent-blue`
- 非选中：`bg-base` + 文字 `text-secondary`
- hover：`bg-hover`
- `+` 按钮独立，hover 显示 `bg-hover` + 文字 primary
- 双击重命名 inline 输入：去掉光标 `|`，改用 1px `accent-blue` 边框

### 3.3 Connection chip（views/connection_chip.rs）

```
┌──────────────────────────────────────────┐
│ ●  腾讯云 #1  [SSH]              ▾  ×    │
└──────────────────────────────────────────┘
```

- 高度 32px → 36px
- 底色用 `bg-elevated`（比 terminal 区更亮一点点，划出层次）
- 下边一条 1px `border-subtle`
- ⊖ 改成 `▾`（折叠语义更明确）
- × hover `accent-red`
- left-padding 加大到 `px_4`

### 3.4 Host 表单 modal（views/host_form.rs）

具体细节先不在 spec 里铺开（host_form 当前文件没读，先看代码再决定）。
原则：

- 整体背景 `bg-elevated` + `rounded_xl` + 居中
- 输入框 `bg-base`（比卡片底深，反差感）+ `rounded_md` + `border-subtle`
- focused：边框 `accent-blue`
- 认证方式 KeyFile/Password 两选 → segmented control（左右各占一半，选中那个 `bg-selected`）
- 私钥已选时显示带钥匙 emoji 的小 chip：`🔑 SHA256:...`（不变）或改 `●` + monospace 文本
- 按钮：取消（secondary 文字）+ 保存（`accent-blue` 实心）

---

## 4. 实施分解

按文件拆 4 个 task（详见 plan）：

1. 抽 `views/theme.rs` — 集中色值 / 字号 / 半径常量，避免散在各文件
2. 默认页改造
3. Tab 栏改造
4. Connection chip 改造
5. Host form 改造

每 task 一个 commit。`theme.rs` 优先，其它 task 依赖它。

---

## 5. 已知风险

| 风险 | 应对 |
|---|---|
| GPUI 0.x 不支持 `font_weight` 或不稳定 | 仅用 size + color 区分层次；weight 留待 GPUI 升级 |
| `box-shadow` API 是否成熟 | 不依赖 shadow，改用 1px subtle border 区分层次 |
| 不同 Windows 字体下 `▾✎×` 渲染差异 | trial 后挑稳定符号；最差回退当前 emoji |
| segmented control 在 GPUI 没现成组件 | 自实现：两个 div 横排，按 `auth_kind` 给选中那个加 `bg-selected` |
| 改完用户不喜欢颜色 | iterate；保持 token 集中在 theme.rs，调一处全局生效 |

---

## 6. 验证

每个 task 完成后用户跑 `cargo run` 看实物，截图反馈。**不写自动化 UI 测试**
（GPUI 单元测试只能验逻辑层，看不到像素差）。

质量门禁：
```
cargo +nightly fmt --all
cargo +nightly clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

每条 task 之后跑一次。
