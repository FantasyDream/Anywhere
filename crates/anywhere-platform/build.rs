//! Compiles the macOS native shim.
//!
//! Set `ANYWHERE_SKIP_NATIVE=1` to skip compilation and only type-check the Rust side.
fn main() {
    println!("cargo:rerun-if-changed=native/macos.m");
    println!("cargo:rerun-if-changed=native/macos_menu.m");
    println!("cargo:rerun-if-changed=native/macos_process.h");
    println!("cargo:rerun-if-changed=native/macos_application_names.h");
    println!("cargo:rerun-if-env-changed=ANYWHERE_SKIP_NATIVE");
    println!("cargo:rerun-if-env-changed=ANYWHERE_MENU_IN_HOST");
    let skip = std::env::var("ANYWHERE_SKIP_NATIVE").is_ok_and(|value| value == "1");
    let menu_in_host = std::env::var("ANYWHERE_MENU_IN_HOST").as_deref() == Ok("1");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    if !skip {
        let mut build = cc::Build::new();
        build.file("native/macos.m").flag("-fobjc-arc");
        if !menu_in_host {
            build.file("native/macos_menu.m");
        }
        build.compile("launcher_macos");
    }
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
}
