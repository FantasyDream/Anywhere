//! File rows reuse existing Open File / Open Directory execution and Usage.
use crate::core::{
    action::descriptor::ActionRegistry,
    context::{ContextValue, ExecutionContext, ResolvedContext},
    ranking::score::{self, UsageStats},
    search::{
        candidate::{Candidate, CandidateTarget},
        candidate_source::{AsyncCandidateBatch, AsyncCandidateSource, AsyncSearchRequest},
        query::{Query, SearchFields},
    },
};
use anyhow::Result;
use anywhere_platform::{FileSearchHit, Platform};
use anywhere_protocol::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Default)]
pub struct FileSearchRequest {
    pub filter: FileSearchFilter,
}

pub struct FileSearchSource {
    platform: Arc<dyn Platform>,
    actions: Arc<ActionRegistry>,
    worker: Arc<Semaphore>,
    index_worker: Arc<Semaphore>,
    page_worker: Arc<Semaphore>,
    preparation_worker: Arc<Semaphore>,
    active: Arc<std::sync::Mutex<Option<Arc<super::scan::ScanStore>>>>,
    pub policy: Arc<std::sync::RwLock<FileSearchPolicy>>,
}

#[derive(Debug, Clone)]
pub struct FileSearchPolicy {
    pub enabled: bool,
    pub generation: u64,
    pub options: anywhere_platform::FileSearchOptions,
}
impl Default for FileSearchPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            generation: 0,
            options: anywhere_platform::FileSearchOptions {
                include_hidden: true,
                ..Default::default()
            },
        }
    }
}

struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub fn eligible_query(query: &str) -> bool {
    if query.len() > 4096 {
        return false;
    }
    let query = query.trim();
    !query.is_empty()
        && (query.len() <= 256 || std::path::Path::new(query).components().count() > 1)
        && !query.chars().any(char::is_control)
}

/// Execution ownership is domain data, independent of the Host's object icon.
pub fn is_search_target(candidate: &Candidate) -> bool {
    candidate.context().is_some_and(|c| {
        c.provider_id == "file-search" || c.provider_id.starts_with("file-search-link:")
    })
}

pub fn validate_target(context: &ResolvedContext) -> Result<()> {
    let path = context
        .value
        .path()
        .ok_or_else(|| anyhow::anyhow!("file target missing"))?;
    let canonical = path.canonicalize()?;
    let metadata = canonical.metadata()?;
    let link = path.symlink_metadata()?.file_type().is_symlink();
    let known_link = link
        && context.provider_id
            == format!(
                "file-search-link:{:016x}",
                anywhere_platform::search_storage::path_fingerprint(&canonical)
            );
    if (canonical != path && !known_link)
        || (context.type_id == FILE && !known_link && !metadata.is_file())
        || (context.type_id == DIRECTORY && !metadata.is_dir())
    {
        anyhow::bail!("file search target changed; search again");
    }
    Ok(())
}
impl FileSearchSource {
    pub fn new(platform: Arc<dyn Platform>, actions: Arc<ActionRegistry>) -> Self {
        Self {
            platform,
            actions,
            worker: Arc::new(Semaphore::new(1)),
            index_worker: Arc::new(Semaphore::new(1)),
            page_worker: Arc::new(Semaphore::new(1)),
            preparation_worker: Arc::new(Semaphore::new(1)),
            active: Arc::new(std::sync::Mutex::new(None)),
            policy: Arc::new(std::sync::RwLock::new(FileSearchPolicy::default())),
        }
    }
    pub fn clear(&self) {
        if let Some(store) = self.active.lock().unwrap().take() {
            store.cancel();
        }
    }
    pub async fn more(
        &self,
        query_id: u64,
        displayed: std::collections::HashSet<std::path::PathBuf>,
        execution: ExecutionContext,
    ) -> Result<AsyncCandidateBatch> {
        let store = self
            .active
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("search expired"))?;
        if store.query_id != query_id {
            anyhow::bail!("search expired");
        }
        let permit = tokio::select! { p = self.page_worker.clone().acquire_owned() => p?, _ = execution.superseded() => anyhow::bail!("search expired"), };
        let actions = self.actions.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let page = store.more(&displayed, &|| !execution.is_current())?;
            Ok(scan_batch(page, &actions))
        })
        .await?
    }
}

#[async_trait::async_trait]
impl AsyncCandidateSource for FileSearchSource {
    fn reset(&self) {
        self.clear();
    }
    fn accepts_mode(&self, mode: InputMode) -> bool {
        mode == InputMode::FileSearch
    }
    fn id(&self) -> &'static str {
        "files"
    }
    fn accepts(&self, query: &str) -> bool {
        eligible_query(query) && self.policy.read().is_ok_and(|policy| policy.enabled)
    }
    fn debounce(&self) -> std::time::Duration {
        std::time::Duration::from_millis(40)
    }
    fn timeout(&self) -> Option<std::time::Duration> {
        None
    }
    async fn candidates(&self, query: &str, execution: ExecutionContext) -> Result<Vec<Candidate>> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let request = AsyncSearchRequest {
            text: query.into(),
            options: Some(Arc::new(FileSearchRequest::default())),
        };
        let stream = self.stream(request, execution, tx);
        tokio::pin!(stream);
        let mut rows = Vec::new();
        loop {
            tokio::select! {
                batch = rx.recv() => if let Some(batch) = batch { rows = batch.candidates; } else { break; },
                result = &mut stream => { result?; while let Ok(batch) = rx.try_recv() { rows = batch.candidates; } break; },
            }
        }
        Ok(rows)
    }
    async fn stream(
        &self,
        request: AsyncSearchRequest,
        execution: ExecutionContext,
        sender: tokio::sync::mpsc::Sender<AsyncCandidateBatch>,
    ) -> Result<()> {
        let policy = self
            .policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .clone();
        if !policy.enabled {
            return Ok(());
        }
        let permit = tokio::select! { p = self.preparation_worker.clone().acquire_owned() => p?, _ = execution.superseded() => return Ok(()), };
        let preparation = request.clone();
        let options = policy.options.clone();
        let store = Arc::new(
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                super::scan::ScanStore::new(
                    &preparation.text,
                    preparation
                        .options::<FileSearchRequest>()
                        .copied()
                        .unwrap_or_default()
                        .filter,
                    options,
                )
            })
            .await??,
        );
        {
            let guard = self
                .policy
                .read()
                .map_err(|_| anyhow::anyhow!("search settings unavailable"))?;
            let mut active = self.active.lock().unwrap();
            if !execution.is_current() || guard.generation != policy.generation || !guard.enabled {
                return Ok(());
            }
            if let Some(old) = active.replace(store.clone()) {
                old.cancel();
            }
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(cancelled.clone());
        let current = {
            let execution = execution.clone();
            let policy_lock = self.policy.clone();
            let store = store.clone();
            let cancelled = cancelled.clone();
            Arc::new(move || {
                cancelled.load(Ordering::Acquire)
                    || store.cancelled.load(Ordering::Acquire)
                    || !execution.is_current()
                    || !policy_lock
                        .read()
                        .is_ok_and(|p| p.enabled && p.generation == policy.generation)
            })
        };
        let index = async {
            let permit = tokio::select! { p = self.index_worker.clone().acquire_owned() => p?, _ = execution.superseded() => return Ok::<_, anyhow::Error>(()), };
            if current() {
                return Ok(());
            }
            let platform = self.platform.clone();
            let query = request.text.clone();
            let check = current.clone();
            let mut options = policy.options.clone();
            options.engine = anywhere_platform::FileSearchEngine::SystemIndex;
            let index_store = store.clone();
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                if index_store.is_path_query() {
                    return Ok(Vec::new());
                }
                let hits = platform.search_files_with_options(&query, 64, &options, &*check)?;
                Ok::<_, anyhow::Error>(index_store.verify_index_hits(hits, &*check))
            })
            .await?;
            // A late index batch must never replace confirmed traversal results.
            let progress = store.progress();
            if !current() && progress.scanned_entries == 0 && !progress.finished {
                if let Ok(hits) = result {
                    let _ = sender
                        .send(scan_batch(
                            super::scan::ScanPage { hits, progress },
                            &self.actions,
                        ))
                        .await;
                }
            }
            Ok(())
        };
        let scan = async {
            tokio::select! { _ = tokio::time::sleep(std::time::Duration::from_millis(110)) => {}, _ = execution.superseded() => return Ok::<_, anyhow::Error>(()), }
            let permit = tokio::select! { p = self.worker.clone().acquire_owned() => p?, _ = execution.superseded() => return Ok(()), };
            if current() {
                return Ok(());
            }
            let store = store.clone();
            let check = current.clone();
            let tx = sender.clone();
            let actions = self.actions.clone();
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let result = store.run(&*check, |page| {
                    if check() {
                        anyhow::bail!("search cancelled");
                    }
                    tx.blocking_send(scan_batch(page, &actions))
                        .map_err(|_| anyhow::anyhow!("search receiver closed"))
                });
                if let Err(error) = result {
                    if !check() {
                        store.fail(&error);
                        match store.page(None, &*check) {
                            Ok(page) => {
                                let _ = tx.blocking_send(scan_batch(page, &actions));
                            }
                            Err(error) => {
                                store.fail(&error);
                                let _ = tx.blocking_send(AsyncCandidateBatch {
                                    update:
                                        crate::core::search::candidate_source::BatchUpdate::Replace,
                                    candidates: Vec::new(),
                                    state: SearchSourceState::Partial,
                                    metadata: Some(Arc::new(store.progress())),
                                });
                            }
                        }
                    }
                }
            })
            .await?;
            Ok(())
        };
        let (_, result) = tokio::join!(index, scan);
        result
    }
}

fn scan_batch(page: super::scan::ScanPage, actions: &ActionRegistry) -> AsyncCandidateBatch {
    // The walker owns stable page order; matching and filtering have already happened.
    let candidates = page
        .hits
        .iter()
        .flat_map(|hit| {
            let mut rows = file_candidates(vec![hit.hit()], "", actions);
            for candidate in &mut rows {
                candidate.presentation.score = f32::from(hit.score);
            }
            rows
        })
        .collect();
    AsyncCandidateBatch {
        update: crate::core::search::candidate_source::BatchUpdate::Replace,
        candidates,
        state: if !page.progress.finished {
            SearchSourceState::Loading
        } else if page.progress.issue_count > 0 {
            SearchSourceState::Partial
        } else {
            SearchSourceState::Ready
        },
        metadata: Some(Arc::new(page.progress)),
    }
}

pub fn file_candidates(
    hits: Vec<FileSearchHit>,
    text: &str,
    actions: &ActionRegistry,
) -> Vec<Candidate> {
    let query = Query::parse(text);
    let mut seen = std::collections::BTreeSet::new();
    let mut candidates = Vec::new();
    for hit in hits.into_iter().take(64) {
        if !seen.insert(hit.path.clone()) {
            continue;
        }
        let Some(name) = hit.path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        let matched = SearchFields::new(name, &[], &[]).matched(&query);
        if !text.is_empty() && matched.is_none() {
            continue;
        }
        let (id, kind, value) = if hit.is_directory {
            (
                "core.directory.open",
                DIRECTORY,
                ContextValue::Directory(hit.path.clone()),
            )
        } else {
            ("core.file.open", FILE, ContextValue::File(hit.path.clone()))
        };
        let Some(action) = actions.get_shared(id) else {
            continue;
        };
        let mut context =
            ResolvedContext::new(kind, value, ContextSource::UserInput, "file-search");
        let fingerprint = anywhere_platform::search_storage::path_fingerprint(&hit.path);
        context.id = format!("file-target:{fingerprint:016x}");
        if hit
            .path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            if let Ok(target) = hit.path.canonicalize() {
                context.provider_id = format!(
                    "file-search-link:{:016x}",
                    anywhere_platform::search_storage::path_fingerprint(&target)
                );
            }
        }
        candidates.push(Candidate {
            presentation: CandidatePresentation {
                localized_titles: Default::default(),
                id: format!("action:{id}:{kind}:{fingerprint:016x}"),
                kind: CandidateKind::File,
                is_menu: false,
                file_kind: Some(if hit.is_directory {
                    FileObjectKind::Directory
                } else {
                    FileObjectKind::File
                }),
                title: name.to_owned(),
                subtitle: hit.path.parent().map(|v| v.to_string_lossy().into_owned()),
                icon: IconDescriptor {
                    key: if hit.is_directory { "folder" } else { "file" }.into(),
                },
                score: score::score(
                    matched,
                    true,
                    ContextSource::ForegroundApplication,
                    0,
                    0,
                    UsageStats::default(),
                    0,
                ),
                group_id: "files".into(),
                availability: Availability::enabled(),
                command_id: Some(id.into()),
                bundle_id: None,
                owning_app: None,
                menu_path: Vec::new(),
                context_keys: Vec::new(),
                execution_context_id: Some(context.id.clone()),
                execution_target_label: None,
                home_eligible: false,
                primary_action: true,
                section_label: Some("Files".into()),
                shortcut: None,
            },
            target: CandidateTarget::from_shared_action(action, context),
        });
    }
    if !text.is_empty() {
        candidates.sort_by(crate::core::search::candidate::compare_candidates);
    }
    candidates.truncate(64);
    candidates
}

/// Common file operations for the selected search object. These are execution
/// targets, not Context graph nodes or additional ranked search results.
pub fn result_actions(
    candidates: &[Candidate],
    search: &crate::core::search::engine::ActionSearch<'_>,
) -> Vec<Candidate> {
    // Result-object actions never become activation Contexts or scopes. Each one
    // retains the exact resolved target and uses the normal Registry/availability.
    let mut result_actions = Vec::new();
    for file in candidates
        .iter()
        .filter(|c| c.presentation.kind == CandidateKind::File)
    {
        let Some(context) = file.context() else {
            continue;
        };
        let offers = search.search(&Query::parse(""), std::slice::from_ref(context), &[]);
        for mut offer in offers.candidates {
            if !matches!(
                offer.presentation.command_id.as_deref(),
                Some(
                    "core.file.open"
                        | "core.file.reveal"
                        | "core.file.copy"
                        | "core.directory.open"
                        | "core.directory.copy"
                        | "core.directory.terminal"
                )
            ) {
                continue;
            }
            if let (Some(command), Some(path)) =
                (&offer.presentation.command_id, context.value.path())
            {
                let fingerprint = anywhere_platform::search_storage::path_fingerprint(path);
                offer.presentation.id = format!(
                    "file-action:{command}:{}:{fingerprint:016x}",
                    context.type_id
                );
            }
            offer.presentation.kind = CandidateKind::Action;
            offer.presentation.file_kind = file.presentation.file_kind;
            offer.presentation.group_id = "files".into();
            offer.presentation.execution_context_id = Some(context.id.clone());
            offer.presentation.context_keys.clear();
            offer.presentation.home_eligible = false;
            offer.presentation.shortcut = None;
            result_actions.push(offer);
        }
    }
    result_actions
}
