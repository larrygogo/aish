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

use aish_types::HostId;

use crate::bridge::Bridge;
use crate::persistence;
use crate::state::{AppState, HostFormDraft, HostFormState, SshEvent};

/// 当前 focus 的 input 字段。auth_kind == KeyFile 走 KeyPath；== Password 走 Password。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Label,
    Host,
    Port,
    User,
    KeyPath,
    Password,
}

impl FocusField {
    /// 给定当前 auth_kind，跳到下一个有效字段（跳过当前 auth 不需要的）。
    fn next(self, auth_kind: crate::state::AuthKind) -> Self {
        use crate::state::AuthKind;
        match (self, auth_kind) {
            (FocusField::Label, _) => FocusField::Host,
            (FocusField::Host, _) => FocusField::Port,
            (FocusField::Port, _) => FocusField::User,
            (FocusField::User, AuthKind::KeyFile) => FocusField::KeyPath,
            (FocusField::User, AuthKind::Password) => FocusField::Password,
            (FocusField::KeyPath, _) => FocusField::Label,
            (FocusField::Password, _) => FocusField::Label,
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
                    FocusField::Password => &mut draft.password,
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
                    FocusField::Password => &mut draft.password,
                };
                target.pop();
                draft.error = None;
                cx.notify();
            }
        });
    }

    fn cycle_focus(&mut self, cx: &mut Context<Self>) {
        // 取当前 modal 的 draft.auth_kind 决定 next() 跳到哪
        let auth_kind = self
            .state
            .read(cx)
            .modal
            .as_ref()
            .and_then(|m| match m {
                HostFormState::Adding(d) => Some(d.auth_kind),
                HostFormState::Editing { draft: d, .. } => Some(d.auth_kind),
                HostFormState::DeleteConfirm { .. } => None,
            })
            .unwrap_or(crate::state::AuthKind::KeyFile);
        self.focus_field = self.focus_field.next(auth_kind);
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
        // 把 modal 取出（同时清空），决定后续动作
        let action = self.state.update(cx, |state, _cx| state.modal.take());

        let needs_persist = match action {
            Some(HostFormState::DeleteConfirm { id, .. }) => {
                self.state.update(cx, |state, cx| {
                    state.remove_host(id);
                    cx.notify();
                });
                // 删 host 同步删 keyring（idempotent，NoEntry 不报错）
                crate::persistence::delete_secret_for(id);
                true
            }
            Some(HostFormState::Adding(draft)) => self.handle_add_or_edit(None, draft, cx),
            Some(HostFormState::Editing { id, draft }) => {
                self.handle_add_or_edit(Some(id), draft, cx)
            }
            None => false,
        };

        if needs_persist {
            let hosts = self.state.read(cx).hosts.clone();
            self.bridge.spawn(async move {
                if let Err(e) = persistence::save_hosts(&hosts) {
                    tracing::error!("save hosts.json failed: {}", e);
                }
            });
        }
    }

    /// 处理添加/编辑保存：校验失败重新塞回 modal 并显示红字。返回是否成功（需持久化）。
    fn handle_add_or_edit(
        &mut self,
        id: Option<HostId>,
        draft: HostFormDraft,
        cx: &mut Context<Self>,
    ) -> bool {
        match draft.into_config(id) {
            Ok(cfg) => {
                self.state.update(cx, |state, cx| {
                    if let Some(id) = id {
                        state.update_host(id, cfg);
                    } else {
                        state.add_host(cfg);
                    }
                    cx.notify();
                });
                true
            }
            Err(err) => {
                let mut new_draft = draft.clone();
                new_draft.error = Some(err);
                self.state.update(cx, |state, cx| {
                    state.modal = match id {
                        Some(id) => Some(HostFormState::Editing {
                            id,
                            draft: new_draft,
                        }),
                        None => Some(HostFormState::Adding(new_draft)),
                    };
                    cx.notify();
                });
                false
            }
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let ctrl = event.keystroke.modifiers.control;

        match key {
            "escape" => self.cancel(cx),
            "tab" => self.cycle_focus(cx),
            "enter" => self.save(cx),
            "backspace" => self.backspace(cx),
            // Ctrl+T: 切换 auth_kind
            "t" if ctrl => self.toggle_auth_kind(cx),
            // Ctrl+E: 切换 password_visible
            "e" if ctrl => self.toggle_password_visible(cx),
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

    /// 切换 auth_kind（KeyFile ↔ Password）。focus 重置到 Label 避免指向不可见字段。
    fn toggle_auth_kind(&mut self, cx: &mut Context<Self>) {
        use crate::state::AuthKind;
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                draft.auth_kind = match draft.auth_kind {
                    AuthKind::KeyFile => AuthKind::Password,
                    AuthKind::Password => AuthKind::KeyFile,
                };
                draft.error = None;
                cx.notify();
            }
        });
        self.focus_field = FocusField::Label;
    }

    /// 切换 password_visible（mask ↔ 明文）。
    fn toggle_password_visible(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(modal) = &mut state.modal {
                let draft = match modal {
                    HostFormState::Adding(d) | HostFormState::Editing { draft: d, .. } => d,
                    HostFormState::DeleteConfirm { .. } => return,
                };
                draft.password_visible = !draft.password_visible;
                cx.notify();
            }
        });
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
        ));

    // auth radio（当前选中: ● 否则 ○）— M2d 用 Ctrl+T 切换，无 mouse listener
    let auth_kind = draft.auth_kind;
    let kf_marker = if auth_kind == crate::state::AuthKind::KeyFile {
        "● 密钥"
    } else {
        "○ 密钥"
    };
    let pw_marker = if auth_kind == crate::state::AuthKind::Password {
        "● 密码"
    } else {
        "○ 密码"
    };
    col = col.child(
        div()
            .flex()
            .flex_row()
            .gap_4()
            .py_1()
            .child(
                div()
                    .px_2()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(13.0))
                    .child(kf_marker),
            )
            .child(
                div()
                    .px_2()
                    .text_color(rgb(0xeeeeee))
                    .text_size(px(13.0))
                    .child(pw_marker),
            ),
    );

    // 根据 auth_kind 显示 KeyPath 或 Password 字段
    use crate::state::AuthKind;
    col = match auth_kind {
        AuthKind::KeyFile => col.child(field_row(
            "key path",
            &draft.key_path,
            focus_field == FocusField::KeyPath,
        )),
        AuthKind::Password => col.child(password_field_row(
            &draft.password,
            draft.password_visible,
            focus_field == FocusField::Password,
        )),
    };

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
            .child("Tab 切换字段，Ctrl+T 切 auth 类型，Ctrl+E 切密码可见，Enter 保存，Esc 取消"),
    )
    .into_any_element()
}

/// 密码字段行：input(mask/明文) + 👁 toggle 图标。
/// 编辑模式下 password 为空时显示 placeholder「(unchanged) 输入新密码所换」。
fn password_field_row(password: &str, visible: bool, focused: bool) -> gpui::AnyElement {
    let display: SharedString = if password.is_empty() {
        SharedString::from("(unchanged) 输入新密码所换")
    } else if visible {
        SharedString::from(password.to_string())
    } else {
        SharedString::from("•".repeat(password.chars().count()))
    };
    let border_color = if focused {
        rgb(0x4a90e2)
    } else {
        rgb(0x444444)
    };
    let text_color = if password.is_empty() {
        rgb(0x555555)
    } else {
        rgb(0xeeeeee)
    };
    let eye = if visible { "👁" } else { "👁‍🗨" };
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
                .child("password"),
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
                .text_color(text_color)
                .text_size(px(13.0))
                .child(display),
        )
        .child(
            div()
                .px_2()
                .text_color(rgb(0xaaaaaa))
                .text_size(px(14.0))
                .child(eye),
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
