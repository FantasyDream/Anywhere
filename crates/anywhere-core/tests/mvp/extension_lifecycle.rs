use crate::support::*;
use anywhere_core::application::settings_service::{ModuleStatus, SettingsChange, SettingsService};
const V2: &str = r#"protocol_version = 2
version = "1.0.0"
capabilities = ["clipboard.write"]
id = "controlled"
name = "Controlled Copy"
[[commands]]
id = "copy"
title = "Controlled Copy"
accepts = ["core.text"]
action = {type = "copy", text = "{{context.value}}"}
"#;

#[tokio::test]
async fn ui_definition_changes_require_restart_review_and_revoke_retained_views() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("detail.toml");
    let raw = include_str!("../../../../plugins/examples/context-detail.toml");
    std::fs::write(&manifest, raw).unwrap();
    let boot = app();
    let path = tmp.path().join("extensions.json");
    let dirs = vec![tmp.path().into()];
    let mut inspected = SettingsService::open(path.clone(), dirs.clone(), &boot).unwrap();
    let before = status(&inspected, "plugin:example_detail");
    assert!(before.needs_approval);
    assert_eq!(before.capabilities, ["ui.render"]);
    inspected
        .change(
            &boot,
            SettingsChange::Approve {
                key: before.key.clone(),
                token: before.approval_token,
            },
        )
        .unwrap();
    assert!(!status(&inspected, &before.key).active);
    assert!(status(&inspected, &before.key).detail.contains("Restart"));
    let app = app();
    let mut loaded = SettingsService::open(path, dirs, &app).unwrap();
    assert!(status(&loaded, &before.key).active);
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "inspected object".into(), 1)
        .await
        .unwrap();
    let row = candidate(&settled(&app, id, 1).await, "example_detail.inspect");
    assert!(matches!(
        app.execute(id, 1, row.clone()).await.unwrap().view,
        Some(HostView::PluginDetail { .. })
    ));
    app.suspend_session(id).await.unwrap();
    // Revocation remains immediate; startup-only loading cannot keep an old offer alive.
    loaded
        .change(
            &app,
            SettingsChange::Revoke {
                key: before.key.clone(),
            },
        )
        .unwrap();
    assert!(app.resume_session(id).unwrap().is_none());
    assert!(app.execute(id, 1, row.clone()).await.is_err());
    std::fs::write(&manifest, raw.replace("Selected object", "Changed label")).unwrap();
    loaded.refresh(&app).unwrap();
    let changed = status(&loaded, &before.key);
    assert!(changed.needs_approval && !changed.active && !changed.configurable);
    assert!(loaded
        .change(
            &app,
            SettingsChange::Approve {
                key: changed.key,
                token: changed.approval_token
            }
        )
        .is_err());
}

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
fn approve(manager: &SettingsService) -> SettingsChange {
    let s = status(manager, "plugin:controlled");
    SettingsChange::Approve {
        key: s.key,
        token: s.approval_token,
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
async fn native_status_uses_app_version_and_preserves_metadata_through_disable_and_restart() {
    use anywhere_core::features::descriptor::APP_VERSION;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("plugin.toml"), V2).unwrap();
    let path = tmp.path().join("extensions.json");
    let directories = vec![tmp.path().into()];
    let app = app();
    let mut manager = SettingsService::open(path.clone(), directories.clone(), &app).unwrap();
    let native: Vec<_> = manager
        .statuses()
        .into_iter()
        .filter(|s| s.first_party)
        .collect();
    let expected: [(&str, &str, &[&str]); 8] = [
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
    ];
    assert_eq!(native.len(), expected.len());
    for (record, (id, name, capabilities)) in native.iter().zip(expected) {
        assert_eq!(record.key, format!("first-party:{id}"));
        assert_eq!(record.id, id);
        assert_eq!(record.name, name);
        assert_eq!(record.capabilities, capabilities);
        assert_eq!(record.version, APP_VERSION);
        assert!(record.active && record.enabled && record.configurable);
        assert!(!record.needs_approval && !record.legacy);
        assert!(record.approval_token.is_empty());
    }
    // External package versions remain independent, including unapproved packages.
    assert_eq!(status(&manager, "plugin:controlled").version, "1.0.0");
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    let disabled = status(&manager, "first-party:url");
    assert!(!disabled.enabled && !disabled.active);
    assert_eq!(disabled.version, APP_VERSION);
    drop(manager);
    let boot = self::app();
    let manager = SettingsService::open(path, directories, &boot).unwrap();
    let restarted = status(&manager, "first-party:url");
    assert!(!restarted.active && !restarted.enabled);
    assert_eq!(restarted.name, "URL");
    assert_eq!(restarted.capabilities, ["url.open", "clipboard.write"]);
    assert_eq!(restarted.version, APP_VERSION);
    assert_eq!(status(&manager, "plugin:controlled").version, "1.0.0");
}

#[tokio::test]
async fn module_toggle_revokes_retained_session_recognizers_and_shortcuts_and_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    let app = app();
    let mut manager = SettingsService::open(path.clone(), vec![], &app).unwrap();
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
    let mut manager = SettingsService::open(path, vec![], &restarted).unwrap();
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
async fn approval_persists_but_updates_new_permissions_and_revocation_block_old_actions() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("plugin.toml");
    std::fs::write(&manifest, V2).unwrap();
    let path = tmp.path().join("extensions.json");
    let dirs = vec![tmp.path().into()];
    let platform = Arc::new(TestPlatform::default());
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let mut manager = SettingsService::open(path.clone(), dirs.clone(), &app).unwrap();
    assert!(status(&manager, "plugin:controlled").needs_approval);
    assert!(app
        .supported_shortcut_actions(&["controlled.copy".into()])
        .unwrap()
        .is_empty());
    let request = approve(&manager);
    manager.change(&app, request).unwrap();
    assert!(!status(&manager, "plugin:controlled").active);
    assert!(status(&manager, "plugin:controlled")
        .detail
        .contains("Restart"));
    assert!(app
        .supported_shortcut_actions(&["controlled.copy".into()])
        .unwrap()
        .is_empty());
    let restarted = self::app();
    let mut loaded = SettingsService::open(path.clone(), dirs.clone(), &restarted).unwrap();
    assert!(status(&loaded, "plugin:controlled").active);
    let id = restarted.activate().await.unwrap().unwrap().session_id;
    restarted
        .update_input(id, "approved text".into(), 1)
        .await
        .unwrap();
    let row = candidate(&settled(&restarted, id, 1).await, "controlled.copy");
    assert_eq!(
        restarted.execute(id, 1, row.clone()).await.unwrap().status,
        ActionStatus::Succeeded
    );
    loaded
        .change(
            &restarted,
            SettingsChange::Revoke {
                key: "plugin:controlled".into(),
            },
        )
        .unwrap();
    assert!(restarted.execute(id, 1, row.clone()).await.is_err());
    assert!(restarted
        .supported_shortcut_actions(&["controlled.copy".into()])
        .unwrap()
        .is_empty());
    // The approved startup definition stays captured; file changes require a fresh process.
    std::fs::write(
        &manifest,
        V2.replace(
            "[\"clipboard.write\"]",
            "[\"clipboard.write\", \"url.open\"]",
        ),
    )
    .unwrap();
    loaded.refresh(&restarted).unwrap();
    assert!(status(&loaded, "plugin:controlled").needs_approval);
    assert!(loaded.change(&restarted, approve(&loaded)).is_err());
    let fresh = self::app();
    let inspected = SettingsService::open(path, dirs, &fresh).unwrap();
    assert!(status(&inspected, "plugin:controlled").needs_approval);
    assert!(status(&inspected, "plugin:controlled")
        .capabilities
        .contains(&"url.open".into()));
    assert!(platform.calls.lock().unwrap().is_empty());
}

#[test]
fn stale_ui_and_replaced_installations_cannot_receive_a_grant() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("plugin.toml");
    std::fs::write(&manifest, V2).unwrap();
    let app = app();
    let mut manager = SettingsService::open(
        tmp.path().join("extensions.json"),
        vec![tmp.path().into()],
        &app,
    )
    .unwrap();
    let stale = SettingsChange::Approve {
        key: "plugin:controlled".into(),
        token: "previous-startup".into(),
    };
    manager.refresh(&app).unwrap();
    assert!(manager.change(&app, stale).is_err());
    let stale = approve(&manager);
    std::fs::write(&manifest, V2.replace("{{context.value}}", "changed target")).unwrap();
    assert!(manager.change(&app, stale).is_err());
    assert!(!tmp.path().join("extensions.json").exists());
    std::fs::write(&manifest, V2).unwrap();
    let request = approve(&manager);
    manager.change(&app, request).unwrap();
    // Same id/definition at a new path is a different installation.
    std::fs::rename(&manifest, tmp.path().join("replacement.toml")).unwrap();
    manager.refresh(&app).unwrap();
    assert!(status(&manager, "plugin:controlled").needs_approval);
}

#[test]
fn duplicate_ids_block_every_installation_and_legacy_can_be_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("one.toml"), V2).unwrap();
    std::fs::write(tmp.path().join("two.toml"), V2).unwrap();
    std::fs::write(
        tmp.path().join("legacy.toml"),
        anywhere_plugin::EXAMPLE_PLUGIN,
    )
    .unwrap();
    let app = app();
    let mut manager = SettingsService::open(
        tmp.path().join("extensions.json"),
        vec![tmp.path().into(), tmp.path().into()],
        &app,
    )
    .unwrap();
    let duplicate = manager
        .statuses()
        .into_iter()
        .find(|s| s.id == "controlled")
        .unwrap();
    assert!(!duplicate.configurable && !duplicate.active);
    assert!(duplicate.detail.contains("Duplicate"));
    assert!(status(&manager, "plugin:example").active);
    manager
        .change(&app, enable("plugin:example", false))
        .unwrap();
    assert!(!status(&manager, "plugin:example").active);
    assert!(app
        .supported_shortcut_actions(&["example.search".into()])
        .unwrap()
        .is_empty());
    manager
        .change(&app, enable("plugin:example", true))
        .unwrap();
    assert!(status(&manager, "plugin:example").active);
}

#[tokio::test]
async fn failed_save_rolls_back_catalog_preferences_and_live_session() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    let app = app();
    let mut manager = SettingsService::open(path.clone(), vec![], &app).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.org/project".into(), 1)
        .await
        .unwrap();
    let row = candidate(&settled(&app, id, 1).await, "core.url.copy");
    let policy = manager.feature_policy();
    std::fs::remove_file(tmp.path().join("features.json")).unwrap();
    std::fs::create_dir(tmp.path().join("features.json")).unwrap(); // Destination cannot be atomically replaced.
    assert!(manager
        .change(&app, enable("first-party:url", false))
        .is_err());
    assert!(status(&manager, "first-party:url").active);
    assert!(Arc::ptr_eq(&policy, &manager.feature_policy()));
    assert!(app.execute(id, 1, row).await.is_ok());
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 2); // Temp file cleaned up.
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
    let mut manager =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
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
        let mut manager = SettingsService::open(path.clone(), vec![], &app).unwrap();
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
        // Repair externally; Reload restores the valid policy without restarting Core.
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
    let mut manager =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
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
fn native_migration_preserves_approval_and_does_not_reimport_legacy_disable_keys() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(temporary.path().join("plugin.toml"), V2).unwrap();
    let path = temporary.path().join("extensions.json");
    let feature_path = temporary.path().join("features.json");
    let boot = app();
    let mut initial =
        SettingsService::open(path.clone(), vec![temporary.path().into()], &boot).unwrap();
    let request = approve(&initial);
    initial.change(&boot, request).unwrap();
    // Reconstruct a pre-migration installation with a real, origin-bound approval.
    drop(initial);
    std::fs::remove_file(&feature_path).unwrap();
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    legacy["disabled"] = serde_json::json!(["first-party:url", "first-party:future_tool"]);
    std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let app = self::app();
    let mut manager =
        SettingsService::open(path.clone(), vec![temporary.path().into()], &app).unwrap();
    assert!(!status(&manager, "first-party:url").active);
    assert!(status(&manager, "plugin:controlled").active);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let policy = manager.feature_policy();
    assert!(!policy.is_enabled("url") && !policy.is_enabled("future_tool"));
    manager
        .change(&app, enable("first-party:url", true))
        .unwrap();
    let current = manager.feature_policy();
    assert_eq!(current.generation(), policy.generation() + 1);
    assert!(!policy.is_enabled("url")); // Published snapshots cannot be mutated in place.
    assert!(current.is_enabled("url"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes); // A native change writes only features.json.
    manager
        .change(&app, enable("plugin:controlled", false))
        .unwrap();
    assert!(Arc::ptr_eq(&current, &manager.feature_policy()));
    let external: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(external["approvals"], legacy["approvals"]);
    assert!(external["disabled"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("first-party:url")));
    manager.refresh(&app).unwrap();
    assert!(Arc::ptr_eq(&current, &manager.feature_policy()));
    assert!(status(&manager, "first-party:url").active);
    let manager = SettingsService::open(path, vec![temporary.path().into()], &self::app()).unwrap();
    assert!(status(&manager, "first-party:url").active);
    assert!(!status(&manager, "plugin:controlled").active);
    assert!(!status(&manager, "plugin:controlled").needs_approval);
    assert!(!manager.feature_policy().is_enabled("future_tool"));
}

#[test]
fn invalid_feature_file_is_read_only_without_blocking_external_settings() {
    use anywhere_core::persistence::features::FeaturePreferences;
    for broken in ["not JSON", r#"{"schema_version":99,"disabled":[]}"#] {
        let temporary = tempfile::tempdir().unwrap();
        std::fs::write(
            temporary.path().join("plugin.toml"),
            anywhere_plugin::EXAMPLE_PLUGIN,
        )
        .unwrap();
        let feature_path = temporary.path().join("features.json");
        std::fs::write(&feature_path, broken).unwrap();
        let app = app();
        let mut manager = SettingsService::open(
            temporary.path().join("extensions.json"),
            vec![temporary.path().into()],
            &app,
        )
        .unwrap();
        assert!(!status(&manager, "first-party:url").active);
        assert!(!status(&manager, "first-party:url").configurable);
        assert!(status(&manager, "plugin:example").active);
        let recovery = manager.feature_policy();
        assert!(!recovery.is_configurable());
        assert!(manager
            .change(&app, enable("first-party:url", true))
            .is_err());
        manager
            .change(&app, enable("plugin:example", false))
            .unwrap();
        assert!(Arc::ptr_eq(&recovery, &manager.feature_policy()));
        assert_eq!(std::fs::read_to_string(&feature_path).unwrap(), broken);
        FeaturePreferences::default().save(&feature_path).unwrap(); // Explicit external repair.
        manager.refresh(&app).unwrap();
        assert!(status(&manager, "first-party:url").active);
        assert!(!status(&manager, "plugin:example").active);
        assert_eq!(
            manager.feature_policy().generation(),
            recovery.generation() + 1
        );
    }
}

#[test]
fn invalid_external_preferences_do_not_block_existing_native_configuration() {
    use anywhere_core::persistence::{extensions::PluginPreferences, features::FeaturePreferences};
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(
        temporary.path().join("plugin.toml"),
        anywhere_plugin::EXAMPLE_PLUGIN,
    )
    .unwrap();
    FeaturePreferences::default()
        .save(&temporary.path().join("features.json"))
        .unwrap();
    let path = temporary.path().join("extensions.json");
    let broken = r#"{"schema_version":99,"disabled":[],"approvals":{}}"#;
    std::fs::write(&path, broken).unwrap();
    let app = app();
    let mut manager =
        SettingsService::open(path.clone(), vec![temporary.path().into()], &app).unwrap();
    assert!(status(&manager, "first-party:url").active);
    assert!(!status(&manager, "plugin:example").active);
    assert!(!status(&manager, "plugin:example").configurable);
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    let current = manager.feature_policy();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    assert!(manager
        .change(&app, enable("plugin:example", true))
        .is_err());
    PluginPreferences::default().save(&path).unwrap(); // Explicit external repair.
    manager.refresh(&app).unwrap();
    assert!(Arc::ptr_eq(&current, &manager.feature_policy()));
    assert!(!status(&manager, "plugin:example").active);
    assert!(status(&manager, "plugin:example")
        .detail
        .contains("Restart"));
    let boot = self::app();
    let restarted = SettingsService::open(path, vec![temporary.path().into()], &boot).unwrap();
    assert!(status(&restarted, "plugin:example").active);
    assert!(!status(&manager, "first-party:url").active);
}

#[tokio::test]
async fn failed_external_save_preserves_native_policy_and_live_plugin_execution() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(
        temporary.path().join("plugin.toml"),
        anywhere_plugin::EXAMPLE_PLUGIN,
    )
    .unwrap();
    let path = temporary.path().join("extensions.json");
    let app = app();
    let mut manager =
        SettingsService::open(path.clone(), vec![temporary.path().into()], &app).unwrap();
    let policy = manager.feature_policy();
    let feature_bytes = std::fs::read(temporary.path().join("features.json")).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "hello".into(), 1).await.unwrap();
    let row = candidate(&settled(&app, id, 1).await, "example.search");
    std::fs::create_dir(&path).unwrap();
    assert!(manager
        .change(&app, enable("plugin:example", false))
        .is_err());
    assert!(status(&manager, "plugin:example").active);
    assert!(Arc::ptr_eq(&policy, &manager.feature_policy()));
    assert_eq!(
        std::fs::read(temporary.path().join("features.json")).unwrap(),
        feature_bytes
    );
    assert_eq!(
        app.execute(id, 1, row).await.unwrap().status,
        ActionStatus::Succeeded
    );
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 4); // Temporary writes cleaned up.
}

#[test]
fn preferences_damaged_after_open_are_not_overwritten_by_a_stale_settings_change() {
    for (filename, key, broken) in [
        (
            "features.json",
            "first-party:url",
            r#"{"schema_version":99,"disabled":[]}"#,
        ),
        (
            "extensions.json",
            "plugin:example",
            r#"{"schema_version":99,"disabled":[],"approvals":{}}"#,
        ),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        std::fs::write(
            temporary.path().join("plugin.toml"),
            anywhere_plugin::EXAMPLE_PLUGIN,
        )
        .unwrap();
        let app = app();
        let mut manager = SettingsService::open(
            temporary.path().join("extensions.json"),
            vec![temporary.path().into()],
            &app,
        )
        .unwrap();
        let policy = manager.feature_policy();
        let path = temporary.path().join(filename);
        std::fs::write(&path, broken).unwrap();
        assert!(manager.change(&app, enable(key, false)).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), broken);
        assert!(status(&manager, key).active);
        assert!(Arc::ptr_eq(&policy, &manager.feature_policy()));
    }
}
