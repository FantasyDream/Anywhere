//! Request currency and optional contribution boundaries; no GUI or external tools.
use super::{source_catalog_tests::QuietPlatform, Application, LauncherApplication};
use crate::{
    core::{
        action::descriptor::ActionRegistry,
        context::{
            engine::ContextGraph, provider::*, registry::ContextProviderRegistry, ContextValue,
            ExecutionContext, GraphStep, ResolvedContext,
        },
        ranking::usage::MemoryUsage,
        search::candidate_source::*,
    },
    features::policy::{FeaturePolicy, FeaturePolicySnapshot},
};
use anywhere_protocol::*;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

fn app() -> Application {
    Application::new(
        Arc::new(QuietPlatform),
        super::builtin_registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    )
}
fn disabled(ids: &[&str]) -> Arc<FeaturePolicySnapshot> {
    Arc::new(FeaturePolicySnapshot::new(
        1,
        ids.iter().map(|id| (*id).into()).collect(),
        false,
    ))
}
fn text() -> ResolvedContext {
    ResolvedContext::new(
        TEXT,
        ContextValue::Text("input".into()),
        ContextSource::UserInput,
        "test",
    )
}

#[test]
fn policy_guard_rejects_same_generation_replacement_and_aba_without_stamp_change() {
    let policy = Arc::new(FeaturePolicy::default());
    let old = policy.snapshot();
    let ticket = ExecutionContext {
        guard: Some(policy.request_guard()),
        ..ExecutionContext::detached(1, 0)
    };
    let unchanged = policy.snapshot();
    *policy.current.write().unwrap() = unchanged;
    assert!(ticket.is_current());
    // Generation is local to its configuration owner, so it is insufficient on its own.
    *policy.current.write().unwrap() = Arc::new(FeaturePolicySnapshot::new(
        old.generation(),
        Default::default(),
        false,
    ));
    assert!(!ticket.is_current());
    *policy.current.write().unwrap() = disabled(&["node"]);
    *policy.current.write().unwrap() = Arc::new(FeaturePolicySnapshot::new(
        old.generation(),
        Default::default(),
        false,
    ));
    assert!(!ticket.is_current());
}

#[tokio::test]
async fn real_node_expansion_inherits_owner_and_disabled_factory_loses_shortcut_eligibility() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        r#"{"scripts":{"build":"echo ok","custom":"echo custom"}}"#,
    )
    .unwrap();
    let provider = crate::features::developer::provider::ProjectProvider::new(
        crate::features::developer::provider::ProjectKind::Node,
    );
    let directory = ResolvedContext::new(
        DIRECTORY,
        ContextValue::Directory(tmp.path().into()),
        ContextSource::UserInput,
        "test",
    );
    let contexts = provider
        .resolve(&directory, &ExecutionContext::detached(1, 0))
        .await
        .unwrap();
    let mut registry = super::builtin_registry();
    let (mut expanded, diagnostics) = registry.for_contexts(&contexts);
    assert!(diagnostics.is_empty());
    let build = "developer.node.build";
    assert_eq!(expanded.get(build).unwrap().contributor(), Some("node"));
    assert_eq!(
        registry.get("core.git.status").unwrap().contributor(),
        Some("git")
    );
    assert_eq!(
        registry.get("core.directory.copy").unwrap().contributor(),
        None
    );
    assert!(registry.supports_persistent_binding(build));
    registry.retain_contributors(|id| id != "node");
    expanded.retain_contributors(|id| id != "node");
    assert!(!registry.supports_persistent_binding(build));
    assert!(registry.for_contexts(&contexts).0.get(build).is_none());
    assert!(expanded.get(build).is_none());
    assert!(registry.get("developer.cargo.check").is_some());
    registry.retain_contributors(|_| false);
    assert!(registry.get("core.directory.copy").is_some());
    assert!(registry.get("core.url.copy").is_none());
}

struct Empty(&'static str);
impl CandidateSource for Empty {
    fn id(&self) -> &'static str {
        self.0
    }
    fn candidates(&self, _: &SearchContext<'_>) -> Vec<crate::core::search::candidate::Candidate> {
        vec![]
    }
}
struct SlowSource;
#[async_trait::async_trait]
impl AsyncCandidateSource for SlowSource {
    fn id(&self) -> &'static str {
        "node.async"
    }
    async fn candidates(
        &self,
        _: &str,
        _: ExecutionContext,
    ) -> anyhow::Result<Vec<crate::core::search::candidate::Candidate>> {
        std::future::pending().await
    }
}
#[tokio::test]
async fn atomic_policy_publication_filters_all_contributions_and_revokes_retained_actions() {
    let mut app = app();
    app.register_candidate_source(Arc::new(Empty("base")));
    let initial = app.activate().await.unwrap().unwrap();
    let state = app
        .update_context_input(
            initial.session_id,
            "https://example.com".into(),
            1,
            None,
            InputMode::Search,
        )
        .await
        .unwrap()
        .unwrap();
    let old_row = state
        .candidates
        .iter()
        .find(|c| c.command_id.as_deref() == Some("core.url.open"))
        .unwrap()
        .id
        .clone();
    app.suspend_session(initial.session_id).await.unwrap();
    let old_policy = app.resolver.feature_policy.snapshot();
    let ticket = app.resolver.execution(initial.session_id, 1);
    let (registry, providers) = super::composition::configured_builtins(&Default::default());
    let mut sources = SourceSet {
        synchronous: vec![Arc::new(Empty("node.sync"))],
        asynchronous: vec![Arc::new(SlowSource)],
        ..Default::default()
    };
    assert!(providers.ids().contains(&"developer.project.node"));
    sources.assign_contributor("node");
    let policy = disabled(&["url", "node"]);
    assert!(app
        .publish_feature_catalog(
            registry.clone(),
            providers.clone(),
            sources.clone(),
            policy.clone(),
            || anyhow::bail!("disk full")
        )
        .is_err());
    assert!(app.resolver.feature_policy.is_current(&old_policy));
    assert!(ticket.is_current());
    assert!(app.resume_session(initial.session_id).unwrap().is_some());
    app.suspend_session(initial.session_id).await.unwrap();
    app.publish_feature_catalog(registry.clone(), providers, sources, policy.clone(), || {
        Ok(())
    })
    .unwrap();
    assert!(app.resolver.feature_policy.is_current(&policy));
    assert!(!ticket.is_current());
    assert!(app.resume_session(initial.session_id).unwrap().is_none());
    assert!(app.execute(initial.session_id, 1, old_row).await.is_err());
    assert!(app
        .resolver
        .providers
        .lock()
        .unwrap()
        .ids()
        .iter()
        .all(|id| *id != "core.url" && *id != "developer.project.node"));
    {
        let sources = app.resolver.sources.lock().unwrap();
        assert_eq!(
            sources
                .synchronous()
                .iter()
                .map(|s| s.id())
                .collect::<Vec<_>>(),
            ["base"]
        );
        assert!(sources.asynchronous().is_empty());
    }
    // Restoring a captured registry cannot resurrect disabled optional actions/factories.
    app.replace_registry(registry);
    assert!(app
        .supported_shortcut_actions(&["core.url.open".into(), "developer.node.build".into()])
        .unwrap()
        .is_empty());
    assert!(!app
        .shortcut_actions()
        .unwrap()
        .iter()
        .any(|a| a.action_id == "core.url.open"));
    let fresh = app.activate().await.unwrap().unwrap();
    let state = app
        .update_context_input(
            fresh.session_id,
            "https://example.com".into(),
            1,
            None,
            InputMode::Search,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(state.context_summary.iter().all(|c| c.type_id != URL));
    assert!(state
        .candidates
        .iter()
        .all(|c| c.command_id.as_deref() != Some("core.url.open")));
}

struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
struct PendingProvider {
    descriptor: ProviderDescriptor,
    entered: tokio::sync::Notify,
    dropped: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl ContextProvider for PendingProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        _: &ResolvedContext,
        _: &ExecutionContext,
    ) -> anyhow::Result<Vec<ResolvedContext>> {
        let _flag = DropFlag(self.dropped.clone());
        self.entered.notify_one();
        std::future::pending().await
    }
}
#[tokio::test]
async fn policy_change_cancels_uncooperative_pending_provider_and_rejects_completed_late_context() {
    let app = app();
    let dropped = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(PendingProvider {
        descriptor: ProviderDescriptor {
            id: "node.probe".into(),
            accepts: vec![TEXT.into()],
            produces: vec![URL.into()],
            priority: 0,
        },
        entered: Default::default(),
        dropped: dropped.clone(),
    });
    let mut ticket = ExecutionContext::detached(1, 0);
    ticket.guard = Some(app.resolver.feature_policy.request_guard());
    let mut run = ContextGraph::start(vec![text()], &ticket);
    let providers: [Arc<dyn ContextProvider>; 1] = [provider.clone()];
    assert!(matches!(run.step(&providers), GraphStep::Pending));
    provider.entered.notified().await;
    app.publish_feature_catalog(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet::default(),
        disabled(&["node"]),
        || Ok(()),
    )
    .unwrap();
    let outcome = tokio::time::timeout(
        Duration::from_secs(1),
        run.await_pending(None, None, Instant::now()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(matches!(outcome, StepOutcome::Abandoned));
    assert!(dropped.load(Ordering::Acquire));
    // Even a previously completed outcome awaiting settlement cannot enrich an old graph.
    run.settle(StepOutcome::Completed {
        result: Ok(vec![ResolvedContext::new(
            URL,
            ContextValue::Url("https://example.com".parse().unwrap()),
            ContextSource::UserInput,
            "node.probe",
        )]),
        duration_us: 0,
    });
    let GraphStep::Done { contexts, .. } = run.step(&[]) else {
        panic!("old graph must end")
    };
    assert_eq!(contexts.len(), 1);
    assert!(contexts.iter().all(|c| c.type_id == TEXT));
}

struct PendingStream {
    entered: tokio::sync::Notify,
    sender: std::sync::Mutex<Option<tokio::sync::mpsc::Sender<AsyncCandidateBatch>>>,
    dropped: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl AsyncCandidateSource for PendingStream {
    fn id(&self) -> &'static str {
        "node.stream"
    }
    fn debounce(&self) -> Duration {
        Duration::ZERO
    }
    fn timeout(&self) -> Option<Duration> {
        None
    }
    async fn candidates(
        &self,
        _: &str,
        _: ExecutionContext,
    ) -> anyhow::Result<Vec<crate::core::search::candidate::Candidate>> {
        unreachable!()
    }
    async fn stream(
        &self,
        _: AsyncSearchRequest,
        _: ExecutionContext,
        sender: tokio::sync::mpsc::Sender<AsyncCandidateBatch>,
    ) -> anyhow::Result<()> {
        let _flag = DropFlag(self.dropped.clone());
        *self.sender.lock().unwrap() = Some(sender);
        self.entered.notify_one();
        std::future::pending().await
    }
}
#[tokio::test]
async fn policy_currency_cancels_pending_stream_and_closes_late_batch_channel() {
    let app = app();
    let dropped = Arc::new(AtomicBool::new(false));
    let source = Arc::new(PendingStream {
        entered: Default::default(),
        sender: Default::default(),
        dropped: dropped.clone(),
    });
    let workers = Arc::new(tokio::sync::Semaphore::new(1));
    let published = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut ticket = ExecutionContext::detached(1, 0);
    ticket.guard = Some(app.resolver.feature_policy.request_guard());
    super::source_runtime::spawn_source(
        source.clone(),
        AsyncSearchRequest {
            text: "q".into(),
            options: None,
        },
        ticket,
        workers.clone(),
        Arc::new(|| true),
        Arc::new({
            let published = published.clone();
            move |_| {
                published.fetch_add(1, Ordering::SeqCst);
            }
        }),
    );
    tokio::time::timeout(Duration::from_secs(1), source.entered.notified())
        .await
        .unwrap();
    app.publish_feature_catalog(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet::default(),
        disabled(&["node"]),
        || Ok(()),
    )
    .unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(1), workers.acquire())
        .await
        .unwrap()
        .unwrap();
    assert!(dropped.load(Ordering::Acquire));
    drop(permit);
    let sender = source.sender.lock().unwrap().take().unwrap();
    assert!(sender
        .send(AsyncCandidateBatch {
            update: BatchUpdate::Replace,
            candidates: vec![],
            state: SearchSourceState::Ready,
            metadata: None
        })
        .await
        .is_err());
    assert_eq!(published.load(Ordering::SeqCst), 0);
}

#[cfg(test)]
#[cfg(unix)]
#[tokio::test]
async fn native_policy_change_does_not_stop_an_already_running_process_session() {
    let app = app();
    let tmp = tempfile::tempdir().unwrap();
    let release = tmp.path().join("release");
    let id = app
        .executions
        .start(anywhere_platform::process::ProcessRequest {
            title: "held task".into(),
            program: "/bin/sh".into(),
            arguments: vec![
                "-c".into(),
                "while [ ! -f \"$1\" ]; do sleep 0.02; done; printf completed".into(),
                "held-task".into(),
                release.to_string_lossy().into_owned(),
            ],
            directory: tmp.path().into(),
            environment: Default::default(),
        })
        .unwrap();
    assert_eq!(
        app.execution_session(&id).unwrap().summary.status,
        ExecutionSessionStatus::Running
    );
    app.publish_feature_catalog(
        super::builtin_registry(),
        ContextProviderRegistry::default(),
        SourceSet::default(),
        disabled(&["node", "rust", "maven", "gradle"]),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        app.execution_session(&id).unwrap().summary.status,
        ExecutionSessionStatus::Running
    );
    std::fs::write(release, "finish").unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while app
            .execution_session(&id)
            .unwrap()
            .summary
            .status
            .is_active()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let detail = app.execution_session(&id).unwrap();
    assert_eq!(detail.summary.status, ExecutionSessionStatus::Succeeded);
    assert_eq!(detail.summary.exit_code, Some(0));
    assert_eq!(detail.stdout, "completed");
}
