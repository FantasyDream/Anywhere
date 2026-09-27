use crate::support::*;

#[tokio::test]
async fn history_load_errors_remain_visible_without_preventing_activation() {
    struct BrokenHistory;
    impl UsageRepository for BrokenHistory {
        fn load(&self, _: &str) -> Result<anywhere_core::core::ranking::usage::UsageSnapshot> {
            bail!("read unavailable")
        }
        fn load_global(&self) -> Result<anywhere_core::core::ranking::usage::UsageSnapshot> {
            bail!("global read unavailable")
        }
        fn record(&mut self, _: &str, _: &str, _: &str, _: i64) -> Result<()> {
            Ok(())
        }
    }
    let application = Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::new(BrokenHistory),
        vec![],
    );
    let state = settled(
        &application,
        application.activate().await.unwrap().unwrap().session_id,
        0,
    )
    .await;
    assert!(state
        .diagnostics
        .iter()
        .any(|message| message.contains("read unavailable")));
    assert!(state
        .diagnostics
        .iter()
        .any(|message| message.contains("global read unavailable")));
}
/// A single-thread runtime must remain able to cancel while storage is blocked.
#[tokio::test]
async fn slow_history_does_not_block_async_cancellation() {
    use anywhere_core::core::ranking::usage::UsageSnapshot;
    struct SlowHistory {
        gate: Arc<CaptureGate>,
        started: Arc<Notify>,
        entered: std::sync::mpsc::Sender<()>,
    }
    impl UsageRepository for SlowHistory {
        fn load(&self, _: &str) -> Result<UsageSnapshot> {
            self.entered.send(()).unwrap();
            self.started.notify_one();
            self.gate.wait();
            Ok(UsageSnapshot::new())
        }
        fn load_global(&self) -> Result<UsageSnapshot> {
            Ok(UsageSnapshot::new())
        }
        fn record(&mut self, _: &str, _: &str, _: &str, _: i64) -> Result<()> {
            Ok(())
        }
    }
    let gate = CaptureGate::new();
    let started = Arc::new(Notify::new());
    let (entered, enter_rx) = std::sync::mpsc::channel();
    let (cancelled, cancel_rx) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn({
        let gate = gate.clone();
        move || {
            let entered = enter_rx.recv_timeout(Duration::from_secs(5)).is_ok();
            let responsive = entered && cancel_rx.recv_timeout(Duration::from_secs(1)).is_ok();
            gate.release();
            responsive
        }
    });
    let application = Arc::new(Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::new(SlowHistory {
            gate,
            started: started.clone(),
            entered,
        }),
        vec![],
    ));
    let session = application.begin_activation().unwrap();
    let capture = tokio::spawn({
        let application = application.clone();
        async move { application.activate_session(session).await }
    });
    started.notified().await;
    application.deactivate(session).unwrap();
    let _ = cancelled.send(());
    let responsive = watchdog.join().unwrap();
    capture.await.unwrap().unwrap();
    assert!(
        responsive,
        "history IO prevented the async worker from cancelling"
    );
}

/// A new first-party action needs a bundle and handler, not a central dispatch arm.
#[tokio::test]
async fn launching_an_application_records_usage_under_the_ranked_id() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("Applications");
    for name in ["Google Chrome", "Notes"] {
        std::fs::create_dir_all(home.join(format!("{name}.app/Contents"))).unwrap();
    }
    let platform = Arc::new(TestPlatform::new(home, false));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform,
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let state = app.activate().await.unwrap().unwrap();
    app.catalog_ready().await;
    let searched = app
        .update_input(state.session_id, "chrome".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(searched.candidates[0].title, "Google Chrome");
    assert!(app
        .execute(
            state.session_id,
            searched.revision,
            searched.candidates[0].id.clone()
        )
        .await
        .unwrap()
        .succeeded());
    app.deactivate(state.session_id).unwrap();

    let usage = history.lock().unwrap();
    let recorded = usage.0.keys().cloned().collect::<Vec<_>>();
    // `command_id` must be the id the matcher reads back; `source_app` stays the
    // foreground application, which is what scopes the usage bucket.
    assert_eq!(
        recorded,
        vec![(
            anywhere_core::core::ranking::usage::application_usage_id("app.googlechrome.com"),
            anywhere_core::core::ranking::usage::APPLICATION_CONTEXT_TYPE.to_owned(),
            "test.editor".to_owned(),
        )],
        "usage must be recorded under the id the matcher reads"
    );
}

/// Reported twice: actions used many times never rise. History is keyed by the
/// foreground application, so activation must look it up in that bucket — not in the
/// empty one, which is why every usage term silently read as zero.
#[tokio::test]
async fn activation_loads_history_scoped_to_the_foreground_application() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    for _ in 0..12 {
        history
            .lock()
            .unwrap()
            .record(
                "core.directory.copy",
                DIRECTORY,
                "com.apple.finder",
                1789445000,
            )
            .unwrap();
    }
    let app = Application::new(
        platform,
        registry(),
        Box::new(SharedHistory(history)),
        vec![],
    );
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    let top = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("core.directory.copy"))
        .expect("the directory actions need an ambient context");
    assert!(
        top.score >= 500.0,
        "usage never reached ranking: {} scored {}",
        top.title,
        top.score
    );
}

/// End-to-end against a real SQLite history, mirroring the reported machine: a
/// Finder-scoped repository directory where `Copy Path` was used many times.
#[tokio::test]
async fn real_sqlite_history_lifts_a_repeatedly_used_action_to_the_top() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let database = dir.path().join("usage.sqlite3");
    let now = 1789445000i64;
    {
        let mut history = SqliteUsageRepository::open(&database).unwrap();
        for (command, count) in [
            ("core.directory.copy", 8u32),
            ("core.directory.terminal", 5),
            ("core.directory.open", 4),
            ("core.git.status", 1),
        ] {
            for _ in 0..count {
                history
                    .record(command, DIRECTORY, "com.apple.finder", now)
                    .unwrap();
            }
        }
    }
    let platform = Arc::new(TestPlatform::new(root, true));
    let app = Application::new(
        platform,
        registry(),
        Box::new(SqliteUsageRepository::open(&database).unwrap()),
        vec![],
    );
    let state = app.activate().await.unwrap().unwrap();
    let state = settled(&app, state.session_id, state.revision).await;
    let ranked = state
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.command_id.clone().unwrap_or_default(),
                candidate.score,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ranked.first().map(|entry| entry.0.as_str()),
        Some("core.directory.copy"),
        "8 uses must outrank a one-use higher-priority default: {ranked:?}"
    );
}

/// An action that closes the window still counts as used.
///
/// The host dismisses the launcher when the window loses focus, so a successful action
/// ends its own session. Recording usage after the side effect must not be conditional on
/// the session still being current, or the actions that work best would never rank up.
#[tokio::test]
async fn usage_is_recorded_even_when_the_action_ends_its_own_session() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Arc::new(Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    ));
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let copy_id = candidate(&state, "core.directory.copy");

    // The action ends its own session: the host hides the launcher while it is in flight.
    let action = tokio::spawn({
        let app = app.clone();
        let session = state.session_id;
        let revision = state.revision;
        async move { app.execute(session, revision, copy_id).await }
    });
    // Wait for the side effect, not for a delay: the session is dismissed while the action
    // is past its currency gate and still in flight.
    tokio::time::timeout(Duration::from_secs(5), async {
        while platform.calls.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("the action never reached the platform");
    app.deactivate(state.session_id).unwrap();
    let result = action.await.unwrap().unwrap();
    assert!(result.succeeded(), "{result:?}");

    let recorded = history.lock().unwrap().load_global().unwrap();
    let key = ("core.directory.copy".to_owned(), DIRECTORY.to_owned());
    let stats = recorded
        .get(&key)
        .unwrap_or_else(|| panic!("no usage row for {key:?}: {recorded:?}"));
    assert_eq!(stats.execute_count, 1);
}

#[tokio::test]
async fn successful_action_with_history_failure_reports_a_warning() {
    struct UnwritableHistory;
    impl anywhere_core::core::ranking::usage::UsageRepository for UnwritableHistory {
        fn load(&self, _: &str) -> Result<anywhere_core::core::ranking::usage::UsageSnapshot> {
            Ok(Default::default())
        }
        fn load_global(&self) -> Result<anywhere_core::core::ranking::usage::UsageSnapshot> {
            Ok(Default::default())
        }
        fn record(&mut self, _: &str, _: &str, _: &str, _: i64) -> Result<()> {
            bail!("disk full")
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().into(), false));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(UnwritableHistory),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let state = app
        .update_input(session, "https://github.com".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let result = app
        .execute(session, state.revision, candidate(&state, "core.url.copy"))
        .await
        .unwrap();
    assert!(result.succeeded());
    assert!(!result.close_window);
    assert!(result.notice().contains("disk full"));
    assert_eq!(platform.calls.lock().unwrap().len(), 1);
}
