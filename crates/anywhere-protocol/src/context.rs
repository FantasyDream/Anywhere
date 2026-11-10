use super::identifiers::{
    CandidateId, ContextId, ContextKey, ContextTypeId, ProviderId, Revision, SessionId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Host tool execution target; never a captured user object or a UI Scope.
pub const LAUNCHER_ENTRY_CONTEXT: &str = "core.launcher";
pub const TEXT: &str = "core.text";
pub const URL: &str = "core.url";
/// Canonical JSON context type introduced in V0.2.
pub const JSON_DOCUMENT: &str = "json.document";
/// Legacy V0.1 JSON context type. Normalized to `json.document` at the registry boundary.
pub const JSON_LEGACY: &str = "core.json";
pub const FILE: &str = "core.file";
pub const FILE_SELECTION: &str = "core.file_selection";
pub const DIRECTORY: &str = "core.directory";
pub const APPLICATION: &str = "core.application";
pub const GIT_REPOSITORY: &str = "git.repository";

pub fn is_json_context(type_id: &str) -> bool {
    type_id == JSON_DOCUMENT || type_id == JSON_LEGACY
}

/// V0.1 type ids accepted from existing plugins, mapped to their V0.2 canonical name.
pub fn canonical_context_type(type_id: &str) -> &str {
    if type_id == JSON_LEGACY {
        JSON_DOCUMENT
    } else {
        type_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    UserInput,
    ForegroundApplication,
    ApplicationContext,
    FinderSelection,
    SelectedText,
    BrowserCurrentPage,
    Clipboard,
}

/// Open type identifier + transport value. New types don't change this protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextDescriptor {
    pub id: ContextId,
    pub type_id: ContextTypeId,
    pub value: String,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    pub source: ContextSource,
    pub provider_id: ProviderId,
    pub source_context_id: Option<ContextId>,
    pub confidence: f32,
    pub depth: u8,
    pub session_id: SessionId,
    pub revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPresentation {
    pub id: ContextId,
    pub type_id: ContextTypeId,
    pub label: String,
    pub detail: String,
    pub source: ContextSource,
    pub key: ContextKey,
    pub root_key: ContextKey,
    pub icon_key: String,
    pub capabilities: Vec<String>,
    pub candidate_ids: Vec<CandidateId>,
    pub primary_candidate_id: Option<CandidateId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCaptureStatus {
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

/// How a revision obtains Context, independently of Scope filtering.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMode {
    /// Full activation graph for explicit global action resolution.
    #[default]
    CapturedEnvironment,
    Environment,
    Content,
    Search,
    /// Dedicated file workspace; no captured Context or application candidates.
    FileSearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCapturePresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Age of this exact clipboard snapshot at capture, not time of reading it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clipboard_age_ms: Option<u64>,
    pub source: ContextSource,
    pub status: ContextCaptureStatus,
    pub captured_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMatcher {
    pub type_id: ContextTypeId,
}
