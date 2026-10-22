use crate::support::*;
use anywhere_core::application::SnapshotWait;
use anywhere_platform::ContextReadError;
use std::sync::atomic::AtomicUsize;

#[tokio::test]
async fn home_path_inputs_offer_file_and_directory_actions_for_the_expanded_target() {
    let home = anywhere_platform::paths::home_directory().expect("test user's home");
    let fixture = tempfile::tempdir_in(&home).unwrap();
    let file = fixture.path().join("文件 with spaces.txt");
    std::fs::write(&file, "contents").unwrap();
    let platform = Arc::new(ContextPlatform::new("test.editor"));
    let app = application(platform.clone());
    let initial = app.activate().await.unwrap().unwrap();
    let mut revision = 0;
    for mode in [
        InputMode::Content,
        InputMode::Search,
        InputMode::Environment,
    ] {
        for prefix in ["~", "～"] {
            for (target, kind, command) in [
                (fixture.path(), DIRECTORY, "core.directory.copy"),
                (file.as_path(), FILE, "core.file.copy"),
            ] {
                revision += 1;
                let input = format!(
                    "{prefix}/{}",
                    target.strip_prefix(&home).unwrap().to_str().unwrap()
                );
                let state = app
                    .update_context_input(initial.session_id, input.clone(), revision, None, mode)
                    .await
                    .unwrap()
                    .unwrap();
                let state = settled(&app, state.session_id, state.revision).await;
                assert_eq!(state.input, input, "keep the user's input spelling");
                assert!(state
                    .context_summary
                    .iter()
                    .any(|context| context.type_id == kind));
                assert!(app
                    .execute(state.session_id, state.revision, candidate(&state, command))
                    .await
                    .unwrap()
                    .succeeded());
                assert_eq!(
                    platform.copies.lock().unwrap().last().unwrap(),
                    target.canonicalize().unwrap().to_str().unwrap()
                );
            }
        }
    }
}

struct ContextPlatform {
    application: ApplicationInfo,
    finder_paths: Vec<PathBuf>,
    selected: Mutex<Option<String>>,
    browser_url: Option<String>,
    browser_denied: bool,
    clipboard: Option<String>,
    capture_error: Option<(ContextSource, ContextReadError)>,
    selected_gate: Option<Arc<CaptureGate>>,
    selected_denied: bool,
    selected_started: Notify,
    selected_reads: AtomicUsize,
    browser_reads: AtomicUsize,
    clipboard_reads: AtomicUsize,
    copies: Mutex<Vec<String>>,
    opened_urls: Mutex<Vec<String>>,
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
            capture_error: None,
            selected_gate: None,
            selected_denied: false,
            selected_started: Notify::new(),
            selected_reads: AtomicUsize::new(0),
            browser_reads: AtomicUsize::new(0),
            clipboard_reads: AtomicUsize::new(0),
            copies: Mutex::new(Vec::new()),
            opened_urls: Mutex::new(Vec::new()),
        }
    }

    fn check_capture(&self, source: ContextSource) -> Result<()> {
        if let Some((failed_source, error)) = self.capture_error {
            if failed_source == source {
                return Err(error.into());
            }
        }
        Ok(())
    }
}

impl Platform for ContextPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(Some(self.application.clone()))
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn file_selection_supported(&self, application: &ApplicationInfo) -> bool {
        application.bundle_id == "com.apple.finder" || application.bundle_id == "test.file-manager"
    }
    fn finder_selected_paths(&self, _: &ApplicationInfo) -> Result<Vec<PathBuf>> {
        self.check_capture(ContextSource::FinderSelection)?;
        Ok(self.finder_paths.clone())
    }
    fn selected_text(&self, _: &ApplicationInfo) -> Result<Option<String>> {
        self.check_capture(ContextSource::SelectedText)?;
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
        self.check_capture(ContextSource::BrowserCurrentPage)?;
        self.browser_reads.fetch_add(1, Ordering::SeqCst);
        if self.browser_denied {
            return Err(anywhere_platform::ContextReadError::PermissionDenied.into());
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
        self.check_capture(ContextSource::Clipboard)?;
        self.clipboard_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.clipboard.clone())
    }
    fn open_url(&self, url: &str) -> Result<()> {
        self.opened_urls.lock().unwrap().push(url.into());
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
async fn file_selection_uses_adapter_capability_instead_of_finder_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().canonicalize().unwrap();
    let mut fake = ContextPlatform::new("test.file-manager");
    fake.finder_paths = vec![path.clone()];
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    let state = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::FinderSelection,
        DIRECTORY,
    )
    .await;
    let result = app
        .execute(
            state.session_id,
            state.revision,
            candidate(&state, "core.directory.copy"),
        )
        .await
        .unwrap();
    assert!(result.succeeded());
}

#[tokio::test]
async fn capture_failures_preserve_precise_status_and_other_sources() {
    for (source, error, expected) in [
        (
            ContextSource::SelectedText,
            ContextReadError::Unsupported,
            ContextCaptureStatus::Unsupported,
        ),
        (
            ContextSource::SelectedText,
            ContextReadError::TimedOut,
            ContextCaptureStatus::TimedOut,
        ),
        (
            ContextSource::SelectedText,
            ContextReadError::TooLarge,
            ContextCaptureStatus::TooLarge,
        ),
        (
            ContextSource::FinderSelection,
            ContextReadError::TooManyItems,
            ContextCaptureStatus::TooManyItems,
        ),
        (
            ContextSource::BrowserCurrentPage,
            ContextReadError::PermissionDenied,
            ContextCaptureStatus::PermissionDenied,
        ),
        (
            ContextSource::Clipboard,
            ContextReadError::PermissionDenied,
            ContextCaptureStatus::PermissionDenied,
        ),
        (
            ContextSource::Clipboard,
            ContextReadError::Unavailable,
            ContextCaptureStatus::Unavailable,
        ),
    ] {
        let mut fake = ContextPlatform::new(if source == ContextSource::FinderSelection {
            "com.apple.finder"
        } else {
            "com.microsoft.edgemac"
        });
        fake.capture_error = Some((source, error));
        fake.browser_url = Some("https://example.org/page".into());
        fake.clipboard = Some("https://example.org/copied".into());
        let fake = Arc::new(fake);
        let app = application(fake.clone());
        let initial = app.activate().await.unwrap().unwrap();
        let survivor = if source == ContextSource::Clipboard {
            ContextSource::BrowserCurrentPage
        } else {
            ContextSource::Clipboard
        };
        capture_state(&app, initial.session_id, 0, survivor, URL).await;
        let mut checked = false;
        for _ in 0..200 {
            let state = app.latest_state(initial.session_id).unwrap();
            if state
                .context_captures
                .iter()
                .any(|capture| capture.source == source && capture.status == expected)
            {
                assert!(state
                    .context_summary
                    .iter()
                    .all(|context| context.source != source));
                assert!(state
                    .candidates
                    .iter()
                    .any(|offer| offer.command_id.as_deref() == Some("core.url.open")));
                if source == ContextSource::Clipboard {
                    assert!(app
                        .execute(
                            state.session_id,
                            state.revision,
                            candidate(&state, "core.url.copy")
                        )
                        .await
                        .unwrap()
                        .succeeded());
                    assert_eq!(*fake.copies.lock().unwrap(), ["https://example.org/page"]);
                }
                checked = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(checked, "{source:?} never published {expected:?}");
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
async fn copied_finder_selection_hides_redundant_collection_copy() {
    let temp = tempfile::tempdir().unwrap();
    let paths = vec![temp.path().join("one.txt"), temp.path().join("two.txt")];
    for path in &paths {
        std::fs::write(path, "data").unwrap();
    }
    let mut fake = ContextPlatform::new("com.apple.finder");
    fake.finder_paths = paths.clone();
    fake.clipboard = Some(serde_json::to_string(&paths).unwrap());
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::FinderSelection,
        FILE_SELECTION,
    )
    .await;
    capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, TEXT).await;
    let state = app
        .update_input(initial.session_id, "copy".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert!(state
        .candidates
        .iter()
        .chain(&state.context_candidates)
        .all(|offer| offer.command_id.as_deref() != Some("core.file_selection.copy_paths")));
    assert!(state.scopes.iter().all(|scope| scope.id != "file"));
    assert!(state
        .context_summary
        .iter()
        .any(|context| context.type_id == FILE_SELECTION));
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
    let selection = state
        .context_summary
        .iter()
        .find(|context| context.type_id == FILE_SELECTION)
        .unwrap();
    assert!(state
        .context_summary
        .iter()
        .filter(|context| context.type_id == FILE)
        .all(|context| context.root_key == selection.key));
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
async fn finder_multi_directory_git_enrichment_keeps_one_root_across_queries() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let first = root.join("one");
    let second = root.join("two");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let mut fake = ContextPlatform::new("com.apple.finder");
    fake.finder_paths = vec![first, second];
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    let state = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::FinderSelection,
        GIT_REPOSITORY,
    )
    .await;
    let selection = state
        .context_summary
        .iter()
        .find(|context| context.type_id == FILE_SELECTION)
        .unwrap();
    let key = selection.key.clone();
    assert_eq!(state.preferred_context_key.as_deref(), Some(key.as_str()));
    assert!(state
        .context_summary
        .iter()
        .filter(|context| matches!(context.type_id.as_str(), DIRECTORY | GIT_REPOSITORY))
        .all(|context| context.root_key == key));
    assert_eq!(
        state
            .context_summary
            .iter()
            .filter(|context| !context.candidate_ids.is_empty())
            .count(),
        1
    );
    let git = state
        .context_candidates
        .iter()
        .find(|offer| offer.command_id.as_deref() == Some("core.git.status"))
        .unwrap();
    assert_eq!(git.context_keys.as_slice(), std::slice::from_ref(&key));
    assert_eq!(
        git.execution_target_label.as_deref(),
        Some(root.to_string_lossy().as_ref())
    );
    let updated = app
        .update_input(initial.session_id, "copy".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.preferred_context_key.as_deref(), Some(key.as_str()));
    assert!(updated
        .context_candidates
        .iter()
        .any(
            |offer| offer.command_id.as_deref() == Some("core.git.status")
                && offer.context_keys.contains(&key)
        ));
}

#[tokio::test]
async fn canonical_url_offer_keeps_membership_in_both_selected_text_and_page() {
    let mut fake = ContextPlatform::new("com.google.Chrome");
    fake.browser_url = Some("https://example.org/page".into());
    *fake.selected.lock().unwrap() = fake.browser_url.clone();
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    let _ = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::SelectedText,
        URL,
    )
    .await;
    let state = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::BrowserCurrentPage,
        URL,
    )
    .await;
    let selected = state
        .context_summary
        .iter()
        .find(|context| context.source == ContextSource::SelectedText && context.type_id == TEXT)
        .unwrap();
    let page = state
        .context_summary
        .iter()
        .find(|context| context.source == ContextSource::BrowserCurrentPage)
        .unwrap();
    let open = state
        .context_candidates
        .iter()
        .find(|offer| offer.command_id.as_deref() == Some("core.url.open"))
        .unwrap();
    assert!(open.context_keys.contains(&selected.key));
    assert!(open.context_keys.contains(&page.key));
    assert_eq!(
        state.preferred_context_key.as_deref(),
        Some(selected.key.as_str())
    );
}

#[tokio::test]
async fn copied_current_page_hides_copy_url_and_merges_open_across_sources() {
    let mut fake = ContextPlatform::new("com.microsoft.edgemac");
    fake.browser_url = Some("https://example.org/page".into());
    fake.clipboard = Some("  https://EXAMPLE.org:443/page\n".into());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    let _ = capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, URL).await;
    let _ = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::BrowserCurrentPage,
        URL,
    )
    .await;
    let state = app
        .update_scoped_input(
            initial.session_id,
            "".into(),
            1,
            Some("app.com.microsoft.edgemac".into()),
        )
        .await
        .unwrap()
        .unwrap();
    let browser = state
        .context_summary
        .iter()
        .find(|context| {
            context.source == ContextSource::BrowserCurrentPage && context.type_id == URL
        })
        .unwrap();
    let clipboard = state
        .context_summary
        .iter()
        .find(|context| context.source == ContextSource::Clipboard && context.type_id == TEXT)
        .unwrap();
    let rows: Vec<_> = state
        .candidates
        .iter()
        .filter(|offer| offer.command_id.as_deref() == Some("core.url.open"))
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "open must merge identical targets across sources"
    );
    assert!(rows[0].context_keys.contains(&browser.key));
    assert!(rows[0].context_keys.contains(&clipboard.key));
    assert!(state
        .candidates
        .iter()
        .chain(&state.context_candidates)
        .all(|offer| offer.command_id.as_deref() != Some("core.url.copy")));
    assert!(app
        .execute(
            state.session_id,
            state.revision,
            candidate(&state, "core.url.open")
        )
        .await
        .unwrap()
        .succeeded());
    assert_eq!(
        *fake.opened_urls.lock().unwrap(),
        ["https://example.org/page"]
    );
    assert!(fake.copies.lock().unwrap().is_empty());
    for (revision, scope) in [
        (2, Some("app.com.microsoft.edgemac".into())),
        (3, Some("web".into())),
        (4, None),
    ] {
        let searched = app
            .update_scoped_input(initial.session_id, "copy".into(), revision, scope)
            .await
            .unwrap()
            .unwrap();
        assert!(searched
            .candidates
            .iter()
            .chain(&searched.context_candidates)
            .all(|offer| offer.command_id.as_deref() != Some("core.url.copy")));
    }
}

#[tokio::test]
async fn different_current_page_and_clipboard_urls_keep_distinct_execution_targets() {
    let mut fake = ContextPlatform::new("com.microsoft.edgemac");
    fake.browser_url = Some("https://example.org/page".into());
    fake.clipboard = Some("https://example.org/copied?token=private".into());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    let _ = capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, URL).await;
    let _ = capture_state(
        &app,
        initial.session_id,
        0,
        ContextSource::BrowserCurrentPage,
        URL,
    )
    .await;
    let state = app
        .update_scoped_input(
            initial.session_id,
            "".into(),
            1,
            Some("app.com.microsoft.edgemac".into()),
        )
        .await
        .unwrap()
        .unwrap();
    for (command, count) in [("core.url.copy", 1), ("core.url.open", 2)] {
        assert_eq!(
            state
                .candidates
                .iter()
                .filter(|offer| offer.command_id.as_deref() == Some(command))
                .count(),
            count
        );
    }
    let opens: Vec<_> = state
        .candidates
        .iter()
        .filter(|offer| offer.command_id.as_deref() == Some("core.url.open"))
        .collect();
    assert_ne!(opens[0].id, opens[1].id);
    for offer in &opens {
        assert!(app
            .execute(state.session_id, state.revision, offer.id.clone())
            .await
            .unwrap()
            .succeeded());
    }
    assert_eq!(
        *fake.opened_urls.lock().unwrap(),
        [
            "https://example.org/page",
            "https://example.org/copied?token=private"
        ]
    );
    assert!(app
        .execute(
            state.session_id,
            state.revision,
            candidate(&state, "core.url.copy")
        )
        .await
        .unwrap()
        .succeeded());
    assert_eq!(*fake.copies.lock().unwrap(), ["https://example.org/page"]);
    assert!(state.candidates.iter().all(|offer| !offer
        .subtitle
        .as_deref()
        .unwrap_or_default()
        .contains("token=private")));
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
        fake.clipboard = Some("https://example.org/copied".into());
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
        let _ = capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, URL).await;
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
            .any(|context| context.type_id == URL && context.source == ContextSource::Clipboard));
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

#[tokio::test]
async fn recognized_clipboard_url_keeps_actions_but_hides_its_raw_value_in_presentation() {
    let mut fake = ContextPlatform::new("test.editor");
    fake.clipboard = Some("https://example.org/?token=private".into());
    let app = application(Arc::new(fake));
    let initial = app.activate().await.unwrap().unwrap();
    let captured = capture_state(&app, initial.session_id, 0, ContextSource::Clipboard, URL).await;
    assert!(captured.context_summary.iter().any(|context| {
        context.source == ContextSource::Clipboard
            && context.type_id == URL
            && context.detail == "Copied URL"
    }));
    assert!(captured
        .candidates
        .iter()
        .any(|candidate| { candidate.command_id.as_deref() == Some("core.url.open") }));
    assert!(captured.candidates.iter().all(|candidate| {
        !candidate
            .subtitle
            .as_deref()
            .unwrap_or_default()
            .contains("token=private")
    }));
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

#[tokio::test]
async fn content_input_search_and_current_app_use_separate_graphs_without_changing_scopes() {
    let mut fake = ContextPlatform::new("com.microsoft.edgemac");
    fake.browser_url = Some("https://example.org/page".into());
    fake.clipboard = Some("https://example.org/copied".into());
    *fake.selected.lock().unwrap() = Some("https://example.org/selected".into());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    app.wait_action_binding(initial.session_id, 0, "core.url.open")
        .await
        .unwrap();
    for (revision, text) in [
        (1, "https://example.org/selected"),
        (2, "https://example.org/edited"),
    ] {
        let state = app
            .update_context_input(
                initial.session_id,
                text.into(),
                revision,
                None,
                InputMode::Content,
            )
            .await
            .unwrap()
            .unwrap();
        assert!(state
            .context_summary
            .iter()
            .all(|ctx| ctx.source == ContextSource::UserInput));
        let opens: Vec<_> = state
            .candidates
            .iter()
            .filter(|c| c.command_id.as_deref() == Some("core.url.open"))
            .collect();
        assert_eq!(opens.len(), 1);
        assert_eq!(opens[0].execution_target_label.as_deref(), Some(text));
        assert!(app
            .execute(state.session_id, revision, opens[0].id.clone())
            .await
            .unwrap()
            .succeeded());
    }
    assert_eq!(
        *fake.opened_urls.lock().unwrap(),
        ["https://example.org/selected", "https://example.org/edited"]
    );
    let home = app
        .update_context_input(
            initial.session_id,
            "".into(),
            3,
            None,
            InputMode::Environment,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(home.context_summary.iter().all(|c| !matches!(
        c.source,
        ContextSource::Clipboard | ContextSource::SelectedText
    )));
    assert!(home
        .scopes
        .iter()
        .any(|s| s.id == "app.com.microsoft.edgemac" && s.is_current_app));
    assert_eq!(
        home.candidates
            .iter()
            .filter(|c| c.command_id.as_deref() == Some("core.url.open"))
            .count(),
        1
    );
    let empty = app
        .update_context_input(initial.session_id, "".into(), 4, None, InputMode::Search)
        .await
        .unwrap()
        .unwrap();
    assert!(empty.context_summary.is_empty());
    assert!(empty.candidates.is_empty());
    assert!(empty.scopes.is_empty());
    assert_eq!(fake.clipboard_reads.load(Ordering::SeqCst), 1);
    assert_eq!(fake.selected_reads.load(Ordering::SeqCst), 1);
    assert_eq!(fake.browser_reads.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_capture_updates_metadata_without_joining_the_content_graph() {
    let gate = CaptureGate::new();
    let mut fake = ContextPlatform::new("test.editor");
    *fake.selected.lock().unwrap() = Some("https://example.org/late".into());
    fake.selected_gate = Some(gate.clone());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let initial = app.activate().await.unwrap().unwrap();
    fake.selected_started.notified().await;
    let content = app
        .update_context_input(
            initial.session_id,
            "https://example.org/input".into(),
            1,
            None,
            InputMode::Content,
        )
        .await
        .unwrap()
        .unwrap();
    let seq = content.snapshot_seq;
    gate.release();
    for _ in 0..200 {
        let state = app.latest_state(initial.session_id).unwrap();
        if state.snapshot_seq > seq
            && state.context_captures.iter().any(|c| {
                c.source == ContextSource::SelectedText
                    && c.text.as_deref() == Some("https://example.org/late")
            })
        {
            assert!(state
                .context_summary
                .iter()
                .all(|ctx| ctx.source == ContextSource::UserInput));
            assert!(state
                .candidates
                .iter()
                .filter(|c| c.command_id.as_deref() == Some("core.url.open"))
                .all(|c| c.execution_target_label.as_deref() == Some("https://example.org/input")));
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("late source metadata was not published");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_activation_never_exposes_captured_text_actions() {
    let gate = CaptureGate::new();
    let mut fake = ContextPlatform::new("com.microsoft.edgemac");
    fake.browser_url = Some("https://example.org/page".into());
    fake.clipboard = Some("https://example.org/copied".into());
    *fake.selected.lock().unwrap() = Some("https://example.org/selected".into());
    fake.selected_gate = Some(gate.clone());
    let fake = Arc::new(fake);
    let app = application(fake.clone());
    let session = app.begin_activation().unwrap();
    let initial = app
        .activate_session_in_mode(session, false, InputMode::Environment)
        .await
        .unwrap()
        .unwrap();
    fake.selected_started.notified().await;
    assert!(initial.context_summary.iter().all(|c| !matches!(
        c.source,
        ContextSource::Clipboard | ContextSource::SelectedText
    )));
    gate.release();
    for _ in 0..200 {
        let state = app.latest_state(session).unwrap();
        assert!(state.context_summary.iter().all(|c| !matches!(
            c.source,
            ContextSource::Clipboard | ContextSource::SelectedText
        )));
        if state.context_complete
            && state.context_captures.iter().any(|c| {
                c.source == ContextSource::SelectedText
                    && c.text.as_deref() == Some("https://example.org/selected")
            })
            && state
                .context_summary
                .iter()
                .any(|c| c.source == ContextSource::BrowserCurrentPage && c.type_id == URL)
        {
            let targets: Vec<_> = state
                .candidates
                .iter()
                .filter(|c| c.command_id.as_deref() == Some("core.url.open"))
                .map(|c| c.execution_target_label.as_deref())
                .collect();
            assert_eq!(targets, [Some("https://example.org/page")]);
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("environment activation did not finish");
}
