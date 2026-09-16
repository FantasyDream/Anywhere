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

pub trait CandidateSource {
    /// Candidates this source contributes for the given query and contexts.
    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate>;
}
