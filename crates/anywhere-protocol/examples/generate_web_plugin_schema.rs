//! Development-only schema generation. Package semantics remain checked by anywhere-plugin.
use anywhere_protocol::web_plugin::{WebPluginManifest, API_VERSION, SCHEMA_VERSION};
use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let check = match arguments.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("expected no arguments or --check".into()),
    };
    let mut schema = serde_json::to_value(schemars::schema_for!(WebPluginManifest))?;
    schema["properties"]["schemaVersion"]["const"] = SCHEMA_VERSION.into();
    schema["properties"]["apiVersion"]["const"] = API_VERSION.into();
    let value = serde_json::to_string_pretty(&schema)? + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/web-plugin-manifest.schema.json");
    if check {
        if std::fs::read_to_string(&path)? != value {
            return Err(
                "Web plugin schema drift; regenerate with generate_web_plugin_schema".into(),
            );
        }
    } else {
        std::fs::write(&path, value)?;
    }
    println!("Web plugin Manifest schema OK");
    Ok(())
}
