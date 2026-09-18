//! Application-owned state for one launcher activation.
use crate::core::context::ResolvedContext;
use crate::core::ranking::score::UsageStats;
use crate::core::search::candidate::Candidate;
use crate::core::session::facts::{FactValue, SessionFacts};
use anyhow::Result;
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::*;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

#[derive(Default)]
pub struct LauncherSession {
    pub state: LauncherState,
    pub activation_started: bool,
    /// Session identity: who was in front when we opened.
    pub frontmost: Option<ApplicationInfo>,
    /// Session-scoped single-flight directory capture.
    pub directory: Option<Arc<crate::features::filesystem::provider::DirectoryMemo>>,
    /// Environment snapshots for availability (clipboard, git, …).
    pub environment: BTreeMap<String, FactValue>,
    /// Contexts reusable across revisions without repeating OS work.
    pub context_cache: Vec<ResolvedContext>,
    pub committed_revision: Option<Revision>,
    /// Contexts the committed revision published, for a later re-match against a source
    /// that arrived late (the application index) without re-resolving anything.
    pub committed_contexts: Vec<ResolvedContext>,
    /// Context rows already published for the committed revision, for late counts.
    pub published_contexts: usize,
    pub snapshot_seq: u32,
    pub source_app: String,
    pub usage: Arc<HashMap<(String, String), UsageStats>>,
    pub app_usage: Arc<HashMap<(String, String), UsageStats>>,
    /// Committed candidates keyed by stable id, for execution.
    pub state_candidates: HashMap<ActionId, Candidate>,
    /// Revision the candidate map above was built for.
    ///
    /// A new revision is current from the moment its input arrives, but its candidates
    /// only exist once `commit` builds them. Without this an execute request that named
    /// the new revision could still be served from the previous revision's candidates,
    /// which carry the previous revision's context and execution target.
    pub candidates_revision: Option<Revision>,
    /// Scope ids in first-seen order for this session. Hidden groups keep their slot.
    pub frozen_scope_ids: Vec<ScopeId>,
}

impl LauncherSession {
    pub fn record_fact(&mut self, key: impl Into<String>, value: FactValue) -> Result<()> {
        let key = key.into();
        crate::core::session::facts::validate_write(&key, &value)?;
        self.environment.insert(key, value);
        Ok(())
    }

    /// The read-only environment an availability rule may inspect.
    pub fn facts(&self) -> SessionFacts {
        SessionFacts::from_map(self.frontmost.clone(), self.environment.clone())
            .expect("session facts are validated at capture")
    }
}
