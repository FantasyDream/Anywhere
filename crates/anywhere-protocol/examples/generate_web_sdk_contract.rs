//! One source for the Web call shape, codes and byte budgets used by the SDK.
use anywhere_protocol::web_bridge::*;
use anywhere_protocol::web_plugin::{PluginSnapshot, WebCurrentApp, WebInvocation};
use serde_json::{json, Value};
use std::{error::Error, path::Path};

fn ts(schema: &Value) -> String {
    if let Some(value) = schema.get("const") {
        return value.to_string();
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(" | ");
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return reference
            .strip_prefix("#/$defs/")
            .expect("local DTO reference")
            .into();
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        return variants.iter().map(ts).collect::<Vec<_>>().join(" | ");
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        return types
            .iter()
            .map(|kind| {
                let mut shape = schema.clone();
                shape["type"] = kind.clone();
                ts(&shape)
            })
            .collect::<Vec<_>>()
            .join(" | ");
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => "string".into(),
        Some("integer" | "number") => "number".into(),
        Some("boolean") => "boolean".into(),
        Some("null") => "null".into(),
        Some("array") => format!("Array<{}>", ts(&schema["items"])),
        Some("object") => {
            let Some(properties) = schema["properties"].as_object() else {
                return if schema["additionalProperties"] == false {
                    "Record<string, never>".into()
                } else {
                    "JsonValue".into()
                };
            };
            let fields = properties
                .iter()
                .map(|(name, shape)| {
                    let optional = !schema["required"]
                        .as_array()
                        .is_some_and(|items| items.iter().any(|v| v == name));
                    format!(
                        "{}{}: {}",
                        serde_json::to_string(name).unwrap(),
                        if optional { "?" } else { "" },
                        ts(shape)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            format!("{{ {fields} }}")
        }
        _ => "JsonValue".into(),
    }
}

fn output_schema<T: schemars::JsonSchema>() -> Result<Value, serde_json::Error> {
    serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .for_serialize()
            .into_generator()
            .into_root_schema_for::<T>(),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let check = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [arg] if arg == "--check" => true,
        _ => return Err("expected --check or no arguments".into()),
    };
    let mut calls = serde_json::to_value(schemars::schema_for!(WebCall))?;
    if let Some(variants) = calls["oneOf"].as_array_mut() {
        for variant in variants {
            if let Some(props) = variant["properties"]["params"]["properties"].as_object_mut() {
                for prop in props.values_mut() {
                    let is_nullable = match prop.get("type") {
                        Some(Value::String(kind)) => kind == "null",
                        Some(Value::Array(kinds)) => kinds.iter().any(|k| k == "null"),
                        _ => false,
                    };
                    if !is_nullable && prop.get("default") == Some(&Value::Null) {
                        if let Some(map) = prop.as_object_mut() {
                            map.remove("default");
                        }
                    }
                }
            }
        }
    }
    let codes = serde_json::to_value(schemars::schema_for!(WebErrorCode))?;
    let invocation = output_schema::<WebInvocation>()?;
    let app = output_schema::<WebCurrentApp>()?;
    let snapshot = output_schema::<PluginSnapshot>()?;
    let mut methods = serde_json::Map::new();
    let mut timeouts = serde_json::Map::new();
    let mut result_schemas = Vec::new();
    for spec in WEB_METHODS {
        let result = serde_json::to_value((spec.result_schema)())?;
        methods.insert(
            spec.name.into(),
            json!({"roles":spec.roles,"permission":spec.permission,"timeoutMs":spec.timeout_ms,"result":result}),
        );
        timeouts.insert(spec.name.into(), spec.timeout_ms.into());
        result_schemas.push((spec.name, result));
    }
    let limits = json!({"frameBytes":MAX_FRAME_BYTES,"chunkBytes":MAX_CHUNK_BYTES,"textBytes":MAX_TEXT_BYTES,"transferBytes":MAX_TRANSFER_BYTES,"running":MAX_RUNNING,"queue":MAX_PENDING-MAX_RUNNING,"timeoutMs":REQUEST_TIMEOUT_MS});
    let schema = serde_json::to_string_pretty(
        &json!({"protocol":PROTOCOL,"limits":limits,"calls":calls,"methods":methods,"errors":codes,"invocation":invocation,"currentApp":app,"snapshot":snapshot}),
    )? + "\n";
    let mut typescript = format!("// Generated by generate_web_sdk_contract; do not edit.\nexport type JsonValue = null | boolean | number | string | JsonValue[] | {{ [key: string]: JsonValue }};\nexport const protocol = {PROTOCOL} as const;\nexport const limits = {} as const;\nexport const errorCodes = {} as const;\nexport type Calls = {{\n", serde_json::to_string_pretty(&limits)?, codes["enum"]);
    for variant in calls["oneOf"].as_array().ok_or("missing call variants")? {
        let method = variant["properties"]["method"]["const"]
            .as_str()
            .ok_or("missing method")?;
        typescript += &format!(
            "  {}: {};\n",
            serde_json::to_string(method)?,
            ts(&variant["properties"]["params"])
        );
    }
    typescript += "};\n";
    typescript += &format!(
        "export const methodTimeouts = {} as const;\nexport type CallResults = {{\n",
        serde_json::to_string_pretty(&timeouts)?
    );
    for (name, result) in &result_schemas {
        typescript += &format!("  {}: {};\n", serde_json::to_string(name)?, ts(result));
    }
    typescript += "};\n";
    let mut emitted = std::collections::BTreeSet::from([
        "WebInvocation".to_owned(),
        "WebCurrentApp".to_owned(),
        "PluginSnapshot".to_owned(),
    ]);
    for (name, schema) in [
        ("WebInvocation", &invocation),
        ("WebCurrentApp", &app),
        ("PluginSnapshot", &snapshot),
    ] {
        typescript += &format!("export type {name} = {};\n", ts(schema));
        if let Some(definitions) = schema["$defs"].as_object() {
            for (name, definition) in definitions {
                if emitted.insert(name.clone()) {
                    typescript += &format!("export type {name} = {};\n", ts(definition));
                }
            }
        }
    }
    for (_, schema) in result_schemas {
        if let Some(definitions) = schema["$defs"].as_object() {
            for (name, definition) in definitions {
                if emitted.insert(name.clone()) {
                    typescript += &format!("export type {name} = {};\n", ts(definition));
                }
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, expected) in [
        ("resources/web-plugin-bridge.schema.json", schema),
        ("packages/plugin-sdk/src/contract.ts", typescript),
    ] {
        let path = root.join(path);
        if check {
            if std::fs::read_to_string(&path)?.replace("\r\n", "\n") != expected {
                return Err("Web SDK contract drift; regenerate generate_web_sdk_contract".into());
            }
        } else {
            std::fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
            std::fs::write(path, expected)?;
        }
    }
    println!("Web SDK contract OK");
    Ok(())
}
