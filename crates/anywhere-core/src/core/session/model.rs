//! The session model.
//!
//! Named `model` rather than `session` because a module may not share its parent's name.
use super::facts::SessionFacts;
use crate::core::context::ResolvedContext;
use crate::core::ranking::score::UsageStats;
use crate::core::search::candidate::Candidate;
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::*;
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct LauncherSession {
    pub state: LauncherState,
    /// Immutable ambient capture: the original frontmost application.
    pub frontmost: Option<ApplicationInfo>,
    /// Session-scoped single-flight directory capture.
    pub directory: Option<Arc<crate::features::filesystem::provider::DirectoryMemo>>,
    /// Snapshot of the real text clipboard, read once at activation.
    pub clipboard: Option<String>,
    pub git_available: bool,
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
    pub usage: HashMap<(String, String), UsageStats>,
    pub app_usage: HashMap<(String, String), UsageStats>,
    /// Committed candidates keyed by stable id, for execution.
    pub state_candidates: HashMap<ActionId, Candidate>,
    /// Revision the candidate map above was built for.
    ///
    /// A new revision is current from the moment its input arrives, but its candidates
    /// only exist once `commit` builds them. Without this an execute request that named
    /// the new revision could still be served from the previous revision's candidates,
    /// which carry the previous revision's context and execution target.
    pub candidates_revision: Option<Revision>,
}

impl LauncherSession {
    /// The read-only environment an availability rule may inspect.
    pub fn facts(&self) -> SessionFacts {
        SessionFacts {
            clipboard_text: self.clipboard.clone(),
            git_available: self.git_available,
            frontmost: self.frontmost.clone(),
        }
    }

    /// The application whose usage history this session reads and writes.
    pub fn history_bucket(&self) -> &str {
        &self.source_app
    }
}
