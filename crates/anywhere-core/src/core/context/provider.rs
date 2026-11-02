//! The context provider contract.
//!
//! A provider turns one context into further contexts — `core.text` into `core.url`, a
//! directory into a `git.repository`. It never knows about budgets, timeouts or the user
//! interface: the caller decides how long to wait and what to do with the result.
use crate::core::context::{ExecutionContext, ResolvedContext};
use anyhow::Result;
use anywhere_protocol::{ContextTypeId, ProviderId};
use async_trait::async_trait;

/// Tracing sites use literal names; MVP tests verify they match this contract.
/// `context_resolution_total` is an event name, while `first_snapshot_duration`
/// maps to `first_snapshot_duration_us` on the first committed snapshot of a revision.
pub mod metrics {
    pub const PROVIDER_DURATION: &str = "provider_duration";
    pub const CONTEXT_RESOLUTION_TOTAL: &str = "context_resolution_total";
    pub const FIRST_SNAPSHOT_DURATION: &str = "first_snapshot_duration";
    pub const MATCHER_DURATION: &str = "matcher_duration";
    pub const RANKING_DURATION: &str = "ranking_duration";
    pub const HOTKEY_TO_VISIBLE: &str = "hotkey_to_visible_ms";
    pub const LATE_CONTEXT_COUNT: &str = "late_context_count";

    pub const ALL: [&str; 7] = [
        PROVIDER_DURATION,
        CONTEXT_RESOLUTION_TOTAL,
        FIRST_SNAPSHOT_DURATION,
        MATCHER_DURATION,
        RANKING_DURATION,
        HOTKEY_TO_VISIBLE,
        LATE_CONTEXT_COUNT,
    ];
}

/// What a provider declares about itself.
#[derive(Debug, Clone)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub accepts: Vec<ContextTypeId>,
    pub produces: Vec<ContextTypeId>,
    pub priority: i32,
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;
    /// Resolve `input` into further contexts.
    ///
    /// Called at most once per `(provider, input, session, revision)`: a caller that stops
    /// waiting keeps the started call and resumes it, so a provider never has to defend
    /// against being re-entered for work it has already begun.
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>>;
}

pub enum StepOutcome {
    Completed {
        result: Result<Vec<ResolvedContext>>,
        duration_us: u64,
    },
    /// The caller superseded this run before polling or while waiting for the provider.
    Abandoned,
}
