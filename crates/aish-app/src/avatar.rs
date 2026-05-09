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
