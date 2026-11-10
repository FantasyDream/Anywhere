//! Controlled worker ordering around retention, process planning and completed effects.
use super::{source_catalog_tests::QuietPlatform, Application};
use crate::core::{
    action::{
        availability::AvailabilityRule,
        bundle::ActionSpec,
        descriptor::{ActionExecutor, ActionRegistry},
        handler::{ActionHandler, ActionHandlerRef, ExecutionOutput},
    },
    context::{ContextValue, ResolvedContext},
    ranking::{score::UsageStats, usage::MemoryUsage},
    search::{engine::ActionSearch, query::Query},
    session::facts::SessionFacts,
};
use anyhow::Result;
use anywhere_platform::Platform;
use anywhere_protocol::*;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Gate {
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    signal: Condvar,
}
impl Gate {
    fn wait(&self) {
        self.entered.notify_one();
        let (_released, timeout) = self
            .signal
            .wait_timeout_while(
                self.released.lock().unwrap(),
                Duration::from_secs(5),
                |released| !*released,
            )
            .unwrap();
        assert!(!timeout.timed_out(), "execution gate not released");
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.signal.notify_all();
    }
    async fn entered(&self) {
        if tokio::time::timeout(Duration::from_secs(2), self.entered.notified())
            .await
            .is_err()
        {
            self.release();
            panic!("execution gate not entered");
        }
    }
}
struct Handler {
    calls: Arc<AtomicUsize>,
    gate: Option<Arc<Gate>>,
    process: bool,
}
impl ActionHandler for Handler {
    fn execute(&self, _: &ResolvedContext, _: &dyn Platform) -> Result<ExecutionOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        if self.process {
            Ok(ExecutionOutput::process(
                anywhere_platform::process::ProcessRequest {
                    title: "synthetic plan".into(),
                    program: "anywhere-nonexistent-test-program".into(),
                    arguments: vec![],
                    directory: std::env::temp_dir(),
                    environment: Default::default(),
                },
            ))
        } else {
            Ok(ExecutionOutput::preview("completed".into()))
        }
    }
}
fn fixture(handler: Handler) -> (Application, SessionId, CandidateId) {
    let mut definition = ActionSpec {
        id: "test.execution",
        title: "Execution",
        localized_titles: &[],
        accepts: TEXT,
        handler: &Noop,
        availability: AvailabilityRule::Enabled,
        base_priority: 0,
        icon: "text",
        aliases: &[],
        keywords: &[],
        primary_action: false,
    }
    .descriptor("text");
    definition.executor = ActionExecutor::Native(ActionHandlerRef::owned(handler));
    let mut registry = ActionRegistry::default();
    registry.register(definition).unwrap();
    let facts = SessionFacts::default();
    let usage = |_: &str, _: &str| UsageStats::default();
    let engine = ActionSearch::new(
        &registry,
        &usage,
        &facts,
        0,
        crate::features::context_semantic,
    );
    let context = ResolvedContext::new(
        TEXT,
        ContextValue::Text("synthetic".into()),
        ContextSource::UserInput,
        "test",
    );
    let candidate = engine
        .search(&Query::parse(""), &[context], &[])
        .candidates
        .remove(0);
    let id = candidate.presentation.id.clone();
    let app = Application::new(
        Arc::new(QuietPlatform),
        registry,
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let session_id = app.begin_activation().unwrap();
    {
        let mut session = app.session().unwrap();
        session.execution.revision = Some(0);
        session.execution.action_catalog_version = Some(app.resolver.catalog.version());
        session
            .execution
            .candidates
            .insert(id.clone().into(), candidate);
    }
    (app, session_id, id)
}
struct Noop;
impl ActionHandler for Noop {
    fn execute(&self, _: &ResolvedContext, _: &dyn Platform) -> Result<ExecutionOutput> {
        unreachable!()
    }
}
fn uses(app: &Application) -> u32 {
    app.resolver
        .history
        .lock()
        .unwrap()
        .load_global()
        .unwrap()
        .get(&("test.execution".into(), TEXT.into()))
        .map_or(0, |stats| stats.execute_count)
}

#[test]
fn queued_execution_keeps_its_epoch_across_suspend_and_resume() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let (app, session, id) = fixture(Handler {
            calls: calls.clone(),
            gate: None,
            process: false,
        });
        let gate = Arc::new(Gate::default());
        let blocker = tokio::task::spawn_blocking({
            let gate = gate.clone();
            move || gate.wait()
        });
        gate.entered().await;
        let mut queued = Box::pin(app.execute_use_case(session, 0, id.clone()));
        // Poll exactly through preparation and enqueueing while the only worker is held.
        std::future::poll_fn(|cx| {
            assert!(queued.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        let suspended = app.suspend_session(session).await;
        let resumed = app.resume_session(session);
        gate.release();
        blocker.await.unwrap();
        suspended.unwrap();
        assert!(resumed.unwrap().is_some());
        let result = queued.await.unwrap();
        assert_eq!(result.status, ActionStatus::Failed);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(uses(&app), 0);
        assert!(app
            .execute_use_case(session, 0, id)
            .await
            .unwrap()
            .succeeded());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(uses(&app), 1);
    });
}

#[tokio::test]
async fn completed_effect_keeps_usage_when_dismissed_before_handler_returns() {
    let calls = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Gate::default());
    let (app, session, id) = fixture(Handler {
        calls: calls.clone(),
        gate: Some(gate.clone()),
        process: false,
    });
    let app = Arc::new(app);
    let pending = tokio::spawn({
        let app = app.clone();
        async move { app.execute_use_case(session, 0, id).await }
    });
    gate.entered().await;
    // Handler has completed its synthetic effect, but cannot return until after dismissal.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    use super::LauncherApplication;
    let dismissed = app.deactivate(session);
    gate.release();
    dismissed.unwrap();
    let result = pending.await.unwrap().unwrap();
    assert!(result.succeeded());
    assert_eq!(result.output_text.as_deref(), Some("completed"));
    assert_eq!(uses(&app), 1);
}

#[tokio::test]
async fn process_plan_cannot_start_after_suspend_and_resume() {
    let gate = Arc::new(Gate::default());
    let (app, session, id) = fixture(Handler {
        calls: Default::default(),
        gate: Some(gate.clone()),
        process: true,
    });
    let app = Arc::new(app);
    let pending = tokio::spawn({
        let app = app.clone();
        async move { app.execute_use_case(session, 0, id).await }
    });
    gate.entered().await;
    let suspended = app.suspend_session(session).await;
    let resumed = app.resume_session(session);
    gate.release();
    suspended.unwrap();
    assert!(resumed.unwrap().is_some());
    let result = pending.await.unwrap().unwrap();
    assert_eq!(result.status, ActionStatus::Failed);
    assert_eq!(
        result.error.as_deref(),
        Some("action expired before the task started")
    );
    assert!(app.execution_sessions().is_empty());
    assert_eq!(uses(&app), 0);
}
