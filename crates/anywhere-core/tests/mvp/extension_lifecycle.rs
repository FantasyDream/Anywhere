use crate::support::*;
use anywhere_core::application::settings_service::{ModuleStatus, SettingsChange, SettingsService};

fn status(manager: &SettingsService, key: &str) -> ModuleStatus {
    manager
        .statuses()
        .into_iter()
        .find(|s| s.key == key)
        .unwrap()
}
fn enable(key: &str, enabled: bool) -> SettingsChange {
    SettingsChange::Enable {
        key: key.into(),
        enabled,
    }
}
fn app() -> Application {
    Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    )
}

#[tokio::test]
async fn clipboard_uses_regular_action_navigation_and_disable_revokes_history_access() {
    let temporary = tempfile::tempdir().unwrap();
    let app = app();
    let mut manager =
        SettingsService::open(temporary.path().join("extensions.json"), &app).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "clipboard".into(), 1).await.unwrap();
    let state = settled(&app, id, 1).await;
    let shelf = candidate(&state, "core.clipboard.shelf");
    assert!(matches!(app.execute(id, 1, shelf).await.unwrap().view,
        Some(HostView::NativeTool { id, .. }) if id == "clipboard.shelf"));
    assert!(!manager.clipboard().status().unwrap().recording);
    let row = candidate(&state, "core.clipboard.history");
    assert!(matches!(app.execute(id, 1, row).await.unwrap().view,
        Some(HostView::NativeTool { id, .. }) if id == "clipboard.history"));
    let history = manager.clipboard();
    history.set_recording(true).unwrap();
    manager
        .change(&app, enable("first-party:clipboard", false))
        .unwrap();
    assert!(!history.status().unwrap().available);
    assert!(history.search("").is_err());
    assert!(history.set_recording(true).is_err());
    assert!(app.shortcut_actions().unwrap().iter().all(|a| ![
        "core.clipboard.history",
        "core.clipboard.shelf"
    ]
    .contains(&a.action_id.as_str())));
    manager
        .change(&app, enable("first-party:clipboard", true))
        .unwrap();
    assert!(history.status().unwrap().available);
    assert!(history.status().unwrap().recording);
    assert!(history.search("").unwrap().is_empty());
}

#[tokio::test]
async fn native_status_uses_app_version_and_preserves_metadata_through_disable_and_restart() {
    use anywhere_core::features::descriptor::APP_VERSION;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    let app = app();
    let mut manager = SettingsService::open(path.clone(), &app).unwrap();
    let native: Vec<_> = manager
        .statuses()
        .into_iter()
        .filter(|s| s.first_party)
        .collect();
    let expected: [(&str, &str, &[&str]); 10] = [
        ("git", "Git", &["git.read", "clipboard.write"]),
        ("url", "URL", &["url.open", "clipboard.write"]),
        ("rust", "Rust", &["process.start"]),
        ("maven", "Maven", &["process.start"]),
        ("gradle", "Gradle", &["process.start"]),
        ("node", "Node", &["process.start"]),
        ("spring_boot", "Spring Boot", &["process.start"]),
        (
            "saved_directories",
            "Saved Directories",
            &["storage", "file.open"],
        ),
        (
            "clipboard",
            "Clipboard History",
            &["storage", "clipboard.write", "ui.render"],
        ),
        (
            "application_history",
            "App History",
            &["storage", "ui.render", "file.open", "clipboard.write"],
        ),
    ];
    assert_eq!(native.len(), expected.len());
    for (record, (id, name, capabilities)) in native.iter().zip(expected) {
        assert_eq!(record.key, format!("first-party:{id}"));
        assert_eq!(record.id, id);
        assert_eq!(record.name, name);
        assert_eq!(record.capabilities, capabilities);
        assert_eq!(record.version, APP_VERSION);
        assert!(record.active && record.enabled && record.configurable);
    }
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    let disabled = status(&manager, "first-party:url");
    assert!(!disabled.enabled && !disabled.active);
    assert_eq!(disabled.version, APP_VERSION);
    drop(manager);
    let boot = self::app();
    let manager = SettingsService::open(path, &boot).unwrap();
    let restarted = status(&manager, "first-party:url");
    assert!(!restarted.active && !restarted.enabled);
    assert_eq!(restarted.name, "URL");
    assert_eq!(restarted.capabilities, ["url.open", "clipboard.write"]);
    assert_eq!(restarted.version, APP_VERSION);
}

#[tokio::test]
async fn module_toggle_revokes_retained_session_recognizers_and_shortcuts_and_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    let app = app();
    let mut manager = SettingsService::open(path.clone(), &app).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.org/project".into(), 1)
        .await
        .unwrap();
    let old = settled(&app, id, 1).await;
    let row = candidate(&old, "core.url.copy");
    app.suspend_session(id).await.unwrap();
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    assert!(!status(&manager, "first-party:url").active);
    assert!(app.resume_session(id).unwrap().is_none());
    assert!(app.execute(id, 1, row).await.is_err());
    assert!(app
        .supported_shortcut_actions(&["core.url.copy".into()])
        .unwrap()
        .is_empty());
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.org/project".into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    assert!(!state.context_summary.iter().any(|c| c.type_id == URL));
    assert!(!state.scopes.iter().any(|s| s.id == "web"));
    let restarted = self::app();
    let mut manager = SettingsService::open(path, &restarted).unwrap();
    assert!(!status(&manager, "first-party:url").active);
    manager
        .change(&restarted, enable("first-party:url", true))
        .unwrap();
    assert_eq!(
        restarted
            .supported_shortcut_actions(&["core.url.copy".into()])
            .unwrap(),
        ["core.url.copy"]
    );
    let id = restarted.activate().await.unwrap().unwrap().session_id;
    restarted
        .update_input(id, "https://example.org/project".into(), 1)
        .await
        .unwrap();
    assert!(settled(&restarted, id, 1)
        .await
        .context_summary
        .iter()
        .any(|c| c.type_id == URL));
}

#[tokio::test]
async fn failed_save_rolls_back_catalog_preferences_and_live_session() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    let app = app();
    let mut manager = SettingsService::open(path.clone(), &app).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.org/project".into(), 1)
        .await
        .unwrap();
    let row = candidate(&settled(&app, id, 1).await, "core.url.copy");
    let policy = manager.feature_policy();
    std::fs::remove_file(tmp.path().join("features.json")).unwrap();
    std::fs::create_dir(tmp.path().join("features.json")).unwrap();
    assert!(manager
        .change(&app, enable("first-party:url", false))
        .is_err());
    assert!(status(&manager, "first-party:url").active);
    assert!(Arc::ptr_eq(&policy, &manager.feature_policy()));
    assert!(app.execute(id, 1, row).await.is_ok());
    let non_wal_entries = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !name.ends_with("-wal") && !name.ends_with("-shm")
        })
        .count();
    assert_eq!(non_wal_entries, 3);
}

#[tokio::test]
async fn a_provider_completion_from_the_old_configuration_cannot_republish_context() {
    struct Slow {
        descriptor: ProviderDescriptor,
        started: Arc<Notify>,
        release: Arc<Notify>,
    }
    #[async_trait]
    impl ContextProvider for Slow {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }
        async fn resolve(
            &self,
            _: &ResolvedContext,
            _: &ExecutionContext,
        ) -> Result<Vec<ResolvedContext>> {
            self.started.notify_one();
            self.release.notified().await;
            Ok(vec![ResolvedContext::new(
                URL,
                ContextValue::Text("https://example.org/old".into()),
                ContextSource::UserInput,
                "old-provider",
            )])
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app();
    let mut manager = SettingsService::open(tmp.path().join("extensions.json"), &app).unwrap();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    app.register_provider(Arc::new(Slow {
        descriptor: ProviderDescriptor {
            id: "old.slow".into(),
            accepts: vec![TEXT.into()],
            produces: vec![URL.into()],
            priority: 0,
        },
        started: started.clone(),
        release: release.clone(),
    }))
    .unwrap();
    let app = Arc::new(app);
    let id = app.activate().await.unwrap().unwrap().session_id;
    let pending = tokio::spawn({
        let app = app.clone();
        async move { app.update_input(id, "hello".into(), 1).await }
    });
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    release.notify_one();
    pending.await.unwrap().unwrap();
    assert!(app.latest_state(id).is_none());
    let fresh = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(fresh, "hello".into(), 1).await.unwrap();
    assert!(!settled(&app, fresh, 1)
        .await
        .context_summary
        .iter()
        .any(|c| c.type_id == URL));
}

#[tokio::test]
async fn invalid_preferences_preserve_file_and_base_launcher_with_read_only_recovery() {
    for broken in [
        r#"{"schema_version":99,"disabled":[],"approvals":{}}"#,
        "not json",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("extensions.json");
        std::fs::write(&path, broken).unwrap();
        let app = app();
        let mut manager = SettingsService::open(path.clone(), &app).unwrap();
        assert!(manager
            .statuses()
            .iter()
            .all(|s| !s.active && !s.configurable));
        assert!(manager
            .change(&app, enable("first-party:url", true))
            .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        let id = app.activate().await.unwrap().unwrap().session_id;
        app.update_context_input(
            id,
            tmp.path().to_str().unwrap().into(),
            1,
            None,
            InputMode::Search,
        )
        .await
        .unwrap();
        let state = settled(&app, id, 1).await;
        assert!(state
            .candidates
            .iter()
            .any(|c| c.command_id.as_deref() == Some("core.directory.open")));
        std::fs::write(
            &path,
            r#"{"schema_version":1,"disabled":[],"approvals":{}}"#,
        )
        .unwrap();
        manager.refresh(&app).unwrap();
        assert!(status(&manager, "first-party:url").active);
    }
}

#[tokio::test]
async fn disabling_url_keeps_other_modules_and_restoring_recovers_web_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app();
    let mut manager = SettingsService::open(tmp.path().join("extensions.json"), &app).unwrap();
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    assert!(status(&manager, "first-party:git").active);
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.com".into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    assert!(!state.context_summary.iter().any(|c| c.type_id == URL));
    assert!(!state.scopes.iter().any(|s| s.id == "web"));
    assert_eq!(
        app.supported_shortcut_actions(&["core.url.open".into(), "core.git.status".into()])
            .unwrap(),
        ["core.git.status"]
    );
    manager
        .change(&app, enable("first-party:url", true))
        .unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.com".into(), 1)
        .await
        .unwrap();
    assert!(settled(&app, id, 1)
        .await
        .scopes
        .iter()
        .any(|s| s.id == "web"));
}

#[test]
fn native_migration_does_not_reimport_legacy_disable_keys() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("extensions.json");
    let legacy = serde_json::json!({
        "schema_version": 1,
        "disabled": ["first-party:url", "first-party:future_tool"],
        "approvals": {"plugin:controlled": "retained"}
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let app = self::app();
    let mut manager = SettingsService::open(path.clone(), &app).unwrap();
    assert!(!status(&manager, "first-party:url").active);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let policy = manager.feature_policy();
    assert!(!policy.is_enabled("url") && !policy.is_enabled("future_tool"));
    manager
        .change(&app, enable("first-party:url", true))
        .unwrap();
    let current = manager.feature_policy();
    assert_eq!(current.generation(), policy.generation() + 1);
    assert!(!policy.is_enabled("url"));
    assert!(current.is_enabled("url"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    manager.refresh(&app).unwrap();
    assert!(Arc::ptr_eq(&current, &manager.feature_policy()));
    assert!(status(&manager, "first-party:url").active);
    let manager = SettingsService::open(path, &self::app()).unwrap();
    assert!(status(&manager, "first-party:url").active);
    assert!(!manager.feature_policy().is_enabled("future_tool"));
}

#[test]
fn invalid_feature_file_enters_read_only_recovery_until_repaired() {
    use anywhere_core::persistence::features::FeaturePreferences;
    for broken in ["not JSON", r#"{"schema_version":99,"disabled":[]}"#] {
        let temporary = tempfile::tempdir().unwrap();
        let feature_path = temporary.path().join("features.json");
        std::fs::write(&feature_path, broken).unwrap();
        let app = app();
        let mut manager =
            SettingsService::open(temporary.path().join("extensions.json"), &app).unwrap();
        assert!(!status(&manager, "first-party:url").active);
        assert!(!status(&manager, "first-party:url").configurable);
        let recovery = manager.feature_policy();
        assert!(!recovery.is_configurable());
        assert!(manager
            .change(&app, enable("first-party:url", true))
            .is_err());
        assert!(Arc::ptr_eq(&recovery, &manager.feature_policy()));
        assert_eq!(std::fs::read_to_string(&feature_path).unwrap(), broken);
        FeaturePreferences::default().save(&feature_path).unwrap();
        manager.refresh(&app).unwrap();
        assert!(status(&manager, "first-party:url").active);
        assert_eq!(
            manager.feature_policy().generation(),
            recovery.generation() + 1
        );
    }
}

#[test]
fn preferences_damaged_after_open_are_not_overwritten_by_a_stale_settings_change() {
    let temporary = tempfile::tempdir().unwrap();
    let app = app();
    let mut manager =
        SettingsService::open(temporary.path().join("extensions.json"), &app).unwrap();
    let policy = manager.feature_policy();
    let path = temporary.path().join("features.json");
    let broken = r#"{"schema_version":99,"disabled":[]}"#;
    std::fs::write(&path, broken).unwrap();
    assert!(manager
        .change(&app, enable("first-party:url", false))
        .is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), broken);
    assert!(status(&manager, "first-party:url").active);
    assert!(Arc::ptr_eq(&policy, &manager.feature_policy()));
}
