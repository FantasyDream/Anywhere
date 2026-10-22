use crate::support::*;
use anywhere_core::core::search::candidate::Candidate;
use anywhere_core::core::search::candidate_source::AsyncCandidateSource;
use anywhere_core::features::filesystem::search::file_candidates;
use anywhere_platform::FileSearchHit;
use std::time::Instant;

struct Files {
    hits: Vec<FileSearchHit>,
    calls: AtomicUsize,
    started: Notify,
    release: Notify,
    fail: bool,
}
#[async_trait]
impl AsyncCandidateSource for Files {
    fn accepts_mode(&self, mode: InputMode) -> bool {
        mode == InputMode::FileSearch
    }
    fn id(&self) -> &'static str {
        "files"
    }
    fn accepts(&self, query: &str) -> bool {
        anywhere_core::features::filesystem::search::eligible_query(query)
    }
    async fn candidates(&self, query: &str, _: ExecutionContext) -> Result<Vec<Candidate>> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.started.notify_one();
        if query == "old" {
            self.release.notified().await;
        }
        if self.fail {
            bail!("fixture index unavailable");
        }
        Ok(file_candidates(self.hits.clone(), query, &registry()))
    }
}
fn source(hits: Vec<FileSearchHit>, fail: bool) -> Arc<Files> {
    Arc::new(Files {
        hits,
        calls: AtomicUsize::new(0),
        started: Notify::new(),
        release: Notify::new(),
        fail,
    })
}

struct StreamingFiles {
    initial: Vec<FileSearchHit>,
    later: FileSearchHit,
    release: Notify,
    fail_after_batch: bool,
}
#[async_trait]
impl AsyncCandidateSource for StreamingFiles {
    fn accepts_mode(&self, mode: InputMode) -> bool {
        mode == InputMode::FileSearch
    }
    fn id(&self) -> &'static str {
        "files"
    }
    fn accepts(&self, text: &str) -> bool {
        text == "report"
    }
    async fn candidates(&self, _: &str, _: ExecutionContext) -> Result<Vec<Candidate>> {
        bail!("fixture uses stream")
    }
    async fn stream(
        &self,
        request: anywhere_core::core::search::candidate_source::AsyncSearchRequest,
        _: ExecutionContext,
        sender: tokio::sync::mpsc::Sender<
            anywhere_core::core::search::candidate_source::AsyncCandidateBatch,
        >,
    ) -> Result<()> {
        let batch = |hits: Vec<FileSearchHit>, finished| {
            anywhere_core::core::search::candidate_source::AsyncCandidateBatch {
                update: anywhere_core::core::search::candidate_source::BatchUpdate::Replace,
                metadata: Some(Arc::new(FileSearchProgress {
                    query_id: 77,
                    scanned_entries: if finished { 200 } else { 100 },
                    confirmed_matches: hits.len() as u64,
                    finished,
                    ..Default::default()
                })),
                candidates: file_candidates(hits, &request.text, &registry()),
                state: if finished {
                    SearchSourceState::Ready
                } else {
                    SearchSourceState::Loading
                },
            }
        };
        sender.send(batch(self.initial.clone(), false)).await?;
        self.release.notified().await;
        let mut hits = vec![self.later.clone()];
        hits.extend(self.initial.clone());
        sender.send(batch(hits, !self.fail_after_batch)).await?;
        if self.fail_after_batch {
            bail!("fixture storage failure after a published batch");
        }
        Ok(())
    }
}

#[tokio::test]
async fn streaming_completion_and_failure_preserve_the_frozen_selection_and_report_coverage() {
    for (fail, freeze) in [(false, false), (false, true), (true, false), (true, true)] {
        let temp = tempfile::tempdir().unwrap();
        let make = |name: &str| {
            let path = temp.path().join(name);
            std::fs::write(&path, "").unwrap();
            FileSearchHit {
                path: path.canonicalize().unwrap(),
                is_directory: false,
            }
        };
        let source = Arc::new(StreamingFiles {
            initial: vec![make("report-z.txt"), make("report-y.txt")],
            later: make("report-a.txt"),
            release: Notify::new(),
            fail_after_batch: fail,
        });
        let mut app = app(Arc::new(TestPlatform::default()));
        app.register_async_candidate_source(source.clone()).unwrap();
        let id = app.activate().await.unwrap().unwrap().session_id;
        app.update_file_search_input(id, "report".into(), 1, FileSearchFilter::All)
            .await
            .unwrap();
        let initial = files_ready(&app, id, 1).await;
        if freeze {
            app.freeze_file_results(id, 1).unwrap();
        }
        source.release.notify_one();
        let final_state = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let state = app.latest_state(id).unwrap();
                if state.file_search.as_ref().is_some_and(|p| p.finished) {
                    break state;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            initial.candidates.iter().map(|c| &c.id).collect::<Vec<_>>(),
            final_state
                .candidates
                .iter()
                .take(initial.candidates.len())
                .map(|c| &c.id)
                .collect::<Vec<_>>()
        );
        let progress = final_state.file_search.as_ref().unwrap();
        assert_eq!(progress.confirmed_matches, 3);
        assert_eq!(final_state.candidates.len(), if freeze { 2 } else { 3 });
        assert_eq!(
            final_state.candidate_list_version,
            initial.candidate_list_version
        );
        if freeze {
            assert!(progress.needs_refresh);
        }
        assert_eq!(
            final_state.search_sources[0].state,
            if fail {
                SearchSourceState::Partial
            } else {
                SearchSourceState::Ready
            }
        );
        assert_eq!(progress.issue_count, u64::from(fail));
        app.deactivate(id).unwrap();
    }
}
async fn files_ready(app: &Application, id: SessionId, revision: Revision) -> LauncherState {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(state) = app.latest_state(id) {
                if state.revision == revision
                    && state
                        .candidates
                        .iter()
                        .any(|c| c.kind == CandidateKind::File)
                {
                    return state;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn file_search_entry_is_a_real_action_without_context_or_scope_and_supports_bindings() {
    let platform = Arc::new(TestPlatform::default());
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let id = app.activate().await.unwrap().unwrap().session_id;
    let actions = app.shortcut_actions().unwrap();
    let action = actions
        .iter()
        .find(|a| a.action_id == "core.files.search")
        .unwrap();
    assert_eq!(action.context_types, ["core.launcher"]);
    let ids = std::slice::from_ref(&action.action_id);
    assert_eq!(app.supported_shortcut_actions(ids).unwrap(), ids);
    app.update_context_input(id, "".into(), 1, None, InputMode::Search)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    assert!(state.context_summary.is_empty());
    assert!(state.scopes.is_empty());
    assert!(!state
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.files.search")));
    let binding = app
        .resolve_action_binding(id, 1, "core.files.search")
        .unwrap();
    assert_eq!(
        binding.len(),
        1,
        "hidden entry still supports Action shortcuts"
    );
    assert_eq!(
        app.execute(id, 1, binding[0].id.clone())
            .await
            .unwrap()
            .view,
        Some(HostView::FileSearch)
    );
    app.update_context_input(id, "文件搜索".into(), 2, None, InputMode::Search)
        .await
        .unwrap();
    let state = settled(&app, id, 2).await;
    let entry = state
        .candidates
        .iter()
        .find(|c| c.command_id.as_deref() == Some("core.files.search"))
        .unwrap();
    assert!(state
        .context_summary
        .iter()
        .all(|c| c.type_id != "core.launcher"));
    assert!(state.scopes.iter().all(|s| s.id != "tools"));
    assert!(!entry.home_eligible);
    let output = app.execute(id, 2, entry.id.clone()).await.unwrap();
    assert_eq!(output.view, Some(HostView::FileSearch));
    assert!(!output.close_window && output.output_text.is_none());
    assert!(platform.calls.lock().unwrap().is_empty());
    assert_eq!(
        history.lock().unwrap().load("test.editor").unwrap()
            [&("core.files.search".into(), "core.launcher".into())]
            .execute_count,
        2
    );
    app.update_context_input(
        id,
        "unrelated".into(),
        3,
        Some("folder".into()),
        InputMode::Environment,
    )
    .await
    .unwrap();
    let state = settled(&app, id, 3).await;
    assert!(!state
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.files.search")));
    let binding = app
        .resolve_action_binding(id, 3, "core.files.search")
        .unwrap();
    assert_eq!(binding.len(), 1);
    assert_eq!(
        app.execute(id, 3, binding[0].id.clone())
            .await
            .unwrap()
            .view,
        Some(HostView::FileSearch)
    );
}

#[tokio::test]
async fn workspace_entry_requires_a_matching_name_or_explicit_alias() {
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        registry(),
        Box::new(MemoryUsage::default()),
        vec![],
    );
    let id = app.activate().await.unwrap().unwrap().session_id;
    let cases = [
        ("File Search", true),
        ("files", true),
        ("文件搜索", true),
        ("搜索文件", true),
        ("find", false),
        ("folder", false),
        ("spotlight", false),
        ("settings", false),
        ("files unrelated", false),
        ("文件搜索 不存在", false),
    ];
    for (index, (query, expected)) in cases.iter().enumerate() {
        let revision = index as Revision + 1;
        app.update_context_input(id, (*query).into(), revision, None, InputMode::Search)
            .await
            .unwrap();
        let state = settled(&app, id, revision).await;
        assert_eq!(
            state
                .candidates
                .iter()
                .any(|c| c.command_id.as_deref() == Some("core.files.search")),
            *expected,
            "query: {query}"
        );
        assert_eq!(
            app.resolve_action_binding(id, revision, "core.files.search")
                .unwrap()
                .len(),
            1,
            "visibility must not revoke the shortcut offer"
        );
    }
}

#[tokio::test]
async fn preview_requires_a_current_file_row_and_never_executes_or_records_usage() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("report.txt");
    std::fs::write(&path, "hello").unwrap();
    let platform = Arc::new(TestPlatform::default());
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let mut app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    app.register_async_candidate_source(source(
        vec![FileSearchHit {
            path: path.clone(),
            is_directory: false,
        }],
        false,
    ))
    .unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_context_input(id, "report".into(), 1, None, InputMode::FileSearch)
        .await
        .unwrap();
    let state = files_ready(&app, id, 1).await;
    assert!(
        state.context_summary.is_empty()
            && state.context_candidates.is_empty()
            && state.scopes.is_empty()
    );
    assert!(state
        .candidates
        .iter()
        .all(|c| c.kind == CandidateKind::File));
    let row = state.candidates[0].id.clone();
    // The Open operation shares the business action, but must not overwrite the
    // object row in the execution table. Reopening Actions still needs that row.
    for _ in 0..2 {
        let operations = app.file_actions(id, 1, row.clone()).unwrap();
        assert!(!operations.is_empty());
        assert!(operations
            .iter()
            .all(|a| a.kind == CandidateKind::Action && a.id != row));
        let current = app.latest_state(id).unwrap();
        assert_eq!(current.candidate(&row).unwrap().kind, CandidateKind::File);
        assert_eq!(
            app.preview_file(id, 1, row.clone())
                .await
                .unwrap()
                .text
                .as_deref(),
            Some("hello")
        );
    }
    let preview = app.preview_file(id, 1, row.clone()).await.unwrap();
    assert_eq!(preview.text.as_deref(), Some("hello"));
    assert_eq!(preview.path, path.to_string_lossy());
    assert!(app.preview_file(id, 1, "forged-file".into()).await.is_err());
    assert!(platform.calls.lock().unwrap().is_empty());
    assert!(history
        .lock()
        .unwrap()
        .load("test.editor")
        .unwrap()
        .is_empty());
    std::fs::remove_file(path).unwrap();
    assert!(app.preview_file(id, 1, row.clone()).await.is_err());
    app.update_context_input(id, "files".into(), 2, None, InputMode::Search)
        .await
        .unwrap();
    assert!(app.preview_file(id, 1, row).await.is_err());
}

#[tokio::test]
async fn a_platform_without_a_file_source_reports_unavailable_in_the_workspace() {
    let app = app(Arc::new(TestPlatform::default()));
    let id = app.activate().await.unwrap().unwrap().session_id;
    let state = app
        .update_context_input(id, "report".into(), 1, None, InputMode::FileSearch)
        .await
        .unwrap()
        .unwrap();
    assert!(state.candidates.is_empty());
    assert_eq!(
        state.search_sources,
        [SearchSourcePresentation {
            id: "files".into(),
            state: SearchSourceState::Unavailable
        }]
    );
}
#[tokio::test]
async fn async_files_open_current_targets_and_record_only_business_usage() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join("report.txt");
    std::fs::write(&file, "report").unwrap();
    let folder = root.join("report folder");
    std::fs::create_dir(&folder).unwrap();
    let platform = Arc::new(TestPlatform::new(root.clone(), false));
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let mut app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let files = source(
        vec![
            FileSearchHit {
                path: file.clone(),
                is_directory: false,
            },
            FileSearchHit {
                path: folder.clone(),
                is_directory: true,
            },
        ],
        false,
    );
    app.register_async_candidate_source(files.clone()).unwrap();
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert_eq!(files.calls.load(Ordering::Acquire), 0);
    let immediate = app
        .update_context_input(
            initial.session_id,
            "report".into(),
            1,
            None,
            InputMode::FileSearch,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(immediate
        .candidates
        .iter()
        .all(|c| c.kind != CandidateKind::File));
    let state = files_ready(&app, initial.session_id, 1).await;
    assert!(state
        .context_candidates
        .iter()
        .all(|c| c.kind != CandidateKind::File));
    assert!(state.scopes.iter().all(|s| s.id != "files"));
    for candidate in state
        .candidates
        .iter()
        .filter(|c| c.kind == CandidateKind::File)
    {
        let result = app
            .execute(state.session_id, state.revision, candidate.id.clone())
            .await
            .unwrap();
        assert!(result.succeeded() && result.close_window);
    }
    let calls = platform.calls.lock().unwrap();
    assert!(calls
        .iter()
        .any(|(op, path)| op == "file" && path == file.to_str().unwrap()));
    assert!(calls
        .iter()
        .any(|(op, path)| op == "directory" && path == folder.to_str().unwrap()));
    let rows = history.lock().unwrap().load("test.editor").unwrap();
    assert_eq!(
        rows[&("core.file.open".into(), FILE.into())].execute_count,
        1
    );
    assert_eq!(
        rows[&("core.directory.open".into(), DIRECTORY.into())].execute_count,
        1
    );
}
#[tokio::test]
async fn new_query_and_dismissal_cancel_old_search_and_reject_old_execution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let old = root.join("old.txt");
    let new = root.join("new.txt");
    std::fs::write(&old, "old").unwrap();
    std::fs::write(&new, "new").unwrap();
    let platform = Arc::new(TestPlatform::new(root, false));
    let mut app = app(platform.clone());
    let files = source(
        vec![
            FileSearchHit {
                path: old,
                is_directory: false,
            },
            FileSearchHit {
                path: new,
                is_directory: false,
            },
        ],
        false,
    );
    app.register_async_candidate_source(files.clone()).unwrap();
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    app.update_context_input(
        initial.session_id,
        "old".into(),
        1,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    files.started.notified().await;
    app.update_context_input(
        initial.session_id,
        "new".into(),
        2,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    files.release.notify_one();
    let latest = files_ready(&app, initial.session_id, 2).await;
    assert!(latest
        .candidates
        .iter()
        .filter(|c| c.kind == CandidateKind::File)
        .all(|c| c.title == "new.txt"));
    let id = latest
        .candidates
        .iter()
        .find(|c| c.kind == CandidateKind::File)
        .unwrap()
        .id
        .clone();
    app.update_context_input(
        initial.session_id,
        "other".into(),
        3,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    assert!(app.execute(initial.session_id, 2, id).await.is_err());
    app.deactivate(initial.session_id).unwrap();
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert!(app.latest_state(initial.session_id).is_none());
    assert!(platform.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn scopes_content_empty_and_short_queries_do_not_query_file_index() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = app(Arc::new(TestPlatform::new(temp.path().into(), true)));
    let files = source(vec![], false);
    app.register_async_candidate_source(files.clone()).unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    for (revision, input, scope, mode) in [
        (
            1,
            "report",
            Some("folder".into()),
            InputMode::CapturedEnvironment,
        ),
        (2, "report", None, InputMode::Content),
        (3, "", None, InputMode::FileSearch),
        (4, "a", None, InputMode::FileSearch),
        (5, "report", None, InputMode::Search),
        (6, "report", None, InputMode::Environment),
    ] {
        app.update_context_input(id, input.into(), revision, scope, mode)
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert_eq!(files.calls.load(Ordering::Acquire), 0);
}
#[tokio::test]
async fn removed_file_fails_without_side_effect_or_usage() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("report.txt");
    std::fs::write(&path, "report").unwrap();
    let platform = Arc::new(TestPlatform::default());
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let mut app = Application::new(
        platform.clone(),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    app.register_async_candidate_source(source(
        vec![FileSearchHit {
            path: path.clone(),
            is_directory: false,
        }],
        false,
    ))
    .unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_context_input(id, "report".into(), 1, None, InputMode::FileSearch)
        .await
        .unwrap();
    let state = files_ready(&app, id, 1).await;
    let row = state
        .candidates
        .iter()
        .find(|c| c.kind == CandidateKind::File)
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!app
        .execute(id, 1, row.id.clone())
        .await
        .unwrap()
        .succeeded());
    assert!(platform.calls.lock().unwrap().is_empty());
    assert!(history
        .lock()
        .unwrap()
        .load("test.editor")
        .unwrap()
        .is_empty());
}
#[test]
fn file_results_are_limited_deduplicated_and_ignore_unrelated_names() {
    let hits = (0..100)
        .map(|i| FileSearchHit {
            path: PathBuf::from(format!("/tmp/report {i}.txt")),
            is_directory: false,
        })
        .collect::<Vec<_>>();
    let mut duplicate = hits.clone();
    duplicate.extend(hits);
    let candidates = file_candidates(duplicate, "report", &registry());
    assert_eq!(candidates.len(), 64);
    assert!(candidates
        .iter()
        .all(|c| c.presentation.kind == CandidateKind::File && !c.presentation.home_eligible));
    assert_eq!(
        candidates
            .iter()
            .map(|c| &c.presentation.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        64
    );
    assert!(file_candidates(
        vec![FileSearchHit {
            path: "/tmp/unrelated.txt".into(),
            is_directory: false
        }],
        "report",
        &registry()
    )
    .is_empty());
}

#[tokio::test]
async fn unavailable_workspace_does_not_change_normal_context_actions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let mut app = app(Arc::new(TestPlatform::new(root, true)));
    app.register_async_candidate_source(source(vec![], true))
        .unwrap();
    let id = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0)
        .await
        .session_id;
    app.update_context_input(id, "git".into(), 1, None, InputMode::FileSearch)
        .await
        .unwrap();
    let failed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let state = app.latest_state(id).unwrap();
            if state
                .search_sources
                .iter()
                .any(|s| s.state == SearchSourceState::Unavailable)
            {
                break state;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(failed.candidates.is_empty());
    assert!(failed.context_summary.is_empty());
    assert!(failed.scopes.is_empty());
    app.update_context_input(id, "git".into(), 2, None, InputMode::Environment)
        .await
        .unwrap();
    let state = settled(&app, id, 2).await;
    assert!(state
        .candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("core.git.status")));
    assert!(state.search_sources.is_empty());
}

#[tokio::test]
async fn registry_revocation_removes_cached_file_rows_and_old_execution() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("report.txt");
    std::fs::write(&path, "report").unwrap();
    let mut app = app(Arc::new(TestPlatform::default()));
    app.register_async_candidate_source(source(
        vec![FileSearchHit {
            path,
            is_directory: false,
        }],
        false,
    ))
    .unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_context_input(id, "report".into(), 1, None, InputMode::FileSearch)
        .await
        .unwrap();
    let state = files_ready(&app, id, 1).await;
    let row = state
        .candidates
        .iter()
        .find(|c| c.kind == CandidateKind::File)
        .unwrap();
    app.replace_registry(ActionRegistry::default());
    assert!(app
        .latest_state(id)
        .unwrap()
        .candidates
        .iter()
        .all(|c| c.kind != CandidateKind::File));
    assert!(app.execute(id, 1, row.id.clone()).await.is_err());
}

#[test]
fn file_type_changes_and_redirected_paths_are_rejected() {
    use anywhere_core::features::filesystem::search::validate_target;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("report.txt");
    std::fs::write(&path, "report").unwrap();
    let context = ResolvedContext::new(
        FILE,
        ContextValue::File(path.clone()),
        ContextSource::UserInput,
        "file-search",
    );
    assert!(validate_target(&context).is_ok());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(validate_target(&context).is_err());
    #[cfg(unix)]
    {
        let link = temp.path().join("redirected.txt");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        let context = ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(link),
            ContextSource::UserInput,
            "file-search",
        );
        assert!(validate_target(&context).is_err());
    }
}

#[tokio::test]
async fn file_search_reports_loading_ready_empty_and_failure_separately() {
    for fail in [false, true] {
        let mut app = app(Arc::new(TestPlatform::default()));
        app.register_async_candidate_source(source(vec![], fail))
            .unwrap();
        let initial = app.activate().await.unwrap().unwrap();
        let state = app
            .update_context_input(
                initial.session_id,
                "nothing".into(),
                1,
                None,
                InputMode::FileSearch,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.search_sources[0].state, SearchSourceState::Loading);
        let state = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let state = app.latest_state(initial.session_id).unwrap();
                if state
                    .search_sources
                    .first()
                    .is_some_and(|s| s.state != SearchSourceState::Loading)
                {
                    break state;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            state.search_sources[0].state,
            if fail {
                SearchSourceState::Unavailable
            } else {
                SearchSourceState::Ready
            }
        );
    }
}

#[tokio::test]
async fn reconfiguration_revokes_rows_actions_and_pending_publication() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = root.join("old.txt");
    std::fs::write(&path, "old").unwrap();
    let mut app = app(Arc::new(TestPlatform::default()));
    let files = source(
        vec![FileSearchHit {
            path,
            is_directory: false,
        }],
        false,
    );
    app.register_async_candidate_source(files.clone()).unwrap();
    let initial = app.activate().await.unwrap().unwrap();
    app.update_context_input(
        initial.session_id,
        "old".into(),
        1,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    files.started.notified().await;
    app.set_file_search(false, vec![], vec![]).unwrap();
    files.release.notify_one();
    tokio::time::sleep(Duration::from_millis(80)).await;
    let state = app.latest_state(initial.session_id).unwrap();
    assert!(state.search_sources.is_empty() && state.result_actions.is_empty());
    assert!(state
        .candidates
        .iter()
        .all(|c| c.kind != CandidateKind::File));
    app.update_context_input(
        initial.session_id,
        "old".into(),
        2,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert_eq!(files.calls.load(Ordering::Acquire), 1);
    app.set_file_search(true, vec![], vec![]).unwrap();
    app.update_context_input(
        initial.session_id,
        "old".into(),
        3,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    files.started.notified().await;
    files.release.notify_one();
    let state = files_ready(&app, initial.session_id, 3).await;
    let actions = app
        .file_actions(
            initial.session_id,
            state.revision,
            state.candidates[0].id.clone(),
        )
        .unwrap();
    let copy = actions
        .iter()
        .find(|a| a.command_id.as_deref() == Some("core.file.copy"))
        .unwrap();
    assert!(state.context_candidates.iter().all(|c| c.id != copy.id));
    app.set_file_search(true, vec![root.clone()], vec![])
        .unwrap();
    assert!(app
        .execute(initial.session_id, 3, copy.id.clone())
        .await
        .is_err());
    assert!(app
        .latest_state(initial.session_id)
        .unwrap()
        .result_actions
        .is_empty());

    // Changing either new setting must revoke execution offers, even with identical roots.
    for (revision, engine, include_hidden) in [
        (4, anywhere_platform::FileSearchEngine::Combined, false),
        (5, anywhere_platform::FileSearchEngine::Combined, true),
    ] {
        app.update_context_input(
            initial.session_id,
            "old".into(),
            revision,
            None,
            InputMode::FileSearch,
        )
        .await
        .unwrap();
        files.started.notified().await;
        files.release.notify_one();
        let state = files_ready(&app, initial.session_id, revision).await;
        let actions = app
            .file_actions(
                initial.session_id,
                state.revision,
                state.candidates[0].id.clone(),
            )
            .unwrap();
        let copy = actions
            .iter()
            .find(|a| a.command_id.as_deref() == Some("core.file.copy"))
            .unwrap();
        app.configure_file_search(
            true,
            anywhere_platform::FileSearchOptions {
                roots: vec![root.clone()],
                engine,
                include_hidden,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(app
            .execute(initial.session_id, revision, copy.id.clone())
            .await
            .is_err());
        let state = app.latest_state(initial.session_id).unwrap();
        assert!(state.result_actions.is_empty());
        assert!(state
            .candidates
            .iter()
            .all(|c| c.kind != CandidateKind::File));
    }
}

#[tokio::test]
async fn file_secondary_actions_copy_reveal_and_reject_deleted_targets() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = root.join("report.txt");
    std::fs::write(&path, "report").unwrap();
    let platform = Arc::new(TestPlatform::new(root, false));
    let mut app = Application::new(
        platform.clone(),
        registry(),
        Box::new(MemoryUsage::default()),
        vec![],
    );
    app.register_async_candidate_source(source(
        vec![FileSearchHit {
            path: path.clone(),
            is_directory: false,
        }],
        false,
    ))
    .unwrap();
    let initial = app.activate().await.unwrap().unwrap();
    app.update_context_input(
        initial.session_id,
        "report".into(),
        1,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    let state = files_ready(&app, initial.session_id, 1).await;
    let primary = state
        .candidates
        .iter()
        .find(|c| c.kind == CandidateKind::File)
        .unwrap();
    let actions = app
        .file_actions(initial.session_id, 1, primary.id.clone())
        .unwrap();
    for id in ["core.file.copy", "core.file.reveal"] {
        let action = actions
            .iter()
            .find(|c| c.command_id.as_deref() == Some(id))
            .unwrap();
        assert_eq!(action.execution_context_id, primary.execution_context_id);
        assert!(app
            .execute(initial.session_id, 1, action.id.clone())
            .await
            .unwrap()
            .succeeded());
    }
    assert!(platform
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(op, value)| op == "copy" && value == path.to_str().unwrap()));
    std::fs::remove_file(&path).unwrap();
    let copy = actions
        .iter()
        .find(|c| c.command_id.as_deref() == Some("core.file.copy"))
        .unwrap();
    let count = platform.calls.lock().unwrap().len();
    assert!(!app
        .execute(initial.session_id, 1, copy.id.clone())
        .await
        .unwrap()
        .succeeded());
    assert_eq!(platform.calls.lock().unwrap().len(), count);
}

#[test]
fn search_configuration_is_bounded_and_keeps_unavailable_roots_for_recovery() {
    let app = app(Arc::new(TestPlatform::default()));
    assert!(app
        .set_file_search(true, vec!["relative".into()], vec![])
        .is_err());
    assert!(app
        .set_file_search(true, vec![], vec!["nested/name".into()])
        .is_err());
    assert!(app
        .set_file_search(
            true,
            vec!["/missing/search-directory".into()],
            vec!["logs".into()]
        )
        .is_ok());
    assert!(app
        .set_file_search(true, vec!["/tmp".into(); 9], vec![])
        .is_err());
}

#[cfg(unix)]
#[test]
fn directory_links_are_listed_without_recursion_and_changed_targets_are_rejected() {
    use anywhere_core::features::filesystem::{scan::ScanStore, search::validate_target};
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let link = root.join("report-link");
    std::fs::write(outside.path().join("report-must-not-be-scanned.txt"), "").unwrap();
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
    let store = ScanStore::new(
        "report",
        FileSearchFilter::Folders,
        anywhere_platform::FileSearchOptions {
            roots: vec![root.clone()],
            include_hidden: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut page = None;
    store
        .run(&|| false, |batch| {
            page = Some(batch);
            Ok(())
        })
        .unwrap();
    let page = page.unwrap();
    assert_eq!(page.progress.issue_count, 0);
    assert_eq!(page.hits.len(), 1);
    assert_eq!(page.hits[0].path, link);
    assert!(page.hits[0].is_directory);
    let candidates = file_candidates(vec![page.hits[0].hit()], "", &registry());
    let context = candidates[0].context().unwrap();
    assert_eq!(context.value.path(), Some(link.as_path()));
    assert!(validate_target(context).is_ok());
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&root, &link).unwrap();
    assert!(validate_target(context).is_err());
}

#[cfg(unix)]
#[test]
fn non_unicode_parents_do_not_merge_valid_children_and_are_reported_as_partial() {
    use anywhere_core::features::filesystem::{scan::ScanStore, search::validate_target};
    use std::os::unix::ffi::OsStringExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let parents: Vec<_> = [0xfe, 0xff]
        .into_iter()
        .map(|byte| root.join(std::ffi::OsString::from_vec(vec![byte])))
        .collect();
    let raw_candidates = file_candidates(
        parents
            .iter()
            .map(|parent| FileSearchHit {
                path: parent.join("report.txt"),
                is_directory: false,
            })
            .collect(),
        "",
        &registry(),
    );
    assert_ne!(
        raw_candidates[0].presentation.id,
        raw_candidates[1].presentation.id
    );
    assert_ne!(
        raw_candidates[0].presentation.execution_context_id,
        raw_candidates[1].presentation.execution_context_id
    );
    for parent in parents {
        if let Err(error) = std::fs::create_dir(&parent) {
            #[cfg(target_os = "macos")]
            if error.raw_os_error() == Some(92) {
                // APFS rejects non-Unicode filenames (EILSEQ).
                eprintln!("Native path identity verified; this filesystem rejects the non-Unicode traversal fixture.");
                return;
            }
            panic!("create raw filename fixture: {error}");
        }
        std::fs::write(parent.join("report.txt"), "").unwrap();
    }
    let store = ScanStore::new(
        "report",
        FileSearchFilter::Files,
        anywhere_platform::FileSearchOptions {
            roots: vec![root],
            include_hidden: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut page = None;
    store
        .run(&|| false, |batch| {
            page = Some(batch);
            Ok(())
        })
        .unwrap();
    let page = page.unwrap();
    assert!(page.progress.finished);
    assert_eq!(page.progress.issue_count, 2);
    assert_eq!(page.hits.len(), 2);
    let candidates = file_candidates(
        page.hits.iter().map(|hit| hit.hit()).collect(),
        "",
        &registry(),
    );
    assert_ne!(candidates[0].presentation.id, candidates[1].presentation.id);
    assert_ne!(
        candidates[0].presentation.execution_context_id,
        candidates[1].presentation.execution_context_id
    );
    for candidate in candidates {
        assert!(validate_target(candidate.context().unwrap()).is_ok());
    }
}

/// Simulates a filesystem call that cannot return immediately on cancellation.
/// The real FileSearchSource still owns admission and cancel-on-drop behavior.
struct SlowIndex {
    active: AtomicUsize,
    peak: AtomicUsize,
    calls: AtomicUsize,
    cancelled: AtomicUsize,
    hold_old: std::sync::atomic::AtomicBool,
    started: Notify,
    hit: FileSearchHit,
    allow_copy: std::sync::atomic::AtomicBool,
    copied: Mutex<Vec<String>>,
}
impl Platform for SlowIndex {
    fn file_search_supported(&self) -> bool {
        true
    }
    fn search_files_with_options(
        &self,
        query: &str,
        limit: usize,
        _: &anywhere_platform::FileSearchOptions,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<FileSearchHit>> {
        self.search_files(query, limit, cancelled)
    }
    fn search_files(
        &self,
        query: &str,
        _: usize,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<FileSearchHit>> {
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        self.peak.fetch_max(active, Ordering::AcqRel);
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.started.notify_one();
        let start = Instant::now();
        while query == "old"
            && self.hold_old.load(Ordering::Acquire)
            && start.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(2));
        }
        let result = if cancelled() {
            self.cancelled.fetch_add(1, Ordering::AcqRel);
            vec![]
        } else {
            vec![self.hit.clone()]
        };
        self.active.fetch_sub(1, Ordering::AcqRel);
        Ok(result)
    }
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(None)
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn open_url(&self, _: &str) -> Result<()> {
        bail!("unexpected side effect")
    }
    fn open_file(&self, _: &std::path::Path) -> Result<()> {
        bail!("unexpected side effect")
    }
    fn reveal_file(&self, _: &std::path::Path) -> Result<()> {
        bail!("unexpected side effect")
    }
    fn open_directory(&self, _: &std::path::Path) -> Result<()> {
        bail!("unexpected side effect")
    }
    fn open_terminal(&self, _: &std::path::Path) -> Result<()> {
        bail!("unexpected side effect")
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        if !self.allow_copy.load(Ordering::Acquire) {
            bail!("unexpected side effect");
        }
        self.copied.lock().unwrap().push(text.to_owned());
        Ok(())
    }
}
fn slow_index(path: PathBuf) -> Arc<SlowIndex> {
    Arc::new(SlowIndex {
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
        cancelled: AtomicUsize::new(0),
        hold_old: std::sync::atomic::AtomicBool::new(true),
        allow_copy: std::sync::atomic::AtomicBool::new(false),
        copied: Mutex::new(Vec::new()),
        started: Notify::new(),
        hit: FileSearchHit {
            path,
            is_directory: false,
        },
    })
}
#[tokio::test]
async fn rapid_revisions_keep_the_os_worker_serial_even_when_a_cancelled_call_is_slow() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("report.txt");
    std::fs::write(&path, "report").unwrap();
    let index = slow_index(path);
    let app = Application::new(
        index.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    app.configure_file_search(
        true,
        anywhere_platform::FileSearchOptions {
            roots: vec![temp.path().canonicalize().unwrap()],
            include_hidden: true,
            ..Default::default()
        },
    )
    .unwrap();
    let first = app.activate().await.unwrap().unwrap();
    app.update_context_input(
        first.session_id,
        "old".into(),
        1,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(1), index.started.notified())
        .await
        .unwrap();
    for revision in 2..=20 {
        app.update_context_input(
            first.session_id,
            format!("report {revision}"),
            revision,
            None,
            InputMode::FileSearch,
        )
        .await
        .unwrap();
    }
    app.update_context_input(
        first.session_id,
        "report".into(),
        21,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert_eq!(
        index.calls.load(Ordering::Acquire),
        1,
        "new query must wait for the old worker's permit"
    );
    index.hold_old.store(false, Ordering::Release);
    let state = files_ready(&app, first.session_id, 21).await;
    assert!(state
        .candidates
        .iter()
        .filter(|c| c.kind == CandidateKind::File)
        .all(|c| c.title == "report.txt"));
    // Traversal can now finish before the independently serialized index query.
    tokio::time::timeout(Duration::from_secs(3), async {
        while index.calls.load(Ordering::Acquire) < 2 || index.active.load(Ordering::Acquire) > 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        index.calls.load(Ordering::Acquire),
        2,
        "superseded debounce jobs must not reach the OS"
    );
    assert_eq!(index.peak.load(Ordering::Acquire), 1);
    assert_eq!(index.active.load(Ordering::Acquire), 0);
    assert_eq!(index.cancelled.load(Ordering::Acquire), 1);
    app.deactivate(first.session_id).unwrap();
}
#[tokio::test]
async fn new_activation_cannot_receive_the_previous_sessions_file_batch() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("report.txt");
    std::fs::write(&path, "report").unwrap();
    let index = slow_index(path);
    let app = Application::new(
        index.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    app.configure_file_search(
        true,
        anywhere_platform::FileSearchOptions {
            roots: vec![temp.path().canonicalize().unwrap()],
            include_hidden: true,
            ..Default::default()
        },
    )
    .unwrap();
    let first = app.activate().await.unwrap().unwrap();
    app.update_context_input(
        first.session_id,
        "old".into(),
        1,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(1), index.started.notified())
        .await
        .unwrap();
    let next = app.activate().await.unwrap().unwrap();
    assert_ne!(next.session_id, first.session_id);
    app.update_context_input(
        next.session_id,
        "report".into(),
        1,
        None,
        InputMode::FileSearch,
    )
    .await
    .unwrap();
    index.hold_old.store(false, Ordering::Release);
    let state = files_ready(&app, next.session_id, 1).await;
    assert_eq!(state.session_id, next.session_id);
    assert!(app.latest_state(first.session_id).is_none());
    assert_eq!(index.peak.load(Ordering::Acquire), 1);
    assert_eq!(index.cancelled.load(Ordering::Acquire), 1);
    app.deactivate(next.session_id).unwrap();
}

#[tokio::test]
async fn complete_engine_appends_all_unindexed_matches_and_retains_execution_targets() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let unrelated = root.join("unrelated.txt");
    std::fs::write(&unrelated, "").unwrap();
    let index = slow_index(unrelated);
    index.hold_old.store(false, Ordering::Release);
    index.allow_copy.store(true, Ordering::Release);
    let app = Application::new(
        index.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let mut expected = std::collections::BTreeSet::new();
    for i in 0..131 {
        let pdf = root.join(format!("report-{i:03}.pdf"));
        std::fs::write(&pdf, "").unwrap();
        expected.insert(pdf.file_name().unwrap().to_str().unwrap().to_owned());
        std::fs::write(root.join(format!("report-{i:03}.txt")), "").unwrap();
    }
    std::fs::create_dir(root.join("report-folder")).unwrap();
    app.configure_file_search(
        true,
        anywhere_platform::FileSearchOptions {
            roots: vec![root],
            include_hidden: true,
            ..Default::default()
        },
    )
    .unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_file_search_input(id, "report".into(), 1, FileSearchFilter::Pdf)
        .await
        .unwrap();
    let mut state = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(s) = app.latest_state(id) {
                if s.file_search.as_ref().is_some_and(|p| p.finished) {
                    break s;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(state.search_sources[0].state, SearchSourceState::Ready);
    assert_eq!(state.file_search.as_ref().unwrap().confirmed_matches, 131);
    let contexts: std::collections::BTreeSet<_> = state
        .candidates
        .iter()
        .map(|c| c.execution_context_id.clone().unwrap())
        .collect();
    assert_eq!(
        contexts.len(),
        64,
        "each file owns its secondary action targets"
    );
    for action in &state.result_actions {
        assert!(contexts.contains(action.execution_context_id.as_ref().unwrap()));
    }
    let original_ids: Vec<_> = state.candidates.iter().map(|c| c.id.clone()).collect();
    let mut batches = 0;
    while state.file_search.as_ref().unwrap().next_cursor.is_some() {
        let old_len = state.candidates.len();
        state = app.load_more_files(id, 1).await.unwrap().unwrap();
        assert!(state.candidates.len() > old_len);
        assert_eq!(
            state.candidates[..64]
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>(),
            original_ids
        );
        batches += 1;
        assert!(batches <= 3);
    }
    let found: std::collections::BTreeSet<_> =
        state.candidates.iter().map(|c| c.title.clone()).collect();
    assert_eq!(state.candidates.len(), expected.len());
    assert_eq!(found, expected);
    // An earlier row remains an authorized execution target after appends.
    let actions = app
        .file_actions(id, 1, state.candidates[0].id.clone())
        .unwrap();
    let earlier = actions
        .iter()
        .find(|c| {
            c.execution_context_id == state.candidates[0].execution_context_id
                && c.command_id.as_deref() == Some("core.file.copy")
        })
        .unwrap();
    let copied = app.execute(id, 1, earlier.id.clone()).await.unwrap();
    assert_eq!(
        copied.status,
        anywhere_protocol::ActionStatus::Succeeded,
        "{:?}",
        copied.error
    );
    assert_eq!(
        index.copied.lock().unwrap().as_slice(),
        &[
            PathBuf::from(state.candidates[0].subtitle.as_ref().unwrap())
                .join(&state.candidates[0].title)
                .to_string_lossy()
                .into_owned()
        ]
    );
    let again = app.load_more_files(id, 1).await.unwrap().unwrap();
    assert_eq!(again.candidates.len(), expected.len());
    app.update_file_search_input(id, "report".into(), 2, FileSearchFilter::Folders)
        .await
        .unwrap();
    assert!(app.load_more_files(id, 1).await.is_err());
    let state = files_ready(&app, id, 2).await;
    assert_eq!(state.candidates.len(), 1);
    assert_eq!(state.candidates[0].title, "report-folder");
    app.deactivate(id).unwrap();
}

struct ConcurrentSource {
    id: &'static str,
    hit: FileSearchHit,
    started: Notify,
    release: Notify,
    held: bool,
}
#[async_trait]
impl AsyncCandidateSource for ConcurrentSource {
    fn id(&self) -> &'static str {
        self.id
    }
    fn accepts(&self, query: &str) -> bool {
        query == "report"
    }
    async fn candidates(&self, _: &str, _: ExecutionContext) -> Result<Vec<Candidate>> {
        bail!("uses stream")
    }
    async fn stream(
        &self,
        request: anywhere_core::core::search::candidate_source::AsyncSearchRequest,
        _: ExecutionContext,
        sender: tokio::sync::mpsc::Sender<
            anywhere_core::core::search::candidate_source::AsyncCandidateBatch,
        >,
    ) -> Result<()> {
        use anywhere_core::core::search::candidate_source::{AsyncCandidateBatch, BatchUpdate};
        assert!(
            request.options.is_none(),
            "general sources must not receive file filters"
        );
        sender
            .send(AsyncCandidateBatch {
                update: BatchUpdate::Replace,
                candidates: file_candidates(vec![self.hit.clone()], &request.text, &registry()),
                state: SearchSourceState::Ready,
                metadata: None,
            })
            .await?;
        self.started.notify_one();
        if self.held {
            self.release.notified().await;
            sender
                .send(AsyncCandidateBatch {
                    update: BatchUpdate::Replace,
                    candidates: vec![],
                    state: SearchSourceState::Ready,
                    metadata: None,
                })
                .await?;
        }
        Ok(())
    }
}
#[tokio::test]
async fn independent_sources_start_concurrently_and_replace_only_their_own_rows() {
    let directory = tempfile::tempdir().unwrap();
    let source = |id, name: &str, held| {
        let path = directory.path().join(name);
        std::fs::write(&path, "").unwrap();
        Arc::new(ConcurrentSource {
            id,
            hit: FileSearchHit {
                path: path.canonicalize().unwrap(),
                is_directory: false,
            },
            started: Notify::new(),
            release: Notify::new(),
            held,
        })
    };
    let slow = source("alpha", "report-alpha.txt", true);
    let fast = source("beta", "report-beta.txt", false);
    let mut application = app(Arc::new(TestPlatform::default()));
    application
        .register_async_candidate_source(slow.clone())
        .unwrap();
    application
        .register_async_candidate_source(fast.clone())
        .unwrap();
    let id = application.activate().await.unwrap().unwrap().session_id;
    application
        .update_context_input(id, "report".into(), 1, None, InputMode::Search)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), fast.started.notified())
        .await
        .expect("fast source must start while alpha is still streaming");
    slow.started.notified().await;
    slow.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let state = application.latest_state(id).unwrap();
            if state
                .candidates
                .iter()
                .any(|c| c.title == "report-beta.txt")
                && !state
                    .candidates
                    .iter()
                    .any(|c| c.title == "report-alpha.txt")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn duplicate_source_registration_is_rejected_without_replacing_the_first() {
    let mut app = app(Arc::new(TestPlatform::default()));
    let first = source(vec![], false);
    app.register_async_candidate_source(first.clone()).unwrap();
    assert!(app
        .register_async_candidate_source(source(vec![], false))
        .is_err());
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_file_search_input(id, "report".into(), 1, FileSearchFilter::All)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), first.started.notified())
        .await
        .unwrap();
    assert_eq!(first.calls.load(Ordering::Acquire), 1);
}
