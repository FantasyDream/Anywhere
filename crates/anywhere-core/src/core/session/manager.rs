//! Resolution engine: context stepping, budget control, snapshot publication.
//!
//! Deliberately separate from the [`Application`](crate::application::launcher_service::Application)
//! facade so a background completion task can own a clone and publish late context
//! without borrowing the facade.
use crate::{
    applications::ApplicationCatalog,
    command::CommandRegistry,
    context::ResolvedContext,
    core::search::candidate::Candidate,
    core::search::engine::SearchEngine,
    core::session::facts::SessionFacts,
    features::applications::index::IndexedApplication,
    history::UsageRepository,
    provider::{
        stamp, ContextGraph, ContextGraphRun, ContextProvider, CurrentDirectoryProvider,
        ExecutionContext, GitRepositoryProvider, GraphStep, InputTypeProvider, TextProvider,
    },
    query::Query,
    ranking::UsageStats,
};
use anyhow::{anyhow, Result};
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{broadcast, watch};

#[derive(Default)]
pub struct Session {
    pub state: LauncherState,
    /// Immutable ambient capture: the original frontmost application.
    pub frontmost: Option<ApplicationInfo>,
    /// Session-scoped single-flight directory capture.
    pub directory: Option<Arc<crate::core::context::provider::DirectoryMemo>>,
    /// Snapshot of the real text clipboard, read once at activation.
    pub clipboard: Option<String>,
    pub git_available: bool,
    /// Contexts reusable across revisions without repeating OS work.
    pub context_cache: Vec<ResolvedContext>,
    pub committed_revision: Option<Revision>,
    /// Contexts the committed revision published, for a later re-match against a source
    /// that arrived late (the application index) without re-resolving anything.
    pub committed_contexts: Vec<ResolvedContext>,
    /// Context rows already published for the committed revision, for late counts.
    pub published_contexts: usize,
    pub snapshot_seq: u32,
    pub source_app: String,
    pub usage: HashMap<(String, String), UsageStats>,
    pub app_usage: HashMap<(String, String), UsageStats>,
    /// Committed candidates keyed by stable id, for execution.
    pub state_candidates: HashMap<ActionId, Candidate>,
    /// Revision the candidate map above was built for.
    ///
    /// A new revision is current from the moment its input arrives, but its candidates
    /// only exist once `commit` builds them. Without this an execute request that named
    /// the new revision could still be served from the previous revision's candidates,
    /// which carry the previous revision's context and execution target.
    pub candidates_revision: Option<Revision>,
}

/// Cloneable engine shared by the facade and its background completion tasks.
#[derive(Clone)]
pub struct Resolver {
    pub platform: Arc<dyn Platform>,
    pub registry: Arc<CommandRegistry>,
    /// Stateless providers; the directory provider is added per revision.
    pub providers: Vec<Arc<dyn ContextProvider>>,
    pub session: Arc<Mutex<Session>>,
    pub current: Arc<AtomicU64>,
    pub events: broadcast::Sender<LauncherEvent>,
    pub latest_tx: watch::Sender<Option<Arc<LauncherState>>>,
    pub catalog: Arc<Mutex<ApplicationCatalog>>,
    pub catalog_rx: watch::Receiver<bool>,
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
    pub fn new(
        platform: Arc<dyn Platform>,
        registry: CommandRegistry,
        history: Box<dyn UsageRepository>,
    ) -> Self {
        let (latest_tx, _) = watch::channel(None);
        let (_, catalog_rx) = watch::channel(false);
        Self {
            platform,
            registry: Arc::new(registry),
            providers: vec![
                Arc::new(TextProvider::default()),
                Arc::new(InputTypeProvider::default()),
                Arc::new(GitRepositoryProvider::default()),
            ],
            session: Arc::new(Mutex::new(Session::default())),
            current: Arc::new(AtomicU64::new(0)),
            events: broadcast::channel(256).0,
            latest_tx,
            catalog: Arc::new(Mutex::new(ApplicationCatalog::empty())),
            catalog_rx,
            history: Arc::new(Mutex::new(history)),
        }
    }

    pub fn register_provider(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        let mut graph = ContextGraph::default();
        for existing in self.providers.drain(..) {
            graph.register(existing)?;
        }
        graph.register(provider)?;
        self.providers = graph.into_providers();
        Ok(())
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

    /// Wait until the background application scan has published its catalog.
    ///
    /// Checks the current value before waiting: the sender only ever goes `false → true`,
    /// so a receiver that subscribed after the scan finished would see no further change
    /// and wait out the whole timeout for a catalog it already has.
    pub async fn catalog_ready(&self) {
        let mut rx = self.catalog_rx.clone();
        if !*rx.borrow_and_update() {
            let _ = tokio::time::timeout(Duration::from_secs(10), rx.changed()).await;
        }
    }

    pub fn catalog_applications(&self) -> Vec<IndexedApplication> {
        self.catalog
            .lock()
            .map(|catalog| catalog.applications().to_vec())
            .unwrap_or_default()
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
        let (revision, contexts, complete, diagnostics) = {
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
                session.state.diagnostics.clone(),
            )
        };
        let state = self
            .commit(
                session_id,
                revision,
                contexts,
                Vec::new(),
                complete,
                Instant::now(),
            )
            .ok()
            .flatten();
        // `commit` appends diagnostics; this pass added none, so keep the published list
        // from growing on every re-match.
        if state.is_some() {
            if let Ok(mut session) = self.session.lock() {
                if session.state.session_id == session_id && session.state.revision == revision {
                    session.state.diagnostics = diagnostics;
                }
            }
        }
        state
    }

    /// Providers for one revision: stateless built-ins plus the directory memo.
    fn providers_for(&self, session_id: SessionId) -> Vec<Arc<dyn ContextProvider>> {
        let directory = self
            .session
            .lock()
            .ok()
            .filter(|session| session.state.session_id == session_id)
            .and_then(|session| session.directory.clone());
        let mut providers = self.providers.clone();
        if let Some(directory) = directory {
            providers.push(Arc::new(CurrentDirectoryProvider::new(directory)));
        }
        providers.sort_by(|a, b| {
            b.descriptor()
                .priority
                .cmp(&a.descriptor().priority)
                .then_with(|| a.descriptor().id.cmp(&b.descriptor().id))
        });
        providers
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
                    let Some(outcome) = run.await_pending(budget_deadline, grace_deadline).await
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
        Some(crate::core::session::manager::seeds_from(&session, input))
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
        if !self.is_current(session_id, revision) {
            tracing::info!(session_id, revision, "snapshot_discarded");
            return Ok(None);
        }
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow!("session lock poisoned"))?;
        // Re-check under the lock: the revision that superseded this one may have taken
        // the lock first, in which case publishing here would overwrite a newer state
        // with an older revision's contexts.
        if session.state.session_id != session_id
            || session.state.revision != revision
            || !self.is_current(session_id, revision)
        {
            tracing::info!(session_id, revision, "snapshot_discarded_under_lock");
            return Ok(None);
        }
        // Matching always runs again, even for a revision that is already published with a
        // complete context: the matcher reads sources outside the context graph — the
        // application index arriving after the first snapshot is the reason — and a
        // re-publish is the only way a snapshot that was built before such a source
        // existed can carry it. The contexts themselves are de-duplicated by
        // `published_contexts`, so a repeat of the same context never double-counts.
        if session.committed_revision == Some(revision) {
            let incoming = visible_contexts(&contexts).len();
            let added = incoming.saturating_sub(session.published_contexts);
            if added > 0 {
                tracing::info!(
                    session_id,
                    revision,
                    late_context_count = added,
                    "late_context_appended"
                );
            }
        }
        // Cache only the *ambient* environment so the next revision needs no OS work.
        //
        // A directory or repository the user typed is an input, not the environment:
        // caching it made a later query seed from the previous input, so a path typed
        // once kept contributing its actions (and its execution target) afterwards.
        // Input-derived context is rebuilt from the input on every revision.
        for context in &contexts {
            if context.type_id != DIRECTORY && context.type_id != GIT_REPOSITORY {
                continue;
            }
            if context.source == ContextSource::UserInput {
                continue;
            }
            let known = session.context_cache.iter().any(|cached| {
                cached.type_id == context.type_id && cached.value.text() == context.value.text()
            });
            if !known {
                session.context_cache.push(context.clone());
            }
        }
        // Activation decides this up front, because history is read before the first
        // commit. This stays as the fallback for a session created without a frontmost
        // application.
        if session.source_app.is_empty() {
            session.source_app = session
                .frontmost
                .as_ref()
                .map(|application| application.bundle_id.clone())
                .unwrap_or_default();
        }
        let query = Query::parse(&session.state.input);
        let facts = SessionFacts {
            clipboard_text: session.clipboard.clone(),
            git_available: session.git_available,
            frontmost: session.frontmost.clone(),
        };
        let applications = self.catalog_applications();
        let usage = session.usage.clone();
        let app_usage = session.app_usage.clone();
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
        let matcher = SearchEngine {
            registry: &self.registry,
            applications: &applications,
            usage: &usage_of,
            app_usage: &app_usage_of,
            facts: &facts,
            now: now(),
        };
        let set = matcher.search(&query, &contexts);
        let mut all_diagnostics = session.state.diagnostics.clone();
        all_diagnostics.extend(diagnostics);
        session.snapshot_seq = session.snapshot_seq.saturating_add(1);
        let sequence = session.snapshot_seq;
        let summary = visible_contexts(&contexts);
        let late_contexts = summary.len().saturating_sub(session.published_contexts);
        session.published_contexts = summary.len();
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
            scopes: set.scopes.clone(),
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
            applications_duration = set.apps_us,
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
        Ok(Some(state))
    }
}

/// Seed contexts for one revision: memoized environment data, the frozen frontmost
/// application, then the raw input text.
///
/// Callers must already hold the session lock.
pub fn seeds_from(session: &Session, input: &str) -> Vec<ResolvedContext> {
    let mut seeds = session.context_cache.clone();
    if let Some(frontmost) = &session.frontmost {
        seeds.push(crate::core::context::provider::frontmost_context(frontmost));
    }
    if let Some(text) = crate::core::context::resolvers::text_context(input) {
        seeds.push(text);
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

    fn fixture() -> Session {
        Session {
            state: LauncherState {
                session_id: 1,
                ..Default::default()
            },
            frontmost: Some(anywhere_platform::ApplicationInfo {
                bundle_id: "com.apple.finder".into(),
                name: "Finder".into(),
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
