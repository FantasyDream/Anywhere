use crate::support::*;
use anywhere_core::{
    application::settings_service::{SettingsChange, SettingsService},
    features::saved_directories::{FORGET, OPEN, SAVE},
    persistence::module_storage::ModuleStorageHost,
};

fn application(platform: Arc<TestPlatform>) -> Application {
    Application::new(platform, registry(), Box::<MemoryUsage>::default(), vec![])
}
async fn search(app: &Application, query: &str) -> LauncherState {
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_context_input(id, query.into(), 1, None, InputMode::Search)
        .await
        .unwrap();
    settled(app, id, 1).await
}
async fn save(app: &Application, path: &Path) {
    let state = search(app, path.to_str().unwrap()).await;
    let result = app
        .execute(state.session_id, state.revision, candidate(&state, SAVE))
        .await
        .unwrap();
    assert_eq!(result.status, ActionStatus::Succeeded);
}
#[tokio::test]
async fn saved_directory_source_roundtrip_ranking_open_forget_and_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("unique_saved_project");
    std::fs::create_dir(&project).unwrap();
    let platform = Arc::new(TestPlatform::default());
    let app = application(platform.clone());
    let policy = tmp.path().join("extensions.json");
    let mut manager = SettingsService::open(policy.clone(), vec![], &app).unwrap();
    save(&app, &project).await;
    let state = search(&app, "unique_saved_project").await;
    let row = state
        .candidates
        .iter()
        .find(|c| c.command_id.as_deref() == Some(OPEN))
        .unwrap();
    assert_eq!(row.title, "unique_saved_project");
    assert!(row.score >= 4000.0);
    let initial_actions = app
        .file_actions(state.session_id, state.revision, row.id.clone())
        .unwrap();
    assert_eq!(
        initial_actions.first().unwrap().command_id.as_deref(),
        Some(OPEN)
    );
    assert_eq!(
        app.execute(state.session_id, state.revision, row.id.clone())
            .await
            .unwrap()
            .status,
        ActionStatus::Succeeded
    );
    assert_eq!(
        *platform.calls.lock().unwrap(),
        [(
            "file".into(),
            project.canonicalize().unwrap().to_str().unwrap().into()
        )]
    );
    assert!(!search(&app, "totally_unrelated")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
    // Data refresh keeps the startup source and its bounded service cache.
    manager.refresh(&app).unwrap();
    let restarted = application(Arc::new(TestPlatform::default()));
    let _loaded = SettingsService::open(policy, vec![], &restarted).unwrap();
    assert!(search(&restarted, "unique_saved_project")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
    let object = search(&app, "unique_saved_project").await;
    let actions = app
        .file_actions(object.session_id, object.revision, candidate(&object, OPEN))
        .unwrap();
    assert_eq!(actions.first().unwrap().command_id.as_deref(), Some(OPEN));
    let forget = actions
        .iter()
        .find(|c| c.command_id.as_deref() == Some(FORGET))
        .unwrap();
    let result = app
        .execute(object.session_id, object.revision, forget.id.clone())
        .await
        .unwrap();
    assert_eq!(result.status, ActionStatus::Succeeded);
    assert!(!search(&app, "unique_saved_project")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
    manager.refresh(&app).unwrap();
    assert!(!search(&app, "unique_saved_project")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
}

#[tokio::test]
async fn source_disable_reenable_and_missing_directory_reject_stale_execution() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("saved_lifecycle");
    std::fs::create_dir(&project).unwrap();
    let platform = Arc::new(TestPlatform::default());
    let app = application(platform.clone());
    let mut manager =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    save(&app, &project).await;
    let old = search(&app, "saved_lifecycle").await;
    let old_row = candidate(&old, OPEN);
    manager
        .change(
            &app,
            SettingsChange::Enable {
                key: "first-party:saved_directories".into(),
                enabled: false,
            },
        )
        .unwrap();
    assert!(app
        .execute(old.session_id, old.revision, old_row)
        .await
        .is_err());
    assert!(!search(&app, "saved_lifecycle")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
    assert!(app
        .supported_shortcut_actions(&[SAVE.into()])
        .unwrap()
        .is_empty());
    manager
        .change(
            &app,
            SettingsChange::Enable {
                key: "first-party:saved_directories".into(),
                enabled: true,
            },
        )
        .unwrap();
    let live = search(&app, "saved_lifecycle").await;
    let row = candidate(&live, OPEN);
    std::fs::remove_dir(&project).unwrap();
    assert_eq!(
        app.execute(live.session_id, live.revision, row)
            .await
            .unwrap()
            .status,
        ActionStatus::Failed
    );
    assert!(platform.calls.lock().unwrap().is_empty());
    // Missing directories can still be forgotten without traversing the filesystem.
    let object = search(&app, "saved_lifecycle").await;
    let actions = app
        .file_actions(object.session_id, object.revision, candidate(&object, OPEN))
        .unwrap();
    let forget = actions
        .iter()
        .find(|c| c.command_id.as_deref() == Some(FORGET))
        .unwrap();
    assert_eq!(
        app.execute(object.session_id, object.revision, forget.id.clone())
            .await
            .unwrap()
            .status,
        ActionStatus::Succeeded
    );
}

#[tokio::test]
async fn corrupt_module_data_is_isolated_and_refresh_can_recover_without_affecting_json() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extension-storage.sqlite3");
    let host = ModuleStorageHost::open(&path).unwrap();
    let store = host.bind("saved_directories").unwrap();
    store.put("directories", b"not valid JSON").unwrap();
    let app = application(Arc::new(TestPlatform::default()));
    let mut manager =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    assert!(
        !manager
            .statuses()
            .iter()
            .find(|s| s.id == "saved_directories")
            .unwrap()
            .active
    );
    assert!(
        manager
            .statuses()
            .iter()
            .find(|s| s.id == "url")
            .unwrap()
            .active
    );
    assert!(app
        .supported_shortcut_actions(&[SAVE.into()])
        .unwrap()
        .is_empty());
    let url = search(&app, "https://example.org").await;
    assert!(url.context_summary.iter().any(|c| c.type_id == URL));
    store.remove("directories").unwrap();
    manager.refresh(&app).unwrap();
    assert!(
        manager
            .statuses()
            .iter()
            .find(|s| s.id == "saved_directories")
            .unwrap()
            .active
    );
    assert_eq!(
        app.supported_shortcut_actions(&[SAVE.into()]).unwrap(),
        [SAVE]
    );
}

#[tokio::test]
async fn another_instance_forget_blocks_a_cached_open_without_side_effects() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("shared_saved_project");
    std::fs::create_dir(&directory).unwrap();
    let policy = temporary.path().join("extensions.json");
    let platform = Arc::new(TestPlatform::default());
    let first = application(platform.clone());
    let _first_manager = SettingsService::open(policy.clone(), vec![], &first).unwrap();
    save(&first, &directory).await;
    let cached = search(&first, "shared_saved_project").await;
    let row = candidate(&cached, OPEN);
    let second = application(Arc::new(TestPlatform::default()));
    let _second_manager = SettingsService::open(policy, vec![], &second).unwrap();
    let object = search(&second, "shared_saved_project").await;
    let actions = second
        .file_actions(object.session_id, object.revision, candidate(&object, OPEN))
        .unwrap();
    let forget = actions
        .iter()
        .find(|action| action.command_id.as_deref() == Some(FORGET))
        .unwrap();
    assert_eq!(
        second
            .execute(object.session_id, object.revision, forget.id.clone())
            .await
            .unwrap()
            .status,
        ActionStatus::Succeeded
    );
    assert_eq!(
        first
            .execute(cached.session_id, cached.revision, row)
            .await
            .unwrap()
            .status,
        ActionStatus::Failed
    );
    assert!(platform.calls.lock().unwrap().is_empty());
    assert!(!search(&first, "shared_saved_project")
        .await
        .candidates
        .iter()
        .any(|candidate| candidate.command_id.as_deref() == Some(OPEN)));
}

#[tokio::test]
async fn replaced_broken_database_rebinds_typed_service_and_revokes_old_rows() {
    let temporary = tempfile::tempdir().unwrap();
    let original = temporary.path().join("original_saved_project");
    let replacement = temporary.path().join("replacement_saved_project");
    let added = temporary.path().join("added_saved_project");
    for path in [&original, &replacement, &added] {
        std::fs::create_dir(path).unwrap();
    }
    let database = temporary.path().join("extension-storage.sqlite3");
    let platform = Arc::new(TestPlatform::default());
    let app = application(platform.clone());
    let mut manager =
        SettingsService::open(temporary.path().join("extensions.json"), vec![], &app).unwrap();
    save(&app, &original).await;
    let cached = search(&app, "original_saved_project").await;
    let row = candidate(&cached, OPEN);
    std::fs::rename(&database, temporary.path().join("detached.sqlite3")).unwrap();
    std::fs::write(&database, b"broken database").unwrap();
    manager.refresh(&app).unwrap();
    assert!(
        !manager
            .statuses()
            .iter()
            .find(|status| status.id == "saved_directories")
            .unwrap()
            .active
    );
    assert!(
        manager
            .statuses()
            .iter()
            .find(|status| status.id == "url")
            .unwrap()
            .active
    );
    assert!(app
        .execute(cached.session_id, cached.revision, row)
        .await
        .is_err());
    assert_eq!(std::fs::read(&database).unwrap(), b"broken database");
    assert!(platform.calls.lock().unwrap().is_empty());
    std::fs::remove_file(&database).unwrap();
    let host = ModuleStorageHost::open(&database).unwrap();
    let storage = host.bind("saved_directories").unwrap();
    storage
        .put(
            "directories",
            &serde_json::to_vec(&serde_json::json!({
                "version": 1, "paths": [replacement.canonicalize().unwrap()]
            }))
            .unwrap(),
        )
        .unwrap();
    manager.refresh(&app).unwrap();
    assert!(
        manager
            .statuses()
            .iter()
            .find(|status| status.id == "saved_directories")
            .unwrap()
            .active
    );
    assert!(!search(&app, "original_saved_project")
        .await
        .candidates
        .iter()
        .any(|candidate| candidate.command_id.as_deref() == Some(OPEN)));
    assert!(search(&app, "replacement_saved_project")
        .await
        .candidates
        .iter()
        .any(|candidate| candidate.command_id.as_deref() == Some(OPEN)));
    save(&app, &added).await;
    let persisted: serde_json::Value =
        serde_json::from_slice(&storage.get("directories").unwrap().unwrap()).unwrap();
    let paths: Vec<std::path::PathBuf> =
        serde_json::from_value(persisted["paths"].clone()).unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&replacement.canonicalize().unwrap()));
    assert!(paths.contains(&added.canonicalize().unwrap()));
    assert!(!paths.contains(&original.canonicalize().unwrap()));
}

#[tokio::test]
async fn git_and_developer_switches_preserve_existing_ids_and_disable_only_their_contribution() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(project.join(".git")).unwrap();
    std::fs::write(
        project.join("package.json"),
        r#"{"scripts":{"build":"echo build"}}"#,
    )
    .unwrap();
    let app = application(Arc::new(TestPlatform::default()));
    let mut manager =
        SettingsService::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    let state = search(&app, project.to_str().unwrap()).await;
    assert!(state
        .context_summary
        .iter()
        .any(|c| c.type_id == GIT_REPOSITORY));
    assert!(state
        .context_summary
        .iter()
        .any(|c| c.type_id == PROJECT_NODE));
    assert!(state
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("developer.node.build")));
    manager
        .change(
            &app,
            SettingsChange::Enable {
                key: "first-party:git".into(),
                enabled: false,
            },
        )
        .unwrap();
    let state = search(&app, project.to_str().unwrap()).await;
    assert!(!state
        .context_summary
        .iter()
        .any(|c| c.type_id == GIT_REPOSITORY));
    assert!(state
        .context_summary
        .iter()
        .any(|c| c.type_id == PROJECT_NODE));
    assert!(app
        .supported_shortcut_actions(&["core.git.status".into()])
        .unwrap()
        .is_empty());
    manager
        .change(
            &app,
            SettingsChange::Enable {
                key: "first-party:node".into(),
                enabled: false,
            },
        )
        .unwrap();
    assert!(app
        .supported_shortcut_actions(&["developer.node.build".into()])
        .unwrap()
        .is_empty());
    assert!(!search(&app, project.to_str().unwrap())
        .await
        .context_summary
        .iter()
        .any(|c| c.type_id == PROJECT_NODE));
}

#[tokio::test]
async fn external_action_conflict_does_not_override_first_party_storage_actions() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("conflict.toml"),
        r#"protocol_version=1
id="saved_directories"
name="Conflicting plugin"
[[commands]]
id="save"
title="Wrong save"
accepts=["filesystem.directory"]
action={type="copy",text="wrong"}
"#,
    )
    .unwrap();
    let platform = Arc::new(TestPlatform::default());
    let app = application(platform.clone());
    let manager = SettingsService::open(
        tmp.path().join("extensions.json"),
        vec![tmp.path().into()],
        &app,
    )
    .unwrap();
    assert!(
        !manager
            .statuses()
            .iter()
            .find(|s| s.key == "plugin:saved_directories")
            .unwrap()
            .active
    );
    let project = tmp.path().join("conflict_project");
    std::fs::create_dir(&project).unwrap();
    save(&app, &project).await;
    assert!(platform.calls.lock().unwrap().is_empty());
    assert!(search(&app, "conflict_project")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
}
