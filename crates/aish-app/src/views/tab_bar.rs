//! 顶部 Tab 栏：显示所有 tabs，点击切换，× 按钮关闭，末尾 + 按钮新建默认页。
//!
//! 关闭 tab 时若该 tab 引用了 connection，发 `SessionCommand::Disconnect` 并
//! `state.remove_connection`，让 actor 优雅退出。
//!
//! 双击 tab 标题进入 inline 重命名模式：
//! - 字母 / 数字 / 标点 → 追加到 buffer
//! - Backspace → 删一个字符
//! - Enter → commit
//! - Escape → 放弃

use std::sync::Arc;
use std::time::Duration;

use aish_types::TabId;
use gpui::{
    div, point, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent,
    MouseDownEvent, ScrollDelta, ScrollHandle, ScrollWheelEvent, Window,
};

use aish_ui::{ContextMenu, DropdownMenu, IconName, MenuItem, TextInput};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TabContent};

/// drag payload：tab 拖拽 reorder 用。'static + Clone 满足 GPUI on_drag<T>
/// 的 T 约束（GPUI 把 payload 包 Arc<dyn Any> 跨 drag 生命周期传递）。
#[derive(Clone)]
struct DraggedTab {
    source_id: TabId,
}

/// drag 拖影：用户拖 tab 时跟随 cursor 的 ghost preview。GPUI on_drag 要求
/// 构造一个 Entity<W: Render>，本 struct 是最简实现（rounded card + 文字）。
struct TabDragPreview {
    title: String,
}

impl gpui::Render for TabDragPreview {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = aish_ui::theme(cx);
        // 防御性限宽 + ellipsis：长 title（如 tmux pane title 几十字）会让 ghost
        // preview 拉到屏幕宽度。max_w 200 / overflow_hidden / whitespace_nowrap
        // / text_ellipsis 与 TabItem 限宽规则一致，ghost 看起来跟原 tab 同尺寸。
        div()
            .max_w(px(200.0))
            .px(t.spacing.px_3)
            .py(t.spacing.px_2)
            .bg(t.colors.popover)
            .border_1()
            .border_color(t.colors.primary)
            .rounded(t.radius.md)
            .text_size(t.font_size.sm)
            .text_color(t.colors.foreground)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            // M24 elevation-2 — 拖影浮起感，与 drop target 区分
            .shadow(aish_ui::elevation_2(t.kind))
            .child(self.title.clone())
    }
}

pub struct TabBarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
    /// tabs 横向滚动 handle。绑定到 scroll 容器后可读 offset / max_offset 决定
    /// 是否显示左右 < > 箭头，并通过 set_offset 编程式滚动。
    scroll_handle: ScrollHandle,
    /// 当前正在 inline 重命名的 tab。`None` = 无编辑。
    editing_tab: Option<TabId>,
    /// 重命名输入框 entity（long-lived 复用）。editing 时把 tab.title 灌进去
    /// 替换 TabItem.title 槽位渲染；commit / 切 tab 时清 editing。
    /// 用 aish_ui::TextInput 替代之前手糊 div + on_key_down，自动获得 IME /
    /// 中文 / focus / Enter submit / 复制粘贴 全套能力。
    rename_input: Entity<TextInput>,
    /// tab 右键菜单 entity。long-lived 复用，菜单内容每帧根据 menu_tab_id
    /// 重设（DropdownMenu 含 closure 捕获 tab_id）。
    /// tab 右键菜单 entity。RootView 在 root 顶层 mount 这个 entity 让
    /// menu / backdrop 浮在 terminal / session_picker 之上 — tab_bar 自己
    /// mount 时 paint 顺序在下游 view 之前，会被盖掉。
    context_menu: Entity<ContextMenu>,
    /// 当前右键菜单针对的 tab id。`None` = 菜单关闭 / 不渲染 content。
    /// 设置 / 清除时通过 ContextMenu.on_close 同步。
    menu_tab_id: Option<TabId>,
    /// 键盘导航当前选中的菜单项索引。0..MENU_ITEM_COUNT。
    /// 右键打开时重置 0，↑/↓ 调整，Enter 触发 handle_menu_select。
    menu_active_idx: usize,
}

/// tab 右键菜单 item 数量。与 build menu items 数量一致（见 render 内
/// DropdownMenu.items() 长度）。
/// tab 右键菜单项数（重命名 / 折叠到 Home / 关闭 / 关闭其他）。
/// 与 render 内 DropdownMenu.items(...) 顺序绑定。
const TAB_MENU_ITEM_COUNT: usize = 4;

impl TabBarView {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        // ScrollHandle.max_offset 在首次 paint 后才被 GPUI 写入，render 阶段
        // 读到的是上一帧值，首次 mount 时 max_offset = 0 → show_right 永远
        // false 即便 tabs 实际溢出。spawn 一个轻量循环每 200ms 触发 cx.notify
        // 让 render 重跑读取最新 max_offset / offset 状态。
        // 不用 observe 因为 scroll state 不属于任何 Entity；不用 Window
        // on_next_frame 因为 new() 时没有 window 引用。timer 是 GPUI 标准
        // pattern（参考 toast.rs / text_input.rs blink timer）。
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            if this.update(cx, |_this, cx| cx.notify()).is_err() {
                break;
            }
        })
        .detach();

        // 重命名输入框：long-lived entity，三个 callback：
        // - on_submit (Enter)：commit_rename
        // - on_blur (点击 input 外部失焦)：commit_rename（保留改动，符合
        //   macOS / 浏览器 inline edit 体感）
        // - on_cancel (Esc)：cancel_rename（不保留改动，恢复原 title）
        //
        // 用 borderless 模式：input 不自带 h/bg/border/padding，只渲染文字 +
        // cursor + selection。'input 的视觉外壳'交给外层 editing tab box
        // 接管（colors.input bg + primary border）—— 整个 tab 看起来就是一个
        // 完整 input，不是 'tab 里塞了个小 input'。
        let rename_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.borderless(true);
            i
        });
        let weak_submit = cx.weak_entity();
        let weak_blur = cx.weak_entity();
        let weak_cancel = cx.weak_entity();
        rename_input.update(cx, |i, _cx| {
            i.on_submit(move |text, _window, cx| {
                let text = text.to_string();
                if let Some(this) = weak_submit.upgrade() {
                    this.update(cx, move |this, cx| this.commit_rename(text, cx));
                }
            });
            i.on_blur(move |text, _window, cx| {
                let text = text.to_string();
                if let Some(this) = weak_blur.upgrade() {
                    this.update(cx, move |this, cx| this.commit_rename(text, cx));
                }
            });
            i.on_cancel(move |_window, cx| {
                if let Some(this) = weak_cancel.upgrade() {
                    this.update(cx, |this, cx| this.cancel_rename(cx));
                }
            });
        });

        // 右键 context menu：on_close 同步 menu_tab_id = None。
        // 用户点 menu 外 / Esc 关菜单时清状态，下帧 render 不再设 content。
        let context_menu = cx.new(ContextMenu::new);
        let weak_menu_close = cx.weak_entity();
        let weak_menu_key = cx.weak_entity();
        context_menu.update(cx, move |m, _cx| {
            m.on_close(move |_w, cx| {
                if let Some(this) = weak_menu_close.upgrade() {
                    this.update(cx, |this, _cx| {
                        this.menu_tab_id = None;
                    });
                }
            });
            // 键盘导航：↑/↓/Enter
            m.on_key(move |ev, w, cx| {
                if let Some(this) = weak_menu_key.upgrade() {
                    this.update(cx, |this, cx| this.handle_menu_key(ev, w, cx));
                }
            });
        });

        Self {
            state,
            bridge,
            tx,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
            editing_tab: None,
            rename_input,
            context_menu,
            menu_tab_id: None,
            menu_active_idx: 0,
        }
    }

    /// context menu 键盘导航：↑/↓ 移动 active_idx，Enter 触发选中项 action。
    fn handle_menu_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_id) = self.menu_tab_id else {
            return;
        };
        match ev.keystroke.key.as_str() {
            "up" => {
                self.menu_active_idx = if self.menu_active_idx == 0 {
                    TAB_MENU_ITEM_COUNT - 1
                } else {
                    self.menu_active_idx - 1
                };
                cx.notify();
            }
            "down" => {
                self.menu_active_idx = (self.menu_active_idx + 1) % TAB_MENU_ITEM_COUNT;
                cx.notify();
            }
            "enter" => {
                let idx = self.menu_active_idx;
                self.handle_menu_select(tab_id, idx, window, cx);
            }
            _ => {}
        }
    }

    /// 提交重命名（由 TextInput Enter / on_blur 触发）。
    fn commit_rename(&mut self, new_title: String, cx: &mut Context<Self>) {
        let trimmed = new_title.trim();
        let final_title = if trimmed.is_empty() {
            "新连接".to_string()
        } else {
            trimmed.to_string()
        };
        if let Some(id) = self.editing_tab.take() {
            self.state.update(cx, |s, cx| {
                s.rename_tab_locked(id, final_title);
                cx.notify();
            });
        }
        cx.notify();
    }

    /// 取消重命名（由 TextInput Esc 触发 on_cancel）：丢弃 input 内容，
    /// 不写回 tab.title，editing_tab 清掉。
    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.editing_tab = None;
        cx.notify();
    }

    /// 左箭头 click：把 scroll offset 向 0 方向调（回到初始）150px。
    ///
    /// **GPUI scroll offset 符号约定**（见 div.rs clamp_scroll_position）：
    /// - offset.x ∈ [-max_offset.x, 0]，**负数**表示向右滚（content 相对 viewport 左移）
    /// - max_offset.x = content_size - viewport_size，**正数**（≥ 0）
    /// - offset.x = 0：未滚动 / 最左
    /// - offset.x = -max_offset.x：滚到最右
    fn handle_scroll_left(&mut self, cx: &mut Context<Self>) {
        let cur = self.scroll_handle.offset();
        let new_x = (cur.x + px(150.0)).min(px(0.0));
        self.scroll_handle.set_offset(point(new_x, cur.y));
        cx.notify();
    }

    /// 右箭头 click：offset.x - 150（更负），clamp 到 -max_offset.x（最远向右）。
    fn handle_scroll_right(&mut self, cx: &mut Context<Self>) {
        let cur = self.scroll_handle.offset();
        let max = self.scroll_handle.max_offset();
        let new_x = (cur.x - px(150.0)).max(-max.x);
        self.scroll_handle.set_offset(point(new_x, cur.y));
        cx.notify();
    }

    /// 处理 tab 标题点击。
    /// - click_count >= 2 → 进 inline 编辑模式（用 aish_ui::TextInput entity）
    /// - 单击 + 已有 editing → commit 当前 editing（点别处自动提交）+ select 新 tab
    /// - 单击 + 无 editing → 仅 select tab
    fn handle_tab_click(
        &mut self,
        id: TabId,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 已经在 editing 这个 tab：所有 click 让 TextInput 自己处理（cursor 定位
        // / 拖选 / 双击选词 / 三击选行 全套交互都走 input），TabItem 整体 noop。
        // 主要靠 TextInput 自身 mouse_down 的 stop_propagation 防冒泡（不会到这），
        // 这里是防御性兜底：万一 GPUI 派发顺序变化导致冒泡仍触发，也不会破坏编辑。
        if self.editing_tab == Some(id) {
            return;
        }
        if click_count >= 2 {
            // 进入编辑：current title 灌进 TextInput + focus + 切到 editing 状态
            let current_title = self
                .state
                .read(cx)
                .tabs
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.title.clone())
                .unwrap_or_default();
            self.editing_tab = Some(id);
            self.rename_input.update(cx, |i, cx| {
                i.set_text(current_title, cx);
                // 进入编辑自动全选，方便用户直接输入覆盖原 title（与浏览器
                // / 文件管理器双击重命名一致体感）
                i.select_all();
                i.focus(window, cx);
                cx.notify();
            });
            cx.notify();
            return;
        }
        // 单击：若有 editing 先 commit（拿出当前 input text）
        if self.editing_tab.is_some() {
            let cur_text = self.rename_input.read(cx).text().to_string();
            self.commit_rename(cur_text, cx);
        }
        self.state.update(cx, |s, cx| {
            s.select_tab(id);
            cx.notify();
        });
    }

    /// wheel scroll：完全忽略 delta 大小，每 tick 固定滚 60px。
    ///
    /// 之前按 delta × 0.3 缩放，但 Windows 给的 wheel delta 在快速滚轮 / 高
    /// DPI 鼠标上可能 raw 几百 px，0.3 倍仍跨 1+ tab，用户反馈"跨 45 行"。
    /// 改为只看方向（sign），步长写死 60px ≈ 1/3 tab 宽度。fast scroll 多
    /// tick 累积仍快但每 tick 可预测，不会单 tick 跳过半屏。
    fn handle_wheel(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let sign: f32 = match ev.delta {
            ScrollDelta::Pixels(p) => {
                let v = if p.x.abs() > p.y.abs() { p.x } else { p.y };
                if v > px(0.0) {
                    1.0
                } else if v < px(0.0) {
                    -1.0
                } else {
                    0.0
                }
            }
            ScrollDelta::Lines(l) => {
                let v = if l.x.abs() > l.y.abs() { l.x } else { l.y };
                if v > 0.0 {
                    1.0
                } else if v < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
        };
        if sign == 0.0 {
            return;
        }
        let step = px(60.0 * sign);
        let cur = self.scroll_handle.offset();
        let max = self.scroll_handle.max_offset();
        let new_x = (cur.x + step).clamp(-max.x, px(0.0));
        self.scroll_handle.set_offset(point(new_x, cur.y));
        cx.stop_propagation();
        cx.notify();
    }

    fn handle_close(&mut self, id: TabId, cx: &mut Context<Self>) {
        // 1. 拿到 tab content（若是 connection 需要发 Disconnect）
        let content = self
            .state
            .read(cx)
            .tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.content.clone());
        if let Some(TabContent::Connection(conn)) = content {
            // 给 actor 发 Disconnect（fire-and-forget；actor 自然退出后 sessions 也会清）
            if let Some(sender) = self.state.read(cx).sessions.get(&conn).cloned() {
                self.bridge.spawn(async move {
                    let _ = sender.send(SessionCommand::Disconnect).await;
                });
            }
            // 清 per-conn 状态 + 把所有引用它的 tab 转回 Default
            self.state.update(cx, |s, cx| {
                s.remove_connection(conn);
                cx.notify();
            });
        }
        // 2. 移除 tab 本身
        self.state.update(cx, |s, cx| {
            s.close_tab(id);
            cx.notify();
        });
        // 关闭的是正在 editing 的 tab → 清 editing 状态（避免 dangling reference）
        if self.editing_tab == Some(id) {
            self.editing_tab = None;
        }
    }

    /// 右键菜单 select 路由：按 idx 调对应 action。
    /// idx 与 build_context_menu 内 MenuItem 顺序绑定，不要乱动顺序。
    fn handle_menu_select(
        &mut self,
        tab_id: TabId,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match idx {
            // Rename → 复用 handle_tab_click(click_count=2) 逻辑进编辑模式
            0 => self.handle_tab_click(tab_id, 2, window, cx),
            // 折叠到 Home（保 actor 仅关 tab）
            1 => self.handle_detach(tab_id, cx),
            // Close（disconnect + 关 tab）
            2 => self.handle_close(tab_id, cx),
            // Close others
            3 => self.close_others(tab_id, cx),
            _ => {}
        }
        // action 执行后关闭菜单
        self.context_menu.update(cx, |m, cx| m.close(cx));
        self.menu_tab_id = None;
    }

    /// 折叠到 Home：close_tab 删 tab 但**不** Disconnect / remove_connection，
    /// actor 后台跑，Home Active Sessions 区列出 orphan conn 让用户随时
    /// 点 Open 重 attach 新 tab。Default tab 没 actor 可保，fallback close_tab。
    fn handle_detach(&mut self, id: TabId, cx: &mut Context<Self>) {
        let content = self
            .state
            .read(cx)
            .tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.content.clone());
        // Default tab 无 actor，与"关闭"等价
        let is_conn = matches!(content, Some(TabContent::Connection(_)));
        self.state.update(cx, |s, cx| {
            s.close_tab(id);
            if is_conn {
                // 折叠后切到 Home，让用户立刻看到 Active Sessions 内的 orphan
                s.sidebar = crate::state::SidebarTab::Home;
            }
            cx.notify();
        });
        if self.editing_tab == Some(id) {
            self.editing_tab = None;
        }
    }

    /// 关闭除 keep_id 之外的所有 tab。逐个调 handle_close（含 Disconnect 发送）。
    fn close_others(&mut self, keep_id: TabId, cx: &mut Context<Self>) {
        let others: Vec<TabId> = self
            .state
            .read(cx)
            .tabs
            .iter()
            .filter(|t| t.id != keep_id)
            .map(|t| t.id)
            .collect();
        for id in others {
            self.handle_close(id, cx);
        }
    }

    fn handle_new_tab(&mut self, cx: &mut Context<Self>) {
        // M4a：+ 按钮切回 Home，让用户从 Home 选 host 开始新连接
        self.state.update(cx, |s, cx| {
            s.sidebar = crate::state::SidebarTab::Home;
            cx.notify();
        });
    }
}

impl Focusable for TabBarView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TabBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 每帧重设 context menu content（AnyElement 不 Clone，render 内 take 消耗）。
        // 仅当 menu 目标 tab 已选定时构造内容，避免无谓 DropdownMenu 实例化。
        if let Some(tab_id) = self.menu_tab_id {
            // "折叠到 Home" 仅对 Connection tab 有意义（Default tab 没 actor 可保），
            // 但简化版统一展示让用户操作一致，handler 内对 Default tab fallback
            // 走 close_tab（与"关闭"等价）。
            let weak = cx.weak_entity();
            let menu = DropdownMenu::new("tab-context-menu")
                .items(vec![
                    MenuItem::new("重命名").icon(IconName::Pencil),
                    MenuItem::new("折叠到 Home"),
                    MenuItem::new("关闭").icon(IconName::X).shortcut("Ctrl+W"),
                    MenuItem::new("关闭其他"),
                ])
                .min_width(gpui::px(180.0))
                .selected_idx(Some(self.menu_active_idx))
                .on_select(move |idx, window, cx| {
                    let idx = *idx;
                    if let Some(this) = weak.upgrade() {
                        this.update(cx, |this, cx| {
                            this.handle_menu_select(tab_id, idx, window, cx);
                        });
                    }
                });
            self.context_menu.update(cx, |m, _cx| {
                m.content(menu);
            });
        }
        let app = self.state.read(cx);
        let selected = app.selected_tab;
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        let tab_items: Vec<gpui::AnyElement> = app
            .tabs
            .iter()
            .map(|t| {
                let id = t.id;
                let title = t.title.clone();
                let is_selected = selected == Some(id);
                let is_connection = matches!(t.content, TabContent::Connection(_));
                let is_editing = self.editing_tab == Some(id);
                // connection tab 是否还有活跃 actor（actor 退出后 sessions 里
                // 没了 → false）。用于绿/灰点 + 标题色的"在线/已断"指示。
                let is_alive = match t.content {
                    TabContent::Connection(c) => app.is_session_active(c),
                    _ => false,
                };

                // editing 模式：跳过 TabItem，直接渲染一个**更宽、不裁切**的 inline
                // editor box。原因：TabItem 设计目的是"展示长 title 不撑爆 tab bar"，
                // 它的 max_w(200) + title 容器 overflow_hidden + text_ellipsis 会把
                // input 的 cursor / 右侧文字直接裁掉（66px 留给 title，裁切再叠加）。
                // editing 时这些约束反而是干扰，单独路径完全规避。
                //
                // 布局：[● 连接 dot]  [<TextInput>]   — 不带 close/badge（避免误触
                // 丢编辑），bg 用 background (active 色，editing tab 一定是 selected)。
                if is_editing {
                    let dot_color = if is_alive {
                        colors.success
                    } else {
                        colors.muted_foreground
                    };
                    // 整个 tab box 当 input 视觉外壳：
                    // - bg colors.input：与 HostForm 里 TextInput 视觉一致
                    // - 1px primary 全围 border：表明'编辑中'，替代 active bar
                    //   （bar 在 borderless input 下会与 input cursor 冲突）
                    // - cursor_text：鼠标移入显示文本光标，符合 input 体感
                    // - my 2px：上下留 2px 让 border 不贴 tab bar 边沿，
                    //   总高度仍是 40 + 4 = 44 但视觉上和 40px tab 接近
                    return div()
                        .id(gpui::SharedString::from(format!("tab-editing-{}", id)))
                        .h(px(36.0))
                        .my(px(2.0))
                        // w 固定 240：editing 时 box 宽度不能随内容变化。
                        // 之前 min_w(180) + max_w(280) 但没 w，flex 子元素自然
                        // 宽 = content，input 打字时 text_row 自然宽变大 → box
                        // 跟着 grow → '一边输入 box 一边变大'体感不像 input。
                        // 固定 240：横向 scroll 跟随 cursor 由 TextInput 内部
                        // scroll_offset 负责（看不见的字交给 input 自己卷动）。
                        .w(px(240.0))
                        .flex_shrink_0()
                        .px(theme.spacing.px_3)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(theme.spacing.px_2)
                        .text_size(font_size.sm)
                        .rounded(theme.radius.sm)
                        .bg(colors.input)
                        .border_1()
                        .border_color(colors.primary)
                        .cursor_text()
                        .when(is_connection, |d| {
                            d.child(
                                div()
                                    .text_color(dot_color)
                                    .text_size(font_size.xs)
                                    .child("●"),
                            )
                        })
                        // input 占满剩余空间；min_w(0) 让 flex_1 真生效。
                        // overflow_hidden 关键：borderless TextInput 不自带裁切，
                        // 超长 title 会溢出到相邻 tab 上；这里裁掉，cursor 在
                        // 末尾可能看不见（aish_ui::TextInput 暂未实现水平滚动
                        // 跟随 cursor），但优先保证不污染其他 tab。
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .child(self.rename_input.clone()),
                        )
                        .into_any_element();
                }

                // 关闭按钮（始终可见）
                let close_btn = aish_ui::IconButton::new(
                    gpui::SharedString::from(format!("tab-close-{}", id)),
                    aish_ui::IconName::X,
                )
                .small()
                .ghost()
                .on_click(cx.listener(
                    move |this, _ev: &MouseDownEvent, _w, cx| {
                        // 拦住事件不冒泡到外层 tab listener 触发 select_tab
                        cx.stop_propagation();
                        this.handle_close(id, cx);
                    },
                ));

                // 连接 tab：活跃 = 绿点，已断 = 灰点；默认页 tab 不带前缀
                let prefix: gpui::AnyElement = if is_connection {
                    let dot_color = if is_alive {
                        colors.success
                    } else {
                        colors.muted_foreground
                    };
                    div()
                        .text_color(dot_color)
                        .text_size(font_size.xs)
                        .child("●")
                        .into_any_element()
                } else {
                    div().child("").into_any_element()
                };

                let title_color = if is_connection && !is_alive {
                    colors.muted_foreground
                } else if is_selected {
                    colors.foreground
                } else {
                    colors.secondary_foreground
                };
                let title_el = div()
                    .text_color(title_color)
                    .child(title)
                    .into_any_element();

                // suffix: SSH chip（connection tab 专属）+ 关闭按钮
                let suffix = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .when(is_connection, |d| {
                        d.child(aish_ui::Badge::new("SSH").primary())
                    })
                    .child(close_btn)
                    .into_any_element();

                let title_for_preview = t.title.clone();
                let tab_item =
                    aish_ui::TabItem::new(gpui::SharedString::from(format!("tab-{}", id)))
                        .prefix(prefix)
                        .title(title_el)
                        .suffix(suffix)
                        .active(is_selected)
                        .on_click(cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                            this.handle_tab_click(id, ev.click_count, w, cx);
                        }));

                // 包一层 stateful wrapper div 加 GPUI drag/drop。wrapper id 与
                // TabItem 内部 id 不同避免冲突。
                // - on_drag：drag 开始构造 TabDragPreview ghost；payload 含 source_id
                // - on_drop<DraggedTab>：drop 触发时算 reorder（state.move_tab）
                // - drag_over::<DraggedTab>：drag hover 时高亮 drop target（accent_active）
                div()
                    .id(gpui::SharedString::from(format!("tab-wrap-{}", id)))
                    .flex_shrink_0()
                    .on_drag(DraggedTab { source_id: id }, move |_p, _offset, _w, cx| {
                        cx.new(|_| TabDragPreview {
                            title: title_for_preview.clone(),
                        })
                    })
                    .on_drop(cx.listener(move |this, dragged: &DraggedTab, _w, cx| {
                        let src = dragged.source_id;
                        this.state.update(cx, |s, cx| {
                            if s.move_tab(src, id) {
                                cx.notify();
                            }
                        });
                    }))
                    .drag_over::<DraggedTab>(|style, _dragged, _w, cx| {
                        // hover 标记 drop target —— 用 accent_active 与
                        // M17 Card / NavItem / TabItem 的 mouse-down 反馈同色
                        let mut s = style;
                        s.background = Some(aish_ui::theme(cx).colors.accent_active.into());
                        s
                    })
                    // 鼠标中键关闭 tab（Chrome / VSCode 标准）。on_mouse_down
                    // 直接触发 close，不等 mouse_up —— 与 X 按钮 on_click 行为一致。
                    .on_mouse_down(
                        gpui::MouseButton::Middle,
                        cx.listener(move |this, _ev: &MouseDownEvent, _w, cx| {
                            this.handle_close(id, cx);
                        }),
                    )
                    // 右键打开 context menu：菜单内容（DropdownMenu）由
                    // render 主循环每帧根据 menu_tab_id 重设，避免 closure
                    // 捕获问题。这里仅写 menu_tab_id + open_at(ev.position)。
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |this, ev: &MouseDownEvent, _w, cx| {
                            this.menu_tab_id = Some(id);
                            // 键盘导航：每次新打开重置 active_idx = 0 (首项)
                            this.menu_active_idx = 0;
                            let pos = ev.position;
                            this.context_menu.update(cx, |m, cx| m.open_at(pos, cx));
                            cx.notify();
                        }),
                    )
                    .child(tab_item)
                    .into_any_element()
            })
            .collect();

        // 末尾 + 按钮新建默认页：Chrome 风格 mini-tab 外观
        // - 全高 40px 与 TabItem 一致，与 tab 同一 baseline 不悬浮
        // - 宽 40px 方形，比 tab 窄，但与 tab 视觉同源
        // - bg(card) 与 tab bar 同色（idle 时几乎隐形，只露 + icon）
        // - hover bg secondary_hover、active bg secondary_active，
        //   与 TabItem hover/active 完全一致的视觉反馈
        // - 不用 IconButton 包装：IconButton 自带 rounded + 固定 padding，
        //   与"和 tab 同 baseline 的方形区域"语义冲突；直接 div 更干净
        let hover_bg = colors.secondary_hover;
        let active_bg = colors.secondary_active;
        let plus_btn = div()
            .id("tab-new")
            .h(px(40.0))
            .w(px(40.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(colors.card)
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| this.handle_new_tab(cx)),
            )
            .child(
                aish_ui::icon(aish_ui::IconName::Plus)
                    .size(px(16.0))
                    .text_color(colors.muted_foreground),
            );

        // 外层布局：[ < 箭头?] [ tabs 横向滚动容器 ] [ > 箭头?] [ + 按钮固定 ]
        //
        // 滚动箭头显示条件（基于 GPUI ScrollHandle 精确状态）：
        // GPUI 符号约定：
        //   - offset.x ∈ [-max_offset.x, 0]，负数表示向右滚（更负 = 更靠右）
        //   - max_offset.x = content_size - viewport_size，正数（>0 = 有溢出）
        // 所以：
        //   - show_left：offset.x < 0（已向右滚过，能回左）
        //   - show_right：还有向右空间 → offset.x > -max_offset.x，即
        //     offset.x + max_offset.x > 0
        //   - max_offset.x == 0（没溢出）→ show_right = false（offset 必为 0 + 0 = 0）
        let offset_x = self.scroll_handle.offset().x;
        let max_x = self.scroll_handle.max_offset().x;
        let show_left = offset_x < px(-0.5);
        let show_right = offset_x + max_x > px(0.5);

        let arrow_left = div()
            .id("tab-bar-arrow-left")
            .h_full()
            .w(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(colors.muted_foreground)
            .hover(move |s| s.bg(colors.secondary_hover).text_color(colors.foreground))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_scroll_left(cx);
                }),
            )
            .child(
                // GPUI svg 元素是 monochrome，需要 svg 自己 .text_color() 设
                // paint color；父 div 的 text_color 不会 inherit 到 svg 元素
                // → svg 不画（之前箭头不显示的根因）。
                aish_ui::icon(aish_ui::IconName::ChevronLeft)
                    .size(px(14.0))
                    .text_color(colors.muted_foreground),
            );

        let arrow_right = div()
            .id("tab-bar-arrow-right")
            .h_full()
            .w(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(colors.muted_foreground)
            .hover(move |s| s.bg(colors.secondary_hover).text_color(colors.foreground))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    this.handle_scroll_right(cx);
                }),
            )
            .child(
                aish_ui::icon(aish_ui::IconName::ChevronRight)
                    .size(px(14.0))
                    .text_color(colors.muted_foreground),
            );

        div()
            .track_focus(&self.focus_handle)
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .bg(colors.card)
            .border_b_1()
            .border_color(colors.border)
            .h(px(40.0))
            .when(show_left, |d| d.child(arrow_left))
            .child(
                div()
                    .id("tab-bar-scroll")
                    .flex_1()
                    // flex item 默认 min_width=auto 拒绝 shrink → tab_items
                    // 撑大容器让 overflow_x_scroll 失效。min_w(0) 强制允许
                    // 压缩到 0，flex_1 才取父 div 剩余空间，溢出的 tab 滚动。
                    .min_w(px(0.0))
                    .h_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    // overflow_hidden 而非 overflow_x_scroll：
                    // GPUI 内置 wheel handler 仅在 overflow.x == Overflow::Scroll
                    // 时触发（div.rs:2690 if overflow.x == Scroll {...}），用
                    // Hidden 让它不跑，我的 on_scroll_wheel 唯一接管。
                    // track_scroll 不依赖 overflow，仍然让 ScrollHandle 同步
                    // offset 让 children 按 offset transform paint（实现滚动
                    // 视觉效果）。
                    .overflow_hidden()
                    .track_scroll(&self.scroll_handle)
                    // 自己的 wheel handler：固定 60px/tick，详见 handle_wheel
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _w, cx| {
                        this.handle_wheel(ev, cx);
                    }))
                    .children(tab_items),
            )
            .when(show_right, |d| d.child(arrow_right))
            .child(plus_btn)
        // context_menu entity 不在这 mount —— RootView 在 root 顶层 mount
        // 让 absolute backdrop / anchored 浮层盖在 terminal / session_picker
        // 之上（tab_bar 在下游 view 之前 paint，自己 mount 会被盖）。
    }
}

impl TabBarView {
    /// 暴露 context_menu entity 让 RootView 在 root 顶层 mount。
    pub fn context_menu_entity(&self) -> Entity<ContextMenu> {
        self.context_menu.clone()
    }
}
