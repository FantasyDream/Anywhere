//! One source format for bundled and trusted user-owned history adapters.
#[cfg(test)]
mod adapters_tests;
#[cfg(test)]
mod catalog_tests;
mod definition;
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
        let mut packages: Vec<Package> = Vec::new();
        let mut pending: Vec<_> = BUNDLED_HISTORY_SOURCES
            .iter()
            .map(|(manifest, script)| {
                Ok((
                    serde_json::from_str::<serde_json::Value>(manifest)?,
                    *script,
                ))
            })
            .collect::<Result<_>>()?;
        while !pending.is_empty() {
            let previous = pending.len();
            let mut deferred = Vec::new();
            for (value, script) in pending {
                if value
                    .get("extends")
                    .and_then(|v| v.as_str())
                    .is_some_and(|base| !packages.iter().any(|p| p.manifest.id == base))
                {
                    deferred.push((value, script));
                    continue;
                }
                let mut package = definition::resolve(value, script.map(str::to_owned), &packages)?;
                ensure!(
                    !packages
                        .iter()
                        .any(|p| p.manifest.id == package.manifest.id),
                    "duplicate bundled source"
                );
                package.built_in = true;
                packages.push(package);
            }
            ensure!(
                deferred.len() < previous,
                "unknown or cyclic bundled source template"
            );
            pending = deferred;
        }
        ensure!(packages.len() <= 32, "too many bundled sources");
        packages.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
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
            let package = definition::resolve(
                serde_json::from_str(&file.manifest)?,
                file.script,
                &self.packages,
            )?;
            ensure!(
                ids.insert(package.manifest.id.clone()),
                "duplicate user source"
            );
            users.push(package);
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
