//! Bundled and user definitions share validation and parser inheritance.
use super::{validate_manifest, Manifest, Package};
use anyhow::{ensure, Result};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn resolve(
    mut value: Value,
    script: Option<String>,
    templates: &[Package],
) -> Result<Package> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("source must be an object"))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("source id missing"))?
        .to_owned();
    let extends = object
        .remove("extends")
        .map(serde_json::from_value::<String>)
        .transpose()?;
    let base = templates
        .iter()
        .find(|p| p.manifest.id == extends.as_deref().unwrap_or(&id));
    ensure!(
        extends.is_none() || base.is_some(),
        "unknown source template"
    );
    if let Some(base) = base {
        for (key, inherited) in [
            ("reader", serde_json::to_value(&base.manifest.reader)?),
            ("file_hint", base.manifest.file_hint.clone().into()),
        ] {
            object.entry(key).or_insert(inherited);
        }
        if id == base.manifest.id {
            object
                .entry("config_paths")
                .or_insert(serde_json::to_value(&base.manifest.config_paths)?);
        }
    }
    let manifest: Manifest = serde_json::from_value(value)?;
    validate_manifest(&manifest)?;
    let original = templates.iter().find(|p| p.manifest.id == manifest.id);
    if let Some(original) = original {
        ensure!(
            manifest.applications.iter().collect::<BTreeSet<_>>()
                == original
                    .manifest
                    .applications
                    .iter()
                    .collect::<BTreeSet<_>>(),
            "use a new source id when changing application identity"
        );
    }
    let script = script
        .or_else(|| base.map(|p| p.script.clone()))
        .ok_or_else(|| anyhow::anyhow!("transform.js required"))?;
    ensure!(!script.trim().is_empty(), "empty source transform");
    Ok(Package {
        manifest,
        script,
        built_in: original.is_some(),
    })
}
