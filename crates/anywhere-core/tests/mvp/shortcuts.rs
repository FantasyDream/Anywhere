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
