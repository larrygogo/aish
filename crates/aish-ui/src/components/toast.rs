//! Toast — 自动消失的提示。
//!
//! 三层：
//! - `Toast` 数据结构
//! - `ToastManager` Entity（队列 + 渲染 + 定时清理）
//! - `ToastHandle` Global（持有 Entity<ToastManager> 的引用，让任意位置都能 push）
//!
//! M31：close X 按钮升 IconButtonEntity stateful，每个 toast 持独立 entity
//! 让 press feedback 生效。close_buttons HashMap 跟随 toasts vec 增删同步
//! retain，避免 entity 泄漏。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{div, prelude::*, px, App, Context, Entity, IntoElement, Render, SharedString, Window};

use crate::components::IconButtonEntity;
use crate::icons::{icon, IconName};
use crate::theme::theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    fn icon_name(&self) -> IconName {
        match self {
            ToastKind::Info => IconName::Info,
            ToastKind::Success => IconName::Check,
            ToastKind::Warning => IconName::AlertTriangle,
            ToastKind::Error => IconName::AlertCircle,
        }
    }
}

#[derive(Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: SharedString,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct ToastManager {
    toasts: Vec<Toast>,
    next_id: u64,
    /// M31：per-toast close X 按钮 entity（key = toast.id）。push 时插入，
    /// dismiss / cleanup_expired retain 同步清掉过期 entry 防泄漏。
    close_buttons: HashMap<u64, Entity<IconButtonEntity>>,
}

impl ToastManager {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let this = Self {
            toasts: Vec::new(),
            next_id: 1,
            close_buttons: HashMap::new(),
        };
        this.start_cleanup_timer(cx);
        this
    }

    pub fn push(&mut self, kind: ToastKind, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.toasts.push(Toast {
            id,
            kind,
            message: msg.into(),
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        });

        // M31：为此 toast 创建 close X button entity，weak callback 触发 dismiss
        let weak = cx.weak_entity();
        let btn = cx.new(|cx| {
            let mut b = IconButtonEntity::new(("toast-close", id as usize), IconName::X, cx);
            b.small().ghost().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.dismiss(id, cx));
                }
            });
            b
        });
        self.close_buttons.insert(id, btn);

        cx.notify();
    }

    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        let before = self.toasts.len();
        self.toasts.retain(|t| t.id != id);
        if self.toasts.len() != before {
            self.close_buttons.remove(&id);
            cx.notify();
        }
    }

    pub(crate) fn cleanup_expired(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let before = self.toasts.len();
        self.toasts
            .retain(|t| now.duration_since(t.created_at) < t.duration);
        if self.toasts.len() != before {
            // 同步 retain close_buttons：仅保留仍存在的 toast id
            let alive: std::collections::HashSet<u64> = self.toasts.iter().map(|t| t.id).collect();
            self.close_buttons.retain(|id, _| alive.contains(id));
            cx.notify();
        }
    }

    fn start_cleanup_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            if this
                .update(cx, |this, cx| this.cleanup_expired(cx))
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    pub fn toasts(&self) -> &[Toast] {
        &self.toasts
    }
}

impl Render for ToastManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 提前取 token，避免 theme(cx) 借用与 render_toast(cx mut) 的冲突
        let (right_spacing, col_gap) = {
            let t = theme(cx);
            (t.spacing.px_4, t.spacing.px_2)
        };
        let toasts = self.toasts.clone();
        // 位置：右下角。M31：从 close_buttons HashMap 取每条 toast 对应的
        // close_btn entity，传入 render_toast。
        let children: Vec<gpui::AnyElement> = toasts
            .into_iter()
            .filter_map(|toast| {
                let btn = self.close_buttons.get(&toast.id).cloned()?;
                Some(render_toast(toast, btn, cx))
            })
            .collect();
        div()
            .absolute()
            .bottom(px(96.0))
            .right(right_spacing)
            .flex()
            .flex_col_reverse()
            .gap(col_gap)
            .children(children)
    }
}

fn render_toast(
    toast: Toast,
    close_btn: Entity<IconButtonEntity>,
    cx: &mut App,
) -> gpui::AnyElement {
    let t = theme(cx);
    let (border_color, fg_color) = match toast.kind {
        ToastKind::Info => (t.colors.accent, t.colors.foreground),
        ToastKind::Success => (t.colors.success, t.colors.foreground),
        ToastKind::Warning => (t.colors.warning, t.colors.foreground),
        ToastKind::Error => (t.colors.destructive, t.colors.foreground),
    };

    let toast_id = toast.id;

    // 视觉：rounded card + 1px kind 色 border + shadow + popover bg。
    // 之前用 absolute 4px 左条强调 kind，但与项目整体灰阶 + indicator 条
    // 风格统一去除（绿/红色实色条与 modern minimal 不搭）。kind 区分靠
    // icon 颜色 + 1px border 颜色微妙提示，不抢眼。
    let card = div()
        .relative()
        .min_w(px(300.0))
        .max_w(px(440.0))
        // M27 anatomy.overlay.toast_p (12)
        .p(t.anatomy.overlay.toast_p)
        .rounded(t.radius.lg)
        .bg(t.colors.popover)
        .border_1()
        .border_color(border_color)
        // M24 elevation-3：modal/toast 最高悬浮层，dark alpha 0.65 / light 0.22
        .shadow(crate::theme::elevation_3(t.kind))
        .flex()
        .flex_row()
        .items_start() // icon 与文字第一行顶部对齐（长文字换行时仍对齐）
        // M27 anatomy.overlay.toast_gap (12)
        .gap(t.anatomy.overlay.toast_gap)
        // svg 必须套 flex_shrink_0 div：GPUI svg 元素本身设了 size(18px)，
        // 但 flex 默认 shrink=1，在长 message 挤压 max_w 时会把 svg 压到
        // width=0 触发 'can't render at a zero size' 报错刷屏。包一层固定宽
        // 高的 flex_shrink_0 div 把 svg 钉住。
        .child(
            div()
                .flex_shrink_0()
                .w(px(18.0))
                .h(px(18.0))
                .child(
                    icon(toast.kind.icon_name())
                        .size(px(18.0))
                        .text_color(border_color),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0)) // 让 flex_1 真生效允许 text_ellipsis / 换行
                .pt(px(1.0)) // 让文字 baseline 与 icon 中心更对齐
                .text_size(t.font_size.sm)
                .text_color(fg_color)
                .child(toast.message),
        )
        // close_btn 同样钉住宽度，长 message 不会把 X 挤掉
        .child(div().flex_shrink_0().child(close_btn));

    // M30：每条 toast 包 enter 动画（slow 250ms ease_out_quint opacity 0→1）。
    // exit 不做（D-4 Phase 3：自动 dismiss / X click 直接 unmount，避免三态机
    // 复杂度 — toast 队列里可能多个 closing 状态混乱）。
    // 注：spec D-4 原方案是 opacity + translate_x，但 GPUI div 不支持 transform
    // translate（仅 svg），简化为 opacity-only。手测后视觉够 subtle。
    let easing = t.motion.easing_standard.clone();
    crate::theme::animate_or_skip(
        card,
        t,
        ("motion-toast-enter", toast_id as usize),
        gpui::Animation::new(t.motion.slow).with_easing(move |d| easing(d)),
        |el, delta| el.opacity(delta),
    )
}

#[derive(Clone)]
pub struct ToastHandle(pub Entity<ToastManager>);

impl gpui::Global for ToastHandle {}

/// 公共 API：从任意 cx push toast。要求 `ToastHandle` 已注册为 global。
pub fn toast(cx: &mut App, kind: ToastKind, msg: impl Into<SharedString>) {
    let handle = cx.global::<ToastHandle>().clone();
    let msg = msg.into();
    handle.0.update(cx, |m, cx| m.push(kind, msg, cx));
}

pub fn toast_info(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Info, msg);
}
pub fn toast_success(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Success, msg);
}
pub fn toast_warning(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Warning, msg);
}
pub fn toast_error(cx: &mut App, msg: impl Into<SharedString>) {
    toast(cx, ToastKind::Error, msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_name_per_kind() {
        assert_eq!(ToastKind::Info.icon_name(), IconName::Info);
        assert_eq!(ToastKind::Success.icon_name(), IconName::Check);
        assert_eq!(ToastKind::Warning.icon_name(), IconName::AlertTriangle);
        assert_eq!(ToastKind::Error.icon_name(), IconName::AlertCircle);
    }

    #[test]
    fn cleanup_logic_removes_expired() {
        let now = Instant::now();
        let toasts = vec![
            Toast {
                id: 1,
                kind: ToastKind::Info,
                message: "old".into(),
                created_at: now - Duration::from_secs(10),
                duration: Duration::from_secs(3),
            },
            Toast {
                id: 2,
                kind: ToastKind::Info,
                message: "fresh".into(),
                created_at: now,
                duration: Duration::from_secs(3),
            },
        ];
        let kept: Vec<_> = toasts
            .into_iter()
            .filter(|t| now.duration_since(t.created_at) < t.duration)
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, 2);
    }

    #[test]
    fn dismiss_removes_by_id() {
        let mut toasts = vec![
            Toast {
                id: 1,
                kind: ToastKind::Info,
                message: "a".into(),
                created_at: Instant::now(),
                duration: Duration::from_secs(3),
            },
            Toast {
                id: 2,
                kind: ToastKind::Info,
                message: "b".into(),
                created_at: Instant::now(),
                duration: Duration::from_secs(3),
            },
        ];
        toasts.retain(|t| t.id != 1);
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].id, 2);
    }

    #[test]
    fn next_id_wraps_safely() {
        let mut id: u64 = u64::MAX;
        id = id.wrapping_add(1);
        assert_eq!(id, 0);
    }
}
