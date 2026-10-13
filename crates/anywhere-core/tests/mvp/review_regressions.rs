use crate::support::*;
use anywhere_core::{
    application::extension_manager::ExtensionManager,
    extensions::saved_directories::{FORGET, OPEN, SAVE},
    persistence::module_storage::ModuleStorageHost,
};
fn review_app() -> Application {
    Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    )
}
async fn search(app: &Application, text: &str) -> LauncherState {
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_context_input(id, text.into(), 1, None, InputMode::Search)
        .await
        .unwrap();
    settled(app, id, 1).await
}
async fn save(app: &Application, path: &Path) {
    let state = search(app, path.to_str().unwrap()).await;
    assert_eq!(
        app.execute(state.session_id, state.revision, candidate(&state, SAVE))
            .await
            .unwrap()
            .status,
        ActionStatus::Succeeded
    );
}
#[tokio::test]
async fn two_independent_instances_merge_saved_data() {
    let tmp = tempfile::tempdir().unwrap();
    let policy = tmp.path().join("extensions.json");
    let alpha = tmp.path().join("alpha");
    let beta = tmp.path().join("beta");
    std::fs::create_dir(&alpha).unwrap();
    std::fs::create_dir(&beta).unwrap();
    let a = review_app();
    let _a = ExtensionManager::open(policy.clone(), vec![], &a).unwrap();
    let b = review_app();
    let _b = ExtensionManager::open(policy.clone(), vec![], &b).unwrap();
    save(&a, &alpha).await;
    save(&b, &beta).await;
    let reopened = review_app();
    let _c = ExtensionManager::open(policy, vec![], &reopened).unwrap();
    assert!(search(&reopened, "alpha")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
    assert!(search(&reopened, "beta")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
}
#[tokio::test]
async fn reserved_prefixes_are_normal_names_and_forget_is_a_secondary_action() {
    let tmp = tempfile::tempdir().unwrap();
    let app = review_app();
    let _m = ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    for name in ["remove draft", "forget archive", "Unpin notes"] {
        let folder = tmp.path().join(name);
        std::fs::create_dir(&folder).unwrap();
        save(&app, &folder).await;
        let state = search(&app, name).await;
        assert!(!state
            .candidates
            .iter()
            .any(|c| c.command_id.as_deref() == Some(FORGET)));
        let open = candidate(&state, OPEN);
        let actions = app
            .file_actions(state.session_id, state.revision, open.clone())
            .unwrap();
        let forget = actions
            .iter()
            .find(|c| c.command_id.as_deref() == Some(FORGET))
            .unwrap();
        assert_eq!(
            app.execute(state.session_id, state.revision, forget.id.clone())
                .await
                .unwrap()
                .status,
            ActionStatus::Succeeded
        );
        assert!(folder.exists());
        assert_eq!(
            app.execute(state.session_id, state.revision, open)
                .await
                .unwrap()
                .status,
            ActionStatus::Failed
        );
        assert!(!search(&app, name)
            .await
            .candidates
            .iter()
            .any(|c| c.command_id.as_deref() == Some(OPEN)));
    }
}
#[tokio::test]
async fn saved_directory_usage_participates_in_shared_ranking() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("project");
    std::fs::create_dir(&path).unwrap();
    let history_path = tmp.path().join("usage.sqlite3");
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::new(SqliteUsageRepository::open(&history_path).unwrap()),
        vec![],
    );
    let _m = ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    save(&app, &path).await;
    let before = search(&app, "project").await;
    let score = before
        .candidates
        .iter()
        .find(|c| c.command_id.as_deref() == Some(OPEN))
        .unwrap()
        .score;
    app.execute(before.session_id, before.revision, candidate(&before, OPEN))
        .await
        .unwrap();
    let usage = SqliteUsageRepository::open(&history_path)
        .unwrap()
        .load_global()
        .unwrap();
    assert_eq!(
        usage
            .get(&(OPEN.into(), "saved.directory".into()))
            .unwrap()
            .execute_count,
        1
    );
    let after = search(&app, "project").await;
    assert!(
        after
            .candidates
            .iter()
            .find(|c| c.command_id.as_deref() == Some(OPEN))
            .unwrap()
            .score
            > score
    );
}
#[test]
fn replacing_failed_database_recovers_on_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("extension-storage.sqlite3");
    let host = ModuleStorageHost::open(&db).unwrap();
    host.bind("saved_directories")
        .unwrap()
        .put("directories", b"broken json")
        .unwrap();
    let app = review_app();
    let mut m = ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    assert!(
        !m.statuses()
            .iter()
            .find(|s| s.id == "saved_directories")
            .unwrap()
            .active
    );
    std::fs::rename(&db, tmp.path().join("broken.backup")).unwrap();
    let _fresh = ModuleStorageHost::open(&db).unwrap();
    m.reload(&app).unwrap();
    assert!(
        m.statuses()
            .iter()
            .find(|s| s.id == "saved_directories")
            .unwrap()
            .active
    );
    let fresh_app = review_app();
    let fresh_manager =
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &fresh_app).unwrap();
    assert!(
        fresh_manager
            .statuses()
            .iter()
            .find(|s| s.id == "saved_directories")
            .unwrap()
            .active
    );
}

#[tokio::test]
async fn external_forget_revokes_a_stale_open_offer_and_preserves_other_saved_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let policy = tmp.path().join("extensions.json");
    let alpha = tmp.path().join("alpha");
    let beta = tmp.path().join("beta");
    std::fs::create_dir(&alpha).unwrap();
    std::fs::create_dir(&beta).unwrap();
    let a = review_app();
    let _a = ExtensionManager::open(policy.clone(), vec![], &a).unwrap();
    let b = review_app();
    let _b = ExtensionManager::open(policy, vec![], &b).unwrap();
    save(&a, &alpha).await;
    save(&b, &beta).await;
    let old = search(&a, "alpha").await;
    let target = search(&b, "alpha").await;
    let actions = b
        .file_actions(target.session_id, target.revision, candidate(&target, OPEN))
        .unwrap();
    let forget = actions
        .iter()
        .find(|c| c.command_id.as_deref() == Some(FORGET))
        .unwrap();
    assert_eq!(
        b.execute(target.session_id, target.revision, forget.id.clone())
            .await
            .unwrap()
            .status,
        ActionStatus::Succeeded
    );
    assert_eq!(
        a.execute(old.session_id, old.revision, candidate(&old, OPEN))
            .await
            .unwrap()
            .status,
        ActionStatus::Failed
    );
    assert!(search(&a, "beta")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
}

#[tokio::test]
async fn replacement_refreshes_already_loaded_state_and_keeps_writing_to_the_new_database() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("extension-storage.sqlite3");
    let app = review_app();
    let mut manager =
        ExtensionManager::open(tmp.path().join("extensions.json"), vec![], &app).unwrap();
    let alpha = tmp.path().join("alpha");
    let beta = tmp.path().join("beta");
    std::fs::create_dir(&alpha).unwrap();
    std::fs::create_dir(&beta).unwrap();
    save(&app, &alpha).await;
    std::fs::rename(&db, tmp.path().join("old.sqlite3")).unwrap();
    let fresh = ModuleStorageHost::open(&db).unwrap();
    manager.reload(&app).unwrap();
    assert!(!search(&app, "alpha")
        .await
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some(OPEN)));
    save(&app, &beta).await;
    let value = fresh
        .bind("saved_directories")
        .unwrap()
        .get("directories")
        .unwrap()
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&value).unwrap();
    assert_eq!(
        saved["paths"],
        serde_json::json!([beta.canonicalize().unwrap()])
    );
}
