use crate::support::*;
use anywhere_core::core::action::{
    availability::AvailabilityRule,
    bundle::ActionSpec,
    handler::{ActionHandler, ExecutionOutput},
};

struct ChangedSearch;
impl ActionHandler for ChangedSearch {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        platform.open_url(&format!(
            "https://changed.example/search?q={}",
            context.value.text()
        ))?;
        Ok(ExecutionOutput::dismiss())
    }
}

/// Force the worker to wait, replace the definition, then verify no old side effect occurs.
fn assert_queued_reload_rejected(
    original: ActionRegistry,
    replacement: ActionRegistry,
    command: &str,
    query: &str,
) {
    use std::{future::Future, task::Poll};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let platform = Arc::new(TestPlatform::new(temp.path().canonicalize().unwrap(), true));
        let application = Application::new(
            platform.clone(),
            original,
            Box::<MemoryUsage>::default(),
            vec![],
        );
        let session = application.activate().await.unwrap().unwrap().session_id;
        let _ = settled(&application, session, 0).await;
        let initial = application
            .update_input(session, query.into(), 1)
            .await
            .unwrap()
            .unwrap();
        let state = settled(&application, session, initial.revision).await;
        let id = candidate(&state, command);
        let gate = CaptureGate::new();
        let entered = Arc::new(Notify::new());
        let blocker = tokio::task::spawn_blocking({
            let gate = gate.clone();
            let entered = entered.clone();
            move || {
                entered.notify_one();
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while !gate.released() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        });
        entered.notified().await;
        let mut execution = Box::pin(application.execute(session, state.revision, id.clone()));
        std::future::poll_fn(|cx| {
            assert!(execution.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        application.replace_registry(replacement);
        // Recomputed same-ID definitions also cannot accept a stale visible row ID.
        assert!(application
            .execute(session, state.revision, id)
            .await
            .is_err());
        gate.release();
        blocker.await.unwrap();
        let result = execution.await.unwrap();
        assert!(!result.succeeded(), "old catalog executed: {result:?}");
        assert!(
            platform.calls.lock().unwrap().is_empty(),
            "a stale definition reached the platform"
        );
    });
}

#[test]
fn queued_action_removal_revokes_execution() {
    assert_queued_reload_rejected(
        registry(),
        anywhere_core::application::builtin_registry(),
        "example.search",
        "review witness",
    );
}

#[test]
fn queued_same_id_action_replacement_revokes_execution() {
    let mut replacement = anywhere_core::application::builtin_registry();
    replacement
        .register(
            ActionSpec {
                localized_titles: &[],
                id: "example.search",
                title: "Search Example",
                accepts: TEXT,
                handler: &ChangedSearch,
                availability: AvailabilityRule::Enabled,
                base_priority: 40,
                icon: "globe",
                aliases: &["example"],
                keywords: &["web"],
                primary_action: false,
            }
            .descriptor("web"),
        )
        .unwrap();
    assert_queued_reload_rejected(registry(), replacement, "example.search", "review witness");
}

#[test]
fn persistent_binding_uses_business_id_and_checks_current_catalog() {
    let registry = registry();
    let binding = registry.persistent_binding("core.directory.copy").unwrap();
    let restored = serde_json::from_str::<
        anywhere_core::core::action::identity::PersistentActionBinding,
    >(&serde_json::to_string(&binding).unwrap())
    .unwrap();
    assert_eq!(binding, restored);
    assert_eq!(binding.action_id.as_str(), "core.directory.copy");
    assert_eq!(binding.context_types, vec![DIRECTORY.to_owned()]);
    assert!(registry
        .persistent_binding("candidate:core.directory.copy")
        .is_err());
    assert!(ActionRegistry::default()
        .persistent_binding(binding.action_id.as_str())
        .is_err());
}
