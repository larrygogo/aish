//! Host avatar：
//! - 调色板版（按 string hash 选色 + 首字母）：未探测到 OS 时 fallback
//! - 发行版品牌色版：探测到 os_kind 后用品牌色背景 +（真 SVG logo 或单字母）

use issh_ui::IconName;

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

/// Host 卡片 avatar 内容模式：
/// - `Svg`：内嵌 simpleicons.org logo（白色 svg on 品牌色背景）
/// - `Letter`：未提供 SVG 的发行版用单字母 + 品牌色（视觉与首字母 fallback
///   一致，区别只是背景色用发行版品牌色而非 hash 调色板）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsAvatar {
    Svg { icon: IconName, bg: u32 },
    Letter { letter: char, bg: u32 },
}

/// 把 `/etc/os-release` 的 ID 字段（小写）映射到 avatar 内容。
/// 返回 None 表示未识别的 distro，调用方继续 fallback 到 host label 首字母
/// + palette 色。
///
/// 已支持 SVG 的发行版（7 个常用）：ubuntu / debian / arch / alpine /
/// centos / fedora / rhel(redhat)
/// 仅有品牌色 + 字母的发行版：rocky / mint / manjaro / nixos / gentoo /
/// opensuse / raspbian / elementary
pub fn os_avatar_for(os_kind: &str) -> Option<OsAvatar> {
    let kind = os_kind.trim().to_ascii_lowercase();
    match kind.as_str() {
        "ubuntu" => Some(OsAvatar::Svg {
            icon: IconName::DistroUbuntu,
            bg: 0xe95420,
        }),
        "debian" => Some(OsAvatar::Svg {
            icon: IconName::DistroDebian,
            bg: 0xa81d33,
        }),
        "arch" => Some(OsAvatar::Svg {
            icon: IconName::DistroArch,
            bg: 0x1793d1,
        }),
        "alpine" => Some(OsAvatar::Svg {
            icon: IconName::DistroAlpine,
            bg: 0x0d597f,
        }),
        "centos" => Some(OsAvatar::Svg {
            icon: IconName::DistroCentos,
            bg: 0x932279,
        }),
        "fedora" => Some(OsAvatar::Svg {
            icon: IconName::DistroFedora,
            bg: 0x294172,
        }),
        "rhel" | "redhat" => Some(OsAvatar::Svg {
            icon: IconName::DistroRedhat,
            bg: 0xee0000,
        }),
        // macOS：苹果 logo + Apple Space Gray (#1d1d1f) 背景
        "macos" | "darwin" => Some(OsAvatar::Svg {
            icon: IconName::DistroMacos,
            bg: 0x1d1d1f,
        }),
        "rocky" => Some(OsAvatar::Letter {
            letter: 'R',
            bg: 0x10b981,
        }),
        "linuxmint" | "mint" => Some(OsAvatar::Letter {
            letter: 'M',
            bg: 0x86be3c,
        }),
        "manjaro" => Some(OsAvatar::Letter {
            letter: 'M',
            bg: 0x35bf5c,
        }),
        "nixos" => Some(OsAvatar::Letter {
            letter: 'N',
            bg: 0x5277c3,
        }),
        "gentoo" => Some(OsAvatar::Letter {
            letter: 'G',
            bg: 0x54487a,
        }),
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "suse" | "sles" => {
            Some(OsAvatar::Letter {
                letter: 'S',
                bg: 0x73ba25,
            })
        }
        "raspbian" => Some(OsAvatar::Letter {
            letter: 'P',
            bg: 0xa22846,
        }),
        "elementary" => Some(OsAvatar::Letter {
            letter: 'E',
            bg: 0x64baff,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_avatar_for_ubuntu_returns_svg() {
        match os_avatar_for("ubuntu") {
            Some(OsAvatar::Svg { icon, .. }) => assert_eq!(icon, IconName::DistroUbuntu),
            other => panic!("expected Svg(Ubuntu), got {:?}", other),
        }
    }

    #[test]
    fn os_avatar_for_nixos_returns_letter_n() {
        match os_avatar_for("nixos") {
            Some(OsAvatar::Letter { letter, .. }) => assert_eq!(letter, 'N'),
            other => panic!("expected Letter('N'), got {:?}", other),
        }
    }

    #[test]
    fn os_avatar_for_case_insensitive() {
        assert!(os_avatar_for("UBUNTU").is_some());
        assert!(os_avatar_for(" Debian ").is_some());
    }

    #[test]
    fn os_avatar_for_unknown_returns_none() {
        assert!(os_avatar_for("some-future-distro").is_none());
    }

    #[test]
    fn os_avatar_aliases_match() {
        assert_eq!(os_avatar_for("redhat"), os_avatar_for("rhel"));
        assert_eq!(os_avatar_for("mint"), os_avatar_for("linuxmint"));
        assert_eq!(os_avatar_for("opensuse"), os_avatar_for("suse"));
    }
}
