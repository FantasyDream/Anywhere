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
    core::search::engine::ActionSearch,
    core::search::query::Query,
    features::applications::index::ApplicationCatalog,
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
    pub registry: Arc<ActionRegistry>,
    /// Stateless providers; the directory provider is added per revision.
    pub providers: ContextProviderRegistry,
    pub sources: Vec<Arc<dyn CandidateSource + Send + Sync>>,
    pub session: Arc<Mutex<LauncherSession>>,
    pub current: Arc<AtomicU64>,
    pub events: broadcast::Sender<LauncherEvent>,
    pub latest_tx: watch::Sender<Option<Arc<LauncherState>>>,
    pub catalog: Arc<Mutex<Arc<ApplicationCatalog>>>,
    pub catalog_rx: watch::Receiver<u64>,
    pub catalog_issued: Arc<AtomicU64>,
    pub history: Arc<Mutex<Box<dyn UsageRepository>>>,
}

/// Outcome of one resolution pass. `run` lets the caller resume an incomplete pass
/// with the same `(provider, context, session, revision)` execution-dedup state.
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
            registry: Arc::new(registry),
            // Registration order does not matter: the registry sorts by priority, then id.
            providers: super::composition::builtin_providers(),
            sources: Vec::new(),
            session: Arc::new(Mutex::new(LauncherSession::default())),
            current: Arc::new(AtomicU64::new(0)),
            events: broadcast::channel(256).0,
            latest_tx,
            catalog: Arc::new(Mutex::new(Arc::new(ApplicationCatalog::empty()))),
            catalog_rx,
            catalog_issued: Arc::new(AtomicU64::new(0)),
            history: Arc::new(Mutex::new(history)),
        }
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

    pub fn catalog_snapshot(&self) -> Arc<ApplicationCatalog> {
        self.catalog.lock().expect("catalog lock poisoned").clone()
    }

    /// Revision currently published for the live session, if any.
    pub fn committed_revision(&self) -> Option<Revision> {
        self.session.lock().ok()?.committed_revision
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
            let revision = session.committed_revision?;
            if !self.is_current(session_id, revision) {
                return None;
            }
            (
                revision,
                session.committed_contexts.clone(),
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
    fn providers_for(&self, session_id: SessionId) -> Vec<Arc<dyn ContextProvider>> {
        let directory = self
            .session
            .lock()
            .ok()
            .filter(|session| session.state.session_id == session_id)
            .and_then(|session| session.directory.clone());
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
        Some(super::composition::session_seeds(&session, input))
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
            let (input, facts, usage, app_usage, expected_sequence, actions) = {
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
                if session.committed_revision == Some(revision) {
                    // A source refresh may have captured contexts before a late pass
                    // completed. Never replace those richer contexts with an old copy.
                    if session.committed_contexts.len() > contexts.len()
                        || (session.state.context_complete && !resolve_complete)
                    {
                        contexts = session.committed_contexts.clone();
                    }
                    resolve_complete |= session.state.context_complete;
                }
                (
                    session.state.input.clone(),
                    session.facts(),
                    session.usage.clone(),
                    session.app_usage.clone(),
                    session.snapshot_seq,
                    session.actions.clone(),
                )
            };
            let query = Query::parse(&input);
            let catalog = self.catalog_snapshot();
            let usage_of = |command: &str, kind: &str| {
                usage
                    .get(&(command.to_owned(), kind.to_owned()))
                    .copied()
                    .unwrap_or_default()
            };
            let app_usage_of = |command: &str, kind: &str| {
                app_usage
                    .get(&(command.to_owned(), kind.to_owned()))
                    .copied()
                    .unwrap_or_default()
            };
            let search = ActionSearch::new(&actions, &usage_of, &facts, now());
            let application_source =
                super::composition::catalog_source(&catalog, &app_usage_of, now());
            let mut extra: Vec<&dyn CandidateSource> = vec![&application_source];
            extra.extend(
                self.sources
                    .iter()
                    .map(|source| source.as_ref() as &dyn CandidateSource),
            );
            let set = search.search(&query, &contexts, &extra);

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
            if session.snapshot_seq != expected_sequence || !Arc::ptr_eq(&session.actions, &actions)
            {
                // Another commit won for this same revision. Recompute from its latest
                // state instead of losing enrichment or publishing out of order.
                continue;
            }
            for context in &contexts {
                if !matches!(context.type_id.as_str(), DIRECTORY | GIT_REPOSITORY)
                    || context.source == ContextSource::UserInput
                {
                    continue;
                }
                if !session.context_cache.iter().any(|cached| {
                    cached.type_id == context.type_id && cached.value.text() == context.value.text()
                }) {
                    session.context_cache.push(context.clone());
                }
            }
            let mut all_diagnostics = session.state.diagnostics.clone();
            for diagnostic in &diagnostics {
                if !all_diagnostics.contains(diagnostic) {
                    all_diagnostics.push(diagnostic.clone());
                }
            }
            session.snapshot_seq = session.snapshot_seq.saturating_add(1);
            let sequence = session.snapshot_seq;
            let summary = visible_contexts(&contexts);
            let late_contexts = summary.len().saturating_sub(session.published_contexts);
            session.published_contexts = summary.len();
            let scopes = crate::core::search::scope::apply_session_order(
                set.scopes,
                &mut session.frozen_scope_ids,
            );
            let state = LauncherState {
                session_id,
                revision,
                snapshot_seq: sequence,
                input: session.state.input.clone(),
                context_complete: resolve_complete,
                context_summary: summary,
                candidates: set
                    .candidates
                    .iter()
                    .map(|candidate| candidate.presentation.clone())
                    .collect(),
                scopes,
                diagnostics: all_diagnostics,
            };
            session.committed_revision = Some(revision);
            session.committed_contexts = contexts;
            session.state = state.clone();
            session.state_candidates = set
                .candidates
                .iter()
                .map(|candidate| (candidate.presentation.id.clone(), candidate.clone()))
                .collect();
            session.candidates_revision = Some(revision);
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
                scope_ids = ?state
                    .scopes
                    .iter()
                    .map(|scope| scope.id.as_str())
                    .collect::<Vec<_>>(),
                scope_candidates = ?state
                    .scopes
                    .iter()
                    .map(|scope| scope.candidate_ids.len())
                    .collect::<Vec<_>>(),
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
                    .map(|candidate| format!("{}={:.0}", candidate.title, candidate.score))
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
                matcher_duration = set.matcher_us,
                sources_duration = set.sources_us,
                "matcher_completed"
            );
            tracing::info!(
                session_id,
                revision,
                ranking_duration = set.ranking_us,
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

/// Seed contexts for one revision: memoized environment data, the frozen frontmost
/// application, then the raw input text.
///
/// Callers must already hold the session lock.
pub fn seeds_from(session: &LauncherSession, input: &str) -> Vec<ResolvedContext> {
    super::composition::session_seeds(session, input)
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

/// Context summary shown by the host; the raw foreground application row stays internal.
fn visible_contexts(contexts: &[ResolvedContext]) -> Vec<ContextPresentation> {
    contexts
        .iter()
        .filter(|context| {
            !(context.type_id == APPLICATION
                && context.source == ContextSource::ForegroundApplication)
        })
        .map(ResolvedContext::presentation)
        .collect()
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
            frontmost: Some(anywhere_platform::ApplicationInfo {
                bundle_id: "com.apple.finder".into(),
                name: "Finder".into(),
                pid: 1,
                instance_id: None,
            }),
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
        session.context_cache = vec![directory("/env", ContextSource::ApplicationContext)];

        let seeds = seeds_from(&session, "https://example.com");
        assert!(
            seeds.iter().any(|seed| seed.value.text() == "/env"),
            "the ambient directory is reused: {seeds:?}"
        );

        // What `commit` keeps, given a mixed batch: ambient in, user input out.
        let mut kept: Vec<ResolvedContext> = Vec::new();
        for context in [
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
        ] {
            if context.type_id != DIRECTORY && context.type_id != GIT_REPOSITORY {
                continue;
            }
            if context.source == ContextSource::UserInput {
                continue;
            }
            kept.push(context);
        }
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|c| !c.value.text().contains("/typed")));

        session.context_cache = kept;
        let seeds = seeds_from(&session, "chrome");
        assert!(
            !seeds
                .iter()
                .any(|seed| seed.value.text().contains("/typed")),
            "input-derived context leaked into the seeds: {seeds:?}"
        );
    }
}
