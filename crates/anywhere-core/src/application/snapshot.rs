//! Snapshot contents: search, diagnostics, and the context summary.
//!
//! Called with data already copied out of the session lock; publication revalidates it.
use crate::core::action::descriptor::ActionRegistry;
use crate::core::context::ResolvedContext;
use crate::core::ranking::usage::{lookup_usage, UsageSnapshot};
use crate::core::search::candidate::{Candidate, CandidateSet};
use crate::core::search::candidate_source::CandidateSource;
use crate::core::search::engine::ActionSearch;
use crate::core::search::query::Query;
use crate::core::session::facts::SessionFacts;
use crate::features::applications::index::ApplicationCatalog;
use anywhere_protocol::*;
use std::sync::Arc;

pub(crate) mod cache;

pub struct PreparedSnapshot {
    pub set: CandidateSet,
    pub diagnostics: Vec<String>,
    pub summary: Vec<ContextPresentation>,
    pub context_candidates: Vec<crate::core::search::candidate::Candidate>,
    pub result_actions: Vec<crate::core::search::candidate::Candidate>,
    pub host_actions: Vec<crate::core::search::candidate::Candidate>,
    pub preferred_context_key: Option<ContextKey>,
    pub scopes: Vec<ScopeEntry>,
    pub shortcut_catalog: Vec<super::shortcuts::ShortcutAction>,
}

#[derive(Clone, Copy)]
pub struct SnapshotInput<'a> {
    pub action_catalog_version: u64,
    pub input: &'a str,
    pub input_mode: InputMode,
    pub selected_scope: Option<&'a str>,
    pub facts: &'a SessionFacts,
    pub usage: &'a Arc<UsageSnapshot>,
    pub app_usage: &'a UsageSnapshot,
    pub actions: &'a Arc<ActionRegistry>,
    pub contexts: &'a [ResolvedContext],
    pub incoming_diagnostics: &'a [String],
    pub prior_diagnostics: &'a [String],
    pub catalog: &'a ApplicationCatalog,
    pub sources: &'a [Arc<dyn CandidateSource + Send + Sync>],
    pub search_candidates: &'a [crate::core::search::candidate::Candidate],
    pub object_actions: &'a [crate::core::search::candidate::Candidate],
    pub now: i64,
}

pub fn prepare(
    input: SnapshotInput<'_>,
    cache: &std::sync::Mutex<Option<cache::OfferCache>>,
) -> PreparedSnapshot {
    let usage_of = |command: &str, kind: &str| lookup_usage(input.usage, command, kind);
    let offers = cache::prepare(&input, cache, &usage_of);
    let SnapshotInput {
        action_catalog_version,
        input,
        input_mode,
        selected_scope,
        facts,
        usage,
        app_usage,
        actions: base_actions,
        contexts,
        incoming_diagnostics,
        prior_diagnostics,
        catalog,
        sources,
        search_candidates,
        object_actions,
        now,
    } = input;
    let action_diagnostics = offers.diagnostics;
    let actions = &offers.actions;
    let app_usage_of = |command: &str, kind: &str| lookup_usage(app_usage, command, kind);
    let search = ActionSearch::new(
        actions,
        &usage_of,
        facts,
        now,
        crate::features::context_semantic,
    );
    let application_source = crate::features::applications::source::ApplicationSource::from_catalog(
        catalog,
        &app_usage_of,
        now,
    );
    let mut extra: Vec<&dyn CandidateSource> = vec![&application_source];
    // Cached objects cannot authorize an old or revoked action implementation.
    let mut ready_candidates = search_candidates.to_vec();
    validate_ready_candidates(&mut ready_candidates, actions, facts);
    let ready_source = crate::core::search::candidate_source::ReadySource(&ready_candidates);
    extra.push(&ready_source);
    let workspace = crate::features::tools::EntrySource { search: &search };
    if input_mode != InputMode::Content {
        extra.push(&workspace);
    }
    extra.extend(
        sources
            .iter()
            .map(|source| source.as_ref() as &dyn CandidateSource),
    );
    let query = Query::parse(if input_mode == InputMode::Content {
        ""
    } else {
        input
    });
    let mut set = if input_mode == InputMode::FileSearch {
        CandidateSet {
            candidates: std::mem::take(&mut ready_candidates),
            matcher_us: 0,
            sources_us: 0,
            ranking_us: 0,
        }
    } else if query.is_empty() {
        search.search_cached_empty(contexts, &extra, selected_scope, &offers.candidates)
    } else {
        search.search_scoped(&query, contexts, &extra, selected_scope)
    };
    let mut diagnostics = prior_diagnostics.to_vec();
    for diagnostic in action_diagnostics {
        if !diagnostics.contains(&diagnostic) {
            diagnostics.push(diagnostic);
        }
    }
    for diagnostic in incoming_diagnostics {
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
    // Menu browsing and scope generation retain full offers even with a query.
    let offers_candidate_count = offers.candidates.len();
    let mut context_candidates = if input.trim().is_empty() && selected_scope.is_none() {
        set.candidates
            .iter()
            .filter(|candidate| {
                candidate.presentation.kind == CandidateKind::Action
                    && candidate
                        .context()
                        .is_some_and(|c| c.type_id != crate::features::tools::ENTRY_CONTEXT)
            })
            .cloned()
            .collect()
    } else {
        offers.candidates
    };
    let mut host_actions = workspace.binding_offers(&context_candidates);
    let mut result_actions =
        revalidate_object_actions(object_actions, actions, base_actions, facts);
    // A definition reload must not silently substitute a same-ID implementation for
    // a request selected from an older visible snapshot. Query re-ranking does not
    // advance this version; persistent bindings use ActionId instead of this row ID.
    stamp_candidate_ids(
        set.candidates
            .iter_mut()
            .chain(&mut context_candidates)
            .chain(&mut result_actions)
            .chain(&mut host_actions),
        action_catalog_version,
    );
    let scopes = if input.trim().is_empty()
        && selected_scope.is_none()
        && context_candidates.len() != offers_candidate_count
    {
        crate::core::search::scope::build(
            &context_candidates,
            actions,
            usage,
            facts.frontmost.as_ref(),
            now,
        )
    } else {
        offers.scopes
    };
    let summary = crate::application::context_projection::build(contexts, &context_candidates);
    let preferred_context_key = crate::application::context_projection::preferred(&summary);
    let shortcut_catalog = offers.shortcuts;
    PreparedSnapshot {
        shortcut_catalog,
        set,
        diagnostics,
        summary,
        context_candidates,
        preferred_context_key,
        result_actions,
        host_actions,
        scopes,
    }
}

fn revalidate_object_actions(
    object_actions: &[crate::core::search::candidate::Candidate],
    actions: &ActionRegistry,
    base_actions: &ActionRegistry,
    facts: &SessionFacts,
) -> Vec<crate::core::search::candidate::Candidate> {
    let object_registry = if object_actions.iter().any(|candidate| {
        matches!(
            &candidate.target,
            crate::core::search::candidate::CandidateTarget::Action { action, .. }
                if actions.get_shared(&action.id).is_none()
        )
    }) {
        let mut object_contexts = Vec::new();
        for candidate in object_actions {
            if let Some(context) = candidate.context() {
                if !object_contexts.contains(context) {
                    object_contexts.push(context.clone());
                }
            }
        }
        Some(base_actions.for_contexts(&object_contexts).0)
    } else {
        None
    };
    object_actions
        .iter()
        .filter_map(|candidate| {
            let mut candidate = candidate.clone();
            if let crate::core::search::candidate::CandidateTarget::Action { action, context } =
                &mut candidate.target
            {
                let current = actions.get_shared(&action.id).or_else(|| {
                    object_registry
                        .as_ref()
                        .and_then(|registry| registry.get_shared(&action.id))
                })?;
                if !current.matches(context) {
                    return None;
                }
                candidate.presentation.availability = crate::core::action::availability::evaluate(
                    &current.availability,
                    context,
                    facts,
                );
                *action = current;
            }
            candidate
                .presentation
                .availability
                .is_visible()
                .then_some(candidate)
        })
        .collect()
}

pub(super) fn stamp_candidate_ids<'a>(
    candidates: impl IntoIterator<Item = &'a mut Candidate>,
    version: u64,
) {
    if version <= 1 {
        return;
    }
    let suffix = format!(":catalog:{version}");
    for candidate in candidates {
        if candidate.context().is_some() {
            candidate.presentation.id.push_str(&suffix);
        }
    }
}

pub(super) fn validate_ready_candidates(
    candidates: &mut Vec<crate::core::search::candidate::Candidate>,
    actions: &ActionRegistry,
    facts: &SessionFacts,
) {
    candidates.retain_mut(|candidate| {
        if let crate::core::search::candidate::CandidateTarget::Action { action, context } =
            &mut candidate.target
        {
            let Some(current) = actions.get_shared(&action.id) else {
                return false;
            };
            if !current.matches(context) {
                return false;
            }
            candidate.presentation.availability =
                crate::core::action::availability::evaluate(&current.availability, context, facts);
            *action = current;
            return candidate.presentation.availability.is_visible();
        }
        true
    });
}
