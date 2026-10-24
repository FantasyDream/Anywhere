use crate::support::*;
use anywhere_core::application::settings_service::{SettingsChange, SettingsService};

#[tokio::test]
async fn history_entries_match_current_app_and_bind_independently_without_duplicate_targets() {
    for (bundle, source, other) in [
        ("com.jetbrains.intellij", "idea", "vscode"),
        ("com.microsoft.VSCode", "vscode", "idea"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let platform = Arc::new(TestPlatform {
            frontmost_bundle: Some(bundle),
            ..Default::default()
        });
        let app = app(platform);
        let mut settings =
            SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
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
