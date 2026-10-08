use crate::support::*;
use anywhere_core::core::search::candidate::Candidate;
use anywhere_core::core::search::candidate_source::AsyncCandidateSource;
use anywhere_core::features::filesystem::search::file_candidates;
use anywhere_platform::FileSearchHit;

struct Files {
    hits: Vec<FileSearchHit>,
    calls: AtomicUsize,
    started: Notify,
    release: Notify,
    fail: bool,
}
#[async_trait]
impl AsyncCandidateSource for Files {
    fn id(&self) -> &'static str {
        "fixture-files"
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
    app.register_async_candidate_source(files.clone());
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    assert_eq!(files.calls.load(Ordering::Acquire), 0);
    let immediate = app
        .update_context_input(
            initial.session_id,
            "report".into(),
            1,
            None,
            InputMode::Search,
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
    app.register_async_candidate_source(files.clone());
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    app.update_input(initial.session_id, "old".into(), 1)
        .await
        .unwrap();
    files.started.notified().await;
    app.update_input(initial.session_id, "new".into(), 2)
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
    app.update_input(initial.session_id, "other".into(), 3)
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
    app.register_async_candidate_source(files.clone());
    let id = app.activate().await.unwrap().unwrap().session_id;
    for (revision, input, scope, mode) in [
        (
            1,
            "report",
            Some("folder".into()),
            InputMode::CapturedEnvironment,
        ),
        (2, "report", None, InputMode::Content),
        (3, "", None, InputMode::Search),
        (4, "a", None, InputMode::Search),
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
    ));
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "report".into(), 1).await.unwrap();
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
    let hits = (0..20)
        .map(|i| FileSearchHit {
            path: PathBuf::from(format!("/tmp/report {i}.txt")),
            is_directory: false,
        })
        .collect::<Vec<_>>();
    let mut duplicate = hits.clone();
    duplicate.extend(hits);
    let candidates = file_candidates(duplicate, "report", &registry());
    assert_eq!(candidates.len(), 8);
    assert!(candidates
        .iter()
        .all(|c| c.presentation.kind == CandidateKind::File && !c.presentation.home_eligible));
    assert_eq!(
        candidates
            .iter()
            .map(|c| &c.presentation.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
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
async fn unavailable_file_index_does_not_remove_current_context_actions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let mut app = app(Arc::new(TestPlatform::new(root, true)));
    let files = source(Vec::new(), true);
    app.register_async_candidate_source(files.clone());
    let initial = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let first = app
        .update_input(initial.session_id, "git".into(), 1)
        .await
        .unwrap()
        .unwrap();
    files.started.notified().await;
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let state = app.latest_state(initial.session_id).unwrap();
            if state.snapshot_seq > first.snapshot_seq {
                assert!(state
                    .candidates
                    .iter()
                    .any(|c| c.command_id.as_deref() == Some("core.git.status")));
                assert!(state
                    .candidates
                    .iter()
                    .all(|c| c.kind != CandidateKind::File));
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
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
    ));
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "report".into(), 1).await.unwrap();
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
