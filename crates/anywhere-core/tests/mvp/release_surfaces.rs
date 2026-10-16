//! Deferred tools must not leave executable entries or activation suggestions.
use crate::support::*;
use anywhere_core::application::settings_service::SettingsService;

#[tokio::test]
async fn deferred_tool_has_no_actions_shortcuts_scope_or_prefill_but_preserves_settings() {
    let temp = tempfile::tempdir().unwrap();
    let feature_path = temp.path().join("features.json");
    let original = br#"{"schema_version":1,"disabled":["json","future_tool"]}"#;
    std::fs::write(&feature_path, original).unwrap();
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let settings =
        SettingsService::open(temp.path().join("extensions.json"), vec![], &app).unwrap();
    assert!(settings.statuses().iter().all(|status| status.id != "json"));
    assert_eq!(std::fs::read(feature_path).unwrap(), original);
    assert!(app
        .supported_shortcut_actions(&[
            "core.json.workspace".into(),
            "core.json.format".into(),
            "core.json.minify".into(),
            "core.json.copy".into(),
        ])
        .unwrap()
        .is_empty());
    let session = app.activate().await.unwrap().unwrap().session_id;
    for (revision, input) in [(1, "json"), (2, "{\"key\":1}")] {
        app.update_input(session, input.into(), revision)
            .await
            .unwrap();
        let state = settled(&app, session, revision).await;
        assert!(state
            .candidates
            .iter()
            .chain(&state.context_candidates)
            .all(|row| !row
                .command_id
                .as_deref()
                .unwrap_or_default()
                .starts_with("core.json.")));
        assert!(state.scopes.iter().all(|scope| scope.id != "json"));
        assert!(state
            .context_summary
            .iter()
            .all(|context| context.type_id != JSON_DOCUMENT));
    }
    for source in [ContextSource::SelectedText, ContextSource::Clipboard] {
        let capture = ContextCapturePresentation {
            source,
            status: ContextCaptureStatus::Captured,
            captured_at_ms: 1000,
            clipboard_age_ms: Some(0),
            text: Some("{\"key\":1}".into()),
        };
        assert!(app
            .choose_activation_input(&[capture], 1000, true, true, true)
            .is_none());
    }
}
