//! Bounded file reads shared by project recognizers; no project command execution.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{io::Read, path::Path};

pub const NODE_METADATA: &str = "node.metadata";
pub const CARGO_METADATA: &str = "rust.metadata";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
}
impl PackageManager {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
        }
    }
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "npm" => Ok(Self::Npm),
            "pnpm" => Ok(Self::Pnpm),
            "yarn" => Ok(Self::Yarn),
            _ => bail!("unsupported package manager"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeMetadata {
    version: u32,
    pub manager: PackageManager,
    pub scripts: Vec<String>,
}
impl NodeMetadata {
    pub fn new(manager: &str, scripts: Vec<String>) -> Result<Self> {
        Ok(Self {
            version: SCHEMA_VERSION,
            manager: PackageManager::parse(manager)?,
            scripts,
        })
    }
    pub fn write(&self, properties: &mut BTreeMap<String, String>) -> Result<()> {
        properties.insert(NODE_METADATA.into(), serde_json::to_string(self)?);
        for script in &self.scripts {
            properties.insert(
                super::node::capability(self.manager.as_str(), script),
                "true".into(),
            );
            properties.insert(super::provider::script_property(script), "true".into());
        }
        Ok(())
    }
    pub fn read(properties: &BTreeMap<String, String>) -> Result<Self> {
        let metadata: Self = decode(properties, NODE_METADATA)?;
        if metadata.version != SCHEMA_VERSION {
            bail!("unsupported Node metadata version");
        }
        if metadata.scripts.len() > super::node::MAX_SCRIPTS
            || metadata
                .scripts
                .iter()
                .any(|name| !super::node::supported_script_name(name))
        {
            bail!("invalid Node script metadata");
        }
        for script in &metadata.scripts {
            let key = super::node::capability(metadata.manager.as_str(), script);
            if properties.get(&key).map(String::as_str) != Some("true") {
                bail!("missing Node script capability");
            }
        }
        Ok(metadata)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoTargetMetadata {
    pub package: String,
    pub binary: String,
    pub manifest: String,
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
#[serde(deny_unknown_fields)]
pub struct CargoMetadata {
    version: u32,
    pub targets: Vec<CargoTargetMetadata>,
    /// A recognition failure is different from a valid library with no binary.
    pub discovery_failed: bool,
}
impl CargoMetadata {
    pub fn new(targets: Vec<CargoTargetMetadata>, discovery_failed: bool) -> Self {
        Self {
            version: SCHEMA_VERSION,
            targets,
            discovery_failed,
        }
    }
    pub fn write(&self, properties: &mut BTreeMap<String, String>) -> Result<()> {
        properties.insert(CARGO_METADATA.into(), serde_json::to_string(self)?);
        for target in &self.targets {
            properties.insert(target.key(), "true".into());
        }
        Ok(())
    }
    pub fn read(properties: &BTreeMap<String, String>) -> Result<Self> {
        let metadata: Self = decode(properties, CARGO_METADATA)?;
        if metadata.version != SCHEMA_VERSION {
            bail!("unsupported Cargo metadata version");
        }
        if metadata.targets.len() > 128
            || (metadata.discovery_failed && !metadata.targets.is_empty())
        {
            bail!("invalid Cargo target metadata");
        }
        for target in &metadata.targets {
            if [&target.package, &target.binary].iter().any(|name| {
                name.is_empty() || name.len() > 200 || name.chars().any(char::is_control)
            }) || target.manifest.is_empty()
                || Path::new(&target.manifest)
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                bail!("invalid Cargo target metadata");
            }
            if properties.get(&target.key()).map(String::as_str) != Some("true") {
                bail!("missing Cargo target capability");
            }
        }
        Ok(metadata)
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    properties: &BTreeMap<String, String>,
    key: &str,
) -> Result<T> {
    let raw = properties.get(key).context("project metadata is missing")?;
    if raw.len() > MAX_MANIFEST_BYTES {
        bail!("project metadata exceeds size limit");
    }
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
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
