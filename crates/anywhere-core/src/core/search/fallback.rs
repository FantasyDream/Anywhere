//! Text actions offered as a last resort.
//!
//! The built-in text actions (a plugin's "search the web", for instance) stay reachable
//! when the query matched none of their own searchable fields. They are a fallback source,
//! appended after ranking so they can never displace a real query match or a typed-object
//! action.
use crate::core::action::availability;
use crate::core::action::descriptor::ActionRegistry;
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use crate::core::ranking::score;
use crate::core::search::candidate::{action_candidate_id, compare_candidates, Candidate};
use crate::core::search::candidate_source::{CandidateSource, SearchContext, SourceStage};
use crate::core::search::query::Query;
use crate::core::session::facts::SessionFacts;
use anywhere_protocol::*;

pub struct TextFallbackSource<'a> {
    pub registry: &'a ActionRegistry,
    pub facts: &'a SessionFacts,
}

impl CandidateSource for TextFallbackSource<'_> {
    fn id(&self) -> &'static str {
        "fallback"
    }

    fn stage(&self) -> SourceStage {
        SourceStage::Fallback
    }

    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        self.build(search.query, &[])
    }

    fn contribute(&self, search: &SearchContext<'_>, existing: &[Candidate]) -> Vec<Candidate> {
        if search.query.is_empty() {
            return Vec::new();
        }
        self.build(search.query, existing)
    }
}

impl TextFallbackSource<'_> {
    fn build(&self, query: &Query, existing: &[Candidate]) -> Vec<Candidate> {
        let context = ResolvedContext::new(
            TEXT,
            ContextValue::Text(query.raw_input.clone().into()),
            ContextSource::UserInput,
            "core.text_fallback",
        );
        let mut fallbacks = self
            .registry
            .matching(&context)
            .filter_map(|action| {
                let availability =
                    availability::evaluate(&action.availability, &context, self.facts);
                if !availability.is_visible() {
                    return None;
                }
                let level = action.search().matched(query);
                Some(Candidate {
                    presentation: CandidatePresentation {
                        shortcut: None,
                        id: action_candidate_id(&action.id, &context),
                        kind: CandidateKind::Action,
                        title: action.title.clone(),
                        subtitle: Some(context.display_detail()),
                        icon: IconDescriptor {
                            key: action.icon.clone(),
                        },
                        score: score::fallback_score(level, action.base_priority),
                        group_id: action.group_id.clone(),
                        availability,
                        command_id: Some(action.id.clone()),
                        bundle_id: None,
                        owning_app: None,
                        menu_path: action.menu_path.clone(),
                    },
                    target: crate::core::search::candidate::CandidateTarget::from_action(
                        action.clone(),
                        context.clone(),
                    ),
                })
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
