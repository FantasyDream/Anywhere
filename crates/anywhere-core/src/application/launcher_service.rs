//! Launcher facade: activation capture, request lifecycle, execution, subscription.
use crate::{
    application::session_runtime::Resolver, core::action::descriptor::ActionRegistry,
    core::context::engine::ContextGraphRun, core::ranking::usage::UsageRepository,
};
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use std::{
    sync::{Arc, Mutex},
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
        candidate_id: CandidateId,
    ) -> Result<ActionResult>;
    /// Small PNG for an application candidate, or `None` when there is no icon.
    ///
    /// Presentation only: it never runs while a snapshot is being built.
    fn application_icon(&self, candidate_id: &str) -> Result<Option<Vec<u8>>>;
    fn deactivate(&self, session_id: SessionId) -> Result<()>;
}

pub struct Application {
    pub(super) tool_windows: Mutex<Vec<std::sync::Weak<Application>>>,
    pub(super) object_selection_order: std::sync::atomic::AtomicU64,
    pub(super) previews: Arc<Mutex<crate::features::preview::PreviewStore>>,
    pub(super) web_rpc_workers: Arc<tokio::sync::Semaphore>,
    pub(super) web_plugins: Arc<Mutex<super::web_plugins::WebPluginService>>,
    pub(super) executions: Arc<crate::core::execution::ExecutionSessionManager>,
    pub(super) platform: Arc<dyn Platform>,
    /// The engine, shared with the background tasks that publish late results.
    pub(super) resolver: Resolver,
    pub(super) startup_diagnostics: Vec<String>,
    pub(super) budget: Duration,
    pub(super) content_grace: Duration,
    pub(super) app_actions: crate::features::app_actions::AppActionCatalog,
    pub(super) file_search_policy:
        Arc<std::sync::RwLock<crate::features::filesystem::search::FileSearchPolicy>>,
    pub(super) file_search_source: Arc<crate::features::filesystem::search::FileSearchSource>,
    pub(super) isolated_file_search: bool,
    pub(super) preview_worker: Arc<tokio::sync::Semaphore>,
    pub(super) context_capture: super::context_capture::ContextCaptureService,
}

impl Application {
    /// Typed system port for app-owned services; shared UI never selects an OS adapter.
    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref()
    }
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
        platform.start_clipboard_tracking();
        let resolver = Resolver::new(registry, history);
        let (source, sources) =
            super::composition::builtin_sources(platform.clone(), resolver.base_registry());
        let file_search_policy = source.policy.clone();
        for source in sources {
            resolver
                .catalog
                .register_async_source(source)
                .expect("invalid built-in source");
        }
        let context_capture =
            super::context_capture::ContextCaptureService::new(platform.clone(), resolver.clone());
        Self {
            tool_windows: Default::default(),
            object_selection_order: Default::default(),
            previews: Default::default(),
            web_rpc_workers: Arc::new(tokio::sync::Semaphore::new(
                anywhere_protocol::web_bridge::MAX_RUNNING,
            )),
            web_plugins: Default::default(),
            executions: Arc::new(Default::default()),
            platform,
            resolver,
            file_search_policy,
            file_search_source: source,
            isolated_file_search: false,
            preview_worker: Arc::new(tokio::sync::Semaphore::new(1)),
            startup_diagnostics,
            budget,
            content_grace: DEFAULT_FIRST_CONTENT_GRACE,
            app_actions: crate::features::app_actions::AppActionCatalog::default(),
            context_capture,
        }
    }

    pub fn execution_sessions(&self) -> Result<Vec<ExecutionSessionSummary>> {
        self.executions.list()
    }

    pub fn execution_updates(&self) -> tokio::sync::watch::Receiver<u64> {
        self.executions.updates()
    }

    pub fn execution_session(&self, id: &str) -> Result<Option<ExecutionSessionDetail>> {
        self.executions.detail(id)
    }

    pub fn stop_execution_session(&self, id: &str) -> Result<()> {
        self.executions.stop(id)
    }

    pub fn remove_execution_session(&self, id: &str) -> Result<()> {
        self.executions.remove(id)
    }

    pub fn shutdown_execution_sessions(&self) -> Result<()> {
        self.executions.shutdown()
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

    /// Presentation may resume for 15 seconds; side effects are revoked immediately.
    pub async fn suspend_session(&self, session_id: SessionId) -> Result<()> {
        self.suspend_session_now(session_id)
    }

    /// Revoke interaction and execution eligibility without waiting on I/O.
    ///
    /// The retained snapshot expires asynchronously after the shared launcher
    /// retention interval. Calling this repeatedly for the same suspension does
    /// not extend that interval because resume advances the execution epoch.
    pub fn suspend_session_now(&self, session_id: SessionId) -> Result<()> {
        let epoch = {
            let mut session = self.session()?;
            if session.state.session_id != session_id || self.resolver.is_abandoned() {
                return Ok(());
            }
            session.suspended = true;
            self.web_plugins
                .lock()
                .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
                .suspend();
            self.resolver.session.advance_execution() + 1
        };
        let resolver = self.resolver.clone();
        let files = self.file_search_source.clone();
        let web_plugins = self.web_plugins.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(
                anywhere_protocol::policy::LAUNCHER_RETENTION_MS,
            ))
            .await;
            let mut session = resolver.session.access_for_revoke();
            if session.state.session_id == session_id
                && session.suspended
                && resolver.session.execution_epoch() == epoch
            {
                session.invalidate_execution();
                let mut web = crate::core::sync::lock_for_revoke(&web_plugins, "web_plugins");
                web.invalidate();
                drop(web);
                resolver.abandon_session();
                files.clear();
            }
        });
        Ok(())
    }

    pub fn resume_session(&self, session_id: SessionId) -> Result<Option<LauncherState>> {
        {
            let mut session = self.session()?;
            if session.state.session_id != session_id
                || !session.suspended
                || self.resolver.is_abandoned()
            {
                return Ok(None);
            }
            session.suspended = false;
            self.resolver.session.advance_execution();
        }
        self.resolver.recompute_current(session_id);
        if let Some(state) = self.latest_state(session_id) {
            return Ok(Some(state));
        }
        // A pending query has no committed execution offers yet. Preserve its
        // input so the Host can resume/reissue it without displaying stale rows.
        let session = self.session()?;
        if session.state.session_id != session_id
            || session.suspended
            || self.resolver.is_abandoned()
        {
            return Ok(None);
        }
        let mut state = session.state.clone();
        state.candidates = Arc::new(Vec::new());
        state.result_actions = Arc::default();
        state.context_candidates = Arc::default();
        Ok(Some(state))
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

    pub fn replace_registry(&self, registry: crate::core::action::descriptor::ActionRegistry) {
        let session_id = {
            let Ok(mut session) = self.session() else {
                let mut web = crate::core::sync::lock_for_revoke(&self.web_plugins, "web_plugins");
                web.invalidate();
                tracing::error!("replace_registry_session_unavailable");
                return;
            };
            let mut web = match crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins") {
                Ok(web) => web,
                Err(error) => {
                    tracing::error!(%error, "replace_registry_web_unavailable");
                    let mut web =
                        crate::core::sync::lock_for_revoke(&self.web_plugins, "web_plugins");
                    web.invalidate();
                    return;
                }
            };
            let registry = match web.restore_actions(&registry) {
                Ok(registry) => registry,
                Err(error) => {
                    tracing::error!(%error, "replace_registry_restore_web_actions_failed");
                    web.invalidate();
                    return;
                }
            };
            web.invalidate();
            self.revoke_tool_windows();
            // Catalog invalidation and session replacement are one publication boundary.
            self.resolver.catalog.replace_registry(registry);
            session.actions = self.resolver.base_registry();
            session.execution = Default::default();
            session.state.session_id
        };
        // Also rebuild when foreground capture failed and there is no menu discovery.
        self.resolver.recompute_current(session_id);
        self.refresh_app_actions(session_id, None);
    }

    /// Commit policy persistence before atomically publishing the new catalog.
    /// Provider runs, retained contexts and execution offers belong to the old configuration.
    #[cfg(test)]
    pub(crate) fn publish_feature_catalog(
        &self,
        registry: ActionRegistry,
        providers: crate::core::context::registry::ContextProviderRegistry,
        sources: crate::core::search::candidate_source::SourceSet,
        policy: Arc<crate::features::policy::FeaturePolicySnapshot>,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.publish_selected_catalog(
            super::feature_catalog::CatalogSelection {
                registry,
                providers,
                sources,
                policy,
                excluded: Default::default(),
            },
            persist,
        )
    }

    pub(super) fn publish_selected_catalog(
        &self,
        mut selection: super::feature_catalog::CatalogSelection,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let mut session = self.session()?;
        let mut web = self
            .web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?;
        selection.registry = web.restore_actions(&selection.registry)?;
        selection.providers = web.restore_providers(
            &selection.providers,
            self.resolver.session.clone(),
            self.platform.clone(),
        )?;
        let retired = self
            .resolver
            .catalog
            .publish(selection, persist, |registry| {
                web.invalidate();
                self.resolver.session.advance_execution();
                session.actions = registry;
                session.invalidate_execution();
                self.resolver.abandon_session();
            })?;
        self.revoke_tool_windows();
        drop(session);
        retired.reset();
        self.file_search_source.clear();
        Ok(())
    }

    pub fn excluded_apps(&self) -> Vec<String> {
        self.resolver.application_index.excluded()
    }

    pub fn set_capture_sources(
        &self,
        finder: bool,
        selected_text: bool,
        browser_page: bool,
        clipboard: bool,
    ) {
        self.context_capture
            .set_sources(finder, selected_text, browser_page, clipboard, || {
                // Policy and execution revocation share the capture -> session -> web lock order.
                let mut session = self.resolver.session.access_for_revoke();
                self.resolver.session.advance_execution();
                session.execution = Default::default();
                let mut web = crate::core::sync::lock_for_revoke(&self.web_plugins, "web_plugins");
                self.revoke_tool_windows();
                web.invalidate();
            });
    }

    pub fn set_excluded_apps(&self, ids: Vec<String>) {
        match self.resolver.application_index.set_excluded(ids) {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                tracing::warn!(%error, "application_exclusions_update_failed");
                return;
            }
        }
        if let Ok(session) = self.session() {
            let session_id = session.state.session_id;
            drop(session);
            if session_id == 0 {
                return;
            }
            if self.resolver.recompute_current(session_id).is_none() {
                tracing::warn!(session_id, "excluded_apps_refresh_failed");
            }
        }
    }

    /// True once clipboard, git availability and history have been applied or abandoned.
    pub fn supplemental_ready(&self, session_id: SessionId) -> bool {
        self.session()
            .map(|session| session.state.session_id == session_id && !session.supplemental_pending)
            .unwrap_or(false)
    }

    /// Block until a newer snapshot for `session_id` is committed.
    pub async fn wait_for_snapshot(&self, session_id: SessionId, after: u64) -> SnapshotWait {
        if !self.resolver.is_session_active(session_id) {
            return SnapshotWait::Superseded;
        }
        if let Some(state) = self.beyond(session_id, after) {
            return SnapshotWait::Updated(Box::new(state));
        }
        let mut updates = self.state_updates();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            if !self.resolver.is_session_active(session_id) {
                return SnapshotWait::Superseded;
            }
            if let Some(state) = self.beyond(session_id, after) {
                return SnapshotWait::Updated(Box::new(state));
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return SnapshotWait::TimedOut;
            }
            match tokio::time::timeout(remaining, updates.changed()).await {
                Ok(Ok(())) => continue,
                Ok(Err(_)) => return SnapshotWait::Closed,
                Err(_) => return SnapshotWait::TimedOut,
            }
        }
    }

    fn beyond(&self, session_id: SessionId, after: u64) -> Option<LauncherState> {
        self.latest_state(session_id)
            .filter(|state| snapshot_cursor(state) > after)
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
        self.resolver.application_index.ready().await;
    }

    /// Signalled whenever the application index is published.
    ///
    /// The scan starts at activation, so an app search typed within the first ~60ms would
    /// otherwise match an empty index and no later input would revisit it. The bridge
    /// re-commits the current snapshot when this fires.
    pub fn index_updates(&self) -> watch::Receiver<u64> {
        self.resolver.application_index.updates()
    }

    /// Additional product sources use the same matching, sorting and publication path.
    pub fn register_candidate_source(
        &mut self,
        source: crate::core::search::candidate_source::CandidateSourceRef,
    ) {
        self.resolver.catalog.register_source(source);
    }

    pub fn register_async_candidate_source(
        &mut self,
        source: Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>,
    ) -> Result<()> {
        self.resolver.catalog.register_async_source(source)
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
            .access()
            .map_err(|_| anyhow::anyhow!("session lock poisoned"))
    }

    /// Re-publish the current snapshot once the activation's application scan finishes.
    ///
    /// The scan starts at activation and takes ~60ms on a real machine, so a search typed
    /// immediately after opening would match an empty index: the first snapshot never
    /// waits for the scan, and only a new input would otherwise revisit that revision.
    ///
    /// The rebuild happens after the revision settles. Publishing it earlier would emit a
    /// snapshot that is not final with no late context behind it, and hosts treat a new
    /// `snapshot_seq` as "there is more to show now".
    pub(super) fn spawn_index_recompute(&self, session_id: SessionId, generation: u64) {
        let resolver = self.resolver.clone();
        let mut index = self.index_updates();
        let mut states = self.state_updates();
        tokio::spawn(async move {
            while *index.borrow_and_update() < generation {
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
        match self.resolver.session.access() {
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

    pub(super) async fn publish_resolution(
        &self,
        session_id: SessionId,
        revision: Revision,
        run: crate::core::context::engine::ContextGraphRun,
        start: Instant,
        mut diagnostics: Vec<String>,
    ) -> Result<Option<LauncherState>> {
        let Some(resolution) = self
            .resolver
            .resolve_until_with_grace(session_id, run, self.budget, self.content_grace, start)
            .await
        else {
            return Ok(None);
        };
        let complete_by_budget = resolution.complete;
        let late_run = resolution.run;
        diagnostics.extend(resolution.diagnostics);
        let committed = self.resolver.commit(
            session_id,
            revision,
            resolution.contexts,
            diagnostics.clone(),
            complete_by_budget,
            start,
        )?;
        if !complete_by_budget {
            self.spawn_completion(session_id, revision, late_run, start, diagnostics);
        }
        Ok(committed)
    }

    /// Raw premultiplied BGRA8 pixels for an application candidate icon, avoiding PNG round-trips.
    pub fn application_icon_bgra(
        &self,
        candidate_id: &str,
    ) -> Result<Option<anywhere_platform::RawIconImage>> {
        let path = {
            let session = self.session()?;
            session
                .execution_candidate(candidate_id)
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
        self.platform.application_icon_bgra(&path, ICON_PIXELS)
    }

    /// Raw premultiplied BGRA8 pixels for a file/directory candidate icon.
    pub fn file_icon_bgra(
        &self,
        candidate_id: &str,
    ) -> Result<Option<anywhere_platform::RawIconImage>> {
        let path = {
            let session = self.session()?;
            session
                .execution_candidate(candidate_id)
                .and_then(|candidate| candidate.context())
                .and_then(|context| context.value.path())
                .map(|path| path.to_string_lossy().into_owned())
        };
        let Some(path) = path else {
            return Ok(None);
        };
        self.platform.file_icon_bgra(&path, ICON_PIXELS)
    }
}

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
        self.update_query_use_case(session_id, input, revision, None)
            .await
    }

    /// See `execute.rs`.
    async fn execute(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
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
                .execution_candidate(candidate_id)
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

    fn deactivate(&self, session_id: SessionId) -> Result<()> {
        let mut session = self.resolver.session.access_for_revoke();
        if session.state.session_id != session_id || self.resolver.is_abandoned() {
            return Ok(());
        }
        let mut web = crate::core::sync::lock_for_revoke(&self.web_plugins, "web_plugins");
        web.invalidate();
        drop(web);
        self.resolver.session.advance_execution();
        session.invalidate_execution();
        self.resolver.abandon_session();
        let scan = self.file_search_source.take_cancelled_scan();
        drop(session);
        if let (Some(scan), Ok(runtime)) = (scan, tokio::runtime::Handle::try_current()) {
            // Dropping the last scan reference can unlink its temporary workspace.
            runtime.spawn_blocking(move || drop(scan));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotWait {
    Updated(Box<LauncherState>),
    TimedOut,
    Closed,
    Superseded,
}

fn snapshot_cursor(state: &LauncherState) -> u64 {
    (u64::from(state.revision) << 32) | u64::from(state.snapshot_seq)
}
