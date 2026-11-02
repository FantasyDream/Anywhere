//! Captured project metadata. Manifest readers validate external files;
//! action expansion only decodes the captured values.
use super::node::PackageManager;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{io::Read, path::Path};

pub const NODE_METADATA: &str = "node.metadata";
pub const CARGO_METADATA: &str = "rust.metadata";

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub manager: PackageManager,
    pub scripts: Vec<String>,
}
impl NodeMetadata {
    pub fn write(&self, properties: &mut BTreeMap<String, String>) -> Result<()> {
        properties.insert(NODE_METADATA.into(), serde_json::to_string(self)?);
        for script in &self.scripts {
            properties.insert(
                super::node::capability(self.manager.as_str(), script),
                "true".into(),
            );
        }
        Ok(())
    }
    pub fn read(properties: &BTreeMap<String, String>) -> Result<Self> {
        decode(properties, NODE_METADATA)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CargoTargetMetadata {
    pub package: String,
    pub binary: String,
}
impl CargoTargetMetadata {
    pub fn key(&self) -> String {
        format!(
            "rust.run.{}",
            encoded(&format!("{}\0{}", self.package, self.binary))
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CargoMetadata {
    pub targets: Vec<CargoTargetMetadata>,
    /// A recognition failure is different from a valid library with no binary.
    pub discovery_failed: bool,
}
impl CargoMetadata {
    pub fn write(&self, properties: &mut BTreeMap<String, String>) -> Result<()> {
        properties.insert(CARGO_METADATA.into(), serde_json::to_string(self)?);
        for target in &self.targets {
            properties.insert(target.key(), "true".into());
        }
        Ok(())
    }
    pub fn read(properties: &BTreeMap<String, String>) -> Result<Self> {
        decode(properties, CARGO_METADATA)
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    properties: &BTreeMap<String, String>,
    key: &str,
) -> Result<T> {
    let raw = properties.get(key).context("project metadata is missing")?;
    // Keep potentially sensitive input values out of diagnostics.
    serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("project metadata has invalid fields or types"))
}
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;

pub fn read_manifest(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).context("project manifest could not be read")?;
    if !file.metadata()?.is_file() {
        bail!("project manifest must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take((MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        bail!("project manifest exceeds the 256 KiB limit");
    }
    String::from_utf8(bytes).context("project manifest must be UTF-8")
}

pub fn encoded(value: &str) -> String {
    hex::encode(value)
}
