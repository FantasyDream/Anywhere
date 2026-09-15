//! Compiles the small native shim for the target platform.
//!
//! Set `ANYWHERE_SKIP_NATIVE=1` to skip compilation and only type-check the Rust side.
//! That is what makes `cargo check --target x86_64-pc-windows-msvc` usable from macOS,
//! where the Windows SDK headers the shim needs are absent. It never yields a working
//! binary: the extern symbols stay unresolved, so only `check` succeeds, not `build`.
fn main() {
    println!("cargo:rerun-if-changed=native/macos.m");
    println!("cargo:rerun-if-changed=native/windows.cpp");
    println!("cargo:rerun-if-env-changed=ANYWHERE_SKIP_NATIVE");
    let skip = std::env::var("ANYWHERE_SKIP_NATIVE").is_ok_and(|value| value == "1");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        if !skip {
            cc::Build::new()
                .file("native/macos.m")
                .flag("-fobjc-arc")
                .compile("launcher_macos");
        }
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if !skip {
            cc::Build::new()
                .cpp(true)
                .std("c++17")
                .define("UNICODE", None)
                .define("_UNICODE", None)
                .file("native/windows.cpp")
                .compile("launcher_windows");
        }
        for library in ["ole32", "oleaut32", "shell32", "user32", "uuid"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
}
