//! 运行时可访问的应用图标（128px PNG）。
//! 文件由 build.rs 写入 $OUT_DIR，include_bytes! 在编译期嵌入。

/// 128×128 PNG 字节，供 About 页面等场景使用。
#[allow(dead_code)]
pub const LOGO_128: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icons/logo_128.png"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_128_is_valid_png() {
        assert!(
            LOGO_128.starts_with(b"\x89PNG\r\n\x1a\n"),
            "logo_128.png 不是有效 PNG"
        );
    }

    #[test]
    fn logo_128_size_reasonable() {
        assert!(
            LOGO_128.len() > 1024,
            "logo_128.png 过小（{}B），可能渲染异常",
            LOGO_128.len()
        );
    }
}
