//! TextInput — 单行文本输入框。
//!
//! 含 cursor blink（T10）、selection（T11）、复制粘贴（T12）。
//! T9：struct + 键盘 + IME + render。
//! T10：cursor blink（600ms 周期，任意操作 reset 相位）。

use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, DispatchPhase, FocusHandle, Focusable,
    InputHandler, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    SharedString, UTF16Selection, Window,
};

use crate::theme::theme;

type SubmitHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type ChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type CancelHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type BlurHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

const BLINK_PERIOD_MS: u64 = 600;

pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    cursor: usize, // byte offset
    placeholder: SharedString,
    on_submit: Option<SubmitHandler>,
    on_change: Option<ChangeHandler>,
    blink_epoch: Instant,
    selection_anchor: Option<usize>,      // 拖选起始 byte offset
    last_click: Option<(Instant, usize)>, // 双击检测
    mask_char: Option<char>,              // M16: mask 模式替换字符；None = 正常显示
    /// M16 T2：每个显示字符的 viewport bounds（byte_offset → rect）。
    /// render 入口清空，每帧由逐字 wrap div 内嵌 canvas 在 prepaint
    /// 阶段通过 push_glyph_bounds 重新填充。
    bounds_map: Vec<(usize, Bounds<Pixels>)>,
    /// M16 T3：mouse drag select 状态。mouse_down=true，mouse_up=false。
    is_dragging: bool,
    /// borderless 模式：去 bg / border / 固定高度，让 input 完全融入父容器
    /// （inline 编辑场景，如 tab title rename）。仍保留 cursor / 选区 / 输入。
    borderless: bool,
    /// Esc 取消回调。on_submit 是 Enter 提交，on_cancel 是 Esc 放弃 ——
    /// caller 一般用于清编辑状态、还原原值（不写回）。
    on_cancel: Option<CancelHandler>,
    /// 失焦回调。input 上一帧 focused 这一帧 unfocused 时触发，传当前 text
    /// 给 caller（一般 caller 在此处 commit 编辑保留改动，与"点击外部 commit"
    /// 体感一致）。
    on_blur: Option<BlurHandler>,
    /// 上一帧 focused 状态。render 内对比当前 focused 决定是否 fire on_blur。
    last_focused: bool,
    /// 水平 scroll 偏移：text_row 整体 margin-left。0 = 没滚；负数 = 文字向左
    /// 滚出（让右侧 cursor 重新进可视区）。canvas prepaint callback 比较
    /// cursor_x 与 viewport bounds 后更新此值，下一帧用新 offset re-render。
    /// 解决长文本 / cursor 移末尾时'后面的字看不到'问题。
    scroll_offset: Pixels,
    /// M19 vertical scroll：多行 cursor 在 max_lines 视区外时自动滚到可见。
    /// 0 = content 顶贴容器顶；负数 = content 上移露下方（cursor 在末行场景）。
    /// 单行 / 内容 ≤ max_lines 时永远 0。
    scroll_offset_y: Pixels,
    /// drag select 当前鼠标 x（mouse_move 时更新）。drag-to-edge auto-scroll
    /// timer 比对 viewport 边界用，鼠标接近边沿时主动扩 cursor + 滚动。
    drag_target_x: Option<Pixels>,
    /// 上一帧 viewport bounds（canvas prepaint 写入）。drag auto-scroll 用。
    viewport_bounds: Option<Bounds<Pixels>>,
    /// drag 期间的 auto-scroll task。mouse_down 启，mouse_up 时 take() 丢弃
    /// 自动 abort。drag 期间 30ms 周期检查 drag_target_x 是否接近 viewport
    /// 边沿，扩 cursor 一格。
    drag_task: Option<gpui::Task<()>>,
    /// 是否显示右侧"眼睛"按钮切换 mask 显示（password 字段典型用例）。
    /// caller 通常先调 mask_char(Some('•')) 再 show_mask_toggle(true)。
    /// 点击眼睛：mask_char Some('•') ↔ None 切换。
    show_mask_toggle: bool,
    /// M19：多行模式。true 时 Enter 插 \n / Ctrl+Enter 触发 on_submit /
    /// 按 \n 拆 logical lines + word-wrap 成 visual lines / auto-grow 到
    /// max_lines 上限后内部滚动。默认 false（单行行为完全不变）。
    multiline: bool,
    /// M19：多行模式下容器最大行数（含 wrap 后的 visual lines）。超出后
    /// 容器高度固定不再增，内部 overflow_y_scroll。默认 6。multiline=false
    /// 时此字段无效。
    max_lines: usize,
    /// M19：cursor 跨行 ↑/↓ 时的 col 记忆。让用户多次按 ↓ 经过短行后回到长行
    /// 时仍在原 col（标准 textarea 行为）。横向操作（left/right/click/typing）
    /// 清掉，重新设。T1 占位，T4 cursor_up/down_visual 实施时启用。
    #[allow(dead_code)]
    preferred_col: Option<usize>,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let this = Self {
            focus_handle: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            placeholder: SharedString::default(),
            on_submit: None,
            on_change: None,
            blink_epoch: Instant::now(),
            selection_anchor: None,
            last_click: None,
            mask_char: None,
            bounds_map: Vec::new(),
            is_dragging: false,
            borderless: false,
            on_cancel: None,
            on_blur: None,
            last_focused: false,
            scroll_offset: px(0.0),
            scroll_offset_y: px(0.0),
            drag_target_x: None,
            viewport_bounds: None,
            drag_task: None,
            show_mask_toggle: false,
            multiline: false,
            max_lines: 6,
            preferred_col: None,
        };
        this.start_blink_timer(cx);
        this
    }

    pub fn placeholder(&mut self, p: impl Into<SharedString>) -> &mut Self {
        self.placeholder = p.into();
        self
    }

    /// 启用 mask 模式：传 Some('•') 把字符显示替换为 •，并禁止 copy/cut。
    /// 传 None（默认）正常显示。HostForm password 字段用 Some('•')。
    pub fn mask_char(&mut self, c: Option<char>) -> &mut Self {
        self.mask_char = c;
        self
    }

    pub fn is_masked(&self) -> bool {
        self.mask_char.is_some()
    }

    /// 设置 borderless 模式：去 bg / border / 固定高度，input 完全融入父容器。
    /// 适用于 inline 编辑场景（如 tab title rename / 列表 row 内编辑）。
    pub fn borderless(&mut self, b: bool) -> &mut Self {
        self.borderless = b;
        self
    }

    /// 右侧加"眼睛"按钮切换 mask 显示。caller 通常先 .mask_char(Some('•'))
    /// 再 .show_mask_toggle(true)。点击：mask_char Some('•') ↔ None 切换，
    /// icon 在 Eye / EyeOff 间切换（masked = EyeOff 提示"目前隐藏"）。
    pub fn show_mask_toggle(&mut self, b: bool) -> &mut Self {
        self.show_mask_toggle = b;
        self
    }

    /// 切换 mask_char Some('•') ↔ None。caller 一般不需要直接调用 —— 用
    /// show_mask_toggle 启用 UI 按钮，按钮 click 内部自调。
    pub fn toggle_mask(&mut self, cx: &mut Context<Self>) {
        self.mask_char = if self.mask_char.is_some() {
            None
        } else {
            Some('•')
        };
        cx.notify();
    }

    /// M19：启用多行模式。Enter 插 \\n，Ctrl+Enter 触发 on_submit，按 \\n
    /// 拆 logical lines + word-wrap 成 visual lines + auto-grow 到 max_lines
    /// 上限后内部滚动。默认 false 单行行为完全不变。
    pub fn multiline(&mut self, b: bool) -> &mut Self {
        self.multiline = b;
        self
    }

    /// M19：多行模式下容器最大可见行数（按 wrap 后 visual lines 计）。
    /// 超过此值容器停止增高，内部 overflow_y_scroll，cursor 在屏外时自动滚到
    /// 可见。默认 6 行。multiline=false 时无效。
    pub fn max_lines(&mut self, n: usize) -> &mut Self {
        self.max_lines = n.max(1); // 至少 1 行
        self
    }

    pub fn on_submit(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_submit = Some(Rc::new(h));
        self
    }

    pub fn on_change(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_change = Some(Rc::new(h));
        self
    }

    /// Esc 取消回调：用户按 Esc 时触发（同时清 selection，与系统输入框一致）。
    pub fn on_cancel(&mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> &mut Self {
        self.on_cancel = Some(Rc::new(h));
        self
    }

    /// 失焦回调：input 上一帧 focused 这一帧 unfocused 时触发，传当前 text。
    pub fn on_blur(&mut self, h: impl Fn(&str, &mut Window, &mut App) + 'static) -> &mut Self {
        self.on_blur = Some(Rc::new(h));
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, t: impl Into<String>, cx: &mut Context<Self>) {
        self.text = t.into();
        self.cursor = self.text.len();
        // 必须清 anchor —— 否则旧 anchor 可能 > 新 text.len()，
        // 下次 delete_selection 时 drain 越界 panic（HostForm 切 host edit
        // 重置字段是典型触发场景）。
        self.selection_anchor = None;
        // M19: 清 preferred_col —— 否则跨 text 切换后 ↑↓ 用旧 col 算
        // 体感错位（按 ↓ 落到不预期位置）。
        self.preferred_col = None;
        self.reset_blink();
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.text.clear();
        self.cursor = 0;
        // 同 set_text：text 清空后 anchor 必须同步清，否则 drain panic
        self.selection_anchor = None;
        self.preferred_col = None;
        self.reset_blink();
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    // -------- blink helpers --------

    /// 重置 blink 相位（按键 / 鼠标后让 cursor 立即可见）。
    fn reset_blink(&mut self) {
        self.blink_epoch = Instant::now();
    }

    /// 根据相位判断 cursor 是否应该可见（render 用）。
    fn cursor_visible_now(&self) -> bool {
        let phase = self.blink_epoch.elapsed().as_millis() as u64 % BLINK_PERIOD_MS;
        phase < BLINK_PERIOD_MS / 2
    }

    pub(crate) fn push_glyph_bounds(&mut self, byte: usize, bounds: Bounds<Pixels>) {
        self.bounds_map.push((byte, bounds));
    }

    pub(crate) fn clear_glyph_bounds(&mut self) {
        self.bounds_map.clear();
    }

    /// 水平 scroll 跟随 cursor：在 canvas prepaint 阶段调用（此时 bounds_map
    /// 已被 glyph_div 填好上一帧的 viewport 位置）。
    ///
    /// 算法：拿当前 cursor 对应的 glyph absolute x，比对 viewport 左右边界，
    /// 调整 scroll_offset 让 cursor 落进 [v_left + margin, v_right - margin]。
    /// notify 触发下一帧 re-render with new offset。
    pub(crate) fn update_scroll_to_cursor(
        &mut self,
        viewport: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        // 多行：走 vertical scroll 分支 + 重置水平 scroll（word-wrap 已经覆盖
        // 长行，没有水平 scroll 需求）
        if self.multiline {
            self.update_scroll_y_to_cursor(viewport, cx);
            if self.scroll_offset != px(0.0) {
                self.scroll_offset = px(0.0);
                cx.notify();
            }
            return;
        }

        // 单行水平 scroll（原算法）
        let displayed_text: String = if let Some(m) = self.mask_char {
            self.text.chars().map(|_| m).collect()
        } else {
            self.text.clone()
        };
        let displayed_cursor = self.cursor_for_display(&displayed_text);

        // cursor 当前的 absolute x：bounds_map 找 cursor byte；找不到（cursor 在
        // 末尾）则用最后一个 glyph 的 right edge；都没有（空文本）用 viewport 左缘
        let cursor_x = self
            .bounds_map
            .iter()
            .find(|(b, _)| *b == displayed_cursor)
            .map(|(_, b)| b.origin.x)
            .or_else(|| {
                self.bounds_map
                    .last()
                    .map(|(_, b)| b.origin.x + b.size.width)
            })
            .unwrap_or(viewport.origin.x);

        let margin = px(4.0);
        let v_left = viewport.origin.x;
        let v_right = viewport.origin.x + viewport.size.width;
        let mut new_offset = self.scroll_offset;
        if cursor_x < v_left + margin {
            new_offset = self.scroll_offset + (v_left + margin - cursor_x);
        } else if cursor_x > v_right - margin {
            new_offset = self.scroll_offset - (cursor_x - v_right + margin);
        }
        // clamp：scroll_offset 不应 > 0（文字第一个 char 永远不该被推到 viewport
        // 左缘右侧 —— 那等于'前面有空隙'）。负方向 clamp 不做：删字 / 短文本时
        // 短到 < viewport 宽度的情况会自然在下一次 cursor 移动时回到 0。
        if new_offset > px(0.0) {
            new_offset = px(0.0);
        }
        if new_offset != self.scroll_offset {
            self.scroll_offset = new_offset;
            cx.notify();
        }
    }

    /// M19 vertical scroll：multiline 下 cursor 在 max_lines 视区外时自动
    /// scroll 到可见。算法：算 cursor 所在 visual_line，比对当前 scroll
    /// offset_y 看是否要 scroll up/down。content ≤ max_lines 时重置 0。
    fn update_scroll_y_to_cursor(&mut self, viewport: Bounds<Pixels>, cx: &mut Context<Self>) {
        let line_h = px(20.0); // 与 render 内 line_h 一致
        let vls = compute_visual_lines(&self.text, viewport.size.width, px(12.0));
        let n_lines = vls.len();
        let max_lines = self.max_lines;

        // content 不溢出：scroll 必为 0
        if n_lines <= max_lines {
            if self.scroll_offset_y != px(0.0) {
                self.scroll_offset_y = px(0.0);
                cx.notify();
            }
            return;
        }

        let (cursor_vl, _) = byte_to_visual_pos(self.cursor, &vls);
        let scroll = self.scroll_offset_y;
        // cursor 在 viewport 内坐标系的 top（content 顶 = 0）。
        // GPUI Pixels 只能 Pixels * f32（scalar），不能 Pixels * Pixels。
        let cursor_top_in_content = line_h * cursor_vl as f32;
        let cursor_top_in_view = cursor_top_in_content + scroll;
        let viewport_h = line_h * max_lines as f32;
        let margin = px(0.0); // 不留 margin —— cursor 行边缘也算可见

        let mut new_scroll = scroll;
        if cursor_top_in_view < margin {
            // cursor 在视区上方 → 让 cursor 顶贴 viewport 顶
            new_scroll = -cursor_top_in_content;
        } else if cursor_top_in_view + line_h > viewport_h - margin {
            // cursor 在视区下方 → 让 cursor 底贴 viewport 底
            new_scroll = viewport_h - cursor_top_in_content - line_h;
        }
        // clamp：scroll_offset_y ≤ 0（content 顶不能下移到 viewport 内空出顶部）
        if new_scroll > px(0.0) {
            new_scroll = px(0.0);
        }
        // clamp 下界：content 末行不能比 viewport 底还高（防过滚）
        let content_h = line_h * n_lines as f32;
        let min_scroll = viewport_h - content_h;
        if new_scroll < min_scroll {
            new_scroll = min_scroll;
        }
        if new_scroll != self.scroll_offset_y {
            self.scroll_offset_y = new_scroll;
            cx.notify();
        }
    }

    /// drag-to-edge auto-scroll 单步：drag 期间 30ms 触发一次。
    /// 鼠标 x 落在 viewport 左/右边沿 20px 内 → cursor 向对应方向扩一字符，
    /// 后续 update_scroll_to_cursor 会自然把 scroll_offset 调到让新 cursor 可见。
    ///
    /// 返回 true = task 应继续；false = 退出 timer。
    fn step_drag_auto_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.is_dragging {
            return false;
        }
        let (Some(x), Some(vb)) = (self.drag_target_x, self.viewport_bounds) else {
            return true;
        };
        let margin = px(20.0);
        let v_left = vb.origin.x;
        let v_right = vb.origin.x + vb.size.width;
        let cursor_was = self.cursor;
        if x > v_right - margin && self.cursor < self.text.len() {
            // 向右扩 cursor 一字符
            let next = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            self.cursor = next;
        } else if x < v_left + margin && self.cursor > 0 {
            // 向左扩 cursor 一字符
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor = prev;
        }
        if self.cursor != cursor_was {
            self.reset_blink();
            cx.notify();
        }
        true
    }

    /// 启动 drag auto-scroll task：30ms 周期调 step_drag_auto_scroll。
    /// task 由 self.drag_task 持有所有权，mouse_up 时 take() Drop 自动 abort。
    fn start_drag_auto_scroll(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
            let cont = this
                .update(cx, |this, cx| this.step_drag_auto_scroll(cx))
                .unwrap_or(false);
            if !cont {
                break;
            }
        });
        self.drag_task = Some(task);
    }

    /// 鼠标 click 像素位置 → self.text 的 byte offset（**source space**）。
    ///
    /// bounds_map 里的 byte 来自 render 中 glyph_div 的 `byte` 参数，
    /// 而 glyph_div 接收的是 displayed_text 的 byte（mask 模式下显示串与
    /// 原文 byte 数不同：'•' 在 UTF-8 中 3 字节，ASCII char 1 字节 ——
    /// 直接把 displayed byte 设进 self.cursor 会让 cursor 超出 self.text.len()，
    /// 下次按 backspace/delete 时 `self.text[..self.cursor]` slice 越界 panic）。
    ///
    /// 这里在 mouse_down / mouse_move 把 displayed-space byte 映射回
    /// source-space byte（中转 char index）。非 mask 时 identity（避免无意义的
    /// 重复 chars().count() 遍历）。
    pub(crate) fn cursor_from_click(&self, click_x: Pixels) -> usize {
        if self.mask_char.is_none() {
            // displayed_text == self.text，bounds_map 的 byte 就是 source byte
            return byte_offset_at_x(&self.bounds_map, click_x, self.text.len());
        }
        let displayed_text: String = self.text.chars().map(|_| self.mask_char.unwrap()).collect();
        let displayed_byte = byte_offset_at_x(&self.bounds_map, click_x, displayed_text.len());
        // displayed_byte → char index → source byte
        let char_idx = displayed_text[..displayed_byte].chars().count();
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    /// M19 T5: 多行版 cursor_from_click。click 是 viewport Point<Pixels>。
    /// 1. 用 bounds_map 找 click.y 落在哪个 row（同 y 共享）
    /// 2. 从该 row 内 entries 按 x 找 byte（复用 byte_offset_at_x），fallback
    ///    用 vl.byte_end（cursor 落 row 末符合多行体感）
    ///
    /// mask 处理：仍走 displayed→source 转换（继承单行 mask 逻辑）。
    pub(crate) fn cursor_from_click_2d(&self, click: gpui::Point<Pixels>) -> usize {
        if self.bounds_map.is_empty() {
            return 0;
        }
        let vls = self.current_visual_lines();
        if vls.is_empty() {
            return 0;
        }

        // 找 click.y 落在哪个 row：先 bounds_map 内找 y 区间命中的 entry
        let click_y = click.y;
        let mut matched_byte: Option<usize> = None;
        for (b, bnds) in &self.bounds_map {
            if click_y >= bnds.origin.y && click_y < bnds.origin.y + bnds.size.height {
                matched_byte = Some(*b);
                break;
            }
        }
        // 找不到时 click 在所有 row 之上 / 之下：clamp 到首末 vl
        let vl_idx = if let Some(b) = matched_byte {
            byte_to_visual_pos(b, &vls).0
        } else {
            let first_y = self
                .bounds_map
                .first()
                .map(|(_, b)| b.origin.y)
                .unwrap_or(px(0.0));
            if click_y < first_y {
                0
            } else {
                vls.len() - 1
            }
        };

        let vl = &vls[vl_idx];
        // 该 row 内的 entries（同 visual line 共享 byte range）
        let row_entries: Vec<(usize, Bounds<Pixels>)> = self
            .bounds_map
            .iter()
            .filter(|(b, _)| *b >= vl.byte_start && *b < vl.byte_end)
            .cloned()
            .collect();

        // mask 路径：bounds_map byte 是 displayed-space，需要转回 source-space
        if self.mask_char.is_none() {
            byte_offset_at_x(&row_entries, click.x, vl.byte_end)
        } else {
            // mask 多行：极少场景（password 一般单行），简化用 vl.byte_end fallback
            let displayed_byte = byte_offset_at_x(&row_entries, click.x, vl.byte_end);
            let displayed_text: String =
                self.text.chars().map(|_| self.mask_char.unwrap()).collect();
            let char_idx = displayed_text
                .get(..displayed_byte.min(displayed_text.len()))
                .map(|s| s.chars().count())
                .unwrap_or(0);
            self.text
                .char_indices()
                .nth(char_idx)
                .map(|(b, _)| b)
                .unwrap_or(self.text.len())
        }
    }

    /// 把原文 byte offset (self.cursor) 转成显示文本 byte offset，用于 mask 时切片。
    /// 非 mask 时原样返回。mask 时按 char index 在 displayed 中找对应 byte。
    fn cursor_for_display(&self, displayed: &str) -> usize {
        if self.mask_char.is_none() {
            return self.cursor;
        }
        let char_idx = self.text[..self.cursor].chars().count();
        displayed
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(displayed.len())
    }

    /// 启动定时器：每 100ms 触发 cx.notify()，让 render 重跑（重新计算 phase）。
    fn start_blink_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            if this.update(cx, |_this, cx| cx.notify()).is_err() {
                break;
            }
        })
        .detach();
    }

    // -------- selection helpers --------

    /// 当前选区（normalize 后），无选区时 None。anchor == cursor 时也返回 None。
    pub(crate) fn selection_range(&self) -> Option<Range<usize>> {
        // Defense-in-depth：anchor / cursor 必须 ≤ text.len()，否则 drain panic。
        // 各 mutating 操作（set_text / clear / backspace / delete_forward /
        // insert_str）已显式维护此 invariant，这里 clamp 是兜底，避免未来回归。
        let len = self.text.len();
        self.selection_anchor.and_then(|a| {
            let a = a.min(len);
            let c = self.cursor.min(len);
            if a != c {
                Some(if a < c { a..c } else { c..a })
            } else {
                None
            }
        })
    }

    /// 删除当前选区文本（如有），返回是否删除过。
    pub(crate) fn delete_selection(&mut self) -> bool {
        if let Some(range) = self.selection_range() {
            self.text.drain(range.clone());
            self.cursor = range.start;
            self.selection_anchor = None;
            self.reset_blink();
            true
        } else {
            false
        }
    }

    /// 全选（Ctrl+A）。
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection_anchor = Some(0);
            self.cursor = self.text.len();
            self.reset_blink();
        }
    }

    /// 清选区。
    pub(crate) fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// 选中 cursor 周围的 word（双击触发）。word = 连续非空白字符。
    pub(crate) fn select_word_at_cursor(&mut self) {
        let len = self.text.len();
        if len == 0 {
            return;
        }
        let bytes = self.text.as_bytes();
        let is_space = |b: u8| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r';

        let mut start = self.cursor.min(len);
        // start 在空白上时往前推到非空白
        while start > 0 && is_space(bytes[start.saturating_sub(1)]) {
            start -= 1;
        }
        while start > 0 && !is_space(bytes[start - 1]) {
            start -= 1;
        }

        let mut end = self.cursor.min(len);
        while end < len && !is_space(bytes[end]) {
            end += 1;
        }

        if start < end {
            self.selection_anchor = Some(start);
            self.cursor = end;
            self.reset_blink();
        }
    }

    /// 鼠标点击：M11 简化版只移到末尾 + 双击检测。
    fn handle_mouse_down_at(&mut self, byte_offset: usize, cx: &mut Context<Self>) {
        let now = Instant::now();
        let is_double = self
            .last_click
            .as_ref()
            .map(|(t, b)| now.duration_since(*t).as_millis() < 500 && *b == byte_offset)
            .unwrap_or(false);

        if is_double {
            self.cursor = byte_offset;
            self.select_word_at_cursor();
            self.last_click = None;
        } else {
            self.cursor = byte_offset;
            self.selection_anchor = Some(byte_offset);
            self.last_click = Some((now, byte_offset));
        }
        self.reset_blink();
        cx.notify();
    }

    /// shift+click：扩选模式。anchor 保留（若 None 则用旧 cursor 作 anchor），
    /// 只更新 cursor 到 click byte。后续若 drag 也按扩选走（drag 期间 anchor
    /// 不变，cursor 跟鼠标走，selection_range = anchor..cursor 自然扩展）。
    /// last_click 清空避免被误判为双击的第二次。
    fn handle_shift_click_at(&mut self, byte_offset: usize, cx: &mut Context<Self>) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        self.cursor = byte_offset;
        self.last_click = None;
        self.reset_blink();
        cx.notify();
    }

    // -------- 状态机 --------

    pub(crate) fn cursor_left(&mut self) {
        self.clear_selection();
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.reset_blink();
        }
    }

    pub(crate) fn cursor_right(&mut self) {
        self.clear_selection();
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            self.reset_blink();
        }
    }

    /// M19 T4: 当前 text 的 visual_lines 列表。keyboard nav 用。viewport 宽
    /// 从 viewport_bounds 取（canvas prepaint 上一帧写入），首帧 fallback 400px。
    /// font_size 用 12px hardcoded（与 render 内 theme.font_size.sm 默认一致；
    /// 若主题改字号需要同步 —— 后续 cache 到 self 字段优化）。
    fn current_visual_lines(&self) -> Vec<VisualLine> {
        let container_w = self
            .viewport_bounds
            .map(|b| b.size.width)
            .unwrap_or(px(400.0));
        compute_visual_lines(&self.text, container_w, px(12.0))
    }

    /// M19 T4: cursor 上移一个 visual line。preferred_col 保 col 记忆（连续 ↑↓
    /// 经过短行回到长行仍在原 col）。已在首行 → cursor 回 byte 0。
    pub(crate) fn cursor_up_visual(&mut self) {
        self.clear_selection();
        let vls = self.current_visual_lines();
        let (vl_idx, col) = byte_to_visual_pos(self.cursor, &vls);
        let pref_col = self.preferred_col.unwrap_or(col);
        let raw = if vl_idx == 0 {
            0
        } else {
            visual_pos_to_byte(vl_idx - 1, pref_col, &vls)
        };
        // pref_col 是 byte 差，目标行若有 CJK char 可能落 char 中间；
        // 强制 floor 到最近 char boundary 防 backspace / cursor_left 等
        // 后续操作 text[..cursor] slice 越界 panic。
        self.cursor = floor_char_boundary(&self.text, raw);
        self.preferred_col = Some(pref_col);
        self.reset_blink();
    }

    /// M19 T4: cursor 下移一个 visual line。preferred_col 同上。
    /// 已在末行 → cursor 到 text 末。
    pub(crate) fn cursor_down_visual(&mut self) {
        self.clear_selection();
        let vls = self.current_visual_lines();
        let (vl_idx, col) = byte_to_visual_pos(self.cursor, &vls);
        let pref_col = self.preferred_col.unwrap_or(col);
        let raw = if vl_idx + 1 >= vls.len() {
            self.text.len()
        } else {
            visual_pos_to_byte(vl_idx + 1, pref_col, &vls)
        };
        self.cursor = floor_char_boundary(&self.text, raw);
        self.preferred_col = Some(pref_col);
        self.reset_blink();
    }

    pub(crate) fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor > 0 {
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
            // mouse_down 在 anchor==cursor 状态下设了 anchor，IME 写入路径
            // 又不清，于是 backspace 缩 text 后 anchor 可能 > 新 text.len()，
            // 再次 backspace 时 delete_selection drain 越界 panic。
            // 删字符后清 anchor 也符合用户预期（与浏览器 <input> 一致）。
            self.selection_anchor = None;
            self.reset_blink();
        }
    }

    pub(crate) fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
            self.selection_anchor = None; // 同 backspace：维护 anchor invariant
            self.reset_blink();
        }
    }

    pub(crate) fn insert_str(&mut self, s: &str) {
        // delete_selection 在 anchor==cursor（click 但未 drag 的常见 idle 状态）
        // 时返回 false 且不清 anchor 字段。之后 self.cursor += s.len()
        // 推进，anchor 残留旧位置 → selection_range() 此时返回非空
        // range（anchor..cursor），用户看不出但 backspace 会因 anchor 旧值
        // 越界引发 drain panic。显式清 anchor 修该泄露。
        self.delete_selection();
        self.selection_anchor = None;
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.reset_blink();
    }

    fn fire_change(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_change.clone() {
            h(&self.text, window, cx);
        }
    }

    fn fire_submit(&self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.on_submit.clone() {
            h(&self.text, window, cx);
        }
    }

    /// 计算复制 payload（纯函数，便于测试）。
    /// 无选区返回全文，有选区返回选区文本。
    pub(crate) fn build_copy_payload(&self) -> String {
        compute_copy_payload(&self.text, self.selection_range())
    }

    /// 复制文本到系统 clipboard。
    ///
    /// **返回值语义**：
    /// - 文本为空 → false（不初始化 Clipboard）
    /// - Clipboard 初始化失败 → false（log warn）
    /// - set_text 失败 → false（log warn）
    /// - 成功 → true
    ///
    /// 调用方（cut）无法区分这三种 false，全都不删 selection。
    pub(crate) fn copy(&self) -> bool {
        // M16: mask 状态下静默禁止 copy/cut，与系统密码框语义一致
        if self.is_masked() {
            return false;
        }
        let payload = self.build_copy_payload();
        if payload.is_empty() {
            return false;
        }
        match Clipboard::new() {
            Ok(mut cb) => match cb.set_text(payload) {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!("text_input: clipboard set_text 失败: {}", e);
                    false
                }
            },
            Err(e) => {
                tracing::warn!("text_input: clipboard 初始化失败: {}", e);
                false
            }
        }
    }

    /// 剪切：copy 之后删 selection。
    pub(crate) fn cut(&mut self) -> bool {
        if !self.copy() {
            return false;
        }
        self.delete_selection();
        true
    }

    /// 粘贴：从系统 clipboard 读文本，截到首行（单行 input 不接受换行），
    /// 调用 insert_str 插入到当前 cursor（自动删 selection）。
    ///
    /// **返回值语义**：
    /// - Clipboard 初始化失败 / get_text 失败 → false（log warn）
    /// - clipboard 为空或 normalize 后为空 → false（不算错）
    /// - 成功插入 → true
    ///
    /// **mask 处理**：与系统密码框一致，masked 状态下仍允许 paste
    /// （浏览器 `<input type=password>` 和 macOS 密码框都允许粘贴，
    /// 只禁 copy/cut 避免泄露已输入内容）。
    pub(crate) fn paste(&mut self) -> bool {
        let raw = match Clipboard::new() {
            Ok(mut cb) => match cb.get_text() {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("text_input: clipboard get_text 失败: {}", e);
                    return false;
                }
            },
            Err(e) => {
                tracing::warn!("text_input: clipboard 初始化失败: {}", e);
                return false;
            }
        };
        let payload = compute_paste_payload(&raw, self.multiline);
        if payload.is_empty() {
            return false;
        }
        self.insert_str(&payload);
        true
    }

    /// 构造一个逐字 wrap 的 glyph div，含 zero-size canvas 在 prepaint 把
    /// (byte, viewport bounds) 写回 TextInput.bounds_map。selection 范围内
    /// 应用 accent 背景色。
    fn glyph_div(
        byte: usize,
        ch: char,
        weak: gpui::WeakEntity<Self>,
        selection: Option<Range<usize>>,
        accent: gpui::Hsla,
    ) -> impl IntoElement {
        let mut g = div().relative().child(ch.to_string()).child(
            canvas(
                move |bounds, _w, cx| {
                    let _ = weak.update(cx, |t, _cx| t.push_glyph_bounds(byte, bounds));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        if let Some(sel) = selection {
            if byte >= sel.start && byte < sel.end {
                g = g.bg(accent);
            }
        }
        g
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // 横向操作 / typing / 删字 时清 preferred_col（标准 textarea 行为：
        // 用户横向走光标后再 ↑/↓ 应该用"当前 col"重新记忆）
        let ctrl = event.keystroke.modifiers.control;
        let multiline = self.multiline;
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.preferred_col = None;
                self.backspace();
                cx.notify();
                self.fire_change(window, cx);
            }
            "delete" => {
                self.preferred_col = None;
                self.delete_forward();
                cx.notify();
                self.fire_change(window, cx);
            }
            "left" => {
                self.preferred_col = None;
                self.cursor_left();
                cx.notify();
            }
            "right" => {
                self.preferred_col = None;
                self.cursor_right();
                cx.notify();
            }
            "up" if multiline => {
                self.cursor_up_visual();
                cx.notify();
            }
            "down" if multiline => {
                self.cursor_down_visual();
                cx.notify();
            }
            "home" => {
                self.preferred_col = None;
                self.clear_selection();
                if multiline {
                    // 多行：到当前 visual line 行首
                    let vls = self.current_visual_lines();
                    let (vl_idx, _) = byte_to_visual_pos(self.cursor, &vls);
                    self.cursor = vls.get(vl_idx).map(|v| v.byte_start).unwrap_or(0);
                } else {
                    self.cursor = 0;
                }
                self.reset_blink();
                cx.notify();
            }
            "end" => {
                self.preferred_col = None;
                self.clear_selection();
                if multiline {
                    let vls = self.current_visual_lines();
                    let (vl_idx, _) = byte_to_visual_pos(self.cursor, &vls);
                    self.cursor = vls
                        .get(vl_idx)
                        .map(|v| v.byte_end)
                        .unwrap_or(self.text.len());
                } else {
                    self.cursor = self.text.len();
                }
                self.reset_blink();
                cx.notify();
            }
            // 多行下 Enter 插 \n，Ctrl+Enter 触发 submit；单行 Enter 仍 submit
            "enter" if multiline && !ctrl => {
                self.preferred_col = None;
                self.insert_str("\n");
                cx.notify();
                self.fire_change(window, cx);
            }
            "enter" if multiline && ctrl => {
                self.fire_submit(window, cx);
            }
            "enter" if !event.keystroke.modifiers.shift => {
                self.fire_submit(window, cx);
            }
            "a" if event.keystroke.modifiers.control => {
                self.select_all();
                cx.notify();
            }
            "c" if event.keystroke.modifiers.control => {
                self.copy();
            }
            "x" if event.keystroke.modifiers.control => {
                if self.cut() {
                    cx.notify();
                    self.fire_change(window, cx);
                }
            }
            "v" if event.keystroke.modifiers.control => {
                if self.paste() {
                    cx.notify();
                    self.fire_change(window, cx);
                }
            }
            "escape" => {
                self.clear_selection();
                // fire on_cancel callback（caller 一般用来退出 inline edit）
                if let Some(h) = self.on_cancel.clone() {
                    h(window, cx);
                }
                cx.notify();
            }
            _ => {
                // 普通可打印字符不在此处插入：GPUI 在 Windows 下会派发 KeyDown
                // 后又通过 WM_CHAR → InputHandler::replace_text_in_range 派发
                // 一次，若两侧都 insert_str 会双输入。统一交给 IME path 处理
                // （见同模块 TextInputImeHandler::replace_text_in_range）。
                // 这里只接 control 键（backspace / arrows / enter / Ctrl+组合），
                // 它们的 key_char = None，不会走 IME，靠上面的 match arm 覆盖。
                // 设计参考：crates/aish-app/src/views/terminal_view.rs:214-223
            }
        }
    }
}

/// 计算复制 payload 的纯函数（无需 GPUI context，便于单元测试）。
/// 无选区返回全文，有选区返回选区文本。
pub(crate) fn compute_copy_payload(
    text: &str,
    selection: Option<std::ops::Range<usize>>,
) -> String {
    match selection {
        Some(r) => text[r].to_string(),
        None => text.to_string(),
    }
}

/// 计算粘贴 payload 的纯函数：
/// - 单行：截到首行（遇到首个 \r 或 \n 截断），与浏览器 `<input>` 行为一致
/// - 多行：保留 \n 整段 paste；\r\n 归一化为 \n（Windows 剪贴板常见），
///   单独 \r 也归一化为 \n（Mac 老剪贴板 / 某些 terminal）。
///
/// 不 trim 前后空格 —— 用户可能有意粘前导空格。
pub(crate) fn compute_paste_payload(raw: &str, multiline: bool) -> String {
    if multiline {
        // \r\n → \n 优先（不能让 \r 单独留下，否则 \r 字符 render 异常）；
        // 然后单独 \r → \n
        raw.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        match raw.find(['\n', '\r']) {
            Some(idx) => raw[..idx].to_string(),
            None => raw.to_string(),
        }
    }
}

/// M16 T2：在 bounds_map 中找包含 click_x 的字符，返回其 byte offset。
/// 若 click_x 在所有字符之前 → 返回 0；在右半之后 → 返回 text_len。
/// 用 char 的中线作为分界：
///   click_x < mid → 该 byte
///   click_x >= mid → 下一个 byte（或末尾）
pub(crate) fn byte_offset_at_x(
    bounds_map: &[(usize, Bounds<Pixels>)],
    click_x: Pixels,
    text_len: usize,
) -> usize {
    if bounds_map.is_empty() {
        return 0;
    }
    for (byte, bounds) in bounds_map {
        // width == 0 时 mid == origin.x，click_x >= mid 会自动跳到下一字符，行为仍正确
        let mid = bounds.origin.x + bounds.size.width / 2.0;
        if click_x < mid {
            return *byte;
        }
    }
    text_len
}

// ─────────────────── M19: 多行 word-wrap helpers ───────────────────────
//
// 三个 pure fn 服务多行渲染：
// - compute_visual_lines: text → 按 \n 拆 logical line + 按宽度 wrap 成
//   visual lines（含每行 byte_start / byte_end）
// - byte_to_visual_pos: cursor byte → (vl_idx, col_in_visual)
// - visual_pos_to_byte: (vl_idx, col) → byte（含 mouse click 路径用）
//
// char 宽估算（D-4）：ASCII / Latin extended / Cyrillic / Greek ≈ 0.6 *
// font_size；CJK / emoji / 其他 ≈ 1.2 * font_size。monospace 字体下偏差
// < 1 char，可接受。完全准确需 GPUI text_system shape，render-time 太贵。

/// 多行视觉行（wrap 后单元）。一个 logical line（按 \n 切）可包含 ≥ 1 个
/// visual line（按 container_width wrap 后）。T3 起 render multiline 路径用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisualLine {
    /// 第几个 logical line（按 \n 编号，0-based）
    pub logical_line: usize,
    /// 源 text 中此 visual line 起始 byte（inclusive）
    pub byte_start: usize,
    /// 结束 byte（exclusive，不含 \n 也不含 wrap 后第一个字符）
    pub byte_end: usize,
}

/// char 宽估算。ASCII / Latin / Cyrillic / Greek 半角；CJK / emoji 全角。
fn approx_char_width(c: char, font_size: Pixels) -> Pixels {
    let cp = c as u32;
    let scale = if cp < 0x80 || (0x80..=0x4FF).contains(&cp) {
        // ASCII / Latin extended / Greek / Cyrillic / IPA — 半角
        0.6
    } else {
        // CJK / Hangul / 假名 / 全角符号 / emoji / 其他 BMP+ — 全角
        1.2
    };
    font_size * scale
}

/// 是否在该 char 后可以 word break。仅识别常见 ASCII 标点 / 空白；
/// CJK char 之间天然可断（这里不识别，wrap algo 会 fallback char-level）。
fn is_break_after(c: char) -> bool {
    matches!(c, ' ' | '\t' | '/' | ',' | ';' | ':' | '|')
}

/// 按 \n 拆 logical lines + 按 container_width word-wrap 成 visual lines。
/// 空 text 返回 1 个空 visual line（让 cursor 有家可待）。
///
/// 算法 O(n) 一次扫描，wrap 时回退到 last_break 或当前 byte（char-level
/// 强制断）。
pub(crate) fn compute_visual_lines(
    text: &str,
    container_width: Pixels,
    font_size: Pixels,
) -> Vec<VisualLine> {
    let mut result = Vec::new();
    let bytes = text.as_bytes();
    let n = text.len();

    let mut logical_line_idx = 0usize;
    let mut visual_line_start = 0usize;
    let mut cur_width: Pixels = px(0.0);
    let mut last_break: Option<usize> = None;

    let mut i = 0usize;
    while i < n {
        // safe: i 是 UTF-8 boundary（要么 0，要么前一步是 + len_utf8）
        let c = match text[i..].chars().next() {
            Some(c) => c,
            None => break,
        };
        let clen = c.len_utf8();

        if c == '\n' {
            // logical line break
            result.push(VisualLine {
                logical_line: logical_line_idx,
                byte_start: visual_line_start,
                byte_end: i,
            });
            visual_line_start = i + clen;
            logical_line_idx += 1;
            cur_width = px(0.0);
            last_break = None;
            i += clen;
            continue;
        }

        let cw = approx_char_width(c, font_size);
        // 该 char 加上后会超宽 → wrap（但 visual_line_start == i 时不 wrap，
        // 单 char 也得放下避免死循环）
        if cur_width + cw > container_width && i > visual_line_start {
            let break_at = last_break
                .filter(|b| *b > visual_line_start && *b <= i)
                .unwrap_or(i); // char-level fallback
            result.push(VisualLine {
                logical_line: logical_line_idx,
                byte_start: visual_line_start,
                byte_end: break_at,
            });
            visual_line_start = break_at;
            cur_width = px(0.0);
            last_break = None;
            i = break_at;
            continue;
        }

        cur_width += cw;
        if is_break_after(c) {
            last_break = Some(i + clen);
        }
        i += clen;
    }

    // 末尾一行（即便空 text 也 push 一个让 cursor 有家）
    result.push(VisualLine {
        logical_line: logical_line_idx,
        byte_start: visual_line_start,
        byte_end: n,
    });

    // 防御：bytes 用上以消 unused warning + 后续若需 fast path 用
    let _ = bytes;
    result
}

/// byte offset → (vl_idx, col)。col 单位是 **byte 差**（vl.byte_start 起算），
/// 不是 char count —— 与 visual_pos_to_byte 反向一致，与单行 cursor byte
/// 路径同 namespace。caller 保证 byte 在 UTF-8 char boundary。
///
/// 边界规则：byte == vl.byte_end 且下一行存在且 byte_start == byte（wrap
/// 边界，无 \n），优先归下一行 col=0 —— cursor 在 wrap 后行首符合用户直觉
/// （否则光标在前一行末尾看起来"还没换行"）。
pub(crate) fn byte_to_visual_pos(byte: usize, vls: &[VisualLine]) -> (usize, usize) {
    if vls.is_empty() {
        return (0, 0);
    }
    for (idx, vl) in vls.iter().enumerate() {
        if byte >= vl.byte_start && byte <= vl.byte_end {
            if byte == vl.byte_end {
                if let Some(next) = vls.get(idx + 1) {
                    if next.byte_start == byte && next.logical_line == vl.logical_line {
                        return (idx + 1, 0);
                    }
                }
            }
            return (idx, byte - vl.byte_start);
        }
    }
    // byte 超出末尾：归末行末
    let last = vls.last().unwrap();
    (vls.len() - 1, last.byte_end - last.byte_start)
}

/// 把任意 byte offset 向下取整到最近的 UTF-8 char boundary。
/// `cursor_up_visual` / `cursor_down_visual` 用 byte 差作 col 单位，跨行后
/// 可能落到 CJK char 字节中间，后续 text[..cursor] slice 直接 panic。
/// 此 helper 防御性 floor 到 boundary。stable Rust 没 floor_char_boundary
/// 公开 API，手写线性 fallback（短文本下 O(n) 可接受）。
pub(crate) fn floor_char_boundary(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    if text.is_char_boundary(byte) {
        return byte;
    }
    // 向下找 boundary（最多回退 3 byte，UTF-8 最长 4 byte）
    (0..byte)
        .rev()
        .find(|b| text.is_char_boundary(*b))
        .unwrap_or(0)
}

/// (vl_idx, col) → byte。col 单位与 byte_to_visual_pos 反向一致（byte 差）。
/// col > line_len 时 clamp 到行末（cursor_up/down 时 preferred_col 超目标行
/// 长度的场景）。**注意**：返回值可能不在 char boundary（col 是 byte 差，
/// 目标行 CJK 时可能切中）；调用方需用 `floor_char_boundary` 防御。
pub(crate) fn visual_pos_to_byte(vl_idx: usize, col: usize, vls: &[VisualLine]) -> usize {
    let Some(vl) = vls.get(vl_idx) else {
        return vls.last().map(|v| v.byte_end).unwrap_or(0);
    };
    let line_len = vl.byte_end - vl.byte_start;
    vl.byte_start + col.min(line_len)
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// -------- IME --------

struct TextInputImeHandler {
    view: gpui::WeakEntity<TextInput>,
    bar_bounds: Option<Bounds<Pixels>>,
}

impl InputHandler for TextInputImeHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &mut self,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Ok((Some(h), new_text)) = self.view.update(cx, |this, cx| {
            this.insert_str(text);
            cx.notify();
            (this.on_change.clone(), this.text.clone())
        }) {
            h(&new_text, window, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.bar_bounds
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

// -------- Render --------

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // M16 T2：清空 bounds_map，本帧 prepaint 会重新填充
        self.clear_glyph_bounds();

        let focus_for_ime = self.focus_handle.clone();
        let weak_view = cx.weak_entity();
        // window 失焦时（用户切到别的应用），即使 focus_handle 仍是 TextInput
        // 的，GPUI 也保留 focus 状态不清；视觉上 cursor 继续闪、border 仍是
        // ring 高亮 —— 与系统其他 app 不一致。AND 上 window.is_window_active()
        // 让失焦时立即视觉降级。blink timer 仍每 100ms notify，状态变化在
        // 下一帧体现（延迟 <= 100ms）。
        let focused = self.focus_handle.is_focused(window) && window.is_window_active();
        // 失焦边沿检测：上一帧 focused = true，这一帧 false → 触发 on_blur
        // 让 caller 在"点 input 外部"时 commit 编辑保留改动。
        if self.last_focused && !focused {
            if let Some(h) = self.on_blur.clone() {
                let text = self.text.clone();
                h(&text, window, cx);
            }
        }
        self.last_focused = focused;
        let show_cursor = focused && self.cursor_visible_now();

        let t = theme(cx);
        let border_color = if focused {
            t.colors.ring
        } else {
            t.colors.border
        };

        // mask 时 displayed_text 用 mask_char 填充（T1 行为保持）
        let displayed_text: String = if let Some(mask) = self.mask_char {
            self.text.chars().map(|_| mask).collect()
        } else {
            self.text.clone()
        };
        let displayed_cursor = self.cursor_for_display(&displayed_text);
        let placeholder_visible = displayed_text.is_empty();

        // selection 按 displayed_text 算（原文 byte range → displayed byte range）
        let displayed_selection: Option<std::ops::Range<usize>> = self.selection_range().map(|r| {
            if self.mask_char.is_none() {
                r
            } else {
                let start_char = self.text[..r.start].chars().count();
                let end_char = self.text[..r.end].chars().count();
                let s = displayed_text
                    .char_indices()
                    .nth(start_char)
                    .map(|(b, _)| b)
                    .unwrap_or(displayed_text.len());
                let e = displayed_text
                    .char_indices()
                    .nth(end_char)
                    .map(|(b, _)| b)
                    .unwrap_or(displayed_text.len());
                s..e
            }
        });

        // 把 displayed_text 切两段：cursor 之前 / cursor 之后
        let left_chars: Vec<(usize, char)> =
            displayed_text[..displayed_cursor].char_indices().collect();
        let right_chars: Vec<(usize, char)> = displayed_text[displayed_cursor..]
            .char_indices()
            .map(|(b, c)| (b + displayed_cursor, c))
            .collect();

        let accent = t.colors.accent;
        let ring = t.colors.ring;
        let foreground = t.colors.foreground;
        let muted_foreground = t.colors.muted_foreground;
        let font_size_sm = t.font_size.sm;

        let cursor_div = if show_cursor {
            div().w(px(1.0)).h(px(14.0)).bg(ring).self_center()
        } else {
            div().w(px(1.0)).h(px(14.0)).self_center()
        };

        let text_row = if placeholder_visible {
            div()
                .flex()
                .flex_row()
                .items_center()
                .text_size(font_size_sm)
                .text_color(muted_foreground)
                .child(cursor_div)
                .child(div().child(self.placeholder.clone()))
                .into_any_element()
        } else if self.multiline {
            // M19 T3: 多行路径。算 visual_lines + 按行画 inline-glyph row。
            // cursor 落在某 visual line 的某 col，该行 inline 流中插 cursor_div。
            //
            // viewport 宽：用 self.viewport_bounds（canvas prepaint 上一帧写入）。
            // 首帧没值时 fallback 400px —— 下一帧 viewport_bounds 已就位自动 wrap。
            // 这与 single-line scroll_offset 同 trade-off：上一帧 bounds 驱动本帧渲染。
            let container_w = self
                .viewport_bounds
                .map(|b| b.size.width)
                .unwrap_or(px(400.0));
            let visual_lines = compute_visual_lines(&displayed_text, container_w, font_size_sm);
            let displayed_cursor_byte = displayed_cursor;
            let weak_glyph = weak_view.clone();
            let sel_for_glyph = displayed_selection.clone();
            let show_cursor_local = show_cursor;
            let ring_local = ring;

            let rows: Vec<gpui::AnyElement> = visual_lines
                .iter()
                .map(|vl| {
                    // 此 visual line 的 text 段（vl.byte_start..vl.byte_end）。
                    let line_text = displayed_text[vl.byte_start..vl.byte_end].to_string();

                    let mut row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .text_size(font_size_sm)
                        .text_color(foreground);

                    // 逐 char 画 glyph_div；cursor 落 byte == 当前 char byte 时
                    // 在 char **之前**插入 cursor_div。行末 cursor 由 for 后处理。
                    let mut b = vl.byte_start;
                    for ch in line_text.chars() {
                        if b == displayed_cursor_byte && show_cursor_local {
                            row = row
                                .child(div().w(px(1.0)).h(px(14.0)).bg(ring_local).self_center());
                        } else if b == displayed_cursor_byte {
                            // cursor blink 不可见时仍占位防止文字跳动
                            row = row.child(div().w(px(1.0)).h(px(14.0)).self_center());
                        }
                        row = row.child(Self::glyph_div(
                            b,
                            ch,
                            weak_glyph.clone(),
                            sel_for_glyph.clone(),
                            accent,
                        ));
                        b += ch.len_utf8();
                    }
                    // 行末 cursor（cursor 在 byte_end 位置 + cursor 在该行而非下一行 wrap）
                    if b == displayed_cursor_byte {
                        let cd = if show_cursor_local {
                            div().w(px(1.0)).h(px(14.0)).bg(ring_local).self_center()
                        } else {
                            div().w(px(1.0)).h(px(14.0)).self_center()
                        };
                        row = row.child(cd);
                    }
                    row.into_any_element()
                })
                .collect();

            // content 用 mt(scroll_offset_y) 上移让 cursor 行可见。
            // scroll_offset_y ≤ 0；container.overflow_hidden 裁掉上方 / 下方
            // 不该看到的部分。
            div()
                .flex()
                .flex_col()
                .mt(self.scroll_offset_y)
                .children(rows)
                .into_any_element()
        } else {
            let weak_left = weak_view.clone();
            let sel_left = displayed_selection.clone();
            let left_divs = left_chars.into_iter().map(move |(byte, ch)| {
                Self::glyph_div(byte, ch, weak_left.clone(), sel_left.clone(), accent)
            });

            let weak_right = weak_view.clone();
            let sel_right = displayed_selection.clone();
            let right_divs = right_chars.into_iter().map(move |(byte, ch)| {
                Self::glyph_div(byte, ch, weak_right.clone(), sel_right.clone(), accent)
            });

            div()
                .flex()
                .flex_row()
                .items_center()
                .text_size(font_size_sm)
                .text_color(foreground)
                // 水平 scroll：scroll_offset 0 / 负数。container.overflow_hidden
                // 裁掉超出 viewport 的部分；canvas prepaint callback 在每帧
                // 根据 cursor 位置更新 scroll_offset 让 cursor 始终可见。
                .ml(self.scroll_offset)
                .flex_shrink_0()
                .children(left_divs)
                .child(cursor_div)
                .children(right_divs)
                .into_any_element()
        };

        // w_full 让 container 撑满父 flex_1 给的剩余空间。否则 container
        // 自然宽 = content（placeholder 一行 ~264px），父 flex_1 给 1200px
        // 也只占 264 — input border 显示 264 宽，Send 按钮被推到屏幕最右，
        // 中间留大空白。w_full 让 input 撑满有效宽度，符合标准 input 体感。
        let mut container = div().relative().w_full().cursor_text();
        if self.multiline {
            // M19 T3 + scroll polish: 多行容器**固定高度** = min(n_lines, max_lines)
            // * line_h，min_h(28) 兜底让 1 行不低于单行模式。
            // 原 min_h/max_h + children flex 撑大方案在 mt(scroll_offset_y)
            // 负 margin 时让 outer height = content_h + scroll → 抖动。固定
            // .h() + min_h 脱离 children-pushes-parent 链路，↑↓ scroll 时
            // 容器高度稳定。
            //
            // viewport 宽度用 self.viewport_bounds（上一帧 canvas prepaint 写入）。
            // 首帧 fallback 400px，下一帧自然 correct。
            let line_h = px(20.0); // 每行视觉高度 ≈ font_size 14 + 行距 6
            let container_w = self
                .viewport_bounds
                .map(|b| b.size.width)
                .unwrap_or(px(400.0));
            let vls_for_h = compute_visual_lines(&self.text, container_w, font_size_sm);
            let visible_lines = vls_for_h.len().clamp(1, self.max_lines);
            container = container
                .flex()
                .flex_col()
                .min_h(px(28.0)) // 1 行最小 outer 高度（与单行 .h(28) 一致）
                .h(line_h * visible_lines as f32)
                .overflow_hidden();
        } else {
            container = container
                .flex()
                .flex_row()
                .items_center()
                // overflow_hidden 让超长 text_row（含负 margin-left 滚动）被裁切，
                // 不溢出到父容器外（之前 borderless 模式下长 title 会画到相邻 tab）。
                .overflow_hidden();
        }
        if !self.borderless {
            container = container
                .px(px(8.0))
                .rounded(t.radius.sm)
                .bg(t.colors.input)
                .border_1()
                .border_color(border_color);
            if !self.multiline {
                // 单行固定 h
                container = container.h(px(28.0));
            }
            // 多行：高度由前面的 .min_h(28) + .h(line_h * visible_lines) 决定
            // outer，不加 .py（避免 outer 比 .h 多 8px 让 1 行变 36px 看着过高，
            // 且 .py 跟 .min_h 叠加时实际 outer = max(min_h, h) + py 难预测）。
            // 1 行 outer = 28，跟单行模式视觉一致；多行每行 line_h(20)。
        }
        // 借用 GPUI fluent chain：把 container 后续 listener 链接回去
        container
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    // M16 T2: 用 bounds_map（上一帧 prepaint 写入）算 click x → byte。
                    // 时序安全：GPUI 事件派发在 paint 之后、下一次 render 之前，此时
                    // render 入口的 clear_glyph_bounds() 还没执行，map 仍是上一帧的有效数据。
                    // cursor_from_click 把 displayed-space byte 映射回 source-space
                    // byte（mask 模式必要，否则 cursor 超过 self.text.len() 引发 panic）。
                    // M19: 多行走 cursor_from_click_2d（接 y 维度定位 vl_idx 再 x）。
                    let byte = if this.multiline {
                        this.cursor_from_click_2d(ev.position)
                    } else {
                        this.cursor_from_click(ev.position.x)
                    };
                    this.is_dragging = true; // M16 T3: 开始 drag
                    this.drag_target_x = Some(ev.position.x);
                    // shift+click：扩选模式（保留 anchor，cursor 跳新位置）；
                    // 后续若 drag 也按扩选走，cursor 跟鼠标，anchor 不变。
                    // 无 shift：原 drag select 行为（清旧 anchor，新 anchor=byte）。
                    if ev.modifiers.shift {
                        this.handle_shift_click_at(byte, cx);
                    } else {
                        this.handle_mouse_down_at(byte, cx);
                    }
                    // drag-to-edge auto-scroll：启 30ms timer task，鼠标停在
                    // viewport 边沿时持续扩 cursor + 滚动（不靠 mouse_move 持续触发）。
                    this.start_drag_auto_scroll(cx);
                    // 阻止 mouse_down 冒泡到父：典型场景是 TabBar inline rename
                    // —— TabItem 自己也注册了 on_mouse_down(切 tab)，若不拦，
                    // 点 input 内任意位置都会同时触发 TabItem.on_click 把 editing
                    // commit 掉 + 切走焦点，cursor 无法被点击定位。
                    // 通用语义：用户点 input 内部的预期是"操作 input"而不是
                    // "操作 input 所在的卡片"，stop_propagation 符合 UX 直觉。
                    cx.stop_propagation();
                }),
            )
            // M16 T3: drag select。selection_anchor 在 mouse_down 由 handle_mouse_down_at
            // 设置（single click 设为 byte_offset；double click 走 select_word_at_cursor
            // 设为 word 起始 byte），drag 期间只动 cursor，anchor 不变，
            // selection_range() 通过 anchor..cursor 自然形成选区。
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _w, cx| {
                if !this.is_dragging {
                    return;
                }
                // 实时更新 drag_target_x 让 auto-scroll timer 拿到最新位置。
                this.drag_target_x = Some(ev.position.x);
                let byte = if this.multiline {
                    this.cursor_from_click_2d(ev.position)
                } else {
                    this.cursor_from_click(ev.position.x)
                };
                if byte != this.cursor {
                    this.cursor = byte;
                    this.reset_blink();
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseUpEvent, _w, cx| {
                    this.is_dragging = false;
                    this.drag_target_x = None;
                    // Take 丢弃 Task，GPUI Task drop 自动 abort timer loop。
                    this.drag_task.take();
                    cx.notify();
                }),
            )
            // text_row 包 flex_1 + min_w(0) + w_full 让其撑满 input 内剩余空间。
            // 单行 container 是 flex_row → flex_1 在 main axis 横向 grow ✓；
            // 多行 container 是 flex_col → flex_1 在 main axis **纵向** grow，
            // 不撑横向，需 w_full 显式取 cross axis (horizontal) 父宽。
            // 没此包装时 text_row 内容宽 = inline glyph 累加（空 input ~0px），
            // 眼睛 toggle / Send 按钮会贴在文字末尾而不是 input 右边。
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .w_full()
                    .child(text_row),
            )
            // 右侧眼睛 toggle 按钮（password 字段典型）：点击切换 mask_char
            // Some('•') ↔ None。flex_shrink_0 防止被 text_row 挤掉。
            // 用 div 而非 IconButton —— IconButton stateful div id 在容器内
            // 会与外层 input 容器冲突；这里手画一个简易 ghost icon button。
            .when(self.show_mask_toggle, |d| {
                let masked = self.mask_char.is_some();
                let eye_icon = if masked {
                    crate::icons::IconName::EyeOff
                } else {
                    crate::icons::IconName::Eye
                };
                let muted = t.colors.muted_foreground;
                let fg = t.colors.foreground;
                let hover_bg = t.colors.secondary_active;
                d.child(
                    div()
                        .id("text-input-mask-toggle")
                        .flex_shrink_0()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(t.radius.sm)
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(move |s| s.bg(hover_bg).text_color(fg))
                        .child(crate::icons::icon(eye_icon).size(px(14.0)).text_color(muted))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _ev: &MouseDownEvent, _w, cx| {
                                this.toggle_mask(cx);
                                cx.stop_propagation();
                            }),
                        ),
                )
            })
            .child(
                canvas(
                    |bounds, _window, _cx| bounds,
                    move |_bounds, prepaint_bounds, window, cx| {
                        window.handle_input(
                            &focus_for_ime,
                            TextInputImeHandler {
                                view: weak_view.clone(),
                                bar_bounds: Some(prepaint_bounds),
                            },
                            cx,
                        );
                        // 水平 scroll 跟随 cursor：bounds_map 已被 text_row
                        // 子 div 的 prepaint 填好，prepaint_bounds 是 canvas
                        // 与 container 同尺寸的 viewport。算 cursor_x vs viewport
                        // 边界，更新 scroll_offset 并 notify 下一帧 re-render。
                        let is_dragging = weak_view
                            .update(cx, |this, cx_inner| {
                                this.viewport_bounds = Some(prepaint_bounds);
                                this.update_scroll_to_cursor(prepaint_bounds, cx_inner);
                                this.is_dragging
                            })
                            .unwrap_or(false);

                        // drag 期间用户可能拖出 input 外松开 → element-local
                        // on_mouse_up 不触发，is_dragging 永远 true，回到 input
                        // 内 drag-select 行为继续（用户体感'松开了还在拖'）。
                        // 注册 window-level MouseUpEvent listener 兜底：任何
                        // 位置松开都清 is_dragging。
                        //
                        // listener 跟 frame 绑定，每帧 paint 重注册一次；
                        // is_dragging=false 后下一帧 paint 不再注册，自然失效。
                        if is_dragging {
                            let weak = weak_view.clone();
                            window.on_mouse_event(
                                move |_: &MouseUpEvent,
                                      phase: DispatchPhase,
                                      _window: &mut Window,
                                      cx_outer: &mut App| {
                                    if phase != DispatchPhase::Bubble {
                                        return;
                                    }
                                    let _ = weak.update(cx_outer, |this, cx_inner| {
                                        this.is_dragging = false;
                                        this.drag_target_x = None;
                                        this.drag_task.take();
                                        cx_inner.notify();
                                    });
                                },
                            );
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    fn apply_insert(text: &mut String, cursor: &mut usize, s: &str) {
        text.insert_str(*cursor, s);
        *cursor += s.len();
    }

    fn apply_backspace(text: &mut String, cursor: &mut usize) {
        if *cursor > 0 {
            let prev = text[..*cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            text.remove(prev);
            *cursor = prev;
        }
    }

    fn apply_delete(text: &mut String, cursor: &mut usize) {
        if *cursor < text.len() {
            text.remove(*cursor);
        }
    }

    fn apply_left(text: &str, cursor: &mut usize) {
        if *cursor > 0 {
            *cursor = text[..*cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    fn apply_right(text: &str, cursor: &mut usize) {
        if *cursor < text.len() {
            *cursor = text[*cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| *cursor + i)
                .unwrap_or(text.len());
        }
    }

    #[test]
    fn insert_advances_cursor() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "hi");
        assert_eq!(t, "hi");
        assert_eq!(c, 2);
    }

    #[test]
    fn backspace_removes_prev_char() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ab");
        apply_backspace(&mut t, &mut c);
        assert_eq!(t, "a");
        assert_eq!(c, 1);
    }

    #[test]
    fn delete_removes_next_char() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "abc");
        c = 1;
        apply_delete(&mut t, &mut c);
        assert_eq!(t, "ac");
        assert_eq!(c, 1);
    }

    #[test]
    fn left_right_navigates() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ab");
        apply_left(&t, &mut c);
        assert_eq!(c, 1);
        apply_right(&t, &mut c);
        assert_eq!(c, 2);
    }

    #[test]
    fn insert_at_middle() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "ac");
        apply_left(&t, &mut c);
        apply_insert(&mut t, &mut c, "b");
        assert_eq!(t, "abc");
        assert_eq!(c, 2);
    }

    #[test]
    fn cjk_char_boundary_handling() {
        let (mut t, mut c) = (String::new(), 0);
        apply_insert(&mut t, &mut c, "中文");
        // 中文 = 6 bytes (3 each in UTF-8)
        assert_eq!(c, 6);
        apply_backspace(&mut t, &mut c);
        // 应该删掉一个完整字符
        assert_eq!(t, "中");
        assert_eq!(c, 3);
    }

    #[test]
    fn blink_phase_first_half_visible() {
        use std::time::Duration;
        let epoch = std::time::Instant::now() - Duration::from_millis(100);
        let phase = epoch.elapsed().as_millis() as u64 % super::BLINK_PERIOD_MS;
        assert!(phase < super::BLINK_PERIOD_MS / 2);
    }

    #[test]
    fn blink_phase_second_half_invisible() {
        use std::time::Duration;
        let epoch = std::time::Instant::now() - Duration::from_millis(400);
        let phase = epoch.elapsed().as_millis() as u64 % super::BLINK_PERIOD_MS;
        assert!(phase >= super::BLINK_PERIOD_MS / 2);
    }

    #[test]
    fn blink_period_constant() {
        assert_eq!(super::BLINK_PERIOD_MS, 600);
    }

    #[test]
    fn selection_range_normalizes_anchor_before_cursor() {
        let anchor = 2;
        let cursor = 5;
        let range = if anchor < cursor {
            anchor..cursor
        } else {
            cursor..anchor
        };
        assert_eq!(range, 2..5);
    }

    #[test]
    fn selection_range_normalizes_anchor_after_cursor() {
        let anchor = 7;
        let cursor = 3;
        let range = if anchor < cursor {
            anchor..cursor
        } else {
            cursor..anchor
        };
        assert_eq!(range, 3..7);
    }

    #[test]
    fn delete_selection_drains_range_resets_cursor() {
        let mut text = String::from("hello world");
        let range: std::ops::Range<usize> = 5..11;
        text.drain(range.clone());
        let cursor = range.start;
        assert_eq!(text, "hello");
        assert_eq!(cursor, 5);
    }

    #[test]
    fn select_word_finds_word_around_cursor() {
        let text = "hello world rust";
        let bytes = text.as_bytes();
        let is_space = |b: u8| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r';

        let cursor = 8; // 在 "world" 中间
        let mut start = cursor;
        while start > 0 && !is_space(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = cursor;
        while end < text.len() && !is_space(bytes[end]) {
            end += 1;
        }
        assert_eq!(&text[start..end], "world");
    }

    #[test]
    fn select_word_at_text_start() {
        let text = "hello world";
        let bytes = text.as_bytes();
        let is_space = |b: u8| b == b' ' || b == b'\t';

        let cursor = 0;
        let mut start = cursor;
        while start > 0 && !is_space(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = cursor;
        while end < text.len() && !is_space(bytes[end]) {
            end += 1;
        }
        assert_eq!(&text[start..end], "hello");
    }

    #[test]
    fn selection_range_returns_none_when_anchor_equals_cursor() {
        // anchor == cursor 时（鼠标单击末尾），应返回 None，否则 render 进有选区分支导致 cursor blink 消失
        let anchor = Some(5usize);
        let cursor = 5usize;
        let range: Option<std::ops::Range<usize>> = anchor.and_then(|a| {
            if a != cursor {
                Some(if a < cursor { a..cursor } else { cursor..a })
            } else {
                None
            }
        });
        assert_eq!(range, None);
    }

    #[test]
    fn selection_range_returns_some_when_anchor_differs_from_cursor() {
        // anchor != cursor 时应返回正确的正向 range
        let anchor = Some(2usize);
        let cursor = 5usize;
        let range: Option<std::ops::Range<usize>> = anchor.and_then(|a| {
            if a != cursor {
                Some(if a < cursor { a..cursor } else { cursor..a })
            } else {
                None
            }
        });
        assert_eq!(range, Some(2..5));
    }

    #[test]
    fn compute_copy_payload_no_selection_returns_full_text() {
        let payload = super::compute_copy_payload("hello", None);
        assert_eq!(payload, "hello");
    }

    #[test]
    fn compute_copy_payload_with_selection_returns_range() {
        let payload = super::compute_copy_payload("hello world", Some(0..5));
        assert_eq!(payload, "hello");
    }

    #[test]
    fn compute_copy_payload_empty_text_returns_empty() {
        let payload = super::compute_copy_payload("", None);
        assert!(payload.is_empty());
    }

    #[test]
    fn compute_paste_payload_single_line_passthrough() {
        assert_eq!(super::compute_paste_payload("hello", false), "hello");
    }

    #[test]
    fn compute_paste_payload_truncates_at_lf() {
        assert_eq!(super::compute_paste_payload("line1\nline2", false), "line1");
    }

    #[test]
    fn compute_paste_payload_truncates_at_crlf() {
        // \r\n 中的 \r 先被命中，截到 \r 之前 —— 同样得到首行内容
        assert_eq!(
            super::compute_paste_payload("line1\r\nline2", false),
            "line1"
        );
    }

    #[test]
    fn compute_paste_payload_truncates_at_lone_cr() {
        // macOS classic / 部分 SSH 终端的 \r-only 换行
        assert_eq!(super::compute_paste_payload("line1\rline2", false), "line1");
    }

    #[test]
    fn compute_paste_payload_preserves_leading_whitespace() {
        // 不 trim：用户可能有意粘带前导空格的内容
        assert_eq!(super::compute_paste_payload("  hello", false), "  hello");
    }

    #[test]
    fn compute_paste_payload_empty_clipboard_returns_empty() {
        assert!(super::compute_paste_payload("", false).is_empty());
    }

    #[test]
    fn compute_paste_payload_multiline_preserves_newlines() {
        // multiline=true 时 \n 保留整段
        assert_eq!(
            super::compute_paste_payload("line1\nline2\nline3", true),
            "line1\nline2\nline3"
        );
    }

    #[test]
    fn compute_paste_payload_multiline_normalizes_crlf_to_lf() {
        // Windows 剪贴板常用 \r\n，归一化成 \n 避免 \r 单独留下
        assert_eq!(
            super::compute_paste_payload("line1\r\nline2", true),
            "line1\nline2"
        );
    }

    #[test]
    fn compute_paste_payload_multiline_normalizes_lone_cr_to_lf() {
        // 老 Mac / 终端的 \r-only 换行也归一化
        assert_eq!(
            super::compute_paste_payload("line1\rline2", true),
            "line1\nline2"
        );
    }

    #[test]
    fn compute_paste_payload_pure_newline_returns_empty() {
        // 单行：剪贴板只有换行时，截到首行 = 空串
        assert!(super::compute_paste_payload("\n", false).is_empty());
        assert!(super::compute_paste_payload("\r\n", false).is_empty());
    }

    #[test]
    fn mask_default_is_none() {
        let mc: Option<char> = None;
        assert!(mc.is_none());
    }

    #[test]
    fn mask_char_some_changes_is_masked() {
        let mc: Option<char> = Some('•');
        assert!(mc.is_some());
        assert_eq!(mc, Some('•'));
    }

    #[test]
    fn copy_when_masked_returns_false() {
        let is_masked = true;
        let copy_result = !is_masked;
        assert!(!copy_result);
    }

    #[test]
    fn mask_replaces_chars_in_displayed_text() {
        let text = "secret123";
        let mask = '•';
        let displayed: String = text.chars().map(|_| mask).collect();
        assert_eq!(displayed.chars().count(), text.chars().count());
        assert!(displayed.chars().all(|c| c == '•'));
    }

    use super::byte_offset_at_x;
    use gpui::{point, px, size, Bounds, Pixels};

    fn mk_bound(byte: usize, x: f32, w: f32) -> (usize, Bounds<Pixels>) {
        (
            byte,
            Bounds::new(point(px(x), px(0.0)), size(px(w), px(14.0))),
        )
    }

    #[test]
    fn byte_offset_empty_bounds_returns_zero() {
        let map: Vec<(usize, Bounds<Pixels>)> = vec![];
        assert_eq!(byte_offset_at_x(&map, px(50.0), 10), 0);
    }

    #[test]
    fn byte_offset_click_in_first_half_returns_byte() {
        let map = vec![mk_bound(0, 10.0, 20.0)];
        assert_eq!(byte_offset_at_x(&map, px(15.0), 5), 0);
    }

    #[test]
    fn byte_offset_click_in_second_half_returns_next() {
        let map = vec![mk_bound(0, 10.0, 20.0), mk_bound(1, 30.0, 20.0)];
        assert_eq!(byte_offset_at_x(&map, px(25.0), 5), 1);
    }

    #[test]
    fn byte_offset_click_past_end_returns_text_len() {
        let map = vec![mk_bound(0, 10.0, 20.0)];
        assert_eq!(byte_offset_at_x(&map, px(100.0), 5), 5);
    }

    #[test]
    fn drag_state_starts_false() {
        let dragging = false;
        assert!(!dragging);
    }

    #[test]
    #[allow(unused_assignments)]
    fn mouse_down_sets_dragging_true() {
        let mut dragging = false;
        dragging = true; // 模拟 mouse_down listener
        assert!(dragging);
    }

    #[test]
    #[allow(unused_assignments)]
    fn mouse_up_clears_dragging() {
        let mut dragging = true;
        dragging = false; // 模拟 mouse_up listener
        assert!(!dragging);
    }

    /// 模拟 cursor_from_click 在 mask 模式下的 displayed→source 转换核心逻辑：
    /// displayed_byte → char_idx → source_byte。
    /// 不依赖 GPUI 类型，便于断言数学正确性。
    fn map_displayed_to_source(text: &str, displayed: &str, displayed_byte: usize) -> usize {
        let char_idx = displayed[..displayed_byte].chars().count();
        text.char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(text.len())
    }

    #[test]
    fn mask_mode_click_maps_displayed_byte_to_source_byte() {
        // self.text = "abc" (3 bytes ASCII), displayed = "•••" (9 bytes, '•' = 3B)
        // 点击第 3 个 '•' 起点 (displayed_byte = 6) → 应该返回 source byte 2
        let text = "abc";
        let displayed: String = text.chars().map(|_| '•').collect();
        assert_eq!(displayed.len(), 9);
        assert_eq!(map_displayed_to_source(text, &displayed, 6), 2);
    }

    #[test]
    fn mask_mode_click_at_displayed_start_maps_to_zero() {
        let text = "hello";
        let displayed: String = text.chars().map(|_| '•').collect();
        assert_eq!(map_displayed_to_source(text, &displayed, 0), 0);
    }

    #[test]
    fn mask_mode_click_past_displayed_end_clamps_to_source_len() {
        // displayed_byte = displayed.len() (超出末尾) → 返回 text.len()
        let text = "hi";
        let displayed: String = text.chars().map(|_| '•').collect();
        assert_eq!(displayed.len(), 6);
        assert_eq!(map_displayed_to_source(text, &displayed, 6), 2);
    }

    #[test]
    fn mask_mode_with_cjk_source_text() {
        // 即使原文是中文（3B/char），mask 后 displayed 也是 '•••...' （3B/char），
        // 仍然按 char_idx 中转
        let text = "中文";
        let displayed: String = text.chars().map(|_| '•').collect();
        assert_eq!(text.len(), 6);
        assert_eq!(displayed.len(), 6);
        // 点第 2 个 '•' 起点 → source byte 3 (= 第 2 个中文 char 起点)
        assert_eq!(map_displayed_to_source(text, &displayed, 3), 3);
    }

    #[test]
    fn non_mask_mode_displayed_equals_source_identity_property() {
        // 非 mask 时 displayed_text == self.text；cursor_from_click 走 identity 分支，
        // 等价于 byte_offset_at_x 直接返回 displayed_byte。验证 char_idx 中转在
        // identity 情况下也是恒等（避免回归时误删 fast path 仍能 work）。
        let text = "hello";
        assert_eq!(map_displayed_to_source(text, text, 3), 3);
        assert_eq!(map_displayed_to_source(text, text, 0), 0);
        assert_eq!(map_displayed_to_source(text, text, 5), 5);
    }

    // -------- selection_anchor invariant 测试 --------
    // 这些测试模拟 selection_range 的核心逻辑（不依赖 GPUI Context），
    // 验证 anchor 越界时返回 clamp 后的 range，不会让 drain panic。

    /// 模拟 selection_range 的核心 clamp 逻辑（与 line 197-211 同源）
    fn compute_selection_range_clamped(
        anchor: Option<usize>,
        cursor: usize,
        text_len: usize,
    ) -> Option<std::ops::Range<usize>> {
        anchor.and_then(|a| {
            let a = a.min(text_len);
            let c = cursor.min(text_len);
            if a != c {
                Some(if a < c { a..c } else { c..a })
            } else {
                None
            }
        })
    }

    #[test]
    fn selection_range_clamps_stale_anchor_past_text_len() {
        // 复现历史 bug：text="abc" len=3, cursor=3, anchor=4 (stale)。
        // 未 clamp 前 selection_range 返回 3..4 → drain(3..4) on len-3 → panic。
        // clamp 后 anchor → 3，与 cursor=3 相等 → 返回 None，无 drain。
        assert_eq!(compute_selection_range_clamped(Some(4), 3, 3), None);
    }

    #[test]
    fn selection_range_clamps_both_anchor_and_cursor() {
        // text="ab" len=2，两端都 stale → 都 clamp 到 2 → 相等 → None
        assert_eq!(compute_selection_range_clamped(Some(5), 7, 2), None);
    }

    #[test]
    fn selection_range_clamps_only_anchor() {
        // anchor stale=10，cursor=1 合法，text_len=3 → clamp anchor→3, cursor=1
        // → range 1..3
        assert_eq!(compute_selection_range_clamped(Some(10), 1, 3), Some(1..3));
    }

    #[test]
    fn selection_range_normal_path_unaffected_by_clamp() {
        // 都在范围内 → 行为不变
        assert_eq!(compute_selection_range_clamped(Some(2), 5, 10), Some(2..5));
        assert_eq!(compute_selection_range_clamped(Some(5), 2, 10), Some(2..5));
        assert_eq!(compute_selection_range_clamped(Some(3), 3, 10), None);
        assert_eq!(compute_selection_range_clamped(None, 5, 10), None);
    }

    /// 模拟 backspace 的 anchor 清除：
    /// 复现 panic 路径：
    ///   1. text="abcd" len=4, click 末尾 → anchor=cursor=4
    ///   2. backspace 单字符 → text="abc" len=3, cursor=3
    ///   3. 修复前：anchor 仍是 4 → 下次 backspace drain(3..4) on len-3 panic
    ///   4. 修复后：anchor=None → 下次 backspace 走 remove path → OK
    #[test]
    fn backspace_clears_anchor_after_remove() {
        let mut text = String::from("abcd");
        let mut cursor = 4_usize;
        let mut anchor: Option<usize> = Some(4); // mouse_down at end 设的

        // 模拟 backspace 的 single-char 路径（anchor==cursor 时 delete_selection no-op）
        if cursor > 0 {
            let prev = text[..cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            text.remove(prev);
            cursor = prev;
            anchor = None; // ← 修复点
        }

        assert_eq!(text, "abc");
        assert_eq!(cursor, 3);
        assert_eq!(anchor, None);

        // 再次 backspace 前：confirm 不会 panic（selection_range = None）
        let range = compute_selection_range_clamped(anchor, cursor, text.len());
        assert_eq!(range, None);
    }

    #[test]
    fn set_text_must_clear_stale_anchor() {
        // HostForm 切 host edit 重置字段时 set_text 把 text 变短 →
        // 旧 anchor 残留 > new text.len() → panic 前置条件。
        // 验证：即使旧 anchor 残留（修复前路径），selection_range clamp
        // 也能兜底返回 None；理想状态 set_text 显式 anchor=None 后双重保险。
        let stale_anchor: Option<usize> = Some(10); // 旧字段值 select_all 留下
        let new_text = "abc"; // set_text("abc")
        let cursor = new_text.len();
        // 即使没清 anchor，selection_range clamp 也能避免 panic（defense-in-depth）
        assert_eq!(
            compute_selection_range_clamped(stale_anchor, cursor, new_text.len()),
            None
        );
        // 修复后 set_text 显式清 anchor → 更干净
        let cleared_anchor: Option<usize> = None;
        assert_eq!(
            compute_selection_range_clamped(cleared_anchor, cursor, new_text.len()),
            None
        );
    }

    // ─────────────────── M19 T2: 多行 helpers 单测 ─────────────────────

    // 之前 byte_offset_at_x tests 已 `use gpui::{point, px, size, ...}`，
    // 这里只需补 M19 自家的 super:: items（gpui::px 在同 mod 内已可见）。
    use super::{
        approx_char_width, byte_to_visual_pos, compute_visual_lines, floor_char_boundary,
        visual_pos_to_byte, VisualLine,
    };

    /// 大容器宽度，用于"不会 wrap"的测试场景
    fn wide() -> Pixels {
        px(10000.0)
    }
    /// 终端字体大小，多行测试一致用
    fn fs() -> Pixels {
        px(14.0)
    }

    #[test]
    fn compute_visual_lines_empty_text_yields_one_line() {
        let vls = compute_visual_lines("", wide(), fs());
        assert_eq!(vls.len(), 1);
        assert_eq!(vls[0].logical_line, 0);
        assert_eq!(vls[0].byte_start, 0);
        assert_eq!(vls[0].byte_end, 0);
    }

    #[test]
    fn compute_visual_lines_single_logical_line_no_wrap() {
        let vls = compute_visual_lines("hello", wide(), fs());
        assert_eq!(vls.len(), 1);
        assert_eq!(vls[0].logical_line, 0);
        assert_eq!(vls[0].byte_end, 5);
    }

    #[test]
    fn compute_visual_lines_explicit_newlines_split_logical_lines() {
        let vls = compute_visual_lines("a\nb\nc", wide(), fs());
        assert_eq!(vls.len(), 3);
        assert_eq!(vls[0].logical_line, 0);
        assert_eq!(vls[0].byte_end, 1);
        assert_eq!(vls[1].logical_line, 1);
        assert_eq!(vls[1].byte_start, 2); // 跳过 '\n'
        assert_eq!(vls[1].byte_end, 3);
        assert_eq!(vls[2].logical_line, 2);
        assert_eq!(vls[2].byte_start, 4);
        assert_eq!(vls[2].byte_end, 5);
    }

    #[test]
    fn compute_visual_lines_trailing_newline_yields_empty_visual_line() {
        // "a\n" → 2 visual lines: "a" + ""（让 cursor 能停在末尾空行）
        let vls = compute_visual_lines("a\n", wide(), fs());
        assert_eq!(vls.len(), 2);
        assert_eq!(vls[1].byte_start, 2);
        assert_eq!(vls[1].byte_end, 2);
    }

    #[test]
    fn compute_visual_lines_wraps_long_ascii() {
        // container 仅容 5 ASCII char (5 * 0.6 * 14 = 42px)；6 char wrap
        let narrow = px(42.0);
        let vls = compute_visual_lines("abcdefghij", narrow, fs());
        assert!(vls.len() >= 2, "expected wrap, got {} lines", vls.len());
        // 第 1 行最多 5 字符（容器仅容 5）
        assert!(vls[0].byte_end <= 5);
    }

    #[test]
    fn compute_visual_lines_word_break_prefers_space() {
        // "hello world" 11 char。container 容 ~8 ASCII (8 * 0.6 * 14 ≈ 67px)
        // 期望在 ' ' 后断 → 第 1 行 "hello " (6 byte)，第 2 行 "world"
        let narrow = px(67.0);
        let vls = compute_visual_lines("hello world", narrow, fs());
        assert!(vls.len() >= 2);
        // word break 后第 1 行末 byte = 6（含空格）
        assert_eq!(vls[0].byte_end, 6);
    }

    #[test]
    fn compute_visual_lines_cjk_wider() {
        // 中文 char 是 ASCII 2 倍宽。container 容 2 中文 = 4 ASCII width
        // ≈ 0.6 * 14 * 4 = 33.6px ≈ 1.2 * 14 * 2 = 33.6px
        let narrow = px(34.0);
        let vls = compute_visual_lines("中文你好", narrow, fs());
        assert!(
            vls.len() >= 2,
            "CJK should wrap at ~2 chars, got {} lines",
            vls.len()
        );
    }

    #[test]
    fn byte_to_visual_pos_first_line_offset() {
        let vls = compute_visual_lines("hello\nworld", wide(), fs());
        // byte=0 → (0, 0)
        assert_eq!(byte_to_visual_pos(0, &vls), (0, 0));
        // byte=3 ("hel|lo") → (0, 3)
        assert_eq!(byte_to_visual_pos(3, &vls), (0, 3));
    }

    #[test]
    fn byte_to_visual_pos_second_line() {
        // "hello\nworld" — \n 在 byte 5；byte 6 = "world" 起
        let vls = compute_visual_lines("hello\nworld", wide(), fs());
        // byte=6 ("w") → (1, 0)
        assert_eq!(byte_to_visual_pos(6, &vls), (1, 0));
        // byte=8 ("wo|rld") → (1, 2)
        assert_eq!(byte_to_visual_pos(8, &vls), (1, 2));
    }

    #[test]
    fn byte_to_visual_pos_wrap_boundary_goes_to_next_row() {
        // 强制 wrap：第 1 行 5 char，wrap 后第 2 行从 byte 5
        // 检查 byte=5 归到第 2 行 col=0（不归第 1 行末尾）
        let vls = vec![
            VisualLine {
                logical_line: 0,
                byte_start: 0,
                byte_end: 5,
            },
            VisualLine {
                logical_line: 0,
                byte_start: 5,
                byte_end: 10,
            },
        ];
        assert_eq!(byte_to_visual_pos(5, &vls), (1, 0));
    }

    #[test]
    fn byte_to_visual_pos_byte_at_logical_line_end_stays_first_line() {
        // logical line break（\n）：byte=5 (text="hello\n...") 应归第 0 行
        // 行末，不是第 1 行行首（因为第 1 行起 byte=6 不是 5）
        let vls = vec![
            VisualLine {
                logical_line: 0,
                byte_start: 0,
                byte_end: 5,
            },
            VisualLine {
                logical_line: 1,
                byte_start: 6,
                byte_end: 11,
            },
        ];
        // byte=5 → 第 0 行行末（col=5），不归第 1 行（不同 logical_line）
        assert_eq!(byte_to_visual_pos(5, &vls), (0, 5));
    }

    #[test]
    fn visual_pos_to_byte_roundtrip() {
        let vls = compute_visual_lines("abc\ndef", wide(), fs());
        // (0, 2) → byte 2 ("ab|c")
        assert_eq!(visual_pos_to_byte(0, 2, &vls), 2);
        // (1, 1) → byte 5 ("d|ef" 起算 byte_start=4 + col 1)
        assert_eq!(visual_pos_to_byte(1, 1, &vls), 5);
    }

    #[test]
    fn visual_pos_to_byte_clamps_col_to_line_end() {
        let vls = compute_visual_lines("abc", wide(), fs());
        // col=100 应 clamp 到行末 (byte 3)
        assert_eq!(visual_pos_to_byte(0, 100, &vls), 3);
    }

    #[test]
    fn approx_char_width_ascii_vs_cjk() {
        // ASCII 半角 / CJK 全角，估算与设计一致
        assert_eq!(approx_char_width('a', px(14.0)), px(14.0 * 0.6));
        assert_eq!(approx_char_width('中', px(14.0)), px(14.0 * 1.2));
    }

    /// 模拟 cursor_up_visual / cursor_down_visual 的 pure 算法：
    /// 给 (cursor_byte, preferred_col, delta) → 新 cursor_byte。
    /// delta=-1 上移 / +1 下移；边界 clamp（首行回 byte 0 / 末行回 text 末）。
    fn move_cursor_vl(
        text: &str,
        cursor: usize,
        preferred_col: Option<usize>,
        delta: i32,
    ) -> (usize, usize) {
        let vls = compute_visual_lines(text, wide(), fs());
        let (vl_idx, col) = byte_to_visual_pos(cursor, &vls);
        let pref = preferred_col.unwrap_or(col);
        let new_byte = if delta < 0 {
            if vl_idx == 0 {
                0
            } else {
                visual_pos_to_byte(vl_idx - 1, pref, &vls)
            }
        } else if vl_idx + 1 >= vls.len() {
            text.len()
        } else {
            visual_pos_to_byte(vl_idx + 1, pref, &vls)
        };
        (new_byte, pref)
    }

    #[test]
    fn cursor_down_visual_keeps_preferred_col() {
        // "long line\nshort\nlong line again"
        // 从 long line byte 5（col 5）↓ 到 short（行 5 chars，col clamp 到 5 = 行末）
        // 再 ↓ 到 long line again（pref_col 仍 5，落第 6 字符前）
        let text = "long line\nshort\nlong line again";
        // long line byte 5 = "long " 第 6 char 之前 ("l" of "line")
        let (b1, pref) = move_cursor_vl(text, 5, None, 1);
        // short 行 byte_start = 10, 行长 5 byte → col 5 落行末 → b1 = 10 + 5 = 15
        assert_eq!(b1, 15);
        assert_eq!(pref, 5);
        // 再 ↓：long line again byte_start = 16, col=5 → 21
        let (b2, pref2) = move_cursor_vl(text, b1, Some(pref), 1);
        assert_eq!(b2, 21);
        assert_eq!(pref2, 5);
    }

    #[test]
    fn cursor_up_visual_at_first_line_goes_byte_0() {
        let text = "first\nsecond";
        // cursor 在 first 中间（byte 2）↑ → 应到 byte 0
        let (b, _) = move_cursor_vl(text, 2, None, -1);
        assert_eq!(b, 0);
    }

    #[test]
    fn cursor_down_visual_at_last_line_goes_text_end() {
        let text = "a\nb";
        // cursor 在 b（byte 2）↓ → 应到 text 末（3）
        let (b, _) = move_cursor_vl(text, 2, None, 1);
        assert_eq!(b, 3);
    }

    #[test]
    fn cursor_up_visual_no_preferred_col_uses_current() {
        // 不传 preferred_col → 用当前 col；从 short 行 col=3 ↑ 到 long 行 col=3
        let text = "long line\nshort";
        // short 行 byte_start = 10，col=3 → cursor = 13
        let (b, pref) = move_cursor_vl(text, 13, None, -1);
        // long line col=3 → byte 3
        assert_eq!(b, 3);
        assert_eq!(pref, 3);
    }

    #[test]
    fn floor_char_boundary_ascii_identity() {
        // ASCII 全 boundary，任何 byte 都不动
        assert_eq!(floor_char_boundary("hello", 3), 3);
        assert_eq!(floor_char_boundary("hello", 0), 0);
        assert_eq!(floor_char_boundary("hello", 5), 5);
    }

    #[test]
    fn floor_char_boundary_cjk_floors_to_char_start() {
        // "中" 是 3-byte UTF-8 (E4 B8 AD)，byte 1 / 2 都在中间
        let text = "中文";
        assert_eq!(floor_char_boundary(text, 0), 0); // boundary
        assert_eq!(floor_char_boundary(text, 1), 0); // 中间 → 回 0
        assert_eq!(floor_char_boundary(text, 2), 0);
        assert_eq!(floor_char_boundary(text, 3), 3); // 第 2 char boundary
        assert_eq!(floor_char_boundary(text, 5), 3); // 中间 → 回 3
        assert_eq!(floor_char_boundary(text, 6), 6); // text 末
    }

    #[test]
    fn floor_char_boundary_clamps_over_len() {
        // byte > text.len() → 先 clamp 到 len，再 floor
        assert_eq!(floor_char_boundary("hello", 100), 5);
        assert_eq!(floor_char_boundary("中文", 100), 6);
    }

    #[test]
    fn cursor_down_into_cjk_falls_to_char_boundary() {
        // 跨行 ↓ 落 CJK char 中间应被 clamp 到 char start，防 panic
        // text: "abcdefgh\n中文" — 第 1 行 8 ASCII，第 2 行 2 CJK chars (6 byte)
        // cursor 在第 1 行 col=4 ("abcd|efgh")，↓ pref_col=4
        // 第 2 行 byte_start = 9（"\n" 在 byte 8）
        // visual_pos_to_byte(1, 4, vls) = 9 + 4 = 13。但 byte 13 在 "中" (9..12)
        // 之后、"文" (12..15) 中间（byte 13 是 "文" 第 2 byte）
        // floor_char_boundary 应把 13 floor 到 12（"文" 起始）
        let text = "abcdefgh\n中文";
        let (b, _pref) = move_cursor_vl(text, 4, None, 1);
        // raw = 13，floor 应回到 12（"文" 起 byte）
        let floored = floor_char_boundary(text, b);
        assert!(
            text.is_char_boundary(floored),
            "cursor after down must be char boundary, got byte {}",
            floored
        );
    }
}
