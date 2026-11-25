fn main() {
    println!("cargo:rerun-if-changed=assets/app.res");
    // 预编译的 Windows 资源（图标 + 版本信息），由 assets/app.rc 经 windres -O res 生成。
    // 直接交给链接器，避免构建时依赖 rc.exe。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let res = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app.res");
        println!("cargo:rustc-link-arg={}", res.display());
    }
}
