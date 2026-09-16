//! Facade-level scenarios through the same public API the host uses.
use anyhow::{bail, Result};
use anywhere_core::{
    applications::ApplicationCatalog,
    context::{ContextValue, ResolvedContext},
    core::action::descriptor::CommandRegistry,
    core::context::provider::{ContextProvider, ExecutionContext, ProviderDescriptor},
    core::ranking::usage::{MemoryUsage, UsageRepository},
    features::applications::index::IndexedApplication,
    persistence::usage::SqliteUsageRepository,
    Application, LauncherApplication,
};
use anywhere_platform::{ApplicationInfo, BundleApplication, GitOperation, GitOutput, Platform};
use anywhere_protocol::*;
use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::sync::Notify;

/// One-shot gate used to hold a directory capture open while a test asserts.
///
/// `release()` is idempotent, so the test can release a gate before or after the
/// capture arrives without racing on a barrier participant count.
struct CaptureGate {
    released: Mutex<bool>,
    signal: Condvar,
}

impl CaptureGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            signal: Condvar::new(),
        })
    }
    fn wait(&self) {
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.signal.wait(released).unwrap();
        }
    }
    fn release(&self) {
        let mut released = self.released.lock().unwrap();
        *released = true;
        self.signal.notify_all();
    }
    fn released(&self) -> bool {
        *self.released.lock().unwrap()
    }
}

#[derive(Default)]
struct TestPlatform {
    directory: PathBuf,
    finder: bool,
    directory_captures: AtomicUsize,
    git_calls: AtomicUsize,
    launches: AtomicUsize,
    clipboard: Mutex<Option<String>>,
    calls: Mutex<Vec<(String, String)>>,
    fail: bool,
    /// Successive directory captures block on these gates until the test releases them.
    gates: Mutex<Vec<Arc<CaptureGate>>>,
    /// Simulates Finder Automation latency so the first snapshot budget cuts short.
    capture_delay: Option<Duration>,
    /// Names the fake platform reports as this bundle's localized names.
    localized: Vec<String>,
}

impl TestPlatform {
    fn new(directory: PathBuf, finder: bool) -> Self {
        Self {
            directory,
            finder,
            ..Default::default()
        }
    }
    /// Queue a gate for the next directory capture, returning it for the test.
    fn push_gate(&self) -> Arc<CaptureGate> {
        let gate = CaptureGate::new();
        self.gates.lock().unwrap().push(gate.clone());
        gate
    }
    fn call(&self, operation: &str, value: &str) -> Result<()> {
        if self.fail {
            bail!("simulated platform failure");
        }
        self.calls
            .lock()
            .unwrap()
            .push((operation.into(), value.into()));
        Ok(())
    }
}

impl Platform for TestPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(Some(ApplicationInfo {
            bundle_id: if self.finder {
                "com.apple.finder"
            } else {
                "test.editor"
            }
            .into(),
            name: if self.finder { "Finder" } else { "Editor" }.into(),
        }))
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        if !self.finder {
            return Ok(None);
        }
        self.directory_captures.fetch_add(1, Ordering::Relaxed);
        let gate = self.gates.lock().unwrap().pop();
        if let Some(gate) = gate {
            // Runs on a blocking worker, so a plain std condvar is safe here.
            gate.wait();
        }
        if let Some(delay) = self.capture_delay {
            std::thread::sleep(delay);
        }
        Ok(Some(self.directory.clone()))
    }
    fn bundle_application(&self, bundle: &Path) -> Result<Option<BundleApplication>> {
        let stem = bundle
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Some(BundleApplication {
            bundle_id: format!("app.{}.com", stem.replace(' ', "").to_lowercase()),
            name: stem,
            aliases: Vec::new(),
            path: Some(bundle.to_string_lossy().into_owned()),
            icon: None,
        }))
    }
    fn preferred_languages(&self) -> Vec<String> {
        if self.localized.is_empty() {
            Vec::new()
        } else {
            vec!["zh-Hans-CN".into()]
        }
    }
    fn bundle_localized_names(&self, _: &Path, languages: &[String]) -> Result<Vec<String>> {
        Ok(if languages.is_empty() {
            Vec::new()
        } else {
            self.localized.clone()
        })
    }
    fn launch_application(&self, path: &str) -> Result<bool> {
        self.launches.fetch_add(1, Ordering::Relaxed);
        self.call("launch", path)?;
        Ok(true)
    }
    fn application_icon(&self, path: &str, size: u32) -> Result<Option<Vec<u8>>> {
        // A four-byte PNG header is enough to prove the bytes travel the whole path.
        if !path.ends_with(".app") {
            return Ok(None);
        }
        assert_eq!(
            size, 64,
            "icons are requested at the small presentation size"
        );
        Ok(Some(vec![0x89, b'P', b'N', b'G']))
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        Ok(self.clipboard.lock().unwrap().clone())
    }
    fn git_available(&self) -> bool {
        true
    }
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        self.git_calls.fetch_add(1, Ordering::Relaxed);
        self.call(
            "git",
            &format!("{}:{}", operation.as_str(), repository.display()),
        )?;
        Ok(GitOutput {
            text: format!("git {} output", operation.as_str()),
            truncated: false,
        })
    }
    fn open_url(&self, url: &str) -> Result<()> {
        self.call("url", url)
    }
    fn open_file(&self, path: &Path) -> Result<()> {
        self.call("file", path.to_str().unwrap())
    }
    fn reveal_file(&self, path: &Path) -> Result<()> {
        self.call("reveal", path.to_str().unwrap())
    }
    fn open_directory(&self, path: &Path) -> Result<()> {
        self.call("directory", path.to_str().unwrap())
    }
    fn open_terminal(&self, path: &Path) -> Result<()> {
        self.call("terminal", path.to_str().unwrap())
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        self.call("copy", text)
    }
}

/// Hands the same memory history to the core and to the test.
struct SharedHistory(Arc<Mutex<MemoryUsage>>);
impl UsageRepository for SharedHistory {
    fn load(&self, source_app: &str) -> Result<anywhere_core::core::ranking::usage::UsageSnapshot> {
        self.0.lock().unwrap().load(source_app)
    }
    fn load_global(&self) -> Result<anywhere_core::core::ranking::usage::UsageSnapshot> {
        self.0.lock().unwrap().load_global()
    }
    fn record(
        &mut self,
        command_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .record(command_id, context_type, source_app, now)
    }
}

fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::builtin();
    registry
        .register_plugin(anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap())
        .unwrap();
    registry
}
fn app(platform: Arc<TestPlatform>) -> Application {
    Application::new(platform, registry(), Box::<MemoryUsage>::default(), vec![])
}
fn app_with_budget(platform: Arc<TestPlatform>, budget: Duration) -> Application {
    Application::with_budget(
        platform,
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
        budget,
    )
}
fn candidate(state: &LauncherState, command: &str) -> String {
    state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some(command))
        .unwrap_or_else(|| panic!("missing {command} in {state:?}"))
        .id
        .clone()
}
fn titles(state: &LauncherState) -> Vec<&str> {
    state
        .candidates
        .iter()
        .map(|candidate| candidate.title.as_str())
        .collect()
}
fn scope<'a>(state: &'a LauncherState, id: &str) -> Vec<&'a str> {
    state
        .scopes
        .iter()
        .find(|scope| scope.id == id)
        .map(|scope| {
            scope
                .candidate_ids
                .iter()
                .filter_map(|id| state.candidate(id))
                .map(|candidate| candidate.title.as_str())
                .collect()
        })
        .unwrap_or_default()
}
fn scope_ids(state: &LauncherState) -> Vec<&str> {
    state.scopes.iter().map(|scope| scope.id.as_str()).collect()
}

/// Wait until a revision reports a complete context graph.
async fn settled(app: &Application, session: SessionId, revision: Revision) -> LauncherState {
    for _ in 0..200 {
        let state = {
            let subscription = app.subscribe(session);
            subscription.latest_state()
        };
        if let Some(state) = state {
            if state.revision == revision && state.session_id == session && state.context_complete {
                return state;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("revision {revision} never completed");
}

/// Records every tracing field name and log-message value the core emits.
struct FieldRecorder(Arc<Mutex<Vec<String>>>);
impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for FieldRecorder {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct Visitor(Arc<Mutex<Vec<String>>>);
        impl tracing::field::Visit for Visitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                let mut seen = self.0.lock().unwrap();
                seen.push(field.name().to_owned());
                if field.name() == "message" {
                    seen.push(format!("{value:?}").trim_matches('"').to_owned());
                }
            }
            fn record_u64(&mut self, field: &tracing::field::Field, _: u64) {
                self.0.lock().unwrap().push(field.name().to_owned());
            }
            fn record_i64(&mut self, field: &tracing::field::Field, _: i64) {
                self.0.lock().unwrap().push(field.name().to_owned());
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                let mut seen = self.0.lock().unwrap();
                seen.push(field.name().to_owned());
                if field.name() == "message" {
                    seen.push(value.to_owned());
                }
            }
        }
        event.record(&mut Visitor(self.0.clone()));
    }
}

/// The one recorder every test shares, installed as the process-wide subscriber.
fn recorded_fields() -> &'static Arc<Mutex<Vec<String>>> {
    static FIELDS: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
    FIELDS.get_or_init(|| {
        let fields = Arc::new(Mutex::new(Vec::new()));
        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(FieldRecorder(fields.clone()));
        let _ = tracing::subscriber::set_global_default(subscriber);
        fields
    })
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
    assert_eq!(scope_ids(&state)[0], "all");
    assert_eq!(scope_ids(&state)[1], "app");
    assert_eq!(
        scope(&state, "app").len(),
        3,
        "Finder owns the captured directory"
    );
    assert!(scope(&state, "folder").contains(&"Copy Path"));
    assert!(
        app.execute(
            session,
            state.revision,
            candidate(&state, "core.directory.terminal")
        )
        .await
        .unwrap()
        .success
    );

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
            .all(|context| context.type_id != APPLICATION),
        "the raw foreground application row stays internal"
    );
    assert!(
        app.execute(session, url.revision, candidate(&url, "core.url.copy"))
            .await
            .unwrap()
            .success
    );
    assert_eq!(
        platform.calls.lock().unwrap().last().unwrap(),
        &("copy".into(), "https://github.com/".into())
    );
    // An id from a superseded revision must not execute.
    assert!(app
        .execute(session, url.revision, state.candidates[0].id.clone())
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
        assert!(
            app.execute(session, json.revision, candidate(&json, command))
                .await
                .unwrap()
                .success
        );
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
        assert!(
            app.execute(session, file.revision, candidate(&file, command))
                .await
                .unwrap()
                .success
        );
    }
    let directory = app
        .update_input(session, dir.path().to_string_lossy().into_owned(), 4)
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
    assert!(
        app.execute(session, text.revision, candidate(&text, "example.search"))
            .await
            .unwrap()
            .success
    );
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
async fn git_directory_offers_git_scope_and_runs_git_only_on_request() {
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
    assert!(scope_ids(&state).contains(&"git"));
    assert!(scope_ids(&state).contains(&"folder"));
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
        assert!(result.success, "{command}: {result:?}");
        assert!(!result.close_window, "git output stays in the preview");
        assert_eq!(result.output_text.as_deref(), Some(expected));
    }
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 3);
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
    assert!(launched.success);
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
    assert!(
        app.execute(
            state.session_id,
            searched.revision,
            searched.candidates[0].id.clone()
        )
        .await
        .unwrap()
        .success
    );
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
                "core.directory.open",
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
        .find(|candidate| candidate.command_id.as_deref() == Some("core.directory.open"))
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
    assert!(error.to_string().contains("git is not installed"));
    assert_eq!(inner.git_calls.load(Ordering::Relaxed), 0);
}

struct DelayedProvider {
    descriptor: ProviderDescriptor,
    started: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl ContextProvider for DelayedProvider {
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
        self.started.notify_one();
        self.release.notified().await;
        Ok(vec![ResolvedContext::new(
            "test.enriched",
            ContextValue::Extension("late".into()),
            input.source,
            "test.delayed",
        )])
    }
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
    assert!(scope_ids(&late).contains(&"git"));
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
    assert_eq!(
        copy_id,
        format!(
            "action:core.directory.copy:{DIRECTORY}:{}",
            dir.path().canonicalize().unwrap().display()
        )
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
        url_id.contains("example.org"),
        "revision 1's action must belong to revision 1's input: {url_id}"
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
    assert!(executed.success, "{executed:?}");
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
    assert!(result.success, "{result:?}");

    let recorded = history.lock().unwrap().load_global().unwrap();
    let key = ("core.directory.copy".to_owned(), DIRECTORY.to_owned());
    let stats = recorded
        .get(&key)
        .unwrap_or_else(|| panic!("no usage row for {key:?}: {recorded:?}"));
    assert_eq!(stats.execute_count, 1);
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
            !result.success,
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
    assert!(late.scopes.iter().any(|scope| scope.id == "git"));
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
        state.scopes.iter().any(|scope| scope.id == "folder"),
        "the Folder scope must be present on the first snapshot"
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
    // The real bundle roots are scanned, so the query below is matched against this
    // machine's applications. `chrome` is present on any developer machine and was the
    // reported case; the assertion only needs one application candidate to appear.
    let platform = Arc::new(TestPlatform {
        capture_delay: Some(Duration::from_millis(60)),
        ..TestPlatform::new(dir.path().canonicalize().unwrap(), false)
    });
    let app = Arc::new(
        app_with_budget(platform, Duration::from_millis(20)).with_content_grace(Duration::ZERO),
    );
    let mut index = app.index_updates();
    // Publish a catalog the scan will replace, so the search below really does run
    // against an index that does not yet contain the query's application.
    *app.resolver.catalog.lock().unwrap() = ApplicationCatalog::new(Vec::new());
    let first = app.activate().await.unwrap().unwrap();
    assert!(
        !first
            .candidates
            .iter()
            .any(|candidate| candidate.kind == CandidateKind::Application),
        "the scan must not have published yet: {first:?}"
    );
    assert!(!*index.borrow(), "the app index is not ready yet");
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
    let scope_ids = state
        .scopes
        .iter()
        .map(|scope| scope.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        scope_ids,
        ["all", "app", "git", "folder"],
        "the scope bar must match the documented order"
    );
    assert!(state.context_complete, "one snapshot should be enough here");
}

/// The grace is bounded: a capture slower than budget plus grace must not block the
/// first snapshot, and the late pass still fills the actions in.
#[tokio::test]
async fn first_snapshot_grace_is_bounded_and_late_pass_still_completes() {
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
    assert!(titles(&state).is_empty(), "{:?}", titles(&state));

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
    assert!(!result.success);
    assert!(!result.close_window);
    assert!(result.message.contains("simulated platform failure"));
}

#[test]
fn invalid_plugin_does_not_partially_register_and_discovery_continues() {
    let mut registry = registry();
    let mut plugin = anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap();
    plugin.id = "broken".into();
    let mut bad = plugin.commands[0].clone();
    bad.id = "bad".into();
    bad.action = DeclarativeAction::ExecuteBuiltin {
        command_id: "core.json.format".into(),
    };
    plugin.commands.push(bad);
    assert!(registry.register_plugin(plugin).is_err());
    // Nothing from the rejected manifest may be registered, under any id.
    assert!(registry.get("broken.search").is_none());
    assert!(registry.get("broken.bad").is_none());
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("good.toml"),
        anywhere_plugin::EXAMPLE_PLUGIN,
    )
    .unwrap();
    std::fs::write(dir.path().join("bad.toml"), "this is not toml").unwrap();
    let discovered = anywhere_plugin::discover(dir.path());
    assert_eq!(discovered.manifests.len(), 1);
    assert_eq!(discovered.diagnostics.len(), 1);
    assert!(discovered.diagnostics[0].contains("bad.toml"));
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
    assert!(result.success);
    assert!(!result.close_window);
    assert!(result.message.contains("disk full"));
    assert_eq!(platform.calls.lock().unwrap().len(), 1);
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
                        "applications_duration" => {
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
        *app.resolver.catalog.lock().unwrap() = ApplicationCatalog::new(
            (0..2_000)
                .map(|index| IndexedApplication {
                    bundle_id: format!("com.example.application{index}"),
                    name: format!("Example Application {index}"),
                    path: format!("/Applications/Example {index}.app"),
                    aliases: vec![format!("app{index}")],
                    keywords: vec!["example".into()],
                })
                .collect(),
        );

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

#[tokio::test]
async fn every_named_performance_metric_is_emitted_for_one_activation() {
    use anywhere_core::provider::metrics;
    let fields = recorded_fields();
    fields.lock().unwrap().clear();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform::new(root, true));
    // Zero budget so the first snapshot is cut short and a late pass follows.
    let app = app_with_budget(platform, Duration::ZERO).with_content_grace(Duration::ZERO);
    let state = app.activate().await.unwrap().unwrap();
    let _ = settled(&app, state.session_id, state.revision).await;

    let seen = fields.lock().unwrap().clone();
    for metric in metrics::ALL {
        // hotkey_to_visible is the native host's, not the core's.
        if metric == metrics::HOTKEY_TO_VISIBLE {
            assert!(
                include_str!("../../../apps/flutter/macos/Runner/Main.swift").contains(metric),
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
