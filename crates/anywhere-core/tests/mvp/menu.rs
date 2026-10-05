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
async fn menu_actions_belong_to_application_context_without_a_menu_root() {
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
    assert_eq!(context_types(&state), [APPLICATION]);
    let action = state
        .candidates
        .iter()
        .find(|candidate| candidate.title == "Toggle Terminal")
        .unwrap();
    assert_eq!(action.group_id, "app.test.editor");
    assert_eq!(action.owning_app.as_deref(), Some("test.editor"));
    assert!(action.home_eligible);
    assert_eq!(action.section_label.as_deref(), Some("Editor"));
    assert!(state.context_summary.iter().any(
        |context| context.type_id == APPLICATION && action.context_keys.contains(&context.key)
    ));
    assert!(action
        .command_id
        .as_deref()
        .unwrap()
        .starts_with("app.test.editor."));
    let _ = settled(&app, first.session_id, 0).await;
    let menu_id = action.command_id.as_deref().unwrap();
    assert!(app
        .resolve_action_binding(first.session_id, 0, menu_id)
        .unwrap_err()
        .to_string()
        .contains("persistent shortcuts"));
    assert!(app
        .shortcut_actions()
        .unwrap()
        .iter()
        .all(|option| option.action_id != menu_id));
}

#[tokio::test]
async fn unauthorized_menu_access_hides_the_application_context_entry() {
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
        !context_types(&latest).contains(&APPLICATION),
        "an editor without accessibility must not show an empty current app scope: {:?}",
        context_types(&latest)
    );
    assert!(
        !latest
            .context_summary
            .iter()
            .any(|context| context.type_id == APPLICATION && !context.candidate_ids.is_empty()),
        "inaccessible menus have no application entry"
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
    assert!(!context_types(&latest).contains(&APPLICATION));
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
async fn late_menu_discovery_keeps_directory_identity_and_adds_application_context() {
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
    let preferred = first.preferred_context_key.clone();
    assert_eq!(context_types(&first), [DIRECTORY]);
    gate.release();
    let late = wait_for_title(&app, first.session_id, "New Folder").await;
    assert_eq!(context_types(&late), [DIRECTORY, APPLICATION]);
    assert_eq!(late.preferred_context_key, preferred);
    assert!(context_actions(&late, DIRECTORY).contains(&"Git Status"));
    assert!(context_actions(&late, APPLICATION).contains(&"New Folder"));
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
async fn unverified_menu_execution_neither_records_success_nor_failure() {
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

    let success = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(success.succeeded(), "{}", success.notice());
    assert!(success.close_window);
    assert!(history.lock().unwrap().0.is_empty());

    *platform.menu_perform_error.lock().unwrap() = Some("menu is unavailable".into());
    let failure = app
        .execute(state.session_id, state.revision, about.id.clone())
        .await
        .unwrap();
    assert!(!failure.succeeded());
    assert!(history.lock().unwrap().0.is_empty());
}

#[tokio::test]
async fn trusted_empty_menus_hide_the_application_context_entry() {
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
        !context_types(&latest).contains(&APPLICATION),
        "no menu items must not create a current app scope: {:?}",
        context_types(&latest)
    );
    assert!(
        latest.diagnostics.is_empty(),
        "an expected empty menu must not distract universal search: {:?}",
        latest.diagnostics
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
    assert!(context_types(&state).contains(&DIRECTORY));
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

#[tokio::test]
async fn rescan_capabilities_reject_old_requests_even_when_business_id_is_reused() {
    let platform = Arc::new(TestPlatform::default());
    let mut save = sample_menu("Save", &["File", "Save"], true);
    save.identifiers[1] = Some("reused".into());
    with_menus(&platform, vec![save]);
    let app = app(platform.clone());
    let warm = app.activate().await.unwrap().unwrap();
    wait_for_title(&app, warm.session_id, "Save").await;
    app.deactivate(warm.session_id).unwrap();
    let gate = CaptureGate::new();
    *platform.menu_gate.lock().unwrap() = Some(gate.clone());
    let old = app.activate().await.unwrap().unwrap();
    let save = old.candidates.iter().find(|c| c.title == "Save").unwrap();
    let mut delete = sample_menu("Delete", &["Tools", "Delete"], true);
    delete.identifiers[1] = Some("reused".into());
    *platform.menu_items.lock().unwrap() = vec![delete];
    gate.release();
    let new = wait_for_title(&app, old.session_id, "Delete").await;
    let delete = new.candidates.iter().find(|c| c.title == "Delete").unwrap();
    assert_eq!(old.revision, new.revision);
    assert!(new.snapshot_seq > old.snapshot_seq);
    assert_eq!(save.command_id, delete.command_id);
    assert_ne!(save.id, delete.id);
    assert!(app
        .execute(old.session_id, old.revision, save.id.clone())
        .await
        .is_err());
    assert_eq!(platform.menu_performs.load(Ordering::SeqCst), 0);
    assert!(app
        .execute(new.session_id, new.revision, delete.id.clone())
        .await
        .unwrap()
        .succeeded());
    assert_eq!(platform.menu_performs.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reranking_preserves_the_current_menu_capability() {
    let platform = Arc::new(TestPlatform::default());
    with_menus(
        &platform,
        vec![sample_menu("Save", &["File", "Save"], true)],
    );
    let app = app(platform);
    let first = app.activate().await.unwrap().unwrap();
    let first = wait_for_title(&app, first.session_id, "Save").await;
    let save = first.candidates.iter().find(|c| c.title == "Save").unwrap();
    let next = app
        .update_input(first.session_id, "Save".into(), first.revision + 1)
        .await
        .unwrap()
        .unwrap();
    let next = settled(&app, first.session_id, next.revision).await;
    let filtered = next.candidates.iter().find(|c| c.title == "Save").unwrap();
    assert_eq!(save.id, filtered.id);
    assert_eq!(save.command_id, filtered.command_id);
}

#[tokio::test]
async fn sqlite_menu_history_is_quarantined_across_identity_transitions_and_reopen() {
    use anywhere_core::features::app_actions::descriptors;
    for scenario in [
        "reassigned",
        "duplicate-survivor",
        "identifier-lost",
        "renamed-path",
        "same-path",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("usage.db");
        let platform = Arc::new(TestPlatform::default());
        let identity = platform.frontmost_application().unwrap().unwrap();
        let mut original = sample_menu("Uncommon", &["File", "Uncommon"], true);
        if scenario != "renamed-path" && scenario != "same-path" {
            original.identifiers[1] = Some("reused".into());
        }
        let original_id = descriptors(&identity, vec![original.clone()])[0].id.clone();
        let mut current = original.clone();
        if scenario == "reassigned" || scenario == "duplicate-survivor" {
            current.title = "Other Uncommon".into();
            current.menu_path = vec!["Tools".into(), current.title.clone()];
        } else if scenario == "identifier-lost" {
            current.identifiers[1] = None;
        } else if scenario == "renamed-path" {
            current.menu_path[0] = "文件".into();
        } else {
            current.shortcut = Some("⌘D".into());
        }
        let current_id = descriptors(&identity, vec![current.clone()])[0].id.clone();
        if scenario == "duplicate-survivor" {
            assert!(descriptors(&identity, vec![original, current.clone()])
                .iter()
                .all(|action| action.id != original_id));
        }
        if scenario == "identifier-lost" || scenario == "renamed-path" {
            assert_ne!(original_id, current_id);
        } else {
            assert_eq!(original_id, current_id);
        }
        let mut history = SqliteUsageRepository::open(&database).unwrap();
        history
            .record(&original_id, APPLICATION, "test.editor", 100)
            .unwrap();
        drop(history);
        with_menus(&platform, vec![current.clone()]);
        let app = Application::new(
            platform,
            registry(),
            Box::new(SqliteUsageRepository::open(&database).unwrap()),
            vec![],
        );
        let state = app.activate().await.unwrap().unwrap();
        wait_for_title(&app, state.session_id, &current.title).await;
        let state = settled(&app, state.session_id, state.revision).await;
        let candidate = state
            .candidates
            .iter()
            .find(|c| c.title == current.title)
            .unwrap();
        assert!(!candidate.home_eligible, "legacy usage promoted {scenario}");
        let score = candidate.score;
        assert!(app
            .execute(state.session_id, state.revision, candidate.id.clone())
            .await
            .unwrap()
            .succeeded());
        app.deactivate(state.session_id).unwrap();
        drop(app);
        let mut history = SqliteUsageRepository::open(&database).unwrap();
        let rows = history.load("test.editor").unwrap();
        assert_eq!(
            rows[&(original_id.clone(), APPLICATION.to_owned())].execute_count,
            1
        );
        assert_eq!(rows.len(), 1);
        // Even a large legacy count cannot affect ranking on the next App instance.
        for _ in 0..40 {
            history
                .record(&current_id, APPLICATION, "test.editor", 200)
                .unwrap();
        }
        drop(history);
        let platform = Arc::new(TestPlatform::default());
        with_menus(&platform, vec![current.clone()]);
        let app = Application::new(
            platform,
            registry(),
            Box::new(SqliteUsageRepository::open(&database).unwrap()),
            vec![],
        );
        let state = app.activate().await.unwrap().unwrap();
        wait_for_title(&app, state.session_id, &current.title).await;
        let state = settled(&app, state.session_id, state.revision).await;
        let candidate = state
            .candidates
            .iter()
            .find(|c| c.title == current.title)
            .unwrap();
        assert_eq!(candidate.score, score, "legacy usage affected {scenario}");
        assert!(!candidate.home_eligible);
        app.deactivate(state.session_id).unwrap();
    }
}

#[test]
fn queued_menu_execution_rechecks_the_discovery_before_calling_platform() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(async {
            let platform = Arc::new(TestPlatform::default());
            with_menus(
                &platform,
                vec![sample_menu("Save", &["File", "Save"], true)],
            );
            let app = app(platform.clone());
            let state = app.activate().await.unwrap().unwrap();
            wait_for_title(&app, state.session_id, "Save").await;
            let state = settled(&app, state.session_id, state.revision).await;
            let save = state.candidates.iter().find(|c| c.title == "Save").unwrap();
            let gate = CaptureGate::new();
            let worker_gate = gate.clone();
            let (started, ready) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                started.send(()).unwrap();
                worker_gate.wait();
            });
            ready.await.unwrap();
            let execution = app.execute(state.session_id, state.revision, save.id.clone());
            tokio::pin!(execution);
            tokio::select! {
                _ = &mut execution => panic!("execution bypassed the occupied worker"),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {},
            }
            *platform.menu_trusted.lock().unwrap() = false;
            assert!(!app.prompt_accessibility().unwrap());
            // The platform could now execute the old locator, but its catalog capability
            // was revoked. Worker validation must prevent the call independently of AX.
            *platform.menu_trusted.lock().unwrap() = true;
            gate.release();
            let result = execution.await.unwrap();
            assert!(!result.succeeded());
            assert!(result.notice().contains("discovery expired"));
            assert_eq!(platform.menu_performs.load(Ordering::SeqCst), 0);
            blocker.await.unwrap();
        });
}
