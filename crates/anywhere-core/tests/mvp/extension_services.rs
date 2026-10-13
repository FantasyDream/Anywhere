use crate::support::*;
use anywhere_core::{
    application::extension_manager::{ExtensionChange, ExtensionManager},
    extensions::saved_directories::{FORGET, OPEN, SAVE},
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
    let mut manager = ExtensionManager::open(policy.clone(), vec![], &app).unwrap();
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
    // Rebuilding configuration shares the bounded cache, including subsequent saves/removals.
    manager.reload(&app).unwrap();
    let restarted = application(Arc::new(TestPlatform::default()));
    let _loaded = ExtensionManager::open(policy, vec![], &restarted).unwrap();
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
    manager.reload(&app).unwrap();
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
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    save(&app, &project).await;
    let old = search(&app, "saved_lifecycle").await;
    let old_row = candidate(&old, OPEN);
    manager
        .change(
            &app,
            ExtensionChange::Enable {
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
            ExtensionChange::Enable {
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
async fn corrupt_module_data_is_isolated_and_reload_can_recover_without_affecting_json() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("extension-storage.sqlite3");
    let host = ModuleStorageHost::open(&path).unwrap();
    let store = host.bind("saved_directories").unwrap();
    store.put("directories", b"not valid JSON").unwrap();
    let app = application(Arc::new(TestPlatform::default()));
    let mut manager =
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
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
            .find(|s| s.id == "json")
            .unwrap()
            .active
    );
    assert!(app
        .supported_shortcut_actions(&[SAVE.into()])
        .unwrap()
        .is_empty());
    let json = search(&app, "{\"key\":1}").await;
    assert!(json
        .context_summary
        .iter()
        .any(|c| c.type_id == JSON_DOCUMENT));
    store.remove("directories").unwrap();
    manager.reload(&app).unwrap();
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
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
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
            ExtensionChange::Enable {
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
            ExtensionChange::Enable {
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
    let manager = ExtensionManager::open(
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
