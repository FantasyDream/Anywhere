//! Application-owned state for one launcher activation.
use crate::core::context::ResolvedContext;
use crate::core::ranking::usage::UsageSnapshot;
use crate::core::search::candidate::Candidate;
use crate::core::session::facts::{FactValue, SessionFacts};
use anyhow::Result;
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

#[derive(Default)]
pub struct SessionIdentity {
    pub clipboard_age_ms: Option<u64>,
    pub frontmost: Option<ApplicationInfo>,
    pub source_app: String,
    /// One-shot activation reads. They survive query revisions but never a new session.
    pub captures: Vec<ActivationCapture>,
}

pub use anywhere_protocol::ContextCaptureStatus as CaptureStatus;

#[derive(Debug, Clone)]
pub struct ActivationCapture {
    pub source: ContextSource,
    pub captured_at_ms: i64,
    pub status: CaptureStatus,
    pub contexts: Vec<ResolvedContext>,
}

#[derive(Default)]
pub struct SessionResolution {
    /// Immutable snapshots share this allocation; only file-result commits take
    /// a mutable copy, and only when this Arc is still observed elsewhere.
    pub search_candidates: Arc<Vec<Candidate>>,
    /// Incremental deduplication set for displayed file paths so pagination never
    /// rescans and clones every displayed `PathBuf` on each page append.
    pub displayed_file_paths: HashSet<PathBuf>,
    pub object_actions: Vec<Candidate>,
    pub last_object_selection: u64,
    pub object_action_epoch: Arc<std::sync::atomic::AtomicU64>,
    pub source_candidates: BTreeMap<String, Vec<Candidate>>,
    pub search_sources: Vec<SearchSourcePresentation>,
    pub file_search: Option<FileSearchProgress>,
    pub file_results_frozen: bool,
    pub context_cache: Vec<ResolvedContext>,
    pub committed_revision: Option<Revision>,
    pub committed_contexts: Vec<ResolvedContext>,
    pub published_contexts: usize,
    pub snapshot_seq: u32,
    pub frozen_scope_ids: Vec<GroupId>,
    /// Facts and usage copied for a snapshot. A commit retries when this changes.
    pub inputs_epoch: u64,
}

#[derive(Default)]
pub struct SessionExecution {
    pub candidates: HashMap<crate::core::action::identity::ExecutionCandidateId, Candidate>,
    pub file_candidates: Vec<Candidate>,
    pub file_candidate_indices: HashMap<crate::core::action::identity::ExecutionCandidateId, usize>,
    pub revision: Option<Revision>,
    pub action_catalog_version: Option<u64>,
}

#[derive(Default)]
pub struct SessionMenu {
    pub generation: u64,
    pub trusted: bool,
}

#[derive(Default)]
pub struct LauncherSession {
    pub input_mode: InputMode,
    pub file_filter: FileSearchFilter,
    pub state: LauncherState,
    pub activation_started: bool,
    pub suspended: bool,
    pub identity: SessionIdentity,
    pub environment: BTreeMap<String, FactValue>,
    pub resolution: SessionResolution,
    pub execution: SessionExecution,
    pub menu: SessionMenu,
    pub usage: Arc<UsageSnapshot>,
    pub app_usage: Arc<UsageSnapshot>,
    pub actions: Arc<crate::core::action::descriptor::ActionRegistry>,
    /// Latest immutable published snapshot shared with `latest_tx` and callers
    /// without re-cloning `LauncherState` on every publication.
    pub published_state: Option<Arc<LauncherState>>,
    /// Clipboard, git availability and history are applied after the first snapshot.
    pub supplemental_pending: bool,
}

impl LauncherSession {
    pub fn reserved(
        session_id: SessionId,
        actions: Arc<crate::core::action::descriptor::ActionRegistry>,
    ) -> Self {
        Self {
            state: LauncherState {
                session_id,
                ..Default::default()
            },
            actions,
            ..Default::default()
        }
    }

    pub fn begin_revision(&mut self, revision: Revision, input: String) {
        self.resolution
            .object_action_epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        // A resolver may still be reading the old revision. Start with a fresh
        // list so cancellation never copies those candidates just to discard them.
        self.resolution.search_candidates = Arc::default();
        self.execution.file_candidates = Vec::new();
        self.execution.file_candidate_indices.clear();
        self.resolution.displayed_file_paths.clear();
        self.resolution.object_actions.clear();
        self.resolution.source_candidates.clear();
        self.resolution.search_sources.clear();
        self.resolution.file_search = None;
        self.resolution.file_results_frozen = false;
        self.published_state = None;
        self.state.revision = revision;
        self.state.input = input;
        self.state.context_complete = false;
        self.resolution.committed_revision = None;
        self.resolution.snapshot_seq = 0;
        self.resolution.published_contexts = 0;
    }

    pub fn invalidate_execution(&mut self) {
        // End an activation, rather than merely emptying retained allocations.
        let id = self.state.session_id;
        let actions = self.actions.clone();
        *self = Self::reserved(id, actions);
    }

    pub fn record_fact(&mut self, key: impl Into<String>, value: FactValue) -> Result<()> {
        let key = key.into();
        crate::core::session::facts::validate_write(&key, &value)?;
        self.environment.insert(key, value);
        self.note_snapshot_inputs();
        Ok(())
    }

    pub fn record_capture(&mut self, capture: ActivationCapture) {
        self.identity
            .captures
            .retain(|held| held.source != capture.source);
        self.identity.captures.push(capture);
        self.resolution.inputs_epoch = self.resolution.inputs_epoch.saturating_add(1);
    }

    pub fn replace_usage(&mut self, usage: Arc<UsageSnapshot>, app_usage: Arc<UsageSnapshot>) {
        self.usage = usage;
        self.app_usage = app_usage;
        self.note_snapshot_inputs();
    }

    pub(super) fn note_snapshot_inputs(&mut self) {
        self.resolution.inputs_epoch = self.resolution.inputs_epoch.saturating_add(1);
    }

    pub fn facts(&self) -> SessionFacts {
        SessionFacts::from_map(self.identity.frontmost.clone(), self.environment.clone())
            .expect("session facts are validated at capture")
    }

    pub fn execution_candidate(&self, id: &str) -> Option<&Candidate> {
        self.execution.candidates.get(id).or_else(|| {
            self.execution
                .file_candidate_indices
                .get(id)
                .and_then(|index| self.execution.file_candidates.get(*index))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::filesystem::search::file_candidates;
    use anywhere_platform::FileSearchHit;
    use std::path::PathBuf;

    #[test]
    fn new_revision_releases_the_previous_search_allocation() {
        let mut session = LauncherSession::default();
        for shared in [false, true] {
            session.resolution.search_candidates = Arc::new(Vec::with_capacity(512));
            let reader = shared.then(|| session.resolution.search_candidates.clone());
            session.begin_revision(2, String::new());
            assert_eq!(session.resolution.search_candidates.capacity(), 0);
            if let Some(old) = reader {
                assert!(old.capacity() >= 512);
            }
        }
    }

    #[test]
    fn file_execution_indices_resolve_the_published_candidate() {
        let registry = crate::application::builtin_registry();
        let rows = file_candidates(
            (0..3)
                .map(|index| FileSearchHit {
                    path: PathBuf::from(format!("/synthetic/file-{index}.txt")),
                    is_directory: false,
                })
                .collect(),
            "",
            &registry,
        );
        let expected = rows[0].presentation.id.clone();
        let mut session = LauncherSession::reserved(1, Arc::new(registry));
        session.execution.file_candidate_indices = rows
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.presentation.id.clone().into(), index))
            .collect();
        session.execution.file_candidates = rows;

        assert_eq!(
            session
                .execution_candidate(&expected)
                .unwrap()
                .presentation
                .id,
            expected
        );
        assert!(session.execution.candidates.is_empty());
    }
}
