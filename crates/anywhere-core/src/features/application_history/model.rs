//! Only ordinary metadata crosses the source boundary; no executable open commands.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub const MAX_ENTRIES: usize = 4096;
pub const MAX_INPUT: usize = 8 * 1024 * 1024;
pub const MAX_OUTPUT: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Directory { path: PathBuf },
    File { path: PathBuf },
    Workspace { path: PathBuf },
    Remote { uri: String },
}
impl Target {
    pub fn text(&self) -> String {
        match self {
            Self::Directory { path } | Self::File { path } | Self::Workspace { path } => {
                path.to_string_lossy().into_owned()
            }
            Self::Remote { uri } => uri.clone(),
        }
    }
    pub fn local_path(&self) -> Option<&Path> {
        match self {
            Self::Directory { path } | Self::File { path } | Self::Workspace { path } => Some(path),
            Self::Remote { .. } => None,
        }
    }
    pub fn validate(&self) -> bool {
        let text = self.text();
        !text.is_empty()
            && text.len() <= 16 * 1024
            && !text.contains('\0')
            && match self {
                Self::Remote { uri } => {
                    url::Url::parse(uri).is_ok_and(|u| matches!(u.scheme(), "vscode-remote"))
                }
                _ => self.local_path().is_some_and(Path::is_absolute),
            }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub key: String,
    pub title: String,
    pub target: Target,
    #[serde(default)]
    pub last_used_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourcePreference {
    pub id: String,
    pub path: PathBuf,
}
#[derive(Debug, Clone)]
pub struct SourceDescriptor {
    pub id: String,
    pub title: String,
    pub file_hint: String,
    pub applications: Vec<String>,
    pub action: SourceAction,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAction {
    pub title: String,
    pub localized_titles: std::collections::BTreeMap<String, String>,
    pub aliases: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageInput {
    pub source_id: String,
    pub title: String,
    pub localized_titles: std::collections::BTreeMap<String, String>,
}
pub type Cancelled = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;
pub trait SourceLoader: Send + Sync {
    fn descriptors(&self) -> Vec<SourceDescriptor>;
    fn read(&self, source: &SourcePreference, cancelled: Cancelled) -> anyhow::Result<Vec<Entry>>;
}
#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub source_id: String,
    pub entry: Entry,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Disabled,
    Unconfigured,
    ReadFailed,
    Busy,
    Stale,
}
pub struct SearchResult {
    pub items: Vec<Item>,
    pub failed_sources: Vec<String>,
    pub notice: Option<Notice>,
}
