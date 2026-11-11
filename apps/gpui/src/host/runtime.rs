use super::{directory_access::DirectoryAccess, login_item, HostPreferences, ProductPaths};
use crate::settings::{FeatureSetting, SettingsSnapshot};
use anyhow::{Context, Result};
use anywhere_core::application::frontend_runtime::{
    ApplicationRuntime, RuntimePaths, WebDocumentView,
};
use anywhere_core::application::SnapshotWait;
use anywhere_core::features::application_history::model::{
    Notice as HistoryNotice, SourceMode, Target,
};
use anywhere_core::features::clipboard::service::HistoryNotice as ClipboardNotice;
use anywhere_core::LauncherApplication as _;
use anywhere_protocol::{
    ActionResult, ExecutionSessionDetail, ExecutionSessionStatus, ExecutionSessionSummary, GroupId,
    InputMode, LauncherState, Revision, SessionId,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver as Receiver};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardItem {
    pub id: i64,
    pub summary: String,
    pub kind: String,
    pub copied_at_ms: i64,
    pub byte_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardSnapshot {
    pub recording: bool,
    pub recording_enabled: bool,
    pub available: bool,
    pub paused_until_ms: Option<i64>,
    pub retention_days: u32,
    pub stored_entries: u32,
    pub stored_bytes: u32,
    pub notice: Option<ClipboardNotice>,
    pub items: Vec<ClipboardItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHistorySource {
    pub id: String,
    pub title: String,
    pub file_hint: String,
    pub action_enabled: bool,
    pub path: Option<PathBuf>,
    pub mode: SourceMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHistoryItem {
    pub id: String,
    pub object_key: String,
    pub source_id: String,
    pub title: String,
    pub value: String,
    pub kind: &'static str,
    pub can_open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHistorySnapshot {
    pub source_id: String,
    pub items: Vec<AppHistoryItem>,
    pub failed_sources: Vec<String>,
    pub notice: Option<HistoryNotice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionRow {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub command: String,
    pub status: ExecutionSessionStatus,
    pub exit_code: Option<i32>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionDetail {
    pub row: ExecutionRow,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub entry: String,
    pub enabled: bool,
    pub available: bool,
    pub rollback: bool,
    pub permission_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPackageReview {
    pub token: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub permissions: Vec<String>,
    pub updating: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebDocument {
    pub handle: String,
    pub origin: String,
    pub entry: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewFile {
    pub id: String,
    pub paths: Vec<String>,
    pub title: String,
}

#[derive(Debug)]
pub enum LiveEvent {
    State(LauncherState),
    ActionResult(ActionResult),
    Settings(SettingsSnapshot),
    SettingsError {
        message: String,
        snapshot: Option<SettingsSnapshot>,
    },
    Preferences {
        preferences: HostPreferences,
        host_error: Option<String>,
    },
    Clipboard(ClipboardSnapshot),
    AppHistory(AppHistorySnapshot),
    AppHistorySources {
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
    runtime: tokio::runtime::Runtime,
    application: Arc<ApplicationRuntime>,
    session_id: SessionId,
    preferences_path: PathBuf,
    directory_access: Arc<DirectoryAccess>,
    search_access_error: Arc<Mutex<Option<String>>>,
}

impl LiveLauncher {
    pub fn open() -> Result<Self> {
        let paths = ProductPaths::discover()?;
        Self::open_at(paths.runtime_paths())
    }

    pub fn open_at(paths: RuntimePaths) -> Result<Self> {
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
        let session_id = application
            .application()
            .begin_activation()
            .context("reserve launcher session")?;
        Ok(Self {
            runtime,
            application,
            session_id,
            preferences_path,
            directory_access,
            search_access_error: Arc::new(Mutex::new(access_error)),
        })
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn data_directory(&self) -> &std::path::Path {
        &self.application.paths().data_directory
    }

    pub(crate) fn web_runtime(&self) -> Arc<ApplicationRuntime> {
        self.application.clone()
    }

    pub fn preferences(&self) -> Result<HostPreferences> {
        HostPreferences::load(&self.preferences_path)
    }

    pub fn search_access_error(&self) -> Option<String> {
        self.search_access_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub fn activate(&self, input_mode: InputMode) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application
                .activate_session_in_mode(session_id, false, input_mode)
                .await
            {
                Ok(Some(state)) => LiveEvent::State(state),
                Ok(None) => LiveEvent::Error("Launcher session was superseded".to_owned()),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn update_input(
        &self,
        input: String,
        revision: Revision,
        scope: Option<GroupId>,
        mode: InputMode,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application
                .update_context_input(session_id, input, revision, scope, mode)
                .await
            {
                Ok(Some(state)) => LiveEvent::State(state),
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn update_file_input(
        &self,
        input: String,
        revision: Revision,
        filter: anywhere_protocol::FileSearchFilter,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application
                .update_file_search_input(session_id, input, revision, filter)
                .await
            {
                Ok(Some(state)) => LiveEvent::State(state),
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    /// Follow every later snapshot for this session. The loop ends when the UI
    /// drops the receiver or Core closes the session.
    pub fn follow_snapshots(&self, after: u64) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        let (sender, receiver) = unbounded_channel();
        self.runtime.spawn(async move {
            let mut cursor = after;
            loop {
                match application.wait_for_snapshot(session_id, cursor).await {
                    SnapshotWait::Updated(state) => {
                        cursor = (u64::from(state.revision) << 32) | u64::from(state.snapshot_seq);
                        if sender.send(LiveEvent::State(*state)).is_err() {
                            break;
                        }
                    }
                    SnapshotWait::TimedOut => continue,
                    SnapshotWait::Closed | SnapshotWait::Superseded => {
                        let _ = sender.send(LiveEvent::Ignored);
                        break;
                    }
                }
            }
        });
        receiver
    }

    pub fn execute(&self, revision: Revision, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application
                .execute(session_id, revision, candidate_id)
                .await
            {
                Ok(result) => LiveEvent::ActionResult(result),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn application_icon(&self, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            let bytes = application.application_icon(&candidate_id)?;
            Ok(LiveEvent::ApplicationIcon {
                candidate_id,
                bytes,
            })
        })
    }

    pub fn object_actions(&self, revision: Revision, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application.file_actions(session_id, revision, candidate_id) {
                Ok(_) => match application.latest_state(session_id) {
                    Some(state) => LiveEvent::State(state),
                    None => LiveEvent::Error("object actions expired".to_owned()),
                },
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn load_more_files(&self, revision: Revision) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application.load_more_files(session_id, revision).await {
                Ok(Some(state)) => LiveEvent::State(state),
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn preview_file(&self, revision: Revision, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id;
        self.spawn(async move {
            match application
                .preview_file(session_id, revision, candidate_id)
                .await
            {
                Ok(value) => LiveEvent::Preview(preview(value)),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn preview_clipboard(&self, id: i64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let history = {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard()
            };
            Ok(LiveEvent::Preview(preview(
                application
                    .application()
                    .preview_clipboard(history.as_ref(), id)?,
            )))
        })
    }

    pub fn settings_snapshot(&self) -> Result<SettingsSnapshot> {
        settings_snapshot_from(&self.application)
    }

    pub fn set_clipboard_recording(&self, enabled: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_settings(move || {
            let settings = application.settings().map_err(anyhow::Error::new)?;
            settings
                .clipboard()
                .request_recording(enabled, application.application().platform())
        })
    }

    pub fn change_feature(&self, key: String, enabled: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_settings(move || {
            let mut settings = application.settings().map_err(anyhow::Error::new)?;
            settings
                .change(
                    application.application(),
                    anywhere_core::application::settings_service::SettingsChange::Enable {
                        key,
                        enabled,
                    },
                )
                .map(|_| ())
        })
    }

    pub fn set_preferences(&self, preferences: HostPreferences) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        let path = self.preferences_path.clone();
        let directory_access = self.directory_access.clone();
        let access_error = self.search_access_error.clone();
        self.spawn_blocking(move || {
            let roots = if preferences.file_search_enabled {
                directory_access.reconcile(&preferences.file_search_roots)?
            } else {
                directory_access.clear();
                Vec::new()
            };
            apply_preferences(&application, &preferences, &roots)?;
            preferences.save(&path)?;
            if let Ok(mut error) = access_error.lock() {
                *error = None;
            }
            let host_error = login_item::apply(preferences.launch_at_login)
                .err()
                .map(|error| format!("{error:#}"));
            Ok(LiveEvent::Preferences {
                preferences,
                host_error,
            })
        })
    }

    pub fn authorize_search_directories(
        &self,
        selected: Vec<String>,
        roots: Vec<String>,
    ) -> Receiver<LiveEvent> {
        let directory_access = self.directory_access.clone();
        let access_error = self.search_access_error.clone();
        self.spawn_blocking(move || {
            let selected = selected.into_iter().map(PathBuf::from).collect::<Vec<_>>();
            let roots = directory_access.authorize(&selected, &roots)?;
            if let Ok(mut error) = access_error.lock() {
                *error = None;
            }
            Ok(LiveEvent::DirectoriesAuthorized { roots })
        })
    }

    pub fn clipboard_snapshot(&self, query: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            tracing::debug!(query_length = query.len(), "clipboard_snapshot_started");
            let snapshot = clipboard_snapshot_from(&application, &query)?;
            tracing::debug!(
                item_count = snapshot.items.len(),
                "clipboard_snapshot_completed"
            );
            Ok(LiveEvent::Clipboard(snapshot))
        })
    }

    pub fn copy_clipboard(&self, id: i64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings
                    .clipboard()
                    .copy(id, application.application().platform())?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                "",
            )?))
        })
    }

    pub fn delete_clipboard(&self, id: i64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard().delete(id)?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                "",
            )?))
        })
    }

    pub fn clear_clipboard(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard().clear()?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                "",
            )?))
        })
    }

    pub fn set_clipboard_pause(&self, minutes: u32) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings
                    .clipboard()
                    .pause(minutes, application.application().platform())?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                "",
            )?))
        })
    }

    pub fn set_clipboard_retention(&self, days: u32) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard().set_retention_days(days)?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                "",
            )?))
        })
    }

    pub fn app_history_sources(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            Ok(LiveEvent::AppHistorySources {
                session_id: None,
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn configure_app_history(
        &self,
        id: String,
        path: String,
        mode: SourceMode,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let mut settings = application.settings().map_err(anyhow::Error::new)?;
                settings.configure_application_history(
                    application.application(),
                    &id,
                    mode,
                    (!path.trim().is_empty()).then_some(path.as_str()),
                )?;
            }
            Ok(LiveEvent::AppHistorySources {
                session_id: None,
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn set_app_history_action_enabled(&self, id: String, enabled: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let mut settings = application.settings().map_err(anyhow::Error::new)?;
                settings.set_application_history_action_enabled(
                    application.application(),
                    &id,
                    enabled,
                )?;
            }
            Ok(LiveEvent::AppHistorySources {
                session_id: None,
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn search_app_history(
        &self,
        session_id: u64,
        query: String,
        source_id: String,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            let service = {
                let settings = match application.settings() {
                    Ok(settings) => settings,
                    Err(error) => return LiveEvent::Error(error.to_string()),
                };
                match settings.application_history() {
                    Ok(service) => service,
                    Err(error) => return LiveEvent::Error(format!("{error:#}")),
                }
            };
            match service
                .search_wait(session_id, query, source_id.clone())
                .await
            {
                Ok(result) => LiveEvent::AppHistory(AppHistorySnapshot {
                    source_id,
                    items: result
                        .items
                        .iter()
                        .map(|item| {
                            let kind = match &item.entry.target {
                                Target::Directory { .. } | Target::ObsidianVault { .. } => {
                                    "directory"
                                }
                                Target::File { .. } => "file",
                                Target::Workspace { .. } => "workspace",
                                Target::Remote { .. } => "remote",
                            };
                            AppHistoryItem {
                                id: item.id.clone(),
                                object_key: format!("{}:{}", item.source_id, item.entry.key),
                                source_id: item.source_id.clone(),
                                title: item.entry.title.clone(),
                                value: item.entry.target.text(),
                                kind,
                                can_open: !matches!(item.entry.target, Target::Remote { .. }),
                            }
                        })
                        .collect(),
                    failed_sources: result.failed_sources,
                    notice: result.notice,
                }),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn open_app_history_session(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let settings = application.settings().map_err(anyhow::Error::new)?;
            let session_id = settings.application_history()?.open_session()?;
            Ok(LiveEvent::AppHistorySources {
                session_id: Some(session_id),
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn use_app_history_item(&self, id: String, copy: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let settings = application.settings().map_err(anyhow::Error::new)?;
            settings.application_history()?.use_item(
                &id,
                copy,
                application.application().platform(),
            )?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn execution_rows(&self) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            Ok(LiveEvent::Executions(
                application
                    .execution_sessions()
                    .into_iter()
                    .map(execution_row)
                    .collect(),
            ))
        })
    }

    pub fn execution_detail(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application
                .execution_session(&id)
                .map(|detail| LiveEvent::ExecutionDetail(execution_detail(detail)))
                .ok_or_else(|| anyhow::anyhow!("execution session not found"))
        })
    }

    pub fn stop_execution(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.stop_execution_session(&id)?;
            Ok(LiveEvent::Executions(
                application
                    .execution_sessions()
                    .into_iter()
                    .map(execution_row)
                    .collect(),
            ))
        })
    }

    pub fn web_packages(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || Ok(LiveEvent::WebPackages(web_packages_from(&application)?)))
    }

    pub fn inspect_web_package(&self, path: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let review = application.inspect_web_package(std::path::Path::new(&path))?;
            Ok(LiveEvent::WebReview(WebPackageReview {
                token: review.token,
                id: review.id,
                name: review.name,
                version: review.version,
                digest: review.digest,
                permissions: review
                    .permissions
                    .iter()
                    .map(web_permission_label)
                    .collect(),
                updating: review.updating,
            }))
        })
    }

    pub fn cancel_web_review(&self, token: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.cancel_web_review(&token)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn approve_web_package(&self, token: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.approve_web_package(&token)?;
            Ok(LiveEvent::WebPackages(web_packages_from(&application)?))
        })
    }

    pub fn change_web_package(&self, id: String, operation: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.change_web_package(&id, &operation)?;
            Ok(LiveEvent::WebPackages(web_packages_from(&application)?))
        })
    }

    pub fn open_web_document(
        &self,
        invocation_id: String,
        resume_handle: Option<String>,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application
                .open_web_document_view(&invocation_id, resume_handle.as_deref())
                .await
            {
                Ok(document) => LiveEvent::WebDocument(web_document(document)),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn close_web_document(&self, handle: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.close_web_document(&handle)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn dispatch_web_frame(
        &self,
        handle: String,
        origin: String,
        main_frame: bool,
        frame: String,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application
                .dispatch_web_frame(&handle, &origin, main_frame, &frame)
                .await
            {
                Ok(frame) => LiveEvent::WebFrameReply { handle, frame },
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn wait_web_state(&self, handle: String, after: u64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application.wait_web_state(&handle, after).await {
                Ok(Some(frame)) => LiveEvent::WebState { handle, frame },
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn retain_preview(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.retain_preview(&id)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn release_preview(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.release_preview(&id)?;
            Ok(LiveEvent::Ignored)
        })
    }

    fn spawn(
        &self,
        future: impl std::future::Future<Output = LiveEvent> + Send + 'static,
    ) -> Receiver<LiveEvent> {
        let (sender, receiver) = unbounded_channel();
        self.runtime.spawn(async move {
            let _ = sender.send(future.await);
        });
        receiver
    }

    fn spawn_blocking(
        &self,
        operation: impl FnOnce() -> Result<LiveEvent> + Send + 'static,
    ) -> Receiver<LiveEvent> {
        let (sender, receiver) = unbounded_channel();
        self.runtime.spawn_blocking(move || {
            let event = match operation() {
                Ok(event) => event,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            };
            let _ = sender.send(event);
        });
        receiver
    }

    fn spawn_settings(
        &self,
        operation: impl FnOnce() -> Result<()> + Send + 'static,
    ) -> Receiver<LiveEvent> {
        let (sender, receiver) = unbounded_channel();
        let application = self.application.clone();
        self.runtime.spawn_blocking(move || {
            let event = match operation() {
                Ok(()) => match settings_snapshot_from(&application) {
                    Ok(snapshot) => LiveEvent::Settings(snapshot),
                    Err(error) => LiveEvent::Error(format!("{error:#}")),
                },
                Err(error) => LiveEvent::SettingsError {
                    message: format!("{error:#}"),
                    snapshot: settings_snapshot_from(&application).ok(),
                },
            };
            let _ = sender.send(event);
        });
        receiver
    }
}

fn reconcile_access_at_startup(
    directory_access: &DirectoryAccess,
    preferences: &HostPreferences,
) -> (Vec<PathBuf>, Option<String>) {
    if !preferences.file_search_enabled {
        directory_access.clear();
        return (Vec::new(), None);
    }
    if preferences.file_search_roots.is_empty() {
        return (
            Vec::new(),
            Some("File search needs at least one authorized directory.".to_owned()),
        );
    }
    match directory_access.reconcile(&preferences.file_search_roots) {
        Ok(roots) => (roots, None),
        Err(error) => (
            Vec::new(),
            Some(format!("File search is disabled: {error:#}")),
        ),
    }
}

fn apply_preferences(
    application: &ApplicationRuntime,
    preferences: &HostPreferences,
    authorized_roots: &[PathBuf],
) -> Result<()> {
    preferences.validate()?;
    application.application().set_capture_sources(
        preferences.capture_finder_selection,
        preferences.capture_selected_text,
        preferences.capture_browser_current_page,
        preferences.capture_clipboard,
    );
    application
        .application()
        .set_excluded_apps(preferences.excluded_apps.clone());
    let enabled = preferences.file_search_enabled && !authorized_roots.is_empty();
    application.application().configure_file_search(
        enabled,
        anywhere_platform::FileSearchOptions {
            roots: authorized_roots.to_vec(),
            excluded_directory_names: preferences.file_search_excluded_names.clone(),
            engine: preferences.file_search_engine(),
            include_hidden: preferences.file_search_include_hidden,
            restrict_to_roots: true,
        },
    )?;
    Ok(())
}

fn settings_snapshot_from(application: &ApplicationRuntime) -> Result<SettingsSnapshot> {
    let settings = application.settings().map_err(anyhow::Error::new)?;
    let clipboard_recording = settings
        .clipboard()
        .status()
        .map(|status| status.recording_enabled)
        .unwrap_or(false);
    let features = settings
        .statuses()
        .into_iter()
        .filter(|status| status.first_party && status.configurable && !status.legacy)
        .map(|status| FeatureSetting {
            key: status.id,
            name: status.name,
            enabled: status.enabled,
        })
        .collect();
    Ok(SettingsSnapshot {
        clipboard_recording,
        features,
    })
}

fn clipboard_snapshot_from(
    application: &ApplicationRuntime,
    query: &str,
) -> Result<ClipboardSnapshot> {
    let settings = application.settings().map_err(anyhow::Error::new)?;
    let history = settings.clipboard();
    let status = history.status()?;
    let (stored_entries, stored_bytes) = history.usage()?;
    let items = if status.available {
        history
            .search(query)?
            .into_iter()
            .map(|item| ClipboardItem {
                id: item.id,
                summary: item.summary,
                kind: item.kind,
                copied_at_ms: item.copied_at_ms,
                byte_count: item.byte_count,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(ClipboardSnapshot {
        recording: status.recording,
        recording_enabled: status.recording_enabled,
        available: status.available,
        paused_until_ms: status.policy.paused_until_ms,
        retention_days: status.policy.retention_days,
        stored_entries,
        stored_bytes,
        notice: status.notice,
        items,
    })
}

fn app_history_sources_from(application: &ApplicationRuntime) -> Result<Vec<AppHistorySource>> {
    let settings = application.settings().map_err(anyhow::Error::new)?;
    let service = settings.application_history()?;
    service.refresh()?;
    Ok(service
        .sources()
        .into_iter()
        .map(|status| AppHistorySource {
            id: status.descriptor.id,
            title: status.descriptor.title,
            file_hint: status.descriptor.file_hint,
            action_enabled: status.action_enabled,
            path: status.path,
            mode: status.mode,
        })
        .collect())
}

fn execution_row(value: ExecutionSessionSummary) -> ExecutionRow {
    ExecutionRow {
        id: value.id.to_string(),
        title: value.title,
        directory: value.directory,
        command: value.command.join(" "),
        status: value.status,
        exit_code: value.exit_code,
        started_at_ms: value.started_at_ms,
        finished_at_ms: value.finished_at_ms,
        output_truncated: value.output_truncated,
        error: value.error,
    }
}

fn execution_detail(value: ExecutionSessionDetail) -> ExecutionDetail {
    ExecutionDetail {
        row: execution_row(value.summary),
        stdout: value.stdout,
        stderr: value.stderr,
    }
}

fn preview(value: anywhere_core::features::preview::PreviewResource) -> PreviewFile {
    PreviewFile {
        id: value.id,
        paths: value.paths,
        title: value.title,
    }
}

fn web_packages_from(application: &ApplicationRuntime) -> Result<Vec<WebPackage>> {
    Ok(application
        .web_package_statuses()?
        .into_iter()
        .map(|status| WebPackage {
            id: status.id,
            name: status.name,
            version: status.version,
            digest: status.digest,
            entry: status.entry,
            enabled: status.enabled,
            available: status.available,
            rollback: status.rollback,
            permission_count: status.permissions.len(),
        })
        .collect())
}

fn web_document(document: WebDocumentView) -> WebDocument {
    WebDocument {
        handle: document.binding.runtime_id().to_owned(),
        origin: document.origin,
        entry: document.entry,
        title: document.title,
    }
}

fn web_permission_label(permission: &anywhere_protocol::web_plugin::WebPermission) -> String {
    use anywhere_protocol::web_plugin::WebPermission;
    match permission {
        WebPermission::ContextRead { types } => format!("context.read:{}", types.join(",")),
        WebPermission::CurrentApp {} => "current-app".to_owned(),
        WebPermission::ClipboardReadText {} => "clipboard.read-text".to_owned(),
        WebPermission::ClipboardWriteText {} => "clipboard.write-text".to_owned(),
        WebPermission::LocalStorage {} => "local-storage".to_owned(),
        WebPermission::InputRead {} => "input.read".to_owned(),
        WebPermission::InputWrite {} => "input.write".to_owned(),
        WebPermission::FrontmostApp {} => "frontmost-app".to_owned(),
    }
}
