//! Source preferences, publication and execution share one revocable lifetime.
use super::{
    model::*,
    preferences::{self, Preferences},
};
use crate::core::{
    search::query::{Query, SearchFields},
    storage::ExtensionStorage,
};
use anyhow::{bail, ensure, Result};
use sha2::{Digest, Sha256};
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
    pub fn configure(&self, id: &str, path: Option<&str>) -> Result<()> {
        self.configure_mode(
            id,
            if path.is_some_and(|p| !p.trim().is_empty()) {
                SourceMode::Custom
            } else {
                SourceMode::Disabled
            },
            path,
        )
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
        self.save_configuration(next)
    }
    pub fn set_action_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let _configuration = self.configuration.lock().unwrap();
        ensure!(
            self.loader.descriptors().iter().any(|s| s.id == id),
            "unknown source"
        );
        let current = self
            .storage
            .as_ref()
            .map_err(|_| anyhow::anyhow!("history preferences unavailable"))?
            .get("sources")?;
        let mut next = preferences::decode(current.as_deref(), &self.loader.descriptors())?
            .into_iter()
            .find(|s| s.id == id)
            .unwrap_or(SourceConfiguration {
                id: id.into(),
                mode: SourceMode::Automatic,
                path: None,
                action_enabled: true,
            });
        next.action_enabled = enabled;
        if enabled && next.mode == SourceMode::Disabled {
            next.mode = SourceMode::Automatic;
        }
        self.save_configuration(next)
    }
    pub fn disabled_action_ids(&self) -> std::collections::BTreeSet<String> {
        self.sources()
            .into_iter()
            .filter(|s| !s.action_enabled)
            .map(|s| format!("core.application_history.{}.open", s.descriptor.id))
            .collect()
    }
    fn save_configuration(&self, next: SourceConfiguration) -> Result<()> {
        preferences::validate(std::slice::from_ref(&next))?;
        let storage = self
            .storage
            .as_ref()
            .map_err(|_| anyhow::anyhow!("history preferences unavailable"))?;
        let updated = storage
            .update("sources", &mut |current| {
                let mut merged = preferences::decode(current, &self.loader.descriptors())?;
                merged.retain(|source| source.id != next.id);
                merged.push(next.clone());
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
    pub fn open_session(&self) -> Result<u64> {
        self.refresh()?;
        let mut state = self.state.lock().unwrap();
        let owner = self
            .owner
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| anyhow::anyhow!("history session exhausted"))?
            + 1;
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.items.clear();
        Ok(owner)
    }
    pub fn cancel_session(&self, owner: u64) {
        let mut state = self.state.lock().unwrap();
        if owner == self.owner.load(Ordering::Acquire) {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            state.items.clear();
        }
    }
    pub fn search(&self, owner: u64, text: &str, filter: &str) -> Result<SearchResult> {
        ensure!(text.len() <= 1024, "history query too large");
        let epoch = {
            let mut state = self.state.lock().unwrap();
            if owner != self.owner.load(Ordering::Acquire) {
                return Ok(SearchResult {
                    items: vec![],
                    failed_sources: vec![],
                    notice: Some(Notice::Stale),
                });
            }
            let epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
            state.items.clear();
            epoch
        };
        let empty = |notice| SearchResult {
            items: vec![],
            failed_sources: vec![],
            notice: Some(notice),
        };
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(empty(Notice::Disabled));
        }
        let Ok(_worker) = self.worker.try_lock() else {
            return Ok(empty(Notice::Busy));
        };
        let cancelled = self.cancellation(epoch);
        let current = || !cancelled();
        let sources = self.state.lock().unwrap().sources.clone();
        if !sources
            .iter()
            .any(|source| filter.is_empty() || source.id == filter)
        {
            return Ok(empty(Notice::Unconfigured));
        }
        let query = Query::parse(text);
        let mut ranked = Vec::new();
        let mut failed = Vec::new();
        for source in sources
            .iter()
            .filter(|s| filter.is_empty() || filter == s.id)
        {
            if !current() {
                return Ok(empty(Notice::Stale));
            }
            match self.loader.read(source, cancelled.clone()) {
                Ok(entries) => {
                    for (order, entry) in entries.into_iter().enumerate() {
                        let fields = SearchFields::new(&entry.title, &[entry.target.text()], &[]);
                        let matched = fields.matched(&query);
                        if !query.is_empty() && matched.is_none() {
                            continue;
                        }
                        let id = format!(
                            "{}:{}:{:x}",
                            source.id,
                            epoch,
                            Sha256::digest(entry.key.as_bytes())
                        );
                        ranked.push((
                            matched,
                            source.id.clone(),
                            order,
                            Item {
                                id,
                                source_id: source.id.clone(),
                                entry,
                            },
                        ));
                    }
                }
                Err(_) => failed.push(source.id.clone()),
            }
        }
        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(a.1.cmp(&b.1))
                .then(b.3.entry.last_used_at.cmp(&a.3.entry.last_used_at))
                .then(a.2.cmp(&b.2))
        });
        let items: Vec<_> = ranked.into_iter().map(|x| x.3).collect();
        let mut state = self.state.lock().unwrap();
        if !current() {
            return Ok(empty(Notice::Stale));
        }
        state.items = items.clone();
        Ok(SearchResult {
            notice: (!failed.is_empty()).then_some(Notice::ReadFailed),
            failed_sources: failed,
            items,
        })
    }
    pub fn use_item(
        &self,
        id: &str,
        copy: bool,
        platform: &dyn anywhere_platform::Platform,
    ) -> Result<()> {
        let epoch = self.epoch.load(Ordering::Acquire);
        ensure!(
            self.enabled.load(Ordering::Acquire),
            "history feature disabled"
        );
        let (item, source) = {
            let state = self.state.lock().unwrap();
            let item = state
                .items
                .iter()
                .find(|i| i.id == id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("history selection expired"))?;
            let source = state
                .sources
                .iter()
                .find(|s| s.id == item.source_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("history source disabled"))?;
            (item, source)
        };
        let _worker = self
            .worker
            .try_lock()
            .map_err(|_| anyhow::anyhow!("history reader busy"))?;
        let cancelled = self.cancellation(epoch);
        let fresh = self.loader.read(&source, cancelled.clone())?;
        ensure!(
            !cancelled()
                && fresh
                    .iter()
                    .any(|e| e.key == item.entry.key && e.target == item.entry.target),
            "history target changed"
        );
        if copy {
            return platform.copy_text(&item.entry.target.text());
        }
        let Some(path) = item.entry.target.local_path() else {
            bail!("remote history targets are not supported yet");
        };
        ensure!(
            match item.entry.target {
                Target::Directory { .. } | Target::ObsidianVault { .. } => path.is_dir(),
                _ => path.is_file(),
            },
            "history target unavailable"
        );
        let descriptor = self
            .loader
            .descriptors()
            .into_iter()
            .find(|d| d.id == item.source_id)
            .ok_or_else(|| anyhow::anyhow!("source unavailable"))?;
        let apps = platform.installed_applications();
        let matches: Vec<_> = apps
            .applications
            .iter()
            .filter(|app| descriptor.applications.contains(&app.bundle_id))
            .collect();
        ensure!(
            matches.len() == 1 && !cancelled(),
            "source application unavailable or ambiguous"
        );
        let app = matches[0]
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("application path unavailable"))?;
        if let Target::ObsidianVault { vault_id, .. } = &item.entry.target {
            ensure!(
                matches[0].bundle_id == "md.obsidian",
                "vault target requires Obsidian"
            );
            let mut uri = url::Url::parse("obsidian://open")?;
            uri.query_pairs_mut().append_pair("vault", vault_id);
            return platform.open_url_in_application(std::path::Path::new(app), uri.as_str());
        }
        platform
            .open_file_in_application(std::path::Path::new(app), path)
            .map_err(|_| anyhow::anyhow!("history open could not be submitted"))
    }
}
