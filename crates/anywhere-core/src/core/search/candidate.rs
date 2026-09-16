//! Candidate construction: the shape of one row, and how rows are compared.
//!
//! Ordering lives here because it is a property of candidates; *deciding* the order is
//! ranking's job, and *finding* them is the sources' job.
use super::query::Query;
use crate::core::action::descriptor::ActionDescriptor;
use crate::core::context::ResolvedContext;
use anywhere_protocol::*;
use std::collections::BTreeMap;

/// A candidate and everything needed to present or execute it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub presentation: CandidatePresentation,
    pub action: Option<ActionDescriptor>,
    pub context: Option<ResolvedContext>,
    pub application: Option<crate::features::applications::index::IndexedApplication>,
}

/// Everything the search produced for this revision.
pub struct CandidateSet {
    pub candidates: Vec<Candidate>,
    pub scopes: Vec<ScopeEntry>,
    /// Usage total per dynamic scope, used to order the scope bar.
    pub scope_use: BTreeMap<GroupId, u32>,
    /// Action matching: applicability, availability and query match per context.
    pub matcher_us: u64,
    /// Application-index matching, reported separately so it cannot be mistaken for
    /// sorting cost.
    pub apps_us: u64,
    /// Scoring and ordering of the candidates that were matched.
    pub ranking_us: u64,
}

/// Stable business identity: survives re-ranking, renumbering and late enrichment.
pub fn action_candidate_id(command: &str, context: &ResolvedContext) -> String {
    format!(
        "action:{command}:{}:{}",
        context.type_id,
        context.value.text()
    )
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

/// Keep one row per action and canonical object, preferring the highest-ranked occurrence.
/// Scope membership is unioned so ownership survives deduplication.
pub fn deduplicate_actions(candidates: &mut Vec<Candidate>) {
    let mut merged: Vec<Candidate> = Vec::with_capacity(candidates.len());
    let mut index: BTreeMap<(ActionId, ContextTypeId, String), usize> = BTreeMap::new();
    for candidate in candidates.drain(..) {
        let Some(command) = candidate.presentation.command_id.clone() else {
            merged.push(candidate);
            continue;
        };
        let context = candidate
            .context
            .as_ref()
            .map(|context| (context.type_id.clone(), context.value.text()))
            .unwrap_or_default();
        let key = (command, context.0, context.1);
        match index.get(&key) {
            Some(position) => {
                let existing = &mut merged[*position];
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

/// The query as a candidate source sees it.
pub fn is_searching(query: &Query) -> bool {
    !query.is_empty()
}
