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
    let directory = root.join("resources/application-history");
    println!("cargo:rerun-if-changed=resources/application-history");
    let mut packages: Vec<_> = std::fs::read_dir(directory)?.collect::<Result<_, _>>()?;
    packages.sort_by_key(|entry| entry.file_name());
    let mut embedded =
        String::from("const BUNDLED_HISTORY_SOURCES: &[(&str, Option<&str>)] = &[\n");
    for package in packages {
        if !package.file_type()?.is_dir() {
            continue;
        }
        let manifest = package.path().join("source.json");
        let script = package.path().join("transform.js");
        assert!(manifest.is_file(), "history source requires source.json");
        let script = if script.is_file() {
            format!("Some(include_str!({script:?}))")
        } else {
            "None".into()
        };
        embedded.push_str(&format!("(include_str!({manifest:?}), {script}),\n"));
    }
    embedded.push_str("];\n");
    std::fs::write(output.join("history_sources.rs"), embedded)?;
    Ok(())
}
