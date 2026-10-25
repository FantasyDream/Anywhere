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
    service.set_action_enabled("webstorm", true).unwrap();
    settings.synchronize_application_history(&app).unwrap();
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
    service.set_action_enabled("webstorm", false).unwrap();
    settings.synchronize_application_history(&app).unwrap();
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
        .application_history()
        .unwrap()
        .set_action_enabled("example", true)
        .unwrap();
    settings.synchronize_application_history(&app).unwrap();
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
        .application_history()
        .unwrap()
        .set_action_enabled("example", false)
        .unwrap();
    settings.synchronize_application_history(&app).unwrap();
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
        .application_history()
        .unwrap()
        .set_action_enabled("example", true)
        .unwrap();
    settings.synchronize_application_history(&app).unwrap();
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
                .application_history()
                .unwrap()
                .set_action_enabled(source, true)
                .unwrap();
        }
        settings.synchronize_application_history(&app).unwrap();
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
        // Match the host's smaller, five-action Home when Browse Menus is present.
        // A project adds more eligible actions than fit; merely matching is insufficient.
        let home: Vec<_> = state
            .context_candidates
            .iter()
            .filter(|row| row.home_eligible)
            .collect();
        assert!(home.len() > 5, "fixture must overflow Home: {home:?}");
        assert!(home
            .iter()
            .take(5)
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
