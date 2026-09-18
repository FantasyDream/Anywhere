fn main() {
    println!("cargo:rerun-if-env-changed=ANYWHERE_MENU_IN_HOST");
    if std::env::var("ANYWHERE_MENU_IN_HOST").as_deref() == Ok("1") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-undefined,dynamic_lookup");
    }
}
