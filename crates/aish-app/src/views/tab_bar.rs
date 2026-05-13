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
    div, point, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, MouseDownEvent,
    ScrollDelta, ScrollHandle, ScrollWheelEvent, Window,
};

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TabContent};

pub struct TabBarView {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    /// 保留 focus_handle 兼容 Focusable trait（其他 view 可能 expect TabBarView
    /// is Focusable）。inline rename 已禁用，目前 focus 没实际承载键盘事件。
    focus_handle: FocusHandle,
    /// tabs 横向滚动 handle。绑定到 scroll 容器后可读 offset / max_offset 决定
    /// 是否显示左右 < > 箭头，并通过 set_offset 编程式滚动。
    scroll_handle: ScrollHandle,
}

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
        Self {
            state,
            bridge,
            tx,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
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
    ///
    /// inline rename 模式已禁用（之前双击进入 editing_tab + 手糊 div 键盘
    /// 监听，但 GPUI 的 key_char 路径在 IME / 中文输入时不可靠，且缺少
    /// "点击外部失焦" 机制，用户陷入"既不能编辑又无法失焦"。后续可改
    /// aish_ui::Dialog + TextInput 实现 rename 弹窗。当前所有 click 仅切换 tab。
    fn handle_tab_click(
        &mut self,
        id: TabId,
        _click_count: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |s, cx| {
            s.select_tab(id);
            cx.notify();
        });
    }

    /// wheel scroll 缩放：GPUI 内置 wheel scroll 步长按平台默认（Windows
    /// 每 tick ≈ 100-120px），对 tab bar 这种紧凑 UI 偏快。拦截 wheel event
    /// 自己按 0.3 倍缩放 + 横向滚 + clamp。stop_propagation 阻止 GPUI 内置
    /// scroll 再叠加（否则两路同时滚，速度更快）。
    fn handle_wheel(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        // 优先用横向 delta（横向滚轮）；没有时用纵向 delta（普通滚轮 → 横滚）
        let dy = match ev.delta {
            ScrollDelta::Pixels(p) => {
                if p.x.abs() > p.y.abs() {
                    p.x
                } else {
                    p.y
                }
            }
            ScrollDelta::Lines(l) => {
                // Lines mode 按 12px/line 估算（与 GPUI 内部一致）
                let raw = if l.x.abs() > l.y.abs() { l.x } else { l.y };
                px(raw * 12.0)
            }
        };
        // 用户滚轮上滚（dy > 0）→ tabs 向左移（offset.x 增大）→ "看到右边"
        // 用户滚轮下滚（dy < 0）→ tabs 向右移（offset.x 减小）→ "看到左边"
        // GPUI ScrollDelta 约定：垂直滚轮 y 上正下负，反映用户感知
        let scaled = dy * 0.3;
        let cur = self.scroll_handle.offset();
        let max = self.scroll_handle.max_offset();
        let new_x = (cur.x + scaled).clamp(-max.x, px(0.0));
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
        let _ = id; // editing 已禁用，不再清 editing state
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
        let app = self.state.read(cx);
        let selected = app.selected_tab;
        let theme = aish_ui::theme(cx);
        let colors = theme.colors;
        let font_size = theme.font_size;

        let tab_items: Vec<_> = app
            .tabs
            .iter()
            .map(|t| {
                let id = t.id;
                let title = t.title.clone();
                let is_selected = selected == Some(id);
                let is_connection = matches!(t.content, TabContent::Connection(_));
                // connection tab 是否还有活跃 actor（actor 退出后 sessions 里
                // 没了 → false）。用于绿/灰点 + 标题色的"在线/已断"指示。
                let is_alive = match t.content {
                    TabContent::Connection(c) => app.is_session_active(c),
                    _ => false,
                };

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

                // 标题部分：已断的 connection tab 标题 muted 弱化
                let title_color = if is_connection && !is_alive {
                    colors.muted_foreground
                } else if is_selected {
                    colors.foreground
                } else {
                    colors.secondary_foreground
                };
                let title_el: gpui::AnyElement = div()
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

                aish_ui::TabItem::new(gpui::SharedString::from(format!("tab-{}", id)))
                    .prefix(prefix)
                    .title(title_el)
                    .suffix(suffix)
                    .active(is_selected)
                    .on_click(cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                        this.handle_tab_click(id, ev.click_count, w, cx);
                    }))
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
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll_handle)
                    // 拦 wheel 自己缩 0.3 倍滚动，避免 GPUI 内置 wheel 速度
                    // 在 tab bar 紧凑 UI 下过快（每 tick 跳 100px+ 体感粗暴）
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _w, cx| {
                        this.handle_wheel(ev, cx);
                    }))
                    .children(tab_items),
            )
            .when(show_right, |d| d.child(arrow_right))
            .child(plus_btn)
    }
}
