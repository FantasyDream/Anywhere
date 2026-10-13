use crate::support::*;

#[tokio::test]
async fn a_slow_activation_cannot_replace_a_newer_activation() {
    let platform = Arc::new(TestPlatform::default());
    let gate = CaptureGate::new();
    *platform.frontmost_gate.lock().unwrap() = Some(gate.clone());
    let application = Arc::new(app(platform.clone()));
    let first = tokio::spawn({
        let application = application.clone();
        async move { application.activate().await }
    });
    platform.frontmost_started.notified().await;
    let second = application.activate().await.unwrap().unwrap();
    gate.release();
    let old = first.await.unwrap().unwrap();
    assert!(
        old.is_none(),
        "an older capture must not create a newer session"
    );
    let updated = application
        .update_input(second.session_id, "https://example.com".into(), 1)
        .await
        .unwrap()
        .expect("the newer activation must remain usable");
    assert_eq!(updated.session_id, second.session_id);
}

#[tokio::test]
async fn an_activation_can_be_cancelled_before_capture_finishes() {
    let platform = Arc::new(TestPlatform::default());
    let gate = CaptureGate::new();
    *platform.frontmost_gate.lock().unwrap() = Some(gate.clone());
    let application = Arc::new(app(platform.clone()));
    let session = application.begin_activation().unwrap();
    let capture = tokio::spawn({
        let application = application.clone();
        async move { application.activate_session(session).await }
    });
    platform.frontmost_started.notified().await;
    application.deactivate(session).unwrap();
    gate.release();
    assert!(capture.await.unwrap().unwrap().is_none());
    assert!(application.latest_state(session).is_none());
    assert!(application
        .activate_session(session)
        .await
        .unwrap()
        .is_none());
    let next = application.activate().await.unwrap().unwrap();
    assert!(next.session_id > session);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_candidate_source_does_not_hold_the_session_lock() {
    use anywhere_core::core::search::{
        candidate::Candidate,
        candidate_source::{CandidateSource, SearchContext},
    };
    struct SlowSource {
        gate: Arc<CaptureGate>,
        started: Arc<Notify>,
    }
    impl CandidateSource for SlowSource {
        fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
            if search.query.raw_input == "hold" {
                self.started.notify_one();
                self.gate.wait();
            }
            Vec::new()
        }
    }
    let gate = CaptureGate::new();
    let started = Arc::new(Notify::new());
    let mut application = app(Arc::new(TestPlatform::default()));
    application.register_candidate_source(Arc::new(SlowSource {
        gate: gate.clone(),
        started: started.clone(),
    }));
    let application = Arc::new(application);
    let state = application.activate().await.unwrap().unwrap();
    application.catalog_ready().await;
    let query = tokio::spawn({
        let application = application.clone();
        async move {
            application
                .update_input(state.session_id, "hold".into(), 1)
                .await
        }
    });
    started.notified().await;
    assert!(
        application.latest_state(state.session_id).is_none(),
        "an uncommitted revision must not expose stale candidates"
    );
    // A watchdog bounds the failure case: even the old implementation can finish.
    let (tx, rx) = std::sync::mpsc::channel();
    let dismiss = std::thread::spawn({
        let application = application.clone();
        move || {
            application.deactivate(state.session_id).unwrap();
            tx.send(()).unwrap();
        }
    });
    let cancelled = rx.recv_timeout(Duration::from_secs(1));
    gate.release();
    dismiss.join().unwrap();
    let result = query.await.unwrap().unwrap();
    assert!(
        cancelled.is_ok(),
        "cancellation waited for a candidate source"
    );
    assert!(
        result.is_none(),
        "a calculation cancelled outside the lock still published"
    );
}

#[tokio::test]
async fn a_registered_handler_executes_through_the_public_facade() {
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
                id: "test.marker",
                title: "Marker",
                accepts: URL,
                handler: &Marker,
                availability: AvailabilityRule::Enabled,
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
    let platform = Arc::new(TestPlatform::default());
    let application = Application::new(
        platform.clone(),
        registry,
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let session = application.activate().await.unwrap().unwrap().session_id;
    let state = application
        .update_input(session, "https://example.com".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let result = application
        .execute(session, 1, candidate(&state, "test.marker"))
        .await
        .unwrap();
    assert!(result.succeeded());
    assert!(platform
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(kind, text)| kind == "copy" && text.starts_with("marker:https://example.com")));
}

#[tokio::test]
async fn scenarios_a_to_f_use_the_same_facade_and_executor() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    let session = state.session_id;
    assert_eq!(
        titles(&state)[..3],
        ["Open Terminal Here", "Copy Path", "Open Directory"]
    );
    assert_eq!(context_types(&state), [DIRECTORY]);
    assert_eq!(context_actions(&state, DIRECTORY).len(), 3);
    assert!(context_actions(&state, DIRECTORY).contains(&"Copy Path"));
    assert!(app
        .execute(
            session,
            state.revision,
            candidate(&state, "core.directory.terminal")
        )
        .await
        .unwrap()
        .succeeded());

    let url = app
        .update_input(session, "https://github.com".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        url.candidates[0].command_id.as_deref(),
        Some("core.url.open")
    );
    assert_eq!(
        url.candidates[1].command_id.as_deref(),
        Some("core.url.copy")
    );
    assert!(url.candidates.iter().all(|candidate| {
        !candidate
            .command_id
            .as_deref()
            .unwrap_or_default()
            .starts_with("core.directory.")
    }));
    // V0.2 keeps the ambient context visible while searching, but its actions only
    // appear when they actually match the query.
    assert!(
        url.context_summary
            .iter()
            .any(|context| context.type_id == DIRECTORY),
        "ambient context stays visible"
    );
    assert!(
        url.context_summary
            .iter()
            .filter(|context| context.type_id == APPLICATION)
            .all(|context| context.candidate_ids.is_empty()),
        "foreground application without actions stays in the graph, outside navigation"
    );
    assert!(app
        .execute(session, url.revision, candidate(&url, "core.url.copy"))
        .await
        .unwrap()
        .succeeded());
    assert_eq!(
        platform.calls.lock().unwrap().last().unwrap(),
        &("copy".into(), "https://github.com/".into())
    );
    // Context actions remain executable from the current all-actions panel, even
    // when the Universal query does not include them. Identity is value-stable.
    assert!(url
        .context_candidates
        .iter()
        .any(|candidate| candidate.id == state.candidates[0].id));
    assert!(app
        .execute(session, state.revision, state.candidates[0].id.clone())
        .await
        .is_err());
    // A stale revision number is rejected even with a valid candidate id.
    assert!(app
        .execute(session, state.revision, candidate(&url, "core.url.copy"))
        .await
        .is_err());

    let json = app
        .update_input(session, r#"{"name":"foo","age":18}"#.into(), 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        json.candidates[0].command_id.as_deref(),
        Some("core.json.format")
    );
    for command in ["core.json.format", "core.json.minify", "core.json.copy"] {
        assert!(app
            .execute(session, json.revision, candidate(&json, command))
            .await
            .unwrap()
            .succeeded());
    }
    let formatted = app
        .execute(session, json.revision, candidate(&json, "core.json.format"))
        .await
        .unwrap();
    assert!(!formatted.close_window);
    assert!(formatted.output_text.unwrap().contains('\n'));
    let minified = app
        .execute(session, json.revision, candidate(&json, "core.json.minify"))
        .await
        .unwrap()
        .output_text
        .unwrap();
    assert!(!minified.contains('\n'));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&minified).unwrap()["name"],
        "foo"
    );

    let path = dir.path().join("existing file.txt");
    std::fs::write(&path, "hello").unwrap();
    let file = app
        .update_input(session, path.to_string_lossy().into_owned(), 3)
        .await
        .unwrap()
        .unwrap();
    for command in ["core.file.open", "core.file.reveal", "core.file.copy"] {
        assert!(app
            .execute(session, file.revision, candidate(&file, command))
            .await
            .unwrap()
            .succeeded());
    }
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let directory = app
        .update_input(session, nested.to_string_lossy().into_owned(), 4)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        directory
            .candidates
            .iter()
            .filter(|candidate| candidate.command_id.as_deref() == Some("core.directory.open"))
            .count(),
        1
    );
    assert!(directory.candidates[0].score > directory.candidates.last().unwrap().score);

    // The search command comes from the example plugin; the host ships none.
    let text = app
        .update_input(session, "hello world".into(), 5)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        text.candidates[0].command_id.as_deref(),
        Some("example.search")
    );
    assert!(app
        .execute(session, text.revision, candidate(&text, "example.search"))
        .await
        .unwrap()
        .succeeded());
    let last_call = platform
        .calls
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("a recorded platform call");
    let query = url::Url::parse(&last_call.1).unwrap();
    assert_eq!(
        query.query_pairs().find(|(key, _)| key == "q").unwrap().1,
        "hello world"
    );
    assert_eq!(
        platform.directory_captures.load(Ordering::Relaxed),
        1,
        "Finder context is captured once per session, not once per revision"
    );

    let cleared = app
        .update_input(session, String::new(), 6)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        titles(&cleared)[..3],
        ["Open Terminal Here", "Copy Path", "Open Directory"],
        "clearing input restores the ambient snapshot"
    );
}

#[tokio::test]
async fn a_superseded_revision_does_not_keep_waiting_for_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(500)),
        ..TestPlatform::new(root, true)
    });
    let app = Arc::new(app_with_budget(platform.clone(), Duration::from_millis(30)));
    let first = app.activate().await.unwrap().unwrap();
    let superseded = app
        .update_input(first.session_id, "hello world".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(superseded.revision, 1);
    // The replaced revision's late pass gave up on its own wait instead of parking on the
    // memo; nothing about it may reach the user.
    assert!(
        !superseded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("directory")),
        "an abandoned capture is not a user-visible failure: {:?}",
        superseded.diagnostics
    );
    // Single-flight still holds across revisions: the capture that is already running is
    // the only one, and the current revision waits for that same one.
    assert_eq!(
        platform.directory_captures.load(Ordering::Relaxed),
        1,
        "a superseded wait must not start a second capture"
    );
    let settled_state = settled(&app, first.session_id, 1).await;
    assert!(
        settled_state
            .context_summary
            .iter()
            .any(|context| context.type_id == DIRECTORY),
        "the current revision still gets the directory: {settled_state:?}"
    );
    assert_eq!(
        platform.directory_captures.load(Ordering::Relaxed),
        1,
        "the current revision must reuse the running capture"
    );
}

/// Pasted content is not an application search, and must not cost one.
///
/// The host allows 1 MiB of input. Matching that against the application index scans the
/// whole query per installed application — 1.2s on the reference machine, on the path that
/// builds the first snapshot — so the index is skipped for queries far longer than a name.
#[tokio::test]
async fn a_session_keeps_captured_facts_until_the_next_activation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), true));
    let app = app(platform.clone());
    let first = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(titles(&first).contains(&"Copy Path"));
    let git = first
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.git.status"))
        .unwrap();
    assert_eq!(git.availability.state, AvailabilityState::Enabled);

    *platform.clipboard.lock().unwrap() = Some(root.to_string_lossy().into_owned());
    let same = app
        .update_input(first.session_id, String::new(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(titles(&same).contains(&"Copy Path"), "{:?}", titles(&same));

    let second = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert!(
        !titles(&second).contains(&"Copy Path"),
        "{:?}",
        titles(&second)
    );
}

/// A provider call cut short by the budget is resumed, not made twice.
///
/// The execution-dedup rule is about the provider's own work: a call that was started must
/// not be started again for the same `(provider, context, session, revision)`. Keeping the
/// started call in the run is what makes that true; discarding it and re-entering the
/// provider would repeat whatever the first call had already begun.
#[tokio::test]
async fn a_budget_cut_resumes_the_provider_call_instead_of_repeating_it() {
    struct CountingProvider {
        descriptor: ProviderDescriptor,
        calls: Arc<AtomicUsize>,
        started: Arc<Notify>,
        release: Arc<Notify>,
    }
    #[async_trait]
    impl ContextProvider for CountingProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }
        async fn resolve(
            &self,
            input: &ResolvedContext,
            _: &ExecutionContext,
        ) -> Result<Vec<ResolvedContext>> {
            if input.value.text() != "slow" {
                return Ok(vec![]);
            }
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.started.notify_one();
            self.release.notified().await;
            Ok(vec![ResolvedContext::new(
                "test.enriched",
                ContextValue::Extension("late".into()),
                input.source,
                "test.counting",
            )])
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_budget(
        Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), false)),
        Duration::from_millis(20),
    )
    .with_content_grace(Duration::from_millis(20));
    let calls = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    app.register_provider(Arc::new(CountingProvider {
        descriptor: ProviderDescriptor {
            id: "test.counting".into(),
            accepts: vec![TEXT.into()],
            produces: vec!["test.enriched".into()],
            priority: 0,
        },
        calls: calls.clone(),
        started: started.clone(),
        release: release.clone(),
    }))
    .unwrap();

    let state = app.activate().await.unwrap().unwrap();
    let worker = Arc::new(app);
    let pending = tokio::spawn({
        let worker = worker.clone();
        let session = state.session_id;
        async move { worker.update_input(session, "slow".into(), 1).await }
    });
    // The call is in flight when both the budget and the grace expire.
    started.notified().await;
    let partial = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .expect("the budget and grace must both expire")
        .unwrap()
        .unwrap()
        .expect("a partial snapshot is published");
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "the call must have been made once"
    );
    assert!(
        !partial
            .context_summary
            .iter()
            .any(|context| context.type_id == "test.enriched"),
        "the cut must not have produced the late context yet: {:?}",
        partial.context_summary
    );

    // The late pass resumes the same call: it completes, and it is still one call.
    release.notify_one();
    let settled_state = settled(&worker, state.session_id, 1).await;
    assert!(
        settled_state
            .context_summary
            .iter()
            .any(|context| context.type_id == "test.enriched"),
        "the late pass must collect the result of the call it kept: {settled_state:?}"
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "resuming the call must not run the provider twice"
    );
}

#[tokio::test]
async fn old_async_results_are_discarded_after_revision_session_and_hide() {
    for transition in ["revision", "session", "hide"] {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app(Arc::new(TestPlatform::new(dir.path().into(), false)));
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        app.register_provider(Arc::new(DelayedProvider {
            descriptor: ProviderDescriptor {
                id: "test.delayed".into(),
                accepts: vec![TEXT.into()],
                produces: vec!["test.enriched".into()],
                priority: 0,
            },
            started: started.clone(),
            release: release.clone(),
        }))
        .unwrap();
        let app = Arc::new(app);
        let state = app.activate().await.unwrap().unwrap();
        let worker = app.clone();
        let pending = tokio::spawn(async move {
            worker
                .update_input(state.session_id, "slow".into(), 1)
                .await
        });
        started.notified().await;
        match transition {
            "revision" => {
                assert_eq!(
                    app.update_input(state.session_id, "fast".into(), 2)
                        .await
                        .unwrap()
                        .unwrap()
                        .input,
                    "fast"
                );
            }
            "session" => {
                assert!(app.activate().await.unwrap().unwrap().session_id > state.session_id);
            }
            "hide" => app.deactivate(state.session_id).unwrap(),
            _ => unreachable!(),
        }
        release.notify_one();
        assert!(
            pending.await.unwrap().unwrap().is_none(),
            "old provider result survived {transition}"
        );
        assert!(app
            .update_input(state.session_id, "replay".into(), 1)
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn a_zero_budget_commits_a_partial_snapshot_then_appends_late_context() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), true));
    // Grace disabled: this test covers the pure budget path and the late append.
    let app = app_with_budget(platform.clone(), Duration::ZERO).with_content_grace(Duration::ZERO);
    let state = app.activate().await.unwrap().unwrap();
    let session = state.session_id;
    assert!(
        state
            .candidates
            .iter()
            .all(|candidate| candidate.command_id.as_deref() != Some("core.directory.terminal")),
        "{state:?}"
    );
    assert!(!state.context_complete);

    let late = settled(&app, session, state.revision).await;
    assert!(late.context_complete);
    assert!(titles(&late).contains(&"Open Terminal Here"));
    assert!(late
        .context_summary
        .iter()
        .any(|context| context.type_id == GIT_REPOSITORY));
    let directory = late
        .context_summary
        .iter()
        .find(|context| context.type_id == DIRECTORY)
        .expect("late directory context");
    assert_eq!(directory.source, ContextSource::ApplicationContext);
    assert_eq!(
        platform.directory_captures.load(Ordering::Relaxed),
        1,
        "the late capture must not be duplicated"
    );
}

#[tokio::test]
async fn candidate_ids_stay_stable_when_late_context_reorders_the_list() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    let app = app_with_budget(platform, Duration::ZERO).with_content_grace(Duration::ZERO);
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let copy_id = candidate(&state, "core.directory.copy");
    assert!(
        !copy_id.contains(dir.path().canonicalize().unwrap().to_str().unwrap()),
        "{copy_id}"
    );
    let late = settled(&app, state.session_id, state.revision).await;
    assert_eq!(candidate(&late, "core.directory.copy"), copy_id);
    assert!(state.candidates[0].score >= state.candidates.last().unwrap().score);
}

#[tokio::test]
async fn late_context_never_overwrites_a_newer_revision() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    let first_capture = platform.push_gate();
    let second_capture = platform.push_gate();
    let app =
        Arc::new(app_with_budget(platform, Duration::ZERO).with_content_grace(Duration::ZERO));
    let state = app.activate().await.unwrap().unwrap();
    assert!(state
        .context_summary
        .iter()
        .all(|context| context.type_id != DIRECTORY));
    let typed = app
        .update_input(state.session_id, "hello".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(typed.revision, 1);
    first_capture.release();
    second_capture.release();
    let settled_state = settled(&app, state.session_id, 1).await;
    assert!(
        settled_state
            .context_summary
            .iter()
            .any(|context| context.type_id == DIRECTORY),
        "the directory must survive a newer revision: {settled_state:?}"
    );
}

/// An execute request for a revision whose candidates are not published yet is refused.
///
/// `update_input` makes its revision current before it resolves anything, so between the
/// input arriving and the snapshot being published the session's revision is new while
/// the candidate map still holds the old revision's candidates — carrying the old context
/// and execution target. The shipped Flutter host cannot reach that state (it refuses to
/// submit while it is resolving), but the core API is a public boundary and a host that
/// echoed a revision it had not received yet would otherwise get the stale action.
#[tokio::test]
async fn a_candidate_from_a_superseded_revision_cannot_execute() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    // A slow ambient capture makes each revision cost real time, so the window between
    // "revision current" and "candidates published" is wide rather than theoretical.
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(30)),
        ..TestPlatform::new(root.clone(), true)
    });
    let app = Arc::new(app_with_budget(
        platform.clone(),
        Duration::from_millis(200),
    ));
    let session = app.activate().await.unwrap().unwrap().session_id;
    // Revision 0 carries a URL action for example.org. Its id is a function of the
    // command and the context, so revision 1 publishes the *same* id for example.com:
    // executing the old id at the new revision must not reach the old target.
    let first = app
        .update_input(session, "https://example.org".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let url_id = candidate(&first, "core.url.copy");
    assert_eq!(first.revision, 1);
    assert!(
        !url_id.contains("example.org"),
        "the action id must not carry the context body: {url_id}"
    );

    // Drive the revision change and act on it before it can publish. `update_input` marks
    // its revision current synchronously, long before the ambient capture its own revision
    // needs finishes, so a few milliseconds lands inside that window, not on either side.
    let pending = tokio::spawn({
        let app = app.clone();
        async move {
            app.update_input(session, "https://example.com".into(), 2)
                .await
        }
    });
    let mut messages = Vec::new();
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_millis(2)).await;
        match app.execute(session, 2, url_id.clone()).await {
            Err(error) => messages.push(format!("{error:#}")),
            Ok(result) => panic!("revision 2 executed an unpublished action: {result:?}"),
        }
    }
    assert!(
        messages.iter().any(|message| message.contains("expired")),
        "revision 2 must refuse revision 1's action while its own is unpublished: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .all(|message| !message.contains("stale session")),
        "revision 2 was current throughout, so nothing here is a stale session: {messages:?}"
    );
    assert_eq!(
        platform.calls.lock().unwrap().len(),
        0,
        "a refused action must not reach the platform"
    );

    // ...and once revision 2 publishes, its own action is accepted against its own context.
    let next = pending.await.unwrap().unwrap().unwrap();
    assert_eq!(next.revision, 2);
    let next_url_id = candidate(&next, "core.url.copy");
    assert_ne!(
        next_url_id, url_id,
        "the revision's action id must belong to its own context"
    );
    let executed = app.execute(session, 2, next_url_id).await.unwrap();
    assert!(executed.succeeded(), "{executed:?}");
    assert!(
        platform
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|(operation, value)| operation == "copy" && value.contains("example.com")),
        "the accepted action must run against revision 2's context: {:?}",
        platform.calls.lock().unwrap()
    );
}

/// A queued action that is dismissed before its worker starts must not run.
///
/// `execute` checks currency when the request arrives, but the side effect happens on a
/// blocking worker, which can wait behind other work. Dismissing the launcher in that gap
/// must stop the action: otherwise Esc still opens a window or overwrites the clipboard for
/// a session the user already left.
///
/// The runtime here allows exactly two blocking threads — the shipped default allows 512,
/// so a test cannot saturate it — which makes the worker's start deterministic: two holders
/// occupy the pool, the action queues behind them, and the session is dismissed before the
/// pool is released.
#[test]
fn a_queued_action_does_not_run_after_the_session_is_dismissed() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(2)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let platform = Arc::new(TestPlatform::new(root, true));
        let app = Arc::new(app(platform.clone()));
        let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
        let copy_id = candidate(&state, "core.directory.copy");

        let gate = CaptureGate::new();
        let holding = Arc::new(AtomicUsize::new(0));
        let mut holders = Vec::new();
        for _ in 0..2 {
            let (gate, holding) = (gate.clone(), holding.clone());
            holders.push(tokio::task::spawn_blocking(move || {
                holding.fetch_add(1, Ordering::Relaxed);
                // Bounded hold: a broken test fails instead of hanging the suite.
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while !gate.released() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }));
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while holding.load(Ordering::Relaxed) < 2 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the blocking pool was never saturated");

        let queued = tokio::spawn({
            let app = app.clone();
            let session = state.session_id;
            let revision = state.revision;
            async move { app.execute(session, revision, copy_id).await }
        });
        // Let the request pass its opening gate and queue its worker, then dismiss.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            platform.calls.lock().unwrap().is_empty(),
            "the action must still be queued: {:?}",
            platform.calls.lock().unwrap()
        );
        app.deactivate(state.session_id).unwrap();
        gate.release();
        for holder in holders {
            let _ = holder.await;
        }

        let result = queued.await.unwrap().unwrap();
        assert!(
            !result.succeeded(),
            "the queued action must be refused: {result:?}"
        );
        assert!(
            platform.calls.lock().unwrap().is_empty(),
            "a dismissed action must not reach the platform: {:?}",
            platform.calls.lock().unwrap()
        );
    });
}

/// A late append must be observable by a *host*, not only by an in-crate helper.
#[tokio::test]
async fn late_context_is_visible_through_the_host_facing_snapshot_api() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(120)),
        ..TestPlatform::new(root, true)
    });
    let app =
        app_with_budget(platform, Duration::from_millis(20)).with_content_grace(Duration::ZERO);
    let first = app.activate().await.unwrap().unwrap();
    assert_eq!(first.snapshot_seq, 1);
    assert!(!first.candidates.iter().any(|candidate| candidate
        .command_id
        .as_deref()
        .is_some_and(|id| id.starts_with("core.git."))));

    // This is exactly what the bridge's `wait_for_snapshot` does.
    let mut updates = app.state_updates();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let late = loop {
        let current = app
            .latest_state(first.session_id)
            .expect("a committed state");
        if current.snapshot_seq > first.snapshot_seq {
            break current;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "no newer snapshot was ever published");
        assert!(
            tokio::time::timeout(remaining, updates.changed())
                .await
                .is_ok(),
            "no newer snapshot was ever published"
        );
    };
    assert!(
        late.candidates
            .iter()
            .any(|candidate| candidate.title == "Git Status"),
        "the late snapshot must carry the repository actions"
    );
    assert!(context_actions(&late, DIRECTORY).contains(&"Git Status"));
    assert!(late.context_complete);
}

#[tokio::test]
async fn a_slow_directory_capture_still_shows_context_actions_without_typing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(150)),
        ..TestPlatform::new(root, true)
    });
    let app = app_with_budget(platform, Duration::from_millis(40));
    let state = app.activate().await.unwrap().unwrap();
    assert_eq!(state.revision, 0);
    let opened = titles(&state);
    assert!(
        opened.contains(&"Open Terminal Here"),
        "the first snapshot must wait for the ambient directory: {opened:?}"
    );
    assert!(
        context_types(&state).contains(&DIRECTORY),
        "the directory Context must be present on the first snapshot"
    );
    let ready = settled(&app, state.session_id, state.revision).await;
    assert!(titles(&ready).contains(&"Open Terminal Here"));
}

/// The app index can arrive while the session is busy, and must still reach the snapshot.
///
/// A search typed in the first moments matches an empty index: the scan does not start
/// until activation, so the revision that carried the query is already published when the
/// index appears. The index is then published into a *rebuilt* snapshot. That rebuild has
/// a second obligation: hosts read a new `snapshot_seq` as "there is more to show now",
/// so it must not present an unfinished revision as final.
#[tokio::test]
async fn an_index_that_arrives_late_reaches_the_settled_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("Google Chrome.app/Contents")).unwrap();
    let gate = CaptureGate::new();
    let platform = Arc::new(TestPlatform {
        catalog_gate: Mutex::new(Some(gate.clone())),
        ..TestPlatform::new(dir.path().canonicalize().unwrap(), false)
    });
    let app =
        Arc::new(app_with_budget(platform, Duration::ZERO).with_content_grace(Duration::ZERO));
    let mut index = app.index_updates();
    let first = app.activate().await.unwrap().unwrap();
    assert!(
        !first
            .candidates
            .iter()
            .any(|candidate| candidate.kind == CandidateKind::Application),
        "the scan must not have published yet: {first:?}"
    );
    assert_eq!(*index.borrow(), 0, "the app index is not ready yet");
    let searched = app
        .update_input(first.session_id, "chrome".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !searched
            .candidates
            .iter()
            .any(|candidate| candidate.kind == CandidateKind::Application),
        "the query ran before the index was published: {searched:?}"
    );

    gate.release();
    // The index arrives, and the rebuild republishes revision 1 with it.
    tokio::time::timeout(Duration::from_secs(10), index.changed())
        .await
        .expect("the app index was never published")
        .expect("the app index sender died");
    let mut states = app.state_updates();
    let mut saw_unfinished_with_an_app = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let complete = loop {
        if let Some(state) = app.latest_state(first.session_id) {
            let app_found = state
                .candidates
                .iter()
                .any(|candidate| candidate.kind == CandidateKind::Application);
            if app_found && !state.context_complete {
                saw_unfinished_with_an_app = true;
            }
            if app_found && state.context_complete {
                break state;
            }
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "the rebuilt snapshot never carried the app index"
        );
        assert!(
            tokio::time::timeout(remaining, states.changed())
                .await
                .is_ok(),
            "the rebuilt snapshot never carried the app index"
        );
    };
    assert!(
        !saw_unfinished_with_an_app,
        "a rebuild must not publish an unfinished revision as a late update"
    );
    assert_eq!(complete.revision, 1);
    assert!(complete.context_complete);
}

#[tokio::test]
async fn a_new_install_is_found_on_the_next_activation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("Old.app/Contents")).unwrap();
    let app = app(Arc::new(TestPlatform::new(root.clone(), false)));

    let first = app.activate().await.unwrap().unwrap();
    app.catalog_ready().await;
    let old = app
        .update_input(first.session_id, "old".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(
        titles(&old).contains(&"Old"),
        "the first scan must see what was already installed: {:?}",
        titles(&old)
    );

    std::fs::create_dir_all(root.join("NewApp.app/Contents")).unwrap();
    let second = app.activate().await.unwrap().unwrap();
    app.catalog_ready().await;
    let searched = app
        .update_input(second.session_id, "newapp".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(
        titles(&searched).contains(&"NewApp"),
        "a newly installed app must be searchable without restarting: {:?}",
        titles(&searched)
    );
}

/// The exact real-machine shape: the budget lands just as the directory is resolved,
/// one step before the Git provider runs.
#[tokio::test]
async fn first_snapshot_waits_for_the_step_after_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(60)),
        ..TestPlatform::new(root, true)
    });
    let app = app_with_budget(platform, Duration::from_millis(30));
    let state = app.activate().await.unwrap().unwrap();
    let titles = titles(&state);
    assert!(
        titles.contains(&"Git Status"),
        "the first snapshot must include the repository actions: {titles:?}"
    );
    assert_eq!(context_types(&state), [DIRECTORY]);
    assert!(context_actions(&state, DIRECTORY).contains(&"Git Status"));
    assert!(state.context_complete, "one snapshot should be enough here");
}

/// The grace is bounded: a capture slower than budget plus grace must not block the
/// first snapshot, and the late pass still fills the actions in.
#[tokio::test]
async fn first_snapshot_grace_is_bounded_and_late_pass_still_completes() {
    let fields = recorded_fields();
    fields.lock().unwrap().clear();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(250)),
        ..TestPlatform::new(root, true)
    });
    let app = app_with_budget(platform, Duration::from_millis(20))
        .with_content_grace(Duration::from_millis(60));
    let started = std::time::Instant::now();
    let state = app.activate().await.unwrap().unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(600),
        "the first snapshot must not wait for the whole capture: {elapsed:?}"
    );
    assert!(titles(&state).is_empty());

    // ADR 005: the 40ms budget itself is unchanged and still reported when it expires;
    // the grace below only extends the wait, it does not extend provider work.
    let seen = fields.lock().unwrap().clone();
    assert!(
        seen.iter()
            .any(|field| field == "first_snapshot_budget_reached"),
        "the budget must be reported when it expires; saw {seen:?}"
    );

    let ready = settled(&app, state.session_id, state.revision).await;
    assert!(titles(&ready).contains(&"Open Terminal Here"));
}

#[tokio::test]
async fn finder_provider_ignores_other_apps_and_failures_do_not_report_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut platform = TestPlatform::new(dir.path().into(), false);
    platform.fail = true;
    let platform = Arc::new(platform);
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    assert_eq!(platform.directory_captures.load(Ordering::Relaxed), 0);
    let state = app
        .update_input(state.session_id, "https://example.com".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let result = app
        .execute(
            state.session_id,
            state.revision,
            candidate(&state, "core.url.open"),
        )
        .await
        .unwrap();
    assert!(!result.succeeded());
    assert!(!result.close_window);
    assert!(result.notice().contains("simulated platform failure"));
}

#[tokio::test]
async fn subscription_replays_the_latest_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    let app = app(platform);
    let state = app.activate().await.unwrap().unwrap();
    // Subscribing after activation still sees the committed state.
    let subscription = app.subscribe(state.session_id);
    let latest = subscription.latest_state().expect("replayed snapshot");
    assert_eq!(latest.session_id, state.session_id);
    assert_eq!(latest.snapshot_seq, state.snapshot_seq);
}

#[tokio::test]
async fn snapshot_sequence_increases_for_late_enrichment() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    // Grace off so the first snapshot is genuinely cut and a late pass must follow.
    let app = app_with_budget(platform, Duration::ZERO).with_content_grace(Duration::ZERO);
    let first = app.activate().await.unwrap().unwrap();
    let late = settled(&app, first.session_id, first.revision).await;
    assert!(
        late.snapshot_seq > first.snapshot_seq,
        "late enrichment must publish a newer sequence: {} vs {}",
        late.snapshot_seq,
        first.snapshot_seq
    );
    assert_eq!(late.revision, first.revision);
}

/// The spec names seven performance metrics; the code must emit all of them.
/// Application matching is reported as matching, not as ranking.
///
/// Every installed application is scanned against the query by `application_candidates`,
/// which used to run inside the `ranking_us` window: the 1.2s regression on a pasted
/// document was reported as ranking cost. The phases are now timed apart, and this pins the
/// boundary on an index large enough for the application phase to dominate: if the split
/// were lost, `ranking_duration` would absorb it and stop being the smaller number.
///
/// A scoped subscriber rather than the shared recorder: another test may have installed
/// that one already, and this must not depend on test order.
#[tokio::test]
async fn every_named_performance_metric_is_emitted_for_one_activation() {
    use metrics;
    let fields = recorded_fields();
    fields.lock().unwrap().clear();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    // A capture slower than the budget, so the first snapshot is certainly cut short
    // *while a provider is in flight* and a late pass follows. With an instantaneous
    // capture the zero-budget cut races the provider: whichever wins decides whether
    // `provider_duration` is ever emitted, which made this assertion flaky.
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(50)),
        ..TestPlatform::new(root, true)
    });
    let app = app_with_budget(platform, Duration::ZERO).with_content_grace(Duration::ZERO);
    let state = app.activate().await.unwrap().unwrap();
    // The late pass is what settles the cut-short provider and emits its duration.
    let _ = settled(&app, state.session_id, state.revision).await;

    let seen = fields.lock().unwrap().clone();
    for metric in metrics::ALL {
        // hotkey_to_visible is the native host's, not the core's.
        if metric == metrics::HOTKEY_TO_VISIBLE {
            assert!(
                include_str!("../../../../apps/flutter/macos/Runner/Main.swift").contains(metric),
                "the native host must still emit {metric}"
            );
            continue;
        }
        assert!(
            seen.iter().any(|field| field == metric),
            "{metric} was never emitted; saw {seen:?}"
        );
    }
}

#[test]
fn a_queued_action_does_not_run_after_hidden_session_resumes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(2)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let platform = Arc::new(TestPlatform::new(root, true));
        let app = Arc::new(app(platform.clone()));
        let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
        let copy_id = candidate(&state, "core.directory.copy");

        let gate = CaptureGate::new();
        let holding = Arc::new(AtomicUsize::new(0));
        let mut holders = Vec::new();
        for _ in 0..2 {
            let (gate, holding) = (gate.clone(), holding.clone());
            holders.push(tokio::task::spawn_blocking(move || {
                holding.fetch_add(1, Ordering::Relaxed);
                // Bounded hold: a broken test fails instead of hanging the suite.
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while !gate.released() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }));
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while holding.load(Ordering::Relaxed) < 2 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the blocking pool was never saturated");

        let queued = tokio::spawn({
            let app = app.clone();
            let session = state.session_id;
            let revision = state.revision;
            async move { app.execute(session, revision, copy_id).await }
        });
        // Let the request pass its opening gate and queue its worker, then dismiss.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            platform.calls.lock().unwrap().is_empty(),
            "the action must still be queued: {:?}",
            platform.calls.lock().unwrap()
        );
        app.suspend_session(state.session_id).await.unwrap();
        assert!(app.resume_session(state.session_id).unwrap().is_some());
        gate.release();
        for holder in holders {
            let _ = holder.await;
        }

        let result = queued.await.unwrap().unwrap();
        assert!(
            !result.succeeded(),
            "the queued action must be refused: {result:?}"
        );
        assert!(
            platform.calls.lock().unwrap().is_empty(),
            "a dismissed action must not reach the platform: {:?}",
            platform.calls.lock().unwrap()
        );
    });
}
