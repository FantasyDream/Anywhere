//! Text actions offered as a last resort.
//!
//! The built-in text actions (a plugin's "search the web", for instance) stay reachable
//! when the query matched none of their own searchable fields. They are appended after
//! sorting so they can never displace a real query match or a typed-object action.
use crate::core::action::descriptor::ActionRegistry;
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use crate::core::ranking::score;
use crate::core::search::candidate::{action_candidate_id, compare_candidates, Candidate};
use crate::core::search::query::Query;
use anywhere_protocol::*;

pub struct TextFallbackSource<'a> {
    pub registry: &'a ActionRegistry,
}

impl TextFallbackSource<'_> {
    pub fn candidates(&self, query: &Query, existing: &[Candidate]) -> Vec<Candidate> {
        let context = ResolvedContext::new(
            TEXT,
            ContextValue::Text(query.raw_input.clone()),
            ContextSource::UserInput,
            "core.text_fallback",
        );
        let mut fallbacks = self
            .registry
            .matching(&context)
            .map(|action| {
                let level = action.search.matched(query);
                Candidate {
                    presentation: CandidatePresentation {
                        id: action_candidate_id(&action.id, &context),
                        kind: CandidateKind::Action,
                        title: action.title.clone(),
                        subtitle: Some(display_detail(&context)),
                        icon: IconDescriptor {
                            key: action.icon.clone(),
                        },
                        score: score::fallback_score(level, action.base_priority),
                        group_id: action.group_id.clone(),
                        availability: Availability::enabled(),
                        command_id: Some(action.id.clone()),
                        bundle_id: None,
                        owning_app: None,
                    },
                    action: Some(action.clone()),
                    context: Some(context.clone()),
                    application: None,
                }
            })
            .filter(|candidate| {
                !existing.iter().any(|existing| {
                    existing.presentation.command_id == candidate.presentation.command_id
                })
            })
            .collect::<Vec<_>>();
        fallbacks.sort_by(compare_candidates);
        fallbacks
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
