//! Action matching: which actions apply to the resolved contexts, and what they look like.
//!
//! Produces candidates only. Ordering belongs to ranking, scope membership to the scope
//! builder, and visibility to the availability evaluator.
use super::availability;
use super::descriptor::{ActionDescriptor, ActionRegistry};
use crate::core::context::{ContextSource, ResolvedContext};
use crate::core::ranking::score::{self, UsageStats};
use crate::core::search::candidate::{action_candidate_id, Candidate};
use crate::core::search::candidate_source::{CandidateSource, SearchContext};
use crate::core::search::query::Query;
use crate::core::session::facts::{context_ownership, Ownership, SessionFacts};
use anywhere_protocol::*;

pub struct ActionMatcher<'a> {
    pub registry: &'a ActionRegistry,
    pub usage: &'a dyn Fn(&str, &str) -> UsageStats,
    pub facts: &'a SessionFacts,
    pub now: i64,
}

/// Actions reach the search pipeline through the same contract as any other source; what is
/// core about them is only that the launcher always has them.
impl CandidateSource for ActionMatcher<'_> {
    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        self.match_actions(search.query, search.contexts)
    }
}

impl ActionMatcher<'_> {
    /// Every action the contexts make applicable, scored but not yet ordered.
    pub fn match_actions(&self, query: &Query, contexts: &[ResolvedContext]) -> Vec<Candidate> {
        let searching = !query.is_empty();
        let mut candidates = Vec::new();
        for context in contexts {
            // Ownership is per context, never carried over: an input-derived object
            // must not inherit the ambient application just because an ambient context
            // was visited earlier in this loop.
            let owned_by = match context_ownership(context, self.facts) {
                Ownership::App(app) => Some(app),
                Ownership::Ambient => None,
            };
            for action in self.registry.matching(context) {
                let match_level = action.search.matched(query);
                if searching && match_level.is_none() && !availability::is_user_object(context) {
                    continue;
                }
                let availability = availability::evaluate(action, context, self.facts);
                if !availability.is_visible() {
                    continue;
                }
                let stats = (self.usage)(&action.id, &context.type_id);
                let mut score = score::score(
                    match_level,
                    searching,
                    context.source,
                    &context.type_id,
                    action.base_priority,
                    stats,
                    self.now,
                );
                if !searching {
                    score += home_bonus(context);
                }
                candidates.push(Candidate {
                    presentation: CandidatePresentation {
                        id: action_candidate_id(&action.id, context),
                        kind: CandidateKind::Action,
                        title: action.title.clone(),
                        subtitle: Some(subtitle(action, context)),
                        icon: IconDescriptor {
                            key: action.icon.clone(),
                        },
                        score,
                        group_id: action.group_id.clone(),
                        availability,
                        command_id: Some(action.id.clone()),
                        bundle_id: None,
                        owning_app: owned_by.clone().or_else(|| action.owning_app.clone()),
                    },
                    action: Some(action.clone()),
                    context: Some(context.clone()),
                    application: None,
                });
            }
        }
        candidates
    }
}

/// Home-only weighting: the ambient object leads, so its actions come first.
///
/// There is deliberately no per-group term here: Git actions only ever accept a
/// `git.repository` context, so a rule keyed on a directory could never apply.
fn home_bonus(context: &ResolvedContext) -> f32 {
    match context.source {
        ContextSource::ApplicationContext | ContextSource::ForegroundApplication => 200.0,
        ContextSource::UserInput => 0.0,
    }
}

fn subtitle(action: &ActionDescriptor, context: &ResolvedContext) -> String {
    let detail = display_detail(context);
    match &action.subtitle {
        Some(subtitle) => format!("{subtitle} · {detail}"),
        None => detail,
    }
}

/// Short subtitle text; execution still uses the full transport value.
fn display_detail(context: &ResolvedContext) -> String {
    const MAX: usize = 120;
    let text = context.value.text();
    if text.chars().count() <= MAX {
        return text;
    }
    let mut shortened = text.chars().take(MAX).collect::<String>();
    shortened.push('…');
    shortened
}
