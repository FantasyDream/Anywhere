//! Explicit directory bookmarks: bounded memory search and durable module-owned data.
use super::{ExtensionRuntime, FirstPartyExtension};
use crate::{
    core::{
        action::{
            availability::AvailabilityRule,
            descriptor::{ActionDefinition, ActionExecutor, ActionRegistry},
            handler::{ActionHandler, ActionHandlerRef, ExecutionOutput},
            matcher::ActionMatcher,
        },
        context::{ContextSource, ContextValue, ResolvedContext},
        search::{
            candidate::Candidate,
            candidate_source::{CandidateSource, SearchContext},
            query::{Query, SearchFields},
        },
        storage::ExtensionStorage,
    },
    features::contribution::FeatureContribution,
};
use anyhow::{ensure, Context, Result};
use anywhere_platform::Platform;
use anywhere_protocol::{extension::FirstPartyManifest, DIRECTORY};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
pub const SAVE: &str = "saved_directories.save";
pub const OPEN: &str = "saved_directories.open";
pub const FORGET: &str = "saved_directories.forget";
const SAVED: &str = "saved.directory";
const LIMIT: usize = 100;
const KEY: &str = "directories";

pub struct Extension;
pub fn manifest() -> &'static FirstPartyManifest {
    static M: OnceLock<FirstPartyManifest> = OnceLock::new();
    M.get_or_init(|| {
        anywhere_plugin::extension::parse_first_party_manifest(include_str!(
            "../../../../extensions/saved_directories/manifest.toml"
        ))
        .expect("invalid saved-directories manifest")
    })
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u32,
    paths: Vec<PathBuf>,
}
struct DirectoryStore {
    storage: Arc<dyn ExtensionStorage>,
    saved: Mutex<Saved>,
    writes: Mutex<()>,
}
impl DirectoryStore {
    fn decode(raw: Option<&[u8]>) -> Result<Saved> {
        let saved: Saved = match raw {
            Some(raw) => serde_json::from_slice(raw)?,
            None => Saved {
                version: 1,
                paths: vec![],
            },
        };
        ensure!(
            saved.version == 1
                && saved.paths.len() <= LIMIT
                && saved.paths.iter().all(|p| p.is_absolute()),
            "invalid saved directories"
        );
        Ok(saved)
    }
    fn load(storage: Arc<dyn ExtensionStorage>) -> Result<Self> {
        let saved = Self::decode(storage.get(KEY)?.as_deref())?;
        Ok(Self {
            storage,
            saved: Mutex::new(saved),
            writes: Mutex::new(()),
        })
    }
    fn paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .saved
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))?
            .paths
            .clone())
    }
    fn change(&self, path: PathBuf, save: bool) -> Result<()> {
        let _write = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directory writes unavailable"))?;
        let committed = self.storage.update(KEY, &mut |raw| {
            let mut next = Self::decode(raw)?;
            next.paths.retain(|p| p != &path);
            if save {
                ensure!(
                    next.paths.len() < LIMIT,
                    "saved directory limit is 100; forget an existing directory first"
                );
                next.paths.push(path.clone());
                next.paths.sort();
            }
            Ok(Some(serde_json::to_vec(&next)?))
        })?;
        let next = Self::decode(committed.as_deref())?;
        *self
            .saved
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))? = next;
        Ok(())
    }
}
impl super::ExtensionState for DirectoryStore {
    fn refresh(&self) -> Result<()> {
        let _write = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directory writes unavailable"))?;
        let next = Self::decode(self.storage.get(KEY)?.as_deref())?;
        *self
            .saved
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))? = next;
        Ok(())
    }
}
enum Operation {
    Save,
    Open,
    Forget,
}
struct DirectoryAction {
    store: Arc<DirectoryStore>,
    operation: Operation,
}
impl ActionHandler for DirectoryAction {
    fn binding_policy(&self) -> crate::core::action::identity::BindingPolicy {
        use crate::core::action::identity::BindingPolicy;
        if matches!(self.operation, Operation::Save) {
            BindingPolicy::Persistent
        } else {
            BindingPolicy::Unavailable {
                reason: "Select a saved directory from global search.",
            }
        }
    }
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let path = context.value.path().context("directory required")?;
        match self.operation {
            Operation::Save => {
                let canonical = path.canonicalize()?;
                ensure!(canonical.is_dir(), "directory no longer exists");
                self.store.change(canonical, true)?;
            }
            Operation::Forget => self.store.change(path.into(), false)?,
            Operation::Open => {
                // Object execution, unlike matching, must revalidate against
                // changes committed by another application instance.
                super::ExtensionState::refresh(self.store.as_ref())?;
                ensure!(
                    self.store.paths()?.iter().any(|p| p == path),
                    "directory is no longer saved"
                );
                let canonical = path.canonicalize()?;
                ensure!(
                    canonical == path && canonical.is_dir(),
                    "saved directory changed or no longer exists"
                );
                super::services(manifest(), platform)?.open_file(&canonical)?;
            }
        }
        Ok(ExecutionOutput::dismiss())
    }
}
fn definition(id: &str, title: &str, accepts: &str, handler: DirectoryAction) -> ActionDefinition {
    ActionDefinition {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        shortcut: None,
        accepts: vec![anywhere_protocol::ContextMatcher {
            type_id: accepts.into(),
        }],
        executor: ActionExecutor::Native(ActionHandlerRef::owned(handler)),
        availability: AvailabilityRule::Enabled,
        base_priority: 0,
        icon: "folder".into(),
        group_id: "folder".into(),
        aliases: vec!["bookmark".into(), "收藏目录".into()],
        keywords: vec!["directory".into(), "save".into()],
        owning_app: None,
        menu_path: vec![],
        home_eligible: false,
        primary_action: false,
        search_title: None,
    }
}
impl FirstPartyExtension for Extension {
    fn manifest(&self) -> &'static FirstPartyManifest {
        manifest()
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new("saved_directories", vec![], vec![])
    }
    fn runtime(&self, storage: Option<Arc<dyn ExtensionStorage>>) -> Result<ExtensionRuntime> {
        self.runtime_with_state(storage, None)
    }
    fn runtime_with_state(
        &self,
        storage: Option<Arc<dyn ExtensionStorage>>,
        state: Option<Arc<dyn super::ExtensionState>>,
    ) -> Result<ExtensionRuntime> {
        let store = if let Some(state) = state {
            state.refresh()?;
            let state: Arc<dyn std::any::Any + Send + Sync> = state;
            state
                .downcast::<DirectoryStore>()
                .map_err(|_| anyhow::anyhow!("invalid saved directory state"))?
        } else {
            Arc::new(DirectoryStore::load(
                storage.context("saved directories storage not granted")?,
            )?)
        };
        let actions = vec![
            definition(
                SAVE,
                "Save Directory",
                DIRECTORY,
                DirectoryAction {
                    store: store.clone(),
                    operation: Operation::Save,
                },
            ),
            definition(
                OPEN,
                "Open Saved Directory",
                SAVED,
                DirectoryAction {
                    store: store.clone(),
                    operation: Operation::Open,
                },
            ),
            definition(
                FORGET,
                "Forget Saved Directory",
                SAVED,
                DirectoryAction {
                    store: store.clone(),
                    operation: Operation::Forget,
                },
            ),
        ];
        let mut registry = ActionRegistry::default();
        for action in &actions {
            registry.register(action.clone())?;
        }
        Ok(ExtensionRuntime {
            actions,
            sources: crate::core::search::candidate_source::SourceSet {
                synchronous: vec![Arc::new(SavedDirectorySource {
                    store: store.clone(),
                    registry,
                })],
                ..Default::default()
            },
            state: Some(store),
        })
    }
}
struct SavedDirectorySource {
    store: Arc<DirectoryStore>,
    registry: ActionRegistry,
}
impl CandidateSource for SavedDirectorySource {
    fn id(&self) -> &'static str {
        "saved_directories"
    }
    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        if search.scope_id.is_some() || search.query.is_empty() {
            return vec![];
        }
        let query = search.query;
        let ranking = search.ranking;
        let matcher = ActionMatcher::new(
            &self.registry,
            ranking.usage,
            ranking.facts,
            ranking.now,
            |_| 0,
        );
        let mut results = vec![];
        for path in self.store.paths().unwrap_or_default() {
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy();
            let level = SearchFields::new(
                &name,
                &[path.to_string_lossy().into_owned()],
                &["saved directory".into(), "bookmark".into()],
            )
            .matched(query);
            if !query.is_empty() && level.is_none() {
                continue;
            }
            let context = ResolvedContext::new(
                SAVED,
                ContextValue::Directory(path.clone()),
                ContextSource::UserInput,
                "saved_directories",
            );
            for mut candidate in matcher
                .match_actions(&Query::parse(""), &[context])
                .into_iter()
                .filter(|c| c.presentation.command_id.as_deref() == Some(OPEN))
            {
                candidate.presentation.title = name.to_string();
                candidate.presentation.subtitle =
                    Some(format!("Saved directory · {}", path.display()));
                candidate.presentation.score = crate::core::ranking::score::score(
                    level,
                    true,
                    ContextSource::UserInput,
                    0,
                    0,
                    (ranking.usage)(OPEN, SAVED),
                    ranking.now,
                );
                candidate.presentation.primary_action = true;
                candidate.presentation.section_label = Some("Saved Directories".into());
                results.push(candidate);
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::module_storage::ModuleStorageHost;
    #[test]
    fn quota_failures_preserve_disk_and_cache_and_search_is_memory_only() {
        let storage = ModuleStorageHost::memory()
            .unwrap()
            .bind("saved_directories")
            .unwrap();
        let store = DirectoryStore::load(storage.clone()).unwrap();
        for i in 0..LIMIT {
            store
                .change(PathBuf::from(format!("/project_{i:03}")), true)
                .unwrap();
        }
        let before = storage.get(KEY).unwrap().unwrap();
        assert!(store.change("/overflow".into(), true).is_err());
        assert_eq!(store.paths().unwrap().len(), LIMIT);
        assert_eq!(storage.get(KEY).unwrap().unwrap(), before);
        store.change("/project_000".into(), true).unwrap(); // Saving twice is idempotent.
        store.change("/project_001".into(), false).unwrap();
        let before = storage.get(KEY).unwrap().unwrap();
        assert!(store
            .change(
                PathBuf::from(format!(
                    "/{}",
                    "x".repeat(crate::core::storage::MAX_VALUE_BYTES)
                )),
                true
            )
            .is_err());
        assert_eq!(storage.get(KEY).unwrap().unwrap(), before);
        assert_eq!(store.paths().unwrap().len(), LIMIT - 1);
    }
    #[test]
    fn bounded_source_does_not_read_storage_in_the_query_path() {
        struct ReadOnce(std::sync::atomic::AtomicUsize);
        impl ExtensionStorage for ReadOnce {
            fn update(
                &self,
                _: &str,
                _: &mut crate::core::storage::StorageUpdate<'_>,
            ) -> Result<Option<Vec<u8>>> {
                unreachable!()
            }
            fn get(&self, _: &str) -> Result<Option<Vec<u8>>> {
                assert_eq!(self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed), 0);
                Ok(Some(serde_json::to_vec(&Saved {
                    version: 1,
                    paths: (0..LIMIT)
                        .map(|i| PathBuf::from(format!("/project_{i:03}")))
                        .collect(),
                })?))
            }
            fn put(&self, _: &str, _: &[u8]) -> Result<()> {
                unreachable!()
            }
            fn remove(&self, _: &str) -> Result<()> {
                unreachable!()
            }
        }
        let runtime = Extension
            .runtime(Some(Arc::new(ReadOnce(
                std::sync::atomic::AtomicUsize::new(0),
            ))))
            .unwrap();
        let source = &runtime.sources.synchronous[0];
        let query = Query::parse("project");
        let start = std::time::Instant::now();
        for _ in 0..100 {
            assert_eq!(
                source
                    .candidates(&SearchContext {
                        query: &query,
                        contexts: &[],
                        scope_id: None,
                        ranking: Default::default()
                    })
                    .len(),
                LIMIT
            );
        }
        eprintln!(
            "saved directories: 100 queries / 100 rows in {:?}",
            start.elapsed()
        );
        assert!(source
            .candidates(&SearchContext {
                query: &Query::parse(""),
                contexts: &[],
                scope_id: None,
                ranking: Default::default()
            })
            .is_empty());
        assert!(source
            .candidates(&SearchContext {
                query: &query,
                contexts: &[],
                scope_id: Some("app:any"),
                ranking: Default::default()
            })
            .is_empty());
    }

    #[test]
    fn a_slow_storage_write_does_not_block_query_reads() {
        struct SlowStorage {
            started: std::sync::mpsc::Sender<()>,
            gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
        }
        impl ExtensionStorage for SlowStorage {
            fn update(
                &self,
                key: &str,
                change: &mut crate::core::storage::StorageUpdate<'_>,
            ) -> Result<Option<Vec<u8>>> {
                let value = change(None)?;
                self.put(key, value.as_deref().unwrap())?;
                Ok(value)
            }
            fn get(&self, _: &str) -> Result<Option<Vec<u8>>> {
                Ok(None)
            }
            fn put(&self, _: &str, _: &[u8]) -> Result<()> {
                self.started.send(()).unwrap();
                let (lock, wake) = &*self.gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = wake.wait(released).unwrap();
                }
                Ok(())
            }
            fn remove(&self, _: &str) -> Result<()> {
                unreachable!()
            }
        }
        let (started, entered) = std::sync::mpsc::channel();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let store = Arc::new(
            DirectoryStore::load(Arc::new(SlowStorage {
                started,
                gate: gate.clone(),
            }))
            .unwrap(),
        );
        let write = std::thread::spawn({
            let store = store.clone();
            move || store.change("/new".into(), true)
        });
        entered
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let read = std::thread::spawn({
            let store = store.clone();
            move || sent.send(store.paths().unwrap()).unwrap()
        });
        let previous = received.recv_timeout(std::time::Duration::from_secs(1));
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        write.join().unwrap().unwrap();
        read.join().unwrap();
        assert!(previous.unwrap().is_empty());
        assert_eq!(store.paths().unwrap(), [PathBuf::from("/new")]);
    }
}
