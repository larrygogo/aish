//! Keybinding 数据模型 + keystroke 字符串编解码 + 严格匹配（Phase A + B）。
//!
//! Phase A：Settings UI 捕获 + 持久化到 app_state.toml + 显示自定义键。
//! Phase B：RootView::handle_global_key 与 terminal_view 的 key handler
//! 都走 `current_for(action_id, &bindings)` + `matches(keystroke, &expected)`
//! 实现真路由生效（保存后立即生效）。
//!
//! keystroke 字符串格式：`{modifier}-{modifier}-...{key}`，modifier 顺序固定
//! `ctrl, alt, shift, cmd`（GPUI Keystroke 同序），key 是 GPUI 给的 raw key
//! 名（如 "c", "1", "enter", ","）。
//!   - "ctrl-shift-c", "cmd-k", "ctrl-1", "cmd-,"

use gpui::Keystroke;

/// 一个可绑定的 action。id 跟 Settings → Shortcuts 的 shortcut_row id 对应
/// （去 `sc-` 前缀），label 是给用户看的中文说明。
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    /// 仅 macOS 显示（如 Cmd+, 打开 Settings 是 Mac 通用约定，其他平台无意义）。
    pub mac_only: bool,
}

/// 应用支持的 action 列表。顺序 = Settings 页展示顺序。
pub const ACTIONS: &[Action] = &[
    Action {
        id: "palette",
        label: "打开命令面板",
        mac_only: false,
    },
    Action {
        id: "copy",
        label: "复制选中文本",
        mac_only: false,
    },
    Action {
        id: "paste",
        label: "粘贴",
        mac_only: false,
    },
    Action {
        id: "new-tab",
        label: "新建标签页",
        mac_only: false,
    },
    Action {
        id: "close-tab",
        label: "关闭标签页",
        mac_only: false,
    },
    Action {
        id: "home",
        label: "切到主页",
        mac_only: false,
    },
    Action {
        id: "terminal",
        label: "切到终端",
        mac_only: false,
    },
    Action {
        id: "settings",
        label: "切到设置",
        mac_only: false,
    },
    Action {
        id: "mac-settings",
        label: "打开设置（macOS 通用）",
        mac_only: true,
    },
];

/// 返回 action 在当前 OS 下的默认 keystroke 字符串。
pub fn default_for(action_id: &str) -> &'static str {
    let mac = cfg!(target_os = "macos");
    match action_id {
        // palette 实际默认双绑 (Cmd+P / Cmd+K)；Phase A 仅单绑显示 Cmd+K
        // （paseo / Linear / VSCode 通用 ⌘K 风），Cmd+P 仍由 hardcoded 路由
        // 接受，等 Phase B 多绑值时再扩展。
        "palette" => {
            if mac {
                "cmd-k"
            } else {
                "ctrl-p"
            }
        }
        "copy" => {
            if mac {
                "cmd-c"
            } else {
                "ctrl-shift-c"
            }
        }
        "paste" => {
            if mac {
                "cmd-v"
            } else {
                "ctrl-shift-v"
            }
        }
        "new-tab" => {
            if mac {
                "cmd-t"
            } else {
                "ctrl-t"
            }
        }
        "close-tab" => {
            if mac {
                "cmd-w"
            } else {
                "ctrl-w"
            }
        }
        "home" => {
            if mac {
                "cmd-1"
            } else {
                "ctrl-1"
            }
        }
        "terminal" => {
            if mac {
                "cmd-2"
            } else {
                "ctrl-2"
            }
        }
        "settings" => {
            if mac {
                "cmd-3"
            } else {
                "ctrl-3"
            }
        }
        "mac-settings" => "cmd-,",
        _ => "",
    }
}

/// GPUI Keystroke → `"ctrl-shift-c"` 风字符串。
pub fn keystroke_to_string(ks: &Keystroke) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if ks.modifiers.control {
        parts.push("ctrl");
    }
    if ks.modifiers.alt {
        parts.push("alt");
    }
    if ks.modifiers.shift {
        parts.push("shift");
    }
    if ks.modifiers.platform {
        parts.push("cmd");
    }
    parts.push(ks.key.as_str());
    parts.join("-")
}

/// 给用户看的格式化。macOS 用紧排符号 `⌃⌥⇧⌘K`，其他平台用 `Ctrl+Shift+K`。
pub fn format_for_display(keystroke_str: &str) -> String {
    let mac = cfg!(target_os = "macos");
    let parts: Vec<&str> = keystroke_str.split('-').collect();
    if mac {
        let mut out = String::new();
        for p in &parts {
            match *p {
                "ctrl" => out.push('⌃'),
                "alt" => out.push('⌥'),
                "shift" => out.push('⇧'),
                "cmd" => out.push('⌘'),
                k => {
                    if k.len() == 1 {
                        out.push_str(&k.to_uppercase());
                    } else {
                        out.push_str(k);
                    }
                }
            }
        }
        out
    } else {
        parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let s = match *p {
                    "ctrl" => "Ctrl".to_string(),
                    "alt" => "Alt".to_string(),
                    "shift" => "Shift".to_string(),
                    "cmd" => "Win".to_string(),
                    k if k.len() == 1 => k.to_uppercase(),
                    k => {
                        let mut chars = k.chars();
                        match chars.next() {
                            Some(c) => c.to_uppercase().chain(chars).collect(),
                            None => String::new(),
                        }
                    }
                };
                if i == 0 {
                    s
                } else {
                    format!("+{}", s)
                }
            })
            .collect()
    }
}

/// 拿 action 当前生效的 keystroke 字符串 — 优先用户 override，否则 default。
/// Phase B 路由用本函数取「当前应该匹配的键」。
pub fn current_for(action_id: &str, custom: &std::collections::HashMap<String, String>) -> String {
    custom
        .get(action_id)
        .cloned()
        .unwrap_or_else(|| default_for(action_id).to_string())
}

/// 严格匹配 keystroke 与 expected 字符串 — modifier 集合完全相等 + key 字面相等。
/// 不允许 "extra modifiers"（如 `ctrl-p` 不匹配 `ctrl-shift-p`），跟当前
/// hardcoded 路由（`!m.shift && !m.alt` 显式排除）行为一致。
///
/// 空 expected → false（防 default_for 给未知 action 返回空串时误匹配空 keystroke）。
pub fn matches(keystroke: &gpui::Keystroke, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    let parts: Vec<&str> = expected.split('-').collect();
    let Some(key) = parts.last() else {
        return false;
    };
    let mods = &parts[..parts.len() - 1];

    let expect_ctrl = mods.contains(&"ctrl");
    let expect_alt = mods.contains(&"alt");
    let expect_shift = mods.contains(&"shift");
    let expect_cmd = mods.contains(&"cmd");

    keystroke.modifiers.control == expect_ctrl
        && keystroke.modifiers.alt == expect_alt
        && keystroke.modifiers.shift == expect_shift
        && keystroke.modifiers.platform == expect_cmd
        && keystroke.key.as_str().eq_ignore_ascii_case(key)
}

/// 验证 keystroke 是否能作为可绑定快捷键 — 必须有至少一个 modifier + 一个 key。
/// 防止用户绑了「a」这种裸键导致跟普通输入冲突。
pub fn is_valid_binding(keystroke_str: &str) -> bool {
    let parts: Vec<&str> = keystroke_str.split('-').collect();
    if parts.len() < 2 {
        return false;
    }
    parts[..parts.len() - 1]
        .iter()
        .any(|p| matches!(*p, "ctrl" | "alt" | "shift" | "cmd"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn ks(mods: Modifiers, key: &str) -> Keystroke {
        Keystroke {
            modifiers: mods,
            key: key.into(),
            key_char: None,
        }
    }

    #[test]
    fn keystroke_to_string_simple_ctrl_c() {
        let k = ks(
            Modifiers {
                control: true,
                ..Default::default()
            },
            "c",
        );
        assert_eq!(keystroke_to_string(&k), "ctrl-c");
    }

    #[test]
    fn keystroke_to_string_modifier_order_fixed() {
        let k = ks(
            Modifiers {
                control: true,
                shift: true,
                alt: true,
                platform: true,
                ..Default::default()
            },
            "k",
        );
        assert_eq!(keystroke_to_string(&k), "ctrl-alt-shift-cmd-k");
    }

    #[test]
    fn is_valid_binding_requires_modifier() {
        assert!(is_valid_binding("ctrl-c"));
        assert!(is_valid_binding("cmd-shift-p"));
        assert!(!is_valid_binding("a"));
        assert!(!is_valid_binding("enter"));
    }

    #[test]
    fn default_for_known_action_nonempty() {
        for a in ACTIONS {
            assert!(!default_for(a.id).is_empty(), "{} has no default", a.id);
        }
    }

    #[test]
    fn default_for_unknown_returns_empty() {
        assert_eq!(default_for("does-not-exist"), "");
    }

    #[test]
    fn matches_strict_modifier_set() {
        let k_ctrl_p = ks(
            Modifiers {
                control: true,
                ..Default::default()
            },
            "p",
        );
        assert!(matches(&k_ctrl_p, "ctrl-p"));
        assert!(!matches(&k_ctrl_p, "ctrl-shift-p"));
        assert!(!matches(&k_ctrl_p, "cmd-p"));
        assert!(!matches(&k_ctrl_p, "p"));
    }

    #[test]
    fn matches_key_case_insensitive() {
        let k = ks(
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
            "C",
        );
        assert!(matches(&k, "ctrl-shift-c"));
    }

    #[test]
    fn matches_empty_expected_false() {
        let k = ks(
            Modifiers {
                control: true,
                ..Default::default()
            },
            "p",
        );
        assert!(!matches(&k, ""));
    }

    #[test]
    fn current_for_falls_back_to_default() {
        let mut custom = std::collections::HashMap::new();
        assert_eq!(current_for("palette", &custom), default_for("palette"));
        custom.insert("palette".to_string(), "ctrl-shift-x".to_string());
        assert_eq!(current_for("palette", &custom), "ctrl-shift-x");
    }
}
