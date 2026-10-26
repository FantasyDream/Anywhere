use crate::support::*;
use anywhere_core::application::settings_service::{SettingsChange, SettingsService};

#[tokio::test]
async fn uninstalled_history_actions_are_absent_from_search_and_shortcut_targets_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(Arc::new(TestPlatform::new(tmp.path().into(), false)));
    let mut settings =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    let service = settings.application_history().unwrap();
    assert!(service
        .sources()
        .iter()
        .all(|source| !source.action_enabled));
    let id = app.activate().await.unwrap().unwrap().session_id;
    let first = settled(&app, id, 0).await;
    let action = "core.application_history.webstorm.open";
    assert!(app
        .resolve_action_binding(id, first.revision, action)
        .is_err());
    let off = app
        .update_input(id, "WebStorm Recent Projects".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(!off.candidates.iter().any(|row| row
        .command_id
        .as_deref()
        .is_some_and(|id| id.starts_with("core.application_history."))));
    settings
        .set_application_history_action_enabled(&app, "webstorm", true)
        .unwrap();
    app.deactivate(id).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    settled(&app, id, 0).await;
    let on = app
        .update_input(id, "WebStorm Recent Projects".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(on
        .candidates
        .iter()
        .any(|row| row.command_id.as_deref() == Some(action)));
    assert_eq!(
        app.resolve_action_binding(id, on.revision, action)
            .unwrap()
            .len(),
        1
    );
    use anywhere_core::features::application_history::model::SourceMode;
    assert!(settings
        .configure_application_history(&app, "webstorm", SourceMode::Custom, None)
        .is_err());
    assert_eq!(
        app.resolve_action_binding(id, on.revision, action)
            .unwrap()
            .len(),
        1
    );
    settings
        .configure_application_history(&app, "webstorm", SourceMode::Disabled, None)
        .unwrap();
    assert!(app.resolve_action_binding(id, on.revision, action).is_err());
}

#[tokio::test]
async fn a_broken_user_adapter_does_not_break_the_native_feature_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let directory = tmp.path().join("application-history-sources/broken");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("source.json"), "{invalid").unwrap();
    let app = app(Arc::new(TestPlatform::default()));
    let settings = SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    let statuses = settings.statuses();
    assert!(
        !statuses
            .iter()
            .find(|s| s.id == "application_history")
            .unwrap()
            .active
    );
    assert!(statuses.iter().find(|s| s.id == "git").unwrap().active);
    let first = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(app
        .resolve_action_binding(
            first.session_id,
            first.revision,
            "core.application_history.idea.open"
        )
        .is_err());
}

#[tokio::test]
async fn user_history_actions_register_once_and_switch_off_search_bindings_and_old_execution() {
    let tmp = tempfile::tempdir().unwrap();
    let directory = tmp.path().join("application-history-sources/example");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("source.json"), r#"{"schema":1,"id":"example","title":"Example","file_hint":"history.json","applications":["example.editor"],"action":{"title":"Example Recent Items"},"reader":{"format":"json"}}"#).unwrap();
    std::fs::write(
        directory.join("transform.js"),
        "export function transform() { return []; }",
    )
    .unwrap();
    let app = app(Arc::new(TestPlatform {
        frontmost_bundle: Some("example.editor"),
        directory: tmp.path().into(),
        ..Default::default()
    }));
    let mut settings =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    settings
        .set_application_history_action_enabled(&app, "example", true)
        .unwrap();
    let action = "core.application_history.example.open";
    let first = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let old = candidate(&first, action);
    assert_eq!(
        app.resolve_action_binding(first.session_id, first.revision, action)
            .unwrap()
            .len(),
        1
    );
    settings
        .set_application_history_action_enabled(&app, "example", false)
        .unwrap();
    assert!(app
        .execute(first.session_id, first.revision, old)
        .await
        .is_err());
    assert!(app
        .resolve_action_binding(first.session_id, first.revision, action)
        .is_err());
    app.deactivate(first.session_id).unwrap();
    let off = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(!off
        .context_candidates
        .iter()
        .any(|row| row.command_id.as_deref() == Some(action)));
    let query = app
        .update_input(off.session_id, "Example Recent Items".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(!query
        .candidates
        .iter()
        .any(|row| row.command_id.as_deref() == Some(action)));
    settings
        .set_application_history_action_enabled(&app, "example", true)
        .unwrap();
    app.deactivate(off.session_id).unwrap();
    let on = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(on
        .context_candidates
        .iter()
        .any(|row| row.command_id.as_deref() == Some(action)));
    assert_eq!(
        app.resolve_action_binding(on.session_id, on.revision, action)
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn history_entries_match_current_app_and_bind_independently_without_duplicate_targets() {
    for (bundle, source, other) in [
        ("com.jetbrains.intellij", "idea", "vscode"),
        ("com.microsoft.VSCode", "vscode", "idea"),
        ("com.todesktop.230313mzl4w4u92", "cursor", "vscode"),
        ("com.jetbrains.WebStorm", "webstorm", "idea"),
        ("md.obsidian", "obsidian", "idea"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(project.join(".git")).unwrap();
        std::fs::write(
            project.join("Cargo.toml"),
            "[package]\nname='history-home'\nversion='0.1.0'\n",
        )
        .unwrap();
        let platform = Arc::new(TestPlatform {
            frontmost_bundle: Some(bundle),
            directory: project,
            ..Default::default()
        });
        let app = app(platform);
        let mut settings =
            SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
        // Route tests explicitly enable their synthetic sources; installation defaults
        // are exercised separately with an empty installed-application snapshot.
        for source in [source, other] {
            settings
                .set_application_history_action_enabled(&app, source, true)
                .unwrap();
        }
        let id = app.activate().await.unwrap().unwrap().session_id;
        let state = settled(&app, id, 0).await;
        let own = format!("core.application_history.{source}.open");
        let another = format!("core.application_history.{other}.open");
        assert!(state
            .candidates
            .iter()
            .any(|row| row.command_id.as_deref() == Some(&own)));
        assert!(!state
            .candidates
            .iter()
            .any(|row| row.command_id.as_deref() == Some(&another)));
        // Home no longer promises a fixed slot: every available action competes
        // through the shared usage ordering, including the app history entry.
        assert!(state
            .context_candidates
            .iter()
            .any(|row| row.command_id.as_deref() == Some(&own)));
        assert!(state.scopes.iter().any(|scope| scope.id == "git"));
        assert!(state.scopes.iter().any(|scope| scope.id == "rust"));
        assert!(state
            .scopes
            .iter()
            .any(|scope| scope.id == format!("app.{bundle}") && scope.is_current_app));
        assert!(!state
            .scopes
            .iter()
            .any(|scope| scope.id == "tools" || scope.id == "history"));
        // A current-app row and its global entry represent one route, not two choices.
        let binding = app
            .resolve_action_binding(id, state.revision, &own)
            .unwrap();
        assert_eq!(binding.len(), 1);
        let output = app
            .execute(id, state.revision, binding[0].id.clone())
            .await
            .unwrap();
        let Some(HostView::NativeTool { id: page, input }) = output.view else {
            panic!("expected a history page")
        };
        assert_eq!(page, "application.history");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&input).unwrap()["source_id"],
            source
        );
        // The other app's explicit global shortcut has its own fixed route.
        let binding = app
            .resolve_action_binding(id, state.revision, &another)
            .unwrap();
        assert_eq!(binding.len(), 1);
        let output = app
            .execute(id, state.revision, binding[0].id.clone())
            .await
            .unwrap();
        let Some(HostView::NativeTool { input, .. }) = output.view else {
            panic!("expected a history page")
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&input).unwrap()["source_id"],
            other
        );
        settings
            .change(
                &app,
                SettingsChange::Enable {
                    key: "first-party:application_history".into(),
                    enabled: false,
                },
            )
            .unwrap();
        assert!(app
            .resolve_action_binding(id, state.revision, &own)
            .is_err());
    }
}

#[tokio::test]
async fn history_objects_reuse_actions_without_leaking_targets_to_usage_or_accepting_stale_offers()
{
    use anywhere_core::features::application_history::model::SourceMode;
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("history-only-project");
    std::fs::create_dir_all(project.join(".git")).unwrap();
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='history-only'\nversion='0.1.0'\n",
    )
    .unwrap();
    let workspace = project.join("multiple.code-workspace");
    std::fs::write(
        &workspace,
        r#"{"folders":[{"path":"first"},{"path":"second"}]}"#,
    )
    .unwrap();
    let source = temp.path().join("recent.xml");
    let document = |path: &Path| {
        format!("<application><component><option name='recentPaths'><list><option value='{}'/></list></option></component></application>", path.display())
    };
    std::fs::write(&source, document(&project)).unwrap();
    let platform = Arc::new(TestPlatform::default());
    let usage = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(usage.clone())),
        vec![],
    );
    let mut settings =
        SettingsService::open(temp.path().join("extensions.json"), vec![], &app).unwrap();
    let history = settings.application_history().unwrap();
    settings
        .set_application_history_action_enabled(&app, "idea", true)
        .unwrap();
    settings
        .configure_application_history(&app, "idea", SourceMode::Custom, source.to_str())
        .unwrap();
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let owner = history.open_session().unwrap();
    let item = history.search(owner, "", "idea").unwrap().items.remove(0);
    let actions = app
        .history_object_actions(
            history.clone(),
            state.session_id,
            state.revision,
            item.id.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        actions
            .first()
            .and_then(|action| action.command_id.as_deref()),
        Some("core.directory.copy")
    );
    for command in [
        "core.directory.copy",
        "core.git.status",
        "developer.cargo.check",
    ] {
        assert!(
            actions
                .iter()
                .any(|a| a.command_id.as_deref() == Some(command)),
            "missing {command}"
        );
    }
    let current = app.latest_state(state.session_id).unwrap();
    assert_eq!(
        current
            .scopes
            .iter()
            .filter(|scope| scope.is_current_app)
            .map(|scope| &scope.id)
            .collect::<Vec<_>>(),
        state
            .scopes
            .iter()
            .filter(|scope| scope.is_current_app)
            .map(|scope| &scope.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        platform.git_calls.load(Ordering::Relaxed),
        0,
        "discovery must not run Git commands"
    );
    let copy = actions
        .iter()
        .find(|a| a.command_id.as_deref() == Some("core.directory.copy"))
        .unwrap()
        .id
        .clone();
    let result = app
        .execute(state.session_id, state.revision, copy.clone())
        .await
        .unwrap();
    assert!(result.succeeded());
    assert_eq!(
        platform.calls.lock().unwrap().last(),
        Some(&(
            "copy".into(),
            project
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        ))
    );
    let records = usage.lock().unwrap().load_global().unwrap();
    assert!(records
        .keys()
        .any(|(action, kind)| action == "core.directory.copy" && kind == DIRECTORY));
    assert!(!format!("{records:?}").contains("history-only-project"));

    // Same Action and path, new object-actions request: the old offer is revoked.
    let second = app
        .history_object_actions(
            history.clone(),
            state.session_id,
            state.revision,
            item.id.clone(),
        )
        .await
        .unwrap();
    assert!(app
        .execute(state.session_id, state.revision, copy)
        .await
        .is_err());
    let current = second
        .iter()
        .find(|a| a.command_id.as_deref() == Some("core.directory.copy"))
        .unwrap()
        .id
        .clone();
    std::fs::write(&source, document(&workspace)).unwrap();
    let calls = platform.calls.lock().unwrap().len();
    assert!(!app
        .execute(state.session_id, state.revision, current)
        .await
        .unwrap()
        .succeeded());
    assert_eq!(platform.calls.lock().unwrap().len(), calls);
    let code_source = temp.path().join("state.vscdb");
    let db = rusqlite::Connection::open(&code_source).unwrap();
    db.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT)")
        .unwrap();
    let value = serde_json::json!({"entries":[{"workspace":{"id":"multi-root","configPath":url::Url::from_file_path(&workspace).unwrap().as_str()}}]}).to_string();
    db.execute(
        "INSERT INTO ItemTable VALUES (?1,?2)",
        rusqlite::params!["history.recentlyOpenedPathsList", value],
    )
    .unwrap();
    // Simulate an out-of-band change: execution must revalidate the source even
    // before an application-level catalog refresh. Normal settings use the seam above.
    history.set_action_enabled("vscode", true).unwrap();
    history
        .configure_mode("vscode", SourceMode::Custom, code_source.to_str())
        .unwrap();
    let file = history.search(owner, "", "vscode").unwrap().items.remove(0);
    let actions = app
        .history_object_actions(history.clone(), state.session_id, state.revision, file.id)
        .await
        .unwrap();
    assert!(actions
        .iter()
        .any(|a| a.command_id.as_deref() == Some("core.file.copy")));
    assert!(actions
        .iter()
        .all(|a| a.group_id != "git" && a.group_id != "rust"));
    let current = actions
        .iter()
        .find(|a| a.command_id.as_deref() == Some("core.file.copy"))
        .unwrap()
        .id
        .clone();
    history.set_action_enabled("vscode", false).unwrap();
    let calls = platform.calls.lock().unwrap().len();
    assert!(!app
        .execute(state.session_id, state.revision, current)
        .await
        .unwrap()
        .succeeded());
    assert_eq!(platform.calls.lock().unwrap().len(), calls);
}
