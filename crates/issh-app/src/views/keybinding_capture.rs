//! Keybinding 捕获 dialog — Settings → Shortcuts 点「自定义」时弹出。
//!
//! 行为：
//!   - on_key 接收 Dialog 透传的 KeyDownEvent，组合成 keystroke 字符串
//!   - 显示「当前捕获：⌃⇧C」chip，没捕获时显示提示「按下你想绑定的组合键」
//!   - Enter（无 modifier）= 保存 captured（如果有合法 binding）
//!   - Esc 由 Dialog 自身处理，回 handle_close → 清 pending_keybinding_capture
//!   - 不合法（仅按 modifier 或裸键无 modifier）忽略不更新
//!
//! Phase B 之后：保存写盘后 RootView::handle_global_key 与 terminal_view 的
//! key handler 都读 state.keybindings 即时生效（不需重启）。

use gpui::{div, prelude::*, px, Context, Entity, IntoElement, KeyDownEvent, SharedString, Window};
use issh_ui::{theme, Dialog, TypographyExt};

use crate::keybindings::{
    self, default_for, format_for_display, is_valid_binding, keystroke_to_string,
};
use crate::state::AppState;

pub struct KeybindingCaptureView {
    state: Entity<AppState>,
    dialog: Entity<Dialog>,
    /// 当前会话已捕获的 keystroke 字符串。None = 还未按任何键 / 刚清空。
    /// Enter 保存后清回 None（dialog 关闭，下次开新 action 时重置）。
    captured: Option<String>,
    /// mirror state.pending_keybinding_capture，防重复 open。
    is_open_for: Option<String>,
}

impl KeybindingCaptureView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, _state, cx| {
            this.sync_from_state(cx);
            cx.notify();
        })
        .detach();

        let dialog = cx.new(Dialog::new);
        let weak_close = cx.weak_entity();
        let weak_key = cx.weak_entity();
        dialog.update(cx, move |d, _cx| {
            d.title("自定义快捷键");
            d.width(px(440.0));
            d.on_close(move |_window, cx| {
                if let Some(this) = weak_close.upgrade() {
                    this.update(cx, |this, cx| this.handle_close(cx));
                }
            });
            d.on_key(move |ev, _w, cx| {
                if let Some(this) = weak_key.upgrade() {
                    this.update(cx, |this, cx| this.handle_dialog_key(ev, cx));
                }
            });
        });

        Self {
            state,
            dialog,
            captured: None,
            is_open_for: None,
        }
    }

    fn handle_dialog_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let key = ks.key.as_str();

        // Enter（无 modifier）= 保存当前捕获
        if key == "enter" && !ks.modifiers.modified() {
            if let Some(s) = self.captured.clone() {
                if is_valid_binding(&s) {
                    self.save(s, cx);
                }
            }
            return;
        }

        // 跳过纯 modifier press（key 为空或仅 modifier 名）— GPUI 会在按下/松开
        // modifier 单独发一个 KeyDownEvent，我们不想把「按住 Ctrl」当成 binding。
        if key.is_empty()
            || matches!(
                key,
                "control" | "shift" | "alt" | "command" | "platform" | "function" | "meta"
            )
        {
            return;
        }

        // Esc 让 Dialog 自身处理（已 hookup on_close → handle_close）
        if key == "escape" {
            return;
        }

        // 组合 keystroke 字符串，validity 检查
        let candidate = keystroke_to_string(ks);
        if is_valid_binding(&candidate) {
            self.captured = Some(candidate);
            cx.notify();
        }
        // 不合法（裸键）则忽略不更新 — 用户看到 captured 没变化，知道需要加
        // modifier。底部提示文案已说明。
    }

    fn handle_close(&mut self, cx: &mut Context<Self>) {
        // Esc / 点击遮罩关闭 = 不保存
        self.state.update(cx, |s, cx| {
            s.pending_keybinding_capture = None;
            cx.notify();
        });
    }

    fn save(&mut self, ks: String, cx: &mut Context<Self>) {
        let Some(action_id) = self.is_open_for.clone() else {
            return;
        };
        // 1) 更新内存 state
        self.state.update(cx, |s, cx| {
            s.keybindings.insert(action_id.clone(), ks.clone());
            s.pending_keybinding_capture = None;
            cx.notify();
        });
        // 2) 写盘 — 读 snapshot 改 keybindings 字段后写回，避免覆盖其他字段
        let mut snapshot = crate::app_state_file::load_app_state();
        snapshot.keybindings.insert(action_id, ks);
        crate::app_state_file::save_app_state(&snapshot);
    }

    fn sync_from_state(&mut self, cx: &mut Context<Self>) {
        let pending = self.state.read(cx).pending_keybinding_capture.clone();
        match pending {
            Some(ref action_id) if self.is_open_for.as_ref() != Some(action_id) => {
                self.is_open_for = Some(action_id.clone());
                self.captured = None;
                self.dialog.update(cx, |d, cx| d.open(cx));
            }
            None if self.is_open_for.is_some() => {
                self.is_open_for = None;
                self.captured = None;
                self.dialog.update(cx, |d, cx| d.close(cx));
            }
            _ => {}
        }
    }
}

impl Render for KeybindingCaptureView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let action_id = match &self.is_open_for {
            Some(id) => id.clone(),
            None => return self.dialog.clone().into_any_element(),
        };

        let t = theme(cx);
        let colors = t.colors;

        // 找 action label 给用户看
        let action_label = keybindings::ACTIONS
            .iter()
            .find(|a| a.id == action_id)
            .map(|a| a.label)
            .unwrap_or("");

        // 当前显示：用户已捕获 → 显示 captured；否则显示当前 (override 或 default)
        let current_display = if let Some(c) = &self.captured {
            format_for_display(c)
        } else {
            let cur = self
                .state
                .read(cx)
                .keybindings
                .get(&action_id)
                .cloned()
                .unwrap_or_else(|| default_for(&action_id).to_string());
            format_for_display(&cur)
        };

        let chip_bg = if self.captured.is_some() {
            colors.accent
        } else {
            colors.secondary
        };
        let chip_fg = if self.captured.is_some() {
            colors.accent_foreground
        } else {
            colors.secondary_foreground
        };

        let body =
            div()
                .flex()
                .flex_col()
                .gap(t.spacing.px_4)
                .child(
                    // 1) 说明文案：为哪个 action 设置 + 提示按键
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.spacing.px_1)
                        .child(div().typography(issh_ui::TypeRole::Body, t).child(
                            SharedString::from(format!("为「{}」设置新组合键", action_label)),
                        ))
                        .child(
                            div()
                                .typography(issh_ui::TypeRole::Caption, t)
                                .text_color(colors.muted_foreground)
                                .child(
                                    "按下你想绑定的组合键（必须含 Ctrl / Shift / Alt / Cmd 之一）",
                                ),
                        ),
                )
                .child(
                    // 2) 大显示框：居中 chip
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py(t.spacing.px_4)
                        .child(
                            div()
                                .px(t.spacing.px_3)
                                .py(t.spacing.px_2)
                                .rounded(t.radius.md)
                                .bg(chip_bg)
                                .border_1()
                                .border_color(colors.border)
                                .text_color(chip_fg)
                                .typography(issh_ui::TypeRole::Code, t)
                                .child(SharedString::from(current_display)),
                        ),
                )
                .child(
                    // 3) 操作提示
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .typography(issh_ui::TypeRole::Caption, t)
                        .text_color(colors.muted_foreground)
                        .child(if self.captured.is_some() {
                            "Enter 确认 · Esc 取消"
                        } else {
                            "按任意组合键 · Esc 取消"
                        })
                        .child(div().child("保存后立即生效")),
                );

        self.dialog.update(cx, |d, _cx| {
            d.body(body);
        });

        self.dialog.clone().into_any_element()
    }
}
