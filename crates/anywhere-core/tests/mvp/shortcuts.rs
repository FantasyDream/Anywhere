use crate::support::*;

#[tokio::test]
async fn binding_waits_for_supplemental_capture_and_keeps_disabled_reason() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    let gate = CaptureGate::new();
    let platform = Arc::new(TestPlatform {
        clipboard_gate: Mutex::new(Some(gate.clone())),
        no_git: true,
        ..TestPlatform::new(dir.path().canonicalize().unwrap(), true)
    });
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    platform.clipboard_started.notified().await;
    let pending = app.resolve_action_binding(first.session_id, 0, "core.directory.copy");
    gate.release();
    assert!(pending.unwrap_err().to_string().contains("loading"));
    let _ = settled(&app, first.session_id, 0).await;
    assert_eq!(
        app.resolve_action_binding(first.session_id, 0, "core.directory.copy")
            .unwrap()
            .len(),
        1
    );
    assert!(app
        .resolve_action_binding(first.session_id, 0, "core.git.status")
        .unwrap_err()
        .to_string()
        .contains("Git 不可用"));
}

#[tokio::test]
async fn binding_resolves_current_business_action_outside_query_and_scope_and_records_usage() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), true));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let _ = settled(&app, session, 0).await;
    let queried = app
        .update_scoped_input(session, "no matching action".into(), 1, Some("git".into()))
        .await
        .unwrap()
        .unwrap();
    let state = settled(&app, session, queried.revision).await;
    assert!(state.candidates.is_empty());
    let actions = app
        .resolve_action_binding(session, 1, "core.directory.copy")
        .unwrap();
    assert_eq!(actions.len(), 1);
    assert!(app
        .execute(session, 1, actions[0].id.clone())
        .await
        .unwrap()
        .succeeded());
    assert!(platform
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(kind, value)| kind == "copy" && value == &root.to_string_lossy()));
    assert!(history
        .lock()
        .unwrap()
        .0
        .keys()
        .any(|(id, _, _)| id == "core.directory.copy"));
    app.deactivate(session).unwrap();
    assert!(app
        .resolve_action_binding(session, 1, "core.directory.copy")
        .is_err());
}

#[tokio::test]
async fn binding_does_not_guess_between_distinct_objects_or_reuse_old_context() {
    let platform = Arc::new(TestPlatform {
        clipboard: Mutex::new(Some("https://example.org/clipboard".into())),
        ..Default::default()
    });
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let _ = settled(&app, session, 0).await;
    app.update_input(session, "https://example.org/input".into(), 1)
        .await
        .unwrap();
    let _ = settled(&app, session, 1).await;
    let choices = app
        .resolve_action_binding(session, 1, "core.url.open")
        .unwrap();
    assert_eq!(choices.len(), 2);
    assert_ne!(
        choices[0].execution_context_id,
        choices[1].execution_context_id
    );
    assert!(platform.calls.lock().unwrap().is_empty());
    // Copy of the clipboard URL is Hidden; only the distinct typed URL can be copied.
    assert_eq!(
        app.resolve_action_binding(session, 1, "core.url.copy")
            .unwrap()
            .len(),
        1
    );
    app.update_input(session, "".into(), 2).await.unwrap();
    let _ = settled(&app, session, 2).await;
    assert!(app
        .resolve_action_binding(session, 1, "core.url.open")
        .is_err());
    assert!(app
        .resolve_action_binding(session, 2, "core.url.copy")
        .is_err());
    app.replace_registry(ActionRegistry::default());
    assert!(app
        .resolve_action_binding(session, 2, "core.url.open")
        .is_err());
}

#[tokio::test]
async fn shortcut_catalog_expands_declared_scripts_without_context_or_row_ids() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"check:types":"echo PRIVATE_BODY"}}"#,
    )
    .unwrap();
    let app = Application::new(
        Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true)),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let _ = settled(&app, session, 0).await;
    let options = app.shortcut_actions().unwrap();
    let script = options
        .iter()
        .find(|a| a.title.contains("check:types"))
        .unwrap();
    assert!(script.action_id.starts_with("developer.node."));
    assert_eq!(script.context_types, vec![PROJECT_NODE]);
    assert!(options
        .iter()
        .all(|a| !a.action_id.contains(":catalog:") && !a.title.contains("PRIVATE_BODY")));
    assert!(app
        .resolve_action_binding(session, 0, "not.an.action")
        .is_err());
    app.deactivate(session).unwrap();
    assert!(app
        .shortcut_actions()
        .unwrap()
        .iter()
        .any(|a| a.action_id == script.action_id));
}

#[tokio::test]
async fn global_binding_waits_for_new_capture_and_can_be_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let gate = CaptureGate::new();
    let platform = Arc::new(TestPlatform {
        clipboard_gate: Mutex::new(Some(gate.clone())),
        ..TestPlatform::new(dir.path().canonicalize().unwrap(), true)
    });
    let app = Arc::new(app(platform.clone()));
    let first = app.activate().await.unwrap().unwrap();
    platform.clipboard_started.notified().await;
    let pending = tokio::spawn({
        let app = app.clone();
        async move {
            app.wait_action_binding(first.session_id, 0, "core.directory.copy")
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(!pending.is_finished());
    app.deactivate(first.session_id).unwrap();
    assert!(pending
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("expired"));
    gate.release();
    let second = app.activate().await.unwrap().unwrap();
    assert_ne!(second.session_id, first.session_id);
    let result = app
        .wait_action_binding(second.session_id, 0, "core.directory.copy")
        .await
        .unwrap();
    assert_eq!(result.state.session_id, second.session_id);
    assert!(result.state.context_complete);
    assert_eq!(result.candidates.len(), 1);
    assert!(platform.calls.lock().unwrap().is_empty()); // Resolution has no side effects.
    assert!(app
        .wait_action_binding(first.session_id, 0, "core.directory.copy")
        .await
        .is_err());
}

#[tokio::test]
async fn global_binding_times_out_without_executing_when_permission_read_is_blocked() {
    let gate = CaptureGate::new();
    let platform = Arc::new(TestPlatform {
        clipboard_gate: Mutex::new(Some(gate.clone())),
        ..Default::default()
    });
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    platform.clipboard_started.notified().await;
    let result = app
        .wait_action_binding(state.session_id, 0, "core.url.open")
        .await;
    gate.release();
    assert!(result.unwrap_err().to_string().contains("timed out"));
    assert!(platform.calls.lock().unwrap().is_empty());
    app.deactivate(state.session_id).unwrap();
}

fn route(
    action: &str,
    app: Option<&str>,
    context: Option<&str>,
) -> anywhere_core::application::ShortcutRoute {
    anywhere_core::application::ShortcutRoute {
        action_id: action.into(),
        app_bundle_id: app.map(Into::into),
        context_type: context.map(Into::into),
    }
}

#[tokio::test]
async fn intent_matches_captured_app_and_context_before_availability_outside_scope() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let _ = settled(&app, first.session_id, 0).await;
    app.update_scoped_input(
        first.session_id,
        "https://example.org/current".into(),
        1,
        Some("git".into()),
    )
    .await
    .unwrap();
    let _ = settled(&app, first.session_id, 1).await;
    let rules = [
        route("core.directory.copy", None, None),
        route("core.directory.copy", None, Some(DIRECTORY)),
        route("core.url.open", Some("com.apple.finder"), Some(URL)),
    ];
    let chosen = app
        .resolve_shortcut_intent(first.session_id, 1, &rules)
        .unwrap();
    assert!(!chosen.requires_choice);
    assert_eq!(chosen.candidates.len(), 1);
    assert_eq!(
        chosen.candidates[0].command_id.as_deref(),
        Some("core.url.open")
    );
    assert!(platform.calls.lock().unwrap().is_empty());
    assert!(app
        .execute(first.session_id, 1, chosen.candidates[0].id.clone())
        .await
        .unwrap()
        .succeeded());
    assert!(platform
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(op, value)| op == "url" && value == "https://example.org/current"));
    assert!(app
        .shortcut_applications()
        .unwrap()
        .iter()
        .any(|a| a.bundle_id == "com.apple.finder" && a.title == "Finder"));
}

#[tokio::test]
async fn intent_equal_specificity_forces_choice_even_after_deduplicating_same_target() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(Arc::new(TestPlatform::new(
        dir.path().canonicalize().unwrap(),
        true,
    )));
    let state = app.activate().await.unwrap().unwrap();
    let _ = settled(&app, state.session_id, 0).await;
    let rules = [
        route("core.directory.copy", Some("com.apple.finder"), None),
        route("core.directory.copy", None, Some(DIRECTORY)),
    ];
    let result = app
        .resolve_shortcut_intent(state.session_id, 0, &rules)
        .unwrap();
    assert!(result.requires_choice);
    assert_eq!(result.candidates.len(), 1); // No automatic execution even for the same target.
    let different = [
        route("core.directory.copy", Some("com.apple.finder"), None),
        route("core.directory.terminal", None, Some(DIRECTORY)),
    ];
    let result = app
        .resolve_shortcut_intent(state.session_id, 0, &different)
        .unwrap();
    assert!(result.requires_choice);
    assert_eq!(result.candidates.len(), 2);
}

#[tokio::test]
async fn intent_unavailable_specific_action_never_uses_fallback_or_wrong_context_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    let platform = Arc::new(TestPlatform {
        no_git: true,
        ..TestPlatform::new(dir.path().canonicalize().unwrap(), true)
    });
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    let _ = settled(&app, state.session_id, 0).await;
    let rules = [
        route("core.directory.copy", None, None),
        route(
            "core.git.status",
            Some("com.apple.finder"),
            Some(GIT_REPOSITORY),
        ),
    ];
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &rules)
        .unwrap_err()
        .to_string()
        .contains("Git 不可用"));
    let wrong_type = [
        route("core.directory.copy", None, None),
        route(
            "core.directory.copy",
            Some("com.apple.finder"),
            Some(GIT_REPOSITORY),
        ),
    ];
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &wrong_type)
        .unwrap_err()
        .to_string()
        .contains("required context type"));
    assert!(platform.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn intent_rejects_no_match_duplicate_predicates_bad_input_and_stale_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(Arc::new(TestPlatform::new(
        dir.path().canonicalize().unwrap(),
        true,
    )));
    let state = app.activate().await.unwrap().unwrap();
    let _ = settled(&app, state.session_id, 0).await;
    let nomatch = [route("core.directory.copy", Some("com.apple.safari"), None)];
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &nomatch)
        .unwrap_err()
        .to_string()
        .contains("No shortcut rule"));
    let duplicate = [
        route("core.directory.copy", None, None),
        route("core.directory.terminal", None, None),
    ];
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &duplicate)
        .unwrap_err()
        .to_string()
        .contains("distinct"));
    let malformed = [route(
        "core.directory.copy",
        None,
        Some("core.directory/../../"),
    )];
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &malformed)
        .is_err());
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &[])
        .is_err());
    let rules = [route("core.directory.copy", None, None)];
    let resolved = app
        .wait_shortcut_intent(state.session_id, 0, &rules)
        .await
        .unwrap();
    assert_eq!(resolved.candidates.len(), 1);
    app.replace_registry(ActionRegistry::default());
    assert!(app
        .resolve_shortcut_intent(state.session_id, 0, &rules)
        .is_err());
    assert!(app
        .execute(state.session_id, 0, resolved.candidates[0].id.clone())
        .await
        .is_err());
}

#[tokio::test]
async fn registration_catalog_distinguishes_revocation_from_missing_project() {
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let script = "developer.node.pnpm.script.636865636b3a7479706573";
    let cargo = "developer.cargo.run.61707000617070";
    let ids = vec![
        "core.directory.copy",
        script,
        cargo,
        "removed.plugin.action",
        "developer.node.pnpm.script.00",
        "developer.cargo.run.00",
        "developer.node.pnpm.script.636865636B",
        "menu:untrusted",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let supported = app.supported_shortcut_actions(&ids).unwrap();
    assert_eq!(supported, vec!["core.directory.copy", cargo, script]);
    // No node/rust Context exists: the installed factory still supports future capture.
    assert!(!app
        .shortcut_actions()
        .unwrap()
        .iter()
        .any(|a| a.action_id == script));
    app.replace_registry(ActionRegistry::default());
    assert!(app.supported_shortcut_actions(&ids).unwrap().is_empty());
    app.replace_registry(registry());
    assert_eq!(app.supported_shortcut_actions(&ids).unwrap(), supported);
    assert!(app
        .supported_shortcut_actions(&vec!["x".into(); 2049])
        .is_err());
}

#[tokio::test]
async fn plugin_binding_registration_tracks_catalog_replacement_without_context() {
    let mut installed = registry();
    installed
        .register_plugin(anywhere_plugin::parse_manifest(crate::search::WRAP_PLUGIN).unwrap())
        .unwrap();
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        installed.clone(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let ids = vec!["wrap.status".into()];
    assert_eq!(app.supported_shortcut_actions(&ids).unwrap(), ids);
    app.replace_registry(registry());
    assert!(app.supported_shortcut_actions(&ids).unwrap().is_empty());
    app.replace_registry(installed);
    assert_eq!(app.supported_shortcut_actions(&ids).unwrap(), ids);
}
