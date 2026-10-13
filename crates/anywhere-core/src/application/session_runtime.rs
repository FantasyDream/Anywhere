//! Resolution engine: context stepping, budget control, snapshot publication.
//!
//! Deliberately separate from the [`Application`](crate::application::launcher_service::Application)
//! facade so a background completion task can own a clone and publish late context
//! without borrowing the facade.
use super::session_model::LauncherSession;
use crate::{
    core::action::descriptor::ActionRegistry,
    core::context::engine::{stamp, ContextGraph, ContextGraphRun, GraphStep},
    core::context::provider::ContextProvider,
    core::context::registry::ContextProviderRegistry,
    core::context::{ExecutionContext, ResolvedContext},
    core::ranking::usage::UsageRepository,
    core::search::candidate_source::CandidateSource,
    features::applications::index::ApplicationCatalog,
    features::filesystem::provider::DirectoryMemo,
};
use anyhow::{anyhow, Result};
use anywhere_protocol::*;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{broadcast, watch};

/// Cloneable engine shared by the facade and its background completion tasks.
#[derive(Clone)]
pub struct Resolver {
    pub(super) registry: Arc<Mutex<Arc<ActionRegistry>>>,
    /// Stateless providers; the directory provider is added per revision.
    pub(super) providers: ContextProviderRegistry,
    pub(super) sources: Vec<Arc<dyn CandidateSource + Send + Sync>>,
    pub(super) async_sources:
        Vec<Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>>,
    pub(super) source_workers: Arc<tokio::sync::Semaphore>,
    pub(super) session: Arc<Mutex<LauncherSession>>,
    pub(super) current: Arc<AtomicU64>,
    /// Definition/authorization lifetime, independent of query and menu discovery.
    pub(super) action_catalog_version: Arc<AtomicU64>,
    pub(super) execution_epoch: Arc<AtomicU64>,
    pub(super) events: broadcast::Sender<LauncherEvent>,
    pub(super) latest_tx: watch::Sender<Option<Arc<LauncherState>>>,
    pub(super) catalog: Arc<Mutex<CatalogSlot>>,
    pub(super) catalog_rx: watch::Receiver<u64>,
    pub(super) catalog_issued: Arc<AtomicU64>,
    pub(super) shortcut_catalog: Arc<Mutex<Vec<super::shortcuts::ShortcutAction>>>,
    pub(super) history: Arc<Mutex<Box<dyn UsageRepository>>>,
    directory: Arc<Mutex<Option<Arc<DirectoryMemo>>>>,
}

/// Published application index. The generation check and the write share this lock.
pub(super) struct CatalogSlot {
    pub(super) generation: u64,
    /// Unfiltered scan. Exclusions are applied into `catalog`.
    pub(super) full: Arc<ApplicationCatalog>,
    pub(super) catalog: Arc<ApplicationCatalog>,
}
pub struct Resolution {
    pub contexts: Vec<ResolvedContext>,
    pub diagnostics: Vec<String>,
    /// False when the caller's budget cut the pass short.
    pub complete: bool,
    pub run: ContextGraphRun,
}

impl Resolver {
    pub fn new(registry: ActionRegistry, history: Box<dyn UsageRepository>) -> Self {
        let (latest_tx, _) = watch::channel(None);
        let (_, catalog_rx) = watch::channel(0);
        Self {
            registry: Arc::new(Mutex::new(Arc::new(registry))),
            // Registration order does not matter: the registry sorts by priority, then id.
            providers: super::composition::builtin_providers(),
            sources: Vec::new(),
            async_sources: Vec::new(),
            source_workers: Arc::new(tokio::sync::Semaphore::new(
                anywhere_protocol::policy::MAX_CONCURRENT_SEARCH_SOURCES as usize,
            )),
            session: Arc::new(Mutex::new(LauncherSession::default())),
            current: Arc::new(AtomicU64::new(0)),
            action_catalog_version: Arc::new(AtomicU64::new(1)),
            execution_epoch: Arc::new(AtomicU64::new(0)),
            events: broadcast::channel(256).0,
            latest_tx,
            catalog: Arc::new(Mutex::new(CatalogSlot {
                generation: 0,
                full: Arc::new(ApplicationCatalog::empty()),
                catalog: Arc::new(ApplicationCatalog::empty()),
            })),
            catalog_rx,
            catalog_issued: Arc::new(AtomicU64::new(0)),
            shortcut_catalog: Arc::new(Mutex::new(Vec::new())),
            history: Arc::new(Mutex::new(history)),
            directory: Arc::new(Mutex::new(None)),
        }
    }

    pub fn base_registry(&self) -> Arc<ActionRegistry> {
        self.registry
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    pub fn replace_base_registry(&self, registry: ActionRegistry) {
        if let Ok(mut slot) = self.registry.lock() {
            *slot = Arc::new(registry);
        }
    }

    pub(super) fn with_catalog_rx(mut self, catalog_rx: watch::Receiver<u64>) -> Self {
        self.catalog_rx = catalog_rx;
        self
    }

    pub(super) fn set_directory(&self, directory: Option<Arc<DirectoryMemo>>) {
        if let Ok(mut slot) = self.directory.lock() {
            *slot = directory;
        }
    }

    pub(super) fn clear_directory(&self) {
        self.set_directory(None);
    }

    pub(super) fn mark_current(&self, session_id: SessionId, revision: Revision) {
        self.current
            .store(stamp(session_id, revision), Ordering::Release);
    }

    pub(super) fn reset_session(&self, session_id: SessionId) {
        self.clear_directory();
        self.mark_current(session_id, 0);
        self.latest_tx.send_replace(None);
    }

    pub(super) fn abandon_session(&self) {
        self.current.store(0, Ordering::Release);
        self.clear_directory();
        self.latest_tx.send_replace(None);
    }

    pub(super) fn is_abandoned(&self) -> bool {
        self.current.load(Ordering::Acquire) == 0
    }

    pub fn register_provider(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        self.providers.register(provider)
    }

    pub fn execution(&self, session_id: SessionId, revision: Revision) -> ExecutionContext {
        ExecutionContext {
            session_id,
            revision,
            current: self.current.clone(),
            events: self.events.clone(),
        }
    }

    pub fn is_current(&self, session_id: SessionId, revision: Revision) -> bool {
        self.current.load(Ordering::Acquire) == stamp(session_id, revision)
    }

    pub fn is_session_active(&self, session_id: SessionId) -> bool {
        let current = self.current.load(Ordering::Acquire);
        current != 0 && (current >> 32) == u64::from(session_id)
    }

    /// Wait until the latest issued catalog scan has published.
    ///
    /// A receiver that subscribed after an earlier scan would otherwise return
    /// immediately and miss a scan started by the current activation.
    pub async fn catalog_ready(&self) {
        let expected = self.catalog_issued.load(Ordering::Acquire);
        let mut rx = self.catalog_rx.clone();
        while *rx.borrow_and_update() < expected {
            if tokio::time::timeout(Duration::from_secs(10), rx.changed())
                .await
                .is_err()
            {
                return;
            }
        }
    }

    fn catalog_view(&self) -> Result<(Arc<ApplicationCatalog>, u64)> {
        self.catalog
            .lock()
            .map(|guard| (guard.catalog.clone(), guard.generation))
            .map_err(|_| anyhow!("catalog lock poisoned"))
    }

    /// Revision currently published for the live session, if any.
    pub fn committed_revision(&self) -> Option<Revision> {
        self.session.lock().ok()?.resolution.committed_revision
    }

    /// Rebuild the published snapshot for the revision that is already committed.
    ///
    /// Used when a source the matcher reads — the application index — arrives after the
    /// first snapshot was built. The contexts are the ones that revision already
    /// published; matching them again publishes a new `snapshot_seq`, which is what the
    /// host's late-update path observes. Returns the new snapshot, or `None` when no
    /// revision is committed or it is no longer current.
    pub fn recompute_current(&self, session_id: SessionId) -> Option<LauncherState> {
        let (revision, contexts, complete) = {
            let session = self.session.lock().ok()?;
            if session.state.session_id != session_id {
                return None;
            }
            let revision = session.resolution.committed_revision?;
            if !self.is_current(session_id, revision) {
                return None;
            }
            (
                revision,
                session.resolution.committed_contexts.clone(),
                session.state.context_complete,
            )
        };
        self.commit(
            session_id,
            revision,
            contexts,
            Vec::new(),
            complete,
            Instant::now(),
        )
        .ok()
        .flatten()
    }

    /// Providers for one revision: stateless built-ins plus the directory memo.
    fn providers_for(&self, _session_id: SessionId) -> Vec<Arc<dyn ContextProvider>> {
        let directory = self
            .directory
            .lock()
            .ok()
            .and_then(|directory| directory.clone());
        super::composition::session_providers(self.providers.clone(), directory)
    }

    pub fn build_run(
        &self,
        execution: &ExecutionContext,
        seeds: Vec<ResolvedContext>,
    ) -> ContextGraphRun {
        ContextGraph::start(seeds, execution)
    }

    /// Resolve with no time limit: the late pass after a first snapshot.
    pub async fn resolve_until(
        &self,
        session_id: SessionId,
        run: ContextGraphRun,
        started: Instant,
    ) -> Option<Resolution> {
        self.resolve_inner(session_id, run, None, None, started)
            .await
    }

    /// Resolve the first snapshot under a caller-controlled budget.
    ///
    /// When the budget expires while a provider is still in flight, the wait is
    /// extended by `grace`. The graph is sequential, so at most one provider is
    /// ever pending, and a budget that lands mid-call would otherwise publish a
    /// half-finished list: the real machine committed a 3-action snapshot 0ms
    /// before the Git provider ran and would have added 3 more.
    pub async fn resolve_until_with_grace(
        &self,
        session_id: SessionId,
        run: ContextGraphRun,
        budget: Duration,
        grace: Duration,
        started: Instant,
    ) -> Option<Resolution> {
        self.resolve_inner(session_id, run, Some(budget), Some(grace), started)
            .await
    }

    async fn resolve_inner(
        &self,
        session_id: SessionId,
        mut run: ContextGraphRun,
        budget: Option<Duration>,
        grace: Option<Duration>,
        started: Instant,
    ) -> Option<Resolution> {
        let budget_deadline = budget.map(|budget| started + budget);
        let grace_deadline =
            budget_deadline.and_then(|deadline| grace.map(|grace| deadline + grace));
        let revision = run.execution.revision;
        loop {
            if !self.is_current(session_id, revision) {
                tracing::info!(
                    session_id,
                    revision,
                    duration_us = started.elapsed().as_micros() as u64,
                    "context_resolution_abandoned"
                );
                return None;
            }
            let providers = self.providers_for(session_id);
            match run.step(&providers) {
                GraphStep::Done {
                    contexts,
                    diagnostics,
                } => {
                    tracing::info!(
                        session_id,
                        revision,
                        duration_us = started.elapsed().as_micros() as u64,
                        complete = true,
                        "context_resolution_total"
                    );
                    return Some(Resolution {
                        contexts,
                        diagnostics,
                        complete: true,
                        run,
                    });
                }
                GraphStep::Pending => {
                    // Wait on the detached call the run already started: a cut leaves it
                    // running, and the late pass collects this same result.
                    let Some(outcome) = run
                        .await_pending(budget_deadline, grace_deadline, started)
                        .await
                    else {
                        return Some(self.cut_short(session_id, revision, run, started));
                    };
                    run.settle(outcome);
                }
            }
        }
    }

    fn cut_short(
        &self,
        session_id: SessionId,
        revision: Revision,
        run: ContextGraphRun,
        started: Instant,
    ) -> Resolution {
        tracing::info!(
            session_id,
            revision,
            duration_us = started.elapsed().as_micros() as u64,
            contexts = run.contexts().len(),
            complete = false,
            "context_resolution_total"
        );
        // The run travels with the result, including the provider call that was still in
        // flight: the late pass resumes exactly this graph, so that call is polled to
        // completion instead of being discarded and made a second time.
        let contexts = run.contexts().to_vec();
        let diagnostics = run.diagnostics().to_vec();
        Resolution {
            contexts,
            diagnostics,
            complete: false,
            run,
        }
    }

    /// Seed contexts for a session/revision.
    ///
    /// Returns `None` when the session is gone or no longer the current one.
    pub fn seeds_for(&self, session_id: SessionId, input: &str) -> Option<Vec<ResolvedContext>> {
        let session = self.session.lock().ok()?;
        if session.state.session_id != session_id {
            return None;
        }
        Some(seeds_from(&session, input))
    }

    /// Publish a snapshot. `None` means this session/revision is no longer current,
    /// which is how stale results are discarded instead of reaching the host.
    pub fn commit(
        &self,
        session_id: SessionId,
        revision: Revision,
        contexts: Vec<ResolvedContext>,
        diagnostics: Vec<String>,
        resolve_complete: bool,
        started: Instant,
    ) -> Result<Option<LauncherState>> {
        let mut contexts = contexts;
        let mut resolve_complete = resolve_complete;
        loop {
            // Take only immutable inputs while holding the session lock. Search and
            // sorting can be slow; new input and cancellation must remain able to run.
            let (
                input,
                input_mode,
                selected_scope,
                facts,
                usage,
                app_usage,
                expected_sequence,
                actions,
                prior_diagnostics,
                inputs_epoch,
                action_catalog_version,
                search_candidates,
                object_actions,
            ) = {
                let session = self
                    .session
                    .lock()
                    .map_err(|_| anyhow!("session lock poisoned"))?;
                if session.state.session_id != session_id
                    || session.state.revision != revision
                    || !self.is_current(session_id, revision)
                {
                    return Ok(None);
                }
                if session.input_mode == InputMode::Environment {
                    contexts.retain(|context| {
                        !matches!(
                            context.source,
                            ContextSource::Clipboard | ContextSource::SelectedText
                        )
                    });
                }
                if session.input_mode == InputMode::FileSearch {
                    contexts.clear();
                }
                if matches!(session.input_mode, InputMode::Content | InputMode::Search) {
                    contexts.retain(|context| context.source == ContextSource::UserInput);
                }
                if session.resolution.committed_revision == Some(revision) {
                    contexts = union_contexts(&session.resolution.committed_contexts, contexts);
                    resolve_complete |= session.state.context_complete;
                }
                (
                    session.state.input.clone(),
                    session.input_mode,
                    session.state.selected_scope.clone(),
                    session.facts(),
                    session.usage.clone(),
                    session.app_usage.clone(),
                    session.resolution.snapshot_seq,
                    session.actions.clone(),
                    session.state.diagnostics.clone(),
                    session.resolution.inputs_epoch,
                    self.action_catalog_version.load(Ordering::Acquire),
                    session.resolution.search_candidates.clone(),
                    session.resolution.object_actions.clone(),
                )
            };
            let (catalog, catalog_generation) = self.catalog_view()?;
            let prepared = super::snapshot::prepare(super::snapshot::SnapshotInput {
                action_catalog_version,
                input: &input,
                input_mode,
                selected_scope: selected_scope.as_deref(),
                facts: &facts,
                usage: &usage,
                app_usage: &app_usage,
                actions: &actions,
                contexts: &contexts,
                incoming_diagnostics: &diagnostics,
                prior_diagnostics: &prior_diagnostics,
                catalog: &catalog,
                sources: &self.sources,
                search_candidates: &search_candidates,
                object_actions: &object_actions,
                now: now(),
            });

            let mut session = self
                .session
                .lock()
                .map_err(|_| anyhow!("session lock poisoned"))?;
            if session.state.session_id != session_id
                || session.state.revision != revision
                || !self.is_current(session_id, revision)
            {
                return Ok(None);
            }
            if session.resolution.snapshot_seq != expected_sequence
                || session.resolution.inputs_epoch != inputs_epoch
                || self.action_catalog_version.load(Ordering::Acquire) != action_catalog_version
                || !Arc::ptr_eq(&session.actions, &actions)
            {
                continue;
            }
            let catalog_moved = self
                .catalog
                .lock()
                .map(|slot| slot.generation != catalog_generation)
                .map_err(|_| anyhow!("catalog lock poisoned"))?;
            if catalog_moved {
                continue;
            }
            crate::features::remember_environment(&mut session.resolution.context_cache, &contexts);
            session.resolution.snapshot_seq = session.resolution.snapshot_seq.saturating_add(1);
            let sequence = session.resolution.snapshot_seq;
            let summary = prepared.summary;
            let late_contexts = summary
                .len()
                .saturating_sub(session.resolution.published_contexts);
            session.resolution.published_contexts = summary.len();
            let mut scopes = prepared.scopes;
            // Current App has a reserved first slot but is never displayed without
            // visible actions. Other late groups only append to the session order.
            if session.resolution.frozen_scope_ids.is_empty() {
                if let Some(app) = &facts.frontmost {
                    session
                        .resolution
                        .frozen_scope_ids
                        .push(crate::core::action::group::groups::for_app(&app.bundle_id));
                }
            }
            crate::core::search::scope::freeze(
                &mut scopes,
                &mut session.resolution.frozen_scope_ids,
            );
            let mut state = LauncherState {
                session_id,
                revision,
                snapshot_seq: sequence,
                input: session.state.input.clone(),
                context_complete: resolve_complete,
                result_actions: prepared
                    .result_actions
                    .iter()
                    .map(|c| c.presentation.clone())
                    .collect(),
                search_sources: session.resolution.search_sources.clone(),
                file_search: session.resolution.file_search.clone().map(Box::new),
                context_summary: summary,
                context_candidates: prepared
                    .context_candidates
                    .iter()
                    .map(|candidate| candidate.presentation.clone())
                    .collect(),
                preferred_context_key: prepared.preferred_context_key,
                scopes,
                selected_scope,
                context_captures: session
                    .identity
                    .captures
                    .iter()
                    .map(|capture| ContextCapturePresentation {
                        text: if capture.status == ContextCaptureStatus::Captured {
                            capture.contexts.iter().find_map(|ctx| match &ctx.value {
                                crate::core::context::ContextValue::Text(text) => {
                                    Some(text.to_string())
                                }
                                _ => None,
                            })
                        } else {
                            None
                        },
                        clipboard_age_ms: if capture.source == ContextSource::Clipboard {
                            session.identity.clipboard_age_ms
                        } else {
                            None
                        },
                        source: capture.source,
                        status: capture.status,
                        captured_at_ms: capture.captured_at_ms,
                    })
                    .collect(),
                candidates: prepared
                    .set
                    .candidates
                    .iter()
                    .map(|candidate| candidate.presentation.clone())
                    .collect::<Vec<_>>()
                    .into(),
                candidate_list_version: session.state.candidate_list_version,
                diagnostics: {
                    let mut diagnostics = prepared.diagnostics;
                    for item in &session.state.diagnostics {
                        if !diagnostics.contains(item) {
                            diagnostics.push(item.clone());
                        }
                    }
                    diagnostics
                },
            };
            if !contexts.is_empty() {
                let base = self.base_registry();
                let options = prepared.shortcut_catalog;
                if let Ok(mut catalog) = self.shortcut_catalog.lock() {
                    *catalog = options
                        .into_iter()
                        .filter(|a| {
                            base.get(&a.action_id).is_none()
                                && base.supports_persistent_binding(&a.action_id)
                        })
                        .take(256)
                        .collect();
                }
            }
            if *state.candidates != *session.state.candidates {
                let append = session.input_mode == InputMode::FileSearch
                    && state.candidates.starts_with(&session.state.candidates);
                if !append || session.state.candidate_list_version == 0 {
                    state.candidate_list_version = session
                        .state
                        .candidate_list_version
                        .checked_add(1)
                        .ok_or_else(|| anyhow!("candidate list version exhausted"))?;
                }
            }
            session.resolution.committed_revision = Some(revision);
            session.resolution.committed_contexts = contexts;
            session.state = state.clone();
            session.execution.candidates = prepared
                .set
                .candidates
                .iter()
                .chain(&prepared.context_candidates)
                .chain(&prepared.result_actions)
                .chain(&prepared.host_actions)
                .map(|candidate| (candidate.presentation.id.clone().into(), candidate.clone()))
                .collect();
            session.execution.revision = Some(revision);
            session.execution.action_catalog_version = Some(action_catalog_version);
            // `first_snapshot` marks the snapshot the caller's budget produced; its
            // duration is the spec's `first_snapshot_duration`.
            let is_first_snapshot = sequence == 1;
            tracing::info!(
                session_id,
                revision,
                snapshot_seq = sequence,
                complete = resolve_complete,
                first_snapshot = is_first_snapshot,
                candidates = state.candidates.len(),
                contexts = state.context_summary.len(),
                candidate_kinds = ?state
                    .candidates
                    .iter()
                    .map(|candidate| candidate.kind)
                    .collect::<Vec<_>>(),
                // Ranked head, so a ranking question can be answered from the log
                // instead of by reproducing the exact usage history by hand.
                top_candidates = ?state
                    .candidates
                    .iter()
                    .take(6)
                    .map(|candidate| format!("{}={:.0}", if candidate.kind == CandidateKind::File { "file" } else { &candidate.title }, candidate.score))
                    .collect::<Vec<_>>(),
                late_context_count = late_contexts,
                first_snapshot_duration = if is_first_snapshot {
                    started.elapsed().as_micros() as u64
                } else {
                    0
                },
                duration_us = started.elapsed().as_micros() as u64,
                "snapshot_committed"
            );
            // The matcher times itself, so this is the candidate work alone and not the
            // session lock or the snapshot bookkeeping around it. The application index is
            // reported separately: matching installed applications is the same kind of work as
            // matching actions, and folding it into `ranking_duration` made that number
            // describe sorting when it was really measuring a name search.
            tracing::info!(
                session_id,
                revision,
                matcher_duration = prepared.set.matcher_us,
                sources_duration = prepared.set.sources_us,
                "matcher_completed"
            );
            tracing::info!(
                session_id,
                revision,
                ranking_duration = prepared.set.ranking_us,
                "ranking_completed"
            );
            self.latest_tx.send_replace(Some(Arc::new(state.clone())));
            let _ = self.events.send(LauncherEvent::ActionsUpdated {
                state: state.clone(),
            });
            return Ok(Some(state));
        }
    }
}

fn union_contexts(
    committed: &[ResolvedContext],
    incoming: Vec<ResolvedContext>,
) -> Vec<ResolvedContext> {
    let mut merged = committed.to_vec();
    let mut incoming_ids = std::collections::HashMap::new();
    for mut context in incoming {
        let existing = merged.iter().find(|held| {
            held.type_id == context.type_id
                && held.value.shared_text() == context.value.shared_text()
                && held.source == context.source
        });
        if let Some(held) = existing {
            incoming_ids.insert(context.id, held.id.clone());
        } else {
            let old_id = std::mem::take(&mut context.id);
            context.id = format!("c{}", merged.len());
            context.source_context_id = context
                .source_context_id
                .and_then(|parent| incoming_ids.get(&parent).cloned());
            incoming_ids.insert(old_id, context.id.clone());
            merged.push(context);
        }
    }
    merged
}

/// Seed contexts for one revision: memoized environment data, the frozen frontmost
/// application, then the raw input text.
///
/// Callers must already hold the session lock.
pub fn seeds_from(session: &LauncherSession, input: &str) -> Vec<ResolvedContext> {
    if session.input_mode == InputMode::FileSearch {
        return Vec::new();
    }
    if matches!(session.input_mode, InputMode::Content | InputMode::Search) {
        return crate::features::text::provider::text_context(input)
            .into_iter()
            .collect();
    }
    let mut seeds = crate::features::session_seeds(
        session.identity.frontmost.as_ref(),
        session.resolution.context_cache.clone(),
        input,
    );
    for capture in &session.identity.captures {
        if session.input_mode == InputMode::Environment
            && matches!(
                capture.source,
                ContextSource::Clipboard | ContextSource::SelectedText
            )
        {
            continue;
        }
        seeds.extend(capture.contexts.iter().cloned());
    }
    seeds
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::ContextValue;

    fn fixture() -> LauncherSession {
        LauncherSession {
            state: LauncherState {
                session_id: 1,
                ..Default::default()
            },
            identity: crate::application::session_model::SessionIdentity {
                frontmost: Some(anywhere_platform::ApplicationInfo {
                    bundle_id: "com.apple.finder".into(),
                    name: "Finder".into(),
                    pid: 1,
                    instance_id: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn directory(path: &str, source: ContextSource) -> ResolvedContext {
        ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(path.into()),
            source,
            "test",
        )
    }

    /// The cache is what makes a later revision reuse the environment without new OS
    /// work. Only ambient context may enter it: a directory the user typed is an input,
    /// and caching it made every later revision seed from the previous input, so a path
    /// typed once kept contributing its Folder/Git actions and stayed the execution
    /// target after the input had moved on.
    #[test]
    fn only_ambient_context_is_reusable_across_revisions() {
        let mut session = fixture();
        session.resolution.context_cache =
            vec![directory("/env", ContextSource::ApplicationContext)];

        let seeds = seeds_from(&session, "https://example.com");
        assert!(
            seeds.iter().any(|seed| seed.value.text() == "/env"),
            "the ambient directory is reused: {seeds:?}"
        );

        let mut kept = Vec::new();
        crate::features::remember_environment(
            &mut kept,
            &[
                directory("/env", ContextSource::ApplicationContext),
                directory("/typed", ContextSource::UserInput),
                ResolvedContext::new(
                    GIT_REPOSITORY,
                    ContextValue::Repository("/typed".into()),
                    ContextSource::UserInput,
                    "test",
                ),
                ResolvedContext::new(
                    GIT_REPOSITORY,
                    ContextValue::Repository("/env".into()),
                    ContextSource::ApplicationContext,
                    "test",
                ),
            ],
        );
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|c| !c.value.text().contains("/typed")));

        session.resolution.context_cache = kept;
        let seeds = seeds_from(&session, "chrome");
        assert!(
            !seeds
                .iter()
                .any(|seed| seed.value.text().contains("/typed")),
            "input-derived context leaked into the seeds: {seeds:?}"
        );
    }
}
