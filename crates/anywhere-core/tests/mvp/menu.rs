use crate::support::*;

fn sample_menu(title: &str, path: &[&str], enabled: bool) -> DiscoveredMenuItem {
    DiscoveredMenuItem {
        title: title.into(),
        menu_path: path.iter().map(|part| (*part).to_owned()).collect(),
        identifiers: vec![None; path.len()],
        shortcut: None,
        enabled,
    }
}

fn with_menus(platform: &TestPlatform, items: Vec<DiscoveredMenuItem>) {
    *platform.menu_trusted.lock().unwrap() = true;
    *platform.menu_items.lock().unwrap() = items;
}

async fn wait_for_title(app: &Application, session: SessionId, title: &str) -> LauncherState {
    for _ in 0..200 {
        if let Some(state) = app.latest_state(session) {
            if state
                .candidates
                .iter()
                .any(|candidate| candidate.title == title)
            {
                return state;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("candidate {title:?} never appeared");
}

#[tokio::test]
async fn menu_actions_join_the_current_app_scope_and_do_not_create_another() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu(
            "Toggle Terminal",
            &["View", "Toggle Terminal"],
            true,
        )],
    );
    let app = app(platform);
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "Toggle Terminal").await;
    assert_eq!(scope_ids(&state), ["app"]);
    assert!(state.scopes.iter().all(|scope| scope.id == "app"));
    let action = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "Toggle Terminal")
        .unwrap();
    assert_eq!(action.group_id, "app");
    assert_eq!(action.owning_app.as_deref(), Some("test.editor"));
    assert!(action
        .command_id
        .as_deref()
        .unwrap()
        .starts_with("app.test.editor."));
}

#[tokio::test]
async fn unauthorized_menu_access_hides_the_current_app_scope() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    *platform.menu_items.lock().unwrap() = vec![sample_menu("About", &["App", "About"], true)];
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let latest = app.latest_state(state.session_id).unwrap();
    assert!(
        !scope_ids(&latest).contains(&"app"),
        "an editor without accessibility must not show an empty current app scope: {:?}",
        scope_ids(&latest)
    );
}

#[tokio::test]
async fn revoked_access_removes_menus_but_preserves_other_actions_and_can_recover() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let about = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "About")
        .unwrap();

    *platform.menu_trusted.lock().unwrap() = false;
    assert!(!app.prompt_accessibility().unwrap());
    let revoked = app.latest_state(first.session_id).unwrap();
    assert!(!revoked
        .candidates
        .iter()
        .any(|candidate| candidate.title == "About"));
    assert!(revoked
        .candidates
        .iter()
        .any(|candidate| candidate.command_id.as_deref() == Some("core.directory.open")));
    assert!(app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .is_err());
    assert_eq!(platform.menu_performs.load(Ordering::SeqCst), 0);

    *platform.menu_items.lock().unwrap() = vec![sample_menu("New", &["App", "New"], true)];
    *platform.menu_trusted.lock().unwrap() = true;
    assert!(app.prompt_accessibility().unwrap());
    let recovered = wait_for_title(&app, first.session_id, "New").await;
    assert!(!recovered
        .candidates
        .iter()
        .any(|candidate| candidate.title == "About"));
}

#[tokio::test]
async fn discovery_started_before_access_revocation_cannot_republish_menus() {
    let platform = Arc::new(TestPlatform {
        menu_snapshot_before_gate: true,
        ..Default::default()
    });
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    for _ in 0..100 {
        if platform.menu_discoveries.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(platform.menu_discoveries.load(Ordering::SeqCst) > 0);

    *platform.menu_trusted.lock().unwrap() = false;
    assert!(!app.prompt_accessibility().unwrap());
    gate.release();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let latest = app.latest_state(first.session_id).unwrap();
    assert!(!latest
        .candidates
        .iter()
        .any(|candidate| candidate.title == "About"));
    assert!(!scope_ids(&latest).contains(&"app"));
}

#[tokio::test]
async fn a_disabled_menu_action_is_hidden() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu("Save", &["File", "Save"], false)],
    );
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let latest = app.latest_state(state.session_id).unwrap();
    assert!(
        !latest
            .candidates
            .iter()
            .any(|candidate| candidate.title == "Save"),
        "disabled menus must not appear: {:?}",
        latest
            .candidates
            .iter()
            .map(|candidate| &candidate.title)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn late_menu_discovery_does_not_reorder_existing_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    with_menus(
        &platform,
        vec![sample_menu("New Folder", &["File", "New Folder"], true)],
    );
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let app = app(platform);
    let first = app.activate().await.unwrap().unwrap();
    assert!(
        !first
            .candidates
            .iter()
            .any(|candidate| candidate.title == "New Folder"),
        "slow discovery must not block the first snapshot"
    );
    let before = scope_ids(&first);
    assert!(before.contains(&"app") || before.contains(&"folder") || before.contains(&"git"));
    gate.release();
    let late = wait_for_title(&app, first.session_id, "New Folder").await;
    let after = scope_ids(&late);
    assert_eq!(after.first().copied(), Some("app"));
    for id in &before {
        assert!(
            after.contains(id),
            "{id} disappeared after late menus: {after:?}"
        );
        if *id != "app" {
            let before_pos = before.iter().position(|entry| entry == id).unwrap();
            let after_pos = after.iter().position(|entry| entry == id).unwrap();
            let before_app = before.iter().position(|entry| *entry == "app");
            if let Some(app_at) = before_app {
                if before_pos > app_at {
                    assert!(after_pos > 0);
                }
            }
        }
    }
    assert!(!after.iter().any(|id| id.starts_with("app.")));
}

#[tokio::test]
async fn a_warm_menu_cache_is_on_the_first_snapshot() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu(
            "Toggle Terminal",
            &["View", "Toggle Terminal"],
            true,
        )],
    );
    let app = app(platform);
    let first = app.activate().await.unwrap().unwrap();
    wait_for_title(&app, first.session_id, "Toggle Terminal").await;
    app.deactivate(first.session_id).unwrap();
    let second = app.activate().await.unwrap().unwrap();
    assert!(
        second
            .candidates
            .iter()
            .any(|candidate| candidate.title == "Toggle Terminal"),
        "a warm cache must reach the first snapshot: {:?}",
        second
            .candidates
            .iter()
            .map(|candidate| &candidate.title)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_missing_menu_item_fails_without_crashing() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let about = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "About")
        .unwrap();
    *platform.menu_perform_error.lock().unwrap() = Some("menu item was not found".into());
    let result = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(!result.succeeded());
    assert!(result.notice().contains("not found"));
    assert!(!result.close_window);
}

#[tokio::test]
async fn an_enabled_menu_action_performs() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let about = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "About")
        .unwrap();
    let result = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(result.succeeded(), "{}", result.notice());
    assert_eq!(platform.menu_performs.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn successful_menu_execution_records_one_use_and_failure_records_none() {
    let platform = Arc::new(TestPlatform::default());
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let about = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "About")
        .unwrap();
    let command_id = about.command_id.clone().unwrap();

    let success = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(success.succeeded(), "{}", success.notice());
    assert!(success.close_window);
    assert_eq!(
        history
            .lock()
            .unwrap()
            .0
            .get(&(
                command_id.clone(),
                APPLICATION.to_owned(),
                "test.editor".to_owned(),
            ))
            .unwrap()
            .execute_count,
        1
    );

    *platform.menu_perform_error.lock().unwrap() = Some("menu is unavailable".into());
    let failure = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(!failure.succeeded());
    assert_eq!(
        history
            .lock()
            .unwrap()
            .0
            .get(&(command_id, APPLICATION.to_owned(), "test.editor".to_owned()))
            .unwrap()
            .execute_count,
        1
    );
}

#[tokio::test]
async fn trusted_empty_menus_hide_the_current_app_scope() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(&platform, vec![]);
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let latest = app.latest_state(state.session_id).unwrap();
    assert!(
        !scope_ids(&latest).contains(&"app"),
        "no menu items must not create a current app scope: {:?}",
        scope_ids(&latest)
    );
}

#[tokio::test]
async fn a_pid_or_version_change_misses_the_menu_cache() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    *platform.menu_version.lock().unwrap() = Some("1.0".into());
    with_menus(
        &platform,
        vec![sample_menu(
            "Toggle Terminal",
            &["View", "Toggle Terminal"],
            true,
        )],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    wait_for_title(&app, first.session_id, "Toggle Terminal").await;
    app.deactivate(first.session_id).unwrap();

    platform.frontmost_pid.store(99, Ordering::Relaxed);
    *platform.menu_version.lock().unwrap() = Some("2.0".into());
    *platform.menu_items.lock().unwrap() = vec![sample_menu("About", &["App", "About"], true)];
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let second = app.activate().await.unwrap().unwrap();
    assert!(
        !second.candidates.iter().any(|candidate| {
            candidate.title == "Toggle Terminal" || candidate.title == "About"
        }),
        "stale cache must not reach the first snapshot: {:?}",
        second
            .candidates
            .iter()
            .map(|candidate| &candidate.title)
            .collect::<Vec<_>>()
    );
    gate.release();
    wait_for_title(&app, second.session_id, "About").await;
}

#[tokio::test]
async fn a_menu_action_disabled_after_discovery_fails_without_crashing() {
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let about = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "About")
        .unwrap();
    *platform.menu_items.lock().unwrap() = vec![sample_menu("About", &["App", "About"], false)];
    let result = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(!result.succeeded());
    assert!(result.notice().contains("不可用") || result.notice().contains("not found"));
    assert!(!result.close_window);
    assert_eq!(platform.menu_performs.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn menu_discovery_and_execute_emit_tracing_fields() {
    let fields = recorded_fields();
    fields.lock().unwrap().clear();
    let platform = Arc::new(TestPlatform::new(
        tempfile::tempdir().unwrap().path().into(),
        false,
    ));
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let about = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "About")
        .unwrap();
    let result = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(result.succeeded(), "{}", result.notice());
    let seen = fields.lock().unwrap().clone();
    for field in [
        "app_action_discovery_duration",
        "menu_item_count",
        "visible_action_count",
        "cache_hit",
        "execute_duration",
        "app_action_discovery",
        "app_action_execute",
    ] {
        assert!(
            seen.iter().any(|name| name == field),
            "{field} was never emitted; saw {seen:?}"
        );
    }
}

#[tokio::test]
async fn menu_permission_does_not_disable_finder_directory_actions() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = settled(&app, first.session_id, first.revision).await;
    assert!(scope_ids(&state).contains(&"app"));
    assert!(state
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.directory.open")));
    assert!(!state.candidates.iter().any(|c| c
        .command_id
        .as_deref()
        .is_some_and(|id| id.starts_with("app."))));
    assert_eq!(platform.menu_discoveries.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn prompting_granted_access_does_not_duplicate_discovery() {
    let platform = Arc::new(TestPlatform::default());
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    for _ in 0..100 {
        if platform.menu_discoveries.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    app.prompt_accessibility().unwrap();
    app.prompt_accessibility().unwrap();
    gate.release();
    wait_for_title(&app, first.session_id, "About").await;
    assert_eq!(platform.menu_discoveries.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn missing_menu_locator_refreshes_the_current_session() {
    let platform = Arc::new(TestPlatform::default());
    with_menus(&platform, vec![sample_menu("Old", &["App", "Old"], true)]);
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "Old").await;
    let old = state.candidates.iter().find(|c| c.title == "Old").unwrap();
    *platform.menu_items.lock().unwrap() = vec![sample_menu("New", &["App", "New"], true)];
    let result = app
        .execute(state.session_id, state.revision, old.id.clone())
        .await
        .unwrap();
    assert!(!result.succeeded());
    let refreshed = wait_for_title(&app, first.session_id, "New").await;
    assert!(!refreshed.candidates.iter().any(|c| c.id == old.id));
    assert!(refreshed.snapshot_seq > state.snapshot_seq);
    assert_eq!(refreshed.revision, state.revision);
}

#[tokio::test]
async fn stale_discovery_cannot_replace_a_newer_session_or_its_cache() {
    let platform = Arc::new(TestPlatform {
        menu_snapshot_before_gate: true,
        ..Default::default()
    });
    with_menus(&platform, vec![sample_menu("Old", &["App", "Old"], true)]);
    let old_gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(old_gate.clone());
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    for _ in 0..100 {
        if platform.menu_discoveries.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    app.deactivate(first.session_id).unwrap();
    *platform.menu_items.lock().unwrap() = vec![sample_menu("New", &["App", "New"], true)];
    let second = app.activate().await.unwrap().unwrap();
    wait_for_title(&app, second.session_id, "New").await;
    old_gate.release();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!app
        .latest_state(second.session_id)
        .unwrap()
        .candidates
        .iter()
        .any(|c| c.title == "Old"));
    app.deactivate(second.session_id).unwrap();
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let third = app.activate().await.unwrap().unwrap();
    gate.release();
    assert!(third.candidates.iter().any(|c| c.title == "New"));
    assert!(!third.candidates.iter().any(|c| c.title == "Old"));
}

#[tokio::test]
async fn captured_menu_handler_rejects_restarted_application_even_with_same_pid() {
    let platform = Arc::new(TestPlatform::default());
    with_menus(
        &platform,
        vec![sample_menu("About", &["App", "About"], true)],
    );
    let app = app(platform.clone());
    let first = app.activate().await.unwrap().unwrap();
    let state = wait_for_title(&app, first.session_id, "About").await;
    let candidate = state
        .candidates
        .iter()
        .find(|c| c.title == "About")
        .unwrap();
    *platform.menu_instance.lock().unwrap() = Some("restarted".into());
    let result = app
        .execute(state.session_id, state.revision, candidate.id.clone())
        .await
        .unwrap();
    assert!(!result.succeeded());
    assert!(result.notice().contains("instance changed"));
    assert_eq!(platform.menu_performs.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_execution_refresh_supersedes_the_same_sessions_slow_scan() {
    let platform = Arc::new(TestPlatform {
        menu_snapshot_before_gate: true,
        ..Default::default()
    });
    with_menus(&platform, vec![sample_menu("Old", &["App", "Old"], true)]);
    let app = app(platform.clone());
    let warm = app.activate().await.unwrap().unwrap();
    wait_for_title(&app, warm.session_id, "Old").await;
    app.deactivate(warm.session_id).unwrap();
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let state = app.activate().await.unwrap().unwrap();
    for _ in 0..100 {
        if platform.menu_discoveries.load(Ordering::SeqCst) >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let old = state.candidates.iter().find(|c| c.title == "Old").unwrap();
    *platform.menu_items.lock().unwrap() = vec![sample_menu("New", &["App", "New"], true)];
    let result = app
        .execute(state.session_id, state.revision, old.id.clone())
        .await
        .unwrap();
    assert!(!result.succeeded());
    wait_for_title(&app, state.session_id, "New").await;
    gate.release();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let latest = app.latest_state(state.session_id).unwrap();
    assert!(latest.candidates.iter().any(|c| c.title == "New"));
    assert!(!latest.candidates.iter().any(|c| c.title == "Old"));
}

#[tokio::test]
async fn hidden_menu_reappears_with_the_same_identity_when_enabled() {
    let platform = Arc::new(TestPlatform::default());
    with_menus(
        &platform,
        vec![sample_menu("Save", &["File", "Save"], false)],
    );
    let expected = anywhere_core::features::app_actions::descriptors(
        &platform.frontmost_application().unwrap().unwrap(),
        platform.menu_items.lock().unwrap().clone(),
    )[0]
    .id
    .clone();
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    assert!(!state
        .candidates
        .iter()
        .any(|c| c.command_id.as_ref() == Some(&expected)));
    app.deactivate(state.session_id).unwrap();
    let mut item = sample_menu("Save", &["File", "Save"], true);
    item.shortcut = Some("⌘S".into());
    *platform.menu_items.lock().unwrap() = vec![item];
    let next = app.activate().await.unwrap().unwrap();
    let ready = wait_for_title(&app, next.session_id, "Save").await;
    let save = ready.candidates.iter().find(|c| c.title == "Save").unwrap();
    assert_eq!(save.command_id.as_ref(), Some(&expected));
    assert_eq!(save.shortcut.as_deref(), Some("⌘S"));
}
