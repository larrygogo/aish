//! 添加 / 编辑 / 删除确认 modal。
//!
//! M12 重写为基于 `aish_ui::Dialog + Tabs + TextInput` 的组件化版本。
//!
//! 三种状态由 `AppState.modal: Option<HostFormState>` 决定：
//!   - `HostFormState::Adding(draft)` — 添加
//!   - `HostFormState::Editing { id, draft }` — 编辑
//!   - `HostFormState::DeleteConfirm { id, label }` — 删除确认
//!
//! HostFormModal observe state.modal 变化：
//!   - modal 从 None → Some：dialog.open + 把 draft 同步到 6 个 TextInput
//!   - modal 从 Some → None：dialog.close
//!
//! 保存 / 取消 / 删除业务方法不变，仅改为通过 `input.read(cx).text()` 取值。

use std::sync::Arc;

use aish_types::HostId;
use aish_ui::{theme, Button, Dialog, TextInput, TypographyExt};
use gpui::{
    div, prelude::*, App, Context, Entity, Focusable, IntoElement, MouseDownEvent,
    PathPromptOptions, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::persistence;
use crate::state::{AppState, AuthKind, HostFormDraft, HostFormState, SshEvent};

pub struct HostFormModal {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    #[allow(dead_code)]
    tx: tokio::sync::mpsc::Sender<SshEvent>,
    dialog: Entity<Dialog>,
    /// M29 D-3：auth 切换从 Tabs Entity → enum 字段。
    /// 默认 KeyFile（与 M12 Tabs 默认 active=0 等价）。
    auth_kind: AuthKind,
    label_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
    user_input: Entity<TextInput>,
    keyfile_input: Entity<TextInput>,
    password_input: Entity<TextInput>,
    /// 已 sync 过的 modal 镜像键（None / Adding / Editing(id) / DeleteConfirm(id)）。
    /// 用于检测 modal 切换，避免每帧都把 state.modal 的内容覆盖到 TextInput
    /// （否则用户输入会被覆盖回原值）。
    synced_key: SyncedKey,
    /// host 字段实时校验错误（None = 无错误 / 空）。on_change 时更新，
    /// render 时在 host_input 下方显示红色小字。空串不算错误（用户还没填完）。
    host_error: Option<&'static str>,
    port_error: Option<&'static str>,
}

/// host 字段实时校验。空 OK（等 save 时再报必填）；含空格 / 协议前缀 → 错误。
/// 不做严格 IP / FQDN regex，仅挡明显错误，避免过度阻挠用户输入。
fn validate_host(s: &str) -> Option<&'static str> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.contains(char::is_whitespace) {
        return Some("不能含空格");
    }
    if s.contains("://") {
        return Some("不要带协议前缀（如 ssh://）");
    }
    None
}

/// port 字段实时校验。空 OK（fallback 22）；非数字 / 越界 → 错误。
fn validate_port(s: &str) -> Option<&'static str> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    match s.parse::<u32>() {
        Err(_) => Some("必须是数字"),
        Ok(n) if !(1..=65535).contains(&n) => Some("范围 1 - 65535"),
        Ok(_) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncedKey {
    None,
    Adding,
    Editing(HostId),
    DeleteConfirm(HostId),
}

impl HostFormModal {
    pub fn new(
        state: Entity<AppState>,
        bridge: Arc<Bridge>,
        tx: tokio::sync::mpsc::Sender<SshEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |this, _state, cx| {
            this.sync_from_state(cx);
            cx.notify();
        })
        .detach();

        let dialog = cx.new(Dialog::new);
        let weak = cx.weak_entity();
        dialog.update(cx, move |d, _cx| {
            d.title("Host");
            d.width(gpui::px(480.0)); // M29 D-8: 460 → 480 让 label-on-top + Radio 更宽松
            d.on_close(move |_window, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.cancel(cx));
                }
            });
        });

        // M29 D-3: auth 切换从 Tabs Entity 改 enum 字段，初始 KeyFile

        let label_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("My Server");
            i
        });
        let host_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("example.com");
            i
        });
        let port_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("22");
            i
        });

        // 实时校验：on_change 时算 validate_host/port 结果写到 host_error/port_error
        // 字段，render 在 input 下方显示红色错误提示。
        let weak_h = cx.weak_entity();
        host_input.update(cx, |i, _cx| {
            i.on_change(move |text, _w, cx| {
                let err = validate_host(text);
                if let Some(this) = weak_h.upgrade() {
                    this.update(cx, |this, cx| {
                        if this.host_error != err {
                            this.host_error = err;
                            cx.notify();
                        }
                    });
                }
            });
        });
        let weak_p = cx.weak_entity();
        port_input.update(cx, |i, _cx| {
            i.on_change(move |text, _w, cx| {
                let err = validate_port(text);
                if let Some(this) = weak_p.upgrade() {
                    this.update(cx, |this, cx| {
                        if this.port_error != err {
                            this.port_error = err;
                            cx.notify();
                        }
                    });
                }
            });
        });
        let user_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("root");
            i
        });
        let keyfile_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("~/.ssh/id_rsa");
            i
        });
        let password_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("password")
                .mask_char(Some('•'))
                .show_mask_toggle(true);
            i
        });

        Self {
            state,
            bridge,
            tx,
            dialog,
            auth_kind: AuthKind::KeyFile,
            label_input,
            host_input,
            port_input,
            user_input,
            keyfile_input,
            password_input,
            synced_key: SyncedKey::None,
            host_error: None,
            port_error: None,
        }
    }

    /// 根据 state.modal 的当前值，同步 dialog 显隐 + 把 draft 内容塞到 6 个 input。
    /// 用 `synced_key` 防止每帧 observe 都覆盖用户输入。
    fn sync_from_state(&mut self, cx: &mut Context<Self>) {
        let current = self.state.read(cx).modal.as_ref().map(|m| match m {
            HostFormState::Adding(_) => SyncedKey::Adding,
            HostFormState::Editing { id, .. } => SyncedKey::Editing(*id),
            HostFormState::DeleteConfirm { id, .. } => SyncedKey::DeleteConfirm(*id),
        });

        match (self.synced_key, current) {
            // modal 关闭：dialog 也关
            (_, None) => {
                if self.synced_key != SyncedKey::None {
                    self.synced_key = SyncedKey::None;
                    self.dialog.update(cx, |d, cx| d.close(cx));
                }
            }
            // modal 切换：dialog open + 把 draft 内容同步到 input + 注册
            // focus_chain（Tab 在 5 个 input 之间循环；keyfile 与 password
            // 互斥取决于 auth_tabs，但都加进 chain 也无害 —— 隐藏的 input
            // 也持 FocusHandle，Tab 跳到时再切回可见的也可以接受）
            (prev, Some(next)) if prev != next => {
                self.synced_key = next;
                self.fill_inputs_from_modal(cx);
                let chain = vec![
                    self.label_input.read(cx).focus_handle(cx),
                    self.host_input.read(cx).focus_handle(cx),
                    self.port_input.read(cx).focus_handle(cx),
                    self.user_input.read(cx).focus_handle(cx),
                    self.keyfile_input.read(cx).focus_handle(cx),
                    self.password_input.read(cx).focus_handle(cx),
                ];
                self.dialog.update(cx, |d, cx| {
                    d.focus_chain(chain);
                    d.open(cx);
                });
            }
            // 同 key 不动（用户正在编辑，避免覆盖输入）
            _ => {}
        }
    }

    /// 把 state.modal 内的 draft 字段塞到 6 个 TextInput + 切换 auth_tabs。
    fn fill_inputs_from_modal(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.state.read(cx).modal.as_ref().and_then(|m| match m {
            HostFormState::Adding(d) => Some(d.clone()),
            HostFormState::Editing { draft, .. } => Some(draft.clone()),
            HostFormState::DeleteConfirm { .. } => None,
        });

        if let Some(draft) = snapshot {
            let label = draft.label.clone();
            let host = draft.host.clone();
            let port = if draft.port.is_empty() {
                "22".into()
            } else {
                draft.port.clone()
            };
            let user = if draft.user.is_empty() {
                "root".into()
            } else {
                draft.user.clone()
            };
            let key_path = draft.key_path.clone();
            let auth_kind = draft.auth_kind;

            self.label_input.update(cx, |i, cx| i.set_text(label, cx));
            self.host_input.update(cx, |i, cx| i.set_text(host, cx));
            self.port_input.update(cx, |i, cx| i.set_text(port, cx));
            self.user_input.update(cx, |i, cx| i.set_text(user, cx));
            self.keyfile_input
                .update(cx, |i, cx| i.set_text(key_path, cx));
            self.password_input.update(cx, |i, cx| i.clear(cx));

            // M29 D-3：直接 set enum 字段（之前 set Tabs Entity active idx）
            self.auth_kind = auth_kind;
        }
        // DeleteConfirm 不需要 input 内容
    }

    /// 从 6 个 input + auth_kind 拼出 HostFormDraft 用于 save。
    fn collect_draft(&self, cx: &App) -> HostFormDraft {
        let auth_kind = self.auth_kind;
        HostFormDraft {
            label: self.label_input.read(cx).text().to_string(),
            host: self.host_input.read(cx).text().to_string(),
            port: self.port_input.read(cx).text().to_string(),
            user: self.user_input.read(cx).text().to_string(),
            auth_kind,
            key_path: self.keyfile_input.read(cx).text().to_string(),
            password: self.password_input.read(cx).text().to_string(),
            password_visible: false,
            error: None,
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.modal = None;
            cx.notify();
        });
    }

    /// 保存（添加 / 编辑 / 删除确认）。
    fn save(&mut self, cx: &mut Context<Self>) {
        // 先确定 modal 类型（不取走，校验失败时仍要更新原 modal 的 error 字段）
        let kind = self.state.read(cx).modal.as_ref().map(|m| match m {
            HostFormState::Adding(_) => SyncedKey::Adding,
            HostFormState::Editing { id, .. } => SyncedKey::Editing(*id),
            HostFormState::DeleteConfirm { id, .. } => SyncedKey::DeleteConfirm(*id),
        });

        let needs_persist = match kind {
            Some(SyncedKey::DeleteConfirm(id)) => {
                self.state.update(cx, |state, cx| {
                    state.remove_host(id);
                    state.modal = None;
                    cx.notify();
                });
                crate::persistence::delete_secret_for(id);
                true
            }
            Some(SyncedKey::Adding) => {
                let draft = self.collect_draft(cx);
                self.handle_add_or_edit(None, draft, cx)
            }
            Some(SyncedKey::Editing(id)) => {
                let draft = self.collect_draft(cx);
                self.handle_add_or_edit(Some(id), draft, cx)
            }
            _ => false,
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

    /// 校验失败时塞回 modal（含 error）。返回是否成功（需持久化）。
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
                    state.modal = None;
                    cx.notify();
                });
                true
            }
            Err(err) => {
                let mut new_draft = draft;
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

    fn pick_keyfile(&mut self, cx: &mut Context<Self>) {
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from("选择 SSH 密钥文件")),
        };
        let receiver = cx.prompt_for_paths(options);
        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    this.update(cx, |this, cx| {
                        let s = path.to_string_lossy().to_string();
                        this.keyfile_input.update(cx, |i, cx| i.set_text(s, cx));
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

impl Render for HostFormModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal_kind = self.state.read(cx).modal.as_ref().map(|m| match m {
            HostFormState::Adding(d) => ("add", Some(d.error.clone()), None::<String>),
            HostFormState::Editing { draft, .. } => ("edit", Some(draft.error.clone()), None),
            HostFormState::DeleteConfirm { label, .. } => ("delete", None, Some(label.clone())),
        });

        let Some((kind, err_opt, label_opt)) = modal_kind else {
            // modal 为 None，dialog 也已通过 observe 关闭
            return self.dialog.clone().into_any_element();
        };

        // 提前拷贝 token，避免 theme(cx) 的不可变借用跨整个 render（与下面
        // keyfile_row(cx)/buttons_row(cx) 的可变借用冲突）。
        //
        // M26 note: host_form 整 render 内大量 cx mut borrow chain
        // (keyfile_row / buttons_row / cx.listener 等)，无法稳定持 &Theme，
        // 因此本 view 内文字仍 inline `.text_size + .text_color`，等价 typography
        // role (Body/Caption/Label) 但不走 .typography() ext。
        let (colors, font_size, spacing, form_field_gap) = {
            let t = theme(cx);
            (t.colors, t.font_size, t.spacing, t.anatomy.form.field_gap)
        };
        // M29 D-3：auth 切换从 Tabs idx → enum 字段
        let auth_kind = self.auth_kind;
        let is_edit = kind == "edit";
        let is_delete = kind == "delete";
        let title = match kind {
            "add" => "添加 Host",
            "edit" => "编辑 Host",
            "delete" => "确认删除",
            _ => "Host",
        };
        let primary_label = if is_delete { "Delete" } else { "Save" };

        let body: gpui::AnyElement = if is_delete {
            let label = label_opt.unwrap_or_default();
            div()
                .flex()
                .flex_col()
                .gap(spacing.px_3)
                .child(
                    // M26 等价 Body (13/400/fg)
                    div()
                        .text_size(font_size.sm)
                        .text_color(colors.foreground)
                        .child(format!("将永久删除 host：{}", label)),
                )
                .child(
                    // M26 等价 Caption (12/400/muted)
                    div()
                        .text_size(font_size.xs)
                        .text_color(colors.muted_foreground)
                        .child("Enter 确认 · Esc 取消"),
                )
                // delete confirm dialog 的 primary 是 Delete，不受实时校验影响
                .child(buttons_row(primary_label, true, false, cx))
                .into_any_element()
        } else {
            let err = err_opt.flatten();
            // Save 按钮 disabled 联动实时校验：host/port 任一有 inline error
            // 时禁用，避免用户带着错误 submit。空字段不算 error（validator 空 OK），
            // 进入 save() 时由 draft.into_config 报"必填"，所以空白态保持可点 Save。
            let save_disabled = self.host_error.is_some() || self.port_error.is_some();
            // M29 D-7: 把 host_error / port_error 联动到 input.error(bool)
            // 视觉，让红 border 提示用户哪个 input 错。
            let host_err_active = self.host_error.is_some();
            let port_err_active = self.port_error.is_some();
            self.host_input.update(cx, |i, _| {
                i.error(host_err_active);
            });
            self.port_input.update(cx, |i, _| {
                i.error(port_err_active);
            });
            // M29 D-2: 字段 gap 12（anatomy.form.field_gap）替代 spacing.px_3
            // 等价值 12 但语义清晰
            div()
                .flex()
                .flex_col()
                .gap(form_field_gap)
                .child(field_row(cx, "label", self.label_input.clone(), None))
                .child(field_row(
                    cx,
                    "host",
                    self.host_input.clone(),
                    self.host_error,
                ))
                .child(field_row(
                    cx,
                    "port",
                    self.port_input.clone(),
                    self.port_error,
                ))
                .child(field_row(cx, "user", self.user_input.clone(), None))
                // M29 D-3: Radio 横排替代 Tabs Entity
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(spacing.px_3)
                        .child(
                            aish_ui::Radio::new("host-form-auth-keyfile")
                                .label("Key File")
                                .checked(matches!(auth_kind, AuthKind::KeyFile))
                                .on_click(cx.listener(
                                    |this, _ev: &MouseDownEvent, _w, cx| {
                                        this.auth_kind = AuthKind::KeyFile;
                                        cx.notify();
                                    },
                                )),
                        )
                        .child(
                            aish_ui::Radio::new("host-form-auth-password")
                                .label("Password")
                                .checked(matches!(auth_kind, AuthKind::Password))
                                .on_click(cx.listener(
                                    |this, _ev: &MouseDownEvent, _w, cx| {
                                        this.auth_kind = AuthKind::Password;
                                        cx.notify();
                                    },
                                )),
                        ),
                )
                .child(match auth_kind {
                    AuthKind::KeyFile => {
                        let kf = self.keyfile_input.clone();
                        keyfile_row(kf, cx).into_any_element()
                    }
                    AuthKind::Password => {
                        field_row(cx, "password", self.password_input.clone(), None)
                            .into_any_element()
                    }
                })
                .when_some(err, |d, e| {
                    d.child(
                        // M26 等价 Body + destructive
                        div()
                            .text_size(font_size.sm)
                            .text_color(colors.destructive)
                            .child(e),
                    )
                })
                .child(buttons_row(primary_label, is_edit, save_disabled, cx))
                .into_any_element()
        };

        self.dialog.update(cx, |d, _cx| {
            d.title(title);
            d.body(body);
        });

        self.dialog.clone().into_any_element()
    }
}

/// M29 D-1: field_row 改 label-on-top layout（之前 label 80px 左栅格 +
/// secondary_fg 弱化得像 placeholder；现在 label 显眼，input 占满宽，
/// inline error 与 input 同列对齐）。
///
/// anatomy:
/// - 字段块 flex_col + inline_gap 6（label↔input）+ inline_gap 4（input↔error）
/// - label: Label role (13/500/fg)，**不再** secondary_fg override
/// - error: Body role (13/400) + destructive override（之前 Caption 12/muted
///   太弱，看不清出错）
fn field_row(
    cx: &App,
    label: &'static str,
    input: Entity<TextInput>,
    error: Option<&'static str>,
) -> impl IntoElement {
    let t = theme(cx);
    div()
        .flex()
        .flex_col()
        .gap(t.anatomy.form.inline_gap) // label↔input 6
        .child(
            div()
                .typography(aish_ui::TypeRole::Label, t)
                .child(label),
        )
        .child(input)
        .when_some(error, |d, msg| {
            d.child(
                div()
                    .typography(aish_ui::TypeRole::Body, t)
                    .text_color(t.colors.destructive)
                    .child(msg),
            )
        })
}

fn keyfile_row(
    keyfile_input: Entity<TextInput>,
    cx: &mut Context<HostFormModal>,
) -> impl IntoElement {
    // M29 D-1: label-on-top + input/picker 横排
    let (inline_gap, label_gap) = {
        let t = theme(cx);
        (t.anatomy.form.inline_gap, t.spacing.px_2)
    };
    div()
        .flex()
        .flex_col()
        .gap(inline_gap)
        .child({
            let t = theme(cx);
            div()
                .typography(aish_ui::TypeRole::Label, t)
                .child("key path")
        })
        .child(
            // input + picker button 横排
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(label_gap)
                .child(div().flex_1().child(keyfile_input))
                .child(
                    Button::new("pick-keyfile")
                        .label("…")
                        .secondary()
                        .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                            this.pick_keyfile(cx);
                        })),
                ),
        )
}

fn buttons_row(
    primary_label: &'static str,
    show_delete: bool,
    save_disabled: bool,
    cx: &mut Context<HostFormModal>,
) -> impl IntoElement {
    let spacing_px_2 = {
        let t = theme(cx);
        t.spacing.px_2
    };
    // 删 footer 内 "Cancel" 按钮 —— Dialog 顶部已有 X 关闭 + Esc 关闭，重复
    // 一个 Cancel 让 footer 视觉拥挤且语义重复。保留 Delete + 主操作（Save /
    // Add / Connect），用户用 X / Esc 取消（与系统 modal 习惯一致）。
    let mut row = div().flex().flex_row().gap(spacing_px_2).justify_end();
    if show_delete && primary_label != "Delete" {
        // edit 模式额外加一个 Delete 按钮（DeleteConfirm 通过 home.rs 单独触发）
        row = row.child(
            Button::new("host-delete")
                .label("Delete")
                .destructive()
                .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                    // 从 Editing 切到 DeleteConfirm
                    this.state.update(cx, |s, cx| {
                        if let Some(HostFormState::Editing { id, draft }) = &s.modal {
                            s.modal = Some(HostFormState::DeleteConfirm {
                                id: *id,
                                label: draft.label.clone(),
                            });
                            cx.notify();
                        }
                    });
                })),
        );
    }
    row.child(
        Button::new("host-save")
            .label(primary_label)
            .primary()
            .disabled(save_disabled)
            .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                this.save(cx);
            })),
    )
}
