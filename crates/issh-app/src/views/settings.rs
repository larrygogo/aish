//! SettingsView — Card 分组布局（2026-05-12 UI-polish 重写）。
//!
//! 原版（M4b 静态三段 + M12 加 Switch 演示位）问题：
//!   - 全部 section 平铺无分组，section header 11px 灰字识别度低
//!   - 行高 py(2~4px) 过密
//!   - SETTINGS 主标题 20px 后跟 1px border 显得突兀
//!
//! 重写后：每个 section 用 issh_ui::Card outlined 包裹（header = section
//! title），section 间 gap_4 间距；行内统一 px_4 / py_3 内边距；shortcut
//! 表格化 grid 风格（左列固定 200px）。
//!
//! Ctrl+1/2/3 由 RootView 全局 on_key_down 路由 — 真实键盘绑定生效。
//! Ctrl+W / Ctrl+T / Ctrl+Tab / Ctrl+Shift+V 在 terminal_view focused 时生效
//! （tab 操作 + 终端粘贴）。

use gpui::{div, prelude::*, px, AnyElement, Context, Entity, IntoElement, SharedString, Window};
use issh_ui::theme::ThemeKind;
use issh_ui::{theme, Button, Card, Kbd, Switch, Theme, TypographyExt};

pub struct SettingsView {
    /// M39: state Entity for reading settings_section + observe 变化 → 重 render。
    state: Entity<crate::state::AppState>,
    /// scrollbar 状态 — ScrollPage 接管 wheel / scrollbar / 拖拽。
    scrollbar: issh_ui::ScrollbarHandle,
    /// M31：About section 两个 secondary button entity（press feedback 80ms）。
    open_config_btn: Entity<Button>,
    open_github_btn: Entity<Button>,
}

impl SettingsView {
    pub fn new(state: Entity<crate::state::AppState>, cx: &mut Context<Self>) -> Self {
        // M39: observe state 变化 (settings_section 切换时 trigger 重 render)
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let open_config_btn = cx.new(|cx| {
            let mut b = Button::new("settings-open-config-dir", cx);
            b.label("打开配置目录").secondary().on_click(|_ev, _w, cx| {
                match crate::app_state_file::config_dir() {
                    Some(dir) => {
                        // dir 不存在时 reveal_path 部分平台会报错，先确保存在
                        let _ = std::fs::create_dir_all(&dir);
                        cx.reveal_path(&dir);
                    }
                    None => {
                        issh_ui::toast_error(cx, "无法定位配置目录");
                    }
                }
            });
            b
        });
        let open_github_btn = cx.new(|cx| {
            let mut b = Button::new("settings-open-github", cx);
            b.label("查看 GitHub").secondary().on_click(|_ev, _w, cx| {
                cx.open_url("https://github.com/larrygogo/issh");
            });
            b
        });

        Self {
            state,
            scrollbar: issh_ui::ScrollbarHandle::new(),
            open_config_btn,
            open_github_btn,
        }
    }
}

/// M39 paseo 风：section label 渲染在 card **外**上方，灰色 Caption，与
/// card 间留 8px gap。card 自身不再带 header，整张 card 就是 rows list。
/// 参考 paseo 截图风格 (Navigation / Tabs & Panes / Projects)。
fn section_label_external(title: &'static str, t: &Theme) -> AnyElement {
    div()
        .pb_2()
        .px_1() // 略缩进让 label 跟 card 左边缘视觉对齐
        .typography(issh_ui::TypeRole::Caption, t)
        .text_color(t.colors.muted_foreground)
        .child(title)
        .into_any_element()
}

/// M35 T15 / M39 / Phase A keybinding 自定义：shortcut 专用行。
///
/// 接受 action_id（如 "palette"），从 state.keybindings 读 user override，
/// 没有则走 keybindings::default_for。显示用 format_for_display 转 ⌘⇧K 风。
/// 「自定义」点击 → 设 pending_keybinding_capture = Some(action_id) → 触发
/// KeybindingCaptureView 弹窗。已自定义时多显示「重置」按钮（清掉 override）。
fn shortcut_row(
    state: &Entity<crate::state::AppState>,
    action_id: &'static str,
    desc: &str,
    t: &Theme,
    cx: &Context<SettingsView>,
) -> AnyElement {
    let app = state.read(cx);
    let custom = app.keybindings.get(action_id).cloned();
    let keys_raw = custom
        .clone()
        .unwrap_or_else(|| crate::keybindings::default_for(action_id).to_string());
    let keys_display = crate::keybindings::format_for_display(&keys_raw);
    let is_overridden = custom.is_some();

    let state_for_rebind = state.clone();
    let action_id_for_rebind = action_id.to_string();
    let state_for_reset = state.clone();
    let action_id_for_reset = action_id.to_string();

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_4()
        .py(px(10.0))
        .gap(t.spacing.px_3)
        .child(
            div()
                .flex_1()
                .typography(issh_ui::TypeRole::Body, t)
                .child(SharedString::from(desc.to_string())),
        )
        .child(
            // Kbd chip 显示当前键（自定义优先）
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(Kbd::new(action_id, SharedString::from(keys_display))),
        )
        .when(is_overridden, |d| {
            // 「重置」ghost button — 仅在已 override 时显示，清掉 user override
            // 让该 action 回到 default_for 默认值。
            let state_for_reset = state_for_reset.clone();
            let action_id_for_reset = action_id_for_reset.clone();
            d.child(
                div()
                    .id(SharedString::from(format!("shortcut-reset-{}", action_id)))
                    .px(t.spacing.px_2)
                    .py_0p5()
                    .rounded(t.radius.md)
                    .typography(issh_ui::TypeRole::Caption, t)
                    .text_color(t.colors.muted_foreground)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.secondary_hover))
                    .on_mouse_down(gpui::MouseButton::Left, move |_ev, _w, cx| {
                        let id = action_id_for_reset.clone();
                        state_for_reset.update(cx, |s, cx| {
                            s.keybindings.remove(&id);
                            cx.notify();
                        });
                        let mut snapshot = crate::app_state_file::load_app_state();
                        snapshot.keybindings.remove(&id);
                        crate::app_state_file::save_app_state(&snapshot);
                    })
                    .child("重置"),
            )
        })
        .child(
            // 「自定义」ghost button — 触发 KeybindingCaptureView 弹窗
            div()
                .id(SharedString::from(format!("shortcut-rebind-{}", action_id)))
                .px(t.spacing.px_2)
                .py_0p5()
                .rounded(t.radius.md)
                .typography(issh_ui::TypeRole::Caption, t)
                .text_color(t.colors.muted_foreground)
                .cursor_pointer()
                .hover(|s| s.bg(t.colors.secondary_hover))
                .on_mouse_down(gpui::MouseButton::Left, move |_ev, _w, cx| {
                    let id = action_id_for_rebind.clone();
                    state_for_rebind.update(cx, |s, cx| {
                        s.pending_keybinding_capture = Some(id);
                        cx.notify();
                    });
                })
                .child("自定义"),
        )
        .into_any_element()
}

/// M43 主题选择 row：左侧主题名 + 中间 5 色块预览 + 右侧选中 ✓。
/// 点击切到该主题：构造 Theme → set_global → refresh_windows → 写盘
/// app_state.theme。颜色实时跨整个 app + terminal 联动。
fn theme_row(kind: issh_ui::ThemeKind, current: issh_ui::ThemeKind, t: &Theme) -> AnyElement {
    let is_selected = kind == current;
    let swatches = issh_ui::preview_swatches(kind);
    let display_name = kind.display_name();
    let theme_key = kind.as_key();

    div()
        .id(SharedString::from(format!("theme-row-{}", theme_key)))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_4()
        .py(px(10.0))
        .gap(t.spacing.px_3)
        .cursor_pointer()
        .hover(|s| s.bg(t.colors.secondary_hover))
        .on_mouse_down(gpui::MouseButton::Left, move |_ev, _w, cx| {
            let cur_reduced = issh_ui::theme(cx).reduced_motion;
            let mut new_theme = match kind {
                issh_ui::ThemeKind::Dark => Theme::dark(),
                issh_ui::ThemeKind::Light => Theme::light(),
                issh_ui::ThemeKind::Moshi => Theme::moshi(),
                issh_ui::ThemeKind::Dracula => Theme::dracula(),
                issh_ui::ThemeKind::Nord => Theme::nord(),
                issh_ui::ThemeKind::SolarizedDark => Theme::solarized_dark(),
                issh_ui::ThemeKind::Gruvbox => Theme::gruvbox(),
                issh_ui::ThemeKind::CatppuccinMocha => Theme::catppuccin_mocha(),
                issh_ui::ThemeKind::SolarizedLight => Theme::solarized_light(),
                issh_ui::ThemeKind::CatppuccinLatte => Theme::catppuccin_latte(),
                issh_ui::ThemeKind::GithubLight => Theme::github_light(),
                issh_ui::ThemeKind::RosePineDawn => Theme::rose_pine_dawn(),
            };
            new_theme.reduced_motion = cur_reduced;
            cx.set_global(new_theme);
            cx.refresh_windows();
            let mut snapshot = crate::app_state_file::load_app_state();
            snapshot.theme = Some(theme_key.to_string());
            crate::app_state_file::save_app_state(&snapshot);
        })
        .child(
            div()
                .flex_1()
                .typography(issh_ui::TypeRole::Body, t)
                .child(SharedString::from(display_name)),
        )
        .child({
            // 5 色块预览（bg / fg / red / green / blue），跟用户参考截图同顺序
            let mut row = div().flex().flex_row().gap(px(4.0));
            for color in swatches {
                row = row.child(
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(t.radius.sm)
                        .bg(color)
                        .border_1()
                        .border_color(t.colors.border),
                );
            }
            row
        })
        .when(is_selected, |d| {
            d.child(
                issh_ui::icon(issh_ui::IconName::Check)
                    .size(t.icon_size.sm)
                    .text_color(t.colors.accent_foreground),
            )
        })
        .into_any_element()
}

/// 标签 + 可选副标题 + 控件横向行（用于 Appearance 的 Dark mode 这种
/// label + Switch）。helper 非空时在 label 下方显示 muted_foreground xs 灰字，
/// 用于提示控件状态 / 限制（如"Light theme not implemented"）。
fn control_row(
    label: &'static str,
    helper: Option<&'static str>,
    control: AnyElement,
    t: &Theme,
) -> AnyElement {
    // M26: label = Label (13/500/fg) / helper = Caption (12/muted)
    let left = div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(div().typography(issh_ui::TypeRole::Label, t).child(label))
        .when_some(helper, |d, h| {
            d.child(div().typography(issh_ui::TypeRole::Caption, t).child(h))
        });
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_4()
        .py(px(10.0))
        .child(left)
        .child(control)
        .into_any_element()
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = env!("CARGO_PKG_VERSION");
        // build.rs 注入；git 不可用时 fallback "unknown"。
        let git_hash = env!("ISSH_GIT_HASH");
        let version_str = if git_hash == "unknown" {
            format!("issh v{}", version)
        } else {
            format!("issh v{} ({})", version, git_hash)
        };
        let t = theme(cx);
        let colors = t.colors;
        // M30：reduced_motion 偏好直接从 global Theme 读，Switch 反映真值。
        let reduced_motion = t.reduced_motion;

        // ───── 页面标题 ─────
        // M26 T2: page title 用 Title1 (20/600/fg)。
        // M27: pb 走 anatomy.page.header_to_content_gap (16) — 之前 pb_6=24
        // 偏松，统一让 page_title 与首 Card 间隔 16，与 home page header 一致。
        let page_title = div()
            .pb(t.anatomy.page.header_to_content_gap)
            .typography(issh_ui::TypeRole::Title1, t)
            .child("设置");

        // ───── Appearance ─────
        // M39: 合并 dark switch + variant select 为单 theme select（参考 paseo
        // Theme 单 dropdown 模式）。删除 dark_switch + variant_select_row 二段
        // UX，用 self.theme_select 一个控件提供 4 选项: 默认 / Midnight /
        // Warp Aurora / 浅色。Select 默认 BottomEnd placement 右对齐 trigger
        // 防右上角下拉溢出。

        // M30：reduced motion Switch — 切换"减少动画"偏好，写盘 + 更新 Theme
        let motion_switch = Switch::new("settings-reduced-motion")
            .checked(reduced_motion)
            .on_change(cx.listener(|_this, new_value: &bool, _w, cx| {
                let new_reduced = *new_value;
                // 切 reduced_motion 时保留当前主题（按 kind 重新构造同主题）。
                // M43 主题包加入后通过 themes::theme_for(kind) 派发；现仅 Light
                // / Dark factories 实现，其余 variant fallback dark（Step 3 补全）。
                let kind = theme(cx).kind;
                let mut new_theme = match kind {
                    ThemeKind::Light => Theme::light(),
                    _ => Theme::dark(),
                };
                new_theme.reduced_motion = new_reduced;
                cx.set_global(new_theme);
                cx.refresh_windows();
                let mut snapshot = crate::app_state_file::load_app_state();
                snapshot.reduced_motion = Some(new_reduced);
                crate::app_state_file::save_app_state(&snapshot);
            }))
            .into_any_element();

        // ───── 外观（仅含「减少动画」)─────
        let appearance_label = section_label_external("外观", t);
        let appearance_card = Card::new("settings-appearance")
            .outlined()
            .no_padding()
            .body(div().flex().flex_col().child(control_row(
                "减少动画",
                Some("关闭 dialog 入场 / toast 出现等动画"),
                motion_switch,
                t,
            )));

        // ───── M43 主题包：DARK / LIGHT 分组列表 + 5 色块预览 ─────
        // 跟用户参考截图同结构：每项左侧主题名 + 右侧 [bg, fg, red, green, blue]
        // 色块 + 选中 ✓。点击切主题（cx.set_global + 写盘）。
        let current_kind = t.kind;
        let theme_dark_label = section_label_external("DARK", t);
        let theme_light_label = section_label_external("LIGHT", t);
        let theme_dark_card = Card::new("settings-theme-dark")
            .outlined()
            .no_padding()
            .body({
                let mut body = div().flex().flex_col();
                for kind in issh_ui::ALL_THEMES.iter().filter(|k| k.is_dark()) {
                    body = body.child(theme_row(*kind, current_kind, t));
                }
                body
            });
        let theme_light_card = Card::new("settings-theme-light")
            .outlined()
            .no_padding()
            .body({
                let mut body = div().flex().flex_col();
                for kind in issh_ui::ALL_THEMES.iter().filter(|k| !k.is_dark()) {
                    body = body.child(theme_row(*kind, current_kind, t));
                }
                body
            });

        // ───── Keyboard Shortcuts ─────
        // M35 T15 / Phase A keybinding 自定义：按 ACTIONS 列表 +
        // shortcut_row 读 state.keybindings override，展示 default_for 兜底。
        // mac_only action 在非 mac 平台跳过。
        let mac = cfg!(target_os = "macos");
        let shortcuts_label = section_label_external("快捷键", t);
        let shortcuts_card = Card::new("settings-shortcuts")
            .outlined()
            .no_padding() // row helpers 自带 px_4 padding
            .body({
                let mut body = div().flex().flex_col();
                for action in crate::keybindings::ACTIONS {
                    if action.mac_only && !mac {
                        continue;
                    }
                    body = body.child(shortcut_row(&self.state, action.id, action.label, t, cx));
                }
                body
            });

        // ───── About 简化版（用户反馈「关于我们的页面做的简洁一些」）─────
        // 删 logo hero / 删 actions_row (两按钮单独行) / 删构建日期 / 删
        // 代码仓库 / 删许可证 row, 仅保留:
        // - 「版本」row + 右侧 version 值
        // - 「配置目录」row + 右侧 「打开」button
        // - 「GitHub」row + 右侧 「查看」button
        // 跟 paseo about 截图同结构 (Version + Release channel + App updates
        // 三 row 平铺无 hero)。
        let about_label = section_label_external("关于", t);
        let about_card = Card::new("settings-about").outlined().no_padding().body(
            div()
                .flex()
                .flex_col()
                .child(control_row(
                    "版本",
                    None,
                    div()
                        .typography(issh_ui::TypeRole::Body, t)
                        .text_color(colors.muted_foreground)
                        .child(version_str.clone())
                        .into_any_element(),
                    t,
                ))
                .child(control_row(
                    "配置目录",
                    Some("hosts.json / app_state.toml 所在位置"),
                    self.open_config_btn.clone().into_any_element(),
                    t,
                ))
                .child(control_row(
                    "GitHub",
                    Some("查看源码 / 提交问题 / 加 star"),
                    self.open_github_btn.clone().into_any_element(),
                    t,
                )),
        );

        // ───── 整页布局 ─────
        // ScrollPage.scrollbar + flex_1 触发 thumb 可见 + 可拖；caller 父
        // 必须 flex_col（这里 size_full + flex_col 包一下）。bg 在外 flex_col
        // 上设了，ScrollPage 自己不带 bg 避免 scrollbar overlay 不在 viewport
        // 内显示时露 background 不一致。
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .child(
                // M27: ScrollPage padding 走 anatomy.page；Card 之间 gap 走
                // anatomy.page.section_gap（16）— 之前 gap_4=16 等价但解耦
                // 让后续调密度只改 anatomy 一处生效。
                issh_ui::ScrollPage::new("settings-scroll")
                    .scrollbar(&self.scrollbar)
                    .flex_1()
                    .px(t.anatomy.page.outer_px)
                    .py(t.anatomy.page.outer_py_top)
                    .child(page_title)
                    .child(
                        // M39 sidebar sub-nav 拆分: 按 settings_section 仅渲染
                        // 对应 section card (General → appearance, Shortcuts
                        // → shortcuts, About → about)。section label 跟 card
                        // 组对显示。
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.anatomy.page.section_gap)
                            .when(
                                self.state.read(cx).settings_section
                                    == crate::state::SettingsSection::General,
                                |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(t.anatomy.page.section_gap)
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(theme_dark_label)
                                                    .child(theme_dark_card),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(theme_light_label)
                                                    .child(theme_light_card),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(appearance_label)
                                                    .child(appearance_card),
                                            ),
                                    )
                                },
                            )
                            .when(
                                self.state.read(cx).settings_section
                                    == crate::state::SettingsSection::Shortcuts,
                                |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(shortcuts_label)
                                            .child(shortcuts_card),
                                    )
                                },
                            )
                            .when(
                                self.state.read(cx).settings_section
                                    == crate::state::SettingsSection::About,
                                |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(about_label)
                                            .child(about_card),
                                    )
                                },
                            ),
                    ),
            )
    }
}
