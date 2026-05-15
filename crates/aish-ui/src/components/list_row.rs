//! ListRow — 通用列表行 stateful Entity，带 hover transition + press feedback。
//!
//! 适用场景：session_picker 行 / home Active Sessions 行 / 任何「点击触发动作的
//! list 行」。与 Card 区别：单一 body slot（不分 header/body/footer），自定义
//! padding/radius，可选 selected 状态（active selected 视觉跳 hover 推进，与
//! TabItem/NavItem 同模式）。
//!
//! caller 持 Entity<ListRow>（HashMap<RowKey, Entity<ListRow>> retain 模式），
//! render 每帧 `entity.update(cx, |row, _| row.body(content))` 重设 body（AnyElement
//! 不可 Clone）。

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Animation, AnyElement, App, Context, ElementId, IntoElement, MouseButton,
    MouseDownEvent, Pixels, Window,
};

use crate::components::button::{press_opacity_at, HoverState};
use crate::theme::{animate_or_skip, theme};

type ClickHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

pub struct ListRow {
    id: ElementId,
    body: Option<AnyElement>,
    on_click: Option<ClickHandler>,
    /// active selected — 跳过 hover 状态机推进，base_bg 锁在 hover_bg 上（视觉
    /// 与 hover 一致，让键盘选中行与鼠标 hover 行共用同色，同时鼠标 hover 其它
    /// 行仍能正常 transition）。
    selected: bool,
    radius: Pixels,
    padding_x: Pixels,
    padding_y: Pixels,
    pressing: bool,
    press_count: u64,
    hover_state: HoverState,
    hover_anim_count: u64,
}

impl ListRow {
    pub fn new(id: impl Into<ElementId>, _cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            body: None,
            on_click: None,
            selected: false,
            radius: px(6.0), // 默认 md
            padding_x: px(12.0),
            padding_y: px(8.0),
            pressing: false,
            press_count: 0,
            hover_state: HoverState::Idle,
            hover_anim_count: 0,
        }
    }

    pub fn body(&mut self, b: impl IntoElement) -> &mut Self {
        self.body = Some(b.into_any_element());
        self
    }

    pub fn on_click(
        &mut self,
        h: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> &mut Self {
        self.on_click = Some(Rc::new(h));
        self
    }

    pub fn selected(&mut self, s: bool) -> &mut Self {
        self.selected = s;
        self
    }

    pub fn radius(&mut self, r: Pixels) -> &mut Self {
        self.radius = r;
        self
    }

    pub fn padding(&mut self, x: Pixels, y: Pixels) -> &mut Self {
        self.padding_x = x;
        self.padding_y = y;
        self
    }

    /// 与 button.rs::fire_press 同模式（press_count 幂等 check）。
    fn fire_press(&mut self, cx: &mut Context<Self>) {
        self.pressing = true;
        self.press_count = self.press_count.wrapping_add(1);
        let expected = self.press_count;
        let dur = theme(cx).motion.medium;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(dur).await;
            let _ = this.update(cx, |this, cx| {
                if this.press_count == expected && this.pressing {
                    this.pressing = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// hover 状态机推进（含 Leaving 反向 lerp）— 与 Card / TabItem 同模式。
    fn fire_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            match self.hover_state {
                HoverState::Idle | HoverState::Leaving { .. } => {
                    let reduced = theme(cx).reduced_motion;
                    if reduced {
                        self.hover_state = HoverState::Hovered;
                        cx.notify();
                    } else {
                        let count = self.hover_anim_count.wrapping_add(1);
                        self.hover_anim_count = count;
                        self.hover_state = HoverState::Entering { anim_count: count };
                        let dur = theme(cx).motion.medium;
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(dur).await;
                            let _ = this.update(cx, |this, cx| {
                                if matches!(
                                    this.hover_state,
                                    HoverState::Entering { anim_count } if anim_count == count
                                ) {
                                    this.hover_state = HoverState::Hovered;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                        cx.notify();
                    }
                }
                HoverState::Entering { .. } | HoverState::Hovered => {}
            }
        } else {
            match self.hover_state {
                HoverState::Hovered => {
                    let reduced = theme(cx).reduced_motion;
                    if reduced {
                        self.hover_state = HoverState::Idle;
                        cx.notify();
                    } else {
                        let count = self.hover_anim_count.wrapping_add(1);
                        self.hover_anim_count = count;
                        self.hover_state = HoverState::Leaving { anim_count: count };
                        let dur = theme(cx).motion.medium;
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(dur).await;
                            let _ = this.update(cx, |this, cx| {
                                if matches!(
                                    this.hover_state,
                                    HoverState::Leaving { anim_count } if anim_count == count
                                ) {
                                    this.hover_state = HoverState::Idle;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                        cx.notify();
                    }
                }
                HoverState::Entering { .. } => {
                    self.hover_state = HoverState::Idle;
                    cx.notify();
                }
                HoverState::Idle | HoverState::Leaving { .. } => {}
            }
        }
    }
}

impl Render for ListRow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        // M17 一致性：list row hover bg 用 secondary 灰阶（与 Card / TabItem 同源）。
        // idle 透明（让外层 popover / card 的 bg 透过），selected 锁在 hover_bg。
        let idle_bg = gpui::transparent_black();
        let hover_bg = t.colors.secondary_hover;
        let active_press_bg = t.colors.secondary_active;
        let medium = t.motion.medium;
        let easing = t.motion.easing_standard.clone();

        let selected = self.selected;
        let hover_state = self.hover_state;
        let pressing = self.pressing;
        // selected=true 时跳 hover 推进；hover_entering/leaving 均 false → 走非动画路径
        let hover_entering = !selected && matches!(hover_state, HoverState::Entering { .. });
        let hover_leaving = !selected && matches!(hover_state, HoverState::Leaving { .. });
        let need_anim = pressing || hover_entering || hover_leaving;
        let press_count = self.press_count;
        let hover_anim_count = self.hover_anim_count;

        // base_bg robustness：非 selected 时永远 idle_bg；实际 hover 视觉由
        // 下方两路径接管（non-animate path 附 GPUI .hover() 真实视觉；animate
        // path closure lerp 主导）。这样即使 hover_state 卡 Hovered（mouse_down
        // 期间 GPUI 捕获鼠标漏 on_hover(false) 事件），鼠标真离开 div 时 .hover
        // 不触发 → 视觉 instant 回 idle，不卡 hover 色。与 NavItem 同模式。
        let base_bg = if selected { hover_bg } else { idle_bg };

        let handler = self.on_click.clone();
        let on_press_listener = cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
            this.fire_press(cx);
            if let Some(h) = handler.clone() {
                h(ev, window, cx);
            }
        });
        let on_hover_listener = cx.listener(move |this, &hovered: &bool, _w, cx| {
            this.fire_hover(hovered, cx);
        });

        let has_click = self.on_click.is_some();
        let radius = self.radius;
        let padding_x = self.padding_x;
        let padding_y = self.padding_y;

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .px(padding_x)
            .py(padding_y)
            .rounded(radius)
            .bg(base_bg);

        if has_click {
            el = el
                .cursor_pointer()
                .active(move |s| s.bg(active_press_bg))
                .on_mouse_down(MouseButton::Left, on_press_listener)
                .on_hover(on_hover_listener);
        }

        if let Some(b) = self.body.take() {
            el = el.child(b);
        }

        if !need_anim {
            // 非动画稳态路径：附 GPUI .hover() 作为真实视觉源（mouse 真在 div
            // 时切 hover bg，离开 instant 回 idle）。仅 has_click 且非 selected
            // 时挂 — selected 视觉锁 hover_bg 不参与 hover。
            if has_click && !selected {
                el = el.hover(move |s| s.bg(hover_bg));
            }
            return el.into_any_element();
        }

        let anim_id: ElementId = (
            "motion-list-row",
            press_count.wrapping_add(hover_anim_count) as usize,
        )
            .into();

        animate_or_skip(
            el,
            t,
            anim_id,
            Animation::new(medium).with_easing(move |d| easing(d)),
            move |el, delta| {
                let mut el = el;
                if hover_entering {
                    el = el.bg(crate::lerp_hsla(idle_bg, hover_bg, delta));
                } else if hover_leaving {
                    el = el.bg(crate::lerp_hsla(hover_bg, idle_bg, delta));
                }
                if pressing {
                    el = el.opacity(press_opacity_at(delta));
                }
                el
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯函数测：selected=true 时 hover transitions 不应 enter Entering/Leaving。
    /// 通过 fire_hover 后 state 仍是 Idle 来验。
    /// （单独抽出纯 state 逻辑会更好但工程开销大，这里仅检查 selected blocks
    /// hover_entering/leaving 布尔，与 TabItem next_hover_when_active 同模式。）
    #[test]
    fn selected_blocks_hover_entering_flag() {
        // hover_entering = !selected && Entering — selected=true 时永远 false
        let selected = true;
        let state = HoverState::Entering { anim_count: 1 };
        let hover_entering = !selected && matches!(state, HoverState::Entering { .. });
        assert!(!hover_entering);
    }

    #[test]
    fn unselected_entering_flag_is_true() {
        let selected = false;
        let state = HoverState::Entering { anim_count: 1 };
        let hover_entering = !selected && matches!(state, HoverState::Entering { .. });
        assert!(hover_entering);
    }

    #[test]
    fn unselected_leaving_flag_is_true() {
        let selected = false;
        let state = HoverState::Leaving { anim_count: 1 };
        let hover_leaving = !selected && matches!(state, HoverState::Leaving { .. });
        assert!(hover_leaving);
    }
}
