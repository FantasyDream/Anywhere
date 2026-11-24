use super::{
    candidate::{CandidatePresentation, Scope, ScopeEntry},
    context::{ContextCapturePresentation, ContextPresentation},
    identifiers::{ContextKey, GroupId, Revision, SessionId},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchSourceState {
    Loading,
    Ready,
    Unavailable,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchSourcePresentation {
    pub id: String,
    pub state: SearchSourceState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSearchProgress {
    pub query_id: u64,
    #[serde(default)]
    pub roots: Vec<String>,
    pub scanned_entries: u64,
    pub confirmed_matches: u64,
    pub finished: bool,
    pub issue_count: u64,
    #[serde(default)]
    pub inaccessible_count: u64,
    pub issue_examples: Vec<String>,
    pub next_cursor: Option<String>,
    pub previous_cursor: Option<String>,
    pub page: u32,
    pub needs_refresh: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LauncherState {
    pub session_id: SessionId,
    pub revision: Revision,
    /// Monotonic within a session/revision; a higher value always supersedes a lower one.
    #[serde(default)]
    pub snapshot_seq: u32,
    pub input: String,
    /// True once the context graph for this revision has no pending work.
    #[serde(default)]
    pub context_complete: bool,
    #[serde(default)]
    pub search_sources: Vec<SearchSourcePresentation>,
    #[serde(default)]
    pub file_search: Option<Box<FileSearchProgress>>,
    #[serde(default)]
    pub result_actions: Arc<Vec<CandidatePresentation>>,
    pub context_summary: Vec<ContextPresentation>,
    #[serde(default)]
    pub context_candidates: Arc<Vec<CandidatePresentation>>,
    pub preferred_context_key: Option<ContextKey>,
    #[serde(default)]
    pub scopes: Vec<ScopeEntry>,
    pub selected_scope: Option<GroupId>,
    #[serde(default)]
    pub context_captures: Vec<ContextCapturePresentation>,
    pub candidates: Arc<Vec<CandidatePresentation>>,
    /// Stable across append/progress; changes when the object list is replaced.
    #[serde(default)]
    pub candidate_list_version: u32,
    pub diagnostics: Vec<String>,
}

impl LauncherState {
    pub fn candidate(&self, id: &str) -> Option<&CandidatePresentation> {
        self.candidates
            .iter()
            .chain(self.context_candidates.iter())
            .find(|candidate| candidate.id == id)
    }
    /// Missing groups remain empty; they never widen to Universal.
    pub fn scoped_candidates(&self, scope: &Scope) -> Vec<&CandidatePresentation> {
        match scope {
            Scope::Universal => self
                .candidates
                .iter()
                .filter(|candidate| candidate.availability.is_visible())
                .collect(),
            Scope::Group(key) => match self.scopes.iter().find(|entry| entry.id == *key) {
                Some(entry) => self
                    .context_candidates
                    .iter()
                    .filter(|candidate| entry.contains(candidate))
                    .collect(),
                None => Vec::new(),
            },
        }
    }
}
