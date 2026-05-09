// aish-app 构建脚本
//
// 图标生成已迁移到 Bun 脚本 (scripts/gen-icons.js)，
// 此 build.rs 仅负责 Windows 平台嵌入预生成的 ICO 到 EXE 资源。

fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=../../assets/aish.ico");

        let ico_path = std::path::Path::new("../../assets/aish.ico");
        if ico_path.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico_path.to_str().unwrap());
            res.set("ProductName", "aish");
            res.set("FileDescription", "aish - AI Shell");
            res.set("LegalCopyright", "Copyright (c) 2026 larry");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=图标资源编译失败: {e}");
            }
        } else {
            println!("cargo:warning=未找到 assets/aish.ico，跳过图标嵌入");
        }
    }
}
