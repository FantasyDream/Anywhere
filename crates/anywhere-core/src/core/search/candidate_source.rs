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
    pub scope_id: Option<&'a str>,
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
    fn id(&self) -> &'static str {
        "source"
    }

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

/// Slow sources run outside snapshot preparation. Their batches are published only
/// for the initiating revision, then enter the normal synchronous ranking pipeline.
#[async_trait::async_trait]
pub trait AsyncCandidateSource: Send + Sync {
    fn id(&self) -> &'static str;
    fn accepts(&self, query: &str) -> bool {
        !query.trim().is_empty()
    }
    async fn candidates(
        &self,
        query: &str,
        execution: crate::core::context::ExecutionContext,
    ) -> anyhow::Result<Vec<Candidate>>;
}

pub struct ReadySource<'a>(pub &'a [Candidate]);
impl CandidateSource for ReadySource<'_> {
    fn id(&self) -> &'static str {
        "async-results"
    }
    fn candidates(&self, _: &SearchContext<'_>) -> Vec<Candidate> {
        self.0.to_vec()
    }
}
