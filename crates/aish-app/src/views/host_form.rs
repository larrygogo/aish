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
    div, prelude::*, AnyElement, App, Context, Entity, Focusable, IntoElement, MouseDownEvent,
    PathPromptOptions, SharedString, Window,
};

use crate::bridge::Bridge;
use crate::persistence;
use crate::state::{AppState, AuthKind, HostFormDraft, HostFormState};

pub struct HostFormModal {
    state: Entity<AppState>,
    bridge: Arc<Bridge>,
    dialog: Entity<Dialog>,
    /// M29 D-6：delete confirm 拆独立 dialog（380 窄 + destructive 视觉）。
    /// 与 add/edit dialog 独立 open/close，避免共用 body 时分支爆炸。
    delete_dialog: Entity<Dialog>,
    /// M29 D-9：delete_dialog 默认 focus 的 Cancel button focus handle。
    /// M31：直接从 delete_cancel_btn entity 的 focus_handle() 取（每次 sync
    /// 时调），不再独立维护 FocusHandle 字段 — entity 已内置且唯一。
    /// M31：6 个 button 升 stateful entity，全部带 press feedback。
    delete_cancel_btn: Entity<Button>,
    delete_confirm_btn: Entity<Button>,
    pick_keyfile_btn: Entity<Button>,
    host_delete_btn: Entity<Button>,
    host_cancel_btn: Entity<Button>,
    host_save_btn: Entity<Button>,
    /// M29 D-3：auth 切换从 Tabs Entity → enum 字段。
    /// 默认 KeyFile（与 M12 Tabs 默认 active=0 等价）。
    auth_kind: AuthKind,
    /// M35 T10: 单行 `user@host:port` 快速输入。on_change 时 parse 成功就
    /// 自动 mirror 写入下方 4 个 TextInput（user / host / port）；解析失败
    /// 不报错，用户可继续在 4 字段填。
    connection_input: Entity<TextInput>,
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

/// M35 T10: 解析单行 SSH 连接字符串 `user@host:port`。
///
/// 接受格式：
/// - `user@host`          → `(user, host, "22")`
/// - `user@host:port`     → `(user, host, port)`
/// - `host`               → `("", host, "22")` (无 user — caller 自决填默认值)
/// - `host:port`          → `("", host, port)`
/// - `user@[::1]:22`      → IPv6 带方括号格式
///
/// 失败（空 / 含空格 / 含多 @ / port 非数字或越界 / host 为空）→ `None`，
/// caller 不报错，让用户继续在 4 字段表单填。
pub(crate) fn parse_connection_string(s: &str) -> Option<(String, String, String)> {
    let s = s.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        return None;
    }
    // 1. 拆 user@rest（rest 不应再含 @）
    let (user, rest): (String, String) = match s.find('@') {
        Some(idx) => {
            let (u, r) = s.split_at(idx);
            (u.to_string(), r[1..].to_string())
        }
        None => (String::new(), s.to_string()),
    };
    if rest.contains('@') {
        return None;
    }

    // 2. 拆 host:port — IPv6 [::1]:port 优先识别方括号
    let (host, port) = if let Some(stripped) = rest.strip_prefix('[') {
        let end_bracket = stripped.find(']')?;
        let host = format!("[{}]", &stripped[..end_bracket]);
        let after = &stripped[end_bracket + 1..];
        if after.is_empty() {
            (host, "22".to_string())
        } else {
            let p = after.strip_prefix(':')?;
            match p.parse::<u32>() {
                Ok(n) if (1..=65535).contains(&n) => (host, p.to_string()),
                _ => return None,
            }
        }
    } else {
        // 非 IPv6：rfind ':' 取最后一个分隔点 host vs port
        match rest.rfind(':') {
            Some(idx) => {
                let (h, p) = rest.split_at(idx);
                let p_str = &p[1..];
                match p_str.parse::<u32>() {
                    Ok(n) if (1..=65535).contains(&n) => (h.to_string(), p_str.to_string()),
                    _ => return None,
                }
            }
            None => (rest, "22".to_string()),
        }
    };

    if host.is_empty() || host == "[]" {
        return None;
    }
    Some((user, host, port))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncedKey {
    None,
    Adding,
    Editing(HostId),
    DeleteConfirm(HostId),
}

impl HostFormModal {
    pub fn new(state: Entity<AppState>, bridge: Arc<Bridge>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, _state, cx| {
            this.sync_from_state(cx);
            cx.notify();
        })
        .detach();

        let dialog = cx.new(Dialog::new);
        let weak = cx.weak_entity();
        dialog.update(cx, move |d, _cx| {
            d.title("主机");
            d.width(gpui::px(480.0)); // M29 D-8: 460 → 480 让 label-on-top + Radio 更宽松
            d.on_close(move |_window, cx| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |this, cx| this.cancel(cx));
                }
            });
        });

        // M29 D-6: delete confirm 独立 dialog（380 窄 + 标题改 "删除 Host?"）
        let delete_dialog = cx.new(Dialog::new);
        let weak_del = cx.weak_entity();
        delete_dialog.update(cx, move |d, _cx| {
            d.title("删除主机？");
            d.width(gpui::px(380.0));
            d.on_close(move |_window, cx| {
                if let Some(this) = weak_del.upgrade() {
                    this.update(cx, |this, cx| this.cancel(cx));
                }
            });
        });
        // M29 D-3: auth 切换从 Tabs Entity 改 enum 字段，初始 KeyFile

        // M35 T10: 顶部「快速输入」字段，user@host:port 单行格式
        let connection_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("user@host:port");
            i
        });
        let label_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("我的服务器（可选）");
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

        // M35 T10: connection_input on_change → parse + auto-fill user/host/port
        let weak_conn = cx.weak_entity();
        connection_input.update(cx, |i, _cx| {
            i.on_change(move |text, _w, cx| {
                if let Some((user, host, port)) = parse_connection_string(text) {
                    if let Some(this) = weak_conn.upgrade() {
                        this.update(cx, |this, cx| {
                            // 仅在 parse 成功时填字段，不覆盖非空字段以外的内容；
                            // 但用户预期是「typed user@host:port 后字段自动填」，
                            // 所以强制覆盖（避免半残）。
                            this.user_input.update(cx, |i, cx| i.set_text(user, cx));
                            this.host_input.update(cx, |i, cx| i.set_text(host, cx));
                            this.port_input.update(cx, |i, cx| i.set_text(port, cx));
                            // 清错误状态（已 valid 不该残留 host_error / port_error）
                            this.host_error = None;
                            this.port_error = None;
                            cx.notify();
                        });
                    }
                }
                // parse 失败：不报错 / 不清字段，用户继续在 4 字段填
            });
        });
        let keyfile_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("~/.ssh/id_rsa");
            i
        });
        let password_input = cx.new(|cx| {
            let mut i = TextInput::new(cx);
            i.placeholder("密码")
                .mask_char(Some('•'))
                .show_mask_toggle(true);
            i
        });

        // M31: 6 个 button entity，weak.upgrade callback 透传 self method
        let weak_dc = cx.weak_entity();
        let delete_cancel_btn = cx.new(|cx| {
            let mut b = Button::new("delete-cancel", cx);
            b.label("取消").on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_dc.upgrade() {
                    this.update(cx, |this, cx| this.cancel(cx));
                }
            });
            b
        });
        let weak_df = cx.weak_entity();
        let delete_confirm_btn = cx.new(|cx| {
            let mut b = Button::new("delete-confirm", cx);
            b.label("删除").destructive().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_df.upgrade() {
                    this.update(cx, |this, cx| this.save(cx));
                }
            });
            b
        });
        let weak_pk = cx.weak_entity();
        let pick_keyfile_btn = cx.new(|cx| {
            let mut b = Button::new("pick-keyfile", cx);
            b.label("…").secondary().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_pk.upgrade() {
                    this.update(cx, |this, cx| this.pick_keyfile(cx));
                }
            });
            b
        });
        let weak_hd = cx.weak_entity();
        let host_delete_btn = cx.new(|cx| {
            let mut b = Button::new("host-delete", cx);
            b.label("删除").destructive().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_hd.upgrade() {
                    this.update(cx, |this, cx| {
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
                    });
                }
            });
            b
        });
        let weak_hc = cx.weak_entity();
        let host_cancel_btn = cx.new(|cx| {
            let mut b = Button::new("host-cancel", cx);
            b.label("取消").on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_hc.upgrade() {
                    this.update(cx, |this, cx| this.cancel(cx));
                }
            });
            b
        });
        let weak_hs = cx.weak_entity();
        let host_save_btn = cx.new(|cx| {
            let mut b = Button::new("host-save", cx);
            b.label("保存").primary().on_click(move |_ev, _w, cx| {
                if let Some(this) = weak_hs.upgrade() {
                    this.update(cx, |this, cx| this.save(cx));
                }
            });
            b
        });

        Self {
            state,
            bridge,
            dialog,
            delete_dialog,
            delete_cancel_btn,
            delete_confirm_btn,
            pick_keyfile_btn,
            host_delete_btn,
            host_cancel_btn,
            host_save_btn,
            auth_kind: AuthKind::KeyFile,
            connection_input,
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
            // modal 关闭：两个 dialog 都关（R5 防双 open）
            (_, None) if self.synced_key != SyncedKey::None => {
                self.synced_key = SyncedKey::None;
                self.dialog.update(cx, |d, cx| d.close(cx));
                self.delete_dialog.update(cx, |d, cx| d.close(cx));
            }
            // modal 切换：根据 next 决定 open 哪个 dialog，close 另一个（R5）
            (prev, Some(next)) if prev != next => {
                self.synced_key = next;
                match next {
                    SyncedKey::DeleteConfirm(_) => {
                        // 切到 delete confirm：先关 add/edit dialog，再开 delete_dialog
                        // M29 D-9 / R10：initial_focus 给 Cancel button，
                        // Enter 触发 Cancel 而非 删除（避免误删）
                        // M31：cancel button focus_handle 直接从 entity 取
                        self.dialog.update(cx, |d, cx| d.close(cx));
                        let cancel_fh = self.delete_cancel_btn.read(cx).focus_handle();
                        self.delete_dialog.update(cx, |d, cx| {
                            d.initial_focus(cancel_fh);
                            d.open(cx);
                        });
                    }
                    SyncedKey::Adding | SyncedKey::Editing(_) => {
                        // 切到 add/edit：关 delete_dialog（edit → Delete 路径），
                        // 同步 input + focus_chain，再开 dialog
                        // M29 D-9：initial_focus 给 label_input，open 后 cursor
                        // 立即闪在 label 字段（无需用户点一下才能输入）
                        self.delete_dialog.update(cx, |d, cx| d.close(cx));
                        self.fill_inputs_from_modal(cx);
                        let label_fh = self.label_input.read(cx).focus_handle(cx);
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
                            d.initial_focus(label_fh);
                            d.open(cx);
                        });
                    }
                    SyncedKey::None => {
                        // 不会发生（None 已在上面分支处理）
                    }
                }
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

            // M35 T10: connection_input 在每次 modal 重新打开时清空（add /
            // edit 模式都从空白起步，避免上次内容残留）
            self.connection_input.update(cx, |i, cx| i.clear(cx));
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

impl HostFormModal {
    /// 构造 delete_dialog 的 body（label / 提示 / Cancel + 删除）。
    /// M29 D-6：从 add/edit dialog 的 body 分支拆出，独立 dialog 380 窄。
    /// M31：Cancel + 删除 button 走 entity clone，press feedback 自动生效。
    fn build_delete_body(&self, label: String, cx: &mut Context<Self>) -> AnyElement {
        let (colors, font_size, spacing) = {
            let t = theme(cx);
            (t.colors, t.font_size, t.spacing)
        };
        div()
            .flex()
            .flex_col()
            .gap(spacing.px_3)
            .child(
                // Body (13/400/fg)：主提示文案
                div()
                    .text_size(font_size.sm)
                    .text_color(colors.foreground)
                    .child(format!("将永久删除 \"{}\"，此操作不可撤销。", label)),
            )
            .child(
                // Caption (12/400/muted)：键盘提示
                div()
                    .text_size(font_size.xs)
                    .text_color(colors.muted_foreground)
                    .child("Esc 取消"),
            )
            .child(
                // footer：Cancel（focus 默认 — dialog.initial_focus 已 set） + 删除
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(spacing.px_2)
                    .child(self.delete_cancel_btn.clone())
                    .child(self.delete_confirm_btn.clone()),
            )
            .into_any_element()
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
            // modal 为 None — 两个 dialog 都已通过 observe 关闭，渲染 add/edit
            // 的 close 状态（任一空 dialog 都行，dialog.render 在 !open 时返回
            // 空 div）
            return self.dialog.clone().into_any_element();
        };

        // M29 D-6：delete 走独立 dialog 分支
        if kind == "delete" {
            let label = label_opt.unwrap_or_default();
            let body = self.build_delete_body(label, cx);
            self.delete_dialog.update(cx, |d, _cx| {
                d.body(body);
            });
            return self.delete_dialog.clone().into_any_element();
        }

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
        let title = match kind {
            "add" => "添加 Host",
            "edit" => "编辑 Host",
            _ => "Host",
        };
        let primary_label = "Save";

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
        let body: gpui::AnyElement = div()
            .flex()
            .flex_col()
            .gap(form_field_gap)
            // M35 T10: 顶部「快速输入」字段 — user@host:port 一行直填 4 字段
            .child(field_row(
                cx,
                "快速输入",
                self.connection_input.clone(),
                None,
            ))
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
                            .label("私钥文件")
                            .checked(matches!(auth_kind, AuthKind::KeyFile))
                            .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                                this.auth_kind = AuthKind::KeyFile;
                                cx.notify();
                            })),
                    )
                    .child(
                        aish_ui::Radio::new("host-form-auth-password")
                            .label("密码")
                            .checked(matches!(auth_kind, AuthKind::Password))
                            .on_click(cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                                this.auth_kind = AuthKind::Password;
                                cx.notify();
                            })),
                    ),
            )
            .child(match auth_kind {
                AuthKind::KeyFile => {
                    // KeyFile 模式：keyfile path picker + 可选 passphrase 输入
                    // passphrase 复用 password_input entity（已含 mask + show_toggle），
                    // runtime 切 placeholder 让 UI 语义明确（passphrase ≠ password）
                    self.password_input.update(cx, |i, _| {
                        i.placeholder("密钥短语（可选，用于加密的私钥）");
                    });
                    let kf = self.keyfile_input.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(form_field_gap)
                        .child(keyfile_row(kf, self.pick_keyfile_btn.clone(), cx))
                        .child(field_row(
                            cx,
                            "密钥短语",
                            self.password_input.clone(),
                            None,
                        ))
                        .into_any_element()
                }
                AuthKind::Password => {
                    self.password_input.update(cx, |i, _| {
                        i.placeholder("密码");
                    });
                    field_row(cx, "密码", self.password_input.clone(), None)
                        .into_any_element()
                }
            })
            // M35 T10: label 字段移到底部（90% 用户不填，可选项不该占首位）
            .child(field_row(
                cx,
                "显示名（可选）",
                self.label_input.clone(),
                None,
            ))
            .when_some(err, |d, e| {
                d.child(
                    // M26 等价 Body + destructive
                    div()
                        .text_size(font_size.sm)
                        .text_color(colors.destructive)
                        .child(e),
                )
            })
            .child(buttons_row(
                primary_label,
                is_edit,
                save_disabled,
                self.host_delete_btn.clone(),
                self.host_cancel_btn.clone(),
                self.host_save_btn.clone(),
                cx,
            ))
            .into_any_element();

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
    pick_btn: Entity<Button>,
    cx: &mut Context<HostFormModal>,
) -> impl IntoElement {
    // M29 D-1: label-on-top + input/picker 横排
    // M31：pick_btn 走 entity，press feedback 自动生效
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
                .child("私钥路径")
        })
        .child(
            // input + picker button 横排
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(label_gap)
                .child(div().flex_1().child(keyfile_input))
                .child(pick_btn),
        )
}

/// M29 D-7：footer 两端对齐 + border-top。
///
/// 布局：
/// - 顶部 border + pt（与 body 视觉切割）+ mt（与 body 间距）
/// - justify_between：左 [Delete or 占位] / 右 [Cancel] [Save]
/// - edit 模式 show_delete=true 显示左侧 Delete（destructive），其余空 div 占位
/// - footer 内 Cancel 回归 — delete dialog footer 已有 Cancel，对齐两 dialog
///   的语义；用户也可以靠 Esc 关闭，但 Cancel 按钮让"取消"更显眼。
fn buttons_row(
    primary_label: &'static str,
    show_delete: bool,
    save_disabled: bool,
    delete_btn: Entity<Button>,
    cancel_btn: Entity<Button>,
    save_btn: Entity<Button>,
    cx: &mut Context<HostFormModal>,
) -> impl IntoElement {
    // M31: save_btn label / disabled 状态每帧 update（其他 button 静态配置在 new() 已 set）
    save_btn.update(cx, |b, _| {
        b.label(primary_label).disabled(save_disabled);
    });
    let (spacing_px_2, spacing_px_3, border_color, form_footer_gap) = {
        let t = theme(cx);
        (
            t.spacing.px_2,
            t.spacing.px_3,
            t.colors.border,
            t.anatomy.form.footer_gap,
        )
    };

    // 左侧：edit 模式 Delete，其它情况空 div（保持 justify_between 布局占位）
    let left: AnyElement = if show_delete {
        delete_btn.into_any_element()
    } else {
        div().into_any_element()
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_color(border_color)
        .pt(spacing_px_3)
        .mt(spacing_px_3)
        .gap(form_footer_gap)
        .child(left)
        .child(
            div()
                .flex()
                .flex_row()
                .gap(spacing_px_2)
                .child(cancel_btn)
                .child(save_btn),
        )
}

#[cfg(test)]
mod tests {
    use super::parse_connection_string;

    #[test]
    fn parse_typical_user_host_port() {
        assert_eq!(
            parse_connection_string("larry@1.2.3.4:22"),
            Some(("larry".into(), "1.2.3.4".into(), "22".into()))
        );
    }

    #[test]
    fn parse_user_host_default_port() {
        assert_eq!(
            parse_connection_string("larry@example.com"),
            Some(("larry".into(), "example.com".into(), "22".into()))
        );
    }

    #[test]
    fn parse_host_only_no_user() {
        assert_eq!(
            parse_connection_string("example.com"),
            Some((String::new(), "example.com".into(), "22".into()))
        );
    }

    #[test]
    fn parse_host_with_port_no_user() {
        assert_eq!(
            parse_connection_string("example.com:2222"),
            Some((String::new(), "example.com".into(), "2222".into()))
        );
    }

    #[test]
    fn parse_user_with_dashes() {
        // 含 - 的 user / host 是合法（reverse-game 等典型 user 名）
        assert_eq!(
            parse_connection_string("my-user@my-host.com:22"),
            Some(("my-user".into(), "my-host.com".into(), "22".into()))
        );
    }

    #[test]
    fn parse_ipv6_with_port() {
        assert_eq!(
            parse_connection_string("user@[::1]:22"),
            Some(("user".into(), "[::1]".into(), "22".into()))
        );
    }

    #[test]
    fn parse_empty_returns_none() {
        assert_eq!(parse_connection_string(""), None);
        assert_eq!(parse_connection_string("   "), None);
    }

    #[test]
    fn parse_invalid_port_returns_none() {
        assert_eq!(parse_connection_string("user@host:99999"), None);
        assert_eq!(parse_connection_string("user@host:abc"), None);
        assert_eq!(parse_connection_string("user@host:0"), None);
    }

    #[test]
    fn parse_with_whitespace_returns_none() {
        assert_eq!(parse_connection_string("user @host"), None);
        assert_eq!(parse_connection_string("user@ host"), None);
    }

    #[test]
    fn parse_double_at_returns_none() {
        assert_eq!(parse_connection_string("a@b@c"), None);
    }
}
