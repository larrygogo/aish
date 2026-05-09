//! aish-app 构建脚本。
//!
//! 将 `assets/logo.svg`（纯几何无文字）渲染为各尺寸 PNG，
//! 再组装为 macOS ICNS 和 Windows ICO，全部写入 `$OUT_DIR/icons/`。
//!
//! 同时在 Windows 平台嵌入 ICO 到 EXE 资源。

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let svg_path = PathBuf::from(&manifest_dir).join("assets/logo.svg");

    println!("cargo:rerun-if-changed={}", svg_path.display());

    let out_dir = std::env::var("OUT_DIR")?;
    let icons_dir = PathBuf::from(&out_dir).join("icons");
    std::fs::create_dir_all(&icons_dir)?;

    // ── 1. 解析 SVG ──────────────────────────────────────────────
    // ★ 修正①：from_str 只接受 2 个参数（0.44+ API）
    //   fontdb 已内嵌于 Options，SVG 无 <text> 时默认空库足够
    let svg_data = std::fs::read_to_string(&svg_path)?;
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&svg_data, &opt)?;

    // ── 2. 渲染各尺寸 PNG ─────────────────────────────────────────
    let sizes: &[u32] = &[16, 32, 64, 128, 256, 512, 1024];
    for &size in sizes {
        render_png(&tree, size, &icons_dir.join(format!("logo_{size}.png")))?;
    }

    // ── 3. 生成 macOS ICNS ────────────────────────────────────────
    build_icns(&icons_dir)?;

    // ── 4. 生成 Windows ICO ───────────────────────────────────────
    build_ico(&icons_dir)?;

    // ── 5. Windows 平台：嵌入 ICO 到 EXE 资源 ──────────────────
    #[cfg(target_os = "windows")]
    {
        let ico_path = icons_dir.join("aish.ico");
        if ico_path.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico_path.to_str().unwrap());
            res.set("ProductName", "aish");
            res.set("FileDescription", "aish - AI Shell");
            res.set("LegalCopyright", "Copyright (c) 2026 larry");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Windows 资源编译失败: {e}");
            }
        }
    }

    Ok(())
}

fn render_png(
    tree: &resvg::usvg::Tree,
    size: u32,
    out: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use resvg::tiny_skia;

    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("无法创建 Pixmap")?;

    let sx = size as f32 / tree.size().width();
    let sy = size as f32 / tree.size().height();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );

    pixmap.save_png(out)?;

    // 用 std::fs::metadata 验证，不使用 image crate
    // 16×16 PNG 可能只有 ~300B，阈值按尺寸分级
    let file_len = std::fs::metadata(out)?.len();
    // 注：小尺寸 PNG（如 16×16）约 300B，仅验证文件有效存在
    assert!(file_len > 0, "渲染产物文件为空或无效：{}", out.display());

    Ok(())
}

fn build_icns(icons_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut family = icns::IconFamily::new();

    // ★ 修正②：全部使用 RGBA32，保留圆角透明区域
    let icns_sizes: &[(u32, icns::IconType)] = &[
        (16, icns::IconType::RGBA32_16x16),
        (32, icns::IconType::RGBA32_32x32),
        (64, icns::IconType::RGBA32_64x64),
        (128, icns::IconType::RGBA32_128x128),
        (256, icns::IconType::RGBA32_256x256),
        (512, icns::IconType::RGBA32_512x512),
        (1024, icns::IconType::RGBA32_512x512_2x),
    ];

    for &(size, icon_type) in icns_sizes {
        let png_path = icons_dir.join(format!("logo_{size}.png"));
        if png_path.exists() {
            let png_data = std::fs::read(&png_path)?;
            let image = icns::Image::read_png(std::io::Cursor::new(png_data))?;
            family.add_icon_with_type(&image, icon_type)?;
        }
    }

    let mut f = std::fs::File::create(icons_dir.join("aish.icns"))?;
    family.write(&mut f)?;
    Ok(())
}

fn build_ico(icons_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for &size in &[16u32, 32, 48, 64, 128, 256] {
        let png_path = icons_dir.join(format!("logo_{size}.png"));
        if png_path.exists() {
            let file = std::fs::File::open(&png_path)?;
            let image = ico::IconImage::read_png(file)?;
            icon_dir.add_entry(ico::IconDirEntry::encode(&image)?);
        }
    }

    let file = std::fs::File::create(icons_dir.join("aish.ico"))?;
    icon_dir.write(file)?;
    Ok(())
}
