//! Host avatar 调色板：按 string hash 选色，保证同一 host label 总是同色。

const AVATAR_PALETTE: &[u32] = &[
    0x6366f1, // indigo
    0x8b5cf6, // violet
    0xec4899, // pink
    0xef4444, // red
    0xf59e0b, // amber
    0x10b981, // emerald
    0x06b6d4, // cyan
    0x3b82f6, // blue
];

pub fn avatar_color_for(label: &str) -> u32 {
    let mut sum: u32 = 0;
    for b in label.bytes() {
        sum = sum.wrapping_add(b as u32);
    }
    AVATAR_PALETTE[(sum as usize) % AVATAR_PALETTE.len()]
}

/// 把 `/etc/os-release` 的 ID 字段（小写）映射到 Nerd Font glyph + 推荐色。
///
/// 返回 `(glyph, bg_color)` —— bg_color 是 logo 背景方块的色（按发行版品牌色，
/// 与首字母 avatar 的 palette 解耦）。未知 ID 返回 None，调用方 fallback 首字母。
///
/// Nerd Font v3 nf-linux-* 系列 codepoint：
///   ubuntu  U+F31B  debian  U+F306  arch    U+F303  alpine  U+F300
///   centos  U+F304  fedora  U+F30A  redhat  U+F316  rocky   U+F32B
///   mint    U+F30E  manjaro U+F312  nixos   U+F313  gentoo  U+F30D
///   opensuse U+F314 raspbian U+F315  elementary U+F309
///   tux fallback U+F17C (Linux 企鹅，未明确发行版时用)
pub fn os_logo_for(os_kind: &str) -> Option<(&'static str, u32)> {
    let kind = os_kind.trim().to_ascii_lowercase();
    match kind.as_str() {
        "ubuntu" => Some(("\u{f31b}", 0xe95420)),
        "debian" => Some(("\u{f306}", 0xa81d33)),
        "arch" => Some(("\u{f303}", 0x1793d1)),
        "alpine" => Some(("\u{f300}", 0x0d597f)),
        "centos" => Some(("\u{f304}", 0x932279)),
        "fedora" => Some(("\u{f30a}", 0x294172)),
        "rhel" | "redhat" => Some(("\u{f316}", 0xee0000)),
        "rocky" => Some(("\u{f32b}", 0x10b981)),
        "linuxmint" | "mint" => Some(("\u{f30e}", 0x86be3c)),
        "manjaro" => Some(("\u{f312}", 0x35bf5c)),
        "nixos" => Some(("\u{f313}", 0x5277c3)),
        "gentoo" => Some(("\u{f30d}", 0x54487a)),
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "suse" | "sles" => {
            Some(("\u{f314}", 0x73ba25))
        }
        "raspbian" => Some(("\u{f315}", 0xa22846)),
        "elementary" => Some(("\u{f309}", 0x64baff)),
        // 兜底：任何未列出的 Linux 发行版（如 amzn / ol / clear-linux 等）
        // 用通用 Tux 企鹅图标 + 中性灰
        _ => Some(("\u{f17c}", 0x6b7280)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_logo_for_ubuntu_returns_ubuntu_glyph() {
        let (glyph, _bg) = os_logo_for("ubuntu").unwrap();
        assert_eq!(glyph, "\u{f31b}");
    }

    #[test]
    fn os_logo_for_case_insensitive() {
        assert!(os_logo_for("UBUNTU").is_some());
        assert!(os_logo_for(" Debian ").is_some());
    }

    #[test]
    fn os_logo_for_unknown_id_falls_back_to_tux() {
        let (glyph, _) = os_logo_for("some-future-distro").unwrap();
        assert_eq!(glyph, "\u{f17c}");
    }

    #[test]
    fn os_logo_for_aliases_match() {
        assert_eq!(os_logo_for("redhat"), os_logo_for("rhel"));
        assert_eq!(os_logo_for("mint"), os_logo_for("linuxmint"));
        assert_eq!(os_logo_for("opensuse"), os_logo_for("suse"));
    }
}
