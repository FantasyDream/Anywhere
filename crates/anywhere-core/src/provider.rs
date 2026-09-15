//! Context providers and the caller-driven context graph.
//!
//! Providers never know about budgets or timeouts. The caller decides how long to
//! wait, then keeps stepping the run to append late context.
use crate::context::{ContextValue, ResolvedContext};
use crate::input::{text_context, InputEngine};
use anyhow::{bail, Result};
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use async_trait::async_trait;
use std::{
    collections::HashSet,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{broadcast, watch};

pub const MAX_DEPTH: u8 = 3;

/// The performance metrics V0.2 must record.
///
/// The spec names seven metrics; this module lists the exact name each one appears
/// under in the log, so the contract is stated once and a test asserts every one of
/// them is emitted. Most are tracing *fields*; `context_resolution_total` is a log
/// *message* name, because it marks a completion rather than carrying a duration.
///
/// `first_snapshot_duration` is the `first_snapshot_duration_us` field of the first
/// `snapshot_committed` line of a revision (`snapshot_seq == 1`).
///
/// The emission sites spell these names inline: `tracing` requires literal field
/// names, so a site cannot reference a const. The test in `tests/mvp.rs` is what
/// keeps the two ends honest.
pub mod metrics {
    pub const PROVIDER_DURATION: &str = "provider_duration";
    pub const CONTEXT_RESOLUTION_TOTAL: &str = "context_resolution_total";
    pub const FIRST_SNAPSHOT_DURATION: &str = "first_snapshot_duration";
    pub const MATCHER_DURATION: &str = "matcher_duration";
    pub const RANKING_DURATION: &str = "ranking_duration";
    pub const HOTKEY_TO_VISIBLE: &str = "hotkey_to_visible_ms";
    pub const LATE_CONTEXT_COUNT: &str = "late_context_count";

    pub const ALL: [&str; 7] = [
        PROVIDER_DURATION,
        CONTEXT_RESOLUTION_TOTAL,
        FIRST_SNAPSHOT_DURATION,
        MATCHER_DURATION,
        RANKING_DURATION,
        HOTKEY_TO_VISIBLE,
        LATE_CONTEXT_COUNT,
    ];
}
const MAX_CONTEXTS: usize = 128;

#[derive(Debug, Clone)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub accepts: Vec<ContextTypeId>,
    pub produces: Vec<ContextTypeId>,
    pub priority: i32,
}

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
    pub fn is_current(&self) -> bool {
        self.current.load(Ordering::Acquire) == stamp(self.session_id, self.revision)
    }
    pub(crate) fn emit(&self, event: LauncherEvent) {
        if self.is_current() {
            let _ = self.events.send(event);
        }
    }
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;
    /// Resolve `input` into further contexts.
    ///
    /// Must be safe to call again for the same `(input, session, revision)`: the budget
    /// may discard this future mid-flight, and the late pass re-enters the provider so
    /// the work is not lost. Keep any OS work behind a single-flight memo rather than
    /// repeating it per call.
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>>;
}

/// Emits the preserved raw text for any non-empty user input.
pub struct TextProvider {
    descriptor: ProviderDescriptor,
}
impl Default for TextProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.text_provider".into(),
                accepts: vec![],
                produces: vec![TEXT.into()],
                priority: 0,
            },
        }
    }
}
#[async_trait]
impl ContextProvider for TextProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        _: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let ContextValue::Text(raw) = &input.value else {
            return Ok(vec![]);
        };
        Ok(text_context(raw).into_iter().collect())
    }
}

/// Convenience for callers that need the text seed without the full engine.
pub fn text_provider_contexts(raw: &str) -> Vec<ResolvedContext> {
    text_context(raw).into_iter().collect()
}

/// `core.text` to `core.url` / `json.document` / `core.file` / `core.directory`.
pub struct InputTypeProvider {
    descriptor: ProviderDescriptor,
    engine: InputEngine,
}
impl Default for InputTypeProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.input_types".into(),
                accepts: vec![TEXT.into()],
                produces: vec![
                    URL.into(),
                    JSON_DOCUMENT.into(),
                    FILE.into(),
                    DIRECTORY.into(),
                ],
                priority: 0,
            },
            engine: InputEngine,
        }
    }
}
#[async_trait]
impl ContextProvider for InputTypeProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        _: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        if !matches!(input.value, ContextValue::Text(_)) {
            return Ok(vec![]);
        }
        Ok(self.engine.resolve(&input.value.text()))
    }
}

/// Activation root context. Captured by the caller before any host is told to show.
pub fn frontmost_context(application: &ApplicationInfo) -> ResolvedContext {
    ResolvedContext::new(
        APPLICATION,
        ContextValue::Application {
            bundle_id: application.bundle_id.clone(),
            name: application.name.clone(),
        },
        ContextSource::ForegroundApplication,
        "platform.frontmost_application",
    )
}

/// Resolves the original application directory at most once per session.
///
/// Capture starts on first use and is memoized in the session, so a later revision
/// reuses the running task instead of starting a second Finder Automation request.
/// The original application identity is frozen before the host takes focus and is
/// never re-read afterwards.
pub struct CurrentDirectoryProvider {
    descriptor: ProviderDescriptor,
    memo: Arc<DirectoryMemo>,
}
impl CurrentDirectoryProvider {
    pub fn new(memo: Arc<DirectoryMemo>) -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "platform.current_directory".into(),
                accepts: vec![APPLICATION.into()],
                produces: vec![DIRECTORY.into()],
                priority: 0,
            },
            memo,
        }
    }
}
#[async_trait]
impl ContextProvider for CurrentDirectoryProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        _: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let ContextValue::Application { bundle_id, .. } = &input.value else {
            return Ok(vec![]);
        };
        // The memo is bound to the application that actually owned the directory.
        if bundle_id != self.memo.bundle_id() {
            return Ok(vec![]);
        }
        match self.memo.directory().await {
            Ok(Some(directory)) => Ok(vec![ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(directory),
                ContextSource::ApplicationContext,
                &self.descriptor.id,
            )]),
            Ok(None) => Ok(vec![]),
            Err(error) => bail!("{error}"),
        }
    }
}

/// Session-scoped, single-flight capture of one application's current directory.
pub struct DirectoryMemo {
    bundle_id: String,
    rx: watch::Receiver<Option<Result<Option<PathBuf>, String>>>,
    /// Held so the channel stays open for later revisions.
    _tx: watch::Sender<Option<Result<Option<PathBuf>, String>>>,
}

impl DirectoryMemo {
    /// Start the capture immediately. The task is independent of any request, so
    /// early typing cannot lose the directory and no second request can duplicate it.
    pub fn start(platform: Arc<dyn Platform>, frontmost: ApplicationInfo) -> Arc<Self> {
        let (tx, rx) = watch::channel(None);
        let publisher = tx.clone();
        let bundle_id = frontmost.bundle_id.clone();
        let bundle_id_for_log = bundle_id.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            let result =
                tokio::task::spawn_blocking(move || platform.application_directory(&frontmost))
                    .await
                    .unwrap_or_else(|error| Err(anyhow::anyhow!("directory task failed: {error}")))
                    .map_err(|error| format!("{error:#}"));
            tracing::info!(
                bundle_id = bundle_id_for_log,
                duration_us = started.elapsed().as_micros() as u64,
                found = result
                    .as_ref()
                    .map(|value| value.is_some())
                    .unwrap_or(false),
                "directory_capture_finished"
            );
            let _ = publisher.send(Some(result));
        });
        Arc::new(Self {
            bundle_id,
            rx,
            _tx: tx,
        })
    }

    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    /// Wait for the capture. A second caller shares the same result.
    pub async fn directory(&self) -> Result<Option<PathBuf>, String> {
        let mut rx = self.rx.clone();
        if rx.borrow().is_none() {
            let _ = tokio::time::timeout(DIRECTORY_TIMEOUT, rx.changed()).await;
        }
        let settled = rx.borrow().clone();
        match settled {
            Some(result) => result,
            None => Err("original application directory capture timed out".into()),
        }
    }
}

const DIRECTORY_TIMEOUT: Duration = Duration::from_secs(30);

/// Metadata-only repository detection. Never runs Git.
pub struct GitRepositoryProvider {
    descriptor: ProviderDescriptor,
}
impl Default for GitRepositoryProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.git_repository".into(),
                accepts: vec![DIRECTORY.into()],
                produces: vec![GIT_REPOSITORY.into()],
                priority: 10,
            },
        }
    }
}
#[async_trait]
impl ContextProvider for GitRepositoryProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        _: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let Some(path) = input.value.path() else {
            return Ok(vec![]);
        };
        let Some(root) = repository_root(path) else {
            return Ok(vec![]);
        };
        Ok(vec![ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository(root),
            input.source,
            &self.descriptor.id,
        )])
    }
}

/// Walk up to the nearest repository root. A `.git` entry may be a directory or,
/// for linked worktrees and submodules, a file pointing at the real git dir.
pub fn repository_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let marker = current.join(".git");
        if marker.is_dir() || marker.is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// One unit of pending graph work: a provider call plus the metadata needed
/// to validate and attribute its result.
pub struct PendingStep {
    pub provider_id: ProviderId,
    pub source_context_id: ContextId,
    pub produces: Vec<ContextTypeId>,
    pub future: Pin<Box<dyn Future<Output = StepOutcome> + Send>>,
}

pub enum GraphStep {
    /// Poll the provider call, then call `settle` and step again.
    Pending(Box<PendingStep>),
    /// The whole graph for this revision finished, or was superseded.
    Done {
        contexts: Vec<ResolvedContext>,
        diagnostics: Vec<String>,
    },
}

pub enum StepOutcome {
    Completed {
        result: Result<Vec<ResolvedContext>>,
        duration_us: u64,
    },
    /// The caller abandoned this run before the provider was polled.
    Abandoned,
}

#[derive(Default)]
pub struct ContextGraph {
    providers: Vec<Arc<dyn ContextProvider>>,
}

impl ContextGraph {
    /// Register one provider. Duplicate ids are rejected.
    pub fn register(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        let descriptor = provider.descriptor();
        if self
            .providers
            .iter()
            .any(|p| p.descriptor().id == descriptor.id)
        {
            bail!("duplicate provider id: {}", descriptor.id);
        }
        self.providers.push(provider);
        self.providers.sort_by(|a, b| {
            b.descriptor()
                .priority
                .cmp(&a.descriptor().priority)
                .then_with(|| a.descriptor().id.cmp(&b.descriptor().id))
        });
        Ok(())
    }

    /// Provider ids in execution order, for diagnostics and tests.
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor().id.as_str())
            .collect()
    }

    pub fn providers(&self) -> &[Arc<dyn ContextProvider>] {
        &self.providers
    }

    /// Take the sorted provider list, for callers that manage their own graph.
    pub fn into_providers(self) -> Vec<Arc<dyn ContextProvider>> {
        self.providers
    }

    /// Start a run for one revision. Sequential: at most one provider call is in flight.
    ///
    /// Supersession is detected through the execution stamp, so no separate abort
    /// signal exists: there is nothing to cancel a blocking provider with.
    pub fn start(seeds: Vec<ResolvedContext>, execution: &ExecutionContext) -> ContextGraphRun {
        let mut run = ContextGraphRun {
            execution: execution.clone(),
            contexts: Vec::new(),
            equivalents: HashSet::new(),
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

#[derive(Clone)]
pub struct ContextGraphRun {
    pub execution: ExecutionContext,
    pub(crate) contexts: Vec<ResolvedContext>,
    equivalents: HashSet<(String, String, ContextSource)>,
    visited: HashSet<(ProviderId, ContextId, SessionId, Revision)>,
    pub(crate) diagnostics: Vec<String>,
    cursor: usize,
    pending: Option<(ProviderId, ContextId, Vec<ContextTypeId>)>,
}

impl ContextGraphRun {
    pub fn contexts(&self) -> &[ResolvedContext] {
        &self.contexts
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    fn insert(&mut self, mut context: ResolvedContext) {
        // Source is significant: explicit user input must not be swallowed by ambient context.
        if !self.equivalents.insert((
            context.type_id.clone(),
            context.value.text(),
            context.source,
        )) {
            return;
        }
        context.session_id = self.execution.session_id;
        context.revision = self.execution.revision;
        context.id = format!("c{}", self.contexts.len());
        self.execution.emit(LauncherEvent::ContextAdded {
            context: context.descriptor(),
        });
        self.contexts.push(context);
    }

    /// Undo the bookkeeping for a provider call the late pass still has to make.
    ///
    /// A caller whose budget expires discards the in-flight future, so the same
    /// `(provider, context, session, revision)` work must stay retryable: the late pass
    /// has to make that call again. If it did not, the work would be skipped forever —
    /// which is what happened when the frame rate was cut short on a slow directory
    /// capture: the capture itself had already started (the provider spawns it before it
    /// awaits), but nothing ever awaited its result, so the late pass had nothing to do
    /// and the snapshot stayed empty.
    ///
    /// Re-entering a provider is therefore part of the contract, not an accident: a
    /// provider whose call is cut must be safe to call again for the same input. The
    /// directory memo is single-flight for exactly this reason, and the remaining
    /// providers are pure.
    pub fn abandon_pending(&mut self) {
        if let Some((provider_id, source_context_id, _)) = self.pending.take() {
            self.visited.remove(&(
                provider_id,
                source_context_id,
                self.execution.session_id,
                self.execution.revision,
            ));
        }
    }

    fn finished(&mut self) -> GraphStep {
        GraphStep::Done {
            contexts: std::mem::take(&mut self.contexts),
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    /// Next provider call, or the terminal state of this run.
    pub fn step(&mut self, providers: &[Arc<dyn ContextProvider>]) -> GraphStep {
        loop {
            if !self.execution.is_current() {
                return self.finished();
            }
            if self.cursor >= self.contexts.len() {
                return self.finished();
            }
            let input = self.contexts[self.cursor].clone();
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
            // Record the invocation only now. A caller that discards this run before
            // polling the future (budget expiry) must be able to retry the same call.
            self.visited.insert(key);
            self.pending = Some((id.clone(), input.id.clone(), produces.clone()));
            return GraphStep::Pending(Box::new(PendingStep {
                provider_id: id,
                source_context_id: input.id.clone(),
                produces,
                future: self.poll(provider, input),
            }));
        }
    }

    /// Provider call for `input`; the caller polls the returned future.
    fn poll(
        &mut self,
        provider: Arc<dyn ContextProvider>,
        input: ResolvedContext,
    ) -> Pin<Box<dyn Future<Output = StepOutcome> + Send>> {
        let execution = self.execution.clone();
        Box::pin(async move {
            if !execution.is_current() {
                return StepOutcome::Abandoned;
            }
            tracing::info!(
                session_id = execution.session_id,
                revision = execution.revision,
                provider_id = provider.descriptor().id,
                source_context_id = input.id,
                "provider_started"
            );
            let started = std::time::Instant::now();
            let result = provider.resolve(&input, &execution).await;
            StepOutcome::Completed {
                result,
                duration_us: started.elapsed().as_micros() as u64,
            }
        })
    }

    /// Fold one provider outcome into the graph.
    pub fn settle(&mut self, outcome: StepOutcome) {
        let Some((provider_id, source_context_id, produces)) = self.pending.take() else {
            return;
        };
        let StepOutcome::Completed {
            result,
            duration_us,
        } = outcome
        else {
            return;
        };
        let input = self.contexts[self.cursor].clone();
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
                ContextValue::Text(value),
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
        graph: &ContextGraph,
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
            match run.step(graph.providers()) {
                GraphStep::Pending(pending) => {
                    let outcome = pending.future.await;
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
            let mut graph = ContextGraph::default();
            graph
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
            let (contexts, diagnostics) = drain(&graph, &execution()).await;
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

    struct Probe {
        descriptor: ProviderDescriptor,
    }
    #[async_trait]
    impl ContextProvider for Probe {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }
        async fn resolve(
            &self,
            _: &ResolvedContext,
            _: &ExecutionContext,
        ) -> Result<Vec<ResolvedContext>> {
            Ok(vec![])
        }
    }

    #[test]
    fn provider_ids_report_execution_order() {
        let mut graph = ContextGraph::default();
        for (id, priority) in [("b.later", 0), ("a.first", 10), ("c.last", 0)] {
            graph
                .register(Arc::new(Probe {
                    descriptor: ProviderDescriptor {
                        id: id.into(),
                        accepts: vec![],
                        produces: vec![],
                        priority,
                    },
                }))
                .unwrap();
        }
        // Higher priority first, then provider id: the graph is deterministic.
        let ids = graph
            .providers()
            .iter()
            .map(|provider| provider.descriptor().id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["a.first", "b.later", "c.last"]);
        assert_eq!(graph.provider_ids(), ids);
    }

    #[test]
    fn repository_detection_handles_directories_files_and_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(repository_root(&nested), Some(root.clone()));
        assert_eq!(repository_root(&root), Some(root.clone()));
        // A worktree/submodule marker is a file, not a directory.
        let worktree = dir.path().join("worktree");
        std::fs::create_dir(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: ../elsewhere\n").unwrap();
        assert_eq!(repository_root(&worktree), Some(worktree));
        // Outside any repository there is no root.
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(repository_root(&plain.path().canonicalize().unwrap()), None);
    }
}
