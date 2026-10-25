//! Facade-level scenarios through the same public API the host uses.
pub(crate) use anyhow::{bail, Result};
pub(crate) use anywhere_core::{
    core::action::descriptor::ActionRegistry,
    core::context::provider::{metrics, ContextProvider, ProviderDescriptor},
    core::context::{ContextValue, ExecutionContext, ResolvedContext},
    core::ranking::usage::{MemoryUsage, UsageRepository},
    persistence::usage::SqliteUsageRepository,
    Application, LauncherApplication,
};
pub(crate) use anywhere_platform::{
    ApplicationInfo, BundleApplication, DiscoveredMenuItem, GitOperation, GitOutput, MenuDiscovery,
    MenuLocator, Platform,
};
pub(crate) use anywhere_protocol::*;
pub(crate) use async_trait::async_trait;
pub(crate) use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicI32, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::Duration,
};
pub(crate) use tokio::sync::Notify;

/// One-shot gate used to hold a directory capture open while a test asserts.
///
/// `release()` is idempotent, so the test can release a gate before or after the
/// capture arrives without racing on a barrier participant count.
pub(crate) struct CaptureGate {
    pub(crate) released: Mutex<bool>,
    pub(crate) signal: Condvar,
}

impl CaptureGate {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            signal: Condvar::new(),
        })
    }
    pub(crate) fn wait(&self) {
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.signal.wait(released).unwrap();
        }
    }
    pub(crate) fn release(&self) {
        let mut released = self.released.lock().unwrap();
        *released = true;
        self.signal.notify_all();
    }
    pub(crate) fn released(&self) -> bool {
        *self.released.lock().unwrap()
    }
}

#[derive(Default)]
pub(crate) struct TestPlatform {
    pub(crate) frontmost_bundle: Option<&'static str>,
    pub(crate) directory: PathBuf,
    pub(crate) finder: bool,
    pub(crate) directory_captures: AtomicUsize,
    pub(crate) git_calls: AtomicUsize,
    pub(crate) launches: AtomicUsize,
    pub(crate) clipboard: Mutex<Option<String>>,
    /// A one-shot block for a clipboard permission prompt in activation tests.
    pub(crate) clipboard_gate: Mutex<Option<Arc<CaptureGate>>>,
    pub(crate) clipboard_started: Notify,
    pub(crate) calls: Mutex<Vec<(String, String)>>,
    pub(crate) fail: bool,
    /// Successive directory captures block on these gates until the test releases them.
    pub(crate) gates: Mutex<Vec<Arc<CaptureGate>>>,
    /// Simulates Finder Automation latency so the first snapshot budget cuts short.
    pub(crate) capture_delay: Option<Duration>,
    pub(crate) directory_error: Mutex<Option<anywhere_platform::ContextReadError>>,
    pub(crate) frontmost_gate: Mutex<Option<Arc<CaptureGate>>>,
    pub(crate) frontmost_started: Notify,
    pub(crate) catalog_gate: Mutex<Option<Arc<CaptureGate>>>,
    pub(crate) no_git: bool,
    pub(crate) git_output: Mutex<Option<GitOutput>>,
    pub(crate) git_error: Mutex<Option<String>>,
    pub(crate) git_panics: bool,
    pub(crate) clipboard_error: bool,
    pub(crate) menu_trusted: Mutex<bool>,
    pub(crate) menu_items: Mutex<Vec<DiscoveredMenuItem>>,
    pub(crate) menu_gate: Mutex<Option<Arc<CaptureGate>>>,
    pub(crate) menu_delay: Option<Duration>,
    pub(crate) menu_version: Mutex<Option<String>>,
    pub(crate) menu_perform_error: Mutex<Option<String>>,
    pub(crate) menu_performs: AtomicUsize,
    pub(crate) menu_discoveries: AtomicUsize,
    pub(crate) menu_snapshot_before_gate: bool,
    pub(crate) menu_instance: Mutex<Option<String>>,
    pub(crate) frontmost_pid: AtomicI32,
}

impl TestPlatform {
    pub(crate) fn new(directory: PathBuf, finder: bool) -> Self {
        Self {
            directory,
            finder,
            ..Default::default()
        }
    }
    /// Queue a gate for the next directory capture, returning it for the test.
    pub(crate) fn push_gate(&self) -> Arc<CaptureGate> {
        let gate = CaptureGate::new();
        self.gates.lock().unwrap().push(gate.clone());
        gate
    }
    pub(crate) fn call(&self, operation: &str, value: &str) -> Result<()> {
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
    fn is_application_entry(&self, path: &Path) -> bool {
        path.extension().is_some_and(|extension| extension == "app")
    }
    fn application_roots(&self) -> Vec<PathBuf> {
        vec![self.directory.clone()]
    }

    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        let gate = self.frontmost_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            self.frontmost_started.notify_one();
            gate.wait();
        }
        Ok(Some(ApplicationInfo {
            bundle_id: self
                .frontmost_bundle
                .unwrap_or(if self.finder {
                    "com.apple.finder"
                } else {
                    "test.editor"
                })
                .into(),
            name: if self.finder { "Finder" } else { "Editor" }.into(),
            instance_id: Some(
                self.menu_instance
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| "test-process".into()),
            ),
            pid: {
                let pid = self.frontmost_pid.load(Ordering::Relaxed);
                if pid == 0 {
                    42
                } else {
                    pid
                }
            },
        }))
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        if !self.finder && self.frontmost_bundle.is_none() {
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
        if let Some(error) = *self.directory_error.lock().unwrap() {
            return Err(anyhow::Error::new(error).context("original directory capture"));
        }
        Ok(Some(self.directory.clone()))
    }
    fn bundle_application(&self, bundle: &Path) -> Result<Option<BundleApplication>> {
        let gate = self.catalog_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.wait();
        }

        let stem = bundle
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Some(BundleApplication {
            localized_names: Default::default(),
            bundle_id: format!("app.{}.com", stem.replace(' ', "").to_lowercase()),
            name: stem,
            aliases: Vec::new(),
            path: Some(bundle.to_string_lossy().into_owned()),
            icon: None,
        }))
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
        if self.clipboard_error {
            bail!("read failed");
        }
        let text = self.clipboard.lock().unwrap().clone();
        let gate = self.clipboard_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            self.clipboard_started.notify_one();
            gate.wait();
        }
        Ok(text)
    }
    fn git_available(&self) -> bool {
        if self.git_panics {
            panic!("git probe exploded");
        }
        !self.no_git
    }
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        self.git_calls.fetch_add(1, Ordering::Relaxed);
        self.call(
            "git",
            &format!("{}:{}", operation.as_str(), repository.display()),
        )?;
        if let Some(error) = self.git_error.lock().unwrap().as_ref() {
            bail!("{error}");
        }
        if let Some(output) = self.git_output.lock().unwrap().clone() {
            return Ok(output);
        }
        Ok(GitOutput {
            text: match operation {
                GitOperation::BranchName => "feature/current".into(),
                GitOperation::CommitHash => "0123456789abcdef0123456789abcdef01234567".into(),
                GitOperation::ShortCommitHash => "0123456".into(),
                GitOperation::RemoteUrl => "git@example.com:team/repo.git".into(),
                _ => format!("git {} output", operation.as_str()),
            },
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
    fn accessibility_trusted(&self, _prompt: bool) -> bool {
        *self.menu_trusted.lock().unwrap()
    }
    fn discover_menu_actions(&self, _: &ApplicationInfo) -> Result<MenuDiscovery> {
        if !*self.menu_trusted.lock().unwrap() {
            return Ok(MenuDiscovery::Untrusted);
        }
        self.menu_discoveries.fetch_add(1, Ordering::SeqCst);
        let early = self
            .menu_snapshot_before_gate
            .then(|| self.menu_items.lock().unwrap().clone());
        let gate = self.menu_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.wait();
        }
        if let Some(delay) = self.menu_delay {
            std::thread::sleep(delay);
        }
        Ok(MenuDiscovery::Ready(early.unwrap_or_else(|| {
            self.menu_items.lock().unwrap().clone()
        })))
    }
    fn perform_menu_action(&self, app: &ApplicationInfo, locator: &MenuLocator) -> Result<()> {
        if !self.accessibility_trusted(false) {
            bail!("accessibility permission is not granted");
        }
        if self.frontmost_application()?.as_ref() != Some(app) {
            bail!("application instance changed");
        }
        if let Some(error) = self.menu_perform_error.lock().unwrap().clone() {
            bail!("{error}");
        }
        let items = self.menu_items.lock().unwrap();
        let Some(item) = items.iter().find(|item| {
            item.menu_path == locator.titles && item.identifiers == locator.identifiers
        }) else {
            bail!("menu item was not found");
        };
        if !item.enabled {
            bail!("菜单项当前不可用");
        }
        self.menu_performs.fetch_add(1, Ordering::Relaxed);
        self.call("menu", &locator.titles.join(">"))
    }
    fn application_version(&self, _: &str) -> Option<String> {
        self.menu_version.lock().unwrap().clone()
    }
}

/// Hands the same memory history to the core and to the test.
pub(crate) struct SharedHistory(pub(crate) Arc<Mutex<MemoryUsage>>);
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

pub(crate) fn registry() -> ActionRegistry {
    let mut registry = anywhere_core::application::builtin_registry();
    registry
        .register_plugin(anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap())
        .unwrap();
    registry
}
pub(crate) fn app_with_budget(platform: Arc<TestPlatform>, budget: Duration) -> Application {
    Application::with_budget(
        platform,
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
        budget,
    )
}
pub(crate) fn candidate(state: &LauncherState, command: &str) -> String {
    state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some(command))
        .unwrap_or_else(|| panic!("missing {command} in {state:?}"))
        .id
        .clone()
}
pub(crate) fn titles(state: &LauncherState) -> Vec<&str> {
    state
        .candidates
        .iter()
        .map(|candidate| candidate.title.as_str())
        .collect()
}
pub(crate) fn context_actions<'a>(state: &'a LauncherState, type_id: &str) -> Vec<&'a str> {
    state
        .context_summary
        .iter()
        .find(|context| context.type_id == type_id && context.key == context.root_key)
        .map(|context| {
            state
                .context_candidates
                .iter()
                .filter(|candidate| candidate.context_keys.contains(&context.key))
                .collect::<Vec<_>>()
                .into_iter()
                .map(|candidate| candidate.title.as_str())
                .collect()
        })
        .unwrap_or_default()
}
pub(crate) fn context_types(state: &LauncherState) -> Vec<&str> {
    state
        .context_summary
        .iter()
        .filter(|context| context.key == context.root_key && !context.candidate_ids.is_empty())
        .map(|context| context.type_id.as_str())
        .collect()
}

/// Records tracing field names emitted while a test is running.
pub(crate) struct FieldRecorder(Arc<Mutex<Vec<String>>>);
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
            fn record_bool(&mut self, field: &tracing::field::Field, _: bool) {
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
pub(crate) struct DelayedProvider {
    pub(crate) descriptor: ProviderDescriptor,
    pub(crate) started: Arc<Notify>,
    pub(crate) release: Arc<Notify>,
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

pub(crate) fn app(platform: Arc<TestPlatform>) -> Application {
    Application::new(platform, registry(), Box::<MemoryUsage>::default(), vec![])
}

pub(crate) async fn settled(
    app: &Application,
    session: SessionId,
    revision: Revision,
) -> LauncherState {
    for _ in 0..200 {
        if !app.supplemental_ready(session) {
            tokio::time::sleep(Duration::from_millis(5)).await;
            continue;
        }
        let state = {
            let subscription = app.subscribe(session);
            subscription.latest_state()
        };
        if let Some(state) = state {
            if state.revision == revision
                && state.session_id == session
                && state.context_complete
                && app.supplemental_ready(session)
            {
                return state;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("revision {revision} never completed");
}

/// Serializes tests that clear and inspect the shared process-wide recorder.
pub(crate) static RECORDING_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Records every tracing field name and log-message value the core emits.
pub(crate) fn recorded_fields() -> &'static Arc<Mutex<Vec<String>>> {
    static FIELDS: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
    FIELDS.get_or_init(|| {
        let fields = Arc::new(Mutex::new(Vec::new()));
        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(FieldRecorder(fields.clone()));
        let _ = tracing::subscriber::set_global_default(subscriber);
        fields
    })
}
