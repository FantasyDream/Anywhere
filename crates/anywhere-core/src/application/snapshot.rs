//! Snapshot contents: search, diagnostics, and the context summary.
//!
//! Called with data already copied out of the session lock; publication revalidates it.
use crate::core::action::descriptor::ActionRegistry;
use crate::core::context::ResolvedContext;
use crate::core::ranking::score::UsageStats;
use crate::core::search::candidate::CandidateSet;
use crate::core::search::candidate_source::CandidateSource;
use crate::core::search::engine::ActionSearch;
use crate::core::search::query::Query;
use crate::core::session::facts::SessionFacts;
use crate::features::applications::index::ApplicationCatalog;
use anywhere_protocol::*;
use std::{collections::HashMap, sync::Arc};

pub struct PreparedSnapshot {
    pub set: CandidateSet,
    pub diagnostics: Vec<String>,
    pub summary: Vec<ContextPresentation>,
    pub context_candidates: Vec<crate::core::search::candidate::Candidate>,
    pub preferred_context_key: Option<ContextKey>,
    pub scopes: Vec<ScopeEntry>,
}

pub struct SnapshotInput<'a> {
    pub input: &'a str,
    pub selected_scope: Option<&'a str>,
    pub facts: &'a SessionFacts,
    pub usage: &'a HashMap<(String, String), UsageStats>,
    pub app_usage: &'a HashMap<(String, String), UsageStats>,
    pub actions: &'a ActionRegistry,
    pub contexts: &'a [ResolvedContext],
    pub incoming_diagnostics: &'a [String],
    pub prior_diagnostics: &'a [String],
    pub catalog: &'a ApplicationCatalog,
    pub sources: &'a [Arc<dyn CandidateSource + Send + Sync>],
    pub now: i64,
}

pub fn prepare(input: SnapshotInput<'_>) -> PreparedSnapshot {
    let SnapshotInput {
        input,
        selected_scope,
        facts,
        usage,
        app_usage,
        actions,
        contexts,
        incoming_diagnostics,
        prior_diagnostics,
        catalog,
        sources,
        now,
    } = input;
    let usage_of = |command: &str, kind: &str| {
        usage
            .get(&(command.to_owned(), kind.to_owned()))
            .copied()
            .unwrap_or_default()
    };
    let app_usage_of = |command: &str, kind: &str| {
        app_usage
            .get(&(command.to_owned(), kind.to_owned()))
            .copied()
            .unwrap_or_default()
    };
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
    extra.extend(
        sources
            .iter()
            .map(|source| source.as_ref() as &dyn CandidateSource),
    );
    let mut set = search.search_scoped(&Query::parse(input), contexts, &extra, selected_scope);
    let mut diagnostics = prior_diagnostics.to_vec();
    for diagnostic in incoming_diagnostics {
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
    // Menu browsing and scope generation retain full offers even with a query.
    let context_candidates = if input.trim().is_empty() && selected_scope.is_none() {
        set.candidates
            .iter()
            .filter(|candidate| candidate.context().is_some())
            .cloned()
            .collect()
    } else {
        let offers = search.search(&Query::parse(""), contexts, &[]);
        set.matcher_us += offers.matcher_us;
        set.ranking_us += offers.ranking_us;
        offers.candidates
    };
    let scopes = crate::core::search::scope::build(
        &context_candidates,
        actions,
        usage,
        facts.frontmost.as_ref(),
        now,
    );
    let summary = crate::core::context::navigation::build(contexts, &context_candidates);
    let preferred_context_key = crate::core::context::navigation::preferred(&summary);
    PreparedSnapshot {
        set,
        diagnostics,
        summary,
        context_candidates,
        preferred_context_key,
        scopes,
    }
}
