fn main() {
    println!("cargo:rerun-if-changed=native/macos.m");
    println!("cargo:rerun-if-changed=native/windows.cpp");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/macos.m")
            .flag("-fobjc-arc")
            .compile("launcher_macos");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .define("UNICODE", None)
            .define("_UNICODE", None)
            .file("native/windows.cpp")
            .compile("launcher_windows");
        for library in ["ole32", "oleaut32", "shell32", "user32", "uuid"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
}
