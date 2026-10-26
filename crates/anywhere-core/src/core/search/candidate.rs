//! Candidate construction: the shape of one row, and how rows are compared.
//!
//! Ordering lives here because it is a property of candidates; *deciding* the order is
//! ranking's job, and *finding* them is the sources' job.
use crate::core::action::descriptor::ActionDescriptor;
use crate::core::context::ResolvedContext;
use crate::core::ranking::usage::UsageStats;
use anywhere_protocol::*;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A candidate and everything needed to present or execute it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub presentation: CandidatePresentation,
    pub target: CandidateTarget,
    /// Object offers may expire independently of an Action's stable definition.
    pub guard: Option<Arc<dyn CandidateGuard>>,
    /// In-memory ranking data only; never exposed as UI policy or persisted here.
    pub usage: UsageStats,
}

/// Rechecked on the blocking execution worker, including before a planned spawn.
/// Implementations validate caller-owned data; the search engine knows no feature.
pub trait CandidateGuard: std::fmt::Debug + Send + Sync {
    fn validate(&self) -> anyhow::Result<()>;
}

/// Execution data for the two candidate types the product currently supports.
/// Keep this typed until another real execution model requires a wider contract.
#[derive(Debug, Clone)]
pub enum CandidateTarget {
    Action {
        action: Arc<ActionDescriptor>,
        context: Arc<ResolvedContext>,
    },
    LaunchApplication {
        path: String,
        icon_source: String,
    },
}

impl CandidateTarget {
    pub fn uses_usage(&self) -> bool {
        match self {
            Self::Action { action, .. } => action.executor.uses_usage(),
            Self::LaunchApplication { .. } => true,
        }
    }
    pub fn records_usage(&self) -> bool {
        match self {
            Self::Action { action, .. } => action.executor.records_usage(),
            Self::LaunchApplication { .. } => true,
        }
    }

    pub fn execution_token(&self) -> Option<u64> {
        match self {
            Self::Action { action, .. } => action.executor.execution_token(),
            Self::LaunchApplication { .. } => None,
        }
    }

    pub fn from_shared_action(action: Arc<ActionDescriptor>, context: ResolvedContext) -> Self {
        Self::Action {
            action,
            context: Arc::new(context),
        }
    }

    pub fn from_action(action: ActionDescriptor, context: ResolvedContext) -> Self {
        Self::Action {
            action: Arc::new(action),
            context: Arc::new(context),
        }
    }
}

impl Candidate {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.guard.as_ref().map_or(Ok(()), |guard| guard.validate())
    }

    pub fn context(&self) -> Option<&ResolvedContext> {
        match &self.target {
            CandidateTarget::Action { context, .. } => Some(context),
            CandidateTarget::LaunchApplication { .. } => None,
        }
    }
}

/// Everything the search produced for this revision.
pub struct CandidateSet {
    pub candidates: Vec<Candidate>,
    /// Action matching: applicability, availability and query match per context.
    pub matcher_us: u64,
    /// Every ranked source after the first (application index today, files later).
    pub sources_us: u64,
    /// Scoring and ordering of the candidates that were matched.
    pub ranking_us: u64,
}

/// Stable business identity: survives re-ranking, renumbering and late enrichment.
pub fn action_candidate_id(command: &str, context: &ResolvedContext) -> String {
    format!(
        "action:{command}:{}:{:016x}",
        context.type_id,
        context.value.identity_fingerprint()
    )
}

/// Stable for re-ranking this target; a new discovery cannot reuse an old capability.
pub fn action_execution_candidate_id(
    action: &ActionDescriptor,
    context: &ResolvedContext,
) -> String {
    let id = action_candidate_id(&action.id, context);
    match action.executor.execution_token() {
        Some(token) => format!("{id}:discovery:{token}"),
        None => id,
    }
}

fn candidate_sort_key(candidate: &Candidate) -> (&str, &str) {
    (
        candidate
            .presentation
            .command_id
            .as_deref()
            .unwrap_or_else(|| candidate.presentation.bundle_id.as_deref().unwrap_or("")),
        candidate.presentation.id.as_str(),
    )
}

/// Highest score first; ties broken by a stable business key so the order never depends on
/// the luck of iteration.
pub fn compare_candidates(a: &Candidate, b: &Candidate) -> std::cmp::Ordering {
    b.presentation
        .score
        .total_cmp(&a.presentation.score)
        .then_with(|| {
            let (a_command, a_id) = candidate_sort_key(a);
            let (b_command, b_id) = candidate_sort_key(b);
            a_command.cmp(b_command).then_with(|| a_id.cmp(b_id))
        })
}

/// Empty action lists share one order across Current App and domain scopes.
/// Exact counts avoid the query scorer's cap and floating-point precision loss.
pub fn compare_empty_query_candidates(a: &Candidate, b: &Candidate) -> std::cmp::Ordering {
    let last_used = |usage: UsageStats| {
        if usage.execute_count == 0 {
            0
        } else {
            usage.last_executed_at
        }
    };
    b.usage
        .execute_count
        .cmp(&a.usage.execute_count)
        .then_with(|| last_used(b.usage).cmp(&last_used(a.usage)))
        .then_with(|| compare_candidates(a, b))
}

/// Keep one row per action and canonical object, preferring the highest-ranked occurrence.
/// Context root membership is unioned so ownership survives deduplication.
pub fn deduplicate_actions(candidates: &mut Vec<Candidate>) {
    // A tool's object offer is more specific than opening its empty workspace.
    let with_objects: std::collections::BTreeSet<_> = candidates
        .iter()
        .filter_map(|candidate| {
            (candidate
                .context()
                .is_some_and(|c| c.type_id != LAUNCHER_ENTRY_CONTEXT))
            .then(|| candidate.presentation.command_id.clone())
            .flatten()
        })
        .collect();
    candidates.retain(|candidate| {
        !(candidate
            .context()
            .is_some_and(|c| c.type_id == LAUNCHER_ENTRY_CONTEXT)
            && candidate
                .presentation
                .command_id
                .as_ref()
                .is_some_and(|id| with_objects.contains(id)))
    });
    let mut merged: Vec<Candidate> = Vec::with_capacity(candidates.len());
    let mut index: BTreeMap<(ActionId, ContextTypeId, Arc<str>), usize> = BTreeMap::new();
    for candidate in candidates.drain(..) {
        let Some(command) = candidate.presentation.command_id.clone() else {
            merged.push(candidate);
            continue;
        };
        let context = candidate
            .context()
            .map(|context| (context.type_id.clone(), context.value.shared_text()))
            .unwrap_or_else(|| (String::new(), Arc::<str>::from("")));
        let key = (command, context.0, context.1);
        match index.get(&key) {
            Some(position) => {
                let existing = &mut merged[*position];
                for key in &candidate.presentation.context_keys {
                    if !existing.presentation.context_keys.contains(key) {
                        existing.presentation.context_keys.push(key.clone());
                    }
                }
                if existing.presentation.owning_app.is_none() {
                    existing.presentation.owning_app = candidate.presentation.owning_app;
                }
            }
            None => {
                index.insert(key, merged.len());
                merged.push(candidate);
            }
        }
    }
    *candidates = merged;
}
