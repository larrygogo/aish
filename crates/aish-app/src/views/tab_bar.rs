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

use aish_ui::TextInput;

use crate::bridge::Bridge;
use crate::state::{AppState, SessionCommand, SshEvent, TabContent};

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

        // 重命名输入框：long-lived entity，三个 callback：
        // - on_submit (Enter)：commit_rename
        // - on_blur (点击 input 外部失焦)：commit_rename（保留改动，符合
        //   macOS / 浏览器 inline edit 体感）
        // - on_cancel (Esc)：cancel_rename（不保留改动，恢复原 title）
        //
        // 用 TextInput 默认样式（有 bg + border + focus ring），让用户明显
        // 看出"这是个输入框可以编辑"。之前 borderless 视觉太干净反而像静态文字。
        let rename_input = cx.new(TextInput::new);
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

        Self {
            state,
            bridge,
            tx,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
            editing_tab: None,
            rename_input,
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
                let is_editing = self.editing_tab == Some(id);
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

                // 标题部分：editing 时用 TextInput entity 替代；否则普通 div + title
                let title_el: gpui::AnyElement = if is_editing {
                    // TextInput 自带 IME / 中文 / focus / Enter submit / 选区 etc.
                    // 给一个 min_w 让输入框不被 TabItem max_w 限制压缩成 0 宽
                    div()
                        .min_w(px(120.0))
                        .child(self.rename_input.clone())
                        .into_any_element()
                } else {
                    let title_color = if is_connection && !is_alive {
                        colors.muted_foreground
                    } else if is_selected {
                        colors.foreground
                    } else {
                        colors.secondary_foreground
                    };
                    div()
                        .text_color(title_color)
                        .child(title)
                        .into_any_element()
                };

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
    }
}
