//! File rows reuse existing Open File / Open Directory execution and Usage.
use crate::core::{
    action::descriptor::ActionRegistry,
    context::{ContextValue, ExecutionContext, ResolvedContext},
    ranking::score::{self, UsageStats},
    search::{
        candidate::{action_candidate_id, Candidate, CandidateTarget},
        candidate_source::AsyncCandidateSource,
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

pub struct FileSearchSource {
    platform: Arc<dyn Platform>,
    actions: Arc<ActionRegistry>,
    worker: Arc<Semaphore>,
}

struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub fn eligible_query(query: &str) -> bool {
    if query.len() > 256 {
        return false;
    }
    let query = query.trim();
    query.chars().count() >= 2
        && query.len() <= 256
        && !query.chars().any(char::is_control)
        && !query.contains('/')
        && !query.starts_with(['{', '['])
}

pub fn validate_target(context: &ResolvedContext) -> Result<()> {
    let path = context
        .value
        .path()
        .ok_or_else(|| anyhow::anyhow!("file target missing"))?;
    let canonical = path.canonicalize()?;
    let metadata = canonical.metadata()?;
    if canonical != path
        || (context.type_id == FILE && !metadata.is_file())
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
        }
    }
}

#[async_trait::async_trait]
impl AsyncCandidateSource for FileSearchSource {
    fn id(&self) -> &'static str {
        "files"
    }
    fn accepts(&self, query: &str) -> bool {
        eligible_query(query)
    }
    async fn candidates(&self, query: &str, execution: ExecutionContext) -> Result<Vec<Candidate>> {
        let permit = tokio::select! {
            permit = self.worker.clone().acquire_owned() => permit?,
            _ = execution.superseded() => return Ok(Vec::new()),
        };
        if !execution.is_current() {
            return Ok(Vec::new());
        }
        let platform = self.platform.clone();
        let text = query.to_owned();
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(cancelled.clone());
        let hits = tokio::task::spawn_blocking(move || {
            let _permit = permit; // A canceled waiter cannot release a running OS query.
            platform.search_files(&text, 64, &|| {
                cancelled.load(Ordering::Acquire) || !execution.is_current()
            })
        })
        .await??;
        Ok(file_candidates(hits, query, &self.actions))
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
        if matched.is_none() {
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
        let Some(action) = actions.get(id) else {
            continue;
        };
        let context = ResolvedContext::new(kind, value, ContextSource::UserInput, "file-search");
        candidates.push(Candidate {
            presentation: CandidatePresentation {
                id: action_candidate_id(id, &context),
                kind: CandidateKind::File,
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
                execution_context_id: None,
                execution_target_label: None,
                home_eligible: false,
                primary_action: true,
                section_label: Some("Files".into()),
                shortcut: None,
            },
            target: CandidateTarget::from_action(action.clone(), context),
        });
    }
    candidates.sort_by(crate::core::search::candidate::compare_candidates);
    candidates.truncate(8);
    candidates
}
