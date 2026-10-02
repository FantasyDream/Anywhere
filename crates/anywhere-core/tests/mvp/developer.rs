use super::support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cargo_action_uses_the_graph_scope_history_and_independent_execution_lifetime() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'anywhere-session-fixture'\nversion = '0.1.0'\nedition = '2021'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn answer() -> u32 { 42 }\n").unwrap();
    std::fs::write(
        root.join("build.rs"),
        "fn main() { std::thread::sleep(std::time::Duration::from_millis(200)); }\n",
    )
    .unwrap();
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        Arc::new(TestPlatform::new(root.clone(), true)),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    assert!(state
        .context_summary
        .iter()
        .any(|context| context.type_id == PROJECT_RUST));
    assert!(state.scopes.iter().any(|scope| scope.id == "rust"));
    let check = state
        .context_candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("developer.cargo.check"))
        .unwrap();
    let result = app
        .execute(state.session_id, state.revision, check.id.clone())
        .await
        .unwrap();
    assert!(result.succeeded(), "{:?}", result.error);
    let id = result.execution_session_id.unwrap();
    assert_ne!(id, state.session_id.to_string());
    app.deactivate(state.session_id).unwrap();
    assert!(!app.execution_sessions().is_empty());
    // A later Launcher activation must neither cancel nor replace the started task.
    let next = app.activate().await.unwrap().unwrap();
    assert_ne!(next.session_id, state.session_id);
    let detail = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let detail = app.execution_session(&id).unwrap();
            if !detail.summary.status.is_active() {
                break detail;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        detail.summary.status,
        ExecutionSessionStatus::Succeeded,
        "{}",
        detail.stderr
    );
    assert_eq!(detail.summary.exit_code, Some(0));
    assert_eq!(detail.summary.directory, root.to_string_lossy());
    assert_eq!(detail.summary.command[1..], ["check", "--color", "never"]);
    assert!(detail.stderr.contains("Finished"));
    let usage = history.lock().unwrap().load("com.apple.finder").unwrap();
    assert_eq!(
        usage[&("developer.cargo.check".into(), PROJECT_RUST.into())].execute_count,
        1
    );
    app.shutdown_execution_sessions();
}

#[tokio::test]
async fn project_without_manifest_has_no_rust_scope_and_deleted_manifest_cannot_execute() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    assert!(!state.scopes.iter().any(|scope| scope.id == "rust"));
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    app.update_input(state.session_id, root.to_string_lossy().into_owned(), 1)
        .await
        .unwrap();
    let state = settled(&app, state.session_id, 1).await;
    let check = state
        .context_candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("developer.cargo.check"))
        .unwrap();
    std::fs::remove_file(root.join("Cargo.toml")).unwrap();
    let result = app
        .execute(state.session_id, 1, check.id.clone())
        .await
        .unwrap();
    assert_eq!(result.status, ActionStatus::Failed);
    assert!(result.execution_session_id.is_none());
    assert!(app.execution_sessions().is_empty());
}
