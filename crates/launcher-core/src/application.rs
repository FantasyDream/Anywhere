use crate::{
    command::CommandRegistry,
    context::{ContextValue, ResolvedContext},
    execution,
    history::UsageHistory,
    input::InputEngine,
    provider::{
        stamp, ContextGraph, ContextProvider, CurrentDirectoryProvider, ExecutionContext,
        FrontmostApplicationProvider,
    },
    ranking::{rank, RankedAction},
};
use anyhow::{bail, Context, Result};
use launcher_platform::Platform;
use launcher_protocol::*;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
use tracing::Instrument;

#[async_trait::async_trait]
pub trait LauncherApplication: Send + Sync {
    async fn activate(&self) -> Result<Option<LauncherState>>;
    async fn update_input(
        &self,
        session_id: SessionId,
        input: String,
        revision: Revision,
    ) -> Result<Option<LauncherState>>;
    async fn execute(&self, session_id: SessionId, action_id: ActionId) -> Result<ActionResult>;
    fn subscribe(&self, session_id: SessionId) -> SessionSubscription;
}

#[derive(Default)]
struct Session {
    state: LauncherState,
    ambient: Vec<ResolvedContext>,
    source_app: String,
    ranked: Vec<RankedAction>,
}

pub struct Application {
    platform: Arc<dyn Platform>,
    registry: Arc<CommandRegistry>,
    graph: ContextGraph,
    history: Arc<Mutex<Box<dyn UsageHistory>>>,
    // ponytail: one short state lock for a single launcher window; no OS/await work under it.
    session: Mutex<Session>,
    current: Arc<AtomicU64>,
    events: broadcast::Sender<LauncherEvent>,
    startup_diagnostics: Vec<String>,
}

impl Application {
    pub fn new(
        platform: Arc<dyn Platform>,
        registry: CommandRegistry,
        history: Box<dyn UsageHistory>,
        startup_diagnostics: Vec<String>,
    ) -> Self {
        let mut graph = ContextGraph::default();
        graph
            .register(Arc::new(CurrentDirectoryProvider::new(platform.clone())))
            .expect("unique built-in provider");
        Self {
            platform,
            registry: Arc::new(registry),
            graph,
            history: Arc::new(Mutex::new(history)),
            session: Mutex::new(Session::default()),
            current: Arc::new(AtomicU64::new(0)),
            events: broadcast::channel(256).0,
            startup_diagnostics,
        }
    }

    pub fn register_provider(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        self.graph.register(provider)
    }

    fn execution(&self, session_id: SessionId, revision: Revision) -> ExecutionContext {
        ExecutionContext {
            session_id,
            revision,
            current: self.current.clone(),
            events: self.events.clone(),
        }
    }

    async fn finish(
        &self,
        execution: &ExecutionContext,
        input: String,
        seeds: Vec<ResolvedContext>,
        mut diagnostics: Vec<String>,
        activation: Option<Instant>,
    ) -> Result<Option<LauncherState>> {
        let (contexts, provider_diagnostics) = self.graph.enrich(seeds, execution).await;
        if !execution.is_current() {
            return Ok(None);
        }
        if let Some(start) = activation {
            tracing::info!(
                duration_us = start.elapsed().as_micros() as u64,
                "activation_context_captured"
            );
        }
        diagnostics.extend(provider_diagnostics);
        let source_app = contexts
            .iter()
            .find_map(|c| match &c.value {
                ContextValue::Application { bundle_id, .. } => Some(bundle_id.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let history = self.history.clone();
        let history_app = source_app.clone();
        let usage = tokio::task::spawn_blocking(move || {
            history
                .lock()
                .map_err(|_| anyhow::anyhow!("history lock poisoned"))?
                .load(&history_app)
        })
        .await?;
        let usage = match usage {
            Ok(usage) => usage,
            Err(error) => {
                diagnostics.push(format!("Usage history unavailable: {error:#}"));
                Default::default()
            }
        };
        if !execution.is_current() {
            return Ok(None);
        }
        // The ambient snapshot is retained for clearing input and for history, but
        // non-empty input exclusively supplies the contexts offered for execution.
        let active_contexts: Vec<_> = contexts
            .iter()
            .filter(|context| input.trim().is_empty() || context.source == ContextSource::UserInput)
            .cloned()
            .collect();
        let ranked = rank(&self.registry, &active_contexts, now(), |command, kind| {
            usage
                .get(&(command.into(), kind.into()))
                .copied()
                .unwrap_or_default()
        });
        let state = LauncherState {
            session_id: execution.session_id,
            revision: execution.revision,
            input,
            context_summary: active_contexts
                .iter()
                .map(ResolvedContext::presentation)
                .collect(),
            actions: ranked.iter().map(|a| a.presentation.clone()).collect(),
            diagnostics,
        };
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
        if !execution.is_current() {
            return Ok(None);
        }
        if activation.is_some() {
            session.ambient = contexts;
        }
        session.ranked = ranked;
        session.source_app = source_app;
        session.state = state.clone();
        execution.emit(LauncherEvent::ActionsUpdated {
            state: state.clone(),
        });
        Ok(Some(state))
    }

    /// Invalidates work when Esc closes a session, even if a provider is still resolving.
    pub fn deactivate(&self, session_id: SessionId) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
        if session.state.session_id == session_id {
            self.current.store(0, Ordering::Release);
            session.ranked.clear();
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl LauncherApplication for Application {
    async fn activate(&self) -> Result<Option<LauncherState>> {
        let start = Instant::now();
        let execution = {
            let mut session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
            let session_id = session
                .state
                .session_id
                .checked_add(1)
                .context("session id exhausted; restart launcher")?;
            *session = Session {
                state: LauncherState {
                    session_id,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.current.store(stamp(session_id, 0), Ordering::Release);
            self.execution(session_id, 0)
        };
        async {
            tracing::info!("session_created");
            let mut diagnostics = self.startup_diagnostics.clone();
            let ambient = match FrontmostApplicationProvider::capture(self.platform.clone()).await {
                Ok(ambient) => ambient,
                Err(error) => {
                    diagnostics.push(format!("Frontmost application: {error:#}"));
                    vec![]
                }
            };
            let state = self
                .finish(&execution, String::new(), ambient, diagnostics, Some(start))
                .await?;
            tracing::info!(
                duration_us = start.elapsed().as_micros() as u64,
                "activation_actions_ready"
            );
            Ok(state)
        }
        .instrument(tracing::info_span!(
            "activate",
            session_id = execution.session_id,
            revision = 0
        ))
        .await
    }

    async fn update_input(
        &self,
        session_id: SessionId,
        input: String,
        revision: Revision,
    ) -> Result<Option<LauncherState>> {
        // Bound work and bridge allocations from an untrusted UI boundary.
        if input.len() > 1024 * 1024 {
            bail!("input exceeds 1 MiB");
        }
        let start = Instant::now();
        let (mut seeds, execution, diagnostics) = {
            let mut session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
            if session.state.session_id != session_id
                || revision <= session.state.revision
                || self.current.load(Ordering::Acquire) == 0
            {
                return Ok(None);
            }
            session.state.revision = revision;
            session.state.input = input.clone();
            session.ranked.clear();
            self.current
                .store(stamp(session_id, revision), Ordering::Release);
            (
                session.ambient.clone(),
                self.execution(session_id, revision),
                self.startup_diagnostics.clone(),
            )
        };
        async {
            tracing::info!("input_revision_changed");
            let raw = input.clone();
            seeds.extend(tokio::task::spawn_blocking(move || InputEngine.resolve(&raw)).await?);
            tracing::info!(
                duration_us = start.elapsed().as_micros() as u64,
                "input_context_resolved"
            );
            let state = self
                .finish(&execution, input, seeds, diagnostics, None)
                .await?;
            tracing::info!(
                duration_us = start.elapsed().as_micros() as u64,
                "input_actions_ready"
            );
            Ok(state)
        }
        .instrument(tracing::info_span!("update_input", session_id, revision))
        .await
    }

    async fn execute(&self, session_id: SessionId, action_id: ActionId) -> Result<ActionResult> {
        let (action, source_app, revision) = {
            let session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
            if session.state.session_id != session_id
                || self.current.load(Ordering::Acquire) != stamp(session_id, session.state.revision)
            {
                bail!("stale session");
            }
            let action = session
                .ranked
                .iter()
                .find(|action| action.presentation.id == action_id)
                .context("action expired; choose a current action")?
                .clone();
            (action, session.source_app.clone(), session.state.revision)
        };
        let platform = self.platform.clone();
        let registry = self.registry.clone();
        let history = self.history.clone();
        let execution_ticket = self.execution(session_id, revision);
        let command_id = action.command.id.clone();
        tracing::info!(session_id, revision, command_id, "action_execution_started");
        let result = tokio::task::spawn_blocking(move || {
            if !execution_ticket.is_current() {
                bail!("action expired before execution");
            }
            let output = execution::execute(
                &action.command.executor,
                &action.context,
                platform.as_ref(),
                &registry,
            )?;
            let history_result = history
                .lock()
                .map_err(|_| anyhow::anyhow!("history lock poisoned"))
                .and_then(|mut history| {
                    history.record(
                        &action.command.id,
                        &action.context.type_id,
                        &source_app,
                        now(),
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
                    tracing::error!(session_id, revision, command_id, error = %error, "usage_record_failed");
                }
                tracing::info!(
                    session_id,
                    revision,
                    command_id,
                    "action_execution_completed"
                );
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
                tracing::error!(session_id, revision, command_id, error = %error, "action_execution_failed");
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

    fn subscribe(&self, session_id: SessionId) -> SessionSubscription {
        SessionSubscription {
            receiver: self.events.subscribe(),
            session_id,
            current: self.current.clone(),
        }
    }
}

pub struct SessionSubscription {
    receiver: broadcast::Receiver<LauncherEvent>,
    session_id: SessionId,
    current: Arc<AtomicU64>,
}
impl SessionSubscription {
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
                && self.current.load(Ordering::Acquire) == stamp(session_id, revision)
            {
                return Ok(event);
            }
        }
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}
