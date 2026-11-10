mod application_history;
mod dto;
pub use dto::*;
mod clipboard;
mod conversions;
mod web;
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
static EXTENSIONS: OnceLock<Mutex<anywhere_core::application::settings_service::SettingsService>> =
    OnceLock::new();

pub fn ping() -> String {
    queries::ping()
}

pub fn clipboard_history_status() -> Result<ClipboardHistoryStatusDto> {
    clipboard::status()
}
pub fn clipboard_history(
    query: String,
    after: Option<String>,
) -> Result<Option<ClipboardHistoryDto>> {
    clipboard::search(query, after)
}

pub fn set_clipboard_recording(enabled: bool) -> Result<()> {
    clipboard::set_recording(enabled)
}
pub fn pause_clipboard_recording(minutes: u32) -> Result<()> {
    clipboard::pause(minutes)
}
pub fn set_clipboard_retention(days: u32) -> Result<()> {
    clipboard::set_retention(days)
}
pub fn copy_clipboard_entry(id: String) -> Result<()> {
    clipboard::copy(id)
}
pub fn delete_clipboard_entry(id: String) -> Result<()> {
    clipboard::delete(id)
}
pub fn clear_clipboard_history() -> Result<()> {
    clipboard::clear()
}

pub fn app_history_sources() -> Result<Vec<AppHistorySourceDto>> {
    application_history::sources()
}
pub fn set_app_history_action_enabled(id: String, enabled: bool) -> Result<()> {
    application_history::set_action_enabled(id, enabled)
}
pub fn app_history_custom_directory() -> Result<String> {
    application_history::custom_directory(false)
}
pub fn open_app_history_custom_directory() -> Result<()> {
    application_history::custom_directory(true).map(|_| ())
}
pub fn configure_app_history_source(
    id: String,
    path: String,
    mode: AppHistorySourceModeDto,
) -> Result<()> {
    application_history::configure(id, path, mode)
}
pub async fn app_history_search(
    session_id: u32,
    query: String,
    source: String,
) -> Result<AppHistoryDto> {
    application_history::search(session_id, query, source).await
}
pub fn open_app_history_session() -> Result<u32> {
    application_history::open()
}
pub fn cancel_app_history(session_id: u32) -> Result<()> {
    application_history::cancel(session_id)
}
pub fn use_app_history_item(id: String, copy: bool) -> Result<()> {
    application_history::use_item(id, copy)
}
pub async fn app_history_object_actions(
    session_id: u32,
    revision: u32,
    id: String,
) -> Result<Vec<CandidateDto>> {
    application_history::object_actions(session_id, revision, id).await
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

pub fn shortcut_applications() -> Result<Vec<ShortcutApplicationDto>> {
    shortcuts::shortcut_applications()
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

/// Refresh preferences and feature data; no module or plugin code is loaded.
pub fn refresh_features() -> Result<Vec<ExtensionStatusDto>> {
    settings::refresh_features()
}

pub fn extension_status() -> Result<Vec<ExtensionStatusDto>> {
    settings::extension_status()
}
pub fn set_extension_enabled(key: String, enabled: bool) -> Result<Vec<ExtensionStatusDto>> {
    settings::change_extension(
        anywhere_core::application::settings_service::SettingsChange::Enable { key, enabled },
    )
}
pub fn approve_extension(key: String, token: String) -> Result<Vec<ExtensionStatusDto>> {
    settings::change_extension(
        anywhere_core::application::settings_service::SettingsChange::Approve { key, token },
    )
}
pub fn revoke_extension(key: String) -> Result<Vec<ExtensionStatusDto>> {
    settings::change_extension(
        anywhere_core::application::settings_service::SettingsChange::Revoke { key },
    )
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

pub async fn preview_file(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<NativePreviewDto> {
    files::preview_file(session_id, revision, candidate_id).await
}
pub fn preview_clipboard_entry(id: String) -> Result<NativePreviewDto> {
    clipboard::preview(id)
}
pub fn retain_preview(id: String) -> Result<()> {
    application()?.retain_preview(&id)
}
pub fn release_preview(id: String) -> Result<()> {
    application()?.release_preview(&id)
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

pub fn web_packages() -> Result<Vec<WebPackageDto>> {
    web::statuses()
}
pub fn inspect_web_package(path: String) -> Result<WebReviewDto> {
    web::inspect(path)
}
pub fn cancel_web_review(token: String) -> Result<()> {
    web::cancel_review(token)
}
pub async fn approve_web_package(token: String) -> Result<Vec<WebPackageDto>> {
    web::approve(token)
}
pub async fn change_web_package(id: String, operation: String) -> Result<Vec<WebPackageDto>> {
    web::change(id, operation)
}
pub async fn open_web_document(
    invocation_id: String,
    resume_handle: Option<String>,
) -> Result<WebDocumentDto> {
    web::open(invocation_id, resume_handle).await
}
pub fn close_web_document(handle: String) -> Result<()> {
    web::close(handle)
}
pub async fn dispatch_web_frame(
    handle: String,
    origin: String,
    main_frame: bool,
    frame: String,
) -> Result<String> {
    web::dispatch(handle, origin, main_frame, frame).await
}
pub fn web_resource(handle: String, path: String) -> Result<Vec<u8>> {
    web::resource(handle, path)
}

/// One bounded latest-state wait, scoped to the Host-issued document handle.
pub async fn wait_web_state(handle: String, after: u64) -> Result<Option<String>> {
    web::wait_state(handle, after).await
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    fn row(i: usize) -> protocol::CandidatePresentation {
        protocol::CandidatePresentation {
            localized_titles: Default::default(),
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
            primary_action: true,
            section_label: None,
        }
    }
    #[test]
    fn localized_labels_cross_the_bridge_without_replacing_identity_or_paths() {
        let mut application = row(0);
        application.kind = protocol::CandidateKind::Application;
        application.title = "Reminders".into();
        application.localized_titles = [
            ("zh".into(), "提醒事项".into()),
            ("en".into(), "Reminders".into()),
        ]
        .into();
        application.bundle_id = Some("com.apple.reminders".into());
        let id = application.id.clone();
        let path = application.subtitle.clone();
        let dto = conversions::candidate_dto(application);
        assert_eq!(dto.localized_titles["zh"], "提醒事项");
        assert_eq!(dto.localized_titles["en"], "Reminders");
        assert_eq!(dto.id, id);
        assert_eq!(dto.subtitle, path);
        assert_eq!(dto.bundle_id.as_deref(), Some("com.apple.reminders"));
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
