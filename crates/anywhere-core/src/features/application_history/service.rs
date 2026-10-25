//! Source preferences, publication and execution share one revocable lifetime.
use super::{
    model::*,
    preferences::{self, Preferences},
};
use crate::core::storage::ExtensionStorage;
use anyhow::{ensure, Result};

mod execution;
mod query;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

struct State {
    configurations: Vec<SourceConfiguration>,
    sources: Vec<SourcePreference>,
    items: Vec<Item>,
}
pub struct HistoryService {
    storage: Result<Arc<dyn ExtensionStorage>, String>,
    loader: Arc<dyn SourceLoader>,
    installed_applications: std::collections::BTreeSet<String>,
    state: Mutex<State>,
    worker: Mutex<()>,
    configuration: Mutex<()>,
    epoch: Arc<AtomicU64>,
    owner: AtomicU64,
    enabled: Arc<AtomicBool>,
}
impl HistoryService {
    pub fn new(
        storage: Result<Arc<dyn ExtensionStorage>, String>,
        loader: Arc<dyn SourceLoader>,
    ) -> Self {
        Self {
            storage,
            loader,
            installed_applications: Default::default(),
            state: Mutex::new(State {
                configurations: vec![],
                sources: vec![],
                items: vec![],
            }),
            worker: Mutex::new(()),
            configuration: Mutex::new(()),
            epoch: Arc::new(AtomicU64::new(1)),
            owner: AtomicU64::new(0),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }
    /// Startup metadata only; implicit defaults never become saved user choices.
    pub fn with_installed_applications(
        mut self,
        applications: impl IntoIterator<Item = String>,
    ) -> Self {
        self.installed_applications = applications.into_iter().collect();
        self
    }
    fn action_enabled(
        &self,
        source: &SourceDescriptor,
        setting: Option<&SourceConfiguration>,
    ) -> bool {
        setting.map_or_else(
            || {
                source
                    .applications
                    .iter()
                    .any(|id| self.installed_applications.contains(id))
            },
            |s| s.action_enabled && s.mode != SourceMode::Disabled,
        )
    }
    pub fn refresh(&self) -> Result<()> {
        let _configuration = self.configuration.lock().unwrap();
        ensure!(
            self.loader.configuration_error().is_none(),
            "custom history adapters unavailable; repair configuration and restart"
        );
        let bytes = self
            .storage
            .as_ref()
            .map_err(|_| anyhow::anyhow!("history preferences unavailable"))?
            .get("sources")?;
        let configurations = preferences::decode(bytes.as_deref(), &self.loader.descriptors())?;
        self.publish_configuration(configurations);
        Ok(())
    }
    fn publish_configuration(&self, configurations: Vec<SourceConfiguration>) {
        let sources = self
            .loader
            .descriptors()
            .into_iter()
            .filter_map(|descriptor| {
                let setting = configurations.iter().find(|s| s.id == descriptor.id);
                if !self.action_enabled(&descriptor, setting) {
                    return None;
                }
                let path = match setting.map_or(SourceMode::Automatic, |s| s.mode) {
                    SourceMode::Automatic => self.loader.detect(&descriptor.id),
                    SourceMode::Custom => setting.and_then(|s| s.path.clone()),
                    SourceMode::Disabled => None,
                }?;
                Some(SourcePreference {
                    id: descriptor.id,
                    path,
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.state.lock().unwrap();
        if state.configurations != configurations || state.sources != sources {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            state.configurations = configurations;
            state.sources = sources;
            state.items.clear();
        }
    }
    pub fn sources(&self) -> Vec<SourceStatus> {
        let state = self.state.lock().unwrap();
        self.loader
            .descriptors()
            .into_iter()
            .map(|d| SourceStatus {
                action_enabled: self
                    .action_enabled(&d, state.configurations.iter().find(|s| s.id == d.id)),
                mode: state
                    .configurations
                    .iter()
                    .find(|p| p.id == d.id)
                    .map_or(SourceMode::Automatic, |s| s.mode),
                path: state
                    .sources
                    .iter()
                    .find(|p| p.id == d.id)
                    .map(|s| s.path.clone())
                    .or_else(|| {
                        state
                            .configurations
                            .iter()
                            .find(|s| s.id == d.id)
                            .and_then(|s| s.path.clone())
                    }),
                descriptor: d,
            })
            .collect()
    }
    pub fn page_input(&self, id: &str) -> Result<PageInput> {
        ensure!(
            self.enabled.load(Ordering::Acquire),
            "history feature disabled"
        );
        ensure!(
            self.sources()
                .iter()
                .any(|s| s.descriptor.id == id && s.action_enabled),
            "history action disabled"
        );
        let source = self
            .loader
            .descriptors()
            .into_iter()
            .find(|source| source.id == id)
            .ok_or_else(|| anyhow::anyhow!("unknown history source"))?;
        Ok(PageInput {
            source_id: source.id,
            title: source.action.title,
            localized_titles: source.action.localized_titles,
        })
    }
    pub fn configure_mode(&self, id: &str, mode: SourceMode, path: Option<&str>) -> Result<()> {
        let _configuration = self.configuration.lock().unwrap();
        ensure!(
            self.loader.descriptors().iter().any(|d| d.id == id),
            "unknown source"
        );
        let path = if mode == SourceMode::Custom {
            let path = path
                .filter(|p| !p.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("custom source path required"))?;
            let path = anywhere_platform::paths::expand_home(path.trim())
                .ok_or_else(|| anyhow::anyhow!("invalid source path"))?
                .canonicalize()?;
            ensure!(path.is_file(), "source must be a file");
            std::fs::File::open(&path)?;
            Some(path)
        } else {
            None
        };
        let next = SourceConfiguration {
            id: id.into(),
            mode,
            path,
            action_enabled: mode != SourceMode::Disabled,
        };
        self.save_configuration(id, |_| next.clone())
    }
    pub fn set_action_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let _configuration = self.configuration.lock().unwrap();
        ensure!(
            self.loader.descriptors().iter().any(|s| s.id == id),
            "unknown source"
        );
        self.save_configuration(id, |current| {
            let mut next = current.cloned().unwrap_or(SourceConfiguration {
                id: id.into(),
                mode: SourceMode::Automatic,
                path: None,
                action_enabled: true,
            });
            next.action_enabled = enabled;
            if enabled && next.mode == SourceMode::Disabled {
                next.mode = SourceMode::Automatic;
            }
            next
        })
    }
    pub fn disabled_action_ids(&self) -> std::collections::BTreeSet<String> {
        self.sources()
            .into_iter()
            .filter(|s| !s.action_enabled)
            .map(|s| format!("core.application_history.{}.open", s.descriptor.id))
            .collect()
    }
    fn save_configuration(
        &self,
        id: &str,
        change: impl Fn(Option<&SourceConfiguration>) -> SourceConfiguration,
    ) -> Result<()> {
        let storage = self
            .storage
            .as_ref()
            .map_err(|_| anyhow::anyhow!("history preferences unavailable"))?;
        let updated = storage
            .update("sources", &mut |current| {
                let mut merged = preferences::decode(current, &self.loader.descriptors())?;
                // Derive a partial change from the latest committed row inside
                // the transaction, including changes made by other instances.
                let next = change(merged.iter().find(|source| source.id == id));
                merged.retain(|source| source.id != id);
                merged.push(next);
                preferences::validate(&merged)?;
                Ok(Some(serde_json::to_vec(&Preferences {
                    schema: 2,
                    sources: merged,
                })?))
            })?
            .ok_or_else(|| anyhow::anyhow!("history preferences missing"))?;
        let next = serde_json::from_slice::<Preferences>(&updated)?.sources;
        self.publish_configuration(next);
        Ok(())
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        self.cancel();
    }
    fn cancellation(&self, expected: u64) -> Cancelled {
        let epoch = self.epoch.clone();
        let enabled = self.enabled.clone();
        Arc::new(move || {
            epoch.load(Ordering::Acquire) != expected || !enabled.load(Ordering::Acquire)
        })
    }
    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.items.clear();
    }
}
