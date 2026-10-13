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
    pub ranking: SourceRanking<'a>,
}

/// All sources use the activation's immutable facts, history and clock.
#[derive(Clone, Copy)]
pub struct SourceRanking<'a> {
    pub usage: &'a dyn Fn(&str, &str) -> crate::core::ranking::score::UsageStats,
    pub facts: &'a crate::core::session::facts::SessionFacts,
    pub now: i64,
}
impl Default for SourceRanking<'_> {
    fn default() -> Self {
        static FACTS: std::sync::LazyLock<crate::core::session::facts::SessionFacts> =
            std::sync::LazyLock::new(Default::default);
        Self {
            usage: &|_, _| Default::default(),
            facts: &FACTS,
            now: 0,
        }
    }
}

/// When a source's rows enter the pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceStage {
    /// Matched against the query and contexts; participates in ranking.
    Ranked,
    /// Appended after ranking so it cannot displace a real match.
    Fallback,
}

pub type CandidateSourceRef = std::sync::Arc<dyn CandidateSource + Send + Sync>;

/// One contribution and one catalog lifetime for both synchronous and slow sources.
#[derive(Default, Clone)]
pub struct SourceSet {
    pub synchronous: Vec<CandidateSourceRef>,
    pub asynchronous: Vec<std::sync::Arc<dyn AsyncCandidateSource>>,
}
impl SourceSet {
    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.synchronous
            .iter()
            .map(|s| s.id())
            .chain(self.asynchronous.iter().map(|s| s.id()))
    }
    pub fn reset(&self) {
        for source in &self.asynchronous {
            source.reset();
        }
    }
}

/// Base services survive module changes; module contributions replace atomically.
#[derive(Default)]
pub struct SourceCatalog {
    pub base: SourceSet,
    pub extensions: SourceSet,
}
impl SourceCatalog {
    pub fn synchronous(&self) -> Vec<CandidateSourceRef> {
        self.base
            .synchronous
            .iter()
            .chain(&self.extensions.synchronous)
            .cloned()
            .collect()
    }
    pub fn asynchronous(&self) -> Vec<std::sync::Arc<dyn AsyncCandidateSource>> {
        self.base
            .asynchronous
            .iter()
            .chain(&self.extensions.asynchronous)
            .cloned()
            .collect()
    }
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
    /// Release ephemeral caches/subscriptions on catalog removal. Persistent data
    /// belongs to module state/storage; the source must remain reusable after reset.
    fn reset(&self) {}
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
