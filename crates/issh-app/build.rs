//! issh-app 构建脚本。
//!
//! 将 `assets/icons/issh.svg`（像素风终端 `>_` 图标）渲染为各尺寸 PNG，
//! 再组装为 macOS ICNS 和 Windows ICO，全部写入 `$OUT_DIR/icons/`。
//!
//! 同时在 Windows 平台嵌入 ICO 到 EXE 资源。

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    // 单一 SVG 真相来源：repo 根 assets/icons/issh.svg
    let svg_path = PathBuf::from(&manifest_dir)
        .join("..")
        .join("..")
        .join("assets")
        .join("icons")
        .join("issh.svg");

    println!("cargo:rerun-if-changed={}", svg_path.display());

    let out_dir = std::env::var("OUT_DIR")?;
    let icons_dir = PathBuf::from(&out_dir).join("icons");
    std::fs::create_dir_all(&icons_dir)?;

    // git info 注入 — Settings About 卡片显示。失败 fallback "unknown"
    // （CI / 用户机 build 没 git 仓库时优雅降级）。HEAD 变化时重 build。
    inject_git_env();
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // ── 1. 解析 SVG ──────────────────────────────────────────────
    let svg_data = std::fs::read_to_string(&svg_path)?;
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&svg_data, &opt)?;

    // ── 2. 渲染各尺寸 PNG ─────────────────────────────────────────
    // 包含 48px（Windows ICO 标准尺寸）
    let sizes: &[u32] = &[16, 32, 48, 64, 128, 256, 512, 1024];
    for &size in sizes {
        render_png(&tree, size, &icons_dir.join(format!("issh-{size}.png")))?;
    }

    // ── 3. 生成 macOS ICNS ────────────────────────────────────────
    build_icns(&icons_dir)?;

    // ── 4. 生成 Windows ICO ───────────────────────────────────────
    build_ico(&icons_dir)?;

    // ── 5. Windows 平台：嵌入 ICO 到 EXE 资源 ──────────────────
    #[cfg(target_os = "windows")]
    {
        let ico_path = icons_dir.join("issh.ico");
        if ico_path.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico_path.to_str().unwrap());
            res.set("ProductName", "issh");
            res.set("FileDescription", "issh - AI Shell");
            res.set("LegalCopyright", "Copyright (c) 2026 larry");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Windows 资源编译失败: {e}");
            }
        }
    }

    Ok(())
}

/// 跑 git 命令注入 ISSH_GIT_HASH + ISSH_BUILD_DATE 环境变量。
/// 失败时 set "unknown" 让 env! 不爆 + Settings 显示 "unknown" 不崩。
fn inject_git_env() {
    use std::process::Command;
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    // 最近 commit 日期（%cd %as 都行；--date=short 走 YYYY-MM-DD）
    let date = Command::new("git")
        .args(["log", "-1", "--format=%cd", "--date=short"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=ISSH_GIT_HASH={hash}");
    println!("cargo:rustc-env=ISSH_BUILD_DATE={date}");
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

    let file_len = std::fs::metadata(out)?.len();
    assert!(file_len > 0, "渲染产物文件为空或无效：{}", out.display());

    Ok(())
}

fn build_icns(icons_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut family = icns::IconFamily::new();

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
        let png_path = icons_dir.join(format!("issh-{size}.png"));
        if png_path.exists() {
            let png_data = std::fs::read(&png_path)?;
            let image = icns::Image::read_png(std::io::Cursor::new(png_data))?;
            family.add_icon_with_type(&image, icon_type)?;
        }
    }

    let mut f = std::fs::File::create(icons_dir.join("issh.icns"))?;
    family.write(&mut f)?;
    Ok(())
}

fn build_ico(icons_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for &size in &[16u32, 32, 48, 64, 128, 256] {
        let png_path = icons_dir.join(format!("issh-{size}.png"));
        if png_path.exists() {
            let file = std::fs::File::open(&png_path)?;
            let image = ico::IconImage::read_png(file)?;
            icon_dir.add_entry(ico::IconDirEntry::encode(&image)?);
        }
    }

    let file = std::fs::File::create(icons_dir.join("issh.ico"))?;
    icon_dir.write(file)?;
    Ok(())
}
