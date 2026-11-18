use anywhere_core::features::{
    application_history::model::{Notice as HistoryNotice, SourceMode},
    clipboard::service::HistoryNotice as ClipboardNotice,
};
use anywhere_protocol::ExecutionSessionStatus;
use gpui_kit::SharedString;
use std::path::PathBuf;

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
    pub storage_usage: Option<(u32, u32)>,
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
pub struct AppHistoryObjectAction {
    pub id: String,
    pub title: String,
    pub localized_titles: std::collections::BTreeMap<String, String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutCatalogAction {
    pub action_id: String,
    pub title: String,
    pub localized_titles: std::collections::BTreeMap<String, String>,
    pub context_types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutCandidate {
    pub candidate_id: String,
    pub title: String,
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
    pub output_revision: i64,
    pub exit_code: Option<i32>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionDetail {
    pub row: ExecutionRow,
    pub stdout: SharedString,
    pub stderr: SharedString,
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
