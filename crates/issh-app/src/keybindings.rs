//! Keybinding 数据模型 + keystroke 字符串编解码（Phase A）。
//!
//! Phase A 范围：Settings UI 捕获 + 持久化到 app_state.toml + 显示自定义键。
//! Phase B（未实现）：把 RootView / terminal_view / 各 view 的 hardcoded
//! match 路由改成读 keybindings map。在 Phase A 阶段用户改了 binding 能在
//! Settings 看到新键，但**实际触发仍走默认 hardcoded 路由**，故 capture
//! dialog 内提示「Phase B 实现后立即生效」。
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
}
