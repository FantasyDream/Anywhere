//! Development-only schema generation. Package semantics remain checked by anywhere-plugin.
use anywhere_protocol::web_plugin::{
    WebPluginManifest, API_VERSION, COMMAND_ICONS, MAX_PERMISSIONS, SCHEMA_VERSION,
};
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
    schema["properties"]["permissions"]["maxItems"] = MAX_PERMISSIONS.into();
    if let Some(titles) =
        schema["$defs"]["WebCommand"]["properties"]["localizedTitles"].as_object_mut()
    {
        titles.insert("maxProperties".into(), 32.into());
    }
    schema["x-anywhere-commandIcons"] = serde_json::to_value(COMMAND_ICONS)?;
    let mut editor = schema.clone();
    editor["properties"]["$schema"] = serde_json::json!({ "type": "string" });
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    for (name, schema) in [
        ("web-plugin-manifest.schema.json", schema),
        ("web-plugin-manifest-editor.schema.json", editor),
    ] {
        let path = resources.join(name);
        let value = serde_json::to_string_pretty(&schema)? + "\n";
        if check {
            if std::fs::read_to_string(&path)?.replace("\r\n", "\n") != value {
                return Err(format!("{name} drift; run generate_web_plugin_schema").into());
            }
        } else {
            std::fs::write(&path, value)?;
        }
    }
    println!("Web plugin Manifest schema OK");
    Ok(())
}
