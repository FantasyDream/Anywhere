//! The context engine: a sequential walk from the seeds to everything they imply.
//!
//! Seeds are inserted first; each step picks the highest-priority provider that accepts the
//! context at the cursor and has not run for it yet, runs that one call, and folds the
//! result in. At most one call is ever in flight, which is why a caller can stop waiting at
//! any moment and resume the same run later without repeating work: the call is detached
//! into its own task and the run keeps the receiver.
//!
//! Deduplication lives here rather than in providers. `(provider, context, session,
//! revision)` is recorded before the call starts, so a provider is never asked twice for
//! work it has already begun, and a context that arrives twice by different routes is kept
//! once.
use crate::core::context::provider::{ContextProvider, StepOutcome};
use crate::core::context::ResolvedContext;

use anywhere_protocol::*;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, oneshot};

/// How deep a provider chain may usefully go.
pub const MAX_DEPTH: u8 = 3;
/// Upper bound on contexts in one run, so a pathological chain cannot grow without limit.
pub const MAX_CONTEXTS: usize = 128;

#[derive(Clone)]
pub struct ExecutionContext {
    pub session_id: SessionId,
    pub revision: Revision,
    pub(crate) current: Arc<AtomicU64>,
    pub(crate) events: broadcast::Sender<LauncherEvent>,
}

pub(crate) fn stamp(session: SessionId, revision: Revision) -> u64 {
    ((session as u64) << 32) | revision as u64
}

impl ExecutionContext {
    /// An execution context for one session and revision, owned by the caller.
    ///
    /// For callers that drive the graph themselves with their own provider list — tools and
    /// tests. The launcher service gets its contexts from the resolver instead, which is what
    /// makes supersession observable across revisions.
    pub fn detached(session_id: SessionId, revision: Revision) -> Self {
        let (events, _) = broadcast::channel(16);
        Self {
            session_id,
            revision,
            current: Arc::new(AtomicU64::new(stamp(session_id, revision))),
            events,
        }
    }

    pub fn is_current(&self) -> bool {
        self.current.load(Ordering::Acquire) == stamp(self.session_id, self.revision)
    }

    /// Resolves once this session or revision is superseded.
    ///
    /// Polls the stamp rather than listening for an event: supersession is a single atomic
    /// store with no notification behind it, and this is only ever awaited alongside the
    /// work it protects.
    pub async fn superseded(&self) {
        while self.is_current() {
            tokio::time::sleep(SUPERSEDED_POLL).await;
        }
    }

    pub(crate) fn emit(&self, event: LauncherEvent) {
        if self.is_current() {
            let _ = self.events.send(event);
        }
    }
}

/// How often a cancelled wait re-checks the execution stamp.
const SUPERSEDED_POLL: Duration = Duration::from_millis(10);

/// Starts a run. The providers themselves belong to the [`ContextProviderRegistry`], which
/// is what decides their order; a run is only ever handed the list it should walk.
///
/// [`ContextProviderRegistry`]: crate::core::context::registry::ContextProviderRegistry
#[derive(Default)]
pub struct ContextGraph;

impl ContextGraph {
    /// Start a run for one revision. Sequential: at most one provider call is in flight.
    ///
    /// Supersession is detected through the execution stamp, so no separate abort
    /// signal exists: there is nothing to cancel a blocking provider with.
    pub fn start(seeds: Vec<ResolvedContext>, execution: &ExecutionContext) -> ContextGraphRun {
        let mut run = ContextGraphRun {
            execution: execution.clone(),
            contexts: crate::core::context::ContextSet::new(),
            visited: HashSet::new(),
            diagnostics: Vec::new(),
            cursor: 0,
            pending: None,
        };
        for seed in seeds {
            run.insert(seed);
        }
        run
    }
}

/// Run one provider call to completion, detached from whoever is waiting for it.
///
/// The result is sent whether or not anyone is still waiting, so the work is never lost and
/// never repeated.
async fn run_provider(
    provider: Arc<dyn ContextProvider>,
    input: ResolvedContext,
    execution: ExecutionContext,
    outcome: oneshot::Sender<StepOutcome>,
) {
    let result = if !execution.is_current() {
        StepOutcome::Abandoned
    } else {
        tracing::info!(
            session_id = execution.session_id,
            revision = execution.revision,
            provider_id = provider.descriptor().id,
            source_context_id = input.id,
            "provider_started"
        );
        let started = std::time::Instant::now();
        StepOutcome::Completed {
            result: provider.resolve(&input, &execution).await,
            duration_us: started.elapsed().as_micros() as u64,
        }
    };
    // A closed receiver only means nobody is waiting any more.
    let _ = outcome.send(result);
}

/// Wait for a detached provider call under the first-snapshot time policy.
///
/// `None` means the deadline (and grace) expired first. The call keeps running: the
/// receiver stays in the run, and the late pass awaits it with no deadline at all.
///
/// `started` is the beginning of the resolution this wait belongs to, recorded so the
/// budget can be reported against the same origin the rest of the timings use.
async fn await_outcome(
    outcome: &mut oneshot::Receiver<StepOutcome>,
    deadline: Option<Instant>,
    grace: Option<Instant>,
    started: Instant,
) -> Option<StepOutcome> {
    let Some(deadline) = deadline else {
        // The late pass waits however long the provider needs.
        return outcome.await.ok();
    };
    let budget = deadline.saturating_duration_since(Instant::now());
    if !budget.is_zero() {
        tokio::select! {
            biased;
            received = &mut *outcome => return received.ok(),
            _ = tokio::time::sleep(budget) => {}
        }
    }
    // The budget is spent, whether or not anything was still pending. Recording it here
    // rather than at publication keeps the field meaning "the budget expired", which is
    // what the acceptance criterion measures; the grace below only extends the *wait*.
    tracing::info!(
        extension_us = grace
            .map(|grace| grace.saturating_duration_since(Instant::now()).as_micros() as u64)
            .unwrap_or(0),
        duration_us = started.elapsed().as_micros() as u64,
        "first_snapshot_budget_reached"
    );
    // The budget is gone. Waiting a little longer is worth it because a frame cut here
    // would publish a half-finished list: on the real machine the budget landed exactly
    // before the Git provider ran.
    let grace = grace
        .map(|grace| grace.saturating_duration_since(Instant::now()))
        .filter(|remaining| !remaining.is_zero())?;
    tracing::info!(
        extension_us = grace.as_micros() as u64,
        "first_snapshot_grace_extended"
    );
    tokio::select! {
        biased;
        received = &mut *outcome => received.ok(),
        _ = tokio::time::sleep(grace) => None,
    }
}

/// One resolution pass over the context graph.
///
/// Deliberately not `Clone`: the run owns the one in-flight provider call, so a budget cut
/// hands that call to the late pass instead of dropping it. Dropping it lost the work the
/// provider had already started and forced a second call for the same
/// `(provider, context, session, revision)`, which is exactly what the execution-dedup rule
/// forbids.
///
/// One unit of pending graph work: a provider call plus the metadata needed to fold its
/// result back into the graph.
pub struct PendingStep {
    pub provider_id: ProviderId,
    pub source_context_id: ContextId,
    pub produces: Vec<ContextTypeId>,
    /// The started call, detached into its own task.
    ///
    /// Detaching is the point: a budget cut stops *waiting*, and dropping a future that is
    /// awaiting a `spawn_blocking` handle would cancel the awaited task and lose the work the
    /// provider had already begun. A detached task keeps running to completion whatever the
    /// caller does, and the receiver is how the late pass collects the same result.
    outcome: oneshot::Receiver<StepOutcome>,
}

pub enum GraphStep {
    /// Poll `run.pending()`, then call `settle` and step again.
    Pending,
    /// The whole graph for this revision finished, or was superseded.
    Done {
        contexts: Vec<ResolvedContext>,
        diagnostics: Vec<String>,
    },
}

pub struct ContextGraphRun {
    pub execution: ExecutionContext,
    pub(crate) contexts: crate::core::context::ContextSet,
    visited: HashSet<(ProviderId, ContextId, SessionId, Revision)>,
    pub(crate) diagnostics: Vec<String>,
    cursor: usize,
    /// The call that has been started and not yet settled, kept across a budget cut.
    pending: Option<PendingStep>,
}

impl ContextGraphRun {
    pub fn contexts(&self) -> &[ResolvedContext] {
        self.contexts.as_slice()
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    fn insert(&mut self, context: ResolvedContext) {
        let kept =
            self.contexts
                .insert(context, self.execution.session_id, self.execution.revision);
        if !kept {
            return;
        }
        let Some(context) = self.contexts.as_slice().last() else {
            return;
        };
        self.execution.emit(LauncherEvent::ContextAdded {
            context: context.descriptor(),
        });
    }

    /// Await the started call, or `None` when it has no pending call.
    ///
    /// Returns `None` again if the caller's deadline expires first: the task keeps running
    /// and a later pass awaits the same receiver. `started` is the origin of the resolution
    /// this wait belongs to, used to report the budget against the same clock.
    pub async fn await_pending(
        &mut self,
        deadline: Option<Instant>,
        grace: Option<Instant>,
        started: Instant,
    ) -> Option<StepOutcome> {
        let pending = self.pending.as_mut()?;
        await_outcome(&mut pending.outcome, deadline, grace, started).await
    }

    /// Whether a started call is still waiting to be polled.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn finished(&mut self) -> GraphStep {
        GraphStep::Done {
            contexts: self.contexts.take(),
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    /// Next provider call, or the terminal state of this run.
    pub fn step(&mut self, providers: &[Arc<dyn ContextProvider>]) -> GraphStep {
        // A started call comes first. It is already running — `await_pending` decides
        // whether to keep waiting for it — so stepping past its input would lose the
        // context it is about to produce and then skip the call as already visited.
        if self.has_pending() {
            return GraphStep::Pending;
        }
        loop {
            if !self.execution.is_current() {
                return self.finished();
            }
            if self.cursor >= self.contexts.len() {
                return self.finished();
            }
            let input = self.contexts.row(self.cursor).clone();
            if input.depth >= MAX_DEPTH {
                self.cursor += 1;
                continue;
            }
            let mut chosen = None;
            for candidate in providers {
                let descriptor = candidate.descriptor();
                if !descriptor.accepts.contains(&input.type_id) {
                    continue;
                }
                let key = (
                    descriptor.id.clone(),
                    input.id.clone(),
                    self.execution.session_id,
                    self.execution.revision,
                );
                if !self.visited.contains(&key) {
                    chosen = Some((
                        candidate.clone(),
                        descriptor.id.clone(),
                        descriptor.produces.clone(),
                        key,
                    ));
                    break;
                }
            }
            let Some((provider, id, produces, key)) = chosen else {
                self.cursor += 1;
                continue;
            };
            // Record the invocation and start the call in one step: the call lives in the
            // run, so it is resumed rather than repeated if this pass is cut short.
            self.visited.insert(key);
            let (tx, outcome) = oneshot::channel();
            let execution = self.execution.clone();
            tokio::spawn(run_provider(provider, input.clone(), execution, tx));
            self.pending = Some(PendingStep {
                provider_id: id,
                source_context_id: input.id.clone(),
                produces,
                outcome,
            });
            return GraphStep::Pending;
        }
    }

    /// Fold one provider outcome into the graph.
    pub fn settle(&mut self, outcome: StepOutcome) {
        let Some(PendingStep {
            provider_id,
            source_context_id,
            produces,
            ..
        }) = self.pending.take()
        else {
            return;
        };
        let StepOutcome::Completed {
            result,
            duration_us,
        } = outcome
        else {
            return;
        };
        // `settle` folds in the call that was made for the context at `cursor`, but between
        // starting the call and settling it the cursor can already have walked past that
        // context — `step` skips a context whose only provider is visited. Finding the input
        // by identity keeps the two in step; the visited marks, not the cursor, are what
        // stop any call from being made twice.
        let Some(position) = self.contexts.position(&source_context_id) else {
            return;
        };
        self.cursor = position;
        let input = self.contexts.row(self.cursor).clone();
        match result {
            Ok(outputs) => {
                for mut output in outputs {
                    if !produces.contains(&output.type_id)
                        || !output.confidence.is_finite()
                        || !(0.0..=1.0).contains(&output.confidence)
                    {
                        self.diagnostics
                            .push(format!("{provider_id} returned an invalid context"));
                        continue;
                    }
                    if self.contexts.len() >= MAX_CONTEXTS {
                        self.diagnostics
                            .push(format!("context graph reached {MAX_CONTEXTS} contexts"));
                        break;
                    }
                    output.provider_id = provider_id.clone();
                    output.depth = input.depth + 1;
                    output.source_context_id = Some(input.id.clone());
                    if input.source == ContextSource::UserInput {
                        output.source = ContextSource::UserInput;
                    }
                    self.insert(output);
                }
                self.execution.emit(LauncherEvent::ProviderCompleted {
                    session_id: self.execution.session_id,
                    revision: self.execution.revision,
                    provider_id: provider_id.clone(),
                    source_context_id: source_context_id.clone(),
                    duration_us,
                });
                tracing::info!(
                    session_id = self.execution.session_id,
                    revision = self.execution.revision,
                    provider_id,
                    provider_duration = duration_us,
                    "provider_completed"
                );
            }
            Err(error) => {
                let message = format!("{provider_id}: {error:#}");
                tracing::error!(
                    session_id = self.execution.session_id,
                    revision = self.execution.revision,
                    provider_id,
                    provider_duration = duration_us,
                    error = %error,
                    "provider_failed"
                );
                self.execution.emit(LauncherEvent::ProviderFailed {
                    session_id: self.execution.session_id,
                    revision: self.execution.revision,
                    provider_id,
                    source_context_id,
                    duration_us,
                    message: message.clone(),
                });
                self.diagnostics.push(message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::provider::ProviderDescriptor;
    use crate::core::context::registry::ContextProviderRegistry;
    use crate::core::context::{ContextValue, ResolvedContext};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicUsize;

    struct Chain {
        descriptor: ProviderDescriptor,
        calls: Arc<AtomicUsize>,
        duplicate: bool,
    }
    #[async_trait]
    impl ContextProvider for Chain {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }
        async fn resolve(
            &self,
            input: &ResolvedContext,
            _: &ExecutionContext,
        ) -> Result<Vec<ResolvedContext>> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let value = if self.duplicate {
                input.value.text()
            } else {
                format!("{}+", input.value.text())
            };
            Ok(vec![ResolvedContext::new(
                TEXT,
                ContextValue::Text(value.into()),
                input.source,
                "chain",
            )])
        }
    }

    pub(crate) fn execution() -> ExecutionContext {
        ExecutionContext {
            session_id: 1,
            revision: 0,
            current: Arc::new(AtomicU64::new(stamp(1, 0))),
            events: broadcast::channel(32).0,
        }
    }

    async fn drain(
        registry: &ContextProviderRegistry,
        execution: &ExecutionContext,
    ) -> (Vec<ResolvedContext>, Vec<String>) {
        let mut run = ContextGraph::start(
            vec![ResolvedContext::new(
                TEXT,
                ContextValue::Text("start".into()),
                ContextSource::UserInput,
                "test",
            )],
            execution,
        );
        loop {
            match run.step(registry.providers()) {
                GraphStep::Pending => {
                    // The run owns the started call; this is the same detached task the
                    // resolver awaits and the late pass would collect.
                    let Some(outcome) = run
                        .await_pending(None, None, std::time::Instant::now())
                        .await
                    else {
                        continue;
                    };
                    run.settle(outcome);
                }
                GraphStep::Done {
                    contexts,
                    diagnostics,
                } => return (contexts, diagnostics),
            }
        }
    }

    #[tokio::test]
    async fn enrichment_caps_depth_deduplicates_and_tracks_provenance() {
        for duplicate in [true, false] {
            let calls = Arc::new(AtomicUsize::new(0));
            let mut registry = ContextProviderRegistry::default();
            registry
                .register(Arc::new(Chain {
                    descriptor: ProviderDescriptor {
                        id: "chain".into(),
                        accepts: vec![TEXT.into()],
                        produces: vec![TEXT.into()],
                        priority: 0,
                    },
                    calls: calls.clone(),
                    duplicate,
                }))
                .unwrap();
            let (contexts, diagnostics) = drain(&registry, &execution()).await;
            assert!(diagnostics.is_empty());
            assert_eq!(contexts.len(), if duplicate { 1 } else { 4 });
            assert_eq!(calls.load(Ordering::Relaxed), if duplicate { 1 } else { 3 });
            assert!(contexts
                .iter()
                .all(|c| c.depth <= 3 && c.session_id == 1 && c.revision == 0));
            if !duplicate {
                assert_eq!(contexts[1].source_context_id.as_deref(), Some("c0"));
            }
        }
    }
}
