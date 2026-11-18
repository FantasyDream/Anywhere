use super::runtime_support::{
    app_history_object_action, app_history_sources_from, apply_preferences,
    clipboard_snapshot_from, execution_detail, execution_row, preview, reconcile_access_at_startup,
    settings_snapshot_from, snapshot_cursor, web_document, web_packages_from, web_permission_label,
};
use super::{
    directory_access::DirectoryAccess,
    dto::{
        AppHistoryItem, AppHistoryObjectAction, AppHistorySnapshot, AppHistorySource,
        ClipboardSnapshot, ExecutionDetail, ExecutionRow, PreviewFile, ShortcutCandidate,
        ShortcutCatalogAction, WebDocument, WebPackage, WebPackageReview,
    },
    login_item, ActionShortcutBinding, HostPreferences, ProductPaths,
};
use crate::settings::SettingsSnapshot;
use anyhow::{Context, Result};
use anywhere_core::application::frontend_runtime::{ApplicationRuntime, RuntimePaths};
use anywhere_core::application::ShortcutRoute;
use anywhere_core::features::application_history::model::{SourceMode, Target};
use anywhere_core::LauncherApplication as _;
use anywhere_protocol::{ActionResult, GroupId, InputMode, LauncherState, Revision, SessionId};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc::{channel, Receiver};

mod actions;
mod app_history;
mod clipboard;
mod launcher;
mod settings;
mod tasks;
mod web;

#[derive(Debug)]
pub enum LiveEvent {
    SharedState(Arc<LauncherState>),
    ActionResult(ActionResult),
    Settings(SettingsSnapshot),
    SettingsError {
        message: String,
        snapshot: Option<SettingsSnapshot>,
    },
    LoginItemReconciled(Option<String>),
    Preferences {
        preferences: HostPreferences,
        host_error: Option<String>,
    },
    Clipboard(ClipboardSnapshot),
    AppHistory(AppHistorySnapshot),
    AppHistoryAction {
        item_id: String,
        copy: bool,
    },
    AppHistoryObjectActions {
        request: u64,
        item_id: String,
        revision: Revision,
        actions: Vec<AppHistoryObjectAction>,
    },
    AppHistoryObjectActionsFailed {
        request: u64,
    },
    AppHistoryAdaptersOpened {
        directory: String,
    },
    ShortcutCatalog(Vec<ShortcutCatalogAction>),
    ActionShortcutReady {
        action_id: String,
        revision: Revision,
        candidates: Vec<ShortcutCandidate>,
    },
    ActionShortcutFailed {
        action_id: String,
        message: String,
    },
    AppHistorySources {
        request: u64,
        session_id: Option<u64>,
        sources: Vec<AppHistorySource>,
    },
    Executions(Vec<ExecutionRow>),
    ExecutionDetail(ExecutionDetail),
    WebPackages(Vec<WebPackage>),
    WebReview(WebPackageReview),
    WebDocument(WebDocument),
    WebFrameReply {
        handle: String,
        frame: String,
    },
    WebState {
        handle: String,
        frame: String,
    },
    DirectoriesAuthorized {
        roots: Vec<String>,
    },
    ApplicationIcon {
        candidate_id: String,
        bytes: Option<Vec<u8>>,
    },
    Preview(PreviewFile),
    Ignored,
    Error(String),
}

/// Owns the Tokio runtime and the shared, frontend-neutral application runtime.
/// GPUI never runs blocking Core or platform work on its render thread.
pub struct LiveLauncher {
    runtime: Option<tokio::runtime::Runtime>,
    application: Arc<ApplicationRuntime>,
    session_id: AtomicU32,
    preferences_path: PathBuf,
    explicit_data_directory: bool,
    directory_access: Arc<DirectoryAccess>,
    search_access_error: Arc<Mutex<Option<String>>>,
    preferences_write: Arc<tokio::sync::Mutex<()>>,
    preferences_cache: Arc<Mutex<HostPreferences>>,
    settings_cache: Arc<Mutex<SettingsSnapshot>>,
}

impl LiveLauncher {
    pub fn open() -> Result<Self> {
        let paths = ProductPaths::discover()?;
        Self::open_with_isolation(paths.runtime_paths(), paths.explicit_data_directory)
    }

    pub fn open_at(paths: RuntimePaths) -> Result<Self> {
        Self::open_with_isolation(paths, true)
    }

    fn open_with_isolation(paths: RuntimePaths, explicit_data_directory: bool) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("anywhere-core")
            .enable_all()
            .build()
            .context("create Core runtime")?;
        let application = {
            let _entered = runtime.enter();
            ApplicationRuntime::open(paths.clone())?
        };
        let preferences_path = paths.data_directory.join("settings.json");
        let preferences = HostPreferences::load(&preferences_path)?;
        let directory_access = Arc::new(DirectoryAccess::new(
            paths
                .data_directory
                .join("search-access")
                .join("bookmarks.json"),
        ));
        let (authorized_roots, access_error) =
            reconcile_access_at_startup(&directory_access, &preferences);
        apply_preferences(&application, &preferences, &authorized_roots)?;
        let settings_cache = settings_snapshot_from(&application)?;
        Ok(Self {
            runtime: Some(runtime),
            application,
            session_id: AtomicU32::new(0),
            preferences_path,
            explicit_data_directory,
            directory_access,
            search_access_error: Arc::new(Mutex::new(access_error)),
            preferences_write: Arc::new(tokio::sync::Mutex::new(())),
            preferences_cache: Arc::new(Mutex::new(preferences)),
            settings_cache: Arc::new(Mutex::new(settings_cache)),
        })
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id.load(Ordering::Acquire)
    }

    pub fn data_directory(&self) -> &std::path::Path {
        &self.application.paths().data_directory
    }

    pub fn app_history_adapters_directory(&self) -> PathBuf {
        self.data_directory().join("application-history-sources")
    }

    pub(crate) fn web_runtime(&self) -> Arc<ApplicationRuntime> {
        self.application.clone()
    }

    pub fn preferences(&self) -> Result<HostPreferences> {
        self.preferences_cache
            .lock()
            .map(|preferences| preferences.clone())
            .map_err(|_| anyhow::anyhow!("preferences cache is unavailable"))
    }

    pub fn search_access_error(&self) -> Option<String> {
        self.search_access_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    fn spawn(
        &self,
        future: impl std::future::Future<Output = LiveEvent> + Send + 'static,
    ) -> Receiver<LiveEvent> {
        let (sender, receiver) = channel(1);
        self.runtime().spawn(async move {
            let _ = sender.send(future.await).await;
        });
        receiver
    }

    fn spawn_blocking(
        &self,
        operation: impl FnOnce() -> Result<LiveEvent> + Send + 'static,
    ) -> Receiver<LiveEvent> {
        let (sender, receiver) = channel(1);
        let preferences = self.preferences_cache.clone();
        self.runtime().spawn_blocking(move || {
            let event = match operation() {
                Ok(event) => event,
                Err(error) => {
                    let locale = preferences
                        .lock()
                        .map(|preferences| preferences.ui_language.locale())
                        .unwrap_or(crate::l10n::Locale::En);
                    LiveEvent::Error(super::directory_access::localized_error(&error, locale))
                }
            };
            let _ = sender.blocking_send(event);
        });
        receiver
    }

    fn spawn_settings(
        &self,
        operation: impl FnOnce() -> Result<()> + Send + 'static,
    ) -> Receiver<LiveEvent> {
        let (sender, receiver) = channel(1);
        let application = self.application.clone();
        let settings_cache = self.settings_cache.clone();
        self.runtime().spawn_blocking(move || {
            let event = match operation() {
                Ok(()) => match settings_snapshot_from(&application) {
                    Ok(snapshot) => {
                        if let Ok(mut cached) = settings_cache.lock() {
                            *cached = snapshot.clone();
                        }
                        LiveEvent::Settings(snapshot)
                    }
                    Err(error) => LiveEvent::Error(format!("{error:#}")),
                },
                Err(error) => LiveEvent::SettingsError {
                    message: format!("{error:#}"),
                    snapshot: settings_snapshot_from(&application).ok(),
                },
            };
            let _ = sender.blocking_send(event);
        });
        receiver
    }

    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime
            .as_ref()
            .expect("LiveLauncher runtime is unavailable only after Drop")
    }
}

impl Drop for LiveLauncher {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // Application shutdown must not block the AppKit/GPUI main thread
            // while long-running Core work or native helpers finish.
            runtime.shutdown_background();
        }
    }
}
