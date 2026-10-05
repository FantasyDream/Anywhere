use anyhow::{Context, Result};
use anywhere_core::{persistence::usage::SqliteUsageRepository, Application, LauncherApplication};
use anywhere_protocol as protocol;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

static APPLICATION: OnceLock<Application> = OnceLock::new();
static PLUGIN_DIRS: OnceLock<Vec<PathBuf>> = OnceLock::new();
static PLUGIN_REPORTS: OnceLock<Mutex<Vec<PluginStatusDto>>> = OnceLock::new();

pub fn ping() -> String {
    anywhere_core::ping().to_owned()
}

pub struct ShortcutActionDto {
    pub action_id: String,
    pub title: String,
    pub context_types: Vec<String>,
}

pub async fn shortcut_actions() -> Result<Vec<ShortcutActionDto>> {
    Ok(application()?
        .shortcut_actions()?
        .into_iter()
        .map(|action| ShortcutActionDto {
            action_id: action.action_id,
            title: action.title,
            context_types: action.context_types,
        })
        .collect())
}

pub async fn resolve_action_binding(
    session_id: u32,
    revision: u32,
    action_id: String,
) -> Result<Vec<CandidateDto>> {
    Ok(application()?
        .resolve_action_binding(session_id, revision, &action_id)?
        .into_iter()
        .map(candidate_dto)
        .collect())
}

pub fn initialize(data_directory: String, plugin_directory: String) -> Result<()> {
    if APPLICATION.get().is_some() {
        return Ok(());
    }
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let data = Path::new(&data_directory);
    std::fs::create_dir_all(data)?;
    let mut diagnostics = Vec::new();
    let history = match SqliteUsageRepository::open(&data.join("usage.sqlite3")) {
        Ok(history) => {
            Box::new(history) as Box<dyn anywhere_core::core::ranking::usage::UsageRepository>
        }
        Err(error) => {
            tracing::error!(error = %error, "usage_history_unavailable");
            diagnostics.push(format!(
                "Usage history is unavailable; this session will not be saved: {error:#}"
            ));
            Box::new(anywhere_core::core::ranking::usage::MemoryUsage::default())
        }
    };
    let mut registry = anywhere_core::application::builtin_registry();
    let user_plugins = data.join("plugins");
    std::fs::create_dir_all(&user_plugins)?;
    let dirs = vec![PathBuf::from(&plugin_directory), user_plugins];
    let _ = PLUGIN_DIRS.set(dirs.clone());
    let reports = load_plugins(&mut registry, &dirs);
    for report in &reports {
        if !report.ok {
            diagnostics.push(format!("Plugin {}: {}", report.id, report.detail));
        }
    }
    *plugin_reports().lock().expect("plugin reports") = reports;
    #[cfg(target_os = "macos")]
    let platform = Arc::new(anywhere_platform::MacOsPlatform);
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!("The host supports macOS only");
    #[cfg(target_os = "macos")]
    APPLICATION
        .set(Application::new(platform, registry, history, diagnostics))
        .map_err(|_| anyhow::anyhow!("application already initialized"))?;
    Ok(())
}

fn application() -> Result<&'static Application> {
    APPLICATION.get().context("call initialize first")
}

/// Reserve synchronously so Dart can cancel even before foreground capture finishes.
#[flutter_rust_bridge::frb(sync)]
pub fn begin_activation() -> Result<u32> {
    application()?.begin_activation()
}

pub async fn activate(
    session_id: u32,
    prompt_accessibility: bool,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .activate_session_with(session_id, prompt_accessibility)
        .await?
        .map(Into::into))
}

pub async fn update_input(
    session_id: u32,
    input: String,
    revision: u32,
    scope_id: Option<String>,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .update_scoped_input(session_id, input, revision, scope_id)
        .await?
        .map(Into::into))
}

/// Latest committed snapshot for a session, read straight from the core.
///
/// The core is the only source of truth: a late pass publishes there, and nothing
/// here has to be kept in step with it.
pub fn latest_state(session_id: u32) -> Option<LauncherStateDto> {
    application().ok()?.latest_state(session_id).map(Into::into)
}

/// Block until the core publishes a snapshot beyond `after` for this session.
pub async fn wait_for_snapshot(session_id: u32, after: u64) -> Result<SnapshotWaitDto> {
    Ok(
        match application()?.wait_for_snapshot(session_id, after).await {
            anywhere_core::application::SnapshotWait::Updated(state) => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Updated,
                state: Some((*state).into()),
            },
            anywhere_core::application::SnapshotWait::TimedOut => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::TimedOut,
                state: None,
            },
            anywhere_core::application::SnapshotWait::Closed => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Closed,
                state: None,
            },
            anywhere_core::application::SnapshotWait::Superseded => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Superseded,
                state: None,
            },
        },
    )
}

pub async fn execute(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<ActionResultDto> {
    Ok(application()?
        .execute(session_id, revision, candidate_id)
        .await?
        .into())
}

/// Small PNG for an application candidate, or `None` when there is no icon.
pub fn application_icon(candidate_id: String) -> Result<Option<Vec<u8>>> {
    let result = application()?.application_icon(&candidate_id);
    match &result {
        Ok(Some(bytes)) => tracing::debug!(candidate_id, bytes = bytes.len(), "app_icon_served"),
        Ok(None) => tracing::info!(candidate_id, "app_icon_absent"),
        Err(error) => tracing::error!(candidate_id, error = %error, "app_icon_failed"),
    }
    result
}

pub fn deactivate(session_id: u32) -> Result<()> {
    application()?.deactivate(session_id)
}

pub fn execution_sessions() -> Result<Vec<ExecutionSessionSummaryDto>> {
    Ok(application()?
        .execution_sessions()
        .into_iter()
        .map(Into::into)
        .collect())
}

pub fn execution_session(id: String) -> Result<Option<ExecutionSessionDetailDto>> {
    Ok(application()?
        .execution_session(&id)
        .map(|detail| ExecutionSessionDetailDto {
            summary: detail.summary.into(),
            stdout: detail.stdout,
            stderr: detail.stderr,
        }))
}

pub fn stop_execution_session(id: String) -> Result<()> {
    application()?.stop_execution_session(&id)
}

pub fn remove_execution_session(id: String) -> Result<()> {
    application()?.remove_execution_session(&id)
}

pub fn shutdown_execution_sessions() -> Result<()> {
    application()?.shutdown_execution_sessions();
    Ok(())
}

#[flutter_rust_bridge::frb(sync)]
pub fn prompt_accessibility() -> Result<bool> {
    application()?.prompt_accessibility()
}

#[flutter_rust_bridge::frb(sync)]
pub fn plugin_status() -> Vec<PluginStatusDto> {
    plugin_reports()
        .lock()
        .map(|reports| reports.clone())
        .unwrap_or_default()
}

pub fn reload_plugins() -> Result<Vec<PluginStatusDto>> {
    let dirs = PLUGIN_DIRS
        .get()
        .cloned()
        .context("call initialize first")?;
    let mut registry = anywhere_core::application::builtin_registry();
    let reports = load_plugins(&mut registry, &dirs);
    *plugin_reports().lock().expect("plugin reports") = reports.clone();
    application()?.replace_registry(registry);
    Ok(reports)
}

pub fn excluded_apps() -> Result<Vec<String>> {
    Ok(application()?.excluded_apps())
}

pub fn set_excluded_apps(bundle_ids: Vec<String>) -> Result<()> {
    application()?.set_excluded_apps(bundle_ids);
    Ok(())
}

pub fn set_context_sources(
    finder_selection: bool,
    selected_text: bool,
    browser_current_page: bool,
    clipboard: bool,
) -> Result<()> {
    application()?.set_capture_sources(
        finder_selection,
        selected_text,
        browser_current_page,
        clipboard,
    );
    Ok(())
}

fn plugin_reports() -> &'static Mutex<Vec<PluginStatusDto>> {
    PLUGIN_REPORTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn load_plugins(
    registry: &mut anywhere_core::core::action::descriptor::ActionRegistry,
    dirs: &[PathBuf],
) -> Vec<PluginStatusDto> {
    let mut reports = Vec::new();
    for directory in dirs {
        let discovery = anywhere_plugin::discover(directory);
        for diagnostic in discovery.diagnostics {
            reports.push(PluginStatusDto {
                id: directory.display().to_string(),
                ok: false,
                detail: diagnostic,
            });
        }
        for manifest in discovery.manifests {
            let id = manifest.id.clone();
            let name = manifest.name.clone();
            match registry.register_plugin(manifest) {
                Ok(()) => reports.push(PluginStatusDto {
                    id,
                    ok: true,
                    detail: name,
                }),
                Err(error) => {
                    tracing::error!(plugin_id = id, error = %error, "plugin_load_failure");
                    reports.push(PluginStatusDto {
                        id,
                        ok: false,
                        detail: format!("{error:#}"),
                    });
                }
            }
        }
    }
    reports
}

// Explicit conversion keeps generated/Flutter types out of the protocol and core.
#[derive(Clone, Debug)]
pub struct LauncherStateDto {
    pub session_id: u32,
    pub revision: u32,
    pub snapshot_seq: u32,
    pub input: String,
    pub context_complete: bool,
    pub contexts: Vec<ContextDto>,
    pub context_candidates: Vec<CandidateDto>,
    pub preferred_context_key: Option<String>,
    pub scopes: Vec<ScopeDto>,
    pub selected_scope: Option<String>,
    pub context_captures: Vec<ContextCaptureDto>,
    pub candidates: Vec<CandidateDto>,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct ContextDto {
    pub id: String,
    pub type_id: String,
    pub label: String,
    pub detail: String,
    pub source: ContextSourceDto,
    pub key: String,
    pub root_key: String,
    pub icon_key: String,
    pub capabilities: Vec<String>,
    pub candidate_ids: Vec<String>,
    pub primary_candidate_id: Option<String>,
}
#[derive(Clone, Debug)]
pub struct ContextCaptureDto {
    pub source: ContextSourceDto,
    pub status: ContextCaptureStatusDto,
    pub captured_at_ms: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextCaptureStatusDto {
    Captured,
    Empty,
    PermissionDenied,
    Unsupported,
    ApplicationChanged,
    TimedOut,
    TooLarge,
    TooManyItems,
    Unavailable,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextSourceDto {
    UserInput,
    ForegroundApplication,
    ApplicationContext,
    FinderSelection,
    SelectedText,
    BrowserCurrentPage,
    Clipboard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateKindDto {
    Application,
    Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvailabilityDto {
    Enabled,
    Disabled,
    Hidden,
}

#[derive(Clone, Debug)]
pub struct ScopeDto {
    pub id: String,
    pub label: String,
    pub is_current_app: bool,
}

#[derive(Clone, Debug)]
pub struct CandidateDto {
    pub id: String,
    pub kind: CandidateKindDto,
    pub title: String,
    pub menu_path: Vec<String>,
    pub context_keys: Vec<String>,
    pub execution_context_id: Option<String>,
    pub execution_target_label: Option<String>,
    pub home_eligible: bool,
    pub primary_action: bool,
    pub section_label: Option<String>,
    pub subtitle: Option<String>,
    pub shortcut: Option<String>,
    pub icon_key: String,
    pub score: f32,
    pub group_id: String,
    pub availability: AvailabilityDto,
    pub availability_reason: Option<String>,
    pub command_id: Option<String>,
    pub bundle_id: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotWaitKindDto {
    Updated,
    TimedOut,
    Closed,
    Superseded,
}

#[derive(Clone, Debug)]
pub struct SnapshotWaitDto {
    pub kind: SnapshotWaitKindDto,
    pub state: Option<LauncherStateDto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStatusDto {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ActionResultDto {
    pub session_id: u32,
    pub revision: u32,
    pub status: ActionStatusDto,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub output_text: Option<String>,
    pub close_window: bool,
    pub execution_session_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionSessionStatusDto {
    Running,
    Stopping,
    Succeeded,
    Failed,
    Stopped,
}

#[derive(Clone, Debug)]
pub struct ExecutionSessionSummaryDto {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub command: Vec<String>,
    pub status: ExecutionSessionStatusDto,
    pub exit_code: Option<i32>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExecutionSessionDetailDto {
    pub summary: ExecutionSessionSummaryDto,
    pub stdout: String,
    pub stderr: String,
}

impl From<protocol::ExecutionSessionSummary> for ExecutionSessionSummaryDto {
    fn from(summary: protocol::ExecutionSessionSummary) -> Self {
        use protocol::ExecutionSessionStatus::*;
        Self {
            id: summary.id,
            title: summary.title,
            directory: summary.directory,
            command: summary.command,
            status: match summary.status {
                Running => ExecutionSessionStatusDto::Running,
                Stopping => ExecutionSessionStatusDto::Stopping,
                Succeeded => ExecutionSessionStatusDto::Succeeded,
                Failed => ExecutionSessionStatusDto::Failed,
                Stopped => ExecutionSessionStatusDto::Stopped,
            },
            exit_code: summary.exit_code,
            started_at_ms: summary.started_at_ms,
            finished_at_ms: summary.finished_at_ms,
            output_truncated: summary.output_truncated,
            error: summary.error,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PluginStatusDto {
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

impl From<protocol::LauncherState> for LauncherStateDto {
    fn from(state: protocol::LauncherState) -> Self {
        Self {
            session_id: state.session_id,
            revision: state.revision,
            snapshot_seq: state.snapshot_seq,
            input: state.input,
            context_complete: state.context_complete,
            contexts: state
                .context_summary
                .into_iter()
                .map(|context| ContextDto {
                    id: context.id,
                    type_id: context.type_id,
                    label: context.label,
                    detail: context.detail,
                    source: context_source_dto(context.source),
                    key: context.key,
                    root_key: context.root_key,
                    icon_key: context.icon_key,
                    capabilities: context.capabilities,
                    candidate_ids: context.candidate_ids,
                    primary_candidate_id: context.primary_candidate_id,
                })
                .collect(),
            context_candidates: state
                .context_candidates
                .into_iter()
                .map(candidate_dto)
                .collect(),
            preferred_context_key: state.preferred_context_key,
            selected_scope: state.selected_scope,
            scopes: state
                .scopes
                .into_iter()
                .map(|scope| ScopeDto {
                    id: scope.id,
                    label: scope.label,
                    is_current_app: scope.is_current_app,
                })
                .collect(),
            context_captures: state
                .context_captures
                .into_iter()
                .map(|capture| ContextCaptureDto {
                    source: context_source_dto(capture.source),
                    status: context_capture_status_dto(capture.status),
                    captured_at_ms: capture.captured_at_ms,
                })
                .collect(),
            candidates: state.candidates.into_iter().map(candidate_dto).collect(),
            diagnostics: state.diagnostics,
        }
    }
}

fn candidate_dto(candidate: protocol::CandidatePresentation) -> CandidateDto {
    CandidateDto {
        id: candidate.id,
        kind: kind_dto(candidate.kind),
        title: candidate.title,
        menu_path: candidate.menu_path,
        context_keys: candidate.context_keys,
        execution_context_id: candidate.execution_context_id,
        execution_target_label: candidate.execution_target_label,
        home_eligible: candidate.home_eligible,
        primary_action: candidate.primary_action,
        section_label: candidate.section_label,
        subtitle: candidate.subtitle,
        shortcut: candidate.shortcut,
        icon_key: candidate.icon.key,
        score: candidate.score,
        group_id: candidate.group_id,
        availability: availability_dto(candidate.availability.state),
        availability_reason: candidate.availability.reason,
        command_id: candidate.command_id,
        bundle_id: candidate.bundle_id,
    }
}

fn kind_dto(kind: protocol::CandidateKind) -> CandidateKindDto {
    match kind {
        protocol::CandidateKind::Application => CandidateKindDto::Application,
        protocol::CandidateKind::Action => CandidateKindDto::Action,
    }
}

fn context_source_dto(source: protocol::ContextSource) -> ContextSourceDto {
    match source {
        protocol::ContextSource::UserInput => ContextSourceDto::UserInput,
        protocol::ContextSource::ForegroundApplication => ContextSourceDto::ForegroundApplication,
        protocol::ContextSource::ApplicationContext => ContextSourceDto::ApplicationContext,
        protocol::ContextSource::FinderSelection => ContextSourceDto::FinderSelection,
        protocol::ContextSource::SelectedText => ContextSourceDto::SelectedText,
        protocol::ContextSource::BrowserCurrentPage => ContextSourceDto::BrowserCurrentPage,
        protocol::ContextSource::Clipboard => ContextSourceDto::Clipboard,
    }
}

fn context_capture_status_dto(status: protocol::ContextCaptureStatus) -> ContextCaptureStatusDto {
    match status {
        protocol::ContextCaptureStatus::Captured => ContextCaptureStatusDto::Captured,
        protocol::ContextCaptureStatus::Empty => ContextCaptureStatusDto::Empty,
        protocol::ContextCaptureStatus::PermissionDenied => {
            ContextCaptureStatusDto::PermissionDenied
        }
        protocol::ContextCaptureStatus::Unavailable => ContextCaptureStatusDto::Unavailable,
        protocol::ContextCaptureStatus::Unsupported => ContextCaptureStatusDto::Unsupported,
        protocol::ContextCaptureStatus::ApplicationChanged => {
            ContextCaptureStatusDto::ApplicationChanged
        }
        protocol::ContextCaptureStatus::TimedOut => ContextCaptureStatusDto::TimedOut,
        protocol::ContextCaptureStatus::TooLarge => ContextCaptureStatusDto::TooLarge,
        protocol::ContextCaptureStatus::TooManyItems => ContextCaptureStatusDto::TooManyItems,
        protocol::ContextCaptureStatus::Failed => ContextCaptureStatusDto::Failed,
    }
}

fn availability_dto(state: protocol::AvailabilityState) -> AvailabilityDto {
    match state {
        protocol::AvailabilityState::Enabled => AvailabilityDto::Enabled,
        protocol::AvailabilityState::Disabled => AvailabilityDto::Disabled,
        protocol::AvailabilityState::Hidden => AvailabilityDto::Hidden,
    }
}

impl From<protocol::ActionResult> for ActionResultDto {
    fn from(result: protocol::ActionResult) -> Self {
        Self {
            session_id: result.session_id,
            revision: result.revision,
            status: match result.status {
                protocol::ActionStatus::Succeeded => ActionStatusDto::Succeeded,
                protocol::ActionStatus::Failed => ActionStatusDto::Failed,
            },
            error: result.error,
            warning: result.warning,
            output_text: result.output_text,
            close_window: result.close_window,
            execution_session_id: result.execution_session_id,
        }
    }
}
