use crate::support::*;

/// A clipboard permission prompt must not hold back frecency.
#[tokio::test]
async fn history_publishes_before_blocked_clipboard() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), true));
    let gate = CaptureGate::new();
    *platform.clipboard_gate.lock().unwrap() = Some(gate.clone());
    *platform.clipboard.lock().unwrap() = Some(root.to_string_lossy().into_owned());

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
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history)),
        vec![],
    );

    let first = app.activate().await.unwrap().unwrap();
    let started = tokio::time::timeout(
        Duration::from_secs(2),
        platform.clipboard_started.notified(),
    )
    .await;
    if started.is_err() {
        gate.release();
    }
    started.expect("clipboard read never started");

    let ranked = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(state) = app.latest_state(first.session_id) {
                if let Some(copy) = state
                    .candidates
                    .iter()
                    .find(|item| item.command_id.as_deref() == Some("core.directory.copy"))
                {
                    if copy.score >= 500.0 {
                        break state;
                    }
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    if ranked.is_err() {
        gate.release();
    }
    let ranked = ranked.expect("history stayed behind the blocked clipboard read");
    assert!(
        !app.supplemental_ready(first.session_id),
        "the clipboard is still blocked, so the supplemental group is unfinished"
    );
    assert!(ranked
        .candidates
        .iter()
        .any(
            |item| item.command_id.as_deref() == Some("core.directory.copy") && item.score >= 500.0
        ));

    gate.release();
    settled(&app, first.session_id, first.revision).await;
}
