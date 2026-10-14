mod conversions;
use conversions::*;
mod executions;
mod files;
mod queries;
mod settings;
mod shortcuts;
use anyhow::{Context, Result};
use anywhere_core::{persistence::usage::SqliteUsageRepository, Application, LauncherApplication};
use anywhere_protocol as protocol;
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

static APPLICATION: OnceLock<Application> = OnceLock::new();
static EXTENSIONS: OnceLock<
    Mutex<anywhere_core::application::extension_manager::ExtensionManager>,
> = OnceLock::new();

pub fn ping() -> String {
    queries::ping()
}

pub struct ShortcutActionDto {
    pub action_id: String,
    pub title: String,
    pub context_types: Vec<String>,
}

pub async fn supported_shortcut_actions(action_ids: Vec<String>) -> Result<Vec<String>> {
    shortcuts::supported_shortcut_actions(action_ids).await
}

pub async fn shortcut_actions() -> Result<Vec<ShortcutActionDto>> {
    shortcuts::shortcut_actions().await
}

pub async fn resolve_action_binding(
    session_id: u32,
    revision: u32,
    action_id: String,
) -> Result<Vec<CandidateDto>> {
    shortcuts::resolve_action_binding(session_id, revision, action_id).await
}

#[derive(Clone, Debug)]
pub struct BindingResolutionDto {
    pub state: LauncherStateDto,
    pub candidates: Vec<CandidateDto>,
    pub requires_choice: bool,
}

/// Fresh global activation only: wait for capture and retain exact execution targets.
pub async fn wait_action_binding(
    session_id: u32,
    revision: u32,
    action_id: String,
) -> Result<BindingResolutionDto> {
    shortcuts::wait_action_binding(session_id, revision, action_id).await
}

pub fn initialize(data_directory: String, plugin_directory: String) -> Result<()> {
    settings::initialize(data_directory, plugin_directory)
}

fn application() -> Result<&'static Application> {
    APPLICATION.get().context("call initialize first")
}

#[derive(Clone, Debug)]
pub struct ShortcutApplicationDto {
    pub bundle_id: String,
    pub title: String,
}
pub fn shortcut_applications() -> Result<Vec<ShortcutApplicationDto>> {
    shortcuts::shortcut_applications()
}
#[derive(Clone, Debug)]
pub struct ShortcutRouteDto {
    pub action_id: String,
    pub app_bundle_id: Option<String>,
    pub context_type: Option<String>,
}

pub fn resolve_shortcut_intent(
    session_id: u32,
    revision: u32,
    routes: Vec<ShortcutRouteDto>,
) -> Result<BindingResolutionDto> {
    shortcuts::resolve_shortcut_intent(session_id, revision, routes)
}
pub async fn wait_shortcut_intent(
    session_id: u32,
    revision: u32,
    routes: Vec<ShortcutRouteDto>,
) -> Result<BindingResolutionDto> {
    shortcuts::wait_shortcut_intent(session_id, revision, routes).await
}

/// Reserve synchronously so Dart can cancel even before foreground capture finishes.
#[flutter_rust_bridge::frb(sync)]
pub fn begin_activation() -> Result<u32> {
    queries::begin_activation()
}

pub async fn activate(
    session_id: u32,
    prompt_accessibility: bool,
    captured_environment: bool,
) -> Result<Option<LauncherStateDto>> {
    queries::activate(session_id, prompt_accessibility, captured_environment).await
}

pub async fn update_input(
    session_id: u32,
    input: String,
    revision: u32,
    scope_id: Option<String>,
    input_mode: InputModeDto,
) -> Result<Option<LauncherStateDto>> {
    queries::update_input(session_id, input, revision, scope_id, input_mode).await
}

/// File filters are part of the initiating query, before enumeration/page limits.
pub async fn update_file_input(
    session_id: u32,
    input: String,
    revision: u32,
    filter: String,
) -> Result<Option<LauncherStateDto>> {
    files::update_file_input(session_id, input, revision, filter).await
}

/// Resolve secondary actions for the selected object through the shared host flow.
pub fn object_actions(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<Vec<CandidateDto>> {
    files::file_actions(session_id, revision, candidate_id)
}

pub fn file_actions(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<Vec<CandidateDto>> {
    files::file_actions(session_id, revision, candidate_id)
}

pub async fn load_more_files(session_id: u32, revision: u32) -> Result<Option<LauncherStateDto>> {
    files::load_more_files(session_id, revision).await
}

pub fn freeze_file_results(session_id: u32, revision: u32) -> Result<()> {
    files::freeze_file_results(session_id, revision)
}

/// Latest committed snapshot for a session, read straight from the core.
///
/// The core is the only source of truth: a late pass publishes there, and nothing
/// here has to be kept in step with it.
pub fn latest_state(session_id: u32) -> Option<LauncherStateDto> {
    queries::latest_state(session_id)
}

/// Block until the core publishes a snapshot beyond `after` for this session.
pub async fn wait_for_snapshot(session_id: u32, after: u64) -> Result<SnapshotWaitDto> {
    queries::wait_for_snapshot(session_id, after).await
}

pub async fn execute(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<ActionResultDto> {
    queries::execute(session_id, revision, candidate_id).await
}

/// Small PNG for an application candidate, or `None` when there is no icon.
pub fn application_icon(candidate_id: String) -> Result<Option<Vec<u8>>> {
    queries::application_icon(candidate_id)
}

pub async fn suspend_launcher(session_id: u32) -> Result<()> {
    queries::suspend_launcher(session_id).await
}

pub fn resume_launcher(session_id: u32) -> Result<Option<LauncherStateDto>> {
    queries::resume_launcher(session_id)
}

pub fn deactivate(session_id: u32) -> Result<()> {
    queries::deactivate(session_id)
}

pub fn execution_sessions() -> Result<Vec<ExecutionSessionSummaryDto>> {
    executions::execution_sessions()
}

pub fn execution_session(id: String) -> Result<Option<ExecutionSessionDetailDto>> {
    executions::execution_session(id)
}

pub fn stop_execution_session(id: String) -> Result<()> {
    executions::stop_execution_session(id)
}

pub fn remove_execution_session(id: String) -> Result<()> {
    executions::remove_execution_session(id)
}

pub fn shutdown_execution_sessions() -> Result<()> {
    executions::shutdown_execution_sessions()
}

#[flutter_rust_bridge::frb(sync)]
pub fn prompt_accessibility() -> Result<bool> {
    queries::prompt_accessibility()
}

#[flutter_rust_bridge::frb(sync)]
pub fn plugin_status() -> Vec<PluginStatusDto> {
    settings::plugin_status()
}

pub fn reload_plugins() -> Result<Vec<PluginStatusDto>> {
    settings::reload_plugins()
}

/// Modern reload returns the same authoritative catalog as enable/grant/revoke.
pub fn reload_extensions() -> Result<Vec<ExtensionStatusDto>> {
    settings::reload_extensions()
}

pub fn extension_status() -> Result<Vec<ExtensionStatusDto>> {
    settings::extension_status()
}
pub fn set_extension_enabled(key: String, enabled: bool) -> Result<Vec<ExtensionStatusDto>> {
    settings::change_extension(
        anywhere_core::application::extension_manager::ExtensionChange::Enable { key, enabled },
    )
}
pub fn approve_extension(key: String, token: String) -> Result<Vec<ExtensionStatusDto>> {
    settings::change_extension(
        anywhere_core::application::extension_manager::ExtensionChange::Approve { key, token },
    )
}
pub fn revoke_extension(key: String) -> Result<Vec<ExtensionStatusDto>> {
    settings::change_extension(
        anywhere_core::application::extension_manager::ExtensionChange::Revoke { key },
    )
}

#[derive(Clone, Debug)]
pub struct ExtensionStatusDto {
    pub key: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub first_party: bool,
    pub legacy: bool,
    pub enabled: bool,
    pub active: bool,
    pub needs_approval: bool,
    pub configurable: bool,
    pub capabilities: Vec<String>,
    pub approval_token: String,
    pub detail: String,
}
impl From<anywhere_core::application::extension_manager::ExtensionStatus> for ExtensionStatusDto {
    fn from(s: anywhere_core::application::extension_manager::ExtensionStatus) -> Self {
        Self {
            key: s.key,
            id: s.id,
            name: s.name,
            version: s.version,
            first_party: s.first_party,
            legacy: s.legacy,
            enabled: s.enabled,
            active: s.active,
            needs_approval: s.needs_approval,
            configurable: s.configurable,
            capabilities: s.capabilities,
            approval_token: s.approval_token,
            detail: s.detail,
        }
    }
}

pub fn excluded_apps() -> Result<Vec<String>> {
    settings::excluded_apps()
}

pub fn set_excluded_apps(bundle_ids: Vec<String>) -> Result<()> {
    settings::set_excluded_apps(bundle_ids)
}

pub fn set_file_search(
    enabled: bool,
    roots: Vec<String>,
    excluded_directory_names: Vec<String>,
    engine: FileSearchEngineDto,
    include_hidden: bool,
    restrict_to_roots: bool,
) -> Result<()> {
    settings::set_file_search(
        enabled,
        roots,
        excluded_directory_names,
        engine,
        include_hidden,
        restrict_to_roots,
    )
}

#[derive(Clone, Copy)]
pub enum FileSearchEngineDto {
    SystemIndex,
    Ripgrep,
    Combined,
}

pub fn set_context_sources(
    finder_selection: bool,
    selected_text: bool,
    browser_current_page: bool,
    clipboard: bool,
) -> Result<()> {
    settings::set_context_sources(
        finder_selection,
        selected_text,
        browser_current_page,
        clipboard,
    )
}

// Explicit conversion keeps generated/Flutter types out of the protocol and core.
#[derive(Clone, Debug)]
#[flutter_rust_bridge::frb]
pub struct LauncherStateDto {
    pub session_id: u32,
    pub revision: u32,
    pub snapshot_seq: u32,
    #[frb(default = "0")]
    pub candidate_list_version: u32,
    pub input: String,
    pub context_complete: bool,
    pub search_sources: Vec<SearchSourceDto>,
    pub file_search: Option<FileSearchProgressDto>,
    pub result_actions: Vec<CandidateDto>,
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
pub struct FileSearchProgressDto {
    pub query_id: u64,
    pub roots: Vec<String>,
    pub scanned_entries: u64,
    pub confirmed_matches: u64,
    pub finished: bool,
    pub issue_count: u64,
    pub inaccessible_count: u64,
    pub issue_examples: Vec<String>,
    pub next_cursor: Option<String>,
    pub previous_cursor: Option<String>,
    pub page: u32,
    pub needs_refresh: bool,
}

#[derive(Clone, Debug)]
pub struct SearchSourceDto {
    pub id: String,
    pub state: SearchSourceStateDto,
}
#[derive(Clone, Copy, Debug)]
pub enum SearchSourceStateDto {
    Loading,
    Ready,
    Unavailable,
    Partial,
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
    pub text: Option<String>,
    pub clipboard_age_ms: Option<u64>,
    pub source: ContextSourceDto,
    pub status: ContextCaptureStatusDto,
    pub captured_at_ms: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputModeDto {
    Environment,
    Content,
    Search,
    FileSearch,
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
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvailabilityDto {
    Enabled,
    Disabled,
    Hidden,
}

#[derive(Clone, Debug)]
#[flutter_rust_bridge::frb]
pub struct ScopeDto {
    pub id: String,
    pub label: String,
    pub is_current_app: bool,
    #[frb(default = "const []")]
    pub action_ids: Vec<String>,
    #[frb(default = "const []")]
    pub menu_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileObjectKindDto {
    File,
    Directory,
}

#[derive(Clone, Debug)]
#[flutter_rust_bridge::frb]
pub struct CandidateDto {
    pub id: String,
    pub kind: CandidateKindDto,
    #[frb(default = "false")]
    pub is_menu: bool,
    pub file_kind: Option<FileObjectKindDto>,
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
#[flutter_rust_bridge::frb]
pub struct SnapshotWaitDto {
    pub kind: SnapshotWaitKindDto,
    #[frb(default = "CandidateUpdateDto.replace")]
    pub candidate_update: CandidateUpdateDto,
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
    pub view: Option<HostViewDto>,
    pub plugin_detail: Option<PluginDetailDto>,
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
    pub output_revision: i64,
    pub output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExecutionSessionDetailDto {
    pub summary: ExecutionSessionSummaryDto,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug)]
pub struct PluginStatusDto {
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostViewDto {
    FileSearch,
    PluginDetail,
}

#[derive(Clone, Debug)]
pub struct PluginDetailDto {
    pub schema_version: u32,
    pub title: String,
    pub body: Option<String>,
    pub rows: Vec<PluginKeyValueDto>,
}

#[derive(Clone, Debug)]
pub struct PluginKeyValueDto {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct FilePreviewDto {
    pub path: String,
    pub is_directory: bool,
    pub size_bytes: u64,
    pub modified_ms: Option<i64>,
    pub text: Option<String>,
    pub notice: Option<String>,
}

pub async fn preview_file(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<FilePreviewDto> {
    files::preview_file(session_id, revision, candidate_id).await
}

#[derive(Clone, Debug)]
pub struct ActivationInputDto {
    pub source: ContextSourceDto,
    pub text: String,
    pub valid_until_ms: Option<i64>,
}
/// Recognition runs on the bridge worker; filesystem checks never block Host UI.
pub fn choose_activation_input(
    captures: Vec<ContextCaptureDto>,
    now_ms: i64,
    selected_enabled: bool,
    clipboard_enabled: bool,
    allow_pending_selection: bool,
) -> Option<ActivationInputDto> {
    queries::choose_activation_input(
        captures,
        now_ms,
        selected_enabled,
        clipboard_enabled,
        allow_pending_selection,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateUpdateDto {
    Replace,
    Retain,
    Append,
}
/// A client missing a list replacement receives a full snapshot. Stable list
/// generations permit a bounded tail transfer; progress-only updates send no rows.
pub async fn wait_for_snapshot_delta(
    session_id: u32,
    after: u64,
    candidate_list_version: u32,
    loaded_count: u32,
) -> Result<SnapshotWaitDto> {
    queries::wait_for_snapshot_delta(session_id, after, candidate_list_version, loaded_count).await
}
fn snapshot_delivery(
    mut state: protocol::LauncherState,
    version: u32,
    count: u32,
) -> SnapshotWaitDto {
    let update = if state.file_search.is_some()
        && version != 0
        && version == state.candidate_list_version
        && count as usize <= state.candidates.len()
    {
        let tail = state.candidates[count as usize..].to_vec();
        let kind = if tail.is_empty() {
            CandidateUpdateDto::Retain
        } else {
            CandidateUpdateDto::Append
        };
        state.candidates = std::sync::Arc::new(tail);
        kind
    } else {
        CandidateUpdateDto::Replace
    };
    SnapshotWaitDto {
        kind: SnapshotWaitKindDto::Updated,
        state: Some(state.into()),
        candidate_update: update,
    }
}

pub async fn load_more_files_delta(
    session_id: u32,
    revision: u32,
    candidate_list_version: u32,
    loaded_count: u32,
) -> Result<SnapshotWaitDto> {
    files::load_more_files_delta(session_id, revision, candidate_list_version, loaded_count).await
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    fn row(i: usize) -> protocol::CandidatePresentation {
        protocol::CandidatePresentation {
            id: format!("row-{i}"),
            kind: protocol::CandidateKind::File,
            is_menu: false,
            file_kind: Some(protocol::FileObjectKind::File),
            title: format!("report-{i}.txt"),
            subtitle: Some(format!("/fixture/{}", "directory/".repeat(24))),
            shortcut: None,
            icon: protocol::IconDescriptor { key: "file".into() },
            score: 0.0,
            group_id: "files".into(),
            availability: protocol::Availability::enabled(),
            command_id: Some("core.file.open".into()),
            bundle_id: None,
            owning_app: None,
            menu_path: vec![],
            context_keys: vec![],
            execution_context_id: None,
            execution_target_label: None,
            home_eligible: false,
            primary_action: true,
            section_label: None,
        }
    }
    #[test]
    fn large_snapshots_deliver_only_progress_or_new_rows_and_resync_missed_replacements() {
        for count in [1000, 10000] {
            let state = protocol::LauncherState {
                candidate_list_version: 3,
                candidates: (0..count).map(row).collect::<Vec<_>>().into(),
                file_search: Some(Box::default()),
                ..Default::default()
            };
            let start = std::time::Instant::now();
            for _ in 0..100 {
                let packet = snapshot_delivery(state.clone(), 3, count as u32);
                assert_eq!(packet.candidate_update, CandidateUpdateDto::Retain);
                assert!(packet.state.unwrap().candidates.is_empty());
            }
            let delta_us = start.elapsed().as_micros();
            let start = std::time::Instant::now();
            for _ in 0..100 {
                let full: LauncherStateDto = state.clone().into();
                assert_eq!(full.candidates.len(), count);
            }
            let full_us = start.elapsed().as_micros();
            println!("{count} rows / 100 DTO updates: full={full_us}us, metadata={delta_us}us; metadata transfers 0 rows");
            let tail = snapshot_delivery(state.clone(), 3, (count - 64) as u32);
            assert_eq!(tail.candidate_update, CandidateUpdateDto::Append);
            assert_eq!(tail.state.unwrap().candidates.len(), 64);
            for (version, length) in [(2, count as u32), (3, count as u32 + 1)] {
                let replacement = snapshot_delivery(state.clone(), version, length);
                assert_eq!(replacement.candidate_update, CandidateUpdateDto::Replace);
                assert_eq!(replacement.state.unwrap().candidates.len(), count);
            }
        }
    }
}
