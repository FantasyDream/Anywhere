use crate::support::*;
use anywhere_core::core::search::candidate::Candidate;
use anywhere_core::core::search::candidate_source::{CandidateSource, SearchContext};

struct UniversalOnly;
impl CandidateSource for UniversalOnly {
    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        assert!(
            search.scope_id.is_none(),
            "Other search sources must not run inside Action Scope"
        );
        Vec::new()
    }
}

#[tokio::test]
async fn existing_scopes_keep_slots_and_scoped_search_filters_before_other_sources() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let mut app = app(Arc::new(TestPlatform::new(root, true)));
    app.register_candidate_source(Arc::new(UniversalOnly));
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let scope_ids: Vec<_> = initial
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    assert!(scope_ids.contains(&"git".into()));
    assert!(scope_ids.contains(&"folder".into()));
    assert_eq!(initial.scopes[0].id, "app.com.apple.finder");
    assert!(initial.scopes[0].is_current_app);
    assert_eq!(initial.scopes[0].label, "Finder");
    let filtered = app
        .update_scoped_input(initial.session_id, "copy".into(), 1, Some("git".into()))
        .await
        .unwrap()
        .unwrap();
    assert!(
        filtered.candidates.is_empty(),
        "Git must not include Folder's matching Copy Path"
    );
    assert_eq!(
        filtered.scopes[..scope_ids.len()]
            .iter()
            .map(|scope| scope.id.clone())
            .collect::<Vec<_>>(),
        scope_ids
    );
    let git = app
        .update_scoped_input(initial.session_id, "sta".into(), 2, Some("git".into()))
        .await
        .unwrap()
        .unwrap();
    assert!(git
        .candidates
        .iter()
        .any(|candidate| candidate.title == "Git Status"));
    assert!(git
        .candidates
        .iter()
        .all(|candidate| candidate.kind == CandidateKind::Action && candidate.group_id == "git"));
    let missing = app
        .update_scoped_input(initial.session_id, "copy".into(), 3, Some("missing".into()))
        .await
        .unwrap()
        .unwrap();
    assert!(
        missing.candidates.is_empty(),
        "Unknown scopes cannot widen to Universal"
    );
    assert!(
        app.execute(initial.session_id, 2, candidate(&git, "core.git.status"))
            .await
            .is_err(),
        "Superseded scope requests still reject execution"
    );
}

#[tokio::test]
async fn app_scope_is_first_and_menu_name_queries_do_not_expand_all_menus() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), false));
    *platform.menu_trusted.lock().unwrap() = true;
    *platform.menu_items.lock().unwrap() = [
        ("Toggle Terminal", vec!["View", "Toggle Terminal"]),
        ("Export", vec!["File", "Export"]),
        ("Unavailable", vec!["File", "Unavailable"]),
    ]
    .into_iter()
    .map(|(title, path)| DiscoveredMenuItem {
        title: title.into(),
        menu_path: path.iter().map(|part| (*part).into()).collect(),
        identifiers: vec![None; path.len()],
        shortcut: None,
        enabled: title != "Unavailable",
    })
    .collect();
    let app = app(platform);
    let initial = app.activate().await.unwrap().unwrap();
    let mut state = initial.clone();
    for _ in 0..200 {
        if state.scopes.iter().any(|scope| scope.is_current_app) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        state = app.latest_state(initial.session_id).unwrap();
    }
    assert_eq!(state.scopes[0].id, "app.test.editor");
    assert_eq!(state.scopes[0].label, "Editor");
    assert!(!state.scopes.iter().any(|scope| scope.id == "menu"));
    assert!(!state
        .context_candidates
        .iter()
        .any(|candidate| candidate.title == "Unavailable"));
    assert!(
        state
            .context_candidates
            .iter()
            .find(|candidate| candidate.title == "Toggle Terminal")
            .unwrap()
            .home_eligible
    );
    assert!(
        !state
            .context_candidates
            .iter()
            .find(|candidate| candidate.title == "Export")
            .unwrap()
            .home_eligible
    );
    let app_only = app
        .update_input(initial.session_id, "editor".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(!app_only.candidates.iter().any(|candidate| candidate
        .command_id
        .as_deref()
        .is_some_and(|id| id.starts_with("app.test.editor."))));
    let qualified = app
        .update_input(initial.session_id, "editor terminal".into(), 2)
        .await
        .unwrap()
        .unwrap();
    let terminal = qualified
        .candidates
        .iter()
        .find(|candidate| candidate.title == "Toggle Terminal")
        .unwrap();
    assert_eq!(terminal.subtitle.as_deref(), Some("View"));
    let scoped = app
        .update_scoped_input(
            initial.session_id,
            "export".into(),
            3,
            Some("app.test.editor".into()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(titles(&scoped), ["Export", "Search the Web"]);
    assert!(scoped
        .candidates
        .iter()
        .all(|candidate| candidate.kind == CandidateKind::Action));
}

#[tokio::test]
async fn current_app_scope_searches_and_executes_domain_actions_without_menus() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), true));
    *platform.clipboard.lock().unwrap() = Some("https://example.com".into());
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let mut app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    app.register_candidate_source(Arc::new(UniversalOnly));
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(state.scopes[0].is_current_app);
    assert_eq!(state.scopes[0].label, "Finder");
    let scope = Some(state.scopes[0].id.clone());
    let home = app
        .update_scoped_input(state.session_id, "".into(), 1, scope.clone())
        .await
        .unwrap()
        .unwrap();
    for group in ["folder", "git", "web"] {
        assert!(
            home.candidates
                .iter()
                .any(|action| action.group_id == group),
            "{group}"
        );
    }
    assert!(
        history.lock().unwrap().0.is_empty(),
        "Opening Current App must not record usage"
    );
    for (revision, query, command, group) in [
        (2, "copy path", "core.directory.copy", "folder"),
        (3, "git status", "core.git.status", "git"),
        (4, "default browser", "core.url.open", "web"),
    ] {
        let result = app
            .update_scoped_input(state.session_id, query.into(), revision, scope.clone())
            .await
            .unwrap()
            .unwrap();
        let action = result
            .candidates
            .iter()
            .find(|action| action.command_id.as_deref() == Some(command))
            .unwrap_or_else(|| panic!("{query} missing from Current App"));
        assert_eq!(
            action.group_id, group,
            "Aggregating scopes must not rewrite Action Group"
        );
        assert!(result
            .candidates
            .iter()
            .all(|action| action.kind == CandidateKind::Action));
        assert!(result
            .candidates
            .iter()
            .all(|action| action.availability.is_visible()));
        assert!(app
            .execute(state.session_id, revision, action.id.clone())
            .await
            .unwrap()
            .succeeded());
    }
    assert_eq!(
        platform.calls.lock().unwrap().as_slice(),
        &[
            ("copy".into(), root.to_string_lossy().into_owned()),
            ("git".into(), format!("status:{}", root.display())),
            ("url".into(), "https://example.com/".into()),
        ]
    );
    let usage = history.lock().unwrap();
    for (command, context) in [
        ("core.directory.copy", DIRECTORY),
        ("core.git.status", GIT_REPOSITORY),
        ("core.url.open", URL),
    ] {
        assert_eq!(
            usage
                .0
                .get(&(command.into(), context.into(), "com.apple.finder".into()))
                .unwrap()
                .execute_count,
            1
        );
    }
}

#[test]
fn universal_menu_search_preserves_direct_titles_and_application_qualifiers() {
    use anywhere_core::core::ranking::score::UsageStats;
    use anywhere_core::core::search::{engine::ActionSearch, query::Query};
    use anywhere_core::core::session::facts::SessionFacts;
    for (bundle_id, name, title, query, app_query) in [
        ("com.apple.finder", "Finder", "Find", "find", "finder"),
        (
            "com.microsoft.VSCode",
            "Visual Studio Code",
            "Toggle Terminal",
            "vscode terminal",
            "vscode",
        ),
        (
            "com.apple.Preview",
            "Preview",
            "Actual Size",
            "preview actual size",
            "preview",
        ),
    ] {
        let application = ApplicationInfo {
            bundle_id: bundle_id.into(),
            name: name.into(),
            pid: 42,
            instance_id: Some("test".into()),
        };
        let menu = |title: &str| DiscoveredMenuItem {
            title: title.into(),
            menu_path: vec!["View".into(), title.into()],
            identifiers: vec![None, None],
            shortcut: None,
            enabled: true,
        };
        let registry = anywhere_core::features::app_actions::convert::registry(
            &ActionRegistry::default(),
            &application,
            vec![menu(title), menu("Unrelated")],
        )
        .unwrap();
        let context =
            anywhere_core::features::applications::context::frontmost_context(&application);
        let usage = |_: &str, _: &str| UsageStats::default();
        let facts = SessionFacts::default();
        let search = ActionSearch::new(
            &registry,
            &usage,
            &facts,
            1000,
            anywhere_core::features::context_semantic,
        );
        let matched = search.search(&Query::parse(query), std::slice::from_ref(&context), &[]);
        assert_eq!(matched.candidates.len(), 1, "{query}");
        assert_eq!(matched.candidates[0].presentation.title, title, "{query}");
        assert!(
            search
                .search(&Query::parse(app_query), &[context], &[])
                .candidates
                .is_empty(),
            "{app_query} alone must not expand application menus"
        );
    }
}
