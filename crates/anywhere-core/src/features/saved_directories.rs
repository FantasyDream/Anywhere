//! Explicit directory bookmarks: bounded memory search and durable module-owned data.
use super::{BuiltinFeature, FeatureBindings};
use crate::features::{descriptor::FeatureDescriptor, services::FeatureServices};
mod repository;
pub(crate) mod service;
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
    },
    features::contribution::FeatureContribution,
};
use anyhow::{ensure, Context, Result};
use anywhere_platform::Platform;
use anywhere_protocol::{extension::ExtensionCapability as Capability, DIRECTORY};
use service::SavedDirectoriesService;
use std::sync::Arc;
pub const SAVE: &str = "saved_directories.save";
pub const OPEN: &str = "saved_directories.open";
pub const FORGET: &str = "saved_directories.forget";
const SAVED: &str = "saved.directory";

pub struct Feature;
pub const DESCRIPTOR: FeatureDescriptor = FeatureDescriptor::new(
    "saved_directories",
    "Saved Directories",
    &[Capability::Storage, Capability::OpenFile],
);
pub fn descriptor() -> &'static FeatureDescriptor {
    &DESCRIPTOR
}
enum Operation {
    Save,
    Open,
    Forget,
}
struct DirectoryAction {
    store: Arc<SavedDirectoriesService>,
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
                self.store.save(&canonical)?;
            }
            Operation::Forget => self.store.forget(path)?,
            Operation::Open => {
                let canonical = self.store.resolve_for_open(path)?;
                crate::features::host_services::for_feature(descriptor(), platform)?
                    .open_file(&canonical)?;
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
impl BuiltinFeature for Feature {
    fn descriptor(&self) -> &'static FeatureDescriptor {
        descriptor()
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new(self.descriptor().id, vec![], vec![])
    }
    fn bind_services(&self, services: &mut FeatureServices) -> Result<FeatureBindings> {
        let store = services.saved_directories();
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
            registry.register_owned(action.clone(), Some(descriptor().id))?;
        }
        Ok(FeatureBindings {
            actions,
            sources: crate::core::search::candidate_source::SourceSet {
                synchronous: vec![Arc::new(SavedDirectorySource {
                    store: store.clone(),
                    registry,
                })],
                ..Default::default()
            },
        })
    }
}
struct SavedDirectorySource {
    store: Arc<SavedDirectoriesService>,
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
    use crate::core::storage::ExtensionStorage;
    use crate::persistence::module_storage::ModuleStorageHost;
    use repository::{KEY, LIMIT};
    use std::{path::PathBuf, sync::Mutex};
    #[test]
    fn quota_failures_preserve_disk_and_cache_and_search_is_memory_only() {
        let storage = ModuleStorageHost::memory()
            .unwrap()
            .bind("saved_directories")
            .unwrap();
        let store = SavedDirectoriesService::open(storage.clone()).unwrap();
        for i in 0..LIMIT {
            store
                .save(&PathBuf::from(format!("/project_{i:03}")))
                .unwrap();
        }
        let before = storage.get(KEY).unwrap().unwrap();
        assert!(store.save(std::path::Path::new("/overflow")).is_err());
        assert_eq!(store.paths().unwrap().len(), LIMIT);
        assert_eq!(storage.get(KEY).unwrap().unwrap(), before);
        store.save(std::path::Path::new("/project_000")).unwrap(); // Saving twice is idempotent.
        store.forget(std::path::Path::new("/project_001")).unwrap();
        let before = storage.get(KEY).unwrap().unwrap();
        assert!(store
            .save(&PathBuf::from(format!(
                "/{}",
                "x".repeat(crate::core::storage::MAX_VALUE_BYTES)
            )))
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
                Ok(Some(serde_json::to_vec(&serde_json::json!({
                    "version": 1,
                    "paths": (0..LIMIT).map(|i| format!("/project_{i:03}")).collect::<Vec<_>>()
                }))?))
            }
            fn put(&self, _: &str, _: &[u8]) -> Result<()> {
                unreachable!()
            }
            fn remove(&self, _: &str) -> Result<()> {
                unreachable!()
            }
        }
        let mut services = FeatureServices::new(Ok(Arc::new(ReadOnce(
            std::sync::atomic::AtomicUsize::new(0),
        ))));
        let bindings = Feature.bind_services(&mut services).unwrap();
        services.saved_directories().refresh().unwrap();
        let source = &bindings.sources.synchronous[0];
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
            SavedDirectoriesService::open(Arc::new(SlowStorage {
                started,
                gate: gate.clone(),
            }))
            .unwrap(),
        );
        let write = std::thread::spawn({
            let store = store.clone();
            move || store.save(std::path::Path::new("/new"))
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
