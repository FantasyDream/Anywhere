use crate::support::*;

fn registry_with_wrap() -> ActionRegistry {
    let mut registry = registry();
    registry
        .register_plugin(anywhere_plugin::parse_manifest(crate::search::WRAP_PLUGIN).unwrap())
        .unwrap();
    registry
}

#[tokio::test]
async fn wrapping_a_git_action_is_disabled_when_git_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform {
        no_git: true,
        ..TestPlatform::new(root, true)
    });
    let app = Application::new(
        platform.clone(),
        registry_with_wrap(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    for command in ["core.git.status", "wrap.status"] {
        let status = state
            .candidates
            .iter()
            .find(|candidate| candidate.command_id.as_deref() == Some(command))
            .unwrap_or_else(|| panic!("missing {command}"));
        assert_eq!(status.availability.state, AvailabilityState::Disabled);
        let error = app
            .execute(state.session_id, state.revision, status.id.clone())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Git 不可用"),
            "{command}: {error}"
        );
    }
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn wrapping_copy_path_is_hidden_when_the_clipboard_already_holds_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform {
        clipboard: Mutex::new(Some(root.to_string_lossy().into_owned())),
        ..TestPlatform::new(root, true)
    });
    let app = Application::new(
        platform,
        registry_with_wrap(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let visible = state
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == CandidateKind::Action)
        .filter_map(|candidate| candidate.command_id.as_deref())
        .collect::<Vec<_>>();
    assert!(!visible.contains(&"core.directory.copy"), "{visible:?}");
    assert!(!visible.contains(&"wrap.copy"), "{visible:?}");
    let searched = app
        .update_input(state.session_id, "copy".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let searched = searched
        .candidates
        .iter()
        .filter_map(|candidate| candidate.command_id.as_deref())
        .collect::<Vec<_>>();
    assert!(!searched.contains(&"core.directory.copy"), "{searched:?}");
    assert!(!searched.contains(&"wrap.copy"), "{searched:?}");
}

#[tokio::test]
async fn wrapping_url_keeps_plugin_identity_and_records_usage() {
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        registry_with_wrap(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let url = app
        .update_input(session, "https://example.org/wrapped".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let pretty = url
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("wrap.url"))
        .expect("wrapped URL action");
    assert_eq!(pretty.title, "Copy URL");
    assert!(pretty
        .subtitle
        .as_deref()
        .is_some_and(|subtitle| subtitle.contains("Wrap")));
    assert!(app
        .execute(session, url.revision, pretty.id.clone())
        .await
        .unwrap()
        .succeeded());
    let recorded = history
        .lock()
        .unwrap()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        recorded
            .iter()
            .any(|(action, kind, _)| { action == "wrap.url" && kind == URL }),
        "{recorded:?}"
    );
}

#[test]
fn invalid_plugin_does_not_partially_register_and_discovery_continues() {
    let mut registry = registry();
    let mut plugin = anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap();
    plugin.id = "broken".into();
    let mut bad = plugin.commands[0].clone();
    bad.id = "bad".into();
    bad.action = DeclarativeAction::ExecuteBuiltin {
        command_id: "core.json.format".into(),
    };
    plugin.commands.push(bad);
    assert!(registry.register_plugin(plugin).is_err());
    // Nothing from the rejected manifest may be registered, under any id.
    assert!(registry.get("broken.search").is_none());
    assert!(registry.get("broken.bad").is_none());
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("good.toml"),
        anywhere_plugin::EXAMPLE_PLUGIN,
    )
    .unwrap();
    std::fs::write(dir.path().join("bad.toml"), "this is not toml").unwrap();
    let discovered = anywhere_plugin::discover(dir.path());
    assert_eq!(discovered.manifests.len(), 1);
    assert_eq!(discovered.diagnostics.len(), 1);
    assert!(discovered.diagnostics[0].contains("bad.toml"));
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
fn queued_plugin_removal_revokes_execution() {
    assert_queued_reload_rejected(
        registry(),
        anywhere_core::application::builtin_registry(),
        "example.search",
        "review witness",
    );
}

#[test]
fn queued_same_id_plugin_replacement_revokes_execution() {
    let mut replacement = anywhere_core::application::builtin_registry();
    replacement
        .register_plugin(
            anywhere_plugin::parse_manifest(&anywhere_plugin::EXAMPLE_PLUGIN.replace(
                "https://example.com/search",
                "https://changed.example/search",
            ))
            .unwrap(),
        )
        .unwrap();
    assert_queued_reload_rejected(registry(), replacement, "example.search", "review witness");
}

#[test]
fn queued_execute_builtin_target_removal_revokes_execution() {
    use anywhere_core::core::action::{
        availability::AvailabilityRule,
        descriptor::{ActionDefinition, ActionExecutor},
    };
    let mut replacement = ActionRegistry::default();
    // Keep the wrapper definition while revoking the referenced native implementation.
    replacement
        .register(ActionDefinition {
            localized_titles: Default::default(),
            id: "wrap.copy".into(),
            title: "Wrapped Copy".into(),
            subtitle: None,
            shortcut: None,
            accepts: vec![ContextMatcher {
                type_id: DIRECTORY.into(),
            }],
            executor: ActionExecutor::Declarative(DeclarativeAction::ExecuteBuiltin {
                command_id: "core.directory.copy".into(),
            }),
            availability: AvailabilityRule::Enabled,
            base_priority: 0,
            icon: "copy".into(),
            group_id: "folder".into(),
            aliases: vec![],
            keywords: vec![],
            owning_app: None,
            menu_path: vec![],
            home_eligible: true,
            primary_action: false,
            search_title: None,
        })
        .unwrap();
    assert_queued_reload_rejected(registry_with_wrap(), replacement, "wrap.copy", "");
}

#[test]
fn persistent_binding_uses_business_id_and_checks_current_catalog() {
    let registry = registry_with_wrap();
    let binding = registry.persistent_binding("wrap.copy").unwrap();
    let restored = serde_json::from_str::<
        anywhere_core::core::action::identity::PersistentActionBinding,
    >(&serde_json::to_string(&binding).unwrap())
    .unwrap();
    assert_eq!(binding, restored);
    assert_eq!(binding.action_id.as_str(), "wrap.copy");
    assert_eq!(binding.context_types, vec![DIRECTORY.to_owned()]);
    assert!(registry.persistent_binding("candidate:wrap.copy").is_err());
    assert!(ActionRegistry::default()
        .persistent_binding(binding.action_id.as_str())
        .is_err());
}
