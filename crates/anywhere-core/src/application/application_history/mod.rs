//! One source format for bundled and trusted user-owned history adapters.
#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod discovery_tests;
#[cfg(test)]
mod open_tests;
mod script;
#[cfg(test)]
mod tests;

use crate::features::application_history::model::*;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

include!(concat!(env!("OUT_DIR"), "/history_sources.rs"));

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    id: String,
    title: String,
    file_hint: String,
    #[serde(default)]
    config_paths: Vec<String>,
    applications: Vec<String>,
    action: SourceAction,
    reader: crate::persistence::application_history::Reader,
}
struct Package {
    manifest: Manifest,
    script: String,
    built_in: bool,
}
pub struct SourceCatalog {
    packages: Vec<Package>,
    config_directory: Option<std::path::PathBuf>,
    configuration_error: Option<String>,
}
impl SourceCatalog {
    pub fn bundled() -> Result<Arc<Self>> {
        Self::bundled_with_config_directory(anywhere_platform::paths::configuration_directory())
    }
    /// Explicit root keeps fixtures isolated from personal application data.
    pub fn bundled_with_config_directory(
        config_directory: Option<std::path::PathBuf>,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::load_bundled(config_directory)?))
    }
    pub fn with_user_directory(directory: &std::path::Path) -> Result<Arc<Self>> {
        Self::with_directories(
            anywhere_platform::paths::configuration_directory(),
            directory,
        )
    }
    pub fn with_directories(
        config_directory: Option<std::path::PathBuf>,
        directory: &std::path::Path,
    ) -> Result<Arc<Self>> {
        let mut catalog = Self::load_bundled(config_directory)?;
        if catalog.load_users(directory).is_err() {
            catalog.configuration_error = Some("Custom history adapters are invalid. Repair their source.json / transform.js files and restart.".into());
        }
        Ok(Arc::new(catalog))
    }
    fn load_bundled(config_directory: Option<std::path::PathBuf>) -> Result<Self> {
        let mut packages = Vec::new();
        let mut ids = BTreeSet::new();
        for (manifest, script) in BUNDLED_HISTORY_SOURCES {
            let manifest: Manifest = serde_json::from_str(manifest)?;
            ensure!(
                manifest.schema == 1 && ids.insert(manifest.id.clone()),
                "invalid source manifest"
            );
            validate_manifest(&manifest)?;
            packages.push(Package {
                manifest,
                script: (*script).into(),
                built_in: true,
            });
        }
        Ok(Self {
            packages,
            config_directory,
            configuration_error: None,
        })
    }
    fn load_users(&mut self, directory: &std::path::Path) -> Result<()> {
        let files = crate::persistence::application_history::packages::read(directory)?;
        let mut users = Vec::new();
        let mut ids = BTreeSet::new();
        for file in files {
            let mut value: serde_json::Value = serde_json::from_str(&file.manifest)?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("source must be an object"))?;
            let id = object
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("source id missing"))?
                .to_owned();
            ensure!(ids.insert(id.clone()), "duplicate user source");
            let extends = object
                .remove("extends")
                .map(serde_json::from_value::<String>)
                .transpose()?;
            let base = self
                .packages
                .iter()
                .find(|p| p.manifest.id == extends.as_deref().unwrap_or(&id));
            ensure!(
                extends.is_none() || base.is_some(),
                "unknown built-in source template"
            );
            if let Some(base) = base {
                if !object.contains_key("reader") {
                    object.insert(
                        "reader".into(),
                        serde_json::to_value(&base.manifest.reader)?,
                    );
                }
                if !object.contains_key("file_hint") {
                    object.insert("file_hint".into(), base.manifest.file_hint.clone().into());
                }
                if id == base.manifest.id && !object.contains_key("config_paths") {
                    object.insert(
                        "config_paths".into(),
                        serde_json::to_value(&base.manifest.config_paths)?,
                    );
                }
            }
            let manifest: Manifest = serde_json::from_value(value)?;
            validate_manifest(&manifest)?;
            if let Some(original) = self.packages.iter().find(|p| p.manifest.id == manifest.id) {
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
            let script = file
                .script
                .or_else(|| base.map(|p| p.script.clone()))
                .ok_or_else(|| anyhow::anyhow!("transform.js required"))?;
            ensure!(!script.trim().is_empty(), "empty source transform");
            let built_in = self.packages.iter().any(|p| p.manifest.id == manifest.id);
            users.push(Package {
                manifest,
                script,
                built_in,
            });
        }
        ensure!(
            self.packages
                .iter()
                .filter(|p| !ids.contains(&p.manifest.id))
                .count()
                + users.len()
                <= 32,
            "too many sources"
        );
        self.packages.retain(|p| !ids.contains(&p.manifest.id));
        self.packages.extend(users);
        Ok(())
    }
}
fn validate_manifest(manifest: &Manifest) -> Result<()> {
    ensure!(
        manifest.schema == 1
            && crate::features::descriptor::valid_id(&manifest.id)
            && manifest.id.len() <= 64,
        "invalid source identity"
    );
    ensure!(
        !manifest.title.trim().is_empty()
            && manifest.title.len() <= 256
            && !manifest.action.title.trim().is_empty()
            && manifest.action.title.len() <= 256
            && manifest.file_hint.len() <= 256,
        "invalid source title"
    );
    ensure!(
        !manifest.applications.is_empty()
            && manifest.applications.len() <= 32
            && manifest
                .applications
                .iter()
                .all(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)),
        "invalid application identities"
    );
    ensure!(
        manifest.action.aliases.len() <= 32
            && manifest.action.aliases.iter().all(|v| v.len() <= 256)
            && manifest.action.localized_titles.len() <= 16
            && manifest
                .action
                .localized_titles
                .iter()
                .all(|(k, v)| k.len() <= 32 && !v.trim().is_empty() && v.len() <= 256),
        "invalid source search metadata"
    );
    crate::persistence::application_history::discovery::validate(&manifest.config_paths)
}
impl SourceLoader for SourceCatalog {
    fn configuration_error(&self) -> Option<&str> {
        self.configuration_error.as_deref()
    }
    fn detect(&self, id: &str) -> Option<std::path::PathBuf> {
        let package = self.packages.iter().find(|p| p.manifest.id == id)?;
        crate::persistence::application_history::discovery::detect(
            self.config_directory.as_deref()?,
            &package.manifest.config_paths,
        )
    }
    fn descriptors(&self) -> Vec<SourceDescriptor> {
        self.packages
            .iter()
            .map(|p| SourceDescriptor {
                built_in: p.built_in,
                id: p.manifest.id.clone(),
                title: p.manifest.title.clone(),
                file_hint: p.manifest.file_hint.clone(),
                applications: p.manifest.applications.clone(),
                action: p.manifest.action.clone(),
            })
            .collect()
    }
    fn read(&self, source: &SourcePreference, cancelled: Cancelled) -> Result<Vec<Entry>> {
        ensure!(
            self.configuration_error.is_none(),
            "custom history adapters unavailable"
        );
        let package = self
            .packages
            .iter()
            .find(|p| p.manifest.id == source.id)
            .ok_or_else(|| anyhow::anyhow!("unknown history source"))?;
        let started = Instant::now();
        let cancelled: Cancelled =
            Arc::new(move || cancelled() || started.elapsed() > Duration::from_secs(5));
        let input = crate::persistence::application_history::read(
            &source.path,
            &package.manifest.reader,
            cancelled.clone(),
        )?;
        script::transform(&package.script, &input, cancelled)
    }
}
