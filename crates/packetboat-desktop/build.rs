fn main() {
    tauri_build::build();

    // Workaround：cargo test 的测试二进制拿不到 tauri_build 嵌入的 Windows
    // manifest，而 tao 导入的 TaskDialogIndirect 是 comctl32 v6 专属函数；没有
    // manifest 时 loader 会解析到 v5，测试 exe 启动即报
    // STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)。
    // 这里给所有目标嵌入带 Common-Controls v6 依赖的 manifest，再用
    // /MANIFEST:NO 抵消 bins 上的嵌入（linker 取最后一个 /MANIFEST 开关），
    // 因为主程序的 manifest 已由 tauri_build 以资源形式嵌入，重复嵌入会产生
    // 冲突的 RT_MANIFEST 资源。
    // 参考: https://github.com/tauri-apps/tauri/pull/4383#issuecomment-1212221864
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
}
