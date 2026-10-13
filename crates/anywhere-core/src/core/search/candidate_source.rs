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

/// Domain filters travel with the initiating revision, never through global policy.
#[derive(Clone)]
pub struct AsyncSearchRequest {
    pub text: String,
    pub options: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
}

#[derive(Clone, Copy)]
pub enum BatchUpdate {
    Replace,
    Append,
}

pub struct AsyncCandidateBatch {
    pub update: BatchUpdate,
    pub candidates: Vec<Candidate>,
    pub state: anywhere_protocol::SearchSourceState,
    pub metadata: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
}

impl AsyncSearchRequest {
    pub fn options<T: std::any::Any>(&self) -> Option<&T> {
        self.options.as_deref()?.downcast_ref()
    }
}
impl AsyncCandidateBatch {
    pub fn metadata<T: std::any::Any>(&self) -> Option<&T> {
        self.metadata.as_deref()?.downcast_ref()
    }
}

/// Slow sources run outside snapshot preparation. Their batches are published only
/// for the initiating revision, then enter the normal synchronous ranking pipeline.
#[async_trait::async_trait]
pub trait AsyncCandidateSource: std::any::Any + Send + Sync {
    fn id(&self) -> &'static str;
    fn accepts_mode(&self, mode: anywhere_protocol::InputMode) -> bool {
        matches!(
            mode,
            anywhere_protocol::InputMode::Environment
                | anywhere_protocol::InputMode::Search
                | anywhere_protocol::InputMode::CapturedEnvironment
        )
    }
    fn accepts(&self, query: &str) -> bool {
        !query.trim().is_empty()
    }
    async fn candidates(
        &self,
        query: &str,
        execution: crate::core::context::ExecutionContext,
    ) -> anyhow::Result<Vec<Candidate>>;
    fn debounce(&self) -> std::time::Duration {
        std::time::Duration::from_millis(150)
    }
    fn timeout(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(3))
    }
    async fn stream(
        &self,
        request: AsyncSearchRequest,
        execution: crate::core::context::ExecutionContext,
        sender: tokio::sync::mpsc::Sender<AsyncCandidateBatch>,
    ) -> anyhow::Result<()> {
        let candidates = self.candidates(&request.text, execution).await?;
        let _ = sender
            .send(AsyncCandidateBatch {
                update: BatchUpdate::Replace,
                candidates,
                state: anywhere_protocol::SearchSourceState::Ready,
                metadata: None,
            })
            .await;
        Ok(())
    }
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
