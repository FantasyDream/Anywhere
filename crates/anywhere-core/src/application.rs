//! Launcher facade: activation capture, request lifecycle, execution, subscription.
use crate::{
    applications, command::CommandRegistry, execution, history::UsageHistory,
    provider::DirectoryMemo, resolver::Resolver,
};
use anyhow::{bail, Context, Result};
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;
use tracing::Instrument;

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
const MAX_INPUT_BYTES: usize = 1024 * 1024;
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
    platform: Arc<dyn Platform>,
    /// The engine, shared with the background tasks that publish late results.
    pub resolver: Resolver,
    startup_diagnostics: Vec<String>,
    budget: Duration,
    content_grace: Duration,
    catalog_tx: watch::Sender<bool>,
    /// The application index is one scan per process; an activation must not redo it.
    catalog_started: Arc<std::sync::atomic::AtomicBool>,
}

impl Application {
    pub fn new(
        platform: Arc<dyn Platform>,
        registry: CommandRegistry,
        history: Box<dyn UsageHistory>,
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
        registry: CommandRegistry,
        history: Box<dyn UsageHistory>,
        startup_diagnostics: Vec<String>,
        budget: Duration,
    ) -> Self {
        let mut resolver = Resolver::new(platform.clone(), registry, history);
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
        self.resolver
            .session
            .lock()
            .ok()
            .filter(|session| session.state.session_id == session_id)
            .map(|session| session.state.clone())
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

    /// Extra providers are stacked on the built-ins by priority then id.
    pub fn register_provider(
        &mut self,
        provider: Arc<dyn crate::provider::ContextProvider>,
    ) -> Result<()> {
        self.resolver.register_provider(provider)
    }

    fn session(&self) -> Result<std::sync::MutexGuard<'_, crate::resolver::Session>> {
        self.resolver
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("session lock poisoned"))
    }

    /// One-shot background application scan; never blocks the first snapshot.
    ///
    /// Once per process, not once per activation: the applications on this machine do not
    /// change between two openings of the launcher, and re-walking every bundle root on
    /// each activation bought nothing. The catalog stays in the resolver, so a later
    /// activation already has it.
    fn spawn_catalog_scan(&self) {
        if self
            .catalog_started
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        let platform = self.platform.clone();
        let roots = applications::standard_roots(dirs_home().as_deref());
        let catalog = self.resolver.catalog.clone();
        let tx = self.catalog_tx.clone();
        tokio::spawn(async move {
            let scanned = applications::scan_async(platform, roots).await;
            let count = scanned.applications().len();
            if let Ok(mut handle) = catalog.lock() {
                *handle = scanned;
            }
            let _ = tx.send(true);
            // The next input revision or late result publishes the searched applications.
            tracing::info!(count, "app_index_published");
        });
    }

    /// Re-publish the current snapshot once the application index appears.
    ///
    /// The scan starts at activation and takes ~60ms on a real machine, so a search typed
    /// immediately after opening would match an empty index: the first snapshot never
    /// waits for the scan, and only a new input would otherwise revisit that revision.
    /// Only the first opening per process has to wait; later activations already have the
    /// catalog.
    ///
    /// The rebuild happens after the revision settles. Publishing it earlier would emit a
    /// snapshot that is not final with no late context behind it, and hosts treat a new
    /// `snapshot_seq` as "there is more to show now".
    fn spawn_index_recompute(&self, session_id: SessionId) {
        let resolver = self.resolver.clone();
        let mut index = self.catalog_tx.subscribe();
        let mut states = self.state_updates();
        tokio::spawn(async move {
            if !*index.borrow() && index.changed().await.is_err() {
                return;
            }
            let published = states.borrow().clone();
            let settled = published.is_some_and(|state| {
                state.session_id == session_id
                    && Some(state.revision) == resolver.committed_revision()
                    && state.context_complete
            });
            if !settled {
                // Wait for the late pass to finish this revision; its own commit will
                // publish the catalog, and this loop only needs to catch a revision that
                // settled before the scan did.
                loop {
                    if states.changed().await.is_err() {
                        return;
                    }
                    let Some(state) = states.borrow().clone() else {
                        continue;
                    };
                    if state.session_id != session_id {
                        return;
                    }
                    if state.context_complete {
                        break;
                    }
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
    fn spawn_completion(
        &self,
        session_id: SessionId,
        revision: Revision,
        run: crate::provider::ContextGraphRun,
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
    fn source_app_of(frontmost: Option<&ApplicationInfo>) -> String {
        frontmost
            .map(|application| application.bundle_id.clone())
            .unwrap_or_default()
    }

    fn load_usage(&self, source_app: &str) -> (UsageMap, UsageMap) {
        let history = self.resolver.history.clone();
        let source_app = source_app.to_owned();
        let scoped = history
            .lock()
            .map_err(|_| anyhow::anyhow!("history lock poisoned"))
            .and_then(|history| history.load(&source_app));
        let global = history
            .lock()
            .map_err(|_| anyhow::anyhow!("history lock poisoned"))
            .and_then(|history| history.load_global());
        (scoped.unwrap_or_default(), global.unwrap_or_default())
    }
}

type UsageMap = std::collections::HashMap<(String, String), crate::ranking::UsageStats>;

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[async_trait::async_trait]
impl LauncherApplication for Application {
    async fn activate(&self) -> Result<Option<LauncherState>> {
        let start = Instant::now();
        // The original foreground application is captured before the host shows anything.
        let platform = self.platform.clone();
        let frontmost = tokio::task::spawn_blocking(move || platform.frontmost_application())
            .await
            .context("foreground capture task failed")?
            .unwrap_or(None);
        let mut diagnostics = self.startup_diagnostics.clone();
        let git_available = {
            let platform = self.platform.clone();
            tokio::task::spawn_blocking(move || platform.git_available())
                .await
                .unwrap_or(false)
        };
        let clipboard = match tokio::task::spawn_blocking({
            let platform = self.platform.clone();
            move || platform.clipboard_text()
        })
        .await
        {
            Ok(Ok(text)) => text,
            Ok(Err(error)) => {
                diagnostics.push(format!("Clipboard snapshot: {error:#}"));
                None
            }
            Err(error) => {
                diagnostics.push(format!("Clipboard snapshot task: {error}"));
                None
            }
        };
        let source_app = Self::source_app_of(frontmost.as_ref());
        let session_id = {
            let mut session = self.session()?;
            let session_id = session
                .state
                .session_id
                .checked_add(1)
                .context("session id exhausted; restart launcher")?;
            let directory = frontmost
                .as_ref()
                .map(|frontmost| DirectoryMemo::start(self.platform.clone(), frontmost.clone()));
            *session = crate::resolver::Session {
                frontmost: frontmost.clone(),
                directory,
                clipboard,
                git_available,
                state: LauncherState {
                    session_id,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.resolver.current.store(
                crate::provider::stamp(session_id, 0),
                std::sync::atomic::Ordering::Release,
            );
            session_id
        };
        tracing::info!(
            session_id,
            duration_us = start.elapsed().as_micros() as u64,
            "activation_context_captured"
        );
        self.spawn_catalog_scan();
        let (usage, app_usage) = self.load_usage(&source_app);
        if let Ok(mut session) = self.resolver.session.lock() {
            if session.state.session_id == session_id {
                session.usage = usage;
                session.app_usage = app_usage;
                // Same value the commit path would derive, decided once here.
                session.source_app = source_app;
            }
        }
        let execution = self.resolver.execution(session_id, 0);
        let Some(seeds) = self.resolver.seeds_for(session_id, "") else {
            return Ok(None);
        };
        let run = self.resolver.build_run(&execution, seeds);
        let state = async {
            tracing::info!("session_created");
            let Some(resolution) = self
                .resolver
                .resolve_until_with_grace(session_id, run, self.budget, self.content_grace, start)
                .await
            else {
                return Ok(None);
            };
            let complete_by_budget = resolution.complete;
            let run = resolution.run;
            let mut diagnostics = diagnostics;
            diagnostics.extend(resolution.diagnostics);
            let committed = self.resolver.commit(
                session_id,
                0,
                resolution.contexts,
                diagnostics.clone(),
                complete_by_budget,
                start,
            )?;
            if committed.is_some() {
                tracing::info!(
                    session_id,
                    duration_us = start.elapsed().as_micros() as u64,
                    "activation_actions_ready"
                );
            }
            if !complete_by_budget {
                self.spawn_completion(session_id, 0, run, start, diagnostics);
            }
            Ok(committed)
        }
        .instrument(tracing::info_span!("activate", session_id, revision = 0))
        .await;
        self.spawn_index_recompute(session_id);
        state
    }

    async fn update_input(
        &self,
        session_id: SessionId,
        input: String,
        revision: Revision,
    ) -> Result<Option<LauncherState>> {
        if input.len() > MAX_INPUT_BYTES {
            bail!("input exceeds 1 MiB");
        }
        let start = Instant::now();
        let execution = {
            let mut session = self.session()?;
            if session.state.session_id != session_id
                || revision <= session.state.revision
                || self
                    .resolver
                    .current
                    .load(std::sync::atomic::Ordering::Acquire)
                    == 0
            {
                return Ok(None);
            }
            // A superseded request never publishes again.
            session.state.revision = revision;
            session.state.input = input.clone();
            session.state.context_complete = false;
            session.committed_revision = None;
            session.snapshot_seq = 0;
            session.published_contexts = 0;
            self.resolver.current.store(
                crate::provider::stamp(session_id, revision),
                std::sync::atomic::Ordering::Release,
            );
            // Build seeds from the guard we already hold; `seeds_for` would deadlock here.
            crate::resolver::seeds_from(&session, &input)
        };
        tracing::info!("input_revision_changed");
        tracing::info!(
            session_id,
            revision,
            duration_us = start.elapsed().as_micros() as u64,
            contexts = execution.len(),
            "input_context_resolved"
        );
        let execution_context = self.resolver.execution(session_id, revision);
        let run = self.resolver.build_run(&execution_context, execution);
        let state = async {
            let Some(resolution) = self
                .resolver
                .resolve_until_with_grace(session_id, run, self.budget, self.content_grace, start)
                .await
            else {
                return Ok(None);
            };
            let complete_by_budget = resolution.complete;
            let late_run = resolution.run;
            let mut diagnostics = self.startup_diagnostics.clone();
            diagnostics.extend(resolution.diagnostics);
            let committed = self.resolver.commit(
                session_id,
                revision,
                resolution.contexts,
                diagnostics.clone(),
                complete_by_budget,
                start,
            )?;
            if committed.is_some() {
                tracing::info!(
                    session_id,
                    revision,
                    duration_us = start.elapsed().as_micros() as u64,
                    "input_actions_ready"
                );
            }
            if !complete_by_budget {
                self.spawn_completion(session_id, revision, late_run, start, diagnostics);
            }
            Ok(committed)
        }
        .instrument(tracing::info_span!("update_input", session_id, revision))
        .await;
        state
    }

    async fn execute(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: ActionId,
    ) -> Result<ActionResult> {
        let (candidate, source_app) = {
            let session = self.session()?;
            if session.state.session_id != session_id
                || self
                    .resolver
                    .current
                    .load(std::sync::atomic::Ordering::Acquire)
                    != crate::provider::stamp(session_id, session.state.revision)
                || session.state.revision != revision
            {
                bail!("stale session");
            }
            // The revision is current, but its candidates may not be published yet:
            // `update_input` makes the new revision current before it resolves, so the
            // map still holds the previous revision's candidates. They carry the previous
            // context and execution target, so serving them here would execute an action
            // that belongs to an input the user has already replaced.
            if session.candidates_revision != Some(revision) {
                bail!("action expired; choose a current action");
            }
            let candidate = session
                .state_candidates
                .get(&candidate_id)
                .cloned()
                .context("action expired; choose a current action")?;
            if !candidate.presentation.availability.is_executable() {
                bail!(
                    "{}",
                    candidate
                        .presentation
                        .availability
                        .reason
                        .clone()
                        .unwrap_or_else(|| "action is not available".into())
                );
            }
            (candidate, session.source_app.clone())
        };
        let platform = self.platform.clone();
        let registry = self.resolver.registry.clone();
        let history = self.resolver.history.clone();
        let record_id = candidate
            .presentation
            .command_id
            .clone()
            .or_else(|| {
                candidate
                    .presentation
                    .bundle_id
                    .as_ref()
                    .map(|bundle| crate::history::application_usage_id(bundle))
            })
            .context("candidate has no executable identity")?;
        let context_type = candidate
            .context
            .as_ref()
            .map(|context| context.type_id.clone())
            .unwrap_or_else(|| crate::history::APPLICATION_CONTEXT_TYPE.to_owned());
        tracing::info!(
            session_id,
            revision,
            candidate_id,
            command_id = record_id,
            "action_execution_started"
        );
        let result = tokio::task::spawn_blocking(move || {
            let output = match (
                &candidate.command,
                &candidate.context,
                &candidate.application,
            ) {
                (Some(command), Some(context), _) => {
                    execution::execute(&command.executor, context, platform.as_ref(), &registry)?
                }
                (None, None, Some(application)) => {
                    if !platform.launch_application(&application.path)? {
                        bail!("could not launch {}", application.path);
                    }
                    execution::ExecutionOutput {
                        text: None,
                        close_window: true,
                    }
                }
                _ => bail!("candidate cannot be executed"),
            };
            // No currency re-check here. The side effect has already happened, so
            // refusing at this point cannot prevent anything; it only loses the usage
            // record. Every action that closes the window ends its own session (the host
            // dismisses on focus loss), so a re-check would fail exactly the successful
            // executions whose frequency the ranking needs, and it would report
            // "expired" for an action the user watched succeed. Currency is enforced
            // before the side effect, in `execute`'s opening gate.
            let history_result = history
                .lock()
                .map_err(|_| anyhow::anyhow!("history lock poisoned"))
                .and_then(|mut history| {
                    history.record(
                        &record_id,
                        &context_type,
                        &source_app,
                        crate::resolver::now(),
                    )
                });
            Ok::<_, anyhow::Error>((output, history_result))
        })
        .await?;
        match result {
            Ok((output, history_result)) => {
                let mut message = "Done".to_owned();
                let mut close_window = output.close_window;
                if let Err(error) = history_result {
                    message = format!("Action completed, but usage could not be saved: {error:#}");
                    close_window = false;
                    tracing::error!(session_id, revision, error = %error, "usage_record_failed");
                }
                tracing::info!(session_id, revision, "action_execution_completed");
                Ok(ActionResult {
                    session_id,
                    revision,
                    success: true,
                    message,
                    output_text: output.text,
                    close_window,
                })
            }
            Err(error) => {
                tracing::error!(session_id, revision, error = %error, "action_execution_failed");
                Ok(ActionResult {
                    session_id,
                    revision,
                    success: false,
                    message: format!("{error:#}"),
                    output_text: None,
                    close_window: false,
                })
            }
        }
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
                .and_then(|candidate| candidate.application.as_ref())
                .map(|application| application.icon_source().to_owned())
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
                    == crate::provider::stamp(session_id, revision)
            {
                return Ok(event);
            }
        }
    }
}
