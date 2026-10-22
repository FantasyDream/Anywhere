#[path = "../build_support/feature_catalog.rs"]
mod feature_catalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let check = match arguments.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("usage: generate_feature_catalog [--check]".into()),
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = feature_catalog::discover(root)?;
    feature_catalog::write_or_check(
        &root.join("src/features/catalog_generated.rs"),
        &feature_catalog::render(&sources),
        check,
    )?;
    println!(
        "Builtin feature catalogue {}: {} registration modules",
        if check { "verified" } else { "generated" },
        sources.len()
    );
    Ok(())
}
