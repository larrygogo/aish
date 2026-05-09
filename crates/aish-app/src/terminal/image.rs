use std::io::Cursor;

use image::{ImageFormat, RgbaImage};

pub fn encode_rgba_to_png(w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<Vec<u8>> {
    let img = RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or_else(|| {
        anyhow::anyhow!(
            "RGBA buffer size mismatch: expected {} bytes, got {}",
            w * h * 4,
            rgba.len()
        )
    })?;
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_rgba_to_png_produces_valid_png_header() {
        let rgba = vec![
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let png = encode_rgba_to_png(2, 2, &rgba).expect("encode should succeed");
        assert_eq!(
            &png[0..4],
            b"\x89PNG",
            "output must start with PNG signature"
        );
        assert!(png.len() > 30, "PNG must have at least header + IHDR");
    }

    #[test]
    fn encode_rgba_to_png_size_mismatch_returns_error() {
        let bad = vec![0u8; 10];
        let result = encode_rgba_to_png(4, 4, &bad);
        assert!(result.is_err(), "must fail on buffer size mismatch");
        assert!(
            result.unwrap_err().to_string().contains("mismatch"),
            "error must mention mismatch"
        );
    }

    #[test]
    fn encode_rgba_to_png_1x1_red_pixel() {
        let rgba = vec![255u8, 0, 0, 255];
        let png = encode_rgba_to_png(1, 1, &rgba).unwrap();
        // PNG IHDR chunk: width at bytes 16-19, height at 20-23 (big-endian u32)
        assert_eq!(&png[0..4], b"\x89PNG");
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(width, 1);
        assert_eq!(height, 1);
    }
}
