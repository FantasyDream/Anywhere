#[path = "build_support/feature_catalog.rs"]
mod feature_catalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let output = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    println!("cargo:rerun-if-changed=src/features");
    println!("cargo:rerun-if-changed=build_support/feature_catalog.rs");
    let sources = feature_catalog::discover(&root)?;
    feature_catalog::write_or_check(
        &output.join("builtin_feature_catalog.rs"),
        &feature_catalog::render(&sources),
        false,
    )?;
    Ok(())
}
