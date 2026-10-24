//! Composition for the official source packages. No external script installation.
mod script;
#[cfg(test)]
mod tests;

use crate::features::application_history::model::*;
use anyhow::{ensure, Result};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

include!(concat!(env!("OUT_DIR"), "/history_sources.rs"));

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    id: String,
    title: String,
    file_hint: String,
    applications: Vec<String>,
    reader: crate::persistence::application_history::Reader,
}
struct Package {
    manifest: Manifest,
    script: &'static str,
}
pub struct OfficialSources {
    packages: Vec<Package>,
}
impl OfficialSources {
    pub fn bundled() -> Result<Arc<Self>> {
        let mut packages = Vec::new();
        let mut ids = BTreeSet::new();
        for (manifest, script) in BUNDLED_HISTORY_SOURCES {
            let manifest: Manifest = serde_json::from_str(manifest)?;
            ensure!(
                manifest.schema == 1 && ids.insert(manifest.id.clone()),
                "invalid source manifest"
            );
            ensure!(
                !manifest.applications.is_empty() && !manifest.title.is_empty(),
                "source identity missing"
            );
            packages.push(Package { manifest, script });
        }
        Ok(Arc::new(Self { packages }))
    }
}
impl SourceLoader for OfficialSources {
    fn descriptors(&self) -> Vec<SourceDescriptor> {
        self.packages
            .iter()
            .map(|p| SourceDescriptor {
                id: p.manifest.id.clone(),
                title: p.manifest.title.clone(),
                file_hint: p.manifest.file_hint.clone(),
                applications: p.manifest.applications.clone(),
            })
            .collect()
    }
    fn read(&self, source: &SourcePreference, cancelled: Cancelled) -> Result<Vec<Entry>> {
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
        script::transform(package.script, &input, cancelled)
    }
}
