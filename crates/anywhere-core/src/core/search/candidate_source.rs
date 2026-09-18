//! A source of search candidates.
//!
//! Everything the user can pick from arrives through this trait, so the search engine has
//! no per-feature branches: applications, actions, and later files, clipboard entries or
//! history all look the same to it.
use crate::core::context::ResolvedContext;
use crate::core::search::candidate::Candidate;
use crate::core::search::query::Query;

/// What a source is allowed to see.
pub struct SearchContext<'a> {
    pub query: &'a Query,
    pub contexts: &'a [ResolvedContext],
}

/// When a source's rows enter the pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceStage {
    /// Matched against the query and contexts; participates in ranking.
    Ranked,
    /// Appended after ranking so it cannot displace a real match.
    Fallback,
}

pub trait CandidateSource {
    fn stage(&self) -> SourceStage {
        SourceStage::Ranked
    }

    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate>;

    /// Fallback sources receive the ranked list so they can skip duplicates.
    fn contribute(&self, search: &SearchContext<'_>, existing: &[Candidate]) -> Vec<Candidate> {
        let _ = existing;
        self.candidates(search)
    }
}
