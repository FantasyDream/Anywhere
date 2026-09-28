//! Action matching: which actions apply to the resolved contexts, and what they look like.
//!
//! Produces candidates only. Ordering belongs to ranking, scope membership to the scope
//! builder, and visibility to the availability evaluator.
use super::availability;
use super::descriptor::{ActionDescriptor, ActionRegistry};
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use crate::core::ranking::score::{self, UsageStats};
use crate::core::search::candidate::{action_candidate_id, Candidate};
use crate::core::search::candidate_source::{CandidateSource, SearchContext};
use crate::core::search::query::Query;
use crate::core::session::facts::{context_ownership, Ownership, SessionFacts};
use anywhere_protocol::*;

pub struct ActionMatcher<'a> {
    registry: &'a ActionRegistry,
    usage: &'a dyn Fn(&str, &str) -> UsageStats,
    facts: &'a SessionFacts,
    now: i64,
    semantic: fn(&str) -> i32,
}

impl<'a> ActionMatcher<'a> {
    pub fn new(
        registry: &'a ActionRegistry,
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
        facts: &'a SessionFacts,
        now: i64,
        semantic: fn(&str) -> i32,
    ) -> Self {
        Self {
            registry,
            usage,
            facts,
            now,
            semantic,
        }
    }

    pub fn registry(&self) -> &'a ActionRegistry {
        self.registry
    }
}

/// Actions reach the search pipeline through the same contract as any other source; what is
/// core about them is only that the launcher always has them.
impl CandidateSource for ActionMatcher<'_> {
    fn id(&self) -> &'static str {
        "actions"
    }

    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        self.match_actions(search.query, search.contexts)
    }
}

impl ActionMatcher<'_> {
    /// Every action the contexts make applicable, scored but not yet ordered.
    pub fn match_actions(&self, query: &Query, contexts: &[ResolvedContext]) -> Vec<Candidate> {
        let searching = !query.is_empty();
        let grouped_finder_selection = contexts.iter().any(|context| {
            context.source == ContextSource::FinderSelection
                && matches!(&context.value, ContextValue::FileSelection(paths) if paths.len() > 1)
        });
        let mut candidates = Vec::new();
        for context in contexts {
            // The collection is the user's object. Expanding selected children for
            // providers must not create an Open/Reveal row for every selected file.
            if grouped_finder_selection
                && context.source == ContextSource::FinderSelection
                && matches!(context.type_id.as_str(), FILE | DIRECTORY)
            {
                continue;
            }
            // Ownership is per context, never carried over: an input-derived object
            // must not inherit the ambient application just because an ambient context
            // was visited earlier in this loop.
            let owned_by = match context_ownership(context, self.facts) {
                Ownership::App(app) => Some(app),
                Ownership::Ambient => None,
            };
            for action in self.registry.matching(context) {
                let match_level = action.search().matched(query);
                if searching && match_level.is_none() && !availability::is_user_object(context) {
                    continue;
                }
                let availability =
                    availability::evaluate(&action.availability, context, self.facts);
                if !availability.is_visible() {
                    continue;
                }
                let stats = (self.usage)(&action.id, &context.type_id);
                let mut score = score::score(
                    match_level,
                    searching,
                    context.source,
                    (self.semantic)(&context.type_id),
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
                        shortcut: action.shortcut.clone(),
                        icon: IconDescriptor {
                            key: action.icon.clone(),
                        },
                        score,
                        group_id: action.group_id.clone(),
                        availability,
                        command_id: Some(action.id.clone()),
                        bundle_id: None,
                        owning_app: owned_by.clone().or_else(|| action.owning_app.clone()),
                        menu_path: action.menu_path.clone(),
                    },
                    target: crate::core::search::candidate::CandidateTarget::from_action(
                        action.clone(),
                        context.clone(),
                    ),
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
        ContextSource::FinderSelection => 240.0,
        ContextSource::SelectedText => 220.0,
        ContextSource::BrowserCurrentPage => 210.0,
        ContextSource::ApplicationContext | ContextSource::ForegroundApplication => 200.0,
        ContextSource::Clipboard => 80.0,
        ContextSource::UserInput => 0.0,
    }
}

fn subtitle(action: &ActionDescriptor, context: &ResolvedContext) -> String {
    let detail = context.display_detail();
    match &action.subtitle {
        Some(subtitle) => format!("{subtitle} · {detail}"),
        None => detail,
    }
}
