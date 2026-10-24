//! Launcher tool entries use the shared Action registry, independent of any domain.
use crate::core::{
    context::{ContextValue, ResolvedContext},
    search::{
        candidate::Candidate,
        candidate_source::{CandidateSource, SearchContext},
        engine::ActionSearch,
        query::Query,
    },
};
use anywhere_protocol::ContextSource;

pub use anywhere_protocol::LAUNCHER_ENTRY_CONTEXT as ENTRY_CONTEXT;

/// The Launcher capability is an execution target, not an ambient Context or Scope.
pub struct EntrySource<'a, 'b> {
    pub search: &'a ActionSearch<'b>,
}
impl EntrySource<'_, '_> {
    /// Shortcut execution must remain available independently of visible search.
    pub fn binding_offers(&self, contextual: &[Candidate]) -> Vec<Candidate> {
        let mut entries = self.offers(&Query::parse(""));
        // The same command already has a more specific captured target. This
        // follows visible search deduplication and avoids a false shortcut choice.
        entries.retain(|entry| {
            !contextual.iter().any(|offer| {
                offer.presentation.command_id == entry.presentation.command_id
                    && offer
                        .context()
                        .is_some_and(|context| context.type_id != ENTRY_CONTEXT)
            })
        });
        entries
    }

    fn offers(&self, query: &Query) -> Vec<Candidate> {
        let context = ResolvedContext::new(
            ENTRY_CONTEXT,
            ContextValue::Text("".into()),
            ContextSource::ForegroundApplication,
            "core.launcher",
        );
        self.search.search(query, &[context], &[]).candidates
    }
}
impl CandidateSource for EntrySource<'_, '_> {
    fn id(&self) -> &'static str {
        "workspace-actions"
    }
    fn candidates(&self, request: &SearchContext<'_>) -> Vec<Candidate> {
        if request.scope_id.is_some() || request.query.is_empty() {
            return Vec::new();
        }
        self.offers(request.query)
    }
}
