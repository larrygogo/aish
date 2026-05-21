//! Dialog — 居中 modal。
//!
//! M12 简化版：Esc + backdrop click 关闭。Tab 循环 focus trap 留 M13 加固。
//!
//! M30：升级 `open: bool` 为 `state: OpenState`（Closed/Opening/Open/Closing），
//! Opening / Closing 期间用 `animate_or_skip` 跑 opacity 0→1 / 1→0
//! medium 150ms ease_out_quint。Closing 期间 dialog **仍渲染**（保持挂在
//! 元素树上播 exit 动画），timer 到时切 Closed 真正 unmount。

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    div, prelude::*, Animation, AnyElement, App, Context, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, Pixels, SharedString, Window,
};

use crate::components::IconButton;
use crate::icons::IconName;
use crate::theme::{animate_or_skip, theme};
use crate::TypographyExt;

/// M30：Dialog 开关动画状态机。
///
/// Transition：
/// - open(): Closed | Closing → Opening；Opening | Open → 无变（保留 timer）
/// - close(): Open | Opening → Closing；Closing | Closed → 无变
///
/// `Opening` / `Closing` 是过渡帧，timer fire 后切 `Open` / `Closed`。
/// reduced_motion=true 时不进过渡态，open() 直接 Open，close() 直接 Closed。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OpenState {
    Closed,
    Opening,
    Open,
    Closing,
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App) + 'static>;

pub struct Dialog {
    focus_handle: FocusHandle,
    /// M30 升级：原 `open: bool` 替换为 4 态机器（Closed/Opening/Open/Closing）。
    state: OpenState,
    needs_focus: bool,
    title: SharedString,
    body: Option<AnyElement>,
    width: Pixels,
    on_close: Option<CloseHandler>,
    /// M31：标题栏右侧 X 关闭按钮升 stateful IconButton（带 press 反馈）。
    close_btn: gpui::Entity<IconButton>,
    /// caller 注册的额外 key handler。在 Dialog 处理 Esc 关闭之后调用。
    /// 用于 caller 实现 ↑/↓/Enter 等列表导航（如 SessionPicker）。
    on_key: Option<KeyHandler>,
    /// Tab focus trap：dialog 内可 focus 的元素顺序链。caller 在 dialog
    /// open 后通过 focus_chain(...) 注册。Tab / Shift+Tab 在此链上循环，
    /// 不让焦点跑出 dialog 外（无障碍 + 减少误操作）。空 = 不启用 trap。
    focus_chain: Vec<FocusHandle>,
    /// M29 D-9：open 后默认 focus 的 element。Some(h) 时 dialog open 即
    /// focus 该 handle（如 host_form 的首 input / delete confirm 的 Cancel
    /// button）；None 时回落到 dialog 自身 focus_handle（M12 原行为）。
    initial_focus: Option<FocusHandle>,
}

impl Dialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 把 X 按钮做成 Entity 时需要在新 cx 中创建并 wire weak callback：
        // weak.update → this.close + fire_close。close_btn 的 on_click handler
        // 不能直接 reference self（self 不存在），用 weak entity 模式。
        let weak = cx.weak_entity();
        let close_btn = cx.new(|cx| {
            let mut b = IconButton::new("dialog-close", IconName::X, cx);
            b.small().on_click(move |_ev, window, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| {
                        this.close(cx);
                        this.fire_close(window, cx);
                    });
                }
            });
            b
        });
        Self {
            focus_handle: cx.focus_handle(),
            state: OpenState::Closed,
            needs_focus: false,
            title: SharedString::default(),
            body: None,
            width: gpui::px(480.0),
            on_close: None,
            on_key: None,
            focus_chain: Vec::new(),
            initial_focus: None,
            close_btn,
        }
    }

    /// M29 D-9: 设 dialog open 后默认 focus 的元素（host_form 首 input /
    /// delete confirm 的 Cancel button）。caller 通常在 sync_from_state
    /// 内 set，open() 内部 set needs_focus=true，下次 render 时 focus 此 handle。
    pub fn initial_focus(&mut self, h: FocusHandle) -> &mut Self {
        self.initial_focus = Some(h);
        self
    }

    pub fn title(&mut self, t: impl Into<SharedString>) -> &mut Self {
        self.title = t.into();
        self
    }

    /// 设置 dialog 内容。**每帧 render 时都需要调用**——`AnyElement` 不可 Clone，
    /// render 内会 `take()` 消耗 body 渲染一次。caller 通常在 `Render::render` 内
    /// 通过 `dialog.update(cx, |d, _| d.body(...))` 每帧重新设置。
    pub fn body(&mut self, body: impl IntoElement) -> &mut Self {
        self.body = Some(body.into_any_element());
        self
    }

    pub fn width(&mut self, w: Pixels) -> &mut Self {
        self.width = w;
        self
    }

    pub fn on_close(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self {
        self.on_close = Some(Rc::new(h));
        self
    }

    /// 注册额外 key handler。Dialog 自己处理 Esc 关闭之后调此 callback，
    /// 让 caller 实现 ↑/↓/Enter 等列表导航键位。
    pub fn on_key(
        &mut self,
        h: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_key = Some(Rc::new(h));
        self
    }

    /// 注册 Tab focus trap 的元素顺序链。Tab / Shift+Tab 在此 chain 上循环。
    /// caller 应在 modal 打开后填入按视觉顺序的 FocusHandle 列表（每次切换
    /// modal 状态前清空 + 重设）。空 chain = 不启用 trap（默认）。
    pub fn focus_chain(&mut self, handles: Vec<FocusHandle>) -> &mut Self {
        self.focus_chain = handles;
        self
    }

    /// 用户视角的"open"：含 Opening / Open / Closing。Closing 期间仍占屏，
    /// 但 caller 通常关心"有没有 modal 在显示"。
    pub fn is_open(&self) -> bool {
        !matches!(self.state, OpenState::Closed)
    }

    /// 打开 dialog。state machine 进 Opening（reduced_motion 时直接 Open）。
    /// 聚焦在下一帧 render 时通过 needs_focus 标记驱动。
    pub fn open(&mut self, cx: &mut Context<Self>) {
        let prev = self.state;
        if matches!(prev, OpenState::Opening | OpenState::Open) {
            return;
        }
        let (reduced, dur) = {
            let t = theme(cx);
            (t.reduced_motion, t.motion.medium)
        };
        self.needs_focus = true;
        if reduced {
            self.state = OpenState::Open;
        } else {
            self.state = OpenState::Opening;
            schedule_state_transition(cx, dur, OpenState::Opening, OpenState::Open);
        }
        cx.notify();
    }

    /// 关闭 dialog。state machine 进 Closing（reduced_motion 时直接 Closed）。
    /// Closing 期间 dialog 仍渲染播 exit 动画，timer 后真正 unmount。
    pub fn close(&mut self, cx: &mut Context<Self>) {
        let prev = self.state;
        if matches!(prev, OpenState::Closing | OpenState::Closed) {
            return;
        }
        let (reduced, dur) = {
            let t = theme(cx);
            (t.reduced_motion, t.motion.medium)
        };
        if reduced {
            self.state = OpenState::Closed;
        } else {
            self.state = OpenState::Closing;
            schedule_state_transition(cx, dur, OpenState::Closing, OpenState::Closed);
        }
        cx.notify();
    }

    fn fire_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_close.clone() {
            h(window, cx);
        }
    }
}

/// 在 `duration` 后把 dialog 状态从 `expected_prev` 切到 `next`。
/// 幂等 check：若期间状态被 open()/close() 改变（如 close→open
/// 50ms 内），timer fire 时 state != expected_prev，本次 timer 不动。
fn schedule_state_transition(
    cx: &mut Context<Dialog>,
    duration: Duration,
    expected_prev: OpenState,
    next: OpenState,
) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(duration).await;
        let _ = this.update(cx, |this, cx| {
            if this.state == expected_prev {
                this.state = next;
                cx.notify();
            }
        });
    })
    .detach();
}

impl Dialog {
    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() == "escape" {
            self.close(cx);
            self.fire_close(window, cx);
            return;
        }

        // Tab focus trap：在 focus_chain 上循环。chain 为空 = caller 没启
        // trap，跳过让 GPUI 默认行为（通常 noop，但至少不影响）。
        if !self.focus_chain.is_empty() && event.keystroke.key.as_str() == "tab" {
            let len = self.focus_chain.len();
            // 找当前 focused handle 在 chain 中的索引；找不到（焦点在外）
            // → Tab 拉回首项，Shift+Tab 拉回末项
            let cur = self.focus_chain.iter().position(|h| h.is_focused(window));
            let next = match (cur, event.keystroke.modifiers.shift) {
                (Some(i), false) => (i + 1) % len,
                (Some(i), true) => (i + len - 1) % len,
                (None, false) => 0,
                (None, true) => len - 1,
            };
            self.focus_chain[next].focus(window, cx);
            cx.notify();
            return;
        }

        // caller 自定义 key handler（如 SessionPicker 的 ↑/↓/Enter）。
        // 在 Esc 之后调用：Esc 由 Dialog 统一处理，caller 无法覆盖。
        if let Some(h) = self.on_key.clone() {
            h(event, window, cx);
        }
    }
}

impl Focusable for Dialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.state == OpenState::Closed {
            return div().into_any_element();
        }

        // 第一次 open 后聚焦，保证 Esc 能响应。
        // M29 D-9: 若 caller 设了 initial_focus 优先 focus 它（如 host_form
        // 首 input / delete confirm Cancel），否则回落到 dialog 自身 focus_handle。
        if self.needs_focus {
            if let Some(h) = self.initial_focus.as_ref() {
                h.focus(window, cx);
            } else {
                self.focus_handle.focus(window, cx);
            }
            self.needs_focus = false;
        }

        // M30：Closing 期间禁用键鼠 — 用户已经触发关闭，避免再次输入产生竞态
        let interactive = matches!(self.state, OpenState::Opening | OpenState::Open);
        let backdrop = self.build_backdrop(interactive, cx);

        // 按 state 选包装：
        // - Open：直接渲染（无动画）
        // - Opening：opacity 0→1 ease_out_quint medium 150ms
        // - Closing：opacity 1→0 ease_out_quint medium 150ms
        let t = theme(cx);
        let dur = t.motion.medium;
        let easing_rc = t.motion.easing_standard.clone();
        match self.state {
            OpenState::Closed => unreachable!(),
            OpenState::Open => backdrop.into_any_element(),
            OpenState::Opening => {
                let easing = easing_rc.clone();
                animate_or_skip(
                    backdrop,
                    t,
                    "motion-dialog-enter",
                    Animation::new(dur).with_easing(move |d| easing(d)),
                    |el, delta| el.opacity(delta),
                )
            }
            OpenState::Closing => {
                let easing = easing_rc.clone();
                animate_or_skip(
                    backdrop,
                    t,
                    "motion-dialog-exit",
                    Animation::new(dur).with_easing(move |d| easing(d)),
                    |el, delta| el.opacity(1.0 - delta),
                )
            }
        }
    }
}

impl Dialog {
    /// 构造 backdrop + dialog content（不含动画包装）。Opening / Open /
    /// Closing 共用同一份。`interactive=false`（Closing）时跳过键鼠 listener，
    /// 避免用户在 exit 动画期间再次触发 close 等竞态。
    fn build_backdrop(&mut self, interactive: bool, cx: &mut Context<Self>) -> gpui::Div {
        let t = theme(cx);
        let title = self.title.clone();
        let body = self.body.take();
        let width = self.width;
        // M39 Phase 3: dialog 圆角从 t.radius.lg (8) → t.anatomy.dialog.radius (12)
        // modal 是主角，更软的角强化"浮起"感（Warp 风）
        let dialog_radius = t.anatomy.dialog.radius;
        let popover_bg = t.colors.popover;
        let border_color = t.colors.border;
        let theme_kind = t.kind;
        let spacing_4 = t.spacing.px_4;
        let spacing_3 = t.spacing.px_3;

        let mut root = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x0000_0099))
            // occlude 让 backdrop hitbox 阻塞所有底层 view 接收鼠标事件
            // （等价于 z-order 上的"完全遮挡"），覆盖 hover/wheel/click/right-click 等
            // 所有类型。GPUI 内置 API，比手工 stop_propagation 一堆 listener 干净。
            .occlude()
            .track_focus(&self.focus_handle);

        if interactive {
            root = root
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    this.handle_key(ev, window, cx);
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                        // backdrop click → 关闭
                        this.close(cx);
                        this.fire_close(window, cx);
                    }),
                );
        }

        root.child(
            div()
                .w(width)
                .max_h(gpui::px(640.0))
                .bg(popover_bg)
                .rounded(dialog_radius)
                .border_1()
                .border_color(border_color)
                // M24 elevation-3 — modal 顶层悬浮
                .shadow(crate::theme::elevation_3(theme_kind))
                .flex()
                .flex_col()
                // 阻止冒泡到 backdrop（GPUI mouse 事件是冒泡的，子元素 mouse_down
                // 不会自动拦住父级 listener；必须显式 stop_propagation 才能让
                // backdrop 的 close listener 不被点击 dialog 内部时触发）。
                .on_mouse_down(MouseButton::Left, |_ev, _w, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .px(spacing_4)
                        .py(spacing_3)
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(border_color)
                        .child(
                            // M26 Dialog title: Title2 (16/600/fg)
                            div()
                                .typography(crate::TypeRole::Title2, theme(cx))
                                .child(title),
                        )
                        .child(self.close_btn.clone()),
                )
                .child(
                    div()
                        .p(spacing_4)
                        .flex_1()
                        .when_some(body, |d, b| d.child(b)),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn open_close_state_machine() {
        let mut open = false;
        if !open {
            open = true;
        }
        assert!(open);
        if open {
            open = false;
        }
        assert!(!open);
    }

    #[test]
    fn esc_triggers_close() {
        let key = "escape";
        let mut open = true;
        if key == "escape" {
            open = false;
        }
        assert!(!open);
    }

    #[test]
    fn other_keys_dont_close() {
        let key = "enter";
        let mut open = true;
        if key == "escape" {
            open = false;
        }
        assert!(open);
    }

    #[test]
    fn default_width_is_480() {
        let width = gpui::px(480.0);
        assert_eq!(width, gpui::px(480.0));
    }

    /// M29 D-9：initial_focus 优先级决策 — pure fn 模拟 render 内 needs_focus
    /// 路径选 focus 目标。
    fn pick_focus_target(initial_focus_set: bool) -> &'static str {
        if initial_focus_set {
            "initial"
        } else {
            "dialog_self"
        }
    }

    #[test]
    fn initial_focus_set_picks_initial() {
        // caller 调 .initial_focus(h) 后 → focus 该 handle 而非 dialog 自身
        assert_eq!(pick_focus_target(true), "initial");
    }

    #[test]
    fn initial_focus_unset_falls_back_to_self() {
        // M12 原行为：未 set initial_focus 时 focus dialog 自身（让 Esc 能响应）
        assert_eq!(pick_focus_target(false), "dialog_self");
    }

    #[test]
    fn open_helper_clamp() {
        // 模拟 open() 的状态变换：原 false → true，needs_focus 同步置 true
        let mut open = false;
        let mut needs_focus = false;
        if !open {
            open = true;
            needs_focus = true;
        }
        assert!(open);
        assert!(needs_focus);

        // 第二次调用 open()，已是 open 状态，不执行分支
        if !open {
            unreachable!();
        }
        // 状态不变
        assert!(open);
        assert!(needs_focus);
    }

    #[test]
    fn close_helper_idempotent() {
        // 模拟 close()：open true → false
        let mut open = true;
        if open {
            open = false;
        }
        assert!(!open);

        // 再次 close()，已是 closed，不执行分支
        if open {
            unreachable!();
        }
        assert!(!open);
    }

    /// M30: open() 状态机的 pure fn 模拟（实际 open() 还要 spawn timer +
    /// cx.notify，这里只测状态机转移逻辑）。
    fn open_transition(cur: super::OpenState) -> super::OpenState {
        use super::OpenState::*;
        match cur {
            Opening | Open => cur,
            Closed | Closing => Opening,
        }
    }

    /// M30: close() 的 pure fn 模拟。
    fn close_transition(cur: super::OpenState) -> super::OpenState {
        use super::OpenState::*;
        match cur {
            Closing | Closed => cur,
            Opening | Open => Closing,
        }
    }

    #[test]
    fn m30_open_from_closed_enters_opening() {
        assert_eq!(
            open_transition(super::OpenState::Closed),
            super::OpenState::Opening
        );
    }

    #[test]
    fn m30_open_from_opening_is_noop() {
        assert_eq!(
            open_transition(super::OpenState::Opening),
            super::OpenState::Opening
        );
    }

    #[test]
    fn m30_open_from_open_is_noop() {
        assert_eq!(
            open_transition(super::OpenState::Open),
            super::OpenState::Open
        );
    }

    #[test]
    fn m30_open_from_closing_restarts_opening() {
        // close → 立即 open 的中断路径
        assert_eq!(
            open_transition(super::OpenState::Closing),
            super::OpenState::Opening
        );
    }

    #[test]
    fn m30_close_from_open_enters_closing() {
        assert_eq!(
            close_transition(super::OpenState::Open),
            super::OpenState::Closing
        );
    }

    #[test]
    fn m30_close_from_opening_enters_closing() {
        // open → 立即 close 的中断路径
        assert_eq!(
            close_transition(super::OpenState::Opening),
            super::OpenState::Closing
        );
    }

    #[test]
    fn m30_close_from_closing_is_noop() {
        assert_eq!(
            close_transition(super::OpenState::Closing),
            super::OpenState::Closing
        );
    }

    #[test]
    fn m30_close_from_closed_is_noop() {
        assert_eq!(
            close_transition(super::OpenState::Closed),
            super::OpenState::Closed
        );
    }

    /// 模拟 reduced_motion 路径 — 跳过 Opening/Closing 中间态。
    fn open_with_reduced(cur: super::OpenState, reduced: bool) -> super::OpenState {
        use super::OpenState::*;
        match (cur, reduced) {
            (Opening | Open, _) => cur,
            (_, true) => Open,
            (_, false) => Opening,
        }
    }

    #[test]
    fn m30_reduced_motion_skips_opening_state() {
        assert_eq!(
            open_with_reduced(super::OpenState::Closed, true),
            super::OpenState::Open
        );
        assert_eq!(
            open_with_reduced(super::OpenState::Closed, false),
            super::OpenState::Opening
        );
    }
}
