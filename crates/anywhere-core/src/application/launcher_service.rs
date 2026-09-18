//! Launcher facade: activation capture, request lifecycle, execution, subscription.
use crate::{
    application::session_runtime::Resolver, core::action::descriptor::ActionRegistry,
    core::context::engine::ContextGraphRun, core::ranking::usage::UsageRepository,
    features::applications::index as applications,
};
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;

/// Caller-controlled first-snapshot budget. Providers never see it.
pub const DEFAULT_FIRST_SNAPSHOT_BUDGET: Duration = Duration::from_millis(40);
/// Extra time the first snapshot may spend on a provider call that is still in flight.
///
/// The graph is sequential, so at most one call is ever pending, and a budget that
/// lands mid-call would publish a half-finished list: a real machine committed a
/// 3-action snapshot 0ms before the Git provider ran and would have added 3 more.
/// A Finder Automation capture measured ~250ms, so the budget alone also produced an
/// empty launcher that demanded typing first. The extension is bounded by this value
/// and applies only when a call is actually pending.
pub const DEFAULT_FIRST_CONTENT_GRACE: Duration = Duration::from_millis(400);
/// Largest input the host may send, in bytes.
pub(crate) const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Icon edge in pixels; small enough that one screen of rows stays cheap.
const ICON_PIXELS: u32 = 64;

#[async_trait::async_trait]
pub trait LauncherApplication: Send + Sync {
    async fn activate(&self) -> Result<Option<LauncherState>>;
    async fn update_input(
        &self,
        session_id: SessionId,
        input: String,
        revision: Revision,
    ) -> Result<Option<LauncherState>>;
    async fn execute(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: ActionId,
    ) -> Result<ActionResult>;
    /// Small PNG for an application candidate, or `None` when there is no icon.
    ///
    /// Presentation only: it never runs while a snapshot is being built.
    fn application_icon(&self, candidate_id: &str) -> Result<Option<Vec<u8>>>;
    /// Subscribe to state updates for one session, replaying the latest snapshot.
    fn subscribe(&self, session_id: SessionId) -> SessionSubscription;
    fn deactivate(&self, session_id: SessionId) -> Result<()>;
}

pub struct Application {
    pub(super) platform: Arc<dyn Platform>,
    /// The engine, shared with the background tasks that publish late results.
    pub(super) resolver: Resolver,
    pub(super) startup_diagnostics: Vec<String>,
    pub(super) budget: Duration,
    pub(super) content_grace: Duration,
    pub(super) catalog_tx: watch::Sender<bool>,
    /// One catalog scan per process; later sessions reuse the immutable snapshot.
    pub(super) catalog_started: Arc<std::sync::atomic::AtomicBool>,
    pub(super) app_actions: crate::features::app_actions::AppActionCatalog,
}

impl Application {
    pub fn new(
        platform: Arc<dyn Platform>,
        registry: ActionRegistry,
        history: Box<dyn UsageRepository>,
        startup_diagnostics: Vec<String>,
    ) -> Self {
        Self::with_budget(
            platform,
            registry,
            history,
            startup_diagnostics,
            DEFAULT_FIRST_SNAPSHOT_BUDGET,
        )
    }

    pub fn with_budget(
        platform: Arc<dyn Platform>,
        registry: ActionRegistry,
        history: Box<dyn UsageRepository>,
        startup_diagnostics: Vec<String>,
        budget: Duration,
    ) -> Self {
        let mut resolver = Resolver::new(registry, history);
        let (catalog_tx, catalog_rx) = watch::channel(false);
        resolver.catalog_rx = catalog_rx;
        Self {
            platform,
            resolver,
            startup_diagnostics,
            budget,
            content_grace: DEFAULT_FIRST_CONTENT_GRACE,
            catalog_tx,
            catalog_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_actions: crate::features::app_actions::AppActionCatalog::default(),
        }
    }

    /// Override the first-content grace.
    ///
    /// The shipped hosts use `DEFAULT_FIRST_CONTENT_GRACE`; this exists so the budget
    /// matrix (budget 0, grace 0, grace shorter than a slow capture) stays testable
    /// without a second constructor.
    #[doc(hidden)]
    pub fn with_content_grace(mut self, grace: Duration) -> Self {
        self.content_grace = grace;
        self
    }

    /// Latest committed snapshot for a session, whatever task published it.
    ///
    /// Late context is published by a background task, so this is the only way a
    /// bridge can observe an enrichment that happened after `activate` returned.
    pub fn latest_state(&self, session_id: SessionId) -> Option<LauncherState> {
        let state = self.resolver.latest_tx.borrow().clone()?;
        (state.session_id == session_id && self.resolver.is_current(session_id, state.revision))
            .then(|| (*state).clone())
    }

    pub fn is_session_active(&self, session_id: SessionId) -> bool {
        self.resolver.is_session_active(session_id)
    }

    /// Channel signalled whenever a snapshot is committed.
    pub fn state_updates(&self) -> watch::Receiver<Option<Arc<LauncherState>>> {
        self.resolver.latest_tx.subscribe()
    }

    /// Wait until the background application index has been published.
    ///
    /// The first snapshot never waits for this; it exists for callers that want
    /// application search results on the very first query.
    pub async fn catalog_ready(&self) {
        self.resolver.catalog_ready().await;
    }

    /// Signalled whenever the application index is published.
    ///
    /// The scan starts at activation, so an app search typed within the first ~60ms would
    /// otherwise match an empty index and no later input would revisit it. The bridge
    /// re-commits the current snapshot when this fires.
    pub fn index_updates(&self) -> watch::Receiver<bool> {
        self.catalog_tx.subscribe()
    }

    /// Additional product sources use the same matching, sorting and publication path.
    pub fn register_candidate_source(
        &mut self,
        source: Arc<dyn crate::core::search::candidate_source::CandidateSource + Send + Sync>,
    ) {
        self.resolver.sources.push(source);
    }

    /// Extra providers are stacked on the built-ins by priority then id.
    pub fn register_provider(
        &mut self,
        provider: Arc<dyn crate::core::context::provider::ContextProvider>,
    ) -> Result<()> {
        self.resolver.register_provider(provider)
    }

    pub(super) fn session(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, crate::application::session_model::LauncherSession>> {
        self.resolver
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("session lock poisoned"))
    }

    /// Background application scan; never blocks the first snapshot.
    ///
    /// Concurrent activations share the first scan; refresh is outside this MVP.
    pub(super) fn spawn_catalog_scan(&self) {
        if self
            .catalog_started
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        let platform = self.platform.clone();
        let catalog = self.resolver.catalog.clone();
        let tx = self.catalog_tx.clone();
        tokio::spawn(async move {
            let scanned = applications::scan_async(platform).await;
            let count = scanned.applications().len();
            if let Ok(mut handle) = catalog.lock() {
                *handle = Arc::new(scanned);
            }
            tx.send_replace(true);
            tracing::info!(count, "app_index_published");
        });
    }

    /// Re-publish the current snapshot once the initial application scan finishes.
    ///
    /// The scan starts at activation and takes ~60ms on a real machine, so a search typed
    /// immediately after opening would match an empty index: the first snapshot never
    /// waits for the scan, and only a new input would otherwise revisit that revision.
    ///
    /// The rebuild happens after the revision settles. Publishing it earlier would emit a
    /// snapshot that is not final with no late context behind it, and hosts treat a new
    /// `snapshot_seq` as "there is more to show now".
    pub(super) fn spawn_index_recompute(&self, session_id: SessionId) {
        let resolver = self.resolver.clone();
        let mut index = self.catalog_tx.subscribe();
        let mut states = self.state_updates();
        tokio::spawn(async move {
            while !*index.borrow_and_update() {
                if !resolver.is_session_active(session_id) {
                    return;
                }
                tokio::select! {
                    changed = index.changed() => { if changed.is_err() { return; } }
                    changed = states.changed() => { if changed.is_err() { return; } }
                }
            }
            // Cancellation publishes None. Check the session before waiting as well as
            // after a wake-up, including when this task starts after dismissal.
            loop {
                if !resolver.is_session_active(session_id) {
                    return;
                }
                let published = states.borrow_and_update().clone();
                if published.is_some_and(|state| {
                    state.session_id == session_id
                        && Some(state.revision) == resolver.committed_revision()
                        && state.context_complete
                }) {
                    break;
                }
                if states.changed().await.is_err() {
                    return;
                }
            }
            match resolver.recompute_current(session_id) {
                Some(state) => {
                    tracing::info!(
                        session_id,
                        revision = state.revision,
                        snapshot_seq = state.snapshot_seq,
                        candidates = state.candidates.len(),
                        "snapshot_rebuilt_for_app_index"
                    );
                }
                None => tracing::info!(session_id, "app_index_rebuild_skipped",),
            }
        });
    }

    /// Late context arrives here; a single resolution per revision.
    pub(super) fn spawn_completion(
        &self,
        session_id: SessionId,
        revision: Revision,
        run: ContextGraphRun,
        started: Instant,
        startup_diagnostics: Vec<String>,
    ) {
        let resolver = self.resolver.clone();
        // Only continue for the session that is still current; otherwise the late
        // result could not be published anyway.
        match self.resolver.session.lock() {
            Ok(session) if session.state.session_id == session_id => {}
            _ => return,
        }
        tokio::spawn(async move {
            if let Some(resolution) = resolver.resolve_until(session_id, run, started).await {
                let mut diagnostics = startup_diagnostics;
                diagnostics.extend(resolution.diagnostics);
                let _ = resolver.commit(
                    session_id,
                    revision,
                    resolution.contexts,
                    diagnostics,
                    true,
                    started,
                );
            }
        });
    }

    /// The history bucket of a session: the application that was in front, which is
    /// what every usage row is keyed by.
    ///
    /// Activation must resolve this *before* reading history. Looking the rows up under
    /// an empty key found nothing, so every frequency and recency term read as zero and
    /// repeated use could never lift an action.
    pub(super) fn source_app_of(frontmost: Option<&ApplicationInfo>) -> String {
        frontmost
            .map(|application| application.bundle_id.clone())
            .unwrap_or_default()
    }

    pub(super) async fn load_usage(&self, source_app: String) -> (UsageMap, UsageMap, Vec<String>) {
        let history = self.resolver.history.clone();
        let loaded = tokio::task::spawn_blocking(move || {
            let history = history
                .lock()
                .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
            Ok::<_, anyhow::Error>((history.load(&source_app), history.load_global()))
        })
        .await;
        let mut diagnostics = Vec::new();
        let (scoped, global) = match loaded {
            Ok(Ok((scoped, global))) => {
                let mut recover = |result: anyhow::Result<UsageMap>| match result {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!(error = %error, "usage_load_failed");
                        diagnostics.push(format!("Usage history could not be loaded: {error:#}"));
                        UsageMap::default()
                    }
                };
                (recover(scoped), recover(global))
            }
            failure => {
                let message = format!("Usage history task failed: {failure:?}");
                tracing::warn!(%message, "usage_load_failed");
                diagnostics.push(message);
                (UsageMap::default(), UsageMap::default())
            }
        };
        (scoped, global, diagnostics)
    }
}

type UsageMap =
    std::collections::HashMap<(String, String), crate::core::ranking::score::UsageStats>;

#[async_trait::async_trait]
impl LauncherApplication for Application {
    /// See `activate.rs`.
    async fn activate(&self) -> Result<Option<LauncherState>> {
        let session_id = self.begin_activation()?;
        self.activate_session(session_id).await
    }

    /// See `update_query.rs`.
    async fn update_input(
        &self,
        session_id: SessionId,
        input: String,
        revision: Revision,
    ) -> Result<Option<LauncherState>> {
        self.update_query_use_case(session_id, input, revision)
            .await
    }

    /// See `execute.rs`.
    async fn execute(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: ActionId,
    ) -> Result<ActionResult> {
        self.execute_use_case(session_id, revision, candidate_id)
            .await
    }

    /// Small PNG for an application candidate, fetched on demand for visible rows.
    ///
    /// Never called while building a snapshot: the icon is presentation only and must
    /// not affect ranking, ordering or the time to first snapshot.
    fn application_icon(&self, candidate_id: &str) -> Result<Option<Vec<u8>>> {
        let path = {
            let session = self.session()?;
            session
                .state_candidates
                .get(candidate_id)
                .and_then(|candidate| match &candidate.target {
                    crate::core::search::candidate::CandidateTarget::LaunchApplication {
                        icon_source,
                        ..
                    } => Some(icon_source.clone()),
                    _ => None,
                })
        };
        let Some(path) = path else {
            return Ok(None);
        };
        self.platform.application_icon(&path, ICON_PIXELS)
    }

    fn subscribe(&self, session_id: SessionId) -> SessionSubscription {
        SessionSubscription {
            receiver: self.resolver.events.subscribe(),
            session_id,
            current: self.resolver.current.clone(),
            latest: self.resolver.latest_tx.subscribe(),
        }
    }

    fn deactivate(&self, session_id: SessionId) -> Result<()> {
        let mut session = self.session()?;
        if session.state.session_id == session_id {
            self.resolver
                .current
                .store(0, std::sync::atomic::Ordering::Release);
            session.directory = None;
            session.state_candidates.clear();
            session.context_cache.clear();
            self.resolver.latest_tx.send_replace(None);
        }
        Ok(())
    }
}

pub struct SessionSubscription {
    receiver: tokio::sync::broadcast::Receiver<LauncherEvent>,
    session_id: SessionId,
    current: Arc<std::sync::atomic::AtomicU64>,
    latest: watch::Receiver<Option<Arc<LauncherState>>>,
}

impl SessionSubscription {
    /// Latest committed state, replayed even if it was published before subscribing.
    pub fn latest_state(&self) -> Option<LauncherState> {
        self.latest.borrow().as_ref().map(|state| (**state).clone())
    }

    pub async fn recv(&mut self) -> Result<LauncherEvent> {
        loop {
            let event = self.receiver.recv().await?;
            let (session_id, revision) = match &event {
                LauncherEvent::ContextAdded { context } => (context.session_id, context.revision),
                LauncherEvent::ActionsUpdated { state } => (state.session_id, state.revision),
                LauncherEvent::ProviderCompleted {
                    session_id,
                    revision,
                    ..
                }
                | LauncherEvent::ProviderFailed {
                    session_id,
                    revision,
                    ..
                } => (*session_id, *revision),
            };
            if session_id == self.session_id
                && self.current.load(std::sync::atomic::Ordering::Acquire)
                    == crate::core::context::engine::stamp(session_id, revision)
            {
                return Ok(event);
            }
        }
    }
}
