use crate::support::*;
use anywhere_core::application::SnapshotWait;
use std::sync::atomic::AtomicUsize;

struct ContextPlatform {
    application: ApplicationInfo,
    finder_paths: Vec<PathBuf>,
    selected: Mutex<Option<String>>,
    browser_url: Option<String>,
    browser_denied: bool,
    clipboard: Option<String>,
    selected_gate: Option<Arc<CaptureGate>>,
    selected_denied: bool,
    selected_started: Notify,
    selected_reads: AtomicUsize,
    browser_reads: AtomicUsize,
    clipboard_reads: AtomicUsize,
    copies: Mutex<Vec<String>>,
}

impl ContextPlatform {
    fn new(bundle_id: &str) -> Self {
        Self {
            application: ApplicationInfo {
                bundle_id: bundle_id.into(),
                name: bundle_id.into(),
                pid: 42,
                instance_id: Some("captured-instance".into()),
            },
            finder_paths: Vec::new(),
            selected: Mutex::new(None),
            browser_url: None,
            browser_denied: false,
            clipboard: None,
            selected_gate: None,
            selected_denied: false,
            selected_started: Notify::new(),
            selected_reads: AtomicUsize::new(0),
            browser_reads: AtomicUsize::new(0),
            clipboard_reads: AtomicUsize::new(0),
            copies: Mutex::new(Vec::new()),
        }
    }
}

impl Platform for ContextPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(Some(self.application.clone()))
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn finder_selected_paths(&self, _: &ApplicationInfo) -> Result<Vec<PathBuf>> {
        Ok(self.finder_paths.clone())
    }
    fn selected_text(&self, _: &ApplicationInfo) -> Result<Option<String>> {
        self.selected_reads.fetch_add(1, Ordering::SeqCst);
        if self.selected_denied {
            return Err(anywhere_platform::SelectedTextError::PermissionDenied.into());
        }
        let selected = self.selected.lock().unwrap().clone();
        if let Some(gate) = &self.selected_gate {
            self.selected_started.notify_one();
            gate.wait();
        }
        Ok(selected)
    }
    fn browser_current_url(&self, _: &ApplicationInfo) -> Result<Option<String>> {
        self.browser_reads.fetch_add(1, Ordering::SeqCst);
        if self.browser_denied {
            anyhow::bail!("Browser Automation permission denied");
        }
        Ok(self.browser_url.clone())
    }
    fn browser_current_url_supported(&self, application: &ApplicationInfo) -> bool {
        matches!(
            application.bundle_id.as_str(),
            "com.apple.Safari" | "com.google.Chrome" | "com.microsoft.edgemac"
        )
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        self.clipboard_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.clipboard.clone())
    }
    fn open_url(&self, _: &str) -> Result<()> {
        Ok(())
    }
    fn open_file(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn reveal_file(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn open_directory(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn open_terminal(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        self.copies.lock().unwrap().push(text.into());
        Ok(())
    }
}

#[tokio::test]
async fn denied_selected_text_reports_status_without_hiding_other_actions() {
    let mut fake = ContextPlatform::new("test.editor");
    fake.selected_denied = true;
    fake.clipboard = Some("https://example.org/copied".into());
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    let _ = capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, URL).await;
    for _ in 0..200 {
        if let Some(state) = app.latest_state(initial.session_id) {
            if state.context_captures.iter().any(|capture| {
                capture.source == ContextSource::SelectedText
                    && capture.status == ContextCaptureStatus::PermissionDenied
            }) {
                assert!(state.context_summary.iter().any(|context| {
                    context.source == ContextSource::Clipboard && context.type_id == URL
                }));
                assert!(state
                    .candidates
                    .iter()
                    .any(|candidate| { candidate.command_id.as_deref() == Some("core.url.open") }));
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("permission status was not published");
}

#[tokio::test]
async fn disabled_sources_are_not_read_and_typed_input_still_works() {
    let mut fake = ContextPlatform::new("com.apple.Safari");
    *fake.selected.lock().unwrap() = Some("https://example.org/selected".into());
    fake.browser_url = Some("https://example.org/page".into());
    fake.clipboard = Some("https://example.org/copied".into());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    app.set_capture_sources(false, false, false, false);
    let initial = app.activate().await.unwrap().unwrap();
    assert!(initial.context_captures.is_empty());
    let typed = app
        .update_input(initial.session_id, "https://example.org/typed".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(typed
        .context_summary
        .iter()
        .any(|context| { context.source == ContextSource::UserInput && context.type_id == URL }));
    assert_eq!(fake.selected_reads.load(Ordering::SeqCst), 0);
    assert_eq!(fake.browser_reads.load(Ordering::SeqCst), 0);
    assert_eq!(fake.clipboard_reads.load(Ordering::SeqCst), 0);
}

fn application(platform: Arc<ContextPlatform>) -> Application {
    Application::new(
        platform,
        anywhere_core::application::builtin_registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    )
}

async fn capture_state(
    app: &Application,
    session_id: SessionId,
    revision: Revision,
    source: ContextSource,
    type_id: &str,
) -> LauncherState {
    for _ in 0..200 {
        if let Some(state) = app.latest_state(session_id) {
            if state.revision == revision
                && state
                    .context_captures
                    .iter()
                    .any(|capture| capture.source == source)
                && state
                    .context_summary
                    .iter()
                    .any(|context| context.source == source && context.type_id == type_id)
            {
                return state;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("capture {source:?} did not reach revision {revision}");
}

#[tokio::test]
async fn finder_multiselection_keeps_collection_and_uses_one_action() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("one.txt");
    let second = temp.path().join("two,文件.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let mut fake = ContextPlatform::new("com.apple.finder");
    fake.finder_paths = vec![first.clone(), second.clone()];
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    let state = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::FinderSelection,
        FILE_SELECTION,
    )
    .await;
    assert!(state
        .context_summary
        .iter()
        .any(|context| context.type_id == FILE_SELECTION
            && context.source == ContextSource::FinderSelection));
    assert!(
        state
            .context_summary
            .iter()
            .filter(|context| context.type_id == FILE)
            .count()
            == 2
    );
    let selection_actions = state
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.command_id.as_deref() == Some("core.file_selection.copy_paths")
        })
        .collect::<Vec<_>>();
    assert_eq!(selection_actions.len(), 1);
    assert!(!state
        .candidates
        .iter()
        .any(|candidate| candidate.command_id.as_deref() == Some("core.file.open")));
    app.execute(initial.session_id, 0, selection_actions[0].id.clone())
        .await
        .unwrap();
    let copied = fake.copies.lock().unwrap();
    let paths: Vec<String> = serde_json::from_str(&copied[0]).unwrap();
    assert_eq!(
        paths,
        vec![first.to_string_lossy(), second.to_string_lossy()]
    );
}

#[tokio::test]
async fn selected_text_is_resolved_once_and_survives_typing() {
    let fake = ContextPlatform::new("test.editor");
    *fake.selected.lock().unwrap() = Some("https://example.org/selected".into());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    let home = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::SelectedText,
        URL,
    )
    .await;
    assert!(home
        .context_summary
        .iter()
        .any(|context| context.type_id == URL && context.source == ContextSource::SelectedText));
    let revised = app
        .update_input(initial.session_id, "open".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(revised
        .context_summary
        .iter()
        .any(|context| context.type_id == URL && context.source == ContextSource::SelectedText));
    assert_eq!(fake.selected_reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn supported_browser_urls_and_clipboard_have_distinct_sources_across_revisions() {
    for browser in [
        "com.apple.Safari",
        "com.google.Chrome",
        "com.microsoft.edgemac",
    ] {
        let mut fake = ContextPlatform::new(browser);
        fake.browser_url = Some("https://example.org/page".into());
        fake.clipboard = Some(r#"{"item":42}"#.into());
        let fake = Arc::new(fake);
        let app = application(fake.clone());
        let initial = app.activate().await.unwrap().unwrap();
        let _ = capture_state(
            &app,
            initial.session_id,
            0,
            ContextSource::BrowserCurrentPage,
            URL,
        )
        .await;
        let _ = capture_state(
            &app,
            initial.session_id,
            0,
            ContextSource::Clipboard,
            JSON_DOCUMENT,
        )
        .await;
        let revised = app
            .update_input(initial.session_id, "json".into(), 1)
            .await
            .unwrap()
            .unwrap();
        assert!(revised
            .context_summary
            .iter()
            .any(|context| context.type_id == URL
                && context.source == ContextSource::BrowserCurrentPage));
        assert!(revised
            .context_summary
            .iter()
            .any(|context| context.type_id == JSON_DOCUMENT
                && context.source == ContextSource::Clipboard));
        assert_eq!(fake.browser_reads.load(Ordering::SeqCst), 1, "{browser}");
        assert_eq!(fake.clipboard_reads.load(Ordering::SeqCst), 1, "{browser}");
    }
}

#[tokio::test]
async fn unsupported_browser_does_not_attempt_url_capture() {
    let mut fake = ContextPlatform::new("org.mozilla.firefox");
    fake.browser_url = Some("https://example.org/page".into());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    assert!(initial
        .context_captures
        .iter()
        .all(|capture| capture.source != ContextSource::BrowserCurrentPage));
    assert_eq!(fake.browser_reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn browser_automation_denial_does_not_hide_clipboard_context() {
    let mut fake = ContextPlatform::new("com.microsoft.edgemac");
    fake.browser_denied = true;
    fake.clipboard = Some("https://example.org/copied".into());
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    let _ = capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, URL).await;
    for _ in 0..200 {
        if let Some(state) = app.latest_state(initial.session_id) {
            if state.context_captures.iter().any(|capture| {
                capture.source == ContextSource::BrowserCurrentPage
                    && capture.status == ContextCaptureStatus::PermissionDenied
            }) {
                assert!(state.context_summary.iter().any(|context| {
                    context.source == ContextSource::Clipboard && context.type_id == URL
                }));
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("browser permission status was not published");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_selected_text_joins_current_revision_and_old_session_is_discarded() {
    let gate = CaptureGate::new();
    let mut fake = ContextPlatform::new("test.editor");
    *fake.selected.lock().unwrap() = Some("https://example.org/late".into());
    fake.selected_gate = Some(gate.clone());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let first = app.activate().await.unwrap().unwrap();
    fake.selected_started.notified().await;
    let revised = app
        .update_input(first.session_id, "open".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(!revised
        .context_summary
        .iter()
        .any(|context| context.source == ContextSource::SelectedText));
    gate.release();
    let enriched = capture_state(&app, first.session_id, 1, ContextSource::SelectedText, URL).await;
    assert!(enriched
        .context_summary
        .iter()
        .any(|context| context.type_id == URL && context.source == ContextSource::SelectedText));

    let gate = CaptureGate::new();
    let mut old = ContextPlatform::new("test.editor");
    *old.selected.lock().unwrap() = Some("https://example.org/stale".into());
    old.selected_gate = Some(gate.clone());
    let old = Arc::new(old);
    let application = application(old.clone());
    let stale = application.activate().await.unwrap().unwrap();
    old.selected_started.notified().await;
    *old.selected.lock().unwrap() = Some("https://example.org/fresh".into());
    let fresh = application.activate().await.unwrap().unwrap();
    gate.release();
    let current = capture_state(
        &application,
        fresh.session_id,
        0,
        ContextSource::SelectedText,
        URL,
    )
    .await;
    assert!(!current
        .context_summary
        .iter()
        .any(|context| context.detail.contains("stale")));
    assert!(matches!(
        application.wait_for_snapshot(stale.session_id, 0).await,
        SnapshotWait::Superseded
    ));
}
