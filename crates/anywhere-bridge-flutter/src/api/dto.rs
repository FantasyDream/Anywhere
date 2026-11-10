//! Types crossing the Flutter bridge.
//!
//! Keeping DTO definitions together makes the public bridge surface easy to
//! audit while endpoint orchestration remains in [`super`].

pub struct ClipboardItemDto {
    pub id: String,
    pub summary: String,
    pub kind: String,
    pub copied_at_ms: i64,
    pub byte_count: u32,
}

pub enum ClipboardNoticeDto {
    FeatureDisabled,
    StorageUnavailable,
    AccessRequired,
    ItemTooLarge,
    ReadFailed,
    CaptureUnavailable,
    MonitoringUnavailable,
    Paused,
}
pub struct ClipboardHistoryDto {
    pub revision: Option<String>,
    pub recording: bool,
    pub available: bool,
    pub notice: Option<ClipboardNoticeDto>,
    pub items: Vec<ClipboardItemDto>,
}
pub struct ClipboardHistoryStatusDto {
    pub recording: bool,
    pub recording_enabled: bool,
    pub paused_until_ms: Option<i64>,
    pub retention_days: u32,
    pub stored_entries: u32,
    pub stored_bytes: u32,
    pub available: bool,
    pub notice: Option<ClipboardNoticeDto>,
}
pub struct AppHistorySourceDto {
    pub id: String,
    pub title: String,
    pub file_hint: String,
    pub path: String,
    pub mode: AppHistorySourceModeDto,
    pub action_enabled: bool,
}
pub enum AppHistorySourceModeDto {
    Automatic,
    Custom,
    Disabled,
}
pub struct AppHistoryItemDto {
    pub id: String,
    pub object_key: String,
    pub source_id: String,
    pub title: String,
    pub value: String,
    pub kind: String,
    pub can_open: bool,
}
pub enum AppHistoryNoticeDto {
    Disabled,
    Unconfigured,
    ReadFailed,
    Busy,
    Stale,
}
pub struct AppHistoryDto {
    pub items: Vec<AppHistoryItemDto>,
    pub failed_sources: Vec<String>,
    pub notice: Option<AppHistoryNoticeDto>,
}

#[flutter_rust_bridge::frb]
pub struct ShortcutActionDto {
    pub action_id: String,
    pub title: String,
    #[frb(default = "const {}")]
    pub localized_titles: std::collections::HashMap<String, String>,
    pub context_types: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct BindingResolutionDto {
    pub state: LauncherStateDto,
    pub candidates: Vec<CandidateDto>,
    pub requires_choice: bool,
}
#[derive(Clone, Debug)]
#[flutter_rust_bridge::frb]
pub struct ShortcutApplicationDto {
    pub bundle_id: String,
    pub title: String,
    #[frb(default = "const {}")]
    pub localized_titles: std::collections::HashMap<String, String>,
}
#[derive(Clone, Debug)]
pub struct ShortcutRouteDto {
    pub action_id: String,
    pub app_bundle_id: Option<String>,
    pub context_type: Option<String>,
}
#[derive(Clone, Debug)]
#[flutter_rust_bridge::frb]
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
    #[frb(default = "false")]
    pub restart_required: bool,
    pub capabilities: Vec<String>,
    pub approval_token: String,
    pub detail: String,
}
impl From<anywhere_core::application::settings_service::ModuleStatus> for ExtensionStatusDto {
    fn from(s: anywhere_core::application::settings_service::ModuleStatus) -> Self {
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
            restart_required: s.restart_required,
            capabilities: s.capabilities,
            approval_token: s.approval_token,
            detail: s.detail,
        }
    }
}
#[derive(Clone, Copy)]
pub enum FileSearchEngineDto {
    SystemIndex,
    Ripgrep,
    Combined,
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
    #[frb(default = "const {}")]
    pub localized_titles: std::collections::HashMap<String, String>,
    pub menu_path: Vec<String>,
    pub context_keys: Vec<String>,
    pub execution_context_id: Option<String>,
    pub execution_target_label: Option<String>,
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
    pub native_tool: Option<NativeToolDto>,
    pub web_plugin: Option<WebPluginLaunchDto>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostViewDto {
    FileSearch,
    NativeTool,
    PluginDetail,
    WebPlugin,
}

#[derive(Clone, Debug)]
pub struct WebPluginLaunchDto {
    pub invocation_id: String,
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

/// Qualified local resources borrowed by the platform viewer until release.
#[derive(Clone, Debug)]
pub struct NativePreviewDto {
    pub id: String,
    pub paths: Vec<String>,
    pub title: String,
}
impl From<anywhere_core::features::preview::PreviewResource> for NativePreviewDto {
    fn from(value: anywhere_core::features::preview::PreviewResource) -> Self {
        Self {
            id: value.id,
            paths: value.paths,
            title: value.title,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ActivationInputDto {
    pub source: ContextSourceDto,
    pub text: String,
    pub valid_until_ms: Option<i64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateUpdateDto {
    Replace,
    Retain,
    Append,
}
#[derive(Clone, Debug)]
pub struct NativeToolDto {
    pub id: String,
    pub input: String,
}

/// Host-only package management; never dispatched through the Web SDK.
#[derive(Clone, Debug)]
pub struct WebPackageDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub permissions: Vec<WebPermissionDto>,
    pub enabled: bool,
    pub available: bool,
    pub rollback: bool,
}
#[derive(Clone, Debug)]
pub struct WebReviewDto {
    pub token: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub permissions: Vec<WebPermissionDto>,
    pub updating: bool,
}
#[derive(Clone, Debug)]
pub struct WebDocumentDto {
    pub handle: String,
    pub origin: String,
    pub entry: String,
    pub title: String,
}
#[derive(Clone, Debug)]
pub struct WebPermissionDto {
    pub kind: WebPermissionKindDto,
    pub context_types: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebPermissionKindDto {
    ContextRead,
    CurrentApp,
    ClipboardReadText,
    ClipboardWriteText,
    LocalStorage,
    InputRead,
    InputWrite,
    FrontmostApp,
}
