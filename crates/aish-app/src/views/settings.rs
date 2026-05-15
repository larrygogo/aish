//! SettingsView — Card 分组布局（2026-05-12 UI-polish 重写）。
//!
//! 原版（M4b 静态三段 + M12 加 Switch 演示位）问题：
//!   - 全部 section 平铺无分组，section header 11px 灰字识别度低
//!   - 行高 py(2~4px) 过密
//!   - SETTINGS 主标题 20px 后跟 1px border 显得突兀
//!
//! 重写后：每个 section 用 aish_ui::Card outlined 包裹（header = section
//! title），section 间 gap_4 间距；行内统一 px_4 / py_3 内边距；shortcut
//! 表格化 grid 风格（左列固定 200px）。
//!
//! Ctrl+1/2/3 由 RootView 全局 on_key_down 路由 — 真实键盘绑定生效。
//! Ctrl+W / Ctrl+T / Ctrl+Tab / Ctrl+Shift+V 在 terminal_view focused 时生效
//! （tab 操作 + 终端粘贴）。

use aish_ui::theme::ThemeKind;
use aish_ui::{theme, Button, Card, Kbd, Switch, Theme, TypographyExt};
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, IntoElement, SharedString, Window};

pub struct SettingsView {
    /// scrollbar 状态 — ScrollPage 接管 wheel / scrollbar / 拖拽。
    scrollbar: aish_ui::ScrollbarHandle,
    /// M31：About section 两个 secondary button entity（press feedback 80ms）。
    open_config_btn: Entity<Button>,
    open_github_btn: Entity<Button>,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
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
                        aish_ui::toast_error(cx, "无法定位配置目录");
                    }
                }
            });
            b
        });
        let open_github_btn = cx.new(|cx| {
            let mut b = Button::new("settings-open-github", cx);
            b.label("查看 GitHub").secondary().on_click(|_ev, _w, cx| {
                cx.open_url("https://github.com/larrygogo/aish");
            });
            b
        });
        Self {
            scrollbar: aish_ui::ScrollbarHandle::new(),
            open_config_btn,
            open_github_btn,
        }
    }
}

/// section card 的 header：Title3 (14/SEMIBOLD/fg) + px/py + 底部 border。
fn section_header(title: &'static str, t: &Theme) -> AnyElement {
    div()
        .px_4()
        .py_3()
        .typography(aish_ui::TypeRole::Title3, t)
        .border_b_1()
        .border_color(t.colors.border)
        .child(title)
        .into_any_element()
}

/// 两列行：左 200px 固定，右自然宽。用于 shortcut / info pair。
fn two_column_row(left: &str, right: &str, t: &Theme) -> AnyElement {
    // M26 T5: 左 Label (13/500/fg) + secondary_fg override 弱化（让 right
    // 是主信息）；右 Body (13/400/fg)。语义：left 是字段名，right 是值。
    // M27 anatomy：py(10) 与 control_row 统一行高（settings card 内 row 节奏一致）。
    div()
        .flex()
        .flex_row()
        .items_center()
        .px_4()
        .py(px(10.0))
        .child(
            div()
                .w(px(200.0))
                .typography(aish_ui::TypeRole::Label, t)
                .text_color(t.colors.secondary_foreground)
                .child(SharedString::from(left.to_string())),
        )
        .child(
            div()
                .flex_1()
                .typography(aish_ui::TypeRole::Body, t)
                .child(SharedString::from(right.to_string())),
        )
        .into_any_element()
}

/// M35 T15: shortcut 专用行 — 左侧 Kbd chip 视觉化按键，右侧描述。
/// 与 two_column_row 同样的左 200px 固定 + 右自然宽节奏，但左列用 Kbd
/// chip 替代纯文本。左列设 flex container 让 chip 取自然宽度（不被
/// 200px 列宽拉伸），整列固定让多行 chip 左对齐齐整。
fn shortcut_row(id: &'static str, keys: &str, desc: &str, t: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .px_4()
        .py(px(10.0))
        .child(
            div()
                .w(px(200.0))
                .flex()
                .flex_row()
                .items_center()
                .child(Kbd::new(id, SharedString::from(keys.to_string()))),
        )
        .child(
            div()
                .flex_1()
                .typography(aish_ui::TypeRole::Body, t)
                .child(SharedString::from(desc.to_string())),
        )
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
        .child(div().typography(aish_ui::TypeRole::Label, t).child(label))
        .when_some(helper, |d, h| {
            d.child(div().typography(aish_ui::TypeRole::Caption, t).child(h))
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
        let git_hash = env!("AISH_GIT_HASH");
        let build_date = env!("AISH_BUILD_DATE");
        let version_str = if git_hash == "unknown" {
            format!("aish v{}", version)
        } else {
            format!("aish v{} ({})", version, git_hash)
        };
        let t = theme(cx);
        let colors = t.colors;
        // 当前主题种类从 global Theme 读 —— 切换后 set_global + refresh_windows
        // 让所有 view 重新 render 拿新 theme。Switch 的 checked 状态直接反映
        // global 真值，不再用 self.dark_mode 镜像（之前 镜像 + disabled 是因为
        // Light 未实现的占位）。
        let dark = matches!(t.kind, ThemeKind::Dark);
        // M30：reduced_motion 偏好直接从 global Theme 读，Switch 反映真值。
        let reduced_motion = t.reduced_motion;

        // ───── 页面标题 ─────
        // M26 T2: page title 用 Title1 (20/600/fg)。
        // M27: pb 走 anatomy.page.header_to_content_gap (16) — 之前 pb_6=24
        // 偏松，统一让 page_title 与首 Card 间隔 16，与 home page header 一致。
        let page_title = div()
            .pb(t.anatomy.page.header_to_content_gap)
            .typography(aish_ui::TypeRole::Title1, t)
            .child("Settings");

        // ───── Appearance ─────
        // M18-light 启用：toggle 真切 Theme global + 持久化到 app_state.toml
        // 让重启保留选择。
        let dark_switch = Switch::new("settings-dark-mode")
            .checked(dark)
            .on_change(cx.listener(|_this, new_value: &bool, _w, cx| {
                let dark_now = *new_value;
                // 切主题时**保留** reduced_motion 偏好（dark/light Theme
                // constructor 初始化为 false，会丢用户选择）
                let cur_reduced = theme(cx).reduced_motion;
                let mut new_theme = if dark_now {
                    Theme::dark()
                } else {
                    Theme::light()
                };
                new_theme.reduced_motion = cur_reduced;
                cx.set_global(new_theme);
                // refresh_windows 让所有 view 重 render 拿新 theme global。
                // 不用 cx.notify(this) —— 该 view 自身只是众多受影响 view 之一。
                cx.refresh_windows();
                // 持久化：load 当前 state、改 theme 字段、save。
                let mut snapshot = crate::app_state_file::load_app_state();
                snapshot.theme = Some(if dark_now { "dark" } else { "light" }.to_string());
                crate::app_state_file::save_app_state(&snapshot);
            }))
            .into_any_element();

        // M30：reduced motion Switch — 切换"减少动画"偏好，写盘 + 更新 Theme
        let motion_switch = Switch::new("settings-reduced-motion")
            .checked(reduced_motion)
            .on_change(cx.listener(|_this, new_value: &bool, _w, cx| {
                let new_reduced = *new_value;
                // 切 reduced_motion 时保留 dark/light kind
                let kind = theme(cx).kind;
                let mut new_theme = match kind {
                    ThemeKind::Dark => Theme::dark(),
                    ThemeKind::Light => Theme::light(),
                };
                new_theme.reduced_motion = new_reduced;
                cx.set_global(new_theme);
                cx.refresh_windows();
                let mut snapshot = crate::app_state_file::load_app_state();
                snapshot.reduced_motion = Some(new_reduced);
                crate::app_state_file::save_app_state(&snapshot);
            }))
            .into_any_element();

        let appearance_card = Card::new("settings-appearance")
            .outlined()
            .no_padding() // M27: section_header + row helpers 都自带 px_4，opt-out 防双重
            .header(section_header("Appearance", t))
            .body(
                div()
                    .flex()
                    .flex_col()
                    // M35 T17: Dark mode 加 helper 提示 Light theme 是实验性
                    // — INDEX 历史记录 M15/M17 有 7 个 light token 仍 dark
                    // fallback 未完整调优，用户切到 light 后部分色彩可能不
                    // 协调。明示「实验性」让用户预期对齐。
                    .child(control_row(
                        "Dark mode",
                        Some("关闭后切到 Light 主题（实验性，部分色彩未完整调优）"),
                        dark_switch,
                        t,
                    ))
                    .child(control_row(
                        "减少动画",
                        Some("关闭 dialog 入场 / toast 出现等动画"),
                        motion_switch,
                        t,
                    )),
            );

        // ───── Keyboard Shortcuts ─────
        // M35 T15: shortcut 改用 Kbd chip 视觉化按键。M35 T8 加的
        // Ctrl+P/Cmd+P/Cmd+K palette 也加入列表。Ctrl+1/2/3 由 RootView
        // 全局接管；Ctrl+W / Ctrl+T / Ctrl+Tab 走 terminal_view focused 路径。
        let shortcuts_card = Card::new("settings-shortcuts")
            .outlined()
            .no_padding() // M27: section_header + row helpers 都自带 px_4，opt-out 防双重
            .header(section_header("Keyboard Shortcuts", t))
            .body(
                div()
                    .flex()
                    .flex_col()
                    .child(shortcut_row("sc-palette", "Ctrl+P / ⌘K", "打开命令面板", t))
                    .child(shortcut_row("sc-paste", "Ctrl+Shift+V", "粘贴", t))
                    .child(shortcut_row("sc-new-tab", "Ctrl+T", "新建 tab", t))
                    .child(shortcut_row("sc-close-tab", "Ctrl+W", "关闭 tab", t))
                    .child(shortcut_row("sc-home", "Ctrl+1", "切到 Home", t))
                    .child(shortcut_row("sc-terminal", "Ctrl+2", "切到 Terminal", t))
                    .child(shortcut_row("sc-settings", "Ctrl+3", "切到 Settings", t)),
            );

        // ───── About（版本信息 + 实际可点交互按钮）─────
        // 按钮走 outline variant，与卡片视觉一致；handler 内调 cx.open_url /
        // cx.reveal_path 触发 OS 默认浏览器 / 文件管理器，跨平台一致。
        let actions_row = div()
            .pt_2()
            .flex()
            .flex_row()
            .gap_2()
            .child(self.open_config_btn.clone())
            .child(self.open_github_btn.clone());

        // M35 T15: About 顶部加 logo + 标题 hero — 让\"关于 aish\"有产品感
        // 而非纯属性表。logo 48px PNG 通过 AppAssets path 加载，与 titlebar
        // 同一图（保留蓝/黑双色 brand）。
        let about_hero = div()
            .px_4()
            .py(px(12.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(gpui::img("aish-icon.png").w(px(48.0)).h(px(48.0)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(div().typography(aish_ui::TypeRole::Title2, t).child("aish"))
                    .child(
                        div()
                            .typography(aish_ui::TypeRole::Caption, t)
                            .child("有审美的 SSH 客户端"),
                    ),
            )
            .into_any_element();

        let about_card = Card::new("settings-about")
            .outlined()
            .no_padding() // M27: section_header + row helpers 都自带 px_4，opt-out 防双重
            .header(section_header("About", t))
            .body(
                div()
                    .flex()
                    .flex_col()
                    .child(about_hero)
                    .child(two_column_row("Version", &version_str, t))
                    .child(two_column_row("Build date", build_date, t))
                    .child(two_column_row("Repository", "github.com/larrygogo/aish", t))
                    .child(two_column_row("License", "MIT", t))
                    .child(actions_row),
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
                aish_ui::ScrollPage::new("settings-scroll")
                    .scrollbar(&self.scrollbar)
                    .flex_1()
                    .px(t.anatomy.page.outer_px)
                    .py(t.anatomy.page.outer_py_top)
                    .child(page_title)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.anatomy.page.section_gap)
                            .child(appearance_card)
                            .child(shortcuts_card)
                            .child(about_card),
                    ),
            )
    }
}
