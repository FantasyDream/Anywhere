use anyhow::{bail, Result};
use async_trait::async_trait;
use launcher_core::{
    command::CommandRegistry,
    context::{ContextValue, ResolvedContext},
    history::MemoryHistory,
    provider::{ContextProvider, ExecutionClass, ExecutionContext, ProviderDescriptor},
    Application, LauncherApplication,
};
use launcher_platform::{ApplicationInfo, Platform};
use launcher_protocol::*;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;

struct TestPlatform {
    directory: PathBuf,
    finder: bool,
    directory_captures: AtomicUsize,
    calls: Mutex<Vec<(String, String)>>,
    fail: bool,
}
impl TestPlatform {
    fn new(directory: PathBuf, finder: bool) -> Self {
        Self {
            directory,
            finder,
            directory_captures: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
            fail: false,
        }
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
        Ok(Some(self.directory.clone()))
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
fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::builtin();
    registry
        .register_plugin(launcher_plugin::parse_manifest(launcher_plugin::EXAMPLE_GOOGLE).unwrap())
        .unwrap();
    registry
}
fn app(platform: Arc<TestPlatform>) -> Application {
    Application::new(
        platform,
        registry(),
        Box::<MemoryHistory>::default(),
        vec![],
    )
}
fn action(state: &LauncherState, command: &str) -> String {
    state
        .actions
        .iter()
        .find(|a| a.command_id == command)
        .unwrap_or_else(|| panic!("missing {command} in {state:?}"))
        .id
        .clone()
}

#[tokio::test]
async fn scenarios_a_to_f_use_the_same_facade_and_executor() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(dir.path().canonicalize().unwrap(), true));
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    let session = state.session_id;
    assert_eq!(
        state
            .actions
            .iter()
            .map(|a| a.command_id.as_str())
            .collect::<Vec<_>>(),
        [
            "core.directory.terminal",
            "core.directory.copy",
            "core.directory.open"
        ]
    );
    assert_eq!(state.context_summary.len(), 2);
    assert!(
        app.execute(session, action(&state, "core.directory.terminal"))
            .await
            .unwrap()
            .success
    );

    let url = app
        .update_input(session, "https://github.com".into(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(url.actions[0].command_id, "core.url.open");
    assert_eq!(url.actions[1].command_id, "core.url.copy");
    assert!(url
        .actions
        .iter()
        .all(|a| !a.command_id.starts_with("core.directory.")));
    assert!(url
        .context_summary
        .iter()
        .all(|c| c.type_id != DIRECTORY && c.type_id != APPLICATION));
    assert!(
        app.execute(session, action(&url, "core.url.copy"))
            .await
            .unwrap()
            .success
    );
    assert_eq!(
        platform.calls.lock().unwrap().last().unwrap(),
        &("copy".into(), "https://github.com/".into())
    );
    assert!(app
        .execute(session, state.actions[0].id.clone())
        .await
        .is_err());

    let json = app
        .update_input(session, r#"{"name":"foo","age":18}"#.into(), 2)
        .await
        .unwrap()
        .unwrap();
    for command in ["core.json.format", "core.json.minify", "core.json.copy"] {
        assert!(
            app.execute(session, action(&json, command))
                .await
                .unwrap()
                .success
        );
    }
    let formatted = app
        .execute(session, action(&json, "core.json.format"))
        .await
        .unwrap();
    assert!(!formatted.close_window);
    assert!(formatted.output_text.unwrap().contains('\n'));
    let minified = app
        .execute(session, action(&json, "core.json.minify"))
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
            app.execute(session, action(&file, command))
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
    assert!(directory.actions[0].id.contains(":4:"));
    assert_eq!(
        directory
            .actions
            .iter()
            .filter(|a| a.command_id == "core.directory.open")
            .count(),
        1
    );
    assert!(directory.actions[0].score > directory.actions.last().unwrap().score);

    let text = app
        .update_input(session, "hello world".into(), 5)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(text.actions[0].command_id, "google.search");
    assert_eq!(text.actions.len(), 2);
    assert!(
        app.execute(session, action(&text, "google.search"))
            .await
            .unwrap()
            .success
    );
    let calls = platform.calls.lock().unwrap();
    let query = url::Url::parse(&calls.last().unwrap().1).unwrap();
    assert_eq!(
        query.query_pairs().find(|(key, _)| key == "q").unwrap().1,
        "hello world"
    );
    assert_eq!(
        platform.directory_captures.load(Ordering::Relaxed),
        1,
        "Finder context is an activation snapshot"
    );
    drop(calls);
    let cleared = app
        .update_input(session, String::new(), 6)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cleared.actions.len(), 3);
    assert!(cleared
        .actions
        .iter()
        .all(|a| a.command_id.starts_with("core.directory.")));
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
                execution_class: ExecutionClass::AsyncLocal,
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
async fn finder_provider_ignores_other_apps_and_failures_do_not_report_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut platform = TestPlatform::new(dir.path().into(), false);
    platform.fail = true;
    let platform = Arc::new(platform);
    let app = app(platform.clone());
    let state = app.activate().await.unwrap().unwrap();
    assert!(state.actions.is_empty());
    assert_eq!(platform.directory_captures.load(Ordering::Relaxed), 0);
    let state = app
        .update_input(state.session_id, "https://example.com".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let result = app
        .execute(state.session_id, action(&state, "core.url.open"))
        .await
        .unwrap();
    assert!(!result.success);
    assert!(!result.close_window);
    assert!(result.message.contains("simulated platform failure"));
}

#[test]
fn invalid_plugin_does_not_partially_register_and_discovery_continues() {
    let mut registry = registry();
    let mut plugin = launcher_plugin::parse_manifest(launcher_plugin::EXAMPLE_GOOGLE).unwrap();
    plugin.id = "example".into();
    let mut bad = plugin.commands[0].clone();
    bad.id = "bad".into();
    bad.action = DeclarativeAction::ExecuteBuiltin {
        command_id: "core.json.format".into(),
    };
    plugin.commands.push(bad);
    assert!(registry.register_plugin(plugin).is_err());
    assert!(registry.get("example.search").is_none());
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("good.toml"),
        launcher_plugin::EXAMPLE_GOOGLE,
    )
    .unwrap();
    std::fs::write(dir.path().join("bad.toml"), "this is not toml").unwrap();
    let discovered = launcher_plugin::discover(dir.path());
    assert_eq!(discovered.manifests.len(), 1);
    assert_eq!(discovered.diagnostics.len(), 1);
    assert!(discovered.diagnostics[0].contains("bad.toml"));
}

#[tokio::test]
async fn successful_action_with_history_failure_reports_a_warning() {
    struct UnwritableHistory;
    impl launcher_core::history::UsageHistory for UnwritableHistory {
        fn load(&self, _: &str) -> Result<launcher_core::history::UsageSnapshot> {
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
        .execute(session, action(&state, "core.url.copy"))
        .await
        .unwrap();
    assert!(result.success);
    assert!(!result.close_window);
    assert!(result.message.contains("disk full"));
    assert_eq!(platform.calls.lock().unwrap().len(), 1);
}
