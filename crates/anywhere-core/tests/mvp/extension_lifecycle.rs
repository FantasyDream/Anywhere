use crate::support::*;
use anywhere_core::application::extension_manager::{
    ExtensionChange, ExtensionManager, ExtensionStatus,
};
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
async fn ui_definition_changes_require_new_approval_and_revoke_retained_views() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("detail.toml");
    let raw = include_str!("../../../../plugins/examples/context-detail.toml");
    std::fs::write(&manifest, raw).unwrap();
    let app = app();
    let mut manager = ExtensionManager::open(
        tmp.path().join("extensions.json"),
        vec![tmp.path().into()],
        &app,
    )
    .unwrap();
    let key = "plugin:example_detail";
    let before = status(&manager, key);
    assert!(before.needs_approval);
    assert_eq!(before.capabilities, ["ui.render"]);
    manager
        .change(
            &app,
            ExtensionChange::Approve {
                key: key.into(),
                token: before.approval_token,
            },
        )
        .unwrap();
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
    std::fs::write(&manifest, raw.replace("Selected object", "Changed label")).unwrap();
    manager.reload(&app).unwrap();
    let changed = status(&manager, key);
    assert!(changed.needs_approval && !changed.active);
    assert!(app.resume_session(id).unwrap().is_none());
    assert!(app.execute(id, 1, row).await.is_err());
    manager
        .change(
            &app,
            ExtensionChange::Approve {
                key: key.into(),
                token: changed.approval_token,
            },
        )
        .unwrap();
    assert!(status(&manager, key).active);
    manager
        .change(&app, ExtensionChange::Revoke { key: key.into() })
        .unwrap();
    assert!(!status(&manager, key).active);
}
fn status(manager: &ExtensionManager, key: &str) -> ExtensionStatus {
    manager
        .statuses()
        .into_iter()
        .find(|s| s.key == key)
        .unwrap()
}
fn enable(key: &str, enabled: bool) -> ExtensionChange {
    ExtensionChange::Enable {
        key: key.into(),
        enabled,
    }
}
fn approve(manager: &ExtensionManager) -> ExtensionChange {
    let s = status(manager, "plugin:controlled");
    ExtensionChange::Approve {
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
async fn module_toggle_revokes_retained_session_recognizers_and_shortcuts_and_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    let app = app();
    let mut manager = ExtensionManager::open(path.clone(), vec![], &app).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, r#"{"key":1}"#.into(), 1)
        .await
        .unwrap();
    let old = settled(&app, id, 1).await;
    let row = candidate(&old, "core.json.copy");
    app.suspend_session(id).await.unwrap();
    manager
        .change(&app, enable("first-party:json", false))
        .unwrap();
    assert!(!status(&manager, "first-party:json").active);
    assert!(app.resume_session(id).unwrap().is_none());
    assert!(app.execute(id, 1, row).await.is_err());
    assert!(app
        .supported_shortcut_actions(&["core.json.copy".into()])
        .unwrap()
        .is_empty());
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, r#"{"key":1}"#.into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    assert!(!state
        .context_summary
        .iter()
        .any(|c| c.type_id == JSON_DOCUMENT));
    assert!(!state.scopes.iter().any(|s| s.id == "json"));
    let restarted = self::app();
    let mut manager = ExtensionManager::open(path, vec![], &restarted).unwrap();
    assert!(!status(&manager, "first-party:json").active);
    manager
        .change(&restarted, enable("first-party:json", true))
        .unwrap();
    assert_eq!(
        restarted
            .supported_shortcut_actions(&["core.json.copy".into()])
            .unwrap(),
        ["core.json.copy"]
    );
    let id = restarted.activate().await.unwrap().unwrap().session_id;
    restarted
        .update_input(id, r#"{"key":1}"#.into(), 1)
        .await
        .unwrap();
    assert!(settled(&restarted, id, 1)
        .await
        .context_summary
        .iter()
        .any(|c| c.type_id == JSON_DOCUMENT));
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
    let mut manager = ExtensionManager::open(path.clone(), dirs.clone(), &app).unwrap();
    assert!(status(&manager, "plugin:controlled").needs_approval);
    assert!(app
        .supported_shortcut_actions(&["controlled.copy".into()])
        .unwrap()
        .is_empty());
    let request = approve(&manager);
    manager.change(&app, request).unwrap();
    assert!(status(&manager, "plugin:controlled").active);
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "approved text".into(), 1)
        .await
        .unwrap();
    let row = candidate(&settled(&app, id, 1).await, "controlled.copy");
    app.execute(id, 1, row.clone()).await.unwrap();
    assert_eq!(
        *platform.calls.lock().unwrap(),
        [("copy".into(), "approved text".into())]
    );
    let restarted = self::app();
    let mut loaded = ExtensionManager::open(path, dirs, &restarted).unwrap();
    assert!(status(&loaded, "plugin:controlled").active);
    // Changed capability list, even without a version bump, cannot inherit authorization.
    std::fs::write(
        &manifest,
        V2.replace(
            "[\"clipboard.write\"]",
            "[\"clipboard.write\", \"url.open\"]",
        ),
    )
    .unwrap();
    manager.reload(&app).unwrap();
    assert!(status(&manager, "plugin:controlled").needs_approval);
    assert!(app.execute(id, 1, row).await.is_err());
    let request = approve(&manager);
    manager.change(&app, request).unwrap();
    manager
        .change(
            &app,
            ExtensionChange::Revoke {
                key: "plugin:controlled".into(),
            },
        )
        .unwrap();
    assert!(app
        .supported_shortcut_actions(&["controlled.copy".into()])
        .unwrap()
        .is_empty());
    assert_eq!(platform.calls.lock().unwrap().len(), 1);
    // Version changes and updated action templates also need fresh approval.
    std::fs::write(&manifest, V2.replace("1.0.0", "1.0.1")).unwrap();
    loaded.reload(&restarted).unwrap();
    assert!(status(&loaded, "plugin:controlled").needs_approval);
}

#[test]
fn stale_ui_and_replaced_installations_cannot_receive_a_grant() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("plugin.toml");
    std::fs::write(&manifest, V2).unwrap();
    let app = app();
    let mut manager = ExtensionManager::open(
        tmp.path().join("extensions.json"),
        vec![tmp.path().into()],
        &app,
    )
    .unwrap();
    let stale = approve(&manager);
    manager.reload(&app).unwrap();
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
    manager.reload(&app).unwrap();
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
    let mut manager = ExtensionManager::open(
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
    let mut manager = ExtensionManager::open(path.clone(), vec![], &app).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, r#"{"key":1}"#.into(), 1)
        .await
        .unwrap();
    let row = candidate(&settled(&app, id, 1).await, "core.json.copy");
    std::fs::create_dir(&path).unwrap(); // Destination cannot be atomically replaced.
    assert!(manager
        .change(&app, enable("first-party:json", false))
        .is_err());
    assert!(status(&manager, "first-party:json").active);
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
                JSON_DOCUMENT,
                ContextValue::Text("{}".into()),
                ContextSource::UserInput,
                "old-provider",
            )])
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app();
    let mut manager =
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    app.register_provider(Arc::new(Slow {
        descriptor: ProviderDescriptor {
            id: "old.slow".into(),
            accepts: vec![TEXT.into()],
            produces: vec![JSON_DOCUMENT.into()],
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
        .change(&app, enable("first-party:json", false))
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
        .any(|c| c.type_id == JSON_DOCUMENT));
}

#[tokio::test]
async fn invalid_preferences_preserve_file_and_base_launcher_with_read_only_recovery() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extensions.json");
    for broken in [
        r#"{"schema_version":99,"disabled":[],"approvals":{}}"#,
        "not json",
    ] {
        std::fs::write(&path, broken).unwrap();
        let app = app();
        let mut manager = ExtensionManager::open(path.clone(), vec![], &app).unwrap();
        assert!(manager
            .statuses()
            .iter()
            .all(|s| !s.active && !s.configurable));
        assert!(manager
            .change(&app, enable("first-party:json", true))
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
        manager.reload(&app).unwrap();
        assert!(status(&manager, "first-party:json").active);
    }
}

#[tokio::test]
async fn disabling_url_keeps_other_modules_and_restoring_recovers_web_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app();
    let mut manager =
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    manager
        .change(&app, enable("first-party:url", false))
        .unwrap();
    assert!(status(&manager, "first-party:json").active);
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.com".into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    assert!(!state.context_summary.iter().any(|c| c.type_id == URL));
    assert!(!state.scopes.iter().any(|s| s.id == "web"));
    assert_eq!(
        app.supported_shortcut_actions(&["core.url.open".into(), "core.json.copy".into()])
            .unwrap(),
        ["core.json.copy"]
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
