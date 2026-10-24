//! Source preferences, publication and execution share one revocable lifetime.
use super::model::*;
use crate::core::{
    search::query::{Query, SearchFields},
    storage::ExtensionStorage,
};
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    schema: u32,
    sources: Vec<SourcePreference>,
}
struct State {
    sources: Vec<SourcePreference>,
    items: Vec<Item>,
}
pub struct HistoryService {
    storage: Result<Arc<dyn ExtensionStorage>, String>,
    loader: Arc<dyn SourceLoader>,
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
            state: Mutex::new(State {
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
    pub fn refresh(&self) -> Result<()> {
        let _configuration = self.configuration.lock().unwrap();
        let bytes = self
            .storage
            .as_ref()
            .map_err(|_| anyhow::anyhow!("history preferences unavailable"))?
            .get("sources")?;
        let sources = match bytes {
            Some(bytes) => {
                let p: Preferences = serde_json::from_slice(&bytes)?;
                ensure!(p.schema == 1, "unsupported history preferences");
                self.validate_sources(&p.sources)?;
                p.sources
            }
            None => vec![],
        };
        let mut state = self.state.lock().unwrap();
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.sources = sources;
        state.items.clear();
        Ok(())
    }
    fn validate_sources(&self, sources: &[SourcePreference]) -> Result<()> {
        let descriptors = self.loader.descriptors();
        ensure!(
            sources.len() <= descriptors.len(),
            "too many history sources"
        );
        let mut ids = std::collections::BTreeSet::new();
        for source in sources {
            ensure!(
                ids.insert(&source.id) && descriptors.iter().any(|d| d.id == source.id),
                "unknown or duplicate source"
            );
            ensure!(
                source.path.is_absolute() && source.path.as_os_str().len() <= 16 * 1024,
                "invalid source path"
            );
        }
        Ok(())
    }
    pub fn sources(&self) -> Vec<(SourceDescriptor, Option<SourcePreference>)> {
        let state = self.state.lock().unwrap();
        self.loader
            .descriptors()
            .into_iter()
            .map(|d| {
                let p = state.sources.iter().find(|p| p.id == d.id).cloned();
                (d, p)
            })
            .collect()
    }
    pub fn page_input(&self, id: &str) -> Result<PageInput> {
        ensure!(
            self.enabled.load(Ordering::Acquire),
            "history feature disabled"
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
        let _configuration = self.configuration.lock().unwrap();
        let mut next = self.state.lock().unwrap().sources.clone();
        next.retain(|s| s.id != id);
        ensure!(
            self.loader.descriptors().iter().any(|d| d.id == id),
            "unknown source"
        );
        if let Some(path) = path.filter(|p| !p.trim().is_empty()) {
            let path = anywhere_platform::paths::expand_home(path.trim())
                .ok_or_else(|| anyhow::anyhow!("invalid source path"))?
                .canonicalize()?;
            ensure!(path.is_file(), "source must be a file");
            std::fs::File::open(&path)?;
            next.push(SourcePreference {
                id: id.into(),
                path,
            });
        }
        self.validate_sources(&next)?;
        let storage = self
            .storage
            .as_ref()
            .map_err(|_| anyhow::anyhow!("history preferences unavailable"))?;
        let updated = storage
            .update("sources", &mut |current| {
                let mut merged = match current {
                    Some(bytes) => {
                        let p: Preferences = serde_json::from_slice(bytes)?;
                        ensure!(p.schema == 1, "unsupported history preferences");
                        self.validate_sources(&p.sources)?;
                        p.sources
                    }
                    None => vec![],
                };
                merged.retain(|source| source.id != id);
                if let Some(source) = next.iter().find(|source| source.id == id) {
                    merged.push(source.clone());
                }
                self.validate_sources(&merged)?;
                Ok(Some(serde_json::to_vec(&Preferences {
                    schema: 1,
                    sources: merged,
                })?))
            })?
            .ok_or_else(|| anyhow::anyhow!("history preferences missing"))?;
        let next = serde_json::from_slice::<Preferences>(&updated)?.sources;
        let mut state = self.state.lock().unwrap();
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.sources = next;
        state.items.clear();
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
                Target::Directory { .. } => path.is_dir(),
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
        platform
            .open_file_in_application(std::path::Path::new(app), path)
            .map_err(|_| anyhow::anyhow!("history open could not be submitted"))
    }
}
