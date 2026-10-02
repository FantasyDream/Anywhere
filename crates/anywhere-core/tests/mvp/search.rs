use crate::support::*;

#[tokio::test]
async fn git_directory_offers_one_context_and_runs_git_only_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), true));
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;

    assert_eq!(
        titles(&state)[..3],
        ["Git Status", "Open Terminal Here", "Git Log"]
    );
    assert_eq!(context_types(&state), [DIRECTORY]);
    let status = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.git.status"))
        .unwrap();
    let terminal = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.directory.terminal"))
        .unwrap();
    assert_eq!(status.context_keys, terminal.context_keys);
    assert!(state
        .context_summary
        .iter()
        .any(|context| status.context_keys.contains(&context.key)));
    assert_eq!(status.section_label.as_deref(), Some("Git"));
    assert_eq!(
        platform.git_calls.load(Ordering::Relaxed),
        0,
        "repository detection must never run git"
    );

    for (command, expected) in [
        ("core.git.status", "git status output"),
        ("core.git.log", "git log output"),
        ("core.git.diff", "git diff output"),
    ] {
        let result = app
            .execute(state.session_id, state.revision, candidate(&state, command))
            .await
            .unwrap();
        assert!(result.succeeded(), "{command}: {result:?}");
        assert!(!result.close_window, "git output stays in the preview");
        assert_eq!(result.output_text.as_deref(), Some(expected));
    }
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn context_identity_and_all_actions_survive_a_universal_query() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let app = app(Arc::new(TestPlatform::new(root, true)));
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let key = state.preferred_context_key.clone().unwrap();
    let filtered = app
        .update_input(state.session_id, "copy".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        filtered.preferred_context_key.as_deref(),
        Some(key.as_str())
    );
    let offers = filtered
        .context_candidates
        .iter()
        .filter(|candidate| candidate.context_keys.contains(&key))
        .collect::<Vec<_>>();
    assert!(offers.iter().any(|offer| offer.title == "Copy Path"));
    let git = offers
        .iter()
        .find(|offer| offer.title == "Git Status")
        .unwrap();
    assert!(!filtered
        .candidates
        .iter()
        .any(|candidate| candidate.id == git.id));
    let result = app
        .execute(filtered.session_id, filtered.revision, git.id.clone())
        .await
        .unwrap();
    assert!(
        result.succeeded(),
        "unfiltered context offers retain execution credentials"
    );
    let restored = app
        .update_input(state.session_id, String::new(), 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.preferred_context_key.as_deref(),
        Some(key.as_str())
    );
}

#[tokio::test]
async fn searching_for_actions_matches_partial_words_and_keeps_object_actions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;

    let searched = app
        .update_input(state.session_id, "git sta".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        searched.candidates[0].command_id.as_deref(),
        Some("core.git.status")
    );

    // Typed-object actions stay reachable while searching.
    let url = app
        .update_input(state.session_id, "https://github.com".into(), 2)
        .await
        .unwrap()
        .unwrap();
    let titles = titles(&url);
    assert!(titles.contains(&"Open in Default Browser"), "{titles:?}");
    // The text fallback now comes from the installed plugin; the host contributes none.
    assert!(
        titles.contains(&"Search the Web"),
        "the plugin's text action stays reachable: {titles:?}"
    );
}

/// A capture nobody is waiting for any more must not be waited on.
///
/// The wait has a 30s timeout. Letting it run after the revision is superseded keeps a
/// blocking worker parked for the rest of that timeout, and the caller that replaced the
/// revision has its own wait for the same memo.
#[tokio::test]
async fn a_pasted_document_does_not_match_the_application_index() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    let pasted = "chrome ".repeat(40_000);
    assert!(pasted.len() > 1024);

    let started = std::time::Instant::now();
    let searched = app.update_input(state.session_id, pasted, 1).await.unwrap();
    let elapsed = started.elapsed();
    // A generous bound: the guarded path measures well under a millisecond, and the
    // unguarded one took over a second on this machine.
    assert!(
        elapsed < Duration::from_millis(500),
        "a 1 MiB query took {elapsed:?}; the app index must not be scanned for it"
    );
    if let Some(state) = searched {
        assert!(
            state
                .candidates
                .iter()
                .all(|candidate| candidate.kind != CandidateKind::Application),
            "pasted content must not surface applications: {:?}",
            state
                .candidates
                .iter()
                .map(|candidate| &candidate.title)
                .collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn copy_path_is_hidden_when_the_clipboard_already_holds_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform {
        clipboard: Mutex::new(Some(root.to_string_lossy().into_owned())),
        ..TestPlatform::new(root, true)
    });
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    assert!(
        !titles(&state).contains(&"Copy Path"),
        "{:?}",
        titles(&state)
    );
    assert!(titles(&state).contains(&"Open Terminal Here"));
    assert!(
        titles(&state).contains(&"Open Directory"),
        "{:?}",
        titles(&state)
    );
}

#[tokio::test]
async fn applications_are_searchable_but_never_appear_in_context_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("Applications");
    for name in ["Google Chrome", "Notes"] {
        std::fs::create_dir_all(home.join(format!("{name}.app/Contents"))).unwrap();
    }
    let platform = Arc::new(TestPlatform::new(home.clone(), false));
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    assert!(
        state
            .candidates
            .iter()
            .all(|candidate| candidate.kind != CandidateKind::Application),
        "Home must not list applications"
    );
    // Application search needs the background index; Home never waits for it.
    app.catalog_ready().await;

    let searched = app
        .update_input(state.session_id, "chrome".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(searched.candidates[0].title, "Google Chrome");
    assert_eq!(searched.candidates[0].kind, CandidateKind::Application);
    assert_eq!(
        searched.candidates[0].bundle_id.as_deref(),
        Some("app.googlechrome.com")
    );

    let launched = app
        .execute(
            state.session_id,
            searched.revision,
            searched.candidates[0].id.clone(),
        )
        .await
        .unwrap();
    assert!(launched.succeeded());
    assert_eq!(platform.launches.load(Ordering::Relaxed), 1);
    assert!(platform
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(operation, _)| operation == "launch"));
}

/// Icons are fetched on demand for application candidates only.
#[tokio::test]
async fn application_icons_are_fetched_on_demand_for_application_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("Applications");
    std::fs::create_dir_all(home.join("Google Chrome.app/Contents")).unwrap();
    let platform = Arc::new(TestPlatform::new(home, false));
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    app.catalog_ready().await;
    let searched = app
        .update_input(state.session_id, "chrome".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let application_id = searched.candidates[0].id.clone();
    assert_eq!(searched.candidates[0].kind, CandidateKind::Application);
    assert_eq!(
        app.application_icon(&application_id).unwrap(),
        Some(vec![0x89, b'P', b'N', b'G'])
    );
    // Non-application candidates have no icon, and unknown ids are not an error.
    let url = app
        .update_input(state.session_id, "https://example.com".into(), 2)
        .await
        .unwrap()
        .unwrap();
    let action_id = candidate(&url, "core.url.open");
    assert!(app.application_icon(&action_id).unwrap().is_none());
    assert!(app.application_icon("missing-candidate").unwrap().is_none());

    // Icon lookup must not disturb the committed snapshot or the ordering.
    let after = app.subscribe(state.session_id).latest_state().unwrap();
    assert_eq!(after.revision, 2);
    assert_eq!(
        after.candidates[0].id, url.candidates[0].id,
        "fetching an icon must not reorder candidates"
    );
}

/// The recorded application usage id must be the one ranking later reads back.
#[tokio::test]
async fn a_disabled_candidate_cannot_execute() {
    struct NoGit(Arc<TestPlatform>);
    impl Platform for NoGit {
        fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
            self.0.frontmost_application()
        }
        fn application_directory(&self, app: &ApplicationInfo) -> Result<Option<PathBuf>> {
            self.0.application_directory(app)
        }
        fn bundle_application(&self, bundle: &Path) -> Result<Option<BundleApplication>> {
            self.0.bundle_application(bundle)
        }
        fn clipboard_text(&self) -> Result<Option<String>> {
            self.0.clipboard_text()
        }
        fn git_available(&self) -> bool {
            false
        }
        fn open_url(&self, url: &str) -> Result<()> {
            self.0.open_url(url)
        }
        fn open_file(&self, path: &Path) -> Result<()> {
            self.0.open_file(path)
        }
        fn reveal_file(&self, path: &Path) -> Result<()> {
            self.0.reveal_file(path)
        }
        fn open_directory(&self, path: &Path) -> Result<()> {
            self.0.open_directory(path)
        }
        fn open_terminal(&self, path: &Path) -> Result<()> {
            self.0.open_terminal(path)
        }
        fn copy_text(&self, text: &str) -> Result<()> {
            self.0.copy_text(text)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let inner = Arc::new(TestPlatform::new(root, true));
    let app = Application::new(
        Arc::new(NoGit(inner.clone())),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    let status = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.git.status"))
        .expect("git status stays visible so the group is discoverable");
    assert_eq!(status.availability.state, AvailabilityState::Disabled);
    assert!(status.availability.reason.is_some());
    let error = app
        .execute(state.session_id, state.revision, status.id.clone())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Git 不可用"));
    assert_eq!(inner.git_calls.load(Ordering::Relaxed), 0);
}

pub(crate) const WRAP_PLUGIN: &str = r#"
protocol_version = 1
id = "wrap"
name = "Wrap"

[[commands]]
id = "status"
title = "Wrapped Git Status"
accepts = ["git.repository"]
action = { type = "execute_builtin", command_id = "core.git.status" }

[[commands]]
id = "copy"
title = "Wrapped Copy"
accepts = ["core.directory"]
action = { type = "execute_builtin", command_id = "core.directory.copy" }

[[commands]]
id = "pretty"
title = "Pretty JSON"
accepts = ["core.json"]
action = { type = "execute_builtin", command_id = "core.json.format" }
"#;

#[tokio::test]
async fn a_disabled_text_fallback_cannot_execute() {
    use anywhere_core::core::action::{
        availability::AvailabilityRule,
        bundle::ActionSpec,
        handler::{ActionHandler, ExecutionOutput},
    };
    struct Marker;
    impl ActionHandler for Marker {
        fn execute(
            &self,
            context: &ResolvedContext,
            platform: &dyn Platform,
        ) -> Result<ExecutionOutput> {
            platform.copy_text(&format!("marker:{}", context.value.text()))?;
            Ok(ExecutionOutput::dismiss())
        }
    }
    let mut registry = registry();
    registry
        .register(
            ActionSpec {
                id: "test.text.require",
                title: "Need Git Text",
                accepts: TEXT,
                handler: &Marker,
                availability: AvailabilityRule::require(
                    anywhere_core::core::session::facts::GIT,
                    "git is not installed",
                ),
                base_priority: 0,
                icon: "copy",
                aliases: &[],
                keywords: &[],
                home_eligible: true,
                primary_action: false,
            }
            .descriptor("test"),
        )
        .unwrap();
    let platform = Arc::new(TestPlatform {
        no_git: true,
        ..TestPlatform::default()
    });
    let app = Application::new(
        platform.clone(),
        registry,
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let state = app
        .update_input(session, "zzzznotaname".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let fallback = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("test.text.require"))
        .expect("disabled text fallback stays visible");
    assert_eq!(fallback.availability.state, AvailabilityState::Disabled);
    let error = app
        .execute(session, state.revision, fallback.id.clone())
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("git is not installed"),
        "{error}"
    );
    assert!(platform.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn git_false_and_missing_use_different_disabled_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let unavailable = Arc::new(TestPlatform {
        no_git: true,
        ..TestPlatform::new(root.clone(), true)
    });
    let unavailable_app = app(unavailable);
    let state = settled(
        &unavailable_app,
        unavailable_app
            .activate()
            .await
            .unwrap()
            .unwrap()
            .session_id,
        0,
    )
    .await;
    let status = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.git.status"))
        .unwrap();
    assert_eq!(status.availability.state, AvailabilityState::Disabled);
    assert_eq!(status.availability.reason.as_deref(), Some("Git 不可用"));

    let missing = Arc::new(TestPlatform {
        git_panics: true,
        ..TestPlatform::new(root, true)
    });
    let missing_app = app(missing);
    let state = settled(
        &missing_app,
        missing_app.activate().await.unwrap().unwrap().session_id,
        0,
    )
    .await;
    let status = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.git.status"))
        .unwrap();
    assert_eq!(status.availability.state, AvailabilityState::Disabled);
    assert_eq!(
        status.availability.reason.as_deref(),
        Some("无法确认 Git 是否可用")
    );
    assert!(
        state
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("git")),
        "{:?}",
        state.diagnostics
    );
}

#[tokio::test]
async fn clipboard_absence_or_read_failure_does_not_hide_copy_url() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for clipboard_error in [false, true] {
        let platform = Arc::new(TestPlatform {
            clipboard_error,
            clipboard: Mutex::new(clipboard_error.then(|| "https://example.org/page".into())),
            ..TestPlatform::new(root.clone(), true)
        });
        let app = app(platform.clone());
        let initial = app.activate().await.unwrap().unwrap();
        let _ = settled(&app, initial.session_id, 0).await;
        app.update_input(initial.session_id, "https://example.org/page".into(), 1)
            .await
            .unwrap()
            .unwrap();
        let state = settled(&app, initial.session_id, 1).await;
        assert!(app
            .execute(
                state.session_id,
                state.revision,
                candidate(&state, "core.url.copy")
            )
            .await
            .unwrap()
            .succeeded());
        assert!(platform
            .calls
            .lock()
            .unwrap()
            .contains(&("copy".into(), "https://example.org/page".into())));
    }
}

#[tokio::test]
async fn clipboard_absence_or_read_failure_does_not_hide_copy_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let missing = Arc::new(TestPlatform::new(root.clone(), true));
    let missing_app = app(missing);
    let state = settled(
        &missing_app,
        missing_app.activate().await.unwrap().unwrap().session_id,
        0,
    )
    .await;
    assert!(
        titles(&state).contains(&"Copy Path"),
        "{:?}",
        titles(&state)
    );

    let failed = Arc::new(TestPlatform {
        clipboard_error: true,
        clipboard: Mutex::new(Some("secret-clipboard-body".into())),
        ..TestPlatform::new(root, true)
    });
    let failed_app = app(failed);
    let state = settled(
        &failed_app,
        failed_app.activate().await.unwrap().unwrap().session_id,
        0,
    )
    .await;
    assert!(
        titles(&state).contains(&"Copy Path"),
        "{:?}",
        titles(&state)
    );
    assert!(
        state.diagnostics.iter().any(
            |diagnostic| diagnostic.contains("clipboard") && diagnostic.contains("read failed")
        ),
        "{:?}",
        state.diagnostics
    );
    assert!(
        state
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.contains("secret-clipboard-body")),
        "{:?}",
        state.diagnostics
    );
}

#[test]
fn application_matching_is_not_charged_to_ranking() {
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    #[derive(Default)]
    struct Durations {
        applications: AtomicU64,
        ranking: AtomicU64,
    }
    struct Capture(Arc<Durations>);
    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            struct Visitor<'a>(&'a Durations);
            impl tracing::field::Visit for Visitor<'_> {
                fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                    match field.name() {
                        "sources_duration" => {
                            self.0.applications.store(value, AtomicOrdering::Relaxed);
                        }
                        "ranking_duration" => {
                            self.0.ranking.store(value, AtomicOrdering::Relaxed);
                        }
                        _ => {}
                    }
                }
                fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
            }
            event.record(&mut Visitor(&self.0));
        }
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..2_000 {
            std::fs::create_dir_all(dir.path().join(format!("Example Application {index}.app")))
                .unwrap();
        }
        let app = app(Arc::new(TestPlatform::new(
            dir.path().canonicalize().unwrap(),
            false,
        )));
        // Activate first so the background scan starts, then let it finish: the index is
        // one scan per process, and a catalog injected before it lands would be
        // overwritten by it.
        let state = app.activate().await.unwrap().unwrap();
        app.catalog_ready().await;
        // A real-sized index: the application phase has to be measurable for the split to
        // mean anything.
        let durations = Arc::new(Durations::default());
        {
            use tracing_subscriber::layer::SubscriberExt;
            let subscriber = tracing_subscriber::registry().with(Capture(durations.clone()));
            let _guard = tracing::subscriber::set_default(subscriber);
            let searched = app
                .update_input(state.session_id, "example 1999".into(), 1)
                .await
                .unwrap()
                .unwrap();
            assert!(
                searched
                    .candidates
                    .iter()
                    .any(|candidate| candidate.kind == CandidateKind::Application),
                "the query must reach the application index: {:?}",
                searched
                    .candidates
                    .iter()
                    .map(|candidate| &candidate.title)
                    .collect::<Vec<_>>()
            );
        }
        let applications = durations.applications.load(AtomicOrdering::Relaxed);
        let ranking = durations.ranking.load(AtomicOrdering::Relaxed);
        assert!(
            applications >= 50,
            "matching 2000 applications must be a measurable cost, got {applications}us"
        );
        assert!(
            ranking < applications,
            "ranking must not absorb application matching: ranking {ranking}us vs \
             applications {applications}us"
        );
    });
}
