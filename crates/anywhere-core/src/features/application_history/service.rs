//! Source preferences, publication and execution share one revocable lifetime.
//! Live applications change configuration through SettingsService, which also
//! publishes the action catalog; domain mutations alone serve standalone readers.
use super::{
    model::*,
    preferences::{self, Preferences},
};
use crate::core::storage::ExtensionStorage;
use anyhow::{ensure, Result};

mod execution;
mod query;
pub use query::InteractiveSearch;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

pub(super) struct IndexedEntry {
    pub entry: Entry,
    pub key_digest: String,
    pub search: std::sync::OnceLock<crate::core::search::query::SearchFields>,
}

struct CachedSource {
    source: SourcePreference,
    revision: SourceRevision,
    entries: Arc<[IndexedEntry]>,
}

struct State {
    latest_interactive_revision: Option<u64>,
    cache: Option<CachedSource>,
    configurations: Vec<SourceConfiguration>,
    sources: Vec<SourcePreference>,
    items: Arc<[Item]>,
}
type ApplicationLookup =
    Arc<dyn Fn(&[String]) -> Vec<anywhere_platform::BundleApplication> + Send + Sync>;

pub struct HistoryService {
    storage: Result<Arc<dyn ExtensionStorage>, String>,
    loader: Arc<dyn SourceLoader>,
    installed_applications: std::collections::BTreeSet<String>,
    application_lookup: Option<ApplicationLookup>,
    state: Mutex<State>,
    worker: Arc<tokio::sync::Semaphore>,
    query_changed: tokio::sync::Notify,
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
            application_lookup: None,
            state: Mutex::new(State {
                latest_interactive_revision: None,
                cache: None,
                configurations: vec![],
                sources: vec![],
                items: Default::default(),
            }),
            worker: Arc::new(tokio::sync::Semaphore::new(1)),
            query_changed: tokio::sync::Notify::new(),
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
    pub fn with_application_lookup(mut self, lookup: ApplicationLookup) -> Self {
        self.application_lookup = Some(lookup);
        self
    }

    fn read_for_query(
        &self,
        source: &SourcePreference,
        cancelled: Cancelled,
    ) -> Result<Arc<[IndexedEntry]>> {
        use sha2::{Digest, Sha256};
        let revision = self.loader.revision(source);
        if let Some(revision) = &revision {
            let state = self.state.lock().unwrap();
            if let Some(cache) = state
                .cache
                .as_ref()
                .filter(|cache| cache.source == *source && cache.revision == *revision)
            {
                ensure!(!cancelled(), "history query expired");
                return Ok(cache.entries.clone());
            }
        }
        let entries: Arc<[IndexedEntry]> = self
            .loader
            .read(source, cancelled.clone())?
            .into_iter()
            .map(|entry| IndexedEntry {
                key_digest: hex::encode(Sha256::digest(entry.key.as_bytes())),
                entry,
                search: std::sync::OnceLock::new(),
            })
            .collect::<Vec<_>>()
            .into();
        if let Some(revision) = revision {
            if self.loader.revision(source).as_ref() == Some(&revision) {
                let mut state = self.state.lock().unwrap();
                if !cancelled() {
                    state.cache = Some(CachedSource {
                        source: source.clone(),
                        revision,
                        entries: entries.clone(),
                    });
                }
            }
        }
        Ok(entries)
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
            self.advance_epoch();
            state.configurations = configurations;
            state.sources = sources;
            state.items = Arc::default();
            state.cache = None;
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
        let descriptor = self
            .loader
            .descriptors()
            .into_iter()
            .find(|source| source.id == id)
            .ok_or_else(|| anyhow::anyhow!("unknown source"))?;
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
        self.save_configuration(id, |current| SourceConfiguration {
            id: id.into(),
            mode,
            path: path.clone(),
            // Choosing a location must not undo an explicit action switch,
            // including one committed by another service during validation.
            action_enabled: mode != SourceMode::Disabled
                && self.action_enabled(&descriptor, current),
        })
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
    fn advance_epoch(&self) -> u64 {
        let epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        self.query_changed.notify_waiters();
        epoch
    }
    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        self.advance_epoch();
        state.items = Arc::default();
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Loader {
        reads: AtomicUsize,
        journal: AtomicU64,
    }
    impl SourceLoader for Loader {
        fn descriptors(&self) -> Vec<SourceDescriptor> {
            vec![]
        }
        fn revision(&self, source: &SourcePreference) -> Option<SourceRevision> {
            Some(SourceRevision {
                path: source.path.clone(),
                bytes: 10,
                modified: std::time::SystemTime::UNIX_EPOCH,
                journal: Some((
                    self.journal.load(Ordering::Acquire),
                    std::time::SystemTime::UNIX_EPOCH,
                )),
                rollback_journal: None,
                header_fingerprint: [0; 64],
            })
        }
        fn read(&self, _: &SourcePreference, _: Cancelled) -> Result<Vec<Entry>> {
            self.reads.fetch_add(1, Ordering::AcqRel);
            Ok(vec![])
        }
    }
    #[test]
    fn query_cache_reuses_one_read_but_rechecks_wal_and_session_ownership() {
        let loader = Arc::new(Loader {
            reads: AtomicUsize::new(0),
            journal: AtomicU64::new(0),
        });
        let service = HistoryService::new(Err("synthetic".into()), loader.clone());
        let source = SourcePreference {
            id: "synthetic".into(),
            path: "/synthetic/history.db".into(),
        };
        let current: Cancelled = Arc::new(|| false);
        let first = service.read_for_query(&source, current.clone()).unwrap();
        let second = service.read_for_query(&source, current.clone()).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(loader.reads.load(Ordering::Acquire), 1);
        loader.journal.store(1, Ordering::Release);
        service.read_for_query(&source, current.clone()).unwrap();
        assert_eq!(loader.reads.load(Ordering::Acquire), 2);
        assert!(service.read_for_query(&source, Arc::new(|| true)).is_err());
        service.cancel_session(0);
        service.read_for_query(&source, current).unwrap();
        assert_eq!(loader.reads.load(Ordering::Acquire), 3);
    }
}
