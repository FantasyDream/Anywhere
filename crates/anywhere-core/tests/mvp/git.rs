use crate::support::*;

const NEW_ACTIONS: [&str; 7] = [
    "core.git.copy_remote_url",
    "core.git.copy_branch_name",
    "core.git.copy_commit_hash",
    "core.git.copy_short_commit_hash",
    "core.git.copy_repository_path",
    "core.git.branches",
    "core.git.staged_diff",
];

#[tokio::test]
async fn git_shortcuts_search_and_copy_actions_use_the_repository_root_and_record_usage() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let nested = root.join("nested project");
    std::fs::create_dir(&nested).unwrap();
    let platform = Arc::new(TestPlatform::new(nested, true));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 0);
    assert!(initial
        .context_candidates
        .iter()
        .filter(|c| NEW_ACTIONS.contains(&c.command_id.as_deref().unwrap_or("")))
        .all(|c| !c.home_eligible));
    let searched = app
        .update_scoped_input(
            initial.session_id,
            "copy branch".into(),
            1,
            Some("app.com.apple.finder".into()),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(searched
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.git.copy_branch_name")));
    let git = app
        .update_scoped_input(initial.session_id, "".into(), 2, Some("git".into()))
        .await
        .unwrap()
        .unwrap();
    let stable_ids = NEW_ACTIONS.map(|id| candidate(&git, id));
    for id in NEW_ACTIONS {
        assert!(app
            .shortcut_actions()
            .unwrap()
            .iter()
            .any(|action| action.action_id == id));
        let targets = app
            .resolve_action_binding(git.session_id, git.revision, id)
            .unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].group_id, "git");
    }
    assert_eq!(
        platform.git_calls.load(Ordering::Relaxed),
        0,
        "search and binding discovery must not run Git"
    );
    for (id, value) in [
        (
            "core.git.copy_remote_url",
            "git@example.com:team/repo.git".to_owned(),
        ),
        ("core.git.copy_branch_name", "feature/current".to_owned()),
        (
            "core.git.copy_commit_hash",
            "0123456789abcdef0123456789abcdef01234567".to_owned(),
        ),
        ("core.git.copy_short_commit_hash", "0123456".to_owned()),
        (
            "core.git.copy_repository_path",
            root.to_str().unwrap().to_owned(),
        ),
    ] {
        let result = app
            .execute(git.session_id, git.revision, candidate(&git, id))
            .await
            .unwrap();
        assert!(result.succeeded(), "{result:?}");
        assert!(result.close_window);
        assert!(result.output_text.is_none());
        assert_eq!(
            platform.calls.lock().unwrap().last(),
            Some(&("copy".into(), value))
        );
        let usage = history.lock().unwrap().load_global().unwrap();
        assert_eq!(
            usage
                .get(&(id.into(), GIT_REPOSITORY.into()))
                .unwrap()
                .execute_count,
            1
        );
    }
    for (id, expected) in [
        ("core.git.branches", "git branches output"),
        ("core.git.staged_diff", "git staged-diff output"),
    ] {
        let result = app
            .execute(git.session_id, git.revision, candidate(&git, id))
            .await
            .unwrap();
        assert!(result.succeeded() && !result.close_window);
        assert_eq!(result.output_text.as_deref(), Some(expected));
    }
    let calls = platform.calls.lock().unwrap().clone();
    assert!(calls
        .iter()
        .filter(|(op, _)| op == "git")
        .all(|(_, value)| value.ends_with(root.to_str().unwrap())));
    app.deactivate(git.session_id).unwrap();
    let next = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert_eq!(
        NEW_ACTIONS.map(|id| candidate(&next, id)),
        stable_ids,
        "business action ids survive a new session and a rescan"
    );
}

#[tokio::test]
async fn git_copy_failures_do_not_write_clipboard_or_usage_and_success_reads_fresh_values() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let id = candidate(&state, "core.git.copy_branch_name");
    *platform.git_error.lock().unwrap() = Some("HEAD is detached".into());
    let failed = app
        .execute(state.session_id, state.revision, id.clone())
        .await
        .unwrap();
    assert!(!failed.succeeded());
    assert!(!platform
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(op, _)| op == "copy"));
    assert!(history.lock().unwrap().load_global().unwrap().is_empty());
    *platform.git_error.lock().unwrap() = None;
    for output in [
        GitOutput {
            text: "partial".into(),
            truncated: true,
        },
        GitOutput {
            text: "".into(),
            truncated: false,
        },
        GitOutput {
            text: "first\nsecond".into(),
            truncated: false,
        },
    ] {
        *platform.git_output.lock().unwrap() = Some(output);
        let result = app
            .execute(state.session_id, state.revision, id.clone())
            .await
            .unwrap();
        assert!(!result.succeeded());
        assert!(!platform
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|(operation, _)| operation == "copy"));
        assert!(history.lock().unwrap().load_global().unwrap().is_empty());
    }
    // Values are fetched when executing, never cached by the context provider.
    for branch in ["feature/first", "feature/second"] {
        *platform.git_output.lock().unwrap() = Some(GitOutput {
            text: branch.into(),
            truncated: false,
        });
        assert!(app
            .execute(state.session_id, state.revision, id.clone())
            .await
            .unwrap()
            .succeeded());
        assert_eq!(
            platform.calls.lock().unwrap().last(),
            Some(&("copy".into(), branch.into()))
        );
    }
}

#[tokio::test]
async fn missing_git_disables_commands_but_repository_path_copy_still_works() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform {
        no_git: true,
        ..TestPlatform::new(root.clone(), true)
    });
    let app = app(platform.clone());
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    for id in NEW_ACTIONS
        .into_iter()
        .filter(|id| *id != "core.git.copy_repository_path")
    {
        let action = state
            .candidates
            .iter()
            .find(|c| c.command_id.as_deref() == Some(id))
            .unwrap();
        assert_eq!(action.availability.state, AvailabilityState::Disabled);
        assert!(app
            .execute(state.session_id, state.revision, action.id.clone())
            .await
            .is_err());
    }
    let result = app
        .execute(
            state.session_id,
            state.revision,
            candidate(&state, "core.git.copy_repository_path"),
        )
        .await
        .unwrap();
    assert!(result.succeeded());
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        *platform.calls.lock().unwrap(),
        vec![("copy".into(), root.to_str().unwrap().into())]
    );
}

#[tokio::test]
async fn repository_path_copy_is_hidden_when_clipboard_already_matches() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let app = app(Arc::new(TestPlatform {
        clipboard: Mutex::new(Some(root.to_str().unwrap().into())),
        ..TestPlatform::new(root, true)
    }));
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(!state
        .context_candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.git.copy_repository_path")));
    assert!(state
        .context_candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.git.copy_branch_name")));
}
