//! 添加/编辑/删除确认 modal。
//!
//! 三种状态由 AppState.modal 决定：
//!   - HostFormState::Adding(draft) — 添加模式
//!   - HostFormState::Editing { id, draft } — 编辑模式
//!   - HostFormState::DeleteConfirm { id, label } — 删除确认
//!
//! 表单输入用 KeyDownEvent + key_char append（无真 TextField 控件）。
//! 用户 Tab 切字段，Enter 保存，Esc 取消，Backspace 删除最后一个字符。

use std::sync::Arc;

use gpui::{
    div, hsla, opaque_grey, prelude::*, px, rgb, App, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::persistence;
use crate::state::{AppState, HostFormDraft, HostFormState, SshEvent};

/// 当前 focus 的 input 字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Label,
    Host,
    Port,
    User,
    KeyPath,
}

impl FocusField {
    fn next(self) -> Self {
        match self {
            FocusField::Label => FocusField::Host,
            FocusField::Host => FocusField::Port,
            FocusField::Port => FocusField::User,
            FocusField::User => FocusField::KeyPath,
            FocusField::KeyPath => FocusField::Label,
        }
    }
}

pub struct HostFormModal {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    focus_handle: FocusHandle,
    focus_field: FocusField,
}

impl HostFormModal {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_this, _state, cx| cx.notify()).detach();
        Self {
            state,
            bridge,
            tx,
            focus_handle: cx.focus_handle(),
            focus_field: FocusField::Label,
        }
    }

    /// 把字符 append 到当前 focused 字段。
    fn append_char(&mut self, ch: char, cx: &mut Context<Self>) {
        let field = self.focus_field;
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                let target = match field {
                    FocusField::Label => &mut draft.label,
                    FocusField::Host => &mut draft.host,
                    FocusField::Port => &mut draft.port,
                    FocusField::User => &mut draft.user,
                    FocusField::KeyPath => &mut draft.key_path,
                };
                target.push(ch);
                draft.error = None;
                cx.notify();
            }
        });
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        let field = self.focus_field;
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                let target = match field {
                    FocusField::Label => &mut draft.label,
                    FocusField::Host => &mut draft.host,
                    FocusField::Port => &mut draft.port,
                    FocusField::User => &mut draft.user,
                    FocusField::KeyPath => &mut draft.key_path,
                };
                target.pop();
                draft.error = None;
                cx.notify();
            }
        });
    }

    fn cycle_focus(&mut self, cx: &mut Context<Self>) {
        self.focus_field = self.focus_field.next();
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.modal = None;
            cx.notify();
        });
    }

    /// 保存（添加 / 编辑 / 删除确认）。返回是否需要持久化。
    fn save(&mut self, cx: &mut Context<Self>) {
        let needs_persist = self.state.update(cx, |state, cx| {
            let modal = state.modal.take();
            match modal {
                Some(HostFormState::DeleteConfirm { id, .. }) => {
                    state.remove_host(id);
                    cx.notify();
                    true
                }
                Some(HostFormState::Adding(draft)) => match draft.into_config(None) {
                    Ok(cfg) => {
                        state.add_host(cfg);
                        cx.notify();
                        true
                    }
                    Err(err) => {
                        let mut new_draft = draft.clone();
                        new_draft.error = Some(err);
                        state.modal = Some(HostFormState::Adding(new_draft));
                        cx.notify();
                        false
                    }
                },
                Some(HostFormState::Editing { id, draft }) => match draft.into_config(Some(id)) {
                    Ok(cfg) => {
                        state.update_host(id, cfg);
                        cx.notify();
                        true
                    }
                    Err(err) => {
                        let mut new_draft = draft.clone();
                        new_draft.error = Some(err);
                        state.modal = Some(HostFormState::Editing {
                            id,
                            draft: new_draft,
                        });
                        cx.notify();
                        false
                    }
                },
                None => false,
            }
        });

        if needs_persist {
            let hosts = self.state.read(cx).hosts.clone();
            self.bridge.spawn(async move {
                if let Err(e) = persistence::save_hosts(&hosts) {
                    tracing::error!("save hosts.json failed: {}", e);
                }
            });
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.cancel(cx),
            "tab" => self.cycle_focus(cx),
            "enter" => self.save(cx),
            "backspace" => self.backspace(cx),
            _ => {
                // 优先使用 key_char（系统 IME / 布局感知字符），退回到 key 本身（长度==1时）
                if let Some(ch_str) = event.keystroke.key_char.as_deref() {
                    if let Some(ch) = ch_str.chars().next() {
                        self.append_char(ch, cx);
                    }
                } else if key.len() == 1 {
                    if let Some(ch) = key.chars().next() {
                        self.append_char(ch, cx);
                    }
                }
            }
        }
    }
}

impl Focusable for HostFormModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HostFormModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_field = self.focus_field;
        let modal_state = self.state.read(cx).modal.as_ref().map(|m| match m {
            HostFormState::Adding(d) => ("add", Some(d.clone()), None::<String>),
            HostFormState::Editing { draft, .. } => ("edit", Some(draft.clone()), None),
            HostFormState::DeleteConfirm { label, .. } => ("delete", None, Some(label.clone())),
        });

        // modal == None 不应出现（caller 负责检查），返回空 div
        let Some(ref kind) = modal_state else {
            return div().into_any_element();
        };

        let body: gpui::AnyElement = match kind {
            ("add", Some(draft), _) => render_form_body("添加 Host", draft, focus_field),
            ("edit", Some(draft), _) => render_form_body("编辑 Host", draft, focus_field),
            ("delete", _, Some(label)) => render_delete_body(label),
            _ => return div().into_any_element(),
        };

        let primary_text = if kind.0 == "delete" {
            "Delete (Enter)"
        } else {
            "Save (Enter)"
        };

        // 全屏半透明遮罩 + 居中 modal 卡片。
        // 使用 absolute + top_0 + left_0 + size_full 而非 inset_0（GPUI 无该 API）。
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _w, cx| this.handle_key(ev, cx)))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(hsla(0.0, 0.0, 0.0, 0.6))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(440.0))
                    .bg(rgb(0x252525))
                    .rounded_lg()
                    .border_1()
                    .border_color(opaque_grey(0.3, 1.0))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(body)
                    .child(render_buttons(primary_text)),
            )
            .into_any_element()
    }
}

fn render_form_body(
    title: &str,
    draft: &HostFormDraft,
    focus_field: FocusField,
) -> gpui::AnyElement {
    let title_str = title.to_string();
    let mut col = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(15.0))
                .child(title_str),
        )
        .child(field_row(
            "label",
            &draft.label,
            focus_field == FocusField::Label,
        ))
        .child(field_row(
            "host",
            &draft.host,
            focus_field == FocusField::Host,
        ))
        .child(field_row(
            "port",
            &draft.port,
            focus_field == FocusField::Port,
        ))
        .child(field_row(
            "user",
            &draft.user,
            focus_field == FocusField::User,
        ))
        .child(field_row(
            "key path",
            &draft.key_path,
            focus_field == FocusField::KeyPath,
        ));

    if let Some(err) = &draft.error {
        col = col.child(
            div()
                .text_color(rgb(0xff6666))
                .text_size(px(12.0))
                .child(err.clone()),
        );
    }

    col.child(
        div()
            .text_color(rgb(0x888888))
            .text_size(px(11.0))
            .child("Tab 切换字段，Enter 保存，Esc 取消"),
    )
    .into_any_element()
}

fn field_row(label: &str, value: &str, focused: bool) -> gpui::AnyElement {
    let display: SharedString = if value.is_empty() {
        SharedString::from("(空)")
    } else {
        SharedString::from(value.to_string())
    };
    let border_color = if focused {
        rgb(0x4a90e2)
    } else {
        rgb(0x444444)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(72.0))
                .text_color(rgb(0xaaaaaa))
                .text_size(px(13.0))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .px_2()
                .py_1()
                .bg(rgb(0x1d1d1d))
                .border_1()
                .border_color(border_color)
                .rounded_sm()
                .text_color(if value.is_empty() {
                    rgb(0x555555)
                } else {
                    rgb(0xeeeeee)
                })
                .text_size(px(13.0))
                .child(display),
        )
        .into_any_element()
}

fn render_delete_body(label: &str) -> gpui::AnyElement {
    let label_str = format!("将永久删除 host：{}", label);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xff6666))
                .text_size(px(15.0))
                .child("确认删除？"),
        )
        .child(
            div()
                .text_color(rgb(0xcccccc))
                .text_size(px(13.0))
                .child(label_str),
        )
        .child(
            div()
                .text_color(rgb(0x888888))
                .text_size(px(11.0))
                .child("Enter 确认删除，Esc 取消"),
        )
        .into_any_element()
}

fn render_buttons(primary_text: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .gap_2()
        .justify_end()
        .child(
            div()
                .px_4()
                .py_1()
                .bg(rgb(0x444444))
                .text_color(rgb(0xeeeeee))
                .rounded_sm()
                .child("Cancel (Esc)"),
        )
        .child(
            div()
                .px_4()
                .py_1()
                .bg(rgb(0x4a90e2))
                .text_color(rgb(0xffffff))
                .rounded_sm()
                .child(primary_text.to_string()),
        )
        .into_any_element()
}
